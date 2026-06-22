use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VoxEngine};
use crate::services::audio::AudioStream;
use crate::services::pipeline::PipelineOrchestrator;
use crate::services::audio::PlaybackEngine;
use crate::services::stt::providers::create_stt_provider;
use crate::services::stt::{spawn_stt_worker, SttCommand};
use crate::services::vad::{
    earshot_vad::EarshotVadEngine, ten_onnx::VadEngine as TenVadEngine, VadBackend,
};
use crate::tray::position_tray_window;
use crate::utils::paths;
use ringbuf::traits::Split;
use std::sync::atomic::Ordering;
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

        // Reset all loaded model atomic states immediately
        state.is_vad_loaded.store(false, Ordering::Relaxed);
        state.is_stt_loaded.store(false, Ordering::Relaxed);
        state.is_llm_loaded.store(false, Ordering::Relaxed);
        state.is_tts_loaded.store(false, Ordering::Relaxed);
        state
            .pipeline
            .current_state_atomic
            .store(InteractionState::Idle as u32, Ordering::Relaxed);
        if let Ok(mut state_lock) = state.pipeline.state.lock() {
            *state_lock = InteractionState::Idle;
        }

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
        state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
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
        state.owner.store(
            crate::core::state::InteractionOwner::Tray as u32,
            Ordering::Relaxed,
        );

        if let Some(engine) = state.engine.lock().await.as_ref() {
            let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);
            let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id });
            let _ = engine.stt_tx.send(SttCommand::ResetStream);
            engine.playback_engine.cancel();
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(
                    InteractionOwner::Tray,
                ));

            // Persist Session End
            let conv_id = state.conversation_id.swap(0, Ordering::Relaxed);
            if conv_id != 0 {
                log::info!(
                    "[Session] <<< USER SESSION ENDED (User Disengaged): id={}",
                    conv_id
                );
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

    let (
        _stt_model_path,
        stt_provider,
        vad_model_path_opt,
        vad_backend_opt,
        input_device,
    ) = {
        let (vad_backend, asr_provider, _tray_enabled, input_device) = {
            let settings = state.settings.read().unwrap();
            (
                settings.vad.vad_backend.clone(),
                settings.asr.provider.clone(),
                settings.ui.tray_enabled,
                settings.audio.input_device.clone(),
            )
        };
        let models_dir = paths::get().models.clone();
        let manifest_lock = state.manifest.read().await;

        // Resolve STT model path and create the provider (always pre-loaded)
        app.emit(crate::core::constants::EVENT_MODEL_LOADING, "STT")
            .ok();
        let (stt_path, p) = match asr_provider {
            crate::core::settings::SttProviderConfig::Embedded { ref model_type } => {
                let path = match model_type.as_str() {
                    "nvidia_nemotron" => {
                        models_dir.join(crate::core::constants::MODEL_DIR_STT_NEMOTRON)
                    }
                    _ => models_dir.join(crate::core::constants::MODEL_DIR_STT),
                };
                match create_stt_provider(&asr_provider, &path) {
                    Ok(provider) => {
                        app.emit(crate::core::constants::EVENT_MODEL_READY, "STT")
                            .ok();
                        (path, provider)
                    }
                    Err(e) => {
                        app.emit(
                            crate::core::constants::EVENT_MODEL_FAILED,
                            format!("STT: {}", e),
                        )
                        .ok();
                        return Err(format!("[Pipeline] Failed to create STT provider: {}", e));
                    }
                }
            }
            crate::core::settings::SttProviderConfig::Cloud { .. } => {
                // For cloud providers, model_path is not used (auth is configured separately)
                let path = models_dir.join("stt");
                match create_stt_provider(&asr_provider, &path) {
                    Ok(provider) => {
                        app.emit(crate::core::constants::EVENT_MODEL_READY, "STT")
                            .ok();
                        (path, provider)
                    }
                    Err(e) => {
                        app.emit(
                            crate::core::constants::EVENT_MODEL_FAILED,
                            format!("STT: {}", e),
                        )
                        .ok();
                        return Err(format!("[Pipeline] Failed to create STT provider: {}", e));
                    }
                }
            }
        };

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

        (
            stt_path,
            p,
            vad_path,
            vad_backend,
            input_device,
        )
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    let (stt_tx_internal, stt_rx_internal) = std::sync::mpsc::channel::<SttCommand>();
    let (vad_tx_internal, vad_rx_internal) =
        std::sync::mpsc::channel::<crate::core::state::VadCommand>();
    let (vox_event_tx, vox_event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let stt_handle = spawn_stt_worker(
        app.clone(),
        stt_rx_internal,
        stt_provider,
        Some(vox_event_tx.clone()),
        state.pipeline.cancel_flag.clone(),
        state.is_stt_loaded.clone(),
        state.pipeline.engine_shutdown.clone(),
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

    let (super_tts_path, llm_path) = {
        let llm_model = {
            let settings = state.settings.read().unwrap();
            settings.llm.model.clone()
        };
        let models_dir = paths::get().models.clone();
        let manifest_lock = state.manifest.read().await;

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

        let super_tts = models_dir.join(crate::core::constants::MODEL_DIR_TTS_SUPER);

        (super_tts, llm)
    };

    let playback_energy = state.latest_playback_energy.clone();
    let playback_low = state.latest_playback_low.clone();
    let playback_mid = state.latest_playback_mid.clone();
    let playback_high = state.latest_playback_high.clone();

    let playback_engine = match PlaybackEngine::new(
        std::sync::Arc::clone(&state.pipeline.playback_active),
        std::sync::Arc::clone(&state.pipeline.cancel_flag),
        Arc::clone(&playback_energy),
        Arc::clone(&playback_low),
        Arc::clone(&playback_mid),
        Arc::clone(&playback_high),
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

    // Pipeline Orchestrator now takes Arc<RwLock<VoxSettings>> and the pre-resolved llm_path + super_tts_path
    let orchestrator = PipelineOrchestrator::new(
        std::sync::Arc::clone(&state.pipeline.cancel_flag),
        std::sync::Arc::clone(&state.pipeline.is_paused),
        std::sync::Arc::clone(&state.pipeline.playback_active),
        std::sync::Arc::clone(&state.pipeline.tts_generating),
        std::sync::Arc::clone(&state.pipeline.turn_id),
        std::sync::Arc::clone(&state.pipeline.state),
        vox_event_tx.clone(),
        Arc::clone(&state.settings),
        llm_path,
        super_tts_path,
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

// ─── Test Clip / WAV Decoding ─────────────────────────────────────────────

/// Decode a WAV file to mono f32 samples.
/// Handles both integer and float sample formats. Stereo is averaged to mono.
fn decode_wav_to_mono_f32(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV '{}': {}", path.display(), e))?;
    let spec = reader.spec();

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => {
            // Normalise integer samples to [-1.0, 1.0]
            let max_val = (2u64.pow(spec.bits_per_sample as u32) / 2 - 1) as f64;
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| (s as f64 / max_val) as f32)
                .collect()
        }
    };

    // Average channels to mono
    let mono: Vec<f32> = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };

    log::info!(
        "[TestClip] Decoded WAV: {} samples, {} channels, {} Hz, {} bits",
        mono.len(),
        spec.channels,
        spec.sample_rate,
        spec.bits_per_sample,
    );

    Ok(mono)
}

/// Inject a pre-recorded test clip into the pipeline as if the user spoke it.
///
/// The clip is decoded from a bundled WAV resource, then sent directly to the
/// STT worker as a `SttCommand::Final`, bypassing VAD. If the engine is not
/// running, it is auto-launched first.
#[tauri::command]
pub async fn test_clip(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
    clip_id: String,
) -> Result<(), String> {
    log::info!("[TestClip] Requested: {}", clip_id);

    // 1. Resolve bundled clip path from Tauri resource directory
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?
        .join("test-clips")
        .join(format!("{}.wav", clip_id));

    // Fallback: relative to CARGO_MANIFEST_DIR for dev mode
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test-clips")
        .join(format!("{}.wav", clip_id));

    let clip_path = if resource_path.exists() {
        resource_path
    } else if dev_path.exists() {
        log::info!("[TestClip] Using dev path: {:?}", dev_path);
        dev_path
    } else {
        return Err(format!(
            "Test clip '{}' not found at {:?} or {:?}",
            clip_id, resource_path, dev_path
        ));
    };

    // 2. Decode WAV to mono f32
    let audio = decode_wav_to_mono_f32(&clip_path)?;

    if audio.is_empty() {
        return Err("Decoded audio is empty".to_string());
    }

    log::info!(
        "[TestClip] Decoded {} samples from '{}'",
        audio.len(),
        clip_id
    );

    // 3. Auto-launch engine if not running
    {
        let engine_lock = state.engine.lock().await;
        if engine_lock.is_none() {
            drop(engine_lock);
            log::info!("[TestClip] Engine not running. Launching...");
            launch_engine(app.clone()).await?;
        }
    }

    // 4. Set owner to MainWindow so state_changed / llm_token events route to main window
    state.owner.store(
        InteractionOwner::MainWindow as u32,
        std::sync::atomic::Ordering::Relaxed,
    );
    state
        .pipeline
        .is_engaged
        .store(true, std::sync::atomic::Ordering::Relaxed);

    // Re-acquire engine lock and send clip into the pipeline
    let engine_lock = state.engine.lock().await;
    let engine = engine_lock
        .as_ref()
        .ok_or_else(|| "Engine failed to start after launch".to_string())?;

    let _ = engine
        .vad_tx
        .send(crate::core::state::VadCommand::UpdateOwner(
            InteractionOwner::MainWindow,
        ));

    // Generate a unique turn_id (bump atomic to avoid collision with VAD)
    let turn_id = state
        .pipeline
        .turn_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;

    // WarmUp first — spawns LLM + TTS workers so they're ready when STT finishes
    let _ = engine.pipeline_tx.send(VoxEvent::WarmUp);

    // Emit SpeechStart so the pipeline sets up turn state
    let _ = engine.pipeline_tx.send(VoxEvent::SpeechStart {
        turn_id,
        owner: InteractionOwner::MainWindow,
    });

    // Send the audio as a Final STT command (bypasses VAD completely)
    engine
        .stt_tx
        .send(SttCommand::Final(
            turn_id,
            InteractionOwner::MainWindow,
            audio,
        ))
        .map_err(|e| format!("STT channel closed: {}", e))?;

    log::info!(
        "[TestClip] Injected turn_id={} into pipeline (WarmUp sent)",
        turn_id
    );

    Ok(())
}

/// Cancel a running test clip — flushes the pipeline (cancel flag + Cancelled event + STT ResetStream).
#[tauri::command]
pub async fn test_clip_cancel(
    _app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    log::info!("[TestClip] Cancel requested — flushing pipeline.");

    state
        .pipeline
        .cancel_flag
        .store(true, std::sync::atomic::Ordering::Relaxed);
    state
        .pipeline
        .is_engaged
        .store(false, std::sync::atomic::Ordering::Relaxed);

    if let Some(engine) = state.engine.lock().await.as_ref() {
        let turn_id = state
            .pipeline
            .turn_id
            .load(std::sync::atomic::Ordering::Relaxed);
        let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id });
        let _ = engine.stt_tx.send(SttCommand::ResetStream);
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::UpdateOwner(
                InteractionOwner::Tray,
            ));
    } else {
        log::warn!("[TestClip] No engine to cancel.");
    }

    Ok(())
}

