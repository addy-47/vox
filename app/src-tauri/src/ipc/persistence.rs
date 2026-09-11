use std::sync::{atomic::Ordering, Arc};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

pub use crate::persistence::sessions::{SessionRow, TurnRow};
use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent, VoxEvent},
        state::{AppState, InteractionState},
    },
    persistence::sessions::{
        delete_session as delete_session_row, fetch_session_by_id, fetch_sessions, fetch_turns,
        update_session_metadata,
    },
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

/// Initializes a fresh conversational session, cleanly tearing down any active
/// audio pipeline, resetting working memory with Personal Memory, and resetting
/// session identifiers. Follows lazy persistence (DB row created upon first spoken turn).
#[tauri::command]
pub async fn create_session(
    _project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<SessionRow>, VoxIpcError> {
    // 1. If an assistant session is active, disengage pipeline cleanly
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Idle {
        let event_tx = state.event_tx.lock().clone();
        if let Some(tx) = event_tx {
            if let Err(e) = tx.send(VoxEvent::EndSession) {
                log::warn!(
                    "[IPC::History] Failed to send EndSession during create_session: {}",
                    e
                );
            }
        }
    }

    // 2. Clear conversation and turn tracking
    state.conversation_id.store(0, Ordering::Relaxed);
    state.pipeline_accumulator.lock().clear();

    log::info!("[IPC::History] Reset conversation state for fresh session");
    Ok(None)
}

/// Restores a past session into working memory and returns its metadata and turns.
#[tauri::command]
pub async fn continue_session(
    session_id: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<ContinueSessionResult, VoxIpcError> {
    state
        .conversation_id
        .store(session_id as u64, Ordering::Relaxed);

    let session = fetch_session_by_id(&state.db, session_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
        .ok_or_else(|| VoxIpcError::NotFound(format!("Session {} not found", session_id)))?;

    let turns = fetch_turns(&state.db, session_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    log::info!(
        "[IPC::History] Restored continuation for session {} into working memory",
        session_id
    );

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
