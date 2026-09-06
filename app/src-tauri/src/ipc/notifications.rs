use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::{
        db::VoxDb,
        notifications::{
            self, dismiss_notification as db_dismiss_notification,
            fetch_active_notifications as db_fetch_active,
            mark_all_notifications_read as db_mark_all_read,
            update_notification_status as db_update_status, NotificationRecord,
        },
    },
    services::memory::compaction::coordinator::CompactionCoordinator,
    utils::paths::db_path,
};

/// Retrieves all active notifications ordered newest first.
#[tauri::command]
pub async fn get_notifications() -> Result<Vec<NotificationRecord>, VoxIpcError> {
    let db_path = db_path();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    db_fetch_active(&conn)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Fetch notifications failed: {}", e)))
}

/// Marks all unread notifications as read and broadcasts `notifications_marked_read`.
#[tauri::command]
pub async fn mark_notifications_read(app: AppHandle) -> Result<(), VoxIpcError> {
    let db_path = db_path();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    db_mark_all_read(&conn)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Mark all read failed: {}", e)))?;

    if let Err(e) = emit_ipc(&app, IpcEvent::NotificationsMarkedRead) {
        log::warn!(
            "[Notifications::IPC] Failed to emit notifications_marked_read: {}",
            e
        );
    }

    Ok(())
}

/// Dismisses a notification by id and broadcasts `notification_dismissed`.
#[tauri::command]
pub async fn dismiss_notification(id: String, app: AppHandle) -> Result<(), VoxIpcError> {
    let db_path = db_path();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    db_dismiss_notification(&conn, &id)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Dismiss notification failed: {}", e)))?;

    if let Err(e) = emit_ipc(&app, IpcEvent::NotificationDismissed { id }) {
        log::warn!(
            "[Notifications::IPC] Failed to emit notification_dismissed: {}",
            e
        );
    }

    Ok(())
}

/// Manually triggers compaction for a session from a notification card action.
#[tauri::command]
pub async fn trigger_session_compaction(
    session_id: i64,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let db_path = db_path();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    // Check if an existing notification exists for this session
    if let Ok(Some(mut notif)) =
        notifications::find_active_notification_by_session(&conn, session_id, "session_compaction")
            .await
    {
        let _ = db_update_status(&conn, &notif.id, "in_progress").await;
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