pub async fn start_realtime_session_internal(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<(), String> {
    log::info!("[IPC] start_realtime_session_internal requested");

    // 1. Ensure engine is running
    let engine_guard = state.engine.lock().await;
    let engine = match &*engine_guard {
        Some(e) => e,
        None => {
            return Err("Audio engine is not running. Please start the engine first.".to_string())
        }
    };

    // Update owner to MainWindow and sync VAD immediately
    state.owner.store(
        crate::core::state::InteractionOwner::MainWindow as u32,
        std::sync::atomic::Ordering::Relaxed,
    );
    let _ = engine
        .vad_tx
        .send(crate::core::state::VadCommand::UpdateOwner(
            crate::core::state::InteractionOwner::MainWindow,
        ));

    // 2. Clear any existing realtime engine
    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut old_rt) = rt_guard.take() {
        log::info!("[IPC] Stopping existing realtime session...");
        old_rt.stop();
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::StopRealtime);
    }

    // 3. Load settings to determine active provider
    let settings = state.settings.read().unwrap().clone();
    let mut gemini_config = settings.realtime.gemini.clone();

    // Try to load resumption token from cache
    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
    let mut resumed = false;
    if cache_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<serde_json::Value>(&data) {
                let expires_at = cached["expires_at"].as_u64().unwrap_or(0);
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                let cached_model = cached["model"].as_str().unwrap_or("");
                let cached_handle = cached["handle"].as_str().unwrap_or("");

                if expires_at > now_ms && cached_model == gemini_config.model && !cached_handle.is_empty() {
                    log::info!(
                        "[Realtime] Found valid session resumption token, using handle: {}",
                        if cached_handle.len() > 12 {
                            &cached_handle[..12]
                        } else {
                            cached_handle
                        }
                    );
                    gemini_config.resume_handle = Some(cached_handle.to_string());
                    resumed = true;
                }
            }
        }
    }

    let provider: Box<dyn crate::services::realtime::RealtimeVoiceProvider> = match settings
        .realtime
        .provider
    {
        crate::core::settings::RealtimeProviderKind::GeminiLive => Box::new(
            crate::services::realtime::providers::gemini_live::GeminiLiveProvider::new(
                gemini_config,
                settings.assistant.realtime_prompt.clone(),
                state.pipeline.is_paused.clone(),
            ),
        ),
        crate::core::settings::RealtimeProviderKind::OpenAiRealtime => {
            return Err("OpenAI Realtime provider is not yet implemented".to_string());
        }
        crate::core::settings::RealtimeProviderKind::DeepgramVoiceAgent => Box::new(
            crate::services::realtime::providers::deepgram_live::DeepgramVoiceAgentProvider::new(
                settings.realtime.deepgram.clone(),
                settings.assistant.realtime_prompt.clone(),
                state.pipeline.is_paused.clone(),
            ),
        ),
        crate::core::settings::RealtimeProviderKind::ElevenLabsConvai => {
            return Err("ElevenLabs Conversational AI provider is not yet implemented".to_string());
        }
    };

    let owner: crate::core::state::InteractionOwner = state
        .owner
        .load(std::sync::atomic::Ordering::Relaxed)
        .into();
    let interaction_mode = match owner {
        crate::core::state::InteractionOwner::Tray => settings.interaction.tray_mode.clone(),
        crate::core::state::InteractionOwner::MainWindow
        | crate::core::state::InteractionOwner::Ptt => settings.interaction.main_app_mode.clone(),
        crate::core::state::InteractionOwner::Wizard => {
            crate::core::settings::InteractionMode::Passive
        }
    };

    // 4. Create and start RealtimeEngine
    let is_ptt = interaction_mode == crate::core::settings::InteractionMode::PTT;
    log::info!(
        "[IPC] Starting realtime session: interaction_mode={:?}, is_ptt={}",
        interaction_mode,
        is_ptt
    );
    let tokio_handle = tokio::runtime::Handle::current();
    let mut rt_engine =
        crate::services::realtime::engine::RealtimeEngine::new(provider, tokio_handle);

    let playback_engine = engine.playback_engine.clone();
    let event_tx = engine.pipeline_tx.clone();

    rt_engine
        .start(interaction_mode, playback_engine, event_tx)
        .map_err(|e| format!("Failed to start realtime session: {}", e))?;

    // Get the audio sender and wire it into the VAD actor
    let audio_tx = rt_engine
        .get_audio_sender()
        .ok_or_else(|| "Failed to get realtime audio sender".to_string())?;

    // 5. Update pipeline mode to Realtime in settings
    let current_settings = {
        let mut settings_write = state.settings.write().unwrap();
        settings_write.interaction.pipeline_mode = crate::core::settings::PipelineMode::Realtime;
        let _ = settings_write.save();
        settings_write.clone()
    };

    // 6. Propagate settings update to the pipeline event loop
    let _ = engine
        .pipeline_tx
        .send(crate::core::events::VoxEvent::SettingsUpdated(
            current_settings,
        ));

    // 7. Tell VAD to start routing chunks — pass is_ptt so it applies the
    //    correct routing policy (gated vs. unconditional).
    log::info!(
        "[IPC] Sending StartRealtime to VAD actor (is_ptt={})",
        is_ptt
    );
    let _ = engine
        .vad_tx
        .send(crate::core::state::VadCommand::StartRealtime { tx: audio_tx, is_ptt });

    // Update backend engagement state
    state.pipeline.is_engaged.store(true, std::sync::atomic::Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, std::sync::atomic::Ordering::Relaxed);

    *rt_guard = Some(rt_engine);

    // Spawn 10-minute active idle timeout check task
    let app_clone = app.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        const TIMEOUT_MS: u64 = 10 * 60 * 1000; // 10 minutes
        const WARN_1_MS: u64 = TIMEOUT_MS - 15_000; // 9m 45s (15s warning)
        const WARN_2_MS: u64 = TIMEOUT_MS - 5_000;  // 9m 55s (5s warning)

        loop {
            // Check if still engaged in realtime mode
            let is_engaged = state_clone.pipeline.is_engaged.load(std::sync::atomic::Ordering::Relaxed);
            let is_realtime = {
                let settings = state_clone.settings.read().unwrap();
                settings.interaction.pipeline_mode == crate::core::settings::PipelineMode::Realtime
            };
            
            if !is_engaged || !is_realtime {
                break;
            }

            // Get last activity time
            let last_activity = {
                let rt_guard = state_clone.realtime_engine.lock().await;
                if let Some(rt_engine) = &*rt_guard {
                    rt_engine.last_activity_time()
                } else {
                    0
                }
            };

            if last_activity > 0 {
                let now = chrono::Utc::now().timestamp_millis() as u64;
                let elapsed = now.saturating_sub(last_activity);

                if elapsed >= TIMEOUT_MS {
                    log::warn!("[Realtime] S2S session idle for over 10 minutes. Triggering idle timeout.");
                    
                    // Stop the session internally
                    let mut rt_guard = state_clone.realtime_engine.lock().await;
                    if let Some(mut rt_engine) = rt_guard.take() {
                        rt_engine.stop();
                    }
                    drop(rt_guard);

                    // Update backend engagement state
                    state_clone.pipeline.is_engaged.store(false, std::sync::atomic::Ordering::Relaxed);

                    // Tell VAD to stop routing chunks
                    if let Some(engine) = state_clone.engine.lock().await.as_ref() {
                        let _ = engine
                            .vad_tx
                            .send(crate::core::state::VadCommand::StopRealtime);
                    }

                    // Delete session cache file
                    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
                    if cache_path.exists() {
                        let _ = std::fs::remove_file(cache_path);
                    }



                    // Emit event to frontend
                    let _ = app_clone.emit_to("main", "realtime_session_ended", "idle_timeout".to_string());
                    break;
                } else if elapsed >= WARN_2_MS {
                    let remaining = 600 - (elapsed / 1000);
                    let _ = app_clone.emit_to("main", "realtime_idle_warning", serde_json::json!({ "seconds_remaining": remaining }));
                    tokio::time::sleep(std::time::Duration::from_millis(TIMEOUT_MS - elapsed)).await;
                } else if elapsed >= WARN_1_MS {
                    let remaining = 600 - (elapsed / 1000);
                    let _ = app_clone.emit_to("main", "realtime_idle_warning", serde_json::json!({ "seconds_remaining": remaining }));
                    tokio::time::sleep(std::time::Duration::from_millis(WARN_2_MS - elapsed)).await;
                } else {
                    let time_until_warning = WARN_1_MS - elapsed;
                    tokio::time::sleep(std::time::Duration::from_millis(time_until_warning)).await;
                }
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(WARN_1_MS)).await;
            }
        }
    });

    if resumed {
        let _ = app.emit_to("main", "realtime_session_resumed", ());
    } else {
        let _ = app.emit_to("main", "realtime_session_started", ());
    }

    log::info!("[IPC] Realtime S2S session started successfully.");
    Ok(())
}

