//! ============================================================================
//! benches/common/pipeline_harness.rs — E2E Audio-In to Audio-Out Pipeline Harness
//! ============================================================================

use ringbuf::traits::Split;
use ringbuf::{HeapCons, HeapRb};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::mpsc;
use std::sync::Arc;

use tauri::AppHandle;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{AudioOutputMode, InteractionMode, VoxSettings};
use vox_lib::core::state::{AppState, TelemetryState, VoxEngine};
use vox_lib::services::audio::playback::PlaybackEngineHandles;
use vox_lib::services::audio::{AudioStream, PlaybackEngine, PLAYBACK_BUFFER_SAMPLES};
use vox_lib::services::stt::actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand};
use vox_lib::services::stt::{EmbeddedSttProvider, SttProvider};
use vox_lib::services::vad::actor::{spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles};
use vox_lib::services::vad::earshot_vad::EarshotVadEngine;
use vox_lib::services::vad::{VadBackend, VadCommand};

pub type E2ePipelineSetup = (
    AppHandle<tauri::test::MockRuntime>,
    Arc<AppState>,
    Arc<parking_lot::Mutex<ringbuf::HeapProd<f32>>>,
    Arc<parking_lot::Mutex<HeapCons<f32>>>,
    mpsc::Receiver<VoxEvent>,
);

