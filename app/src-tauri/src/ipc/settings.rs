use crate::core::settings::{reload_policy_for, InteractionMode, SettingReloadPolicy, VoxSettings};
use crate::core::state::AppState;
use crate::ipc::pipeline::{launch_engine, stop_engine};
use crate::utils::paths;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

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

    log::debug!(
        "[Settings] Boot state requested. models_dir={}, settings={}",
        models_dir_exists,
        settings_path
    );

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
pub async fn get_settings(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<VoxSettings, String> {
    state
        .settings
        .read()
        .map_err(|e| e.to_string())
        .map(|s| s.clone())
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
        state
            .is_private_mode
            .store(is_private, std::sync::atomic::Ordering::Relaxed);
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
            log::info!(
                "[Settings] Disabling Tray HUD: Hiding window and evaluating engine offload..."
            );
            if let Some(tray_win) = app.get_webview_window("tray") {
                let _ = tray_win.hide();
            }

            let is_engaged = state
                .pipeline
                .is_engaged
                .load(std::sync::atomic::Ordering::Relaxed);

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
            (
                s.ui.tray_enabled,
                state
                    .pipeline
                    .is_engaged
                    .load(std::sync::atomic::Ordering::Relaxed),
                s.interaction.main_app_mode == InteractionMode::Passive,
            )
        };

        if !tray_enabled && !is_engaged && !is_passive {
            log::info!("[Settings] Main App mode changed to non-passive and Tray is disabled. Stopping engine...");
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = stop_engine(app_clone).await;
            });
        } else if is_passive {
            log::info!(
                "[Settings] Main App mode changed to Passive. Ensuring engine is launched..."
            );
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = launch_engine(app_clone).await;
            });
        }

        // Notify VAD of mode change if Main App is owner
        let owner: crate::core::state::InteractionOwner = state
            .owner
            .load(std::sync::atomic::Ordering::Relaxed)
            .into();
        if owner == crate::core::state::InteractionOwner::MainWindow {
            if let Some(engine) = state.engine.lock().await.as_ref() {
                if let Ok(mode) =
                    serde_json::from_value::<crate::core::settings::InteractionMode>(value.clone())
                {
                    let _ = engine
                        .vad_tx
                        .send(crate::core::state::VadCommand::UpdateMode(mode));
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
        SettingReloadPolicy::Hot => "hot-applied",
        SettingReloadPolicy::WorkerCommand => "dispatched to worker",
        SettingReloadPolicy::Restart => "restart required",
    };

    let message = format!("{}.{} = {} — {}", domain, key, value, action_label);

    log::info!("[Settings] Updated: {}", message);

    if domain == "ui" && key == "theme" {
        let _ = app.emit("theme-changed", value.as_str().unwrap_or("dark"));
    }

    // Notify Pipeline Orchestrator of settings change for local caching
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let current_settings = state.settings.read().unwrap().clone();
        let _ = engine
            .pipeline_tx
            .send(crate::core::events::VoxEvent::SettingsUpdated(
                current_settings,
            ));
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
            settings.ui.accent_seed = value
                .as_str()
                .ok_or("accent_seed must be a string")?
                .to_string();
        }
        ("ui", "tray_enabled") => {
            settings.ui.tray_enabled = value.as_bool().ok_or("tray_enabled must be a boolean")?;
        }
        ("ui", "tray_blur_density") => {
            settings.ui.tray_blur_density = value
                .as_u64()
                .ok_or("tray_blur_density must be a positive integer")?
                as u32;
        }
        ("ui", "tray_glass_tint") => {
            settings.ui.tray_glass_tint =
                value.as_bool().ok_or("tray_glass_tint must be a boolean")?;
        }
        ("ui", "tray_history_limit") => {
            settings.ui.tray_history_limit = value
                .as_u64()
                .ok_or("tray_history_limit must be a positive integer")?
                as u32;
        }
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
        ("vad", "vad_backend") => {
            settings.vad.vad_backend = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid vad_backend: {}", e))?;
        }
        ("asr", "model") => {
            settings.asr.model = value.as_str().ok_or("model must be a string")?.to_string();
        }
        ("asr", "provider") => {
            settings.asr.provider = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT provider: {}", e))?;
        }
        ("asr", "transliterate_enabled") => {
            settings.asr.transliterate_enabled = value
                .as_bool()
                .ok_or("transliterate_enabled must be a boolean")?;
        }
        ("audio", "output_mode") => {
            settings.audio.output_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid output_mode: {}", e))?;
        }
        ("audio", "input_device") => {
            settings.audio.input_device = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid input_device: {}", e))?;
        }
        ("llm", "model") => {
            settings.llm.model = value.as_str().ok_or("model must be a string")?.to_string();
        }
        ("llm", "ctx_size") => {
            let val = value
                .as_u64()
                .ok_or("ctx_size must be a positive integer")?
                as u32;
            if let crate::core::settings::LlmProviderConfig::OpenAiCompat { .. } = settings.llm.provider {
                if val < 8192 {
                    return Err("Cloud/Server LLM providers require a minimum context size of 8192 tokens".to_string());
                }
            }
            settings.llm.ctx_size = val;
        }
        ("llm", "threads") => {
            settings.llm.threads =
                value.as_u64().ok_or("threads must be a positive integer")? as u32;
        }
        ("llm", "provider") => {
            let prov: crate::core::settings::LlmProviderConfig = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid provider: {}", e))?;
            if let crate::core::settings::LlmProviderConfig::OpenAiCompat { .. } = prov {
                if settings.llm.ctx_size < 8192 {
                    settings.llm.ctx_size = 8192;
                }
            }
            settings.llm.provider = prov;
        }
        ("interaction", "main_app_mode") => {
            settings.interaction.main_app_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid main_app_mode: {}", e))?;
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
        ("interaction", "tray_mode") => {
            settings.interaction.tray_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid tray_mode: {}", e))?;
        }
        ("telemetry", "enabled") => {
            settings.telemetry.enabled = value.as_bool().ok_or("enabled must be a boolean")?;
        }
        ("telemetry", "log_level") => {
            settings.telemetry.log_level = value
                .as_str()
                .ok_or("log_level must be a string")?
                .to_string();
        }
        ("tts", "provider") => {
            settings.tts.provider = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid TTS provider: {}", e))?;
        }
        ("tts", "voice") => {
            settings.tts.voice = value.as_i64().ok_or("voice must be an integer")? as i32;
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
        ("persistence", "private_mode") => {
            settings.persistence.private_mode =
                value.as_bool().ok_or("private_mode must be a boolean")?;
        }
        ("persistence", "max_sessions") => {
            settings.persistence.max_sessions = value
                .as_u64()
                .ok_or("max_sessions must be a positive integer")?
                as u32;
        }
        ("persistence", "retention_days") => {
            settings.persistence.retention_days = value
                .as_u64()
                .ok_or("retention_days must be a positive integer")?
                as u32;
        }
        ("assistant", "modular_prompt") => {
            settings.assistant.modular_prompt = value
                .as_str()
                .ok_or("modular_prompt must be a string")?
                .to_string();
        }
        ("assistant", "realtime_prompt") => {
            settings.assistant.realtime_prompt = value
                .as_str()
                .ok_or("realtime_prompt must be a string")?
                .to_string();
        }
        ("realtime", "provider") => {
            settings.realtime.provider = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid realtime provider: {}", e))?;
        }
        ("realtime", "gemini") => {
            settings.realtime.gemini = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid gemini config: {}", e))?;
        }
        ("realtime", "openai") => {
            settings.realtime.openai = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid openai config: {}", e))?;
        }
        ("realtime", "deepgram") => {
            settings.realtime.deepgram = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid deepgram config: {}", e))?;
        }
        ("realtime", "elevenlabs") => {
            settings.realtime.elevenlabs = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid elevenlabs config: {}", e))?;
        }
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
        ("memory", "max_personal_memory_share") => {
            let val = value.as_f64().ok_or("max_personal_memory_share must be a number")? as f32;
            if !(0.0..=1.0).contains(&val) {
                return Err("max_personal_memory_share must be between 0.0 and 1.0".to_string());
            }
            settings.memory.max_personal_memory_share = val;
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
                .ok_or("top_k_facts must be a positive integer")?
                as u32;
            if top_k == 0 || top_k > 100 {
                return Err("top_k_facts must be between 1 and 100".to_string());
            }
            settings.memory.top_k_facts = top_k;
        }
        ("memory", "max_hops") => {
            let max_hops = value
                .as_u64()
                .ok_or("max_hops must be a positive integer")?
                as u32;
            if max_hops == 0 || max_hops > 10 {
                return Err("max_hops must be between 1 and 10".to_string());
            }
            settings.memory.max_hops = max_hops;
        }
        ("memory", "semantic_similarity_cutoff") => {
            let val = value.as_f64().ok_or("semantic_similarity_cutoff must be a number")? as f32;
            if !(0.0..=1.0).contains(&val) {
                return Err("semantic_similarity_cutoff must be between 0.0 and 1.0".to_string());
            }
            settings.memory.semantic_similarity_cutoff = val;
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
                    let _ = engine
                        .vad_tx
                        .send(crate::core::state::VadCommand::UpdateThreshold(v as f32));
                    log::debug!("[Settings] VadCommand::UpdateThreshold({}) dispatched", v);
                }
            }
            ("vad", "ptt_noise_gate") => {
                if let Some(v) = value.as_f64() {
                    let _ = engine
                        .vad_tx
                        .send(crate::core::state::VadCommand::UpdateNoiseGate(v as f32));
                    log::debug!("[Settings] VadCommand::UpdateNoiseGate({}) dispatched", v);
                }
            }
            ("audio", "output_mode") => {
                if let Ok(mode) =
                    serde_json::from_value::<crate::core::settings::AudioOutputMode>(value.clone())
                {
                    let _ = engine
                        .vad_tx
                        .send(crate::core::state::VadCommand::UpdateAudioMode(mode));
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

#[tauri::command]
pub async fn check_llm_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
) -> Result<bool, String> {
    use crate::core::settings::LlmProviderConfig;
    use crate::services::llm::{LlmProvider, OpenAiCompatProvider};
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
                            .join(crate::services::llm::MODEL_DIR_LLM)
                            .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
                    }
                } else {
                    models_dir
                        .join(crate::services::llm::MODEL_DIR_LLM)
                        .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
                }
            } else {
                models_dir
                    .join(crate::services::llm::MODEL_DIR_LLM)
                    .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
            };

            Ok(llm_path.exists())
        }
        LlmProviderConfig::OpenAiCompat {
            base_url,
            model,
            api_key,
            provider_name,
        } => {
            let provider = OpenAiCompatProvider::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let healthy = tokio::task::spawn_blocking(move || provider.health_check())
                .await
                .map_err(|e| e.to_string())?;
            Ok(healthy)
        }
    }
}

