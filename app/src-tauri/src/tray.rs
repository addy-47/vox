use std::time::Duration;
use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, Menu, MenuItemBuilder, PredefinedMenuItem};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[cfg(target_os = "linux")]
use gtk::prelude::WidgetExt;

/// Ensures the "tray" WebviewWindow exists, lazily constructing it if it was closed to save RAM.
pub fn ensure_tray_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window("tray") {
        return Ok(existing);
    }

    log::info!("[Tray] Lazily constructing 'tray' HUD webview window...");
    let window = WebviewWindowBuilder::new(app, "tray", WebviewUrl::App("/tray".into()))
        .title("vox-live")
        .inner_size(420.0, 250.0)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .resizable(false)
        .visible(false)
        .shadow(false)
        .zoom_hotkeys_enabled(false)
        .skip_taskbar(true)
        .build()
        .map_err(|e| format!("Failed to create tray window: {}", e))?;

    setup_tray_window(&window);
    let win_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        position_tray_window(&win_clone).await;
    });

    Ok(window)
}

/// Safely closes and destroys the tray window to reclaim memory when Tray mode is inactive.
pub fn destroy_tray_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        log::info!("[Tray] Destroying 'tray' HUD webview window to save RAM.");
        let _ = window.close();
    }
}

/// Builds the main tray menu. The "Restart Vox" item is included **only** when a
/// renderer crash has been detected (`main_window_destroyed`), so the action is
/// not offered under normal operation.
pub fn build_main_tray_menu(
    app: &AppHandle<tauri::Wry>,
) -> tauri::Result<(Menu<tauri::Wry>, CheckMenuItem<tauri::Wry>)> {
    let state = app.state::<std::sync::Arc<crate::core::state::AppState>>();
    let crash_detected = state
        .main_window_destroyed
        .load(std::sync::atomic::Ordering::Relaxed);

    let tray_menu = Menu::new(app)?;
    let launch_i = MenuItemBuilder::new("Launch Vox").id("launch").build(app)?;
    let live_i = CheckMenuItemBuilder::new("Vox Live")
        .id("live")
        .build(app)?;
    tray_menu.append(&launch_i)?;
    tray_menu.append(&live_i)?;

    if crash_detected {
        tray_menu.append(&PredefinedMenuItem::separator(app)?)?;
        let restart_i = MenuItemBuilder::new("Restart Vox")
            .id("restart")
            .build(app)?;
        tray_menu.append(&restart_i)?;
    }

    tray_menu.append(&PredefinedMenuItem::separator(app)?)?;
    let quit_i = MenuItemBuilder::new("Quit").id("quit").build(app)?;
    tray_menu.append(&quit_i)?;

    Ok((tray_menu, live_i))
}

/// Syncs the "Vox Live" check item against current dictation settings and stores
/// its handle in `AppState` for backend-driven updates.
pub fn sync_live_menu_item(app: &AppHandle<tauri::Wry>, live_i: &CheckMenuItem<tauri::Wry>) {
    let state = app.state::<std::sync::Arc<crate::core::state::AppState>>();
    {
        let mut menu_item_lock = tauri::async_runtime::block_on(state.hud_menu_item.lock());
        *menu_item_lock = Some(live_i.clone());
    }

    let (hud_visible, dictation_enabled, is_tray_mode) = {
        let s = state.settings.read().unwrap();
        let v = tauri::async_runtime::block_on(state.hud_visible.lock());
        (
            *v,
            s.dictation.enabled,
            s.dictation.output_mode == crate::core::settings::DictationOutputMode::Tray,
        )
    };
    let is_clickable = dictation_enabled && is_tray_mode;
    let _ = live_i.set_enabled(is_clickable);
    let _ = live_i.set_checked(hud_visible && is_clickable);
}

/// Rebuilds the main tray menu (honoring the crash flag) and re-applies it to the
/// live tray icon. Used after a renderer crash and after a window rebuild resets it.
pub fn refresh_tray_menu(app: &AppHandle<tauri::Wry>) {
    let (tray_menu, live_i) = match build_main_tray_menu(app) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("[Tray] Failed to rebuild tray menu: {}", e);
            return;
        }
    };
    sync_live_menu_item(app, &live_i);
    if let Some(tray) = app.tray_by_id("vox-tray") {
        let _ = tray.set_menu(Some(tray_menu));
    }
}

/// Configures the tray window with standard HUD settings: frameless, always-on-top, etc.
pub fn setup_tray_window(window: &WebviewWindow) {
    let _ = window.set_decorations(false);
    let _ = window.set_always_on_top(true);
    let _ = window.set_shadow(false);
    let _ = window.set_skip_taskbar(true);
    let _ = window.set_resizable(false);
}

// ─── Positioning Logic ───────────────────────────────────────────────────────

/// Positions the tray window at the top-right of the screen.
///
/// On Linux, this triggers the "virtual layer" setup for click-through support.
pub async fn position_tray_window(window: &WebviewWindow) {
    #[cfg(target_os = "linux")]
    {
        let _ = window.show();
        let win_clone = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            setup_linux_virtual_layer(win_clone.app_handle(), "tray");
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        use tauri_plugin_positioner::{Position, WindowExt};
        let _ = window.move_window(Position::TopRight);
        let _ = window.show();
    }
}

/// Configures a fullscreen transparent "Virtual Layer" on Linux Wayland/X11.
///
/// This creates a click-through region for the HUD while allowing it to appear
/// correctly above other windows despite compositor restrictions.
#[cfg(target_os = "linux")]
pub fn setup_linux_virtual_layer<R: tauri::Runtime>(app: &AppHandle<R>, label: &str) {
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

        // RCA: Making the window fullscreen is what prevents dragging on Linux/Wayland.
        if cur_size.width != size.width || cur_size.height != size.height {
            let _ = window.set_size(tauri::Size::Physical(*size));
            let _ = window.set_position(tauri::Position::Physical(*mon.position()));
            let _ = window.set_always_on_top(true);
        }

        if let Ok(gtk_window) = window.gtk_window() {
            let scale_factor = window.scale_factor().unwrap_or(1.0);

            // Logical units from CSS/React (Sync with TrayApp.tsx and index.css)
            let hud_w_logical = 380.0;
            let hud_h_logical = 250.0;
            let padding_x_logical = 55.0;
            let padding_top_vh = 0.15; // 15vh

            // Convert to physical pixels for region math
            let hud_w = (hud_w_logical * scale_factor) as i32;
            let hud_h = (hud_h_logical * scale_factor) as i32;
            let padding_x = (padding_x_logical * scale_factor) as i32;

            let screen_w = size.width as i32;
            let screen_h = size.height as i32;

            let x = screen_w - hud_w - padding_x;
            let y = (screen_h as f64 * padding_top_vh) as i32;

            // log::debug!("[TRAY] Setting input region: x={}, y={}, w={}, h={} (scale={})", x, y, hud_w, hud_h, scale_factor);

            let rect = cairo::RectangleInt::new(x, y, hud_w, hud_h);
            let region = cairo::Region::create_rectangle(&rect);
            gtk_window.input_shape_combine_region(Some(&region));
        }
    }
}
