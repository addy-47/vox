use std::{
    fs::read_to_string,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc, Arc,
    },
    thread::{Builder, JoinHandle},
};

use ringbuf::traits::Split;
use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc_to, IpcEvent, TranscriptPayload, VoxEvent},
        settings::TtsActiveProvider,
        state::{AppState, InteractionOwner, InteractionState},
    },
    monitoring::TelemetryEvent,
    persistence::{get_tokio_handle, worker::spawn_persistence_worker, PersistenceEvent},
    pipeline::{router::spawn_router, target_window},
    services::{
        audio::{
            playback::{PlaybackEngineHandles, PlaybackTelemetryHandles},
            AudioStream, PlaybackEngine,
        },
        llm::{
            actor::{cool_down_llm, warm_up_llm, LlmWarmUpHandles},
            LlmCommand, QWEN_MODEL_DIR, QWEN_MODEL_FILE,
        },
        memory::{trim_heap, unload_all_onnx_models},
        stt::{
            actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand},
            create_stt_instance_from_settings, SttProvider,
        },
        tts::{
            actor::{cool_down_tts, warm_up_tts, TtsWarmUpHandles},
            resolve_reference_audio, TtsCommand, SUPERTONIC_MODEL_DIR,
        },
        vad::{
            actor::{spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles},
            create_vad_instance_from_settings, VadBackend, VadCommand,
        },
    },
    setup::manifest::VoxManifest,
    utils::paths,
};

/// Native audio and worker thread runtime bundle for the modular voice engine.
pub struct VoxEngine {
    pub audio_stream: AudioStream,
    pub stt_tx: mpsc::Sender<SttCommand>,
    pub vad_tx: mpsc::Sender<VadCommand>,
    pub llm_tx: Option<mpsc::Sender<LlmCommand>>,
    pub tts_tx: Option<mpsc::Sender<TtsCommand>>,
    pub telemetry_tx: crossbeam_channel::Sender<TelemetryEvent>,
    pub pipeline_tx: mpsc::Sender<VoxEvent>,
    pub playback_engine: Arc<PlaybackEngine>,
    pub stt_handle: Option<JoinHandle<()>>,
    pub vad_handle: Option<JoinHandle<()>>,
    pub llm_handle: Option<JoinHandle<()>>,
    pub tts_handle: Option<JoinHandle<()>>,
    pub orchestrator_handle: Option<JoinHandle<()>>,
}

const RING_BUFFER_SIZE: usize = 16000 * 4; // 4s buffer

/// Ensures the persistence background worker is active and holds a valid channel.
fn ensure_persistence_worker(state: &AppState) {
    let mut persist_lock = state.persist_tx.lock();
    if persist_lock.is_none() {
        log::info!("[Core::Engine] Spawning persistence worker");
        let tx = spawn_persistence_worker(
            Arc::clone(&state.db),
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
            if let Ok(content) = read_to_string(&manifest_path) {
                if let Ok(manifest) = serde_json::from_str::<VoxManifest>(&content) {
                    *m = Some(manifest);
                }
            }
        }
    }
}

/// Resolves and instantiates the active STT provider.
fn create_stt_instance(state: &AppState) -> Result<Box<dyn SttProvider>, String> {
    let settings = state
        .settings
        .read()
        .map_err(|e| format!("[Core::Engine] Settings lock poisoned: {}", e))?;
    let models_dir = paths::get().models.clone();
    create_stt_instance_from_settings(&settings.stt, &models_dir)
        .map_err(|e| format!("[Core::Engine] {}", e))
}

/// Resolves and instantiates the active VAD backend engine.
async fn create_vad_instance(state: &AppState) -> Result<VadBackend, String> {
    let settings = state
        .settings
        .read()
        .map_err(|e| format!("[Core::Engine] Settings lock poisoned: {}", e))?;
    let models_dir = paths::get().models.clone();
    create_vad_instance_from_settings(&settings.vad, &models_dir)
        .map_err(|e| format!("[Core::Engine] {}", e))
}