#[tauri::command]
pub async fn check_stt_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::SttProviderConfig>,
) -> Result<bool, String> {
    use crate::core::settings::SttProviderConfig;
    use crate::utils::paths;

    let config = if let Some(prov) = provider {
        prov
    } else {
        let settings = state.settings.read().map_err(|e| e.to_string())?;
        settings.asr.provider.clone()
    };

    match config {
        SttProviderConfig::Embedded { model_type } => {
            let models_dir = paths::get().models.clone();
            let model_path = match model_type.as_str() {
                "nvidia_nemotron" => models_dir.join(crate::services::stt::MODEL_DIR_STT_NEMOTRON),
                _ => models_dir.join(crate::services::stt::MODEL_DIR_STT_QWEN),
            };
            Ok(model_path.exists())
        }
        SttProviderConfig::Cloud { .. } => {
            // Create the cloud provider and check its health (validates credentials)
            match crate::services::stt::providers::create_stt_provider(
                &config,
                &std::path::PathBuf::new(),
            ) {
                Ok(provider) => {
                    let healthy = provider.health_check();
                    Ok(healthy)
                }
                Err(e) => {
                    log::warn!("[Settings] Cloud STT provider health check failed: {}", e);
                    Ok(false)
                }
            }
        }
    }
}

#[tauri::command]
pub async fn check_tts_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::TtsProviderConfig>,
) -> Result<bool, String> {
    use crate::core::settings::TtsProviderConfig;
    use crate::utils::paths;

    let config = if let Some(prov) = provider {
        prov
    } else {
        let settings = state.settings.read().map_err(|e| e.to_string())?;
        settings.tts.provider.clone()
    };

    match config {
        TtsProviderConfig::Supertonic => {
            let models_dir = paths::get().models.clone();
            let model_path = models_dir.join(crate::services::tts::MODEL_DIR_TTS_SUPER);
            Ok(model_path.exists())
        }
        TtsProviderConfig::Chatterbox { .. } => {
            let models_dir = paths::get().models.clone();
            let model_path = models_dir.join(crate::services::tts::MODEL_DIR_TTS_CHATTERBOX);
            Ok(model_path.exists())
        }
        TtsProviderConfig::ChatterboxRemote { ref endpoint, .. } => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .map_err(|e| e.to_string())?;
            let health_url = format!("{}/health", endpoint.trim_end_matches('/'));
            match client.get(&health_url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        if let Ok(body) = resp.json::<serde_json::Value>().await {
                            if body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                                return Ok(true);
                            }
                        }
                    }
                    Ok(false)
                }
                Err(_) => Ok(false),
            }
        }
    }
}

