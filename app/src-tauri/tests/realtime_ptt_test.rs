//! ============================================================================
//! realtime_ptt_test.rs — Realtime PTT & Ghost Audio Gate Integration Tests (Seams 3 & 6)
//! ============================================================================
//! Category     : Integration Test
//! Component    : pipeline/realtime/ptt, services/realtime
//! Prerequisites: None (Isolated mock RealtimeEngine / provider)
//! Execution    : cargo test --test realtime_ptt_test --release -- --nocapture
//! Metrics      : Ghost Audio Gate Rejection, Buffer Lifecycle Integrity
//! ============================================================================

mod common;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::audio::{
    decode_wav_to_mono_16k, stream_audio_to_ring_buffer, stream_silence_frames,
    wait_for_buffer_drain,
};
use common::harness::{
    attach_mock_engine_with_vad_to_state, get_test_app_and_state, setup_vad_actor,
};
use common::paths::get_asset_path;

use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{AudioOutputMode, InteractionMode, RealtimeProviderKind};
use vox_lib::core::state::{InteractionState, VadCommand};
use vox_lib::pipeline::realtime::ptt::{ptt_cancel, ptt_start, ptt_stop};
use vox_lib::services::realtime::engine::RealtimeEngine;
use vox_lib::services::realtime::{RealtimeAudioConfig, RealtimeSession, RealtimeVoiceProvider};
use vox_lib::services::vad::VadActorConfig;

/// Mock Realtime Session that counts audio chunks pushed and activity signals.
struct MockRealtimeSession {
    push_counter: Arc<AtomicUsize>,
    activity_start_counter: Arc<AtomicUsize>,
    activity_end_counter: Arc<AtomicUsize>,
}

impl RealtimeSession for MockRealtimeSession {
    fn send_audio(&self, pcm: &[i16]) -> anyhow::Result<()> {
        self.push_counter.fetch_add(pcm.len(), Ordering::Relaxed);
        Ok(())
    }

