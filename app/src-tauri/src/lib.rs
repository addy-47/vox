pub mod core;
pub mod services;
pub mod ui;

use crate::services::audio::AudioStream;
use crate::services::vad::VadEngine;
use crate::services::stt::{SttEngine, SttCommand};
use crate::ui::tray::position_tray_window;
use crate::core::state::{AppState, VoxEngine, InteractionOwner};
use crate::core::settings::VoxSettings;
use crate::core::events::VoxEvent;
use crate::services::pipeline::PipelineOrchestrator;
use crate::services::playback::PlaybackEngine;

use tauri::menu::Menu;
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, Emitter, State, WebviewWindow};
use ringbuf::traits::Split;
use std::time::{Duration, Instant};
use std::path::PathBuf;
use std::sync::atomic::Ordering;


// ─── STT Worker (Dedicated OS Thread) ────────────────────────────────────────

fn spawn_stt_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<SttCommand>,
    model_path: PathBuf,
    // Optional pipeline event channel — `Some` when Phase 4 pipeline is active.
    pipeline_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
) {
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

        while let Ok(cmd) = rx.recv() {
            match cmd {
                SttCommand::Partial(sid, owner, utterance) => {
                    // UX Throttling: Only run inference if 800ms passed to save CPU
                    if last_emit_time.elapsed() >= Duration::from_millis(800) {
                        match engine.transcribe(&utterance) {
                            Ok(text) => {
                                let text_str: String = text;
                                if !text_str.is_empty() && text_str != last_transcript {
                                    // Phase 5: Forward to pipeline orchestrator for validated emission
                                    if let Some(ref pipeline_tx) = pipeline_event_tx {
                                        let _ = pipeline_tx.send(VoxEvent::TranscriptPartial {
                                            session_id: sid,
                                            owner,
                                            text: text_str.clone(),
                                        });
                                    }
                                    
                                    last_transcript = text_str;
                                }
                                last_emit_time = Instant::now();
                            }
                            Err(e) => log::error!("[STT] Partial transcription failed: {}", e),
                        }
                    }
                }
                SttCommand::Final(sid, owner, utterance) => {
                    match engine.transcribe(&utterance) {
                        Ok(text) => {
                            let text_str: String = text;
                            if !text_str.is_empty() {
                                // Phase 4 pipeline Hand-off
                                if let Some(ref pipeline_tx) = pipeline_event_tx {
                                    let _ = pipeline_tx.send(VoxEvent::TranscriptFinal {
                                        session_id: sid,
                                        owner,
                                        text: text_str,
                                    });
                                }
                            }
                        }
                        Err(e) => log::error!("[STT] Final transcription failed: {}", e),
                    }
                    
                    // Signal UI that processing is complete for PTT
                    let target = match owner {
                        crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                    };
                    let _ = app.emit_to(target, "ptt_status", serde_json::json!({ "state": "IDLE" }));
                    
                    last_transcript.clear();
                    last_emit_time = Instant::now();
                }
                SttCommand::ResetStream => {
                    // Currently transcribe() creates a new stream every time, so this is a no-op 
                    // for the functional logic but we acknowledge it.
                    log::info!("[STT] ResetStream received.");
                }
            }
        }
        log::info!("[STT] Worker thread exiting.");
    });
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Checks if the audio/STT engine is currently running.
#[tauri::command]
async fn check_engine_status(state: State<'_, AppState>) -> Result<bool, String> {
    let lock = state.engine.lock().await;
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
async fn get_settings(state: State<'_, AppState>) -> Result<VoxSettings, String> {
    Ok(state.settings.lock().await.clone())
}

#[tauri::command]
async fn update_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    let changed = {
        let mut settings = state.settings.lock().await;
        if settings.theme != theme {
            settings.theme = theme.clone();
            let _ = settings.save(&state.config_dir);
            true
        } else {
            false
        }
    };
    
    if changed {
        // Notify all windows
        let _ = app.emit("theme-changed", theme);
    }
    Ok(())
}

