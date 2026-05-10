use tauri::State;
use crate::core::state::AppState;
use crate::monitoring::snapshot::RuntimeSnapshot;

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

/// Clear the monitoring history history.
#[tauri::command]
pub fn clear_runtime_history(state: State<'_, std::sync::Arc<AppState>>) {
    state.monitoring.clear();
}
