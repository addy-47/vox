use crate::core::state::AppState;
use crate::setup::manifest::{ModelEntry, VerifiedMarker, VoxManifest};
use crate::setup::model_manager::ModelManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;

pub async fn ensure_manifest_loaded(state: &AppState) -> Result<(), String> {
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

pub fn resolve_model_dest_path(file: &ModelEntry, models_dir: &Path) -> PathBuf {
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

pub fn is_model_file_present(file: &ModelEntry, models_dir: &Path) -> bool {
    let dest_path = resolve_model_dest_path(file, models_dir);
    let verified_path = models_dir.join(&file.path).with_extension("verified");
    let is_archive = file.archive_type.is_some();

    if verified_path.exists() {
        if let Ok(marker) = VerifiedMarker::load(&verified_path) {
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
            let marker = VerifiedMarker {
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

pub fn delete_model_file(file: &ModelEntry, models_dir: &Path) {
    let dest_path = resolve_model_dest_path(file, models_dir);
    let verified_path = models_dir.join(&file.path).with_extension("verified");

    if verified_path.exists() {
        if let Err(e) = std::fs::remove_file(&verified_path) {
            log::warn!(
                "[Setup] Failed to remove verified marker {:?}: {}",
                verified_path,
                e
            );
        }
    }

    if dest_path.exists() {
        if dest_path.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(&dest_path) {
                log::warn!(
                    "[Setup] Failed to remove model directory {:?}: {}",
                    dest_path,
                    e
                );
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
                        log::warn!(
                            "[Setup] Failed to remove empty parent directory {:?}: {}",
                            parent,
                            e
                        );
                    }
                }
            }
        }
    }
}

pub async fn execute_model_setup_task(
    manager: Arc<ModelManager>,
    target_models: Vec<ModelEntry>,
    base_url: String,
    models_dir: PathBuf,
    state: Arc<AppState>,
    _app: AppHandle,
) {
    let total_count = target_models.len();
    if total_count == 0 {
        log::warn!("[SETUP] No models selected for setup.");
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
            *state.setup_running.lock().await = false;
            return;
        }
    }

    log::info!("[SETUP] All requested models downloaded and verified successfully.");
    *state.setup_running.lock().await = false;
}

pub async fn check_model_exists(
    state: &AppState,
    model_id: Option<String>,
) -> Result<bool, String> {
    let id = model_id.ok_or("model_id required for exists action")?;
    ensure_manifest_loaded(state).await?;
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;

    let group = manifest.model_groups.iter().find(|g| g.id == id);
    let Some(group) = group else {
        return Ok(false);
    };

    if id == "edge_tts" || group.files.is_empty() {
        return Ok(true);
    }

    let models_dir = crate::utils::paths::get().models.clone();
    let exists = group
        .files
        .iter()
        .all(|f| is_model_file_present(f, &models_dir));
    Ok(exists)
}

pub async fn download_single_model(
    state: &AppState,
    model_id: Option<String>,
) -> Result<(), String> {
    let id = model_id.ok_or("model_id required for download action")?;
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

        let group = match manifest.model_groups.iter().find(|g| g.id == id) {
            Some(g) => g,
            None => {
                *state.setup_running.lock().await = false;
                return Err(format!("Model group {} not found in manifest", id));
            }
        };

        (group.files.clone(), state.model_manager.clone())
    };

    let setup_running = Arc::clone(&state.setup_running);
    tauri::async_runtime::spawn(async move {
        let p = crate::utils::paths::get();
        let base_url = "https://huggingface.co/addyo07/vox-models/resolve/main";

        for model in target_models {
            if let Err(e) = manager.setup_model(&model, base_url, &p.models).await {
                log::error!("[SETUP] Failed to setup optional model {}: {}", model.id, e);
                *setup_running.lock().await = false;
                return;
            }
        }
        *setup_running.lock().await = false;
    });

    Ok(())
}

pub async fn start_batch_setup(
    app: AppHandle,
    state: Arc<AppState>,
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
        state,
        app,
    ));

    Ok(())
}

pub async fn delete_model_group(state: &AppState, model_id: Option<String>) -> Result<(), String> {
    let id = model_id.ok_or("model_id required for delete action")?;
    ensure_manifest_loaded(state).await?;
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;

    let group = manifest
        .model_groups
        .iter()
        .find(|g| g.id == id)
        .ok_or_else(|| format!("Model group {} not found in manifest", id))?;

    let models_dir = crate::utils::paths::get().models.clone();
    for file in &group.files {
        delete_model_file(file, &models_dir);
    }

    log::info!("[Settings] Deleted model group: {} successfully", id);
    Ok(())
}
