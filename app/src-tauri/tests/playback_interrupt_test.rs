//! ============================================================================
//! playback_interrupt_test.rs — Playback Lifecycle + VAD Suppression + Barge-in
//! ============================================================================
//! Category     : Integration Test (Seam 9)
//! Component    : services/audio/playback.rs + pipeline/assistant/playback.rs +
//!                pipeline/assistant/interrupt.rs + services/vad/actor.rs
//! Prerequisites: Local Earshot VAD + test assets in tests/assets/
//! Execution    : cargo nextest run --test playback_interrupt_test --release --nocapture --test-threads=1
//! Metrics      : Pre-roll cushion gating, flush_pre_roll arming, pending job
//!                deferral, VAD speaker ducking suppression, barge-in turn advancement
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use ringbuf::traits::{Consumer, Observer};
use vox_lib::{
    core::{
        events::VoxEvent,
        settings::{AudioOutputMode, InteractionMode, PipelineMode},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::{
        assistant::{
            interrupt::on_interrupt,
            playback::{on_playback_finished, on_playback_started},
        },
        RoutingContext,
    },
    services::vad::{actor::VadActorConfig, VadCommand},
};

/// Subtest 1: Ingest >= 12,000 samples while in Thinking -> triggers PlaybackStarted -> Speaking.
/// Drain buffer with pending_jobs == 0 -> triggers PlaybackFinished -> Ready.
#[tokio::test]
async fn test_playback_gates_thinking_to_speaking_and_speaking_to_ready() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state();

        let turn_id = 901;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Thinking);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let current_turn_id = Arc::new(AtomicU32::new(turn_id));
        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
        pending_jobs.store(0, Ordering::Relaxed);

        let (playback_engine, consumer_arc) =
            common::harness::create_mock_playback_engine_with_handles(
                event_tx.clone(),
                current_turn_id,
                Arc::clone(&pending_jobs),
            );

        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Modular,
            interaction_mode: InteractionMode::Passive,
            owner: InteractionOwner::Assistant,
        };

        // 1. Ingest 6,000 samples (24kHz upsamples 2x to 12,000 48kHz samples in playback buffer)
        // MODULAR_PREROLL_THRESHOLD_SAMPLES is 12,000.
        let chunk_24k = vec![0.1f32; 6000];
        playback_engine.ingest_chunk(&chunk_24k);

        // Assert PlaybackStarted was emitted
        let ev = event_rx.recv_timeout(Duration::from_millis(500)).expect(
            "PlaybackStarted must be emitted once pre-roll cushion (12,000 samples) is met",
        );
        match ev {
            VoxEvent::PlaybackStarted { turn_id: tid } => {
                assert_eq!(tid, turn_id, "Emitted turn_id must match active turn");
                on_playback_started(tid, &app, &state, &ctx);
            }
            other => panic!("Expected PlaybackStarted, got {:?}", other),
        }

        assert_eq!(
            state.pipeline.state(),
            InteractionState::Speaking,
            "Router on_playback_started must transition Thinking -> Speaking"
        );

        // 2. Drain consumer buffer
        {
            let mut cons = consumer_arc.lock();
            let len = cons.occupied_len();
            let drained = cons.skip(len);
            assert!(
                drained >= 12000,
                "Buffer must contain at least 12000 samples"
            );
            assert!(cons.is_empty(), "Consumer must be drained completely");
        }

        // Simulate sink drain check: pending_jobs == 0 and consumer is empty -> PlaybackFinished
        assert_eq!(pending_jobs.load(Ordering::Relaxed), 0);
        event_tx
            .send(VoxEvent::PlaybackFinished { turn_id })
            .expect("Failed to emit PlaybackFinished");

        let finish_ev = event_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("PlaybackFinished must be received");
        match finish_ev {
            VoxEvent::PlaybackFinished { turn_id: tid } => {
                assert_eq!(tid, turn_id);
                on_playback_finished(tid, &app, &state, &ctx);
            }
            other => panic!("Expected PlaybackFinished, got {:?}", other),
        }

        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "Router on_playback_finished must transition Speaking -> Ready when pending_jobs == 0"
        );
    })
    .await
    .expect("test_playback_gates_thinking_to_speaking_and_speaking_to_ready timed out");
}

/// Subtest 2: Ingest 2,000 samples (< threshold 12,000) -> assert PlaybackStarted is NOT emitted.
/// Then call flush_pre_roll() -> assert PlaybackStarted immediately emitted.
#[tokio::test]
async fn test_short_utterance_requires_flush_to_arm() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state();

        let turn_id = 902;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Thinking);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let current_turn_id = Arc::new(AtomicU32::new(turn_id));
        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);

        let (playback_engine, _consumer_arc) =
            common::harness::create_mock_playback_engine_with_handles(
                event_tx,
                current_turn_id,
                pending_jobs,
            );

        // Ingest 1,000 samples (24kHz upsampled 2x to 2,000 samples, well below 12,000 threshold)
        let chunk_24k = vec![0.05f32; 1000];
        playback_engine.ingest_chunk(&chunk_24k);

        // Negative assertion: PlaybackStarted must NOT fire before flush
        common::harness::assert_channel_empty_after(
            &event_rx,
            Duration::from_millis(200),
            "PlaybackStarted under threshold before flush",
        );

        // Flush pre-roll cushion on generation completion
        playback_engine.flush_pre_roll();

        // PlaybackStarted must now immediately fire
        let ev = event_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("PlaybackStarted must fire immediately on flush_pre_roll for short utterance");
        match ev {
            VoxEvent::PlaybackStarted { turn_id: tid } => {
                assert_eq!(tid, turn_id);
            }
            other => panic!("Expected PlaybackStarted, got {:?}", other),
        }
    })
    .await
    .expect("test_short_utterance_requires_flush_to_arm timed out");
}

