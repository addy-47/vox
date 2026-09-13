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
use ringbuf::{traits::Split, wrap::caching::Caching, HeapCons, HeapRb};
use tauri::AppHandle;
use vox_lib::{
    core::events::VoxEvent,
    services::{
        audio::PlaybackEngine,
        llm::actor::LlmCommand,
        stt::{
            actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand},
            EmbeddedSttProvider, SttProvider,
        },
        tts::actor::TtsCommand,
        vad::{
            actor::{spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles},
            earshot_vad::EarshotVadEngine,
            VadBackend, VadCommand,
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
    };

    let engine = PlaybackEngine::from_parts(producer, handles, discard_request, turn_armed, None);

    (Arc::new(engine), Arc::new(Mutex::new(consumer)))
}

/// Creates a headless playback engine and real sink context with caller-provided event channel and atomics.
pub fn create_headless_playback_with_sink(
    event_tx: mpsc::Sender<VoxEvent>,
    state_atomic: Arc<AtomicU32>,
    current_turn_id: Arc<AtomicU32>,
    pending_synthesis_jobs: Arc<AtomicU32>,
) -> (
    Arc<PlaybackEngine>,
    vox_lib::services::audio::sink::PlaybackStreamContext,
) {
    let rb = HeapRb::<f32>::new(vox_lib::services::audio::PLAYBACK_BUFFER_SAMPLES);
    let (producer, consumer) = rb.split();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let discard_request = Arc::new(AtomicBool::new(false));
    let turn_armed = Arc::new(AtomicBool::new(false));

    let handles = vox_lib::services::audio::playback::PlaybackEngineHandles {
        cancel_flag,
        state_atomic,
        current_turn_id,
        pending_synthesis_jobs,
        playback_intent: Arc::new(AtomicU8::new(0)),
        event_tx,
    };

    let telemetry = vox_lib::services::audio::PlaybackTelemetryHandles {
        energy: Arc::new(AtomicU32::new(0)),
        low: Arc::new(AtomicU32::new(0)),
        mid: Arc::new(AtomicU32::new(0)),
        high: Arc::new(AtomicU32::new(0)),
        underruns: Arc::new(AtomicU64::new(0)),
    };

    let sink = vox_lib::services::audio::sink::PlaybackStreamContext::new(
        consumer,
        handles.clone(),
        Arc::clone(&discard_request),
        Arc::clone(&turn_armed),
        &telemetry,
    );

    let engine = PlaybackEngine::from_parts(producer, handles, discard_request, turn_armed, None);

    (Arc::new(engine), sink)
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

/// Spawns the production VAD actor and returns the ring buffer producer along with channels.
pub fn setup_vad_actor(
    stt_tx: Sender<SttCommand>,
    config: VadActorConfig,
    state_atomic: Arc<AtomicU32>,
    turn_id_atomic: Arc<AtomicU32>,
    audio_suppressed: Arc<AtomicBool>,
    ingestion_gate: Arc<AtomicBool>,
    engine_shutdown: Arc<AtomicBool>,
) -> (
    Sender<VadCommand>,
    Receiver<VoxEvent>,
    RbProducer,
    std::thread::JoinHandle<()>,
) {
    let rb = HeapRb::<f32>::new(65536);
    let (producer, consumer) = rb.split();

    let vad_engine = EarshotVadEngine::new(config.initial_threshold)
        .expect("Failed to initialize Earshot VAD engine");
    let vad_backend = VadBackend::Earshot(vad_engine);
    let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();
    let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();

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
        ingestion_gate,
    };

    let join_handle = std::thread::Builder::new()
        .name("test-vad-actor".to_string())
        .spawn(move || {
            spawn_vad_actor(vad_backend, consumer, vad_channels, vad_handles, config)
                .expect("VAD actor failed");
        })
        .expect("Failed to spawn VAD actor thread");

    (vad_cmd_tx, vox_event_rx, producer, join_handle)
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
            Ok(VoxEvent::TranscriptFinal { turn_id, text }) => {
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

/// Asserts that a standard mpsc::Receiver is empty after a deterministic wait.
/// Mandatory for negative assertion / suppression testing.
pub fn assert_channel_empty_after<T: std::fmt::Debug>(
    rx: &Receiver<T>,
    wait: Duration,
    label: &str,
) {
    std::thread::sleep(wait);
    if let Ok(item) = rx.try_recv() {
        panic!(
            "[{}] Negative assertion failed: expected empty channel, but found item: {:?}",
            label, item
        );
    }
}

/// Constructs the default test `TelemetryState` with fresh atomics and a
/// dropped telemetry channel. Single home for the block previously duplicated
/// across `get_test_app_and_state` / `get_test_app_state`.
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

/// Constructs an AppHandle and managed AppState pair tailored for testing environments.
pub async fn get_test_app_and_state() -> (
    AppHandle<tauri::test::MockRuntime>,
    Arc<vox_lib::core::state::AppState>,
) {
    use tauri::Manager;

    let app = get_test_app_handle();
    let telemetry = make_test_telemetry();

    vox_lib::utils::paths::init();
    let db_conn = vox_lib::persistence::db::VoxDb::open(&vox_lib::utils::paths::db_path())
        .await
        .expect("Failed to open test database");
    let db = Arc::new(db_conn);
    let state = Arc::new(vox_lib::core::state::AppState::new(
        &app, None, telemetry, db,
    ));
    app.manage(state.clone());
    (app, state)
}

/// Constructs an AppState instance tailored for testing environments.
pub async fn get_test_app_state() -> vox_lib::core::state::AppState {
    let telemetry = make_test_telemetry();

    vox_lib::utils::paths::init();
    let db_conn = vox_lib::persistence::db::VoxDb::open(&vox_lib::utils::paths::db_path())
        .await
        .expect("Failed to open test database");
    let db = Arc::new(db_conn);
    let app = get_test_app_handle();
    vox_lib::core::state::AppState::new(&app, None, telemetry, db)
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
        llm_tx: None,
        tts_tx: None,
        telemetry_tx,
        pipeline_tx,
        playback_engine,
        stt_handle: None,
        vad_handle: None,
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
        llm_tx,
        tts_tx,
        telemetry_tx,
        pipeline_tx,
        playback_engine,
        stt_handle: None,
        vad_handle: None,
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
        llm_tx,
        tts_tx,
        telemetry_tx,
        pipeline_tx,
        playback_engine,
        stt_handle: None,
        vad_handle: None,
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
        llm_tx: Some(llm_tx),
        tts_tx: Some(tts_tx),
        telemetry_tx,
        pipeline_tx: pipeline_tx.clone(),
        playback_engine,
        stt_handle: None,
        vad_handle: None,
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
