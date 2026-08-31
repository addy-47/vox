use super::super::{transition, RoutingContext, WINDOW_MAIN};
use crate::core::events::VoiceErrorPayload;
use crate::core::events::{emit_ipc_to, IpcEvent, LlmTokenPayload, TranscriptPayload, VoxEvent};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::engine::RealtimeEngine;
use crate::services::vad::VadCommand;
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use tauri::AppHandle;

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
pub async fn start_session<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let (vad_tx, pipeline_tx, playback_engine) = {
        let guard = state.engine.lock().await;
        let eng = guard.as_ref().ok_or("Audio engine not ready")?;
        (
            eng.vad_tx.clone(),
            eng.pipeline_tx.clone(),
            eng.playback_engine.clone(),
        )
    };

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
            state.pipeline.turn_id.clone(),
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

    *rt_guard = Some(rt_engine);
    ACCUMULATOR.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    log::info!(
        "[RealtimePassive] Realtime passive session started (ID: {})",
        conv_id
    );
    Ok(())
}

/// Pauses the active real-time voice pipeline.
pub async fn pause_session<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    ACCUMULATOR.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if let Err(e) = engine.vad_tx.send(VadCommand::StopRealtime) {
                log::warn!(
                    "[RealtimePassive] Failed to send StopRealtime on pause: {}",
                    e
                );
            }
        }
    }

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
pub async fn resume_session<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let (playback_engine, pipeline_tx, vad_tx) = {
        let guard = state.engine.lock().await;
        let engine = guard.as_ref().ok_or("Engine not initialized")?;
        (
            engine.playback_engine.clone(),
            engine.pipeline_tx.clone(),
            engine.vad_tx.clone(),
        )
    };

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(ref mut rt_engine) = *rt_guard {
        rt_engine
            .start(
                crate::core::settings::InteractionMode::Passive,
                playback_engine,
                pipeline_tx,
                state.pipeline.turn_id.clone(),
            )
            .map_err(|e| format!("[RealtimePassive] Engine restart failed: {}", e))?;

        let audio_tx = rt_engine
            .get_audio_sender()
            .ok_or("Failed to obtain realtime audio sender")?;
        if let Err(e) = vad_tx.send(VadCommand::StartRealtime {
            tx: audio_tx,
            is_ptt: false,
        }) {
            log::warn!(
                "[RealtimePassive] Failed to send StartRealtime on resume: {}",
                e
            );
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    log::info!("[RealtimePassive] Realtime passive session resumed");
    Ok(())
}

/// Ends the active real-time voice assistant session.
pub async fn end_session<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
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

    ACCUMULATOR.lock().clear();
    super::session::purge_session_cache();

    log::info!("[RealtimePassive] Realtime passive session ended");
    Ok(())
}

/// Handles dedicated barge-in interruption for realtime passive pipeline.
fn on_interrupt<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    playback.cancel();

    if let Ok(rt_guard) = state.realtime_engine.try_lock() {
        if let Some(ref rt_engine) = *rt_guard {
            if let Err(e) = rt_engine.cancel() {
                log::warn!(
                    "[RealtimePassive] Error sending cancel to realtime engine: {}",
                    e
                );
            }
        }
    }

    state.pipeline.renew_turn_token();

    let partial_assistant = ACCUMULATOR.lock().take_assistant_response();
    let user_text = ACCUMULATOR.lock().user_transcript();
    let interrupted_turn_id = state.pipeline.peek_turn_id();
    let conv_id = state.conversation_id.load(Ordering::Relaxed);

    if !partial_assistant.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(partial_assistant.clone());
    }

    let persist_lock = state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        if let Err(e) = tx.try_send(
            crate::persistence::events::PersistenceEvent::TurnCompleted {
                conversation_id: conv_id,
                turn_id: interrupted_turn_id,
                user_text,
                assistant_text: partial_assistant,
                stt_latency_ms: 0,
                ttft_ms: 0,
            },
        ) {
            log::warn!(
                "[RealtimePassive] Failed to send TurnCompleted on interrupt: {}",
                e
            );
        }
    }

    ACCUMULATOR.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);
    log::info!(
        "[RealtimePassive] Interruption handled (turn: {})",
        interrupted_turn_id
    );
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
        on_interrupt(app, state, playback);
    } else if current_state == InteractionState::Ready {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Listening, &ctx, app, state);
    }

    let transliterate_enabled = state
        .settings
        .read()
        .map(|s| s.stt.transliterate_enabled)
        .unwrap_or(false);
    let processed_text =
        crate::services::translit::transliterate_if_hi(&text, false, transliterate_enabled);

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::TranscriptPartial(TranscriptPayload {
            turn_id,
            text: processed_text,
            owner: Some(InteractionOwner::Assistant),
        }),
    ) {
        log::warn!("[RealtimePassive] Failed to emit transcript_partial: {}", e);
    }
}

/// Handles incoming final transcription from the real-time server.
fn on_transcript_final<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        return;
    }

    let transliterate_enabled = state
        .settings
        .read()
        .map(|s| s.stt.transliterate_enabled)
        .unwrap_or(false);
    let processed_text =
        crate::services::translit::transliterate_if_hi(&text, true, transliterate_enabled);

    if current_state == InteractionState::Ready {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Listening, &ctx, app, state);
    }

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::TranscriptFinal(TranscriptPayload {
            turn_id,
            text: processed_text.clone(),
            owner: Some(InteractionOwner::Assistant),
        }),
    ) {
        log::warn!("[RealtimePassive] Failed to emit transcript_final: {}", e);
    }

    {
        let mut acc = ACCUMULATOR.lock();
        acc.clear();
        acc.set_user_transcript(processed_text);
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);
}

/// Handles streamed LLM token from the real-time provider.
fn on_llm_token<R: tauri::Runtime>(turn_id: u32, token: String, app: &AppHandle<R>) {
    ACCUMULATOR.lock().push_token(&token);
    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::LlmToken(LlmTokenPayload {
            turn_id,
            token: token.clone(),
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
            if let Err(e) = tx.try_send(
                crate::persistence::events::PersistenceEvent::TurnCompleted {
                    conversation_id: conv_id,
                    turn_id,
                    user_text,
                    assistant_text: full_text,
                    stt_latency_ms: stt_ms,
                    ttft_ms,
                },
            ) {
                log::warn!(
                    "[RealtimePassive] Failed to send TurnCompleted to persist: {}",
                    e
                );
            }
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);
}

/// Handles error event, transitions to Error state, and surfaces error payload to UI.
fn on_error<R: tauri::Runtime>(
    _turn_id: u32,
    message: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Error, &ctx, app, state);

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::VoiceError(VoiceErrorPayload {
            message,
            source: "RealtimePassive".to_string(),
            owner: Some(InteractionOwner::Assistant),
        }),
    ) {
        log::warn!("[RealtimePassive] Failed to emit voice_error: {}", e);
    }
}

/// Handles cancellation event and resets state machine to Ready.
fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!(
        "[RealtimePassive] Interaction cancelled on turn {}",
        turn_id
    );
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
        VoxEvent::Interrupted { .. } => on_interrupt(app, state, playback),
        VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
