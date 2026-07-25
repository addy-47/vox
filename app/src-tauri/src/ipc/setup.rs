use crate::core::state::AppState;
use crate::setup::manifest::VoxManifest;
use crate::setup::runtime_check::{verify_runtime, RuntimeReport};
use crate::setup::update_check::{
    check_app_updates, check_model_updates, ModelUpdateReport, UpdateReport,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateReport, String> {
    check_app_updates().await.map_err(|e| {
        log::error!("[IPC] App update check failed: {}", e);
        e.to_string()
    })
}

#[tauri::command]
pub async fn check_for_model_updates() -> Result<ModelUpdateReport, String> {
    check_model_updates().await.map_err(|e| {
        log::error!("[IPC] Model update check failed: {}", e);
        e.to_string()
    })
}

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
    let manifest_guard: tokio::sync::RwLockReadGuard<Option<VoxManifest>> =
        state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not fetched")?;

    let report = verify_runtime(Some(manifest));
    Ok(report)
}

#[tauri::command]
pub async fn start_model_setup(
    _app: AppHandle,
    state: State<'_, Arc<AppState>>,
    selected_ids: Option<Vec<String>>,
) -> Result<(), String> {
    let manifest_guard: tokio::sync::RwLockReadGuard<Option<VoxManifest>> =
        state.manifest.read().await;
    let manifest = manifest_guard
        .as_ref()
        .ok_or("Manifest not fetched")?
        .clone();

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
    let base_url = "https://huggingface.co/addyo07/vox-models/resolve/main".to_string();
    let state_clone = state.inner().clone();
    let app_clone = _app.clone();

    log::info!(
        "[SETUP] Initializing model setup task (State: {:p})",
        state.inner()
    );

    tauri::async_runtime::spawn(async move {
        // Filter models based on selection, ensuring mandatory model groups are always included
        let target_models: Vec<_> = if let Some(ids) = selected_ids {
            manifest
                .model_groups
                .into_iter()
                .filter(|g| ids.contains(&g.id) || g.files.iter().any(|f| f.required))
                .flat_map(|g| g.files)
                .collect()
        } else {
            manifest
                .model_groups
                .into_iter()
                .filter(|g| g.files.iter().any(|f| f.required))
                .flat_map(|g| g.files)
                .collect()
        };

        let model_ids: Vec<String> = target_models.iter().map(|m| m.id.clone()).collect();
        log::info!(
            "[SETUP] Initializing setup task for {} models: {:?}",
            target_models.len(),
            model_ids
        );

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
            log::info!(
                "[SETUP] [{}/{}] Starting setup for model: {}",
                success_count + 1,
                total_count,
                model.id
            );
            if let Err(e) = manager.setup_model(&model, &base_url, &models_dir).await {
                log::error!("[SETUP] Failed to setup model {}: {}", model.id, e);
                // Emit global failure event to frontend
                let _ = app_clone.emit("model_setup_error", e.to_string());
                let mut is_running = state_clone.setup_running.lock().await;
                *is_running = false;
                return;
            }
            success_count += 1;
            log::info!(
                "[SETUP] [{}/{}] Finished setup for model: {}",
                success_count,
                total_count,
                model.id
            );
        }

        log::info!(
            "[SETUP] All {} models successfully verified and ready.",
            total_count
        );
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
pub async fn complete_setup_wizard(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut settings = state.settings.write().unwrap();
        settings.setup.completed = true;
        settings.save().map_err(|e| e.to_string())?;
    }

    log::info!("[SETUP] Onboarding wizard marked as completed.");

    // Eagerly initialize the transliteration engine now that models have been downloaded
    if let Err(e) = crate::services::translit::init_transliteration_engine() {
        log::error!("[SETUP] Failed to initialize transliteration engine: {}", e);
    }

    // Close the wizard window immediately to prevent UI blocking
    if let Some(wizard_win) = app.get_webview_window("wizard") {
        let _ = wizard_win.close();
    }

    let state_clone = state.inner().clone();
    let app_clone = app.clone();

    // Spawn engine ownership transition and main window initialization in the background
    tauri::async_runtime::spawn(async move {
        // Transition InteractionOwner to Tray and update VAD synced state
        state_clone.owner.store(
            crate::core::state::InteractionOwner::Tray as u32,
            std::sync::atomic::Ordering::Relaxed,
        );
        if let Some(engine) = state_clone.engine.lock().await.as_ref() {
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(
                    crate::core::state::InteractionOwner::Tray,
                ));
        }

        // Show and focus the main window
        if let Some(main_win) = app_clone.get_webview_window("main") {
            let _ = main_win.eval("window.location.replace('/')");
            let _ = main_win.show();
            let _ = main_win.set_focus();
        }
    });

    Ok(())
}

