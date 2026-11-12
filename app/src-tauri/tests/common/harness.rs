//! ============================================================================
//! tests/common/harness.rs — Test Harness Constructors & Actor Lifecycle Helpers
//! ============================================================================

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use ringbuf::{
    traits::{Observer, Split},
    wrap::caching::Caching,
    HeapCons, HeapRb,
};
use tauri::AppHandle;
use vox_lib::{
    core::events::{InteractionOwner, VoxEvent},
    services::{
        audio::{
            aec::{AecCommand, AecProcessor},
            playback::PlaybackEngineHandles,
            sink::{PlaybackReferenceTap, PlaybackStreamContext},
            spawn_aec_worker, AEC_REFERENCE_BUFFER_SAMPLES,
        },
        audio::PlaybackEngine,
        llm::actor::{spawn_llm_worker, LlmCommand},
        stt::{
            actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand},
            EmbeddedSttProvider, SttProvider,
        },
        tts::{
            actor::{spawn_tts_worker, TtsCommand, TtsWorkerHandles},
            providers::{kokoro::KokoroEngine, TtsProvider},
        },
        vad::{
            actor::{spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles},
            earshot_vad::EarshotVadEngine,
            providers::{SileroVadEngine, TenVadEngine},
            VadBackend, VadBackendOption, VadCommand,
        },
    },
};

pub type RbProducer = Caching<Arc<HeapRb<f32>>, true, false>;

/// Creates a mock/headless playback engine and consumer for integration tests without CPAL audio hardware.
pub fn create_mock_playback_engine() -> (Arc<PlaybackEngine>, Arc<Mutex<HeapCons<f32>>>) {
    let (event_tx, _rx) = mpsc::channel();
    create_mock_playback_engine_with_event_tx(event_tx)
}

/// Creates a mock/headless playback engine and consumer with a caller-provided event_tx channel.
pub fn create_mock_playback_engine_with_event_tx(
    event_tx: mpsc::Sender<VoxEvent>,
) -> (Arc<PlaybackEngine>, Arc<Mutex<HeapCons<f32>>>) {
    create_mock_playback_engine_with_handles(
        event_tx,
        Arc::new(AtomicU32::new(0)),
        Arc::new(AtomicU32::new(0)),
    )
}

/// Creates a mock/headless playback engine and consumer with caller-provided event channel and atomics.
pub fn create_mock_playback_engine_with_handles(
    event_tx: mpsc::Sender<VoxEvent>,
    current_turn_id: Arc<AtomicU32>,
    pending_synthesis_jobs: Arc<AtomicU32>,
) -> (Arc<PlaybackEngine>, Arc<Mutex<HeapCons<f32>>>) {
    let rb = HeapRb::<f32>::new(vox_lib::services::audio::PLAYBACK_BUFFER_SAMPLES);
    let (producer, consumer) = rb.split();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let discard_request = Arc::new(AtomicBool::new(false));
    let turn_armed = Arc::new(AtomicBool::new(false));

    let handles = vox_lib::services::audio::playback::PlaybackEngineHandles {
        cancel_flag,
        state_atomic: Arc::new(AtomicU32::new(0)),
        current_turn_id,
        pending_synthesis_jobs,
        playback_intent: Arc::new(AtomicU8::new(0)),
        event_tx,
        is_playback_muted: Arc::new(AtomicBool::new(false)),
        turn_metrics: None,
    };

    let engine = PlaybackEngine::from_parts(producer, handles, discard_request, turn_armed, None);

    (Arc::new(engine), Arc::new(Mutex::new(consumer)))
}

/// Creates a headless PlaybackEngine + PlaybackStreamContext wired identically
/// to `src/core/engine.rs::create_playback_engine`: the sink feeds a
/// `PlaybackReferenceTap` (48kHz -> Butterworth LPF -> 3:1 decimation -> 16kHz)
/// so AEC tests exercise the real reference path instead of synthetic 16kHz slices.
pub fn create_headless_playback_with_aec_tap(
    state: &vox_lib::core::state::AppState,
    event_tx: mpsc::Sender<VoxEvent>,
) -> (
    Arc<PlaybackEngine>,
    vox_lib::services::audio::sink::PlaybackStreamContext,
    ringbuf::HeapCons<f32>,
) {
    let rb = HeapRb::<f32>::new(vox_lib::services::audio::PLAYBACK_BUFFER_SAMPLES);
    let (producer, consumer) = rb.split();

    let aec_ref_rb = HeapRb::<f32>::new(AEC_REFERENCE_BUFFER_SAMPLES);
    let (ref_prod, ref_cons) = aec_ref_rb.split();
    let reference_tap = PlaybackReferenceTap::new(ref_prod);

    let discard_request = Arc::new(AtomicBool::new(false));
    let turn_armed = Arc::new(AtomicBool::new(false));

    let handles = PlaybackEngineHandles {
        cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
        state_atomic: Arc::clone(&state.pipeline.current_state_atomic),
        current_turn_id: Arc::clone(&state.pipeline.turn_id),
        pending_synthesis_jobs: Arc::clone(&state.pipeline.pending_synthesis_jobs),
        playback_intent: Arc::new(AtomicU8::new(0)),
        event_tx,
        is_playback_muted: Arc::clone(&state.pipeline.is_playback_muted),
        turn_metrics: Some(Arc::clone(&state.turn_metrics)),
    };

    let telemetry = vox_lib::services::audio::PlaybackTelemetryHandles {
        energy: Arc::clone(&state.telemetry.latest_playback_energy),
        low: Arc::clone(&state.telemetry.latest_playback_low),
        mid: Arc::clone(&state.telemetry.latest_playback_mid),
        high: Arc::clone(&state.telemetry.latest_playback_high),
        underruns: Arc::clone(&state.pipeline.playback_underruns),
    };

    let sink = PlaybackStreamContext::new(
        consumer,
        handles.clone(),
        Arc::clone(&discard_request),
        Arc::clone(&turn_armed),
        Arc::new(AtomicBool::new(false)),
        &telemetry,
        Some(reference_tap),
    );

    let engine = PlaybackEngine::from_parts(
        producer,
        handles,
        discard_request,
        turn_armed,
        None,
    );

    (Arc::new(engine), sink, ref_cons)
}

