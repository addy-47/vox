//! ============================================================================
//! src/ipc/settings/mutation.rs — Setting update, mutation logic, and persistence commands
//! ============================================================================

use crate::core::settings::{
    get_setting_reload_policy, InteractionMode, SettingReloadPolicy, VoxSettings,
};
use crate::core::state::AppState;
use crate::ipc::pipeline::{launch_engine, stop_engine};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

// ─── Debounce constant ────────────────────────────────────────────────────────

/// Disk write is deferred by this duration after the last setting change.
/// Prevents thrashing disk on rapid slider updates (dozens of changes/sec).
const SETTINGS_SAVE_DEBOUNCE_MS: u64 = 1500;

// ─── Response Types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct SettingUpdateResult {
    pub applied: bool,
    pub reload_policy: String,
    pub message: String,
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

async fn handle_dictation_side_effects(
    app: &AppHandle,
    state: &AppState,
    key: &str,
    value: &serde_json::Value,
) {
    if key == "enabled" {
        let enabled = value.as_bool().unwrap_or(true);
        log::info!("[Settings] Dictation Lifecycle Event: enabled={}", enabled);

        let is_tray_mode = state
            .settings
            .read()
            .map(|s| s.dictation.output_mode == crate::core::settings::DictationOutputMode::Tray)
            .unwrap_or(false);
        let is_clickable = enabled && is_tray_mode;
        let menu_item_lock = state.hud_menu_item.lock().await;
        if let Some(ref live_i) = *menu_item_lock {
            let _ = live_i.set_enabled(is_clickable);
            let hud_visible = *state.hud_visible.lock().await;
            let _ = live_i.set_checked(hud_visible && is_clickable);
        }

        if !enabled {
            state.owner.store(
                crate::core::state::InteractionOwner::MainWindow as u32,
                std::sync::atomic::Ordering::Relaxed,
            );
            if let Some(engine) = state.engine.lock().await.as_ref() {
                if let Err(e) = engine
                    .vad_tx
                    .send(crate::core::state::VadCommand::UpdateOwner(
                        crate::core::state::InteractionOwner::MainWindow,
                    ))
                {
                    log::warn!("[Settings] Failed to send VadCommand::UpdateOwner: {}", e);
                }
            }
            crate::tray::destroy_tray_window(app);

            let is_engaged = state
                .pipeline
                .is_engaged
                .load(std::sync::atomic::Ordering::Relaxed);
            if !is_engaged {
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = stop_engine(app_clone).await;
                });
            }
        } else {
            if is_tray_mode {
                let _ = crate::tray::ensure_tray_window(app);
            }
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = launch_engine(app_clone).await {
                    log::error!("[Settings] Failed to launch engine for dictation: {}", e);
                }
            });
        }
    } else if key == "output_mode" {
        let (enabled, output_mode) = state
            .settings
            .read()
            .map(|s| (s.dictation.enabled, s.dictation.output_mode.clone()))
            .unwrap_or((false, crate::core::settings::DictationOutputMode::Paste));
        let is_tray_mode = output_mode == crate::core::settings::DictationOutputMode::Tray;
        let is_clickable = enabled && is_tray_mode;

        let menu_item_lock = state.hud_menu_item.lock().await;
        if let Some(ref live_i) = *menu_item_lock {
            let _ = live_i.set_enabled(is_clickable);
            let hud_visible = *state.hud_visible.lock().await;
            let _ = live_i.set_checked(hud_visible && is_clickable);
        }

        if enabled && is_tray_mode {
            let _ = crate::tray::ensure_tray_window(app);
        } else if !is_tray_mode {
            crate::tray::destroy_tray_window(app);
        }
    }
}

async fn handle_interaction_side_effects(
    app: &AppHandle,
    state: &AppState,
    key: &str,
    value: &serde_json::Value,
) {
    if key == "mode" {
        let (dictation_enabled, is_engaged, is_passive) = state
            .settings
            .read()
            .map(|s| {
                (
                    s.dictation.enabled,
                    state
                        .pipeline
                        .is_engaged
                        .load(std::sync::atomic::Ordering::Relaxed),
                    s.interaction.mode == InteractionMode::Passive,
                )
            })
            .unwrap_or((false, false, false));

        if !dictation_enabled && !is_engaged && !is_passive {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = stop_engine(app_clone).await;
            });
        } else if is_passive {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = launch_engine(app_clone).await;
            });
        }

        let owner: crate::core::state::InteractionOwner = state
            .owner
            .load(std::sync::atomic::Ordering::Relaxed)
            .into();
        if owner == crate::core::state::InteractionOwner::MainWindow {
            if let Some(engine) = state.engine.lock().await.as_ref() {
                if let Ok(mode) =
                    serde_json::from_value::<crate::core::settings::InteractionMode>(value.clone())
                {
                    if let Err(e) = engine
                        .vad_tx
                        .send(crate::core::state::VadCommand::UpdateMode(mode))
                    {
                        log::warn!("[Settings] Failed to send VadCommand::UpdateMode: {}", e);
                    }
                }
            }
        }
    }
}

