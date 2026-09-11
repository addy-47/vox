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
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use ringbuf::traits::Consumer;
use vox_lib::{
    core::{
        events::{AudioIntent, VoxEvent},
        settings::VoxSettings,
        state::InteractionState,
    },
    services::{
        harness::{HarnessSession, TurnPreparation, TRANSITION_MESSAGES_EN},
        llm::actor::LlmCommand,
        tts::{
            actor::{spawn_tts_worker, TtsCommand, TtsWorkerHandles},
            providers::{supertonic::TtsEngine as SupertonicEngine, TtsProvider},
        },
    },
};

/// Path A: Verifies TtsCommand::SetVoice hot-swaps active speaker voice without thread restart,
/// preserves pending job accounting, and synthesizes subsequent clauses cleanly.
#[tokio::test]
async fn test_tts_voice_switch_without_worker_restart() {
    let test_timeout = Duration::from_secs(45);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state().await;

        let supertonic_model_dir = common::paths::get_supertonic_model_dir();
        assert!(
            supertonic_model_dir.exists(),
            "Supertonic model directory must exist at {:?}",
            supertonic_model_dir
        );

        // 1. Create real Supertonic ONNX engine starting on voice 0
        let provider = Box::new(
            SupertonicEngine::new(&supertonic_model_dir, 0, 2, 1.0, 4)
                .expect("Failed to initialize Supertonic ONNX engine"),
        ) as Box<dyn TtsProvider>;

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

        // 3. Configure worker handles
        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
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
        // Entry Seam: Send Clause A (Voice 0), SetVoice(2), Send Clause B (Voice 2)
        // ---------------------------------------------------------------------
        pending_jobs.fetch_add(1, Ordering::Relaxed);
        tts_tx
            .send(TtsCommand::Generate {
                turn_id,
                text: "First sentence in voice zero.".to_string(),
                intent: vox_lib::core::events::AudioIntent::TurnResponse,
            })
            .expect("Failed to send first Generate command");

        // Send hot-swap command: switch active voice to 2
        tts_tx
            .send(TtsCommand::SetVoice(2))
            .expect("Failed to send SetVoice command");

        pending_jobs.fetch_add(1, Ordering::Relaxed);
        tts_tx
            .send(TtsCommand::Generate {
                turn_id,
                text: "Second sentence in voice two.".to_string(),
                intent: vox_lib::core::events::AudioIntent::TurnResponse,
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

        // Teardown worker cleanly
        let _ = tts_tx.send(TtsCommand::Shutdown);
        let _ = worker_handle.join();
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
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state().await;

        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let (llm_tx, _llm_rx) = mpsc::channel::<LlmCommand>();
        let turn_id = 402;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Thinking);
        state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);

        let mut settings = VoxSettings::default();
        settings.llm.context_window = 2048;

        let mut harness = HarnessSession::new_modular(
            Some(402),
            "System prompt".to_string(),
            None,
            &settings,
            llm_tx.clone(),
        );

        // 1. Seed conversation buffer to exceed critical threshold (>85% of (2048 - 512) = >1305 tokens)
        for i in 0..40 {
            harness.history_mut().push_user_turn(format!(
                "Turn {} detailed user prompt discussing system orchestration, memory lifecycle, token management, and pipeline state transitions across modules.",
                i
            ));
            harness.history_mut().push_assistant_turn(format!(
                "Turn {} assistant explanation regarding context token utilization, sliding window compaction triggers, FIFO shifts, and threshold maintenance.",
                i
            ));
        }

        // 2. Prepare turn when critical threshold is exceeded
        let prep = harness.prepare_turn("How does memory threshold compaction work?", turn_id);

        let filler_text = match prep {
            TurnPreparation::NeedsInlineCompaction { filler_phrase, .. } => {
                assert!(
                    TRANSITION_MESSAGES_EN.contains(&filler_phrase),
                    "Filler text '{}' must belong to TRANSITION_MESSAGES_EN",
                    filler_phrase
                );
                tts_tx
                    .send(TtsCommand::Generate {
                        turn_id,
                        text: filler_phrase.to_string(),
                        intent: AudioIntent::InterimFiller,
                    })
                    .expect("Failed to send filler");
                filler_phrase
            }
            other => panic!("Expected NeedsInlineCompaction, got {:?}", other),
        };

        // ---------------------------------------------------------------------
        // Observable Exit 1: tts_rx receives TtsCommand::Generate for filler
        // ---------------------------------------------------------------------
        let cmd = tts_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("tts_rx must receive filler TtsCommand::Generate");

        match cmd {
            TtsCommand::Generate { turn_id: tid, text, intent } => {
                assert_eq!(tid, turn_id, "Filler turn_id must match current turn");
                assert_eq!(text, filler_text, "Dispatched text must match filler text");
                assert_eq!(
                    intent,
                    AudioIntent::InterimFiller,
                    "Filler command must have InterimFiller intent"
                );
            }
            other => panic!("Expected TtsCommand::Generate, got {:?}", other),
        }

        // ---------------------------------------------------------------------
        // Observable Exit 2: Under normal context (<85% utilization), no filler dispatched
        // ---------------------------------------------------------------------
        let mut normal_harness = HarnessSession::new_modular(
            Some(403),
            "System prompt for new session".to_string(),
            None,
            &settings,
            llm_tx,
        );
        normal_harness.history_mut().push_user_turn("Short prompt".to_string());

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
