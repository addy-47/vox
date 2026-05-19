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
    let base_url = "https://huggingface.co/addyo07/vox-models/resolve/main".to_string();
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
        state_clone.owner.store(crate::core::state::InteractionOwner::Tray as u32, std::sync::atomic::Ordering::Relaxed);
        if let Some(engine) = state_clone.engine.lock().await.as_ref() {
            let _ = engine.vad_tx.send(crate::core::state::VadCommand::UpdateOwner(crate::core::state::InteractionOwner::Tray));
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
        let manifest_path = crate::utils::paths::get().models.join("manifest.json");

        // 1. Try reading from local manifest.json first
        if manifest_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                if let Ok(manifest) = serde_json::from_str::<VoxManifest>(&content) {
                    log::info!("[SETUP] Loaded manifest from local cache at {:?}", manifest_path);
                    *m = Some(manifest);
                    return Ok(());
                }
            }
            log::warn!("[SETUP] Local manifest.json was corrupted or unreadable. Fetching fresh from HF...");
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
                log::info!("[SETUP] Saved manifest to local cache at {:?}", manifest_path);
            }
        }

        *m = Some(manifest);
    }
    Ok(())
}

fn get_model_group_ids(model_id: &str) -> Vec<String> {
    match model_id {
        "ten_vad" => vec!["ten_vad".to_string()],
        "translit" => vec![
            "translit_encoder".to_string(),
            "translit_decoder".to_string(),
            "translit_input_vocab".to_string(),
            "translit_target_vocab".to_string(),
        ],
        "qwen3-asr" => vec![
            "stt_conv_frontend".to_string(),
            "stt_encoder".to_string(),
            "stt_decoder".to_string(),
            "stt_vocab".to_string(),
            "stt_merges".to_string(),
            "stt_config".to_string(),
        ],
        "gemma4" => vec!["llm_gemma_4_q4_k_m".to_string()],
        "kokoro" => vec![
            "tts_kokoro_onnx".to_string(),
            "tts_kokoro_voices".to_string(),
            "tts_kokoro_tokens".to_string(),
            "tts_kokoro_espeak_ng_data".to_string(),
        ],
        "piper_hi" => vec![
            "tts_hi_priyamvada_onnx".to_string(),
            "tts_hi_priyamvada_config".to_string(),
            "tts_piper_hi_espeak_ng_data".to_string(),
        ],
        other => vec![other.to_string()],
    }
}

#[tauri::command]
pub async fn check_model_exists(model_id: String, state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    ensure_manifest_loaded(&state).await?;
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;
    
    let group_ids = get_model_group_ids(&model_id);
    let models_dir = crate::utils::paths::get().models.clone();
    
    for mapped_id in group_ids {
        let model = manifest.models.iter().find(|m| m.id == mapped_id);
        if model.is_none() {
            return Ok(false);
        }
        let model = model.unwrap();
        
        let dest_path = models_dir.join(&model.path);
        let verified_path = dest_path.with_extension("verified");

        if verified_path.exists() {
            continue;
        }

        if dest_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&dest_path) {
                if metadata.len() == model.size_bytes {
                    let marker = crate::setup::manifest::VerifiedMarker {
                        sha256: model.sha256.clone(),
                        verified_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        expected_size: model.size_bytes,
                    };
                    let _ = marker.save(&verified_path);
                    continue;
                }
            }
        }
        
        return Ok(false);
    }

    Ok(true)
}

#[tauri::command]
pub async fn download_optional_model(
    model_id: String, 
    app: AppHandle, 
    state: State<'_, Arc<AppState>>
) -> Result<(), String> {
    ensure_manifest_loaded(&state).await?;
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;
    
    let group_ids = get_model_group_ids(&model_id);
    let mut target_models = Vec::new();
    
    for mapped_id in group_ids {
        let model = manifest.models.iter().find(|m| m.id == mapped_id)
            .ok_or_else(|| format!("Model {} not found in manifest", mapped_id))?.clone();
        target_models.push(model);
    }

    let app_clone = app.clone();
    let model_id_clone = model_id.clone();
    
    // Spawn isolated background task for the heavy optional download
    tauri::async_runtime::spawn(async move {
        let p = crate::utils::paths::get();
        let base_url = "https://huggingface.co/addyo07/vox-models/resolve/main"; 
        
        // Use a new manager to avoid state conflicts with the primary setup
        let manager = crate::setup::model_manager::ModelManager::new(Some(app_clone.clone()));
        
        let mut failed = false;
        let mut last_err = String::new();
        
        for model in target_models {
            if let Err(e) = manager.setup_model(&model, base_url, &p.models).await {
                log::error!("[DOWNLOAD] Failed to download {}: {}", model.id, e);
                last_err = e.to_string();
                failed = true;
                break;
            }
        }
        
        if failed {
            let _ = app_clone.emit("optional_download_failed", last_err);
        } else {
            log::info!("[DOWNLOAD] Successfully downloaded optional model group: {}", model_id_clone);
            let _ = app_clone.emit("optional_download_complete", model_id_clone);
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

#[tauri::command]
pub async fn delete_model(model_id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    ensure_manifest_loaded(&state).await?;
    let manifest_guard = state.manifest.read().await;
    let manifest = manifest_guard.as_ref().ok_or("Manifest not loaded")?;
    
    let group_ids = get_model_group_ids(&model_id);
    let models_dir = crate::utils::paths::get().models.clone();
    
    for mapped_id in group_ids {
        let model = manifest.models.iter().find(|m| m.id == mapped_id)
            .ok_or_else(|| format!("Model {} not found in manifest", mapped_id))?;

        let dest_path = models_dir.join(&model.path);
        let verified_path = dest_path.with_extension("verified");

        // 1. Delete verified marker
        if verified_path.exists() {
            let _ = std::fs::remove_file(&verified_path);
        }

        // 2. Delete model file
        if dest_path.exists() {
            log::info!("[Settings] Deleting file: {:?}", dest_path);
            let _ = std::fs::remove_file(&dest_path);
        }

        // 3. Folder cleanup
        if model_id == "qwen3-asr" || model_id == "kokoro" || model_id == "piper_hi" || model_id == "translit" {
            if let Some(parent) = dest_path.parent() {
                if parent.exists() && parent != models_dir {
                    log::info!("[Settings] Deleting folder: {:?}", parent);
                    let _ = std::fs::remove_dir_all(parent);
                }
            }
        }
    }

    log::info!("[Settings] Deleted model: {} successfully", model_id);
    Ok(())
}
