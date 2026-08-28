use crate::core::error::DictationError;
use crate::core::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Register the global dictation shortcut with press and release listener hooks.
pub fn register_global_hotkey(app: &AppHandle, shortcut_str: &str) -> Result<(), DictationError> {
    let shortcut: Shortcut = shortcut_str.parse().map_err(|e| {
        log::error!(
            "[Dictation::Hotkey] Failed to parse hotkey string '{}': {:?}",
            shortcut_str,
            e
        );
        if let Err(emit_err) = app.emit(
            "dictation_hotkey_conflict",
            serde_json::json!({
                "shortcut": shortcut_str,
                "error": format!("Invalid shortcut format: {:?}", e)
            }),
        ) {
            log::warn!(
                "[Dictation::Hotkey] Failed to emit hotkey parse conflict: {}",
                emit_err
            );
        }
        DictationError::HotkeyRegistrationFailed {
            message: format!("Invalid shortcut string: {:?}", e),
        }
    })?;

    let app_handle = app.clone();
    let shortcut_clone = shortcut_str.to_string();

    let res = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _sc, event| {
            let handle = app_handle.clone();
            match event.state() {
                ShortcutState::Pressed => {
                    tauri::async_runtime::spawn(async move {
                        let state: State<'_, std::sync::Arc<AppState>> = handle.state();
                        if let Err(e) = crate::services::pipeline::dictation::handle_hotkey_press(&handle, &state).await {
                            log::error!("[Dictation::Hotkey] Error in handle_press: {}", e);
                        }
                    });
                }
                ShortcutState::Released => {
                    tauri::async_runtime::spawn(async move {
                        let state: State<'_, std::sync::Arc<AppState>> = handle.state();
                        if let Err(e) = crate::services::pipeline::dictation::handle_hotkey_release(&handle, &state).await {
                            log::error!("[Dictation::Hotkey] Error in handle_release: {}", e);
                        }
                    });
                }
            }
        });

    if let Err(e) = res {
        log::error!(
            "[Dictation::Hotkey] Failed to register global shortcut '{}': {:?}",
            shortcut_clone,
            e
        );
        if let Err(emit_err) = app.emit(
            "dictation_hotkey_conflict",
            serde_json::json!({
                "shortcut": shortcut_clone,
                "error": format!("Registration failed: {:?}", e)
            }),
        ) {
            log::warn!(
                "[Dictation::Hotkey] Failed to emit hotkey registration conflict: {}",
                emit_err
            );
        }
        return Err(DictationError::HotkeyRegistrationFailed {
            message: format!("Failed to register shortcut '{}': {:?}", shortcut_clone, e),
        });
    }

    log::info!(
        "[Dictation::Hotkey] Successfully registered global shortcut '{}' with press/release hooks.",
        shortcut_str
    );
    Ok(())
}
