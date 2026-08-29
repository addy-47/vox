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
