use super::super::{
    transition, RoutingContext, EVENT_LLM_TOKEN, EVENT_PIPELINE_ERROR,
    EVENT_TRANSCRIPT_FINAL, WINDOW_MAIN,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand};
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::engine::RealtimeEngine;
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter, Manager};

static REALTIME_PTT_BUFFER: Mutex<Vec<i16>> = Mutex::new(Vec::new());
static CURRENT_ASSISTANT_RESPONSE: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));
static CURRENT_USER_TRANSCRIPT: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));

/// Ingests f32 audio samples into the realtime Push-To-Talk buffer when recording is active.
pub fn ingest_audio(chunk: &[f32], state: &AppState) {
    if state.pipeline.state() == InteractionState::Listening {
        let i16_samples: Vec<i16> = chunk
            .iter()
            .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        REALTIME_PTT_BUFFER.lock().extend_from_slice(&i16_samples);
    }
}

/// Returns the current sample count in the realtime Push-To-Talk buffer.
pub fn get_buffer_len() -> usize {
    REALTIME_PTT_BUFFER.lock().len()
}

/// Starts a user-gated real-time Push-To-Talk speech-to-speech assistant session.
pub async fn start_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    crate::core::start_audio_engine(app, state).await?;

    let (vad_tx, playback_engine, pipeline_tx) = {
        let guard = state.engine.lock().await;
        let eng = guard.as_ref().ok_or("Audio engine not ready")?;
        (
            eng.vad_tx.clone(),
            eng.playback_engine.clone(),
            eng.pipeline_tx.clone(),
        )
    };

    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut old_rt) = rt_guard.take() {
        old_rt.stop();
        if let Err(e) = vad_tx.send(VadCommand::StopRealtime) {
            log::warn!("[RealtimePTT] Failed to send StopRealtime: {}", e);
        }
    }

    let provider = super::session::create_realtime_provider(state)?;
    let tokio_handle = tokio::runtime::Handle::current();
    let mut rt_engine = RealtimeEngine::new(provider, tokio_handle);

    rt_engine
        .start(
            crate::core::settings::InteractionMode::PTT,
            playback_engine,
            pipeline_tx,
        )
        .map_err(|e| format!("[RealtimePTT] Engine start failed: {}", e))?;

    let audio_tx = rt_engine
        .get_audio_sender()
        .ok_or("Failed to obtain realtime audio sender")?;

    if let Err(e) = vad_tx.send(VadCommand::StartRealtime {
        tx: audio_tx,
        is_ptt: true,
    }) {
        log::warn!("[RealtimePTT] Failed to send StartRealtime to VAD: {}", e);
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    *rt_guard = Some(rt_engine);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let conv_id = now;
    state.conversation_id.store(conv_id, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    let state_arc = app.state::<std::sync::Arc<AppState>>().inner().clone();
    super::session::spawn_realtime_idle_monitor(app.clone(), state_arc);

    log::info!("[RealtimePTT] Realtime PTT session started (ID: {})", conv_id);
    Ok(())
}

/// Ends the active real-time Push-To-Talk voice assistant session.
pub async fn end_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if let Err(e) = engine.vad_tx.send(VadCommand::StopRealtime) {
                log::warn!("[RealtimePTT] Failed to send StopRealtime to VAD: {}", e);
            }
        }
    }

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut rt_engine) = rt_guard.take() {
        rt_engine.stop();
    }

    REALTIME_PTT_BUFFER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    if conv_id != 0 {
        let mem_lock = state.memory_tx.lock();
        if let Some(ref tx) = *mem_lock {
            if let Err(e) = tx.try_send(crate::persistence::memory_worker::MemoryWorkerEvent::SessionEnd {
                session_id: conv_id.to_string(),
                summary: String::new(),
            }) {
                log::trace!("[RealtimePTT] Failed to send SessionEnd to memory worker: {}", e);
            }
        }
    }

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
                log::warn!("[RealtimePTT] Failed to send SessionEnded to persist: {}", e);
            }
        }
    }

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
        drop(guard);
        crate::core::stop_audio_engine(state).await?;
    } else {
        crate::core::stop_audio_engine(state).await?;
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Idle, &ctx, app, state);

    log::info!("[RealtimePTT] Realtime PTT session ended");
    Ok(())
}

