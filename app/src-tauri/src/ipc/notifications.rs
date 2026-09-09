use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::notifications::{
        self, dismiss_notification as db_dismiss_notification,
        fetch_active_notifications as db_fetch_active,
        mark_all_notifications_read as db_mark_all_read,
        update_notification_status as db_update_status, NotificationRecord,
    },
    services::memory::compaction::coordinator::CompactionCoordinator,
};

/// Retrieves all active notifications ordered newest first.
#[tauri::command]
pub async fn get_notifications(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<NotificationRecord>, VoxIpcError> {
    db_fetch_active(&state.db)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Fetch notifications failed: {}", e)))
}

/// Marks all unread notifications as read and broadcasts `notifications_marked_read`.
#[tauri::command]
pub async fn mark_notifications_read(
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    db_mark_all_read(&state.db)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Mark all read failed: {}", e)))?;

    Ok(())
}

/// Dismisses a notification by id.
#[tauri::command]
pub async fn dismiss_notification(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    db_dismiss_notification(&state.db, &id)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Dismiss notification failed: {}", e)))?;

    Ok(())
}

/// Manually triggers compaction for a session from a notification card action.
#[tauri::command]
pub async fn trigger_session_compaction(
    app: AppHandle,
    session_id: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    // Check if an existing notification exists for this session
    if let Ok(Some(mut notif)) =
        notifications::find_active_notification_by_session(&state.db, session_id, "session_compaction")
            .await
    {
        if let Err(e) = db_update_status(&state.db, &notif.id, "in_progress").await {
            log::warn!(
                "[Notifications::IPC] Failed to update status to in_progress: {}",
                e
            );
        }
        notif.status = "in_progress".to_string();
        if let Err(e) = emit_ipc(&app, IpcEvent::NotificationUpdated(notif.clone())) {
            log::warn!(
                "[Notifications::IPC] Failed to emit NotificationUpdated: {}",
                e
            );
        }
    }

    // Trigger coordinator execution asynchronously
    let app_handle = app.clone();
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = CompactionCoordinator::run_compaction_slice(
            &app_handle,
            &app_state,
            session_id,
            "manual",
            None,
        )
        .await
        {
            log::error!(
                "[Notifications::IPC] Manual compaction failed for session {}: {}",
                session_id,
                e
            );
        }
    });

    Ok(())
}
