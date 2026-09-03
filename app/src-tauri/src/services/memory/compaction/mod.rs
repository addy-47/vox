pub mod coordinator;
pub mod prompt;
pub mod runner;

pub use coordinator::{CompactionCoordinator, CompactionExecutionSummary};
pub use prompt::{build_compaction_request, COMPACTION_SYSTEM_PROMPT};
pub use runner::{run_compaction, CompactionResult};

use std::sync::Arc;
use anyhow::Result;
use tauri::AppHandle;

use crate::core::state::AppState;
use crate::persistence::compactions::fetch_uncompacted_sessions;
use crate::persistence::db::VoxDb;

/// Runs a startup sweep across all past sessions to detect any sessions that ended with
/// uncompacted turns (e.g. from an OS crash, hard reboot, or sudden termination).
pub async fn reconcile_uncompacted_sessions_on_boot(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<u32> {
    let db_path = crate::utils::paths::db_path();
    let conn = VoxDb::open(&db_path).await?;

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
        } else if let Err(e) = CompactionCoordinator::notify_uncompacted_session(
            app,
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

    Ok(count)
}
