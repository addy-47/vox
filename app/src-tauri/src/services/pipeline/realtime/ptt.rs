use super::super::{
    transition, RoutingContext, END_REASON_USER, EVENT_LLM_TOKEN, EVENT_PIPELINE_ERROR,
    EVENT_PLAYBACK_FINISHED, EVENT_PLAYBACK_STARTED, EVENT_PTT_STATUS, EVENT_SESSION_ENDED,
    EVENT_SESSION_STARTED, EVENT_SPEECH_END, EVENT_SPEECH_START, EVENT_TRANSCRIPT_FINAL,
    STATUS_IDLE, STATUS_PROCESSING, STATUS_RECORDING, WINDOW_MAIN,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand};
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::engine::RealtimeEngine;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static SPEECH_DETECTED: AtomicBool = AtomicBool::new(false);
static REALTIME_PTT_BUFFER: Mutex<Vec<i16>> = Mutex::new(Vec::new());
static CURRENT_ASSISTANT_RESPONSE: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));

/// Ingests f32 audio samples into the realtime Push-To-Talk buffer when recording is active.
pub fn ingest_audio(chunk: &[f32]) {
    if IS_RECORDING.load(Ordering::Relaxed) {
        let i16_samples: Vec<i16> = chunk
            .iter()
            .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        REALTIME_PTT_BUFFER.lock().extend_from_slice(&i16_samples);
    }
}

/// Returns true if Push-To-Talk audio recording is currently active.
pub fn is_recording() -> bool {
    IS_RECORDING.load(Ordering::Relaxed)
}

/// Returns the current sample count in the realtime Push-To-Talk buffer.
pub fn get_buffer_len() -> usize {
    REALTIME_PTT_BUFFER.lock().len()
}

/// Starts a user-gated real-time Push-To-Talk speech-to-speech assistant session.
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

    state.pipeline.is_engaged.store(true, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    *rt_guard = Some(rt_engine);

    let prompt = state.settings.read().unwrap().persona.realtime_prompt.clone();
    super::super::init_new_session(state, &prompt).await;

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SESSION_STARTED, 0) {
        log::warn!("[RealtimePTT] Failed to emit session_started: {}", e);
    }

    log::info!("[RealtimePTT] Realtime PTT session started");
    Ok(())
}

/// Ends the active real-time Push-To-Talk speech-to-speech assistant session.
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if let Some(engine) = state.engine.lock().await.as_ref() {
        if let Err(e) = engine.vad_tx.send(VadCommand::StopRealtime) {
            log::warn!("[RealtimePTT] Failed to send StopRealtime: {}", e);
        }
        engine.playback_engine.cancel();
    }

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut rt_engine) = rt_guard.take() {
        rt_engine.stop();
    }

    IS_RECORDING.store(false, Ordering::Relaxed);
    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    REALTIME_PTT_BUFFER.lock().clear();

    state.pipeline.is_engaged.store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

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

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SESSION_ENDED, END_REASON_USER.to_string()) {
        log::warn!("[RealtimePTT] Failed to emit session_ended: {}", e);
    }

    log::info!("[RealtimePTT] Realtime PTT session ended");
    Ok(())
}

/// Initiates real-time Push-To-Talk speech streaming and interrupts ongoing playback and cloud response.
pub fn handle_ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if IS_RECORDING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    REALTIME_PTT_BUFFER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            super::session::realtime_barge_in(state, &engine.playback_engine);
        }
    }

    let turn_id = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({
            "state": STATUS_RECORDING,
            "turn_id": turn_id,
        }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit ptt_status RECORDING: {}", e);
    }

    log::info!("[RealtimePTT] PTT recording started with cloud interrupt (Turn: {})", turn_id);
    Ok(())
}

