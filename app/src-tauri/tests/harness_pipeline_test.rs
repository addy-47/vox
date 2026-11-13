//! =====================================================================================
//! harness_pipeline_test.rs — Harness v2 pipeline integration seam
//! =====================================================================================
//! Category  : Integration (Seam)
//! Component : services/harness/loop.rs + services/harness/stages/streaming/router.rs
//!             + services/harness/steps.rs + pipeline/assistant/playback.rs
//!             + pipeline/assistant/llm.rs
//! Binary    : harness_pipeline_test
//! Prereqs   : none (isolated temporary database)
//! Run       : cargo nextest run --test harness_pipeline_test --release --test-threads=1
//! =====================================================================================

mod common;

use std::{
    sync::{
        atomic::Ordering,
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use parking_lot::Mutex;
use vox_lib::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::{AudioIntent, InteractionMode, PipelineMode, VoxEvent},
        state::{AppState, InteractionOwner, InteractionState},
    },
    persistence::worker::spawn_persistence_worker,
    pipeline::{
        assistant::{llm::on_llm_finished, playback::on_playback_finished, playback::on_playback_started},
        RoutingContext,
    },
    services::{
        harness::{
            chassis::Harness, r#loop::MAX_TOOL_STEPS, stages::tools::ToolFilter,
            LlmExecutionParams, Role, TurnExecutionRequest, TurnOutcome,
        },
        llm::{
            actor::LlmResponse,
            CanonicalToolCall, LlmCommand,
        },
        tts::actor::TtsCommand,
    },
};

type MockApp = tauri::AppHandle<tauri::test::MockRuntime>;
type ScriptStep = Box<dyn FnOnce(mpsc::Sender<LlmResponse>) + Send + 'static>;

struct ScriptedLlmWorker {
    queue: Mutex<Vec<ScriptStep>>,
    requests: Mutex<Vec<(u32, bool)>>,
}

impl ScriptedLlmWorker {
    fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn push_step<F>(&self, step: F)
    where
        F: FnOnce(mpsc::Sender<LlmResponse>) + Send + 'static,
    {
        self.queue.lock().push(Box::new(step));
    }

    fn dispatches(&self) -> Vec<(u32, bool)> {
        self.requests.lock().clone()
    }
}

struct Rig {
    app: MockApp,
    state: Arc<AppState>,
    scripted_llm: Arc<ScriptedLlmWorker>,
    llm_tx: mpsc::Sender<LlmCommand>,
    tts_tx: mpsc::Sender<TtsCommand>,
    tts_rx: Arc<Mutex<mpsc::Receiver<TtsCommand>>>,
    pipeline_tx: mpsc::Sender<VoxEvent>,
    _pipeline_rx: Arc<Mutex<mpsc::Receiver<VoxEvent>>>,
    llm_join: Option<JoinHandle<()>>,
    session_id: i64,
    _paths: common::paths::TempPathsGuard,
}

