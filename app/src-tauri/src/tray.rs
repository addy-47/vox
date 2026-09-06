use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

#[cfg(target_os = "linux")]
use gtk::prelude::WidgetExt;
use tauri::{
    menu::{CheckMenuItem, CheckMenuItemBuilder, Menu, MenuItemBuilder, PredefinedMenuItem},
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::core::{
    constants::{
        TRAY_HUD_HEIGHT_LOGICAL, TRAY_HUD_WIDTH_LOGICAL, TRAY_PADDING_TOP_VH,
        TRAY_PADDING_X_LOGICAL, WINDOW_TRAY,
    },
    settings::DictationOutputMode,
    state::AppState,
};

/// Ensures the "tray" WebviewWindow exists, lazily constructing it if it was closed to save RAM.
pub fn ensure_tray_window<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<WebviewWindow<R>, String> {
    if let Some(existing) = app.get_webview_window(WINDOW_TRAY) {
        return Ok(existing);
    }

    log::info!("[Tray] Lazily constructing 'tray' HUD webview window...");
    let window = WebviewWindowBuilder::new(app, WINDOW_TRAY, WebviewUrl::App("/tray".into()))
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
pub fn destroy_tray_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(WINDOW_TRAY) {
        log::info!("[Tray] Destroying 'tray' HUD webview window to save RAM.");
        if let Err(e) = window.close() {
            log::warn!("[Tray] Failed to close tray window: {}", e);
        }
    }
}

/// Builds the main tray menu. The "Restart Vox" item is included **only** when a
/// renderer crash has been detected (`main_window_destroyed`), so the action is
/// not offered under normal operation.
pub fn build_main_tray_menu(
    app: &AppHandle<tauri::Wry>,
) -> tauri::Result<(Menu<tauri::Wry>, CheckMenuItem<tauri::Wry>)> {
    let state = app.state::<Arc<AppState>>();
    let crash_detected = state.main_window_destroyed.load(Ordering::Relaxed);

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
    let state = app.state::<Arc<AppState>>();
    {
        let mut menu_item_lock = state.hud_menu_item.lock();
        *menu_item_lock = Some(live_i.clone());
    }

    let (hud_visible, dictation_enabled, is_tray_mode) = {
        let s = state.settings.read().unwrap_or_else(|p| {
            log::warn!("[Tray] Settings RwLock poisoned; recovering inner state.");
            p.into_inner()
        });
        let v = state.hud_visible.load(Ordering::Relaxed);
        (
            v,
            s.dictation.enabled,
            s.dictation.output_mode == DictationOutputMode::Tray,
        )
    };
    let is_clickable = dictation_enabled && is_tray_mode;
    if let Err(e) = live_i.set_enabled(is_clickable) {
        log::debug!("[Tray] Failed to set menu item enabled: {}", e);
    }
    if let Err(e) = live_i.set_checked(hud_visible && is_clickable) {
        log::debug!("[Tray] Failed to set menu item checked: {}", e);
    }
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
        if let Err(e) = tray.set_menu(Some(tray_menu)) {
            log::warn!("[Tray] Failed to update tray menu: {}", e);
        }
    }
}

/// Configures the tray window with standard HUD settings: frameless, always-on-top, etc.
pub fn setup_tray_window<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    if let Err(e) = window.set_decorations(false) {
        log::debug!("[Tray] Failed to set window decorations: {}", e);
    }
    if let Err(e) = window.set_always_on_top(true) {
        log::debug!("[Tray] Failed to set window always on top: {}", e);
    }
    if let Err(e) = window.set_shadow(false) {
        log::debug!("[Tray] Failed to set window shadow: {}", e);
    }
    if let Err(e) = window.set_skip_taskbar(true) {
        log::debug!("[Tray] Failed to set window skip taskbar: {}", e);
    }
    if let Err(e) = window.set_resizable(false) {
        log::debug!("[Tray] Failed to set window resizable: {}", e);
    }
}

/// Positions the tray window at the top-right of the screen.
///
/// On Linux, this triggers the "virtual layer" setup for click-through support.
pub async fn position_tray_window<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "linux")]
    {
        if let Err(e) = window.show() {
            log::debug!(
                "[Tray] Failed to show tray window during positioning: {}",
                e
            );
        }
        let win_clone = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            setup_linux_virtual_layer(win_clone.app_handle(), WINDOW_TRAY);
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        use tauri_plugin_positioner::{Position, WindowExt};
        if let Err(e) = window.move_window(Position::TopRight) {
            log::debug!("[Tray] Failed to position window: {}", e);
        }
        if let Err(e) = window.show() {
            log::debug!("[Tray] Failed to show window: {}", e);
        }
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
            if let Err(e) = window.set_size(tauri::Size::Physical(*size)) {
                log::debug!("[Tray] Failed to set window size: {}", e);
            }
            if let Err(e) = window.set_position(tauri::Position::Physical(*mon.position())) {
                log::debug!("[Tray] Failed to set window position: {}", e);
            }
            if let Err(e) = window.set_always_on_top(true) {
                log::debug!("[Tray] Failed to set window always on top: {}", e);
            }
        }

        if let Ok(gtk_window) = window.gtk_window() {
            let scale_factor = window.scale_factor().unwrap_or(1.0);

            // Logical units from CSS/React (Sync with TrayApp.tsx and index.css)
            let hud_w_logical = TRAY_HUD_WIDTH_LOGICAL;
            let hud_h_logical = TRAY_HUD_HEIGHT_LOGICAL;
            let padding_x_logical = TRAY_PADDING_X_LOGICAL;
            let padding_top_vh = TRAY_PADDING_TOP_VH;

            // Convert to physical pixels for region math
            let hud_w = (hud_w_logical * scale_factor) as i32;
            let hud_h = (hud_h_logical * scale_factor) as i32;
            let padding_x = (padding_x_logical * scale_factor) as i32;

            let screen_w = size.width as i32;
            let screen_h = size.height as i32;

            let x = screen_w - hud_w - padding_x;
            let y = (screen_h as f64 * padding_top_vh) as i32;

            let rect = cairo::RectangleInt::new(x, y, hud_w, hud_h);
            let region = cairo::Region::create_rectangle(&rect);
            gtk_window.input_shape_combine_region(Some(&region));
        }
    }
}