/// Initiates real-time Push-To-Talk speech streaming and interrupts ongoing playback and cloud response.
pub fn ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if state.pipeline.state() == InteractionState::Idle {
        return Err("Realtime PTT session not active".to_string());
    }

    REALTIME_PTT_BUFFER.lock().clear();
    state.pipeline.renew_turn_token();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            super::session::realtime_barge_in(state, &engine.playback_engine);
            if let Err(e) = engine.vad_tx.send(VadCommand::StartWindowValidation) {
                log::warn!("[RealtimePTT] Failed to send StartWindowValidation to VAD: {}", e);
            }
        }
    }

    let turn_id = state.pipeline.next_turn_id();
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    log::info!("[RealtimePTT] PTT recording started with cloud interrupt (Turn: {})", turn_id);
    Ok(())
}

/// Finalizes real-time Push-To-Talk recording with silence gating and ghost audio protection.
pub fn ptt_stop<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if state.pipeline.state() != InteractionState::Listening {
        return Ok(());
    }

    let turn_id = state.pipeline.peek_turn_id();
    let raw_buffer = REALTIME_PTT_BUFFER.lock().split_off(0);

    if raw_buffer.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return Ok(());
    }

    let validation_result = if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            let (tx, rx) = std::sync::mpsc::channel();
            if engine.vad_tx.send(VadCommand::StopWindowValidation { response_tx: tx }).is_ok() {
                rx.recv_timeout(std::time::Duration::from_millis(500)).ok()
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let is_speech = match validation_result {
        Some(ref val) => val.is_speech_detected,
        None => true,
    };

    if !is_speech {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        log::info!(
            "[RealtimePTT] Non-speech PTT hold discarded without cloud request (Turn: {})",
            turn_id
        );
        return Ok(());
    }

    let buffer_to_send = match validation_result {
        Some(ref val) => {
            let start = val.speech_start_sample.min(raw_buffer.len());
            let end = val.speech_end_sample.min(raw_buffer.len());
            if start < end && (end - start) >= 256 {
                raw_buffer[start..end].to_vec()
            } else {
                raw_buffer
            }
        }
        None => raw_buffer,
    };

    if let Ok(guard) = state.realtime_engine.try_lock() {
        if let Some(ref rt_engine) = *guard {
            rt_engine.push_audio(&buffer_to_send);
        }
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    log::info!("[RealtimePTT] PTT recording finalized (Turn: {})", turn_id);
    Ok(())
}

/// Cancels an in-progress real-time Push-To-Talk stream and resets state to ready.
pub fn ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if state.pipeline.state() != InteractionState::Listening {
        return Ok(());
    }

    let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);
    REALTIME_PTT_BUFFER.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            let (tx, _) = std::sync::mpsc::channel();
            let _ = engine.vad_tx.send(VadCommand::StopWindowValidation { response_tx: tx });
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    log::info!("[RealtimePTT] PTT recording cancelled (Turn: {})", turn_id);
    Ok(())
}

/// Handles speech onset detection and marks speech active for PTT gating.
pub fn on_speech_start(
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    super::session::realtime_barge_in(state, playback);
    state.conversation_manager.lock().on_speech_start();
}

/// Handles speech end detection.
pub fn on_speech_end(audio: Vec<f32>) {
    drop(audio);
}

/// Handles incoming final transcription from the real-time server.
fn on_transcript_final<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>) {
    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_TRANSCRIPT_FINAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": text,
        }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit transcript_final: {}", e);
    }

    CURRENT_ASSISTANT_RESPONSE.lock().clear();
    *CURRENT_USER_TRANSCRIPT.lock() = text;
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
        log::warn!("[RealtimePTT] Failed to emit llm_token: {}", e);
    }
}

/// Transitions pipeline state to assistant speaking when audio playback begins.
fn on_playback_started<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);
}

/// Transitions pipeline state back to ready state upon playback completion.
fn on_playback_finished<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    let full_text = CURRENT_ASSISTANT_RESPONSE.lock().split_off(0);
    if !full_text.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(full_text);
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error<R: tauri::Runtime>(turn_id: u32, message: String, app: &AppHandle<R>, state: &AppState) {
    log::error!("[RealtimePTT] Error on turn {}: {}", turn_id, message);
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
        log::warn!("[RealtimePTT] Failed to emit pipeline_error: {}", e);
    }
}

/// Handles cancellation event and resets state machine to Ready.
fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!("[RealtimePTT] Interaction cancelled on turn {}", turn_id);
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
        VoxEvent::SpeechStart { .. } => {
            on_speech_start(state, playback)
        }
        VoxEvent::SpeechEnd {
            audio_buffer,
            ..
        } => on_speech_end(audio_buffer),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app)
        }
        VoxEvent::LlmToken { turn_id, token } => on_llm_token(turn_id, token, app),
        VoxEvent::PlaybackStarted { .. } => on_playback_started(app, state),
        VoxEvent::PlaybackFinished { .. } => on_playback_finished(app, state),
        VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
