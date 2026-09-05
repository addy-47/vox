//! ============================================================================
//! transcript_to_llm_test.rs — Transcript Handler → Context Harness → LLM Dispatch
//! ============================================================================
//! Category     : Integration Test (Seam 5)
//! Component    : pipeline/assistant/transcript.rs + services/harness/facade.rs + services/llm/actor.rs
//! Prerequisites: None (Mock/Zero external dependencies, in-memory harness)
//! Execution    : cargo nextest run --test transcript_to_llm_test --release --nocapture --test-threads=1
//! Metrics      : Latency, GenerationRequest shape, LLM dispatch, threshold maintenance
//! ============================================================================

mod common;

use futures_util::future::BoxFuture;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::settings::{LlmActiveProvider, LlmModelInfo, PipelineMode};
use vox_lib::core::state::{InteractionOwner, InteractionState};
use vox_lib::pipeline::assistant::transcript::on_transcript_final;
use vox_lib::pipeline::RoutingContext;
use vox_lib::services::harness::Role;
use vox_lib::services::llm::{
    GenerationPurpose, GenerationRequest, LlmCommand, LlmError, LlmProvider, LlmStreamEvent,
    ProviderCapabilities, ProviderKind,
};
use vox_lib::services::tts::TtsCommand;

/// Minimal mock LLM provider capturing calls for integration testing.
#[derive(Clone, Default)]
struct TestMockLlmProvider {
    capabilities: ProviderCapabilities,
}

impl LlmProvider for TestMockLlmProvider {
    fn generate<'a>(
        &'a self,
        _request: GenerationRequest,
        _turn_id: u32,
        _cancel: &'a tokio_util::sync::CancellationToken,
        tx: &'a std::sync::mpsc::Sender<LlmStreamEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let _ = tx.send(LlmStreamEvent::Token("Hello from mock LLM".to_string()));
            let _ = tx.send(LlmStreamEvent::Finished);
            Ok(())
        })
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    fn health_check<'a>(&'a self) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move { Ok(()) })
    }

    fn list_models<'a>(&'a self) -> BoxFuture<'a, Result<Vec<LlmModelInfo>, LlmError>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAiCompat
    }
}

