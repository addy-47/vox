use super::super::{
    transition, RoutingContext, END_REASON_USER, EVENT_LLM_FINISHED, EVENT_LLM_TOKEN,
    EVENT_PIPELINE_ERROR, EVENT_PIPELINE_PAUSED, EVENT_PIPELINE_RESUMED, EVENT_PLAYBACK_FINISHED,
    EVENT_PLAYBACK_STARTED, EVENT_SESSION_ENDED, EVENT_SESSION_STARTED, EVENT_SPEECH_END,
    EVENT_SPEECH_START, EVENT_TRANSCRIPT_FINAL, EVENT_TRANSCRIPT_PARTIAL, WINDOW_MAIN,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::audio::PlaybackEngine;
use crate::services::llm::actor::LlmCommand;
use crate::services::tts::actor::{TtsClauseChunker, TtsCommand};
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};

static CHUNKER: LazyLock<Mutex<TtsClauseChunker>> =
    LazyLock::new(|| Mutex::new(TtsClauseChunker::new()));
static CURRENT_ASSISTANT_RESPONSE: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));

/// Starts an autonomous modular passive voice assistant session.
pub async fn start_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    crate::services::audio::start_audio_engine(app, state).await?;
    super::context::ensure_modular_workers(app, state).await?;

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
            if let Err(e) = tx.send(
                crate::persistence::events::PersistenceEvent::SessionStarted {
                    id: conv_id,
                    timestamp_ms: now,
                },
            ) {
                log::warn!("[ModularPassive] Failed to send SessionStarted to persist: {}", e);
            }
        }
    }

    let prompt = state.settings.read().unwrap().persona.modular_prompt.clone();
    super::super::init_new_session(state, &prompt).await;

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SESSION_STARTED, conv_id) {
        log::warn!("[ModularPassive] Failed to emit session_started: {}", e);
    }

    log::info!("[ModularPassive] Passive session started (ID: {})", conv_id);
    Ok(())
}

/// Pauses the active modular passive voice pipeline.
pub async fn pause_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    state.pipeline.is_paused.store(true, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    CHUNKER.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Paused, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PIPELINE_PAUSED, ()) {
        log::warn!("[ModularPassive] Failed to emit pipeline_paused: {}", e);
    }

    log::info!("[ModularPassive] Passive session paused");
    Ok(())
}

/// Resumes a paused modular passive voice pipeline.
pub async fn resume_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    state.pipeline.is_paused.store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PIPELINE_RESUMED, ()) {
        log::warn!("[ModularPassive] Failed to emit pipeline_resumed: {}", e);
    }

    log::info!("[ModularPassive] Passive session resumed");
    Ok(())
}

/// Ends the active modular passive voice assistant session.
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.pipeline.is_engaged.store(false, Ordering::Relaxed);
    CHUNKER.lock().clear();

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.send(
                crate::persistence::events::PersistenceEvent::SessionEnded {
                    id: conv_id,
                    timestamp_ms: now,
                },
            ) {
                log::warn!("[ModularPassive] Failed to send SessionEnded to persist: {}", e);
            }
        }
    }

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
        drop(guard);
        crate::services::audio::stop_audio_engine(state).await?;
    } else {
        crate::services::audio::stop_audio_engine(state).await?;
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Idle, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SESSION_ENDED, END_REASON_USER.to_string()) {
        log::warn!("[ModularPassive] Failed to emit session_ended: {}", e);
    }

    log::info!("[ModularPassive] Passive session ended");
    Ok(())
}

/// Handles user speech detection onset and aborts ongoing assistant playback.
fn on_speech_start<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed)
        || state.pipeline.is_paused.load(Ordering::Relaxed)
    {
        return;
    }

    CHUNKER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.conversation_manager.lock().on_speech_start();
    state.conversation_manager.lock().pop_last_user_turn();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SPEECH_START, turn_id) {
        log::warn!("[ModularPassive] Failed to emit speech_start: {}", e);
    }
}

/// Handles user speech completion and transitions the pipeline state to thinking.
fn on_speech_end<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed)
        || state.pipeline.is_paused.load(Ordering::Relaxed)
    {
        return;
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SPEECH_END, turn_id) {
        log::warn!("[ModularPassive] Failed to emit speech_end: {}", e);
    }
}

