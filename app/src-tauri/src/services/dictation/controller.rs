use crate::core::error::DictationError;
use crate::core::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct DictationController;

impl DictationController {
    /// Handles global dictation hotkey press.
    pub async fn handle_press(app: &AppHandle) -> Result<(), DictationError> {
        let state: State<'_, std::sync::Arc<AppState>> = app.state();
        crate::services::pipeline::dictation::handle_hotkey_press(app, &state)
            .await
            .map_err(|e| DictationError::EngineNotReady { message: e })
    }

    /// Handles global dictation hotkey release.
    pub async fn handle_release(app: &AppHandle) -> Result<(), DictationError> {
        let state: State<'_, std::sync::Arc<AppState>> = app.state();
        crate::services::pipeline::dictation::handle_hotkey_release(app, &state)
            .await
            .map_err(|e| DictationError::EngineNotReady { message: e })
    }

    /// Cancels active dictation recording.
    pub async fn handle_cancel(app: &AppHandle) -> Result<(), DictationError> {
        let state: State<'_, std::sync::Arc<AppState>> = app.state();
        state
            .pipeline
            .cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = app.emit(
            "ptt_status",
            serde_json::json!({ "state": "IDLE", "owner": "dictation" }),
        );
        Ok(())
    }
}