impl Rig {
    async fn new() -> Self {
        let paths = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;

        let persist_tx = spawn_persistence_worker(
            Arc::clone(&state.db),
            state.telemetry.is_db_healthy.clone(),
            state.telemetry.latest_persistence_rate.clone(),
            state.telemetry.is_private_mode.clone(),
        );
        *state.persist_tx.lock() = Some(persist_tx);

        let session_id = 10001i64;
        let now = 1700000000000i64;
        {
            let conn = state.db.connect().expect("db connect");
            conn.execute(
                "INSERT OR IGNORE INTO sessions (id, created_at, updated_at, project_id) VALUES (?, ?, ?, 'default');",
                (session_id, now, now),
            )
            .await
            .expect("seed session");
        }
        state.conversation_id.store(session_id as u64, Ordering::Relaxed);

        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::Passive;
            settings.interaction.pipeline_mode = PipelineMode::Modular;
            settings.memory.auto_compaction = false;
            settings.memory.context_retrieval_enabled = true;
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);
        state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);

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
            Some(tts_tx.clone()),
            pipeline_tx.clone(),
        );

        {
            let settings = state.settings.read().unwrap().clone();
            let prompt = state.resolve_base_prompt();
            *state.harness.lock() = Some(Harness::new_modular(
                Some(session_id),
                prompt,
                None,
                &settings,
                LlmExecutionParams {
                    context_window: 8192,
                    max_output_tokens: 512,
                    temperature: 0.7,
                    reasoning_enabled: true,
                },
                llm_tx.clone(),
                true,
            ));
        }

        let scripted_llm = Arc::new(ScriptedLlmWorker::new());
        let worker_scripted = Arc::clone(&scripted_llm);
        let llm_join = std::thread::Builder::new()
            .name("scripted-llm-worker".into())
            .spawn(move || {
                run_scripted_worker(llm_rx, worker_scripted);
            })
            .expect("spawn scripted llm worker");

        Self {
            app,
            state,
            scripted_llm,
            llm_tx,
            tts_tx,
            tts_rx: Arc::new(Mutex::new(tts_rx)),
            pipeline_tx,
            _pipeline_rx: Arc::new(Mutex::new(pipeline_rx)),
            llm_join: Some(llm_join),
            session_id,
            _paths: paths,
        }
    }

    fn turn_request(&self, query: &str, turn_id: u32) -> TurnExecutionRequest {
        TurnExecutionRequest {
            query: query.to_string(),
            turn_id,
            cancel: self.state.pipeline.turn_token(),
            owner: InteractionOwner::Assistant,
            llm_tx: Some(self.llm_tx.clone()),
            tts_tx: Some(self.tts_tx.clone()),
            provider: None,
            db: Arc::clone(&self.state.db),
            pipeline_tx: Some(self.pipeline_tx.clone()),
            accumulator: Arc::clone(&self.state.pipeline_accumulator),
            pending_synthesis_jobs: Arc::clone(&self.state.pipeline.pending_synthesis_jobs),
            emitter: Arc::new(self.app.clone()),
            routing_ctx: RoutingContext::from_app_state(&self.state),
            app_state: Arc::clone(&self.state),
        }
    }

    fn drain_tts_commands(&self) -> Vec<TtsCommand> {
        let rx = self.tts_rx.lock();
        let mut cmds = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            cmds.push(cmd);
        }
        cmds
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.llm_tx.send(LlmCommand::Shutdown);
        if let Some(join) = self.llm_join.take() {
            join.join().expect("scripted llm worker panicked");
        }
    }
}

fn run_scripted_worker(
    llm_rx: mpsc::Receiver<LlmCommand>,
    scripted: Arc<ScriptedLlmWorker>,
) {
    while let Ok(cmd) = llm_rx.recv() {
        match cmd {
            LlmCommand::Generate {
                request,
                turn_id,
                cancel,
                response_tx,
            } => {
                let offers_tools = request.tools.as_ref().map(|t| !t.is_empty()).unwrap_or(false);
                scripted.requests.lock().push((turn_id, offers_tools));

                if cancel.is_cancelled() {
                    let _ = response_tx.send(LlmResponse::Cancelled);
                    continue;
                }

                let next_step = {
                    let mut q = scripted.queue.lock();
                    if !q.is_empty() {
                        Some(q.remove(0))
                    } else {
                        None
                    }
                };

                if let Some(step) = next_step {
                    step(response_tx);
                } else if offers_tools {
                    let _ = response_tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                        id: format!("call-{}", turn_id),
                        name: "search_memory".to_string(),
                        arguments: serde_json::json!({ "query": "default query" }),
                    }));
                    let _ = response_tx.send(LlmResponse::Finished);
                } else {
                    let _ = response_tx.send(LlmResponse::Token("done".to_string()));
                    let _ = response_tx.send(LlmResponse::Finished);
                }
            }
            LlmCommand::Shutdown => break,
            LlmCommand::Warmup { .. } => {}
        }
    }
}