async fn handle_setting_side_effects(
    app: &AppHandle,
    state: &AppState,
    domain: &str,
    key: &str,
    value: &serde_json::Value,
) {
    if domain == "history" && key == "private_mode" {
        let is_private = value.as_bool().unwrap_or(false);
        state
            .is_private_mode
            .store(is_private, std::sync::atomic::Ordering::Relaxed);
        log::info!("[Settings] Privacy Mode updated: enabled={}", is_private);
    } else if domain == "dictation" {
        handle_dictation_side_effects(app, state, key, value).await;
    } else if domain == "interaction" {
        handle_interaction_side_effects(app, state, key, value).await;
    } else if domain == "vad" && (key == "backend" || key == "vad_backend") {
        log::info!("[Settings] VAD backend changed. Hot-swapping 3-Tier Engine...");
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = stop_engine(app_clone.clone()).await;
            let _ = launch_engine(app_clone).await;
        });
    }
}

/// Generic settings update command.
///
/// Applies the new value in-memory immediately, returns the reload policy,
/// and schedules a debounced disk write (1.5s after last change).
#[tauri::command]
pub async fn update_setting(
    domain: String,
    key: String,
    value: serde_json::Value,
    app: AppHandle,
) -> Result<SettingUpdateResult, String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let policy = get_setting_reload_policy(&domain, &key);

    let applied = {
        let mut settings = state.settings.write().map_err(|e| e.to_string())?;
        apply_setting_mutation(&mut settings, &domain, &key, &value)?
    };

    if applied {
        handle_setting_side_effects(&app, &state, &domain, &key, &value).await;
    } else {
        return Ok(SettingUpdateResult {
            applied: false,
            reload_policy: policy.as_str().to_string(),
            message: format!("Unknown setting: {}.{}", domain, key),
        });
    }

    if policy == SettingReloadPolicy::WorkerCommand {
        dispatch_worker_command(&app, &domain, &key, &value).await;
    }

    schedule_debounced_save(app.clone(), state.clone()).await;

    let action_label = match policy {
        SettingReloadPolicy::Hot => "hot-applied",
        SettingReloadPolicy::WorkerCommand => "dispatched to worker",
        SettingReloadPolicy::Restart => "restart required",
    };

    let message = format!("{}.{} = {} — {}", domain, key, value, action_label);
    log::info!("[Settings] Updated: {}", message);

    if domain == "appearance" && key == "theme" {
        let _ = app.emit("theme-changed", value.as_str().unwrap_or("dark"));
    }

    if let Some(engine) = state.engine.lock().await.as_ref() {
        if let Ok(current_settings) = state.settings.read() {
            let _ = engine
                .pipeline_tx
                .send(crate::core::events::VoxEvent::SettingsUpdated(Box::new(
                    current_settings.clone(),
                )));
        }
    }

    let _ = app.emit("settings-updated", ());

    Ok(SettingUpdateResult {
        applied: true,
        reload_policy: policy.as_str().to_string(),
        message,
    })
}

/// Convenience command for theme changes (kept for backward compat with existing frontend).
#[tauri::command]
pub async fn update_theme(app: AppHandle, theme: String) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    {
        let mut settings = state.settings.write().map_err(|e| e.to_string())?;
        if settings.appearance.theme == theme {
            return Ok(());
        }
        settings.appearance.theme = theme.clone();
    }
    let _ = app.emit("theme-changed", theme);
    schedule_debounced_save(app.clone(), state.clone()).await;
    Ok(())
}

/// Resets all settings to system defaults.
#[tauri::command]
pub async fn reset_settings(app: AppHandle) -> Result<VoxSettings, String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let defaults = VoxSettings::default();
    {
        let mut settings = state.settings.write().map_err(|e| e.to_string())?;
        *settings = defaults.clone();
    }

    // Immediate apply for theme and other hot settings
    let _ = app.emit("theme-changed", defaults.appearance.theme.clone());

    schedule_debounced_save(app.clone(), state.clone()).await;

    Ok(defaults)
}

