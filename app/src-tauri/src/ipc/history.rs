use crate::core::error::VoxIpcError;
use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use std::sync::Arc;
use tauri::State;

/// Retrieves the current in-memory transcript history (tray ephemeral buffer).
#[tauri::command]
pub async fn get_transcript_history(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, VoxIpcError> {
    let history = state.pipeline.transcript_history.lock();
    Ok(history.iter().cloned().collect())
}

const MAX_HISTORY_TEXT_CHARS: usize = 10_000;

/// Commits a completed session's full text to the ephemeral history buffer.
#[tauri::command]
pub async fn commit_session_to_history(
    text: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        let bounded_text: String = if trimmed.chars().count() > MAX_HISTORY_TEXT_CHARS {
            trimmed.chars().take(MAX_HISTORY_TEXT_CHARS).collect()
        } else {
            trimmed.to_string()
        };

        let limit = state
            .settings
            .read()
            .map(|s| s.history.tray_history_limit as usize)
            .unwrap_or_else(|p| {
                log::warn!("[History] Settings RwLock poisoned; using inner state limit.");
                p.into_inner().history.tray_history_limit as usize
            });

        let mut history = state.pipeline.transcript_history.lock();
        if history.front() != Some(&bounded_text) {
            history.push_front(bounded_text);
            while history.len() > limit {
                history.pop_back();
            }
        }
    }
    Ok(())
}

pub use crate::persistence::sessions::{SessionRow, TurnRow};

/// Returns all sessions ordered by most recent first.
#[tauri::command]
pub async fn get_sessions() -> Result<Vec<SessionRow>, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::sessions::fetch_sessions(&conn, 500)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Returns all turns for a given session, oldest first.
#[tauri::command]
pub async fn get_turns(session_id: i64) -> Result<Vec<TurnRow>, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::sessions::fetch_turns(&conn, session_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Deletes a session and all its turns (CASCADE).
#[tauri::command]
pub async fn delete_session(id: i64) -> Result<(), VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::sessions::delete_session(&conn, id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}
