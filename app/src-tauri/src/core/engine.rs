use std::sync::atomic::Ordering;
use std::sync::Arc;

use ringbuf::traits::Split;
use tauri::{AppHandle, Emitter};

use crate::core::constants::{
    EVENT_MODEL_FAILED, EVENT_MODEL_LOADING, EVENT_MODEL_READY, RING_BUFFER_SIZE,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionState, VadCommand, VoxEngine};
use crate::services::audio::playback::PlaybackTelemetryHandles;
use crate::services::audio::{AudioStream, PlaybackEngine};
use crate::pipeline::router::spawn_router;
use crate::services::stt::actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand};
use crate::services::stt::providers::create_stt_provider;
use crate::services::vad::actor::{
    spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles,
};
use crate::services::vad::{
    earshot_vad::EarshotVadEngine, ten_onnx::VadEngine as TenVadEngine, VadBackend,
};
use crate::utils::paths;

/// Ensures the persistence background worker is active and holds a valid channel.
fn ensure_persistence_worker(state: &AppState) {
    let mut persist_lock = state.persist_tx.lock();
    if persist_lock.is_none() {
        log::info!("[Core::Engine] Spawning persistence worker");
        let tx = crate::persistence::worker::spawn_persistence_worker(
            paths::get().db.clone(),
            Arc::clone(&state.telemetry.is_db_healthy),
            Arc::clone(&state.telemetry.latest_persistence_rate),
            Arc::clone(&state.telemetry.is_private_mode),
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
fn create_stt_instance<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<Box<dyn crate::services::stt::providers::SttProvider>, String> {
    let asr_provider = state
        .settings
        .read()
        .map_err(|e| format!("[Core::Engine] Settings lock poisoned: {}", e))?
        .stt
        .to_provider_config();
    let models_dir = paths::get().models.clone();

    match asr_provider {
        crate::core::settings::SttProviderConfig::Embedded { ref model_type } => {
            let path = match model_type.as_str() {
                "nvidia_nemotron" => models_dir.join(crate::services::stt::NEMOTRON_MODEL_DIR),
                _ => models_dir.join(crate::services::stt::QWEN_ASR_MODEL_DIR),
            };
            create_stt_provider(&asr_provider, &path).map_err(|e| {
                if let Err(emit_err) = app.emit(EVENT_MODEL_FAILED, format!("STT: {}", e)) {
                    log::warn!("[Core::Engine] Failed to emit EVENT_MODEL_FAILED: {}", emit_err);
                }
                format!("[Core::Engine] STT provider creation failed: {}", e)
            })
        }
        crate::core::settings::SttProviderConfig::Cloud { .. } => {
            let path = models_dir.join("stt");
            create_stt_provider(&asr_provider, &path).map_err(|e| {
                if let Err(emit_err) = app.emit(EVENT_MODEL_FAILED, format!("STT: {}", e)) {
                    log::warn!("[Core::Engine] Failed to emit EVENT_MODEL_FAILED: {}", emit_err);
                }
                format!("[Core::Engine] STT provider creation failed: {}", e)
            })
        }
    }
}

/// Resolves and instantiates the active VAD backend engine.
async fn create_vad_instance<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<VadBackend, String> {
    let (vad_backend, threshold) = {
        let s = state
            .settings
            .read()
            .map_err(|e| format!("[Core::Engine] Settings lock poisoned: {}", e))?;
        (s.vad.vad_backend.clone(), s.vad.threshold)
    };

    match vad_backend {
        crate::core::settings::VadBackendOption::Earshot => {
            log::info!("[Core::Engine] Initializing pure-Rust Earshot VAD");
            EarshotVadEngine::new(threshold)
                .map(VadBackend::Earshot)
                .map_err(|e| format!("[Core::Engine] Earshot VAD init failed: {}", e))
        }
        crate::core::settings::VadBackendOption::TenVad => {
            let vad_path = paths::get()
                .models
                .join(crate::services::vad::MODEL_DIR_VAD)
                .join(crate::services::vad::MODEL_FILE_VAD);
            if !vad_path.exists() {
                log::warn!(
                    "[Core::Engine] Ten VAD model missing at {:?}. Falling back to Earshot.",
                    vad_path
                );
                return EarshotVadEngine::new(threshold)
                    .map(VadBackend::Earshot)
                    .map_err(|e| format!("[Core::Engine] Earshot VAD fallback failed: {}", e));
            }
            log::info!("[Core::Engine] Initializing Ten ONNX VAD from {:?}", vad_path);
            TenVadEngine::new(&vad_path, threshold)
                .map(VadBackend::Ten)
                .map_err(|e| {
                    if let Err(emit_err) = app.emit(EVENT_MODEL_FAILED, format!("VAD: {}", e)) {
                        log::warn!(
                            "[Core::Engine] Failed to emit EVENT_MODEL_FAILED: {}",
                            emit_err
                        );
                    }
                    format!("[Core::Engine] Ten VAD init failed: {}", e)
                })
        }
    }
}

/// Creates and initializes the CPAL audio playback engine.
fn create_playback_engine(state: &AppState) -> Result<Arc<PlaybackEngine>, String> {
    let telemetry_handles = PlaybackTelemetryHandles {
        energy: Arc::clone(&state.telemetry.latest_playback_energy),
        low: Arc::clone(&state.telemetry.latest_playback_low),
        mid: Arc::clone(&state.telemetry.latest_playback_mid),
        high: Arc::clone(&state.telemetry.latest_playback_high),
        underruns: Arc::clone(&state.pipeline.playback_underruns),
    };

    let pe = PlaybackEngine::new(
        Arc::clone(&state.pipeline.cancel_flag),
        Arc::clone(&state.pipeline.current_state_atomic),
        telemetry_handles,
    )
    .map_err(|e| format!("[Core::Engine] Playback engine init failed: {}", e))?;

    Ok(Arc::new(pe))
}

/// Starts the global Vox native audio engine, initializes hardware streams, and spawns worker threads.
pub async fn start_audio_engine<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let mut lock = state.engine.lock().await;
    if lock.is_some() {
        log::info!("[Core::Engine] Audio Engine is already running");
        return Ok(());
    }

    log::info!("[Core::Engine] Booting 3-Tier Audio Engine...");
    if let Err(e) = app.emit(EVENT_MODEL_LOADING, "AudioEngine") {
        log::warn!("[Core::Engine] Failed to emit EVENT_MODEL_LOADING: {}", e);
    }

    ensure_manifest_loaded(state).await;
    ensure_persistence_worker(state);

    let (stt_tx, stt_rx) = std::sync::mpsc::channel();
    let (vad_tx, vad_rx) = std::sync::mpsc::channel();
    let (vox_event_tx, vox_event_rx) = std::sync::mpsc::channel();

    let stt_provider = create_stt_instance(app, state)?;
    let vad = create_vad_instance(app, state).await?;

    let (threshold, noise_gate, mode, audio_mode) = {
        let s = state
            .settings
            .read()
            .map_err(|e| format!("[Core::Engine] Settings lock poisoned: {}", e))?;
        (
            s.vad.threshold,
            s.vad.ptt_noise_gate,
            s.interaction.mode.clone(),
            s.audio.output_mode.clone(),
        )
    };

    let ring_buffer = ringbuf::HeapRb::<f32>::new(RING_BUFFER_SIZE);
    let (producer, consumer) = ring_buffer.split();

    state.pipeline.engine_shutdown.store(false, Ordering::Relaxed);

    let vad_config = VadActorConfig {
        initial_threshold: threshold,
        initial_noise_gate: noise_gate,
        initial_mode: mode,
        initial_audio_mode: audio_mode,
    };

    let vad_handles = VadActorHandles {
        is_loaded: Arc::clone(&state.is_vad_loaded),
        state_atomic: Arc::clone(&state.pipeline.current_state_atomic),
        turn_id_atomic: Arc::clone(&state.pipeline.turn_id),
        audio_suppressed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        engine_shutdown: Arc::clone(&state.pipeline.engine_shutdown),
        dropped_counter: Arc::clone(&state.telemetry.dropped_telemetry_events),
    };

    let vad_channels = VadActorChannels {
        stt_tx: stt_tx.clone(),
        vad_rx,
        telemetry_tx: state.telemetry.telemetry_tx.clone(),
        vox_event_tx: Some(vox_event_tx.clone()),
    };

    let vad_handle = std::thread::Builder::new()
        .name("vox-vad-actor".to_string())
        .spawn(move || {
            if let Err(e) = spawn_vad_actor(
                vad,
                consumer,
                vad_channels,
                vad_handles,
                vad_config,
            ) {
                log::error!("[Core::Engine] VAD actor thread terminated with error: {:?}", e);
            }
        })
        .map_err(|e| format!("[Core::Engine] Failed to spawn VAD thread: {}", e))?;

    let stt_channels = SttActorChannels {
        rx: stt_rx,
        pipeline_event_tx: Some(vox_event_tx.clone()),
    };

    let stt_handles = SttActorHandles {
        cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
        is_loaded: Arc::clone(&state.is_stt_loaded),
        engine_shutdown: Arc::clone(&state.pipeline.engine_shutdown),
    };

    let stt_handle = spawn_stt_worker(stt_channels, stt_provider, stt_handles)
        .map_err(|e| format!("[Core::Engine] Failed to spawn STT worker: {}", e))?;

    if let Err(e) = app.emit(EVENT_MODEL_READY, "STT") {
        log::warn!("[Core::Engine] Failed to emit EVENT_MODEL_READY: {}", e);
    }

    let input_device = state
        .settings
        .read()
        .map_err(|e| format!("[Core::Engine] Settings lock poisoned: {}", e))?
        .audio
        .input_device
        .clone();
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
        telemetry_tx: state.telemetry.telemetry_tx.clone(),
        pipeline_tx: vox_event_tx,
        playback_engine,
        stt_handle: Some(stt_handle),
        vad_handle: Some(vad_handle),
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: Some(orchestrator_handle),
    });

    log::info!("[Core::Engine] 3-Tier Audio Engine online");
    Ok(())
}

