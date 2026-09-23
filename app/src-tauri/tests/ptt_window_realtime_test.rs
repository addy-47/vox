//! ============================================================================
//! ptt_window_realtime_test.rs — PTT Window Validation (Realtime) Integration Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : pipeline/assistant/ptt.rs + services/vad/actor.rs + services/realtime/actor.rs
//! Prerequisites: DEEPGRAM_API_KEY in temp/.env, Earshot VAD weights in ~/.vox/models/
//! Execution    : cargo nextest run --test ptt_window_realtime_test --release --nocapture --test-threads=1 -- --ignored
//! Metrics      : Latency, speech commit payload dispatch, sample clamping, state transitions
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
        events::VoxEvent,
        settings::{AudioOutputMode, InteractionMode, PipelineMode, RealtimeProviderKind},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::assistant::ptt::{ptt_cancel, ptt_start, ptt_stop},
    services::{
        realtime::{create_realtime_provider, RealtimeActor},
        stt::actor::SttCommand,
        vad::{actor::VadActorConfig, VadCommand},
    },
};

#[tokio::test]
#[ignore = "Live cloud provider test requiring DEEPGRAM_API_KEY in temp/.env"]
async fn test_ptt_realtime_matrix() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();
        let _ = rustls::crypto::ring::default_provider().install_default();
        vox_lib::utils::paths::init();

        let api_key = common::paths::load_api_key_from_env("DEEPGRAM_API_KEY")
            .expect("DEEPGRAM_API_KEY must be provided in environment or temp/.env");
        assert!(
            !api_key.trim().is_empty(),
            "DEEPGRAM_API_KEY cannot be empty"
        );

        let (app, state) = common::harness::get_test_app_and_state().await;

        // 1. Configure settings for Assistant Realtime PTT with real Deepgram Voice Agent provider
        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::PTT;
            settings.interaction.pipeline_mode = PipelineMode::Realtime;
            settings.realtime.active = RealtimeProviderKind::DeepgramVoiceAgent;
            settings.realtime.deepgram_voice_agent.api_key = api_key;
            settings.realtime.deepgram_voice_agent.model = "gpt-4o-mini".to_string();
            settings.realtime.deepgram_voice_agent.voice = "aura-asteria-en".to_string();
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);

        // 2. Setup Production RealtimeActor wired to real DeepgramVoiceAgentProvider
        let provider = create_realtime_provider(&state)
            .expect("Failed to create real DeepgramVoiceAgentProvider");

        let mut actor = RealtimeActor::new(provider, tokio::runtime::Handle::current());
        let (playback_engine, _) = common::harness::create_mock_playback_engine();
        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();

        let app_clone = app.clone();
        actor = tokio::task::spawn_blocking(move || {
            actor
                .start(InteractionMode::PTT, playback_engine, event_tx, app_clone)
                .expect("Failed to start RealtimeActor with real Deepgram Voice Agent provider");
            actor
        })
        .await
        .expect("spawn_blocking failed");

        *state.realtime_engine.lock().await = Some(actor);

        // 3. Setup production VAD actor in WindowedValidation (PTT) mode
        let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();
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

        common::harness::attach_mock_engine_with_vad_to_state(
            &app,
            &state,
            stt_tx.clone(),
            vad_cmd_tx.clone(),
        );

        // =====================================================================
        // Subtest 1: Speech transmits to Realtime actor -> commit speech turn
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Listening,
                "State must be Listening after ptt_start"
            );

            common::audio::stream_test_clip(common::ASSET_SUPERTONIC_01_EN_FILENAME, &mut producer);

            ptt_stop(&app, &state)
                .await
                .expect("Failed to invoke ptt_stop");

            assert_eq!(
                state.pipeline.state(),
                InteractionState::Thinking,
                "State must transition to Thinking upon Realtime speech commit"
            );

            // Anti-misrouting check: Ensure STT channel was NOT spammed with SttCommand::Final
            assert!(
                stt_rx.try_recv().is_err(),
                "Realtime PTT must NEVER dispatch SttCommand::Final to STT worker"
            );

            // Await server-side response event from Deepgram Voice Agent over live WebSocket
            // Deterministically proves signal_speech_committed was received and processed
            let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
            let mut server_response = None;
            while tokio::time::Instant::now() < deadline {
                match event_rx.try_recv() {
                    Ok(event) => match event {
                        VoxEvent::SpeechStart
                        | VoxEvent::SpeechEnd
                        | VoxEvent::TranscriptFinal { .. }
                        | VoxEvent::LlmFinished { .. } => {
                            server_response = Some(event);
                            break;
                        }
                        _ => {}
                    },
                    Err(mpsc::TryRecvError::Empty) => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }

            assert!(
                server_response.is_some(),
                "Deepgram Voice Agent must respond with server event after speech commit"
            );
        }

        // =====================================================================
        // Subtest 2: Ghost Gate — Silence hold reverts to Ready (zero commit)
        // =====================================================================
        {
            // Drain any prior events
            while event_rx.try_recv().is_ok() {}

            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(state.pipeline.state(), InteractionState::Listening);

            common::audio::stream_silence_frames(&mut producer, 30);
            common::audio::wait_for_buffer_drain(&producer, 5);

            ptt_stop(&app, &state)
                .await
                .expect("Failed to invoke ptt_stop");

            assert_eq!(
                state.pipeline.state(),
                InteractionState::Ready,
                "Ghost gate: state must revert to Ready on silence hold"
            );

            assert!(
                stt_rx.try_recv().is_err(),
                "Ghost gate: silence hold must NOT dispatch to STT worker"
            );

            // Deterministic wait verifying negative assertion (no speech/commit event emitted)
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert!(
                event_rx.try_recv().is_err(),
                "Ghost gate: silence hold must NOT commit or emit speech events"
            );
        }

        // =====================================================================
        // Subtest 3: Cancel discards hold and cancels turn token
        // =====================================================================
        {
            // Drain any prior events
            while event_rx.try_recv().is_ok() {}

            state.pipeline.set_state(InteractionState::Ready);
            ptt_start(&app, &state).expect("Failed to invoke ptt_start");
            assert_eq!(state.pipeline.state(), InteractionState::Listening);

            let turn_token = state.pipeline.turn_token();
            assert!(!turn_token.is_cancelled());

            common::audio::stream_silence_frames(&mut producer, 15);
            common::audio::wait_for_buffer_drain(&producer, 5);

            ptt_cancel(&app, &state).expect("Failed to invoke ptt_cancel");

            assert_eq!(state.pipeline.state(), InteractionState::Ready);
            assert!(
                turn_token.is_cancelled(),
                "turn_token must be cancelled on ptt_cancel"
            );

            assert!(
                stt_rx.try_recv().is_err(),
                "Cancelled PTT must NOT dispatch to STT worker"
            );

            tokio::time::sleep(Duration::from_millis(200)).await;
            assert!(
                event_rx.try_recv().is_err(),
                "Cancelled PTT must NOT commit or emit speech events"
            );
        }

        // =====================================================================
        // Teardown
        // =====================================================================
        if let Some(mut actor) = state.realtime_engine.lock().await.take() {
            actor.stop();
        }

        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        vad_join.join().expect("VAD thread panicked");
    })
    .await
    .expect("Hard test timeout exceeded (60s)");
}
