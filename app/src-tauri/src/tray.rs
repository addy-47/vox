use tauri::{Manager, State, AppHandle, WebviewWindow, Emitter};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;

#[cfg(target_os = "linux")]
use gtk::prelude::WidgetExt;

// ─── Managed State ───────────────────────────────────────────────────────────

/// Tracks if the HUD (Vox Live) is manually enabled via the tray menu.
pub struct HudVisibility(pub Arc<Mutex<bool>>);

/// Stores the handle to the 'Vox Live' checkable menu item for easy sync.
pub struct HudMenuItem(pub Arc<Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>>);

impl HudVisibility {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(false)))
    }
}

impl HudMenuItem {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Hides the transcription tray window.
#[tauri::command]
pub fn hide_tray_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}

/// Synchronizes the backend HUD state with the frontend visibility.
#[tauri::command]
pub async fn sync_hud_visibility(app: AppHandle, visible: bool) {
    let hud_visible_state: State<'_, HudVisibility> = app.state();
    let mut hud_lock = hud_visible_state.0.lock().await;
    *hud_lock = visible;

    let item_state: State<'_, HudMenuItem> = app.state();
    let item_lock = item_state.0.lock().await;
    if let Some(item) = &*item_lock {
        let _ = item.set_checked(visible);
    }
}

/// Sets whether the HUD window should ignore cursor events (click-through).
#[tauri::command]
pub fn set_hud_ignore_cursor(window: WebviewWindow, ignore: bool) {
    #[cfg(target_os = "linux")]
    {
        if ignore {
            if let Ok(gtk_window) = window.gtk_window() {
                gtk_window.input_shape_combine_region(None);
            }
        } else {
            // Restore the HUD-specific hitbox instead of making the whole window interactive
            setup_linux_virtual_layer(window.app_handle(), "tray");
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = window.set_ignore_cursor_events(ignore);
    }
}

/// Updates the interaction mode (Passive vs PTT).
#[tauri::command]
pub async fn update_interaction_mode(app: AppHandle, mode: String) -> Result<(), String> {
    let state: State<'_, crate::InteractionState> = app.state();
    let mut lock = state.0.lock().await;
    
    match mode.to_uppercase().as_str() {
        "PASSIVE" => {
            *lock = crate::InteractionMode::Passive;
            log::info!("[MODE] Switched to PASSIVE mode.");
        }
        "PTT" => {
            *lock = crate::InteractionMode::Ptt;
            log::info!("[MODE] Switched to PTT mode.");
        }
        _ => return Err(format!("Invalid interaction mode: {}", mode)),
    }
    
    let _ = app.emit("mode_changed", mode);
    Ok(())
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
        use tauri_plugin_positioner::{WindowExt, Position};
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

    let mon = window.primary_monitor().ok().flatten()
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

            log::debug!("[TRAY] Setting input region: x={}, y={}, w={}, h={} (scale={})", x, y, hud_w, hud_h, scale_factor);

            let rect = cairo::RectangleInt::new(x, y, hud_w, hud_h);
            let region = cairo::Region::create_rectangle(&rect);
            gtk_window.input_shape_combine_region(Some(&region));
        }
    }
}
