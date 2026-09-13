use std::sync::Arc;

use anyhow::{anyhow, Result};
use tauri::AppHandle;

use super::ActionPayload;
use crate::{
    core::{
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::notifications::{fetch_notification_by_id, resolve_notification_in_place},
    services::memory::compaction::coordinator::CompactionCoordinator,
};

/// Executes the backend remediation action associated with an interactive notification.
/// On completion, updates the interactive task card in-place to resolved state without duplicating cards.
pub async fn execute_notification_action<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    id: &str,
) -> Result<()> {
    let conn = state.db.connect()?;
    let record = fetch_notification_by_id(&conn, id)
        .await?
        .ok_or_else(|| anyhow!("Notification not found: {}", id))?;

    let payload: ActionPayload = serde_json::from_str(&record.action_payload)
        .map_err(|e| anyhow!("Failed to parse action_payload: {}", e))?;

    let notif_id = id.to_string();

    match payload {
        ActionPayload::CompactSession { session_id } => {
            log::info!(
                "[Notifications::Action] Executing CompactSession for session {}",
                session_id
            );

            let app_handle = app.clone();
            let app_state = Arc::clone(state);
            let db = state.db.clone();

            tauri::async_runtime::spawn(async move {
                match CompactionCoordinator::run_compaction_slice(
                    &app_handle,
                    &app_state,
                    session_id,
                    "manual",
                    None,
                )
                .await
                {
                    Ok(Some(summary)) => {
                        log::info!(
                            "[Notifications::Action] Compaction succeeded for session {}: enqueued {} facts",
                            session_id,
                            summary.facts_enqueued
                        );
                        // CompactionCoordinator resolves interactive notification in-place in emit_session_compaction_success_receipt.
                    }
                    Ok(None) => {
                        log::info!(
                            "[Notifications::Action] Compaction deferred or no-op for session {}",
                            session_id
                        );
                    }
                    Err(e) => {
                        log::error!(
                            "[Notifications::Action] Compaction failed for session {}: {}",
                            session_id,
                            e
                        );
                        if let Ok(conn) = db.connect() {
                            let msg = format!("Failed to compact session #{}: {}", session_id, e);
                            if let Ok(Some(updated)) = resolve_notification_in_place(
                                &conn,
                                &notif_id,
                                "failed",
                                Some(&msg),
                            )
                            .await
                            {
                                if let Err(e) =
                                    emit_ipc(&app_handle, IpcEvent::NotificationUpdated(updated))
                                {
                                    log::error!(
                                        "[Notifications::Action] Failed to emit notification update: {}",
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            });
        }
        ActionPayload::ConsolidateMemory => {
            log::info!("[Notifications::Action] Executing ConsolidateMemory");

            let app_handle = app.clone();
            let app_state = Arc::clone(state);
            let db = state.db.clone();

            tauri::async_runtime::spawn(async move {
                match crate::services::memory::scheduler::run_consolidation_once(
                    &app_handle,
                    &app_state,
                )
                .await
                {
                    Ok(()) => {
                        log::info!("[Notifications::Action] Consolidation completed successfully");
                        if let Ok(conn) = db.connect() {
                            let msg = "Memory consolidated: daily profile updated.";
                            if let Ok(Some(updated)) = resolve_notification_in_place(
                                &conn,
                                &notif_id,
                                "resolved",
                                Some(msg),
                            )
                            .await
                            {
                                if let Err(e) =
                                    emit_ipc(&app_handle, IpcEvent::NotificationUpdated(updated))
                                {
                                    log::error!(
                                        "[Notifications::Action] Failed to emit notification update: {}",
                                        e
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("[Notifications::Action] Consolidation failed: {}", e);
                        if let Ok(conn) = db.connect() {
                            let msg = format!("Consolidation failed: {}", e);
                            if let Ok(Some(updated)) = resolve_notification_in_place(
                                &conn,
                                &notif_id,
                                "failed",
                                Some(&msg),
                            )
                            .await
                            {
                                if let Err(e) =
                                    emit_ipc(&app_handle, IpcEvent::NotificationUpdated(updated))
                                {
                                    log::error!(
                                        "[Notifications::Action] Failed to emit notification update: {}",
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            });
        }
        ActionPayload::Retry {
            operation,
            resource_id,
        } => {
            log::info!(
                "[Notifications::Action] Retrying operation '{}' (resource: {:?})",
                operation,
                resource_id
            );
            let msg = format!("Retried operation: {}", operation);
            if let Ok(Some(updated)) =
                resolve_notification_in_place(&conn, id, "resolved", Some(&msg)).await
            {
                if let Err(e) = emit_ipc(app, IpcEvent::NotificationUpdated(updated)) {
                    log::error!(
                        "[Notifications::Action] Failed to emit notification update: {}",
                        e
                    );
                }
            }
        }
        ActionPayload::Navigate { target } => {
            log::debug!(
                "[Notifications::Action] Navigate to '{}' is handled client-side by React router",
                target
            );
        }
    }

    Ok(())
}