/// Sets up production-wiring for an E2E pipeline run without CPAL hardware.
pub fn setup_e2e_pipeline(
    settings: VoxSettings,
) -> E2ePipelineSetup {
    let app = tauri::test::mock_app().handle().clone();
    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();
    let telemetry = Arc::new(TelemetryState {
        telemetry_tx,
        latest_energy: Arc::new(AtomicU32::new(0)),
        latest_vad_prob: Arc::new(AtomicU32::new(0)),
        latest_low: Arc::new(AtomicU32::new(0)),
        latest_mid: Arc::new(AtomicU32::new(0)),
        latest_high: Arc::new(AtomicU32::new(0)),
        latest_playback_energy: Arc::new(AtomicU32::new(0)),
        latest_playback_low: Arc::new(AtomicU32::new(0)),
        latest_playback_mid: Arc::new(AtomicU32::new(0)),
        latest_playback_high: Arc::new(AtomicU32::new(0)),
        latest_sys_cpu: Arc::new(AtomicU32::new(0)),
        latest_sys_ram: Arc::new(AtomicU32::new(0)),
        latest_vox_cpu: Arc::new(AtomicU32::new(0)),
        latest_vox_ram: Arc::new(AtomicU32::new(0)),
        latest_stt_ms: Arc::new(AtomicU32::new(0)),
        latest_ttft_ms: Arc::new(AtomicU32::new(0)),
        latest_voice_latency_ms: Arc::new(AtomicU32::new(0)),
        latest_threads: Arc::new(AtomicU32::new(0)),
        latest_tts_rtf: Arc::new(AtomicU32::new(0)),
        latest_playback_start_ms: Arc::new(AtomicU32::new(0)),
        latest_persistence_rate: Arc::new(AtomicU32::new(0)),
        is_db_healthy: Arc::new(AtomicBool::new(true)),
        is_private_mode: Arc::new(AtomicBool::new(false)),
        dropped_telemetry_events: Arc::new(AtomicU64::new(0)),
    });

    vox_lib::utils::paths::init();
    let state = Arc::new(AppState::new(&app, None, telemetry));
    *state.settings.write().unwrap() = settings.clone();
    tauri::Manager::manage(&app, state.clone());

    // 1. Playback engine with tap
    let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
    *state.event_tx.lock() = Some(event_tx.clone());

    let playback_rb = HeapRb::<f32>::new(PLAYBACK_BUFFER_SAMPLES);
    let (pb_prod, pb_cons) = playback_rb.split();
    let pb_handles = PlaybackEngineHandles {
        cancel_flag: state.pipeline.cancel_flag.clone(),
        state_atomic: state.pipeline.current_state_atomic.clone(),
        current_turn_id: state.pipeline.turn_id.clone(),
        pending_synthesis_jobs: state.pipeline.pending_synthesis_jobs.clone(),
        event_tx: event_tx.clone(),
    };
    let playback_engine = Arc::new(PlaybackEngine::from_parts(
        pb_prod,
        pb_handles,
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
        None,
    ));

    // 2. VAD Input RingBuffer & Actor
    let input_rb = HeapRb::<f32>::new(65536);
    let (in_prod, in_cons) = input_rb.split();
    let in_prod_arc = Arc::new(parking_lot::Mutex::new(in_prod));

    let vad_engine = EarshotVadEngine::new(0.4).expect("Failed to initialize Earshot VAD");
    let vad_backend = VadBackend::Earshot(vad_engine);
    let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();

    // 3. STT Worker (Nemotron)
    let home = dirs::home_dir().expect("Home dir needed");
    let nemotron_dir = home.join(".vox/models/stt/nemotron-3.5");
    let stt_provider = Box::new(
        EmbeddedSttProvider::new(&nemotron_dir, "nvidia_nemotron", 4)
            .expect("Failed to load Nemotron provider"),
    ) as Box<dyn SttProvider>;

    let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();
    let stt_channels = SttActorChannels {
        rx: stt_rx,
        pipeline_event_tx: Some(event_tx.clone()),
        partial_emitter: None,
    };
    let stt_handles = SttActorHandles {
        cancel_flag: state.pipeline.cancel_flag.clone(),
        engine_shutdown: state.pipeline.engine_shutdown.clone(),
    };
    let stt_handle = spawn_stt_worker(stt_channels, stt_provider, stt_handles)
        .expect("Failed to spawn STT worker");

    let vad_config = VadActorConfig {
        initial_threshold: 0.4,
        initial_noise_gate: 0.005,
        initial_silence_duration_ms: settings.vad.silence_duration_ms,
        initial_speech_onset_ms: 32,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    };
    let vad_channels = VadActorChannels {
        stt_tx: stt_tx.clone(),
        vad_rx: vad_cmd_rx,
        telemetry_tx: state.telemetry.telemetry_tx.clone(),
        vox_event_tx: Some(event_tx.clone()),
    };
    let vad_handles = VadActorHandles {
        state_atomic: state.pipeline.current_state_atomic.clone(),
        turn_id_atomic: state.pipeline.turn_id.clone(),
        audio_suppressed: Arc::new(AtomicBool::new(false)),
        engine_shutdown: state.pipeline.engine_shutdown.clone(),
        dropped_counter: Arc::new(AtomicU64::new(0)),
        ingestion_gate: state.pipeline.ingestion_gate.clone(),
    };

    let vad_handle = std::thread::spawn(move || {
        let _ = spawn_vad_actor(vad_backend, in_cons, vad_channels, vad_handles, vad_config);
    });

    // 4. Central Router with Benchmark Observer Split
    let (router_tx, router_rx) = mpsc::channel::<VoxEvent>();
    let (bench_tx, bench_rx) = mpsc::channel::<VoxEvent>();

    // Forward events from actors to BOTH the router pump (driving state) and bench_tx (measuring latency)
    let router_tx_clone = router_tx.clone();
    std::thread::Builder::new()
        .name("bench-event-tee".to_string())
        .spawn(move || {
            while let Ok(ev) = event_rx.recv() {
                let _ = bench_tx.send(ev.clone());
                let _ = router_tx_clone.send(ev);
            }
        })
        .expect("Failed to spawn bench-event-tee");

    let router_handle = vox_lib::pipeline::router::spawn_router(app.clone(), router_rx)
        .expect("Failed to spawn router");

    // Connect Engine
    let engine = VoxEngine {
        audio_stream: AudioStream::mock(),
        stt_tx,
        vad_tx: vad_cmd_tx,
        llm_tx: None,
        tts_tx: None,
        telemetry_tx: state.telemetry.telemetry_tx.clone(),
        pipeline_tx: router_tx,
        playback_engine,
        stt_handle: Some(stt_handle),
        vad_handle: Some(vad_handle),
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: Some(router_handle),
    };

    *state.engine.blocking_lock() = Some(engine);

    // Warm up modular LLM + TTS workers asynchronously
    vox_lib::core::engine::ensure_modular_workers_sync(&app, &state)
        .expect("Failed to warm up modular workers");

    (app, state, in_prod_arc, Arc::new(parking_lot::Mutex::new(pb_cons)), bench_rx)
}
