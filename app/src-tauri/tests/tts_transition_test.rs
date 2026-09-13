//! ============================================================================
//! tts_transition_test.rs — TTS Voice Switch & Context Compaction Filler Dispatch
//! ============================================================================
//! Category     : Integration Test (Seam 8)
//! Component    : services/tts/actor.rs + services/tts/providers/supertonic.rs + services/harness/facade.rs + services/audio/playback.rs
//! Prerequisites: Local Supertonic ONNX model in ~/.vox/models/tts/supertonic-3/
//! Execution    : cargo nextest run --test tts_transition_test --release --nocapture --test-threads=1
//! Metrics      : Voice hot-swap without thread restart, critical threshold compaction filler dispatch, pending job lifecycle
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use ringbuf::traits::Consumer;
use vox_lib::{
    core::{
        events::{AudioIntent, VoxEvent},
        state::InteractionState,
    },
    pipeline::{
        assistant::{
            playback::{on_playback_finished, on_playback_started},
            transcript::on_transcript_final,
        },
        RoutingContext,
    },
    services::{
        harness::{HarnessSession, TurnPreparation, TRANSITION_MESSAGES_EN},
        tts::{
            actor::{spawn_tts_worker, TtsCommand, TtsWorkerHandles},
            providers::{
                supertonic::TtsEngine as SupertonicEngine, SynthesisContext, TtsProvider,
                TtsProviderKind,
            },
        },
    },
};