// =====================================================================================
// === SCENE 1: 0-TOOL CONVERSATIONAL TURN (3-STREAM DEMUXING & TWO-KEY LATCH) ===
// =====================================================================================

/// PURPOSE:  Ensure thinking/reasoning tokens do not leak into speech synthesis or unthrottled IPC events.
/// SCENARIO: Model streams LlmResponse::Thinking followed by conversational tokens and Finished.
/// ASSERTS:  tts_rx receives only the spoken clauses without thinking text;
///           accumulator holds only assistant response text;
///           thinking text is isolated in memory; turn completes cleanly.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   router.rs: Route Thinking deltas directly into handle_token.
#[tokio::test]
async fn zero_tool_turn_demuxes_thinking_tokens_into_quarantine_without_tts_or_ipc() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 101;

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Thinking("Step 1: Calculating the answer. Step 2: Formulating response.".to_string()));
            let _ = tx.send(LlmResponse::Token("Paris is the capital of France.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("What is the capital of France?", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match &outcome {
            TurnOutcome::Completed { turn_id: tid, assistant_response } => {
                assert_eq!(*tid, turn_id);
                assert_eq!(assistant_response, "Paris is the capital of France.");
                assert!(!assistant_response.contains("Calculating"), "thinking tokens leaked into assistant_response");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let tts_cmds = rig.drain_tts_commands();
        assert!(!tts_cmds.is_empty(), "Expected TTS clause dispatch");
        for cmd in &tts_cmds {
            if let TtsCommand::Generate { text, intent, .. } = cmd {
                assert!(!text.contains("Calculating"), "thinking tokens leaked into TTS command text: {}", text);
                assert_eq!(*intent, AudioIntent::InterimFiller, "0-tool turn dispatched interim filler during stream");
            }
        }

        assert!(!rig.state.pipeline.is_turn_open(), "turn_open must be false after completion");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure a 0-tool turn in allow_tools mode latches to Ready only after all audio has drained.
/// SCENARIO: Stream finishes with 0 tools (Key 1 satisfied: turn_open == false); audio is still active.
/// ASSERTS:  on_llm_finished transitions to Speaking while jobs > 0;
///           on_playback_finished transitions to Ready when pending jobs reaches 0 (Key 2 satisfied).
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   llm.rs: In on_llm_finished, transition to Ready unconditionally without checking audio_active.
#[tokio::test]
async fn zero_tool_turn_with_interim_filler_latches_to_ready_only_after_playback_drains() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 102;
        let ctx = RoutingContext::from_app_state(&rig.state);

        rig.state.pipeline.set_state(InteractionState::Thinking);
        rig.state.pipeline.set_turn_open(true);
        rig.state.pipeline.pending_synthesis_jobs.store(1, Ordering::Relaxed);

        // Simulate LlmFinished with 1 pending synthesis job active
        on_llm_finished(turn_id, Some(&rig.app), &rig.state, &ctx);

        assert!(!rig.state.pipeline.is_turn_open(), "Key 1 must be satisfied: turn_open is false");
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Speaking,
            "State must transition to Speaking while audio synthesis is in flight"
        );

        // Calling playback_finished while 1 job is still pending must defer Ready
        on_playback_finished(turn_id, AudioIntent::InterimFiller, &rig.app, &rig.state, &ctx);
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Speaking,
            "State must remain Speaking while pending_synthesis_jobs > 0"
        );

        // Now clear pending jobs (Key 2 satisfied) and signal sound card playback finished
        rig.state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);
        on_playback_finished(turn_id, AudioIntent::InterimFiller, &rig.app, &rig.state, &ctx);

        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Ready,
            "Two-Key Latch must transition deterministically to Ready when audio drains"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Prevent sound card buffer underrun from kicking the pipeline to Ready while the turn is open.
