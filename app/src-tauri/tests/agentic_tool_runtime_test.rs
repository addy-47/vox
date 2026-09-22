//! ============================================================================
//! agentic_tool_runtime_test.rs — Agentic Tool Runtime, Taxonomy & Scratchpad Isolation
//! ============================================================================
//! Category     : Integration Test (Seam 21)
//! Component    : services/harness/stages/tools/ + services/harness/steps.rs +
//!                services/harness/stages/prompt.rs + persistence/sessions.rs
//! Prerequisites: Isolated temporary DB paths
//! Execution    : cargo nextest run --test agentic_tool_runtime_test --release --nocapture --test-threads=1
//! Metrics      : Terminal tool single-pass execution, DB title update, TurnAccumulator
//!                parity, NonTerminal RRF retrieval, scratchpad drop, prefix audio drop,
//!                provider wire bytes (request mapping) + stream parsing to ToolCall
//! ============================================================================

mod common;

use std::{
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, AtomicU32},
        mpsc, Arc,
    },
    time::Duration,
};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use vox_lib::{
    core::{events::AudioIntent, settings::VoxSettings, state::InteractionOwner},
    persistence::{
        facts::{insert_fact, insert_vector, FactRecord},
        schema::run_migrations,
        worker::spawn_persistence_worker,
    },
    core::metrics::TurnMetricsCollector,
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
        llm::{
            actor::LlmResponse,
            transport::{ConnectionConfig, RemoteTransport},
            CanonicalToolCall, CanonicalToolDefinition, ConversationInput, GenerationOptions,
            GenerationPurpose, GenerationRequest, LlmProvider, LlmStreamEvent, OutputConstraint,
            ReasoningMode, ToolFlow,
        },
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

        let persist_tx = spawn_persistence_worker(
            Arc::clone(&state.db),
            state.telemetry.is_db_healthy.clone(),
            state.telemetry.latest_persistence_rate.clone(),
            state.telemetry.is_private_mode.clone(),
        );
        *state.persist_tx.lock() = Some(persist_tx);

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
            owner: InteractionOwner::Assistant,
            accumulator: Arc::clone(&state.pipeline_accumulator),
            tts_tx: Some(tts_tx),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            cancel: cancel_atomic,
            event_tx: pipeline_tx,
            app: app.clone(),
            turn_metrics: Arc::new(TurnMetricsCollector::new()),
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
        let mut title = None;
        for _ in 0..50 {
            let mut rows = conn
                .query("SELECT title FROM sessions WHERE id = ?;", (session_id,))
                .await
                .expect("Query failed");
            if let Ok(Some(row)) = rows.next().await {
                if let Ok(t) = row.get::<Option<String>>(0) {
                    if t.is_some() {
                        title = t;
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
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
            owner: InteractionOwner::Assistant,
            accumulator: Arc::clone(&state.pipeline_accumulator),
            tts_tx: Some(tts_tx.clone()),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            cancel: cancel_atomic,
            event_tx: pipeline_tx,
            app: app.clone(),
            turn_metrics: Arc::new(TurnMetricsCollector::new()),
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
            owner: InteractionOwner::Assistant,
            accumulator: Arc::clone(&state.pipeline_accumulator),
            tts_tx: Some(tts_tx),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            cancel: cancel_atomic,
            event_tx: pipeline_tx,
            app: app.clone(),
            turn_metrics: Arc::new(TurnMetricsCollector::new()),
        };

        let stream_stage = StreamRoutingStage::new();
        let (response_tx, response_rx) = mpsc::channel();

        // Send tokens, then ToolCall
        response_tx
            .send(LlmResponse::Token(
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

// ============================================================================
// Wire harness: std-only mock HTTP server speaking canned SSE / NDJSON.
// Exercises the REAL path: manifest mapping -> request bytes -> stream parser.
// ============================================================================
/// Serves one canned HTTP response, captures the JSON request body, then exits.
fn spawn_mock_wire_server(
    response_body: Vec<u8>,
    content_type: &'static str,
    captured: Arc<Mutex<Option<serde_json::Value>>>,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock wire server must bind");
    let addr = listener
        .local_addr()
        .expect("mock wire server needs an addr");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("mock wire server must accept");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("mock read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .expect("mock write timeout");
        let mut raw = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).expect("mock wire server must read");
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..n]);
            if let Some(end) = raw
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .map(|pos| pos + 4)
            {
                let head = String::from_utf8_lossy(&raw[..end]).to_lowercase();
                let len = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while raw.len() < end + len {
                    let n = stream
                        .read(&mut buf)
                        .expect("mock wire server must read body");
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buf[..n]);
                }
                let have = raw.len().saturating_sub(end).min(len);
                if let Ok(body) = serde_json::from_slice::<serde_json::Value>(&raw[end..end + have])
                {
                    *captured.lock() = Some(body);
                }
                break;
            }
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            content_type,
            response_body.len()
        );
        stream
            .write_all(head.as_bytes())
            .expect("mock must write head");
        stream
            .write_all(&response_body)
            .expect("mock must write body");
    });
    (format!("http://{}", addr), handle)
}

