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

// ─── Managed State ───────────────────────────────────────────────────────────

// ─── Managed State ───────────────────────────────────────────────────────────
struct VoxEngine {
    _audio_stream: AudioStream,
    stt_tx: tokio::sync::mpsc::Sender<SttCommand>,
}

struct EngineState(Arc<Mutex<Option<VoxEngine>>>);

// ─── STT Master Worker (Lazy Loader) ─────────────────────────────────────────
async fn start_stt_master(app: tauri::AppHandle, mut rx: tokio::sync::mpsc::Receiver<SttCommand>) {
    let mut engine: Option<SttEngine> = None;
    let mut last_activity = Instant::now();
    
    // Resolve model path once
    let stt_model_path = {
        let res_path = app.path().resource_dir().unwrap_or_default().join("assets/qwen3-asr");
        if res_path.exists() { res_path }
        else {
            let dev_path = std::env::current_dir().unwrap().join("assets/qwen3-asr");
            if dev_path.exists() { dev_path } else { res_path }
        }
    };

    loop {
        tokio::select! {
            cmd_opt = rx.recv() => {
                match cmd_opt {
                    Some(cmd) => {
                        last_activity = Instant::now();
                        
                        // Lazy load engine if dropped or not yet started
                        if engine.is_none() {
                            log::info!("[STT] >>> Waking up STT Engine (Cold Start)...");
                            match SttEngine::new(&stt_model_path) {
                                Ok(e) => {
                                    engine = Some(e);
                                    let _ = app.emit("engine_status", serde_json::json!({ "status": "active" }));
                                },
                                Err(err) => {
                                    log::error!("[STT] Failed to initialize SttEngine: {}", err);
                                    continue;
                                }
                            }
                        }

                        let stt = engine.as_mut().unwrap();
                        match cmd {
                            SttCommand::Partial(sid, utterance) => {
                                let handle = app.clone();
                                let _ = stt.transcribe(&utterance, |text| {
                                    let _ = handle.emit("transcript_partial", serde_json::json!({ 
                                        "text": text,
                                        "session_id": sid 
                                    }));
                                });
                            }
                            SttCommand::Final(sid, utterance) => {
                                let handle = app.clone();
                                match stt.transcribe(&utterance, |_| {}) {
                                    Ok(text) => {
                                        log::info!("[STT] Final (session: {}): {:?}", sid, text);
                                        let _ = handle.emit("transcript_final", serde_json::json!({ 
                                            "text": text,
                                            "session_id": sid
                                        }));
                                    }
                                    Err(e) => log::error!("[STT] Final transcribe error (session: {}): {}", sid, e),
                                }
                            }
                        }
                    }
                    None => break, // Channel closed
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                // If idle for > 5 mins, drop the engine to release RAM
                if engine.is_some() && last_activity.elapsed() > Duration::from_secs(300) {
                    log::info!("[STT] <<< 5m inactivity. Releasing STT model from RAM.");
                    engine = None;
                    let _ = app.emit("engine_status", serde_json::json!({ "status": "cold" }));
                }
            }
        }
    }
    log::info!("[STT] Master worker exiting.");
}

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
fn hide_tray_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}

#[tauri::command]
async fn check_engine_status(state: State<'_, EngineState>) -> Result<bool, String> {
    let lock = state.0.lock().await;
    Ok(lock.is_some())
}

#[tauri::command]
async fn launch_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state: State<'_, EngineState> = app.state();
    let mut lock = state.0.lock().await;
    
    if lock.is_some() {
        if let Some(window) = app.get_webview_window("tray") {
            let _ = window.show();
            let _ = window.set_focus();
        }
        return Ok(());
    }

    log::info!("[ENGINE] Initializing Core (Audio + VAD)...");

    // ── 1. Channels ──────────────────────────────────────────────────────────
    let (vad_tx, mut vad_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    let (stt_tx, stt_rx) = tokio::sync::mpsc::channel::<SttCommand>(10);

    // ── 2. STT Master (Lazy Loader) ─────────────────────────────────────────
    let app_handle_stt = app.clone();
    tauri::async_runtime::spawn(async move {
        start_stt_master(app_handle_stt, stt_rx).await;
    });

    // ── 3. VAD Engine ────────────────────────────────────────────────────────
    let vad_model_path = {
        let res_path = app.path().resource_dir().unwrap_or_default().join("assets/ten_vad.onnx");
        if res_path.exists() { res_path }
        else {
            let dev_path = std::env::current_dir().unwrap().join("assets/ten_vad.onnx");
            if dev_path.exists() { dev_path } else { res_path }
        }
    };
    
    let mut vad = VadEngine::new(vad_model_path.to_str().expect("invalid model path"))
        .map_err(|e| e.to_string())?;

    let (producer, consumer) = ringbuf::HeapRb::<f32>::new(16000 * 2).split();
    let audio_stream = AudioStream::new(producer).map_err(|e| e.to_string())?;

    // ── 4. VAD loop ──────────────────────────────────────────────────────────
    let stt_tx_clone = stt_tx.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = vad.run_loop(consumer, vad_tx, stt_tx_clone).await {
            log::error!("[VAD] engine error: {}", e);
        }
    });

    // ── 5. Event Forwarder ───────────────────────────────────────────────────
    let app_handle_emit = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = vad_rx.recv().await {
            if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                if msg_type == "speech_start" {
                    if let Some(window) = app_handle_emit.get_webview_window("tray") {
                        log::info!("[HUD] Backend showing tray window on speech_start");
                        let _ = window.show();
                        let _ = window.set_focus();
                    } else {
                        log::error!("[HUD] Backend could not find 'tray' window!");
                    }
                }
                let _ = app_handle_emit.emit(msg_type, &event);
            }
        }
        log::info!("[PIPELINE] Event forwarder exiting.");
    });

    // ── 6. Start Audio ───────────────────────────────────────────────────────
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine_state = EngineState(Arc::new(Mutex::new(None)));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
                                let _ = window.show();
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

                    let monitor = tray_win_clone.primary_monitor().ok().flatten()
                        .or_else(|| tray_win_clone.current_monitor().ok().flatten());

                    if let Some(mon) = monitor {
                        let scale = mon.scale_factor();
                        let screen = mon.size().to_logical::<f64>(scale);
                        let mon_pos = mon.position().to_logical::<f64>(scale);
                        
                        let width = (screen.width * 0.30).max(380.0).min(500.0);
                        let height = (screen.height * 0.25).max(220.0).min(450.0);
                        let _ = tray_win_clone.set_size(tauri::LogicalSize::new(width, height)).ok();
                        
                        let padding = 24.0;
                        let x = mon_pos.x + screen.width - width - padding;
                        let y = mon_pos.y + (screen.height - height) / 2.0;
                        
                        log::info!("[HUD] Monitor Detect: pos={:?},{:?} size={:?}x{:?} scale={:?}", mon_pos.x, mon_pos.y, screen.width, screen.height, scale);
                        log::info!("[HUD] Target Position: Logical({:?}, {:?})", x, y);
                        
                        if let Err(e) = tray_win_clone.set_position(tauri::LogicalPosition::new(x, y)) {
                            log::error!("[HUD] Failed to set window position: {:?}", e);
                        }
                        
                        // Explicitly show after positioning
                        let _ = tray_win_clone.hide(); // Hide first to reset
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        // But wait, the user says it's already showing. 
                        // Let's just make sure it's at the right spot.
                    } else {
                        log::warn!("[HUD] NO MONITOR DETECTED. Window may be misplaced.");
                    }
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
            check_engine_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
