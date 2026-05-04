mod audio;
pub mod vad;
pub mod stt;

use crate::audio::AudioStream;
use crate::vad::VadEngine;
use crate::stt::{SttEngine, SttCommand};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, Emitter};
use ringbuf::traits::Split;

#[tauri::command]
fn hide_tray_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // ── 1. VAD event channel (VAD → Tauri frontend) ───────────────
            let (vad_tx, mut vad_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);

            // ── 2. STT command channel (VAD → STT worker thread) ──────────
            let (stt_tx, stt_rx) = tokio::sync::mpsc::unbounded_channel::<SttCommand>();

            // ── 3. STT worker — dedicated OS thread (CPU-bound, not async) ─
            //
            // Architecture rule: ONNX inference is synchronous/CPU-bound.
            // Putting it inside tauri::async_runtime::spawn would starve
            // the async executor. We use std::thread::spawn + blocking_recv.
            //
            // Throttle: only run encoder+decoder if 800ms of NEW audio has
            // accumulated since the last partial emit (12800 samples @ 16kHz).
            //
            let stt_model_path = std::env::current_dir()?.join("assets/qwen3-asr");
            let app_handle_stt = app_handle.clone();

            std::thread::spawn(move || {
                // Initialize STT engine on this thread
                let mut engine = match SttEngine::new(&stt_model_path) {
                    Ok(e) => {
                        log::info!("[STT] Engine initialized successfully");
                        e
                    },
                    Err(err) => {
                        log::error!("[STT] Failed to initialize SttEngine: {}", err);
                        return;
                    }
                };

                // Convert the async tokio receiver to blocking via blocking_recv
                let mut rx = stt_rx;

                loop {
                    match rx.blocking_recv() {
                        None => break, // channel closed — engine shutting down
                        Some(cmd) => match cmd {
                            SttCommand::Transcribe(utterance) => {
                                // Full utterance from VAD (speech_start → speech_end).
                                // VAD accumulates audio and sends it atomically here.
                                log::info!("[STT] Transcribing {} samples ({:.1}s)",
                                    utterance.len(), utterance.len() as f32 / 16000.0);
                                 let handle = app_handle_stt.clone();
                                 match engine.transcribe(&utterance, |text| {
                                     let _ = handle.emit("transcript_partial", serde_json::json!({
                                         "text": text
                                     }));
                                 }) {
                                     Ok(text) => {
                                         log::info!("[STT] Final: {:?}", text);
                                         let _ = app_handle_stt.emit("transcript_final", serde_json::json!({
                                             "text": text
                                         }));
                                     }
                                     Err(e) => log::error!("[STT] transcribe error: {}", e),
                                 }
                            }
                        }
                    }
                }
            });

            // ── 4. VAD model path (dev vs prod) ───────────────────────────
            let resource_path = app.path().resource_dir()
                .expect("failed to get resource dir")
                .join("assets/ten_vad.onnx");

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

            // ── 5. VAD inference task (async, non-blocking) ───────────────
            tauri::async_runtime::spawn(async move {
                log::info!("[VAD] Engine starting...");
                if let Err(e) = vad.run_loop(consumer, vad_tx, stt_tx).await {
                    log::error!("[VAD] engine error: {}", e);
                }
            });

            // ── 6. VAD event forwarder → Tauri frontend ───────────────────
            let app_handle_emit = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                log::info!("[PIPELINE] Event forwarder started.");
                while let Some(event) = vad_rx.recv().await {
                    log::debug!("[PIPELINE] Received event: {:?}", event);
                    if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                        let msg_type = msg_type.to_string();

                        // Automatically show the tray window when speech starts
                        if msg_type == "speech_start" {
                            if let Some(window) = app_handle_emit.get_webview_window("tray") {
                                let _ = window.show();
                                log::info!("[PIPELINE] Speech detected! Showing tray.");
                            } else {
                                log::error!("[PIPELINE] Tray window NOT FOUND! check tauri.conf.json");
                            }
                        }

                        // Forward transcription events to the tray
                        let _ = app_handle_emit.emit(&msg_type, &event);
                    }
                }
            });

            // ── 7. Start audio capture ────────────────────────────────────
            audio_stream.start()?;
            // Control-leak the stream to prevent it from being dropped when setup ends.
            // cpal::Stream is often !Send/!Sync on some platforms, preventing it from being
            // managed as Tauri State or moved to another thread easily.
            Box::leak(Box::new(audio_stream));

            // ── 8. System tray menu ───────────────────────────────────────
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // ── 9. Position tray HUD — right edge, vertically centered ────
            //
            // Mandate: x = monitor_width - tray_width - 20, y = center
            if let Some(tray_win) = app.get_webview_window("tray") {
                if let Ok(Some(monitor)) = tray_win.primary_monitor() {
                    let screen = monitor.size();
                    let win_size = tray_win
                        .outer_size()
                        .unwrap_or(tauri::PhysicalSize::new(360, 500));
                    let scale = monitor.scale_factor();

                    // 20px padding from right edge (in physical pixels)
                    let padding = (20.0 * scale) as i32;
                    let x = screen.width as i32 - win_size.width as i32 - padding;
                    let y = (screen.height as i32 - win_size.height as i32) / 2;

                    let _ = tray_win.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![hide_tray_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
