use super::super::{
    transition, RoutingContext, END_REASON_USER, EVENT_LLM_TOKEN, EVENT_PIPELINE_ERROR,
    EVENT_PIPELINE_PAUSED, EVENT_PIPELINE_RESUMED, EVENT_PLAYBACK_FINISHED, EVENT_PLAYBACK_STARTED,
    EVENT_SESSION_ENDED, EVENT_SESSION_STARTED, EVENT_SPEECH_END, EVENT_SPEECH_START,
    EVENT_TRANSCRIPT_FINAL, WINDOW_MAIN,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand};
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::engine::RealtimeEngine;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

static CURRENT_ASSISTANT_RESPONSE: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));
static CURRENT_USER_TRANSCRIPT: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));

/// Starts an autonomous real-time WebSocket speech-to-speech assistant session.
pub async fn start_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    crate::services::audio::start_audio_engine(app, state).await?;

    let (vad_tx, pipeline_tx, playback_engine) = {
        let guard = state.engine.lock().await;
        let eng = guard.as_ref().ok_or("Audio engine not ready")?;
        (
            eng.vad_tx.clone(),
            eng.pipeline_tx.clone(),
            eng.playback_engine.clone(),
        )
    };

    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut old_rt) = rt_guard.take() {
        old_rt.stop();
        if let Err(e) = vad_tx.send(VadCommand::StopRealtime) {
            log::warn!("[RealtimePassive] Failed to send StopRealtime: {}", e);
        }
    }

    let provider = super::session::create_realtime_provider(state)?;
    let tokio_handle = tokio::runtime::Handle::current();
    let mut rt_engine = RealtimeEngine::new(provider, tokio_handle);

    rt_engine
        .start(
            crate::core::settings::InteractionMode::Passive,
            playback_engine,
            pipeline_tx,
        )
        .map_err(|e| format!("[RealtimePassive] Engine start failed: {}", e))?;

    let audio_tx = rt_engine
        .get_audio_sender()
        .ok_or("Failed to obtain realtime audio sender")?;

    if let Err(e) = vad_tx.send(VadCommand::StartRealtime {
        tx: audio_tx,
        is_ptt: false,
    }) {
        log::warn!("[RealtimePassive] Failed to send StartRealtime: {}", e);
    }

    state.pipeline.is_engaged.store(true, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    state.pipeline.is_paused.store(false, Ordering::Relaxed);
    *rt_guard = Some(rt_engine);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let conv_id = now;
    state.conversation_id.store(conv_id, Ordering::Relaxed);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(
                crate::persistence::events::PersistenceEvent::SessionStarted {
                    id: conv_id,
                    timestamp_ms: now,
                },
            ) {
                log::warn!("[RealtimePassive] Failed to send SessionStarted to persist: {}", e);
            }
        }
    }

    {
        let mem_lock = state.memory_tx.lock();
        if let Some(ref tx) = *mem_lock {
            if let Err(e) = tx.try_send(crate::persistence::memory_worker::MemoryWorkerEvent::ActiveSessionChanged {
                session_id: conv_id,
            }) {
                log::trace!("[RealtimePassive] Failed to send ActiveSessionChanged to memory worker: {}", e);
            }
        }
    }

    let prompt = state.settings.read().unwrap_or_else(|p| p.into_inner()).persona.realtime_prompt.clone();
    super::super::init_new_session(state, &prompt).await;

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SESSION_STARTED, conv_id) {
        log::warn!("[RealtimePassive] Failed to emit session_started: {}", e);
    }

    log::info!("[RealtimePassive] Realtime passive session started (ID: {})", conv_id);
    Ok(())
}

/// Pauses the active real-time voice pipeline.
pub async fn pause_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    state.pipeline.is_paused.store(true, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Paused, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PIPELINE_PAUSED, ()) {
        log::warn!("[RealtimePassive] Failed to emit pipeline_paused: {}", e);
    }

    log::info!("[RealtimePassive] Realtime passive session paused");
    Ok(())
}

/// Resumes a paused real-time voice pipeline.
pub async fn resume_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    state.pipeline.is_paused.store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PIPELINE_RESUMED, ()) {
        log::warn!("[RealtimePassive] Failed to emit pipeline_resumed: {}", e);
    }

    log::info!("[RealtimePassive] Realtime passive session resumed");
    Ok(())
}

