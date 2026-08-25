use super::{transition, RoutingContext};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::audio::PlaybackEngine;
use crate::services::llm::types::{
    ConversationInput, GenerationOptions, GenerationPurpose, GenerationRequest, OutputConstraint,
};
use crate::services::llm::LlmCommand;
use crate::services::tts::actor::TtsClauseChunker;
use crate::services::tts::TtsCommand;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static SPEECH_DETECTED: AtomicBool = AtomicBool::new(false);
static PTT_BUFFER: Mutex<Vec<f32>> = Mutex::new(Vec::new());
static CHUNKER: LazyLock<Mutex<TtsClauseChunker>> =
    LazyLock::new(|| Mutex::new(TtsClauseChunker::new()));

/// Initializes and warms up the LLM and TTS actor threads if not already loaded.
async fn ensure_modular_workers(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let (llm_path, tts_path, settings) = {
        let s = state.settings.read().unwrap().clone();
        let models_dir = crate::utils::paths::get().models.clone();
        let llm = models_dir
            .join(crate::services::llm::MODEL_DIR_LLM)
            .join(crate::services::llm::MODEL_FILE_LLM_GGUF);
        let tts = models_dir.join(crate::services::tts::MODEL_DIR_TTS_SUPER);
        (llm, tts, s)
    };

    let mut lock = state.engine.lock().await;
    let engine = lock.as_mut().ok_or("Audio engine not ready")?;

    crate::services::llm::actor::warm_up_llm(
        app,
        crate::services::llm::actor::LlmWarmUpHandles {
            llm_tx: &mut engine.llm_tx,
            llm_handle: &mut engine.llm_handle,
            is_loaded: Arc::clone(&state.is_llm_loaded),
            is_sleeping: Arc::clone(&state.is_sleeping),
        },
        &settings,
        &llm_path,
        engine.pipeline_tx.clone(),
    )?;

    crate::services::tts::actor::warm_up_tts(
        app,
        crate::services::tts::actor::TtsWarmUpHandles {
            tts_tx: &mut engine.tts_tx,
            tts_handle: &mut engine.tts_handle,
            cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
            is_loaded: Arc::clone(&state.is_tts_loaded),
            is_sleeping: Arc::clone(&state.is_sleeping),
        },
        &settings,
        &tts_path,
        engine.pipeline_tx.clone(),
    )?;

    Ok(())
}

/// Starts a user-gated modular Push-To-Talk voice assistant session.
pub async fn start_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    crate::services::audio::start_audio_engine(app, state).await?;
    ensure_modular_workers(app, state).await?;

    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    state.pipeline.is_engaged.store(true, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    state.pipeline.is_paused.store(false, Ordering::Relaxed);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let conv_id = now;
    state.conversation_id.store(conv_id, Ordering::Relaxed);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            let _ = tx.send(
                crate::persistence::events::PersistenceEvent::SessionStarted {
                    id: conv_id,
                    timestamp_ms: now,
                },
            );
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "session_started", conv_id) {
        log::warn!("[ModularPTT] Failed to emit session_started: {}", e);
    }

    log::info!("[ModularPTT] PTT session started (ID: {})", conv_id);
    Ok(())
}

/// Ends the active modular Push-To-Talk voice assistant session.
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.pipeline.is_engaged.store(false, Ordering::Relaxed);
    IS_RECORDING.store(false, Ordering::Relaxed);
    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    PTT_BUFFER.lock().clear();
    CHUNKER.lock().clear();

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            let _ = tx.send(crate::persistence::events::PersistenceEvent::SessionEnded {
                id: conv_id,
                timestamp_ms: now,
            });
        }
    }

    let dictation_enabled = state.is_dictation_enabled.load(Ordering::Relaxed);
    if dictation_enabled {
        state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
    } else {
        crate::services::audio::stop_audio_engine(state).await?;
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Idle, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "session_ended", "user".to_string()) {
        log::warn!("[ModularPTT] Failed to emit session_ended: {}", e);
    }

    log::info!("[ModularPTT] PTT session ended");
    Ok(())
}

/// Initiates Push-To-Talk speech recording and interrupts ongoing playback.
pub fn handle_ptt_start(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if IS_RECORDING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    CHUNKER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    PTT_BUFFER.lock().clear();

    let turn_id = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(
        "main",
        "ptt_status",
        serde_json::json!({
            "state": "RECORDING",
            "turn_id": turn_id,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit ptt_status RECORDING: {}", e);
    }

    log::info!("[ModularPTT] PTT recording started (Turn: {})", turn_id);
    Ok(())
}

/// Finalizes Push-To-Talk recording and triggers STT recognition if speech was captured.
pub fn handle_ptt_stop(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if !IS_RECORDING.swap(false, Ordering::SeqCst) {
        return Ok(());
    }

    let buffer = PTT_BUFFER.lock().split_off(0);
    let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);

    if buffer.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        let _ = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" }));
        return Ok(());
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(
        "main",
        "ptt_status",
        serde_json::json!({
            "state": "PROCESSING",
            "turn_id": turn_id,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit ptt_status PROCESSING: {}", e);
    }

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine
                .stt_tx
                .send(crate::services::stt::SttCommand::Final(turn_id, buffer))
            {
                log::warn!("[ModularPTT] Failed to dispatch Final audio to STT: {}", e);
            }
        }
    }

    log::info!("[ModularPTT] PTT recording finalized (Turn: {})", turn_id);
    Ok(())
}

/// Cancels an in-progress Push-To-Talk recording and discards audio buffers.
pub fn handle_ptt_cancel(app: &AppHandle, state: &AppState) -> Result<(), String> {
    IS_RECORDING.store(false, Ordering::Relaxed);
    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    PTT_BUFFER.lock().clear();
    CHUNKER.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" })) {
        log::warn!("[ModularPTT] Failed to emit ptt_status IDLE: {}", e);
    }

    log::info!("[ModularPTT] PTT recording cancelled");
    Ok(())
}

