//! ============================================================================
//! src/ipc/settings/mutation.rs — Setting update, mutation logic, and persistence commands
//! ============================================================================

use crate::core::settings::{reload_policy_for, InteractionMode, SettingReloadPolicy, VoxSettings};
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

// ─── IPC Commands ─────────────────────────────────────────────────────────────

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
            // Disable Tray: Revert interaction owner to MainWindow, hide window, and evaluate engine offload
            log::info!(
                "[Settings] Disabling Tray HUD: Reverting owner to MainWindow, hiding window, and evaluating engine offload..."
            );
            state.owner.store(
                crate::core::state::InteractionOwner::MainWindow as u32,
                std::sync::atomic::Ordering::Relaxed,
            );
            if let Some(engine) = state.engine.lock().await.as_ref() {
                let _ = engine.vad_tx.send(crate::core::state::VadCommand::UpdateOwner(
                    crate::core::state::InteractionOwner::MainWindow,
                ));
            }

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
                Box::new(current_settings),
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
pub(crate) fn apply_setting_mutation(
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
                .ok_or("ctx_size must be a positive integer")? as u32;
            if let crate::core::settings::LlmProviderConfig::OpenAiCompat { .. } =
                settings.llm.provider
            {
                if val < 8192 {
                    return Err(
                        "Cloud/Server LLM providers require a minimum context size of 8192 tokens"
                            .to_string(),
                    );
                }
            }
            settings.llm.ctx_size = val;
        }
        ("llm", "threads") => {
            settings.llm.threads =
                value.as_u64().ok_or("threads must be a positive integer")? as u32;
        }
        ("llm", "provider") => {
            let prov: crate::core::settings::LlmProviderConfig =
                serde_json::from_value(value.clone())
                    .map_err(|e| format!("Invalid provider: {}", e))?;
            if settings.llm.provider == prov {
                return Ok(false);
            }
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
            let val = value
                .as_f64()
                .ok_or("max_personal_memory_share must be a number")? as f32;
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