/// Ends the active real-time speech-to-speech session and tears down the WebSocket connection.
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if let Some(engine) = state.engine.lock().await.as_ref() {
        if let Err(e) = engine.vad_tx.send(VadCommand::StopRealtime) {
            log::warn!("[RealtimePassive] Failed to send StopRealtime: {}", e);
        }
        engine.playback_engine.cancel();
    }

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut rt_engine) = rt_guard.take() {
        rt_engine.stop();
    }

    state.pipeline.is_engaged.store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(
                crate::persistence::events::PersistenceEvent::SessionEnded {
                    id: conv_id,
                    timestamp_ms: now,
                },
            ) {
                log::warn!("[RealtimePassive] Failed to send SessionEnded to persist: {}", e);
            }
        }
    }

    {
        let mem_lock = state.memory_tx.lock();
        if let Some(ref tx) = *mem_lock {
            if let Err(e) = tx.try_send(crate::persistence::memory_worker::MemoryWorkerEvent::SessionEnd {
                session_id: conv_id.to_string(),
                summary: String::new(),
            }) {
                log::trace!("[RealtimePassive] Failed to send SessionEnd to memory worker: {}", e);
            }
        }
    }

    let dictation_enabled = state
        .settings
        .read()
        .map(|s| s.dictation.enabled)
        .unwrap_or(false);

    if dictation_enabled {
        state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
    } else {
        crate::services::audio::stop_audio_engine(state).await?;
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Idle, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SESSION_ENDED, END_REASON_USER.to_string()) {
        log::warn!("[RealtimePassive] Failed to emit session_ended: {}", e);
    }

    log::info!("[RealtimePassive] Realtime passive session ended");
    Ok(())
}

/// Handles user speech detection and transitions state machine to speaking.
fn on_speech_start<R: tauri::Runtime>(
    turn_id: u32,
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed)
        || state.pipeline.is_paused.load(Ordering::Relaxed)
    {
        return;
    }

    super::session::realtime_barge_in(state, playback);

    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SPEECH_START, turn_id) {
        log::warn!("[RealtimePassive] Failed to emit speech_start: {}", e);
    }
}

/// Handles speech end and transitions state machine to thinking.
fn on_speech_end<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed)
        || state.pipeline.is_paused.load(Ordering::Relaxed)
    {
        return;
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SPEECH_END, turn_id) {
        log::warn!("[RealtimePassive] Failed to emit speech_end: {}", e);
    }
}

/// Handles incoming final transcription from the real-time server.
fn on_transcript_final<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>, state: &AppState) {
    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_TRANSCRIPT_FINAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": text,
        }),
    ) {
        log::warn!("[RealtimePassive] Failed to emit transcript_final: {}", e);
    }

    CURRENT_ASSISTANT_RESPONSE.lock().clear();
    *CURRENT_USER_TRANSCRIPT.lock() = text.clone();
    {
        let mut cm = state.conversation_manager.lock();
        if !cm.is_duplicate_user_turn(&text) {
            cm.push_user_turn(text);
        }
    }
}

/// Handles streamed token delta from the real-time server.
fn on_llm_token<R: tauri::Runtime>(turn_id: u32, token: String, app: &AppHandle<R>) {
    CURRENT_ASSISTANT_RESPONSE.lock().push_str(&token);
    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_LLM_TOKEN,
        serde_json::json!({
            "turn_id": turn_id,
            "token": token,
        }),
    ) {
        log::warn!("[RealtimePassive] Failed to emit llm_token: {}", e);
    }
}

/// Transitions pipeline state to assistant speaking when audio playback begins.
fn on_playback_started<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PLAYBACK_STARTED, turn_id) {
        log::warn!("[RealtimePassive] Failed to emit playback_started: {}", e);
    }
}

/// Transitions pipeline state back to listening upon playback completion.
fn on_playback_finished<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let full_text = CURRENT_ASSISTANT_RESPONSE.lock().split_off(0);
    if !full_text.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(full_text.clone());

        let conv_id = state.conversation_id.load(Ordering::Relaxed);
        let user_text = CURRENT_USER_TRANSCRIPT.lock().clone();
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(crate::persistence::events::PersistenceEvent::TurnCompleted {
                conversation_id: conv_id,
                turn_id,
                user_text,
                assistant_text: full_text,
                stt_latency_ms: 0,
                ttft_ms: 0,
            }) {
                log::warn!("[RealtimePassive] Failed to send TurnCompleted to persist: {}", e);
            }
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PLAYBACK_FINISHED, turn_id) {
        log::warn!("[RealtimePassive] Failed to emit playback_finished: {}", e);
    }
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error<R: tauri::Runtime>(turn_id: u32, message: String, app: &AppHandle<R>, state: &AppState) {
    log::error!("[RealtimePassive] Error on turn {}: {}", turn_id, message);
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
        log::warn!("[RealtimePassive] Failed to emit pipeline_error: {}", e);
    }
}

/// Handles cancellation event and resets state machine to Ready.
fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!("[RealtimePassive] Interaction cancelled on turn {}", turn_id);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);
}

/// Main event dispatcher for the realtime passive pipeline domain.
pub fn handle_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
        VoxEvent::SpeechStart { turn_id } => on_speech_start(turn_id, app, state, playback),
        VoxEvent::SpeechEnd { turn_id, .. } => on_speech_end(turn_id, app, state),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::LlmToken { turn_id, token } => on_llm_token(turn_id, token, app),
        VoxEvent::PlaybackStarted { turn_id } => on_playback_started(turn_id, app, state),
        VoxEvent::PlaybackFinished { turn_id } => on_playback_finished(turn_id, app, state),
        VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
