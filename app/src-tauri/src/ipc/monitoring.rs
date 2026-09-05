use crate::core::error::VoxIpcError;
use crate::core::state::AppState;
use crate::monitoring::snapshot::RuntimeSnapshot;
use crate::monitoring::{MemoryProfileLogEvent, ProfilerSnapshot};
use std::sync::Arc;
use tauri::State;

/// Get the most recent runtime snapshot.
/// Throttled pull-based IPC for frontend monitoring.
#[tauri::command]
pub fn get_runtime_snapshot(state: State<'_, Arc<AppState>>) -> Option<RuntimeSnapshot> {
    state.monitoring.get_latest()
}

/// Fetch an immediate, on-demand high-accuracy memory snapshot of the Vox process tree.
#[tauri::command]
pub async fn get_profiler_snapshot<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ProfilerSnapshot, VoxIpcError> {
    use crate::core::constants::{WINDOW_MAIN, WINDOW_TRAY, WINDOW_WIZARD};
    use tauri::Manager;
    let has_main = app.get_webview_window(WINDOW_MAIN).is_some();
    let has_tray = app.get_webview_window(WINDOW_TRAY).is_some();
    let has_wizard = app.get_webview_window(WINDOW_WIZARD).is_some();

    tokio::task::spawn_blocking(move || {
        crate::monitoring::collect_profiler_snapshot(has_main, has_tray, has_wizard)
    })
    .await
    .map_err(|e| VoxIpcError::Internal(format!("Failed to collect memory profiler snapshot: {e}")))
}

/// Record a structured frontend memory profile event to tracing and persisted JSONL log.
#[tauri::command]
pub async fn record_memory_profile_event(event: MemoryProfileLogEvent) -> Result<(), VoxIpcError> {
    tokio::task::spawn_blocking(move || crate::monitoring::persist_memory_profile_event(&event))
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Failed to record memory profile event: {e}")))?
        .map_err(VoxIpcError::Internal)?;
    Ok(())
}
