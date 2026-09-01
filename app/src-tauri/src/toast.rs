use std::sync::LazyLock;
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[cfg(target_os = "linux")]
use gtk::prelude::WidgetExt;

const TOAST_WIDTH: f64 = 360.0;
const TOAST_HEIGHT: f64 = 96.0;
const TOAST_PAD_TOP: f64 = 24.0;

static LAST_TOAST: LazyLock<parking_lot::Mutex<Option<crate::core::events::ToastPayload>>> =
    LazyLock::new(|| parking_lot::Mutex::new(None));

/// Ensures the "toast" WebviewWindow exists, lazily constructing it if absent.
///
/// Window is created `visible(false)` and **stays hidden** until the frontend
/// signals it has mounted and painted its first frame. This prevents the
/// WebKitGTK black flash that occurs when a transparent window is shown
/// before the page has set `html.is-toast → transparent`.
pub fn ensure_toast_window<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<WebviewWindow<R>, String> {
    if let Some(existing) = app.get_webview_window("toast") {
        return Ok(existing);
    }

    log::info!("[Toast] Lazily constructing 'toast' overlay webview window...");
    let window = WebviewWindowBuilder::new(app, "toast", WebviewUrl::App("/toast".into()))
        .title("vox-toast")
        .inner_size(TOAST_WIDTH, TOAST_HEIGHT)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .resizable(false)
        .visible(false)
        .shadow(false)
        .zoom_hotkeys_enabled(false)
        .skip_taskbar(true)
        .focused(false)
        .build()
        .map_err(|e| format!("Failed to create toast window: {}", e))?;

    setup_toast_window(&window);
    // Pre-size virtual layer so the first `show()` is already fullscreen + click-through.
    // Intentionally NOT calling `show()` here — frontend owns the first show after mount.
    #[cfg(target_os = "linux")]
    {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            // Give WebKit a moment to create the GTK widget before querying monitors.
            tokio::time::sleep(Duration::from_millis(120)).await;
            setup_linux_toast_layer(&app_clone, "toast");
        });
    }

    Ok(window)
}

/// Safely closes and destroys the toast window to reclaim memory when idle.
pub fn destroy_toast_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("toast") {
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
}

/// Positions the toast window at top-center with 24px top inset.
///
/// This only sizes/positions — it never calls `show()`. Visibility is owned by
/// the frontend so the window is only presented after the transparent page has
/// painted.
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

    let mon = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.app_handle().primary_monitor().ok().flatten());

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

        if let Ok(gtk_window) = window.gtk_window() {
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
        }
    }
}

/// Shows the toast window (called by frontend after it has painted).
#[tauri::command]
pub fn show_toast_window<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("toast") {
        // Ensure virtual layer is correctly sized before presenting
        #[cfg(target_os = "linux")]
        setup_linux_toast_layer(&app, "toast");
        window
            .show()
            .map_err(|e| format!("Failed to show toast window: {}", e))?;
    }
    Ok(())
}

/// Hides the toast window without destroying it (auto-dismiss via frontend).
#[tauri::command]
pub fn hide_toast_window<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("toast") {
        window
            .hide()
            .map_err(|e| format!("Failed to hide toast window: {}", e))?;
    }
    Ok(())
}

/// Destroys the toast window to reclaim RAM after auto-dismiss.
#[tauri::command]
pub fn destroy_toast_window_cmd<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), String> {
    destroy_toast_window(&app);
    Ok(())
}

/// Returns the last emitted toast for late-joining webviews that missed the `show_toast` event.
///
/// Frontend calls this on mount to avoid losing the burst emitted during window creation.
#[tauri::command]
pub fn get_last_toast() -> Option<crate::core::events::ToastPayload> {
    LAST_TOAST.lock().clone()
}

/// Returns true when the main window is hidden and a toast should supplement the error.
pub fn should_show_error_toast<R: tauri::Runtime>(app: &AppHandle<R>) -> bool {
    match app.get_webview_window(crate::pipeline::WINDOW_MAIN) {
        Some(w) => !w.is_visible().unwrap_or(true),
        None => true,
    }
}

/// Lazily ensures the toast window, positions it, and emits a `show_toast` event.
///
/// The window stays hidden until the frontend mounts and calls `show()` itself
/// after its first paint, eliminating the black flash.
pub fn show_toast<R: tauri::Runtime>(
    app: &AppHandle<R>,
    title: &str,
    message: &str,
    level: crate::core::events::ToastLevel,
) -> Result<(), String> {
    let _window = ensure_toast_window(app)?;
    let title_owned = title.to_string();
    let message_owned = message.to_string();

    let payload = crate::core::events::ToastPayload {
        title: title_owned,
        message: message_owned,
        level,
        duration_ms: None,
    };

    *LAST_TOAST.lock() = Some(payload.clone());

    let payload_for_emit = payload.clone();
    let app_for_emit = app.clone();
    let win_for_show = _window.clone();
    tauri::async_runtime::spawn(async move {
        // Give a fresh webview time to create its GTK widget + mount React listeners.
        tokio::time::sleep(Duration::from_millis(420)).await;
        if let Err(e) = crate::core::events::emit_ipc_to(
            &app_for_emit,
            "toast",
            crate::core::events::IpcEvent::ShowToast(payload_for_emit.clone()),
        ) {
            log::warn!("[Toast] Delayed emit show_toast failed: {}", e);
        }
        // Re-emit once more after content has definitely painted, to survive any late mount.
        tokio::time::sleep(Duration::from_millis(300)).await;
        if let Err(e) = crate::core::events::emit_ipc_to(
            &app_for_emit,
            "toast",
            crate::core::events::IpcEvent::ShowToast(payload_for_emit),
        ) {
            log::debug!("[Toast] Second emit (late mount) failed: {}", e);
        }
        // Fallback: if frontend hasn't shown the window (e.g. event missed / frontend show failed),
        // ensure the window is still presented so the toast isn't silently lost.
        tokio::time::sleep(Duration::from_millis(300)).await;
        if let Some(w) = app_for_emit.get_webview_window("toast") {
            if !w.is_visible().unwrap_or(true) {
                log::warn!("[Toast] Fallback showing toast window (frontend did not show)");
                if let Err(e) = w.show() {
                    log::warn!("[Toast] Fallback show failed: {}", e);
                }
                // Ensure layer after fallback show
                tokio::time::sleep(Duration::from_millis(120)).await;
                setup_linux_toast_layer(&app_for_emit, "toast");
            }
        }
        // Keep a handle alive so the window isn't dropped
        drop(win_for_show);
    });

    if let Err(e) = crate::core::events::emit_ipc_to(
        app,
        "toast",
        crate::core::events::IpcEvent::ShowToast(payload),
    ) {
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
        setup_linux_toast_layer(&app_clone, "toast");
    });

    Ok(())
}
