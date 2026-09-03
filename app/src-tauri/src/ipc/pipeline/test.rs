use std::sync::Arc;
use tauri::{AppHandle, State};

use crate::core::error::VoxIpcError;
use crate::core::state::AppState;

/// Injects a pre-recorded audio clip directly into the active voice pipeline seam.
#[tauri::command]
pub async fn test_clip(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    clip_id: String,
) -> Result<(), VoxIpcError> {
    crate::pipeline::test::execute_test_clip(&app, &state, &clip_id).await
}

/// Cancels a running test clip turn and resets speech recognition / playback.
#[tauri::command]
pub async fn test_clip_cancel(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    crate::pipeline::test::cancel_test_clip(&app, &state).await
}
