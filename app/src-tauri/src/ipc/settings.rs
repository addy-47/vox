use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, Manager, State};

use crate::{
    config::{
        apply_model_parameter_mutation, apply_setting_mutation, apply_settings_patch_tree,
        build_settings_contract, dispatch_worker_command, get_setting_reload_policy,
        handle_setting_side_effects, is_secret_key, request_engine_restart,
        schedule_debounced_save, supported_keys_for_domain, SettingReloadPolicy, SettingsContract,
        VoxSettings,
    },
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    services::tts::auth::redact_secret,
    utils::paths,
};

/// Initial boot payload returned to the frontend during application initialization.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BootState {
    pub settings: VoxSettings,
    pub models_dir_exists: bool,
    pub settings_path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RestartStatusDto {
    pub restarting: bool,
    pub target: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SettingUpdateResult {
    pub applied: bool,
    pub reload_policy: String,
    pub message: String,
    pub restart_scheduled: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResetSettingsResult {
    pub settings: VoxSettings,
    pub reload_policy: String,
    pub message: String,
    pub restart_scheduled: bool,
}

/// Queries the active engine restart status.
#[tauri::command]
pub async fn get_restart_status(
    state: State<'_, Arc<AppState>>,
) -> Result<RestartStatusDto, VoxIpcError> {
    let in_flight = state.restart_in_flight.load(Ordering::Relaxed);
    let requested = state.restart_requested.load(Ordering::Relaxed);
    let restarting = in_flight || requested;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(RestartStatusDto {
        restarting,
        target: if restarting {
            Some("engine".to_string())
        } else {
            None
        },
        timestamp,
    })
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

/// Returns the canonical settings contract including supported keys, reload policies,
/// secret keys, LLM cloud providers, TTS providers, and numeric defaults.
#[tauri::command]
pub async fn get_settings_contract() -> Result<SettingsContract, VoxIpcError> {
    Ok(build_settings_contract())
}

/// Strongly-typed classification of whether a `(domain, key)` setting
/// carries secret material. The catalog here MUST agree with
/// `config::secret_keys()` and `config::mutation::supported_keys_for_domain`
/// — drift between any of those three is a bug, and is guarded by the
/// `redact_and_supported_key_sets_agree` test. This is the ONLY place in the
/// system that answers the question "is this setting a secret?".
fn is_secret_setting(domain: &str, key: &str) -> bool {
    is_secret_key(domain, key)
}

/// Redacts secret-bearing setting values for logs and restart reasons.
/// The true value is still applied and persisted; only its display is masked.
fn redact_setting_value(domain: &str, key: &str, value: &serde_json::Value) -> serde_json::Value {
    if is_secret_setting(domain, key) {
        serde_json::Value::String(redact_secret().to_string())
    } else {
        value.clone()
    }
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

    if domain == "model_parameters" {
        apply_model_parameter_mutation(&state, &key, &value)
            .await
            .map_err(VoxIpcError::InvalidArgument)?;

        if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
            log::warn!("[Settings] Failed to emit settings-updated: {}", e);
        }

        return Ok(SettingUpdateResult {
            applied: true,
            reload_policy: "hot".to_string(),
            message: format!("model_parameters.{} updated in database", key),
            restart_scheduled: false,
        });
    }

    let applied = {
        let mut settings = state
            .settings
            .write()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        apply_setting_mutation(&mut settings, &domain, &key, &value).map_err(|e| {
            log::warn!("[Settings::Mutation] Rejected {}.{}: {}", domain, key, e);
            VoxIpcError::InvalidArgument(e)
        })?
    };

    if !applied {
        let valid = supported_keys_for_domain(&domain).join(", ");
        log::warn!(
            "[Settings::Mutation] Unknown setting: {}.{} (valid {} keys: {})",
            domain,
            key,
            domain,
            valid
        );
        return Err(VoxIpcError::InvalidArgument(format!(
            "Unknown setting: {}.{} (valid {} keys: {})",
            domain, key, domain, valid
        )));
    }

    let policy = get_setting_reload_policy(&domain, &key);
    handle_setting_side_effects(&app, &state, &domain, &key, &value).await;

    if policy == SettingReloadPolicy::WorkerCommand {
        dispatch_worker_command(&app, &domain, &key, &value).await;
    }

    let restart_scheduled = if policy == SettingReloadPolicy::Restart {
        let logged = redact_setting_value(&domain, &key, &value);
        let reason = format!("{}.{} = {}", domain, key, logged);
        request_engine_restart(&app, state.inner(), &reason)
    } else {
        false
    };

    schedule_debounced_save(state.inner().clone()).await;

    let action_label = match policy {
        SettingReloadPolicy::Hot => "hot-applied",
        SettingReloadPolicy::WorkerCommand => "dispatched to worker",
        SettingReloadPolicy::Restart if restart_scheduled => "engine restart scheduled",
        SettingReloadPolicy::Restart => "restart required",
    };

    let message = format!(
        "{}.{} = {} — {}",
        domain,
        key,
        redact_setting_value(&domain, &key, &value),
        action_label
    );
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
        restart_scheduled,
    })
}

