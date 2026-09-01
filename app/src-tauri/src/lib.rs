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

use crate::core::state::AppState;
use crate::ipc::history::{
    commit_session_to_history, delete_session, get_sessions, get_transcript_history, get_turns,
};
use crate::ipc::pipeline::{
    check_engine_status, copy_last_dictation_transcript, end_session, get_dictation_settings,
    get_last_dictation_transcript, get_realtime_session_cache, launch_engine, pause_session,
    ptt_cancel, ptt_start, ptt_stop, resume_session, start_session, stop_engine, test_clip,
    test_clip_cancel,
};
use crate::ipc::settings::{
    check_llm_provider_health, check_stt_provider_health, check_tts_provider_health, get_settings,
    list_llm_models, request_boot_state, request_model_catalog, reset_settings,
    setup_remote_server, update_setting,
};
use crate::ipc::tray::{
    hide_tray_window, set_hud_ignore_cursor, show_main_window, sync_hud_visibility,
    toggle_tray_visibility, update_interaction_mode,
};
#[cfg(target_os = "linux")]
use crate::toast::setup_linux_toast_layer;
#[cfg(target_os = "linux")]
use crate::tray::setup_linux_virtual_layer;

use crate::monitoring::system_monitor::spawn_system_monitor;

use tauri::tray::TrayIconBuilder;
use tauri::{Manager, State};

// ─── App Entry Point ─────────────────────────────────────────────────────────

