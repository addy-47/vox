use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    core::{
        engine::start_audio_engine,
        error::DictationError,
        events::VoxEvent,
        settings::{DictationInteractionMode, DictationOutputMode},
        state::{AppState, AppWindow, InteractionOwner, InteractionState},
    },
    services::notifications::lifecycle,
    tray::{ensure_tray_window, setup_linux_virtual_layer},
};

/// Actions triggered by the global dictation shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    Press,
    Release,
    Toggle,
}

#[cfg(target_os = "linux")]
static WAYLAND_SOCKET_INITIALIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "linux")]
static HOTKEY_SENDER: parking_lot::RwLock<Option<UnboundedSender<HotkeyAction>>> =
    parking_lot::RwLock::new(None);

/// Register the global dictation shortcut with press and release listener hooks.
pub fn register_global_hotkey<R: tauri::Runtime>(
    app: &AppHandle<R>,
    shortcut_str: &str,
    hotkey_tx: UnboundedSender<HotkeyAction>,
) -> Result<(), DictationError> {
    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!(
            "[Dictation::Trace] Failed to unregister previous shortcuts: {:?}",
            e
        );
    }

    let shortcut: Shortcut = shortcut_str.parse().map_err(|e| {
        log::error!(
            "[Dictation::Trace] Failed to parse hotkey string '{}': {:?}",
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
                log::info!(
                    "[Dictation::Trace] OS Global Shortcut Pressed: '{}'",
                    shortcut_clone
                );
                if let Err(e) = hotkey_tx.send(HotkeyAction::Press) {
                    log::warn!("[Dictation::Trace] Failed to queue Press action: {}", e);
                }
            }
            ShortcutState::Released => {
                log::info!(
                    "[Dictation::Trace] OS Global Shortcut Released: '{}'",
                    shortcut_clone
                );
                if let Err(e) = hotkey_tx.send(HotkeyAction::Release) {
                    log::warn!("[Dictation::Trace] Failed to queue Release action: {}", e);
                }
            }
        });

    if let Err(e) = res {
        log::error!(
            "[Dictation::Trace] Failed to register global shortcut '{}': {:?}",
            shortcut_str,
            e
        );
        return Err(DictationError::HotkeyRegistrationFailed {
            message: format!("Failed to register shortcut '{}': {:?}", shortcut_str, e),
        });
    }

    log::info!(
        "[Dictation::Trace] Successfully registered global shortcut '{}' with press/release hooks.",
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

    #[cfg(target_os = "linux")]
    {
        *HOTKEY_SENDER.write() = Some(tx.clone());
        init_linux_wayland_hotkey_daemon(shortcut_str);
    }

    tauri::async_runtime::spawn(async move {
        let mut last_trigger_instant: Option<std::time::Instant> = None;
        let mut listening_start_instant: Option<std::time::Instant> = None;
        while let Some(action) = rx.recv().await {
            use tauri::Manager;
            let state: tauri::State<'_, Arc<AppState>> = app_handle.state();

            let (dictation_enabled, is_ptt) = state
                .settings
                .read()
                .map(|s| {
                    (
                        s.dictation.enabled,
                        s.dictation.interaction_mode == DictationInteractionMode::Ptt,
                    )
                })
                .unwrap_or((false, true));

            if !dictation_enabled || !is_ptt {
                log::debug!(
                    "[Dictation] Hotkey action ignored: dictation enabled={}, is_ptt={}",
                    dictation_enabled,
                    is_ptt
                );
                continue;
            }

            let effective_action = match action {
                HotkeyAction::Toggle => {
                    let now = std::time::Instant::now();
                    if let Some(prev) = last_trigger_instant {
                        if now.duration_since(prev) < std::time::Duration::from_millis(300) {
                            log::debug!(
                                "[Dictation] Throttling rapid hotkey trigger (elapsed: {:?})",
                                prev.elapsed()
                            );
                            continue;
                        }
                    }
                    last_trigger_instant = Some(now);

                    let cur_state = state.pipeline.dictation_state();
                    if cur_state == InteractionState::Listening {
                        if let Some(start) = listening_start_instant {
                            if now.duration_since(start) < std::time::Duration::from_millis(600) {
                                log::debug!("[Dictation] Ignoring toggle within 600ms of start (compositor auto-repeat guard)");
                                continue;
                            }
                        }
                        log::info!(
                            "[Dictation] Toggle hotkey -> Stopping dictation (turn finalized)"
                        );
                        listening_start_instant = None;
                        HotkeyAction::Release
                    } else if cur_state == InteractionState::Thinking {
                        log::debug!("[Dictation] Ignored toggle while transcribing previous turn");
                        continue;
                    } else {
                        log::info!("[Dictation] Toggle hotkey -> Starting dictation");
                        listening_start_instant = Some(now);
                        let notify_app = app_handle.clone();
                        let notify_db = state.db.clone();
                        tauri::async_runtime::spawn(async move {
                            lifecycle::dictation_listening(&notify_app, &notify_db).await;
                        });
                        HotkeyAction::Press
                    }
                }
                other => other,
            };

            // On-demand engine launch if inactive (zero-idle-RAM recovery)
            let mut event_tx_opt = state.event_tx.lock().clone();
            if event_tx_opt.is_none() && effective_action == HotkeyAction::Press {
                log::info!(
                    "[Dictation] Audio engine inactive on hotkey press. Booting on-demand..."
                );
                if let Err(e) = start_audio_engine(&app_handle, &state).await {
                    log::error!("[Dictation] On-demand engine launch failed: {}", e);
                    continue;
                }
                event_tx_opt = state.event_tx.lock().clone();
            }

            if let Some(event_tx) = event_tx_opt {
                match effective_action {
                    HotkeyAction::Press => {
                        let is_tray = state
                            .settings
                            .read()
                            .map(|s| s.dictation.output_mode == DictationOutputMode::Tray)
                            .unwrap_or(false);
                        if is_tray {
                            if let Ok(window) = ensure_tray_window(&app_handle) {
                                setup_linux_virtual_layer(&app_handle, AppWindow::Tray.as_str());
                                let _ = window.show();
                                let _ = window.emit("toggle_tray", ());
                            }
                        }

                        log::debug!("[Dictation::Trace] Dispatched VoxEvent::PttStart to central pipeline router");
                        if let Err(e) = event_tx.send(VoxEvent::PttStart {
                            owner: InteractionOwner::Dictation,
                        }) {
                            log::error!(
                                "[Dictation::Trace] Failed to send VoxEvent::PttStart: {}",
                                e
                            );
                        }
                    }
                    HotkeyAction::Release => {
                        log::debug!("[Dictation::Trace] Dispatched VoxEvent::PttStop to central pipeline router");
                        if let Err(e) = event_tx.send(VoxEvent::PttStop {
                            owner: InteractionOwner::Dictation,
                        }) {
                            log::error!(
                                "[Dictation::Trace] Failed to send VoxEvent::PttStop: {}",
                                e
                            );
                        }
                    }
                    HotkeyAction::Toggle => unreachable!(),
                }
            } else {
                log::warn!(
                    "[Dictation] Event router is not active; dropped hotkey event {:?}",
                    action
                );
            }
        }
    });

    register_global_hotkey(app, shortcut_str, tx)
}

