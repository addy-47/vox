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

struct VoxEngine {
    _audio_stream: AudioStream,
    _stt_tx: tokio::sync::mpsc::UnboundedSender<SttCommand>,
    last_active: Arc<Mutex<Instant>>,
}

struct EngineState(Arc<Mutex<Option<VoxEngine>>>);

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
fn hide_tray_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}

#[tauri::command]
async fn launch_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state: State<'_, EngineState> = app.state();
    let mut lock = state.0.lock().await;
    
    // Update last active time regardless of whether it's already running
    if let Some(engine) = lock.as_ref() {
        let mut last_active = engine.last_active.lock().await;
        *last_active = Instant::now();
        
        // If already running, just show and focus the tray window
        if let Some(window) = app.get_webview_window("tray") {
            let _ = window.show();
            let _ = window.set_focus();
        }
        log::info!("[ENGINE] App already launched. Bringing to focus.");
        return Ok(());
    }

    log::info!("[ENGINE] Launching speech engine (Cold Start)...");

    // ── 1. Channels ──────────────────────────────────────────────────────────
    let (vad_tx, mut vad_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    let (stt_tx, stt_rx) = tokio::sync::mpsc::unbounded_channel::<SttCommand>();

    // ── 2. STT worker (OS Thread) ───────────────────────────────────────────
    let stt_model_path = {
        let res_path = app.path().resource_dir().unwrap_or_default().join("assets/qwen3-asr");
        if res_path.exists() {
            res_path
        } else {
            let dev_path = std::env::current_dir().unwrap().join("assets/qwen3-asr");
            if dev_path.exists() {
                dev_path
            } else {
                res_path
            }
        }
    };
    
    log::info!("[ENGINE] Using STT model path: {:?}", stt_model_path);
    let app_handle_stt = app.clone();
    std::thread::spawn(move || {
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

        let mut rx = stt_rx;
        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                SttCommand::Transcribe(utterance) => {
                    log::info!("[STT] Transcribing {} samples", utterance.len());
                    let handle = app_handle_stt.clone();
                    match engine.transcribe(&utterance, |text| {
                        let _ = handle.emit("transcript_partial", serde_json::json!({ "text": text }));
                    }) {
                        Ok(text) => {
                            log::info!("[STT] Final: {:?}", text);
                            let _ = app_handle_stt.emit("transcript_final", serde_json::json!({ "text": text }));
                        }
                        Err(e) => log::error!("[STT] transcribe error: {}", e),
                    }
                }
            }
        }
        log::info!("[STT] Worker thread exiting.");
    });

    // ── 3. VAD Engine ────────────────────────────────────────────────────────
    let vad_model_path = {
        let res_path = app.path().resource_dir().unwrap_or_default().join("assets/ten_vad.onnx");
        if res_path.exists() {
            res_path
        } else {
            let dev_path = std::env::current_dir().unwrap().join("assets/ten_vad.onnx");
            if dev_path.exists() {
                dev_path
            } else {
                res_path
            }
        }
    };
    log::info!("[ENGINE] Using VAD model path: {:?}", vad_model_path);

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
        log::info!("[VAD] Loop exited.");
    });

    // ── 5. Event Forwarder ───────────────────────────────────────────────────
    let app_handle_emit = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = vad_rx.recv().await {
            if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                if msg_type == "speech_start" {
                    if let Some(window) = app_handle_emit.get_webview_window("tray") {
                        let _ = window.show();
                    }
                }
                let _ = app_handle_emit.emit(msg_type, &event);
            }
        }
        log::info!("[PIPELINE] Event forwarder exiting.");
    });

    // ── 6. Start Audio ───────────────────────────────────────────────────────
    audio_stream.start().map_err(|e| e.to_string())?;

    let last_active = Arc::new(Mutex::new(Instant::now()));
    
    // ── 7. Store in state ────────────────────────────────────────────────────
    *lock = Some(VoxEngine {
        _audio_stream: audio_stream,
        _stt_tx: stt_tx,
        last_active: last_active.clone(),
    });

    // Show and focus window on launch
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.show();
        let _ = window.set_focus();
    }

    // ── 8. Cold Start Timeout (5 minutes) ────────────────────────────────────
    let state_clone = state.0.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let mut lock = state_clone.lock().await;
            if let Some(engine) = lock.as_ref() {
                let last = *engine.last_active.lock().await;
                if last.elapsed() > Duration::from_secs(300) {
                    log::info!("[ENGINE] 5m inactivity timeout. Transitioning to Cold Start (releasing RAM)...");
                    *lock = None; // Drops VoxEngine, stopping audio and STT thread
                    break;
                }
            } else {
                break;
            }
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
                            let _ = launch_engine(handle).await;
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
                if let Ok(Some(monitor)) = tray_win.primary_monitor() {
                    let screen = monitor.size();
                    let win_size = tray_win.outer_size().unwrap_or(tauri::PhysicalSize::new(360, 500));
                    let scale = monitor.scale_factor();
                    let padding = (20.0 * scale) as i32;
                    let x = screen.width as i32 - win_size.width as i32 - padding;
                    let y = (screen.height as i32 - win_size.height as i32) / 2;
                    let _ = tray_win.set_position(tauri::PhysicalPosition::new(x, y));
                }
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
        .invoke_handler(tauri::generate_handler![hide_tray_window, launch_engine])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
