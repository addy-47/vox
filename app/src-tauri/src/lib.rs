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
                let base_path = if cfg!(debug_assertions) {
                    std::env::current_dir().unwrap().join("../..")
                } else {
                    app_handle.path().resource_dir().expect("Failed to get resource dir")
                };

                let backend_script = base_path.join("backend/src/main.py");
                
                #[cfg(windows)]
                let python_bin = base_path.join("backend/.venv/Scripts/python.exe");
                #[cfg(not(windows))]
                let python_bin = base_path.join("backend/.venv/bin/python3");

                let (mut rx, _child) = app_handle.shell()
                    .command(python_bin.to_str().expect("Invalid python path"))
                    .args([backend_script.to_str().expect("Invalid script path")])
                    .spawn()
                    .expect("Failed to spawn python3 backend");

                while let Some(event) = rx.recv().await {
                    if let tauri_plugin_shell::process::CommandEvent::Stdout(line) = event {
                        let line_str = String::from_utf8_lossy(&line);
                        for part in line_str.lines() {
                            let trimmed = part.trim();
                            if trimmed.is_empty() { continue; }
                            
                            match serde_json::from_str::<serde_json::Value>(trimmed) {
                                Ok(json) => {
                                    if let Some(msg_type) = json.get("type").and_then(|v| v.as_str()) {
                                        let _ = app_handle.emit(msg_type, json.clone());
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Malformed IPC JSON frame: {} | Error: {}", trimmed, e);
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
