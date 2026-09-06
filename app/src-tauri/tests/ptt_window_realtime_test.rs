//! ============================================================================
//! ptt_window_realtime_test.rs — PTT Window Validation (Realtime) Integration Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : pipeline/assistant/ptt.rs + services/vad/actor.rs + services/realtime/actor.rs
//! Prerequisites: Earshot VAD weights in ~/.vox/models/
//! Execution    : cargo nextest run --test ptt_window_realtime_test --release --nocapture --test-threads=1
//! Metrics      : Latency, commit count, sample clamping, state transitions
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use anyhow::Result;
use parking_lot::Mutex;
use vox_lib::{
    core::{
        settings::{AudioOutputMode, InteractionMode, PipelineMode, RealtimeProviderKind},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::assistant::ptt::{ptt_cancel, ptt_start, ptt_stop},
    services::{
        realtime::{
            RealtimeActor, RealtimeAudioConfig, RealtimeProviderEvent, RealtimeSession,
            RealtimeVoiceProvider,
        },
        stt::actor::SttCommand,
        vad::{actor::VadActorConfig, VadCommand},
    },
};

/// Thread-safe counters capturing calls to MockRealtimeSession.
#[derive(Default, Clone)]
struct MockSessionCounters {
    pub send_count: Arc<AtomicUsize>,
    pub commit_count: Arc<AtomicUsize>,
    pub committed_samples: Arc<Mutex<Vec<i16>>>,
    pub cancel_count: Arc<AtomicUsize>,
}

struct MockRealtimeSession {
    counters: MockSessionCounters,
}

impl RealtimeSession for MockRealtimeSession {
    fn send_audio(&self, _pcm: &[i16]) -> Result<()> {
        self.counters.send_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn commit_speech_turn(&self, pcm: &[i16]) -> Result<()> {
        self.counters.commit_count.fetch_add(1, Ordering::SeqCst);
        self.counters
            .committed_samples
            .lock()
            .extend_from_slice(pcm);
        Ok(())
    }

    fn cancel(&self) -> Result<()> {
        self.counters.cancel_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn disconnect(&self) -> Result<()> {
        Ok(())
    }
}

struct MockRealtimeProvider {
    counters: MockSessionCounters,
}

impl RealtimeVoiceProvider for MockRealtimeProvider {
    fn kind(&self) -> RealtimeProviderKind {
        RealtimeProviderKind::DeepgramVoiceAgent
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
    ) -> Result<(
        Box<dyn RealtimeSession>,
        tokio::sync::mpsc::Receiver<RealtimeProviderEvent>,
    )> {
        let (_tx, rx) = tokio::sync::mpsc::channel(32);
        let session = Box::new(MockRealtimeSession {
            counters: self.counters.clone(),
        });
        Ok((session, rx))
    }

    fn health_check(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn test_ptt_realtime_matrix() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state();

        // 1. Configure settings for Assistant Realtime PTT
        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::PTT;
            settings.interaction.pipeline_mode = PipelineMode::Realtime;
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);

        // 2. Setup Mock RealtimeActor wired into state.realtime_engine
        let session_counters = MockSessionCounters::default();
        let provider = Box::new(MockRealtimeProvider {
            counters: session_counters.clone(),
        });

        let mut actor = RealtimeActor::new(provider, tokio::runtime::Handle::current());
        let (playback_engine, _) = common::harness::create_mock_playback_engine();
        let (event_tx, _event_rx) = mpsc::channel();
        actor
            .start(InteractionMode::PTT, playback_engine, event_tx, app.clone())
            .expect("Failed to start Mock RealtimeActor");

        *state.realtime_engine.lock().await = Some(actor);

        // 3. Setup production VAD actor in WindowedValidation (PTT) mode
        let (stt_tx, _stt_rx) = mpsc::channel::<SttCommand>();
        let vad_config = VadActorConfig {
            initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
            initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
            initial_silence_duration_ms: vox_lib::core::defaults::DEFAULT_VAD_SILENCE_DURATION_MS,
            initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
            initial_mode: InteractionMode::PTT,
            initial_audio_mode: AudioOutputMode::Headset,
        };

        let audio_suppressed = Arc::new(AtomicBool::new(false));
        let vad_shutdown = Arc::new(AtomicBool::new(false));

        let (vad_cmd_tx, _vox_event_rx, mut producer, vad_join) = common::harness::setup_vad_actor(
            stt_tx.clone(),
            vad_config,
            state.pipeline.current_state_atomic.clone(),
            state.pipeline.turn_id.clone(),
            audio_suppressed.clone(),
            state.pipeline.ingestion_gate.clone(),
            vad_shutdown.clone(),
        );

        common::harness::attach_mock_engine_with_vad_to_state(
            &app,
            &state,
            stt_tx.clone(),
            vad_cmd_tx.clone(),
        );

        // =====================================================================
        // Subtest 1: Speech transmits to Realtime actor -> commit == 1
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Listening,
                "State must be Listening after ptt_start"
            );

            let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
            let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
                .expect("Failed to decode supertonic_01_en_briefing.wav");

            common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
            common::audio::wait_for_buffer_drain(&producer, 5);

            ptt_stop(&app, &state)
                .await
                .expect("Failed to invoke ptt_stop");

            assert_eq!(
                state.pipeline.state(),
                InteractionState::Thinking,
                "State must transition to Thinking upon Realtime commit"
            );

            assert_eq!(
                session_counters.commit_count.load(Ordering::SeqCst),
                1,
                "Exactly 1 speech turn must be committed to Realtime session"
            );

            let committed = session_counters.committed_samples.lock().clone();
            assert!(
                !committed.is_empty(),
                "Committed samples buffer must not be empty"
            );

            // Verify sample conversion invariant (at least some non-zero active speech samples)
            assert!(
                committed.iter().any(|&s| s != 0),
                "Committed samples must contain non-zero audio values"
            );
        }

        // =====================================================================
        // Subtest 2: Ghost Gate — Silence hold reverts to Ready (commit count 0)
        // =====================================================================
        {
            session_counters.commit_count.store(0, Ordering::SeqCst);
            session_counters.committed_samples.lock().clear();

            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(state.pipeline.state(), InteractionState::Listening);

            common::audio::stream_silence_frames(&mut producer, 30);
            common::audio::wait_for_buffer_drain(&producer, 5);

            ptt_stop(&app, &state)
                .await
                .expect("Failed to invoke ptt_stop");

            assert_eq!(
                state.pipeline.state(),
                InteractionState::Ready,
                "Ghost gate: state must revert to Ready on silence hold"
            );

            assert_eq!(
                session_counters.commit_count.load(Ordering::SeqCst),
                0,
                "Ghost gate: silence hold must NOT commit to Realtime session"
            );
        }

        // =====================================================================
        // Subtest 3: Cancel discards hold and cancels turn token
        // =====================================================================
        {
            session_counters.commit_count.store(0, Ordering::SeqCst);

            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(state.pipeline.state(), InteractionState::Listening);

            let turn_token = state.pipeline.turn_token();
            assert!(!turn_token.is_cancelled());

            common::audio::stream_silence_frames(&mut producer, 15);
            common::audio::wait_for_buffer_drain(&producer, 5);

            ptt_cancel(&app, &state).expect("Failed to invoke ptt_cancel");

            assert_eq!(state.pipeline.state(), InteractionState::Ready);
            assert!(
                turn_token.is_cancelled(),
                "turn_token must be cancelled on ptt_cancel"
            );

            assert_eq!(
                session_counters.commit_count.load(Ordering::SeqCst),
                0,
                "Cancelled PTT must NOT commit to Realtime session"
            );
        }

        // =====================================================================
        // Teardown
        // =====================================================================
        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        vad_join.join().expect("VAD thread panicked");
    })
    .await
    .expect("Hard test timeout exceeded (60s)");
}