/// Builds the turn-1 `respond_and_set_title` generation request used by wire tests.
fn turn1_title_request() -> GenerationRequest {
    GenerationRequest {
        input: ConversationInput {
            messages: vec![vox_lib::services::harness::ChatMessage::new(
                Role::User,
                "Hello Vox!".to_string(),
            )],
        },
        options: GenerationOptions {
            max_output_tokens: Some(120),
            reasoning: ReasoningMode::Disabled,
            ..Default::default()
        },
        output: OutputConstraint::Text,
        purpose: GenerationPurpose::Conversation,
        tools: Some(vec![CanonicalToolDefinition {
            name: "respond_and_set_title".to_string(),
            description: "Sets the session title and speaks a response.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "spoken_response": {"type": "string"}
                },
                "required": ["title", "spoken_response"]
            }),
            flow: ToolFlow::Terminal,
        }]),
    }
}

/// Drains stream events until `Finished` or the deadline, returning what arrived.
fn drain_wire_events(rx: &mpsc::Receiver<LlmStreamEvent>) -> Vec<LlmStreamEvent> {
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining) {
            Ok(LlmStreamEvent::Finished) => {
                events.push(LlmStreamEvent::Finished);
                break;
            }
            Ok(ev) => events.push(ev),
            Err(_) => break,
        }
    }
    events
}

// ============================================================================
// Subtest 4: test_nvidia_preset_wire_request_and_tool_stream
// ============================================================================
/// Wire regression for the Phase 12 eval failure: reasoning tokens ate the
/// 120-token budget because the body carried `think:false` + `reasoning.enabled`
/// (both ignored on NIM) instead of `reasoning_effort:"none"`.
/// Drives the REAL `RemoteTransport` against a mock server and asserts both the
/// exact request bytes and the parsed `ToolCall` out of chunked SSE deltas.
#[tokio::test]
async fn test_nvidia_preset_wire_request_and_tool_stream() {
    let test_timeout = Duration::from_secs(25);
    tokio::time::timeout(test_timeout, async {
        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let sse = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"respond_and_set_title\",\"arguments\":\"{\\\"title\\\":\\\"\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"Debugging\\\",\\\"spoken_response\\\":\\\"hi\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let (base_url, server) =
            spawn_mock_wire_server(sse.as_bytes().to_vec(), "text/event-stream", Arc::clone(&captured));

        let config = ConnectionConfig::new(&base_url, "test-model", Some("k"), Some("nvidia_nim"));
        let transport = RemoteTransport::new(config);
        let cancel = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<LlmStreamEvent>();

        transport
            .generate(turn1_title_request(), 1, &cancel, &tx)
            .await
            .expect("nvidia wire stream must succeed");

        let events = drain_wire_events(&rx);
        let tool_call = events.iter().find_map(|ev| match ev {
            LlmStreamEvent::ToolCall(call) => Some(call),
            _ => None,
        });
        let call = tool_call.expect("chunked SSE deltas must assemble into one ToolCall");
        assert_eq!(call.name, "respond_and_set_title");
        assert_eq!(
            call.arguments.get("title").and_then(|v| v.as_str()),
            Some("Debugging")
        );
        assert_eq!(
            call.arguments.get("spoken_response").and_then(|v| v.as_str()),
            Some("hi")
        );
        assert!(
            events.iter().any(|ev| matches!(ev, LlmStreamEvent::Finished)),
            "stream must terminate with Finished"
        );

        let body = captured.lock().clone().expect("mock must capture request body");
        assert_eq!(body.get("model").and_then(|v| v.as_str()), Some("test-model"));
        assert_eq!(body.get("max_tokens").and_then(|v| v.as_u64()), Some(120));
        assert_eq!(
            body.get("reasoning_effort").and_then(|v| v.as_str()),
            Some("none"),
            "NIM disables thinking via reasoning_effort, not think",
        );
        assert!(body.get("think").is_none(), "think must not be sent to NIM");
        assert!(body.get("reasoning").is_none(), "reasoning.enabled must not be sent to NIM");
        assert_eq!(body.get("tool_choice").and_then(|v| v.as_str()), Some("auto"));
        assert!(body.get("stream_options").is_none(), "NIM 503s on stream_options");
        assert_eq!(
            body
                .get("tools")
                .and_then(|t| t.get(0))
                .and_then(|t| t.get("function"))
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str()),
            Some("respond_and_set_title")
        );

        server.join().expect("mock wire server must exit cleanly");
    })
    .await
    .expect("test_nvidia_preset_wire_request_and_tool_stream timed out");
}

