//! ============================================================================
//! src/services/dictation/controller.rs — Dictation Session Controller
//! ============================================================================

use crate::core::error::DictationError;
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::ptt::{discard_ptt_hold_inner, reset_ptt_state_inner};
use crate::services::stt::SttCommand;
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct DictationController;

impl DictationController {
    /// Triggered when the global dictation hotkey is pressed.
    pub async fn handle_press(app: &AppHandle) -> Result<(), DictationError> {
        let state: State<'_, std::sync::Arc<AppState>> = app.state();

        // 1. Ensure audio/STT engine is active (lazy launch on-demand if cold)
        {
            let engine_lock = state.engine.lock().await;
            if engine_lock.is_none() {
                log::info!(
                    "[Dictation::Controller] Engine is cold. Auto-launching audio/STT engine..."
                );
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
        }

        // 2. Set active owner to Dictation
        let owner = InteractionOwner::Dictation;
        state.owner.store(owner as u32, Ordering::Relaxed);
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(owner));
        }

        // 3. Atomic compare-exchange to prevent double-press racing
        if state
            .ptt
            .is_recording
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            log::warn!("[Dictation::Controller] Dictation press received while already recording. Ignoring.");
            return Ok(());
        }

        // 4. Allocate new turn ID and reset PTT buffer
        let turn = {
            let new_turn = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
            state.ptt.turn_id.store(new_turn, Ordering::Relaxed);
            reset_ptt_state_inner(&state.ptt);
            new_turn
        };

        // 5. Notify pipeline of SpeechStart for potential playback cancellation
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine.pipeline_tx.send(VoxEvent::SpeechStart {
                turn_id: turn,
                owner,
            });
        }

        log::info!(
            "[Dictation::Controller] 🎙️ Dictation recording started (turn: {})",
            turn
        );

        // 6. Broadcast PTT status & state transition
        let _ = app.emit(
            "ptt_status",
            json!({ "state": "RECORDING", "session_id": turn, "owner": "dictation" }),
        );
        state
            .pipeline
            .update_interaction_state(InteractionState::UserSpeaking, owner, app);

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

        // If no speech was detected during the hold, discard cleanly
        if !speech_detected {
            log::info!("[Dictation::Controller] Silence only detected. Discarding dictation hold.");
            discard_ptt_hold_inner(&state.ptt);

            let _ = app.emit(
                "ptt_status",
                json!({ "state": "IDLE", "owner": "dictation" }),
            );
            state
                .pipeline
                .update_interaction_state(InteractionState::Idle, owner, app);
            return Ok(());
        }

        // Extract audio buffer and mark recording as stopped
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

        let _ = app.emit(
            "ptt_status",
            json!({ "state": "PROCESSING", "session_id": turn, "owner": "dictation" }),
        );
        state
            .pipeline
            .update_interaction_state(InteractionState::Thinking, owner, app);

        // Send captured buffer to STT worker for final transcription
        let engine_lock = state.engine.lock().await;
        if let Some(engine) = engine_lock.as_ref() {
            let _ = engine
                .stt_tx
                .send(SttCommand::Final(turn, owner, buffer_clone));
        } else {
            log::error!(
                "[Dictation::Controller] Engine not running during dictation finalization."
            );
            return Err(DictationError::EngineNotReady {
                message: "Engine not available for transcription".into(),
            });
        }

        Ok(())
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

        let _ = app.emit(
            "ptt_status",
            json!({ "state": "IDLE", "owner": "dictation" }),
        );
        state
            .pipeline
            .update_interaction_state(InteractionState::Idle, owner, app);

        Ok(())
    }
}
