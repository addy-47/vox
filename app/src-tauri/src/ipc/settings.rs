use tauri::{AppHandle, State, Emitter, Manager};
use std::time::Duration;
use crate::core::state::AppState;
use crate::core::settings::{VoxSettings, SettingReloadPolicy, reload_policy_for, InteractionMode};
use crate::utils::paths;
use crate::ipc::pipeline::{launch_engine, stop_engine};

// ─── Debounce constant ────────────────────────────────────────────────────────

/// Disk write is deferred by this duration after the last setting change.
/// Prevents thrashing disk on rapid slider updates (dozens of changes/sec).
const SETTINGS_SAVE_DEBOUNCE_MS: u64 = 1500;

// ─── Response Types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct BootState {
    pub settings: VoxSettings,
    pub models_dir_exists: bool,
    pub settings_path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelCatalog {
    pub llm: Vec<crate::core::settings::ModelMetadata>,
    pub asr: Vec<crate::core::settings::ModelMetadata>,
    pub tts: Vec<crate::core::settings::ModelMetadata>,
    pub voices: Vec<crate::core::settings::VoiceProfile>,
    pub preset_colors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SettingUpdateResult {
    pub applied: bool,
    pub reload_policy: String,
    pub message: String,
}

// ─── IPC Commands ─────────────────────────────────────────────────────────────

/// Called by the frontend on mount.
///
/// Returns the full settings snapshot plus directory health status.
/// The frontend should boot into a loading/splash state and render only
/// after this resolves successfully.
#[tauri::command]
pub async fn request_boot_state(app: AppHandle) -> Result<BootState, String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let settings = state.settings.read().map_err(|e| e.to_string())?.clone();
    let models_dir_exists = paths::get().models.exists();
    let settings_path = paths::get().settings.to_string_lossy().to_string();

    log::info!("[Settings] Boot state requested. models_dir={}, settings={}", models_dir_exists, settings_path);

    Ok(BootState {
        settings,
        models_dir_exists,
        settings_path,
    })
}

#[tauri::command]
pub async fn request_model_catalog() -> Result<ModelCatalog, String> {
    Ok(ModelCatalog {
        llm: crate::core::settings::get_llm_metadata(),
        asr: crate::core::settings::get_asr_metadata(),
        tts: crate::core::settings::get_tts_metadata(),
        voices: crate::core::settings::get_voice_profiles(),
        preset_colors: crate::core::settings::get_preset_colors(),
    })
}

/// Returns the current settings snapshot.
#[tauri::command]
pub async fn get_settings(state: State<'_, std::sync::Arc<AppState>>) -> Result<VoxSettings, String> {
    state.settings.read().map_err(|e| e.to_string()).map(|s| s.clone())
}