/// Creates and initializes the CPAL audio playback engine.
fn create_playback_engine(
    state: &AppState,
    event_tx: mpsc::Sender<VoxEvent>,
) -> Result<Arc<PlaybackEngine>, String> {
    let telemetry_handles = PlaybackTelemetryHandles {
        energy: Arc::clone(&state.telemetry.latest_playback_energy),
        low: Arc::clone(&state.telemetry.latest_playback_low),
        mid: Arc::clone(&state.telemetry.latest_playback_mid),
        high: Arc::clone(&state.telemetry.latest_playback_high),
        underruns: Arc::clone(&state.pipeline.playback_underruns),
    };

    let engine_handles = PlaybackEngineHandles {
        cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
        state_atomic: Arc::clone(&state.pipeline.current_state_atomic),
        current_turn_id: Arc::clone(&state.pipeline.turn_id),
        pending_synthesis_jobs: Arc::clone(&state.pipeline.pending_synthesis_jobs),
        event_tx,
        playback_intent: Arc::new(AtomicU8::new(0)),
        is_playback_muted: Arc::clone(&state.pipeline.is_playback_muted),
        turn_metrics: Some(Arc::clone(&state.turn_metrics)),
    };

    let pe = PlaybackEngine::new(engine_handles, telemetry_handles)
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

    ensure_manifest_loaded(state).await;
    ensure_persistence_worker(state);

    let (stt_tx, stt_rx) = mpsc::channel();
    let (vad_tx, vad_rx) = mpsc::channel();
    let (vox_event_tx, vox_event_rx) = mpsc::channel();

    let stt_provider = create_stt_instance(state)?;
    let vad = create_vad_instance(state).await?;

    let (threshold, noise_gate, silence_duration_ms, speech_onset_ms, mode, audio_mode) = {
        let s = state
            .settings
            .read()
            .map_err(|e| format!("[Core::Engine] Settings lock poisoned: {}", e))?;
        (
            s.vad.threshold,
            s.vad.ptt_noise_gate,
            s.vad.silence_duration_ms,
            s.vad.speech_onset_ms,
            s.interaction.mode.clone(),
            s.audio.output_mode.clone(),
        )
    };

    let ring_buffer = ringbuf::HeapRb::<f32>::new(RING_BUFFER_SIZE);
    let (producer, consumer) = ring_buffer.split();

    state
        .pipeline
        .engine_shutdown
        .store(false, Ordering::Relaxed);

    let vad_config = VadActorConfig {
        initial_threshold: threshold,
        initial_noise_gate: noise_gate,
        initial_silence_duration_ms: silence_duration_ms,
        initial_speech_onset_ms: speech_onset_ms,
        initial_mode: mode,
        initial_audio_mode: audio_mode,
    };

    let vad_handles = VadActorHandles {
        state_atomic: Arc::clone(&state.pipeline.current_state_atomic),
        turn_id_atomic: Arc::clone(&state.pipeline.turn_id),
        audio_suppressed: Arc::new(AtomicBool::new(false)),
        engine_shutdown: Arc::clone(&state.pipeline.engine_shutdown),
        dropped_counter: Arc::clone(&state.telemetry.dropped_telemetry_events),
        ingestion_gate: Arc::clone(&state.pipeline.ingestion_gate),
    };

    let vad_channels = VadActorChannels {
        stt_tx: stt_tx.clone(),
        vad_rx,
        telemetry_tx: state.telemetry.telemetry_tx.clone(),
        vox_event_tx: Some(vox_event_tx.clone()),
    };

    let vad_handle = Builder::new()
        .name("vox-vad-actor".to_string())
        .spawn(move || {
            if let Err(e) = spawn_vad_actor(vad, consumer, vad_channels, vad_handles, vad_config) {
                log::error!(
                    "[Core::Engine] VAD actor thread terminated with error: {:?}",
                    e
                );
            }
        })
        .map_err(|e| format!("[Core::Engine] Failed to spawn VAD thread: {}", e))?;

    let app_handle = app.clone();
    let partial_emitter = Some(Arc::new(move |turn_id: u32, text: String| {
        let target = target_window(InteractionOwner::Assistant);
        if let Err(e) = emit_ipc_to(
            &app_handle,
            target,
            IpcEvent::TranscriptPartial(TranscriptPayload {
                turn_id,
                text,
                owner: Some(InteractionOwner::Assistant),
            }),
        ) {
            log::trace!(
                "[Core::Engine] Failed to emit partial transcript IPC: {}",
                e
            );
        }
    }) as Arc<dyn Fn(u32, String) + Send + Sync>);

    let stt_channels = SttActorChannels {
        rx: stt_rx,
        pipeline_event_tx: Some(vox_event_tx.clone()),
        partial_emitter,
    };

    let stt_handles = SttActorHandles {
        cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
        engine_shutdown: Arc::clone(&state.pipeline.engine_shutdown),
    };

    let stt_handle = spawn_stt_worker(stt_channels, stt_provider, stt_handles)
        .map_err(|e| format!("[Core::Engine] Failed to spawn STT worker: {}", e))?;

    let input_device = state
        .settings
        .read()
        .map_err(|e| format!("[Core::Engine] Settings lock poisoned: {}", e))?
        .audio
        .input_device
        .clone();
    let audio_stream = AudioStream::new(
        producer,
        input_device,
        Arc::clone(&state.pipeline.ingestion_gate),
    )
    .map_err(|e| e.to_string())?;
    audio_stream.start().map_err(|e| e.to_string())?;

    let playback_engine = create_playback_engine(state, vox_event_tx.clone())?;
    let orchestrator_handle = spawn_router(app.clone(), vox_event_rx)?;

    *lock = Some(VoxEngine {
        audio_stream,
        stt_tx,
        vad_tx,
        llm_tx: None,
        tts_tx: None,
        telemetry_tx: state.telemetry.telemetry_tx.clone(),
        pipeline_tx: vox_event_tx.clone(),
        playback_engine,
        stt_handle: Some(stt_handle),
        vad_handle: Some(vad_handle),
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: Some(orchestrator_handle),
    });

    *state.event_tx.lock() = Some(vox_event_tx);

    log::info!("[Core::Engine] 3-Tier Audio Engine online");
    Ok(())
}

