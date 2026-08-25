use super::{transition, RoutingContext};
use crate::core::events::VoxEvent;
use crate::core::settings::RealtimeProviderKind;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand};
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::engine::RealtimeEngine;
use crate::services::realtime::providers::deepgram_live::DeepgramVoiceAgentProvider;
use crate::services::realtime::providers::gemini_live::GeminiLiveProvider;
use crate::services::realtime::RealtimeVoiceProvider;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static SPEECH_DETECTED: AtomicBool = AtomicBool::new(false);
static REALTIME_PTT_BUFFER: Mutex<Vec<i16>> = Mutex::new(Vec::new());

/// Instantiates the configured cloud real-time voice provider.
fn create_realtime_provider(state: &AppState) -> Result<Box<dyn RealtimeVoiceProvider>, String> {
    let settings = state.settings.read().unwrap().clone();
    match settings.realtime.active {
        RealtimeProviderKind::GeminiLive => Ok(Box::new(GeminiLiveProvider::new(
            settings.realtime.gemini_live.clone(),
            settings.persona.realtime_prompt.clone(),
            state.pipeline.is_paused.clone(),
        ))),
        RealtimeProviderKind::DeepgramVoiceAgent => Ok(Box::new(DeepgramVoiceAgentProvider::new(
            settings.realtime.deepgram_voice_agent.clone(),
            settings.persona.realtime_prompt.clone(),
            state.pipeline.is_paused.clone(),
        ))),
        RealtimeProviderKind::OpenAiRealtime => {
            Err("OpenAI Realtime provider is not implemented".to_string())
        }
        RealtimeProviderKind::ElevenLabsConvai => {
            Err("ElevenLabs Conversational AI provider is not implemented".to_string())
        }
    }
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

    let provider = create_realtime_provider(state)?;
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

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "session_started", 0) {
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

    if let Err(e) = app.emit_to("main", "session_ended", "user".to_string()) {
        log::warn!("[RealtimePTT] Failed to emit session_ended: {}", e);
    }

    log::info!("[RealtimePTT] Realtime PTT session ended");
    Ok(())
}

/// Initiates real-time Push-To-Talk speech streaming and interrupts ongoing playback.
pub fn handle_ptt_start(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if IS_RECORDING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    REALTIME_PTT_BUFFER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

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
        log::warn!("[RealtimePTT] Failed to emit ptt_status RECORDING: {}", e);
    }

    log::info!("[RealtimePTT] PTT recording started (Turn: {})", turn_id);
    Ok(())
}

/// Finalizes real-time Push-To-Talk recording with silence gating and ghost audio protection.
pub fn handle_ptt_stop(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if !IS_RECORDING.swap(false, Ordering::SeqCst) {
        return Ok(());
    }

    let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);

    if !SPEECH_DETECTED.load(Ordering::Relaxed) {
        REALTIME_PTT_BUFFER.lock().clear();
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        let _ = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" }));
        log::info!(
            "[RealtimePTT] Non-speech PTT hold discarded without cloud request (Turn: {})",
            turn_id
        );
        return Ok(());
    }

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
        "main",
        "ptt_status",
        serde_json::json!({
            "state": "PROCESSING",
            "turn_id": turn_id,
        }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit ptt_status PROCESSING: {}", e);
    }

    log::info!("[RealtimePTT] PTT recording finalized (Turn: {})", turn_id);
    Ok(())
}

/// Cancels an in-progress real-time Push-To-Talk stream and resets state to idle.
pub fn handle_ptt_cancel(app: &AppHandle, state: &AppState) -> Result<(), String> {
    IS_RECORDING.store(false, Ordering::Relaxed);
    SPEECH_DETECTED.store(false, Ordering::Relaxed);
    REALTIME_PTT_BUFFER.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" })) {
        log::warn!("[RealtimePTT] Failed to emit ptt_status IDLE: {}", e);
    }

    log::info!("[RealtimePTT] PTT recording cancelled");
    Ok(())
}

/// Handles speech onset detection and marks speech active for PTT gating.
fn on_speech_start(turn_id: u32, app: &AppHandle, playback: &Arc<PlaybackEngine>) {
    playback.cancel();
    if IS_RECORDING.load(Ordering::Relaxed) {
        SPEECH_DETECTED.store(true, Ordering::Relaxed);
    }

    if let Err(e) = app.emit_to("main", "speech_start", turn_id) {
        log::warn!("[RealtimePTT] Failed to emit speech_start: {}", e);
    }
}

/// Handles speech end detection and buffers captured speech audio.
fn on_speech_end(turn_id: u32, app: &AppHandle, audio: Vec<f32>) {
    if IS_RECORDING.load(Ordering::Relaxed) {
        let i16_samples: Vec<i16> = audio
            .iter()
            .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        REALTIME_PTT_BUFFER.lock().extend_from_slice(&i16_samples);
    }

    if let Err(e) = app.emit_to("main", "speech_end", turn_id) {
        log::warn!("[RealtimePTT] Failed to emit speech_end: {}", e);
    }
}

/// Handles incoming final transcription from the real-time server.
fn on_transcript_final(turn_id: u32, text: String, app: &AppHandle, state: &AppState) {
    if let Err(e) = app.emit_to(
        "main",
        "transcript_final",
        serde_json::json!({
            "turn_id": turn_id,
            "text": text,
        }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit transcript_final: {}", e);
    }

    state.conversation_manager.lock().push_user_turn(text);
}

/// Handles streamed token delta from the real-time server.
fn on_llm_token(turn_id: u32, token: String, app: &AppHandle) {
    if let Err(e) = app.emit_to(
        "main",
        "llm_token",
        serde_json::json!({
            "turn_id": turn_id,
            "token": token,
        }),
    ) {
        log::warn!("[RealtimePTT] Failed to emit llm_token: {}", e);
    }
}

/// Transitions pipeline state to assistant speaking when audio playback begins.
fn on_playback_started(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "playback_started", turn_id) {
        log::warn!("[RealtimePTT] Failed to emit playback_started: {}", e);
    }
}

/// Transitions pipeline state back to idle resting state upon playback completion.
fn on_playback_finished(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "playback_finished", turn_id) {
        log::warn!("[RealtimePTT] Failed to emit playback_finished: {}", e);
    }

    let _ = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" }));
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error(turn_id: u32, message: String, app: &AppHandle, state: &AppState) {
    log::error!("[RealtimePTT] Error on turn {}: {}", turn_id, message);
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
        log::warn!("[RealtimePTT] Failed to emit pipeline_error: {}", e);
    }

    let _ = app.emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" }));
}

/// Main event dispatcher for the realtime Push-To-Talk pipeline domain.
pub fn handle_event(
    app: &AppHandle,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
        VoxEvent::SpeechStart { turn_id } => on_speech_start(turn_id, app, playback),
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