/// Generic settings update command.
///
/// Applies the new value in-memory immediately, returns the reload policy,
/// and schedules a debounced disk write (1.5s after last change).
///
/// # Domain/Key Convention
/// - domain: "ui" | "vad" | "audio" | "asr" | "llm" | "tts" | "interaction" | "telemetry" | "persistence" | "assistant"
/// - key: the field name within that domain struct (snake_case)
#[tauri::command]
pub async fn update_setting(
    domain: String,
    key: String,
    value: serde_json::Value,
    app: AppHandle,
) -> Result<SettingUpdateResult, String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let policy = reload_policy_for(&domain, &key);

    let applied = {
        let mut settings = state.settings.write().map_err(|e| e.to_string())?;
        apply_setting_mutation(&mut settings, &domain, &key, &value)?
    };

    if applied && domain == "persistence" && key == "private_mode" {
        let is_private = value.as_bool().unwrap_or(false);
        state.is_private_mode.store(is_private, std::sync::atomic::Ordering::Relaxed);
        log::info!("[Settings] Privacy Mode updated: enabled={}", is_private);
    }

    if applied && domain == "ui" && key == "tray_enabled" {
        let enabled = value.as_bool().unwrap_or(true);
        log::info!("[Settings] Tray Lifecycle Event: enabled={}", enabled);

        // Synchronize System Tray Menu Item (Vox Live)
        {
            let menu_item_lock = state.hud_menu_item.lock().await;
            if let Some(ref live_i) = *menu_item_lock {
                let _ = live_i.set_enabled(enabled);
                // If disabling, also uncheck it to reflect it's offline
                if !enabled {
                    let _ = live_i.set_checked(false);
                } else {
                    // Restore checked state based on current visibility logic if needed
                    let hud_visible = *state.hud_visible.lock().await;
                    let _ = live_i.set_checked(hud_visible);
                }
            }
        }

        if !enabled {
            // Disable Tray: Hide window and check if we can stop engine
            log::info!("[Settings] Disabling Tray HUD: Hiding window and evaluating engine offload...");
            if let Some(tray_win) = app.get_webview_window("tray") {
                let _ = tray_win.hide();
            }
            
            let is_engaged = state.pipeline.is_engaged.load(std::sync::atomic::Ordering::Relaxed);

            log::info!("[Settings] Offload evaluation: engaged={}", is_engaged);

            if !is_engaged {
                log::info!("[Settings] No active consumers (Engage=OFF). Offloading models...");
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = stop_engine(app_clone).await;
                });
            } else {
                log::info!("[Settings] Engine retention: Active consumer(s) engaged.");
            }
        } else {
            // Enable Tray: Launch engine if needed
            log::info!("[Settings] Enabling Tray HUD: Ensuring 3-Tier Engine is active...");
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = launch_engine(app_clone).await {
                    log::error!("[Settings] Failed to launch engine for tray: {}", e);
                }
            });
        }
    }

    if applied && domain == "interaction" && key == "main_app_mode" {
        // Evaluate offload: If switching to PTT and Tray is disabled, we might want to stop engine
        let (tray_enabled, is_engaged, is_passive) = {
            let s = state.settings.read().unwrap();
            (s.ui.tray_enabled, state.pipeline.is_engaged.load(std::sync::atomic::Ordering::Relaxed), s.interaction.main_app_mode == InteractionMode::Passive)
        };

        if !tray_enabled && !is_engaged && !is_passive {
            log::info!("[Settings] Main App mode changed to non-passive and Tray is disabled. Stopping engine...");
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = stop_engine(app_clone).await;
            });
        } else if is_passive {
            log::info!("[Settings] Main App mode changed to Passive. Ensuring engine is launched...");
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = launch_engine(app_clone).await;
            });
        }

        // Notify VAD of mode change if Main App is owner
        let owner: crate::core::state::InteractionOwner = state.owner.load(std::sync::atomic::Ordering::Relaxed).into();
        if owner == crate::core::state::InteractionOwner::MainWindow {
            if let Some(engine) = state.engine.lock().await.as_ref() {
                if let Ok(mode) = serde_json::from_value::<crate::core::settings::InteractionMode>(value.clone()) {
                    let _ = engine.vad_tx.send(crate::core::state::VadCommand::UpdateMode(mode));
                }
            }
        }
    }

    if applied && domain == "vad" && key == "vad_backend" {
        log::info!("[Settings] VAD backend changed. Hot-swapping 3-Tier Engine...");
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = stop_engine(app_clone.clone()).await;
            let _ = launch_engine(app_clone).await;
        });
    }

    if !applied {
        return Ok(SettingUpdateResult {
            applied: false,
            reload_policy: policy.as_str().to_string(),
            message: format!("Unknown setting: {}.{}", domain, key),
        });
    }

    // Dispatch worker command for hot-updatable settings
    if policy == SettingReloadPolicy::WorkerCommand {
        dispatch_worker_command(&app, &domain, &key, &value).await;
    }

    // Schedule debounced disk write
    schedule_debounced_save(app.clone(), state.clone()).await;

    let action_label = match policy {
        SettingReloadPolicy::Hot           => "hot-applied",
        SettingReloadPolicy::WorkerCommand => "dispatched to worker",
        SettingReloadPolicy::Restart       => "restart required",
    };

    let message = format!("{}.{} = {} — {}", domain, key, value, action_label);

    log::info!("[Settings] Updated: {}", message);

    if domain == "ui" && key == "theme" {
        let _ = app.emit("theme-changed", value.as_str().unwrap_or("dark"));
    }

    // Notify Pipeline Orchestrator of settings change for local caching
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let current_settings = state.settings.read().unwrap().clone();
        let _ = engine.pipeline_tx.send(crate::core::events::VoxEvent::SettingsUpdated(current_settings));
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
        if settings.ui.theme == theme {
            return Ok(());
        }
        settings.ui.theme = theme.clone();
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
    let _ = app.emit("theme-changed", defaults.ui.theme.clone());
    
    schedule_debounced_save(app.clone(), state.clone()).await;
    
    Ok(defaults)
}

// ─── Internal Helpers ─────────────────────────────────────────────────────────