/// Main entry point for the Vox application.
///
/// Sets up tray icon, menu events, window management, and auto-launches the
/// engine on startup.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        log::debug!(
            "[Crypto] Ring default provider already installed or failed: {:?}",
            e
        );
    }

    // Suppress ALSA/Jack noisy logs on Linux
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("ALSA_LOG_LEVEL", "0");
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
            // Capture the Tokio runtime handle early
            tauri::async_runtime::spawn(async {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    if crate::persistence::db::TOKIO_HANDLE.set(handle).is_err() {
                        log::debug!("[Persistence] Tokio handle already initialized.");
                    }
                }
            });

            // ── 0. Paths Singleton (must be first) ──────────────────────────────────
            crate::utils::paths::init();
            crate::utils::paths::ensure_dirs().ok();

            // ── 0.1 Logging (must be initialized immediately after paths) ───────────
            let log_guard = crate::utils::logging::init(crate::utils::paths::get().logs.clone());

            // ── Background Manifest Caching (fetches once at boot) ──────────────────
            tauri::async_runtime::spawn(async {
                let cache_dir = crate::utils::paths::cache_dir();

                // Fetch and cache App Manifest
                match crate::setup::manifest::AppManifest::fetch().await {
                    Ok(manifest) => {
                        let path = cache_dir.join("app_manifest.json");
                        if let Ok(content) = serde_json::to_string_pretty(&manifest) {
                            if let Err(e) = std::fs::write(&path, content) {
                                log::warn!("[BOOTSTRAP] Failed to write app manifest cache: {}", e);
                            } else {
                                log::info!("[BOOTSTRAP] Successfully cached app manifest.");
                            }
                        }
                    }
                    Err(e) => log::warn!("[BOOTSTRAP] Failed to fetch/cache app manifest at boot: {}", e),
                }

                // Fetch and cache Models Manifest
                match crate::setup::manifest::VoxManifest::fetch().await {
                    Ok(manifest) => {
                        let path = cache_dir.join("models_manifest.json");
                        if let Ok(content) = serde_json::to_string_pretty(&manifest) {
                            if let Err(e) = std::fs::write(&path, content) {
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
                crate::monitoring::aggregator::TelemetryAggregatorHandles {
                    latest_energy: std::sync::Arc::clone(&latest_energy),
                    latest_vad_prob: std::sync::Arc::clone(&latest_vad_prob),
                    latest_low: std::sync::Arc::clone(&latest_low),
                    latest_mid: std::sync::Arc::clone(&latest_mid),
                    latest_high: std::sync::Arc::clone(&latest_high),
                    latest_sys_cpu: std::sync::Arc::clone(&latest_sys_cpu),
                    latest_sys_ram: std::sync::Arc::clone(&latest_sys_ram),
                    latest_vox_cpu: std::sync::Arc::clone(&latest_vox_cpu),
                    latest_vox_ram: std::sync::Arc::clone(&latest_vox_ram),
                    dropped_events: std::sync::Arc::clone(&dropped_telemetry_events),
                },
            );
            telemetry_worker.start();

            let telemetry_state = std::sync::Arc::new(crate::core::state::TelemetryState {
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

            // ── 0.7 Persistence Worker ─────────────────────────────────────────────
            let persist_tx = crate::persistence::worker::spawn_persistence_worker(
                crate::utils::paths::get().db.clone(),
                std::sync::Arc::clone(&telemetry_state.is_db_healthy),
                std::sync::Arc::clone(&telemetry_state.latest_persistence_rate),
                std::sync::Arc::clone(&telemetry_state.is_private_mode),
            );

            // ── 1. App State ────────────────────────────────────────────────────────
            let mut app_state = AppState::new(
                app.handle(),
                Some(log_guard),
                std::sync::Arc::clone(&telemetry_state),
            );
            app_state.persist_tx = parking_lot::Mutex::new(Some(persist_tx));

            // ── 0.8 Hardware GPU & Tier Resolution ─────────────────────────────────
            let local_gpu_info = crate::utils::hardware::detect_local_gpu();
            log::info!(
                "[BOOTSTRAP] Hardware GPU Detection: vendor='{}', device='{}', tier='{}'",
                local_gpu_info.vendor,
                local_gpu_info.device_name,
                local_gpu_info.resolved_tier
            );

            // ── 0.9 Memory Worker (Gated on Tier 1B+ and MemorySettings) ───────────
            let memory_enabled = {
                let s = app_state.settings.read().unwrap();
                s.memory.pipeline_processing_enabled
            };

            if memory_enabled && local_gpu_info.has_gpu {
                let memory_tx = crate::persistence::memory_worker::spawn_memory_worker(
                    crate::utils::paths::get().db.clone(),
                    std::sync::Arc::clone(&app_state.settings),
                    app_state.memory.graph_version.clone(),
                    app_state.pipeline.state_rx.clone(),
                );
                app_state.memory_tx = parking_lot::Mutex::new(Some(memory_tx));
                log::info!("[BOOTSTRAP] Memory Worker spawned on background thread.");
            } else {
                log::info!(
                    "[BOOTSTRAP] Memory Worker skipped (memory_enabled={}, has_gpu={}).",
                    memory_enabled,
                    local_gpu_info.has_gpu
                );
            }

            // ── 1.5 Monitoring Collector ──────────────────────────────────────────
            let state_arc = std::sync::Arc::new(app_state);
            app.manage(state_arc.clone());

            crate::monitoring::collector::spawn_monitoring_collector(std::sync::Arc::clone(&state_arc));
            spawn_system_monitor(app.handle().clone());
            crate::monitoring::telemetry_emitter::spawn_telemetry_emitter(app.handle().clone());
            crate::services::memory::spawn_state_compaction_observer(std::sync::Arc::clone(&state_arc));

            // ── 1.6 Dictation Global Hotkey Registration ──────────────────────────
            {
                let s = state_arc.settings.read().unwrap();
                if s.dictation.enabled {
                    if let Err(e) = crate::pipeline::dictation::init_dictation_hotkey_listener(
                        app.handle(),
                        &s.dictation.hotkey,
                    ) {
                        log::warn!("[BOOTSTRAP] Could not register global dictation hotkey: {:?}", e);
                    }
                }
            }

            // ── 1. System Tray ───────────────────────────────────────────────────────
            let (tray_menu, live_i) = crate::tray::build_main_tray_menu(app.handle())?;

            // Store live_i handle in state for synchronization
            crate::tray::sync_live_menu_item(app.handle(), &live_i);


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
                            toggle_tray_visibility(handle).await;
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
                            let state: State<'_, std::sync::Arc<AppState>> = app.state();
                            let s = state.settings.read().unwrap();
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
                let state: tauri::State<'_, std::sync::Arc<AppState>> = app.state();
                if let Some(governor) = crate::utils::check_cpu_governor() {
                    let is_optimal = governor == "performance";
                    // Store in AppState so frontend can read from snapshot (avoids race with listener setup)
                    *state.cpu_governor.lock() = governor.clone();
                    state.cpu_governor_optimal.store(is_optimal, std::sync::atomic::Ordering::Relaxed);

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
                use crate::core::state::RuntimeStatus;
                use std::sync::atomic::Ordering;
                let state: State<'_, std::sync::Arc<AppState>> = app.state();
                state.runtime_status.store(RuntimeStatus::Ready as u32, Ordering::Relaxed);
                log::info!("[BOOTSTRAP] Runtime Ready.");
            }

            // ── 1.8.1 Toast test poll (dev — emits every 10s after boot) ─────────
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    let mut tick: u32 = 0;
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                        tick = tick.wrapping_add(1);
                        let title = format!("Toast Test #{tick}");
                        let message = match tick % 4 {
                            0 => "Dictation Copied — transcript on clipboard.".to_string(),
                            1 => "Dictation Pasted — transcript injected.".to_string(),
                            2 => "Paste Blocked by OS — fallback to clipboard.".to_string(),
                            _ => format!("Voice Error — simulated poll tick {tick}."),
                        };
                        let level = match tick % 4 {
                            0 => crate::core::events::ToastLevel::Success,
                            1 => crate::core::events::ToastLevel::Success,
                            2 => crate::core::events::ToastLevel::Warning,
                            _ => crate::core::events::ToastLevel::Error,
                        };
                        if let Err(e) = crate::toast::show_toast(&handle, &title, &message, level) {
                            log::warn!("[Toast::Poll] Failed to emit test toast: {}", e);
                        } else {
                            log::info!("[Toast::Poll] Emitted test toast #{tick}: {}", title);
                        }
                    }
                });
            }

            // ── 2. Conditionally construct tray HUD on demand ─────────────────────────
            {
                let (should_show_tray, setup_completed) = {
                    let state: State<'_, std::sync::Arc<AppState>> = app.state();
                    let s = state.settings.read().unwrap();
                    (
                        s.dictation.enabled
                            && s.dictation.output_mode == crate::core::settings::DictationOutputMode::Tray,
                        s.system.setup_completed,
                    )
                };

                if setup_completed && should_show_tray {
                    log::info!("[BOOTSTRAP] Tray HUD mode active. Lazily constructing tray window...");
                    if let Err(e) = crate::tray::ensure_tray_window(app.handle()) {
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
                    let state: tauri::State<'_, std::sync::Arc<AppState>> = handle.state();

                    // ── 3.1 Auto-detect existing models ────────────────────────────
                    let mut settings = state.settings.write().unwrap();
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

                if setup_completed && dictation_enabled && dictation_mode == crate::core::settings::DictationInteractionMode::Passive {
                    log::info!("[BOOTSTRAP] Passive Dictation enabled. Auto-launching audio/STT engine...");
                    if let Err(e) = launch_engine(handle).await {
                        log::error!("[BOOTSTRAP] Engine auto-launch failed: {}", e);
                    }
                } else if setup_completed && dictation_enabled && dictation_mode == crate::core::settings::DictationInteractionMode::Ptt {
                    log::info!("[BOOTSTRAP] PTT Dictation enabled. Zero-idle-RAM preserved (models will load on-demand when hotkey is triggered).");
                } else if !setup_completed {
                    log::info!("[BOOTSTRAP] Setup not completed. Launching onboarding wizard...");
                    if let Ok(wizard_win) = crate::wizard::ensure_wizard_window(&handle) {
                        if let Err(e) = wizard_win.show() {
                            log::warn!("[BOOTSTRAP] Failed to show wizard window: {}", e);
                        }
                    }
                } else {
                    log::info!("[BOOTSTRAP] Dictation disabled. Skipping engine auto-launch to save resources.");
                }
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
                        let state: tauri::State<'_, std::sync::Arc<AppState>> = handle.state();
                        let dictation_enabled = state.settings.read().map(|s| s.dictation.enabled).unwrap_or(false);

                        if !dictation_enabled && state.pipeline.state() == crate::core::state::InteractionState::Idle {
                            log::info!("[Window] Main window hidden, Dictation is disabled, and assistant is Idle. Offloading engine...");
                            if let Err(e) = crate::ipc::pipeline::stop_engine(handle).await {
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
            check_engine_status,
            launch_engine,
            stop_engine,
            start_session,
            end_session,
            pause_session,
            resume_session,
            test_clip,
            test_clip_cancel,
            get_realtime_session_cache,
            hide_tray_window,
            toggle_tray_visibility,
            sync_hud_visibility,
            set_hud_ignore_cursor,
            update_interaction_mode,
            show_main_window,
            crate::toast::show_toast_window,
            crate::toast::hide_toast_window,
            crate::toast::destroy_toast_window_cmd,
            crate::toast::get_last_toast,
            request_boot_state,
            request_model_catalog,
            check_llm_provider_health,
            check_stt_provider_health,
            check_tts_provider_health,
            list_llm_models,
            crate::ipc::settings::get_cached_capabilities,
            crate::ipc::settings::probe_model_capabilities,
            crate::ipc::settings::validate_llm_token_cap,
            setup_remote_server,
            get_settings,
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
            // Dictation
            get_dictation_settings,
            get_last_dictation_transcript,
            copy_last_dictation_transcript,
            // Voices
            crate::ipc::voices::list_voices,
            crate::ipc::voices::fetch_edge_tts_voices,
            crate::ipc::voices::add_voice_from_file,
            crate::ipc::voices::add_voice_from_recording,
            crate::ipc::voices::start_backend_recording,
            crate::ipc::voices::stop_backend_recording,
            crate::ipc::voices::delete_voice,
            // Monitoring & Profiler
            crate::ipc::monitoring::get_runtime_snapshot,
            crate::ipc::monitoring::get_runtime_history,
            crate::ipc::monitoring::clear_runtime_history,
            crate::ipc::monitoring::get_profiler_snapshot,
            crate::ipc::monitoring::record_memory_profile_event,
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
            crate::ipc::audio::list_output_devices,
            // Memory Subsystem
            crate::ipc::memory::get_graph_version,
            crate::ipc::memory::get_memory_graph_topology,
            crate::ipc::memory::get_memory_fact_detail,
            crate::ipc::memory::get_memory_stats,
            crate::ipc::memory::edit_fact_content,
            crate::ipc::memory::reassign_fact_collection,
            crate::ipc::memory::soft_delete_fact,
            crate::ipc::memory::user_edit_memory,
            crate::ipc::memory::user_delete_memory,
            crate::ipc::memory::get_unresolved_conflicts,
            crate::ipc::memory::resolve_memory_conflict,
            crate::ipc::memory::get_memory_relations,
            crate::ipc::memory::get_memory_queue_status,
            crate::ipc::memory::toggle_pipeline_processing,
            crate::ipc::memory::retry_failed_queue,
            crate::ipc::memory::retry_failed_queue_items,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::Exit => {
                    log::info!("[Vox] Shutting down engine...");
                    let state: State<'_, std::sync::Arc<AppState>> = app_handle.state();

                    // Clear engine (this will drop VoxEngine and close channels)
                    let mut engine_lock = state.engine.blocking_lock();
                    if let Some(engine) = engine_lock.take() {
                        if let Err(e) = engine.pipeline_tx.send(crate::core::events::VoxEvent::Shutdown) {
                            log::trace!("[Vox] Pipeline worker already closed: {}", e);
                        }
                        if let Err(e) = engine.stt_tx.send(crate::services::stt::SttCommand::Shutdown) {
                            log::trace!("[Vox] STT worker already closed: {}", e);
                        }
                        if let Err(e) = engine.vad_tx.send(crate::services::vad::VadCommand::Shutdown) {
                            log::trace!("[Vox] VAD worker already closed: {}", e);
                        }
                    }

                    // Gracefully signal background memory worker to flush and shutdown
                    {
                        let mut memory_tx_lock = state.memory_tx.lock();
                        if let Some(tx) = memory_tx_lock.take() {
                            log::info!("[Vox] Sending Shutdown signal to memory worker...");
                            if let Err(e) = tx.send(crate::persistence::events::MemoryWorkerEvent::Shutdown) {
                                log::trace!("[Vox] Memory worker already closed: {}", e);
                            }
                        }
                    }

                    // Gracefully signal persistence worker to flush and shutdown
                    {
                        let mut persist_tx_lock = state.persist_tx.lock();
                        if let Some(tx) = persist_tx_lock.take() {
                            log::info!("[Vox] Sending Shutdown signal to persistence worker...");
                            if let Err(e) = tx.send(crate::persistence::events::PersistenceEvent::Shutdown) {
                                log::trace!("[Vox] Persistence worker already closed: {}", e);
                            }
                        }
                    }

                    // Allow time for threads to join
                    std::thread::sleep(std::time::Duration::from_millis(150));
                }
                tauri::RunEvent::WindowEvent { label, event: win_event, .. } => {
                    if label == "main" {
                        if let tauri::WindowEvent::Destroyed = win_event {
                            log::warn!(
                                "[Vox] Main window destroyed (renderer crash?). Marking for rebuild on next Launch."
                            );
                            let state = app_handle.state::<std::sync::Arc<AppState>>();
                            state
                                .main_window_destroyed
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            crate::tray::refresh_tray_menu(app_handle);
                        }
                    }
                }
                _ => {}
            }
        });
}
