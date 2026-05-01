mod audio;
mod vad;

use crate::audio::AudioStream;
use crate::vad::VadEngine;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, Emitter};
use ringbuf::traits::Split;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let _app_handle = app.handle().clone();
            
            // 1. Initialize MPSC channel for VAD events -> Tauri
            let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);

            // 2. Initialize VAD Engine and Audio Stream
            let resource_path = app.path().resource_dir()
                .expect("failed to get resource dir")
                .join("assets/ten_vad.onnx");
            
            // Fallback for dev environment where assets are in src-tauri/assets
            let model_path = if resource_path.exists() {
                resource_path
            } else {
                std::env::current_dir()?.join("assets/ten_vad.onnx")
            };

            let mut vad = VadEngine::new(model_path.to_str().expect("invalid model path"))
                .expect("failed to initialize VAD engine");

            let (producer, consumer) = ringbuf::HeapRb::<f32>::new(16000 * 2).split();

            let audio_stream = AudioStream::new(producer)
                .expect("failed to initialize audio stream");

            // 3. Spawn VAD Inference Task
            tauri::async_runtime::spawn(async move {
                if let Err(e) = vad.run_loop(consumer, tx).await {
                    eprintln!("VAD engine error: {}", e);
                }
            });

            // 4. Spawn Event Forwarder (MPSC -> Tauri Emit)
            let app_handle_emitter = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                        let msg_type = msg_type.to_string();
                        let _ = app_handle_emitter.emit(&msg_type, event);
                    }
                }
            });

            // Start Audio Stream
            audio_stream.start()?;

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