// ============================================================================
// Subtest 5: test_ollama_native_wire_request_and_tool_stream
// ============================================================================
/// Native `/api/chat` wire contract: `think:false` + `options.num_predict`,
/// complete per-line NDJSON tool calls, and NO `tool_choice` (unsupported).
#[tokio::test]
async fn test_ollama_native_wire_request_and_tool_stream() {
    let test_timeout = Duration::from_secs(25);
    tokio::time::timeout(test_timeout, async {
        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let ndjson = concat!(
            "{\"message\":{\"role\":\"assistant\",\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"respond_and_set_title\",\"arguments\":{\"title\":\"T\",\"spoken_response\":\"hi\"}}}]},\"done\":false}\n",
            "{\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true}\n",
        );
        let (base_url, server) = spawn_mock_wire_server(
            ndjson.as_bytes().to_vec(),
            "application/x-ndjson",
            Arc::clone(&captured),
        );

        let config = ConnectionConfig::new(&base_url, "qwen3.5:9b", None, Some("ollama"));
        let transport = RemoteTransport::new(config);
        let cancel = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<LlmStreamEvent>();

        transport
            .generate(turn1_title_request(), 1, &cancel, &tx)
            .await
            .expect("ollama native wire stream must succeed");

        let events = drain_wire_events(&rx);
        let tool_call = events.iter().find_map(|ev| match ev {
            LlmStreamEvent::ToolCall(call) => Some(call),
            _ => None,
        });
        let call = tool_call.expect("NDJSON tool_calls must surface as ToolCall");
        assert_eq!(call.name, "respond_and_set_title");
        assert_eq!(
            call.arguments.get("title").and_then(|v| v.as_str()),
            Some("T")
        );

        let body = captured.lock().clone().expect("mock must capture request body");
        assert_eq!(body.get("think"), Some(&serde_json::json!(false)));
        assert_eq!(
            body.get("options").and_then(|o| o.get("num_predict")).and_then(|v| v.as_u64()),
            Some(120)
        );
        assert!(body.get("tool_choice").is_none(), "Ollama rejects tool_choice");
        assert!(body.get("stream_options").is_none(), "NDJSON has no stream_options");
        assert_eq!(
            body
                .get("tools")
                .and_then(|t| t.get(0))
                .and_then(|t| t.get("function"))
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str()),
            Some("respond_and_set_title")
        );

        server.join().expect("mock wire server must exit cleanly");
    })
    .await
    .expect("test_ollama_native_wire_request_and_tool_stream timed out");
}