/// Shuts down all audio engine background threads, flushes persistence, and unloads models.
pub async fn stop_audio_engine(state: &AppState) -> Result<(), String> {
    let mut engine = {
        let mut lock = state.engine.lock().await;
        match lock.take() {
            Some(e) => e,
            None => return Ok(()),
        }
    };

    log::info!("[Core::Engine] Shutting down audio engine threads");

    state
        .pipeline
        .engine_shutdown
        .store(true, Ordering::Relaxed);
    state.pipeline.set_state(InteractionState::Idle);
    *state.event_tx.lock() = None;

    if let Err(e) = engine.pipeline_tx.send(VoxEvent::Shutdown) {
        log::warn!("[Core::Engine] Failed to send Shutdown to pipeline: {}", e);
    }
    if let Err(e) = engine.stt_tx.send(SttCommand::Shutdown) {
        log::warn!("[Core::Engine] Failed to send Shutdown to STT: {}", e);
    }
    if let Err(e) = engine.vad_tx.send(VadCommand::Shutdown) {
        log::warn!("[Core::Engine] Failed to send Shutdown to VAD: {}", e);
    }

    cool_down_llm(&mut engine.llm_tx, Some(&state.llm_provider));
    cool_down_tts(&mut engine.tts_tx);

    let llm_handle = engine.llm_handle.take();
    let tts_handle = engine.tts_handle.take();
    let orch_handle = engine.orchestrator_handle.take();
    let stt_handle = engine.stt_handle.take();
    let vad_handle = engine.vad_handle.take();

    tokio::task::spawn_blocking(move || {
        if let Some(h) = llm_handle {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join LLM handle: {:?}", e);
            }
        }
        if let Some(h) = tts_handle {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join TTS handle: {:?}", e);
            }
        }
        if let Some(h) = orch_handle {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join orchestrator handle: {:?}", e);
            }
        }
        if let Some(h) = stt_handle {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join STT handle: {:?}", e);
            }
        }
        if let Some(h) = vad_handle {
            if let Err(e) = h.join() {
                log::warn!("[Core::Engine] Failed to join VAD handle: {:?}", e);
            }
        }
    })
    .await
    .map_err(|e| format!("Join task failed: {}", e))?;

    {
        let mut persist_lock = state.persist_tx.lock();
        if let Some(tx) = persist_lock.take() {
            if let Err(e) = tx.send(PersistenceEvent::Shutdown) {
                log::warn!(
                    "[Core::Engine] Failed to send Shutdown to persistence: {}",
                    e
                );
            }
        }
    }

    unload_all_onnx_models();
    trim_heap("stop_audio_engine");
    log::info!("[Core::Engine] Audio engine resources cleanly released");
    Ok(())
}

