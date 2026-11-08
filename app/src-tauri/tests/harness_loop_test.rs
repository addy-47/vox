//! ============================================================================
//! harness_loop_test.rs — Harness turn loop seam
//! ============================================================================
//! Category  : Integration (Seam 12 — Harness turn loop → dispatch → commit)
//! Component : services/harness/loop.rs + services/harness/steps.rs
//!             + services/harness/chassis.rs
//! Binary    : harness_loop_test
//! Prereqs   : none. The LLM is a recording stub; no model is loaded.
//! Run       : cargo nextest run --test harness_loop_test --release --test-threads=1
//! ============================================================================
//!
//! WHAT THIS SEAM OWNS
//! How many times the loop may ask the model for another pass, what happens when
//! the turn is cancelled or duplicated, and what the loop commits when it ends.

mod common;

use std::{
    sync::{atomic::Ordering, mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use vox_lib::{
    core::{
        events::{AudioIntent, InteractionMode, PipelineMode},
        state::{AppState, InteractionOwner, InteractionState},
    },
    pipeline::RoutingContext,
    services::{
        harness::{Harness, LlmExecutionParams, TurnExecutionRequest, TurnOutcome},
        llm::{actor::LlmResponse, LlmCommand},
    },
};

type MockApp = tauri::AppHandle<tauri::test::MockRuntime>;

/// One dispatched generation request: (turn id, whether it offered tools).
type Recorded = Arc<Mutex<Vec<(u32, bool)>>>;

struct Rig {
    app: MockApp,
    state: Arc<AppState>,
    recorded: Recorded,
    llm_tx: mpsc::Sender<LlmCommand>,
    tts_tx: mpsc::Sender<vox_lib::services::tts::TtsCommand>,
    pipeline_tx: mpsc::Sender<vox_lib::core::events::VoxEvent>,
    llm_join: Option<JoinHandle<()>>,
    _paths: common::paths::TempPathsGuard,
}

impl Rig {
    async fn new() -> Self {
        let paths = common::paths::TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state().await;

        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::Passive;
            settings.interaction.pipeline_mode = PipelineMode::Modular;
            settings.memory.auto_compaction = false;
            settings.memory.context_retrieval_enabled = false;
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);

        let common::harness::PipelineTestChannels {
            stt_tx,
            vad_tx,
            tts_tx,
            tts_rx: _tts_rx,
            llm_tx,
            llm_rx,
            pipeline_tx,
            pipeline_rx: _pipeline_rx,
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
                Some(1),
                prompt,
                None,
                &settings,
                LlmExecutionParams {
                    context_window: 8192,
                    max_output_tokens: 512,
                    temperature: 0.7,
                    reasoning_enabled: false,
                },
                llm_tx.clone(),
                true,
            ));
        }

        let recorded: Recorded = Arc::new(Mutex::new(Vec::new()));
        let worker_recorded = recorded.clone();
        let llm_join = std::thread::Builder::new()
            .name("test-stub-llm".into())
            .spawn(move || {
                stub_llm_worker(llm_rx, worker_recorded);
            })
            .expect("spawn test-stub-llm");

        Self {
            app,
            state,
            recorded,
            llm_tx,
            tts_tx,
            pipeline_tx,
            llm_join: Some(llm_join),
            _paths: paths,
        }
    }

    fn requests(&self) -> Vec<(u32, bool)> {
        self.recorded.lock().expect("recording lock").clone()
    }

    fn dispatch_count(&self) -> usize {
        self.requests().len()
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
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.llm_tx.send(LlmCommand::Shutdown);
        if let Some(join) = self.llm_join.take() {
            join.join().expect("llm thread panicked");
        }
    }
}

