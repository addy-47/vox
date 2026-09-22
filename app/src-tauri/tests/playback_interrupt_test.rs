//! ============================================================================
//! playback_interrupt_test.rs — Playback Lifecycle + VAD Suppression + Barge-in
//! ============================================================================
//! Category     : Integration Test (Seam 9)
//! Component    : services/audio/playback.rs + services/audio/sink.rs +
//!                pipeline/assistant/playback.rs + pipeline/assistant/interrupt.rs +
//!                pipeline/router.rs + services/vad/actor.rs
//! Prerequisites: Local Earshot VAD + test assets in tests/assets/
//! Execution    : cargo nextest run --test playback_interrupt_test --release --nocapture --test-threads=1
//! Metrics      : Pre-roll cushion gating, real sink drain callback PlaybackFinished
//!                emission, pending job deferral, VAD speaker ducking suppression,
//!                barge-in 6-step atomic lifecycle
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use ringbuf::traits::Observer;
use vox_lib::{
    core::{
        events::{AudioIntent, VoxEvent},
        settings::{AudioOutputMode, InteractionMode},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::router::spawn_router,
    services::vad::{actor::VadActorConfig, VadCommand},
};

/// Subtest 1: Ingest >= 12,000 samples while in Thinking -> triggers PlaybackStarted -> Speaking.
/// Drain buffer with pending_jobs == 0 through real sink callback -> triggers PlaybackFinished -> Ready.
#[tokio::test]
async fn test_playback_gates_thinking_to_speaking_and_speaking_to_ready() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;

        let turn_id = 901;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Thinking);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let current_turn_id = Arc::new(AtomicU32::new(turn_id));
        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
        pending_jobs.store(0, Ordering::Relaxed);

        let (playback_engine, mut sink_ctx) =
            common::harness::create_headless_playback_with_sink(
                event_tx.clone(),
                Arc::clone(&state.pipeline.current_state_atomic),
                current_turn_id,
                Arc::clone(&pending_jobs),
            );

        // Spawn central production router pump
        let router_handle = spawn_router(app.clone(), event_rx)
            .expect("Failed to spawn router thread");

        // 1. Ingest 6,000 samples (24kHz upsamples 2x to 12,000 48kHz samples in playback buffer)
        // MODULAR_PREROLL_THRESHOLD_SAMPLES is 12,000.
        let chunk_24k = vec![0.1f32; 6000];
        playback_engine.ingest_chunk(&chunk_24k);

        // Observable Exit 1: PlaybackStarted was emitted autonomously by PlaybackEngine,
        // and central router processed it to transition Thinking -> Speaking.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Speaking {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Speaking,
            "Central router must transition Thinking -> Speaking upon PlaybackStarted"
        );

        // 2. Audio Output Drain via Real Sink Callback
        // CPAL repeatedly calls process_output_buffer until consumer is drained.
        let mut out = [0.0f32; 1024];
        let mut total_drained = 0;
        while !sink_ctx.consumer.is_empty() {
            let before = sink_ctx.consumer.occupied_len();
            sink_ctx.process_output_buffer(&mut out);
            let after = sink_ctx.consumer.occupied_len();
            total_drained += before.saturating_sub(after);
        }
        assert!(
            total_drained >= 12000,
            "Real sink callback must drain at least 12,000 samples (got {})",
            total_drained
        );
        assert!(sink_ctx.consumer.is_empty(), "Consumer must be drained completely");

        // Autonomous sink completion:
        // Calling process_output_buffer with empty consumer and pending_jobs == 0
        // autonomously sets turn_armed = false and emits PlaybackFinished to event_tx!
        sink_ctx.process_output_buffer(&mut out);

        // Observable Exit 2: Central router receives PlaybackFinished from sink callback
        // and transitions Speaking -> Ready.
        let finish_deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < finish_deadline {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "Central router must transition Speaking -> Ready upon autonomous PlaybackFinished from sink callback"
        );

        // Teardown with bounded joins
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
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
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state().await;

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
            VoxEvent::PlaybackStarted { turn_id: tid, .. } => {
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
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;

        let turn_id = 903;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Speaking);

        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
        pending_jobs.store(1, Ordering::Relaxed);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let current_turn_id = Arc::new(AtomicU32::new(turn_id));

        let (_playback_engine, mut sink_ctx) = common::harness::create_headless_playback_with_sink(
            event_tx.clone(),
            Arc::clone(&state.pipeline.current_state_atomic),
            current_turn_id,
            Arc::clone(&pending_jobs),
        );

        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        // 1. Real sink callback deferral:
        // When buffer is empty and pending_jobs > 0, sink records underrun and suppresses PlaybackFinished.
        let mut out = [0.0f32; 1024];
        sink_ctx.process_output_buffer(&mut out);
        assert_eq!(
            sink_ctx.playback_underruns.load(Ordering::Relaxed),
            1,
            "Sink callback must record underrun when buffer is empty but jobs are pending"
        );
        // Suppression invariant: Speaking must hold for the whole watch window
        // (a single sleep→sample would miss a transient premature transition).
        common::harness::assert_pipeline_state_stable(
            || state.pipeline.state(),
            InteractionState::Speaking,
            Duration::from_millis(300),
            "Sink callback must not emit PlaybackFinished while jobs are pending",
        )
        .await;

        // 2. Router handler deferral (Mutant 9.1 verification):
        // Even if a PlaybackFinished event arrives at router while pending_jobs == 1,
        // router must defer and remain in Speaking.
        event_tx
            .send(VoxEvent::PlaybackFinished {
                turn_id,
                intent: AudioIntent::TurnResponse,
            })
            .expect("Failed to send PlaybackFinished");

        // Router must defer while pending_jobs == 1: watch for stability, not a single sample.
        common::harness::assert_pipeline_state_stable(
            || state.pipeline.state(),
            InteractionState::Speaking,
            Duration::from_millis(300),
            "on_playback_finished must be deferred by router while pending_synthesis_jobs > 0",
        )
        .await;

        // 3. Decrement pending_jobs to 0 and send PlaybackFinished again
        pending_jobs.store(0, Ordering::Relaxed);
        event_tx
            .send(VoxEvent::PlaybackFinished {
                turn_id,
                intent: AudioIntent::TurnResponse,
            })
            .expect("Failed to send PlaybackFinished");

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "on_playback_finished must transition to Ready once pending_synthesis_jobs == 0"
        );

        // Teardown with bounded joins
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_playback_finished_deferred_while_pending timed out");
}

