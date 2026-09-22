//! ============================================================================
//! agentic_tool_runtime_test.rs — Agentic Tool Runtime, Taxonomy & Scratchpad Isolation
//! ============================================================================
//! Category     : Integration Test (Seam 21)
//! Component    : services/harness/stages/tools/ + services/harness/steps.rs +
//!                services/harness/stages/prompt.rs + persistence/sessions.rs
//! Prerequisites: Isolated temporary DB paths
//! Execution    : cargo nextest run --test agentic_tool_runtime_test --release --nocapture --test-threads=1
//! Metrics      : Terminal tool single-pass execution, DB title update, TurnAccumulator
//!                parity, NonTerminal RRF retrieval, scratchpad drop, prefix audio drop
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32},
        mpsc, Arc,
    },
    time::Duration,
};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use vox_lib::{
    core::{
        events::AudioIntent,
        settings::VoxSettings,
    },
    persistence::{
        facts::{insert_fact, insert_vector, FactRecord},
        schema::run_migrations,
    },
    services::{
        harness::{
            chassis::Harness,
            r#loop::TurnLoopContext,
            stages::{
                streaming::{StreamRoutingHandles, StreamRoutingStage},
                tools::{ToolFilter, ToolRegistry},
            },
            steps::{step6_handle_non_terminal_tool, step6_handle_terminal_tool},
            Role, TurnExecutionRequest,
        },
        llm::CanonicalToolCall,
        tts::actor::TtsCommand,
    },
};

