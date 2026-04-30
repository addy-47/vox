use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, Emitter};
use tauri_plugin_shell::ShellExt;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            
            tauri::async_runtime::spawn(async move {
                let (mut rx, _child) = app_handle.shell().command("python")
                    .args(["backend/src/main.py"])
                    .spawn()
                    .expect("Failed to spawn python backend");
                
                while let Some(event) = rx.recv().await {
                    if let tauri_plugin_shell::process::CommandEvent::Stdout(line) = event {
                        let line_str = String::from_utf8_lossy(&line);
                        // The python script might output multiple lines or buffered output,
                        // split by newline to be safe.
                        for part in line_str.lines() {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(part) {
                                if let Some(msg_type) = json.get("type").and_then(|v| v.as_str()) {
                                    let _ = app_handle.emit(msg_type, json.clone());
                                }
                            }
                        }
                    }
                }
            });

            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            // Position Tray HUD on the right edge
            if let Some(window) = app.get_webview_window("tray") {
                if let Some(monitor) = window.current_monitor().unwrap_or(None) {
                    let screen_size = monitor.size();
                    let win_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(360, 500));
                    
                    let x = screen_size.width as i32 - win_size.width as i32;
                    let y = (screen_size.height as i32 - win_size.height as i32) / 2;
                    
                    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