/// Start the real-time speech-to-speech session with the active cloud provider.
#[tauri::command]
pub async fn start_realtime_session(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    if let Err(e) = start_realtime_session_internal(&app, &state).await {
        log::error!("[IPC] start_realtime_session failed: {}", e);
        return Err(e);
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct RealtimeSessionCache {
    pub has_session: bool,
    pub provider: String,
    pub expires_in_seconds: i64,
    pub model: String,
}

#[tauri::command]
pub async fn get_realtime_session_cache() -> Result<RealtimeSessionCache, String> {
    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
    if cache_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<serde_json::Value>(&data) {
                let expires_at = cached["expires_at"].as_u64().unwrap_or(0);
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                let provider = cached["provider"].as_str().unwrap_or("").to_string();
                let model = cached["model"].as_str().unwrap_or("").to_string();
                let expires_in_seconds = (expires_at as i64 - now_ms as i64) / 1000;
                
                return Ok(RealtimeSessionCache {
                    has_session: expires_in_seconds > 0,
                    provider,
                    expires_in_seconds,
                    model,
                });
            }
        }
    }
    
    Ok(RealtimeSessionCache {
        has_session: false,
        provider: String::new(),
        expires_in_seconds: 0,
        model: String::new(),
    })
}

/// Stop the active real-time speech-to-speech session and restore modular mode.
#[tauri::command]
pub async fn stop_realtime_session(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    log::info!("[IPC] stop_realtime_session requested");

    // 1. Tell VAD to stop routing chunks and cancel active playback
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::StopRealtime);
        engine.playback_engine.cancel();
    }

    // 2. Stop and drop the realtime engine
    let mut rt_guard = state.realtime_engine.lock().await;
    if let Some(mut rt_engine) = rt_guard.take() {
        rt_engine.stop();
    }

    // Update backend engagement state and owner
    state.pipeline.is_engaged.store(false, std::sync::atomic::Ordering::Relaxed);
    state.owner.store(crate::core::state::InteractionOwner::Tray as u32, std::sync::atomic::Ordering::Relaxed);
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::UpdateOwner(
                crate::core::state::InteractionOwner::Tray,
            ));
    }

    // Delete session cache file
    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
    if cache_path.exists() {
        let _ = std::fs::remove_file(cache_path);
    }

    // Emit event to frontend
    let _ = app.emit_to("main", "realtime_session_ended", "user".to_string());



    log::info!("[IPC] Realtime S2S session stopped successfully.");
    Ok(())
}

