use crate::core::settings::DictationSettings;
use crate::core::state::AppState;
use crate::services::dictation::clipboard;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

/// Returns current dictation settings.
#[tauri::command]
pub async fn get_dictation_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<DictationSettings, String> {
    let settings = state.settings.read().map_err(|e| e.to_string())?;
    Ok(settings.dictation.clone())
}

/// Returns the last completed dictation transcript for recovery.
#[tauri::command]
pub async fn get_last_dictation_transcript(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<String>, String> {
    let last = state.dictation_last_transcript.lock().clone();
    Ok(last)
}

/// Manually copies the last completed dictation transcript to the system clipboard.
#[tauri::command]
pub async fn copy_last_dictation_transcript(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let last = state.dictation_last_transcript.lock().clone();
    if let Some(text) = last {
        clipboard::set_text(&text).map_err(|e| format!("Failed to copy to clipboard: {:?}", e))?;
        if let Err(e) = app.emit(
            "dictation_transcript_copied",
            serde_json::json!({ "success": true }),
        ) {
            log::warn!("[Dictation] Failed to emit transcript copied event: {}", e);
        }
        Ok(())
    } else {
        Err("No previous dictation transcript available to copy".to_string())
    }
}
