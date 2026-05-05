mod audio;
pub mod vad;
pub mod stt;

use crate::audio::AudioStream;
use crate::vad::VadEngine;
use crate::stt::{SttEngine, SttCommand};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, Emitter, State};
use ringbuf::traits::Split;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use std::path::PathBuf;
use tauri_plugin_positioner;

#[cfg(target_os = "linux")]
use gtk::prelude::*;

// ─── Managed State ───────────────────────────────────────────────────────────

/// Holds the active audio ingestion stream and the communication channel 
/// to the STT worker thread.
struct VoxEngine {
    _audio_stream: AudioStream,
    stt_tx: tokio::sync::mpsc::Sender<SttCommand>,
}

/// Thread-safe wrapper for the VoxEngine, allowed to be None if the engine 
/// hasn't been launched.
struct EngineState(Arc<Mutex<Option<VoxEngine>>>);

// ─── STT Worker (Dedicated OS Thread) ────────────────────────────────────────

fn spawn_stt_worker(app: tauri::AppHandle, mut rx: tokio::sync::mpsc::Receiver<SttCommand>, model_path: PathBuf) {
    std::thread::spawn(move || {
        log::info!("[STT] >>> Dedicated worker thread started.");
        
        let engine = match SttEngine::new(&model_path) {
            Ok(e) => e,
            Err(err) => {
                log::error!("[STT] CRITICAL: Failed to initialize Sherpa engine: {}", err);
                return;
            }
        };

        let mut last_emit_time = Instant::now();
        let mut last_transcript = String::new();

        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                SttCommand::Partial(sid, utterance) => {
                    // UX Throttling: Only run inference if 800ms passed to save CPU
                    if last_emit_time.elapsed() >= Duration::from_millis(800) {
                        match engine.transcribe(&utterance) {
                            Ok(text) => {
                                if !text.is_empty() && text != last_transcript {
                                    let _ = app.emit("transcript_partial", serde_json::json!({ 
                                        "text": text.clone(),
                                        "session_id": sid 
                                    }));
                                    last_transcript = text;
                                }
                                last_emit_time = Instant::now();
                            }
                            Err(e) => log::error!("[STT] Partial decode error: {}", e),
                        }
                    }
                }
                SttCommand::Final(sid, utterance) => {
                    log::info!("[STT] >>> Finalizing utterance (session: {})", sid);
                    match engine.transcribe(&utterance) {
                        Ok(text) => {
                            log::info!("[STT] Result ({}): {:?}", sid, text);
                            let _ = app.emit("transcript_final", serde_json::json!({ 
                                "text": text,
                                "session_id": sid
                            }));
                        }
                        Err(e) => log::error!("[STT] Final decode error: {}", e),
                    }
                    // Reset tracking state. The OfflineRecognizer stream is dropped internally
                    // in engine.transcribe(), clearing the KV cache for the next turn.
                    last_transcript.clear();
                    last_emit_time = Instant::now();
                }
            }
        }
        log::info!("[STT] Worker thread exiting.");
    });
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Hides the transcription tray window.
#[tauri::command]
fn hide_tray_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}

/// Sets whether the HUD window should ignore cursor events (click-through).
#[tauri::command]
fn set_hud_ignore_cursor(window: tauri::WebviewWindow, ignore: bool) {
    let _ = window.set_ignore_cursor_events(ignore);
}

/// Positions the tray window at the top-right of the screen.
/// 
/// On Linux, this triggers the "virtual layer" setup for click-through support.
async fn position_tray_window(window: &tauri::WebviewWindow) {
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
fn setup_linux_virtual_layer<R: tauri::Runtime>(app: &tauri::AppHandle<R>, label: &str) {
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
        
        if cur_size.width != size.width || cur_size.height != size.height {
            let _ = window.set_size(tauri::Size::Physical(*size));
            let _ = window.set_position(tauri::Position::Physical(*mon.position()));
            let _ = window.set_always_on_top(true);
        }

        if let Ok(gtk_window) = window.gtk_window() {
            let hud_w = 420; 
            let hud_h = 500;
            let padding_x = 30;
            
            let x = (size.width as i32) - hud_w - padding_x;
            let y = (size.height as f64 * 0.2) as i32 - 10; 

            let rect = cairo::RectangleInt::new(x, y, hud_w, hud_h);
            let region = cairo::Region::create_rectangle(&rect);
            gtk_window.input_shape_combine_region(Some(&region));
        }
    }
}

/// Checks if the audio/STT engine is currently running.
#[tauri::command]
async fn check_engine_status(state: State<'_, EngineState>) -> Result<bool, String> {
    let lock = state.0.lock().await;
    Ok(lock.is_some())
}

