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
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};

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
        &mut engine.llm_tx,
        &mut engine.llm_handle,
        &settings,
        &llm_path,
        engine.pipeline_tx.clone(),
        Arc::clone(&state.is_llm_loaded),
        Arc::clone(&state.is_sleeping),
    )?;

    crate::services::tts::actor::warm_up_tts(
        app,
        &mut engine.tts_tx,
        &mut engine.tts_handle,
        &settings,
        &tts_path,
        Arc::clone(&state.pipeline.cancel_flag),
        engine.pipeline_tx.clone(),
        Arc::clone(&state.is_tts_loaded),
        Arc::clone(&state.is_sleeping),
    )?;

    Ok(())
}

/// Starts an autonomous modular passive voice assistant session.
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
        log::warn!("[ModularPassive] Failed to emit session_started: {}", e);
    }

    log::info!("[ModularPassive] Passive session started (ID: {})", conv_id);
    Ok(())
}

/// Pauses the active modular passive voice pipeline.
pub async fn pause_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    state.pipeline.is_paused.store(true, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Paused, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "pipeline_paused", ()) {
        log::warn!("[ModularPassive] Failed to emit pipeline_paused: {}", e);
    }

    log::info!("[ModularPassive] Passive session paused");
    Ok(())
}

/// Resumes a paused modular passive voice pipeline.
pub async fn resume_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    state.pipeline.is_paused.store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "pipeline_resumed", ()) {
        log::warn!("[ModularPassive] Failed to emit pipeline_resumed: {}", e);
    }

    log::info!("[ModularPassive] Passive session resumed");
    Ok(())
}

/// Ends the active modular passive voice assistant session and unloads models if idle.
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.pipeline.is_engaged.store(false, Ordering::Relaxed);

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
        log::warn!("[ModularPassive] Failed to emit session_ended: {}", e);
    }

    log::info!("[ModularPassive] Passive session ended");
    Ok(())
}

/// Handles user speech detection onset and aborts ongoing assistant playback.
fn on_speech_start(turn_id: u32, app: &AppHandle, state: &AppState) {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed)
        || state.pipeline.is_paused.load(Ordering::Relaxed)
    {
        return;
    }

    CHUNKER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.conversation_manager.lock().pop_last_user_turn();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "speech_start", turn_id) {
        log::warn!("[ModularPassive] Failed to emit speech_start: {}", e);
    }
}

/// Handles user speech completion and transitions the pipeline state to thinking.
fn on_speech_end(turn_id: u32, app: &AppHandle, state: &AppState, _audio: Vec<f32>) {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed)
        || state.pipeline.is_paused.load(Ordering::Relaxed)
    {
        return;
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "speech_end", turn_id) {
        log::warn!("[ModularPassive] Failed to emit speech_end: {}", e);
    }
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
        log::warn!("[ModularPassive] Failed to emit transcript_partial: {}", e);
    }
}

/// Handles finalized speech transcript and initiates LLM generation workflow.
fn on_transcript_final(turn_id: u32, text: String, app: &AppHandle, state: &AppState) {
    if text.trim().is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
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
        log::warn!("[ModularPassive] Failed to emit transcript_final: {}", e);
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
                    log::warn!(
                        "[ModularPassive] Failed to send LlmCommand::Generate: {}",
                        e
                    );
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
        log::warn!("[ModularPassive] Failed to emit llm_token: {}", e);
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
                            log::warn!(
                                "[ModularPassive] Failed to send TtsCommand::Generate: {}",
                                e
                            );
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
                        log::warn!("[ModularPassive] Failed to send trailing TtsCommand: {}", e);
                    }
                }
            }
        }
    }

    if let Err(e) = app.emit_to("main", "llm_finished", turn_id) {
        log::warn!("[ModularPassive] Failed to emit llm_finished: {}", e);
    }
}

/// Forwards synthesized audio samples to the audio playback buffer.
fn on_tts_chunk(_turn_id: u32, samples: Vec<f32>, playback: &Arc<PlaybackEngine>) {
    playback.ingest_chunk(&samples);
}

/// Updates latest TTS real-time factor metrics upon synthesis completion.
fn on_tts_finished(_turn_id: u32, rtf: f32, state: &AppState) {
    state.latest_tts_rtf.store(rtf.to_bits(), Ordering::Relaxed);
}

/// Transitions pipeline state to assistant speaking when audio playback begins.
fn on_playback_started(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "playback_started", turn_id) {
        log::warn!("[ModularPassive] Failed to emit playback_started: {}", e);
    }
}

/// Finalizes assistant response playback and transitions pipeline back to listening.
fn on_playback_finished(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "playback_finished", turn_id) {
        log::warn!("[ModularPassive] Failed to emit playback_finished: {}", e);
    }
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error(turn_id: u32, message: String, app: &AppHandle, state: &AppState) {
    log::error!("[ModularPassive] Error on turn {}: {}", turn_id, message);
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
        log::warn!("[ModularPassive] Failed to emit pipeline_error: {}", e);
    }
}

/// Main event dispatcher for the modular passive pipeline domain.
pub fn handle_event(
    app: &AppHandle,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
        VoxEvent::SpeechStart { turn_id } => on_speech_start(turn_id, app, state),
        VoxEvent::SpeechEnd {
            turn_id,
            audio_buffer,
        } => on_speech_end(turn_id, app, state, audio_buffer),
        VoxEvent::TranscriptPartial { turn_id, text } => on_transcript_partial(turn_id, text, app),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::LlmToken { turn_id, token } => on_llm_token(turn_id, token, app, state),
        VoxEvent::LlmFinished { turn_id } => on_llm_finished(turn_id, app, state),
        VoxEvent::TtsChunk { turn_id, samples } => on_tts_chunk(turn_id, samples, playback),
        VoxEvent::TtsFinished { turn_id, rtf } => on_tts_finished(turn_id, rtf, state),
        VoxEvent::PlaybackStarted { turn_id } => on_playback_started(turn_id, app, state),
        VoxEvent::PlaybackFinished { turn_id } => on_playback_finished(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