/// Handles interim partial speech recognition results.
fn on_transcript_partial<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>, state: &AppState) {
    let transliterate_enabled = state.settings.read().unwrap().stt.transliterate_enabled;
    let processed_text = crate::services::translit::transliterate_if_hi(&text, false, transliterate_enabled);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_TRANSCRIPT_PARTIAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": processed_text,
        }),
    ) {
        log::warn!("[ModularPassive] Failed to emit transcript_partial: {}", e);
    }
}

/// Handles finalized speech transcript and initiates LLM generation workflow.
fn on_transcript_final<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>, state: &AppState) {
    let transliterate_enabled = state.settings.read().unwrap().stt.transliterate_enabled;
    let processed_text = crate::services::translit::transliterate_if_hi(&text, true, transliterate_enabled);

    if processed_text.trim().is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return;
    }

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_TRANSCRIPT_FINAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": processed_text,
        }),
    ) {
        log::warn!("[ModularPassive] Failed to emit transcript_final: {}", e);
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    CURRENT_ASSISTANT_RESPONSE.lock().clear();

    let settings = state.settings.read().unwrap().clone();
    let cm_arc = Arc::clone(&state.conversation_manager);
    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let cancel_flag = Arc::clone(&state.pipeline.cancel_flag);
    let (tts_tx, llm_tx) = if let Ok(guard) = state.engine.try_lock() {
        guard
            .as_ref()
            .map(|e| (e.tts_tx.clone(), e.llm_tx.clone()))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    tauri::async_runtime::spawn(async move {
        let (request, transition_speech) =
            super::context::build_generation_request(&settings, &cm_arc, conv_id, &processed_text, turn_id).await;

        if let Some(filler) = transition_speech {
            if let Some(ref tx) = tts_tx {
                if let Err(e) = tx.send(TtsCommand::Generate {
                    turn_id,
                    text: filler,
                }) {
                    log::warn!("[ModularPassive] Failed to send filler TTS: {}", e);
                }
            }
        }

        if let Some(ref tx) = llm_tx {
            if let Err(e) = tx.send(LlmCommand::Generate {
                request,
                turn_id,
                cancel_flag,
            }) {
                log::warn!("[ModularPassive] Failed to send LlmCommand::Generate: {}", e);
            }
        }
    });
}

/// Handles streamed LLM token emissions, accumulates clauses, and dispatches TTS synthesis.
fn on_llm_token<R: tauri::Runtime>(turn_id: u32, token: String, app: &AppHandle<R>, state: &AppState) {
    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_LLM_TOKEN,
        serde_json::json!({
            "turn_id": turn_id,
            "token": token,
        }),
    ) {
        log::warn!("[ModularPassive] Failed to emit llm_token: {}", e);
    }

    let clauses = {
        CURRENT_ASSISTANT_RESPONSE.lock().push_str(&token);
        CHUNKER.lock().push_str(&token)
    };
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
fn on_llm_finished<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
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

    let full_text = CURRENT_ASSISTANT_RESPONSE.lock().split_off(0);
    if !full_text.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(full_text);
    }

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_LLM_FINISHED, turn_id) {
        log::warn!("[ModularPassive] Failed to emit llm_finished: {}", e);
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
fn on_playback_started<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PLAYBACK_STARTED, turn_id) {
        log::warn!("[ModularPassive] Failed to emit playback_started: {}", e);
    }
}

/// Transitions pipeline state back to listening upon playback completion.
fn on_playback_finished<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    super::context::trigger_background_compaction(state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PLAYBACK_FINISHED, turn_id) {
        log::warn!("[ModularPassive] Failed to emit playback_finished: {}", e);
    }
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error<R: tauri::Runtime>(turn_id: u32, message: String, app: &AppHandle<R>, state: &AppState) {
    log::error!("[ModularPassive] Error on turn {}: {}", turn_id, message);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Error, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PIPELINE_ERROR,
        serde_json::json!({
            "turn_id": turn_id,
            "message": message,
        }),
    ) {
        log::warn!("[ModularPassive] Failed to emit pipeline_error: {}", e);
    }
}

/// Main event dispatcher for the modular passive pipeline domain.
pub fn handle_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
        VoxEvent::SpeechStart { turn_id } => on_speech_start(turn_id, app, state),
        VoxEvent::SpeechEnd { turn_id, .. } => on_speech_end(turn_id, app, state),
        VoxEvent::TranscriptPartial { turn_id, text } => {
            on_transcript_partial(turn_id, text, app, state)
        }
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
