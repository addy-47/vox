//! ============================================================================
//! llm_test.rs — LLM Actor & Token Streaming Integration Tests (Seam 5)
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/llm/actor, services/llm/providers
//! Prerequisites: Local Qwen GGUF (~/.vox/models/llm/qwen/)
//! Execution    : cargo test --test llm_test --release -- --nocapture
//! Metrics      : Response Token Streaming, Completion Signals, Cancellation
//! ============================================================================

mod common;

use common::harness::get_test_app_handle;
use common::paths::get_qwen_model_path;
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::VoxSettings;
use vox_lib::services::harness::{ChatMessage, Role};
use vox_lib::services::llm::actor::{cool_down_llm, warm_up_llm, LlmCommand, LlmWarmUpHandles};
use vox_lib::services::llm::{
    ConversationInput, GenerationOptions, GenerationPurpose, GenerationRequest, OutputConstraint,
};

/// Helper to set up LLM worker with local embedded Qwen model.
fn setup_test_llm_worker() -> (
    std::sync::mpsc::Sender<LlmCommand>,
    std::sync::mpsc::Receiver<VoxEvent>,
    Option<std::thread::JoinHandle<()>>,
) {
    let app = get_test_app_handle();
    let settings = VoxSettings::default();
    let llm_path = get_qwen_model_path();
    let (event_tx, event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let mut llm_tx: Option<std::sync::mpsc::Sender<LlmCommand>> = None;
    let mut llm_handle: Option<std::thread::JoinHandle<()>> = None;

    let handles = LlmWarmUpHandles {
        llm_tx: &mut llm_tx,
        llm_handle: &mut llm_handle,
        llm_provider_cache: None,
    };

    warm_up_llm(&app, handles, &settings, &llm_path, event_tx)
        .expect("Failed to warm up LLM worker");

    (tts_tx_handle(llm_tx), event_rx, llm_handle)
}

fn tts_tx_handle(
    llm_tx: Option<std::sync::mpsc::Sender<LlmCommand>>,
) -> std::sync::mpsc::Sender<LlmCommand> {
    llm_tx.expect("llm_tx initialized")
}

/// Consolidated LLM Generation & Cancellation Matrix with Single Lifecycle & Hard Timeout.
#[test]
fn test_llm_generation_and_cancel_matrix() {
    let start_time = std::time::Instant::now();
    let max_test_duration = Duration::from_secs(35);

    let (llm_tx, event_rx, llm_handle) = setup_test_llm_worker();

    // 1. Positive Generation: English Prompt -> Token Stream -> LlmFinished
    {
        let request = GenerationRequest {
            input: ConversationInput {
                messages: vec![ChatMessage {
                    role: Role::User,
                    content: "Respond with the word 'READY' and nothing else.".to_string(),
                    timestamp_ms: 0,
                }],
            },
            options: GenerationOptions {
                max_output_tokens: Some(16),
                temperature: Some(0.1),
                ..Default::default()
            },
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
        };

        let accumulator = Arc::new(parking_lot::Mutex::new(
            vox_lib::pipeline::handlers::accumulator::TurnAccumulator::new(),
        ));
        let pending_jobs = Arc::new(std::sync::atomic::AtomicU32::new(0));

        let cancel_token = tokio_util::sync::CancellationToken::new();
        llm_tx
            .send(LlmCommand::Generate {
                request: Box::new(request),
                turn_id: 1,
                cancel: cancel_token,
                accumulator: Arc::clone(&accumulator),
                tts_tx: None,
                pending_synthesis_jobs: Arc::clone(&pending_jobs),
            })
            .expect("Failed to send LlmCommand");

        let mut finished = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(20);

        while std::time::Instant::now() < deadline && !finished {
            if let Ok(VoxEvent::LlmFinished { turn_id }) =
                event_rx.recv_timeout(Duration::from_millis(100))
            {
                assert_eq!(turn_id, 1);
                finished = true;
            }
        }

        let full_response = accumulator.lock().assistant_response.clone();
        println!("\n=== [LLM Response Matrix] ===");
        println!("Response Text  : {}", full_response.trim());

        assert!(
            !full_response.trim().is_empty(),
            "LLM actor must accumulate assistant response text"
        );
        assert!(finished, "LLM actor must emit VoxEvent::LlmFinished");
    }

    // 2. Negative Invariant: Pre-cancelled request halts immediately
    {
        let accumulator = Arc::new(parking_lot::Mutex::new(
            vox_lib::pipeline::handlers::accumulator::TurnAccumulator::new(),
        ));
        let pending_jobs = Arc::new(std::sync::atomic::AtomicU32::new(0));

        let cancel_token = tokio_util::sync::CancellationToken::new();
        cancel_token.cancel();
        let request = GenerationRequest {
            input: ConversationInput {
                messages: vec![ChatMessage {
                    role: Role::User,
                    content: "Write a long essay about the solar system and planets.".to_string(),
                    timestamp_ms: 0,
                }],
            },
            options: GenerationOptions {
                max_output_tokens: Some(128),
                temperature: Some(0.7),
                ..Default::default()
            },
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
        };

        llm_tx
            .send(LlmCommand::Generate {
                request: Box::new(request),
                turn_id: 2,
                cancel: cancel_token,
                accumulator: Arc::clone(&accumulator),
                tts_tx: None,
                pending_synthesis_jobs: Arc::clone(&pending_jobs),
            })
            .expect("Failed to send LlmCommand");

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Ok(VoxEvent::LlmFinished { .. }) =
                event_rx.recv_timeout(Duration::from_millis(50))
            {
                break;
            }
        }

        let emitted_len = accumulator.lock().assistant_response.len();
        println!(
            "\n=== [LLM Cancel Guard] Characters Accumulated: {} ===",
            emitted_len
        );
        assert!(
            emitted_len <= 15,
            "Cancelled LLM generation must halt token generation immediately"
        );
    }

    // 3. Graceful Teardown & Panic Verification
    let mut tx_opt = Some(llm_tx);
    cool_down_llm(&mut tx_opt, None);
    if let Some(handle) = llm_handle {
        handle
            .join()
            .expect("LLM worker thread panicked during shutdown");
    }

    assert!(
        start_time.elapsed() < max_test_duration,
        "LLM matrix test exceeded hard timeout of 35s"
    );
}
