mod audio;
mod vad;
mod stt;

use crate::audio::AudioStream;
use crate::vad::VadEngine;
use crate::stt::{SttEngine, SttCommand};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, Emitter};
use ringbuf::traits::Split;
use std::time::Instant;

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
                    Ok(e) => e,
                    Err(err) => {
                        eprintln!("[STT] Failed to initialize SttEngine: {}", err);
                        return;
                    }
                };

                let mut audio_buffer: Vec<f32> = Vec::with_capacity(16000 * 10);
                let mut samples_at_last_partial: usize = 0;
                let mut last_partial_time = Instant::now();

                // Convert the async tokio receiver to blocking via blocking_recv
                let mut rx = stt_rx;

                loop {
                    match rx.blocking_recv() {
                        None => break, // channel closed — engine shutting down
                        Some(cmd) => match cmd {
                            SttCommand::Audio(chunk) => {
                                audio_buffer.extend_from_slice(&chunk);

                                // Throttle: only transcribe if ≥800ms (12800 samples)
                                // of NEW audio has accumulated since last partial.
                                let new_samples = audio_buffer.len().saturating_sub(samples_at_last_partial);
                                let elapsed_ms = last_partial_time.elapsed().as_millis();

                                if new_samples >= 12800 || elapsed_ms >= 800 {
                                    match engine.transcribe(&audio_buffer) {
                                        Ok(text) if !text.is_empty() => {
                                            let _ = app_handle_stt.emit("transcript_partial", serde_json::json!({
                                                "text": text
                                            }));
                                        }
                                        Ok(_) => {}
                                        Err(e) => eprintln!("[STT] partial transcribe error: {}", e),
                                    }
                                    samples_at_last_partial = audio_buffer.len();
                                    last_partial_time = Instant::now();
                                }
                            }

                            SttCommand::Clear => {
                                // speech_end: run one final definitive pass
                                if !audio_buffer.is_empty() {
                                    match engine.transcribe(&audio_buffer) {
                                        Ok(text) => {
                                            let _ = app_handle_stt.emit("transcript_final", serde_json::json!({
                                                "text": text
                                            }));
                                        }
                                        Err(e) => eprintln!("[STT] final transcribe error: {}", e),
                                    }
                                }
                                // Reset for next utterance
                                audio_buffer.clear();
                                samples_at_last_partial = 0;
                                last_partial_time = Instant::now();
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
                if let Err(e) = vad.run_loop(consumer, vad_tx, stt_tx).await {
                    eprintln!("[VAD] engine error: {}", e);
                }
            });

            // ── 6. VAD event forwarder → Tauri frontend ───────────────────
            let app_handle_emit = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = vad_rx.recv().await {
                    if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                        let msg_type = msg_type.to_string();

                        // Mandate: show tray window immediately on speech_start
                        if msg_type == "speech_start" {
                            if let Some(win) = app_handle_emit.get_webview_window("tray") {
                                let _ = win.show();
                                // Do NOT steal focus — tray must not interrupt the user
                            }
                        }

                        let _ = app_handle_emit.emit(&msg_type, &event);
                    }
                }
            });

            // ── 7. Start audio capture ────────────────────────────────────
            audio_stream.start()?;

            // ── 8. System tray menu ───────────────────────────────────────
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
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
