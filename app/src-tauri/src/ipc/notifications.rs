use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    core::{error::VoxIpcError, events::Severity, state::AppState},
    persistence::notifications::{
        dismiss_notifications as db_dismiss_notifications,
        fetch_active_notifications as db_fetch_active, mark_notifications_read as db_mark_read,
        NotificationFilter, NotificationRecord,
    },
    services::notifications::{
        deliver_transient, execute_notification_action as svc_execute_action,
        transient::{self, DeliveryOutcome, PermissionStatus, TransientAlert},
    },
};

/// Reports the notification permission state without prompting.
#[tauri::command]
pub fn notification_permission_state(app: AppHandle) -> Result<PermissionStatus, VoxIpcError> {
    Ok(transient::permission_status(&app))
}

/// Prompts for notification permission, resolving to the resulting state.
#[tauri::command]
pub fn request_notification_permission(app: AppHandle) -> Result<PermissionStatus, VoxIpcError> {
    Ok(transient::request_permission(&app))
}

/// Fires one synthetic `Action::Transient` alert at the real platform surface.
///
/// WHY THIS EXISTS
/// ---------------
/// `deliver_transient` is the only Android code path that posts a notification,
/// and on a fresh device nothing reaches it: the dictation lifecycle that emits
/// `Action::Transient` is desktop-only (see `services/dictation/**`), and the
/// remaining emitters need models the device does not have yet. So the new
/// Android notification facade could only be verified by inference — "the
/// channels exist" proves nothing about whether an alert actually renders.
///
/// This command makes the path reachable, deterministically, on real hardware.
///
/// INVARIANT — inert unless explicitly armed.
/// ----------------------------------------
/// This ships in RELEASE builds, because release is the only build worth
/// testing (see Pitfall 4: R8 minification is a release-only failure mode, and
/// `android-deploy.sh` produces release). A `#[cfg(debug_assertions)]` gate
/// would therefore be stripped from the very APK under test, so the gate is a
/// sentinel file instead.
///
/// Two locations are accepted, because arming must work on a release APK:
///
///   * `<app-private data dir>/dev_transient_probe` — the normal case.
///   * `<external files dir>/dev_transient_probe` — needed on Android release
///     builds, where `run-as` refuses ("package not debuggable") so `adb` cannot
///     write app-private storage, but external files are shell-writable.
///
/// Absent (the default, and always so on a user's device) this returns
/// `Skipped` without touching the notification surface, so it cannot be used to
/// spam a user's device. Arm it for a test run with:
///
///     adb shell touch /sdcard/Android/data/com.addy.Vox/files/dev_transient_probe
#[tauri::command]
pub fn debug_fire_transient(
    app: AppHandle,
    severity: Option<String>,
) -> Result<DebugTransientOutcome, VoxIpcError> {
    if !probe_is_armed() {
        log::debug!("[Notification::Probe] Not armed; skipping. See the command's doc comment.");
        return Ok(DebugTransientOutcome::Skipped);
    }

    let severity: Severity = severity
        .as_deref()
        .map(Severity::from)
        .unwrap_or(Severity::Info);

    let alert = TransientAlert {
        title: match severity {
            Severity::Info => "Vox probe (info)",
            Severity::Warning => "Vox probe (warning)",
            Severity::Critical => "Vox probe (critical)",
        },
        message: "Transient delivery probe",
        severity,
        duration_ms: None,
    };

    let outcome = deliver_transient(&app, &alert);
    log::info!(
        "[Notification::Probe] severity={} outcome={:?}",
        severity.as_str(),
        outcome
    );
    Ok(match outcome {
        DeliveryOutcome::Shown => DebugTransientOutcome::Shown,
        DeliveryOutcome::Failed => DebugTransientOutcome::Failed,
    })
}

/// Sentinel filename that arms [`debug_fire_transient`].
#[cfg(not(desktop))]
const PROBE_SENTINEL: &str = "dev_transient_probe";

