use crate::core::error::DictationError;
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::ptt::{discard_ptt_hold_inner, reset_ptt_state_inner};
use crate::services::stt::SttCommand;
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

/// Session controller managing global dictation press, release, and cancel lifecycles.
pub struct DictationController;

impl DictationController {
    /// Triggered when the global dictation hotkey is pressed.
    pub async fn handle_press(app: &AppHandle) -> Result<(), DictationError> {
        let state: State<'_, std::sync::Arc<AppState>> = app.state();

        ensure_engine_running(app, &state).await?;

        let owner = InteractionOwner::Dictation;
        state.owner.store(owner as u32, Ordering::Relaxed);
        if let Some(engine) = state.engine.lock().await.as_ref() {
            if let Err(e) = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(owner))
            {
                log::warn!(
                    "[Dictation::Controller] Failed to notify VAD of owner switch: {}",
                    e
                );
            }
        }

        if state
            .ptt
            .is_recording
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            log::warn!("[Dictation::Controller] Dictation press received while already recording. Ignoring.");
            return Ok(());
        }

        let turn = begin_dictation_turn(app, &state, owner).await;
        log::info!(
            "[Dictation::Controller] 🎙️ Dictation recording started (turn: {})",
            turn
        );

        Ok(())
    }

    /// Triggered when the global dictation hotkey is released.
    pub async fn handle_release(app: &AppHandle) -> Result<(), DictationError> {
        let state: State<'_, std::sync::Arc<AppState>> = app.state();

        if !state.ptt.is_recording.load(Ordering::SeqCst) {
            return Ok(());
        }

        let owner = InteractionOwner::Dictation;
        let speech_detected = state.ptt.speech_detected.load(Ordering::Relaxed);

        if !speech_detected {
            handle_silent_dictation_release(app, &state, owner);
            return Ok(());
        }

        let (turn, buffer_clone) = {
            let buffer = state.ptt.audio_buffer.lock();
            let turn = state.ptt.turn_id.load(Ordering::Relaxed);
            state.ptt.is_recording.store(false, Ordering::SeqCst);
            (turn, buffer.clone())
        };

        log::info!(
            "[Dictation::Controller] ⏹️ Dictation recording stopped. Finalizing {} samples (turn: {})...",
            buffer_clone.len(),
            turn
        );

        finalize_dictation_audio(app, &state, turn, owner, buffer_clone).await
    }

    /// Triggered to cancel an active dictation session.
    pub async fn handle_cancel(app: &AppHandle) -> Result<(), DictationError> {
        let state: State<'_, std::sync::Arc<AppState>> = app.state();

        state.ptt.is_recording.store(false, Ordering::SeqCst);
        state.ptt.audio_buffer.lock().clear();
        state.ptt.speech_detected.store(false, Ordering::Relaxed);
        state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

        let owner = InteractionOwner::Dictation;
        log::info!("[Dictation::Controller] ❌ Dictation recording cancelled.");

        if let Err(e) = app.emit(
            "ptt_status",
            json!({ "state": "IDLE", "owner": "dictation" }),
        ) {
            log::warn!(
                "[Dictation::Controller] Failed to emit ptt_status on cancel: {}",
                e
            );
        }

        state
            .pipeline
            .update_interaction_state(InteractionState::Idle, owner, app);

        Ok(())
    }
}

/// Ensures the audio engine is running, lazy-launching it if not initialized.
async fn ensure_engine_running(app: &AppHandle, state: &AppState) -> Result<(), DictationError> {
    let engine_lock = state.engine.lock().await;
    if engine_lock.is_none() {
        log::info!("[Dictation::Controller] Engine is cold. Auto-launching audio/STT engine...");
        drop(engine_lock);
        if let Err(e) = crate::ipc::pipeline::launch_engine(app.clone()).await {
            log::error!(
                "[Dictation::Controller] Failed to lazy-launch engine: {}",
                e
            );
            return Err(DictationError::EngineNotReady {
                message: format!("Failed to initialize audio engine: {}", e),
            });
        }
    }
    Ok(())
}

/// Allocates a new turn ID, resets PTT state, and broadcasts SpeechStart and recording events.
async fn begin_dictation_turn(app: &AppHandle, state: &AppState, owner: InteractionOwner) -> u32 {
    let new_turn = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
    state.ptt.turn_id.store(new_turn, Ordering::Relaxed);
    reset_ptt_state_inner(&state.ptt);

    if let Some(engine) = state.engine.lock().await.as_ref() {
        if let Err(e) = engine.pipeline_tx.send(VoxEvent::SpeechStart {
            turn_id: new_turn,
            owner,
        }) {
            log::warn!(
                "[Dictation::Controller] Failed to send SpeechStart to pipeline: {}",
                e
            );
        }
    }

    if let Err(e) = app.emit(
        "ptt_status",
        json!({ "state": "RECORDING", "session_id": new_turn, "owner": "dictation" }),
    ) {
        log::warn!(
            "[Dictation::Controller] Failed to emit ptt_status RECORDING: {}",
            e
        );
    }

    state
        .pipeline
        .update_interaction_state(InteractionState::UserSpeaking, owner, app);

    new_turn
}

/// Cleans up state and broadcasts Idle transition when no speech was captured during the hold.
fn handle_silent_dictation_release(app: &AppHandle, state: &AppState, owner: InteractionOwner) {
    log::info!("[Dictation::Controller] Silence only detected. Discarding dictation hold.");
    discard_ptt_hold_inner(&state.ptt);

    if let Err(e) = app.emit(
        "ptt_status",
        json!({ "state": "IDLE", "owner": "dictation" }),
    ) {
        log::warn!(
            "[Dictation::Controller] Failed to emit ptt_status IDLE: {}",
            e
        );
    }

    state
        .pipeline
        .update_interaction_state(InteractionState::Idle, owner, app);
}

/// Emits processing state and forwards captured audio to STT worker.
async fn finalize_dictation_audio(
    app: &AppHandle,
    state: &AppState,
    turn: u32,
    owner: InteractionOwner,
    buffer: Vec<f32>,
) -> Result<(), DictationError> {
    if let Err(e) = app.emit(
        "ptt_status",
        json!({ "state": "PROCESSING", "session_id": turn, "owner": "dictation" }),
    ) {
        log::warn!(
            "[Dictation::Controller] Failed to emit ptt_status PROCESSING: {}",
            e
        );
    }

    state
        .pipeline
        .update_interaction_state(InteractionState::Thinking, owner, app);

    let engine_lock = state.engine.lock().await;
    if let Some(engine) = engine_lock.as_ref() {
        if let Err(e) = engine.stt_tx.send(SttCommand::Final(turn, owner, buffer)) {
            log::error!(
                "[Dictation::Controller] Failed to dispatch Final STT command: {}",
                e
            );
            return Err(DictationError::EngineNotReady {
                message: format!("Failed to dispatch STT finalization: {}", e),
            });
        }
    } else {
        log::error!("[Dictation::Controller] Engine not running during dictation finalization.");
        return Err(DictationError::EngineNotReady {
            message: "Engine not available for transcription".into(),
        });
    }

    Ok(())
}