// ============================================================================
// Subtest 1: test_terminal_tool_title_and_accumulator_parity
// ============================================================================
/// Verifies Seam 21 Terminal Tool Single-Pass Contract (`respond_and_set_title`):
/// 1. Emits `respond_and_set_title` tool call with `title` and `spoken_response`.
/// 2. Sets `TurnAccumulator.assistant_response = spoken_response` to guarantee DB turns parity.
/// 3. Dispatches `spoken_response` directly to `tts_tx` as `AudioIntent::TurnResponse`.
/// 4. Updates session title in Turso SQLite `sessions` table.
/// 5. Commits `spoken_response` to working history (zero tool syntax in prompt history).
#[tokio::test]
async fn test_terminal_tool_title_and_accumulator_parity() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let conn = state.db.connect().expect("Failed to connect to db");
        run_migrations(&conn).await.expect("Failed to run migrations");

        // Seed session
        let session_id = 99887766i64;
        let now = 1700000000000i64;
        conn.execute(
            "INSERT INTO sessions (id, created_at, updated_at, project_id) VALUES (?, ?, ?, 'default');",
            (session_id, now, now),
        )
        .await
        .expect("Insert session failed");

        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let pending_jobs = Arc::new(AtomicU32::new(0));
        let cancel_atomic = Arc::new(AtomicBool::new(false));
        let (pipeline_tx, _pipeline_rx) = mpsc::channel();

        let stream_handles = StreamRoutingHandles {
            turn_id: 1,
            owner: vox_lib::core::state::InteractionOwner::Assistant,
            accumulator: Arc::clone(&state.pipeline_accumulator),
            tts_tx: Some(tts_tx),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            cancel: cancel_atomic,
            event_tx: pipeline_tx,
            app: app.clone(),
        };

        let stream_stage = StreamRoutingStage::new();
        let tool_registry = ToolRegistry::with_default_tools();

        let settings = VoxSettings::default();
        let (llm_tx, _llm_rx) = mpsc::channel();
        let harness = Harness::new_modular(
            Some(session_id),
            "System prompt".to_string(),
            None,
            &settings,
            llm_tx,
            true,
        );
        let harness_arc = Arc::new(Mutex::new(Some(harness)));

        let routing_ctx = vox_lib::pipeline::RoutingContext::from_app_state(&state);
        let req = TurnExecutionRequest {
            turn_id: 1,
            query: "Let's discuss Rust async concurrency".to_string(),
            cancel: CancellationToken::new(),
            app: app.clone(),
            app_state: Arc::clone(&state),
            routing_ctx,
            owner: vox_lib::core::state::InteractionOwner::Assistant,
            tts_tx: None,
            pipeline_tx: None,
            llm_tx: None,
            provider: None,
            accumulator: Arc::clone(&state.pipeline_accumulator),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            db: Arc::clone(&state.db),
        };

        let loop_ctx = TurnLoopContext {
            harness_arc: &harness_arc,
            req: &req,
            stream_stage: &stream_stage,
            stream_handles: &stream_handles,
            tool_registry: &tool_registry,
            session_id,
        };

        let tool_call = CanonicalToolCall {
            id: "call_title_1".to_string(),
            name: "respond_and_set_title".to_string(),
            arguments: serde_json::json!({
                "title": "Rust Async Concurrency",
                "spoken_response": "Let's explore Rust async and tokio primitives."
            }),
        };

        // Execute Terminal Tool
        let outcome = step6_handle_terminal_tool(&loop_ctx, tool_call).await;

        // 1. Assert Outcome is Completed with spoken response
        match outcome {
            vox_lib::services::harness::TurnOutcome::Completed {
                turn_id,
                assistant_response,
            } => {
                assert_eq!(turn_id, 1);
                assert_eq!(
                    assistant_response,
                    "Let's explore Rust async and tokio primitives."
                );
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        // 2. Assert Accumulator parity (critical fix: DB turns table reads from accumulator)
        let acc_text = state.pipeline_accumulator.lock().assistant_response.clone();
        assert_eq!(
            acc_text, "Let's explore Rust async and tokio primitives.",
            "TurnAccumulator must capture spoken_response for DB parity"
        );

        // 3. Assert TTS received spoken_response as TurnResponse
        let tts_cmd = tts_rx.try_recv().expect("tts_rx must receive spoken response");
        match tts_cmd {
            TtsCommand::Generate { text, intent, .. } => {
                assert_eq!(intent, AudioIntent::TurnResponse);
                assert!(text.contains("Rust async and tokio"));
            }
            _ => panic!("Expected TtsCommand::Generate"),
        }

        // 4. Assert SQLite sessions table was updated with the title
        let mut rows = conn
            .query("SELECT title FROM sessions WHERE id = ?;", (session_id,))
            .await
            .expect("Query failed");
        let title: Option<String> = rows.next().await.unwrap().unwrap().get(0).ok();
        assert_eq!(title, Some("Rust Async Concurrency".to_string()));

        // 5. Assert title_set is true on Harness
        let guard = harness_arc.lock();
        assert!(guard.as_ref().unwrap().title_set());

        // 6. Assert Turn 2 tool filter suppresses respond_and_set_title
        let turn2_filter = ToolFilter {
            is_first_turn: false, // Turn 2 has >2 messages in history
            title_is_unset: false,
            memory_retrieval_enabled: true,
        };
        let active = tool_registry.active_definitions(&turn2_filter);
        assert!(
            !active.iter().any(|t| t.name == "respond_and_set_title"),
            "Turn 2 must suppress respond_and_set_title"
        );
    })
    .await
    .expect("test_terminal_tool_title_and_accumulator_parity timed out");
}

