use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::LazyLock,
    time::Duration,
};

#[cfg(target_os = "linux")]
use gtk::prelude::{GtkWindowExt, WidgetExt};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::core::{
    events::{emit_ipc_to, IpcEvent, Severity, ToastPayload},
    state::AppWindow,
};

static LAST_TOAST: LazyLock<parking_lot::Mutex<Option<ToastPayload>>> =
    LazyLock::new(|| parking_lot::Mutex::new(None));

const TOAST_WIDTH: f64 = 360.0;
const TOAST_HEIGHT: f64 = 96.0;
const TOAST_PAD_TOP: f64 = 24.0;

/// Delivery outcome of attempting to dispatch a toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastDeliveryOutcome {
    /// Toast overlay window successfully scheduled and displayed.
    Shown,
    /// Toast overlay was suppressed because the main application window is focused in the foreground.
    SuppressedFocused,
    /// Webview window construction or display failed.
    Failed,
}

/// Ensures the "toast" WebviewWindow exists, lazily constructing it if absent.
pub fn ensure_toast_window<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<WebviewWindow<R>, String> {
    if let Some(existing) = app.get_webview_window(AppWindow::Toast.as_str()) {
        return Ok(existing);
    }

    log::info!("[Toast] Lazily constructing 'toast' overlay webview window...");
    let window = WebviewWindowBuilder::new(
        app,
        AppWindow::Toast.as_str(),
        WebviewUrl::App("/toast".into()),
    )
    .title("vox-toast")
    .inner_size(TOAST_WIDTH, TOAST_HEIGHT)
    .transparent(true)
    .decorations(false)
    .always_on_top(true)
    .resizable(false)
    .visible(false)
    .skip_taskbar(true)
    .build()
    .map_err(|e| format!("Failed to create toast window: {}", e))?;

    setup_toast_window(&window);
    #[cfg(target_os = "linux")]
    {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(120)).await;
            setup_linux_toast_layer(&app_clone, AppWindow::Toast.as_str());
        });
    }

    Ok(window)
}

/// Safely closes and destroys the toast window to reclaim memory when idle.
pub fn destroy_toast_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(AppWindow::Toast.as_str()) {
        log::info!("[Toast] Destroying 'toast' overlay window to save RAM.");
        if let Err(e) = window.close() {
            log::warn!("[Toast] Failed to close toast window: {}", e);
        }
    }
}

/// Configures the toast window with standard overlay settings.
pub fn setup_toast_window<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    if let Err(e) = window.set_decorations(false) {
        log::debug!("[Toast] Failed to set window decorations: {}", e);
    }
    if let Err(e) = window.set_always_on_top(true) {
        log::debug!("[Toast] Failed to set window always on top: {}", e);
    }
    if let Err(e) = window.set_shadow(false) {
        log::debug!("[Toast] Failed to set window shadow: {}", e);
    }
    if let Err(e) = window.set_skip_taskbar(true) {
        log::debug!("[Toast] Failed to set window skip taskbar: {}", e);
    }
    if let Err(e) = window.set_resizable(false) {
        log::debug!("[Toast] Failed to set window resizable: {}", e);
    }
    #[cfg(target_os = "linux")]
    with_gtk_window(window, |gtk_window| {
        gtk_window.set_accept_focus(false);
        gtk_window.set_focus_on_map(false);
    });
}

/// Positions the toast window at top-center with 24px top inset.
pub async fn position_toast_window<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "linux")]
    {
        let win_clone = window.clone();
        tokio::time::sleep(Duration::from_millis(80)).await;
        setup_linux_toast_layer(win_clone.app_handle(), "toast");
    }

    #[cfg(not(target_os = "linux"))]
    {
        use tauri_plugin_positioner::{Position, WindowExt};
        if let Err(e) = window.move_window(Position::TopCenter) {
            log::debug!("[Toast] Failed to position window: {}", e);
        }
    }
}

/// Configures a fullscreen transparent "Virtual Layer" for the toast on Linux.
#[cfg(target_os = "linux")]
pub fn setup_linux_toast_layer<R: tauri::Runtime>(app: &AppHandle<R>, label: &str) {
    let window = match app.get_webview_window(label) {
        Some(w) => w,
        None => return,
    };

    let mon = catch_unwind(AssertUnwindSafe(|| {
        window
            .primary_monitor()
            .ok()
            .flatten()
            .or_else(|| window.current_monitor().ok().flatten())
            .or_else(|| window.app_handle().primary_monitor().ok().flatten())
    }))
    .ok()
    .flatten();

    if let Some(mon) = mon {
        let size = mon.size();
        let cur_size = window.outer_size().unwrap_or_default();

        if cur_size.width != size.width || cur_size.height != size.height {
            if let Err(e) = window.set_size(tauri::Size::Physical(*size)) {
                log::debug!("[Toast] Failed to set window size: {}", e);
            }
            if let Err(e) = window.set_position(tauri::Position::Physical(*mon.position())) {
                log::debug!("[Toast] Failed to set window position: {}", e);
            }
            if let Err(e) = window.set_always_on_top(true) {
                log::debug!("[Toast] Failed to set window always on top: {}", e);
            }
        }

        with_gtk_window(&window, |gtk_window| {
            gtk_window.set_accept_focus(false);
            gtk_window.set_focus_on_map(false);

            let scale_factor = window.scale_factor().unwrap_or(1.0);

            let toast_w = (TOAST_WIDTH * scale_factor) as i32;
            let toast_h = (TOAST_HEIGHT * scale_factor) as i32;
            let pad_t = (TOAST_PAD_TOP * scale_factor) as i32;

            let screen_w = size.width as i32;

            let x = (screen_w - toast_w) / 2;
            let y = pad_t;

            let rect = cairo::RectangleInt::new(x, y, toast_w, toast_h);
            let region = cairo::Region::create_rectangle(&rect);
            gtk_window.input_shape_combine_region(Some(&region));
        });
    }
}

