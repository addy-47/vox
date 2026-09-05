//! ============================================================================
//! tts_to_playback_test.rs — Real TTS Synthesis → Playback Ingest & Pre-Roll Gates
//! ============================================================================
//! Category     : Integration Test (Seam 7)
//! Component    : services/tts/actor.rs + services/tts/providers/supertonic.rs + services/audio/playback.rs + pipeline/assistant/playback.rs
//! Prerequisites: Local Supertonic ONNX model in ~/.vox/models/tts/supertonic-3/
//! Execution    : cargo nextest run --test tts_to_playback_test --release --nocapture --test-threads=1
//! Metrics      : Real audio synthesis (RMS > 0), sample ingestion, pre-roll threshold arming (12,000 samples), flush-pre-roll on short utterance, pending accounting
//! ============================================================================

mod common;

use ringbuf::traits::Consumer;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::state::InteractionState;
use vox_lib::pipeline::assistant::playback::on_playback_started;
use vox_lib::pipeline::RoutingContext;
use vox_lib::services::tts::actor::{spawn_tts_worker, TtsCommand, TtsWorkerHandles};
use vox_lib::services::tts::providers::supertonic::TtsEngine as SupertonicEngine;
use vox_lib::services::tts::providers::TtsProvider;

#[tokio::test]
async fn test_real_tts_to_playback_synthesis_and_preroll() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state();

        let supertonic_model_dir = common::paths::get_supertonic_model_dir();
        assert!(
            supertonic_model_dir.exists(),
            "Supertonic model directory must exist at {:?}",
            supertonic_model_dir
        );

        // 1. Create real Supertonic ONNX engine (voice 0, quality_steps 2 for test speed, speed 1.0, 4 threads)
        let provider = Box::new(
            SupertonicEngine::new(&supertonic_model_dir, 0, 2, 1.0, 4)
                .expect("Failed to initialize Supertonic ONNX engine"),
        ) as Box<dyn TtsProvider>;

        // 2. Setup mock playback engine (SPSC ring buffer) and pipeline event capture
        let turn_id = 301;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
        pending_jobs.store(1, Ordering::Relaxed);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let (playback_engine, consumer_arc) =
            common::harness::create_mock_playback_engine_with_handles(
                event_tx.clone(),
                Arc::new(AtomicU32::new(turn_id)),
                Arc::clone(&pending_jobs),
            );

        // 3. Configure state and worker handles
        let (stt_tx, _) = mpsc::channel();
        let (vad_tx, _) = mpsc::channel();
        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let (llm_tx, _) = mpsc::channel();

        common::harness::attach_mock_engine_with_llm_tts_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx),
            Some(tts_tx.clone()),
        );

        state.pipeline.set_state(InteractionState::Thinking);

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
        // Entry Seam: Send TtsCommand::Generate with realistic clause
        // ---------------------------------------------------------------------
        tts_tx
            .send(TtsCommand::Generate {
                turn_id,
                text: "Hello! Welcome to Vox voice assistant.".to_string(),
            })
            .expect("Failed to send Generate to TTS worker");

        // ---------------------------------------------------------------------
        // Observable Exit 1: Real Supertonic synthesis produces samples and ingests to PlaybackEngine
        // ---------------------------------------------------------------------
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut started_received = false;

        while std::time::Instant::now() < deadline {
            match event_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(VoxEvent::PlaybackStarted { turn_id: tid }) => {
                    assert_eq!(tid, turn_id, "PlaybackStarted turn_id must match");
                    started_received = true;
                    break;
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        assert!(
            started_received,
            "Real Supertonic synthesis must produce enough samples to trigger PlaybackStarted within 30s"
        );

        // ---------------------------------------------------------------------
        // Observable Exit 2: Playback buffer contains valid non-empty audio
        // ---------------------------------------------------------------------
        let occupied = playback_engine.buffer_len();
        assert!(
            occupied > 0,
            "Playback ring buffer must have occupied samples (got {})",
            occupied
        );
        log::info!(
            "[Test] Playback ring buffer currently has {} unplayed samples",
            occupied
        );

        // Verify audio content is non-silent (calculate RMS across samples)
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
            "Consumer must drain audio samples generated by Supertonic"
        );
        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();
        assert!(
            rms > 0.001,
            "Synthesized audio must not be pure silence (RMS: {:.5})",
            rms
        );
        log::info!("[Test] Generated audio: {} samples, RMS: {:.4}", samples.len(), rms);

        // ---------------------------------------------------------------------
        // Observable Exit 3: State transition on PlaybackStarted
        // ---------------------------------------------------------------------
        let ctx = RoutingContext {
            pipeline_mode: vox_lib::core::settings::PipelineMode::Modular,
            interaction_mode: vox_lib::core::settings::InteractionMode::PTT,
            owner: vox_lib::core::state::InteractionOwner::Assistant,
        };
        on_playback_started(turn_id, &app, &state, &ctx);
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Speaking,
            "on_playback_started must transition Thinking -> Speaking"
        );

        // ---------------------------------------------------------------------
        // Observable Exit 4: Pending jobs decrements to 0 after chunk completion
        // ---------------------------------------------------------------------
        let poll_deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < poll_deadline {
            if pending_jobs.load(Ordering::Relaxed) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            pending_jobs.load(Ordering::Relaxed),
            0,
            "pending_synthesis_jobs must decrement to 0 upon chunk synthesis completion"
        );

        // Teardown
        let _ = tts_tx.send(TtsCommand::Shutdown);
        let _ = worker_handle.join();
    })
    .await
    .expect("test_real_tts_to_playback_synthesis_and_preroll timed out");
}