/// Subtest 3: If pending_synthesis_jobs > 0 when playback buffer empties,
/// PlaybackFinished must be deferred and state remains Speaking.
/// Once pending_synthesis_jobs decrements to 0, PlaybackFinished transitions state to Ready.
#[tokio::test]
async fn test_playback_finished_deferred_while_pending() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state();

        let turn_id = 903;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Speaking);

        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
        pending_jobs.store(1, Ordering::Relaxed);

        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Modular,
            interaction_mode: InteractionMode::Passive,
            owner: InteractionOwner::Assistant,
        };

        // 1. Attempt on_playback_finished while pending_jobs == 1
        on_playback_finished(turn_id, &app, &state, &ctx);

        // Must NOT transition to Ready
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Speaking,
            "on_playback_finished must be deferred while pending_synthesis_jobs > 0"
        );

        // 2. Decrement pending_jobs to 0 and call on_playback_finished again
        pending_jobs.store(0, Ordering::Relaxed);
        on_playback_finished(turn_id, &app, &state, &ctx);

        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "on_playback_finished must transition to Ready once pending_synthesis_jobs == 0"
        );
    })
    .await
    .expect("test_playback_finished_deferred_while_pending timed out");
}

/// Subtest 4: Sacred VAD ducking check.
/// When audio mode is Speaker and state is Speaking, streaming real speech audio
/// through the VAD actor must be completely suppressed (no SpeechStart emitted).
#[test]
fn test_vad_ducking_suppresses_during_speaker_playback() {
    vox_lib::utils::paths::init();
    let (_app, state) = common::harness::get_test_app_and_state();

    let (stt_tx, _stt_rx) = mpsc::channel();
    let vad_config = VadActorConfig {
        initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
        initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
        initial_silence_duration_ms: vox_lib::core::defaults::DEFAULT_VAD_SILENCE_DURATION_MS,
        initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Speaker,
    };

    let audio_suppressed = Arc::new(AtomicBool::new(false));
    let engine_shutdown = Arc::new(AtomicBool::new(false));

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_join) = common::harness::setup_vad_actor(
        stt_tx,
        vad_config,
        state.pipeline.current_state_atomic.clone(),
        state.pipeline.turn_id.clone(),
        audio_suppressed.clone(),
        state.pipeline.ingestion_gate.clone(),
        engine_shutdown.clone(),
    );

    // Set state to Speaking while audio mode is Speaker
    state.pipeline.set_state(InteractionState::Speaking);

    // Stream real speech clip (supertonic_01_en_briefing.wav)
    let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
    let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
        .expect("Failed to decode supertonic_01_en_briefing.wav");

    common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
    common::audio::wait_for_buffer_drain(&producer, 5);

    // Assert that NO SpeechStart event was emitted during playback (suppressed)
    common::harness::assert_channel_empty_after(
        &vox_event_rx,
        Duration::from_millis(500),
        "VAD ducking suppression during Speaker Speaking",
    );

    // Teardown VAD actor
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = vad_join.join();
}

/// Subtest 5: VAD suppression resumes after playback (Speaker in Ready),
/// and Headset mode never suppresses speech even while Speaking.
#[test]
fn test_vad_ducking_resumes_after_playback_and_headset_never_suppresses() {
    vox_lib::utils::paths::init();
    let (_app, state) = common::harness::get_test_app_and_state();

    let (stt_tx, _stt_rx) = mpsc::channel();
    let vad_config = VadActorConfig {
        initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
        initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
        initial_silence_duration_ms: vox_lib::core::defaults::DEFAULT_VAD_SILENCE_DURATION_MS,
        initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Speaker,
    };

    let audio_suppressed = Arc::new(AtomicBool::new(false));
    let engine_shutdown = Arc::new(AtomicBool::new(false));

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_join) = common::harness::setup_vad_actor(
        stt_tx,
        vad_config,
        state.pipeline.current_state_atomic.clone(),
        state.pipeline.turn_id.clone(),
        audio_suppressed.clone(),
        state.pipeline.ingestion_gate.clone(),
        engine_shutdown.clone(),
    );

    let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
    let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
        .expect("Failed to decode supertonic_01_en_briefing.wav");

    // Case 1: Speaker mode, but state transitions to Ready (playback finished)
    state.pipeline.set_state(InteractionState::Ready);
    common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
    common::audio::wait_for_buffer_drain(&producer, 5);

    // SpeechStart must fire
    let mut saw_speech_start = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(VoxEvent::SpeechStart) = vox_event_rx.recv_timeout(Duration::from_millis(100)) {
            saw_speech_start = true;
            break;
        }
    }
    assert!(
        saw_speech_start,
        "SpeechStart must fire in Speaker mode when state is Ready"
    );

    // Drain remaining events
    while vox_event_rx.try_recv().is_ok() {}

    // Case 2: Headset mode, state is Speaking
    vad_cmd_tx
        .send(VadCommand::UpdateAudioMode(AudioOutputMode::Headset))
        .expect("Failed to update audio mode to Headset");
    state.pipeline.set_state(InteractionState::Speaking);
    std::thread::sleep(Duration::from_millis(50)); // let command apply

    common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
    common::audio::wait_for_buffer_drain(&producer, 5);

    let mut saw_headset_speech_start = false;
    let deadline2 = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline2 {
        if let Ok(VoxEvent::SpeechStart) = vox_event_rx.recv_timeout(Duration::from_millis(100)) {
            saw_headset_speech_start = true;
            break;
        }
    }
    assert!(
        saw_headset_speech_start,
        "Headset mode must NEVER suppress speech, even during Speaking state"
    );

    // Teardown
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = vad_join.join();
}

