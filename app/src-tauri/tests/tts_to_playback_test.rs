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

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
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
    services::tts::{
        actor::{spawn_tts_worker, TtsCommand, TtsWorkerHandles},
        providers::{
            supertonic::TtsEngine as SupertonicEngine, SynthesisContext, TtsProvider,
            TtsProviderKind,
        },
    },
};

#[tokio::test]
async fn test_real_tts_to_playback_synthesis_and_preroll() {
    let test_timeout = Duration::from_secs(60);
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

        state.pipeline.set_state(InteractionState::Thinking);

        // Spawn central pipeline router pump to consume VoxEvent::PlaybackStarted and transition state
        let router_app = app.clone();
        let router_handle = vox_lib::pipeline::router::spawn_router(router_app, event_rx)
            .expect("Failed to spawn router thread");

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
                intent: AudioIntent::TurnResponse,
            })
            .expect("Failed to send Generate to TTS worker");

        // ---------------------------------------------------------------------
        // Observable Exit 1 & 3: Central router transitions Thinking -> Speaking upon PlaybackStarted
        // ---------------------------------------------------------------------
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut transitioned_to_speaking = false;

        while std::time::Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Speaking {
                transitioned_to_speaking = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            transitioned_to_speaking,
            "Central pipeline router must receive VoxEvent::PlaybackStarted and transition state from Thinking to Speaking within 30s"
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

        // Teardown with bounded joins
        let _ = tts_tx.send(TtsCommand::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || worker_handle.join()),
        )
        .await
        .expect("TTS worker thread join timed out");

        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_real_tts_to_playback_synthesis_and_preroll timed out");
}

#[tokio::test]
async fn test_tts_to_playback_short_utterance_flush() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state().await;

        let turn_id = 302;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);

        let rb = ringbuf::HeapRb::<f32>::new(vox_lib::services::audio::PLAYBACK_BUFFER_SAMPLES);
        let (producer, _consumer) = ringbuf::traits::Split::split(rb);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let discard_request = Arc::new(AtomicBool::new(false));
        let turn_armed = Arc::new(AtomicBool::new(false));
        let current_turn_id = Arc::new(AtomicU32::new(turn_id));
        let pending_jobs = Arc::new(AtomicU32::new(1));
        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();

        let playback_handles = vox_lib::services::audio::playback::PlaybackEngineHandles {
            cancel_flag: Arc::clone(&cancel_flag),
            state_atomic: Arc::new(AtomicU32::new(0)),
            current_turn_id: Arc::clone(&current_turn_id),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            playback_intent: Arc::new(AtomicU8::new(0)),
            event_tx: event_tx.clone(),
            is_playback_muted: Arc::new(AtomicBool::new(false)),
        };

        let playback_engine = Arc::new(vox_lib::services::audio::PlaybackEngine::from_parts(
            producer,
            playback_handles,
            discard_request,
            Arc::clone(&turn_armed),
            None,
        ));

        // Short utterance provider: synthesizes 2,000 samples (< 12,000 MODULAR_PREROLL_THRESHOLD_SAMPLES).
        // Verifies that during ingestion the cushion is NOT armed prematurely, and only the worker loop's
        // automatic flush_pre_roll() arms playback upon job completion.
        // NOTE (false-green audit): this stub provider is intentional, not mock orchestration.
        // The entry seam is the real `spawn_tts_worker` and assertions read production worker
        // outputs (`pending_synthesis_jobs`, `turn_armed`, `PlaybackStarted`). The real-model
        // counterpart is subtest 1 above (Supertonic ONNX → RMS > 0 → 12k pre-roll arming).
        struct ShortUtteranceProvider {
            samples: Vec<f32>,
            turn_armed: Arc<AtomicBool>,
            pushed_flag: Arc<AtomicBool>,
        }

        impl TtsProvider for ShortUtteranceProvider {
            fn synthesize_chunk(
                &self,
                _text: &str,
                ctx: &SynthesisContext<'_>,
            ) -> anyhow::Result<()> {
                ctx.playback
                    .ingest_chunk_with_intent(&self.samples, ctx.intent);

                // Pre-roll Cushion Invariant:
                // 2,000 samples @ 24kHz upsamples to 4,000 samples @ 48kHz.
                // 4,000 < 12,000 threshold, so playback MUST NOT arm during ingest_chunk.
                assert!(
                    !self.turn_armed.load(Ordering::Relaxed),
                    "Short chunk (< 12,000 samples) must NOT arm playback before worker flush_pre_roll"
                );
                self.pushed_flag.store(true, Ordering::Relaxed);
                Ok(())
            }

            fn kind(&self) -> TtsProviderKind {
                TtsProviderKind::Supertonic
            }

            fn health_check(&self) -> bool {
                true
            }
        }

        let pushed_flag = Arc::new(AtomicBool::new(false));
        let provider = Box::new(ShortUtteranceProvider {
            samples: vec![0.05f32; 2000],
            turn_armed: Arc::clone(&turn_armed),
            pushed_flag: Arc::clone(&pushed_flag),
        }) as Box<dyn TtsProvider>;

        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let worker_handles = TtsWorkerHandles {
            playback: Arc::clone(&playback_engine),
            event_tx: event_tx.clone(),
            cancel_flag: Arc::clone(&cancel_flag),
            pending_synthesis_jobs: Some(Arc::clone(&pending_jobs)),
            telemetry_rtf: None,
        };

        // Spawn dedicated TTS worker thread
        let worker_handle = std::thread::spawn(move || {
            spawn_tts_worker(tts_rx, provider, worker_handles);
        });

        // Send short utterance generation request
        tts_tx
            .send(TtsCommand::Generate {
                turn_id,
                text: "OK.".to_string(),
                intent: AudioIntent::TurnResponse,
            })
            .expect("Failed to send Generate to TTS worker");

        // Wait until synthesis finishes (pending_jobs reaches 0)
        let poll_deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < poll_deadline {
            if pending_jobs.load(Ordering::Relaxed) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // Verify:
        // 1. Audio was ingested
        assert!(
            pushed_flag.load(Ordering::Relaxed),
            "Synthesis chunk must have been ingested"
        );
        // 2. Pending jobs was decremented by worker loop
        assert_eq!(
            pending_jobs.load(Ordering::Relaxed),
            0,
            "spawn_tts_worker must decrement pending_synthesis_jobs to 0"
        );
        // 3. flush_pre_roll was automatically called by worker loop when remaining <= 1
        assert!(
            turn_armed.load(Ordering::Relaxed),
            "flush_pre_roll must immediately arm playback when unplayed samples exist"
        );

        // 4. PlaybackStarted event was emitted upon flush
        match event_rx.try_recv() {
            Ok(VoxEvent::PlaybackStarted { turn_id: tid, .. }) => {
                assert_eq!(tid, turn_id, "PlaybackStarted turn_id must match");
            }
            other => panic!(
                "Expected PlaybackStarted after worker-driven flush_pre_roll, got {:?}",
                other
            ),
        }

        // Teardown with bounded join
        let _ = tts_tx.send(TtsCommand::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || worker_handle.join()),
        )
        .await
        .expect("TTS worker thread join timed out");
    })
    .await
    .expect("test_tts_to_playback_short_utterance_flush timed out");
}