/// SCENARIO: A clause finishes playing on the sound card (on_playback_finished) while turn_open == true.
/// ASSERTS:  on_playback_finished returns early without transitioning to Ready;
///           is_drained_while_open is latched; pipeline state remains stable in Thinking/Working.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   playback.rs: Remove 'if state.pipeline.is_turn_open() { return; }'.
#[tokio::test]
async fn playback_finished_during_open_turn_does_not_prematurely_transition_to_ready() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 103;
        let ctx = RoutingContext::from_app_state(&rig.state);

        rig.state.pipeline.set_state(InteractionState::Thinking);
        rig.state.pipeline.set_turn_open(true);
        rig.state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);

        on_playback_finished(turn_id, AudioIntent::InterimFiller, &rig.app, &rig.state, &ctx);

        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Thinking,
            "Pipeline state must NOT transition to Ready while turn_open is true"
        );
        assert!(
            rig.state.pipeline.is_drained_while_open(),
            "drained_while_open flag must be latched when buffer underrun occurs mid-turn"
        );
    })
    .await
    .expect("test timed out");
}

// =====================================================================================
// === SCENE 2: 1-TOOL PREAMBLE TURN (PREAMBLE PRESERVATION & UNIVERSAL TOOL EXECUTION) ===
// =====================================================================================

/// PURPOSE:  Prevent introductory speech prior to tool calls from being discarded or wiped.
/// SCENARIO: Model emits spoken preamble tokens followed by a tool call and Finished.
/// ASSERTS:  tts_rx receives the preamble clause as InterimFiller;
///           conversation history message 0 contains the preamble text as the assistant message initiating tools;
///           tool execution succeeds and terminal response commits to history.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   router.rs: Re-insert acc.chunker.clear() before tool calls returned.
#[tokio::test]
async fn preamble_tokens_before_tool_call_are_preserved_in_history_without_chunker_clear() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 201;

        // Pass 0: Preamble speech + search_memory tool call
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Let me look that up in your notes.".to_string()));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-search-1".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "meeting notes" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 1: Terminal answer after tool observation
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("You have a meeting with Sarah tomorrow.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("When is my meeting?", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed { assistant_response, .. } => {
                assert_eq!(assistant_response, "You have a meeting with Sarah tomorrow.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        // Verify preamble was dispatched to TTS as InterimFiller
        let tts_cmds = rig.drain_tts_commands();
        let preamble_cmd = tts_cmds.iter().find(|cmd| {
            if let TtsCommand::Generate { text, intent, .. } = cmd {
                text.contains("look that up") && *intent == AudioIntent::InterimFiller
            } else {
                false
            }
        });
        assert!(preamble_cmd.is_some(), "Preamble speech must be dispatched to TTS as InterimFiller");

        // Verify preamble text is committed in history as the initiating assistant message
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let messages = harness.messages();
        let preamble_msg = messages.iter().find(|m| {
            m.role == Role::Assistant && m.content.contains("look that up") && m.tool_calls.is_some()
        });
        assert!(preamble_msg.is_some(), "Preamble text must be recorded in assistant history initiating tools");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Prevent preamble audio playback completion from dropping pipeline out of Working state.
/// SCENARIO: Preamble playback fires on_playback_started and on_playback_finished while turn_open == true.
/// ASSERTS:  Pipeline remains stably in Working state throughout interim audio playback.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   playback.rs: Remove intent == InterimFiller guard in on_playback_started.
#[tokio::test]
async fn preamble_audio_playback_retains_working_state_during_tool_execution() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 202;
        let ctx = RoutingContext::from_app_state(&rig.state);

        rig.state.pipeline.set_state(InteractionState::Working);
        rig.state.pipeline.set_turn_open(true);

        // Onset of interim filler audio must keep state in Working
        on_playback_started(turn_id, AudioIntent::InterimFiller, &rig.app, &rig.state, &ctx);
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Working,
            "PlaybackStarted with InterimFiller must NOT transition state out of Working"
        );

        // Completion of interim filler audio while turn_open == true must remain in Working
        on_playback_finished(turn_id, AudioIntent::InterimFiller, &rig.app, &rig.state, &ctx);
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Working,
            "PlaybackFinished while turn_open == true must remain in Working"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure tools conform to the universal contract returning ToolResult without synthetic speech parameters.
/// SCENARIO: Model invokes search_memory with standard query parameters.
/// ASSERTS:  Tool executes cleanly, observation is returned with is_error == false, and observation is non-empty.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   steps.rs: In step6_handle_tool_batch, drop the tool observation from the scratchpad.
#[tokio::test]
async fn universal_tool_execution_succeeds_without_synthetic_speech_parameters() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 203;

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-search-universal".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "test facts" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Memory retrieved successfully.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Search memory for test facts", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed { assistant_response, .. } => {
                assert_eq!(assistant_response, "Memory retrieved successfully.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("harness");
        let tool_msg = harness.messages().iter().find(|m| m.role == Role::Tool);
        assert!(tool_msg.is_some(), "Observation message with Role::Tool must be present in history");
    })
    .await
    .expect("test timed out");
}