// ─── Internal Helpers ─────────────────────────────────────────────────────────

/// Applies a mutation to the settings struct by domain+key routing.
/// Returns `true` if the key was recognized and applied.
pub(crate) fn apply_setting_mutation(
    settings: &mut VoxSettings,
    domain: &str,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match (domain, key) {
        // Appearance
        ("appearance", "theme") => {
            settings.appearance.theme = value.as_str().ok_or("theme must be a string")?.to_string();
        }
        ("appearance", "accent_seed") => {
            settings.appearance.accent_seed = value
                .as_str()
                .ok_or("accent_seed must be a string")?
                .to_string();
        }

        // Audio
        ("audio", "output_mode") => {
            settings.audio.output_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid output_mode: {}", e))?;
        }
        ("audio", "input_device") => {
            settings.audio.input_device = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid input_device: {}", e))?;
        }

        // VAD
        ("vad", "threshold") => {
            let threshold = value.as_f64().ok_or("threshold must be a number")? as f32;
            if !(0.0..=1.0).contains(&threshold) {
                return Err("threshold must be between 0.0 and 1.0".to_string());
            }
            settings.vad.threshold = threshold;
        }
        ("vad", "ptt_noise_gate") => {
            settings.vad.ptt_noise_gate =
                value.as_f64().ok_or("ptt_noise_gate must be a number")? as f32;
        }
        ("vad", "backend" | "vad_backend") => {
            settings.vad.backend = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid vad backend: {}", e))?;
        }

        // STT
        ("stt", "active") => {
            settings.stt.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT active provider: {}", e))?;
        }
        ("stt", "model") => {
            settings.stt.embedded.model =
                value.as_str().ok_or("model must be a string")?.to_string();
        }
        ("stt", "transliterate_enabled") => {
            settings.stt.transliterate_enabled = value
                .as_bool()
                .ok_or("transliterate_enabled must be a boolean")?;
        }
        ("stt", "embedded") => {
            settings.stt.embedded = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT embedded config: {}", e))?;
        }
        ("stt", "cloud") => {
            settings.stt.cloud = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT cloud config: {}", e))?;
        }
        ("stt" | "asr", "provider") => {
            if let Ok(config) =
                serde_json::from_value::<crate::core::settings::SttProviderConfig>(value.clone())
            {
                match config {
                    crate::core::settings::SttProviderConfig::Embedded { model_type } => {
                        settings.stt.active = crate::core::settings::SttActiveProvider::Embedded;
                        settings.stt.embedded.model = model_type;
                    }
                    crate::core::settings::SttProviderConfig::Cloud {
                        provider,
                        model,
                        language,
                        region,
                        credentials_path,
                        credentials_json,
                        project_id,
                        endpoint,
                    } => {
                        settings.stt.active = crate::core::settings::SttActiveProvider::Cloud;
                        settings.stt.cloud = crate::core::settings::SttCloudConfig {
                            provider,
                            model,
                            language,
                            region,
                            credentials_path,
                            credentials_json,
                            project_id,
                            endpoint,
                        };
                    }
                }
            }
        }

        // LLM
        ("llm", "active") => {
            settings.llm.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM active provider: {}", e))?;
        }
        ("llm", "model") => {
            let model_str = value.as_str().ok_or("model must be a string")?.to_string();
            match settings.llm.active {
                crate::core::settings::LlmActiveProvider::Embedded => {
                    settings.llm.embedded.model = model_str;
                }
                crate::core::settings::LlmActiveProvider::Server => {
                    settings.llm.server.model = model_str;
                }
                crate::core::settings::LlmActiveProvider::Cloud => {
                    settings.llm.cloud.model = model_str;
                }
            }
        }
        ("llm", "temperature") => {
            settings.llm.temperature = value.as_f64().ok_or("temperature must be a number")? as f32;
        }
        ("llm", "compaction_temperature") => {
            settings.llm.compaction_temperature = value
                .as_f64()
                .ok_or("compaction_temperature must be a number")?
                as f32;
        }
        ("llm", "max_output_tokens") => {
            settings.llm.max_output_tokens = value
                .as_u64()
                .ok_or("max_output_tokens must be a positive integer")?
                as u32;
        }
        ("llm", "context_window") => {
            let val = value
                .as_u64()
                .ok_or("context_window must be a positive integer")? as u32;
            if matches!(
                settings.llm.active,
                crate::core::settings::LlmActiveProvider::Server
                    | crate::core::settings::LlmActiveProvider::Cloud
            ) && val < 8192
            {
                return Err(
                    "Cloud/Server LLM providers require a minimum context size of 8192 tokens"
                        .to_string(),
                );
            }
            settings.llm.context_window = val;
        }
        ("llm", "threads") => {
            settings.llm.threads =
                value.as_u64().ok_or("threads must be a positive integer")? as u32;
        }
        ("llm", "embedded") => {
            settings.llm.embedded = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM embedded config: {}", e))?;
        }
        ("llm", "server") => {
            settings.llm.server = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM server config: {}", e))?;
        }
        ("llm", "cloud") => {
            settings.llm.cloud = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM cloud config: {}", e))?;
        }
        ("llm", "provider") => {
            if let Ok(prov) =
                serde_json::from_value::<crate::core::settings::LlmProviderConfig>(value.clone())
            {
                match prov {
                    crate::core::settings::LlmProviderConfig::Embedded => {
                        settings.llm.active = crate::core::settings::LlmActiveProvider::Embedded;
                    }
                    crate::core::settings::LlmProviderConfig::OpenAiCompat {
                        base_url,
                        model,
                        api_key,
                        provider_name,
                    } => {
                        let is_cloud = provider_name.as_deref().is_some_and(|p| {
                            let pl = p.to_lowercase();
                            pl.contains("nvidia")
                                || pl.contains("groq")
                                || pl.contains("openrouter")
                                || pl.contains("together")
                                || pl.contains("openai")
                                || pl.contains("gemini")
                                || pl.contains("mistral")
                        });
                        if is_cloud {
                            settings.llm.active = crate::core::settings::LlmActiveProvider::Cloud;
                            settings.llm.cloud = crate::core::settings::LlmRemoteConfig {
                                base_url,
                                model,
                                api_key,
                                provider_name,
                            };
                        } else {
                            settings.llm.active = crate::core::settings::LlmActiveProvider::Server;
                            settings.llm.server = crate::core::settings::LlmRemoteConfig {
                                base_url,
                                model,
                                api_key,
                                provider_name,
                            };
                        }
                    }
                }
            }
        }

        // TTS
        ("tts", "active") => {
            settings.tts.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid TTS active provider: {}", e))?;
        }
        ("tts", "voice" | "voice_index") => {
            settings.tts.voice_index = value.as_i64().ok_or("voice must be an integer")? as i32;
        }
        ("tts", "quality_steps") => {
            settings.tts.quality_steps = value
                .as_u64()
                .ok_or("quality_steps must be a positive integer")?
                as u32;
        }
        ("tts", "speed") => {
            settings.tts.speed = value.as_f64().ok_or("speed must be a number")? as f32;
        }
        ("tts", "edge_tts") => {
            settings.tts.edge_tts = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid edge_tts config: {}", e))?;
        }
        ("tts", "supertonic") => {
            settings.tts.supertonic = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid supertonic config: {}", e))?;
        }
        ("tts", "chatterbox") => {
            settings.tts.chatterbox = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid chatterbox config: {}", e))?;
        }
        ("tts", "chatterbox_remote") => {
            settings.tts.chatterbox_remote = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid chatterbox_remote config: {}", e))?;
        }
        ("tts", "provider") => {
            if let Ok(prov) =
                serde_json::from_value::<crate::core::settings::TtsProviderConfig>(value.clone())
            {
                match prov {
                    crate::core::settings::TtsProviderConfig::Supertonic => {
                        settings.tts.active = crate::core::settings::TtsActiveProvider::Supertonic;
                    }
                    crate::core::settings::TtsProviderConfig::EdgeTts { voice } => {
                        settings.tts.active = crate::core::settings::TtsActiveProvider::EdgeTts;
                        settings.tts.edge_tts.voice = voice;
                    }
                    crate::core::settings::TtsProviderConfig::Chatterbox {
                        language,
                        quality_steps,
                        speed,
                        voice_id,
                    } => {
                        settings.tts.active = crate::core::settings::TtsActiveProvider::Chatterbox;
                        settings.tts.chatterbox.language = language;
                        settings.tts.chatterbox.voice_id = voice_id;
                        settings.tts.quality_steps = quality_steps;
                        settings.tts.speed = speed;
                    }
                    crate::core::settings::TtsProviderConfig::ChatterboxRemote {
                        endpoint,
                        language,
                        quality_steps,
                        speed,
                        remote_path,
                        voice_id,
                    } => {
                        settings.tts.active =
                            crate::core::settings::TtsActiveProvider::ChatterboxRemote;
                        settings.tts.chatterbox_remote.endpoint = endpoint;
                        settings.tts.chatterbox_remote.language = language;
                        settings.tts.chatterbox_remote.remote_path = remote_path;
                        settings.tts.chatterbox_remote.voice_id = voice_id;
                        settings.tts.quality_steps = quality_steps;
                        settings.tts.speed = speed;
                    }
                }
            }
        }

        // Interaction
        ("interaction", "mode") => {
            settings.interaction.mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid interaction mode: {}", e))?;
        }
        ("interaction", "auto_sleep_timeout") => {
            settings.interaction.auto_sleep_timeout = value
                .as_u64()
                .ok_or("auto_sleep_timeout must be a positive integer")?
                as u32;
        }
        ("interaction", "pipeline_mode") => {
            settings.interaction.pipeline_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid pipeline_mode: {}", e))?;
        }

        // Dictation
        ("dictation", "enabled") => {
            settings.dictation.enabled = value.as_bool().ok_or("enabled must be a boolean")?;
        }
        ("dictation", "interaction_mode") => {
            settings.dictation.interaction_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid interaction_mode: {}", e))?;
        }
        ("dictation", "hotkey") => {
            settings.dictation.hotkey =
                value.as_str().ok_or("hotkey must be a string")?.to_string();
        }
        ("dictation", "output_mode") => {
            settings.dictation.output_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid output_mode: {}", e))?;
        }

        // History
        ("history", "private_mode") => {
            settings.history.private_mode =
                value.as_bool().ok_or("private_mode must be a boolean")?;
        }
        ("history", "tray_history_limit") => {
            settings.history.tray_history_limit = value
                .as_u64()
                .ok_or("tray_history_limit must be a positive integer")?
                as u32;
        }

        // Persona
        ("persona", "modular_prompt") => {
            settings.persona.modular_prompt = value
                .as_str()
                .ok_or("modular_prompt must be a string")?
                .to_string();
        }
        ("persona", "realtime_prompt") => {
            settings.persona.realtime_prompt = value
                .as_str()
                .ok_or("realtime_prompt must be a string")?
                .to_string();
        }

        // Realtime
        ("realtime", "active" | "provider") => {
            settings.realtime.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid realtime provider: {}", e))?;
        }
        ("realtime", "gemini" | "gemini_live") => {
            settings.realtime.gemini_live = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid gemini config: {}", e))?;
        }
        ("realtime", "openai" | "openai_realtime") => {
            settings.realtime.openai_realtime = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid openai config: {}", e))?;
        }
        ("realtime", "deepgram" | "deepgram_voice_agent") => {
            settings.realtime.deepgram_voice_agent = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid deepgram config: {}", e))?;
        }
        ("realtime", "elevenlabs" | "elevenlabs_convai") => {
            settings.realtime.elevenlabs_convai = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid elevenlabs config: {}", e))?;
        }

        // Memory
        ("memory", "context_retrieval_enabled") => {
            settings.memory.context_retrieval_enabled = value
                .as_bool()
                .ok_or("context_retrieval_enabled must be a boolean")?;
        }
        ("memory", "pipeline_processing_enabled") => {
            settings.memory.pipeline_processing_enabled = value
                .as_bool()
                .ok_or("pipeline_processing_enabled must be a boolean")?;
        }
        ("memory", "max_context_share") => {
            let val = value.as_f64().ok_or("max_context_share must be a number")? as f32;
            if !(0.0..=1.0).contains(&val) {
                return Err("max_context_share must be between 0.0 and 1.0".to_string());
            }
            settings.memory.max_context_share = val;
        }
        ("memory", "context_chaining_window_hours") => {
            settings.memory.context_chaining_window_hours = value
                .as_u64()
                .ok_or("context_chaining_window_hours must be a positive integer")?
                as u32;
        }
        ("memory", "top_k_facts") => {
            let top_k = value
                .as_u64()
                .ok_or("top_k_facts must be a positive integer")? as u32;
            if top_k == 0 || top_k > 100 {
                return Err("top_k_facts must be between 1 and 100".to_string());
            }
            settings.memory.top_k_facts = top_k;
        }
        ("memory", "max_hops") => {
            let max_hops = value
                .as_u64()
                .ok_or("max_hops must be a positive integer")? as u32;
            if max_hops == 0 || max_hops > 10 {
                return Err("max_hops must be between 1 and 10".to_string());
            }
            settings.memory.max_hops = max_hops;
        }
        ("memory", "semantic_similarity_cutoff") => {
            let val = value
                .as_f64()
                .ok_or("semantic_similarity_cutoff must be a number")? as f32;
            if !(0.0..=1.0).contains(&val) {
                return Err("semantic_similarity_cutoff must be between 0.0 and 1.0".to_string());
            }
            settings.memory.semantic_similarity_cutoff = val;
        }

        // System
        ("system", "telemetry_enabled") => {
            settings.system.telemetry_enabled = value
                .as_bool()
                .ok_or("telemetry_enabled must be a boolean")?;
        }
        ("system", "log_level") => {
            settings.system.log_level = value
                .as_str()
                .ok_or("log_level must be a string")?
                .to_string();
        }
        ("system", "setup_completed") => {
            settings.system.setup_completed =
                value.as_bool().ok_or("setup_completed must be a boolean")?;
        }

        _ => return Ok(false),
    }
    Ok(true)
}