#[tauri::command]
async fn engage(state: State<'_, AppState>) -> Result<(), String> {
    let current = state.pipeline.is_engaged.load(Ordering::Relaxed);
    let new_state = !current;
    
    if new_state {
        log::info!("[Pipeline] Engaging main application pipeline...");
        state.pipeline.is_engaged.store(true, Ordering::Relaxed);
        let mut owner = state.owner.lock().await;
        *owner = InteractionOwner::MainWindow;
    } else {
        log::info!("[Pipeline] Disengaging pipeline (Stopping session)...");
        state.pipeline.is_engaged.store(false, Ordering::Relaxed);
        
        // Cancel any ongoing generation/playback
        state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
        
        // Revert ownership to Tray (passive mode)
        let mut owner = state.owner.lock().await;
        *owner = InteractionOwner::Tray;
        
        // Reset STT state to clear any partial transcripts
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine.stt_tx.send(crate::services::stt::SttCommand::ResetStream);
        }
    }
    
    Ok(())
}

#[tauri::command]
async fn launch_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    let mut lock = state.engine.lock().await;
    
    if lock.is_some() {
        if let Some(window) = app.get_webview_window("tray") {
            position_tray_window(&window).await;
            let _ = window.set_focus();
        }
        return Ok(());
    }

    log::info!("[PIPELINE] >>> Launching 3-Tier Audio Engine...");

    // ── 1. Paths ─────────────────────────────────────────────────────────────
    let (stt_model_path, vad_model_path) = {
        let settings = state.settings.lock().await;
        let resource_dir = app.path().resource_dir().unwrap_or_default();
        
        let stt = if settings.stt_model_dir.is_absolute() {
            settings.stt_model_dir.clone()
        } else {
            let p = resource_dir.join(&settings.stt_model_dir);
            if p.exists() { p } else { std::env::current_dir().unwrap().join(&settings.stt_model_dir) }
        };

        let vad = if settings.vad_model_path.is_absolute() {
            settings.vad_model_path.clone()
        } else {
            let p = resource_dir.join(&settings.vad_model_path);
            if p.exists() { p } else { std::env::current_dir().unwrap().join(&settings.vad_model_path) }
        };

        (stt, vad)
    };

    // ── 2. Channels ──────────────────────────────────────────────────────────
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    
    // Internal STT command channel remains tokio for now or we can switch to std
    let (stt_tx_internal, stt_rx_internal) = std::sync::mpsc::channel::<SttCommand>();

    // ── 3. Tier 3: STT Worker (Dedicated OS Thread) ─────────────────────────
    // Create the internal pipeline event channel (VoxEvent bus) - Phase 5: must be std::sync::mpsc
    let (vox_event_tx, vox_event_rx) = std::sync::mpsc::channel::<VoxEvent>();
    spawn_stt_worker(app.clone(), stt_rx_internal, stt_model_path, Some(vox_event_tx.clone()));

    // ── 4. Tier 2: VAD & Router (Dedicated OS Thread) ───────────────────────
    let mut vad = VadEngine::new(&vad_model_path).map_err(|e| e.to_string())?;
    let (producer, consumer) = ringbuf::HeapRb::<f32>::new(16000 * 4).split(); // 4s buffer
    
    // ── Phase 5: High-Frequency Telemetry Aggregator ────────────────────────
    let (telemetry_tx, telemetry_rx) = std::sync::mpsc::channel::<crate::core::state::TelemetryData>();
    let app_handle_telemetry = app.clone();
    std::thread::spawn(move || {
        // Throttled aggregator (15-20Hz)
        let interval = Duration::from_millis(60); // ~16.6Hz
        loop {
            let mut latest = None;
            // Drain the channel to get the latest value
            while let Ok(data) = telemetry_rx.try_recv() {
                latest = Some(data);
            }
            
            if let Some(data) = latest {
                let target = {
                    let state: tauri::State<'_, crate::core::state::AppState> = app_handle_telemetry.state();
                    let owner = state.owner.blocking_lock();
                    match *owner {
                        crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                    }
                };
                let _ = app_handle_telemetry.emit_to(target, "telemetry", data);
            }
            std::thread::sleep(interval);
        }
    });

    let stt_tx_for_vad = stt_tx_internal.clone();
    let app_handle_vad = app.clone();
    let telemetry_tx_for_vad = telemetry_tx.clone();
    let vox_event_tx_for_vad = vox_event_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = vad.run_sync_loop(app_handle_vad, consumer, event_tx, stt_tx_for_vad, telemetry_tx_for_vad, Some(vox_event_tx_for_vad)) {
            log::error!("[VAD] CRITICAL: Worker thread crashed: {}", e);
        }
    });

    // ── 5. Event Forwarder (Tokio Bridge) ────────────────────────────────────
    let app_handle_emit = app.clone();
    tauri::async_runtime::spawn(async move {
        let app_state: State<'_, AppState> = app_handle_emit.state();
        while let Some(event) = event_rx.recv().await {
            if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                if msg_type == "speech_start" {
                    let hud_visible = {
                        let hud_lock = app_state.hud_visible.lock().await;
                        *hud_lock
                    };

                    if hud_visible {
                        if let Some(window) = app_handle_emit.get_webview_window("tray") {
                            let w = window.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = w.show();
                                position_tray_window(&w).await;
                            });
                        }
                    }
                }
                
                let target = {
                    let owner = app_state.owner.blocking_lock();
                    match *owner {
                        crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                    }
                };
                let _ = app_handle_emit.emit_to(target, msg_type, &event);
            }
        }
    });


    // ── 6. Tier 1: Audio Ingestion (Hardware Interrupt) ─────────────────────
    let audio_stream = AudioStream::new(producer).map_err(|e| e.to_string())?;
    audio_stream.start().map_err(|e| e.to_string())?;

    // ── 7. Phase 4: Pipeline Orchestrator + Playback Engine ─────────────────
    let (en_tts_dir, hi_tts_dir, _audio_output_mode) = {
        let settings = state.settings.lock().await;
        let resource_dir = app.path().resource_dir().unwrap_or_default();
        
        let en_tts = if settings.tts_model_dir.is_absolute() {
            settings.tts_model_dir.clone()
        } else {
            let p = resource_dir.join(&settings.tts_model_dir);
            if p.exists() { p } else { std::env::current_dir().unwrap().join(&settings.tts_model_dir) }
        };

        let hi_tts = if settings.tts_hindi_model_dir.is_absolute() {
            settings.tts_hindi_model_dir.clone()
        } else {
            let p = resource_dir.join(&settings.tts_hindi_model_dir);
            if p.exists() { p } else { std::env::current_dir().unwrap().join(&settings.tts_hindi_model_dir) }
        };

        (en_tts, hi_tts, settings.audio_output_mode.clone())
    };

    let pipeline_atomics = {
        let s = state.inner();
        (
            std::sync::Arc::clone(&s.pipeline.cancel_flag),
            std::sync::Arc::clone(&s.pipeline.playback_active),
            std::sync::Arc::clone(&s.pipeline.llm_generating),
            std::sync::Arc::clone(&s.pipeline.tts_generating),
            std::sync::Arc::clone(&s.pipeline.session_id),
            std::sync::Arc::clone(&s.pipeline.state),
            std::sync::Arc::clone(&s.pipeline.is_engaged),
        )
    };
    let (cancel_flag, playback_active, llm_gen, tts_gen, session_id, state_atomic, is_engaged) = pipeline_atomics;

    let playback_engine = match PlaybackEngine::new(
        std::sync::Arc::clone(&playback_active),
        std::sync::Arc::clone(&cancel_flag),
        telemetry_tx.clone(),
    ) {
        Ok(pe) => std::sync::Arc::new(pe),
        Err(e) => {
            log::error!("[Pipeline] PlaybackEngine init failed: {} — TTS output disabled", e);
            return Ok(()); // Non-fatal: STT still works
        }
    };

    // Pipeline orchestrator — spawns on its own coordinator thread
    let vox_settings = state.settings.lock().await.clone();
    let orchestrator = PipelineOrchestrator::new(
        std::sync::Arc::clone(&cancel_flag),
        std::sync::Arc::clone(&playback_active),
        llm_gen,
        tts_gen,
        session_id,
        state_atomic,
        vox_event_tx.clone(),
        vox_settings,
        is_engaged,
    );

    let playback_for_orch = std::sync::Arc::clone(&playback_engine);
    let app_for_orch = app.clone();
    std::thread::Builder::new()
        .name("vox-pipeline".to_string())
        .spawn(move || {
            orchestrator.run_event_loop(
                vox_event_rx,
                en_tts_dir,
                hi_tts_dir,
                playback_for_orch,
                app_for_orch,
            );
        })
        .map_err(|e| e.to_string())?;

    log::info!("[Pipeline] Phase 4 pipeline online (LLM + TTS + Playback)");

    // ── 8. Store in state ────────────────────────────────────────────────────
    *lock = Some(VoxEngine {
        audio_stream,
        stt_tx: stt_tx_internal,
        telemetry_tx,
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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .setup(|app| {
            // ── 0. App State ────────────────────────────────────────────────────────
            let app_state = AppState::new(app.handle());
            app.manage(app_state);

            // ── 1. System Tray ───────────────────────────────────────────────────────
            let tray_menu = Menu::new(app)?;
            let launch_i = tauri::menu::MenuItemBuilder::new("Launch Vox").id("launch").build(app)?;
            let live_i = tauri::menu::CheckMenuItemBuilder::new("Vox Live").id("live").build(app)?;
            let quit_i = tauri::menu::MenuItemBuilder::new("Quit").id("quit").build(app)?;

            tray_menu.append(&launch_i)?;
            tray_menu.append(&live_i)?;
            tray_menu.append(&tauri::menu::PredefinedMenuItem::separator(app)?)?;
            tray_menu.append(&quit_i)?;

            // Store live_i handle in state for synchronization
            {
                let state: State<'_, AppState> = app.state();
                let mut menu_item_lock = tauri::async_runtime::block_on(state.hud_menu_item.lock());
                *menu_item_lock = Some(live_i.clone());
                // Reflect the default hud_visible=true in the menu UI
                let _ = live_i.set_checked(true);
            }

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .on_menu_event(move |app: &tauri::AppHandle, event: tauri::menu::MenuEvent| match event.id.as_ref() {
                    "launch" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let window: WebviewWindow = window;
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "live" => {
                        let handle = app.clone();
                        let live_item = live_i.clone();
                        tauri::async_runtime::spawn(async move {
                            let app_state: State<'_, AppState> = handle.state();
                            let mut hud_lock = app_state.hud_visible.lock().await;
                            let new_state = !*hud_lock;
                            *hud_lock = new_state;

                            if let Some(window) = handle.get_webview_window("tray") {
                                let window: WebviewWindow = window;
                                if new_state {
                                    position_tray_window(&window).await;
                                } else {
                                    let _ = window.hide();
                                }
                                let _ = handle.emit("toggle_hud", ());
                            }
                            let _ = live_item.set_checked(new_state);
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event| {
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
                        crate::ui::tray::setup_linux_virtual_layer(window.app_handle(), window.label());
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
            crate::ui::tray::hide_tray_window,
            launch_engine,
            engage,
            check_engine_status,
            get_settings,
            update_theme,
            crate::ui::tray::set_hud_ignore_cursor,
            crate::ui::tray::sync_hud_visibility,
            crate::ui::tray::update_interaction_mode,
            crate::ui::ptt::ptt_start,
            crate::ui::ptt::ptt_stop,
            crate::ui::ptt::ptt_cancel
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
