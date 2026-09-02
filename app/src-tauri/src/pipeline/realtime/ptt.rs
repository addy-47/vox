use super::super::{transition, RoutingContext, WINDOW_MAIN};
use crate::core::events::VoiceErrorPayload;
use crate::core::events::{emit_ipc_to, IpcEvent, LlmTokenPayload, TranscriptPayload, VoxEvent};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::RealtimeActor;
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

/// Starts a user-gated real-time Push-To-Talk speech-to-speech assistant session.
pub async fn start_session<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let (vad_tx, playback_engine, pipeline_tx) = {
        let guard = state.engine.lock().await;
        let eng = guard.as_ref().ok_or("Audio engine not ready")?;
        (
            eng.vad_tx.clone(),
            eng.playback_engine.clone(),
            eng.pipeline_tx.clone(),
        )
    };

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut old_rt) = rt_guard.take() {
        old_rt.stop();
        if let Err(e) = vad_tx.send(VadCommand::StopRealtime) {
            log::warn!("[RealtimePTT] Failed to send StopRealtime: {}", e);
        }
    }

    let provider = super::session::create_realtime_provider(state)?;
    let tokio_handle = tokio::runtime::Handle::current();
    let mut rt_actor = RealtimeActor::new(provider, tokio_handle);

    rt_actor
        .start(
            crate::core::settings::InteractionMode::PTT,
            playback_engine,
            pipeline_tx,
        )
        .map_err(|e| format!("[RealtimePTT] Actor start failed: {}", e))?;

    let audio_tx = rt_actor
        .get_audio_sender()
        .ok_or("Failed to obtain realtime audio sender")?;

    if let Err(e) = vad_tx.send(VadCommand::StartRealtime {
        tx: audio_tx,
        is_ptt: true,
    }) {
        log::warn!("[RealtimePTT] Failed to send StartRealtime to VAD: {}", e);
    }

    *rt_guard = Some(rt_actor);
    ACCUMULATOR.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    log::info!(
        "[RealtimePTT] Realtime PTT session started (ID: {})",
        conv_id
    );
    Ok(())
}

/// Ends the active real-time Push-To-Talk voice assistant session.
pub async fn end_session<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if let Err(e) = engine.vad_tx.send(VadCommand::StopRealtime) {
                log::warn!("[RealtimePTT] Failed to send StopRealtime to VAD: {}", e);
            }
        }
    }

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut rt_actor) = rt_guard.take() {
        rt_actor.stop();
    }

    ACCUMULATOR.lock().clear();
    super::session::purge_session_cache();

    log::info!("[RealtimePTT] Realtime PTT session ended");
    Ok(())
}

/// Handles dedicated barge-in interruption for realtime PTT pipeline.
pub fn on_interrupt<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    playback.cancel();

    if let Ok(rt_guard) = state.realtime_engine.try_lock() {
        if let Some(ref rt_actor) = *rt_guard {
            if let Err(e) = rt_actor.signal_interrupt() {
                log::warn!(
                    "[RealtimePTT] Error sending interrupt signal to realtime actor: {}",
                    e
                );
            }
        }
    }

    state
        .pipeline
        .pending_synthesis_jobs
        .store(0, Ordering::Relaxed);
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
                "[RealtimePTT] Failed to send TurnCompleted on interrupt: {}",
                e
            );
        }
    }

    ACCUMULATOR.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);
    log::info!(
        "[RealtimePTT] Interruption handled (turn: {})",
        interrupted_turn_id
    );
}

/// Initiates real-time Push-To-Talk speech streaming and interrupts ongoing playback and cloud response.
pub fn ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        return Err("Realtime PTT session not active".to_string());
    }

    if current_state == InteractionState::Thinking || current_state == InteractionState::Speaking {
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                on_interrupt(app, state, &engine.playback_engine);
            }
        }
    }

    let (turn_id, _) = state.pipeline.next_turn();
    ACCUMULATOR.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if let Err(e) = engine.vad_tx.send(VadCommand::StartWindowValidation) {
                log::warn!(
                    "[RealtimePTT] Failed to send StartWindowValidation to VAD: {}",
                    e
                );
            }
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    log::info!(
        "[RealtimePTT] PTT recording started with cloud interrupt (Turn: {})",
        turn_id
    );
    Ok(())
}

