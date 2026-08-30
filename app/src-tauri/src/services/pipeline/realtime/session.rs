use crate::core::settings::RealtimeProviderKind;
use crate::core::state::AppState;
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::providers::deepgram_live::DeepgramVoiceAgentProvider;
use crate::services::realtime::providers::gemini_live::GeminiLiveProvider;
use crate::services::realtime::RealtimeVoiceProvider;
use std::sync::Arc;

/// Instantiates the configured cloud real-time voice provider.
pub fn create_realtime_provider(state: &AppState) -> Result<Box<dyn RealtimeVoiceProvider>, String> {
    let settings = state.settings.read().unwrap_or_else(|p| p.into_inner()).clone();
    match settings.realtime.active {
        RealtimeProviderKind::GeminiLive => Ok(Box::new(GeminiLiveProvider::new(
            settings.realtime.gemini_live.clone(),
            settings.persona.realtime_prompt.clone(),
            state.pipeline.state_rx.clone(),
            state.pipeline.turn_id.clone(),
        ))),
        RealtimeProviderKind::DeepgramVoiceAgent => Ok(Box::new(DeepgramVoiceAgentProvider::new(
            settings.realtime.deepgram_voice_agent.clone(),
            settings.persona.realtime_prompt.clone(),
            state.pipeline.state_rx.clone(),
            state.pipeline.turn_id.clone(),
        ))),
        RealtimeProviderKind::OpenAiRealtime => {
            Err("OpenAI Realtime provider is not implemented".to_string())
        }
        RealtimeProviderKind::ElevenLabsConvai => {
            Err("ElevenLabs Conversational AI provider is not implemented".to_string())
        }
    }
}

/// Cancels realtime playback and issues provider barge-in signal if session is active.
pub fn realtime_barge_in(
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
) {
    if let Ok(rt_guard) = state.realtime_engine.try_lock() {
        if let Some(ref rt_engine) = *rt_guard {
            rt_engine.barge_in(playback);
            return;
        }
    }
    playback.cancel();
}

/// Spawns an idle observer for the realtime pipeline that auto-pauses the session
/// after 10 continuous minutes in the Ready state.
pub fn spawn_realtime_idle_monitor<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: std::sync::Arc<AppState>,
) {
    tauri::async_runtime::spawn(async move {
        let mut state_rx = state.pipeline.state_rx.clone();
        loop {
            if *state_rx.borrow() == crate::core::state::InteractionState::Ready {
                tokio::select! {
                    _ = tokio::time::sleep(crate::services::realtime::REALTIME_IDLE_TIMEOUT) => {
                        if state.pipeline.state() == crate::core::state::InteractionState::Ready {
                            log::info!("[Realtime] Auto-pausing session after 10 minutes of idle Ready state.");
                            if let Ok(mut guard) = state.realtime_engine.try_lock() {
                                if let Some(ref mut engine) = *guard {
                                    engine.stop();
                                }
                            }
                            let ctx = crate::services::pipeline::RoutingContext::from_app_state(&state);
                            crate::services::pipeline::transition(crate::core::state::InteractionState::Paused, &ctx, &app, &state);
                        }
                    }
                    res = state_rx.changed() => {
                        if res.is_err() {
                            break;
                        }
                    }
                }
            } else {
                if state_rx.changed().await.is_err() {
                    break;
                }
                if state.pipeline.state() == crate::core::state::InteractionState::Idle {
                    break;
                }
            }
        }
    });
}
