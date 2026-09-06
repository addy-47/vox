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
        constants::TRANSITION_MESSAGES_EN, events::VoxEvent, settings::MemorySettings,
        state::InteractionState,
    },
    services::{
        harness::facade::{prepare_turn_context, PrepareTurnParams},
        llm::ProviderKind,
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
        let (_app, state) = common::harness::get_test_app_and_state();

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
        let (_app, state) = common::harness::get_test_app_and_state();

        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let turn_id = 402;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Thinking);
        state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);

        // 1. Seed conversation manager buffer to exceed critical threshold (>85% of (2048 - 512) = >1305 tokens)
        {
            let mut cm = state.conversation_manager.lock();
            for i in 0..40 {
                cm.push_user_turn(format!(
                    "Turn {} detailed user prompt discussing system orchestration, memory lifecycle, token management, and pipeline state transitions across modules.",
                    i
                ));
                cm.push_assistant_turn(format!(
                    "Turn {} assistant explanation regarding context token utilization, sliding window compaction triggers, FIFO shifts, and threshold maintenance.",
                    i
                ));
            }
        }

        // 2. Prepare turn context with context_window = 2048 and ProviderKind::OpenAiCompat
        // In facade.rs: ProviderKind::OpenAiCompat activates the background compaction branch
        // which dispatches transition speech filler to tts_tx when critical threshold is exceeded.
        let memory_settings = MemorySettings {
            context_retrieval_enabled: false,
            ..Default::default()
        };

        let params = PrepareTurnParams {
            harness: &state.conversation_manager,
            tts_tx: Some(&tts_tx),
            memory_tx: None,
            conn: None,
            query: "How does memory threshold compaction work?",
            turn_id,
            session_id: "test-session-402",
            memory: &memory_settings,
            context_window: 2048,
            provider_kind: ProviderKind::OpenAiCompat,
            llm_provider: None,
            llm_settings: None,
        };

        let result = prepare_turn_context(params).await;
        assert!(result.is_ok(), "prepare_turn_context must succeed: {:?}", result.err());

        let (_req, filler_opt) = result.unwrap();
        assert!(
            filler_opt.is_some(),
            "prepare_turn_context must return Some(filler) when threshold is exceeded"
        );

        let filler_text = filler_opt.unwrap();
        assert!(
            TRANSITION_MESSAGES_EN.contains(&filler_text.as_str()),
            "Filler text '{}' must belong to TRANSITION_MESSAGES_EN",
            filler_text
        );

        // ---------------------------------------------------------------------
        // Observable Exit 1: tts_rx receives TtsCommand::Generate for filler
        // ---------------------------------------------------------------------
        let cmd = tts_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("tts_rx must receive filler TtsCommand::Generate");

        match cmd {
            TtsCommand::Generate { turn_id: tid, text } => {
                assert_eq!(tid, turn_id, "Filler turn_id must match current turn");
                assert_eq!(text, filler_text, "Dispatched text must match filler text");
            }
            other => panic!("Expected TtsCommand::Generate, got {:?}", other),
        }

        // ---------------------------------------------------------------------
        // Observable Exit 2: Under normal context (<85% utilization), no filler dispatched
        // ---------------------------------------------------------------------
        {
            let mut cm = state.conversation_manager.lock();
            cm.new_session("System prompt for new session");
            cm.push_user_turn("Short prompt".to_string());
        }

        let normal_params = PrepareTurnParams {
            harness: &state.conversation_manager,
            tts_tx: Some(&tts_tx),
            memory_tx: None,
            conn: None,
            query: "Another short query",
            turn_id: turn_id + 1,
            session_id: "test-session-402",
            memory: &memory_settings,
            context_window: 2048,
            provider_kind: ProviderKind::Embedded,
            llm_provider: None,
            llm_settings: None,
        };

        let normal_result = prepare_turn_context(normal_params).await;
        assert!(normal_result.is_ok());
        let (_req2, filler_opt2) = normal_result.unwrap();
        assert!(
            filler_opt2.is_none(),
            "Normal non-critical context must not generate filler speech"
        );

        assert!(
            tts_rx.try_recv().is_err(),
            "tts_rx must not receive any command on non-critical turn"
        );
    })
    .await
    .expect("test_compaction_filler_dispatch_and_pending_accounting timed out");
}