/// Path A: Verifies TtsCommand::SetVoice hot-swaps active speaker voice without thread restart,
/// preserves pending job accounting, and synthesizes subsequent clauses cleanly.
#[tokio::test]
async fn test_tts_voice_switch_without_worker_restart() {
    let test_timeout = Duration::from_secs(45);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;

        let supertonic_model_dir = common::paths::get_supertonic_model_dir();
        assert!(
            supertonic_model_dir.exists(),
            "Supertonic model directory must exist at {:?}",
            supertonic_model_dir
        );

        // 1. Create real Supertonic ONNX engine starting on voice 0, wrapped in VoiceTrackingProvider
        struct VoiceTrackingProvider {
            inner: Box<dyn TtsProvider>,
            active_voice: Arc<AtomicI32>,
        }

        impl TtsProvider for VoiceTrackingProvider {
            fn synthesize_chunk(
                &self,
                text: &str,
                ctx: &SynthesisContext<'_>,
            ) -> anyhow::Result<()> {
                self.inner.synthesize_chunk(text, ctx)
            }

            fn set_voice(&self, voice: i32) {
                self.active_voice.store(voice, Ordering::Relaxed);
                self.inner.set_voice(voice);
            }

            fn set_quality_steps(&self, steps: u32) {
                self.inner.set_quality_steps(steps);
            }

            fn set_speed(&self, speed: f32) {
                self.inner.set_speed(speed);
            }

            fn kind(&self) -> TtsProviderKind {
                self.inner.kind()
            }

            fn health_check(&self) -> bool {
                self.inner.health_check()
            }
        }

        let active_voice = Arc::new(AtomicI32::new(0));
        let inner_engine = Box::new(
            SupertonicEngine::new(&supertonic_model_dir, 0, 2, 1.0, 4)
                .expect("Failed to initialize Supertonic ONNX engine"),
        ) as Box<dyn TtsProvider>;

        let provider = Box::new(VoiceTrackingProvider {
            inner: inner_engine,
            active_voice: Arc::clone(&active_voice),
        }) as Box<dyn TtsProvider>;

        // 2. Setup mock playback engine (SPSC ring buffer) and pipeline event capture
        let turn_id = 401;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
        pending_jobs.store(0, Ordering::Relaxed);

        let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
        let (playback_engine, consumer_arc) =
            common::harness::create_mock_playback_engine_with_handles(
                event_tx.clone(),
                Arc::new(AtomicU32::new(turn_id)),
                Arc::clone(&pending_jobs),
            );

        // 3. Configure worker handles and attach engine to AppState
        let common::harness::PipelineTestChannels {
            stt_tx,
            vad_tx,
            tts_tx,
            tts_rx,
            llm_tx,
            ..
        } = common::harness::setup_pipeline_channels();

        common::harness::attach_mock_engine_with_llm_tts_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx),
            Some(tts_tx.clone()),
        );

        let worker_handles = TtsWorkerHandles {
            playback: Arc::clone(&playback_engine),
            event_tx: event_tx.clone(),
            cancel_flag: Arc::clone(&cancel_flag),
            pending_synthesis_jobs: Some(Arc::clone(&pending_jobs)),
            telemetry_rtf: None,
        };

        // 4. Spawn persistent TTS worker thread
        let worker_handle = std::thread::spawn(move || {
            spawn_tts_worker(tts_rx, provider, worker_handles);
        });

        // ---------------------------------------------------------------------
        // Entry Seam: Send Clause A (Voice 0), SetVoice(2) via Settings IPC, Send Clause B (Voice 2)
        // ---------------------------------------------------------------------
        pending_jobs.fetch_add(1, Ordering::Relaxed);
        tts_tx
            .send(TtsCommand::Generate {
                turn_id,
                text: "First sentence in voice zero.".to_string(),
                intent: AudioIntent::TurnResponse,
            })
            .expect("Failed to send first Generate command");

        // Send hot-swap command through real Settings IPC entry seam
        vox_lib::ipc::settings::core::update_setting(
            "tts".to_string(),
            "voice_index".to_string(),
            serde_json::json!(2),
            app.clone(),
        )
        .await
        .expect("Failed to update voice setting via IPC");

        pending_jobs.fetch_add(1, Ordering::Relaxed);
        tts_tx
            .send(TtsCommand::Generate {
                turn_id,
                text: "Second sentence in voice two.".to_string(),
                intent: AudioIntent::TurnResponse,
            })
            .expect("Failed to send second Generate command");

        // ---------------------------------------------------------------------
        // Observable Exit 1: Both clauses synthesize, pending drops back to 0
        // ---------------------------------------------------------------------
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut completed = false;
        while std::time::Instant::now() < deadline {
            if pending_jobs.load(Ordering::Relaxed) == 0 {
                completed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            completed,
            "Both TTS synthesis jobs must complete and decrement pending_synthesis_jobs to 0"
        );

        // Verify active voice was updated on the worker provider (kills Mutant 8.1!)
        assert_eq!(
            active_voice.load(Ordering::Relaxed),
            2,
            "TTS provider active voice must be updated to 2 via IPC mutation"
        );

        // ---------------------------------------------------------------------
        // Observable Exit 2: Audio from both clauses ingested into playback buffer
        // ---------------------------------------------------------------------
        let occupied = playback_engine.buffer_len();
        assert!(
            occupied > 0,
            "Playback buffer must contain audio from synthesized clauses (got {})",
            occupied
        );

        // Drain samples and verify non-zero RMS energy
        let samples = {
            let mut consumer = consumer_arc.lock();
            let mut drained = Vec::new();
            while let Some(sample) = consumer.try_pop() {
                drained.push(sample);
            }
            drained
        };

        assert!(
            !samples.is_empty(),
            "Consumer must have drained audio samples"
        );

        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();
        assert!(
            rms > 0.001,
            "Synthesized audio must have non-trivial energy (RMS: {})",
            rms
        );

        // Teardown worker cleanly with bounded join
        let _ = tts_tx.send(TtsCommand::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || worker_handle.join()),
        )
        .await
        .expect("TTS worker thread join timed out");
    })
    .await
    .expect("test_tts_voice_switch_without_worker_restart timed out");
}