/// Finalizes a real-time Push-To-Talk speech turn and streams audio buffer to the provider.
pub async fn ptt_stop<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    if state.pipeline.state() != InteractionState::Listening {
        return Ok(());
    }

    let turn_id = state.pipeline.peek_turn_id();

    let vad_tx_opt = {
        let guard = state.engine.lock().await;
        guard.as_ref().map(|e| e.vad_tx.clone())
    };

    let validation_result = if let Some(vad_tx) = vad_tx_opt {
        let (tx, rx) = std::sync::mpsc::channel();
        if vad_tx
            .send(VadCommand::StopWindowValidation { response_tx: tx })
            .is_ok()
        {
            rx.recv_timeout(std::time::Duration::from_millis(
                crate::services::vad::VAD_VALIDATION_TIMEOUT_MS,
            ))
            .ok()
        } else {
            None
        }
    } else {
        None
    };

    let (is_speech, audio) = match validation_result {
        Some(val) => (val.is_speech_detected, val.audio),
        None => (false, Vec::new()),
    };

    if !is_speech || audio.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        log::info!(
            "[RealtimePTT] Non-speech PTT hold discarded without cloud request (Turn: {})",
            turn_id
        );
        return Ok(());
    }

    let i16_samples: Vec<i16> = audio
        .iter()
        .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect();

    let rt_guard = state.realtime_engine.lock().await;
    if let Some(ref rt_actor) = *rt_guard {
        if let Err(e) = rt_actor.signal_speech_committed(&i16_samples) {
            log::warn!("[RealtimePTT] Failed to commit speech turn: {}", e);
        }
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    log::info!(
        "[RealtimePTT] PTT recording finalized and streamed to cloud (Turn: {})",
        turn_id
    );
    Ok(())
}

/// Cancels an in-progress real-time Push-To-Talk stream and resets state to ready.
pub fn ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if state.pipeline.state() != InteractionState::Listening {
        return Ok(());
    }

    state.pipeline.cancel_current_turn();
    ACCUMULATOR.lock().clear();
    let turn_id = state.pipeline.peek_turn_id();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            let (tx, _) = std::sync::mpsc::channel();
            let _ = engine
                .vad_tx
                .send(VadCommand::StopWindowValidation { response_tx: tx });
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    log::info!("[RealtimePTT] PTT recording cancelled (Turn: {})", turn_id);
    Ok(())
}

/// needs to know wht htis does , doenst make sense
/// Handles speech onset detection and marks speech active for PTT gating.
pub fn on_speech_start<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Thinking || current_state == InteractionState::Speaking {
        on_interrupt(app, state, playback);
    }
}

/// Handles incoming final transcription from the real-time server.
fn on_transcript_final<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    let transliterate_enabled = state
        .settings
        .read()
        .map(|s| s.stt.transliterate_enabled)
        .unwrap_or(false);
    let processed_text =
        crate::services::translit::transliterate_if_hi(&text, true, transliterate_enabled);

    if processed_text.trim().is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return;
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::TranscriptFinal(TranscriptPayload {
            turn_id,
            text: processed_text.clone(),
            owner: Some(InteractionOwner::Assistant),
        }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit transcript_final: {}", e);
    }

    let mut acc = ACCUMULATOR.lock();
    acc.clear();
    acc.set_user_transcript(processed_text);

    state
        .pipeline
        .pending_synthesis_jobs
        .store(1, Ordering::Relaxed);
}

/// Handles streamed token delta from the real-time server.
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
        log::warn!("[RealtimePTT] Failed to emit llm_token: {}", e);
    }
}

/// Finalizes assistant turn response in memory and persists to SQLite database.
fn on_llm_finished(turn_id: u32, state: &AppState, playback: &Arc<PlaybackEngine>) {
    state
        .pipeline
        .pending_synthesis_jobs
        .store(0, Ordering::Relaxed);
    playback.flush_pre_roll();

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
                    "[RealtimePTT] Failed to send TurnCompleted to persist: {}",
                    e
                );
            }
        }
    }
}

/// Transitions pipeline state to assistant speaking when audio playback begins.
fn on_playback_started<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    if state.pipeline.state() == InteractionState::Thinking {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Speaking, &ctx, app, state);
    }
}

/// Transitions pipeline state back to ready state upon playback completion.
fn on_playback_finished<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    if state.pipeline.state() == InteractionState::Speaking {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
    }
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error<R: tauri::Runtime>(
    turn_id: u32,
    message: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    log::error!("[RealtimePTT] Error on turn {}: {}", turn_id, message);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Error, &ctx, app, state);

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::VoiceError(VoiceErrorPayload {
            message,
            source: "RealtimePTT".to_string(),
            owner: Some(InteractionOwner::Assistant),
        }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit voice_error: {}", e);
    }
}

/// Resets turn accumulator and transitions state machine to ready on user cancellation.
fn on_cancelled<R: tauri::Runtime>(_turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    state
        .pipeline
        .pending_synthesis_jobs
        .store(0, Ordering::Relaxed);
    ACCUMULATOR.lock().clear();
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);
}

/// Main event dispatcher for the realtime Push-To-Talk pipeline domain.
pub fn handle_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
        VoxEvent::SpeechStart { .. } => on_speech_start(app, state, playback),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::LlmToken { turn_id, token } => on_llm_token(turn_id, token, app),
        VoxEvent::LlmFinished { turn_id } => on_llm_finished(turn_id, state, playback),
        VoxEvent::PlaybackStarted { .. } => on_playback_started(app, state),
        VoxEvent::PlaybackFinished { .. } => on_playback_finished(app, state),
        VoxEvent::Interrupted { .. } => on_interrupt(app, state, playback),
        VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
