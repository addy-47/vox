pub mod core;
pub mod services;
pub mod tray;
pub mod ipc;
pub mod telemetry;
pub mod utils;
pub mod persistence;

use crate::core::state::AppState;
use crate::ipc::pipeline::{check_engine_status, launch_engine, engage};
use crate::ipc::tray::{
    hide_tray_window, sync_hud_visibility, set_hud_ignore_cursor, 
    update_interaction_mode, show_main_window, toggle_hud_visibility
};
use crate::ipc::history::{get_transcript_history, get_sessions, get_turns, delete_session};
use crate::ipc::test::debug_harden_test;
use crate::ipc::settings::{get_settings, update_theme, update_setting, request_boot_state};
use crate::services::ptt::{ptt_start, ptt_stop, ptt_cancel};
use crate::tray::{setup_linux_virtual_layer, setup_tray_window, position_tray_window};

use tauri::menu::Menu;
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, State};

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
            // ── 0. Paths Singleton (must be first) ──────────────────────────────────
            crate::utils::paths::init(app.handle());
            crate::utils::paths::ensure_dirs().ok();

            // ── 0.5 Logging (must be second, relies on paths) ───────────────────────
            let log_guard = crate::utils::logging::init(crate::utils::paths::get().logs.clone());

            // ── 0.6 Telemetry Aggregator ───────────────────────────────────────────
            let (telemetry_worker, telemetry_tx) = crate::telemetry::aggregator::TelemetryAggregator::new();
            telemetry_worker.start();

            // ── 0.7 Persistence Worker ─────────────────────────────────────────────
            let persist_tx = crate::persistence::worker::spawn_persistence_worker(
                crate::utils::paths::get().db.clone()
            );

            // ── 1. App State ────────────────────────────────────────────────────────
            let mut app_state = AppState::new(app.handle(), Some(log_guard), telemetry_tx);
            app_state.persist_tx = Some(persist_tx);
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
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "launch" => {
                        let _ = show_main_window(app.clone());
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
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    setup_tray_window(&tray_win_clone);
                    position_tray_window(&tray_win_clone).await;
                    // Initial hide to ensure it's hidden on startup despite tauri.conf.json
                    let _ = tray_win_clone.hide();
                });
            }

            // ── 3. Auto-launch engine on startup ─────────────────────────────
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = launch_engine(handle).await {
                    log::error!("[BOOTSTRAP] Engine auto-launch failed: {}", e);
                }
            });

            // ── 4. E2E Hardening Test (CLI Triggered) ────────────────────────
            let args: Vec<String> = std::env::args().collect();
            if let Some(wav_path) = args.iter().position(|a| a == "--test-harden").and_then(|i| args.get(i + 1)) {
                let wav_path = wav_path.clone();
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    log::info!("[Harden] CLI Test Triggered. Waiting for engine...");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    
                    match debug_harden_test(handle.clone(), wav_path).await {
                        Ok(res) => {
                            println!("HARDEN_TEST_SUCCESS: {}", res);
                            log::info!("[Harden] Test successful: {}", res);
                        }
                        Err(e) => {
                            println!("HARDEN_TEST_FAILURE: {}", e);
                            log::error!("[Harden] Test failed: {}", e);
                        }
                    }
                    // Exit the app after test
                    std::process::exit(0);
                });
            }

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
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            check_engine_status,
            launch_engine,
            engage,
            hide_tray_window,
            sync_hud_visibility,
            set_hud_ignore_cursor,
            update_interaction_mode,
            show_main_window,
            request_boot_state,
            get_settings,
            update_theme,
            update_setting,
            ptt_start,
            ptt_stop,
            ptt_cancel,
            get_transcript_history,
            get_sessions,
            get_turns,
            delete_session,
            debug_harden_test,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                log::info!("[Vox] Shutting down engine...");
                let state: State<'_, AppState> = app_handle.state();
                
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
