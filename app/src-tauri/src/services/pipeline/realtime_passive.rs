use super::{transition, RoutingContext};
use crate::core::events::VoxEvent;
use crate::core::settings::RealtimeProviderKind;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand};
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::engine::RealtimeEngine;
use crate::services::realtime::providers::deepgram_live::DeepgramVoiceAgentProvider;
use crate::services::realtime::providers::gemini_live::GeminiLiveProvider;
use crate::services::realtime::RealtimeVoiceProvider;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

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
        let _ = vad_tx.send(VadCommand::StopRealtime);
    }

    let provider = create_realtime_provider(state)?;
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

    let _ = vad_tx.send(VadCommand::StartRealtime {
        tx: audio_tx,
        is_ptt: false,
    });

    state.pipeline.is_engaged.store(true, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    state.pipeline.is_paused.store(false, Ordering::Relaxed);
    *rt_guard = Some(rt_engine);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "session_started", 0) {
        log::warn!("[RealtimePassive] Failed to emit session_started: {}", e);
    }

    log::info!("[RealtimePassive] Realtime passive session started");
    Ok(())
}

/// Pauses the active real-time voice pipeline.
pub async fn pause_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    state.pipeline.is_paused.store(true, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Paused, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "pipeline_paused", ()) {
        log::warn!("[RealtimePassive] Failed to emit pipeline_paused: {}", e);
    }

    log::info!("[RealtimePassive] Realtime passive session paused");
    Ok(())
}

/// Resumes a paused real-time voice pipeline.
pub async fn resume_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    state.pipeline.is_paused.store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "pipeline_resumed", ()) {
        log::warn!("[RealtimePassive] Failed to emit pipeline_resumed: {}", e);
    }

    log::info!("[RealtimePassive] Realtime passive session resumed");
    Ok(())
}

/// Ends the active real-time speech-to-speech session and tears down the WebSocket connection.
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let _ = engine.vad_tx.send(VadCommand::StopRealtime);
        engine.playback_engine.cancel();
    }

    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut rt_engine) = rt_guard.take() {
        rt_engine.stop();
    }

    state.pipeline.is_engaged.store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

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

    if let Err(e) = app.emit_to("main", "session_ended", "user".to_string()) {
        log::warn!("[RealtimePassive] Failed to emit session_ended: {}", e);
    }

    log::info!("[RealtimePassive] Realtime passive session ended");
    Ok(())
}

/// Handles user speech detection and transitions state machine to speaking.
fn on_speech_start(
    turn_id: u32,
    app: &AppHandle,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed)
        || state.pipeline.is_paused.load(Ordering::Relaxed)
    {
        return;
    }

    playback.cancel();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "speech_start", turn_id) {
        log::warn!("[RealtimePassive] Failed to emit speech_start: {}", e);
    }
}

/// Handles speech end and transitions state machine to thinking.
fn on_speech_end(turn_id: u32, app: &AppHandle, state: &AppState) {
    if !state.pipeline.is_engaged.load(Ordering::Relaxed)
        || state.pipeline.is_paused.load(Ordering::Relaxed)
    {
        return;
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "speech_end", turn_id) {
        log::warn!("[RealtimePassive] Failed to emit speech_end: {}", e);
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
        log::warn!("[RealtimePassive] Failed to emit transcript_final: {}", e);
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
        log::warn!("[RealtimePassive] Failed to emit llm_token: {}", e);
    }
}

/// Transitions pipeline state to assistant speaking when audio playback begins.
fn on_playback_started(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "playback_started", turn_id) {
        log::warn!("[RealtimePassive] Failed to emit playback_started: {}", e);
    }
}

/// Transitions pipeline state back to listening upon playback completion.
fn on_playback_finished(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    if let Err(e) = app.emit_to("main", "playback_finished", turn_id) {
        log::warn!("[RealtimePassive] Failed to emit playback_finished: {}", e);
    }
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error(turn_id: u32, message: String, app: &AppHandle, state: &AppState) {
    log::error!("[RealtimePassive] Error on turn {}: {}", turn_id, message);
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
        log::warn!("[RealtimePassive] Failed to emit pipeline_error: {}", e);
    }
}

/// Main event dispatcher for the realtime passive pipeline domain.
pub fn handle_event(
    app: &AppHandle,
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
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