/// Handles interim partial speech recognition results.
fn on_transcript_partial(turn_id: u32, text: String, app: &AppHandle) {
    if let Err(e) = app.emit_to(
        "main",
        "transcript_partial",
        serde_json::json!({
            "turn_id": turn_id,
            "text": text,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit transcript_partial: {}", e);
    }
}

/// Handles finalized speech transcript and initiates LLM generation workflow.
fn on_transcript_final(turn_id: u32, text: String, app: &AppHandle, state: &AppState) {
    if text.trim().is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        let _ = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" }));
        return;
    }

    if let Err(e) = app.emit_to(
        "main",
        "transcript_final",
        serde_json::json!({
            "turn_id": turn_id,
            "text": text,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit transcript_final: {}", e);
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    state.conversation_manager.lock().push_user_turn(text);

    let (temperature, max_output_tokens) = {
        let s = state.settings.read().unwrap();
        (s.llm.temperature, s.llm.max_output_tokens)
    };

    let request = GenerationRequest {
        input: ConversationInput {
            messages: state.conversation_manager.lock().get_messages().to_vec(),
        },
        options: GenerationOptions {
            temperature: Some(temperature),
            max_output_tokens: Some(max_output_tokens),
            ..Default::default()
        },
        output: OutputConstraint::Text,
        purpose: GenerationPurpose::Conversation,
    };

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Some(ref tx) = engine.llm_tx {
                if let Err(e) = tx.send(LlmCommand::Generate {
                    request,
                    turn_id,
                    cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
                }) {
                    log::warn!("[ModularPTT] Failed to send LlmCommand::Generate: {}", e);
                }
            }
        }
    }
}

/// Handles streamed LLM token emissions, accumulates clauses, and dispatches TTS synthesis.
fn on_llm_token(turn_id: u32, token: String, app: &AppHandle, state: &AppState) {
    if let Err(e) = app.emit_to(
        "main",
        "llm_token",
        serde_json::json!({
            "turn_id": turn_id,
            "token": token,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit llm_token: {}", e);
    }

    let clauses = CHUNKER.lock().push_str(&token);
    if !clauses.is_empty() {
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                if let Some(ref tx) = engine.tts_tx {
                    for clause in clauses {
                        if let Err(e) = tx.send(TtsCommand::Generate {
                            turn_id,
                            text: clause,
                        }) {
                            log::warn!("[ModularPTT] Failed to send TtsCommand::Generate: {}", e);
                        }
                    }
                }
            }
        }
    }
}

/// Handles completed LLM text synthesis, flushes trailing clauses to TTS, and notifies UI.
fn on_llm_finished(turn_id: u32, app: &AppHandle, state: &AppState) {
    if let Some(remainder) = CHUNKER.lock().flush() {
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                if let Some(ref tx) = engine.tts_tx {
                    if let Err(e) = tx.send(TtsCommand::Generate {
                        turn_id,
                        text: remainder,
                    }) {
                        log::warn!("[ModularPTT] Failed to send trailing TtsCommand: {}", e);
                    }
                }
            }
        }
    }

    if let Err(e) = app.emit_to("main", "llm_finished", turn_id) {
        log::warn!("[ModularPTT] Failed to emit llm_finished: {}", e);
    }
}

/// Forwards synthesized audio samples to the audio playback buffer.
fn on_tts_chunk(samples: Vec<f32>, playback: &Arc<PlaybackEngine>) {
    playback.ingest_chunk(&samples);
}

/// Updates latest TTS real-time factor metrics upon synthesis completion.
fn on_tts_finished(rtf: f32, state: &AppState) {
    state.latest_tts_rtf.store(rtf.to_bits(), Ordering::Relaxed);
}

/// Transitions pipeline state to assistant speaking when audio playback begins.
fn on_playback_started(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "playback_started", turn_id) {
        log::warn!("[ModularPTT] Failed to emit playback_started: {}", e);
    }
}

/// Finalizes assistant response playback and transitions pipeline back to idle resting state.
fn on_playback_finished(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "playback_finished", turn_id) {
        log::warn!("[ModularPTT] Failed to emit playback_finished: {}", e);
    }

    let _ = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" }));
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error(turn_id: u32, message: String, app: &AppHandle, state: &AppState) {
    log::error!("[ModularPTT] Error on turn {}: {}", turn_id, message);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Error, &ctx, app, state);

    if let Err(e) = app.emit_to(
        "main",
        "pipeline_error",
        serde_json::json!({
            "turn_id": turn_id,
            "message": message,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit pipeline_error: {}", e);
    }

    let _ = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" }));
}

/// Main event dispatcher for the modular Push-To-Talk pipeline domain.
pub fn handle_event(
    app: &AppHandle,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
        VoxEvent::TranscriptPartial { turn_id, text } => on_transcript_partial(turn_id, text, app),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::LlmToken { turn_id, token } => on_llm_token(turn_id, token, app, state),
        VoxEvent::LlmFinished { turn_id } => on_llm_finished(turn_id, app, state),
        VoxEvent::TtsChunk { samples, .. } => on_tts_chunk(samples, playback),
        VoxEvent::TtsFinished { rtf, .. } => on_tts_finished(rtf, state),
        VoxEvent::PlaybackStarted { turn_id } => on_playback_started(turn_id, app, state),
        VoxEvent::PlaybackFinished { turn_id } => on_playback_finished(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
