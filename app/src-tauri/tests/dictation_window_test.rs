//! ============================================================================
//! dictation_window_test.rs — Dictation Window Validation & Routing Integration Test
//! ============================================================================
//! Category     : Integration Test (Seam 4)
//! Component    : pipeline/dictation/{mod,ptt,speech,transcript,error}.rs + services/vad/actor.rs + services/dictation/output_router.rs
//! Prerequisites: Local Nemotron STT + Earshot VAD weights in ~/.vox/models/
//! Execution    : cargo nextest run --test dictation_window_test --release --nocapture --test-threads=1
//! Metrics      : Latency, Levenshtein transcript similarity (>= 0.90), state transitions, zero LLM leaks
//! ============================================================================

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{
    AudioOutputMode, DictationInteractionMode, DictationOutputMode, InteractionMode,
};
use vox_lib::core::state::{InteractionOwner, InteractionState};
use vox_lib::pipeline::dictation::transition_dictation;
use vox_lib::services::llm::LlmCommand;
use vox_lib::services::vad::actor::VadActorConfig;
use vox_lib::services::vad::VadCommand;

#[tokio::test]
async fn test_dictation_matrix() {
    let test_timeout = Duration::from_secs(90);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state();

        // 1. Configure settings for Dictation PTT mode with Tray output
        {
            let mut settings = state.settings.write().unwrap();
            settings.dictation.enabled = true;
            settings.dictation.interaction_mode = DictationInteractionMode::Ptt;
            settings.dictation.output_mode = DictationOutputMode::Tray;
        }
        state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
        transition_dictation(InteractionState::Ready, &app, &state);
        state.pipeline.update_ingestion_gate();

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

        // 4. Attach mock engine with LLM capture channel to verify LLM Zero Invariant
        let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
        common::harness::attach_mock_engine_with_llm_vad_to_state(
            &app,
            &state,
            stt_tx.clone(),
            vad_cmd_tx.clone(),
            Some(llm_tx),
        );

        // 5. Setup and spawn central event router
        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        *state.event_tx.lock() = Some(event_tx.clone());
        let router_join = vox_lib::pipeline::router::spawn_router(app.clone(), event_rx)
            .expect("Failed to spawn router thread");

        // =====================================================================
        // Subtest 1: PTT speech window routes to output, NEVER to LLM
        // =====================================================================
        {
            state
                .owner
                .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
            transition_dictation(InteractionState::Ready, &app, &state);
            state.pipeline.update_ingestion_gate();

            // Trigger PttStart via router
            event_tx
                .send(VoxEvent::PttStart)
                .expect("Failed to send PttStart");

            // Allow router pump to transition state
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Listening,
                "Dictation state must transition to Listening on PttStart"
            );

            // Stream single-utterance EN clip while PTT held
            let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
            let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
                .expect("Failed to decode supertonic_01_en_briefing.wav");
            common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
            common::audio::wait_for_buffer_drain(&producer, 5);

            // Release PTT via router
            event_tx
                .send(VoxEvent::PttStop)
                .expect("Failed to send PttStop");

            // Allow router pump to invoke on_ptt_stop and transition to Thinking
            tokio::time::sleep(Duration::from_millis(100)).await;
            let current_state = state.pipeline.dictation_state();
            assert!(
                current_state == InteractionState::Thinking || current_state == InteractionState::Ready,
                "State after valid PTT stop must be Thinking (or Ready if STT finished rapidly), got: {:?}",
                current_state
            );

            // Await TranscriptFinal from pipeline_event_rx
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
                "Dictation PTT EN Transcript",
            );

            // Re-inject TranscriptFinal into router to exercise dictation/transcript.rs
            let turn_id = state.pipeline.peek_turn_id();
            event_tx
                .send(VoxEvent::TranscriptFinal {
                    turn_id,
                    text: transcript.clone(),
                })
                .expect("Failed to send TranscriptFinal to router");

            // Allow router pump to complete routing
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Invariant 1: dictation_last_transcript contains processed text
            let last_tx = state.dictation_last_transcript.lock().clone();
            assert_eq!(
                last_tx.as_deref(),
                Some(transcript.as_str()),
                "dictation_last_transcript must be updated with transcribed text"
            );

            // Invariant 2: Dictation state recovers to Ready
            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Ready,
                "Dictation state must return to Ready after transcript routing"
            );

            // Invariant 3: LLM Zero Invariant — llm_rx channel must be completely empty
            common::harness::assert_channel_empty_after(
                &llm_rx,
                Duration::from_millis(500),
                "Dictation PTT LLM Zero Invariant",
            );
        }

        // =====================================================================
        // Subtest 2: Ghost Hold (silence / immediate release) discards to Ready
        // =====================================================================
        {
            state
                .owner
                .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
            transition_dictation(InteractionState::Ready, &app, &state);

            event_tx
                .send(VoxEvent::PttStart)
                .expect("Failed to send PttStart");
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Listening,
                "Must enter Listening on PttStart"
            );

            // Stream silence frames only
            common::audio::stream_silence_frames(&mut producer, 20);
            common::audio::wait_for_buffer_drain(&producer, 2);

            event_tx
                .send(VoxEvent::PttStop)
                .expect("Failed to send PttStop");
            tokio::time::sleep(Duration::from_millis(150)).await;

            // Ghost gate: state reverts to Ready, no STT Final dispatched
            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Ready,
                "Ghost gate: dictation state must revert to Ready on silence hold"
            );

            common::harness::assert_channel_empty_after(
                &pipeline_event_rx,
                Duration::from_millis(500),
                "Ghost gate: pipeline_event_rx must remain empty",
            );
        }

        // =====================================================================
        // Subtest 3: Cancel via event_tx discards audio to Ready
        // =====================================================================
        {
            state
                .owner
                .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
            transition_dictation(InteractionState::Ready, &app, &state);

            event_tx
                .send(VoxEvent::PttStart)
                .expect("Failed to send PttStart");
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(state.pipeline.dictation_state(), InteractionState::Listening);

            // Stream audio while listening
            let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
            let audio = common::audio::decode_wav_to_mono_16k(&clip_path).unwrap();
            common::audio::stream_audio_to_ring_buffer(&audio[..audio.len().min(4000)], &mut producer);

            // Cancel PTT via event_tx (tray cancellation path)
            event_tx
                .send(VoxEvent::PttCancel)
                .expect("Failed to send PttCancel");
            tokio::time::sleep(Duration::from_millis(100)).await;

            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Ready,
                "PttCancel must revert dictation state to Ready"
            );

            common::harness::assert_channel_empty_after(
                &pipeline_event_rx,
                Duration::from_millis(500),
                "PttCancel: no STT final event must be emitted",
            );
        }

        // =====================================================================
        // Subtest 4: Passive Speech routing without hotkey
        // =====================================================================
        {
            state
                .owner
                .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
            transition_dictation(InteractionState::Ready, &app, &state);

            // Trigger passive speech start
            event_tx
                .send(VoxEvent::SpeechStart)
                .expect("Failed to send SpeechStart");
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Listening,
                "Passive SpeechStart must transition dictation state to Listening"
            );

            // Trigger passive speech end
            event_tx
                .send(VoxEvent::SpeechEnd)
                .expect("Failed to send SpeechEnd");
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Thinking,
                "Passive SpeechEnd must transition dictation state to Thinking"
            );

            // Simulate STT transcript final arrival
            let turn_id = state.pipeline.peek_turn_id();
            let test_text = "Good morning vox dictation test".to_string();
            event_tx
                .send(VoxEvent::TranscriptFinal {
                    turn_id,
                    text: test_text.clone(),
                })
                .expect("Failed to send TranscriptFinal");
            tokio::time::sleep(Duration::from_millis(100)).await;

            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Ready,
                "Passive dictation must return to Ready after transcript"
            );
            assert_eq!(
                state.dictation_last_transcript.lock().as_deref(),
                Some(test_text.as_str())
            );

            common::harness::assert_channel_empty_after(
                &llm_rx,
                Duration::from_millis(300),
                "Passive Dictation LLM Zero Invariant",
            );
        }

        // =====================================================================
        // Subtest 5: Ingestion Gate Purge drops stale audio
        // =====================================================================
        {
            // Close ingestion gate by setting assistant to Idle and dictation to Idle
            state.pipeline.set_state(InteractionState::Idle);
            transition_dictation(InteractionState::Idle, &app, &state);
            state.pipeline.update_ingestion_gate();
            assert!(
                !state.pipeline.ingestion_gate.load(Ordering::Relaxed),
                "Ingestion gate must be closed when both assistant and dictation are Idle"
            );

            // Stream audio while gate is closed
            let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
            let audio = common::audio::decode_wav_to_mono_16k(&clip_path).unwrap();
            common::audio::stream_audio_to_ring_buffer(&audio[..audio.len().min(4000)], &mut producer);

            // Wait for VAD actor loop head to purge buffers
            tokio::time::sleep(Duration::from_millis(150)).await;

            // Reopen gate for dictation
            transition_dictation(InteractionState::Ready, &app, &state);
            state.pipeline.update_ingestion_gate();
            assert!(
                state.pipeline.ingestion_gate.load(Ordering::Relaxed),
                "Ingestion gate must reopen when dictation is Ready"
            );

            // Now initiate a clean PttStart -> PttStop with no new speech
            event_tx
                .send(VoxEvent::PttStart)
                .expect("Failed to send PttStart");
            tokio::time::sleep(Duration::from_millis(50)).await;

            event_tx
                .send(VoxEvent::PttStop)
                .expect("Failed to send PttStop");
            tokio::time::sleep(Duration::from_millis(150)).await;

            // Must revert to Ready via ghost gate, proving stale audio was purged
            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Ready,
                "Gate purge: stale audio must not trigger STT; ghost gate must return Ready"
            );

            common::harness::assert_channel_empty_after(
                &pipeline_event_rx,
                Duration::from_millis(500),
                "Gate purge: pipeline_event_rx must remain empty",
            );
        }

        // =====================================================================
        // Subtest 6: Idle start emits error and rejects recording
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Ready);
            transition_dictation(InteractionState::Idle, &app, &state);

            event_tx
                .send(VoxEvent::PttStart)
                .expect("Failed to send PttStart");
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Must NOT enter Listening
            assert_eq!(
                state.pipeline.dictation_state(),
                InteractionState::Idle,
                "PttStart when Idle must remain Idle"
            );

            common::harness::assert_channel_empty_after(
                &pipeline_event_rx,
                Duration::from_millis(300),
                "Idle start: no STT events emitted",
            );
        }

        // =====================================================================
        // Teardown
        // =====================================================================
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::Relaxed);
        stt_shutdown.store(true, Ordering::Relaxed);

        let _ = router_join.join();
        let _ = vad_join.join();
        let _ = stt_join.join();
    })
    .await
    .expect("test_dictation_matrix timed out");
}
