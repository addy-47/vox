use crate::core::state::AppState;
use crate::setup::manager_ops;
use crate::setup::manifest::VoxManifest;
use crate::setup::runtime_check::{verify_runtime, RuntimeReport};
use crate::setup::update_check::{
    check_app_updates, check_model_updates, ModelUpdateReport, UpdateReport,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, serde::Serialize)]
pub struct UnifiedUpdateReport {
    pub app: Option<UpdateReport>,
    pub models: Option<ModelUpdateReport>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ManageModelsPayload {
    pub action: String,
    pub model_id: Option<String>,
    pub selected_ids: Option<Vec<String>>,
}

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum ManageModelsResult {
    Status(bool),
    Done,
}

/// Check for available Vox application updates, model updates, or both.
#[tauri::command]
pub async fn check_updates(scope: Option<String>) -> Result<UnifiedUpdateReport, String> {
    let s = scope.unwrap_or_else(|| "all".to_string()).to_lowercase();
    let check_app = s == "all" || s == "app";
    let check_models = s == "all" || s == "models";

    let app = if check_app {
        Some(check_app_updates().await.map_err(|e| {
            log::error!("[IPC] App update check failed: {}", e);
            e.to_string()
        })?)
    } else {
        None
    };

    let models = if check_models {
        Some(check_model_updates().await.map_err(|e| {
            log::error!("[IPC] Model update check failed: {}", e);
            e.to_string()
        })?)
    } else {
        None
    };

    Ok(UnifiedUpdateReport { app, models })
}

/// Fetch the remote models manifest or return cached instance.
#[tauri::command]
pub async fn fetch_manifest(state: State<'_, Arc<AppState>>) -> Result<VoxManifest, String> {
    let mut m = state.manifest.write().await;

    if let Some(ref manifest) = *m {
        return Ok(manifest.clone());
    }

    let manifest = VoxManifest::fetch().await.map_err(|e| {
        log::error!("[IPC] Manifest fetch failed: {}", e);
        e.to_string()
    })?;

    *m = Some(manifest.clone());
    Ok(manifest)
}

/// Verify runtime system hardware and model readiness against the manifest.
#[tauri::command]
pub async fn get_runtime_report(state: State<'_, Arc<AppState>>) -> Result<RuntimeReport, String> {
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not fetched")?;
    Ok(verify_runtime(Some(manifest)))
}

/// Return whether the first-run onboarding setup wizard has completed.
#[tauri::command]
pub async fn get_onboarding_status(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let settings = state.settings.read().map_err(|e| e.to_string())?;
    Ok(settings.system.setup_completed)
}

/// Mark onboarding setup as completed, persist configuration, and focus main window.
#[tauri::command]
pub async fn complete_setup_wizard(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut settings = state.settings.write().map_err(|e| e.to_string())?;
        settings.system.setup_completed = true;
        settings
            .save()
            .map_err(|e| format!("Failed to save settings: {}", e))?;
    }

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(main_win) = app_clone.get_webview_window("main") {
            if let Err(e) = main_win.eval("window.location.replace('/')") {
                log::warn!("[Setup] Failed to eval replace on main window: {}", e);
            }
            if let Err(e) = main_win.show() {
                log::warn!("[Setup] Failed to show main window: {}", e);
            }
            if let Err(e) = main_win.set_focus() {
                log::warn!("[Setup] Failed to focus main window: {}", e);
            }
        }
    });

    Ok(())
}

/// Unified model manager command handling downloads, cancellations, presence checks, and deletion.
#[tauri::command]
pub async fn manage_models(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    payload: ManageModelsPayload,
) -> Result<ManageModelsResult, String> {
    match payload.action.to_lowercase().as_str() {
        "exists" | "check" => {
            let exists = manager_ops::check_model_exists(&state, payload.model_id).await?;
            Ok(ManageModelsResult::Status(exists))
        }
        "download" | "download_optional" => {
            manager_ops::download_single_model(&state, payload.model_id).await?;
            Ok(ManageModelsResult::Done)
        }
        "start_setup" | "setup" => {
            manager_ops::start_batch_setup(app, state.inner().clone(), payload.selected_ids)
                .await?;
            Ok(ManageModelsResult::Done)
        }
        "cancel" | "cancel_setup" => {
            state.model_manager.cancel();
            Ok(ManageModelsResult::Done)
        }
        "delete" => {
            manager_ops::delete_model_group(&state, payload.model_id).await?;
            Ok(ManageModelsResult::Done)
        }
        _ => Err(format!("Unknown manage_models action: {}", payload.action)),
    }
}

/// Bring the setup wizard window into foreground focus.
#[tauri::command]
pub async fn reveal_wizard(app: AppHandle) -> Result<(), String> {
    if let Some(wizard_win) = app.get_webview_window("wizard") {
        if let Err(e) = wizard_win.show() {
            log::warn!("[Setup] Failed to show wizard window: {}", e);
        }
        if let Err(e) = wizard_win.set_focus() {
            log::warn!("[Setup] Failed to focus wizard window: {}", e);
        }
    }
    Ok(())
}
