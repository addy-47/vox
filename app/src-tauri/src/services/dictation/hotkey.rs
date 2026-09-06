use std::sync::Arc;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::mpsc::UnboundedSender;

use crate::core::{error::DictationError, events::VoxEvent, state::AppState};

/// Actions triggered by the global dictation shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    Press,
    Release,
}

/// Register the global dictation shortcut with press and release listener hooks.
pub fn register_global_hotkey<R: tauri::Runtime>(
    app: &AppHandle<R>,
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

/// Spawns the async dictation hotkey listener loop and registers global shortcut with OS.
pub fn init_dictation_hotkey_listener<R: tauri::Runtime>(
    app: &AppHandle<R>,
    shortcut_str: &str,
) -> Result<(), DictationError> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<HotkeyAction>();
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        while let Some(action) = rx.recv().await {
            use tauri::Manager;
            let state: tauri::State<'_, Arc<AppState>> = app_handle.state();
            let event_tx_opt = state.event_tx.lock().clone();
            if let Some(tx) = event_tx_opt {
                match action {
                    HotkeyAction::Press => {
                        if let Err(e) = tx.send(VoxEvent::PttStart) {
                            log::error!(
                                "[Dictation::Hotkey] Failed to send VoxEvent::PttStart: {}",
                                e
                            );
                        }
                    }
                    HotkeyAction::Release => {
                        if let Err(e) = tx.send(VoxEvent::PttStop) {
                            log::error!(
                                "[Dictation::Hotkey] Failed to send VoxEvent::PttStop: {}",
                                e
                            );
                        }
                    }
                }
            } else {
                log::warn!("[Dictation::Hotkey] Event router is not active; dropped hotkey event");
            }
        }
    });

    register_global_hotkey(app, shortcut_str, tx)
}
