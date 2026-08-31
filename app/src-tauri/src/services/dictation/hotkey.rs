use crate::core::error::DictationError;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::mpsc::UnboundedSender;

/// Actions triggered by the global dictation shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    Press,
    Release,
}

/// Register the global dictation shortcut with press and release listener hooks.
pub fn register_global_hotkey(
    app: &AppHandle,
    shortcut_str: &str,
    hotkey_tx: UnboundedSender<HotkeyAction>,
) -> Result<(), DictationError> {
    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!(
            "[Dictation::Hotkey] Failed to unregister previous shortcuts: {:?}",
            e
        );
    }

    let shortcut: Shortcut = shortcut_str.parse().map_err(|e| {
        log::error!(
            "[Dictation::Hotkey] Failed to parse hotkey string '{}': {:?}",
            shortcut_str,
            e
        );
        DictationError::HotkeyRegistrationFailed {
            message: format!("Invalid shortcut string: {:?}", e),
        }
    })?;

    let shortcut_clone = shortcut_str.to_string();

    let res = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _sc, event| match event.state() {
            ShortcutState::Pressed => {
                if let Err(e) = hotkey_tx.send(HotkeyAction::Press) {
                    log::warn!("[Dictation::Hotkey] Failed to send Press action: {}", e);
                }
            }
            ShortcutState::Released => {
                if let Err(e) = hotkey_tx.send(HotkeyAction::Release) {
                    log::warn!("[Dictation::Hotkey] Failed to send Release action: {}", e);
                }
            }
        });

    if let Err(e) = res {
        log::error!(
            "[Dictation::Hotkey] Failed to register global shortcut '{}': {:?}",
            shortcut_clone,
            e
        );
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
