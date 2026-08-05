//! ============================================================================
//! src/ipc/settings/catalog.rs — Boot state and model catalog IPC query commands
//! ============================================================================

use crate::core::settings::VoxSettings;
use crate::core::state::AppState;
use crate::utils::paths;
use tauri::{AppHandle, Manager, State};

// ─── Response Types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct BootState {
    pub settings: VoxSettings,
    pub models_dir_exists: bool,
    pub settings_path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelCatalog {
    pub llm: Vec<crate::core::settings::ModelMetadata>,
    pub asr: Vec<crate::core::settings::ModelMetadata>,
    pub tts: Vec<crate::core::settings::ModelMetadata>,
    pub voices: Vec<crate::core::settings::VoiceProfile>,
    pub preset_colors: Vec<String>,
}

// ─── IPC Commands ─────────────────────────────────────────────────────────────

/// Called by the frontend on mount.
///
/// Returns the full settings snapshot plus directory health status.
/// The frontend should boot into a loading/splash state and render only
/// after this resolves successfully.
#[tauri::command]
pub async fn request_boot_state(app: AppHandle) -> Result<BootState, String> {
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

#[tauri::command]
pub async fn request_model_catalog() -> Result<ModelCatalog, String> {
    Ok(ModelCatalog {
        llm: crate::core::settings::get_llm_metadata(),
        asr: crate::core::settings::get_asr_metadata(),
        tts: crate::core::settings::get_tts_metadata(),
        voices: crate::core::settings::get_voice_profiles(),
        preset_colors: crate::core::settings::get_preset_colors(),
    })
}

/// Returns the current settings snapshot.
#[tauri::command]
pub async fn get_settings(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<VoxSettings, String> {
    state
        .settings
        .read()
        .map_err(|e| e.to_string())
        .map(|s| s.clone())
}