/// Dispatches a hot-update command to the appropriate worker thread.
/// Called only for `WorkerCommand` policy settings.
async fn dispatch_worker_command(
    app: &AppHandle,
    domain: &str,
    key: &str,
    value: &serde_json::Value,
) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let engine_lock = state.engine.lock().await;

    if let Some(engine) = engine_lock.as_ref() {
        match (domain, key) {
            ("vad", "threshold") => {
                if let Some(v) = value.as_f64() {
                    if let Err(e) = engine
                        .vad_tx
                        .send(crate::core::state::VadCommand::UpdateThreshold(v as f32))
                    {
                        log::warn!(
                            "[Settings] Failed to send VadCommand::UpdateThreshold: {}",
                            e
                        );
                    }
                    log::debug!("[Settings] VadCommand::UpdateThreshold({}) dispatched", v);
                }
            }
            ("vad", "ptt_noise_gate") => {
                if let Some(v) = value.as_f64() {
                    if let Err(e) = engine
                        .vad_tx
                        .send(crate::core::state::VadCommand::UpdateNoiseGate(v as f32))
                    {
                        log::warn!(
                            "[Settings] Failed to send VadCommand::UpdateNoiseGate: {}",
                            e
                        );
                    }
                    log::debug!("[Settings] VadCommand::UpdateNoiseGate({}) dispatched", v);
                }
            }
            ("audio", "output_mode") => {
                if let Ok(mode) =
                    serde_json::from_value::<crate::core::settings::AudioOutputMode>(value.clone())
                {
                    if let Err(e) = engine
                        .vad_tx
                        .send(crate::core::state::VadCommand::UpdateAudioMode(mode))
                    {
                        log::warn!(
                            "[Settings] Failed to send VadCommand::UpdateAudioMode: {}",
                            e
                        );
                    }
                    log::debug!("[Settings] VadCommand::UpdateAudioMode dispatched");
                }
            }
            _ => {}
        }
    }
}

/// Schedules a debounced settings save: cancels any pending save, spawns a new
/// task that waits `SETTINGS_SAVE_DEBOUNCE_MS` then writes to disk.
async fn schedule_debounced_save(_app: AppHandle, state: State<'_, std::sync::Arc<AppState>>) {
    let mut debounce = state.save_debounce.lock().await;

    // Cancel the previous pending write
    if let Some(handle) = debounce.take() {
        handle.abort();
    }

    let settings_snapshot = { state.settings.read().ok().map(|s| s.clone()) };

    let Some(snapshot) = settings_snapshot else {
        return;
    };

    let handle = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(SETTINGS_SAVE_DEBOUNCE_MS)).await;

        // Use spawn_blocking to avoid stalling the async executor with synchronous I/O
        let result = tokio::task::spawn_blocking(move || snapshot.save()).await;

        match result {
            Ok(Ok(_)) => log::debug!("[Settings] Debounced save completed."),
            Ok(Err(e)) => log::error!("[Settings] Debounced save failed: {}", e),
            Err(e) => log::error!("[Settings] Debounced save task panicked: {}", e),
        }
    });

    *debounce = Some(handle);
}