/// Pumps `PlaybackStreamContext::process_output_buffer` in 10 ms stereo slices
/// (960 f32 samples @ 48 kHz) until the playback buffer drains and the turn is
/// disarmed with no pending synthesis jobs (or `max_frames` elapses).
/// Returns the rendered mono samples (left channel downmix).
pub fn pump_playback_sink_until_finished(
    sink: &mut vox_lib::services::audio::sink::PlaybackStreamContext,
    max_frames: usize,
) -> Vec<f32> {
    let mut stereo_out = [0.0f32; 960];
    let mut rendered_mono = Vec::new();

    for _ in 0..max_frames {
        sink.process_output_buffer(&mut stereo_out);
        for pair in stereo_out.chunks_exact(2) {
            rendered_mono.push(pair[0]);
        }
        if !sink.turn_armed.load(std::sync::atomic::Ordering::Relaxed)
            && sink.consumer.occupied_len() == 0
            && sink.handles.pending_synthesis_jobs.load(std::sync::atomic::Ordering::Relaxed) == 0
        {
            break;
        }
    }

    rendered_mono
}

/// Creates a mock AppHandle for integration testing without desktop event loops.
pub fn get_test_app_handle() -> AppHandle<tauri::test::MockRuntime> {
    tauri::test::mock_app().handle().clone()
}

/// Spawns the production STT worker with local Nemotron model.
pub fn setup_stt_worker<R: tauri::Runtime + 'static>(
    _app: &AppHandle<R>,
) -> (
    Sender<SttCommand>,
    Receiver<VoxEvent>,
    Arc<AtomicBool>,
    std::thread::JoinHandle<()>,
) {
    let nemotron_dir = super::paths::get_nemotron_model_dir();
    let provider = Box::new(
        EmbeddedSttProvider::new(&nemotron_dir, "nemotron", 2)
            .expect("Failed to instantiate EmbeddedSttProvider with Nemotron"),
    ) as Box<dyn SttProvider>;

    let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();
    let (pipeline_event_tx, pipeline_event_rx) = mpsc::channel::<VoxEvent>();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let engine_shutdown = Arc::new(AtomicBool::new(false));

    let channels = SttActorChannels {
        rx: stt_rx,
        pipeline_event_tx: Some(pipeline_event_tx),
        partial_emitter: None,
    };

    let handles = SttActorHandles {
        cancel_flag,
        engine_shutdown: engine_shutdown.clone(),
    };

    let join_handle =
        spawn_stt_worker(channels, provider, handles).expect("Failed to spawn STT worker");

    (stt_tx, pipeline_event_rx, engine_shutdown, join_handle)
}

/// Spawns the production VAD actor wired like src/core/engine.rs.
/// The caller provides `vox_event_tx` (the router's event channel) so VAD
/// events flow straight into `spawn_router`; the returned tuple carries the
/// VAD command channel, capture ring-buffer producer, and worker thread.
#[allow(clippy::too_many_arguments)]
pub fn setup_vad_actor(
    vad_backend: VadBackend,
    stt_tx: Sender<SttCommand>,
    config: VadActorConfig,
    state_atomic: Arc<AtomicU32>,
    turn_id_atomic: Arc<AtomicU32>,
    audio_suppressed: Arc<AtomicBool>,
    ingestion_gate: Arc<AtomicBool>,
    engine_shutdown: Arc<AtomicBool>,
    vox_event_tx: Sender<VoxEvent>,
) -> (
    Sender<VadCommand>,
    RbProducer,
    std::thread::JoinHandle<()>,
) {
    let rb = HeapRb::<f32>::new(65536);
    let (producer, consumer) = rb.split();

    let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();

    let vad_channels = VadActorChannels {
        stt_tx,
        vad_rx: vad_cmd_rx,
        telemetry_tx,
        vox_event_tx: Some(vox_event_tx),
    };

    let vad_handles = VadActorHandles {
        state_atomic,
        turn_id_atomic,
        audio_suppressed,
        engine_shutdown,
        dropped_counter: Arc::new(AtomicU64::new(0)),
        purged_frames_counter: Arc::new(AtomicU64::new(0)),
        ingestion_gate,
        owner_atomic: Arc::new(AtomicU32::new(InteractionOwner::Assistant as u32)),
    };

    let join_handle = std::thread::Builder::new()
        .name("test-vad-actor".to_string())
        .spawn(move || {
            spawn_vad_actor(vad_backend, consumer, vad_channels, vad_handles, config)
                .expect("VAD actor failed");
        })
        .expect("Failed to spawn VAD actor thread");

    (vad_cmd_tx, producer, join_handle)
}

/// Helper creating a concrete VAD backend instance for testing.
pub fn create_test_vad_backend(option: VadBackendOption, threshold: f32) -> VadBackend {
    match option {
        VadBackendOption::Earshot => {
            let engine =
                EarshotVadEngine::new(threshold).expect("Failed to initialize Earshot VAD engine");
            VadBackend::Earshot(engine)
        }
        VadBackendOption::SileroVad => {
            let model_path = super::paths::get_silero_vad_model_path();
            let engine = SileroVadEngine::new(&model_path, threshold, 0.064, 0.032, 30.0)
                .expect("Failed to initialize Silero VAD engine");
            VadBackend::Silero(engine)
        }
        VadBackendOption::TenVad => {
            let model_path = super::paths::get_ten_vad_model_path();
            let engine = TenVadEngine::new(&model_path, threshold, 0.064, 0.032, 30.0)
                .expect("Failed to initialize Ten VAD engine");
            VadBackend::Ten(engine)
        }
    }
}

/// Drains pipeline events until TranscriptFinal for expected turn_id is received.
pub fn drain_for_final_transcript(
    rx: &Receiver<VoxEvent>,
    expected_turn_id: u32,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(VoxEvent::TranscriptFinal { turn_id, text, .. }) => {
                if turn_id == expected_turn_id {
                    return Ok(text);
                }
            }
            Ok(_) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Channel disconnected before TranscriptFinal".to_string());
            }
        }
    }
    Err("Timed out waiting for TranscriptFinal".to_string())
}

