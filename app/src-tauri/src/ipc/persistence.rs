use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::sessions::{
        delete_session as delete_session_row, fetch_sessions, fetch_turns, update_session_metadata,
    },
    utils::paths,
};
pub use crate::{
    ipc::pipeline::ContinueSessionResult,
    persistence::sessions::{SessionRow, TurnRow},
};

/// Retrieves the cached dictation transcript history (tray ephemeral buffer).
#[tauri::command]
pub async fn get_transcript_history(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, VoxIpcError> {
    let file_path = paths::cache_dir().join("dictation_history.jsonl");
    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let content = match tokio::fs::read_to_string(&file_path).await {
        Ok(c) => c,
        Err(e) => {
            log::warn!(
                "[Ipc::History] Failed to read dictation history cache: {}",
                e
            );
            return Ok(Vec::new());
        }
    };

    let history: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                val.get("text")
                    .and_then(|t| t.as_str())
                    .map(ToString::to_string)
            } else {
                None
            }
        })
        .collect();

    *state.pipeline.transcript_history.lock() = history.iter().cloned().collect();

    Ok(history)
}

/// Returns sessions optionally filtered by project, ordered by pinned first then newest.
#[tauri::command]
pub async fn get_sessions(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SessionRow>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    fetch_sessions(&conn, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Returns all turns for a given session, oldest first.
#[tauri::command]
pub async fn get_turns(
    session_id: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<TurnRow>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    fetch_turns(&conn, session_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Updates session metadata fields (title, is_pinned, and/or project_id).
#[tauri::command]
pub async fn update_session(
    app: AppHandle,
    session_id: i64,
    title: Option<String>,
    is_pinned: Option<bool>,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    if title.is_none() && is_pinned.is_none() && project_id.is_none() {
        return Ok(());
    }
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    update_session_metadata(
        &conn,
        session_id,
        title.as_deref(),
        is_pinned,
        project_id.as_deref(),
    )
    .await
    .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    if let Err(e) = emit_ipc(&app, IpcEvent::SessionsChanged) {
        log::warn!("[IPC::History] Failed to emit SessionsChanged: {}", e);
    }

    Ok(())
}

/// Deletes a session. If hard is true, executes permanent purge; otherwise soft delete.
#[tauri::command]
pub async fn delete_session(
    app: AppHandle,
    session_id: i64,
    hard: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    delete_session_row(&conn, session_id, hard.unwrap_or(false))
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    if let Err(e) = emit_ipc(&app, IpcEvent::SessionsChanged) {
        log::warn!("[IPC::History] Failed to emit SessionsChanged: {}", e);
    }

    Ok(())
}
