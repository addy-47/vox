use tauri::{AppHandle, State, Emitter, Manager};
use std::time::Duration;
use crate::core::state::AppState;
use crate::core::settings::{VoxSettings, SettingReloadPolicy, reload_policy_for};
use crate::utils::paths;

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

    let message = match policy {
        SettingReloadPolicy::Hot           => format!("{}.{} applied immediately", domain, key),
        SettingReloadPolicy::WorkerCommand => format!("{}.{} dispatched to worker", domain, key),
        SettingReloadPolicy::Restart       => format!("{}.{} staged — restart required to apply", domain, key),
    };

    log::debug!("[Settings] update_setting: {}.{} = {:?} (policy: {})", domain, key, value, policy.as_str());

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
        ("vad", "threshold") => {
            settings.vad.threshold = value.as_f64().ok_or("threshold must be a number")? as f32;
        }
        ("vad", "ptt_noise_gate") => {
            settings.vad.ptt_noise_gate = value.as_f64().ok_or("ptt_noise_gate must be a number")? as f32;
        }
        ("asr", "model") => {
            settings.asr.model = value.as_str().ok_or("model must be a string")?.to_string();
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
        ("interaction", "main_app_mode") => {
            settings.interaction.main_app_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid main_app_mode: {}", e))?;
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
        ("tts", "en_model") => {
            settings.tts.en_model = value.as_str().ok_or("en_model must be a string")?.to_string();
        }
        ("tts", "hi_model") => {
            settings.tts.hi_model = value.as_str().ok_or("hi_model must be a string")?.to_string();
        }
        ("persistence", "enabled") => {
            settings.persistence.enabled = value.as_bool().ok_or("enabled must be a boolean")?;
        }
        ("persistence", "max_sessions") => {
            settings.persistence.max_sessions = value.as_u64().ok_or("max_sessions must be a positive integer")? as u32;
        }
        ("persistence", "retention_days") => {
            settings.persistence.retention_days = value.as_u64().ok_or("retention_days must be a positive integer")? as u32;
        }
        ("assistant", "system_prompt") => {
            settings.assistant.system_prompt = value.as_str().ok_or("system_prompt must be a string")?.to_string();
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
            ("assistant", "system_prompt") => {
                // Phase 6.3: dispatch to LLM worker when it supports hot-update
                log::debug!("[Settings] system_prompt change staged (LLM worker dispatch in Phase 6.3)");
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