    fn cancel(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn disconnect(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn activity_start(&self) -> anyhow::Result<()> {
        self.activity_start_counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn activity_end(&self) -> anyhow::Result<()> {
        self.activity_end_counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

/// Mock Realtime Voice Provider factory that creates `MockRealtimeSession`.
struct MockRealtimeProvider {
    push_counter: Arc<AtomicUsize>,
    activity_start_counter: Arc<AtomicUsize>,
    activity_end_counter: Arc<AtomicUsize>,
}

impl RealtimeVoiceProvider for MockRealtimeProvider {
    fn kind(&self) -> RealtimeProviderKind {
        RealtimeProviderKind::GeminiLive
    }

    fn audio_config(&self) -> RealtimeAudioConfig {
        RealtimeAudioConfig {
            input_sample_rate: 16000,
            output_sample_rate: 24000,
            requires_input_resampling: false,
            requires_output_resampling: false,
        }
    }

    fn connect(
        &self,
        _interaction_mode: InteractionMode,
        _playback_tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        _event_tx: std::sync::mpsc::Sender<VoxEvent>,
    ) -> anyhow::Result<Box<dyn RealtimeSession>> {
        Ok(Box::new(MockRealtimeSession {
            push_counter: self.push_counter.clone(),
            activity_start_counter: self.activity_start_counter.clone(),
            activity_end_counter: self.activity_end_counter.clone(),
        }))
    }

    fn health_check(&self) -> bool {
        true
    }
}

fn create_mock_engine(
    push_counter: Arc<AtomicUsize>,
    activity_start_counter: Arc<AtomicUsize>,
    activity_end_counter: Arc<AtomicUsize>,
    handle: tokio::runtime::Handle,
) -> RealtimeEngine {
    let provider = Box::new(MockRealtimeProvider {
        push_counter,
        activity_start_counter,
        activity_end_counter,
    });
    let mut engine = RealtimeEngine::new(provider, handle);

    let (dummy_tx, _) = std::sync::mpsc::channel();
    let playback = Arc::new(
        vox_lib::services::audio::PlaybackEngine::new(
            Arc::new(AtomicBool::new(false)),
            Arc::new(std::sync::atomic::AtomicU32::new(0)),
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

    engine
        .start(
            InteractionMode::PTT,
            playback,
            dummy_tx,
            Arc::new(std::sync::atomic::AtomicU32::new(1)),
        )
        .expect("Failed to start mock RealtimeEngine");

    engine
}

/// Invariant (NEGATIVE): Ghost audio gate rejects silence-only PTT holds and prevents cloud transmission.
#[tokio::test]
async fn test_realtime_ptt_ghost_audio_gate_rejects_non_speech() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(10);

    let (app, state) = get_test_app_and_state();

    let push_counter = Arc::new(AtomicUsize::new(0));
    let activity_start_counter = Arc::new(AtomicUsize::new(0));
    let activity_end_counter = Arc::new(AtomicUsize::new(0));
    let mock_engine = create_mock_engine(
        push_counter.clone(),
        activity_start_counter.clone(),
        activity_end_counter.clone(),
        tokio::runtime::Handle::current(),
    );
    *state.realtime_engine.lock().await = Some(mock_engine);

    let (stt_tx, _) = std::sync::mpsc::channel();
    let engine_shutdown = Arc::new(AtomicBool::new(false));
    let vad_config = VadActorConfig {
        initial_threshold: 0.5,
        initial_noise_gate: 0.01,
        initial_mode: InteractionMode::PTT,
        initial_audio_mode: AudioOutputMode::Speaker,
    };
    let (vad_cmd_tx, _vad_vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        stt_tx.clone(),
        vad_config,
        Arc::clone(&state.pipeline.current_state_atomic),
        Arc::new(AtomicBool::new(false)),
        engine_shutdown.clone(),
    );

    attach_mock_engine_with_vad_to_state(&app, &state, stt_tx, vad_cmd_tx.clone());

    // 1. Press PTT
    ptt_start(&app, &state).expect("ptt_start failed");
    assert_eq!(
        state.pipeline.state(),
        InteractionState::Listening,
        "State must be Listening after ptt_start"
    );
    assert_eq!(
        activity_start_counter.load(Ordering::Relaxed),
        1,
        "activity_start must be signaled on ptt_start"
    );

    // 2. Stream purely silent frames into SPSC ring buffer
    stream_silence_frames(&mut producer, 20);
    wait_for_buffer_drain(&producer, 1);

    // 3. Release PTT
    ptt_stop(&app, &state).await.expect("ptt_stop failed");

    // Settle async propagation
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 4. Assert Ghost Audio Gate rejected non-speech -> reverted to Ready
    assert_eq!(
        state.pipeline.state(),
        InteractionState::Ready,
        "Ghost audio gate must revert state to Ready on non-speech release"
    );
    assert_eq!(
        push_counter.load(Ordering::Relaxed),
        0,
        "Zero audio samples must be pushed to RealtimeEngine when speech is absent"
    );
    assert_eq!(
        activity_end_counter.load(Ordering::Relaxed),
        0,
        "activity_end must NOT be called when speech validation fails"
    );

    // Teardown
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    vad_handle
        .join()
        .expect("VAD actor thread panicked during ghost audio gate teardown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "Ghost audio gate test exceeded hard timeout of 10s"
    );
}

/// Positive: Valid speech during PTT is trimmed and flushed to the active RealtimeEngine.
#[tokio::test]
async fn test_realtime_ptt_speech_detected_flushes_to_engine() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(10);

    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

    let (app, state) = get_test_app_and_state();

    let push_counter = Arc::new(AtomicUsize::new(0));
    let activity_start_counter = Arc::new(AtomicUsize::new(0));
    let activity_end_counter = Arc::new(AtomicUsize::new(0));
    let mock_engine = create_mock_engine(
        push_counter.clone(),
        activity_start_counter.clone(),
        activity_end_counter.clone(),
        tokio::runtime::Handle::current(),
    );
    *state.realtime_engine.lock().await = Some(mock_engine);

    let (stt_tx, _) = std::sync::mpsc::channel();
    let engine_shutdown = Arc::new(AtomicBool::new(false));
    let vad_config = VadActorConfig {
        initial_threshold: 0.5,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::PTT,
        initial_audio_mode: AudioOutputMode::Speaker,
    };
    let (vad_cmd_tx, _vad_vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        stt_tx.clone(),
        vad_config,
        Arc::clone(&state.pipeline.current_state_atomic),
        Arc::new(AtomicBool::new(false)),
        engine_shutdown.clone(),
    );

    attach_mock_engine_with_vad_to_state(&app, &state, stt_tx, vad_cmd_tx.clone());

    // 1. Press PTT
    ptt_start(&app, &state).expect("ptt_start failed");
    assert_eq!(
        state.pipeline.state(),
        InteractionState::Listening,
        "State must be Listening after ptt_start"
    );
    assert_eq!(
        activity_start_counter.load(Ordering::Relaxed),
        1,
        "activity_start must be signaled on ptt_start"
    );

    // 2. Stream real speech into SPSC ring buffer
    stream_audio_to_ring_buffer(&audio, &mut producer);
    wait_for_buffer_drain(&producer, 1);

    // 3. Release PTT
    ptt_stop(&app, &state).await.expect("ptt_stop failed");

    // Settle async propagation
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 4. Assert speech was validated and flushed to RealtimeEngine
    assert_eq!(
        state.pipeline.state(),
        InteractionState::Thinking,
        "State must be Thinking after speech flushed to RealtimeEngine"
    );
    assert!(
        push_counter.load(Ordering::Relaxed) > 0,
        "Audio samples must be dispatched to RealtimeEngine when speech is detected"
    );
    assert_eq!(
        activity_end_counter.load(Ordering::Relaxed),
        1,
        "activity_end must be signaled when speech is validated and flushed"
    );

    // Teardown
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    vad_handle
        .join()
        .expect("VAD actor thread panicked during speech flush teardown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "Speech flush test exceeded hard timeout of 10s"
    );
}

/// Guard (NEGATIVE): Cancelling PTT clears accumulated audio and prevents cloud transmission.
#[tokio::test]
async fn test_realtime_ptt_cancel_discards_audio() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(10);

    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

    let (app, state) = get_test_app_and_state();

    let push_counter = Arc::new(AtomicUsize::new(0));
    let activity_start_counter = Arc::new(AtomicUsize::new(0));
    let activity_end_counter = Arc::new(AtomicUsize::new(0));
    let mock_engine = create_mock_engine(
        push_counter.clone(),
        activity_start_counter.clone(),
        activity_end_counter.clone(),
        tokio::runtime::Handle::current(),
    );
    *state.realtime_engine.lock().await = Some(mock_engine);

    let (stt_tx, _) = std::sync::mpsc::channel();
    let engine_shutdown = Arc::new(AtomicBool::new(false));
    let vad_config = VadActorConfig {
        initial_threshold: 0.5,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::PTT,
        initial_audio_mode: AudioOutputMode::Speaker,
    };
    let (vad_cmd_tx, _vad_vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        stt_tx.clone(),
        vad_config,
        Arc::clone(&state.pipeline.current_state_atomic),
        Arc::new(AtomicBool::new(false)),
        engine_shutdown.clone(),
    );

    attach_mock_engine_with_vad_to_state(&app, &state, stt_tx, vad_cmd_tx.clone());

    // 1. Press PTT
    ptt_start(&app, &state).expect("ptt_start failed");
    assert_eq!(
        state.pipeline.state(),
        InteractionState::Listening,
        "State must be Listening after ptt_start"
    );
    assert_eq!(
        activity_start_counter.load(Ordering::Relaxed),
        1,
        "activity_start must be signaled on ptt_start"
    );

    // 2. Stream real speech into SPSC ring buffer
    stream_audio_to_ring_buffer(&audio, &mut producer);
    wait_for_buffer_drain(&producer, 1);

    // 3. Cancel PTT
    ptt_cancel(&app, &state).expect("ptt_cancel failed");

    // 4. Assert State reverted to Ready, turn is cancelled, and 0 samples pushed
    assert_eq!(
        state.pipeline.state(),
        InteractionState::Ready,
        "State must revert to Ready upon PTT cancel"
    );
    assert!(
        state.pipeline.turn_token().is_cancelled(),
        "Current turn token must be marked cancelled after ptt_cancel"
    );
    assert_eq!(
        push_counter.load(Ordering::Relaxed),
        0,
        "Zero audio samples must be pushed to RealtimeEngine when cancelled"
    );
    assert_eq!(
        activity_end_counter.load(Ordering::Relaxed),
        0,
        "activity_end must NOT be called on cancel"
    );

    // Teardown
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    vad_handle
        .join()
        .expect("VAD actor thread panicked during cancel teardown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "PTT cancel test exceeded hard timeout of 10s"
    );
}
