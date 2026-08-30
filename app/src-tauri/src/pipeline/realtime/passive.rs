use super::super::{
    transition, RoutingContext, EVENT_LLM_TOKEN, EVENT_PIPELINE_ERROR,
    EVENT_TRANSCRIPT_FINAL, EVENT_TRANSCRIPT_PARTIAL, WINDOW_MAIN,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand};
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::engine::RealtimeEngine;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};

struct TurnAccumulator {
    assistant_response: String,
    user_transcript: String,
}

impl TurnAccumulator {
    fn new() -> Self {
        Self {
            assistant_response: String::new(),
            user_transcript: String::new(),
        }
    }

    fn clear(&mut self) {
        self.assistant_response.clear();
        self.user_transcript.clear();
    }

    fn push_token(&mut self, token: &str) {
        self.assistant_response.push_str(token);
    }

    fn set_user_transcript(&mut self, text: String) {
        self.user_transcript = text;
    }

    fn take_assistant_response(&mut self) -> String {
        std::mem::take(&mut self.assistant_response)
    }

    fn user_transcript(&self) -> String {
        self.user_transcript.clone()
    }
}

static ACCUMULATOR: LazyLock<Mutex<TurnAccumulator>> =
    LazyLock::new(|| Mutex::new(TurnAccumulator::new()));

/// Starts an autonomous real-time WebSocket speech-to-speech assistant session.
pub async fn start_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    crate::core::start_audio_engine(app, state).await?;

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

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
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

    ACCUMULATOR.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    let state_arc = app.state::<std::sync::Arc<AppState>>().inner().clone();
    super::session::spawn_realtime_idle_monitor(app.clone(), state_arc);

    log::info!("[RealtimePassive] Realtime passive session started (ID: {})", conv_id);
    Ok(())
}

/// Pauses the active real-time voice pipeline.
pub async fn pause_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    ACCUMULATOR.lock().clear();

    if let Ok(mut rt_guard) = state.realtime_engine.try_lock() {
        if let Some(ref mut rt_engine) = *rt_guard {
            rt_engine.stop();
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Paused, &ctx, app, state);

    log::info!("[RealtimePassive] Realtime passive session paused");
    Ok(())
}

/// Resumes a paused real-time voice pipeline.
pub async fn resume_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let (playback_engine, pipeline_tx, vad_tx) = {
        let guard = state.engine.lock().await;
        let engine = guard.as_ref().ok_or("Engine not initialized")?;
        (engine.playback_engine.clone(), engine.pipeline_tx.clone(), engine.vad_tx.clone())
    };

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(ref mut rt_engine) = *rt_guard {
        rt_engine.start(
            crate::core::settings::InteractionMode::Passive,
            playback_engine,
            pipeline_tx,
        ).map_err(|e| format!("[RealtimePassive] Engine restart failed: {}", e))?;

        let audio_tx = rt_engine.get_audio_sender().ok_or("Failed to obtain realtime audio sender")?;
        if let Err(e) = vad_tx.send(VadCommand::StartRealtime { tx: audio_tx, is_ptt: false }) {
            log::warn!("[RealtimePassive] Failed to send StartRealtime on resume: {}", e);
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    log::info!("[RealtimePassive] Realtime passive session resumed");
    Ok(())
}

/// Ends the active real-time voice assistant session.
pub async fn end_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if let Err(e) = engine.vad_tx.send(VadCommand::StopRealtime) {
                log::warn!("[RealtimePassive] Failed to send StopRealtime: {}", e);
            }
        }
    }

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut rt_engine) = rt_guard.take() {
        rt_engine.stop();
    }

    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    ACCUMULATOR.lock().clear();

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

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Idle, &ctx, app, state);

    log::info!("[RealtimePassive] Realtime passive session ended");
    Ok(())
}

/// Handles interim partial speech recognition results from the real-time server.
fn on_transcript_partial<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        return;
    }

    if current_state == InteractionState::Thinking || current_state == InteractionState::Speaking {
        super::session::realtime_barge_in(state, playback);
        state.pipeline.renew_turn_token();
        state.conversation_manager.lock().handle_barge_in();
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Listening, &ctx, app, state);
    } else if current_state == InteractionState::Ready {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Listening, &ctx, app, state);
    }

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_TRANSCRIPT_PARTIAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": text,
        }),
    ) {
        log::warn!("[RealtimePassive] Failed to emit transcript_partial: {}", e);
    }
}

/// Handles incoming final transcription from the real-time server.
fn on_transcript_final<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>, state: &AppState) {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        return;
    }

    if current_state == InteractionState::Ready {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Listening, &ctx, app, state);
    }

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

    {
        let mut acc = ACCUMULATOR.lock();
        acc.clear();
        acc.set_user_transcript(text);
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);
}

/// Handles streamed LLM token from the real-time provider.
fn on_llm_token<R: tauri::Runtime>(turn_id: u32, token: String, app: &AppHandle<R>) {
    ACCUMULATOR.lock().push_token(&token);
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
fn on_playback_started<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);
}

/// Transitions pipeline state back to listening upon playback completion.
fn on_playback_finished<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let full_text = ACCUMULATOR.lock().take_assistant_response();
    if !full_text.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(full_text.clone());

        let conv_id = state.conversation_id.load(Ordering::Relaxed);
        let user_text = ACCUMULATOR.lock().user_transcript();
        let stt_ms = state.telemetry.latest_stt_ms.load(Ordering::Relaxed);
        let ttft_ms = state.telemetry.latest_ttft_ms.load(Ordering::Relaxed);
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(crate::persistence::events::PersistenceEvent::TurnCompleted {
                conversation_id: conv_id,
                turn_id,
                user_text,
                assistant_text: full_text,
                stt_latency_ms: stt_ms,
                ttft_ms,
            }) {
                log::warn!("[RealtimePassive] Failed to send TurnCompleted to persist: {}", e);
            }
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);
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
        VoxEvent::TranscriptPartial { turn_id, text } => {
            on_transcript_partial(turn_id, text, app, state, playback)
        }
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::LlmToken { turn_id, token } => on_llm_token(turn_id, token, app),
        VoxEvent::PlaybackStarted { .. } => on_playback_started(app, state),
        VoxEvent::PlaybackFinished { turn_id } => on_playback_finished(turn_id, app, state),
        VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