#[tauri::command]
pub async fn list_llm_models(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
) -> Result<Vec<crate::core::settings::LlmModelInfo>, String> {
    use crate::core::settings::LlmProviderConfig;
    use crate::services::llm::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider};
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
            let llm_dir = paths::get()
                .models
                .join(crate::services::llm::MODEL_DIR_LLM);
            let models =
                EmbeddedProvider::list_models_in_dir(&llm_dir).map_err(|e| e.to_string())?;
            Ok(models)
        }
        LlmProviderConfig::OpenAiCompat {
            base_url,
            model,
            api_key,
            provider_name,
        } => {
            let provider = OpenAiCompatProvider::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let models = tokio::task::spawn_blocking(move || provider.list_models())
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(models)
        }
    }
}

#[tauri::command]
pub async fn probe_model_capabilities(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
    model_id: Option<String>,
) -> Result<crate::core::settings::ModelCapabilities, String> {
    use crate::services::llm::CapabilityProbeEngine;

    let config = match provider {
        Some(prov) => prov,
        None => {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            settings.llm.provider.clone()
        }
    };

    CapabilityProbeEngine::probe_capabilities(&config, model_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn setup_remote_server(
    app: tauri::AppHandle,
    connection_string: String,
    ssh_port: Option<u16>,
    identity_key_path: Option<String>,
    remote_path: String,
    server_port: u16,
) -> Result<(), String> {
    log::info!(
        "[SetupRemote] Triggering remote server setup. connection_string={}, ssh_port={:?}, identity_key_path={:?}, remote_path={}, server_port={}",
        connection_string,
        ssh_port,
        identity_key_path,
        remote_path,
        server_port
    );

    // 1. Resolve bundled script path from Tauri resource directory
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?
        .join("resources")
        .join("setup_server.sh");

    // Fallback: relative to CARGO_MANIFEST_DIR for dev mode
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("setup_server.sh");

    let script_path = if resource_path.exists() {
        resource_path
    } else if dev_path.exists() {
        log::info!("[SetupRemote] Using dev path: {:?}", dev_path);
        dev_path
    } else {
        return Err(format!(
            "Remote setup script not found at {:?} or {:?}",
            resource_path, dev_path
        ));
    };

    let app_handle = app.clone();
    
    tauri::async_runtime::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::process::Command;
        use std::process::Stdio;

        let mut cmd = Command::new("ssh");
        
        // Pass identity key path if provided
        if let Some(ref key_path) = identity_key_path {
            if !key_path.trim().is_empty() {
                cmd.arg("-i").arg(key_path);
            }
        }
        
        // Pass custom SSH port if provided
        if let Some(port_val) = ssh_port {
            cmd.arg("-p").arg(port_val.to_string());
        }
        
        // Non-interactive option for StrictHostKeyChecking to prevent blocking on TTY
        cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
        
        cmd.arg(&connection_string)
           .arg("bash")
           .arg("-s")
           .arg("--")
           .arg(&remote_path)
           .arg(&server_port.to_string());

        cmd.stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Failed to spawn ssh command: {}", e);
                log::error!("{}", err_msg);
                let _ = app_handle.emit("remote_setup_status", serde_json::json!({
                    "step": "failed",
                    "progress": 0,
                    "log_line": err_msg.clone(),
                    "error": err_msg
                }));
                return;
            }
        };

        // Write setup_server.sh script content to child stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let script_content = match tokio::fs::read_to_string(&script_path).await {
                Ok(content) => content,
                Err(e) => {
                    let err_msg = format!("Failed to read setup script file: {}", e);
                    let _ = app_handle.emit("remote_setup_status", serde_json::json!({
                        "step": "failed",
                        "progress": 0,
                        "log_line": err_msg.clone(),
                        "error": err_msg
                    }));
                    return;
                }
            };
            if let Err(e) = stdin.write_all(script_content.as_bytes()).await {
                log::warn!("Failed to write script to ssh stdin: {}", e);
            }
            let _ = stdin.flush().await;
            drop(stdin);
        }

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let app_handle_clone = app_handle.clone();
        let stdout_loop = async move {
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                log::info!("[RemoteSetup stdout] {}", line);
                
                let mut progress = 0;
                let mut step = "setup";
                
                if line.contains("Phase 1") {
                    step = "setup";
                    progress = 10;
                } else if line.contains("Phase 2") {
                    step = "sync";
                    progress = 25;
                } else if line.contains("Phase 3") {
                    step = "models";
                    progress = 40;
                } else if line.contains("Phase 4") {
                    step = "build";
                    progress = 75;
                } else if line.contains("Phase 5") {
                    step = "launch";
                    progress = 85;
                } else if line.contains("Phase 6") {
                    step = "health";
                    progress = 90;
                } else if line.contains("Phase 7") {
                    step = "smoke";
                    progress = 95;
                } else if line.contains("Smoke test passed") {
                    step = "complete";
                    progress = 100;
                }

                let _ = app_handle_clone.emit("remote_setup_status", serde_json::json!({
                    "step": step,
                    "progress": progress,
                    "log_line": line
                }));
            }
        };

        let app_handle_clone_err = app_handle.clone();
        let stderr_loop = async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                log::warn!("[RemoteSetup stderr] {}", line);
                let _ = app_handle_clone_err.emit("remote_setup_status", serde_json::json!({
                    "step": "log",
                    "progress": 0,
                    "log_line": line
                }));
            }
        };

        tokio::join!(stdout_loop, stderr_loop);

        match child.wait().await {
            Ok(status) => {
                if status.success() {
                    log::info!("[SetupRemote] Setup completed successfully.");
                    let _ = app_handle.emit("remote_setup_status", serde_json::json!({
                        "step": "complete",
                        "progress": 100,
                        "log_line": "Remote setup completed successfully!"
                    }));
                } else {
                    let err_msg = format!("SSH command exited with code: {:?}", status.code());
                    log::error!("[SetupRemote] {}", err_msg);
                    let _ = app_handle.emit("remote_setup_status", serde_json::json!({
                        "step": "failed",
                        "progress": 0,
                        "log_line": err_msg.clone(),
                        "error": err_msg
                    }));
                }
            }
            Err(e) => {
                let err_msg = format!("Failed to wait for SSH child: {}", e);
                log::error!("[SetupRemote] {}", err_msg);
                let _ = app_handle.emit("remote_setup_status", serde_json::json!({
                    "step": "failed",
                    "progress": 0,
                    "log_line": err_msg.clone(),
                    "error": err_msg
                }));
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_apply_setting_mutation_type_safety() {
        let mut settings = VoxSettings::default();

        // 1. Valid key and correct type
        let res = apply_setting_mutation(&mut settings, "ui", "theme", &json!("light"));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.ui.theme, "light");

        // 2. Invalid domain ("invalid_domain")
        let res = apply_setting_mutation(&mut settings, "invalid_domain", "theme", &json!("dark"));
        assert_eq!(res, Ok(false));

        // 3. Unknown key within valid domain
        let res = apply_setting_mutation(&mut settings, "ui", "unknown_key", &json!("val"));
        assert_eq!(res, Ok(false));

        // 4. Type mismatch: string passed to boolean field (tray_enabled)
        let res = apply_setting_mutation(&mut settings, "ui", "tray_enabled", &json!("not_a_bool"));
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("tray_enabled must be a boolean"));

        // 5. Type mismatch: string passed to numeric field (tray_blur_density)
        let res = apply_setting_mutation(&mut settings, "ui", "tray_blur_density", &json!("dense"));
        assert!(res.is_err());

        // 6. Type mismatch: boolean passed to string field (theme)
        let res = apply_setting_mutation(&mut settings, "ui", "theme", &json!(true));
        assert!(res.is_err());
    }

    #[test]
    fn test_setting_numeric_bounds() {
        let mut settings = VoxSettings::default();

        // --- VAD threshold bounds ---
        // Valid threshold (0.75)
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(0.75));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.vad.threshold, 0.75);

        // Lower bound (0.0)
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(0.0));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.vad.threshold, 0.0);

        // Upper bound (1.0)
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(1.0));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.vad.threshold, 1.0);

        // Below 0.0 -> Err
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(-0.1));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("threshold must be between 0.0 and 1.0"));

        // Above 1.0 -> Err
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(1.5));
        assert!(res.is_err());

        // --- Memory top_k_facts bounds ---
        // Valid top_k_facts
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(10));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.memory.top_k_facts, 10);

        // Lower boundary (1)
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(1));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.memory.top_k_facts, 1);

        // Upper boundary (100)
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(100));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.memory.top_k_facts, 100);

        // Zero (0) -> Out of bounds
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(0));
        assert!(res.is_err());

        // Over 100 (101) -> Out of bounds
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(101));
        assert!(res.is_err());
    }
}