/// Collects and concatenates all final turn transcripts across streaming sessions.
pub fn collect_all_final_transcripts(
    rx: &Receiver<VoxEvent>,
    expected_turns: usize,
    timeout: Duration,
) -> String {
    let deadline = Instant::now() + timeout;
    let mut finals = Vec::new();

    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(300)) {
            Ok(VoxEvent::TranscriptFinal { text, .. }) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    finals.push(trimmed.to_string());
                }
                if finals.len() >= expected_turns {
                    std::thread::sleep(Duration::from_millis(200));
                    while let Ok(ev) = rx.try_recv() {
                        if let VoxEvent::TranscriptFinal { text: rem_text, .. } = ev {
                            let rem_trimmed = rem_text.trim();
                            if !rem_trimmed.is_empty() {
                                finals.push(rem_trimmed.to_string());
                            }
                        }
                    }
                    break;
                }
            }
            Ok(_) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if finals.len() >= expected_turns {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    finals.join(" ")
}

/// Asserts that `rx` holds no buffered item.
///
/// This function never waits. Call it only after quiescence is proven, by one of:
/// - the turn's terminal state transition has been observed;
/// - a synchronous handler has returned;
/// - every producer has been shut down and joined;
/// - the sender was dropped.
///
/// Waiting here would convert "the producer has not run yet" into a false pass.
pub fn assert_channel_empty<T>(rx: &Receiver<T>, label: &str) {
    let mut count = 0usize;
    while rx.try_recv().is_ok() {
        count += 1;
    }
    if count > 0 {
        panic!(
            "[{}] expected no messages after quiescence, found {}",
            label, count
        );
    }
}

/// Blocks until the VAD actor has consumed every audio frame queued before the
/// barrier. The actor polls commands at the top of each loop iteration, so a
/// `StopWindowValidation` round-trip proves all earlier chunks were processed.
/// Use before a negative assertion instead of a wall-clock sleep.
pub fn vad_quiescence_barrier(cmd_tx: &Sender<VadCommand>) {
    let (response_tx, response_rx) = mpsc::channel();
    cmd_tx
        .send(VadCommand::StopWindowValidation { response_tx })
        .expect("VAD actor must accept the quiescence barrier");
    response_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("VAD actor did not acknowledge the quiescence barrier");
}

/// Drains everything currently buffered and returns it.
///
/// Use at a scenario boundary, never to make a negative assertion pass. The caller
/// must assert that the returned items belong to the scenario that just ended.
#[must_use]
pub fn drain_pending<T>(rx: &Receiver<T>) -> Vec<T> {
    let mut items = Vec::new();
    while let Ok(item) = rx.try_recv() {
        items.push(item);
    }
    items
}

/// Constructs the default test `TelemetryState` with fresh atomics and a
/// dropped telemetry channel. Single home for the block previously duplicated
/// across the managed app-state builders.
pub fn make_test_telemetry() -> Arc<vox_lib::core::state::TelemetryState> {
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};

    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();
    Arc::new(vox_lib::core::state::TelemetryState {
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
    })
}

/// Bundled channels for pipeline seam tests (STT/VAD/LLM/TTS/pipeline).
/// Single home for the 4–5 line tuple previously duplicated across
/// `transcript_to_llm`, `llm_to_tts`, `tts_to_playback` and `tts_transition`.
/// Destructure at the call site to keep existing variable names:
/// `let PipelineTestChannels { stt_tx, vad_tx, tts_tx, tts_rx, llm_tx, pipeline_tx, pipeline_rx, .. } = setup_pipeline_channels();`
pub struct PipelineTestChannels {
    pub stt_tx: Sender<SttCommand>,
    pub vad_tx: Sender<VadCommand>,
    pub llm_tx: Sender<LlmCommand>,
    pub llm_rx: Receiver<LlmCommand>,
    pub tts_tx: Sender<TtsCommand>,
    pub tts_rx: Receiver<TtsCommand>,
    pub pipeline_tx: Sender<VoxEvent>,
    pub pipeline_rx: Receiver<VoxEvent>,
}

/// Creates a fresh set of pipeline test channels.
pub fn setup_pipeline_channels() -> PipelineTestChannels {
    let (stt_tx, _stt_rx) = mpsc::channel();
    let (vad_tx, _vad_rx) = mpsc::channel();
    let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
    let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
    let (pipeline_tx, pipeline_rx) = mpsc::channel::<VoxEvent>();
    PipelineTestChannels {
        stt_tx,
        vad_tx,
        llm_tx,
        llm_rx,
        tts_tx,
        tts_rx,
        pipeline_tx,
        pipeline_rx,
    }
}

/// Isolated app-state setup: fresh `TempPathsGuard` + `paths::init` + test
/// `AppHandle`/`AppState`. The guard is returned first so it stays alive for
/// the whole test scope: `let (_guard, app, state) = setup_isolated_app_state().await;`
pub async fn setup_isolated_app_state() -> (
    super::paths::TempPathsGuard,
    AppHandle<tauri::test::MockRuntime>,
    Arc<vox_lib::core::state::AppState>,
) {
    let guard = super::paths::TempPathsGuard::new();
    let (app, state) = get_test_app_and_state().await;
    (guard, app, state)
}

/// Synchronous variant for plain `#[test]` functions.
pub fn setup_isolated_app_state_sync() -> (
    super::paths::TempPathsGuard,
    AppHandle<tauri::test::MockRuntime>,
    Arc<vox_lib::core::state::AppState>,
) {
    let guard = super::paths::TempPathsGuard::new();
    let (app, state) = get_test_app_and_state_sync();
    (guard, app, state)
}

