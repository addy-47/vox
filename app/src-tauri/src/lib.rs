#![recursion_limit = "256"]

extern crate symphonia_core;

pub mod core;
pub mod ipc;
pub mod monitoring;
pub mod persistence;
pub mod pipeline;
pub mod services;
pub mod setup;
pub mod toast;
pub mod tray;
pub mod utils;
pub mod window_customizer;
pub mod window_main;
pub mod wizard;

use std::{
    backtrace::Backtrace,
    env::set_var,
    fs::{create_dir_all, write},
    panic::set_hook,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    thread::{current, sleep},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tauri::{tray::TrayIconBuilder, Manager, State};

#[cfg(target_os = "linux")]
use crate::toast::setup_linux_toast_layer;
#[cfg(target_os = "linux")]
use crate::tray::setup_linux_virtual_layer;
use crate::{
    core::{
        events::VoxEvent,
        settings::{DictationInteractionMode, DictationOutputMode},
        state::{AppState, InteractionState, RuntimeStatus, TelemetryState},
    },
    ipc::{
        audio::list_audio_devices,
        memory::{
            consolidate_personal_memory, export_personal_memory, get_active_facts,
            get_personal_memory, import_personal_memory, save_personal_memory,
        },
        monitoring::{get_profiler_snapshot, get_runtime_snapshot, record_memory_profile_event},
        notifications::{
            dismiss_notifications, execute_notification_action, get_notifications,
            mark_notifications_read,
        },
        persistence::{
            continue_session, create_session, delete_session, get_sessions, get_transcript_history,
            get_turns, update_session,
        },
        pipeline::{
            end_session, launch_engine, pause_session, ptt_cancel, ptt_start, ptt_stop,
            resume_session, start_session, stop_engine, test_clip, test_clip_cancel,
        },
        projects::{create_project, delete_project, get_projects, rename_project},
        settings::{
            catalog::get_provider_caps, check_provider_health, get_model_catalog, get_settings,
            list_llm_models, probe_model_capabilities, reset_settings, setup_remote_server,
            update_setting,
        },
        setup::{
            check_updates, complete_setup_wizard, fetch_manifest, get_onboarding_status,
            get_runtime_report, manage_models, reveal_wizard,
        },
        tray::{
            hide_tray_window, set_window_click_through, show_main_window,
            toggle_tray_visibility_internal,
        },
        voices::{
            add_voice_from_file, add_voice_from_recording, delete_voice, list_voices, rename_voice,
            start_backend_recording, stop_backend_recording,
        },
    },
    monitoring::{
        snapshots::spawn_monitoring_collector,
        telemetry::{
            spawn_system_monitor, spawn_telemetry_emitter, TelemetryAggregator,
            TelemetryAggregatorHandles,
        },
    },
    persistence::{db::TOKIO_HANDLE, worker::spawn_persistence_worker, PersistenceEvent},
    services::{
        dictation::init_dictation_hotkey_listener,
        memory::{
            compaction::reconcile_uncompacted_sessions_on_boot,
            scheduler::{check_missed_consolidation_on_boot, spawn_consolidation_scheduler},
            spawn_quiet_ingestion_observer,
        },
        stt::SttCommand,
        vad::VadCommand,
    },
    setup::manifest::{AppManifest, VoxManifest},
    toast::{get_last_toast, manage_toast_window},
    tray::{build_main_tray_menu, ensure_tray_window, refresh_tray_menu, sync_live_menu_item},
    utils::{check_cpu_governor, hardware::detect_local_gpu, logging, paths},
    wizard::ensure_wizard_window,
};

/// Main entry point for the Vox application.
///
/// Sets up tray icon, menu events, window management, and auto-launches the
/// engine on startup.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Global panic containment hook: Captures backtraces, logs at FATAL, and writes emergency reports without dying silently
    set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<Any> panic payload".to_string()
        };

        let backtrace = Backtrace::capture();
        log::error!(
            target: "panic",
            "[FATAL PANIC] Thread '{}' panicked at '{}': {}\nBacktrace:\n{}",
            current().name().unwrap_or("unnamed"),
            location,
            payload,
            backtrace
        );

        // Emergency write to crash_reports if paths are available
        let crash_dir = paths::get().root.join("crash_reports");
        if create_dir_all(&crash_dir).is_ok() {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let crash_file = crash_dir.join(format!("crash_{}.log", timestamp));
            if let Err(err) = write(
                crash_file,
                format!(
                    "Panic: {}\nLocation: {}\nBacktrace:\n{}",
                    payload, location, backtrace
                ),
            ) {
                log::error!("Failed to write crash log: {:?}", err);
            }
        }
    }));

    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        log::debug!(
            "[Crypto] Ring default provider already installed or failed: {:?}",
            e
        );
    }

    // Suppress ALSA/Jack noisy logs on Linux
    #[cfg(target_os = "linux")]
    {
        set_var("ALSA_LOG_LEVEL", "0");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(window_customizer::PinchZoomDisablePlugin)
        .setup(|app| {
            // Synchronous window reveal — runs before any async bootstrap so the
            // main window is mapped immediately. The window is configured
            // visible+maximized in tauri.conf.json, so there is no withdrawn ->
            // normal WM transition to animate on first launch.
            if let Some(main_win) = app.get_webview_window("main") {
                if let Err(e) = main_win.show() {
                    log::warn!("[Vox] Failed to show main window in setup: {}", e);
                }
                if let Err(e) = main_win.set_focus() {
                    log::debug!("[Vox] Failed to focus main window in setup: {}", e);
                }
            } else {
                log::warn!("[Vox] 'main' window not found at boot");
            }

            // Capture the Tokio runtime handle early
            tauri::async_runtime::spawn(async {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    if TOKIO_HANDLE.set(handle).is_err() {
                        log::debug!("[Persistence] Tokio handle already initialized.");
                    }
                }
            });

            // ── 0. Paths Singleton (must be first) ──────────────────────────────────
            paths::init();
            paths::ensure_dirs().ok();

            // Clear ephemeral dictation history cache on boot
            let dictation_cache = paths::cache_dir().join("dictation_history.jsonl");
            if dictation_cache.exists() {
                if let Err(e) = std::fs::remove_file(&dictation_cache) {
                    log::warn!("[Bootstrap] Failed to clear ephemeral dictation cache: {}", e);
                }
            }

            // ── 0.1 Logging (must be initialized immediately after paths) ───────────
            let log_guard = logging::init(paths::get().logs.clone());

            // ── Background Manifest Caching (fetches once at boot) ──────────────────
            tauri::async_runtime::spawn(async {
                let cache_dir = paths::cache_dir();

                // Fetch and cache App Manifest
                match AppManifest::fetch().await {
                    Ok(manifest) => {
                        let path = cache_dir.join("app_manifest.json");
                        if let Ok(content) = serde_json::to_string_pretty(&manifest) {
                            if let Err(e) = write(&path, content) {
                                log::warn!("[BOOTSTRAP] Failed to write app manifest cache: {}", e);
                            } else {
                                log::info!("[BOOTSTRAP] Successfully cached app manifest.");
                            }
                        }
                    }
                    Err(e) => log::warn!("[BOOTSTRAP] Failed to fetch/cache app manifest at boot: {}", e),
                }

                // Fetch and cache Models Manifest
                match VoxManifest::fetch().await {
                    Ok(manifest) => {
                        let path = cache_dir.join("models_manifest.json");
                        if let Ok(content) = serde_json::to_string_pretty(&manifest) {
                            if let Err(e) = write(&path, content) {
                                log::warn!("[BOOTSTRAP] Failed to write models manifest cache: {}", e);
                            } else {
                                log::info!("[BOOTSTRAP] Successfully cached models manifest.");
                            }
                        }
                    }
                    Err(e) => log::warn!("[BOOTSTRAP] Failed to fetch/cache models manifest at boot: {}", e),
                }
            });

            // ── 0.6 Telemetry Aggregator ───────────────────────────────────────────
            let latest_energy = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_vad_prob = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_low = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_mid = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_high = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_playback_energy = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_playback_low = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_playback_mid = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_playback_high = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_sys_cpu = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_sys_ram = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_vox_cpu = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_vox_ram = Arc::new(AtomicU32::new(0));
            let latest_stt_ms = Arc::new(AtomicU32::new(0));
            let latest_ttft_ms = Arc::new(AtomicU32::new(0));
            let latest_voice_latency_ms = Arc::new(AtomicU32::new(0));
            let latest_threads = Arc::new(AtomicU32::new(0));
            let latest_tts_rtf = Arc::new(AtomicU32::new(0f32.to_bits()));
            let latest_playback_start_ms = Arc::new(AtomicU32::new(0));
            let latest_persistence_rate = Arc::new(AtomicU32::new(0f32.to_bits()));
            let is_db_healthy = Arc::new(AtomicBool::new(true));
            let is_private_mode = Arc::new(AtomicBool::new(false));
            let dropped_telemetry_events = Arc::new(AtomicU64::new(0));

            let (telemetry_worker, telemetry_tx) = TelemetryAggregator::new(
                TelemetryAggregatorHandles {
                    latest_energy: Arc::clone(&latest_energy),
                    latest_vad_prob: Arc::clone(&latest_vad_prob),
                    latest_low: Arc::clone(&latest_low),
                    latest_mid: Arc::clone(&latest_mid),
                    latest_high: Arc::clone(&latest_high),
                    latest_sys_cpu: Arc::clone(&latest_sys_cpu),
                    latest_sys_ram: Arc::clone(&latest_sys_ram),
                    latest_vox_cpu: Arc::clone(&latest_vox_cpu),
                    latest_vox_ram: Arc::clone(&latest_vox_ram),
                    dropped_events: Arc::clone(&dropped_telemetry_events),
                },
            );
            telemetry_worker.start();

            let telemetry_state = Arc::new(TelemetryState {
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
            });

            // ── 0.7 Database & Persistence Worker ──────────────────────────────────
            let rt_handle = persistence::db::get_tokio_handle();
            let vox_db = match rt_handle.block_on(persistence::db::VoxDb::open(&paths::get().db)) {
                Ok(db) => db,
                Err(e) => {
                    log::error!("[BOOTSTRAP] Failed to open main database: {}", e);
                    panic!("Database initialization failed: {}", e);
                }
            };
            let migration_conn = match vox_db.connect() {
                Ok(conn) => conn,
                Err(e) => {
                    log::error!("[BOOTSTRAP] Failed to vend migration connection: {}", e);
                    panic!("Database connection failed: {}", e);
                }
            };
            if let Err(e) = rt_handle.block_on(persistence::schema::run_migrations(&migration_conn)) {
                log::error!("[BOOTSTRAP] Database migration failed: {}", e);
                panic!("Database migration failed: {}", e);
            }
            let db = Arc::new(vox_db);

            let persist_tx = spawn_persistence_worker(
                Arc::clone(&db),
                Arc::clone(&telemetry_state.is_db_healthy),
                Arc::clone(&telemetry_state.latest_persistence_rate),
                Arc::clone(&telemetry_state.is_private_mode),
            );

            // ── 1. App State ────────────────────────────────────────────────────────
            let mut app_state = AppState::new(
                app.handle(),
                Some(log_guard),
                Arc::clone(&telemetry_state),
                db,
            );
            app_state.persist_tx = parking_lot::Mutex::new(Some(persist_tx));

            // ── 0.8 Hardware GPU & Tier Resolution ─────────────────────────────────
            let local_gpu_info = detect_local_gpu();
            log::info!(
                "[BOOTSTRAP] Hardware GPU Detection: vendor='{}', device='{}', tier='{}'",
                local_gpu_info.vendor,
                local_gpu_info.device_name,
                local_gpu_info.resolved_tier
            );

            // ── 1.5 Monitoring Collector ──────────────────────────────────────────
            let state_arc = Arc::new(app_state);
            app.manage(state_arc.clone());

            spawn_monitoring_collector(Arc::clone(&state_arc));
            spawn_system_monitor(app.handle().clone());
            spawn_telemetry_emitter(app.handle().clone());
            spawn_quiet_ingestion_observer(Arc::clone(&state_arc));
            spawn_consolidation_scheduler(app.handle().clone(), Arc::clone(&state_arc));

            // ── 1.6 Dictation Global Hotkey Registration ──────────────────────────
            {
                let s = state_arc
                    .settings
                    .read()
                    .unwrap_or_else(|p| {
                        log::warn!("[BOOTSTRAP] Settings RwLock poisoned; recovering inner state.");
                        p.into_inner()
                    });
                if s.dictation.enabled {
                    if let Err(e) = init_dictation_hotkey_listener(
                        app.handle(),
                        &s.dictation.hotkey,
                    ) {
                        log::warn!("[BOOTSTRAP] Could not register global dictation hotkey: {:?}", e);
                    }
                }
            }

            // ── 1. System Tray ───────────────────────────────────────────────────────
            let (tray_menu, live_i) = build_main_tray_menu(app.handle())?;

            // Store live_i handle in state for synchronization
            sync_live_menu_item(app.handle(), &live_i);


            let mut tray_builder = TrayIconBuilder::with_id("vox-tray").menu(&tray_menu);
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
                            if let Err(e) = show_main_window(handle).await {
                                log::warn!("[Tray] Failed to show main window: {}", e);
                            }
                        });
                    }
                    "live" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            toggle_tray_visibility_internal(handle).await;
                        });
                    }
                    "quit" => app.exit(0),
                    "restart" => app.restart(),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                        let app = tray.app_handle().clone();
                        let dictation_enabled = {
                            let state: State<'_, Arc<AppState>> = app.state();
                            let s = state
                                .settings
                                .read()
                                .unwrap_or_else(|p| {
                                    log::warn!("[Tray] Settings RwLock poisoned; recovering inner state.");
                                    p.into_inner()
                                });
                            s.dictation.enabled
                        };
                        if dictation_enabled {
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = launch_engine(app).await {
                                    log::warn!("[Tray] Failed to launch engine on tray click: {}", e);
                                }
                            });
                        }
                    }
                })
                .build(app)?;


            // ── 1.7.5 CPU Governor Check (Linux only — warns if not "performance") ──
            {
                let state: tauri::State<'_, Arc<AppState>> = app.state();
                if let Some(governor) = check_cpu_governor() {
                    let is_optimal = governor == "performance";
                    // Store in AppState so frontend can read from snapshot (avoids race with listener setup)
                    *state.cpu_governor.lock() = governor.clone();
                    state.cpu_governor_optimal.store(is_optimal, Ordering::Relaxed);

                    if !is_optimal {
                        log::warn!(
                            "[BOOTSTRAP] CPU governor is '{}', not 'performance'. \
                             This may degrade voice pipeline performance significantly. \
                             Consider: echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
                            governor
                        );
                    }
                }
            }

            // ── 1.8 Runtime Ready ───────────────────────────────────────────────────
            {
                let state: State<'_, Arc<AppState>> = app.state();
                state.runtime_status.store(RuntimeStatus::Ready as u32, Ordering::Relaxed);
                log::info!("[BOOTSTRAP] Runtime Ready.");
            }

            // ── 2. Conditionally construct tray HUD on demand ─────────────────────────
            {
                let (should_show_tray, setup_completed) = {
                    let state: State<'_, Arc<AppState>> = app.state();
                    let s = state
                        .settings
                        .read()
                        .unwrap_or_else(|p| {
                            log::warn!("[BOOTSTRAP] Settings RwLock poisoned; recovering inner state.");
                            p.into_inner()
                        });
                    (
                        s.dictation.enabled
                            && s.dictation.output_mode == DictationOutputMode::Tray,
                        s.system.setup_completed,
                    )
                };

                if setup_completed && should_show_tray {
                    log::info!("[BOOTSTRAP] Tray HUD mode active. Lazily constructing tray window...");
                    if let Err(e) = ensure_tray_window(app.handle()) {
                        log::error!("[BOOTSTRAP] Failed to construct tray window on startup: {}", e);
                    }
                } else if !setup_completed {
                    log::info!("[BOOTSTRAP] Onboarding setup not completed. 0 tray webviews spawned.");
                } else {
                    log::info!("[BOOTSTRAP] Tray HUD output mode not selected. 0 tray webviews spawned (saving ~250MB RAM).");
                }
            }

            // ── 3. Conditional auto-launch engine on startup ─────────────────────────────
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let (dictation_enabled, dictation_mode, setup_completed) = {
                    let state: tauri::State<'_, Arc<AppState>> = handle.state();

                    // ── 3.1 Auto-detect existing models ────────────────────────────
                    let mut settings = state
                        .settings
                        .write()
                        .unwrap_or_else(|p| {
                            log::warn!("[BOOTSTRAP] Settings RwLock poisoned; recovering inner state for write.");
                            p.into_inner()
                        });
                    if !settings.system.setup_completed && wizard::check_setup_health() {
                        log::info!("[BOOTSTRAP] Existing models detected. Auto-completing setup.");
                        settings.system.setup_completed = true;
                        if let Err(e) = settings.save() {
                            log::warn!("[BOOTSTRAP] Failed to save settings on auto-completion: {}", e);
                        }
                    }

                    (
                        settings.dictation.enabled,
                        settings.dictation.interaction_mode.clone(),
                        settings.system.setup_completed,
                    )
                };

                if setup_completed && dictation_enabled && dictation_mode == DictationInteractionMode::Passive {
                    log::info!("[BOOTSTRAP] Passive Dictation enabled. Auto-launching audio/STT engine...");
                    if let Err(e) = launch_engine(handle).await {
                        log::error!("[BOOTSTRAP] Engine auto-launch failed: {}", e);
                    }
                } else if setup_completed && dictation_enabled && dictation_mode == DictationInteractionMode::Ptt {
                    log::info!("[BOOTSTRAP] PTT Dictation enabled. Zero-idle-RAM preserved (models will load on-demand when hotkey is triggered).");
                } else if !setup_completed {
                    log::info!("[BOOTSTRAP] Setup not completed. Launching onboarding wizard...");
                    if let Ok(wizard_win) = ensure_wizard_window(&handle) {
                        if let Err(e) = wizard_win.show() {
                            log::warn!("[BOOTSTRAP] Failed to show wizard window: {}", e);
                        }
                    }
                } else {
                    log::info!("[BOOTSTRAP] Dictation disabled. Skipping engine auto-launch to save resources.");
                }
            });

            // ── 4. Background boot reconciliation for crash-recovery & uncompacted sessions ──
            let boot_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state: tauri::State<'_, Arc<AppState>> = boot_handle.state();
                if let Err(e) = reconcile_uncompacted_sessions_on_boot(
                    &boot_handle,
                    state.inner(),
                )
                .await
                {
                    log::warn!("[BOOTSTRAP] Boot memory compaction reconciliation failed: {}", e);
                }
                check_missed_consolidation_on_boot(&boot_handle, state.inner()).await;
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
            if window.label() == "toast" {
                if let tauri::WindowEvent::Resized(size) = event {
                    if size.width > 0 && size.height > 0 {
                        #[cfg(target_os = "linux")]
                        setup_linux_toast_layer(window.app_handle(), window.label());
                    }
                }
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Instead of closing, hide the main window to keep app running
                if window.label() == "main" {
                    log::info!("[Window] Close requested for main, hiding instead of closing window.");
                    if let Err(e) = window.hide() {
                        log::warn!("[Window] Failed to hide main window on close request: {}", e);
                    }
                    api.prevent_close();

                    // Evaluate engine offload if the main window is hidden
                    let handle = window.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let state: tauri::State<'_, Arc<AppState>> = handle.state();
                        let dictation_enabled = state.settings.read().map(|s| s.dictation.enabled).unwrap_or(false);

                        if !dictation_enabled && state.pipeline.state() == InteractionState::Idle {
                            log::info!("[Window] Main window hidden, Dictation is disabled, and assistant is Idle. Offloading engine...");
                            if let Err(e) = stop_engine(handle).await {
                                log::warn!("[Window] Failed to stop engine on window hide: {}", e);
                            }
                        } else {
                            log::info!("[Window] Main window hidden. Engine kept alive.");
                        }
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            launch_engine,
            stop_engine,
            start_session,
            end_session,
            pause_session,
            resume_session,
            test_clip,
            test_clip_cancel,
            hide_tray_window,
            set_window_click_through,
            show_main_window,
            manage_toast_window,
            get_last_toast,
            get_settings,
            get_model_catalog,
            get_provider_caps,
            check_provider_health,
            list_llm_models,
            probe_model_capabilities,
            setup_remote_server,
            update_setting,
            reset_settings,
            ptt_start,
            ptt_stop,
            ptt_cancel,
            // Projects
            get_projects,
            create_project,
            rename_project,
            delete_project,
            // History & Session Continuation
            create_session,
            continue_session,
            get_sessions,
            get_turns,
            update_session,
            delete_session,
            get_transcript_history,
            // Personal Memory
            get_personal_memory,
            save_personal_memory,
            consolidate_personal_memory,
            export_personal_memory,
            import_personal_memory,
            get_active_facts,
            // Voices
            list_voices,
            add_voice_from_file,
            add_voice_from_recording,
            start_backend_recording,
            stop_backend_recording,
            delete_voice,
            rename_voice,
            // Monitoring & Profiler
            get_runtime_snapshot,
            get_profiler_snapshot,
            record_memory_profile_event,
            // Setup
            fetch_manifest,
            check_updates,
            get_onboarding_status,
            get_runtime_report,
            manage_models,
            complete_setup_wizard,
            reveal_wizard,
            // Audio
            list_audio_devices,
            // Notifications & Compaction
            get_notifications,
            mark_notifications_read,
            dismiss_notifications,
            execute_notification_action,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::Exit => {
                    log::info!("[Vox] Shutting down engine...");
                    let state: State<'_, Arc<AppState>> = app_handle.state();

                    // Clear engine (this will drop VoxEngine and close channels)
                    let mut engine_lock = state.engine.blocking_lock();
                    if let Some(engine) = engine_lock.take() {
                        if let Err(e) = engine.pipeline_tx.send(VoxEvent::Shutdown) {
                            log::trace!("[Vox] Pipeline worker already closed: {}", e);
                        }
                        if let Err(e) = engine.stt_tx.send(SttCommand::Shutdown) {
                            log::trace!("[Vox] STT worker already closed: {}", e);
                        }
                        if let Err(e) = engine.vad_tx.send(VadCommand::Shutdown) {
                            log::trace!("[Vox] VAD worker already closed: {}", e);
                        }
                    }


                    // Gracefully signal persistence worker to flush and shutdown
                    {
                        let mut persist_tx_lock = state.persist_tx.lock();
                        if let Some(tx) = persist_tx_lock.take() {
                            log::info!("[Vox] Sending Shutdown signal to persistence worker...");
                            if let Err(e) = tx.send(PersistenceEvent::Shutdown) {
                                log::trace!("[Vox] Persistence worker already closed: {}", e);
                            }
                        }
                    }

                    // Allow time for threads to join
                    sleep(Duration::from_millis(150));
                }
                tauri::RunEvent::WindowEvent { label, event: win_event, .. } => {
                    if label == "main" {
                        if let tauri::WindowEvent::Destroyed = win_event {
                            log::warn!(
                                "[Vox] Main window destroyed (renderer crash?). Marking for rebuild on next Launch."
                            );
                            let state = app_handle.state::<Arc<AppState>>();
                            state
                                .main_window_destroyed
                                .store(true, Ordering::Relaxed);
                            refresh_tray_menu(app_handle);
                        }
                    }
                }
                _ => {}
            }
        });
}
