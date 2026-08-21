//! ============================================================================
//! src/ipc/dictation.rs — Tauri IPC Commands for Dictation Subsystem
//! ============================================================================

use crate::core::settings::DictationSettings;
use crate::core::state::AppState;
use crate::services::dictation::clipboard;
use tauri::{AppHandle, Emitter, State};

/// Query current dictation settings.
#[tauri::command]
pub async fn get_dictation_settings(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<DictationSettings, String> {
    let settings = state.settings.read().map_err(|e| e.to_string())?;
    Ok(settings.dictation.clone())
}

/// Query the last completed dictation transcript for recovery (FR-08).
#[tauri::command]
pub async fn get_last_dictation_transcript(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Option<String>, String> {
    let last = state.dictation_last_transcript.lock().clone();
    Ok(last)
}

/// Manually copy the last completed dictation transcript to the clipboard.
#[tauri::command]
pub async fn copy_last_dictation_transcript(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    let last = state.dictation_last_transcript.lock().clone();
    if let Some(text) = last {
        clipboard::set_text(&text).map_err(|e| format!("Failed to copy to clipboard: {:?}", e))?;
        let _ = app.emit(
            "dictation_transcript_copied",
            serde_json::json!({ "success": true }),
        );
        Ok(())
    } else {
        Err("No previous dictation transcript available to copy".to_string())
    }
}
