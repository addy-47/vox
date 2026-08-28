//! ============================================================================
//! realtime_ptt_test.rs — Realtime PTT & Ghost Audio Gate Integration Tests (Seams 3 & 6)
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/pipeline/realtime_ptt, services/realtime
//! Prerequisites: None (Isolated mock RealtimeEngine / provider)
//! Execution    : cargo test --test realtime_ptt_test --release -- --nocapture
//! Metrics      : Ghost Audio Gate Rejection, Buffer Lifecycle Integrity
//! ============================================================================

mod common;

use common::audio::decode_wav_to_mono_16k;
use common::harness::get_test_app_and_state;
use common::paths::get_asset_path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{InteractionMode, RealtimeProviderKind};
use vox_lib::services::audio::playback::PlaybackTelemetryHandles;
use vox_lib::services::audio::PlaybackEngine;
use vox_lib::services::pipeline::realtime::ptt::{
    get_buffer_len, handle_event, handle_ptt_cancel, handle_ptt_start, handle_ptt_stop,
    ingest_audio, is_recording,
};
use vox_lib::services::realtime::engine::RealtimeEngine;
use vox_lib::services::realtime::{RealtimeAudioConfig, RealtimeSession, RealtimeVoiceProvider};
use vox_lib::services::vad::VAD_CHUNK_SIZE;

struct MockSession {
    send_audio_counter: Arc<AtomicUsize>,
}

impl RealtimeSession for MockSession {
    fn send_audio(&self, _pcm: &[i16]) -> anyhow::Result<()> {
        self.send_audio_counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn cancel(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn disconnect(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn activity_start(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn activity_end(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

struct MockProvider {
    send_audio_counter: Arc<AtomicUsize>,
}

impl RealtimeVoiceProvider for MockProvider {
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
        Ok(Box::new(MockSession {
            send_audio_counter: self.send_audio_counter.clone(),
        }))
    }
    fn health_check(&self) -> bool {
        true
    }
}

fn create_mock_playback_engine() -> Arc<PlaybackEngine> {
    let telemetry = PlaybackTelemetryHandles {
        energy: Arc::new(AtomicU32::new(0)),
        low: Arc::new(AtomicU32::new(0)),
        mid: Arc::new(AtomicU32::new(0)),
        high: Arc::new(AtomicU32::new(0)),
        underruns: Arc::new(AtomicU64::new(0)),
    };
    Arc::new(
        PlaybackEngine::new(
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            telemetry,
        )
        .expect("Failed to initialize PlaybackEngine"),
    )
}

fn create_mock_engine(push_counter: Arc<AtomicUsize>, handle: tokio::runtime::Handle) -> RealtimeEngine {
    let mock_provider = Box::new(MockProvider {
        send_audio_counter: push_counter,
    });
    let mut engine = RealtimeEngine::new(mock_provider, handle);
    let playback_engine = create_mock_playback_engine();
    let (event_tx, _event_rx) = std::sync::mpsc::channel::<VoxEvent>();
    engine
        .start(InteractionMode::PTT, playback_engine, event_tx)
        .expect("Failed to start mock RealtimeEngine");
    engine
}

/// Guard (NEGATIVE): Silence/noise PTT hold where SPEECH_DETECTED=false must be rejected at the Ghost Audio Gate.
#[tokio::test]
async fn test_realtime_ptt_ghost_audio_gate_rejects_non_speech() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (app, state) = get_test_app_and_state();

        let push_counter = Arc::new(AtomicUsize::new(0));
        let mock_engine = create_mock_engine(push_counter.clone(), tokio::runtime::Handle::current());
        *state.realtime_engine.lock().await = Some(mock_engine);

        // 1. Upstream Trigger: Start Realtime PTT recording
        handle_ptt_start(&app, &state).expect("handle_ptt_start failed");
        assert!(is_recording(), "IS_RECORDING must be true after start");

        // 2. Ingest 10 frames of silence / background noise (speech start event not dispatched)
        let silence_chunk = vec![0.0001f32; VAD_CHUNK_SIZE];
        for _ in 0..10 {
            ingest_audio(&silence_chunk);
        }
        assert_eq!(get_buffer_len(), 10 * VAD_CHUNK_SIZE, "Buffer must contain ingested frames");

        // 3. Stop PTT hold with production state.realtime_engine dispatch
        handle_ptt_stop(&app, &state).expect("handle_ptt_stop failed");

        // Allow any asynchronous background tasks (such as AudioBridge channel dispatch) to settle
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 4. Assertions: Ghost Audio Gate activated -> Buffer cleared, ZERO cloud pushes
        assert!(!is_recording(), "IS_RECORDING must be false after stop");
        assert_eq!(get_buffer_len(), 0, "REALTIME_PTT_BUFFER must be cleared without dispatch");
        assert_eq!(
            push_counter.load(Ordering::Relaxed),
            0,
            "Ghost Audio Gate must prevent any audio from being pushed to RealtimeEngine"
        );
    })
    .await
    .expect("test_realtime_ptt_ghost_audio_gate_rejects_non_speech timed out");
}

/// Tests that when speech is detected during PTT hold, audio is dispatched to RealtimeEngine.
#[tokio::test]
async fn test_realtime_ptt_speech_detected_flushes_to_engine() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
        let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

        let (app, state) = get_test_app_and_state();

        let push_counter = Arc::new(AtomicUsize::new(0));
        let mock_engine = create_mock_engine(push_counter.clone(), tokio::runtime::Handle::current());
        *state.realtime_engine.lock().await = Some(mock_engine);

        // 1. Start Realtime PTT recording
        handle_ptt_start(&app, &state).expect("handle_ptt_start failed");
        assert!(is_recording(), "IS_RECORDING must be true after start");

        // 2. Ingest speech audio frames and dispatch speech onset event
        for chunk in audio.chunks(VAD_CHUNK_SIZE) {
            ingest_audio(chunk);
        }
        let playback_engine = create_mock_playback_engine();
        handle_event(
            &app,
            &state,
            &playback_engine,
            VoxEvent::SpeechStart { turn_id: 1 },
        );
        assert!(get_buffer_len() >= audio.len(), "Buffer must contain ingested audio");

        // 3. Stop PTT hold -> should flush accumulated audio to engine
        handle_ptt_stop(&app, &state).expect("handle_ptt_stop failed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(!is_recording(), "IS_RECORDING must be false after stop");
        assert_eq!(get_buffer_len(), 0, "Buffer must be drained on release");
        assert!(
            push_counter.load(Ordering::Relaxed) > 0,
            "RealtimeEngine should have received pushed audio chunks"
        );
    })
    .await
    .expect("test_realtime_ptt_speech_detected_flushes_to_engine timed out");
}

