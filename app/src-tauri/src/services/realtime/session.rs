use crate::{
    core::{settings::RealtimeProviderKind, state::AppState},
    services::realtime::{
        providers::{DeepgramVoiceAgentProvider, GeminiLiveProvider},
        RealtimeVoiceProvider, SESSION_CACHE_FILENAME,
    },
    utils::paths::cache_dir,
};

/// Instantiates the configured cloud real-time voice provider.
pub fn create_realtime_provider(
    state: &AppState,
) -> Result<Box<dyn RealtimeVoiceProvider>, String> {
    let mut settings = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let assembled_prompt = state.conversation_manager.lock().assemble_system_prompt();

    // Check cached session resumption token with 2-hour TTL
    let cache_path = cache_dir().join(SESSION_CACHE_FILENAME);
    let mut cached_handle = None;
    if cache_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<serde_json::Value>(&data) {
                let expires_at = cached["expires_at"].as_u64().unwrap_or(0);
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                if now_ms < expires_at {
                    if let Some(handle) = cached["handle"].as_str() {
                        log::info!(
                            "[RealtimeSession] Found valid unexpired session resumption token."
                        );
                        cached_handle = Some(handle.to_string());
                    }
                } else {
                    log::info!("[RealtimeSession] Cached session resumption token expired (>2 hours). Purging...");
                    purge_session_cache();
                }
            }
        }
    }

    if let Some(handle) = cached_handle {
        settings.realtime.gemini_live.resume_handle = Some(handle);
    }

    match settings.realtime.active {
        RealtimeProviderKind::GeminiLive => Ok(Box::new(GeminiLiveProvider::new(
            settings.realtime.gemini_live.clone(),
            assembled_prompt,
            state.pipeline.state_rx.clone(),
            state.pipeline.turn_id.clone(),
        ))),
        RealtimeProviderKind::DeepgramVoiceAgent => Ok(Box::new(DeepgramVoiceAgentProvider::new(
            settings.realtime.deepgram_voice_agent.clone(),
            assembled_prompt,
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

/// Explicitly purges the disk cache file containing the session resumption token.
pub fn purge_session_cache() {
    let cache_path = cache_dir().join(SESSION_CACHE_FILENAME);
    if cache_path.exists() {
        if let Err(e) = std::fs::remove_file(&cache_path) {
            log::warn!(
                "[RealtimeSession] Failed to delete session cache file: {}",
                e
            );
        } else {
            log::info!("[RealtimeSession] Purged session cache file.");
        }
    }
}
