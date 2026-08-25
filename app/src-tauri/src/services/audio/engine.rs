use crate::core::constants::{EVENT_MODEL_FAILED, EVENT_MODEL_LOADING, EVENT_MODEL_READY};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand, VoxEngine};
use crate::services::audio::{AudioStream, PlaybackEngine};
use crate::services::pipeline::router::spawn_router;
use crate::services::stt::providers::create_stt_provider;
use crate::services::stt::{spawn_stt_worker, SttCommand};
use crate::services::vad::{
    earshot_vad::EarshotVadEngine, ten_onnx::VadEngine as TenVadEngine, VadBackend,
};
use crate::utils::paths;
use ringbuf::traits::Split;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Ensures the persistence background worker is active and holds a valid channel.
fn ensure_persistence_worker(state: &AppState) {
    let mut persist_lock = state.persist_tx.lock();
    if persist_lock.is_none() {
        log::info!("[AudioEngine] Spawning persistence worker");
        let tx = crate::persistence::worker::spawn_persistence_worker(
            paths::get().db.clone(),
            Arc::clone(&state.is_db_healthy),
            Arc::clone(&state.latest_persistence_rate),
            Arc::clone(&state.is_private_mode),
        );
        *persist_lock = Some(tx);
    }
}

/// Ensures the models manifest is loaded into memory from disk.
async fn ensure_manifest_loaded(state: &AppState) {
    let mut m = state.manifest.write().await;
    if m.is_none() {
        let manifest_path = paths::get().models.join("models_manifest.json");
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

/// Resolves and instantiates the active STT provider.
fn create_stt_instance(
    app: &AppHandle,
    state: &AppState,
) -> Result<Box<dyn crate::services::stt::providers::SttProvider>, String> {
    let asr_provider = state.settings.read().unwrap().stt.to_provider_config();
    let models_dir = paths::get().models.clone();

    match asr_provider {
        crate::core::settings::SttProviderConfig::Embedded { ref model_type } => {
            let path = match model_type.as_str() {
                "nvidia_nemotron" => models_dir.join(crate::services::stt::MODEL_DIR_STT_NEMOTRON),
                _ => models_dir.join(crate::services::stt::MODEL_DIR_STT_QWEN),
            };
            create_stt_provider(&asr_provider, &path).map_err(|e| {
                let _ = app.emit(EVENT_MODEL_FAILED, format!("STT: {}", e));
                format!("[AudioEngine] STT provider creation failed: {}", e)
            })
        }
        crate::core::settings::SttProviderConfig::Cloud { .. } => {
            let path = models_dir.join("stt");
            create_stt_provider(&asr_provider, &path).map_err(|e| {
                let _ = app.emit(EVENT_MODEL_FAILED, format!("STT: {}", e));
                format!("[AudioEngine] STT provider creation failed: {}", e)
            })
        }
    }
}

/// Resolves and instantiates the active VAD backend engine.
async fn create_vad_instance(app: &AppHandle, state: &AppState) -> Result<VadBackend, String> {
    let (vad_backend, threshold) = {
        let s = state.settings.read().unwrap();
        (s.vad.backend.clone(), s.vad.threshold)
    };

    match vad_backend {
        crate::core::settings::VadBackendOption::Earshot => {
            log::info!("[AudioEngine] Initializing Earshot VAD");
            let engine = EarshotVadEngine::new(threshold).map_err(|e| {
                let _ = app.emit(EVENT_MODEL_FAILED, format!("VAD: {}", e));
                e.to_string()
            })?;
            let _ = app.emit(EVENT_MODEL_READY, "VAD");
            Ok(VadBackend::Earshot(engine))
        }
        crate::core::settings::VadBackendOption::TenVad => {
            let models_dir = paths::get().models.clone();
            let manifest_lock = state.manifest.read().await;
            let vad_path = manifest_lock
                .as_ref()
                .and_then(|m| m.model_groups.iter().find(|g| g.id == "ten_vad"))
                .and_then(|g| g.files.first())
                .map(|f| models_dir.join(&f.path))
                .unwrap_or_else(|| {
                    models_dir
                        .join(crate::services::vad::MODEL_DIR_VAD)
                        .join(crate::services::vad::MODEL_FILE_VAD)
                });

            log::info!("[AudioEngine] Initializing TenVAD at {:?}", vad_path);
            let engine = TenVadEngine::new(&vad_path, threshold).map_err(|e| {
                let _ = app.emit(EVENT_MODEL_FAILED, format!("VAD: {}", e));
                e.to_string()
            })?;
            let _ = app.emit(EVENT_MODEL_READY, "VAD");
            Ok(VadBackend::Ten(engine))
        }
    }
}

/// Instantiates the CPAL playback engine with shared telemetry atomics.
fn create_playback_engine(state: &AppState) -> Result<Arc<PlaybackEngine>, String> {
    PlaybackEngine::new(
        Arc::clone(&state.pipeline.playback_active),
        Arc::clone(&state.pipeline.cancel_flag),
        Arc::clone(&state.latest_playback_energy),
        Arc::clone(&state.latest_playback_low),
        Arc::clone(&state.latest_playback_mid),
        Arc::clone(&state.latest_playback_high),
        Arc::clone(&state.pipeline.playback_underruns),
        Arc::clone(&state.pipeline.is_assistant_speaking),
    )
    .map(Arc::new)
    .map_err(|e| format!("[AudioEngine] PlaybackEngine initialization failed: {}", e))
}

/// Spawns background forwarder routing channel events to active Tauri webview windows.
fn spawn_event_forwarder(app: AppHandle, mut rx: tokio::sync::mpsc::Receiver<serde_json::Value>) {
    tauri::async_runtime::spawn(async move {
        let app_state: tauri::State<'_, Arc<AppState>> = app.state();
        while let Some(event) = rx.recv().await {
            if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                let owner: InteractionOwner = app_state.owner.load(Ordering::Relaxed).into();
                let target = match owner {
                    InteractionOwner::Assistant => "main",
                    InteractionOwner::Dictation => "tray",
                };
                if let Err(e) = app.emit_to(target, msg_type, &event) {
                    log::warn!("[AudioEngine] Failed to emit UI event {}: {}", msg_type, e);
                }
            }
        }
    });
}

/// Initializes and starts all real-time audio threads, STT/VAD actors, and central router.
pub async fn start_audio_engine(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let mut lock = state.engine.lock().await;
    if lock.is_some() {
        return Ok(());
    }

    log::info!("[AudioEngine] Starting 3-Tier Audio Engine");
    state
        .pipeline
        .engine_shutdown
        .store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    ensure_persistence_worker(state);
    ensure_manifest_loaded(state).await;

    let _ = app.emit(EVENT_MODEL_LOADING, "VAD");

    let stt_provider = create_stt_instance(app, state)?;
    let vad = create_vad_instance(app, state).await?;

    let (event_tx, event_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    let (stt_tx, stt_rx) = std::sync::mpsc::channel::<SttCommand>();
    let (vad_tx, vad_rx) = std::sync::mpsc::channel::<VadCommand>();
    let (vox_event_tx, vox_event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let stt_handle = spawn_stt_worker(
        app.clone(),
        stt_rx,
        stt_provider,
        Some(vox_event_tx.clone()),
        Arc::clone(&state.pipeline.cancel_flag),
        Arc::clone(&state.is_stt_loaded),
        Arc::clone(&state.pipeline.engine_shutdown),
    )?;

    let (producer, consumer) = ringbuf::HeapRb::<f32>::new(16000 * 4).split();

    let (threshold, noise_gate, mode, audio_mode) = {
        let settings = state.settings.read().unwrap();
        let owner: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
        let mode = match owner {
            InteractionOwner::Dictation => match settings.dictation.interaction_mode {
                crate::core::settings::DictationInteractionMode::Passive => {
                    crate::core::settings::InteractionMode::Passive
                }
                crate::core::settings::DictationInteractionMode::Ptt => {
                    crate::core::settings::InteractionMode::PTT
                }
            },
            InteractionOwner::Assistant => settings.interaction.mode.clone(),
        };
        (
            settings.vad.threshold,
            settings.vad.ptt_noise_gate,
            mode,
            settings.audio.output_mode.clone(),
        )
    };

    let app_vad = app.clone();
    let stt_vad_tx = stt_tx.clone();
    let telemetry_vad_tx = state.telemetry_tx.clone();
    let vox_vad_tx = vox_event_tx.clone();
    let is_vad_loaded = Arc::clone(&state.is_vad_loaded);
    let playback_active = Arc::clone(&state.pipeline.playback_active);
    let turn_id_atomic = Arc::clone(&state.pipeline.turn_id);
    let owner_atomic = Arc::clone(&state.owner);
    let is_dictation_enabled = Arc::clone(&state.is_dictation_enabled);
    let engine_shutdown = Arc::clone(&state.pipeline.engine_shutdown);
    let dropped_counter = Arc::clone(&state.dropped_telemetry_events);

    let vad_handle = std::thread::Builder::new()
        .name("vox-vad-worker".to_string())
        .spawn(move || {
            if let Err(e) = crate::services::vad::spawn_vad_actor(
                vad,
                app_vad,
                consumer,
                event_tx,
                stt_vad_tx,
                vad_rx,
                telemetry_vad_tx,
                Some(vox_vad_tx),
                is_vad_loaded,
                playback_active,
                turn_id_atomic,
                owner_atomic,
                is_dictation_enabled,
                engine_shutdown,
                dropped_counter,
                threshold,
                noise_gate,
                mode,
                audio_mode,
            ) {
                log::error!("[AudioEngine] VAD worker crashed: {}", e);
            }
        })
        .map_err(|e| e.to_string())?;

    spawn_event_forwarder(app.clone(), event_rx);

    let input_device = state.settings.read().unwrap().audio.input_device.clone();
    let audio_stream = AudioStream::new(producer, input_device).map_err(|e| e.to_string())?;
    audio_stream.start().map_err(|e| e.to_string())?;

    let playback_engine = create_playback_engine(state)?;
    let orchestrator_handle =
        spawn_router(app.clone(), vox_event_rx, Arc::clone(&playback_engine))?;

    *lock = Some(VoxEngine {
        audio_stream,
        stt_tx,
        vad_tx,
        llm_tx: None,
        tts_tx: None,
        telemetry_tx: state.telemetry_tx.clone(),
        pipeline_tx: vox_event_tx,
        playback_engine,
        stt_handle: Some(stt_handle),
        vad_handle: Some(vad_handle),
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: Some(orchestrator_handle),
    });

    log::info!("[AudioEngine] 3-Tier Audio Engine online");
    Ok(())
}

/// Shuts down all audio engine background threads, flushes persistence, and unloads models.
pub async fn stop_audio_engine(state: &AppState) -> Result<(), String> {
    let mut lock = state.engine.lock().await;
    if let Some(mut engine) = lock.take() {
        log::info!("[AudioEngine] Shutting down audio engine threads");

        state
            .pipeline
            .engine_shutdown
            .store(true, Ordering::Relaxed);
        state.is_vad_loaded.store(false, Ordering::Relaxed);
        state.is_stt_loaded.store(false, Ordering::Relaxed);
        state.is_llm_loaded.store(false, Ordering::Relaxed);
        state.is_tts_loaded.store(false, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Idle);

        if let Err(e) = engine.pipeline_tx.send(VoxEvent::Shutdown) {
            log::warn!("[AudioEngine] Failed to send Shutdown to pipeline: {}", e);
        }
        if let Err(e) = engine.stt_tx.send(SttCommand::Shutdown) {
            log::warn!("[AudioEngine] Failed to send Shutdown to STT: {}", e);
        }
        if let Err(e) = engine.vad_tx.send(VadCommand::Shutdown) {
            log::warn!("[AudioEngine] Failed to send Shutdown to VAD: {}", e);
        }

        crate::services::llm::actor::cool_down_llm(&mut engine.llm_tx);
        crate::services::tts::actor::cool_down_tts(&mut engine.tts_tx);

        if let Some(h) = engine.llm_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = engine.tts_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = engine.orchestrator_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = engine.stt_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = engine.vad_handle.take() {
            let _ = h.join();
        }

        {
            let mut persist_lock = state.persist_tx.lock();
            if let Some(tx) = persist_lock.take() {
                if let Err(e) = tx.send(crate::persistence::events::PersistenceEvent::Shutdown) {
                    log::warn!(
                        "[AudioEngine] Failed to send Shutdown to persistence: {}",
                        e
                    );
                }
            }
        }

        crate::services::memory::unload_all_onnx_models();
        crate::services::memory::trim_heap("stop_audio_engine");
        log::info!("[AudioEngine] Audio engine resources cleanly released");
    }
    Ok(())
}
