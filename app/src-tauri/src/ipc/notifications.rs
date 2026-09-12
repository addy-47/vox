use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    core::{error::VoxIpcError, state::AppState},
    persistence::notifications::{
        dismiss_notifications as db_dismiss_notifications,
        fetch_active_notifications as db_fetch_active,
        mark_notifications_read as db_mark_read, NotificationFilter, NotificationRecord,
    },
    services::notifications::execute_notification_action as svc_execute_action,
};

/// Retrieves all active notifications ordered newest first.
#[tauri::command]
pub async fn get_notifications(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<NotificationRecord>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(format!("Fetch notifications failed: {}", e)))?;
    db_fetch_active(&conn)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Fetch notifications failed: {}", e)))
}

/// Marks unread notifications matching the optional filter (or all unread) as read.
#[tauri::command]
pub async fn mark_notifications_read(
    filter: Option<NotificationFilter>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(format!("Mark notifications read failed: {}", e)))?;
    db_mark_read(&conn, filter.as_ref())
        .await
        .map_err(|e| VoxIpcError::Database(format!("Mark notifications read failed: {}", e)))?;

    Ok(())
}

/// Dismisses active notifications matching the optional filter (or all active).
#[tauri::command]
pub async fn dismiss_notifications(
    filter: Option<NotificationFilter>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(format!("Dismiss notifications failed: {}", e)))?;
    db_dismiss_notifications(&conn, filter.as_ref())
        .await
        .map_err(|e| VoxIpcError::Database(format!("Dismiss notifications failed: {}", e)))?;

    Ok(())
}

/// Polymorphic action executor for actionable notification cards.
#[tauri::command]
pub async fn execute_notification_action<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    id: String,
    _action: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    svc_execute_action(&app, state.inner(), &id)
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Execute notification action failed: {}", e)))
}