#[tokio::test]
async fn test_tts_to_playback_short_utterance_flush() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state();

        // Setup mock playback engine with custom event capture
        let (_playback_engine, _consumer_arc) = common::harness::create_mock_playback_engine();
        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();

        let turn_id = 302;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);

        // Recreate playback engine with bound event_tx for this turn
        let rb = ringbuf::HeapRb::<f32>::new(vox_lib::services::audio::PLAYBACK_BUFFER_SAMPLES);
        let (producer, _consumer) = ringbuf::traits::Split::split(rb);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let discard_request = Arc::new(AtomicBool::new(false));
        let turn_armed = Arc::new(AtomicBool::new(false));
        let current_turn_id = Arc::new(AtomicU32::new(turn_id));
        let pending_jobs = Arc::new(AtomicU32::new(1));

        let playback_handles = vox_lib::services::audio::playback::PlaybackEngineHandles {
            cancel_flag: Arc::clone(&cancel_flag),
            state_atomic: Arc::new(AtomicU32::new(0)),
            current_turn_id: Arc::clone(&current_turn_id),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            event_tx,
        };

        let engine = vox_lib::services::audio::PlaybackEngine::from_parts(
            producer,
            playback_handles,
            discard_request,
            Arc::clone(&turn_armed),
            None,
        );

        // Ingest a chunk strictly LESS than MODULAR_PREROLL_THRESHOLD_SAMPLES (12,000)
        // e.g., 2,000 samples at 24kHz (upsampled to 4,000 samples in playback buffer)
        let short_chunk = vec![0.05f32; 2000];
        engine.ingest_chunk(&short_chunk);

        // Assert that PlaybackStarted is NOT armed yet
        assert!(
            !turn_armed.load(Ordering::Relaxed),
            "Playback must NOT arm before reaching 12,000 samples"
        );
        assert!(
            event_rx.try_recv().is_err(),
            "PlaybackStarted event must NOT be emitted when buffer is below threshold"
        );

        // Simulate end of generation flush (flush_pre_roll)
        engine.flush_pre_roll();

        // Assert that flush_pre_roll immediately armed playback
        assert!(
            turn_armed.load(Ordering::Relaxed),
            "flush_pre_roll must immediately arm playback when unplayed samples exist"
        );
        match event_rx.try_recv() {
            Ok(VoxEvent::PlaybackStarted { turn_id: tid }) => {
                assert_eq!(tid, turn_id);
            }
            other => panic!(
                "Expected PlaybackStarted after flush_pre_roll, got {:?}",
                other
            ),
        }
    })
    .await
    .expect("test_tts_to_playback_short_utterance_flush timed out");
}
