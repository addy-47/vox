//! ============================================================================
//! llm_to_tts_test.rs — Real LLM Streaming → Clause Chunking → TTS Dispatch
//! ============================================================================
//! Category     : Integration Test (Seam 6)
//! Component    : services/llm/actor.rs + services/llm/embedded + pipeline/assistant/accumulator.rs + services/tts/actor.rs + pipeline/assistant/llm.rs
//! Prerequisites: Local Qwen 3.5 GGUF model in ~/.vox/models/llm/qwen/
//! Execution    : cargo nextest run --test llm_to_tts_test --release --nocapture --test-threads=1
//! Metrics      : Real token generation, clause chunking determinism, TTS command dispatch, pending accounting
//! ============================================================================

mod common;

use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    time::Duration,
};

use vox_lib::{
    core::{
        events::VoxEvent,
        settings::PipelineMode,
        state::{InteractionOwner, InteractionState},
    },
    pipeline::{assistant::llm::on_llm_finished, RoutingContext},
    services::{
        harness::{ChatMessage, Role},
        llm::{
            ConversationInput, EmbeddedProvider, GenerationOptions, GenerationPurpose,
            GenerationRequest, LlmCommand, OutputConstraint,
        },
        tts::TtsCommand,
    },
};

#[tokio::test]
async fn test_real_llm_to_tts_matrix() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        let (app, state) = common::harness::get_test_app_and_state();

        let qwen_model_path = common::paths::get_qwen_model_path();
        assert!(
            qwen_model_path.exists(),
            "Qwen GGUF model must exist at {:?}",
            qwen_model_path
        );

        // 1. Create real EmbeddedProvider using local Qwen model (ctx_size=2048, n_threads=4)
        let provider = Arc::new(
            EmbeddedProvider::new(&qwen_model_path, 2048, 4)
                .expect("Failed to load local Qwen GGUF model via EmbeddedProvider"),
        );

        // 2. Channels for capturing TTS commands and pipeline events
        let (stt_tx, _) = mpsc::channel();
        let (vad_tx, _) = mpsc::channel();
        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();

        common::harness::attach_mock_engine_with_llm_tts_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx.clone()),
            Some(tts_tx.clone()),
        );

        state.pipeline.set_state(InteractionState::Thinking);

        // 3. Spawn real persistent LLM worker actor thread
        let worker_app = app.clone();
        let worker_event_tx = event_tx.clone();
        let worker_provider = Arc::clone(&provider);
        let worker_handle = std::thread::spawn(move || {
            vox_lib::services::llm::actor::spawn_llm_worker(
                worker_app,
                llm_rx,
                worker_provider,
                worker_event_tx,
            );
        });

        let cancel = state.pipeline.turn_token();
        let accumulator = Arc::clone(&state.pipeline_accumulator);
        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);

        let turn_id = 201;

        // Prompt designed to elicit multiple punctuated sentences
        let input_messages = vec![
            ChatMessage {
                role: Role::System,
                content: "You are a concise voice assistant. Reply in exactly two short sentences.".to_string(),
                timestamp_ms: 1000,
            },
            ChatMessage {
                role: Role::User,
                content: "Hello! State your name and your purpose.".to_string(),
                timestamp_ms: 2000,
            },
        ];

        let request = GenerationRequest {
            input: ConversationInput { messages: input_messages },
            options: GenerationOptions {
                max_output_tokens: Some(40),
                temperature: Some(0.1),
                ..Default::default()
            },
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
        };

        // ---------------------------------------------------------------------
        // Entry Seam: Send real LlmCommand::Generate to worker
        // ---------------------------------------------------------------------
        llm_tx
            .send(LlmCommand::Generate {
                request: Box::new(request),
                turn_id,
                cancel,
                accumulator: Arc::clone(&accumulator),
                tts_tx: Some(tts_tx.clone()),
                pending_synthesis_jobs: Arc::clone(&pending_jobs),
            })
            .expect("Failed to send Generate to LLM worker");

        // ---------------------------------------------------------------------
        // Observable Exit 1: Real tokens streamed, chunked into clauses, and dispatched
        // Wait for VoxEvent::LlmFinished
        // ---------------------------------------------------------------------
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut finished = false;
        while std::time::Instant::now() < deadline {
            if let Ok(VoxEvent::LlmFinished { turn_id: tid }) =
                event_rx.recv_timeout(Duration::from_millis(50))
            {
                if tid == turn_id {
                    finished = true;
                    break;
                }
            }
        }
        assert!(
            finished,
            "Real Qwen model must complete generation and emit VoxEvent::LlmFinished within 30s"
        );

        // ---------------------------------------------------------------------
        // Observable Exit 2: Assistant response accumulated
        // ---------------------------------------------------------------------
        let full_text = accumulator.lock().assistant_response.clone();
        assert!(
            !full_text.trim().is_empty(),
            "Accumulator must capture real generated tokens from Qwen"
        );
        log::info!("[Test] Real Qwen generated text: '{}'", full_text);

        // ---------------------------------------------------------------------
        // Observable Exit 3: Real streaming TTS clauses emitted during generation
        // ---------------------------------------------------------------------
        let mut streaming_clauses = Vec::new();
        while let Ok(cmd) = tts_rx.try_recv() {
            if let TtsCommand::Generate { turn_id: tid, text } = cmd {
                assert_eq!(tid, turn_id, "TTS command turn_id must match turn under test");
                streaming_clauses.push(text);
            }
        }

        assert!(
            !streaming_clauses.is_empty(),
            "Real LLM token streaming must chunk and dispatch at least 1 clause BEFORE LlmFinished: {:?}",
            streaming_clauses
        );

        // ---------------------------------------------------------------------
        // Simulate router dispatch of on_llm_finished to flush tail remainder
        // ---------------------------------------------------------------------
        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Modular,
            interaction_mode: vox_lib::core::settings::InteractionMode::PTT,
            owner: InteractionOwner::Assistant,
        };
        on_llm_finished(turn_id, &state, &ctx);

        // Shutdown worker
        let _ = llm_tx.send(LlmCommand::Shutdown);
        let _ = worker_handle.join();

        // ---------------------------------------------------------------------
        // Observable Exit 4: Collect all clauses (streaming + flushed tail remainder)
        // ---------------------------------------------------------------------
        let mut all_clauses = streaming_clauses;
        while let Ok(cmd) = tts_rx.try_recv() {
            if let TtsCommand::Generate { turn_id: tid, text } = cmd {
                assert_eq!(tid, turn_id, "TTS command turn_id must match turn under test");
                all_clauses.push(text);
            }
        }

        // ---------------------------------------------------------------------
        // Observable Exit 5: pending_synthesis_jobs accounting matches clauses
        // ---------------------------------------------------------------------
        let final_pending = pending_jobs.load(Ordering::Relaxed);
        assert_eq!(
            final_pending,
            all_clauses.len() as u32,
            "pending_synthesis_jobs ({}) must exactly equal dispatched clause count ({})",
            final_pending,
            all_clauses.len()
        );

        for (idx, clause) in all_clauses.iter().enumerate() {
            assert!(
                !clause.trim().is_empty(),
                "Dispatched clause {} must not be empty",
                idx
            );
            log::info!("[Test] Dispatched Clause {}: '{}'", idx + 1, clause);
        }
    })
    .await
    .expect("test_real_llm_to_tts_matrix timed out");
}