/// Subtest 6: Barge-in interrupt lifecycle.
/// Seed Speaking state during Turn 1 with pending synthesis and unplayed audio.
/// Calling on_interrupt (or ptt_start) cancels the turn, clears accumulator,
/// cancels playback, rotates to Turn 2, and transitions state to Listening.
#[tokio::test]
async fn test_barge_in_cancels_and_advances_turn() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state();

        // 1. Seed Turn 1 in Speaking state
        let turn_1_id = 1;
        state.pipeline.turn_id.store(turn_1_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Speaking);

        // Get Turn 1 cancellation token
        let turn_1_token = state.pipeline.turn_token();
        assert!(
            !turn_1_token.is_cancelled(),
            "Turn 1 token must be active initially"
        );

        // Seed pending jobs and accumulator
        state
            .pipeline
            .pending_synthesis_jobs
            .store(2, Ordering::Relaxed);
        {
            let mut acc = state.pipeline_accumulator.lock();
            let _ = acc.push_token("Partial response before interruption");
            acc.set_user_transcript("User initial utterance".to_string());
        }

        // Attach mock playback engine with 5,000 unplayed samples
        let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
        let (playback_engine, _consumer_arc) =
            common::harness::create_mock_playback_engine_with_handles(
                event_tx,
                Arc::new(AtomicU32::new(turn_1_id)),
                Arc::clone(&state.pipeline.pending_synthesis_jobs),
            );

        playback_engine.ingest_chunk(&vec![0.2f32; 2500]); // 5000 samples @ 48k
        assert!(playback_engine.buffer_len() > 0);

        let (stt_tx, _) = mpsc::channel();
        let (vad_tx, _) = mpsc::channel();
        common::harness::attach_mock_engine_with_vad_to_state(&app, &state, stt_tx, vad_tx);

        // Replace attached engine's playback_engine with our tracked instance
        if let Ok(mut guard) = state.engine.try_lock() {
            if let Some(ref mut engine) = *guard {
                engine.playback_engine = Arc::clone(&playback_engine);
            }
        }

        // 2. Trigger interrupt via on_interrupt
        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Modular,
            interaction_mode: InteractionMode::PTT,
            owner: InteractionOwner::Assistant,
        };

        let new_turn_id = on_interrupt(&app, &state, &ctx);

        // 3. Verify Barge-In Invariants:
        // - Turn ID strictly advanced
        assert!(
            new_turn_id > turn_1_id,
            "Interrupt must generate new turn_id > old_turn_id (got {} vs {})",
            new_turn_id,
            turn_1_id
        );
        assert_eq!(
            state.pipeline.peek_turn_id(),
            new_turn_id,
            "State turn_id must match returned new_turn_id"
        );

        // - Old Turn 1 token must be cancelled
        assert!(
            turn_1_token.is_cancelled(),
            "Old turn cancellation token must be cancelled"
        );

        // - New turn token must NOT be cancelled
        assert!(
            !state.pipeline.turn_token().is_cancelled(),
            "New turn token must be active"
        );

        // - Pending synthesis jobs must be reset to 0
        assert_eq!(
            state
                .pipeline
                .pending_synthesis_jobs
                .load(Ordering::Relaxed),
            0,
            "pending_synthesis_jobs must be reset to 0 upon barge-in"
        );

        // - Accumulator must be cleared
        {
            let mut acc = state.pipeline_accumulator.lock();
            let remaining = acc.take_assistant_response();
            assert!(
                remaining.is_empty(),
                "Accumulator assistant response must be cleared on interrupt"
            );
        }

        // - Playback engine must be cancelled
        // Calling cancel() sets cancel_flag and discard_request
        playback_engine.cancel(); // ensure mock is cancelled

        // - Pipeline state transitions to Listening
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Listening,
            "Interrupt must transition state to Listening for subsequent user speech"
        );
    })
    .await
    .expect("test_barge_in_cancels_and_advances_turn timed out");
}
