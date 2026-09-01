use crate::core::settings::VoxSettings;
use crate::core::state::AppState;
use crate::utils::paths;
use tauri::{AppHandle, Manager, State};

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
    pub llm: Vec<crate::setup::manifest::ModelGroup>,
    pub asr: Vec<crate::setup::manifest::ModelGroup>,
    pub tts: Vec<crate::setup::manifest::ModelGroup>,
    pub vad: Vec<crate::setup::manifest::ModelGroup>,
    pub auxiliary: Vec<crate::setup::manifest::ModelGroup>,
    pub model_groups: Vec<crate::setup::manifest::ModelGroup>,
    pub voices: Vec<crate::core::settings::VoiceProfile>,
    pub preset_colors: Vec<String>,
}

/// Called by the frontend on mount to load initial settings snapshot and model paths.
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<BootState, String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let settings = state.settings.read().map_err(|e| e.to_string())?.clone();
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
pub async fn get_model_catalog(app: AppHandle) -> Result<ModelCatalog, String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let manifest_opt = {
        let guard = state.manifest.read().await;
        guard.clone()
    };

    let manifest = if let Some(m) = manifest_opt {
        m
    } else {
        let manifest_path = paths::get().models.join("models_manifest.json");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
            serde_json::from_str::<crate::setup::manifest::VoxManifest>(&content)
                .map_err(|e| e.to_string())?
        } else {
            return Err("Manifest not available".to_string());
        }
    };

    let groups = manifest.model_groups;

    let llm = groups
        .iter()
        .filter(|g| g.category == "llm")
        .cloned()
        .collect();
    let asr = groups
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
        asr,
        tts,
        vad,
        auxiliary,
        model_groups: groups,
        voices: crate::core::settings::get_voice_profiles(),
        preset_colors: crate::core::settings::get_preset_colors(),
    })
}