/// Whether the transient probe has been explicitly armed.
///
/// Checks app-private storage first, then external storage — the latter because
/// `adb` cannot write app-private storage on a release APK (`run-as` refuses a
/// non-debuggable package), while `/sdcard/Android/data/<pkg>/` is shell-writable.
#[cfg(desktop)]
fn probe_is_armed() -> bool {
    crate::utils::paths::get()
        .root
        .join("dev_transient_probe")
        .exists()
}

#[cfg(not(desktop))]
fn probe_is_armed() -> bool {
    if crate::utils::paths::get()
        .root
        .join(PROBE_SENTINEL)
        .exists()
    {
        return true;
    }

    // `Context.getExternalFilesDir()` is the same directory adb reaches at
    // /sdcard/Android/data/<pkg>/files. Resolved through the Activity rather
    // than hardcoded, so it tracks the package name and scoped-storage rules.
    let Some(external) = android_external_files_dir() else {
        return false;
    };
    external.join(PROBE_SENTINEL).exists()
}

/// `Context.getExternalFilesDir()` as a Rust path, or `None` if unreachable.
#[cfg(not(desktop))]
fn android_external_files_dir() -> Option<std::path::PathBuf> {
    let ctx = ndk_context::android_context();
    // SAFETY: the context handle is initialized by ndk-glue at startup and
    // outlives this call; every fallible JNI step below is handled.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let context: jni::objects::JObject =
        unsafe { jni::objects::JObject::from_raw(ctx.context() as jni::sys::jobject) };

    let dir = env
        .call_method(&context, "getExternalFilesDir", "()Ljava/io/File;", &[])
        .and_then(|v| v.l())
        .ok()?;
    if dir.is_null() {
        return None;
    }
    let dir: jni::objects::JObject = unsafe { jni::objects::JObject::from_raw(dir.into_raw()) };

    let absolute = env
        .call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
        .ok()?;
    if absolute.is_null() {
        return None;
    }
    // Bound to a local so the `JavaStr` cannot outlive the `JString` it borrows.
    let absolute: jni::objects::JString =
        unsafe { jni::objects::JString::from_raw(absolute.into_raw()) };
    let path: String = env.get_string(&absolute).ok()?.into();
    Some(std::path::PathBuf::from(path))
}

/// Result of [`debug_fire_transient`], returned so a caller can assert on it
/// without parsing logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugTransientOutcome {
    /// The sentinel was absent; nothing was posted.
    Skipped,
    /// The platform accepted the alert.
    Shown,
    /// Delivery failed.
    Failed,
}

/// Retrieves all active notifications ordered newest first.
#[tauri::command]
pub async fn get_notifications(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<NotificationRecord>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(format!("Fetch notifications failed: {}", e)))?;
    db_fetch_active(&conn)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Fetch notifications failed: {}", e)))
}

/// Marks unread notifications matching the optional filter (or all unread) as read.
#[tauri::command]
pub async fn mark_notifications_read(
    filter: Option<NotificationFilter>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(format!("Mark notifications read failed: {}", e)))?;
    db_mark_read(&conn, filter.as_ref())
        .await
        .map_err(|e| VoxIpcError::Database(format!("Mark notifications read failed: {}", e)))?;

    Ok(())
}

/// Dismisses active notifications matching the optional filter (or all active).
#[tauri::command]
pub async fn dismiss_notifications(
    filter: Option<NotificationFilter>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(format!("Dismiss notifications failed: {}", e)))?;
    db_dismiss_notifications(&conn, filter.as_ref())
        .await
        .map_err(|e| VoxIpcError::Database(format!("Dismiss notifications failed: {}", e)))?;

    Ok(())
}

/// Polymorphic action executor for actionable notification cards.
#[tauri::command]
pub async fn execute_notification_action<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    id: String,
    _action: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    svc_execute_action(&app, state.inner(), &id)
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Execute notification action failed: {}", e)))
}
