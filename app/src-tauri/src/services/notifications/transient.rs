use tauri::AppHandle;

use crate::core::events::Severity;

/// An ephemeral, non-persisted alert.
///
/// `Action::Transient` decides whether the user gets an alert; this decides
/// where it lands. Keeping them separate lets a platform change the surface
/// without the Notification Service's routing changing.
#[derive(Debug, Clone, Copy)]
pub struct TransientAlert<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub severity: Severity,
    pub duration_ms: Option<u64>,
}

/// Result of handing a [`TransientAlert`] to the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryOutcome {
    /// The platform accepted the alert for display.
    Shown,
    /// Delivery failed; the Notification Service elevates to persistent storage
    /// rather than swallowing the alert.
    Failed,
}

/// Notification permission state as the UI needs to see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotAsked,
    /// The platform has no runtime notification permission to ask for.
    NotApplicable,
    /// The bridge could not answer. Distinct from `Denied` so a bridge fault is
    /// never reported to the user as their own choice.
    Unavailable,
}

/// Posts an ephemeral alert to the current platform's transient surface.
pub fn deliver_transient<R: tauri::Runtime>(
    app: &AppHandle<R>,
    alert: &TransientAlert<'_>,
) -> DeliveryOutcome {
    #[cfg(desktop)]
    {
        deliver_desktop(app, alert)
    }

    #[cfg(not(desktop))]
    {
        deliver_android(app, alert)
    }
}

// ─── Desktop: host OS notification daemon ────────────────────────────────────

#[cfg(desktop)]
fn deliver_desktop<R: tauri::Runtime>(
    app: &AppHandle<R>,
    alert: &TransientAlert<'_>,
) -> DeliveryOutcome {
    match crate::toast::show_toast(
        app,
        alert.title,
        alert.message,
        alert.severity,
        alert.duration_ms,
    ) {
        crate::toast::ToastDeliveryOutcome::Shown => DeliveryOutcome::Shown,
        crate::toast::ToastDeliveryOutcome::Failed => DeliveryOutcome::Failed,
    }
}

/// Desktop has no runtime notification permission to query or request.
#[cfg(desktop)]
pub fn permission_status<R: tauri::Runtime>(_app: &AppHandle<R>) -> PermissionStatus {
    PermissionStatus::NotApplicable
}

#[cfg(desktop)]
pub fn request_permission<R: tauri::Runtime>(_app: &AppHandle<R>) -> PermissionStatus {
    PermissionStatus::NotApplicable
}

// ─── Android: notification channels ──────────────────────────────────────────

#[cfg(target_os = "android")]
const CHANNEL_INFO: &str = "vox_info";

#[cfg(target_os = "android")]
const CHANNEL_URGENT: &str = "vox_urgent";

#[cfg(target_os = "android")]
const SLOT_INFO: i32 = 1001;

#[cfg(target_os = "android")]
const SLOT_URGENT: i32 = 1002;

#[cfg(target_os = "android")]
const DEFAULT_DURATION_MS: u64 = 3000;

#[cfg(target_os = "android")]
const NOTIFICATION_ICON: &str = "ic_vox_notification";

/// `Low` keeps background maintenance out of the user's way; `High` is the
/// only importance that overlays other apps, and the only one the system does
/// not suppress while Vox is in the foreground.
#[cfg(target_os = "android")]
fn channel_for(severity: Severity) -> (&'static str, i32) {
    match severity {
        Severity::Info => (CHANNEL_INFO, SLOT_INFO),
        Severity::Warning | Severity::Critical => (CHANNEL_URGENT, SLOT_URGENT),
    }
}

#[cfg(target_os = "android")]
fn deliver_android<R: tauri::Runtime>(
    app: &AppHandle<R>,
    alert: &TransientAlert<'_>,
) -> DeliveryOutcome {
    use tauri_plugin_notification::NotificationExt;

    let (channel_id, id) = channel_for(alert.severity);
    let notification = app.notification();

    let posted = notification
        .builder()
        .id(id)
        .title(alert.title)
        .body(alert.message)
        .channel_id(channel_id)
        .icon(NOTIFICATION_ICON)
        .auto_cancel()
        .show();

    if let Err(e) = posted {
        log::warn!("[Notification::Android] Failed to post transient alert: {}", e);
        return DeliveryOutcome::Failed;
    }

    schedule_dismissal(app, id, alert.duration_ms);
    DeliveryOutcome::Shown
}

/// Removes the notification after its duration.
///
/// INVARIANT: best-effort. If Android kills the process first the notification
/// stays posted, which beats losing a real alert.
#[cfg(target_os = "android")]
fn schedule_dismissal<R: tauri::Runtime>(
    app: &AppHandle<R>,
    id: i32,
    duration_ms: Option<u64>,
) {
    use tauri_plugin_notification::NotificationExt;

    let wait = std::time::Duration::from_millis(duration_ms.unwrap_or(DEFAULT_DURATION_MS));

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(wait).await;
        if let Err(e) = app_handle.notification().remove_active(vec![id]) {
            log::debug!("[Notification::Android] Dismissal of {} did not apply: {}", id, e);
        }
    });
}

/// Creates the notification channels. Safe to call on every launch.
///
/// INVARIANT: never delete and recreate. Channel importance is write-once by
/// the platform; recreating discards the user's override in Settings.
#[cfg(target_os = "android")]
pub fn ensure_channels<R: tauri::Runtime>(app: &AppHandle<R>) {
    use tauri_plugin_notification::{Channel, Importance, NotificationExt};

    let notification = app.notification();

    for channel in [
        Channel::builder(CHANNEL_INFO, "Vox updates")
            .description("Routine background progress.")
            .importance(Importance::Low)
            .build(),
        Channel::builder(CHANNEL_URGENT, "Vox alerts")
            .description("Problems that need your attention.")
            .importance(Importance::High)
            .build(),
    ] {
        if let Err(e) = notification.create_channel(channel) {
            log::warn!("[Notification::Android] Failed to create channel: {}", e);
        }
    }
}

#[cfg(target_os = "android")]
pub fn permission_status<R: tauri::Runtime>(app: &AppHandle<R>) -> PermissionStatus {
    use tauri_plugin_notification::{NotificationExt, PermissionState};

    match app.notification().permission_state() {
        Ok(PermissionState::Granted) => PermissionStatus::Granted,
        Ok(PermissionState::Denied) => PermissionStatus::Denied,
        Ok(PermissionState::Prompt | PermissionState::PromptWithRationale) => {
            PermissionStatus::NotAsked
        }
        Err(e) => {
            log::warn!("[Notification::Android] Permission state unavailable: {}", e);
            PermissionStatus::Unavailable
        }
    }
}

#[cfg(target_os = "android")]
pub fn request_permission<R: tauri::Runtime>(app: &AppHandle<R>) -> PermissionStatus {
    use tauri_plugin_notification::{NotificationExt, PermissionState};

    match app.notification().request_permission() {
        Ok(PermissionState::Granted) => PermissionStatus::Granted,
        Ok(PermissionState::Denied) => PermissionStatus::Denied,
        Ok(PermissionState::Prompt | PermissionState::PromptWithRationale) => {
            PermissionStatus::NotAsked
        }
        Err(e) => {
            log::warn!("[Notification::Android] Permission request failed: {}", e);
            PermissionStatus::Unavailable
        }
    }
}
