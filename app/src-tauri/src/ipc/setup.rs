use crate::core::state::AppState;
use crate::setup::manifest::VoxManifest;
use crate::setup::runtime_check::{verify_runtime, RuntimeReport};
use crate::setup::update_check::{
    check_app_updates, check_model_updates, ModelUpdateReport, UpdateReport,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

/// Check for available Vox desktop application updates.
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateReport, String> {
    check_app_updates().await.map_err(|e| {
        log::error!("[IPC] App update check failed: {}", e);
        e.to_string()
    })
}

/// Check for available model updates against the remote manifest.
#[tauri::command]
pub async fn check_for_model_updates() -> Result<ModelUpdateReport, String> {
    check_model_updates().await.map_err(|e| {
        log::error!("[IPC] Model update check failed: {}", e);
        e.to_string()
    })
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
    let manifest_guard: tokio::sync::RwLockReadGuard<Option<VoxManifest>> =
        state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not fetched")?;

    let report = verify_runtime(Some(manifest));
    Ok(report)
}

async fn execute_model_setup_task(
    manager: Arc<crate::setup::model_manager::ModelManager>,
    target_models: Vec<crate::setup::manifest::ModelEntry>,
    base_url: String,
    models_dir: std::path::PathBuf,
    state: Arc<AppState>,
    app: AppHandle,
) {
    let total_count = target_models.len();
    if total_count == 0 {
        log::warn!("[SETUP] No models selected for setup. Emitting completion immediately.");
        if let Err(e) = app.emit("model_setup_complete", true) {
            log::warn!("[Setup] Failed to emit model_setup_complete: {}", e);
        }
        *state.setup_running.lock().await = false;
        return;
    }

    for (idx, model) in target_models.iter().enumerate() {
        log::info!(
            "[SETUP] [{}/{}] Setting up model: {}",
            idx + 1,
            total_count,
            model.id
        );
        if let Err(e) = manager.setup_model(model, &base_url, &models_dir).await {
            log::error!("[SETUP] Failed to setup model {}: {}", model.id, e);
            if let Err(emit_err) = app.emit::<String>("model_setup_error", e.to_string()) {
                log::warn!("[Setup] Failed to emit model_setup_error: {}", emit_err);
            }
            *state.setup_running.lock().await = false;
            return;
        }
    }

    log::info!(
        "[SETUP] All {} models successfully verified and ready.",
        total_count
    );
    *state.setup_running.lock().await = false;
    if let Err(e) = app.emit("model_setup_complete", true) {
        log::warn!("[Setup] Failed to emit model_setup_complete: {}", e);
    }
}

/// Begin downloading and verifying required or selected models in the background.
#[tauri::command]
pub async fn start_model_setup(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    selected_ids: Option<Vec<String>>,
) -> Result<(), String> {
    let manifest = {
        let guard = state.manifest.read().await;
        guard.as_ref().ok_or("Manifest not fetched")?.clone()
    };

    {
        let mut is_running = state.setup_running.lock().await;
        if *is_running {
            log::warn!("[SETUP] Model setup already in progress. Ignoring duplicate request.");
            return Ok(());
        }
        *is_running = true;
    }

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

    let manager = state.model_manager.clone();
    let models_dir = crate::utils::paths::get().models.clone();
    let base_url = "https://huggingface.co/addyo07/vox-models/resolve/main".to_string();

    tauri::async_runtime::spawn(execute_model_setup_task(
        manager,
        target_models,
        base_url,
        models_dir,
        state.inner().clone(),
        app,
    ));

    Ok(())
}

/// Cancel an ongoing model download/verification operation.
#[tauri::command]
pub async fn cancel_model_setup(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.model_manager.cancel();
    Ok(())
}

/// Check if the first-run onboarding setup wizard has been marked as completed.
#[tauri::command]
pub async fn get_onboarding_status(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let settings = state.settings.read().map_err(|e| e.to_string())?;
    Ok(settings.system.setup_completed)
}

/// Finalize setup wizard, save settings, close wizard window, and reveal main app.
#[tauri::command]
pub async fn complete_setup_wizard(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut settings = state.settings.write().map_err(|e| e.to_string())?;
        settings.system.setup_completed = true;
        settings.save().map_err(|e| e.to_string())?;
    }

    log::info!("[SETUP] Onboarding wizard marked as completed.");

    if let Err(e) = crate::services::translit::init_transliteration_engine() {
        log::error!("[SETUP] Failed to initialize transliteration engine: {}", e);
    }

    if let Some(wizard_win) = app.get_webview_window("wizard") {
        if let Err(e) = wizard_win.close() {
            log::warn!("[Setup] Failed to close wizard window: {}", e);
        }
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

async fn ensure_manifest_loaded(state: &State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut m = state.manifest.write().await;
    if m.is_none() {
        let manifest_path = crate::utils::paths::get()
            .models
            .join("models_manifest.json");

        if manifest_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                if let Ok(manifest) = serde_json::from_str::<VoxManifest>(&content) {
                    *m = Some(manifest);
                    return Ok(());
                }
            }
        }

        let manifest = VoxManifest::fetch()
            .await
            .map_err(|e| format!("Manifest not loaded and failed to fetch: {}", e))?;

        if let Ok(serialized) = serde_json::to_string_pretty(&manifest) {
            if let Err(e) = std::fs::write(&manifest_path, serialized) {
                log::warn!("[Setup] Failed to write manifest to disk: {}", e);
            }
        }

        *m = Some(manifest);
    }
    Ok(())
}

