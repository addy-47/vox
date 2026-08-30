//! ============================================================================
//! tests/common/harness.rs — Test Harness Constructors & Actor Lifecycle Helpers
//! ============================================================================

use ringbuf::traits::Split;
use ringbuf::wrap::caching::Caching;
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::state::VadCommand;
use vox_lib::services::stt::actor::{
    spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand,
};
use vox_lib::services::stt::providers::{EmbeddedSttProvider, SttProvider};
use vox_lib::services::vad::actor::{
    spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles,
};
use vox_lib::services::vad::earshot_vad::EarshotVadEngine;
use vox_lib::services::vad::VadBackend;

pub type RbProducer = Caching<Arc<HeapRb<f32>>, true, false>;

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
        EmbeddedSttProvider::new(&nemotron_dir, "nemotron")
            .expect("Failed to instantiate EmbeddedSttProvider with Nemotron"),
    ) as Box<dyn SttProvider>;

    let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();
    let (pipeline_event_tx, pipeline_event_rx) = mpsc::channel::<VoxEvent>();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let is_loaded = Arc::new(AtomicBool::new(false));
    let engine_shutdown = Arc::new(AtomicBool::new(false));

    let channels = SttActorChannels {
        rx: stt_rx,
        pipeline_event_tx: Some(pipeline_event_tx),
    };

    let handles = SttActorHandles {
        cancel_flag,
        is_loaded,
        engine_shutdown: engine_shutdown.clone(),
    };

    let join_handle = spawn_stt_worker(channels, provider, handles)
        .expect("Failed to spawn STT worker");

    (stt_tx, pipeline_event_rx, engine_shutdown, join_handle)
}

/// Spawns the production VAD actor and returns the ring buffer producer along with channels.
pub fn setup_vad_actor(
    stt_tx: Sender<SttCommand>,
    config: VadActorConfig,
    state_atomic: Arc<AtomicU32>,
    audio_suppressed: Arc<AtomicBool>,
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
        is_loaded: Arc::new(AtomicBool::new(false)),
        state_atomic,
        turn_id_atomic: Arc::new(AtomicU32::new(0)),
        audio_suppressed,
        engine_shutdown,
        dropped_counter: Arc::new(AtomicU64::new(0)),
    };

    let join_handle = std::thread::Builder::new()
        .name("test-vad-actor".to_string())
        .spawn(move || {
            spawn_vad_actor(
                vad_backend,
                consumer,
                vad_channels,
                vad_handles,
                config,
            )
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

/// Constructs an AppHandle and managed AppState pair tailored for testing environments.
pub fn get_test_app_and_state() -> (AppHandle<tauri::test::MockRuntime>, Arc<vox_lib::core::state::AppState>) {
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
    use vox_lib::core::state::TelemetryState;
    use tauri::Manager;

    let app = get_test_app_handle();
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
    let state = Arc::new(vox_lib::core::state::AppState::new(&app, None, telemetry));
    app.manage(state.clone());
    (app, state)
}

/// Constructs an AppState instance tailored for testing environments.
pub fn get_test_app_state() -> vox_lib::core::state::AppState {
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
    use vox_lib::core::state::TelemetryState;

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
    let app = get_test_app_handle();
    vox_lib::core::state::AppState::new(&app, None, telemetry)
}

/// Attaches a mock VoxEngine with a specified VAD command sender to the managed AppState.
pub fn attach_mock_engine_with_vad_to_state<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &vox_lib::core::state::AppState,
    stt_tx: std::sync::mpsc::Sender<SttCommand>,
    vad_tx: std::sync::mpsc::Sender<VadCommand>,
) {
    let (pipeline_tx, _) = std::sync::mpsc::channel();
    let (telemetry_tx, _) = crossbeam_channel::unbounded();
    let playback_engine = Arc::new(
        vox_lib::services::audio::PlaybackEngine::new(
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::clone(&state.pipeline.current_state_atomic),
            vox_lib::services::audio::playback::PlaybackTelemetryHandles {
                energy: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                low: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                mid: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                high: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                underruns: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            },
        )
        .expect("Failed to create mock PlaybackEngine"),
    );

    let engine = vox_lib::core::state::VoxEngine {
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
    state.pipeline.set_state(vox_lib::core::state::InteractionState::Ready);
}

/// Attaches a mock VoxEngine to the managed AppState for testing full production pipeline flows.
pub fn attach_mock_engine_to_state<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &vox_lib::core::state::AppState,
    stt_tx: std::sync::mpsc::Sender<SttCommand>,
) {
    let (vad_tx, _) = std::sync::mpsc::channel();
    attach_mock_engine_with_vad_to_state(app, state, stt_tx, vad_tx);
}