/// Polls `dictation_state()` until it equals `expected` or `timeout` elapses.
/// Returns true on match. Replaces bare `sleep Nms → assert state` with a
/// deadline poll so slow CI routers still converge instead of flaking.
pub async fn wait_for_dictation_state(
    state: &vox_lib::core::state::AppState,
    expected: vox_lib::core::state::InteractionState,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if state.pipeline.dictation_state() == expected {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    state.pipeline.dictation_state() == expected
}

/// Suppression invariant: polls `get_state` for the whole `watch` window and
/// panics the moment it deviates from `expected`. Stronger than a single
/// `sleep → assert still X`, which only samples the endpoint and misses
/// transient premature transitions.
pub async fn assert_pipeline_state_stable<F>(
    get_state: F,
    expected: vox_lib::core::state::InteractionState,
    watch: Duration,
    label: &str,
) where
    F: Fn() -> vox_lib::core::state::InteractionState,
{
    let deadline = Instant::now() + watch;
    while Instant::now() < deadline {
        let current = get_state();
        if current != expected {
            panic!(
                "[{}] Suppression invariant violated: expected stable {:?}, saw {:?} inside watch window",
                label, expected, current
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Synchronous variant for plain `#[test]` functions.
pub fn assert_pipeline_state_stable_sync<F>(
    get_state: F,
    expected: vox_lib::core::state::InteractionState,
    watch: Duration,
    label: &str,
) where
    F: Fn() -> vox_lib::core::state::InteractionState,
{
    let deadline = Instant::now() + watch;
    while Instant::now() < deadline {
        let current = get_state();
        if current != expected {
            panic!(
                "[{}] Suppression invariant violated: expected stable {:?}, saw {:?} inside watch window",
                label, expected, current
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Negative-assertion helper for flags: watches `flag` for the whole `watch`
/// window, failing fast if it is ever set. Replaces `sleep 200ms → assert !flag`.
pub async fn assert_flag_remains_false(
    flag: &std::sync::atomic::AtomicBool,
    watch: Duration,
    label: &str,
) {
    let deadline = Instant::now() + watch;
    while Instant::now() < deadline {
        if flag.load(std::sync::atomic::Ordering::SeqCst) {
            panic!(
                "[{}] Negative assertion failed: flag was set inside watch window",
                label
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Synchronous wrapper around `get_test_app_and_state` for plain `#[test]` functions.
/// Builds a throwaway current-thread runtime for setup only; test bodies stay runtime-free.
pub fn get_test_app_and_state_sync() -> (
    AppHandle<tauri::test::MockRuntime>,
    Arc<vox_lib::core::state::AppState>,
) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build sync test runtime")
        .block_on(get_test_app_and_state())
}

/// Opens the test database and runs the schema migrations against it.
///
/// The database's parent directory must exist before Turso can open it. The
/// app creates it at startup via `paths::ensure_dirs()`, but a test that runs
/// without `TempPathsGuard` resolves the real `~/.vox` root, and that
/// directory does not exist on a machine where the app has never run. Creating
/// it here keeps the harness self-sufficient instead of making every test
/// depend on ambient state.
async fn open_test_db() -> Arc<vox_lib::persistence::VoxDb> {
    vox_lib::utils::paths::init();
    let db_path = vox_lib::utils::paths::db_path();
    if let Some(home) = dirs::home_dir() {
        let live_vox = home.join(".vox");
        assert!(
            !db_path.starts_with(&live_vox),
            "[FATAL] Test attempted to open production database at {:?}! All tests must run inside TempPathsGuard or an isolated VOX_HOME sandbox.",
            db_path
        );
    }
    if let Some(db_parent) = db_path.parent() {
        std::fs::create_dir_all(db_parent).expect("Failed to create the database parent directory");
    }
    let db_conn = vox_lib::persistence::VoxDb::open(&db_path)
        .await
        .expect("Failed to open test database");
    let migration_conn = db_conn
        .connect()
        .expect("Failed to vend migration connection");
    vox_lib::persistence::schema::run_migrations(&migration_conn)
        .await
        .expect("Failed to run migrations on test database");
    Arc::new(db_conn)
}

/// Constructs an AppHandle and managed AppState pair tailored for testing environments.
pub async fn get_test_app_and_state() -> (
    AppHandle<tauri::test::MockRuntime>,
    Arc<vox_lib::core::state::AppState>,
) {
    use tauri::Manager;

    let app = get_test_app_handle();
    let telemetry = make_test_telemetry();

    let db = open_test_db().await;
    let state = Arc::new(vox_lib::core::state::AppState::new(
        &app, None, telemetry, db,
    ));
    app.manage(state.clone());
    (app, state)
}

/// Attaches a mock VoxEngine with a specified VAD command sender to the managed AppState.
pub fn attach_mock_engine_with_vad_to_state<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &vox_lib::core::state::AppState,
    stt_tx: mpsc::Sender<SttCommand>,
    vad_tx: mpsc::Sender<VadCommand>,
) {
    let (pipeline_tx, _) = mpsc::channel();
    let (telemetry_tx, _) = crossbeam_channel::unbounded();
    let (playback_engine, _) = create_mock_playback_engine();

    let engine = vox_lib::core::engine::VoxEngine {
        audio_stream: vox_lib::services::audio::AudioStream::mock(),
        stt_tx,
        vad_tx,
        aec_tx: None,
        llm_tx: None,
        tts_tx: None,
        telemetry_tx,
        pipeline_tx,
        playback_engine,
        aec_handle: None,
        stt_handle: None,
        vad_handle: None,
        route_watcher_handle: None,
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: None,
    };
    if let Ok(mut guard) = state.engine.try_lock() {
        *guard = Some(engine);
    } else {
        *state.engine.blocking_lock() = Some(engine);
    }
    state
        .pipeline
        .set_state(vox_lib::core::state::InteractionState::Ready);
}

/// Attaches a mock VoxEngine to the managed AppState for testing full production pipeline flows.
pub fn attach_mock_engine_to_state<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &vox_lib::core::state::AppState,
    stt_tx: mpsc::Sender<SttCommand>,
) {
    let (vad_tx, _) = mpsc::channel();
    attach_mock_engine_with_vad_to_state(app, state, stt_tx, vad_tx);
}

/// Attaches a mock VoxEngine with VAD, STT, and LLM capture channels for verifying LLM-zero invariants.
pub fn attach_mock_engine_with_llm_vad_to_state<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &vox_lib::core::state::AppState,
    stt_tx: mpsc::Sender<SttCommand>,
    vad_tx: mpsc::Sender<VadCommand>,
    llm_tx: Option<mpsc::Sender<vox_lib::services::llm::LlmCommand>>,
) {
    attach_mock_engine_with_llm_tts_to_state(_app, state, stt_tx, vad_tx, llm_tx, None);
}

/// Attaches a mock VoxEngine with VAD, STT, LLM, and TTS capture channels for pipeline orchestration tests.
pub fn attach_mock_engine_with_llm_tts_to_state<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &vox_lib::core::state::AppState,
    stt_tx: mpsc::Sender<SttCommand>,
    vad_tx: mpsc::Sender<VadCommand>,
    llm_tx: Option<mpsc::Sender<vox_lib::services::llm::LlmCommand>>,
    tts_tx: Option<mpsc::Sender<vox_lib::services::tts::TtsCommand>>,
) {
    let (pipeline_tx, _) = mpsc::channel();
    let (telemetry_tx, _) = crossbeam_channel::unbounded();
    let (playback_engine, _) = create_mock_playback_engine();

    let engine = vox_lib::core::engine::VoxEngine {
        audio_stream: vox_lib::services::audio::AudioStream::mock(),
        stt_tx,
        vad_tx,
        aec_tx: None,
        llm_tx,
        tts_tx,
        telemetry_tx,
        pipeline_tx,
        playback_engine,
        aec_handle: None,
        stt_handle: None,
        vad_handle: None,
        route_watcher_handle: None,
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: None,
    };
    if let Ok(mut guard) = state.engine.try_lock() {
        *guard = Some(engine);
    } else {
        *state.engine.blocking_lock() = Some(engine);
    }
    state
        .pipeline
        .set_state(vox_lib::core::state::InteractionState::Ready);
}

/// Attaches a mock VoxEngine with VAD, STT, LLM, TTS, and pipeline event capture channels.
pub fn attach_mock_engine_with_pipeline_tx_to_state<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &vox_lib::core::state::AppState,
    stt_tx: mpsc::Sender<SttCommand>,
    vad_tx: mpsc::Sender<VadCommand>,
    llm_tx: Option<mpsc::Sender<vox_lib::services::llm::LlmCommand>>,
    tts_tx: Option<mpsc::Sender<vox_lib::services::tts::TtsCommand>>,
    pipeline_tx: mpsc::Sender<VoxEvent>,
) {
    let (telemetry_tx, _) = crossbeam_channel::unbounded();
    let (playback_engine, _) = create_mock_playback_engine();

    let engine = vox_lib::core::engine::VoxEngine {
        audio_stream: vox_lib::services::audio::AudioStream::mock(),
        stt_tx,
        vad_tx,
        aec_tx: None,
        llm_tx,
        tts_tx,
        telemetry_tx,
        pipeline_tx,
        playback_engine,
        aec_handle: None,
        stt_handle: None,
        vad_handle: None,
        route_watcher_handle: None,
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: None,
    };
    if let Ok(mut guard) = state.engine.try_lock() {
        *guard = Some(engine);
    } else {
        *state.engine.blocking_lock() = Some(engine);
    }
    state
        .pipeline
        .set_state(vox_lib::core::state::InteractionState::Ready);
}

/// Attaches an engine with pre-populated dummy LLM and TTS channels so that
/// `ensure_modular_workers_sync` sees `needs_llm == false` and `needs_tts == false`,
/// allowing pure lifecycle testing without model weight I/O contention.
pub fn attach_lifecycle_mock_engine<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    state: &vox_lib::core::state::AppState,
    vad_tx: mpsc::Sender<VadCommand>,
) -> (
    mpsc::Sender<SttCommand>,
    mpsc::Receiver<VoxEvent>,
    mpsc::Sender<VoxEvent>,
) {
    let (stt_tx, _stt_rx) = mpsc::channel::<SttCommand>();
    let (pipeline_tx, pipeline_rx) = mpsc::channel::<VoxEvent>();
    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();
    let (playback_engine, _consumer) = create_mock_playback_engine();
    let (llm_tx, _llm_rx) = mpsc::channel();
    let (tts_tx, _tts_rx) = mpsc::channel();

    let engine = vox_lib::core::engine::VoxEngine {
        audio_stream: vox_lib::services::audio::AudioStream::mock(),
        stt_tx: stt_tx.clone(),
        vad_tx,
        aec_tx: None,
        llm_tx: Some(llm_tx),
        tts_tx: Some(tts_tx),
        telemetry_tx,
        pipeline_tx: pipeline_tx.clone(),
        playback_engine,
        aec_handle: None,
        stt_handle: None,
        vad_handle: None,
        route_watcher_handle: None,
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: None,
    };

    let mut guard = state
        .engine
        .try_lock()
        .expect("state.engine mutex must be uncontended in test setup");
    *guard = Some(engine);

    (stt_tx, pipeline_rx, pipeline_tx)
}

// ============================================================================
// Production-parity scene rigs (capture -> AEC -> VAD -> router)
// ============================================================================

const SCENE_RING_BUFFER_SAMPLES: usize = 65536;

/// Attaches a VoxEngine built from the rig's real components so the production
/// interrupt path (`state.engine` -> `aec_tx` -> `AecCommand::ClearRenderQueue`)
/// and harness dispatch see the same engine the test drives.
#[allow(clippy::too_many_arguments)]
pub fn attach_rig_engine(
    state: &vox_lib::core::state::AppState,
    stt_tx: Sender<SttCommand>,
    vad_tx: Sender<VadCommand>,
    tts_tx: Option<Sender<TtsCommand>>,
    llm_tx: Option<Sender<LlmCommand>>,
    pipeline_tx: mpsc::Sender<VoxEvent>,
    aec_tx: mpsc::Sender<AecCommand>,
    playback_engine: Arc<PlaybackEngine>,
    aec_handle: Option<std::thread::JoinHandle<()>>,
    vad_handle: Option<std::thread::JoinHandle<()>>,
) {
    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();
    let engine = vox_lib::core::engine::VoxEngine {
        audio_stream: vox_lib::services::audio::AudioStream::mock(),
        stt_tx,
        vad_tx,
        aec_tx: Some(aec_tx),
        llm_tx,
        tts_tx,
        telemetry_tx,
        pipeline_tx,
        playback_engine,
        aec_handle,
        stt_handle: None,
        vad_handle,
        route_watcher_handle: None,
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: None,
    };
    if let Ok(mut guard) = state.engine.try_lock() {
        *guard = Some(engine);
    } else {
        *state.engine.blocking_lock() = Some(engine);
    }
}

/// Capture→AEC→VAD chain, wired identically to src/core/engine.rs.
/// Bundles the capture ring producer, VAD command channel, the AEC interrupt
/// sender, and the worker handles needed to attach a matching VoxEngine.
#[allow(clippy::too_many_arguments)]
struct WiredAudioScene {
    cap_prod: RbProducer,
    vad_tx: Sender<VadCommand>,
    aec_tx: mpsc::Sender<AecCommand>,
    aec_handle: std::thread::JoinHandle<()>,
    vad_handle: std::thread::JoinHandle<()>,
    audio_suppressed: Arc<AtomicBool>,
}

fn wire_audio_scene(
    state: &Arc<vox_lib::core::state::AppState>,
    ref_cons: ringbuf::HeapCons<f32>,
    stt_tx: Sender<SttCommand>,
    vox_event_tx: Sender<VoxEvent>,
    vad_config: VadActorConfig,
    vad_backend: VadBackend,
) -> WiredAudioScene {
    let cap_rb = HeapRb::<f32>::new(SCENE_RING_BUFFER_SAMPLES);
    let (cap_prod, cap_cons) = cap_rb.split();

    let clean_rb = HeapRb::<f32>::new(SCENE_RING_BUFFER_SAMPLES);
    let (clean_prod, clean_cons) = clean_rb.split();

    let (aec_tx, aec_rx) = mpsc::channel::<AecCommand>();
    let aec = AecProcessor::new(Some(ref_cons)).expect("Failed to create AecProcessor");
    let aec_handle = spawn_aec_worker(
        aec,
        cap_cons,
        clean_prod,
        Arc::clone(&state.pipeline.engine_shutdown),
        Some(aec_rx),
    )
    .expect("Failed to spawn AEC worker");

    let audio_suppressed = Arc::new(AtomicBool::new(false));
    let vad_handles = VadActorHandles {
        state_atomic: Arc::clone(&state.pipeline.current_state_atomic),
        turn_id_atomic: Arc::clone(&state.pipeline.turn_id),
        audio_suppressed: Arc::clone(&audio_suppressed),
        engine_shutdown: Arc::clone(&state.pipeline.engine_shutdown),
        dropped_counter: Arc::clone(&state.telemetry.dropped_telemetry_events),
        purged_frames_counter: Arc::new(AtomicU64::new(0)),
        ingestion_gate: Arc::clone(&state.pipeline.ingestion_gate),
        owner_atomic: Arc::clone(&state.owner),
    };

    let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();
    let vad_channels = VadActorChannels {
        stt_tx,
        vad_rx: vad_cmd_rx,
        telemetry_tx,
        vox_event_tx: Some(vox_event_tx),
    };
    let vad_handle = std::thread::Builder::new()
        .name("scene-vad-actor".to_string())
        .spawn(move || {
            spawn_vad_actor(vad_backend, clean_cons, vad_channels, vad_handles, vad_config)
                .expect("VAD actor failed");
        })
        .expect("Failed to spawn VAD actor thread");

    WiredAudioScene {
        cap_prod,
        vad_tx: vad_cmd_tx,
        aec_tx,
        aec_handle,
        vad_handle,
        audio_suppressed,
    }
}

/// Headless scene rig for capture->AEC->VAD->STT(chan)->router tests.
/// Everything on the test's path is a real production component; the STT
/// worker is not spawned so tests can inspect raw `SttCommand`s.
pub struct AudioSceneRig {
    pub cap_prod: RbProducer,
    pub stt_rx: Receiver<SttCommand>,
    pub vad_tx: Sender<VadCommand>,
    pub engine: Arc<PlaybackEngine>,
    pub sink: vox_lib::services::audio::sink::PlaybackStreamContext,
    pub router_join: std::thread::JoinHandle<()>,
    pub audio_suppressed: Arc<AtomicBool>,
}

impl AudioSceneRig {
    pub fn new(
        app: AppHandle<tauri::test::MockRuntime>,
        state: Arc<vox_lib::core::state::AppState>,
        vad_config: VadActorConfig,
        vad_backend: VadBackend,
    ) -> Self {
        let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();
        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();

        let (engine, sink, ref_cons) =
            create_headless_playback_with_aec_tap(&state, vox_event_tx.clone());

        let wired = wire_audio_scene(
            &state,
            ref_cons,
            stt_tx.clone(),
            vox_event_tx.clone(),
            vad_config,
            vad_backend,
        );

        attach_rig_engine(
            &state,
            stt_tx,
            wired.vad_tx.clone(),
            None,
            None,
            vox_event_tx,
            wired.aec_tx,
            Arc::clone(&engine),
            Some(wired.aec_handle),
            Some(wired.vad_handle),
        );

        let router_join = vox_lib::pipeline::router::spawn_router(app.clone(), vox_event_rx)
            .expect("Failed to spawn router thread");

        Self {
            cap_prod: wired.cap_prod,
            stt_rx,
            vad_tx: wired.vad_tx,
            engine,
            sink,
            router_join,
            audio_suppressed: wired.audio_suppressed,
        }
    }
}

/// End-to-end scene rig for full audio-in -> audio-out scenes.
/// Real AEC, real Nemotron STT, real Kokoro TTS, real VAD, real router, real
/// production PlaybackStreamContext. Nothing in the audio/router path is mocked.
pub struct E2eVoiceRig {
    pub cap_prod: RbProducer,
    pub vad_tx: Sender<VadCommand>,
    pub tts_tx: Sender<TtsCommand>,
    pub engine: Arc<PlaybackEngine>,
    pub sink: vox_lib::services::audio::sink::PlaybackStreamContext,
    pub router_join: std::thread::JoinHandle<()>,
    pub event_tx: mpsc::Sender<VoxEvent>,
}

impl E2eVoiceRig {
    /// Builds the rig with Kokoro synthesis enabled (full fidelity scenes).
    pub fn new(
        app: AppHandle<tauri::test::MockRuntime>,
        state: Arc<vox_lib::core::state::AppState>,
        vad_config: VadActorConfig,
        vad_backend: VadBackend,
    ) -> Self {
        Self::new_with_kokoro(app, state, vad_config, vad_backend, true)
    }

    /// Real LLM provider supplied by the caller (e.g. SidecarLlmProvider with Qwen).
    /// Kokoro synthesis runs so clause dispatch and playback are exercised with
    /// the provider's real output.
    pub fn new_with_external_llm(
        app: AppHandle<tauri::test::MockRuntime>,
        state: Arc<vox_lib::core::state::AppState>,
        vad_config: VadActorConfig,
        vad_backend: VadBackend,
        llm_provider: Arc<dyn vox_lib::services::llm::LlmProvider>,
    ) -> Self {
        Self::new_with_kokoro_override(app, state, vad_config, vad_backend, true, Some(llm_provider))
    }

    /// Builds the rig with Kokoro synthesis disabled. Clauses still dispatch to the
    /// TTS channel (and accumulate job counts), but no Kokoro worker consumes them —
    /// `PlaybackStarted` never fires, so `Thinking` persists deterministically.
    /// Used by the mid-thought continuation scene.
    pub fn new_without_kokoro(
        app: AppHandle<tauri::test::MockRuntime>,
        state: Arc<vox_lib::core::state::AppState>,
        vad_config: VadActorConfig,
        vad_backend: VadBackend,
    ) -> Self {
        Self::new_with_kokoro(app, state, vad_config, vad_backend, false)
    }

    fn new_with_kokoro(
        app: AppHandle<tauri::test::MockRuntime>,
        state: Arc<vox_lib::core::state::AppState>,
        vad_config: VadActorConfig,
        vad_backend: VadBackend,
        with_kokoro: bool,
    ) -> Self {
        Self::new_with_kokoro_override(app, state, vad_config, vad_backend, with_kokoro, None)
    }

    fn new_with_kokoro_override(
        app: AppHandle<tauri::test::MockRuntime>,
        state: Arc<vox_lib::core::state::AppState>,
        vad_config: VadActorConfig,
        vad_backend: VadBackend,
        with_kokoro: bool,
        llm_override: Option<Arc<dyn vox_lib::services::llm::LlmProvider>>,
    ) -> Self {
        let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();
        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();

        let (engine, sink, ref_cons) =
            create_headless_playback_with_aec_tap(&state, vox_event_tx.clone());

        let partial_emitter: Option<Arc<dyn Fn(u32, String) + Send + Sync>> = None;

        let nemotron_dir = super::paths::get_nemotron_model_dir();
        let stt_provider = Box::new(
            EmbeddedSttProvider::new(&nemotron_dir, "nemotron", 2)
                .expect("Failed to instantiate EmbeddedSttProvider with Nemotron"),
        ) as Box<dyn SttProvider>;
        let stt_channels = SttActorChannels {
            rx: stt_rx,
            pipeline_event_tx: Some(vox_event_tx.clone()),
            partial_emitter,
        };
        let stt_handles = SttActorHandles {
            cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
            engine_shutdown: Arc::clone(&state.pipeline.engine_shutdown),
        };
        spawn_stt_worker(stt_channels, stt_provider, stt_handles)
            .expect("Failed to spawn STT worker");

        let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
        let scripted_text = "Hello! How can I help you today?".to_string();
        let llm_provider = match llm_override {
            Some(provider) => provider,
            None => Arc::new(ScriptedLlmProvider::tokens(&scripted_text))
                as Arc<dyn vox_lib::services::llm::LlmProvider>,
        };
        let llm_worker = std::thread::Builder::new()
            .name("e2e-scripted-llm".to_string())
            .spawn(move || {
                spawn_llm_worker(llm_rx, llm_provider);
            })
            .expect("Failed to spawn scripted LLM worker");
        let _ = llm_worker;

        if with_kokoro {
            let kokoro_dir = super::paths::get_kokoro_model_dir();
            let voice = vox_lib::services::tts::voice::resolve_voice_index_for_provider(
                "kokoro",
                vox_lib::core::defaults::DEFAULT_TTS_KOKORO_VOICE_ID,
            )
            .expect("default Kokoro voice must resolve");
            let tts_provider = Box::new(
                KokoroEngine::new(&kokoro_dir, voice, 1.0, 4)
                    .expect("Failed to initialize Kokoro ONNX engine"),
            ) as Box<dyn TtsProvider>;
            let tts_handles = TtsWorkerHandles {
                playback: Arc::clone(&engine),
                event_tx: vox_event_tx.clone(),
                cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
                pending_synthesis_jobs: Some(Arc::clone(&state.pipeline.pending_synthesis_jobs)),
                telemetry_rtf: None,
                turn_metrics: Some(Arc::clone(&state.turn_metrics)),
            };
            std::thread::Builder::new()
                .name("e2e-kokoro-tts".to_string())
                .spawn(move || {
                    spawn_tts_worker(tts_rx, tts_provider, tts_handles);
                })
                .expect("Failed to spawn Kokoro TTS worker");
        }

        let wired = wire_audio_scene(
            &state,
            ref_cons,
            stt_tx.clone(),
            vox_event_tx.clone(),
            vad_config,
            vad_backend,
        );

        attach_rig_engine(
            &state,
            stt_tx,
            wired.vad_tx.clone(),
            Some(tts_tx.clone()),
            Some(llm_tx.clone()),
            vox_event_tx.clone(),
            wired.aec_tx,
            Arc::clone(&engine),
            Some(wired.aec_handle),
            Some(wired.vad_handle),
        );

        let router_join = vox_lib::pipeline::router::spawn_router(app.clone(), vox_event_rx)
            .expect("Failed to spawn router thread");

        Self {
            cap_prod: wired.cap_prod,
            vad_tx: wired.vad_tx,
            tts_tx,
            engine,
            sink,
            router_join,
            event_tx: vox_event_tx,
        }
    }
}

// ============================================================================
// ScriptedLlmProvider — deterministic LlmProvider for tests (no model weights)
// ============================================================================

#[derive(Debug, Clone)]
pub enum ScriptedEvent {
    Token(String),
    ToolCall(vox_lib::services::llm::CanonicalToolCall),
}

/// Deterministic, scripted LLM provider: replays a fixed list of passes, one event
/// batch per `generate` call, exactly like a real engine streaming a multi-pass turn.
/// Tests script each pass explicitly so tool-call-then-answer flows stay deterministic.
pub struct ScriptedLlmProvider {
    passes: parking_lot::Mutex<Vec<Vec<ScriptedEvent>>>,
    capabilities: vox_lib::services::llm::ProviderCapabilities,
    name: String,
}

impl ScriptedLlmProvider {
    /// Replays one Token per word (word-splitting keeps clause boundaries natural).
    pub fn tokens(text: &str) -> Self {
        let pass: Vec<ScriptedEvent> = text
            .split_inclusive(' ')
            .map(|word| ScriptedEvent::Token(word.to_string()))
            .collect();
        Self::from_passes(vec![pass], "scripted-tokens")
    }

    /// Replays a flat sequence of events in a single pass.
    pub fn from_script(script: Vec<ScriptedEvent>, name: &str) -> Self {
        Self::from_passes(vec![script], name)
    }

    /// Replays events batched into successive passes, consumed in order.
    pub fn from_passes(passes: Vec<Vec<ScriptedEvent>>, name: &str) -> Self {
        Self {
            passes: parking_lot::Mutex::new(passes),
            capabilities: vox_lib::services::llm::ProviderCapabilities::default(),
            name: name.to_string(),
        }
    }

    /// A provider that never streams anything (stream completes empty).
    pub fn empty() -> Self {
        Self::from_passes(Vec::new(), "scripted-empty")
    }

    pub fn tool_call(name: &str, arguments: serde_json::Value) -> vox_lib::services::llm::CanonicalToolCall {
        vox_lib::services::llm::CanonicalToolCall {
            id: format!("call-{name}"),
            name: name.to_string(),
            arguments,
        }
    }
}

impl vox_lib::services::llm::LlmProvider for ScriptedLlmProvider {
    fn generate<'a>(
        &'a self,
        _request: vox_lib::services::llm::GenerationRequest,
        _turn_id: u32,
        cancel: &'a tokio_util::sync::CancellationToken,
        tx: &'a mpsc::Sender<vox_lib::services::llm::LlmStreamEvent>,
    ) -> futures_util::future::BoxFuture<'a, Result<(), vox_lib::services::llm::LlmError>> {
        Box::pin(async move {
            if cancel.is_cancelled() {
                eprintln!("[scripted-llm] generate: cancelled before start");
                return Err(vox_lib::services::llm::LlmError::Cancelled);
            }
            let pass = {
                let mut passes = self.passes.lock();
                if passes.is_empty() {
                    Vec::new()
                } else {
                    passes.remove(0)
                }
            };
            for event in &pass {
                let out = match event {
                    ScriptedEvent::Token(token) => {
                        vox_lib::services::llm::LlmStreamEvent::Token(token.clone())
                    }
                    ScriptedEvent::ToolCall(call) => {
                        vox_lib::services::llm::LlmStreamEvent::ToolCall(call.clone())
                    }
                };
                if tx.send(out).is_err() {
                    return Err(vox_lib::services::llm::LlmError::Other(anyhow::anyhow!(
                        "stream receiver dropped during {}",
                        self.name
                    )));
                }
            }
            let _ = tx.send(vox_lib::services::llm::LlmStreamEvent::Finished);
            Ok(())
        })
    }

    fn capabilities(&self) -> &vox_lib::services::llm::ProviderCapabilities {
        &self.capabilities
    }

    fn health_check<'a>(
        &'a self,
    ) -> futures_util::future::BoxFuture<'a, Result<(), vox_lib::services::llm::LlmError>> {
        Box::pin(async { Ok(()) })
    }

    fn list_models<'a>(
        &'a self,
    ) -> futures_util::future::BoxFuture<'a, Result<Vec<vox_lib::services::llm::LlmModelInfo>, vox_lib::services::llm::LlmError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn kind(&self) -> vox_lib::services::llm::ProviderKind {
        // Warmup is skipped for non-Embedded providers (see services/llm/actor.rs),
        // which keeps the scripted script intact for the actual turn.
        vox_lib::services::llm::ProviderKind::OpenAiCompat
    }
}