fn resolve_model_dest_path(
    file: &crate::setup::manifest::ModelEntry,
    models_dir: &std::path::Path,
) -> std::path::PathBuf {
    if file.archive_type.is_some() {
        let p_str = file.path.as_str();
        if let Some(stripped) = p_str.strip_suffix(".tar.gz") {
            models_dir.join(stripped)
        } else if let Some(stripped) = p_str
            .strip_suffix(".zip")
            .or_else(|| p_str.strip_suffix(".tgz"))
        {
            models_dir.join(stripped)
        } else {
            models_dir.join(&file.path)
        }
    } else {
        models_dir.join(&file.path)
    }
}

fn is_model_file_present(
    file: &crate::setup::manifest::ModelEntry,
    models_dir: &std::path::Path,
) -> bool {
    let dest_path = resolve_model_dest_path(file, models_dir);
    let verified_path = models_dir.join(&file.path).with_extension("verified");
    let is_archive = file.archive_type.is_some();

    if verified_path.exists() {
        if let Ok(marker) = crate::setup::manifest::VerifiedMarker::load(&verified_path) {
            if marker.sha256 == file.sha256
                && dest_path.exists()
                && (is_archive || marker.expected_size == file.size_bytes)
            {
                return true;
            }
        }
    }

    if dest_path.exists() {
        let size_matches = is_archive
            || std::fs::metadata(&dest_path)
                .map(|m| m.len() == file.size_bytes)
                .unwrap_or(false);
        if size_matches {
            let marker = crate::setup::manifest::VerifiedMarker {
                model_id: Some(file.id.clone()),
                sha256: file.sha256.clone(),
                verified_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                expected_size: file.size_bytes,
            };
            if let Err(e) = marker.save(&verified_path) {
                log::warn!("[Setup] Failed to save verification marker: {}", e);
            }
            return true;
        }
    }

    false
}

/// Check if a specific model group exists on disk and has valid SHA verification markers.
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

    if model_id == "edge_tts" || group.files.is_empty() {
        return Ok(true);
    }

    let models_dir = crate::utils::paths::get().models.clone();
    Ok(group
        .files
        .iter()
        .all(|f| is_model_file_present(f, &models_dir)))
}

/// Download an optional model group in the background.
#[tauri::command]
pub async fn download_optional_model(
    model_id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut setup_lock = state.setup_running.lock().await;
        if *setup_lock {
            return Err("Model download or setup is already in progress".to_string());
        }
        *setup_lock = true;
    }

    let (target_models, manager) = {
        let manifest_guard = state.manifest.read().await;
        let manifest = match manifest_guard.as_ref() {
            Some(m) => m,
            None => {
                *state.setup_running.lock().await = false;
                return Err("Manifest not loaded".to_string());
            }
        };

        let group = match manifest.model_groups.iter().find(|g| g.id == model_id) {
            Some(g) => g,
            None => {
                *state.setup_running.lock().await = false;
                return Err(format!("Model group {} not found in manifest", model_id));
            }
        };

        (group.files.clone(), state.inner().model_manager.clone())
    };

    let setup_running = Arc::clone(&state.setup_running);
    tauri::async_runtime::spawn(async move {
        let p = crate::utils::paths::get();
        let base_url = "https://huggingface.co/addyo07/vox-models/resolve/main";

        for model in target_models {
            if let Err(e) = manager.setup_model(&model, base_url, &p.models).await {
                log::error!("[SETUP] Failed to setup optional model {}: {}", model.id, e);
                if let Err(emit_err) = app.emit("optional_model_failed", (model_id.clone(), e.to_string())) {
                    log::warn!("[Setup] Failed to emit optional_model_failed: {}", emit_err);
                }
                *setup_running.lock().await = false;
                return;
            }
        }
        if let Err(e) = app.emit("optional_model_complete", model_id) {
            log::warn!("[Setup] Failed to emit optional_model_complete: {}", e);
        }
        *setup_running.lock().await = false;
    });

    Ok(())
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

fn delete_model_file(file: &crate::setup::manifest::ModelEntry, models_dir: &std::path::Path) {
    let dest_path = resolve_model_dest_path(file, models_dir);
    let verified_path = models_dir.join(&file.path).with_extension("verified");

    if verified_path.exists() {
        if let Err(e) = std::fs::remove_file(&verified_path) {
            log::warn!("[Setup] Failed to remove verified marker {:?}: {}", verified_path, e);
        }
    }

    if dest_path.exists() {
        if dest_path.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(&dest_path) {
                log::warn!("[Setup] Failed to remove model directory {:?}: {}", dest_path, e);
            }
        } else if let Err(e) = std::fs::remove_file(&dest_path) {
            log::warn!("[Setup] Failed to remove model file {:?}: {}", dest_path, e);
        }
    }

    if let Some(parent) = dest_path.parent() {
        if parent.exists() && parent != models_dir {
            if let Ok(entries) = std::fs::read_dir(parent) {
                if entries.count() == 0 {
                    if let Err(e) = std::fs::remove_dir(parent) {
                        log::warn!("[Setup] Failed to remove empty parent directory {:?}: {}", parent, e);
                    }
                }
            }
        }
    }
}

/// Delete a model group and associated verification markers from local storage.
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
        delete_model_file(file, &models_dir);
    }

    log::info!("[Settings] Deleted model group: {} successfully", model_id);
    Ok(())
}