/// Subtest 4: Sacred VAD ducking check.
/// When audio mode is Speaker and state is Speaking, streaming real speech audio
/// through the VAD actor must be completely suppressed (no SpeechStart emitted).
#[test]
fn test_vad_ducking_suppresses_during_speaker_playback() {
    let _guard = common::paths::TempPathsGuard::new();
    vox_lib::utils::paths::init();
    let (_app, state) = common::harness::get_test_app_and_state_sync();

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
    common::audio::stream_test_clip(common::ASSET_SUPERTONIC_01_EN_FILENAME, &mut producer);

    // Assert that NO SpeechStart event was emitted during playback (suppressed)
    common::harness::assert_channel_empty_after(
        &vox_event_rx,
        Duration::from_millis(500),
        "VAD ducking suppression during Speaker Speaking",
    );

    // Teardown VAD actor with bounded thread join
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = vad_join.join();
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(5))
        .expect("VAD actor thread join timed out");
}

/// Subtest 5: VAD suppression resumes after playback (Speaker in Ready),
/// and Headset mode never suppresses speech even while Speaking.
#[test]
fn test_vad_ducking_resumes_after_playback_and_headset_never_suppresses() {
    let _guard = common::paths::TempPathsGuard::new();
    vox_lib::utils::paths::init();
    let (_app, state) = common::harness::get_test_app_and_state_sync();

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

    // Case 1: Speaker mode, but state transitions to Ready (playback finished)
    state.pipeline.set_state(InteractionState::Ready);
    common::audio::stream_test_clip(common::ASSET_SUPERTONIC_01_EN_FILENAME, &mut producer);

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
    // Scheduling allowance for the VAD actor loop to dequeue UpdateAudioMode.
    // Correctness is proven by the deadline polls below, not by this sleep.
    std::thread::sleep(Duration::from_millis(50));

    common::audio::stream_test_clip(common::ASSET_SUPERTONIC_01_EN_FILENAME, &mut producer);

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

    // Teardown with bounded thread join
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = vad_join.join();
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(5))
        .expect("VAD actor thread join timed out");
}

