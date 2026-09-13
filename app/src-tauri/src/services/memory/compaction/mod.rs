pub mod coordinator;
pub mod prompt;
pub mod runner;

use std::sync::Arc;

use anyhow::Result;
pub use coordinator::{CompactionCoordinator, CompactionExecutionSummary};
pub use prompt::{build_compaction_request, COMPACTION_SYSTEM_PROMPT};
pub use runner::{run_compaction, CompactionResult};
use tauri::AppHandle;

use crate::{
    core::state::AppState,
    persistence::{
        compactions::fetch_uncompacted_sessions,
        notifications::{find_notification_by_group, update_interactive_notification},
    },
};

/// Runs a startup sweep across all past sessions to detect any sessions that ended with
/// uncompacted turns (e.g. from an OS crash, hard reboot, or sudden termination).
pub async fn reconcile_uncompacted_sessions_on_boot(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<u32> {
    let conn = state.db.connect()?;

    let uncompacted = fetch_uncompacted_sessions(&conn).await?;
    if uncompacted.is_empty() {
        log::info!("[BootReconciliation] No uncompacted sessions found on boot.");
        return Ok(0);
    }

    log::info!(
        "[BootReconciliation] Detected {} session(s) with uncompacted turns",
        uncompacted.len()
    );

    let auto_compaction = state
        .settings
        .read()
        .map(|s| s.history.auto_compaction)
        .unwrap_or(false);

    let count = uncompacted.len() as u32;

    for item in uncompacted {
        let uncompacted_turns = item.turn_count.saturating_sub(item.last_compacted_turn_id);
        if uncompacted_turns == 0 {
            continue;
        }

        if auto_compaction {
            let app_handle = app.clone();
            let app_state = Arc::clone(state);
            tauri::async_runtime::spawn(async move {
                if let Err(e) = CompactionCoordinator::run_compaction_slice(
                    &app_handle,
                    &app_state,
                    item.session_id,
                    "boot_auto",
                    None,
                )
                .await
                {
                    log::warn!(
                        "[BootReconciliation] Auto-compaction slice failed for session {}: {}",
                        item.session_id,
                        e
                    );
                }
            });
        } else {
            let group_key = format!("session_compaction:{}", item.session_id);
            if let Ok(Some(existing)) = find_notification_by_group(&conn, &group_key).await {
                // Invariant: User dismissal is sovereign. Never resurrect or spawn duplicate rows.
                if existing.status == "dismissed" {
                    log::debug!(
                        "[BootReconciliation] Skipping dismissed notification for session {}",
                        item.session_id
                    );
                    continue;
                }
                if existing.metadata.contains("\"resolution\":\"resolved\"") {
                    log::debug!(
                        "[BootReconciliation] Skipping resolved notification for session {}",
                        item.session_id
                    );
                    continue;
                }

                // If active card exists, update uncompacted turn count in-place
                let new_msg = format!(
                    "Session ended with {} uncompacted turn(s). Compact to extract personal memory facts.",
                    uncompacted_turns
                );
                let new_meta = format!(
                    "{{\"uncompacted_turns\": {}, \"resolution\": \"pending\"}}",
                    uncompacted_turns
                );
                let _ = update_interactive_notification(&conn, &existing.id, &new_msg, &new_meta).await;
            } else if let Err(e) = CompactionCoordinator::notify_uncompacted_session(
                app,
                &state.db,
                item.session_id,
                uncompacted_turns,
            )
            .await
            {
                log::warn!(
                    "[BootReconciliation] Failed to emit notification for session {}: {}",
                    item.session_id,
                    e
                );
            }
        }
    }

    Ok(count)
}