/// Applies a mutation to the settings struct by domain+key routing.
/// Returns `true` if the key was recognized and applied.
fn apply_setting_mutation(
    settings: &mut VoxSettings,
    domain: &str,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match (domain, key) {
        ("ui", "theme") => {
            settings.ui.theme = value.as_str().ok_or("theme must be a string")?.to_string();
        }
        ("ui", "accent_seed") => {
            settings.ui.accent_seed = value.as_str().ok_or("accent_seed must be a string")?.to_string();
        }
        ("ui", "tray_enabled") => {
            settings.ui.tray_enabled = value.as_bool().ok_or("tray_enabled must be a boolean")?;
        }
        ("ui", "tray_blur_density") => {
            settings.ui.tray_blur_density = value.as_u64().ok_or("tray_blur_density must be a positive integer")? as u32;
        }
        ("ui", "tray_glass_tint") => {
            settings.ui.tray_glass_tint = value.as_bool().ok_or("tray_glass_tint must be a boolean")?;
        }
        ("ui", "tray_history_limit") => {
            settings.ui.tray_history_limit = value.as_u64().ok_or("tray_history_limit must be a positive integer")? as u32;
        }
        ("vad", "threshold") => {
            settings.vad.threshold = value.as_f64().ok_or("threshold must be a number")? as f32;
        }
        ("vad", "ptt_noise_gate") => {
            settings.vad.ptt_noise_gate = value.as_f64().ok_or("ptt_noise_gate must be a number")? as f32;
        }
        ("vad", "vad_backend") => {
            settings.vad.vad_backend = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid vad_backend: {}", e))?;
        }
        ("asr", "model") => {
            settings.asr.model = value.as_str().ok_or("model must be a string")?.to_string();
        }
        ("asr", "transliterate_enabled") => {
            settings.asr.transliterate_enabled = value.as_bool().ok_or("transliterate_enabled must be a boolean")?;
        }
        ("audio", "output_mode") => {
            settings.audio.output_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid output_mode: {}", e))?;
        }
        ("llm", "model") => {
            settings.llm.model = value.as_str().ok_or("model must be a string")?.to_string();
        }
        ("llm", "ctx_size") => {
            settings.llm.ctx_size = value.as_u64().ok_or("ctx_size must be a positive integer")? as u32;
        }
        ("llm", "threads") => {
            settings.llm.threads = value.as_u64().ok_or("threads must be a positive integer")? as u32;
        }
        ("llm", "provider") => {
            settings.llm.provider = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid provider: {}", e))?;
        }
        ("interaction", "main_app_mode") => {
            settings.interaction.main_app_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid main_app_mode: {}", e))?;
        }
        ("interaction", "auto_sleep_timeout") => {
            settings.interaction.auto_sleep_timeout = value.as_u64().ok_or("auto_sleep_timeout must be a positive integer")? as u32;
        }
        ("interaction", "pipeline_mode") => {
            settings.interaction.pipeline_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid pipeline_mode: {}", e))?;
        }
        ("interaction", "tray_mode") => {
            settings.interaction.tray_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid tray_mode: {}", e))?;
        }
        ("telemetry", "enabled") => {
            settings.telemetry.enabled = value.as_bool().ok_or("enabled must be a boolean")?;
        }
        ("telemetry", "log_level") => {
            settings.telemetry.log_level = value.as_str().ok_or("log_level must be a string")?.to_string();
        }
        ("tts", "voice") => {
            settings.tts.voice = value.as_i64().ok_or("voice must be an integer")? as i32;
        }
        ("tts", "quality_steps") => {
            settings.tts.quality_steps = value.as_u64().ok_or("quality_steps must be a positive integer")? as u32;
        }
        ("tts", "speed") => {
            settings.tts.speed = value.as_f64().ok_or("speed must be a number")? as f32;
        }
        ("persistence", "private_mode") => {
            settings.persistence.private_mode = value.as_bool().ok_or("private_mode must be a boolean")?;
        }
        ("persistence", "max_sessions") => {
            settings.persistence.max_sessions = value.as_u64().ok_or("max_sessions must be a positive integer")? as u32;
        }
        ("persistence", "retention_days") => {
            settings.persistence.retention_days = value.as_u64().ok_or("retention_days must be a positive integer")? as u32;
        }
        ("assistant", "hindi_prompt") => {
            settings.assistant.hindi_prompt = value.as_str().ok_or("hindi_prompt must be a string")?.to_string();
        }
        ("assistant", "english_prompt") => {
            settings.assistant.english_prompt = value.as_str().ok_or("english_prompt must be a string")?.to_string();
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// Dispatches a hot-update command to the appropriate worker thread.
/// Called only for `WorkerCommand` policy settings.
async fn dispatch_worker_command(app: &AppHandle, domain: &str, key: &str, value: &serde_json::Value) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let engine_lock = state.engine.lock().await;

    if let Some(engine) = engine_lock.as_ref() {
        match (domain, key) {
            ("vad", "threshold") => {
                if let Some(v) = value.as_f64() {
                    let _ = engine.vad_tx.send(crate::core::state::VadCommand::UpdateThreshold(v as f32));
                    log::debug!("[Settings] VadCommand::UpdateThreshold({}) dispatched", v);
                }
            }
            ("vad", "ptt_noise_gate") => {
                if let Some(v) = value.as_f64() {
                    let _ = engine.vad_tx.send(crate::core::state::VadCommand::UpdateNoiseGate(v as f32));
                    log::debug!("[Settings] VadCommand::UpdateNoiseGate({}) dispatched", v);
                }
            }
            ("audio", "output_mode") => {
                if let Ok(mode) = serde_json::from_value::<crate::core::settings::AudioOutputMode>(value.clone()) {
                    let _ = engine.vad_tx.send(crate::core::state::VadCommand::UpdateAudioMode(mode));
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

    let settings_snapshot = {
        state.settings.read().ok().map(|s| s.clone())
    };

    let Some(snapshot) = settings_snapshot else { return; };

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

#[tauri::command]
pub async fn check_llm_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
) -> Result<bool, String> {
    use crate::services::llm::{OpenAiCompatProvider, LlmProvider};
    use crate::core::settings::LlmProviderConfig;
    use crate::utils::paths;

    let (config, llm_model) = {
        if let Some(prov) = provider {
            (prov, "".to_string())
        } else {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            (settings.llm.provider.clone(), settings.llm.model.clone())
        }
    };

    match config {
        LlmProviderConfig::Embedded => {
            let models_dir = paths::get().models.clone();
            let manifest_lock = state.manifest.read().await;

            let llm_path = if let Some(ref manifest) = *manifest_lock {
                if let Some(group) = manifest.model_groups.iter().find(|g| g.id == llm_model) {
                    if let Some(file) = group.files.first() {
                        models_dir.join(&file.path)
                    } else {
                        models_dir
                            .join(crate::core::constants::MODEL_DIR_LLM)
                            .join(crate::core::constants::MODEL_FILE_LLM_GGUF)
                    }
                } else {
                    models_dir
                        .join(crate::core::constants::MODEL_DIR_LLM)
                        .join(crate::core::constants::MODEL_FILE_LLM_GGUF)
                }
            } else {
                models_dir
                    .join(crate::core::constants::MODEL_DIR_LLM)
                    .join(crate::core::constants::MODEL_FILE_LLM_GGUF)
            };

            Ok(llm_path.exists())
        }
        LlmProviderConfig::OpenAiCompat { base_url, model, api_key, .. } => {
            let provider = OpenAiCompatProvider::new(&base_url, &model, api_key.as_deref());
            let healthy = tokio::task::spawn_blocking(move || provider.health_check())
                .await
                .map_err(|e| e.to_string())?;
            Ok(healthy)
        }
    }
}

#[tauri::command]
pub async fn list_remote_llm_models(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
) -> Result<Vec<crate::core::settings::RemoteModelInfo>, String> {
    use crate::services::llm::{OpenAiCompatProvider, EmbeddedProvider, LlmProvider};
    use crate::core::settings::LlmProviderConfig;
    use crate::utils::paths;

    let config = {
        if let Some(prov) = provider {
            prov
        } else {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            settings.llm.provider.clone()
        }
    };

    match config {
        LlmProviderConfig::Embedded => {
            let llm_dir = paths::get().models.join(crate::core::constants::MODEL_DIR_LLM);
            let models = EmbeddedProvider::list_models_in_dir(&llm_dir).map_err(|e| e.to_string())?;
            Ok(models)
        }
        LlmProviderConfig::OpenAiCompat { base_url, model, api_key, .. } => {
            let provider = OpenAiCompatProvider::new(&base_url, &model, api_key.as_deref());
            let models = tokio::task::spawn_blocking(move || provider.list_models())
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(models)
        }
    }
}