/// Stub LLM worker: records every request and answers with a tool call whenever the
/// request offers tools, so the loop keeps asking for another pass.
fn stub_llm_worker(llm_rx: mpsc::Receiver<LlmCommand>, recorded: Recorded) {
    while let Ok(cmd) = llm_rx.recv() {
        match cmd {
            LlmCommand::Generate {
                request,
                turn_id,
                cancel,
                response_tx,
            } => {
                let offers_tools = request.tools.is_some_and(|t| !t.is_empty());
                recorded
                    .lock()
                    .expect("recording lock")
                    .push((turn_id, offers_tools));

                if cancel.is_cancelled() {
                    let _ = response_tx.send(LlmResponse::Cancelled);
                    continue;
                }
                if offers_tools {
                    let _ = response_tx.send(LlmResponse::ToolCall(
                        vox_lib::services::llm::CanonicalToolCall {
                            id: format!("call-{}", turn_id),
                            name: "search_memory".to_string(),
                            arguments: serde_json::json!({ "query": "anything" }),
                        },
                    ));
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

/// PURPOSE:  The loop must stop asking the model for another pass after the tool
///           budget is spent.
/// SCENARIO: The model always answers with a tool call, so every pass asks for another.
/// ASSERTS:  The loop dispatches the capped number of passes and then finishes with one
///           tool-free pass.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Raise `MAX_TOOL_ITERATIONS` or drop the tool-free final pass.
#[tokio::test]
async fn the_tool_loop_stops_at_the_tool_budget() {
    let rig = Rig::new().await;
    let request = rig.turn_request("keep asking", 1);

    let outcome = Harness::execute_turn(&rig.state.harness, request).await;

    assert!(
        matches!(outcome, TurnOutcome::Completed { .. }),
        "the loop must finish with a completed turn, got {:?}",
        outcome
    );
    let requests = rig.requests();
    assert_eq!(
        requests.len(),
        6,
        "five tool rounds plus one tool-free pass, got {} dispatches",
        requests.len()
    );
    assert!(
        requests[..5].iter().all(|(_, tools)| *tools),
        "the first five passes must offer the tools"
    );
    assert!(!requests[5].1, "the final pass must be tool-free");
}

/// PURPOSE:  A cancelled turn must never reach the model.
/// SCENARIO: The turn token is cancelled before the turn starts.
/// ASSERTS:  Nothing is dispatched and the outcome is Cancelled.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Ignore the cancelled token in the intake step.
#[tokio::test]
async fn a_cancelled_turn_never_dispatches() {
    let rig = Rig::new().await;
    rig.state.pipeline.turn_token().cancel();
    let request = rig.turn_request("never sent", 2);

    let outcome = Harness::execute_turn(&rig.state.harness, request).await;

    assert!(
        matches!(outcome, TurnOutcome::Cancelled { .. }),
        "a cancelled turn must not complete, got {:?}",
        outcome
    );
    assert_eq!(
        rig.dispatch_count(),
        0,
        "a cancelled turn must not dispatch"
    );
}

/// PURPOSE:  The same question twice must not start a second turn.
/// SCENARIO: The harness history holds the user turn, then the identical question is
///           asked again.
/// ASSERTS:  The turn is refused before the dispatch and nothing is sent to the model.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the duplicate check in the intake step.
///
/// NOTE: the check compares against the last message of the history. A finished turn
/// leaves the assistant message last, so the duplicate window is the time between the
/// user turn and the answer.
#[tokio::test]
async fn a_duplicate_query_is_refused_before_the_dispatch() {
    let rig = Rig::new().await;
    {
        let mut guard = rig.state.harness.lock();
        let harness = guard.as_mut().expect("Harness must be mounted");
        harness.push_user_turn("the same question".to_string());
    }

    let request = rig.turn_request("the same question", 4);
    let outcome = Harness::execute_turn(&rig.state.harness, request).await;

    assert!(
        matches!(outcome, TurnOutcome::DuplicateIgnored { .. }),
        "the duplicate must be refused, got {:?}",
        outcome
    );
    assert_eq!(rig.dispatch_count(), 0, "the duplicate must not dispatch");
}

/// PURPOSE:  A turn without a mounted harness must fail loudly.
/// ASSERTS:  The outcome is an error, not a silent success.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Return a completed turn when the harness is missing.
#[tokio::test]
async fn an_unmounted_harness_reports_an_error() {
    let rig = Rig::new().await;
    *rig.state.harness.lock() = None;
    let request = rig.turn_request("no harness", 5);

    let outcome = Harness::execute_turn(&rig.state.harness, request).await;

    assert!(
        matches!(outcome, TurnOutcome::Error { .. }),
        "a missing harness must be an error, got {:?}",
        outcome
    );
    assert_eq!(rig.dispatch_count(), 0);
}

/// PURPOSE:  A finished turn must commit both messages to the history.
/// ASSERTS:  The history holds the user query and the assistant response.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the history commit in the completion path.
#[tokio::test]
async fn a_finished_turn_commits_both_messages() {
    let rig = Rig::new().await;
    let request = rig.turn_request("what happened", 6);

    let outcome = Harness::execute_turn(&rig.state.harness, request).await;
    assert!(matches!(outcome, TurnOutcome::Completed { .. }));

    let guard = rig.state.harness.lock();
    let harness = guard.as_ref().expect("Harness must be mounted");
    let messages = harness.messages();
    assert!(
        messages.iter().any(|m| m.content == "what happened"),
        "the user query must be committed"
    );
    assert!(
        messages.iter().any(|m| m.content == "done"),
        "the assistant response must be committed"
    );
}

/// PURPOSE:  At critical token utilization the turn plays a compaction filler and
///           enters Working before compaction proceeds.
/// SCENARIO: The harness history is seeded past the critical threshold (> 85%) and the
///           user speaks again; the inline-compaction path triggers.
/// ASSERTS:  TtsCommand::Generate{InterimFiller} is dispatched with the turn id, the
///           pending synthesis jobs counter is incremented, and the pipeline is Working.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Drop the `enter_non_terminal_phase` call in `step3_execute_compaction`.
#[tokio::test]
async fn critical_threshold_plays_the_compaction_filler() {
    let paths = common::paths::TempPathsGuard::new();
    vox_lib::utils::paths::init();
    let (app, state) = common::harness::get_test_app_and_state().await;

    {
        let mut settings = state.settings.write().unwrap();
        settings.interaction.mode = InteractionMode::Passive;
        settings.interaction.pipeline_mode = PipelineMode::Modular;
        settings.memory.auto_compaction = true;
        settings.memory.context_retrieval_enabled = false;
    }
    state.owner.store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    state.pipeline.set_state(InteractionState::Thinking);
    state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);

    let common::harness::PipelineTestChannels {
        stt_tx,
        vad_tx,
        tts_tx,
        tts_rx,
        llm_tx,
        llm_rx: _llm_rx,
        pipeline_tx,
        pipeline_rx: _pipeline_rx,
    } = common::harness::setup_pipeline_channels();

    common::harness::attach_mock_engine_with_pipeline_tx_to_state(
        &app,
        &state,
        stt_tx,
        vad_tx,
        Some(llm_tx.clone()),
        Some(tts_tx.clone()),
        pipeline_tx,
    );

    {
        let settings = state.settings.read().unwrap().clone();
        let prompt = state.resolve_base_prompt();
        *state.harness.lock() = Some(Harness::new_modular(
            Some(1),
            prompt,
            None,
            &settings,
            LlmExecutionParams {
                context_window: 8192,
                max_output_tokens: 512,
                temperature: 0.7,
                reasoning_enabled: false,
            },
            llm_tx.clone(),
            true,
        ));
    }

    // Seed enough history to cross the critical threshold (>85% of ~7680 usable tokens).
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

    state.pipeline.set_state(InteractionState::Thinking);

    let req = TurnExecutionRequest {
        query: "Final query exceeding threshold".to_string(),
        turn_id: 46,
        cancel: state.pipeline.turn_token(),
        owner: InteractionOwner::Assistant,
        llm_tx: None,
        tts_tx: Some(tts_tx.clone()),
        provider: None,
        db: Arc::clone(&state.db),
        pipeline_tx: None,
        accumulator: Arc::clone(&state.pipeline_accumulator),
        pending_synthesis_jobs: Arc::clone(&state.pipeline.pending_synthesis_jobs),
        emitter: Arc::new(app.clone()),
        routing_ctx: vox_lib::pipeline::RoutingContext::from_app_state(&state),
        app_state: Arc::clone(&state),
    };

    let _outcome = Harness::execute_turn(&state.harness, req).await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut filler_received = None;
    while tokio::time::Instant::now() < deadline {
        if let Ok(cmd) = tts_rx.try_recv() {
            filler_received = Some(cmd);
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let filler_cmd =
        filler_received.expect("Expected filler TtsCommand::Generate on critical threshold");
    match filler_cmd {
        vox_lib::services::tts::TtsCommand::Generate {
            turn_id,
            text,
            intent,
        } => {
            assert_eq!(turn_id, 46);
            assert_eq!(
                intent,
                AudioIntent::InterimFiller,
                "filler must have InterimFiller intent"
            );
            assert!(
                vox_lib::services::harness::TRANSITION_MESSAGES_EN.contains(&text.as_str()),
                "filler text '{}' must be from TRANSITION_MESSAGES_EN",
                text
            );
        }
        other => panic!("Expected TtsCommand::Generate for filler, got {:?}", other),
    }

    assert_eq!(
        state.pipeline.state(),
        InteractionState::Working,
        "state must transition to Working when critical compaction filler is dispatched"
    );
    assert!(
        state.pipeline.pending_synthesis_jobs.load(Ordering::Relaxed) >= 1,
        "pending_synthesis_jobs must be incremented when filler is dispatched"
    );

    // Cancel the turn token to immediately abort the background compaction.
    state.pipeline.turn_token().cancel();
    drop(paths);
}
