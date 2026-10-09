#![recursion_limit = "256"]

extern crate symphonia_core;

pub mod config;
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

#[cfg(target_os = "linux")]
use std::env::set_var;
use std::{
    backtrace::Backtrace,
    fs::write,
    panic::set_hook,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    thread::{current, sleep, Builder as ThreadBuilder},
    time::Duration,
};

use tauri::{Manager, State};

#[cfg(desktop)]
use crate::ipc::tray::toggle_tray_visibility_internal;
#[cfg(target_os = "linux")]
use crate::tray::setup_linux_virtual_layer;
use crate::{
    core::{
        events::VoxEvent,
        state::{AppState, InteractionState, RuntimeStatus, TelemetryState},
    },
    ipc::{
        audio::list_audio_devices,
        catalog::{
            check_provider_health, get_model_capabilities_cache, get_model_catalog,
            get_provider_caps, list_llm_models, probe_model_capabilities, setup_remote_server,
        },
        memory::{
            consolidate_personal_memory, get_ingestion_stats, get_memory_revisions,
            get_observations, get_personal_memory, get_personal_memory_versions,
            regenerate_personal_memory, resolve_memory_revisions, save_personal_memory,
            set_active_personal_memory_version,
        },
        monitoring::{get_profiler_snapshot, get_runtime_snapshot, record_memory_profile_event},
        notifications::{
            dismiss_notifications, execute_notification_action, get_notifications,
            mark_notifications_read,
        },
        persistence::{
            compact_session, delete_session, get_sessions, get_transcript_history, get_turns,
            update_session,
        },
        pipeline::{
            continue_session, create_session, end_session, launch_engine, pause_session,
            ptt_cancel, ptt_start, ptt_stop, resume_session, set_mic_muted, set_playback_muted,
            set_session_private_mode, start_session, stop_engine, submit_text_input,
        },
        projects::{create_project, delete_project, get_projects, rename_project},
        settings::{get_settings, reset_settings, update_setting},
        setup::{
            check_updates, complete_setup_wizard, fetch_manifest, get_onboarding_status,
            get_runtime_report, manage_models, reveal_wizard,
        },
        tray::{hide_tray_window, set_window_click_through, show_main_window},
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
    persistence::{worker::spawn_persistence_worker, PersistenceEvent, TOKIO_HANDLE},
    pipeline::dictation::{DictationInteractionMode, DictationOutputMode},
    services::{
        dictation::init_dictation_hotkey_listener,
        llm::catalog::{load_local_baseline_cache, spawn_catalog_sync},
        memory::{
            compaction::reconcile_uncompacted_sessions_on_boot,
            reconcile_crashed_queue_on_boot,
            scheduler::{check_missed_consolidation_on_boot, spawn_consolidation_scheduler},
            spawn_ingestion_sweep, warmup_tokenizer,
        },
        stt::SttCommand,
        vad::VadCommand,
    },
    setup::manifest::{AppManifest, VoxManifest},
    tray::{ensure_tray_window, refresh_tray_menu},
    utils::{check_cpu_governor, hardware::detect_local_gpu, logging, paths},
    wizard::ensure_wizard_window,
};

#[cfg(desktop)]
fn setup_desktop_plugins<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(window_customizer::PinchZoomDisablePlugin)
}

#[cfg(not(desktop))]
fn setup_desktop_plugins<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
}

