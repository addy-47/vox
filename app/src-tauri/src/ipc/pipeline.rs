use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VoxEngine};
use crate::services::audio::AudioStream;
use crate::services::pipeline::PipelineOrchestrator;
use crate::services::playback::PlaybackEngine;
use crate::services::stt::{spawn_stt_worker, SttCommand};
use crate::services::vad::{
    earshot_vad::EarshotVadEngine, ten_onnx::VadEngine as TenVadEngine, VadBackend,
};
use crate::tray::position_tray_window;
use crate::utils::paths;
use ringbuf::traits::Split;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub async fn check_engine_status(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<bool, String> {
    let lock = state.engine.lock().await;
    Ok(lock.is_some())
}

#[tauri::command]
pub async fn stop_engine(app: AppHandle) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let mut lock = state.engine.lock().await;

    if let Some(mut engine) = lock.take() {
        log::info!("[PIPELINE] >>> Shutting down 3-Tier Audio Engine (Deterministic)...");

        // 1. Signal all threads to exit via Atomic flag (Secondary exit path)
        state
            .pipeline
            .engine_shutdown
            .store(true, Ordering::Relaxed);

        // 2. Signal threads via channels (Primary exit path)
        let _ = engine.pipeline_tx.send(VoxEvent::Shutdown);
        let _ = engine.stt_tx.send(SttCommand::Shutdown);
        let _ = engine.vad_tx.send(crate::core::state::VadCommand::Shutdown);

        // Wait for threads to join
        if let Some(h) = engine.orchestrator_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = engine.stt_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = engine.vad_handle.take() {
            let _ = h.join();
        }
        log::info!("[PIPELINE] Audio Engine threads joined. Resources freed.");

        // 3. Gracefully shutdown Persistence Worker (Architect's requirement)
        // This closes the SQLite connection and flushes the WAL.
        {
            let mut persist_lock = state.persist_tx.lock().unwrap();
            if let Some(tx) = persist_lock.take() {
                let _ = tx.send(crate::persistence::events::PersistenceEvent::Shutdown);
                log::info!("[PIPELINE] Persistence worker signaled to shutdown/flush.");
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn engage(
    state: State<'_, std::sync::Arc<AppState>>,
    app: AppHandle,
) -> Result<(), String> {
    let current = state.pipeline.is_engaged.load(Ordering::Relaxed);
    let new_state = !current;

    if new_state {
        log::info!("[Pipeline] Engaging main application pipeline. Starting User Session.");
        state.pipeline.is_engaged.store(true, Ordering::Relaxed);
        state
            .owner
            .store(InteractionOwner::MainWindow as u32, Ordering::Relaxed);

        // Ensure engine is launched
        if state.engine.lock().await.is_none() {
            log::info!("[Pipeline] Engine not running. Launching for Engagement...");
            launch_engine(app.clone()).await?;
        }

        if let Some(engine) = state.engine.lock().await.as_ref() {
            // Generate conversation ID based on epoch ms
            let conv_id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            state.conversation_id.store(conv_id, Ordering::Relaxed);
            log::info!("[Session] >>> USER SESSION STARTED: id={}", conv_id);

            // Persist Session Start
            {
                let persist_tx = state.persist_tx.lock().unwrap();
                if let Some(ref tx) = *persist_tx {
                    if let Err(_) = tx.try_send(
                        crate::persistence::events::PersistenceEvent::SessionStarted {
                            id: conv_id,
                            timestamp_ms: conv_id,
                        },
                    ) {
                        state
                            .dropped_persistence_events
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }

            let _ = engine.pipeline_tx.send(VoxEvent::WarmUp);
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(
                    InteractionOwner::MainWindow,
                ));
        }
    } else {
        log::info!("[Pipeline] Disengaging pipeline. Ending User Session.");
        state.pipeline.is_engaged.store(false, Ordering::Relaxed);
        state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

        if let Some(engine) = state.engine.lock().await.as_ref() {
            let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);
            let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id });
            let _ = engine.stt_tx.send(SttCommand::ResetStream);
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(
                    InteractionOwner::Tray,
                ));

            // Persist Session End
            let conv_id = state.conversation_id.swap(0, Ordering::Relaxed);
            if conv_id != 0 {
                log::info!("[Session] <<< USER SESSION ENDED (User Disengaged): id={}", conv_id);
                let persist_tx = state.persist_tx.lock().unwrap();
                if let Some(ref tx) = *persist_tx {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    if let Err(_) =
                        tx.try_send(crate::persistence::events::PersistenceEvent::SessionEnded {
                            id: conv_id,
                            timestamp_ms: now,
                        })
                    {
                        state
                            .dropped_persistence_events
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }

        state
            .owner
            .store(InteractionOwner::Tray as u32, Ordering::Relaxed);

        {
            let mut state_lock = state.pipeline.state.lock().unwrap();
            *state_lock = InteractionState::Idle;
        }
        let _ = app.emit_to("main", "state_changed", InteractionState::Idle);
        let _ = app.emit_to("tray", "state_changed", InteractionState::Idle);
    }

    Ok(())
}

#[tauri::command]
pub async fn launch_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let mut lock = state.engine.lock().await;

    if lock.is_some() {
        let (tray_enabled, setup_completed) = {
            let s = state.settings.read().unwrap();
            (s.ui.tray_enabled, s.setup.completed)
        };
        if setup_completed && tray_enabled {
            if let Some(window) = app.get_webview_window("tray") {
                let _ = window.show();
                position_tray_window(&window).await;
                let _ = window.set_focus();
            }
        }
        return Ok(());
    }

    log::info!("[PIPELINE] >>> Launching 3-Tier Audio Engine...");

    // We MUST reset them before spawning workers, otherwise they exit immediately.
    state
        .pipeline
        .engine_shutdown
        .store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    // ── Re-spawn Persistence Worker if needed ──────────────────────────────
    // If the app was idle, the persistence worker was shut down to free the DB lock.
    {
        let mut persist_lock = state.persist_tx.lock().unwrap();
        if persist_lock.is_none() {
            log::info!("[PIPELINE] Re-spawning Persistence Worker...");
            let tx = crate::persistence::worker::spawn_persistence_worker(
                crate::utils::paths::get().db.clone(),
                std::sync::Arc::clone(&state.is_db_healthy),
                std::sync::Arc::clone(&state.latest_persistence_rate),
                std::sync::Arc::clone(&state.is_private_mode),
            );
            *persist_lock = Some(tx);
        }
    }
    // ────────────────────────────────────────────────────────────────────────

    // Ensure manifest is loaded from cache/disk before launching engine
    {
        let mut m = state.manifest.write().await;
        if m.is_none() {
            let manifest_path = crate::utils::paths::get()
                .models
                .join("models_manifest.json");
            if manifest_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) =
                        serde_json::from_str::<crate::setup::manifest::VoxManifest>(&content)
                    {
                        *m = Some(manifest);
                    }
                }
            }
        }
    }

    app.emit(crate::core::constants::EVENT_MODEL_LOADING, "VAD")
        .ok();

    let (stt_model_path, vad_model_path_opt, vad_backend_opt, pre_load, input_device) = {
        let (vad_backend, tray_enabled, input_device) = {
            let settings = state.settings.read().unwrap();
            (
                settings.vad.vad_backend.clone(),
                settings.ui.tray_enabled,
                settings.audio.input_device.clone(),
            )
        };
        let models_dir = paths::get().models.clone();
        let manifest_lock = state.manifest.read().await;

        let stt = models_dir.join(crate::core::constants::MODEL_DIR_STT);

        // Only resolve the VAD model path for TenVAD — earshot has no external model file.
        let vad_path = if vad_backend == crate::core::settings::VadBackendOption::TenVad {
            if let Some(ref manifest) = *manifest_lock {
                if let Some(group) = manifest.model_groups.iter().find(|g| g.id == "ten_vad") {
                    if let Some(file) = group.files.first() {
                        Some(models_dir.join(&file.path))
                    } else {
                        Some(
                            models_dir
                                .join(crate::core::constants::MODEL_DIR_VAD)
                                .join(crate::core::constants::MODEL_FILE_VAD),
                        )
                    }
                } else {
                    Some(
                        models_dir
                            .join(crate::core::constants::MODEL_DIR_VAD)
                            .join(crate::core::constants::MODEL_FILE_VAD),
                    )
                }
            } else {
                Some(
                    models_dir
                        .join(crate::core::constants::MODEL_DIR_VAD)
                        .join(crate::core::constants::MODEL_FILE_VAD),
                )
            }
        } else {
            None
        };

        (stt, vad_path, vad_backend, tray_enabled, input_device)
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    let (stt_tx_internal, stt_rx_internal) = std::sync::mpsc::channel::<SttCommand>();
    let (vad_tx_internal, vad_rx_internal) =
        std::sync::mpsc::channel::<crate::core::state::VadCommand>();
    let (vox_event_tx, vox_event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let stt_handle = spawn_stt_worker(
        app.clone(),
        stt_rx_internal,
        stt_model_path,
        Some(vox_event_tx.clone()),
        state.pipeline.cancel_flag.clone(),
        state.is_stt_loaded.clone(),
        state.pipeline.engine_shutdown.clone(),
        pre_load,
    )?;

    let threshold = state.settings.read().unwrap().vad.threshold;
    let vad = match vad_backend_opt {
        crate::core::settings::VadBackendOption::Earshot => {
            log::info!("[PIPELINE] VAD backend: Earshot (pure Rust, no model file required).");
            match EarshotVadEngine::new(threshold) {
                Ok(engine) => {
                    app.emit(crate::core::constants::EVENT_MODEL_READY, "VAD")
                        .ok();
                    VadBackend::Earshot(engine)
                }
                Err(e) => {
                    app.emit(
                        crate::core::constants::EVENT_MODEL_FAILED,
                        format!("VAD: {}", e),
                    )
                    .ok();
                    return Err(e.to_string());
                }
            }
        }
        crate::core::settings::VadBackendOption::TenVad => {
            let vad_model_path = vad_model_path_opt.ok_or_else(|| {
                "[PIPELINE] TenVAD selected but model path could not be resolved.".to_string()
            })?;
            log::info!(
                "[PIPELINE] VAD backend: TenVAD (ONNX, model={:?}).",
                vad_model_path
            );
            match TenVadEngine::new(&vad_model_path, threshold) {
                Ok(engine) => {
                    app.emit(crate::core::constants::EVENT_MODEL_READY, "VAD")
                        .ok();
                    VadBackend::Ten(engine)
                }
                Err(e) => {
                    app.emit(
                        crate::core::constants::EVENT_MODEL_FAILED,
                        format!("VAD: {}", e),
                    )
                    .ok();
                    return Err(e.to_string());
                }
            }
        }
    };
    let (producer, consumer) = ringbuf::HeapRb::<f32>::new(16000 * 4).split();

    let stt_tx_for_vad = stt_tx_internal.clone();
    let vad_rx_for_vad = vad_rx_internal;
    let app_handle_vad = app.clone();
    let telemetry_tx_for_vad = state.telemetry_tx.clone();
    let vox_event_tx_for_vad = vox_event_tx.clone();
    let state_vad = state.inner().clone();
    let vad_handle = std::thread::Builder::new()
        .name("vox-vad-worker".to_string())
        .spawn(move || {
            if let Err(e) = crate::services::vad::spawn_vad_actor(
                vad,
                app_handle_vad,
                consumer,
                event_tx,
                stt_tx_for_vad,
                vad_rx_for_vad,
                telemetry_tx_for_vad,
                Some(vox_event_tx_for_vad),
                state_vad.is_vad_loaded.clone(),
            ) {
                log::error!("[VAD] CRITICAL: Worker thread crashed: {}", e);
            }
        })
        .map_err(|e| e.to_string())?;

    let app_handle_emit = app.clone();
    tauri::async_runtime::spawn(async move {
        let app_state: State<'_, std::sync::Arc<AppState>> = app_handle_emit.state();
        while let Some(event) = event_rx.recv().await {
            if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                let target = {
                    let owner: InteractionOwner = app_state.owner.load(Ordering::Relaxed).into();
                    match owner {
                        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                        InteractionOwner::Tray => "tray",
                        InteractionOwner::Wizard => "wizard",
                    }
                };
                let _ = app_handle_emit.emit_to(target, msg_type, &event);
            }
        }
    });

    let audio_stream = AudioStream::new(producer, input_device).map_err(|e| e.to_string())?;
    audio_stream.start().map_err(|e| e.to_string())?;

    app.emit(crate::core::constants::EVENT_MODEL_LOADING, "TTS")
        .ok();
    let (en_tts_dir, hi_tts_path, llm_path) = {
        let (hi_voice, llm_model) = {
            let settings = state.settings.read().unwrap();
            (settings.tts.hi_voice.clone(), settings.llm.model.clone())
        };
        let models_dir = paths::get().models.clone();
        let manifest_lock = state.manifest.read().await;

        let en_tts = if let Some(ref manifest) = *manifest_lock {
            if let Some(group) = manifest
                .model_groups
                .iter()
                .find(|g| g.id == "kokoro_english_tts")
            {
                if let Some(file) = group.files.first() {
                    let path = models_dir.join(&file.path);
                    path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| {
                        models_dir.join(crate::core::constants::MODEL_DIR_TTS_EN)
                    })
                } else {
                    models_dir.join(crate::core::constants::MODEL_DIR_TTS_EN)
                }
            } else {
                models_dir.join(crate::core::constants::MODEL_DIR_TTS_EN)
            }
        } else {
            models_dir.join(crate::core::constants::MODEL_DIR_TTS_EN)
        };

        let hi_tts = if let Some(ref manifest) = *manifest_lock {
            if let Some(group) = manifest
                .model_groups
                .iter()
                .find(|g| g.id == "piper_hindi_tts")
            {
                if let Some(file) = group.files.first() {
                    let path = models_dir.join(&file.path);
                    path.parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| {
                            models_dir.join(crate::core::constants::MODEL_DIR_TTS_HI)
                        })
                        .join(&hi_voice)
                } else {
                    models_dir
                        .join(crate::core::constants::MODEL_DIR_TTS_HI)
                        .join(&hi_voice)
                }
            } else {
                models_dir
                    .join(crate::core::constants::MODEL_DIR_TTS_HI)
                    .join(&hi_voice)
            }
        } else {
            models_dir
                .join(crate::core::constants::MODEL_DIR_TTS_HI)
                .join(&hi_voice)
        };

        let llm = if let Some(ref manifest) = *manifest_lock {
            if let Some(group) = manifest.model_groups.iter().find(|g| g.id == llm_model) {
                if let Some(file) = group.files.first() {
                    models_dir.join(&file.path)
                } else {
                    models_dir
                        .join(crate::core::constants::MODEL_DIR_LLM)
                        .join(crate::core::constants::MODEL_FILE_LLM_GGUF)
                }
            } else {
                models_dir
                    .join(crate::core::constants::MODEL_DIR_LLM)
                    .join(crate::core::constants::MODEL_FILE_LLM_GGUF)
            }
        } else {
            models_dir
                .join(crate::core::constants::MODEL_DIR_LLM)
                .join(crate::core::constants::MODEL_FILE_LLM_GGUF)
        };

        (en_tts, hi_tts, llm)
    };

    let playback_energy = Arc::new(AtomicU32::new(0f32.to_bits()));

    let playback_engine = match PlaybackEngine::new(
        std::sync::Arc::clone(&state.pipeline.playback_active),
        std::sync::Arc::clone(&state.pipeline.cancel_flag),
        Arc::clone(&playback_energy),
        std::sync::Arc::clone(&state.pipeline.playback_underruns),
        std::sync::Arc::clone(&state.pipeline.is_assistant_speaking),
    ) {
        Ok(pe) => std::sync::Arc::new(pe),
        Err(e) => {
            log::error!(
                "[Pipeline] PlaybackEngine init failed: {} — TTS output disabled",
                e
            );
            return Ok(());
        }
    };

    // Pipeline Orchestrator now takes Arc<RwLock<VoxSettings>> and the pre-resolved llm_path
    let orchestrator = PipelineOrchestrator::new(
        std::sync::Arc::clone(&state.pipeline.cancel_flag),
        std::sync::Arc::clone(&state.pipeline.playback_active),
        std::sync::Arc::clone(&state.pipeline.tts_generating),
        std::sync::Arc::clone(&state.pipeline.turn_id),
        std::sync::Arc::clone(&state.pipeline.state),
        vox_event_tx.clone(),
        Arc::clone(&state.settings),
        llm_path,
        std::sync::Arc::clone(&state.pipeline.is_engaged),
        std::sync::Arc::clone(&state.pipeline.transcript_history),
        std::sync::Arc::clone(&state.conversation_id),
        state.persist_tx.lock().unwrap().clone(),
        std::sync::Arc::clone(&state.dropped_persistence_events),
        std::sync::Arc::clone(&state.latest_voice_latency_ms),
        std::sync::Arc::clone(&state.latest_tts_rtf),
        std::sync::Arc::clone(&state.latest_playback_start_ms),
        std::sync::Arc::clone(&state.is_llm_loaded),
        std::sync::Arc::clone(&state.is_tts_loaded),
        std::sync::Arc::clone(&state.is_sleeping),
    );

    let playback_for_orch = std::sync::Arc::clone(&playback_engine);
    let app_for_orch = app.clone();
    let orchestrator_handle = std::thread::Builder::new()
        .name("vox-pipeline".to_string())
        .spawn(move || {
            orchestrator.run_event_loop(
                vox_event_rx,
                en_tts_dir,
                hi_tts_path,
                playback_for_orch,
                app_for_orch,
            );
        })
        .map_err(|e| e.to_string())?;

    log::info!("[Pipeline] Phase 4 pipeline online (LLM + TTS + Playback)");

    *lock = Some(VoxEngine {
        audio_stream,
        stt_tx: stt_tx_internal,
        vad_tx: vad_tx_internal,
        telemetry_tx: state.telemetry_tx.clone(),
        pipeline_tx: vox_event_tx.clone(),
        playback_engine,
        stt_handle: Some(stt_handle),
        vad_handle: Some(vad_handle),
        orchestrator_handle: Some(orchestrator_handle),
    });

    Ok(())
}