/// Manages toast window lifecycle states (show, hide, destroy).
#[tauri::command]
pub fn manage_toast_window<R: tauri::Runtime>(
    app: AppHandle<R>,
    action: String,
) -> Result<(), String> {
    match action.to_lowercase().as_str() {
        "show" => {
            if let Some(window) = app.get_webview_window("toast") {
                #[cfg(target_os = "linux")]
                setup_linux_toast_layer(&app, "toast");
                window
                    .show()
                    .map_err(|e| format!("Failed to show toast window: {}", e))?;
            }
        }
        "hide" => {
            if let Some(window) = app.get_webview_window("toast") {
                window
                    .hide()
                    .map_err(|e| format!("Failed to hide toast window: {}", e))?;
            }
        }
        "destroy" => {
            destroy_toast_window(&app);
        }
        _ => return Err(format!("Unknown toast action: {}", action)),
    }
    Ok(())
}

/// Returns the last emitted toast for late-joining webviews that missed the `show_toast` event.
#[tauri::command]
pub fn get_last_toast() -> Option<ToastPayload> {
    LAST_TOAST.lock().clone()
}

/// Returns true when the main window is hidden or unfocused and a toast should supplement the error.
pub fn should_show_error_toast<R: tauri::Runtime>(app: &AppHandle<R>) -> bool {
    match app.get_webview_window(AppWindow::Main.as_str()) {
        Some(w) => {
            let is_visible = w.is_visible().unwrap_or(true);
            let is_focused = w.is_focused().unwrap_or(false);
            !is_visible || !is_focused
        }
        None => true,
    }
}

/// Lazily ensures the toast window, positions it, and emits a `show_toast` event.
/// Suppresses floating overlay window when the main app window is focused, returning `SuppressedFocused`.
pub fn show_toast<R: tauri::Runtime>(
    app: &AppHandle<R>,
    title: &str,
    message: &str,
    severity: Severity,
    duration_ms: Option<u64>,
) -> ToastDeliveryOutcome {
    if let Some(main_win) = app.get_webview_window(AppWindow::Main.as_str()) {
        if main_win.is_visible().unwrap_or(false) && main_win.is_focused().unwrap_or(false) {
            log::debug!("[Toast] Main window is focused; suppressing floating toast overlay");
            return ToastDeliveryOutcome::SuppressedFocused;
        }
    }

    let window_res = catch_unwind(AssertUnwindSafe(|| ensure_toast_window(app)));

    let _window = match window_res {
        Ok(Ok(w)) => w,
        Ok(Err(e)) => {
            log::warn!("[Toast] ensure_toast_window failed: {}", e);
            return ToastDeliveryOutcome::Failed;
        }
        Err(_) => {
            log::debug!("[Toast] Window creation unimplemented on current runtime (mock/headless)");
            return ToastDeliveryOutcome::Failed;
        }
    };
    let title_owned = title.to_string();
    let message_owned = message.to_string();

    let payload = ToastPayload {
        title: title_owned,
        message: message_owned,
        severity,
        duration_ms,
    };

    *LAST_TOAST.lock() = Some(payload.clone());

    let payload_for_emit = payload.clone();
    let app_for_emit = app.clone();
    let win_for_show = _window.clone();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(420)).await;
        if let Err(e) = emit_ipc_to(
            &app_for_emit,
            AppWindow::Toast,
            IpcEvent::ShowToast(payload_for_emit.clone()),
        ) {
            log::warn!("[Toast] Delayed emit show_toast failed: {}", e);
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
        if let Err(e) = emit_ipc_to(
            &app_for_emit,
            AppWindow::Toast,
            IpcEvent::ShowToast(payload_for_emit),
        ) {
            log::debug!("[Toast] Second emit (late mount) failed: {}", e);
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        if let Some(w) = app_for_emit.get_webview_window(AppWindow::Toast.as_str()) {
            if !w.is_visible().unwrap_or(true) {
                log::warn!("[Toast] Fallback showing toast window (frontend did not show)");
                if let Err(e) = w.show() {
                    log::warn!("[Toast] Fallback show failed: {}", e);
                }
                tokio::time::sleep(Duration::from_millis(120)).await;
                #[cfg(target_os = "linux")]
                setup_linux_toast_layer(&app_for_emit, AppWindow::Toast.as_str());
            }
        }
        drop(win_for_show);
    });

    if let Err(e) = emit_ipc_to(app, AppWindow::Toast, IpcEvent::ShowToast(payload)) {
        log::debug!(
            "[Toast] Immediate emit show_toast (expected miss before mount): {}",
            e
        );
    }

    // Ensure layer is correctly sized for the next `show()` triggered by the frontend.
    let win_clone = _window.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        position_toast_window(&win_clone).await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        #[cfg(target_os = "linux")]
        setup_linux_toast_layer(&app_clone, "toast");
    });

    ToastDeliveryOutcome::Shown
}

/// Runs a GTK-handle operation without letting exotic backends panic the caller.
#[cfg(target_os = "linux")]
fn with_gtk_window<R: tauri::Runtime>(
    window: &WebviewWindow<R>,
    f: impl FnOnce(&gtk::ApplicationWindow),
) {
    let result = catch_unwind(AssertUnwindSafe(|| window.gtk_window()));
    match result {
        Ok(Ok(gtk_window)) => f(&gtk_window),
        Ok(Err(e)) => log::debug!("[Toast] No GTK handle available: {}", e),
        Err(_) => log::debug!("[Toast] GTK backend unimplemented (mock/headless runtime)"),
    }
}