/// Shuts down all audio engine background threads, flushes persistence, and unloads models.
pub async fn stop_audio_engine(state: &AppState) -> Result<(), String> {
    let mut lock = state.engine.lock().await;
    if let Some(mut engine) = lock.take() {
        log::info!("[Core::Engine] Shutting down audio engine threads");

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
            log::warn!("[Core::Engine] Failed to send Shutdown to pipeline: {}", e);
        }
        if let Err(e) = engine.stt_tx.send(SttCommand::Shutdown) {
            log::warn!("[Core::Engine] Failed to send Shutdown to STT: {}", e);
        }
        if let Err(e) = engine.vad_tx.send(VadCommand::Shutdown) {
            log::warn!("[Core::Engine] Failed to send Shutdown to VAD: {}", e);
        }

        crate::services::llm::actor::cool_down_llm(&mut engine.llm_tx, Some(&state.llm_provider));
        crate::services::tts::actor::cool_down_tts(&mut engine.tts_tx);

        if let Some(h) = engine.llm_handle.take() {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join LLM handle: {:?}", e);
            }
        }
        if let Some(h) = engine.tts_handle.take() {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join TTS handle: {:?}", e);
            }
        }
        if let Some(h) = engine.orchestrator_handle.take() {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join orchestrator handle: {:?}", e);
            }
        }
        if let Some(h) = engine.stt_handle.take() {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join STT handle: {:?}", e);
            }
        }
        if let Some(h) = engine.vad_handle.take() {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join VAD handle: {:?}", e);
            }
        }

        {
            let mut persist_lock = state.persist_tx.lock();
            if let Some(tx) = persist_lock.take() {
                if let Err(e) = tx.send(crate::persistence::events::PersistenceEvent::Shutdown) {
                    log::warn!(
                        "[Core::Engine] Failed to send Shutdown to persistence: {}",
                        e
                    );
                }
            }
        }

        crate::services::memory::unload_all_onnx_models();
        crate::services::memory::trim_heap("stop_audio_engine");
        log::info!("[Core::Engine] Audio engine resources cleanly released");
    }
    Ok(())
}