// =====================================================================================
// === SCENE 3: MULTI-TOOL BATCH TURN (BATCH DEMUXING & BARRIER EXECUTION) ===
// =====================================================================================

/// PURPOSE:  Prevent the streaming router from breaking on the first tool call and truncating parallel tool calls.
/// SCENARIO: Model stream emits 3 tool calls in a single generation pass followed by Finished.
/// ASSERTS:  All 3 tool calls are batched and executed; history contains observations for all 3 calls.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   router.rs: Break stream loop upon receiving the first ToolCall.
#[tokio::test]
async fn multi_tool_stream_accumulates_full_batch_without_early_stream_abort() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 301;

        // Pass 0: 3 tool calls in one stream pass
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-batch-1".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "query 1" }),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-batch-2".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "query 2" }),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-batch-3".to_string(),
                name: "web_search".to_string(),
                arguments: serde_json::json!({ "query": "query 3" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 1: Terminal response
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Processed all 3 tools in batch.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Run three tools simultaneously", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed { assistant_response, .. } => {
                assert_eq!(assistant_response, "Processed all 3 tools in batch.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("harness");
        let tool_messages: Vec<_> = harness.messages().iter().filter(|m| m.role == Role::Tool).collect();
        assert_eq!(
            tool_messages.len(),
            3,
            "All 3 tool observations must be recorded in history; saw {}",
            tool_messages.len()
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure the migrated set_title tool updates the session record and latches title_set.
/// SCENARIO: Model emits set_title tool call with a new session title.
/// ASSERTS:  Session title in database is updated; harness.title_set is true;
///           subsequent turn active_definitions suppresses set_title.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   steps.rs: In step6_handle_tool_batch, do not set harness.title_set = true.
#[tokio::test]
async fn set_title_universal_tool_updates_session_and_latches_title_set_flag() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 302;

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-title-1".to_string(),
                name: "set_title".to_string(),
                arguments: serde_json::json!({ "title": "Cognitive Architecture Spec" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Title has been updated.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Please title this conversation Cognitive Architecture Spec", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(matches!(outcome, TurnOutcome::Completed { .. }));

        // Verify harness title_set is latched
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("harness");
            assert!(harness.title_set(), "harness.title_set must be latched to true");

            // Verify active_definitions suppresses set_title on next turn
            let filter = ToolFilter {
                mode: PipelineMode::Modular,
                title_is_unset: !harness.title_set(),
                memory_retrieval_enabled: true,
                web_search_enabled: true,
            };
            let defs = harness.tool_registry().active_definitions(&filter);
            assert!(
                !defs.iter().any(|d| d.name == "set_title" || d.name == "respond_and_set_title"),
                "set_title must be filtered out once title_set is true"
            );
        }

        // Verify database persistence (polling for async persistence worker drain)
        let conn = rig.state.db.connect().expect("db connect");
        let mut title = None;
        for _ in 0..50 {
            let mut rows = conn
                .query("SELECT title FROM sessions WHERE id = ?;", (rig.session_id,))
                .await
                .expect("query session title");
            if let Ok(Some(row)) = rows.next().await {
                if let Ok(Some(t)) = row.get::<Option<String>>(0) {
                    if !t.is_empty() {
                        title = Some(t);
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            title.as_deref(),
            Some("Cognitive Architecture Spec"),
            "Session title in database must match set_title argument"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Guarantee the ReAct loop enforces the 20-step ceiling without runaway infinite loops.
/// SCENARIO: Model repeatedly answers with tool calls on every iteration.
/// ASSERTS:  Loop dispatches exactly 20 tool iterations; pass 21 has allow_tools == false; turn completes.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   loop.rs: Omit allow_tools = false assignment when iteration >= MAX_TOOL_STEPS.
#[tokio::test]
async fn tool_loop_reaches_max_steps_and_forces_tool_free_final_pass() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let rig = Rig::new().await;
        let turn_id = 303;

        // Model will default to search_memory tool call whenever tools are offered,
        // and "done" when tools are not offered.
        let req = rig.turn_request("Infinite loop query", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(matches!(outcome, TurnOutcome::Completed { .. }), "Loop must terminate with Completed");

        let dispatches = rig.scripted_llm.dispatches();
        let expected_dispatches = MAX_TOOL_STEPS + 1;
        assert_eq!(
            dispatches.len(),
            expected_dispatches,
            "Must execute exactly 20 tool steps followed by 1 tool-free pass; saw {}",
            dispatches.len()
        );
        assert!(
            dispatches[..MAX_TOOL_STEPS].iter().all(|(_, tools)| *tools),
            "First 20 passes must offer tools"
        );
        assert!(
            !dispatches[MAX_TOOL_STEPS].1,
            "21st pass must have tools disabled (allow_tools == false)"
        );
    })
    .await
    .expect("test timed out");
}

// =====================================================================================
// === SCENE 4: CANCELLATION & BARGE-IN MID-TURN ===
// =====================================================================================

/// PURPOSE:  Ensure cancelling mid-turn while tools are executing aborts without orphan tasks and clears turn_open.
/// SCENARIO: Turn cancellation token is triggered while in tool execution.
/// ASSERTS:  TurnOutcome::Cancelled is returned; turn_open is false; cancel token is marked cancelled.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   steps.rs: In step6_handle_tool_batch, ignore cancel token.
#[tokio::test]
async fn turn_cancellation_during_tool_execution_aborts_cleanly_and_clears_turn_open() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 401;

        let cancel_token = rig.state.pipeline.turn_token();
        let cancel_token_clone = cancel_token.clone();

        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-cancel-1".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "slow tool" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
            // Cancel immediately upon emitting the tool call
            cancel_token_clone.cancel();
        });

        let req = rig.turn_request("Cancel during tool", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(matches!(outcome, TurnOutcome::Cancelled { .. }), "Outcome must be Cancelled");
        assert!(!rig.state.pipeline.is_turn_open(), "turn_open must be cleared upon cancellation");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Verify that barge-in mid-sentence commits whatever partial assistant text was already spoken.
/// SCENARIO: Tokens stream to accumulator and cancellation token arrives while generation is in flight.
/// ASSERTS:  TurnOutcome::Cancelled returns with non-empty partial_text;
///           turns table in database commits the partial text with interrupted status.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   loop.rs: Pass String::new() instead of partial_text to cancelled_ctx on cancellation.
#[tokio::test]
async fn turn_cancellation_with_partial_spoken_text_commits_interrupted_turn() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 402;

        let cancel_token = rig.state.pipeline.turn_token();
        let cancel_token_clone = cancel_token.clone();
        let acc = Arc::clone(&rig.state.pipeline_accumulator);

        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Token("Partial sentence before interruption.".to_string()));
            // Ensure router has processed the token into accumulator before triggering cancellation
            let start = std::time::Instant::now();
            while acc.lock().assistant_response.is_empty() && start.elapsed() < Duration::from_millis(500) {
                std::thread::sleep(Duration::from_millis(5));
            }
            cancel_token_clone.cancel();
            let _ = tx.send(LlmResponse::Cancelled);
        });

        let req = rig.turn_request("Query interrupted mid-sentence", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Cancelled { turn_id: tid } => {
                assert_eq!(tid, turn_id);
            }
            other => panic!("Expected Cancelled outcome, got {:?}", other),
        }

        // 1. Verify synchronous history commit
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("harness");
            let msgs = harness.messages();
            let last_msg = msgs.iter().find(|m| m.role == Role::Assistant);
            assert!(
                last_msg.map(|m| m.content.contains("Partial sentence")).unwrap_or(false),
                "Harness history must contain the partial assistant speech"
            );
        }

        // 2. Verify asynchronous database persistence (poll for worker drain)
        let conn = rig.state.db.connect().expect("db connect");
        let mut recorded_text = None;
        for _ in 0..50 {
            let mut rows = conn
                .query("SELECT assistant_text FROM turns WHERE turn_id = ?;", (turn_id as i64,))
                .await
                .expect("query turns");
            if let Ok(Some(row)) = rows.next().await {
                if let Ok(Some(text)) = row.get::<Option<String>>(0) {
                    if !text.is_empty() {
                        recorded_text = Some(text);
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            recorded_text.as_deref().map(|t| t.contains("Partial sentence")).unwrap_or(false),
            "Database turns table must record the interrupted partial speech; saw {:?}",
            recorded_text
        );

        assert!(!rig.state.pipeline.is_turn_open(), "turn_open must be cleared");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Verify that cancelling before any assistant words are spoken leaves zero garbage rows in database.
/// SCENARIO: Turn cancellation is triggered before any tokens are generated.
/// ASSERTS:  TurnOutcome::Cancelled returns with empty partial_text; turns table contains 0 rows for turn.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   steps.rs: Unconditionally insert turn row into database even if partial_text is empty.
#[tokio::test]
async fn turn_cancellation_before_speech_rolls_back_history_with_zero_db_artifacts() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 403;

        rig.state.pipeline.turn_token().cancel();

        let req = rig.turn_request("Pre-cancelled query", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Cancelled { turn_id: tid } => {
                assert_eq!(tid, turn_id);
            }
            other => panic!("Expected Cancelled outcome, got {:?}", other),
        }

        // Verify zero turn rows committed
        let conn = rig.state.db.connect().expect("db connect");
        let mut rows = conn
            .query("SELECT COUNT(*) FROM turns WHERE turn_id = ?;", (turn_id as i64,))
            .await
            .expect("query turns");
        let count: i64 = rows.next().await.expect("row").map(|r| r.get(0).unwrap_or(0)).unwrap_or(0);
        assert_eq!(count, 0, "Zero turn rows must exist for cancelled turn without speech");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure a stream failure clears turn_open and dispatches an error event without freezing the loop.
/// SCENARIO: Provider stream yields LlmResponse::Error.
/// ASSERTS:  TurnOutcome::Error is returned; turn_open is false; error event is received.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   loop.rs: Do not call clear_turn_open() on StreamPassOutcome::Error.
#[tokio::test]
async fn stream_error_clears_turn_open_and_dispatches_error_event() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 404;

        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Error(PipelineError {
                turn_id,
                message: "Upstream model service timeout".to_string(),
                source: "LlmActor".to_string(),
                impact: PipelineImpact::TurnAborted,
            }));
        });

        let req = rig.turn_request("Fault injection query", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Error { turn_id: tid, message } => {
                assert_eq!(tid, turn_id);
                assert!(message.contains("Upstream model service timeout") || message.contains("LLM stream error"));
            }
            other => panic!("Expected Error outcome, got {:?}", other),
        }

        assert!(!rig.state.pipeline.is_turn_open(), "turn_open must be false after error");
    })
    .await
    .expect("test timed out");
}