/// Updates the client-local appearance (theme, preset, accents, and backgrounds).
#[tauri::command]
pub async fn set_appearance<R: tauri::Runtime>(
    theme: Option<String>,
    preset: Option<String>,
    dark_accent: Option<String>,
    light_accent: Option<String>,
    dark_bg: Option<String>,
    light_bg: Option<String>,
    app: AppHandle<R>,
) -> Result<SettingUpdateResult, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();

    {
        if let Some(ref da) = dark_accent {
            if !crate::config::mutation::is_valid_hex_color(da) {
                return Err(VoxIpcError::InvalidArgument(format!(
                    "dark_accent must be a valid #RRGGBB hex color, got '{da}'"
                )));
            }
        }
        if let Some(ref la) = light_accent {
            if !crate::config::mutation::is_valid_hex_color(la) {
                return Err(VoxIpcError::InvalidArgument(format!(
                    "light_accent must be a valid #RRGGBB hex color, got '{la}'"
                )));
            }
        }
        if let Some(ref db) = dark_bg {
            if !crate::config::mutation::is_valid_hex_color(db) {
                return Err(VoxIpcError::InvalidArgument(format!(
                    "dark_bg must be a valid #RRGGBB hex color, got '{db}'"
                )));
            }
        }
        if let Some(ref lb) = light_bg {
            if !crate::config::mutation::is_valid_hex_color(lb) {
                return Err(VoxIpcError::InvalidArgument(format!(
                    "light_bg must be a valid #RRGGBB hex color, got '{lb}'"
                )));
            }
        }

        let mut settings = state
            .settings
            .write()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        if let Some(t) = theme {
            settings.appearance.theme = t;
        }
        if let Some(p) = preset {
            settings.appearance.preset = p;
        }
        if let Some(da) = dark_accent {
            settings.appearance.dark_accent = da.to_uppercase();
        }
        if let Some(la) = light_accent {
            settings.appearance.light_accent = la.to_uppercase();
        }
        if let Some(db) = dark_bg {
            settings.appearance.dark_bg = db.to_uppercase();
        }
        if let Some(lb) = light_bg {
            settings.appearance.light_bg = lb.to_uppercase();
        }
    }

    schedule_debounced_save(state.inner().clone()).await;

    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
        log::warn!("[Settings] Failed to emit settings-updated: {}", e);
    }

    Ok(SettingUpdateResult {
        applied: true,
        reload_policy: "hot".to_string(),
        message: "Appearance updated locally".to_string(),
        restart_scheduled: false,
    })
}

/// Applies a batched settings patch atomically.
#[tauri::command]
pub async fn apply_settings_patch<R: tauri::Runtime>(
    patch: serde_json::Value,
    app: AppHandle<R>,
) -> Result<SettingUpdateResult, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();

    // Acquire the settings write lock once and apply the whole patch in memory.
    let changed = {
        let mut settings = state
            .settings
            .write()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        apply_settings_patch_tree(&mut settings, &patch).map_err(VoxIpcError::InvalidArgument)?
    };

    // Handle side effects, worker dispatch, and aggregate reload policy.
    let mut restart_needed = false;
    let mut worker_needed = false;
    for (domain, key) in &changed {
        let value = patch
            .get(domain)
            .and_then(|d| d.get(key))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        handle_setting_side_effects(&app, &state, domain, key, &value).await;

        let policy = get_setting_reload_policy(domain, key);
        if policy == SettingReloadPolicy::WorkerCommand {
            dispatch_worker_command(&app, domain, key, &value).await;
            worker_needed = true;
        } else if policy == SettingReloadPolicy::Restart {
            restart_needed = true;
        }
    }

    let restart_scheduled = if restart_needed {
        let reason = "apply_settings_patch".to_string();
        request_engine_restart(&app, state.inner(), &reason)
    } else {
        false
    };

    schedule_debounced_save(state.inner().clone()).await;

    let reload_policy = if restart_needed {
        "restart"
    } else if worker_needed {
        "worker_command"
    } else {
        "hot"
    };

    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
        log::warn!("[Settings::Patch] Failed to emit settings-updated: {}", e);
    }

    Ok(SettingUpdateResult {
        applied: !changed.is_empty(),
        reload_policy: reload_policy.to_string(),
        message: format!("Applied {} setting(s) — {}", changed.len(), reload_policy),
        restart_scheduled,
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
        "memory",
        "private_mode",
        &serde_json::json!(defaults.memory.private_mode),
    )
    .await;
    handle_setting_side_effects(
        &app,
        &state,
        "memory",
        "context_retrieval_enabled",
        &serde_json::json!(defaults.memory.context_retrieval_enabled),
    )
    .await;
    handle_setting_side_effects(
        &app,
        &state,
        "interaction",
        "web_search_enabled",
        &serde_json::json!(defaults.interaction.web_search_enabled),
    )
    .await;

    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
        log::warn!(
            "[Settings::Mutation] Failed to emit settings-updated: {}",
            e
        );
    }

    schedule_debounced_save(state.inner().clone()).await;

    let restart_scheduled =
        request_engine_restart(&app, state.inner(), "reset_settings -> defaults");

    Ok(ResetSettingsResult {
        settings: defaults,
        reload_policy: "restart".to_string(),
        message: if restart_scheduled {
            "Settings reset to defaults. Engine restart scheduled to reinitialize providers."
                .to_string()
        } else {
            "Settings reset to defaults. Restart required to reinitialize providers.".to_string()
        },
        restart_scheduled,
    })
}
