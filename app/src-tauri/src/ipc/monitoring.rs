use crate::core::state::AppState;
use crate::monitoring::snapshot::RuntimeSnapshot;
use crate::monitoring::{MemoryProfileLogEvent, ProfilerSnapshot};
use tauri::State;

/// Get the most recent runtime snapshot.
/// Throttled pull-based IPC for frontend monitoring.
#[tauri::command]
pub fn get_runtime_snapshot(state: State<'_, std::sync::Arc<AppState>>) -> Option<RuntimeSnapshot> {
    state.monitoring.get_latest()
}

/// Get the full history of recent runtime snapshots (~60 seconds).
#[tauri::command]
pub fn get_runtime_history(state: State<'_, std::sync::Arc<AppState>>) -> Vec<RuntimeSnapshot> {
    state.monitoring.get_history()
}

/// Clear the accumulated runtime monitoring history.
#[tauri::command]
pub fn clear_runtime_history(state: State<'_, std::sync::Arc<AppState>>) {
    state.monitoring.clear();
}

/// Fetch an immediate, on-demand high-accuracy memory snapshot of the Vox process tree.
#[tauri::command]
pub async fn get_profiler_snapshot(app: tauri::AppHandle) -> Result<ProfilerSnapshot, String> {
    use tauri::Manager;
    let has_main = app.get_webview_window("main").is_some();
    let has_tray = app.get_webview_window("tray").is_some();
    let has_wizard = app.get_webview_window("wizard").is_some();

    tokio::task::spawn_blocking(move || {
        crate::monitoring::collect_profiler_snapshot(has_main, has_tray, has_wizard)
    })
    .await
    .map_err(|e| format!("[Monitoring] Failed to collect memory profiler snapshot: {e}"))
}

/// Record a structured frontend memory profile event to tracing and persisted JSONL log.
#[tauri::command]
pub async fn record_memory_profile_event(event: MemoryProfileLogEvent) -> Result<(), String> {
    tokio::task::spawn_blocking(move || crate::monitoring::persist_memory_profile_event(&event))
        .await
        .map_err(|e| format!("[Monitoring] Failed to record memory profile event: {e}"))?
}
