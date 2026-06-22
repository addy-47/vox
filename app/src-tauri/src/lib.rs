pub mod core;
pub mod ipc;
pub mod monitoring;
pub mod persistence;
pub mod services;
pub mod setup;
pub mod tray;
pub mod utils;
pub mod wizard;

use crate::core::state::AppState;
use crate::ipc::history::{
    commit_session_to_history, delete_session, get_sessions, get_transcript_history, get_turns,
};
use crate::ipc::pipeline::{
    check_engine_status, engage, get_realtime_session_cache, launch_engine, start_realtime_session, stop_engine,
    stop_realtime_session, test_clip, test_clip_cancel, pause_pipeline, resume_pipeline,
};
use crate::ipc::settings::{
    check_llm_provider_health, check_stt_provider_health, check_tts_provider_health, get_settings,
    list_remote_llm_models, request_boot_state, request_model_catalog, reset_settings,
    update_setting, update_theme,
};
use crate::ipc::tray::{
    hide_tray_window, set_hud_ignore_cursor, show_main_window, sync_hud_visibility,
    toggle_hud_visibility, update_interaction_mode,
};
use crate::services::ptt::{ptt_cancel, ptt_start, ptt_stop};
#[cfg(target_os = "linux")]
use crate::tray::setup_linux_virtual_layer;
use crate::tray::{position_tray_window, setup_tray_window};

use crate::monitoring::system_monitor::spawn_system_monitor;

use tauri::menu::Menu;
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State};

// ─── App Entry Point ─────────────────────────────────────────────────────────

