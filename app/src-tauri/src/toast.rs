use std::process::Command;

use tauri::AppHandle;

use crate::core::events::Severity;

/// Delivery outcome of attempting to dispatch a toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastDeliveryOutcome {
    /// Native desktop notification successfully scheduled and displayed.
    Shown,
    /// Desktop notification dispatch failed.
    Failed,
}

/// Dispatches an ephemeral desktop alert directly to the native host OS notification daemon.
///
/// Supported platforms:
/// - Linux: FreeDesktop Notifications via `notify-send` with Vox icon and Pango bold markup.
/// - macOS: Apple Notification Center via native `osascript` banners.
/// - Windows: Windows Action Center via PowerShell WinRT `ToastNotificationManager`.
pub fn show_toast<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    title: &str,
    message: &str,
    severity: Severity,
    duration_ms: Option<u64>,
) -> ToastDeliveryOutcome {
    dispatch_native_notification(title, message, severity, duration_ms)
}

/// Dispatches a native OS desktop notification across supported target operating systems.
pub fn dispatch_native_notification(
    title: &str,
    message: &str,
    severity: Severity,
    duration_ms: Option<u64>,
) -> ToastDeliveryOutcome {
    let formatted_title = format_title_with_icon(title, severity);
    let formatted_message = format_notification_body(message);

    #[cfg(target_os = "linux")]
    {
        dispatch_linux_notification(&formatted_title, &formatted_message, duration_ms)
    }

    #[cfg(target_os = "macos")]
    {
        dispatch_macos_notification(&formatted_title, &formatted_message)
    }

    #[cfg(target_os = "windows")]
    {
        dispatch_windows_notification(&formatted_title, &formatted_message)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        log::info!(
            "[Notification] Native desktop alert: {} - {}",
            formatted_title,
            formatted_message
        );
        ToastDeliveryOutcome::Shown
    }
}

#[cfg(target_os = "linux")]
fn dispatch_linux_notification(
    title: &str,
    message: &str,
    duration_ms: Option<u64>,
) -> ToastDeliveryOutcome {
    let icon_path = ensure_vox_icon_on_disk();
    let duration_str = duration_ms.unwrap_or(3500).to_string();

    let mut cmd = Command::new("notify-send");
    cmd.args([
        "-a",
        "Vox",
        "-i",
        &icon_path,
        "-t",
        &duration_str,
        "-h",
        "string:category:vox",
        title,
        message,
    ]);

    match cmd.spawn() {
        Ok(_) => ToastDeliveryOutcome::Shown,
        Err(e) => {
            log::warn!("[Toast] Failed to dispatch notify-send: {}", e);
            ToastDeliveryOutcome::Failed
        }
    }
}

#[cfg(target_os = "macos")]
fn dispatch_macos_notification(title: &str, message: &str) -> ToastDeliveryOutcome {
    // Strip XML/HTML bold tags for macOS notification strings and escape quotes
    let clean_msg = message
        .replace("<b>", "")
        .replace("</b>", "")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let clean_title = title.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        r#"display notification "{}" with title "{}" subtitle "Vox""#,
        clean_msg, clean_title
    );

    let mut cmd = Command::new("osascript");
    cmd.args(["-e", &script]);

    match cmd.spawn() {
        Ok(_) => ToastDeliveryOutcome::Shown,
        Err(e) => {
            log::warn!("[Toast] Failed to dispatch osascript notification: {}", e);
            ToastDeliveryOutcome::Failed
        }
    }
}

#[cfg(target_os = "windows")]
fn dispatch_windows_notification(title: &str, message: &str) -> ToastDeliveryOutcome {
    let clean_msg = message
        .replace("<b>", "")
        .replace("</b>", "")
        .replace('\'', "''");
    let clean_title = title.replace('\'', "''");

    let script = format!(
        r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); $xml = [xml]$template.GetXml(); $nodes = $xml.GetElementsByTagName('text'); $nodes.Item(0).AppendChild($xml.CreateTextNode('{}')) > $null; $nodes.Item(1).AppendChild($xml.CreateTextNode('{}')) > $null; $toast = [Windows.UI.Notifications.ToastNotification]::new($xml); [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Vox').Show($toast);"#,
        clean_title, clean_msg
    );

    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ]);

    match cmd.spawn() {
        Ok(_) => ToastDeliveryOutcome::Shown,
        Err(e) => {
            log::warn!(
                "[Toast] Failed to dispatch Windows PowerShell notification: {}",
                e
            );
            ToastDeliveryOutcome::Failed
        }
    }
}

