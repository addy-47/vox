//! ============================================================================
//! ptt_window_realtime_offline_test.rs — Realtime PTT Zero-Local-STT-Leak Contract
//! ============================================================================
//! Category     : Integration Test (Seam 3)
//! Component    : pipeline/assistant/ptt.rs + services/vad/actor.rs
//! Prerequisites: Earshot VAD weights in ~/.vox/models/ (no API key, no network)
//! Execution    : cargo nextest run --test ptt_window_realtime_offline_test --release --nocapture --test-threads=1
//! Metrics      : Ready->Listening transition, ghost-gate silence revert, zero local STT
//!                dispatch in Realtime mode, cancel turn-token propagation.
//! Model        : none beyond the local Earshot VAD. No LLM/STT model is exercised, and
//!                none is needed: the invariant under test is a *routing* invariant.
//!
//! WHY THIS FILE EXISTS
//! --------------------
//! `ptt_window_realtime_test.rs` carries the only other coverage for this seam and is
//! `#[ignore]`d behind `DEEPGRAM_API_KEY`. Per AGENTS.md §3.4 an ignored test runs only
//! with manual approval, which meant the Realtime PTT seam had ZERO coverage in any
//! automated run — despite Realtime being a shipping pipeline mode with its own
//! misrouting failure mode.
//!
//! Six of that file's seven assertions need no cloud provider at all. The most valuable
//! of them is the anti-misrouting invariant, asserted there three times:
//!
//! ```ignore
//! assert!(stt_rx.try_recv().is_err(),
//!     "Realtime PTT must NEVER dispatch SttCommand::Final to STT worker");
//! ```
//!
//! `pipeline/assistant/ptt.rs::dispatch_validated_ptt_audio` branches on
//! `pipeline_mode`: the `Modular` arm sends `SttCommand::Final` to `stt_tx`, and the
//! `Realtime` arm instead clamps to i16 and calls `rt_actor.signal_speech_committed`.
//! Reaching the wrong arm means a Realtime user pays for local STT inference they did
//! not ask for, on every single turn. That is fully verifiable offline.
//!
//! See integration-test-spec.md §0.1.6 (`#[ignore]` Constraint).
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use vox_lib::{
    core::{
        settings::{AudioOutputMode, InteractionMode, PipelineMode},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::assistant::ptt::{ptt_cancel, ptt_start, ptt_stop},
    services::{
        stt::actor::SttCommand,
        vad::{actor::VadActorConfig, VadCommand},
    },
};

/// Drives the Realtime PTT lifecycle and asserts the zero-local-STT-leak contract.
///
/// Entry seams: the real `ptt_start` / `ptt_stop` / `ptt_cancel` handlers, the real
/// `VadActor` in `InteractionMode::PTT`, and a real `mpsc::Receiver<SttCommand>` whose
/// emptiness is the observable exit.
///
/// The audio device is mocked (CPAL cannot open an output device headlessly) but the
/// ring-buffer producer feeds REAL PCM from a real clip, and the VAD actor is a real
/// thread. No realtime engine is mounted, so `signal_speech_committed` has no actor to
/// call — which is irrelevant to the invariant: the assertion is that the `Modular`
/// arm is never taken, and that arm's side effect is a message on `stt_rx`.
#[tokio::test]
async fn test_realtime_ptt_never_dispatches_to_local_stt() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;

        // Realtime pipeline mode is what selects the non-STT dispatch arm.
        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::PTT;
            settings.interaction.pipeline_mode = PipelineMode::Realtime;
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        // Real STT command channel. We never populate it; we assert it stays empty.
        let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();

        // Real VAD actor in PTT windowed-validation mode, using production defaults.
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
            audio_suppressed,
            state.pipeline.ingestion_gate.clone(),
            vad_shutdown.clone(),
        );
        common::harness::attach_mock_engine_with_vad_to_state(
            &app,
            &state,
            stt_tx.clone(),
            vad_cmd_tx.clone(),
        );

        // ---------------------------------------------------------------------
        // Scenario 1: real speech audio must reach Thinking WITHOUT touching STT
        // ---------------------------------------------------------------------
        state.pipeline.set_state(InteractionState::Ready);
        ptt_start(&app, &state).expect("ptt_start must succeed");
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Listening,
            "ptt_start must transition Ready -> Listening"
        );

        common::audio::stream_test_clip(common::ASSET_SUPERTONIC_01_EN_FILENAME, &mut producer);
        ptt_stop(&app, &state)
            .await
            .expect("ptt_stop must succeed");

        // The speech window was validated, so the pipeline must have advanced.
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Thinking,
            "validated speech must advance the pipeline to Thinking in Realtime mode"
        );

        // INVARIANT: the Modular arm was not taken — nothing was sent to local STT.
        assert!(
            stt_rx.try_recv().is_err(),
            "Realtime PTT must NEVER dispatch SttCommand::Final to the local STT worker"
        );

        // ---------------------------------------------------------------------
        // Scenario 2: ghost gate — silence hold must revert to Ready, still no STT
        // ---------------------------------------------------------------------
        while stt_rx.try_recv().is_ok() {}

        state.pipeline.set_state(InteractionState::Ready);
        ptt_start(&app, &state).expect("ptt_start must succeed");
        assert_eq!(state.pipeline.state(), InteractionState::Listening);

        common::audio::stream_silence_frames(&mut producer, 30);
        common::audio::wait_for_buffer_drain(&producer, 5);
        ptt_stop(&app, &state)
            .await
            .expect("ptt_stop must succeed");

        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "ghost gate: a silence hold must revert to Ready, not Thinking"
        );
        assert!(
            stt_rx.try_recv().is_err(),
            "ghost gate: a silence hold must NOT dispatch to the local STT worker"
        );

        // ---------------------------------------------------------------------
        // Scenario 3: cancel must cancel the turn token and still not touch STT
        // ---------------------------------------------------------------------
        while stt_rx.try_recv().is_ok() {}

        state.pipeline.set_state(InteractionState::Ready);
        ptt_start(&app, &state).expect("ptt_start must succeed");
        assert_eq!(state.pipeline.state(), InteractionState::Listening);

        let turn_token = state.pipeline.turn_token();
        assert!(
            !turn_token.is_cancelled(),
            "turn_token must be live after ptt_start"
        );

        common::audio::stream_silence_frames(&mut producer, 15);
        common::audio::wait_for_buffer_drain(&producer, 5);
        ptt_cancel(&app, &state).expect("ptt_cancel must succeed");

        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "ptt_cancel must revert to Ready"
        );
        assert!(
            turn_token.is_cancelled(),
            "ptt_cancel must cancel the in-flight turn token"
        );
        assert!(
            stt_rx.try_recv().is_err(),
            "a cancelled PTT turn must NOT dispatch to the local STT worker"
        );

        // ---------------------------------------------------------------------
        // Teardown: shut the actor down and prove the thread did not panic
        // ---------------------------------------------------------------------
        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        let (join_tx, join_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let panicked = vad_join.join().is_err();
            let _ = join_tx.send(panicked);
        });
        assert!(
            !join_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("VAD actor thread join timed out"),
            "VAD actor thread panicked during the Realtime PTT offline test"
        );
    })
    .await
    .expect("test_realtime_ptt_never_dispatches_to_local_stt timed out");
}
