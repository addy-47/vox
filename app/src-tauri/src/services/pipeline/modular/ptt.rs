use super::super::{
    transition, RoutingContext, END_REASON_USER, EVENT_LLM_FINISHED, EVENT_LLM_TOKEN,
    EVENT_PIPELINE_ERROR, EVENT_PLAYBACK_FINISHED, EVENT_PLAYBACK_STARTED, EVENT_PTT_STATUS,
    EVENT_SESSION_ENDED, EVENT_SESSION_STARTED, EVENT_TRANSCRIPT_FINAL, EVENT_TRANSCRIPT_PARTIAL,
    STATUS_IDLE, STATUS_PROCESSING, STATUS_RECORDING, WINDOW_MAIN,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::audio::PlaybackEngine;
use crate::services::llm::actor::LlmCommand;
use crate::services::tts::actor::{TtsClauseChunker, TtsCommand};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static SPEECH_DETECTED: AtomicBool = AtomicBool::new(false);
static PTT_BUFFER: Mutex<Vec<f32>> = Mutex::new(Vec::new());
static CHUNKER: LazyLock<Mutex<TtsClauseChunker>> =
    LazyLock::new(|| Mutex::new(TtsClauseChunker::new()));
static CURRENT_ASSISTANT_RESPONSE: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));

/// Ingests streaming audio frames into the Push-To-Talk buffer when recording is active.
pub fn ingest_audio(chunk: &[f32]) {
    if IS_RECORDING.load(Ordering::Relaxed) {
        PTT_BUFFER.lock().extend_from_slice(chunk);
    }
}

/// Returns true if Push-To-Talk audio recording is currently active.
pub fn is_recording() -> bool {
    IS_RECORDING.load(Ordering::Relaxed)
}

/// Returns the current sample count in the Push-To-Talk buffer.
pub fn get_buffer_len() -> usize {
    PTT_BUFFER.lock().len()
}

/// Starts a Push-To-Talk modular assistant session.
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
                log::warn!("[ModularPTT] Failed to send SessionStarted to persist: {}", e);
            }
        }
    }

    let prompt = state.settings.read().unwrap().persona.modular_prompt.clone();
    super::super::init_new_session(state, &prompt).await;

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SESSION_STARTED, conv_id) {
        log::warn!("[ModularPTT] Failed to emit session_started: {}", e);
    }

    log::info!("[ModularPTT] Modular PTT session started (ID: {})", conv_id);
    Ok(())
}

/// Ends the active modular Push-To-Talk session.
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    IS_RECORDING.store(false, Ordering::Relaxed);
    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    PTT_BUFFER.lock().clear();
    CHUNKER.lock().clear();
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
            if let Err(e) = tx.send(
                crate::persistence::events::PersistenceEvent::SessionEnded {
                    id: conv_id,
                    timestamp_ms: now,
                },
            ) {
                log::warn!("[ModularPTT] Failed to send SessionEnded to persist: {}", e);
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
        log::warn!("[ModularPTT] Failed to emit session_ended: {}", e);
    }

    log::info!("[ModularPTT] Modular PTT session ended");
    Ok(())
}

/// Begins PTT recording and transitions state to listening.
pub fn handle_ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed) {
        return Err("Modular PTT session not active".to_string());
    }

    IS_RECORDING.store(true, Ordering::Relaxed);
    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    PTT_BUFFER.lock().clear();
    CHUNKER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({ "state": STATUS_RECORDING }),
    ) {
        log::warn!("[ModularPTT] Failed to emit ptt_status RECORDING: {}", e);
    }

    log::info!("[ModularPTT] PTT recording started");
    Ok(())
}

/// Finalizes PTT recording, dispatches audio buffer to STT actor, and transitions to thinking.
pub fn handle_ptt_stop<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if !IS_RECORDING.swap(false, Ordering::Relaxed) {
        return Ok(());
    }

    let audio = PTT_BUFFER.lock().clone();
    PTT_BUFFER.lock().clear();

    if audio.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return Ok(());
    }

    let guard = state.engine.try_lock().map_err(|_| "Engine lock busy")?;
    let engine = guard.as_ref().ok_or("Audio engine not ready")?;
    let turn_id = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed);

    if let Err(e) = engine.stt_tx.send(crate::services::stt::SttCommand::Final(turn_id, audio)) {
        log::warn!("[ModularPTT] Failed to send Final to STT: {}", e);
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({ "state": STATUS_PROCESSING }),
    ) {
        log::warn!("[ModularPTT] Failed to emit ptt_status PROCESSING: {}", e);
    }

    log::info!("[ModularPTT] PTT recording stopped, turn {} dispatched to STT", turn_id);
    Ok(())
}

/// Cancels ongoing PTT recording without dispatching inference.
pub fn handle_ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if !IS_RECORDING.swap(false, Ordering::Relaxed) {
        return Ok(());
    }

    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    PTT_BUFFER.lock().clear();
    CHUNKER.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({ "state": STATUS_IDLE }),
    ) {
        log::warn!("[ModularPTT] Failed to emit ptt_status IDLE: {}", e);
    }

    log::info!("[ModularPTT] PTT recording cancelled");
    Ok(())
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
        log::warn!("[ModularPTT] Failed to emit transcript_partial: {}", e);
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
        log::warn!("[ModularPTT] Failed to emit transcript_final: {}", e);
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
                    log::warn!("[ModularPTT] Failed to send filler TTS: {}", e);
                }
            }
        }

        if let Some(ref tx) = llm_tx {
            if let Err(e) = tx.send(LlmCommand::Generate {
                request,
                turn_id,
                cancel_flag,
            }) {
                log::warn!("[ModularPTT] Failed to send LlmCommand::Generate: {}", e);
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
        log::warn!("[ModularPTT] Failed to emit llm_token: {}", e);
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
                            log::warn!("[ModularPTT] Failed to send TtsCommand::Generate: {}", e);
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
                        log::warn!("[ModularPTT] Failed to send trailing TtsCommand: {}", e);
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
fn on_playback_started<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PLAYBACK_STARTED, turn_id) {
        log::warn!("[ModularPTT] Failed to emit playback_started: {}", e);
    }
}

/// Finalizes assistant response playback and transitions pipeline back to idle resting state.
fn on_playback_finished<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    super::context::trigger_background_compaction(state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PLAYBACK_FINISHED, turn_id) {
        log::warn!("[ModularPTT] Failed to emit playback_finished: {}", e);
    }

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({ "state": STATUS_IDLE }),
    ) {
        log::warn!("[ModularPTT] Failed to emit ptt_status IDLE: {}", e);
    }
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error<R: tauri::Runtime>(turn_id: u32, message: String, app: &AppHandle<R>, state: &AppState) {
    log::error!("[ModularPTT] Error on turn {}: {}", turn_id, message);
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
        log::warn!("[ModularPTT] Failed to emit pipeline_error: {}", e);
    }

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({ "state": STATUS_IDLE }),
    ) {
        log::warn!("[ModularPTT] Failed to emit ptt_status IDLE: {}", e);
    }
}

/// Main event dispatcher for the modular Push-To-Talk pipeline domain.
pub fn handle_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
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