fn format_title_with_icon(title: &str, severity: Severity) -> String {
    if title.starts_with('🎙')
        || title.starts_with('✓')
        || title.starts_with('⚠')
        || title.starts_with('📋')
        || title.starts_with('❌')
    {
        title.to_string()
    } else {
        match severity {
            Severity::Info => {
                if title.to_lowercase().contains("paste") {
                    format!("✓ {}", title)
                } else if title.to_lowercase().contains("clip")
                    || title.to_lowercase().contains("copy")
                {
                    format!("📋 {}", title)
                } else {
                    format!("🎙️ {}", title)
                }
            }
            Severity::Warning => format!("⚠️ {}", title),
            Severity::Critical => format!("❌ {}", title),
        }
    }
}

fn format_notification_body(message: &str) -> String {
    if message.starts_with('\n') {
        message.to_string()
    } else {
        format!("\n{}", message)
    }
}

/// Shows a lifecycle card and returns the server-assigned ID for later in-place replacement.
/// Returns 0 where the platform dispatcher cannot report an ID (updates then degrade to no-ops).
pub fn show_replaceable_toast(
    title: &str,
    message: &str,
    severity: Severity,
    duration_ms: u64,
) -> u32 {
    let formatted_title = format_title_with_icon(title, severity);
    let formatted_message = format_notification_body(message);

    #[cfg(target_os = "linux")]
    {
        first_show_linux(&formatted_title, &formatted_message, duration_ms)
    }

    #[cfg(target_os = "macos")]
    {
        dispatch_macos_notification(&formatted_title, &formatted_message);
        0
    }

    #[cfg(target_os = "windows")]
    {
        dispatch_windows_notification(&formatted_title, &formatted_message);
        0
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        log::info!(
            "[Notification] Native desktop alert: {} - {}",
            formatted_title,
            formatted_message
        );
        0
    }
}

/// Replaces the lifecycle card carrying `server_id` in place.
/// Does nothing when `server_id` is 0 or the platform lacks replace semantics.
pub fn update_replaceable_toast(
    server_id: u32,
    title: &str,
    message: &str,
    severity: Severity,
    duration_ms: u64,
) {
    if server_id == 0 {
        return;
    }
    let formatted_title = format_title_with_icon(title, severity);
    let formatted_message = format_notification_body(message);

    #[cfg(target_os = "linux")]
    {
        replace_linux(server_id, &formatted_title, &formatted_message, duration_ms);
    }

    #[cfg(not(target_os = "linux"))]
    {
        log::debug!(
            "[Notification] Replace unsupported on this platform (id {}): {} - {}",
            server_id,
            formatted_title,
            formatted_message
        );
    }
}

/// Dispatches a first-show Linux card and captures the server ID via `notify-send --print-id`.
#[cfg(target_os = "linux")]
fn first_show_linux(title: &str, message: &str, duration_ms: u64) -> u32 {
    let icon_path = ensure_vox_icon_on_disk();
    let duration_str = duration_ms.to_string();

    let mut cmd = Command::new("notify-send");
    cmd.args([
        "-a",
        "Vox",
        "-i",
        &icon_path,
        "-t",
        &duration_str,
        "-h",
        "string:category:vox",
        "-p",
        title,
        message,
    ]);
    match cmd.output() {
        Ok(output) => parse_notify_server_id(&output.stdout),
        Err(e) => {
            log::warn!("[Toast] Failed to dispatch replaceable notify-send: {}", e);
            0
        }
    }
}

/// Replaces a live Linux card in place via `notify-send --replace-id`.
#[cfg(target_os = "linux")]
fn replace_linux(server_id: u32, title: &str, message: &str, duration_ms: u64) {
    let icon_path = ensure_vox_icon_on_disk();
    let duration_str = duration_ms.to_string();

    let mut cmd = Command::new("notify-send");
    cmd.args([
        "-a",
        "Vox",
        "-i",
        &icon_path,
        "-t",
        &duration_str,
        "-h",
        "string:category:vox",
        "-r",
        &server_id.to_string(),
        title,
        message,
    ]);
    if let Err(e) = cmd.spawn() {
        log::warn!("[Toast] Failed to replace notification {}: {}", server_id, e);
    }
}

/// Parses the server-assigned notification ID printed by `notify-send --print-id`.
#[cfg(target_os = "linux")]
fn parse_notify_server_id(stdout: &[u8]) -> u32 {
    String::from_utf8_lossy(stdout).trim().parse().unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn ensure_vox_icon_on_disk() -> String {
    if let Some(home) = dirs::home_dir() {
        let icon_dir = home.join(".vox").join("icons");
        let icon_file = icon_dir.join("vox.png");
        if icon_file.exists() {
            return icon_file.to_string_lossy().to_string();
        }
        let _ = std::fs::create_dir_all(&icon_dir);
        static VOX_ICON_BYTES: &[u8] = include_bytes!("../icons/128x128.png");
        if std::fs::write(&icon_file, VOX_ICON_BYTES).is_ok() {
            return icon_file.to_string_lossy().to_string();
        }
    }
    "dialog-information".to_string()
}
