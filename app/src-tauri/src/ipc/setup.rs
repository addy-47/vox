use std::sync::Arc;
use tauri::{AppHandle, State, Manager};
use crate::core::state::AppState;
use crate::setup::manifest::VoxManifest;
use crate::setup::runtime_check::{verify_runtime, RuntimeReport};

#[tauri::command]
pub async fn fetch_manifest(state: State<'_, Arc<AppState>>) -> Result<VoxManifest, String> {
    // Acquire write lock immediately to serialize potential concurrent callers
    // This ensures only one thread actually performs the fetch while others wait.
    let mut m = state.manifest.write().await;
    
    if let Some(ref manifest) = *m {
        return Ok(manifest.clone());
    }

    // Perform the fetch while holding the lock to prevent others from starting a fetch
    let manifest = VoxManifest::fetch().await.map_err(|e| {
        log::error!("[IPC] Manifest fetch failed: {}", e);
        e.to_string()
    })?;

    *m = Some(manifest.clone());
    Ok(manifest)
}

#[tauri::command]
pub async fn get_runtime_report(state: State<'_, Arc<AppState>>) -> Result<RuntimeReport, String> {
    let manifest_guard: tokio::sync::RwLockReadGuard<Option<VoxManifest>> = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not fetched")?;
    
    let report = verify_runtime(Some(manifest));
    Ok(report)
}

#[tauri::command]
pub async fn start_model_setup(_app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let manifest_guard: tokio::sync::RwLockReadGuard<Option<VoxManifest>> = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not fetched")?.clone();
    
    let manager = state.model_manager.clone();
    let models_dir = crate::utils::paths::get().models.clone();
    let base_url = "https://huggingface.co/addyo07/Vox/resolve/main".to_string();
    
    tauri::async_runtime::spawn(async move {
        // Sequentially setup each model defined in the manifest
        for model in manifest.models {
            log::info!("[SETUP] Starting setup for model: {}", model.id);
            if let Err(e) = manager.setup_model(&model, &base_url, &models_dir).await {
                log::error!("[SETUP] Failed to setup model {}: {}", model.id, e);
                return;
            }
        }
        
        log::info!("[SETUP] All models verified and ready.");
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_model_setup(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.model_manager.cancel();
    Ok(())
}

#[tauri::command]
pub async fn get_onboarding_status(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let settings = state.settings.read().unwrap();
    Ok(settings.setup.completed)
}

#[tauri::command]
pub async fn complete_setup_wizard(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    {
        let mut settings = state.settings.write().unwrap();
        settings.setup.completed = true;
        settings.save().map_err(|e| e.to_string())?;
    }
    
    log::info!("[SETUP] Onboarding wizard marked as completed.");

    // Window Transition
    if let Some(wizard_win) = app.get_webview_window("wizard") {
        let _ = wizard_win.close();
    }
    
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.show();
        let _ = main_win.set_focus();
    }

    Ok(())
}
#[tauri::command]
pub async fn reveal_wizard(app: AppHandle) -> Result<(), String> {
    if let Some(wizard_win) = app.get_webview_window("wizard") {
        let _ = wizard_win.show();
        let _ = wizard_win.set_focus();
    }
    Ok(())
}
