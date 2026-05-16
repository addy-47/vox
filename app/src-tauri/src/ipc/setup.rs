use std::sync::Arc;
use tauri::{AppHandle, State, Manager, Emitter};
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
pub async fn start_model_setup(
    _app: AppHandle, 
    state: State<'_, Arc<AppState>>, 
    selected_ids: Option<Vec<String>>
) -> Result<(), String> {
    let manifest_guard: tokio::sync::RwLockReadGuard<Option<VoxManifest>> = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not fetched")?.clone();
    
    // Check if setup is already running to prevent concurrent task overlap
    {
        let mut is_running = state.setup_running.lock().await;
        if *is_running {
            log::warn!("[SETUP] Model setup already in progress. Ignoring duplicate request.");
            return Ok(());
        }
        *is_running = true;
    }

    let manager = state.model_manager.clone();
    let models_dir = crate::utils::paths::get().models.clone();
    let base_url = "https://huggingface.co/addyo07/Vox/resolve/main".to_string();
    let state_clone = state.inner().clone();
    let app_clone = _app.clone();
    
    log::info!("[SETUP] Initializing model setup task (State: {:p})", state.inner());
 
    tauri::async_runtime::spawn(async move {
        // Filter models based on selection or requirement
        let target_models: Vec<_> = if let Some(ids) = selected_ids {
            manifest.models.into_iter().filter(|m| ids.contains(&m.id)).collect()
        } else {
            manifest.models.into_iter().filter(|m| m.required).collect()
        };

        let model_ids: Vec<String> = target_models.iter().map(|m| m.id.clone()).collect();
        log::info!("[SETUP] Initializing setup task for {} models: {:?}", target_models.len(), model_ids);

        if target_models.is_empty() {
            log::warn!("[SETUP] No models selected for setup. Emitting completion immediately.");
            let _ = app_clone.emit("model_setup_complete", true);
            let mut is_running = state_clone.setup_running.lock().await;
            *is_running = false;
            return;
        }

        let mut success_count = 0;
        let total_count = target_models.len();

        for model in target_models {
            log::info!("[SETUP] [{}/{}] Starting setup for model: {}", success_count + 1, total_count, model.id);
            if let Err(e) = manager.setup_model(&model, &base_url, &models_dir).await {
                log::error!("[SETUP] Failed to setup model {}: {}", model.id, e);
                // Emit global failure event to frontend
                let _ = app_clone.emit("model_setup_error", e.to_string());
                let mut is_running = state_clone.setup_running.lock().await;
                *is_running = false;
                return;
            }
            success_count += 1;
            log::info!("[SETUP] [{}/{}] Finished setup for model: {}", success_count, total_count, model.id);
        }
        
        log::info!("[SETUP] All {} models successfully verified and ready.", total_count);
        let mut is_running = state_clone.setup_running.lock().await;
        *is_running = false;
        
        // Notify frontend that all models are ready
        let _ = app_clone.emit("model_setup_complete", true);
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
pub async fn check_model_exists(model_id: String, state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;
    
    let model = manifest.models.iter().find(|m| m.id == model_id)
        .ok_or_else(|| format!("Model {} not found in manifest", model_id))?;

    let models_dir = crate::utils::paths::get().models.clone();
    let dest_path = models_dir.join(&model.path);
    let verified_path = dest_path.with_extension("verified");

    Ok(verified_path.exists())
}

#[tauri::command]
pub async fn download_optional_model(
    model_id: String, 
    app: AppHandle, 
    state: State<'_, Arc<AppState>>
) -> Result<(), String> {
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;
    
    let model = manifest.models.iter().find(|m| m.id == model_id)
        .ok_or_else(|| format!("Model {} not found in manifest", model_id))?.clone();

    let app_clone = app.clone();
    
    // Spawn isolated background task for the heavy optional download
    tauri::async_runtime::spawn(async move {
        let p = crate::utils::paths::get();
        let base_url = "https://huggingface.co/addyo07/Vox/resolve/main"; 
        
        // Use a new manager to avoid state conflicts with the primary setup
        let manager = crate::setup::model_manager::ModelManager::new(Some(app_clone.clone()));
        
        if let Err(e) = manager.setup_model(&model, base_url, &p.models).await {
            log::error!("[DOWNLOAD] Failed to download {}: {}", model.id, e);
            let _ = app_clone.emit("optional_download_failed", e.to_string());
        } else {
            log::info!("[DOWNLOAD] Successfully downloaded optional model: {}", model.id);
            let _ = app_clone.emit("optional_download_complete", model.id);
        }
    });

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
