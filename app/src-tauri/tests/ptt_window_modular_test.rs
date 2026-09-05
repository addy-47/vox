//! ============================================================================
//! ptt_window_modular_test.rs — PTT Window Validation (Modular) Integration Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : pipeline/assistant/ptt.rs + services/vad/actor.rs + services/stt/actor.rs
//! Prerequisites: Local Nemotron STT + Earshot VAD weights in ~/.vox/models/
//! Execution    : cargo nextest run --test ptt_window_modular_test --release --nocapture --test-threads=1
//! Metrics      : Latency, Levenshtein transcript similarity (>= 0.90), state transitions
//! ============================================================================

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::settings::{AudioOutputMode, InteractionMode, PipelineMode};
use vox_lib::core::state::{InteractionOwner, InteractionState};
use vox_lib::pipeline::assistant::ptt::{ptt_cancel, ptt_start, ptt_stop};
use vox_lib::services::stt::actor::SttCommand;
use vox_lib::services::vad::actor::VadActorConfig;
use vox_lib::services::vad::VadCommand;

#[tokio::test]
async fn test_ptt_modular_matrix() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state();

        // 1. Configure settings for Assistant Modular PTT
        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::PTT;
            settings.interaction.pipeline_mode = PipelineMode::Modular;
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);

        // 2. Setup production STT worker with local Nemotron
        let (stt_tx, pipeline_event_rx, stt_shutdown, stt_join) =
            common::harness::setup_stt_worker(&app);

        // 3. Setup production VAD actor in WindowedValidation (PTT) mode
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

        // 4. Attach mock engine with VAD & STT command senders to state
        common::harness::attach_mock_engine_with_vad_to_state(
            &app,
            &state,
            stt_tx.clone(),
            vad_cmd_tx.clone(),
        );

        // =====================================================================
        // Subtest 1: Speech transmits to STT -> Thinking -> TranscriptFinal
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Listening,
                "State must transition to Listening on ptt_start"
            );

            // Stream single-utterance EN clip while PTT is held
            let clip_path =
                common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
            let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
                .expect("Failed to decode supertonic_01_en_briefing.wav");

            common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
            common::audio::wait_for_buffer_drain(&producer, 5);

            // Release PTT
            ptt_stop(&app, &state)
                .await
                .expect("Failed to invoke ptt_stop");

            // Assert immediate transition to Thinking
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Thinking,
                "State must transition to Thinking upon speech validation and STT dispatch"
            );

            // Await TranscriptFinal
            let transcript = common::harness::collect_all_final_transcripts(
                &pipeline_event_rx,
                1,
                Duration::from_secs(15),
            );
            assert!(
                !transcript.is_empty(),
                "Transcript must not be empty for validated speech"
            );

            common::scoring::assert_similarity_above(
                &transcript,
                common::ASSET_SUPERTONIC_01_EN_GROUND_TRUTH,
                0.90,
                "PTT Modular Speech Validation",
            );
        }

        // =====================================================================
        // Subtest 2: Ghost Gate — Silence hold reverts to Ready (no speech)
        // =====================================================================
        {
            while pipeline_event_rx.try_recv().is_ok() {}

            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Listening,
                "State must be Listening after ptt_start"
            );

            // Stream silence frames during PTT hold
            common::audio::stream_silence_frames(&mut producer, 30);
            common::audio::wait_for_buffer_drain(&producer, 5);

            // Release PTT
            ptt_stop(&app, &state)
                .await
                .expect("Failed to invoke ptt_stop");

            // Ghost gate: silence must revert to Ready, not Thinking
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Ready,
                "Ghost gate: state must revert to Ready on silence hold"
            );

            // Confirm no TranscriptFinal is emitted
            common::harness::assert_channel_empty_after(
                &pipeline_event_rx,
                Duration::from_millis(500),
                "Ghost gate: pipeline_event_rx must remain empty for silence",
            );
        }

        // =====================================================================
        // Subtest 3: Cancel discards hold and cancels turn token
        // =====================================================================
        {
            while pipeline_event_rx.try_recv().is_ok() {}

            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Listening,
                "State must be Listening after ptt_start"
            );

            let turn_token = state.pipeline.turn_token();
            assert!(
                !turn_token.is_cancelled(),
                "turn_token must be active after ptt_start"
            );

            // Stream some audio chunks
            common::audio::stream_silence_frames(&mut producer, 15);
            common::audio::wait_for_buffer_drain(&producer, 5);

            // Cancel PTT
            ptt_cancel(&app, &state).expect("Failed to invoke ptt_cancel");

            assert_eq!(
                state.pipeline.state(),
                InteractionState::Ready,
                "State must revert to Ready after ptt_cancel"
            );
            assert!(
                turn_token.is_cancelled(),
                "turn_token must be cancelled on ptt_cancel"
            );

            common::harness::assert_channel_empty_after(
                &pipeline_event_rx,
                Duration::from_millis(500),
                "Cancel: pipeline_event_rx must remain empty",
            );
        }

        // =====================================================================
        // Teardown: Shutdown actors and join handles
        // =====================================================================
        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        let _ = stt_tx.send(SttCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        stt_shutdown.store(true, Ordering::SeqCst);

        vad_join.join().expect("VAD thread panicked");
        stt_join.join().expect("STT thread panicked");
    })
    .await
    .expect("Hard test timeout exceeded (60s)");
}