#[cfg(target_os = "linux")]
fn init_linux_wayland_hotkey_daemon(shortcut_str: &str) {
    let p = crate::utils::paths::get();
    let socket_path = p.socket.clone();
    let trigger_path = p.trigger_script.clone();

    // 1. Ensure trigger script is present and executable
    if let Err(e) = ensure_trigger_script(&trigger_path, &socket_path) {
        log::warn!(
            "[Dictation::Wayland] Failed to create trigger script: {:?}",
            e
        );
    }

    // 2. Start Unix domain socket listener once
    if WAYLAND_SOCKET_INITIALIZED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let sock_path_clone = socket_path;
        tauri::async_runtime::spawn(async move {
            run_unix_socket_listener(sock_path_clone).await;
        });
    }

    // 3. Register or sync with GNOME media keys if running under GNOME
    if is_gnome_session() {
        sync_gnome_media_key(shortcut_str, &trigger_path);
    }
}

#[cfg(target_os = "linux")]
fn ensure_trigger_script(
    trigger_path: &std::path::Path,
    socket_path: &std::path::Path,
) -> std::io::Result<()> {
    if let Some(parent) = trigger_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let script_content = format!(
        r#"#!/bin/sh
# Auto-generated by Vox. Triggers dictation via local Unix domain socket.
SOCK="{}"
if [ -S "$SOCK" ]; then
    LOCK="/tmp/vox_trigger_${{USER:-default}}.lock"
    NOW=$(date +%s%3N 2>/dev/null || date +%s)
    if [ -f "$LOCK" ]; then
        LAST=$(cat "$LOCK" 2>/dev/null || echo 0)
        DIFF=$((NOW - LAST))
        if [ "$DIFF" -ge 0 ] && [ "$DIFF" -lt 400 ]; then
            exit 0
        fi
    fi
    echo "$NOW" > "$LOCK" 2>/dev/null
    if command -v nc >/dev/null 2>&1; then
        printf "trigger\n" | nc -U "$SOCK" -w 1 >/dev/null 2>&1
    elif command -v python3 >/dev/null 2>&1; then
        python3 -c "import socket; s = socket.socket(socket.AF_UNIX); s.settimeout(1); s.connect('$SOCK'); s.sendall(b'trigger\n'); s.close()" >/dev/null 2>&1
    fi
fi
"#,
        socket_path.display()
    );

    std::fs::write(trigger_path, script_content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(trigger_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(trigger_path, perms);
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
async fn run_unix_socket_listener(socket_path: std::path::PathBuf) {
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    let listener = match tokio::net::UnixListener::bind(&socket_path) {
        Ok(l) => {
            log::info!(
                "[Dictation::Wayland] Unix domain socket listener bound at {:?}",
                socket_path
            );
            l
        }
        Err(e) => {
            log::error!(
                "[Dictation::Wayland] Failed to bind Unix socket at {:?}: {:?}",
                socket_path,
                e
            );
            return;
        }
    };

    let mut last_socket_trigger: Option<std::time::Instant> = None;
    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 64];
                match tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await {
                    Ok(n) if n > 0 => {
                        let now = std::time::Instant::now();
                        if let Some(prev) = last_socket_trigger {
                            if now.duration_since(prev) < std::time::Duration::from_millis(350) {
                                continue;
                            }
                        }
                        last_socket_trigger = Some(now);

                        let msg = String::from_utf8_lossy(&buf[..n]).trim().to_lowercase();
                        log::debug!("[Dictation::Wayland] Socket message: '{}'", msg);
                        let action = match msg.as_str() {
                            "press" => HotkeyAction::Press,
                            "release" => HotkeyAction::Release,
                            _ => HotkeyAction::Toggle,
                        };
                        let sender_opt = HOTKEY_SENDER.read().clone();
                        if let Some(sender) = sender_opt {
                            if let Err(e) = sender.send(action) {
                                log::warn!("[Dictation::Wayland] Failed to dispatch HotkeyAction from socket: {}", e);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                log::debug!(
                    "[Dictation::Wayland] Error accepting socket connection: {:?}",
                    e
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn is_gnome_session() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    desktop.contains("gnome") || desktop.contains("ubuntu")
}

#[cfg(target_os = "linux")]
fn vox_shortcut_to_gnome(shortcut: &str) -> String {
    let mut mods = Vec::new();
    let mut key = String::new();

    for part in shortcut.split('+') {
        let trimmed = part.trim();
        match trimmed.to_lowercase().as_str() {
            "alt" | "option" => mods.push("<Alt>"),
            "ctrl" | "control" => mods.push("<Ctrl>"),
            "shift" => mods.push("<Shift>"),
            "super" | "command" | "cmd" | "win" | "meta" => mods.push("<Super>"),
            "space" => key = "space".to_string(),
            other => key = other.to_lowercase(),
        }
    }

    format!("{}{}", mods.join(""), key)
}

#[cfg(target_os = "linux")]
fn sync_gnome_media_key(shortcut_str: &str, trigger_path: &std::path::Path) {
    let gnome_binding = vox_shortcut_to_gnome(shortcut_str);
    let path = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/vox-dictation/";
    let schema = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
    let parent_schema = "org.gnome.settings-daemon.plugins.media-keys";

    log::info!(
        "[Dictation::Wayland] Registering GNOME media-key shortcut '{}' ({}) -> {:?}",
        shortcut_str,
        gnome_binding,
        trigger_path
    );

    let _ = std::process::Command::new("gsettings")
        .args([
            "set",
            &format!("{}:{}", schema, path),
            "name",
            "Vox Dictation",
        ])
        .output();

    let _ = std::process::Command::new("gsettings")
        .args([
            "set",
            &format!("{}:{}", schema, path),
            "command",
            &trigger_path.to_string_lossy(),
        ])
        .output();

    let _ = std::process::Command::new("gsettings")
        .args([
            "set",
            &format!("{}:{}", schema, path),
            "binding",
            &gnome_binding,
        ])
        .output();

    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", parent_schema, "custom-keybindings"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if !trimmed.contains("vox-dictation") {
            let mut items: Vec<String> = Vec::new();
            for part in trimmed.split(',') {
                let cleaned = part.replace(['[', ']', '\'', ' '], "");
                if !cleaned.is_empty() && cleaned.starts_with('/') {
                    items.push(cleaned);
                }
            }
            items.push(path.to_string());
            let array_str = format!(
                "[{}]",
                items
                    .iter()
                    .map(|s| format!("'{}'", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let _ = std::process::Command::new("gsettings")
                .args(["set", parent_schema, "custom-keybindings", &array_str])
                .output();
            log::info!(
                "[Dictation::Wayland] Added vox-dictation to GNOME custom-keybindings array: {}",
                array_str
            );
        }
    }
}