/// Main entry point for the Vox application.
///
/// Sets up tray icon, menu events, window management, and auto-launches the
/// engine on startup.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Suppress ALSA/Jack noisy logs on Linux
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("ALSA_LOG_LEVEL", "0");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_dialog::init())
        // .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // ── 0. Runtime Booting ──────────────────────────────────────────────────
            app.emit(crate::core::constants::EVENT_RUNTIME_BOOTING, ()).ok();

            // ── 0. Paths Singleton (must be first) ──────────────────────────────────
            crate::utils::paths::init(app.handle());
            crate::utils::paths::ensure_dirs().ok();

            // ── Background Manifest Caching (fetches once at boot) ──────────────────
            tauri::async_runtime::spawn(async {
                let cache_dir = crate::utils::paths::cache_dir();
                
                // Fetch and cache App Manifest
                match crate::setup::manifest::AppManifest::fetch().await {
                    Ok(manifest) => {
                        let path = cache_dir.join("app_manifest.json");
                        if let Ok(content) = serde_json::to_string_pretty(&manifest) {
                            let _ = std::fs::write(path, content);
                            log::info!("[BOOTSTRAP] Successfully cached app manifest.");
                        }
                    }
                    Err(e) => log::warn!("[BOOTSTRAP] Failed to fetch/cache app manifest at boot: {}", e),
                }

                // Fetch and cache Models Manifest
                match crate::setup::manifest::VoxManifest::fetch().await {
                    Ok(manifest) => {
                        let path = cache_dir.join("models_manifest.json");
                        if let Ok(content) = serde_json::to_string_pretty(&manifest) {
                            let _ = std::fs::write(path, content);
                            log::info!("[BOOTSTRAP] Successfully cached models manifest.");
                        }
                    }
                    Err(e) => log::warn!("[BOOTSTRAP] Failed to fetch/cache models manifest at boot: {}", e),
                }
            });

            // ── 0.5 Logging (must be second, relies on paths) ───────────────────────
            let log_guard = crate::utils::logging::init(crate::utils::paths::get().logs.clone());

            // ── 0.55 Transliteration Engine (eager initialization) ──────────────────
            if let Err(e) = crate::services::translit::init_transliteration_engine() {
                log::warn!("[BOOTSTRAP] Eager transliteration engine initialization skipped: {}", e);
            }

            // ── 0.6 Telemetry Aggregator ───────────────────────────────────────────
            let latest_energy = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_vad_prob = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_low = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_mid = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_high = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_playback_energy = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_playback_low = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_playback_mid = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_playback_high = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_sys_cpu = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_sys_ram = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_vox_cpu = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_vox_ram = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let latest_stt_ms = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let latest_ttft_ms = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let latest_voice_latency_ms = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let latest_threads = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let latest_tts_rtf = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let latest_playback_start_ms = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let latest_persistence_rate = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0f32.to_bits()));
            let is_db_healthy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let is_private_mode = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dropped_telemetry_events = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

            let (telemetry_worker, telemetry_tx) = crate::monitoring::aggregator::TelemetryAggregator::new(
                std::sync::Arc::clone(&latest_energy),
                std::sync::Arc::clone(&latest_vad_prob),
                std::sync::Arc::clone(&latest_low),
                std::sync::Arc::clone(&latest_mid),
                std::sync::Arc::clone(&latest_high),
                std::sync::Arc::clone(&latest_sys_cpu),
                std::sync::Arc::clone(&latest_sys_ram),
                std::sync::Arc::clone(&latest_vox_cpu),
                std::sync::Arc::clone(&latest_vox_ram),
                std::sync::Arc::clone(&latest_stt_ms),
                std::sync::Arc::clone(&latest_ttft_ms),
                std::sync::Arc::clone(&dropped_telemetry_events),
            );
            telemetry_worker.start();

            // ── 0.7 Persistence Worker ─────────────────────────────────────────────
            let persist_tx = crate::persistence::worker::spawn_persistence_worker(
                crate::utils::paths::get().db.clone(),
                std::sync::Arc::clone(&is_db_healthy),
                std::sync::Arc::clone(&latest_persistence_rate),
                std::sync::Arc::clone(&is_private_mode),
            );

            // ── 1. App State ────────────────────────────────────────────────────────
            let mut app_state = AppState::new(
                app.handle(),
                Some(log_guard),
                telemetry_tx,
                latest_energy,
                latest_vad_prob,
                latest_low,
                latest_mid,
                latest_high,
                latest_playback_energy,
                latest_playback_low,
                latest_playback_mid,
                latest_playback_high,
                latest_sys_cpu,
                latest_sys_ram,
                latest_vox_cpu,
                latest_vox_ram,
                latest_stt_ms,
                latest_ttft_ms,
                latest_voice_latency_ms,
                latest_threads,
                latest_tts_rtf,
                latest_playback_start_ms,
                latest_persistence_rate,
                is_db_healthy,
                is_private_mode,
                dropped_telemetry_events,
            );
            app_state.persist_tx = std::sync::Mutex::new(Some(persist_tx));

            // ── 1.5 Monitoring Collector ──────────────────────────────────────────
            let state_arc = std::sync::Arc::new(app_state);
            app.manage(state_arc.clone());
            
            crate::monitoring::collector::spawn_monitoring_collector(std::sync::Arc::clone(&state_arc));
            spawn_system_monitor(app.handle().clone());
            crate::monitoring::telemetry_emitter::spawn_telemetry_emitter(app.handle().clone());

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
                let state: State<'_, std::sync::Arc<AppState>> = app.state();
                let mut menu_item_lock = tauri::async_runtime::block_on(state.hud_menu_item.lock());
                *menu_item_lock = Some(live_i.clone());
                
                let tray_enabled = {
                    let s = state.settings.read().unwrap();
                    s.ui.tray_enabled
                };
                let hud_visible = {
                    let v = tauri::async_runtime::block_on(state.hud_visible.lock());
                    *v
                };

                // Reflect tray_enabled setting in menu UI
                let _ = live_i.set_enabled(tray_enabled);
                let _ = live_i.set_checked(hud_visible);
            }

            let mut tray_builder = TrayIconBuilder::new().menu(&tray_menu);
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            } else {
                log::warn!("[Tray] Default window icon not found. Building tray without explicit icon.");
            }

            let _tray = tray_builder
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "launch" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = show_main_window(handle).await;
                        });
                    }
                    "live" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            toggle_hud_visibility(handle).await;
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                        let app = tray.app_handle().clone();
                        let tray_enabled = {
                            let state: State<'_, std::sync::Arc<AppState>> = app.state();
                            let s = state.settings.read().unwrap();
                            s.ui.tray_enabled
                        };
                        if tray_enabled {
                            tauri::async_runtime::spawn(async move {
                                let _ = launch_engine(app).await;
                            });
                        }
                    }
                })
                .build(app)?;

            
            // ── 1.7.5 CPU Governor Check (Linux only — warns if not "performance") ──
            {
                let state: tauri::State<'_, std::sync::Arc<AppState>> = app.state();
                if let Some(governor) = crate::utils::check_cpu_governor() {
                    let is_optimal = governor == "performance";
                    // Store in AppState so frontend can read from snapshot (avoids race with listener setup)
                    *state.cpu_governor.lock().unwrap() = governor.clone();
                    state.cpu_governor_optimal.store(is_optimal, std::sync::atomic::Ordering::Relaxed);

                    if !is_optimal {
                        log::warn!(
                            "[BOOTSTRAP] CPU governor is '{}', not 'performance'. \
                             This may degrade voice pipeline performance significantly. \
                             Consider: echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
                            governor
                        );
                        let _ = app.emit("cpu_governor_warning", serde_json::json!({
                            "governor": governor,
                            "optimal": is_optimal,
                            "advice": "Switch to 'performance' governor for best voice pipeline performance"
                        }));
                    }
                }
            }

            // ── 1.8 Runtime Ready ───────────────────────────────────────────────────
            {
                use crate::core::state::RuntimeStatus;
                use std::sync::atomic::Ordering;
                let state: State<'_, std::sync::Arc<AppState>> = app.state();
                state.runtime_status.store(RuntimeStatus::Ready as u32, Ordering::Relaxed);
                app.emit(crate::core::constants::EVENT_RUNTIME_READY, ()).ok();
                log::info!("[BOOTSTRAP] Runtime Ready. Tray visible.");
            }

            // ── 2. Position tray HUD ─────────────────────────────────────────
            if let Some(tray_win) = app.get_webview_window("tray") {
                let (tray_enabled, setup_completed) = {
                    let state: State<'_, std::sync::Arc<AppState>> = app.state();
                    let s = state.settings.read().unwrap();
                    (s.ui.tray_enabled, s.setup.completed)
                };

                if setup_completed && tray_enabled {
                    let tray_win_clone = tray_win.clone();
                    tauri::async_runtime::spawn(async move {
                        // Give the window manager a moment to register the window
                        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                        setup_tray_window(&tray_win_clone);
                        position_tray_window(&tray_win_clone).await;
                        let _ = tray_win_clone.hide();
                    });
                } else if !setup_completed {
                    log::info!("[BOOTSTRAP] Onboarding setup not completed. Keeping tray window hidden.");
                } else {
                    log::info!("[BOOTSTRAP] Tray HUD disabled. Closing tray window to save RAM.");
                    let _ = tray_win.close();
                }
            }

            // ── 3. Conditional auto-launch engine on startup ─────────────────────────────
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let (tray_enabled, setup_completed) = {
                    let state: tauri::State<'_, std::sync::Arc<AppState>> = handle.state();
                    
                    // ── 3.1 Auto-detect existing models ────────────────────────────
                    let mut settings = state.settings.write().unwrap();
                    if !settings.setup.completed && wizard::check_setup_health() {
                        log::info!("[BOOTSTRAP] Existing models detected. Auto-completing setup.");
                        settings.setup.completed = true;
                        let _ = settings.save();
                    }
                    
                    (settings.ui.tray_enabled, settings.setup.completed)
                };

                if setup_completed && tray_enabled {
                    log::info!("[BOOTSTRAP] Setup completed. Launching engine...");
                    if let Err(e) = launch_engine(handle).await {
                        log::error!("[BOOTSTRAP] Engine auto-launch failed: {}", e);
                    }
                } else if !setup_completed {
                    log::info!("[BOOTSTRAP] Setup not completed. Launching onboarding wizard...");
                    if let Some(wizard_win) = handle.get_webview_window("wizard") {
                        crate::wizard::setup_wizard_window(&wizard_win);
                    }
                } else {
                    log::info!("[BOOTSTRAP] Tray disabled. Skipping engine auto-launch to save resources.");
                }
            });

            // ── 4. CLI Arguments Handling ────────────────────────
            let _args: Vec<String> = std::env::args().collect();
            // (Future CLI flags can be added here)

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
                // Instead of closing, just hide the window
                if window.label() == "main" || window.label() == "tray" {
                    let label = window.label().to_string();
                    log::info!("[Window] Close requested for {}, hiding instead of closing window.", label);
                    let _ = window.hide();
                    api.prevent_close();
                    
                    // Evaluate engine offload if the main window is hidden
                    if label == "main" {
                        let handle = window.app_handle().clone();
                        tauri::async_runtime::spawn(async move {
                            let state: tauri::State<'_, std::sync::Arc<AppState>> = handle.state();
                            let (tray_enabled, is_engaged) = {
                                let s = state.settings.read().unwrap();
                                (s.ui.tray_enabled, state.pipeline.is_engaged.load(std::sync::atomic::Ordering::Relaxed))
                            };
                            
                            if !tray_enabled && !is_engaged {
                                log::info!("[Window] Main window hidden, Tray is disabled, and app is disengaged. Offloading engine...");
                                let _ = crate::ipc::pipeline::stop_engine(handle).await;
                            } else {
                                log::info!("[Window] Main window hidden. Engine kept alive. Tray enabled: {}, Engaged: {}", tray_enabled, is_engaged);
                            }
                        });
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            check_engine_status,
            launch_engine,
            stop_engine,
            engage,
            test_clip,
            test_clip_cancel,
            start_realtime_session,
            stop_realtime_session,
            get_realtime_session_cache,
            pause_pipeline,
            resume_pipeline,
            hide_tray_window,
            sync_hud_visibility,
            set_hud_ignore_cursor,
            update_interaction_mode,
            show_main_window,
            request_boot_state,
            request_model_catalog,
            check_llm_provider_health,
            check_stt_provider_health,
            check_tts_provider_health,
            list_remote_llm_models,
            get_settings,
            update_theme,
            update_setting,
            reset_settings,
            ptt_start,
            ptt_stop,
            ptt_cancel,
            get_transcript_history,
            commit_session_to_history,
            get_sessions,
            get_turns,
            delete_session,
            // Monitoring
            crate::ipc::monitoring::get_runtime_snapshot,
            crate::ipc::monitoring::get_runtime_history,
            crate::ipc::monitoring::clear_runtime_history,
            // Setup
            crate::ipc::setup::fetch_manifest,
            crate::ipc::setup::check_for_updates,
            crate::ipc::setup::check_for_model_updates,
            crate::ipc::setup::get_onboarding_status,
            crate::ipc::setup::get_runtime_report,
            crate::ipc::setup::start_model_setup,
            crate::ipc::setup::cancel_model_setup,
            crate::ipc::setup::complete_setup_wizard,
            crate::ipc::setup::reveal_wizard,
            crate::ipc::setup::check_model_exists,
            crate::ipc::setup::download_optional_model,
            crate::ipc::setup::delete_model,
            // Audio
            crate::ipc::audio::list_input_devices,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                log::info!("[Vox] Shutting down engine...");
                let state: State<'_, std::sync::Arc<AppState>> = app_handle.state();
                
                // Clear engine (this will drop VoxEngine and close channels)
                let mut engine_lock = state.engine.blocking_lock();
                if let Some(engine) = engine_lock.take() {
                    let _ = engine.pipeline_tx.send(crate::core::events::VoxEvent::Shutdown);
                    let _ = engine.stt_tx.send(crate::services::stt::SttCommand::Shutdown);
                    let _ = engine.vad_tx.send(crate::core::state::VadCommand::Shutdown);
                }
                
                // Allow time for threads to join
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
}