#[cfg(desktop)]
fn setup_platform_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::tray::TrayIconBuilder;

    use crate::tray::{build_main_tray_menu, sync_live_menu_item};

    let (tray_menu, live_i) = build_main_tray_menu(app.handle())?;
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
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle().clone();
                let dictation_enabled = {
                    let state: State<'_, Arc<AppState>> = app.state();
                    let s = state.settings.read().unwrap_or_else(|p| {
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

    Ok(())
}

#[cfg(not(desktop))]
fn setup_platform_tray(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

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

        // Emergency write to crashes directory if paths are available
        if let Some(path) = crate::utils::crash::write_crash_report(
            crate::utils::crash::CRASH_KIND_PANIC,
            &format!("{} at {}", payload, location),
            &backtrace.to_string(),
        ) {
            log::error!("[FATAL PANIC] Crash report written to {}", path);
        }
    }));

    // Heap corruption and invalid accesses from native libraries abort without
    // ever reaching the panic hook above, so they get their own handler.
    crate::utils::crash::install_native_crash_handler();

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

        // Best-effort: make glibc's allocator validate the heap on every
        // malloc/free so a corruption from a native library aborts *at the
        // offending write* instead of minutes later at some unrelated large
        // allocation — which is what made the ZipVoice crash undiagnosable.
        //
        // glibc caches its tunables at libc init, so setting this from `main`
        // is only reliable when the process is launched with it already in the
        // environment. For a guaranteed run, prefix the command:
        //     MALLOC_CHECK_=3 pnpm tauri dev
        // Debug builds only: it roughly doubles allocation cost, which would
        // corrupt the benchmark numbers from `cargo bench --release`.
        #[cfg(all(target_os = "linux", debug_assertions))]
        if std::env::var("MALLOC_CHECK_").is_err() {
            set_var("MALLOC_CHECK_", "3");
            log::debug!(
                "[Crash] MALLOC_CHECK_=3 requested in-process; \
                 export it in the shell if allocator validation does not engage"
            );
        }
    }

    let builder = tauri::Builder::default();
    setup_desktop_plugins(builder)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // The `main` webview is constructed LAZILY, never at startup. It was
            // previously declared in tauri.conf.json (`visible: true`), so Tauri
            // built it during bootstrap even on a first run — which put a second
            // window (the "setup not complete" placeholder) on screen beside the
            // wizard. `main` is now created by `window_main::ensure_main_window`
            // only once setup is complete (see bootstrap step 3 and the tray
            // "Launch" action).
            log::info!("[Vox] Bootstrap: deferring 'main' webview construction until setup completes.");

            // Capture the Tokio runtime handle early
            tauri::async_runtime::spawn(async {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    if TOKIO_HANDLE.set(handle).is_err() {
                        log::debug!("[Persistence] Tokio handle already initialized.");
                    }
                }
            });

            // Pre-warm BPE tokenizer vocabulary to eliminate Turn 1 dispatch latency
            ThreadBuilder::new()
                .name("vox-bpe-warmup".into())
                .spawn(warmup_tokenizer)
                .ok();

            // ── 0. Paths Singleton & Migration (must be first) ──────────────────────
            paths::init();
            paths::migrate_legacy_layout(&paths::get().root);
            paths::ensure_dirs().ok();

            // ── 0.1 Logging (must be initialized immediately after paths) ───────────
            let log_guard = logging::init(paths::logs_dir());

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
            let rt_handle = persistence::get_tokio_handle();
            let vox_db = match rt_handle.block_on(persistence::VoxDb::open(&paths::get().db)) {
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

            // ── 0.9 Model Catalog Baseline (bundled snapshot + background models.dev sync)
            load_local_baseline_cache();
            spawn_catalog_sync();

            // ── 1.5 Monitoring Collector ──────────────────────────────────────────
            let state_arc = Arc::new(app_state);
            app.manage(state_arc.clone());

            spawn_monitoring_collector(Arc::clone(&state_arc));
            spawn_system_monitor(app.handle().clone());
            spawn_telemetry_emitter(app.handle().clone());
            if let Ok(conn) = state_arc.db.connect() {
                tauri::async_runtime::spawn(async move {
                    let _ = reconcile_crashed_queue_on_boot(&conn).await;
                });
            }
            spawn_ingestion_sweep(Arc::clone(&state_arc), Some(app.handle().clone()), None);
            let is_daily_cadence = state_arc
                .settings
                .read()
                .map(|s| s.personal_memory.consolidation_cadence == "daily")
                .unwrap_or(false);
            if is_daily_cadence {
                spawn_consolidation_scheduler(app.handle().clone(), Arc::clone(&state_arc));
            }

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
            setup_platform_tray(app)?;


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
                        settings.dictation.interaction_mode,
                        settings.system.setup_completed,
                    )
                };

                let state: tauri::State<'_, Arc<AppState>> = handle.state();
                if setup_completed && dictation_enabled {
                    state.pipeline.set_dictation_state(InteractionState::Ready);
                } else {
                    state.pipeline.set_dictation_state(InteractionState::Idle);
                }

                if setup_completed {
                    // `main` is no longer declared in tauri.conf.json, so this is
                    // the first and only construction point on a normal launch.
                    if let Err(e) = window_main::ensure_main_window(&handle) {
                        log::error!("[BOOTSTRAP] Failed to construct main window: {}", e);
                    }
                }

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
                        // Deliberately NOT shown here. Mapping the window before
                        // the webview's first paint showed an unpainted white
                        // surface for a few frames. The frontend reveals itself
                        // through `revealWizard()` once React + CSS are mounted;
                        // this fallback only covers the case where that call
                        // never arrives (renderer failure).
                        let fallback_win = wizard_win.clone();
                        tauri::async_runtime::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
                            if let Err(e) = fallback_win.show() {
                                log::warn!("[BOOTSTRAP] Fallback wizard reveal failed: {}", e);
                            } else {
                                log::warn!("[BOOTSTRAP] Wizard revealed by fallback; frontend reveal did not fire.");
                            }
                        });
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
            submit_text_input,
            set_playback_muted,
            set_mic_muted,
            set_session_private_mode,
            hide_tray_window,
            set_window_click_through,
            show_main_window,
            get_settings,
            get_model_catalog,
            get_provider_caps,
            check_provider_health,
            list_llm_models,
            probe_model_capabilities,
            get_model_capabilities_cache,
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
            compact_session,
            get_transcript_history,
            // Personal Memory
            get_personal_memory,
            save_personal_memory,
            consolidate_personal_memory,
            regenerate_personal_memory,
            get_personal_memory_versions,
            set_active_personal_memory_version,
            get_observations,
            get_ingestion_stats,
            get_memory_revisions,
            resolve_memory_revisions,
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

                    // Flush any pending debounced settings save immediately
                    let debounce = state.save_debounce.blocking_lock().take();
                    if let Some(handle) = debounce {
                        handle.abort();
                        let snapshot = state
                            .settings
                            .read()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone();
                        if let Err(e) = snapshot.save() {
                            log::error!("[Vox] Final settings flush failed: {}", e);
                        } else {
                            log::info!("[Vox] Final settings flush completed.");
                        }
                    }

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
