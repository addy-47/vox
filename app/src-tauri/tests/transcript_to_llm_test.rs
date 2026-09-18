//! ============================================================================
//! transcript_to_llm_test.rs — STT Transcript ──► Harness prepare_turn ──► Duplex Pipe
//! ============================================================================
//! Category     : Integration Test (Seam 5)
//! Component    : pipeline/assistant/transcript.rs + services/harness/session.rs + services/llm/actor.rs + services/llm/embedded
//! Prerequisites: Local Qwen 3.5 GGUF model in ~/.vox/models/llm/qwen/
//! Execution    : cargo nextest run --test transcript_to_llm_test --release --nocapture --test-threads=1
//! Metrics      : Latency, GenerationRequest shape, real LLM token generation, threshold maintenance
//! ============================================================================

mod common;

use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    time::Duration,
};

use vox_lib::{
    core::{
        events::{AudioIntent, VoxEvent},
        settings::{AudioOutputMode, InteractionMode, LlmActiveProvider, PipelineMode},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::{assistant::transcript::on_transcript_final, RoutingContext},
    services::{
        harness::{Harness, TRANSITION_MESSAGES_EN},
        llm::{
            actor::{spawn_llm_worker, LlmCommand},
            EmbeddedProvider,
        },
        tts::TtsCommand,
    },
};

#[tokio::test]
async fn test_transcript_to_llm_matrix() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();

        let (app, state) = common::harness::get_test_app_and_state().await;

        let qwen_model_path = common::paths::get_qwen_model_path();
        assert!(
            qwen_model_path.exists(),
            "Qwen GGUF model must exist at {:?}",
            qwen_model_path
        );

        // 1. Configure settings for Assistant Modular LLM with real local EmbeddedProvider
        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::Passive;
            settings.interaction.pipeline_mode = PipelineMode::Modular;
            settings.audio.output_mode = AudioOutputMode::Headset;
            settings.llm.active = LlmActiveProvider::Embedded;
            settings.llm.context_window = 8192;
            settings.llm.max_output_tokens = 512;
            settings.llm.temperature = 0.7;
            settings.working_memory.auto_compaction = true;
            settings.personal_memory.context_retrieval_enabled = false;
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        // 2. Instantiate real local EmbeddedProvider (ctx_size=4200, n_threads=4)
        let provider = Arc::new(
            EmbeddedProvider::new(&qwen_model_path, 4200, 4)
                .expect("Failed to load local Qwen GGUF model via EmbeddedProvider"),
        );
        *state.llm_provider.write() = Some(provider.clone());

        // 3. Channels for capturing STT, VAD, LLM, TTS filler, and pipeline events
        let common::harness::PipelineTestChannels {
            stt_tx,
            vad_tx,
            tts_tx,
            tts_rx,
            llm_tx,
            llm_rx,
            pipeline_tx,
            pipeline_rx,
        } = common::harness::setup_pipeline_channels();

        common::harness::attach_mock_engine_with_pipeline_tx_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx.clone()),
            Some(tts_tx),
            pipeline_tx,
        );

        // 4. Mount production modular Harness
        {
            let settings = state.settings.read().unwrap().clone();
            let prompt = state.resolve_base_prompt();
            *state.harness.lock() = Some(Harness::new_modular(
                Some(1),
                prompt,
                None,
                &settings,
                llm_tx.clone(),
            ));
        }

        // 5. Spawn real background LLM worker thread using production EmbeddedProvider
        let worker_provider = Arc::clone(&provider);
        let worker_handle = std::thread::spawn(move || {
            spawn_llm_worker(llm_rx, worker_provider);
        });

        let ctx = RoutingContext::from_app_state(&state);

        // =====================================================================
        // Subtest 1: Valid transcript dispatches GenerationRequest and yields LlmFinished
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Thinking);
            let turn_id = 42;
            let user_query = "State the capital of France in one word.".to_string();

            on_transcript_final(turn_id, user_query.clone(), &app, &state, &ctx);

            // Accumulator must immediately reflect user transcript
            assert_eq!(
                state.pipeline_accumulator.lock().user_transcript(),
                user_query,
                "Accumulator must store user transcript"
            );

            // Await real LLM generation and stream routing completion from pipeline_rx
            let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
            let mut finished_received = false;
            while tokio::time::Instant::now() < deadline {
                match pipeline_rx.try_recv() {
                    Ok(VoxEvent::LlmFinished { turn_id: finished_turn }) => {
                        assert_eq!(finished_turn, turn_id, "LlmFinished turn_id must match");
                        finished_received = true;
                        break;
                    }
                    Ok(_) => {}
                    Err(mpsc::TryRecvError::Empty) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }

            assert!(
                finished_received,
                "Real LLM generation must route tokens and emit VoxEvent::LlmFinished within 15s"
            );

            // Accumulator must contain non-empty assistant response generated by real local LLM
            let assistant_text = state.pipeline_accumulator.lock().assistant_response.clone();
            assert!(
                !assistant_text.trim().is_empty(),
                "Accumulator assistant_response must be populated with tokens from real local LLM, got: {:?}",
                assistant_text
            );
        }

        // =====================================================================
        // Subtest 2: Empty / whitespace transcript guards to Ready (No LLM dispatch)
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Thinking);
            on_transcript_final(43, "   \n\t  ".to_string(), &app, &state, &ctx);

            // Must revert to Ready
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Ready,
                "Empty transcript must transition state to Ready"
            );

            // Negative assertion: no pipeline events emitted
            common::harness::assert_channel_empty_after(
                &pipeline_rx,
                Duration::from_millis(500),
                "Empty transcript must not generate LLM events",
            );
        }

        // =====================================================================
        // Subtest 3: Non-Thinking state drops transcript silently
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Listening);
            on_transcript_final(44, "Speech during listening".to_string(), &app, &state, &ctx);

            assert_eq!(
                state.pipeline.state(),
                InteractionState::Listening,
                "Transcript received in Listening state must not alter pipeline state"
            );

            common::harness::assert_channel_empty_after(
                &pipeline_rx,
                Duration::from_millis(300),
                "Transcript received in non-Thinking state must be dropped with zero events",
            );
        }

        // =====================================================================
        // Subtest 4: Realtime pipeline mode leaves state in Thinking without modular LLM dispatch
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Thinking);

            let mut realtime_ctx = ctx.clone();
            realtime_ctx.pipeline_mode = PipelineMode::Realtime;

            on_transcript_final(45, "Realtime user query".to_string(), &app, &state, &realtime_ctx);

            assert_eq!(
                state.pipeline.state(),
                InteractionState::Thinking,
                "Realtime transcript must preserve Thinking state awaiting provider stream"
            );

            assert_eq!(
                state.pipeline_accumulator.lock().user_transcript(),
                "Realtime user query",
                "User transcript must be logged in accumulator"
            );

            // Negative assertion: modular LLM receives zero requests
            common::harness::assert_channel_empty_after(
                &pipeline_rx,
                Duration::from_millis(500),
                "Realtime mode must not dispatch to modular LLM pipeline",
            );
        }

        // =====================================================================
        // Subtest 5: Critical threshold triggers transition speech filler & Working state
        // =====================================================================
        {
            // Drain any prior TTS commands from previous turns (e.g. Subtest 1 responses)
            while tts_rx.try_recv().is_ok() {}

            // Seed conversation buffer with enough messages to exceed critical threshold (>85% of usable 7680 = >6528 tokens; 105 turns ≈ 7035 tokens)
            {
                let mut guard = state.harness.lock();
                let harness = guard.as_mut().expect("Harness must be mounted");
                for i in 0..105 {
                    harness.push_user_turn(format!(
                        "Turn {} user statement with sufficient length and detail to accumulate tokens in accountant memory buffer. We are discussing neural networks, integration testing, and long context tracking across conversational agents.",
                        i
                    ));
                    harness.push_assistant_turn(format!(
                        "Turn {} assistant response describing system operations, memory compaction protocols, and pipeline states in detail. High token utilization will trigger inline compaction and transition filler phrase dispatch.",
                        i
                    ));
                }
            }

            state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);
            state.pipeline.set_state(InteractionState::Thinking);

            on_transcript_final(46, "Final query exceeding threshold".to_string(), &app, &state, &ctx);

            // Await filler on tts_rx
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            let mut filler_received = None;
            while tokio::time::Instant::now() < deadline {
                if let Ok(cmd) = tts_rx.try_recv() {
                    filler_received = Some(cmd);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            let filler_cmd = filler_received.expect("Expected filler TtsCommand::Generate on critical threshold");
            match filler_cmd {
                TtsCommand::Generate {
                    turn_id,
                    text,
                    intent,
                } => {
                    assert_eq!(turn_id, 46);
                    assert_eq!(
                        intent,
                        AudioIntent::InterimFiller,
                        "Filler must have InterimFiller intent"
                    );
                    assert!(
                        TRANSITION_MESSAGES_EN.contains(&text.as_str()),
                        "Filler text '{}' must be from TRANSITION_MESSAGES_EN",
                        text
                    );
                }
                other => panic!("Expected TtsCommand::Generate for filler, got {:?}", other),
            }

            // Verify state entered Working and pending_synthesis_jobs was incremented
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Working,
                "State must transition to Working when critical compaction filler is dispatched"
            );

            assert!(
                state.pipeline.pending_synthesis_jobs.load(Ordering::Relaxed) >= 1,
                "pending_synthesis_jobs must be incremented when filler is dispatched"
            );

            // Cancel turn token to immediately abort background compaction LLM execution
            state.pipeline.turn_token().cancel();
        }

        // =====================================================================
        // Teardown
        // =====================================================================
        let _ = llm_tx.send(LlmCommand::Shutdown);
        let join_res = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                worker_handle.join().expect("LLM worker thread panicked");
            }),
        )
        .await;
        assert!(join_res.is_ok(), "LLM worker thread join timed out");
    })
    .await
    .expect("test_transcript_to_llm_matrix timed out");
}