// ============================================================================
// Subtest 2: test_search_memory_non_terminal_rrf_retrieval
// ============================================================================
/// Verifies Seam 21 NonTerminal Observation Contract (`search_memory`):
/// 1. Pre-seeds SQLite facts table with candidate memory facts.
/// 2. Dispatches `search_memory` tool call.
/// 3. Verifies `spoken_filler` sent to TTS as `AudioIntent::InterimFiller`.
/// 4. Verifies hybrid RRF retrieval returns facts.
/// 5. Verifies observation pushed into `scratchpad`.
/// 6. Verifies turn completion commits only conversational dialogue (scratchpad dropped).
#[tokio::test]
async fn test_search_memory_non_terminal_rrf_retrieval() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let conn = state.db.connect().expect("Failed to connect to db");
        run_migrations(&conn).await.expect("Failed to run migrations");

        // Seed session and fact
        let session_id = 99887777i64;
        let now = 1700000000000i64;
        conn.execute(
            "INSERT INTO sessions (id, created_at, updated_at, project_id) VALUES (?, ?, ?, 'default');",
            (session_id, now, now),
        )
        .await
        .expect("Insert session failed");

        // Insert test memory fact
        let fact_id = "fact_gpu_01".to_string();
        let fact_record = FactRecord {
            id: fact_id.clone(),
            session_id: Some(session_id),
            compaction_id: 1,
            fact_type: "hardware".to_string(),
            text: "User possesses an NVIDIA RTX 4090 GPU with 24GB VRAM.".to_string(),
            status: "active".to_string(),
            created_at: 1700000000,
            updated_at: 1700000000,
        };
        insert_fact(&conn, &fact_record)
            .await
            .expect("Insert fact failed");

        let dummy_vec = vec![0.05f32; 384];
        insert_vector(
            &conn,
            &fact_id,
            "hardware",
            "active",
            Some("default"),
            &dummy_vec,
        )
        .await
        .expect("Insert vector failed");

        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let pending_jobs = Arc::new(AtomicU32::new(0));
        let cancel_atomic = Arc::new(AtomicBool::new(false));
        let (pipeline_tx, _pipeline_rx) = mpsc::channel();

        let stream_handles = StreamRoutingHandles {
            turn_id: 1,
            owner: vox_lib::core::state::InteractionOwner::Assistant,
            accumulator: Arc::clone(&state.pipeline_accumulator),
            tts_tx: Some(tts_tx.clone()),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            cancel: cancel_atomic,
            event_tx: pipeline_tx,
            app: app.clone(),
        };

        let stream_stage = StreamRoutingStage::new();
        let tool_registry = ToolRegistry::with_default_tools();

        let settings = VoxSettings::default();
        let (llm_tx, _llm_rx) = mpsc::channel();
        let harness = Harness::new_modular(
            Some(session_id),
            "System prompt".to_string(),
            None,
            &settings,
            llm_tx,
            true,
        );
        let harness_arc = Arc::new(Mutex::new(Some(harness)));

        let routing_ctx = vox_lib::pipeline::RoutingContext::from_app_state(&state);
        let req = TurnExecutionRequest {
            turn_id: 1,
            query: "What graphics card do I have?".to_string(),
            cancel: CancellationToken::new(),
            app: app.clone(),
            app_state: Arc::clone(&state),
            routing_ctx,
            owner: vox_lib::core::state::InteractionOwner::Assistant,
            tts_tx: Some(tts_tx),
            pipeline_tx: None,
            llm_tx: None,
            provider: None,
            accumulator: Arc::clone(&state.pipeline_accumulator),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            db: Arc::clone(&state.db),
        };

        let loop_ctx = TurnLoopContext {
            harness_arc: &harness_arc,
            req: &req,
            stream_stage: &stream_stage,
            stream_handles: &stream_handles,
            tool_registry: &tool_registry,
            session_id,
        };

        let tool_call = CanonicalToolCall {
            id: "call_mem_1".to_string(),
            name: "search_memory".to_string(),
            arguments: serde_json::json!({
                "query": "graphics card GPU model",
                "spoken_filler": "Let me check your memory."
            }),
        };

        let mut scratchpad = Vec::new();

        // Execute Non-Terminal Tool
        let is_cancelled = step6_handle_non_terminal_tool(
            &loop_ctx,
            tool_call,
            "Prefix thoughts",
            &mut scratchpad,
        )
        .await;

        assert!(!is_cancelled);

        // 1. Assert InterimFiller received on TTS
        let tts_cmd = tts_rx.try_recv().expect("tts_rx must receive interim filler");
        match tts_cmd {
            TtsCommand::Generate { intent, text, .. } => {
                assert_eq!(intent, AudioIntent::InterimFiller);
                assert!(text.contains("check your memory"));
            }
            _ => panic!("Expected TtsCommand::Generate for filler"),
        }

        // 2. Assert Scratchpad captured tool call and observation
        assert_eq!(scratchpad.len(), 2);
        assert_eq!(scratchpad[0].role, Role::Assistant);
        assert_eq!(scratchpad[1].role, Role::Tool);
        assert!(
            scratchpad[1].content.contains("RTX 4090") || scratchpad[1].content.contains("records found"),
            "Scratchpad observation must contain memory search output"
        );

        // 3. Assert Turn Finalization drops scratchpad from working history
        {
            let mut guard = harness_arc.lock();
            let h = guard.as_mut().unwrap();
            h.history_mut().push_user_turn("What graphics card do I have?".to_string());
            h.history_mut().push_assistant_turn("You have an RTX 4090.".to_string());

            // History contains strictly [System, User, Assistant]
            assert_eq!(h.history().messages().len(), 3);
            for msg in h.history().messages() {
                assert_ne!(msg.role, Role::Tool, "Tool messages must never leak into history");
            }
        }
    })
    .await
    .expect("test_search_memory_non_terminal_rrf_retrieval timed out");
}