/// Synchronous wrapper for stop_audio_engine executed via the global Tokio runtime handle.
pub fn stop_audio_engine_sync(state: &AppState) -> Result<(), String> {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(stop_audio_engine(state)))
    } else {
        let handle = get_tokio_handle();
        handle.block_on(stop_audio_engine(state))
    }
}

/// Initializes and warms up the LLM and TTS actor threads asynchronously if not already loaded.
pub async fn ensure_modular_workers(state: &AppState) -> Result<(), String> {
    let (llm_path, tts_path, settings) = {
        let s = state
            .settings
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let models_dir = paths::get().models.clone();
        let llm = models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE);
        let tts = models_dir.join(SUPERTONIC_MODEL_DIR);
        (llm, tts, s)
    };

    let voice_id = match settings.tts.active {
        TtsActiveProvider::Chatterbox => settings.tts.chatterbox.voice_id.as_deref(),
        TtsActiveProvider::ChatterboxRemote => settings.tts.chatterbox_remote.voice_id.as_deref(),
        _ => None,
    };
    let conn = state.db.connect().ok();
    let reference_audio = if let Some(ref c) = conn {
        resolve_reference_audio(c, voice_id).await
    } else {
        None
    };

    // Check if workers are already active under a short lock
    let (needs_llm, needs_tts, playback_engine, pipeline_tx) = {
        let lock = state.engine.lock().await;
        let engine = lock.as_ref().ok_or("Audio engine not ready")?;
        (
            engine.llm_tx.is_none(),
            engine.tts_tx.is_none(),
            Arc::clone(&engine.playback_engine),
            engine.pipeline_tx.clone(),
        )
    };

    let mut new_llm_tx = None;
    let mut new_llm_handle = None;
    if needs_llm {
        warm_up_llm(
            LlmWarmUpHandles {
                llm_tx: &mut new_llm_tx,
                llm_handle: &mut new_llm_handle,
                llm_provider_cache: Some(Arc::clone(&state.llm_provider)),
            },
            &settings,
            &llm_path,
        )?;
    }

    let mut new_tts_tx = None;
    let mut new_tts_handle = None;
    if needs_tts {
        warm_up_tts(
            TtsWarmUpHandles {
                tts_tx: &mut new_tts_tx,
                tts_handle: &mut new_tts_handle,
                cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
                playback_engine,
                pending_synthesis_jobs: Some(Arc::clone(&state.pipeline.pending_synthesis_jobs)),
                telemetry_rtf: Some(Arc::clone(&state.telemetry.latest_tts_rtf)),
                turn_metrics: Some(Arc::clone(&state.turn_metrics)),
            },
            &settings,
            &tts_path,
            reference_audio.as_deref(),
            pipeline_tx,
        )?;
    }

    if needs_llm || needs_tts {
        let mut lock = state.engine.lock().await;
        if let Some(ref mut engine) = *lock {
            if needs_llm && engine.llm_tx.is_none() {
                engine.llm_tx = new_llm_tx;
                engine.llm_handle = new_llm_handle;
            }
            if needs_tts && engine.tts_tx.is_none() {
                engine.tts_tx = new_tts_tx;
                engine.tts_handle = new_tts_handle;
            }
        }
    }

    Ok(())
}

/// Synchronous wrapper for ensure_modular_workers to be called safely on the Router OS thread.
pub fn ensure_modular_workers_sync(state: &AppState) -> Result<(), String> {
    let handle = get_tokio_handle();
    handle.block_on(ensure_modular_workers(state))
}
