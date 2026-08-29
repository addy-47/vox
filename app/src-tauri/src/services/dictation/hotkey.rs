use crate::core::error::DictationError;
use crate::core::state::AppState;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

#[derive(Debug, Clone, Copy)]
enum HotkeyAction {
    Press,
    Release,
}

static HOTKEY_TX: OnceLock<UnboundedSender<HotkeyAction>> = OnceLock::new();

fn get_or_init_hotkey_worker(app: &AppHandle) -> UnboundedSender<HotkeyAction> {
    HOTKEY_TX
        .get_or_init(|| {
            let (tx, mut rx) = unbounded_channel::<HotkeyAction>();
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(action) = rx.recv().await {
                    let state: State<'_, std::sync::Arc<AppState>> = app_handle.state();
                    match action {
                        HotkeyAction::Press => {
                            if let Err(e) = crate::services::pipeline::dictation::handle_hotkey_press(&app_handle, &state).await {
                                log::error!("[Dictation::Hotkey] Error in handle_press: {}", e);
                            }
                        }
                        HotkeyAction::Release => {
                            if let Err(e) = crate::services::pipeline::dictation::handle_hotkey_release(&app_handle, &state).await {
                                log::error!("[Dictation::Hotkey] Error in handle_release: {}", e);
                            }
                        }
                    }
                }
            });
            tx
        })
        .clone()
}

/// Register the global dictation shortcut with press and release listener hooks.
pub fn register_global_hotkey(app: &AppHandle, shortcut_str: &str) -> Result<(), DictationError> {
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

    let shortcut_clone = shortcut_str.to_string();
    let hotkey_tx = get_or_init_hotkey_worker(app);

    let res = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _sc, event| {
            match event.state() {
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