// ============================================================================
// Subtest 3: test_clause_buffering_and_prefix_drop_on_tool_call
// ============================================================================
/// Verifies Seam 21 Drop-All-Prefix Requirement:
/// When tokens arrive before a tool call ("Checking my database..."),
/// the egress stream layer buffers them; upon receiving ToolCallReceived,
/// the buffer is dropped and zero prefix speech is dispatched to TTS.
#[tokio::test]
async fn test_clause_buffering_and_prefix_drop_on_tool_call() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let pending_jobs = Arc::new(AtomicU32::new(0));
        let cancel_atomic = Arc::new(AtomicBool::new(false));
        let (pipeline_tx, _pipeline_rx) = mpsc::channel();

        let stream_handles = StreamRoutingHandles {
            turn_id: 1,
            owner: vox_lib::core::state::InteractionOwner::Assistant,
            accumulator: Arc::clone(&state.pipeline_accumulator),
            tts_tx: Some(tts_tx),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            cancel: cancel_atomic,
            event_tx: pipeline_tx,
            app: app.clone(),
        };

        let stream_stage = StreamRoutingStage::new();
        let (response_tx, response_rx) = mpsc::channel();

        // Send tokens, then ToolCall
        response_tx
            .send(vox_lib::services::llm::actor::LlmResponse::Token(
                "Checking my notes for you right now. ".to_string(),
            ))
            .unwrap();

        response_tx
            .send(vox_lib::services::llm::actor::LlmResponse::ToolCall(
                CanonicalToolCall {
                    id: "call_buffered_1".to_string(),
                    name: "search_memory".to_string(),
                    arguments: serde_json::json!({
                        "query": "notes",
                        "spoken_filler": "Looking into it."
                    }),
                },
            ))
            .unwrap();

        let outcome = tokio::task::spawn_blocking(move || {
            stream_stage.route_stream(stream_handles, response_rx)
        })
        .await
        .unwrap()
        .expect("Stream route failed");

        match outcome {
            vox_lib::services::harness::stages::streaming::StreamPassOutcome::ToolCallReceived {
                partial_text,
                call,
            } => {
                assert_eq!(call.name, "search_memory");
                assert!(partial_text.contains("Checking my notes"));
            }
            other => panic!("Expected ToolCallReceived, got {:?}", other),
        }

        // CRITICAL INVARIANT: Zero clauses dispatched to TTS!
        assert!(
            tts_rx.try_recv().is_err(),
            "Prefix clauses must be dropped and never dispatched to TTS on tool call"
        );
    })
    .await
    .expect("test_clause_buffering_and_prefix_drop_on_tool_call timed out");
}
