//! ============================================================================
//! src/ipc/pipeline/realtime.rs — Real-time WebSockets Speech-to-Speech (S2S) session IPC
//! ============================================================================

use crate::core::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[derive(serde::Serialize)]
pub struct RealtimeSessionCache {
    pub has_session: bool,
    pub provider: String,
    pub expires_in_seconds: i64,
    pub model: String,
}

pub async fn start_realtime_session_internal(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<(), String> {
    log::info!("[IPC] start_realtime_session_internal requested");

    // 1. Ensure engine is running
    let engine_guard = state.engine.lock().await;
    let engine = match &*engine_guard {
        Some(e) => e,
        None => {
            return Err("Audio engine is not running. Please start the engine first.".to_string())
        }
    };

    // Update owner to MainWindow and sync VAD immediately
    state.owner.store(
        crate::core::state::InteractionOwner::MainWindow as u32,
        std::sync::atomic::Ordering::Relaxed,
    );
    let _ = engine
        .vad_tx
        .send(crate::core::state::VadCommand::UpdateOwner(
            crate::core::state::InteractionOwner::MainWindow,
        ));

    // 2. Clear any existing realtime engine
    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut old_rt) = rt_guard.take() {
        log::info!("[IPC] Stopping existing realtime session...");
        old_rt.stop();
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::StopRealtime);
    }

    // 3. Load settings to determine active provider
    let settings = state.settings.read().unwrap().clone();
    let mut gemini_config = settings.realtime.gemini.clone();

    // Try to load resumption token from cache
    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
    let mut resumed = false;
    if cache_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<serde_json::Value>(&data) {
                let expires_at = cached["expires_at"].as_u64().unwrap_or(0);
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                let cached_model = cached["model"].as_str().unwrap_or("");
                let cached_handle = cached["handle"].as_str().unwrap_or("");

                if expires_at > now_ms && cached_model == gemini_config.model && !cached_handle.is_empty() {
                    log::info!(
                        "[Realtime] Found valid session resumption token, using handle: {}",
                        if cached_handle.len() > 12 {
                            &cached_handle[..12]
                        } else {
                            cached_handle
                        }
                    );
                    gemini_config.resume_handle = Some(cached_handle.to_string());
                    resumed = true;
                }
            }
        }
    }

    let provider: Box<dyn crate::services::realtime::RealtimeVoiceProvider> = match settings
        .realtime
        .provider
    {
        crate::core::settings::RealtimeProviderKind::GeminiLive => Box::new(
            crate::services::realtime::providers::gemini_live::GeminiLiveProvider::new(
                gemini_config,
                settings.assistant.realtime_prompt.clone(),
                state.pipeline.is_paused.clone(),
            ),
        ),
        crate::core::settings::RealtimeProviderKind::OpenAiRealtime => {
            return Err("OpenAI Realtime provider is not yet implemented".to_string());
        }
        crate::core::settings::RealtimeProviderKind::DeepgramVoiceAgent => Box::new(
            crate::services::realtime::providers::deepgram_live::DeepgramVoiceAgentProvider::new(
                settings.realtime.deepgram.clone(),
                settings.assistant.realtime_prompt.clone(),
                state.pipeline.is_paused.clone(),
            ),
        ),
        crate::core::settings::RealtimeProviderKind::ElevenLabsConvai => {
            return Err("ElevenLabs Conversational AI provider is not yet implemented".to_string());
        }
    };

    let owner: crate::core::state::InteractionOwner = state
        .owner
        .load(std::sync::atomic::Ordering::Relaxed)
        .into();
    let interaction_mode = match owner {
        crate::core::state::InteractionOwner::Tray => settings.interaction.tray_mode.clone(),
        crate::core::state::InteractionOwner::MainWindow
        | crate::core::state::InteractionOwner::Ptt => settings.interaction.main_app_mode.clone(),
        crate::core::state::InteractionOwner::Wizard => {
            crate::core::settings::InteractionMode::Passive
        }
    };

    // 4. Create and start RealtimeEngine
    let is_ptt = interaction_mode == crate::core::settings::InteractionMode::PTT;
    log::info!(
        "[IPC] Starting realtime session: interaction_mode={:?}, is_ptt={}",
        interaction_mode,
        is_ptt
    );
    let tokio_handle = tokio::runtime::Handle::current();
    let mut rt_engine =
        crate::services::realtime::engine::RealtimeEngine::new(provider, tokio_handle);

    let playback_engine = engine.playback_engine.clone();
    let event_tx = engine.pipeline_tx.clone();

    rt_engine
        .start(interaction_mode, playback_engine, event_tx)
        .map_err(|e| format!("Failed to start realtime session: {}", e))?;

    // Get the audio sender and wire it into the VAD actor
    let audio_tx = rt_engine
        .get_audio_sender()
        .ok_or_else(|| "Failed to get realtime audio sender".to_string())?;

    // 5. Update pipeline mode to Realtime in settings
    let current_settings = {
        let mut settings_write = state.settings.write().unwrap();
        settings_write.interaction.pipeline_mode = crate::core::settings::PipelineMode::Realtime;
        let _ = settings_write.save();
        settings_write.clone()
    };

    // 6. Propagate settings update to the pipeline event loop
    let _ = engine
        .pipeline_tx
        .send(crate::core::events::VoxEvent::SettingsUpdated(
            current_settings,
        ));

    // 7. Tell VAD to start routing chunks — pass is_ptt so it applies the
    //    correct routing policy (gated vs. unconditional).
    log::info!(
        "[IPC] Sending StartRealtime to VAD actor (is_ptt={})",
        is_ptt
    );
    let _ = engine
        .vad_tx
        .send(crate::core::state::VadCommand::StartRealtime { tx: audio_tx, is_ptt });

    // Update backend engagement state
    state.pipeline.is_engaged.store(true, std::sync::atomic::Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, std::sync::atomic::Ordering::Relaxed);

    *rt_guard = Some(rt_engine);

    // Spawn 10-minute active idle timeout check task
    let app_clone = app.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        const TIMEOUT_MS: u64 = 10 * 60 * 1000; // 10 minutes
        const WARN_1_MS: u64 = TIMEOUT_MS - 15_000; // 9m 45s (15s warning)
        const WARN_2_MS: u64 = TIMEOUT_MS - 5_000;  // 9m 55s (5s warning)

        loop {
            // Check if still engaged in realtime mode
            let is_engaged = state_clone.pipeline.is_engaged.load(std::sync::atomic::Ordering::Relaxed);
            let is_realtime = {
                let settings = state_clone.settings.read().unwrap();
                settings.interaction.pipeline_mode == crate::core::settings::PipelineMode::Realtime
            };
            
            if !is_engaged || !is_realtime {
                break;
            }

            // Get last activity time
            let last_activity = {
                let rt_guard = state_clone.realtime_engine.lock().await;
                if let Some(rt_engine) = &*rt_guard {
                    rt_engine.last_activity_time()
                } else {
                    0
                }
            };

            if last_activity > 0 {
                let now = chrono::Utc::now().timestamp_millis() as u64;
                let elapsed = now.saturating_sub(last_activity);

                if elapsed >= TIMEOUT_MS {
                    log::warn!("[Realtime] S2S session idle for over 10 minutes. Triggering idle timeout.");
                    
                    // Stop the session internally
                    let mut rt_guard = state_clone.realtime_engine.lock().await;
                    if let Some(mut rt_engine) = rt_guard.take() {
                        rt_engine.stop();
                    }
                    drop(rt_guard);

                    // Update backend engagement state
                    state_clone.pipeline.is_engaged.store(false, std::sync::atomic::Ordering::Relaxed);

                    // Tell VAD to stop routing chunks
                    if let Some(engine) = state_clone.engine.lock().await.as_ref() {
                        let _ = engine
                            .vad_tx
                            .send(crate::core::state::VadCommand::StopRealtime);
                    }

                    // Delete session cache file
                    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
                    if cache_path.exists() {
                        let _ = std::fs::remove_file(cache_path);
                    }

                    // Emit event to frontend
                    let _ = app_clone.emit_to("main", "realtime_session_ended", "idle_timeout".to_string());
                    break;
                } else if elapsed >= WARN_2_MS {
                    let remaining = 600 - (elapsed / 1000);
                    let _ = app_clone.emit_to("main", "realtime_idle_warning", serde_json::json!({ "seconds_remaining": remaining }));
                    tokio::time::sleep(std::time::Duration::from_millis(TIMEOUT_MS - elapsed)).await;
                } else if elapsed >= WARN_1_MS {
                    let remaining = 600 - (elapsed / 1000);
                    let _ = app_clone.emit_to("main", "realtime_idle_warning", serde_json::json!({ "seconds_remaining": remaining }));
                    tokio::time::sleep(std::time::Duration::from_millis(WARN_2_MS - elapsed)).await;
                } else {
                    let time_until_warning = WARN_1_MS - elapsed;
                    tokio::time::sleep(std::time::Duration::from_millis(time_until_warning)).await;
                }
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(WARN_1_MS)).await;
            }
        }
    });

    if resumed {
        let _ = app.emit_to("main", "realtime_session_resumed", ());
    } else {
        let _ = app.emit_to("main", "realtime_session_started", ());
    }

    log::info!("[IPC] Realtime S2S session started successfully.");
    Ok(())
}