/// Path B: Verifies that exceeding critical context threshold triggers transition speech filler
/// dispatch to TTS before compaction, properly incrementing pending synthesis jobs.
#[tokio::test]
async fn test_compaction_filler_dispatch_and_pending_accounting() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state().await;

        let common::harness::PipelineTestChannels {
            stt_tx,
            vad_tx,
            tts_tx,
            tts_rx,
            llm_tx,
            ..
        } = common::harness::setup_pipeline_channels();

        let turn_id = 402;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);

        common::harness::attach_mock_engine_with_llm_tts_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx.clone()),
            Some(tts_tx.clone()),
        );

        state.pipeline.set_state(InteractionState::Thinking);
        state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);

        // Calibrate context window to 4200 (>= EMBEDDED_MODEL_MIN_CONTEXT_WINDOW = 4096)
        {
            let mut settings = state.settings.write().unwrap();
            settings.llm.context_window = 4200;
        }

        let settings = state.settings.read().unwrap().clone();
        let mut harness = HarnessSession::new_modular(
            Some(402),
            "You are a helpful voice assistant.".to_string(),
            None,
            &settings,
            llm_tx.clone(),
        );

        // 1. Seed conversation buffer to exceed critical threshold (>85% of usable 3688 = >3135 tokens)
        for i in 0..50 {
            harness.history_mut().push_user_turn(format!(
                "Turn {} user statement with sufficient length and detail to accumulate tokens in accountant memory buffer. We are discussing neural networks, integration testing, and long context tracking across conversational agents.",
                i
            ));
            harness.history_mut().push_assistant_turn(format!(
                "Turn {} assistant response describing system operations, memory compaction protocols, and pipeline states in detail. High token utilization will trigger inline compaction and transition filler phrase dispatch.",
                i
            ));
        }

        *state.harness.lock() = Some(harness);

        // 2. Real production entry seam: finalized STT transcript triggers cognitive stage
        let ctx = RoutingContext::from_app_state(&state);
        on_transcript_final(
            turn_id,
            "How does memory threshold compaction work?".to_string(),
            &app,
            &state,
            &ctx,
        );

        // ---------------------------------------------------------------------
        // Observable Exit 1: Real router dispatch sends InterimFiller to tts_rx
        // ---------------------------------------------------------------------
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut filler_received = None;
        while std::time::Instant::now() < deadline {
            if let Ok(cmd) = tts_rx.try_recv() {
                filler_received = Some(cmd);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let cmd = filler_received.expect("tts_rx must receive filler TtsCommand::Generate from real router dispatch");
        match cmd {
            TtsCommand::Generate {
                turn_id: tid,
                text,
                intent,
            } => {
                assert_eq!(tid, turn_id, "Filler turn_id must match current turn");
                assert_eq!(
                    intent,
                    AudioIntent::InterimFiller,
                    "Filler command must have InterimFiller intent"
                );
                assert!(
                    TRANSITION_MESSAGES_EN.contains(&text.as_str()),
                    "Filler text '{}' must belong to TRANSITION_MESSAGES_EN",
                    text
                );
            }
            other => panic!("Expected TtsCommand::Generate for filler, got {:?}", other),
        }

        // ---------------------------------------------------------------------
        // Observable Exit 2: State enters Working and pending jobs incremented
        // ---------------------------------------------------------------------
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Working,
            "State must transition to Working when critical compaction filler is dispatched"
        );
        assert!(
            state.pipeline.pending_synthesis_jobs.load(Ordering::Relaxed) >= 1,
            "pending_synthesis_jobs must be incremented when filler is dispatched"
        );

        // ---------------------------------------------------------------------
        // Observable Exit 3: AudioIntent::InterimFiller Playback Gating Contracts
        // ---------------------------------------------------------------------
        // Interim filler playback onset must NOT transition to Speaking (must remain Working)
        on_playback_started(turn_id, AudioIntent::InterimFiller, &app, &state, &ctx);
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Working,
            "Interim filler playback started must keep pipeline locked in Working state"
        );

        // Interim filler playback finished must NOT transition to Ready (must remain Working)
        on_playback_finished(turn_id, AudioIntent::InterimFiller, &app, &state, &ctx);
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Working,
            "Interim filler playback finished must keep pipeline locked in Working state"
        );

        // Contrast with TurnResponse onset: transitions to Speaking
        on_playback_started(turn_id, AudioIntent::TurnResponse, &app, &state, &ctx);
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Speaking,
            "TurnResponse playback onset must transition state to Speaking"
        );

        // Cancel turn token to cleanly abort background compaction task
        state.pipeline.turn_token().cancel();

        // ---------------------------------------------------------------------
        // Observable Exit 4: Under normal context (<85% utilization), zero filler dispatched
        // ---------------------------------------------------------------------
        let mut normal_harness = HarnessSession::new_modular(
            Some(403),
            "System prompt for new session".to_string(),
            None,
            &settings,
            llm_tx,
        );
        normal_harness
            .history_mut()
            .push_user_turn("Short prompt".to_string());

        let normal_prep = normal_harness.prepare_turn("Another short query", turn_id + 1);
        match normal_prep {
            TurnPreparation::Ready(_) => {}
            other => panic!("Normal context must yield Ready, got {:?}", other),
        }

        assert!(
            tts_rx.try_recv().is_err(),
            "tts_rx must not receive any command on non-critical turn"
        );
    })
    .await
    .expect("test_compaction_filler_dispatch_and_pending_accounting timed out");
}