/// Guard (NEGATIVE): Cancelling Realtime PTT must clear buffer and abort dispatch.
#[tokio::test]
async fn test_realtime_ptt_cancel_discards_audio() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
        let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

        let (app, state) = get_test_app_and_state();

        let push_counter = Arc::new(AtomicUsize::new(0));
        let mock_engine = create_mock_engine(push_counter.clone(), tokio::runtime::Handle::current());
        *state.realtime_engine.lock().await = Some(mock_engine);

        // 1. Start Realtime PTT recording
        handle_ptt_start(&app, &state).expect("handle_ptt_start failed");
        assert!(is_recording(), "IS_RECORDING must be true");

        // 2. Ingest audio frames and dispatch speech onset event
        for chunk in audio.chunks(VAD_CHUNK_SIZE) {
            ingest_audio(chunk);
        }
        let playback_engine = create_mock_playback_engine();
        handle_event(
            &app,
            &state,
            &playback_engine,
            VoxEvent::SpeechStart { turn_id: 1 },
        );
        assert!(get_buffer_len() > 0, "Buffer must contain audio frames");

        // 3. Cancel PTT
        handle_ptt_cancel(&app, &state).expect("handle_ptt_cancel failed");

        // 4. Assertions
        assert!(!is_recording(), "IS_RECORDING must be false after cancel");
        assert_eq!(get_buffer_len(), 0, "Buffer must be cleared on cancel");
        assert_eq!(
            push_counter.load(Ordering::Relaxed),
            0,
            "RealtimeEngine must receive 0 chunks when PTT is cancelled"
        );
    })
    .await
    .expect("test_realtime_ptt_cancel_discards_audio timed out");
}
