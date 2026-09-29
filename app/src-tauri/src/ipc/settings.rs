use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::{
    config::{
        apply_setting_mutation, dispatch_worker_command, get_setting_reload_policy,
        handle_setting_side_effects, schedule_debounced_save, SettingReloadPolicy, VoxSettings,
    },
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    utils::paths,
};

/// Initial boot payload returned to the frontend during application initialization.
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResetSettingsResult {
    pub settings: VoxSettings,
    pub reload_policy: String,
    pub message: String,
}

/// Called by the frontend on mount to load initial settings snapshot and model paths.
#[tauri::command]
pub async fn get_settings<R: tauri::Runtime>(app: AppHandle<R>) -> Result<BootState, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    let settings = state
        .settings
        .read()
        .map_err(|e| VoxIpcError::Internal(e.to_string()))?
        .clone();
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

/// Generic settings update command.
#[tauri::command]
pub async fn update_setting<R: tauri::Runtime>(
    domain: String,
    key: String,
    value: serde_json::Value,
    app: AppHandle<R>,
) -> Result<SettingUpdateResult, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();

    let applied = {
        let mut settings = state
            .settings
            .write()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        apply_setting_mutation(&mut settings, &domain, &key, &value)
            .map_err(VoxIpcError::InvalidArgument)?
    };

    if !applied {
        return Err(VoxIpcError::InvalidArgument(format!(
            "Unknown setting: {}.{}",
            domain, key
        )));
    }

    let policy = get_setting_reload_policy(&domain, &key);
    handle_setting_side_effects(&app, &state, &domain, &key, &value).await;

    if policy == SettingReloadPolicy::WorkerCommand {
        dispatch_worker_command(&app, &domain, &key, &value).await;
    }

    schedule_debounced_save(state.inner().clone()).await;

    let action_label = match policy {
        SettingReloadPolicy::Hot => "hot-applied",
        SettingReloadPolicy::WorkerCommand => "dispatched to worker",
        SettingReloadPolicy::Restart => "restart required",
    };

    let message = format!("{}.{} = {} — {}", domain, key, value, action_label);
    log::info!("[Settings] Updated: {}", message);

    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
        log::warn!(
            "[Settings::Mutation] Failed to emit settings-updated: {}",
            e
        );
    }

    Ok(SettingUpdateResult {
        applied: true,
        reload_policy: policy.as_str().to_string(),
        message,
    })
}

/// Resets all settings to system defaults.
#[tauri::command]
pub async fn reset_settings<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<ResetSettingsResult, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    let defaults = VoxSettings::default();
    {
        let mut settings = state
            .settings
            .write()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        *settings = defaults.clone();
    }

    // Push subsystems that inspect live settings back into sync with defaults
    handle_setting_side_effects(
        &app,
        &state,
        "working_memory",
        "private_mode",
        &serde_json::json!(defaults.working_memory.private_mode),
    )
    .await;
    handle_setting_side_effects(
        &app,
        &state,
        "personal_memory",
        "context_retrieval_enabled",
        &serde_json::json!(defaults.personal_memory.context_retrieval_enabled),
    )
    .await;

    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
        log::warn!(
            "[Settings::Mutation] Failed to emit settings-updated: {}",
            e
        );
    }

    schedule_debounced_save(state.inner().clone()).await;

    Ok(ResetSettingsResult {
        settings: defaults,
        reload_policy: "restart".to_string(),
        message: "Settings reset to defaults. Restart required to reinitialize providers.".to_string(),
    })
}