#[tauri::command]
pub async fn pause_pipeline(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    log::info!("[IPC] pause_pipeline requested");

    let is_paused = state.pipeline.is_paused.load(Ordering::SeqCst);
    if is_paused {
        return Ok(());
    }

    state.pipeline.is_paused.store(true, Ordering::SeqCst);
    state.pipeline.cancel_flag.store(true, Ordering::SeqCst);

    let engine_guard = state.engine.lock().await;
    if let Some(engine) = &*engine_guard {
        engine.playback_engine.cancel();
    }

    let (pipeline_mode, owner) = {
        let settings = state.settings.read().unwrap();
        (
            settings.interaction.pipeline_mode.clone(),
            state.owner.load(Ordering::Relaxed).into(),
        )
    };

    if pipeline_mode == crate::core::settings::PipelineMode::Realtime {
        let rt_guard = state.realtime_engine.lock().await;
        if let Some(rt_engine) = &*rt_guard {
            let _ = rt_engine.activity_end();
        }
    } else {
        if let Some(engine) = &*engine_guard {
            let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);
            let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id });
            let _ = engine.stt_tx.send(SttCommand::ResetStream);
        }
    }

    let target = match owner {
        InteractionOwner::Tray => "tray",
        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
        InteractionOwner::Wizard => "wizard",
    };
    let _ = app.emit_to(target, "pipeline_paused", ());

    state.pipeline.update_interaction_state(InteractionState::Idle, owner, &app);

    Ok(())
}