async fn ensure_manifest_loaded(state: &State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut m = state.manifest.write().await;
    if m.is_none() {
        let manifest_path = crate::utils::paths::get()
            .models
            .join("models_manifest.json");

        // 1. Try reading from local models_manifest.json first
        if manifest_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                if let Ok(manifest) = serde_json::from_str::<VoxManifest>(&content) {
                    log::info!(
                        "[SETUP] Loaded manifest from local cache at {:?}",
                        manifest_path
                    );
                    *m = Some(manifest);
                    return Ok(());
                }
            }
            log::warn!("[SETUP] Local models_manifest.json was corrupted or unreadable. Fetching fresh from HF...");
        }

        // 2. Fall back to fetching from HF
        log::info!("[SETUP] Dynamic manifest fetch from HF...");
        let manifest = VoxManifest::fetch().await.map_err(|e| {
            log::error!("[SETUP] Dynamic manifest fetch failed: {}", e);
            format!("Manifest not loaded and failed to fetch: {}", e)
        })?;

        // 3. Save fetched manifest to local cache
        if let Ok(serialized) = serde_json::to_string_pretty(&manifest) {
            if let Err(e) = std::fs::write(&manifest_path, serialized) {
                log::error!("[SETUP] Failed to write local manifest cache: {}", e);
            } else {
                log::info!(
                    "[SETUP] Saved manifest to local cache at {:?}",
                    manifest_path
                );
            }
        }

        *m = Some(manifest);
    }
    Ok(())
}

#[tauri::command]
pub async fn check_model_exists(
    model_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    ensure_manifest_loaded(&state).await?;
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;

    let group = manifest.model_groups.iter().find(|g| g.id == model_id);

    let Some(group) = group else {
        return Ok(false);
    };

    let models_dir = crate::utils::paths::get().models.clone();

    for file in &group.files {
        let is_archive = file.archive_type.is_some();
        let dest_path = if is_archive {
            let p_str = file.path.as_str();
            if p_str.ends_with(".tar.gz") {
                models_dir.join(&p_str[..p_str.len() - 7])
            } else if p_str.ends_with(".zip") || p_str.ends_with(".tgz") {
                models_dir.join(&p_str[..p_str.len() - 4])
            } else {
                models_dir.join(&file.path)
            }
        } else {
            models_dir.join(&file.path)
        };
        let verified_path = models_dir.join(&file.path).with_extension("verified");

        let mut file_ok = false;
        if verified_path.exists() {
            if let Ok(marker) = crate::setup::manifest::VerifiedMarker::load(&verified_path) {
                if marker.sha256 == file.sha256
                    && dest_path.exists()
                    && (is_archive || marker.expected_size == file.size_bytes)
                {
                    file_ok = true;
                }
            }
        }

        if !file_ok && dest_path.exists() {
            if is_archive {
                let marker = crate::setup::manifest::VerifiedMarker {
                    model_id: Some(file.id.clone()),
                    sha256: file.sha256.clone(),
                    verified_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    expected_size: file.size_bytes,
                };
                let _ = marker.save(&verified_path);
                file_ok = true;
            } else if let Ok(metadata) = std::fs::metadata(&dest_path) {
                if metadata.len() == file.size_bytes {
                    let marker = crate::setup::manifest::VerifiedMarker {
                        model_id: Some(file.id.clone()),
                        sha256: file.sha256.clone(),
                        verified_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        expected_size: file.size_bytes,
                    };
                    let _ = marker.save(&verified_path);
                    file_ok = true;
                }
            }
        }

        if !file_ok {
            return Ok(false);
        }
    }

    Ok(true)
}