#[tokio::test]
async fn test_transcript_to_llm_matrix() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state();

        // 1. Configure settings for Assistant Modular LLM
        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.pipeline_mode = PipelineMode::Modular;
            settings.llm.active = LlmActiveProvider::Server;
            settings.llm.max_output_tokens = 512;
            settings.llm.temperature = 0.7;
            settings.memory.context_retrieval_enabled = false;
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        // 2. Set mock provider in AppState cache
        let mock_provider = Arc::new(TestMockLlmProvider::default());
        *state.llm_provider.write() = Some(mock_provider);

        // 3. Channels for observing LLM dispatch, TTS filler, and STT commands
        let (llm_tx, llm_rx) = std::sync::mpsc::channel::<LlmCommand>();
        let (tts_tx, tts_rx) = std::sync::mpsc::channel::<TtsCommand>();
        let (stt_tx, _stt_rx) = std::sync::mpsc::channel();
        let (vad_tx, _vad_rx) = std::sync::mpsc::channel();

        common::harness::attach_mock_engine_with_llm_tts_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx),
            Some(tts_tx),
        );

        let ctx = RoutingContext::from_app_state(&state);

        // =====================================================================
        // Subtest 1: Valid transcript dispatches GenerationRequest to LLM
        // =====================================================================
        {
            state.pipeline.set_state(InteractionState::Thinking);
            let turn_id = 42;
            let user_query = "What is the weather in Tokyo today?".to_string();

            on_transcript_final(turn_id, user_query.clone(), &app, &state, &ctx);

            // Await GenerationRequest from llm_rx
            let received_cmd = tokio::task::spawn_blocking(move || {
                llm_rx.recv_timeout(Duration::from_secs(5))
            })
            .await
            .expect("spawn_blocking panicked")
            .expect("Expected LlmCommand::Generate within 5s");

            match received_cmd {
                LlmCommand::Generate {
                    request,
                    turn_id: received_turn,
                    accumulator,
                    ..
                } => {
                    assert_eq!(received_turn, turn_id, "Turn ID must match");
                    assert_eq!(
                        request.purpose,
                        GenerationPurpose::Conversation,
                        "Purpose must be Conversation"
                    );
                    assert_eq!(
                        request.options.max_output_tokens,
                        Some(512),
                        "Max output tokens must propagate from settings"
                    );

                    // Verify user message appended as last message in request
                    let last_msg = request
                        .input
                        .messages
                        .last()
                        .expect("Messages must not be empty");
                    assert_eq!(last_msg.role, Role::User);
                    assert_eq!(last_msg.content, user_query);

                    // Verify user transcript in accumulator
                    assert_eq!(
                        accumulator.lock().user_transcript(),
                        user_query,
                        "Accumulator must store user transcript"
                    );
                }
                other => panic!("Expected LlmCommand::Generate, got {:?}", other),
            }
        }

        // =====================================================================
        // Subtest 2: Empty / whitespace transcript guards to Ready (No LLM dispatch)
        // =====================================================================
        {
            let (new_llm_tx, new_llm_rx) = std::sync::mpsc::channel::<LlmCommand>();
            if let Ok(mut guard) = state.engine.try_lock() {
                if let Some(ref mut engine) = *guard {
                    engine.llm_tx = Some(new_llm_tx);
                }
            }

            state.pipeline.set_state(InteractionState::Thinking);
            on_transcript_final(43, "   \n\t  ".to_string(), &app, &state, &ctx);

            // Must revert to Ready
            assert_eq!(
                state.pipeline.state(),
                InteractionState::Ready,
                "Empty transcript must transition state to Ready"
            );

            // Channel must remain completely empty
            common::harness::assert_channel_empty_after(
                &new_llm_rx,
                Duration::from_millis(500),
                "Empty transcript must not dispatch to LLM",
            );
        }

        // =====================================================================
        // Subtest 3: Non-Thinking state drops transcript silently
        // =====================================================================
        {
            let (new_llm_tx, new_llm_rx) = std::sync::mpsc::channel::<LlmCommand>();
            if let Ok(mut guard) = state.engine.try_lock() {
                if let Some(ref mut engine) = *guard {
                    engine.llm_tx = Some(new_llm_tx);
                }
            }

            state.pipeline.set_state(InteractionState::Listening);
            on_transcript_final(44, "Speech during listening".to_string(), &app, &state, &ctx);

            common::harness::assert_channel_empty_after(
                &new_llm_rx,
                Duration::from_millis(300),
                "Transcript received in non-Thinking state must be dropped",
            );
        }

        // =====================================================================
        // Subtest 4: Realtime pipeline mode arms pending without LLM dispatch
        // =====================================================================
        {
            let (new_llm_tx, new_llm_rx) = std::sync::mpsc::channel::<LlmCommand>();
            if let Ok(mut guard) = state.engine.try_lock() {
                if let Some(ref mut engine) = *guard {
                    engine.llm_tx = Some(new_llm_tx);
                }
            }
            state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);
            state.pipeline.set_state(InteractionState::Thinking);

            let mut realtime_ctx = ctx.clone();
            realtime_ctx.pipeline_mode = PipelineMode::Realtime;

            on_transcript_final(45, "Realtime user query".to_string(), &app, &state, &realtime_ctx);

            // In Realtime mode, pending_synthesis_jobs is set to 1, but NO LlmCommand is dispatched
            assert_eq!(
                state.pipeline.pending_synthesis_jobs.load(Ordering::Relaxed),
                1,
                "Realtime transcript must arm pending_synthesis_jobs to 1"
            );

            common::harness::assert_channel_empty_after(
                &new_llm_rx,
                Duration::from_millis(300),
                "Realtime mode must not dispatch LlmCommand",
            );
        }

        // =====================================================================
        // Subtest 5: Critical threshold triggers transition speech filler & pending
        // =====================================================================
        {
            let (new_llm_tx, _new_llm_rx) = std::sync::mpsc::channel::<LlmCommand>();
            if let Ok(mut guard) = state.engine.try_lock() {
                if let Some(ref mut engine) = *guard {
                    engine.llm_tx = Some(new_llm_tx);
                }
            }

            // Seed conversation buffer with enough messages to exceed critical threshold (>85% of 2048 = >1740 tokens)
            {
                let mut cm = state.conversation_manager.lock();
                for i in 0..30 {
                    cm.push_user_turn(format!(
                        "Turn {} user statement with sufficient length and detail to accumulate tokens in accountant memory buffer.",
                        i
                    ));
                    cm.push_assistant_turn(format!(
                        "Turn {} assistant response describing system operations, memory compaction protocols, and pipeline states in detail.",
                        i
                    ));
                }
            }

            state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);
            state.pipeline.set_state(InteractionState::Thinking);

            on_transcript_final(46, "Final query exceeding threshold".to_string(), &app, &state, &ctx);

            // Await filler on tts_rx
            let filler_cmd = tokio::task::spawn_blocking(move || {
                tts_rx.recv_timeout(Duration::from_secs(5))
            })
            .await
            .expect("spawn_blocking panicked")
            .expect("Expected filler TtsCommand::Generate on critical threshold maintenance");

            match filler_cmd {
                TtsCommand::Generate { turn_id, text } => {
                    assert_eq!(turn_id, 46);
                    assert!(
                        vox_lib::core::constants::TRANSITION_MESSAGES_EN.contains(&text.as_str()),
                        "Filler text '{}' must be from TRANSITION_MESSAGES_EN",
                        text
                    );
                }
                other => panic!("Expected TtsCommand::Generate for filler, got {:?}", other),
            }

            // Verify pending_synthesis_jobs was incremented for the filler (poll with deadline)
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            let mut pending_incremented = false;
            while std::time::Instant::now() < deadline {
                if state.pipeline.pending_synthesis_jobs.load(Ordering::Relaxed) >= 1 {
                    pending_incremented = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert!(
                pending_incremented,
                "pending_synthesis_jobs must be incremented when filler is dispatched"
            );
        }
    })
    .await
    .expect("test_transcript_to_llm_matrix timed out");
}