#[tauri::command]
pub async fn resume_pipeline(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    log::info!("[IPC] resume_pipeline requested");

    let is_paused = state.pipeline.is_paused.load(Ordering::SeqCst);
    if !is_paused {
        return Ok(());
    }

    state.pipeline.is_paused.store(false, Ordering::SeqCst);

    let (pipeline_mode, interaction_mode, owner) = {
        let settings = state.settings.read().unwrap();
        let owner: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
        let mode = match owner {
            InteractionOwner::Tray => settings.interaction.tray_mode.clone(),
            InteractionOwner::MainWindow | InteractionOwner::Ptt => settings.interaction.main_app_mode.clone(),
            InteractionOwner::Wizard => crate::core::settings::InteractionMode::Passive,
        };
        (
            settings.interaction.pipeline_mode.clone(),
            mode,
            owner,
        )
    };

    if pipeline_mode == crate::core::settings::PipelineMode::Modular {
        state.pipeline.cancel_flag.store(false, Ordering::SeqCst);
        let engine_guard = state.engine.lock().await;
        if let Some(engine) = &*engine_guard {
            let _ = engine.pipeline_tx.send(VoxEvent::WarmUp);
        }
    } else if pipeline_mode == crate::core::settings::PipelineMode::Realtime {
        let is_connected = {
            let rt_guard = state.realtime_engine.lock().await;
            if let Some(rt_engine) = &*rt_guard {
                rt_engine.is_connected()
            } else {
                false
            }
        };

        if !is_connected {
            log::info!("[IPC] Realtime S2S session is disconnected during pause. Reconnecting lazily on resume.");
            if let Err(e) = start_realtime_session_internal(&app, &state).await {
                log::error!("[IPC] Lazy S2S reconnection failed: {}", e);
                return Err(e);
            }
        }
    }

    let target = match owner {
        InteractionOwner::Tray => "tray",
        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
        InteractionOwner::Wizard => "wizard",
    };
    let _ = app.emit_to(target, "pipeline_resumed", ());

    let next_state = match interaction_mode {
        crate::core::settings::InteractionMode::PTT => InteractionState::Idle,
        crate::core::settings::InteractionMode::Passive => InteractionState::Listening,
    };
    state.pipeline.update_interaction_state(next_state, owner, &app);

    Ok(())
}