/// Launches the 3-tier audio processing engine.
/// 
/// 1. Initializes STT worker thread (Tier 3).
/// 2. Initializes VAD worker thread (Tier 2).
/// 3. Starts Audio Ingestion stream (Tier 1).
/// 
/// If the engine is already running, it simply ensures the HUD is positioned.
#[tauri::command]
async fn launch_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state: State<'_, EngineState> = app.state();
    let mut lock = state.0.lock().await;
    
    if lock.is_some() {
        if let Some(window) = app.get_webview_window("tray") {
            position_tray_window(&window).await;
            let _ = window.set_focus();
        }
        return Ok(());
    }

    log::info!("[PIPELINE] >>> Launching 3-Tier Audio Engine...");

    // ── 1. Paths ─────────────────────────────────────────────────────────────
    let resource_dir = app.path().resource_dir().unwrap_or_default();
    
    let stt_model_path = {
        let p = resource_dir.join("assets/qwen3-asr");
        if p.exists() { p } else { std::env::current_dir().unwrap().join("assets/qwen3-asr") }
    };
    
    let vad_model_path = {
        let p = resource_dir.join("assets/ten_vad.onnx");
        if p.exists() { p } else { std::env::current_dir().unwrap().join("assets/ten_vad.onnx") }
    };

    // ── 2. Channels ──────────────────────────────────────────────────────────
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    let (stt_tx, stt_rx) = tokio::sync::mpsc::channel::<SttCommand>(10);

    // ── 3. Tier 3: STT Worker (Dedicated OS Thread) ─────────────────────────
    spawn_stt_worker(app.clone(), stt_rx, stt_model_path);

    // ── 4. Tier 2: VAD & Router (Dedicated OS Thread) ───────────────────────
    let mut vad = VadEngine::new(&vad_model_path).map_err(|e| e.to_string())?;
    let (producer, consumer) = ringbuf::HeapRb::<f32>::new(16000 * 4).split(); // 4s buffer
    
    let stt_tx_for_vad = stt_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = vad.run_sync_loop(consumer, event_tx, stt_tx_for_vad) {
            log::error!("[VAD] CRITICAL: Worker thread crashed: {}", e);
        }
    });

    // ── 5. Event Forwarder (Tokio Bridge) ────────────────────────────────────
    let app_handle_emit = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                if msg_type == "speech_start" {
                    if let Some(window) = app_handle_emit.get_webview_window("tray") {
                        let w = window.clone();
                        tauri::async_runtime::spawn(async move {
                            position_tray_window(&w).await;
                        });
                    }
                }
                let _ = app_handle_emit.emit(msg_type, &event);
            }
        }
    });

    // ── 6. Tier 1: Audio Ingestion (Hardware Interrupt) ─────────────────────
    let audio_stream = AudioStream::new(producer).map_err(|e| e.to_string())?;
    audio_stream.start().map_err(|e| e.to_string())?;

    // ── 7. Store in state ────────────────────────────────────────────────────
    *lock = Some(VoxEngine {
        _audio_stream: audio_stream,
        stt_tx,
    });

    // ── 8. System Telemetry (CPU/RAM) ────────────────────────────────────────
    let app_handle_stats = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_cpu_time: u64 = 0;
        let mut last_check = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            let stats = std::fs::read_to_string("/proc/self/stat").ok();
            let cpu_usage = if let Some(s) = stats {
                let parts: Vec<&str> = s.split_whitespace().collect();
                if parts.len() > 14 {
                    let utime: u64 = parts[13].parse().unwrap_or(0);
                    let stime: u64 = parts[14].parse().unwrap_or(0);
                    let total = utime + stime;
                    let diff = if last_cpu_time > 0 { total - last_cpu_time } else { 0 };
                    last_cpu_time = total;
                    let elapsed = last_check.elapsed().as_secs_f32();
                    last_check = Instant::now();
                    (diff as f32 / 100.0) / elapsed * 100.0
                } else { 0.0 }
            } else { 0.0 };

            let status = std::fs::read_to_string("/proc/self/status").ok();
            let mem_rss_mb = if let Some(s) = status {
                s.lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|kb| kb / 1024)
                    .unwrap_or(0)
            } else { 0 };

            let _ = app_handle_stats.emit("system_stats", serde_json::json!({
                "cpu_usage": cpu_usage,
                "memory_used_mb": mem_rss_mb,
            }));
        }
    });

    Ok(())
}

// ─── App Entry Point ─────────────────────────────────────────────────────────

/// Main entry point for the Vox application.
/// 
/// Sets up tray icon, menu events, window management, and auto-launches the 
/// engine on startup.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine_state = EngineState(Arc::new(Mutex::new(None)));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .manage(engine_state)
        .setup(|app| {

            // ── 1. Tray Menu ─────────────────────────────────────────────────
            let launch_i = MenuItem::with_id(app, "launch", "Launch Vox", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&launch_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "launch" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Some(window) = handle.get_webview_window("tray") {
                                position_tray_window(&window).await;
                                let _ = window.set_focus();
                                let _ = window.set_always_on_top(true);
                            }
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                        let app = tray.app_handle().clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = launch_engine(app).await;
                        });
                    }
                })
                .build(app)?;

            // ── 2. Position tray HUD ─────────────────────────────────────────
            if let Some(tray_win) = app.get_webview_window("tray") {
                let tray_win_clone = tray_win.clone();
                
                tauri::async_runtime::spawn(async move {
                    // Give the window manager a moment to register the window
                    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                    
                    // Force frameless and always on top
                    let _ = tray_win_clone.set_decorations(false);
                    let _ = tray_win_clone.set_always_on_top(true);
                    let _ = tray_win_clone.set_shadow(false);
                    let _ = tray_win_clone.set_skip_taskbar(true);
                    let _ = tray_win_clone.set_resizable(false);

                    position_tray_window(&tray_win_clone).await;
                    
                    // Initial hide to ensure it's hidden on startup despite tauri.conf.json
                    let _ = tray_win_clone.hide();
                });
            }

            // ── 3. Auto-launch on startup ───────────────────────────────────
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = launch_engine(handle).await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "tray" {
                if let tauri::WindowEvent::Resized(size) = event {
                    if size.width > 0 && size.height > 0 {
                        #[cfg(target_os = "linux")]
                        setup_linux_virtual_layer(window.app_handle(), window.label());
                    }
                }
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Instead of closing, just hide the window (unless it's the tray window being properly closed)
                if window.label() == "main" || window.label() == "tray" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            hide_tray_window,
            launch_engine,
            check_engine_status,
            set_hud_ignore_cursor
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