/// Subtest 6: Barge-in interrupt lifecycle.
/// Seed Speaking state during Turn 1 with pending synthesis and unplayed audio.
/// PttStart arriving at the router cancels the turn, clears accumulator,
/// cancels playback, rotates to Turn 2, and transitions state to Listening.
#[tokio::test]
async fn test_barge_in_cancels_and_advances_turn() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

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
        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let (playback_engine, _consumer_arc) =
            common::harness::create_mock_playback_engine_with_handles(
                event_tx.clone(),
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

        // Set state to Speaking (attach_mock_engine sets state to Ready)
        state.pipeline.set_state(InteractionState::Speaking);

        // Configure Assistant interaction mode to PTT
        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::PTT;
        }

        // Spawn central production router
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        // 2. Real Production Entry Seam: PttStart arrives while in Speaking
        event_tx
            .send(VoxEvent::PttStart)
            .expect("Failed to send PttStart");

        // 3. Wait for router to process interrupt and transition to Listening
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Listening {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // 4. Verify Barge-In Invariants:
        let new_turn_id = state.pipeline.peek_turn_id();
        assert!(
            new_turn_id > turn_1_id,
            "Interrupt must generate new turn_id > old_turn_id (got {} vs {})",
            new_turn_id,
            turn_1_id
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

        // - Playback engine must be cancelled by on_interrupt
        assert!(
            playback_engine.is_cancelled(),
            "Playback engine must be cancelled on interrupt"
        );

        // - Pipeline state transitions to Listening
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Listening,
            "Interrupt must transition state to Listening for subsequent user speech"
        );

        // Teardown with bounded joins
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_barge_in_cancels_and_advances_turn timed out");
}

// ============================================================================
// Subtest 4: test_invariant_14_synthesis_guard_latch_lifecycle
// ============================================================================
/// Verifies Invariant 14 Synthesis Guard Latch (`turn_open` and `drained_while_open`):
/// 1. Router sets `turn_open = true` at turn onset.
/// 2. Audio finishes playing (`on_playback_finished`) while LLM stream is still actively generating.
/// 3. Latch catches premature exit: sets `drained_while_open = true` and holds state in `Speaking` (does NOT transition to `Ready`).
/// 4. LLM finishes (`on_llm_finished`): clears `turn_open`, detects `drained_while_open == true` and `pending == 0`, and cleanly transitions to `Ready`.
#[tokio::test]
async fn test_invariant_14_synthesis_guard_latch_lifecycle() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;

        let turn_id = 905;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        let routing_ctx = vox_lib::pipeline::RoutingContext::from_app_state(&state);

        // Turn begins: transcript sets turn_open = true
        state.pipeline.set_state(InteractionState::Thinking);
        state.pipeline.set_turn_open(true);
        state.pipeline.clear_drained_while_open();
        assert!(state.pipeline.is_turn_open());

        // Audio starts playing first clause -> transitions to Speaking
        vox_lib::pipeline::assistant::playback::on_playback_started(
            turn_id,
            AudioIntent::TurnResponse,
            &app,
            &state,
            &routing_ctx,
        );
        assert_eq!(state.pipeline.state(), InteractionState::Speaking);
        assert!(!state.pipeline.is_drained_while_open());

        // Premature audio drain: Playback finishes while turn_open is still true (LLM still generating)
        state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);
        vox_lib::pipeline::assistant::playback::on_playback_finished(
            turn_id,
            AudioIntent::TurnResponse,
            &app,
            &state,
            &routing_ctx,
        );

        // LATCH INVARIANT: Pipeline must NOT transition to Ready! It must remain Speaking and set drained_while_open = true
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Speaking,
            "Premature audio drain while turn_open=true must NOT transition to Ready"
        );
        assert!(
            state.pipeline.is_drained_while_open(),
            "drained_while_open latch must be armed"
        );

        // LLM finishes later: on_llm_finished evaluates the latch and completes transition to Ready
        vox_lib::pipeline::assistant::llm::on_llm_finished(
            turn_id,
            Some(&app),
            &state,
            &routing_ctx,
        );

        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "on_llm_finished must evaluate the armed latch and transition to Ready"
        );
        assert!(
            !state.pipeline.is_turn_open(),
            "turn_open must be cleared on turn completion"
        );
        assert!(
            !state.pipeline.is_drained_while_open(),
            "drained_while_open must be cleared after evaluation"
        );
    })
    .await
    .expect("test_invariant_14_synthesis_guard_latch_lifecycle timed out");
}