/// Start the real-time speech-to-speech session with the active cloud provider.
#[tauri::command]
pub async fn start_realtime_session(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    if let Err(e) = start_realtime_session_internal(&app, &state).await {
        log::error!("[IPC] start_realtime_session failed: {}", e);
        return Err(e);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_realtime_session_cache() -> Result<RealtimeSessionCache, String> {
    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
    if cache_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<serde_json::Value>(&data) {
                let expires_at = cached["expires_at"].as_u64().unwrap_or(0);
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                let provider = cached["provider"].as_str().unwrap_or("").to_string();
                let model = cached["model"].as_str().unwrap_or("").to_string();
                let expires_in_seconds = (expires_at as i64 - now_ms as i64) / 1000;
                
                return Ok(RealtimeSessionCache {
                    has_session: expires_in_seconds > 0,
                    provider,
                    expires_in_seconds,
                    model,
                });
            }
        }
    }
    
    Ok(RealtimeSessionCache {
        has_session: false,
        provider: String::new(),
        expires_in_seconds: 0,
        model: String::new(),
    })
}

/// Stop the active real-time speech-to-speech session and restore modular mode.
#[tauri::command]
pub async fn stop_realtime_session(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    log::info!("[IPC] stop_realtime_session requested");

    // 1. Tell VAD to stop routing chunks and cancel active playback
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::StopRealtime);
        engine.playback_engine.cancel();
    }

    // 2. Stop and drop the realtime engine
    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut rt_engine) = rt_guard.take() {
        rt_engine.stop();
    }

    // Update backend engagement state and owner
    state.pipeline.is_engaged.store(false, std::sync::atomic::Ordering::Relaxed);
    state.owner.store(crate::core::state::InteractionOwner::Tray as u32, std::sync::atomic::Ordering::Relaxed);
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::UpdateOwner(
                crate::core::state::InteractionOwner::Tray,
            ));
    }

    // Delete session cache file
    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
    if cache_path.exists() {
        let _ = std::fs::remove_file(cache_path);
    }

    // Emit event to frontend
    let _ = app.emit_to("main", "realtime_session_ended", "user".to_string());

    log::info!("[IPC] Realtime S2S session stopped successfully.");
    Ok(())
}