#[tauri::command]
pub async fn download_optional_model(
    model_id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let (target_models, manager) = {
        let manifest_guard = state.manifest.read().await;
        let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;

        let group = manifest
            .model_groups
            .iter()
            .find(|g| g.id == model_id)
            .ok_or_else(|| format!("Model group {} not found in manifest", model_id))?;

        (group.files.clone(), state.inner().model_manager.clone())
    };

    let app_clone = app.clone();
    let model_id_clone = model_id.clone();

    // Spawn isolated background task for the heavy optional download
    tauri::async_runtime::spawn(async move {
        let p = crate::utils::paths::get();
        let base_url = "https://huggingface.co/addyo07/vox-models/resolve/main";

        log::info!(
            "[SETUP] Starting optional model group download for: {} ({} files)",
            model_id_clone,
            target_models.len()
        );

        for model in target_models {
            if let Err(e) = manager.setup_model(&model, base_url, &p.models).await {
                log::error!(
                    "[SETUP] Failed to setup optional model file {}: {}",
                    model.id,
                    e
                );
                let _ = app_clone.emit(
                    "optional_model_failed",
                    (model_id_clone.clone(), e.to_string()),
                );
                return;
            }
        }

        log::info!(
            "[SETUP] Optional model group download completed: {}",
            model_id_clone
        );
        let _ = app_clone.emit("optional_model_complete", model_id_clone);
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

#[tauri::command]
pub async fn delete_model(model_id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    ensure_manifest_loaded(&state).await?;
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;

    let group = manifest
        .model_groups
        .iter()
        .find(|g| g.id == model_id)
        .ok_or_else(|| format!("Model group {} not found in manifest", model_id))?;

    let models_dir = crate::utils::paths::get().models.clone();

    for file in &group.files {
        let is_archive = file.archive_type.is_some();
        let dest_path = if is_archive {
            let p_str = file.path.as_str();
            if p_str.ends_with(".tar.gz") {
                models_dir.join(&p_str[..p_str.len() - 7])
            } else if p_str.ends_with(".zip") || p_str.ends_with(".tgz") {
                models_dir.join(&p_str[..p_str.len() - 4])
            } else {
                models_dir.join(&file.path)
            }
        } else {
            models_dir.join(&file.path)
        };
        let verified_path = models_dir.join(&file.path).with_extension("verified");

        // 1. Delete verified marker
        if verified_path.exists() {
            let _ = std::fs::remove_file(&verified_path);
        }

        // 2. Delete model file or directory
        if dest_path.exists() {
            log::info!("[Settings] Deleting: {:?}", dest_path);
            if dest_path.is_dir() {
                let _ = std::fs::remove_dir_all(&dest_path);
            } else {
                let _ = std::fs::remove_file(&dest_path);
            }
        }

        // 3. Folder cleanup
        if let Some(parent) = dest_path.parent() {
            if parent.exists() && parent != models_dir {
                if let Ok(entries) = std::fs::read_dir(parent) {
                    if entries.count() == 0 {
                        log::info!("[Settings] Deleting empty folder: {:?}", parent);
                        let _ = std::fs::remove_dir(parent);
                    }
                }
            }
        }
    }

    log::info!("[Settings] Deleted model group: {} successfully", model_id);
    Ok(())
}
