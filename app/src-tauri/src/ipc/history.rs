use std::sync::{atomic::Ordering, Arc};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

pub use crate::persistence::sessions::{SessionRow, TurnRow};
use crate::{
    core::{
        constants::SYSTEM_PROMPT_MODULAR,
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::sessions::{
        create_session as db_create_session, delete_session as delete_session_row,
        fetch_session_by_id, fetch_sessions, fetch_turns, update_session_metadata,
    },
    pipeline::{init_new_session, resume_session},
    utils::paths,
};

/// Result payload for continuing an existing session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinueSessionResult {
    pub session: SessionRow,
    pub turns: Vec<TurnRow>,
}

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
            log::warn!("[Ipc::History] Failed to read dictation history cache: {}", e);
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

/// Initializes a fresh conversational session, resetting working memory.
#[tauri::command]
pub async fn create_session(
    app: AppHandle,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<SessionRow, VoxIpcError> {
    let session_id = db_create_session(&state.db, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    init_new_session(&state, SYSTEM_PROMPT_MODULAR).await;
    state
        .conversation_id
        .store(session_id as u64, Ordering::Relaxed);

    if let Err(e) = emit_ipc(&app, IpcEvent::SessionsChanged) {
        log::warn!("[IPC::History] Failed to emit SessionsChanged: {}", e);
    }

    let session = fetch_session_by_id(&state.db, session_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
        .ok_or_else(|| VoxIpcError::NotFound(format!("Session {} not found", session_id)))?;

    Ok(session)
}

/// Restores a past session into working memory and returns its metadata and turns.
#[tauri::command]
pub async fn continue_session(
    app: AppHandle,
    session_id: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<ContinueSessionResult, VoxIpcError> {
    resume_session(&state, SYSTEM_PROMPT_MODULAR, session_id)
        .await
        .map_err(|e| VoxIpcError::Pipeline(e.to_string()))?;

    state
        .conversation_id
        .store(session_id as u64, Ordering::Relaxed);

    if let Err(e) = emit_ipc(&app, IpcEvent::SessionsChanged) {
        log::warn!("[IPC::History] Failed to emit SessionsChanged: {}", e);
    }

    let session = fetch_session_by_id(&state.db, session_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
        .ok_or_else(|| VoxIpcError::NotFound(format!("Session {} not found", session_id)))?;

    let turns = fetch_turns(&state.db, session_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    Ok(ContinueSessionResult { session, turns })
}

/// Returns sessions optionally filtered by project, ordered by pinned first then newest.
#[tauri::command]
pub async fn get_sessions(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SessionRow>, VoxIpcError> {
    fetch_sessions(&state.db, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Returns all turns for a given session, oldest first.
#[tauri::command]
pub async fn get_turns(
    session_id: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<TurnRow>, VoxIpcError> {
    fetch_turns(&state.db, session_id)
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
    update_session_metadata(
        &state.db,
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
    delete_session_row(&state.db, session_id, hard.unwrap_or(false))
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    if let Err(e) = emit_ipc(&app, IpcEvent::SessionsChanged) {
        log::warn!("[IPC::History] Failed to emit SessionsChanged: {}", e);
    }

    Ok(())
}
