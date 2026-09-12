use std::sync::Arc;

use anyhow::{anyhow, Result};
use tauri::AppHandle;

use super::{notify, Action, ActionPayload, NotificationCategory, NotificationParams};
use crate::{
    core::{
        events::{emit_ipc, IpcEvent, Severity},
        state::AppState,
    },
    persistence::notifications::{dismiss_notification, fetch_notification_by_id},
    services::memory::compaction::coordinator::CompactionCoordinator,
};

/// Executes the backend remediation action associated with an interactive notification.
/// On completion, auto-dismisses the interactive task card and appends an audited Receipt.
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

    match payload {
        ActionPayload::CompactSession { session_id } => {
            log::info!(
                "[Notifications::Action] Executing CompactSession for session {}",
                session_id
            );

            // Launch compaction slice asynchronously without prematurely dismissing the card
            let app_handle = app.clone();
            let app_state = Arc::clone(state);
            let db = state.db.clone();
            let mut record_dismissed = record.clone();
            record_dismissed.status = "dismissed".to_string();

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
                        // CompactionCoordinator already dismissed interactive card in DB and emitted receipt.
                        // Notify frontend of the dismissed interactive card:
                        let _ = emit_ipc(&app_handle, IpcEvent::NotificationUpdated(record_dismissed));
                    }
                    Ok(None) => {
                        log::info!(
                            "[Notifications::Action] Compaction deferred or no-op for session {}",
                            session_id
                        );
                        // Invariant: Keep card active in state so user can retry when system is idle.
                    }
                    Err(e) => {
                        log::error!(
                            "[Notifications::Action] Compaction failed for session {}: {}",
                            session_id,
                            e
                        );
                        // Invariant: Keep card active in state and notify user of the failure.
                        let _ = notify(
                            &app_handle,
                            &db,
                            NotificationParams {
                                group_key: Some(&format!("session_compaction:{}", session_id)),
                                category: NotificationCategory::SessionCompaction,
                                severity: Severity::Warning,
                                impact: None,
                                action: Action::Receipt,
                                title: &format!("Session #{} Compaction Failed", session_id),
                                message: &format!("Compaction failed: {}", e),
                                session_id: Some(session_id),
                                metadata: None,
                                duration_ms: None,
                            },
                        )
                        .await;
                    }
                }
            });
        }
        ActionPayload::ConsolidateMemory => {
            log::info!("[Notifications::Action] Executing ConsolidateMemory");

            // Launch consolidation asynchronously without prematurely dismissing the card
            let app_handle = app.clone();
            let app_state = Arc::clone(state);
            let db = state.db.clone();
            let mut record_dismissed = record.clone();
            record_dismissed.status = "dismissed".to_string();

            tauri::async_runtime::spawn(async move {
                match crate::services::memory::scheduler::run_consolidation_once(&app_handle, &app_state).await {
                    Ok(()) => {
                        log::info!("[Notifications::Action] Consolidation completed successfully");
                        // run_consolidation_once already dismissed interactive card in DB and emitted receipt.
                        // Notify frontend of the dismissed interactive card:
                        let _ = emit_ipc(&app_handle, IpcEvent::NotificationUpdated(record_dismissed));
                    }
                    Err(e) => {
                        log::error!("[Notifications::Action] Consolidation failed: {}", e);
                        // Invariant: Keep card active in state and notify user of the failure.
                        let _ = notify(
                            &app_handle,
                            &db,
                            NotificationParams {
                                group_key: Some("memory_consolidation:daily"),
                                category: NotificationCategory::MemoryConsolidation,
                                severity: Severity::Warning,
                                impact: None,
                                action: Action::Receipt,
                                title: "Memory Consolidation Failed",
                                message: &format!("Consolidation failed: {}", e),
                                session_id: None,
                                metadata: None,
                                duration_ms: None,
                            },
                        )
                        .await;
                    }
                }
            });
        }
        ActionPayload::Retry { operation, resource_id } => {
            log::info!(
                "[Notifications::Action] Retrying operation '{}' (resource: {:?})",
                operation,
                resource_id
            );
            dismiss_notification(&conn, id).await?;
            let mut updated_record = record.clone();
            updated_record.status = "dismissed".to_string();
            let _ = emit_ipc(app, IpcEvent::NotificationUpdated(updated_record));
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
