use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::{
    core::{
        error::VoxIpcError,
        settings::{caps_for_id, get_preset_colors, ProviderCaps, VoiceProfile, VoxSettings},
        state::AppState,
    },
    services::tts::voice::get_voice_profiles,
    setup::manifest::{ModelGroup, VoxManifest},
    utils::paths,
};

/// Initial boot payload returned to the frontend during application initialization.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BootState {
    pub settings: VoxSettings,
    pub models_dir_exists: bool,
    pub settings_path: String,
}

/// Categorized catalog of available local and cloud AI models.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelCatalog {
    pub llm: Vec<ModelGroup>,
    pub stt: Vec<ModelGroup>,
    pub tts: Vec<ModelGroup>,
    pub vad: Vec<ModelGroup>,
    pub auxiliary: Vec<ModelGroup>,
    pub model_groups: Vec<ModelGroup>,
    pub voices: Vec<VoiceProfile>,
    pub preset_colors: Vec<String>,
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

/// Query the model manifest catalog filtered into distinct model categories.
#[tauri::command]
pub async fn get_model_catalog<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<ModelCatalog, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    let manifest_opt = {
        let guard = state.manifest.read().await;
        guard.clone()
    };

    let manifest = if let Some(m) = manifest_opt {
        m
    } else {
        let manifest_path = paths::get().models.join("models_manifest.json");
        if manifest_path.exists() {
            let p = manifest_path.clone();
            let content = tokio::task::spawn_blocking(move || std::fs::read_to_string(&p))
                .await
                .map_err(|e| VoxIpcError::Internal(e.to_string()))?
                .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
            serde_json::from_str::<VoxManifest>(&content)
                .map_err(|e| VoxIpcError::Internal(e.to_string()))?
        } else {
            return Err(VoxIpcError::NotFound("Manifest not available".to_string()));
        }
    };

    let groups = manifest.model_groups;

    let llm = groups
        .iter()
        .filter(|g| g.category == "llm")
        .cloned()
        .collect();
    let stt = groups
        .iter()
        .filter(|g| g.category == "stt")
        .cloned()
        .collect();
    let tts = groups
        .iter()
        .filter(|g| g.category == "tts")
        .cloned()
        .collect();
    let vad = groups
        .iter()
        .filter(|g| g.category == "vad")
        .cloned()
        .collect();
    let auxiliary = groups
        .iter()
        .filter(|g| {
            g.subcategory.as_deref() == Some("auxiliary")
                || matches!(
                    g.category.as_str(),
                    "translit" | "embedding" | "nli" | "classifier"
                )
        })
        .cloned()
        .collect();

    Ok(ModelCatalog {
        llm,
        stt,
        tts,
        vad,
        auxiliary,
        model_groups: groups,
        voices: get_voice_profiles(),
        preset_colors: get_preset_colors(),
    })
}

#[tauri::command]
pub fn get_provider_caps(provider_id: String) -> ProviderCaps {
    caps_for_id(&provider_id)
}