// ============================================================================
// Shared scene helpers (E2E crowd scenes)
// ============================================================================

/// Simulated render→capture hardware skew (PipeWire/ALSA vs the 40 ms production default).
pub const PIPELINE_DELAY_SAMPLES: usize = 1920; // 120 ms @ 16 kHz
/// Microphone attenuation applied to the simulated speaker output.
pub const PIPELINE_ATTENUATION: f32 = 0.5;

/// Mic timeline: playback echo arriving 120 ms late (hardware skew) plus the
/// user's simultaneous voice. Shared by the E2E crowd scenes.
pub fn scene_mic(echo_source: &[f32], user: Option<&[f32]>) -> Vec<f32> {
    let mut mic = vec![0.0f32; PIPELINE_DELAY_SAMPLES];
    mic.extend(echo_source.iter().map(|s| s * PIPELINE_ATTENUATION));
    if let Some(u) = user {
        for (i, s) in u.iter().enumerate() {
            if i < mic.len() {
                mic[i] += *s;
            } else {
                mic.push(*s);
            }
        }
    }
    mic
}

/// Watches InteractionState transitions across a bounded window and records which
/// states were observed (bits: 1=Listening, 2=Thinking, 4=Speaking, 8=Ready).
/// Spawn before feeding audio to capture transient states.
pub fn spawn_state_watcher(
    state: &Arc<vox_lib::core::state::AppState>,
    window: core::time::Duration,
) -> (Arc<std::sync::atomic::AtomicU32>, std::thread::JoinHandle<()>) {
    let seen = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let seen_clone = Arc::clone(&seen);
    let state = Arc::clone(state);
    let handle = std::thread::Builder::new()
        .name("e2e-state-watcher".to_string())
        .spawn(move || {
            let deadline = std::time::Instant::now() + window;
            while std::time::Instant::now() < deadline {
                let current = state.pipeline.state();
                let bits = match current {
                    vox_lib::core::state::InteractionState::Listening => 0b001,
                    vox_lib::core::state::InteractionState::Thinking => 0b010,
                    vox_lib::core::state::InteractionState::Speaking => 0b100,
                    vox_lib::core::state::InteractionState::Ready => 0b1000,
                    _ => 0,
                };
                if bits != 0 {
                    seen_clone.fetch_or(bits, std::sync::atomic::Ordering::SeqCst);
                }
                if seen_clone.load(std::sync::atomic::Ordering::SeqCst) == 0b1111 {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        })
        .expect("spawn state watcher");
    (seen, handle)
}


