use tauri::{AppHandle, State, Emitter, Manager};
use crate::core::state::AppState;
use crate::core::settings::VoxSettings;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<VoxSettings, String> {
    Ok(state.settings.lock().await.clone())
}

#[tauri::command]
pub async fn update_theme(app: AppHandle, theme: String) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    let changed = {
        let mut settings = state.settings.lock().await;
        if settings.theme != theme {
            settings.theme = theme.clone();
            let _ = settings.save(&state.config_dir);
            true
        } else {
            false
        }
    };
    
    if changed {
        let _ = app.emit("theme-changed", theme);
    }
    Ok(())
}