/// Finalizes real-time Push-To-Talk recording with silence gating and ghost audio protection.
pub fn handle_ptt_stop<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if !IS_RECORDING.swap(false, Ordering::SeqCst) {
        return Ok(());
    }

    let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);

    if !SPEECH_DETECTED.load(Ordering::Relaxed) {
        REALTIME_PTT_BUFFER.lock().clear();
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        if let Err(e) = app.emit_to(
            WINDOW_MAIN,
            EVENT_PTT_STATUS,
            serde_json::json!({ "state": STATUS_IDLE }),
        ) {
            log::warn!("[RealtimePTT] Failed to emit ptt_status IDLE: {}", e);
        }
        log::info!(
            "[RealtimePTT] Non-speech PTT hold discarded without cloud request (Turn: {})",
            turn_id
        );
        return Ok(());
    }

    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    let buffer = REALTIME_PTT_BUFFER.lock().split_off(0);
    if let Ok(guard) = state.realtime_engine.try_lock() {
        if let Some(ref rt_engine) = *guard {
            rt_engine.push_audio(&buffer);
        }
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({
            "state": STATUS_PROCESSING,
            "turn_id": turn_id,
        }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit ptt_status PROCESSING: {}", e);
    }

    log::info!("[RealtimePTT] PTT recording finalized (Turn: {})", turn_id);
    Ok(())
}

/// Cancels an in-progress real-time Push-To-Talk stream and resets state to idle.
pub fn handle_ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    IS_RECORDING.store(false, Ordering::Relaxed);
    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    REALTIME_PTT_BUFFER.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({ "state": STATUS_IDLE }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit ptt_status IDLE: {}", e);
    }

    log::info!("[RealtimePTT] PTT recording cancelled");
    Ok(())
}

/// Handles speech onset detection and marks speech active for PTT gating.
pub fn on_speech_start<R: tauri::Runtime>(
    turn_id: u32,
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    super::session::realtime_barge_in(state, playback);

    if IS_RECORDING.load(Ordering::Relaxed) {
        SPEECH_DETECTED.store(true, Ordering::Relaxed);
    }

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SPEECH_START, turn_id) {
        log::warn!("[RealtimePTT] Failed to emit speech_start: {}", e);
    }
}

/// Handles speech end detection and buffers captured speech audio.
pub fn on_speech_end<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, audio: Vec<f32>) {
    if IS_RECORDING.load(Ordering::Relaxed) {
        let i16_samples: Vec<i16> = audio
            .iter()
            .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        REALTIME_PTT_BUFFER.lock().extend_from_slice(&i16_samples);
    }

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_SPEECH_END, turn_id) {
        log::warn!("[RealtimePTT] Failed to emit speech_end: {}", e);
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
        log::warn!("[RealtimePTT] Failed to emit transcript_final: {}", e);
    }

    CURRENT_ASSISTANT_RESPONSE.lock().clear();
    state.conversation_manager.lock().push_user_turn(text);
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
fn on_playback_started<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PLAYBACK_STARTED, turn_id) {
        log::warn!("[RealtimePTT] Failed to emit playback_started: {}", e);
    }
}

/// Transitions pipeline state back to idle resting state upon playback completion.
fn on_playback_finished<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let full_text = CURRENT_ASSISTANT_RESPONSE.lock().split_off(0);
    if !full_text.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(full_text);
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to(WINDOW_MAIN, EVENT_PLAYBACK_FINISHED, turn_id) {
        log::warn!("[RealtimePTT] Failed to emit playback_finished: {}", e);
    }

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({ "state": STATUS_IDLE }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit ptt_status IDLE: {}", e);
    }
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

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PTT_STATUS,
        serde_json::json!({ "state": STATUS_IDLE }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit ptt_status IDLE: {}", e);
    }
}

/// Main event dispatcher for the realtime Push-To-Talk pipeline domain.
pub fn handle_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
        VoxEvent::SpeechStart { turn_id } => {
            on_speech_start(turn_id, app, state, playback)
        }
        VoxEvent::SpeechEnd {
            turn_id,
            audio_buffer,
        } => on_speech_end(turn_id, app, audio_buffer),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::LlmToken { turn_id, token } => on_llm_token(turn_id, token, app),
        VoxEvent::PlaybackStarted { turn_id } => on_playback_started(turn_id, app, state),
        VoxEvent::PlaybackFinished { turn_id } => on_playback_finished(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
