use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use tauri::{async_runtime, AppHandle, Runtime};
use tokio_util::sync::CancellationToken;

use super::{drain_ingestion_queue_with_progress, INGESTION_THROTTLE_DURATION};
use crate::{
    core::{
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::has_unfinished_items,
};

/// Spawns an asynchronous background task to drain pending and failed ingestion items.
pub fn spawn_ingestion_sweep<R: Runtime>(
    state: Arc<AppState>,
    app: Option<AppHandle<R>>,
    cancel_token: Option<CancellationToken>,
) {
    let is_enabled = state
        .settings
        .read()
        .map(|s| s.personal_memory.pipeline_processing_enabled)
        .unwrap_or(true);

    if !is_enabled {
        log::debug!(
            "[Memory::Ingestion] Ingestion sweep skipped: pipeline_processing_enabled is false"
        );
        return;
    }

    let token = cancel_token.unwrap_or_default();
    *state.ingestion_cancel.lock() = Some(token.clone());

    let db = state.db.clone();
    let state_arc = Arc::clone(&state);

    async_runtime::spawn(async move {
        let conn = match db.connect() {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "[Memory::Ingestion] Failed to vend connection for sweep: {}",
                    e
                );
                *state_arc.ingestion_cancel.lock() = None;
                return;
            }
        };

        match has_unfinished_items(&conn).await {
            Ok(true) => {
                log::info!(
                    "[Memory::Ingestion] Unfinished queue items found; starting ingestion sweep."
                );

                let last_emit = Arc::new(Mutex::new(
                    Instant::now() - Duration::from_secs(10),
                ));
                let app_progress = app.clone();
                let progress_cb = move || {
                    let mut last = last_emit.lock();
                    if last.elapsed() >= INGESTION_THROTTLE_DURATION {
                        *last = Instant::now();
                        if let Some(ref a) = app_progress {
                            if let Err(e) = emit_ipc(a, IpcEvent::MemoryIngestionUpdated) {
                                log::warn!(
                                    "[Memory::Ingestion] Failed to emit progress event: {}",
                                    e
                                );
                            }
                        }
                    }
                };

                if let Err(e) = drain_ingestion_queue_with_progress(
                    &conn,
                    Some(&token),
                    Some(progress_cb),
                )
                .await
                {
                    log::warn!("[Memory::Ingestion] Ingestion sweep error: {}", e);
                }

                if let Some(ref a) = app {
                    if let Err(e) = emit_ipc(a, IpcEvent::MemoryIngestionUpdated) {
                        log::warn!(
                            "[Memory::Ingestion] Failed to emit completion event: {}",
                            e
                        );
                    }
                }
            }
            Ok(false) => {
                log::debug!("[Memory::Ingestion] Queue is quiescent; no sweep needed.");
            }
            Err(e) => {
                log::warn!("[Memory::Ingestion] Failed to check queue status: {}", e);
            }
        }

        *state_arc.ingestion_cancel.lock() = None;
    });
}
