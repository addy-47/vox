use tauri::State;
use crate::core::state::AppState;

/// Retrieves the current in-memory transcript history.
#[tauri::command]
pub async fn get_transcript_history(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let history = state.pipeline.transcript_history.lock().unwrap();
    // Return a clone of the current buffer
    Ok(history.iter().cloned().collect())
}
