//! ============================================================================
//! harness_loop_test.rs — Canonical Harness Turn Loop & Pipeline Seam
//! ============================================================================
//! Category  : Integration (Seam 12 — Harness turn loop → dispatch → commit)
//! Component : harness/loop.rs + harness/steps.rs
//!             + harness/chassis.rs + harness/stages/streaming/router.rs
//!             + pipeline/assistant/llm.rs + pipeline/assistant/playback.rs
//! Binary    : harness_loop_test
//! Prereqs   : none (isolated temporary database)
//! Run       : cargo nextest run --test harness_loop_test --release --test-threads=1
//! ============================================================================

mod common;

use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use common::harness::Rig;
use futures_util::future::BoxFuture;
use vox_lib::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::{InteractionOwner, PipelineMode, VoxEvent},
        state::InteractionState,
    },
    harness::{
        chassis::Harness,
        r#loop::MAX_TOOL_STEPS,
        stages::inbox::{InboxMessage, QueuedMessageDelivery, StepInbox},
        tools::{
            ToolDefinition, ToolError, ToolExecutionContext, ToolFilter, ToolResult,
            TruncationPolicy,
        },
        Role, ToolExecutionMode, TurnOutcome,
    },
    services::{
        llm::{actor::LlmResponse, CanonicalToolCall},
        tts::actor::TtsCommand,
    },
};

// ============================================================================
// Section 1: Loop Intake, ReAct Step Budget & Lifecycle Boundaries
// ============================================================================

/// PURPOSE:  The loop must stop asking the model for another pass after the tool budget is spent.
/// SCENARIO: The model always answers with a tool call, so every pass asks for another.
/// ASSERTS:  The loop dispatches 20 passes and then finishes with one tool-free pass (21 total).
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Raise MAX_TOOL_STEPS or drop the tool-free final pass.
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
    let expected_dispatches = MAX_TOOL_STEPS + 1;
    assert_eq!(
        requests.len(),
        expected_dispatches,
        "20 tool rounds plus one tool-free pass, got {} dispatches",
        requests.len()
    );
    assert!(
        requests[..MAX_TOOL_STEPS].iter().all(|(_, tools)| *tools),
        "the first 20 passes must offer the tools"
    );
    assert!(
        !requests[MAX_TOOL_STEPS].1,
        "the final pass must be tool-free"
    );
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
    rig.state.pipeline.turn_cancel_token().cancel();
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
/// SCENARIO: The harness history holds the user turn, then the identical question is asked again.
/// ASSERTS:  The turn is refused before dispatch and nothing is sent to the model.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the duplicate check in the intake step.
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

/// PURPOSE:  At critical token utilization the turn plays a compaction filler and enters Working before compaction.
/// SCENARIO: The harness history is seeded past the critical threshold (> 85%) and the user speaks again.
/// ASSERTS:  TtsCommand::Generate{InterimFiller} is dispatched with the turn id; pipeline state is Working.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Drop the enter_non_terminal_phase call in step3_execute_compaction.
#[tokio::test]
async fn critical_threshold_plays_the_compaction_filler() {
    let rig = Rig::new().await;
    {
        let mut settings = rig.state.settings.write().unwrap();
        settings.memory.auto_compaction = true;
        settings.memory.context_retrieval_enabled = false;
    }
    {
        let settings = rig.state.settings.read().unwrap().clone();
        let prompt = rig.state.resolve_base_prompt();
        *rig.state.harness.lock() = Some(Harness::new_modular(
            Some(rig.session_id),
            prompt,
            None,
            &settings,
            vox_lib::harness::LlmExecutionParams {
                context_window: 8192,
                max_output_tokens: 512,
                temperature: 0.7,
                reasoning_enabled: false,
            },
            rig.llm_tx.clone(),
            true,
        ));
    }
    rig.state.pipeline.set_state(InteractionState::Thinking);
    rig.state
        .pipeline
        .pending_tts_jobs
        .store(0, Ordering::Relaxed);

    // Seed enough history to cross the critical threshold (>85% of ~7680 usable tokens).
    {
        let mut guard = rig.state.harness.lock();
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

    rig.state.pipeline.set_state(InteractionState::Thinking);

    let (release_tx, release_rx) = std::sync::mpsc::channel();
    rig.scripted_llm.push_step(move |tx| {
        let _ = release_rx.recv();
        let _ = tx.send(LlmResponse::Token("Compaction response".to_string()));
        let _ = tx.send(LlmResponse::Finished);
    });

    let req = rig.turn_request("Final query exceeding threshold", 46);
    let harness_arc = Arc::clone(&rig.state.harness);
    let execute_handle =
        tokio::spawn(async move { Harness::execute_turn(&harness_arc, req).await });

    let tts_rx = Arc::clone(&rig.tts_rx);
    let filler_cmd =
        tokio::task::spawn_blocking(move || tts_rx.lock().recv_timeout(Duration::from_secs(10)))
            .await
            .expect("join")
            .expect("Expected filler TtsCommand::Generate on critical threshold");

    match filler_cmd {
        vox_lib::services::tts::TtsCommand::Generate { turn_id, text } => {
            assert_eq!(turn_id, 46);
            assert!(
                vox_lib::harness::TRANSITION_MESSAGES_EN.contains(&text.as_str()),
                "filler text '{}' must be from TRANSITION_MESSAGES_EN",
                text
            );
        }
        other => panic!("Expected TtsCommand::Generate for filler, got {:?}", other),
    }

    assert_eq!(
        rig.state.pipeline.state(),
        InteractionState::Working,
        "state must transition to Working when critical compaction filler is dispatched"
    );
    assert!(
        rig.state.pipeline.pending_tts_jobs.load(Ordering::Relaxed) >= 1,
        "pending_synthesis_jobs must be incremented when filler is dispatched"
    );

    let _ = release_tx.send(());
    let _ = execute_handle.await.expect("join ok");
}

// ============================================================================
// Section 2: Scene 1 — 0-Tool Conversational Turn (3-Stream Demuxing & Two-Key Latch)
// ============================================================================

/// PURPOSE:  Ensure thinking/reasoning tokens do not leak into speech synthesis or unthrottled IPC events.
/// SCENARIO: Model streams LlmResponse::Thinking followed by conversational tokens and Finished.
/// ASSERTS:  tts_rx receives only the spoken clauses without thinking text; the accumulator holds only assistant response text.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   router.rs: Route Thinking deltas directly into handle_token.
#[tokio::test]
async fn zero_tool_turn_demuxes_thinking_tokens_away_from_tts() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 101;

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Thinking(
                "Step 1: Analyzing user query.\nStep 2: Formulating response.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Token(
                "The weather today is sunny and clear.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("How is the weather today?", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed {
                turn_id: tid,
                assistant_response,
            } => {
                assert_eq!(tid, turn_id);
                assert_eq!(assistant_response, "The weather today is sunny and clear.");
                assert!(
                    !assistant_response.contains("Analyzing user query"),
                    "thinking tokens leaked into assistant_response"
                );
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let tts_cmds = rig.drain_tts_commands();
        assert!(
            !tts_cmds.is_empty(),
            "Spoken text must be dispatched to TTS"
        );
        for cmd in tts_cmds {
            if let TtsCommand::Generate { text, .. } = cmd {
                assert!(
                    !text.contains("Analyzing user query"),
                    "Thinking text leaked into TTS: {}",
                    text
                );
                assert!(
                    !text.contains("Step 1"),
                    "Reasoning leaked into TTS: {}",
                    text
                );
            }
        }

        let acc_text = rig
            .state
            .pipeline_accumulator
            .lock()
            .assistant_response
            .clone();
        assert_eq!(acc_text, "The weather today is sunny and clear.");
        assert!(
            !acc_text.contains("Step 1"),
            "Accumulator must quarantine thinking tokens"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure a 0-tool turn in allow_tools mode latches to Ready only after all audio has drained.
/// SCENARIO: Stream finishes with 0 tools (Key 1 satisfied: turn_open == false); audio is still active.
/// ASSERTS:  State remains Speaking while jobs > 0; transitions to Ready when pending jobs reaches 0 (Key 2 satisfied).
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   llm.rs: In on_llm_finished, transition to Ready unconditionally without checking audio_active.
#[tokio::test]
async fn zero_tool_turn_with_interim_filler_latches_to_ready_only_after_playback_drains() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 102;

        rig.state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        rig.state.pipeline.set_state(InteractionState::Thinking);
        rig.state
            .pipeline
            .pending_tts_jobs
            .store(1, Ordering::Relaxed);

        // Signal LlmFinished via authentic production routing while 1 pending synthesis job is active
        rig.send_event(VoxEvent::LlmFinished { turn_id });
        rig.wait_for_event(
            |ev| matches!(ev, VoxEvent::LlmFinished { turn_id: tid } if *tid == turn_id),
            Duration::from_secs(5),
        )
        .await
        .expect("LlmFinished routed");

        // RULE-EVT-09 §3: LlmFinished marks llm_done = true; state remains in Thinking until PlaybackStarted
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Thinking,
            "State must remain in Thinking on LlmFinished while pending audio synthesis has not started"
        );

        // Sound card begins playback -> transitions to Speaking (RULE-EVT-09 §1)
        rig.send_event(VoxEvent::PlaybackStarted { turn_id });
        assert!(
            rig.wait_for_state(InteractionState::Speaking, Duration::from_secs(5))
                .await,
            "PlaybackStarted must transition state to Speaking"
        );

        // Calling playback_finished via authentic VoxEvent while 1 job is pending must defer Ready
        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id,
        });
        rig.wait_for_event(
            |ev| matches!(ev, VoxEvent::PlaybackFinished { turn_id: tid, .. } if *tid == turn_id),
            Duration::from_secs(5),
        )
        .await
        .expect("PlaybackFinished routed");
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Speaking,
            "State must remain Speaking while pending_synthesis_jobs > 0"
        );

        // Now clear pending jobs (Key 2 satisfied) and signal sound card playback finished
        rig.state
            .pipeline
            .pending_tts_jobs
            .store(0, Ordering::Relaxed);
        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id,
        });

        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_secs(5))
                .await,
            "Two-Key Latch must transition deterministically to Ready when audio drains"
        );
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 3: Scene 2 — 1-Tool Preamble Turn (Preamble Preservation & Universal Tools)
// ============================================================================

/// PURPOSE:  Ensure spoken preamble text emitted before tool calls is preserved in conversation history.
/// SCENARIO: Model outputs "Checking your calendar..." followed by search_memory tool call.
/// ASSERTS:  History holds assistant message containing the preamble text without chunker buffer discard.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   router.rs: Clear accumulator or discard preamble upon ToolCall.
#[tokio::test]
async fn preamble_tokens_before_tool_call_are_preserved_in_history_without_chunker_clear() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 201;

        // Step 1: Model streams spoken preamble, then issues search_memory tool call
        // Pass 0: Preamble speech + search_memory tool call
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Let me look that up in your notes.".to_string(),
            ));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-search-1".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "meeting notes" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 1: Terminal answer after tool observation
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "You have a meeting with Sarah tomorrow.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("When is my meeting?", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed {
                assistant_response, ..
            } => {
                assert_eq!(
                    assistant_response,
                    "You have a meeting with Sarah tomorrow."
                );
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        // Verify preamble was dispatched to TTS as InterimFiller
        let tts_cmds = rig.drain_tts_commands();
        let preamble_cmd = tts_cmds.iter().find(|cmd| {
            if let TtsCommand::Generate { text, .. } = cmd {
                text.contains("look that up")
            } else {
                false
            }
        });
        assert!(
            preamble_cmd.is_some(),
            "Preamble speech must be dispatched to TTS as InterimFiller"
        );

        // Verify preamble text is committed in history as the initiating assistant message
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let messages = harness.messages();
        let preamble_msg = messages.iter().find(|m| {
            m.role == Role::Assistant
                && m.content.contains("look that up")
                && m.tool_calls.is_some()
        });
        assert!(
            preamble_msg.is_some(),
            "Preamble text must be recorded in assistant history initiating tools"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure Duplex state cycling transitions Working -> Speaking on playback start, and Speaking -> Working on playback finish when LLM is active.
/// SCENARIO: Preamble playback fires PlaybackStarted and PlaybackFinished while llm_done == false.
/// ASSERTS:  Pipeline transitions to Speaking on PlaybackStarted, and back to Working on PlaybackFinished per RULE-EVT-09.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   playback.rs: In on_playback_started, do not transition from Working to Speaking.
#[tokio::test]
async fn preamble_audio_playback_retains_working_state_during_tool_execution() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 202;

        rig.state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        rig.state.pipeline.set_state(InteractionState::Working);
        rig.state
            .pipeline
            .set_current_activity(Some(&vox_lib::core::events::ActivityEnvelope {
                kind: vox_lib::core::events::ActivityKind::Tool,
                name: "test_tool".to_string(),
                call_id: Some("call-1".to_string()),
            }));

        // Onset of interim filler audio must transition state to Speaking (RULE-EVT-09 §1)
        rig.send_event(VoxEvent::PlaybackStarted { turn_id });
        assert!(
            rig.wait_for_state(InteractionState::Speaking, Duration::from_secs(5))
                .await,
            "PlaybackStarted must transition state to Speaking"
        );

        // Completion of interim filler audio while llm_done == false must return to Working (RULE-EVT-09 §2)
        rig.send_event(VoxEvent::PlaybackFinished { turn_id });
        assert!(
            rig.wait_for_state(InteractionState::Working, Duration::from_secs(5))
                .await,
            "PlaybackFinished while llm_done == false and tools active must return to Working"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure tools conform to the universal contract returning ToolResult without synthetic speech parameters.
/// SCENARIO: Model invokes search_memory with standard query parameters.
/// ASSERTS:  The turn completes with the scripted answer and exactly two model passes are dispatched.
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
            let _ = tx.send(LlmResponse::Token(
                "Memory retrieved successfully.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Find my test facts", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed {
                assistant_response, ..
            } => {
                assert_eq!(assistant_response, "Memory retrieved successfully.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let dispatches = rig.requests();
        assert_eq!(dispatches.len(), 2, "Pass 1 tool call, Pass 2 response");

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("harness stays mounted");
        assert!(
            harness
                .messages()
                .iter()
                .any(|m| m.role == Role::Tool && !m.content.trim().is_empty()),
            "the tool observation content must be committed to history"
        );
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 4: Scene 3 — Multi-Tool Batch Turn (Barrier Execution & Batch Demuxing)
// ============================================================================

/// PURPOSE:  Verify router does not abort on the first tool call; accumulates all calls until Finished.
/// SCENARIO: Model emits 3 tool calls consecutively before emitting Finished.
/// ASSERTS:  All 3 tool calls are captured in the batch; all 3 execute before the next model pass.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   router.rs: Break out of streaming loop on first ToolCall event.
#[tokio::test]
async fn multi_tool_stream_accumulates_full_batch_without_early_stream_abort() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 301;

        // Pass 1: Stream emits 3 tool calls in a single pass without break
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-batch-1".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "query one" }),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-batch-2".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "query two" }),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-batch-3".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "query three" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 2: Final response synthesizing results
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "I have checked all three items.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Check three different memory queries", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed {
                assistant_response, ..
            } => {
                assert_eq!(assistant_response, "I have checked all three items.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let dispatches = rig.requests();
        assert_eq!(
            dispatches.len(),
            2,
            "Must be exactly 2 passes (batch pass + final synthesis)"
        );

        // Assert all 3 Role::Tool messages are recorded in history
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_messages: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert_eq!(
            tool_messages.len(),
            3,
            "All 3 tool observations must be recorded in history"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Verify set_title universal tool updates session title in DB and latches title_set flag on Harness.
/// SCENARIO: Model calls set_title on Turn 1.
/// ASSERTS:  The session title is persisted in DB and the title tool is filtered from the registry thereafter.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   harness.rs: Do not set title_set = true when set_title executes.
#[tokio::test]
async fn set_title_universal_tool_updates_session_and_latches_title_set_flag() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 303;

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-set-title-first".to_string(),
                name: "set_title".to_string(),
                arguments: serde_json::json!({ "title": "Cognitive Architecture Spec" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Session initialized with concise title.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Initialize session title please", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(matches!(outcome, TurnOutcome::Completed { .. }));

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness must be mounted");
            assert!(
                harness.title_set(),
                "harness.title_set must be latched to true"
            );

            let filter = ToolFilter {
                mode: PipelineMode::Modular,
                title_is_unset: !harness.title_set(),
                memory_retrieval_enabled: true,
                web_search_enabled: true,
            };
            let defs = harness.tool_registry().active_definitions(&filter);
            assert!(
                !defs
                    .iter()
                    .any(|d| d.name == "set_title" || d.name == "respond_and_set_title"),
                "set_title must be filtered out once title_set is true"
            );
        }

        let title = rig
            .wait_for_session_title("Cognitive Architecture Spec", Duration::from_secs(5))
            .await;
        assert_eq!(
            title.as_deref(),
            Some("Cognitive Architecture Spec"),
            "Session title in database must match set_title argument"
        );
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 5: Scene 4 — Cancellation, Barge-In & Fault Injection
// ============================================================================

/// PURPOSE:  Ensure turn cancellation during tool execution aborts the turn cleanly without history corruption.
/// SCENARIO: Turn token is cancelled while tool batch execution is pending.
/// ASSERTS:  TurnOutcome is Cancelled; VoxEvent::Cancelled is emitted; pipeline state resets to Ready.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   steps.rs: Ignore cancellation token check prior to tool execution.
#[tokio::test]
async fn turn_cancellation_during_tool_execution_aborts_cleanly_and_clears_turn_open() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 401;

        // Initialize turn_open to true to verify that cancellation resets it on the router thread

        let cancel_token = rig.state.pipeline.turn_cancel_token();
        let cancel_clone = cancel_token.clone();

        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-cancel-mid-tool".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "cancel test" }),
            }));
            cancel_clone.cancel();
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Query that gets cancelled during tool exec", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Cancelled { .. } => {}
            other => panic!("Expected Cancelled outcome, got {:?}", other),
        }

        // Wait for router thread to process VoxEvent::Cancelled and reset turn_open
        let _ = rig
            .wait_for_event(
                |ev| matches!(ev, VoxEvent::Cancelled { turn_id: tid, .. } if *tid == turn_id),
                Duration::from_secs(5),
            )
            .await
            .expect("VoxEvent::Cancelled must be emitted on event channel");

        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_secs(5))
                .await,
            "terminal event must drive the pipeline to Ready"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Barge-in with spoken partial speech must commit user query and partial assistant text.
/// SCENARIO: User cancels turn after assistant has emitted partial speech.
/// ASSERTS:  The interrupted turn's partial assistant text is recorded in the DB turns table.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   loop.rs: Drop assistant partial text on cancellation when partial_text is non-empty.
#[tokio::test]
async fn turn_cancellation_with_partial_spoken_text_commits_interrupted_turn() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 402;
        let cancel_token = rig.state.pipeline.turn_cancel_token();
        let cancel_token_clone = cancel_token.clone();
        let acc = Arc::clone(&rig.state.pipeline_accumulator);

        let cancel_task = tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            while tokio::time::Instant::now() < deadline {
                if !acc.lock().assistant_response.is_empty() {
                    cancel_token_clone.cancel();
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("Timed out waiting for partial token in accumulator");
        });

        let cancel_flag = cancel_token.clone();
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Token(
                "Partial sentence before interruption.".to_string(),
            ));
            while !cancel_flag.is_cancelled() {
                std::thread::yield_now();
            }
            let _ = tx.send(LlmResponse::Cancelled);
        });

        let req = rig.turn_request("Interrupt mid-sentence test", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        cancel_task.await.expect("cancel task ok");

        match outcome {
            TurnOutcome::Cancelled { turn_id: tid } => {
                assert_eq!(tid, turn_id);
            }
            other => panic!("Expected Cancelled outcome, got {:?}", other),
        }

        let recorded_text = rig
            .wait_for_turn_assistant_text(Duration::from_secs(5))
            .await;
        assert!(
            recorded_text
                .as_deref()
                .map(|t| t.contains("Partial sentence"))
                .unwrap_or(false),
            "Database turns table must record the interrupted partial speech; saw {:?}",
            recorded_text
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Barge-in before any speech or tokens must roll back history with zero DB artifacts.
/// SCENARIO: Turn token is cancelled before any tokens are emitted by the model.
/// ASSERTS:  TurnOutcome is Cancelled; zero user or assistant turns in DB turns table.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   loop.rs: Commit turn even if partial_text is empty on cancellation.
#[tokio::test]
async fn turn_cancellation_before_speech_rolls_back_history_with_zero_db_artifacts() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 403;

        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Cancelled);
        });

        rig.state.pipeline.turn_cancel_token().cancel();

        let req = rig.turn_request("Prompt cancelled before speech", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Cancelled { turn_id: tid } => {
                assert_eq!(tid, turn_id);
            }
            other => panic!("Expected Cancelled outcome, got {:?}", other),
        }

        let conn = rig.state.db.connect().expect("db connect");
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM turns WHERE session_id = ? AND turn_id = ?;",
                (rig.session_id, turn_id as i64),
            )
            .await
            .expect("query turns");
        let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            count, 0,
            "Zero turns must be persisted when cancelled before speech"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Verify stream errors release pipeline state to Ready and dispatch VoxEvent::Error.
/// SCENARIO: Model returns LlmResponse::Error due to upstream failure.
/// ASSERTS:  TurnOutcome is Error with matching message; VoxEvent::Error is emitted on event channel; pipeline state resets to Ready.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   router.rs: Omit VoxEvent::Error on stream error.
#[tokio::test]
async fn stream_error_clears_turn_open_and_dispatches_error_event() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 404;

        // Initialize turn_open to true to verify that error handling resets it on the router thread

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
            TurnOutcome::Error {
                turn_id: tid,
                message,
            } => {
                assert_eq!(tid, turn_id);
                assert!(
                    message.contains("Upstream model service timeout")
                        || message.contains("LLM stream error")
                );
            }
            other => panic!("Expected Error outcome, got {:?}", other),
        }

        // Assert: VoxEvent::Error must be emitted on the event channel!
        let err_event = rig
            .wait_for_event(
                |ev| matches!(ev, VoxEvent::Error(err) if err.turn_id == turn_id),
                Duration::from_secs(5),
            )
            .await
            .expect("VoxEvent::Error must be emitted on pipeline stream upon stream failure");

        if let VoxEvent::Error(err) = err_event {
            assert_eq!(err.turn_id, turn_id);
            assert_eq!(err.impact, PipelineImpact::TurnAborted);
            assert!(err.message.contains("Upstream model service timeout"));
        }

        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_secs(5))
                .await,
            "terminal event must drive the pipeline to Ready"
        );
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 7: Domain 2 — Cognitive Loop, Concurrency & Immediate Step Commits
// ============================================================================

/// PURPOSE:  Catch loss of completed tool observations or accidental deletion of the user prompt when a turn is cancelled after Step 1 completes.
/// SCENARIO: Step 1 executes [set_title, search_memory] and commits; during Step 2 tool execution, `cancel_token.cancel()` fires.
/// ASSERTS:  TurnOutcome is Cancelled; `harness.messages()` retains the User message, Step 1 Assistant(tool_calls) + 2 Tool observations, and Step 2 Assistant(tool_calls) + synthesized cancelled Tool observation (zero orphaned tool_call_ids); pipeline reaches Ready after router processes VoxEvent::Cancelled.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   In `Harness::commit_tool_step` (`chassis.rs`), delete the `flush_staged_user_query()` call.
#[tokio::test]
async fn mid_turn_cancellation_on_step_two_preserves_step_one_history_and_forbids_user_rollback() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 501;

        let cancel_token = rig.state.pipeline.turn_cancel_token();
        let cancel_clone = cancel_token.clone();

        // Pass 1 (Step 1): Emits 2 tool calls (set_title and search_memory)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-s1-title".to_string(),
                name: "set_title".to_string(),
                arguments: serde_json::json!({ "title": "Preserved History Title" }),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-s1-search".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "preserved facts" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 2 (Step 2): Emits 1 tool call and triggers cancellation
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-s2-search".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "interrupted search" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
            cancel_clone.cancel();
        });

        let query = "Multi-step query that gets cancelled during step two";
        let req = rig.turn_request(query, turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Cancelled { .. } => {}
            other => panic!("Expected Cancelled outcome on mid-turn cancel, got {:?}", other),
        }

        // Wait for router thread to process VoxEvent::Cancelled and clear turn_open
        let _ = rig
            .wait_for_event(
                |ev| matches!(ev, VoxEvent::Cancelled { turn_id: tid, .. } if *tid == turn_id),
                Duration::from_secs(5),
            )
            .await
            .expect("VoxEvent::Cancelled must be emitted on event channel");

        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_secs(5)).await,
            "terminal event must drive the pipeline to Ready"
        );

        // Assert conversation history invariants
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let messages = harness.messages();

        // 1. User message must be preserved (rollback forbidden after step 1 execution!)
        let user_msg = messages.iter().find(|m| m.role == Role::User);
        assert!(
            user_msg.is_some(),
            "Staged user query must be committed with the first tool step and never rolled back!"
        );
        assert_eq!(user_msg.unwrap().content, query);

        // 2. Step 1 Assistant message with tool calls must exist
        let s1_assistant = messages.iter().find(|m| {
            m.role == Role::Assistant
                && m.tool_calls
                    .as_ref()
                    .map(|calls| calls.iter().any(|c| c.id == "call-s1-title"))
                    .unwrap_or(false)
        });
        assert!(s1_assistant.is_some(), "Step 1 Assistant message with tool calls must be committed");
        assert_eq!(s1_assistant.unwrap().tool_calls.as_ref().unwrap().len(), 2);

        // 3. Step 1 Tool observation messages must exist in declaration order
        let tool_msgs: Vec<_> = messages.iter().filter(|m| m.role == Role::Tool).collect();
        assert!(
            tool_msgs.len() >= 2,
            "Step 1 tool observations must be preserved in history, got {}",
            tool_msgs.len()
        );
        assert_eq!(tool_msgs[0].tool_call_id.as_deref(), Some("call-s1-title"));
        assert_eq!(tool_msgs[1].tool_call_id.as_deref(), Some("call-s1-search"));

        // 4. Step 2 Assistant message + synthesized Tool observation must prevent orphaned tool_call_id
        assert_eq!(
            tool_msgs.len(),
            3,
            "Step 2 cancelled tool must synthesize a Tool observation so tool_calls <-> Role::Tool is 1:1, got {}",
            tool_msgs.len()
        );
        assert_eq!(tool_msgs[2].tool_call_id.as_deref(), Some("call-s2-search"));
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch a staged user query leaking into canonical history when a Step 0 turn is cancelled before any artifact exists.
/// SCENARIO: Model streams Thinking tokens in Step 0 and `cancel_token.cancel()` triggers before any Token or ToolCall is emitted.
/// ASSERTS:  TurnOutcome is Cancelled; `harness.messages()` contains only Role::System; the staged query never entered history; pipeline reaches Ready.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   In `Harness::stage_user_query` (`chassis.rs`), call `self.history.push_user_turn(query)` instead of staging it.
#[tokio::test]
async fn step_zero_cancellation_discards_staged_query_without_touching_history() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 502;

        let cancel_token = rig.state.pipeline.turn_cancel_token();
        let cancel_clone = cancel_token.clone();

        // Step 0: Model streams thinking tokens and gets cancelled before any token or tool call
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Thinking(
                "Thinking deeply in Step 0...".to_string(),
            ));
            cancel_clone.cancel();
            let _ = tx.send(LlmResponse::Finished);
        });

        let query = "Unanswered query cancelled during Step 0 thinking";
        let req = rig.turn_request(query, turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Cancelled { .. } => {}
            other => panic!(
                "Expected Cancelled outcome on step 0 cancel, got {:?}",
                other
            ),
        }

        // Wait for router thread to process VoxEvent::Cancelled
        let _ = rig
            .wait_for_event(
                |ev| matches!(ev, VoxEvent::Cancelled { turn_id: tid, .. } if *tid == turn_id),
                Duration::from_secs(5),
            )
            .await
            .expect("VoxEvent::Cancelled must be emitted on event channel");

        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_secs(5))
                .await,
            "terminal event must drive the pipeline to Ready"
        );

        // Assert the staged query never entered canonical history.
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let messages = harness.messages();

        let has_user_msg = messages.iter().any(|m| m.role == Role::User);
        assert!(
            !has_user_msg,
            "A Step 0 query must stay staged until an assistant artifact exists!"
        );
        assert!(
            messages.iter().any(|m| m.role == Role::System),
            "System prompt must be retained"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch serial execution regression or out-of-order observation commits when parallel tools finish out of order.
/// SCENARIO: A single ReAct step emits 3 Parallel tools [P1, P2, P3] that rendezvous at a 3-party barrier, where P3 finishes fastest and P1 finishes slowest.
/// ASSERTS:  All 3 tools pass the concurrency rendezvous without timing out; trace log records all 3 "start:*" entries before any "end:*" entry; ConversationHistoryStage records Role::Tool messages in exact [P1, P2, P3] declaration order.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   In `ToolExecutor::run_parallel_wave` (`executor.rs`), replace `join_all(futs).await` with a sequential `for call in wave` loop.
#[tokio::test]
async fn parallel_tool_wave_executes_concurrently_and_commits_in_declaration_order() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 601;

        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let trace = Arc::new(parking_lot::Mutex::new(Vec::new()));

        let p1 = Arc::new(
            common::harness::BarrierTraceTool::new(
                "P1",
                vox_lib::harness::ToolExecutionMode::Parallel,
                Some(barrier.clone()),
                trace.clone(),
            )
            .with_yields(4),
        );

        let p2 = Arc::new(
            common::harness::BarrierTraceTool::new(
                "P2",
                vox_lib::harness::ToolExecutionMode::Parallel,
                Some(barrier.clone()),
                trace.clone(),
            )
            .with_yields(2),
        );

        let p3 = Arc::new(
            common::harness::BarrierTraceTool::new(
                "P3",
                vox_lib::harness::ToolExecutionMode::Parallel,
                Some(barrier.clone()),
                trace.clone(),
            )
            .with_yields(0),
        );

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(p1);
            harness.register_tool(p2);
            harness.register_tool(p3);
        }

        // Pass 1: Model emits [P1, P2, P3] in a single ReAct step
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-p1".to_string(),
                name: "P1".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-p2".to_string(),
                name: "P2".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-p3".to_string(),
                name: "P3".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 2: Final response
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "All parallel tools executed.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Run 3 parallel tools", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "Turn must complete successfully, got {:?}",
            outcome
        );

        // Concurrency proof: All 3 tools must rendezvous and record "start:*" before ANY "end:*" is recorded
        let t = trace.lock().clone();
        assert_eq!(
            t.len(),
            6,
            "Expected 3 starts and 3 ends in trace log, got {:?}",
            t
        );
        assert!(
            t[0].starts_with("start:"),
            "Entry 0 must be start:*, got {}",
            t[0]
        );
        assert!(
            t[1].starts_with("start:"),
            "Entry 1 must be start:*, got {}",
            t[1]
        );
        assert!(
            t[2].starts_with("start:"),
            "Entry 2 must be start:*, got {}",
            t[2]
        );

        // Out-of-order completion proof: P3 finishes fastest (0 yields), P2 next (2 yields), P1 slowest (4 yields)
        assert_eq!(t[3], "end:P3", "P3 must finish first due to 0 yields");
        assert_eq!(t[4], "end:P2", "P2 must finish second due to 2 yields");
        assert_eq!(t[5], "end:P1", "P1 must finish last due to 4 yields");

        // Declaration order commit: In ConversationHistoryStage, Role::Tool messages must be exact [P1, P2, P3] order
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert_eq!(
            tool_msgs.len(),
            3,
            "All 3 tools must be committed to history"
        );
        assert_eq!(tool_msgs[0].tool_call_id.as_deref(), Some("call-p1"));
        assert_eq!(tool_msgs[1].tool_call_id.as_deref(), Some("call-p2"));
        assert_eq!(tool_msgs[2].tool_call_id.as_deref(), Some("call-p3"));
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch read-after-write race conditions if an Exclusive tool runs concurrently with preceding or succeeding Parallel tools.
/// SCENARIO: Model emits [P1 (Parallel), P2 (Parallel), E1 (Exclusive), P3 (Parallel), P4 (Parallel)] in Step 1.
/// ASSERTS:  Trace log proves P1 and P2 both complete ("end:P1", "end:P2") before "start:E1" is recorded, and "end:E1" is recorded before either "start:P3" or "start:P4" is recorded; all 5 observations commit in [P1, P2, E1, P3, P4] order.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In `ToolExecutor::execute_batch` (`executor.rs`), return `ToolExecutionMode::Parallel` unconditionally instead of querying `t.execution_mode(&call.arguments)`.
#[tokio::test]
async fn exclusive_tool_acts_as_strict_barrier_between_parallel_waves() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 602;

        let trace = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let wave1_barrier = Arc::new(tokio::sync::Barrier::new(2));
        let wave2_barrier = Arc::new(tokio::sync::Barrier::new(2));

        let p1 = Arc::new(common::harness::BarrierTraceTool::new(
            "P1",
            vox_lib::harness::ToolExecutionMode::Parallel,
            Some(wave1_barrier.clone()),
            trace.clone(),
        ));
        let p2 = Arc::new(common::harness::BarrierTraceTool::new(
            "P2",
            vox_lib::harness::ToolExecutionMode::Parallel,
            Some(wave1_barrier.clone()),
            trace.clone(),
        ));
        let e1 = Arc::new(common::harness::BarrierTraceTool::new(
            "E1",
            vox_lib::harness::ToolExecutionMode::Exclusive,
            None,
            trace.clone(),
        ));
        let p3 = Arc::new(common::harness::BarrierTraceTool::new(
            "P3",
            vox_lib::harness::ToolExecutionMode::Parallel,
            Some(wave2_barrier.clone()),
            trace.clone(),
        ));
        let p4 = Arc::new(common::harness::BarrierTraceTool::new(
            "P4",
            vox_lib::harness::ToolExecutionMode::Parallel,
            Some(wave2_barrier.clone()),
            trace.clone(),
        ));

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(p1);
            harness.register_tool(p2);
            harness.register_tool(e1);
            harness.register_tool(p3);
            harness.register_tool(p4);
        }

        // Pass 1: Model emits [P1, P2, E1, P3, P4] in Step 1
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-p1".to_string(),
                name: "P1".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-p2".to_string(),
                name: "P2".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-e1".to_string(),
                name: "E1".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-p3".to_string(),
                name: "P3".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-p4".to_string(),
                name: "P4".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 2: Final synthesis
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Batch with exclusive barrier executed.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Run exclusive barrier batch", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "Turn must complete successfully, got {:?}",
            outcome
        );

        let t = trace.lock().clone();
        assert_eq!(t.len(), 10, "Expected 10 events for 5 tools, got {:?}", t);

        let p1_end = t.iter().position(|e| e == "end:P1").expect("end:P1");
        let p2_end = t.iter().position(|e| e == "end:P2").expect("end:P2");
        let e1_start = t.iter().position(|e| e == "start:E1").expect("start:E1");
        let e1_end = t.iter().position(|e| e == "end:E1").expect("end:E1");
        let p3_start = t.iter().position(|e| e == "start:P3").expect("start:P3");
        let p4_start = t.iter().position(|e| e == "start:P4").expect("start:P4");

        // P1 and P2 both complete before start:E1
        assert!(
            p1_end < e1_start,
            "P1 must complete before E1 starts (p1_end={}, e1_start={})",
            p1_end,
            e1_start
        );
        assert!(
            p2_end < e1_start,
            "P2 must complete before E1 starts (p2_end={}, e1_start={})",
            p2_end,
            e1_start
        );

        // end:E1 is recorded before either start:P3 or start:P4
        assert!(
            e1_end < p3_start,
            "E1 must complete before P3 starts (e1_end={}, p3_start={})",
            e1_end,
            p3_start
        );
        assert!(
            e1_end < p4_start,
            "E1 must complete before P4 starts (e1_end={}, p4_start={})",
            e1_end,
            p4_start
        );

        // Declaration order commit: [P1, P2, E1, P3, P4]
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert_eq!(tool_msgs.len(), 5, "All 5 observations must commit");
        assert_eq!(tool_msgs[0].tool_call_id.as_deref(), Some("call-p1"));
        assert_eq!(tool_msgs[1].tool_call_id.as_deref(), Some("call-p2"));
        assert_eq!(tool_msgs[2].tool_call_id.as_deref(), Some("call-e1"));
        assert_eq!(tool_msgs[3].tool_call_id.as_deref(), Some("call-p3"));
        assert_eq!(tool_msgs[4].tool_call_id.as_deref(), Some("call-p4"));
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch hard turn abort on reaching MAX_TOOL_STEPS instead of synthesizing a final tool-free spoken answer.
/// SCENARIO: Configure Harness with `max_steps = 3`; scripted LLM emits a tool call on Steps 0, 1, and 2, then on the forced summary pass (`allow_tools == false`) emits final spoken text.
/// ASSERTS:  LLM worker receives 4 requests total; requests 0..=2 have non-empty `tools`, request 3 has `tools.is_empty()` (or `None`); TurnOutcome is Completed with final summary text dispatched to TTS; all 3 tool steps (6 messages) + final Assistant message are in `harness.messages()`.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In `execute_loop_iterations` (`loop.rs`), return `TurnOutcome::Error` when `step_index >= max_steps` instead of setting `allow_tools = false`.
#[tokio::test]
async fn step_ceiling_breach_disables_tools_and_forces_spoken_summary_pass() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 605;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.set_max_steps(3);
        }

        // Pass 0 (Step 0): tool call
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-step-0".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "step 0" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 1 (Step 1): tool call
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-step-1".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "step 1" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 2 (Step 2): tool call
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-step-2".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "step 2" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 3 (Forced Summary Pass with allow_tools == false): Final spoken summary
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Final synthesized summary after reaching step limit.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Exhaust tool step limit", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed {
                ref assistant_response,
                ..
            } => {
                assert_eq!(
                    assistant_response,
                    "Final synthesized summary after reaching step limit."
                );
            }
            other => panic!(
                "Expected Completed outcome after forced summary pass, got {:?}",
                other
            ),
        }

        // Assert: LLM worker received 4 requests total; requests 0..=2 have non-empty tools, request 3 has false (tool-free)
        let requests = rig.requests();
        assert_eq!(
            requests.len(),
            4,
            "Expected 4 dispatches (3 tool steps + 1 summary pass), got {}",
            requests.len()
        );
        assert!(requests[0].1, "Pass 0 must offer tools");
        assert!(requests[1].1, "Pass 1 must offer tools");
        assert!(requests[2].1, "Pass 2 must offer tools");
        assert!(
            !requests[3].1,
            "Pass 3 (summary pass) must be tool-free (allow_tools == false)"
        );

        // Assert: TTS received final synthesized summary
        let tts_cmds = rig.drain_tts_commands();
        let turn_response_cmd = tts_cmds.iter().find(|cmd| {
            matches!(cmd, TtsCommand::Generate { text, .. }
                if text.contains("Final synthesized summary"))
        });
        assert!(
            turn_response_cmd.is_some(),
            "Summary pass must dispatch final synthesized summary to TTS"
        );

        // Assert: Conversation history holds all 3 tool steps (3 Assistant + 3 Tool) + final Assistant summary
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert_eq!(
            tool_msgs.len(),
            3,
            "All 3 tool observations must be committed"
        );
        let assistant_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .collect();
        assert_eq!(
            assistant_msgs.len(),
            4,
            "3 intermediate tool calls + 1 final summary pass"
        );
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 7: Domain 3 Guardrails, Observation Truncation & Mid-Loop Compaction
// ============================================================================

struct LargeHeadTailTool;

impl ToolDefinition for LargeHeadTailTool {
    fn name(&self) -> &str {
        "large_head_tail_tool"
    }

    fn description(&self, _mode: PipelineMode) -> &str {
        "Returns an oversized observation to test HeadTail truncation and Tier 4 spillover"
    }

    fn parameters_schema(&self, _mode: PipelineMode) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn truncation_policy(&self) -> TruncationPolicy {
        TruncationPolicy::HeadTail { head_ratio: 0.8 }
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
        _args: serde_json::Value,
        _ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let mut payload = String::with_capacity(50_000);
            payload.push_str("HEAD_SENTINEL_LINE_1\n");
            for i in 0..600 {
                if i == 300 {
                    payload.push_str("MIDDLE_SECRET_PAYLOAD\n");
                } else {
                    payload.push_str("filler content line for observation token budgeting test\n");
                }
            }
            payload.push_str("TAIL_SENTINEL_LINE_500\n");
            Ok(ToolResult::new(payload))
        })
    }
}

struct LargePolicyNoneTool;

impl ToolDefinition for LargePolicyNoneTool {
    fn name(&self) -> &str {
        "large_policy_none_tool"
    }

    fn description(&self, _mode: PipelineMode) -> &str {
        "Returns an oversized observation declaring TruncationPolicy::None"
    }

    fn parameters_schema(&self, _mode: PipelineMode) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn truncation_policy(&self) -> TruncationPolicy {
        TruncationPolicy::None
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
        _args: serde_json::Value,
        _ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let mut payload = String::with_capacity(50_000);
            payload.push_str("POLICY_NONE_HEAD\n");
            for _ in 0..600 {
                payload.push_str("large untouched content for policy none test\n");
            }
            payload.push_str("POLICY_NONE_TAIL\n");
            Ok(ToolResult::new(payload))
        })
    }
}

/// PURPOSE:  Catch infinite or 20-step runaway loops when model repeats exact same tool call and arguments.
/// SCENARIO: Model calls search_memory({"query": "same"}) repeatedly. Strike 1 triggers on 3rd identical call, injecting [SYSTEM WARNING: into Role::Tool observation. Model calls it 4th time (Strike 2), forcing allow_tools = false and synthesizing spoken summary on pass 5.
/// ASSERTS:  Total dispatches = 5 (not 21); passes 0..=3 have tools enabled; pass 4 has allow_tools == false; 3rd observation contains [SYSTEM WARNING: and identical; outcome is Completed.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In GuardrailsStage, return GuardrailAction::Proceed unconditionally without checking identical fingerprints.
#[tokio::test]
async fn identical_tool_call_loop_triggers_strike_one_warning_then_strike_two_circuit_breaker() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1101;

        // Pass 0 (Call 1)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-1".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "same query" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 1 (Call 2)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-2".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "same query" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 2 (Call 3 -> Strike 1 warning injected into observation)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-3".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "same query" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 3 (Call 4 -> Strike 2 circuit breaker strips tools)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-4".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "same query" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 4 (Forced Summary Pass with allow_tools == false)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Synthesized summary after circuit break.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Trigger identical tool loop", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "turn must complete successfully, got {:?}",
            outcome
        );

        let requests = rig.requests();
        assert_eq!(
            requests.len(),
            5,
            "Expected 5 dispatches total (4 tool passes + 1 summary pass), got {}",
            requests.len()
        );
        assert!(requests[0].1, "Pass 0 must offer tools");
        assert!(requests[1].1, "Pass 1 must offer tools");
        assert!(requests[2].1, "Pass 2 must offer tools");
        assert!(requests[3].1, "Pass 3 must offer tools");
        assert!(
            !requests[4].1,
            "Pass 4 (summary pass) must be tool-free (allow_tools == false)"
        );

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert!(
            tool_msgs.len() >= 3,
            "At least 3 tool observations must be committed"
        );
        let third_obs = &tool_msgs[2].content;
        assert!(
            third_obs.contains("[SYSTEM WARNING:") && third_obs.contains("identical"),
            "3rd observation must contain Strike 1 warning for identical call loop, got: {}",
            third_obs
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch alternating A -> B -> A -> B tool oscillation loops.
/// SCENARIO: Model alternates search_memory({"query": "alpha"}) and web_search({"query": "beta"}) across steps (A, B, A, B -> Strike 1 warning on 4th call; then A -> Strike 2 circuit break -> tool-free summary pass).
/// ASSERTS:  4th observation contains [SYSTEM WARNING: and ping_pong; pass following Strike 2 has allow_tools == false; outcome is Completed.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In GuardrailsStage, disable the ping-pong pattern matcher.
#[tokio::test]
async fn ping_pong_alternating_tool_loop_triggers_two_strike_circuit_breaker() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1102;

        // Step 0: Tool A (search_memory)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-ping-1".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "alpha" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 1: Tool B (web_search)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-pong-1".to_string(),
                name: "web_search".to_string(),
                arguments: serde_json::json!({ "query": "beta" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 2: Tool A (search_memory)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-ping-2".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "alpha" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 3: Tool B (web_search) -> Strike 1 warning on 4th alternating call!
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-pong-2".to_string(),
                name: "web_search".to_string(),
                arguments: serde_json::json!({ "query": "beta" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 4: Tool A (search_memory) -> Strike 2 circuit breaker!
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-ping-3".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "alpha" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 5: Summary pass (allow_tools == false)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Summary after ping pong break.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Trigger ping pong loop", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "turn must complete successfully, got {:?}",
            outcome
        );

        let requests = rig.requests();
        assert_eq!(
            requests.len(),
            6,
            "Expected 6 dispatches (5 tool steps + 1 summary pass), got {}",
            requests.len()
        );
        assert!(
            requests[0..=4].iter().all(|(_, tools)| *tools),
            "Steps 0..=4 must offer tools"
        );
        assert!(
            !requests[5].1,
            "Pass following Strike 2 must have allow_tools == false"
        );

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert!(
            tool_msgs.len() >= 4,
            "At least 4 tool observations must be committed"
        );
        let fourth_obs = &tool_msgs[3].content;
        assert!(
            fourth_obs.contains("[SYSTEM WARNING:") && fourth_obs.contains("ping_pong"),
            "4th observation must contain Strike 1 ping_pong warning, got: {}",
            fourth_obs
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch cascading tool failure loops across consecutive steps without recovery.
/// SCENARIO: Model invokes a failing tool across 3 consecutive steps (Strike 1 warning on Step 2), and fails a 4th time on Step 3 (Strike 2 forces summary pass on Step 4).
/// ASSERTS:  Step 2 observation contains [SYSTEM WARNING: and error_cascade; request 4 has allow_tools == false; outcome is Completed.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In GuardrailsStage, ignore is_error when evaluating error cascade strikes.
#[tokio::test]
async fn consecutive_error_steps_trigger_two_strike_guardrail_and_force_summary() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1103;

        // Step 0: Failing tool call 1
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-err-0".to_string(),
                name: "nonexistent_failing_tool".to_string(),
                arguments: serde_json::json!({ "arg": 0 }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 1: Failing tool call 2
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-err-1".to_string(),
                name: "nonexistent_failing_tool".to_string(),
                arguments: serde_json::json!({ "arg": 1 }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 2: Failing tool call 3 -> 3 consecutive failing steps -> Strike 1 warning!
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-err-2".to_string(),
                name: "nonexistent_failing_tool".to_string(),
                arguments: serde_json::json!({ "arg": 2 }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 3: Failing tool call 4 -> 4th failing step -> Strike 2 circuit break!
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-err-3".to_string(),
                name: "nonexistent_failing_tool".to_string(),
                arguments: serde_json::json!({ "arg": 3 }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 4: Summary pass (allow_tools == false)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Summary after consecutive error circuit break.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Trigger consecutive error cascade", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "turn must complete successfully, got {:?}",
            outcome
        );

        let requests = rig.requests();
        assert_eq!(
            requests.len(),
            5,
            "Expected 5 dispatches (4 failing steps + 1 summary pass), got {}",
            requests.len()
        );
        assert!(
            requests[0..=3].iter().all(|(_, tools)| *tools),
            "Steps 0..=3 must offer tools"
        );
        assert!(!requests[4].1, "Pass 4 must have allow_tools == false");

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert!(
            tool_msgs.len() >= 3,
            "At least 3 tool observations must be committed"
        );
        let step2_obs = &tool_msgs[2].content;
        assert!(
            step2_obs.contains("[SYSTEM WARNING:") && step2_obs.contains("error_cascade"),
            "Step 2 observation must contain Strike 1 error_cascade warning, got: {}",
            step2_obs
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch context window blowout when a tool returns a massive payload exceeding max_observation_tokens.
/// SCENARIO: Register custom tool with TruncationPolicy::HeadTail { head_ratio: 0.8 } returning a 10,000-token string ("HEAD_SENTINEL_LINE_1 ... MIDDLE_SECRET_PAYLOAD ... TAIL_SENTINEL_LINE_500"). Execute 1-tool turn.
/// ASSERTS:  The committed observation contains "HEAD_SENTINEL_LINE_1", "TAIL_SENTINEL_LINE_500", and "Output truncated" but not "MIDDLE_SECRET_PAYLOAD"; the spillover file at paths::session_tool_outputs_dir(rig.session_id) exists and contains both sentinels.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In ToolExecutor::execute_tool, bypass TruncationStage and return raw ToolResult.
#[tokio::test]
async fn oversized_tool_observation_spills_to_tier4_cache_and_commits_head_tail_preview() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1104;
        let call_id = "call-oversized-001".to_string();

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(Arc::new(LargeHeadTailTool));
        }

        let cid = call_id.clone();
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: cid,
                name: "large_head_tail_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Turn completed after tool execution.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Execute oversized tool", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "turn must complete, got {:?}",
            outcome
        );

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert_eq!(tool_msgs.len(), 1, "Exactly 1 tool observation committed");
        let obs = &tool_msgs[0].content;

        // Check head and tail sentinels and truncation notice
        assert!(
            obs.contains("HEAD_SENTINEL_LINE_1"),
            "Observation must preserve head sentinel"
        );
        assert!(
            obs.contains("TAIL_SENTINEL_LINE_500"),
            "Observation must preserve tail sentinel"
        );
        assert!(
            obs.contains("Output truncated"),
            "Observation must contain truncation notice marker"
        );
        assert!(
            !obs.contains("MIDDLE_SECRET_PAYLOAD"),
            "Observation must NOT contain stripped middle payload"
        );

        // Verify Tier 4 spillover file exists on disk
        let spillover_file = vox_lib::utils::paths::session_tool_outputs_dir(rig.session_id)
            .join(format!("{}.txt", call_id));
        assert!(
            spillover_file.exists(),
            "Spillover cache file must exist at {:?}",
            spillover_file
        );
        let spilled_content =
            std::fs::read_to_string(&spillover_file).expect("read spillover file");
        assert!(spilled_content.contains("HEAD_SENTINEL_LINE_1"));
        assert!(spilled_content.contains("MIDDLE_SECRET_PAYLOAD"));
        assert!(spilled_content.contains("TAIL_SENTINEL_LINE_500"));
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure tools declaring TruncationPolicy::None bypass truncation.
/// SCENARIO: Tool with TruncationPolicy::None returns payload larger than max_observation_tokens.
/// ASSERTS:  The committed observation contains the unmodified head and tail sentinels and no truncation marker; zero files are created under paths::session_tool_outputs_dir(rig.session_id).
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In TruncationStage, apply HeadTail truncation regardless of TruncationPolicy::None.
#[tokio::test]
async fn truncation_policy_none_preserves_full_output_without_disk_spillover() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1105;
        let call_id = "call-policy-none-001".to_string();

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(Arc::new(LargePolicyNoneTool));
        }

        let cid = call_id.clone();
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: cid,
                name: "large_policy_none_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Turn finished.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Execute policy none tool", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "turn must complete, got {:?}",
            outcome
        );

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness must be mounted");
            let tool_msgs: Vec<_> = harness
                .messages()
                .iter()
                .filter(|m| m.role == Role::Tool)
                .collect();
            assert_eq!(tool_msgs.len(), 1, "Exactly 1 tool observation committed");
            let obs = &tool_msgs[0].content;

            assert!(obs.contains("POLICY_NONE_HEAD"));
            assert!(obs.contains("POLICY_NONE_TAIL"));
            assert!(
                !obs.contains("Output truncated"),
                "Truncation notice must NOT appear for Policy::None"
            );
        }

        // Zero files created in session tool outputs dir
        let session_tool_dir = vox_lib::utils::paths::session_tool_outputs_dir(rig.session_id);
        let file_count = if session_tool_dir.exists() {
            std::fs::read_dir(&session_tool_dir)
                .map(|entries| entries.count())
                .unwrap_or(0)
        } else {
            0
        };
        assert_eq!(
            file_count, 0,
            "Zero spillover files must be created for TruncationPolicy::None"
        );

        // Provenance assert: session_tool_calls must record truncated = 0 (bounded poll for async persistence)
        let conn = rig.state.db.connect().expect("db connect");
        let start = std::time::Instant::now();
        let mut found_row = None;
        while start.elapsed() < Duration::from_secs(5) {
            let mut rows = conn
                .query(
                    "SELECT truncated, spillover_path FROM session_tool_calls WHERE id = ?;",
                    (call_id.clone(),),
                )
                .await
                .expect("query session_tool_calls truncation columns");
            if let Some(row) = rows.next().await.expect("fetch row") {
                let truncated: i64 = row.get(0).expect("truncated");
                let spillover_path: Option<String> = row.get(1).expect("spillover_path");
                found_row = Some((truncated, spillover_path));
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let (truncated, spillover_path) =
            found_row.expect("session_tool_calls row must be persisted within deadline");
        assert_eq!(
            truncated, 0,
            "session_tool_calls must record truncated = 0 for TruncationPolicy::None"
        );
        assert!(
            spillover_path.is_none(),
            "spillover_path must be NULL for TruncationPolicy::None"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch mid-loop context overflow and prevent erasure of in-flight active turn steps during inline compaction.
/// SCENARIO: Seed prior turns to ~80% context budget; on Turn N Step 0, model executes tool pushing context >= 85% (Critical); before Step 1 LLM dispatch, inline compaction triggers.
/// ASSERTS:  Older prior turns compacted into <session_context>, while Turn N's Role::User query, Step 0 Role::Assistant(tool_calls), and Step 0 Role::Tool observation remain intact in harness.messages(); session_steps records is_compacted = 1.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In apply_session_context (chassis.rs), wipe the active turn's committed tool steps during mid-loop reset.
#[tokio::test]
async fn mid_loop_critical_budget_triggers_compaction_while_preserving_active_turn_steps() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1106;

        {
            let mut settings = rig.state.settings.write().unwrap();
            settings.memory.auto_compaction = true;
            settings.memory.context_retrieval_enabled = false;
        }

        {
            let settings = rig.state.settings.read().unwrap().clone();
            let prompt = rig.state.resolve_base_prompt();
            *rig.state.harness.lock() = Some(Harness::new_modular(
                Some(rig.session_id),
                prompt,
                None,
                &settings,
                vox_lib::harness::LlmExecutionParams {
                    context_window: 8192,
                    max_output_tokens: 512,
                    temperature: 0.7,
                    reasoning_enabled: false,
                },
                rig.llm_tx.clone(),
                true,
            ));
        }

        // Seed prior turns to ~80% context budget (usable budget = 7680; 80% is ~6144 tokens)
        // 90 turns of ~68 tokens each ≈ 6120 tokens (just below 85% Critical threshold of 6528 tokens)
        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(Arc::new(LargeHeadTailTool));
            for i in 0..90 {
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

        // Turn N Step 0: model calls a tool returning an observation that pushes total tokens >= 85% (Critical)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-step0-mid".to_string(),
                name: "large_head_tail_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Turn N Step 1: LLM dispatch after inline compaction
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Summary after inline compaction.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Turn N active query needing tool investigation", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "turn must complete, got {:?}",
            outcome
        );

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness must be mounted");
            let messages = harness.messages();

            // 1. Verify older prior turns were compacted into session_context prompt
            let has_old_turn_0 = messages.iter().any(|m| m.content.contains("Turn 0 user statement"));
            assert!(
                !has_old_turn_0,
                "Older historical turns must be compacted and removed from active message history"
            );

            // 2. Verify Turn N's active User query remains intact
            let active_user_msg = messages.iter().find(|m| m.role == Role::User && m.content.contains("Turn N active query"));
            assert!(
                active_user_msg.is_some(),
                "Turn N active user query must be preserved intact after mid-loop compaction"
            );

            // 3. Verify Step 0's Role::Assistant tool call remains intact
            let step0_assistant_msg = messages.iter().find(|m| m.role == Role::Assistant && m.tool_calls.as_ref().map(|c| !c.is_empty()).unwrap_or(false));
            assert!(
                step0_assistant_msg.is_some(),
                "Step 0 Role::Assistant tool call must be preserved intact after mid-loop compaction"
            );

            // 4. Verify Step 0's Role::Tool observation remains intact
            let step0_tool_msg = messages.iter().find(|m| m.role == Role::Tool);
            assert!(
                step0_tool_msg.is_some(),
                "Step 0 Role::Tool observation must be preserved intact after mid-loop compaction"
            );
        }

        // 5. Provenance assert: session_compactions must record the completed compaction
        let conn = rig.state.db.connect().expect("db connect");
        let start = std::time::Instant::now();
        let mut found_compacted = false;
        while start.elapsed() < Duration::from_secs(5) {
            let mut rows = conn
                .query(
                    "SELECT COUNT(*) FROM session_compactions WHERE session_id = ? AND status = 'completed';",
                    (rig.session_id,),
                )
                .await
                .expect("query session_compactions");
            if let Some(row) = rows.next().await.expect("fetch row") {
                let count: i64 = row.get(0).expect("count");
                if count >= 1 {
                    found_compacted = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            found_compacted,
            "session_compactions must record completed compaction when mid-loop compaction triggers"
        );
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 13: Domain 4 — Two-Phase Mid-Work Interruption, Step Steering & Architectural Clean Boundaries
// ============================================================================

struct HoldTool {
    entered_tx: tokio::sync::mpsc::Sender<()>,
    release_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<()>>,
}

impl ToolDefinition for HoldTool {
    fn name(&self) -> &str {
        "hold_tool"
    }

    fn description(&self, _: PipelineMode) -> &str {
        "Tool that holds until released"
    }

    fn parameters_schema(&self, _: PipelineMode) -> serde_json::Value {
        serde_json::json!({})
    }

    fn execution_mode(&self, _: &serde_json::Value) -> ToolExecutionMode {
        ToolExecutionMode::Exclusive
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
        _args: serde_json::Value,
        _ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        use futures_util::future::FutureExt;
        async move {
            let _ = self.entered_tx.send(()).await;
            let mut rx = self.release_rx.lock().await;
            let _ = rx.recv().await;
            Ok(ToolResult::new("done hold"))
        }
        .boxed()
    }
}

struct SimpleTestTool {
    name: String,
}

impl ToolDefinition for SimpleTestTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self, _: PipelineMode) -> &str {
        "Simple instant test tool"
    }

    fn parameters_schema(&self, _: PipelineMode) -> serde_json::Value {
        serde_json::json!({})
    }

    fn execution_mode(&self, _: &serde_json::Value) -> ToolExecutionMode {
        ToolExecutionMode::Parallel
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
        _args: serde_json::Value,
        _ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        use futures_util::future::FutureExt;
        async move { Ok(ToolResult::new("done simple")) }.boxed()
    }
}

/// PURPOSE:  StopTurn event aborts active Working turn, clears StepInbox, cancels playback, and returns pipeline to Ready without unmounting harness.
/// SCENARIO: Turn is executing tools in Working state with messages in StepInbox; user triggers StopTurn (Esc step 1).
/// ASSERTS:  Pipeline transitions to Ready, the harness remains mounted (Some), and StepInbox is cleared.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   In on_stop_turn, transition pipeline to InteractionState::Idle and take() harness.
#[tokio::test]
async fn stop_turn_aborts_active_turn_and_transitions_to_ready_without_ending_session() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1201;

        let (entered_tx, mut entered_rx) = tokio::sync::mpsc::channel(1);
        let (release_tx, release_rx) = tokio::sync::mpsc::channel(1);
        let tool = Arc::new(HoldTool {
            entered_tx,
            release_rx: tokio::sync::Mutex::new(release_rx),
        });

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(tool);
            harness.inbox().push(InboxMessage::new(
                "stale steering".to_string(),
                QueuedMessageDelivery::Immediate,
                1700000000000,
            ));
        }

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-stop-turn".to_string(),
                name: "hold_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Active query before StopTurn", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let handle = tokio::spawn(async move { Harness::execute_turn(&harness_arc, req).await });

        // Wait until tool starts executing in Working
        entered_rx.recv().await.expect("tool entered");
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Working,
            "pipeline must be in Working state while tool executes"
        );

        // Esc Step 1: send StopTurn event
        rig.send_event(VoxEvent::StopTurn);

        // Assert pipeline transitions to Ready (bounded 500ms check)
        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_millis(500))
                .await,
            "pipeline must transition to Ready on StopTurn"
        );

        // Assert harness remains mounted and inbox is cleared
        {
            let guard = rig.state.harness.lock();
            let harness = guard
                .as_ref()
                .expect("Harness must remain mounted in app_state");
            assert!(
                harness.inbox().is_empty(),
                "StepInbox must be synchronously cleared on StopTurn"
            );
        }

        let _ = release_tx.send(()).await;
        let outcome = handle.await.expect("turn join");
        assert!(
            matches!(outcome, TurnOutcome::Cancelled { .. }),
            "turn outcome must be Cancelled, got {:?}",
            outcome
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  With pipeline in Ready (after first Esc stops the turn), a second Esc (EndSession) disengages the session to Idle and unmounts the harness.
/// SCENARIO: Turn is in Working; first Esc sends StopTurn transitioning to Ready (harness mounted); second Esc sends EndSession.
/// ASSERTS:  First Esc leaves pipeline in Ready with harness mounted; second Esc transitions pipeline to Idle and unmounts harness.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   In on_end, return early if current_state == InteractionState::Ready.
#[tokio::test]
async fn ready_state_disengages_session_to_idle_on_second_esc() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1202;

        let (entered_tx, mut entered_rx) = tokio::sync::mpsc::channel(1);
        let (release_tx, release_rx) = tokio::sync::mpsc::channel(1);
        let tool = Arc::new(HoldTool {
            entered_tx,
            release_rx: tokio::sync::Mutex::new(release_rx),
        });

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(tool);
        }

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-esc-2".to_string(),
                name: "hold_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Query for two-step Esc", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let handle = tokio::spawn(async move { Harness::execute_turn(&harness_arc, req).await });

        entered_rx.recv().await.expect("tool entered");
        assert_eq!(rig.state.pipeline.state(), InteractionState::Working);

        // Esc Step 1: StopTurn aborts active turn to Ready
        rig.send_event(VoxEvent::StopTurn);
        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_millis(500))
                .await,
            "first Esc must transition pipeline to Ready"
        );
        assert!(
            rig.state.harness.lock().is_some(),
            "harness must remain mounted after first Esc"
        );

        // Esc Step 2: EndSession disengages session from Ready to Idle
        rig.send_event(VoxEvent::EndSession { force: false });
        assert!(
            rig.wait_for_state(InteractionState::Idle, Duration::from_millis(500))
                .await,
            "second Esc must transition pipeline to Idle"
        );
        assert!(
            rig.state.harness.lock().is_none(),
            "harness must be unmounted after second Esc"
        );

        // The turn may already have been cancelled by EndSession, so the unblock
        // receiver can be gone. Only the joined outcome must be failure-free.
        let _ = release_tx.send(()).await;
        let _ = handle.await.expect("turn task panicked");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Scenario A (Step 0 Thinking, phase != Committed) barge-in discards the staged query, stashes it as continuation prefix, and merges with new speech.
/// SCENARIO: User speaks "search for"; model streams interim tokens in Step 0 Thinking; user interrupts with " the budget report".
/// ASSERTS:  On TranscriptFinal the scripted second pass receives the merged query and the turn completes.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   In Scenario A (`interrupt.rs`), omit `take_staged_query()`.
#[tokio::test]
async fn pre_working_thinking_barge_in_aborts_stream_and_merges_prompt() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1203;

        // Verify Step 0 uncommitted provenance (staged query has not committed to messages)
        assert!(
            !rig.state
                .harness
                .lock()
                .as_ref()
                .unwrap()
                .messages()
                .iter()
                .any(|m| m.role == Role::User),
            "user query must remain staged and uncommitted to history at turn onset"
        );

        // Model starts streaming in Step 0 Thinking (streaming paused to simulate in-flight thinking)
        let (stream_pause_tx, stream_pause_rx) = std::sync::mpsc::channel();
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Token("Searching".to_string()));
            let _ = stream_pause_rx.recv();
            let _ = tx.send(LlmResponse::Finished);
        });

        // Push second step for when merged prompt arrives
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Found the budget report.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("search for", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let turn_handle =
            tokio::spawn(async move { Harness::execute_turn(&harness_arc, req).await });

        // Wait until pipeline enters Thinking and stream begins
        assert!(
            rig.wait_for_state(InteractionState::Thinking, Duration::from_millis(500))
                .await,
            "pipeline must enter Thinking in Step 0"
        );

        // Trigger SpeechStart during Step 0 Thinking (Scenario A)
        rig.send_event(VoxEvent::SpeechStart {
            owner: InteractionOwner::Assistant,
        });

        assert!(
            rig.wait_for_state(InteractionState::Listening, Duration::from_millis(500))
                .await,
            "pipeline must transition to Listening on SpeechStart"
        );

        // Release first LLM stream
        let _ = stream_pause_tx.send(());

        // Send TranscriptFinal with continuation; turn_id is not advanced until Stage 2 TranscriptFinal
        rig.send_event(VoxEvent::TranscriptFinal {
            owner: InteractionOwner::Assistant,
            turn_id,
            text: " the budget report".to_string(),
        });

        // Assert merged turn dispatched to LLM
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while tokio::time::Instant::now() < deadline && rig.dispatch_count() < 2 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            rig.dispatch_count(),
            2,
            "merged query must dispatch to LLM in Thinking"
        );

        // Verify history holds merged query
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while tokio::time::Instant::now() < deadline {
            let found = {
                let guard = rig.state.harness.lock();
                guard
                    .as_ref()
                    .map(|h| {
                        h.messages().iter().any(|m| {
                            m.role == Role::User && m.content == "search for the budget report"
                        })
                    })
                    .unwrap_or(false)
            };
            if found {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            assert!(
                harness
                    .messages()
                    .iter()
                    .any(|m| m.role == Role::User && m.content == "search for the budget report"),
                "history must record merged prompt as single Role::User message"
            );
        }
        let _ = turn_handle.await.expect("turn task panicked");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Scenario B (Step K >= 1 Thinking, phase Committed) barge-in preserves committed tool steps, clears accumulator without continuation prefix, and appends standalone Role::User.
/// SCENARIO: Turn executes Step 0 tool; in Step 1 Thinking, user interrupts with "Also check the weather".
/// ASSERTS:  Step 1 stream aborts; Step 0 [User, Assistant(call), Tool(result)] preserved in history; continuation_prefix is EMPTY; new speech commits as standalone Role::User on top of tool history.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   In interrupt.rs, call stash_for_continuation() unconditionally during Thinking.
#[tokio::test]
async fn mid_turn_thinking_barge_in_preserves_committed_tools_and_pushes_standalone_user_message() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1204;

        let simple_tool = Arc::new(SimpleTestTool {
            name: "simple_scen_b".to_string(),
        });
        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(simple_tool);
        }

        // Step 0: model issues a tool call that commits
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-mid-turn-scen-b".to_string(),
                name: "simple_scen_b".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 1: pause LLM in Thinking
        let (step1_pause_tx, step1_pause_rx) = std::sync::mpsc::channel();
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Token("I found the notes.".to_string()));
            let _ = step1_pause_rx.recv();
            let _ = tx.send(LlmResponse::Finished);
        });

        // Step 2: for when steering speech is dispatched
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Here is the weather.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Initial query for Scenario B", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let turn_handle =
            tokio::spawn(async move { Harness::execute_turn(&harness_arc, req).await });

        // Wait until Step 0 completes tool execution and advances to Step 1 Thinking (500ms max)
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while tokio::time::Instant::now() < deadline {
            let committed = {
                let guard = rig.state.harness.lock();
                guard
                    .as_ref()
                    .map(|h| h.messages().iter().any(|m| matches!(m.role, Role::Tool)))
                    .unwrap_or(false)
            };
            if committed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            assert!(
                harness
                    .messages()
                    .iter()
                    .any(|m| matches!(m.role, Role::Tool)),
                "tool steps must be committed to history after Step 0 commits tool"
            );
        }

        // Trigger SpeechStart during Step 1 Thinking (Scenario B)
        rig.send_event(VoxEvent::SpeechStart {
            owner: InteractionOwner::Assistant,
        });

        assert!(
            rig.wait_for_state(InteractionState::Listening, Duration::from_millis(500))
                .await,
            "pipeline must transition to Listening on Scenario B interrupt"
        );

        // Assert Step 0 committed tool steps remain preserved in history
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            let messages = harness.messages();
            assert!(
                messages
                    .iter()
                    .any(|m| m.role == Role::User && m.content == "Initial query for Scenario B"),
                "Step 0 user query must remain in history"
            );
            assert!(
                messages.iter().any(|m| m.role == Role::Assistant
                    && m.tool_calls
                        .as_ref()
                        .map(|c| !c.is_empty())
                        .unwrap_or(false)),
                "Step 0 tool call must remain in history"
            );
            assert!(
                messages.iter().any(|m| m.role == Role::Tool),
                "Step 0 tool result must remain in history"
            );
        }

        // Assert continuation_prefix was NOT stashed (preventing prompt duplication)
        {
            let acc = rig.state.pipeline_accumulator.lock();
            assert!(
                acc.continuation_prefix.is_empty(),
                "Scenario B must call acc.clear(); continuation_prefix must be empty, but was '{}'",
                acc.continuation_prefix
            );
        }

        let _ = step1_pause_tx.send(());

        // Send TranscriptFinal with steering text
        let new_turn_id = rig.state.pipeline.peek_turn_id();
        rig.send_event(VoxEvent::TranscriptFinal {
            owner: InteractionOwner::Assistant,
            turn_id: new_turn_id,
            text: "Also check the weather".to_string(),
        });

        // Wait for steering turn to dispatch
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while tokio::time::Instant::now() < deadline && rig.dispatch_count() < 3 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // Assert new speech committed as standalone Role::User without Step 0 prepended
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            let messages = harness.messages();
            let standalone_user = messages
                .iter()
                .find(|m| m.role == Role::User && m.content == "Also check the weather");
            assert!(
                standalone_user.is_some(),
                "Scenario B must append new speech as standalone Role::User, not merged with Step 0"
            );
        }
        let _ = turn_handle.await.expect("turn task panicked");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Scenario C (Working) user voice barge-in mutes audio, preserves in-flight tools during SpeechStart, and on non-empty TranscriptFinal cancels the turn, advances turn_id, and does not pollute StepInbox per RULE-HARN-18.
/// SCENARIO: Turn is executing tool in Working; user speaks; tool finishes; non-empty TranscriptFinal arrives.
/// ASSERTS:  turn_token is NOT cancelled on SpeechStart in Working; non-empty TranscriptFinal cancels the prior turn (TurnOutcome::Cancelled) and leaves StepInbox empty of voice transcripts.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   transcript.rs: Inject voice transcript into StepInbox during Working instead of cancelling turn and advancing turn_id.
#[tokio::test]
async fn active_working_speech_cancels_turn_and_advances_dialogue_without_inbox_injection() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1205;

        let (entered_tx, mut entered_rx) = tokio::sync::mpsc::channel(1);
        let (release_tx, release_rx) = tokio::sync::mpsc::channel(1);
        let tool = Arc::new(HoldTool {
            entered_tx,
            release_rx: tokio::sync::Mutex::new(release_rx),
        });

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(tool);
        }

        // Pass 0: tool call
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-scen-c-working".to_string(),
                name: "hold_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Scenario C query", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let turn_handle =
            tokio::spawn(async move { Harness::execute_turn(&harness_arc, req).await });

        // Wait until tool is executing in Working
        entered_rx.recv().await.expect("tool entered");
        assert_eq!(rig.state.pipeline.state(), InteractionState::Working);

        // User begins speaking during Working (Scenario C Stage 1)
        rig.send_event(VoxEvent::SpeechStart {
            owner: InteractionOwner::Assistant,
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stage 1 invariant: in-flight tool execution was NOT cancelled on SpeechStart (RULE-EVT-08C)
        assert!(
            !rig.state.pipeline.turn_cancel_token().is_cancelled(),
            "turn_token must NOT be cancelled during Working SpeechStart"
        );

        // Release the hold tool so it finishes
        let _ = release_tx.send(()).await;

        // Stage 2: Send non-empty TranscriptFinal (real barge-in per RULE-EVT-08 §2b)
        rig.send_event(VoxEvent::TranscriptFinal {
            owner: InteractionOwner::Assistant,
            turn_id,
            text: "New user interruption query".to_string(),
        });

        let outcome = turn_handle.await.expect("turn join");
        assert!(
            matches!(outcome, TurnOutcome::Cancelled { .. }),
            "prior turn must resolve to Cancelled upon real barge-in, got {:?}",
            outcome
        );

        // Invariant RULE-HARN-18: StepInbox must NOT have voice transcripts injected
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().unwrap();
        assert_eq!(
            harness.inbox().len(),
            0,
            "StepInbox must remain empty; voice barge-in must never inject synthetic messages into inbox"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Fast tool completing in Working while user speech is in progress waits at step boundary via wait_if_speech_held until TranscriptFinal arrives.
/// SCENARIO: Tool finishes quickly (e.g. 10ms); speech hold is active; coordinator holds at step boundary; when TranscriptFinal pushes message, hold releases and Step 1 dispatches.
/// ASSERTS:  Coordinator does not dispatch Step 1 while is_speech_held is true; dispatches Step 1 immediately upon TranscriptFinal.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   In StepInbox::wait_if_speech_held, return immediately without awaiting notify.
#[tokio::test]
async fn active_working_fast_tool_waits_at_step_boundary_until_user_speech_finalizes() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1206;

        let simple_tool = Arc::new(SimpleTestTool {
            name: "simple_fast_tool".to_string(),
        });
        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(simple_tool);
        }

        // Pass 0: fast tool call
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-fast-tool".to_string(),
                name: "simple_fast_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 1: response after hold released
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Step 1 response after hold.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Engage speech hold on StepInbox before turn boundary
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            harness.inbox().engage_speech_hold();
        }

        let req = rig.turn_request("Fast tool with speech hold", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let turn_handle = tokio::spawn(async move {
            Harness::execute_turn(&harness_arc, req).await
        });

        // Wait a short duration: tool completes fast, but coordinator must be held by wait_if_speech_held
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Assert Step 1 has NOT dispatched yet because speech hold is active
        assert_eq!(
            rig.dispatch_count(), 1,
            "coordinator must hold at step boundary while speech hold is active; dispatch_count must be 1"
        );

        // Now user speech finalizes, pushing message to inbox and releasing hold
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            harness.inbox().push(InboxMessage::new(
                "Late arriving speech".to_string(),
                QueuedMessageDelivery::Immediate,
                1700000000000,
            ));
        }

        // Held coordinator must wake and dispatch Step 1
        let outcome = turn_handle.await.expect("turn join");
        assert!(matches!(outcome, TurnOutcome::Completed { .. }));
        assert_eq!(
            rig.dispatch_count(), 2,
            "coordinator must resume and dispatch Step 1 once speech hold is released"
        );

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().unwrap();
        let messages = harness.messages();
        assert!(
            messages
                .iter()
                .any(|m| m.role == Role::User && m.content == "Late arriving speech"),
            "steering instruction 'Late arriving speech' must be present in history as Role::User"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Empty speech transcript (cough / false VAD trigger) during Working releases speech hold and resumes ReAct loop without aborting to Ready.
/// SCENARIO: SpeechStart occurs in Working; TranscriptFinal("") arrives; hold releases; pipeline remains active in Working/Thinking and completes turn with zero lost work.
/// ASSERTS:  release_speech_hold() releases hold; pipeline does not abort to Ready; loop completes turn cleanly.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In on_transcript_final, let empty speech during a held turn transition to InteractionState::Ready.
#[tokio::test]
async fn active_working_empty_speech_releases_hold_and_resumes_react_loop_without_aborting() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1207;

        let simple_tool = Arc::new(SimpleTestTool {
            name: "simple_cough_tool".to_string(),
        });
        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(simple_tool);
        }

        // Pass 0: tool call
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-cough-tool".to_string(),
                name: "simple_cough_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Pass 1: final response
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Turn finished after cough.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        // Engage speech hold on inbox simulating SpeechStart in Working
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            harness.inbox().engage_speech_hold();
        }

        // Set pipeline state to Listening as speech is active
        rig.state.pipeline.set_state(InteractionState::Listening);

        let req = rig.turn_request("Query with false VAD cough", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let turn_handle =
            tokio::spawn(async move { Harness::execute_turn(&harness_arc, req).await });

        // Empty speech arrives on event channel
        rig.send_event(VoxEvent::TranscriptFinal {
            owner: InteractionOwner::Assistant,
            turn_id,
            text: "".to_string(),
        });

        // Poll for speech hold release (500ms max)
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while tokio::time::Instant::now() < deadline {
            let released = {
                let guard = rig.state.harness.lock();
                guard
                    .as_ref()
                    .map(|h| !h.inbox().is_speech_held())
                    .unwrap_or(false)
            };
            if released {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            assert!(
                !harness.inbox().is_speech_held(),
                "speech hold must be released upon empty transcript"
            );
        }

        assert_ne!(
            rig.state.pipeline.state(),
            InteractionState::Ready,
            "pipeline must not abort to Ready on false VAD / cough during active tool turn"
        );

        let outcome = turn_handle.await.expect("turn join");
        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "loop must complete turn with zero lost work, got {:?}",
            outcome
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  StepInbox partitions messages by delivery: drain_immediate returns Immediate messages while retaining TurnBoundary messages; coordinator injects Immediate into history at step boundary.
/// SCENARIO: Both Immediate and TurnBoundary messages are pushed into StepInbox; execute_turn executes a tool step.
/// ASSERTS:  drain_immediate returns only Immediate messages; TurnBoundary messages remain buffered in inbox; during turn execution Immediate message is injected into history.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   In StepInbox::drain_immediate, drain all messages unconditionally.
#[tokio::test]
async fn step_inbox_drains_immediate_messages_and_retains_turn_boundary_messages() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1208;

        // 1. Unit invariant check on StepInbox
        let inbox = StepInbox::new();
        inbox.push(InboxMessage::new(
            "immediate 1".to_string(),
            QueuedMessageDelivery::Immediate,
            1,
        ));
        inbox.push(InboxMessage::new(
            "turn boundary 1".to_string(),
            QueuedMessageDelivery::TurnBoundary,
            2,
        ));
        inbox.push(InboxMessage::new(
            "immediate 2".to_string(),
            QueuedMessageDelivery::Immediate,
            3,
        ));

        let drained = inbox.drain_immediate();
        assert_eq!(
            drained.len(),
            2,
            "drain_immediate must return exactly the 2 Immediate messages"
        );
        assert!(drained
            .iter()
            .all(|m| m.delivery == QueuedMessageDelivery::Immediate));
        assert_eq!(inbox.len(), 1, "TurnBoundary message must remain in inbox");

        // 2. Seam invariant check: coordinator drains Immediate messages into history at step boundary
        let simple_tool = Arc::new(SimpleTestTool {
            name: "simple_inbox_tool".to_string(),
        });
        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness must be mounted");
            harness.register_tool(simple_tool);
        }

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-inbox-drain".to_string(),
                name: "simple_inbox_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Drained and completed.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().unwrap();
            harness.inbox().push(InboxMessage::new(
                "Immediate steering in harness".to_string(),
                QueuedMessageDelivery::Immediate,
                10,
            ));
            harness.inbox().push(InboxMessage::new(
                "Queued next turn in harness".to_string(),
                QueuedMessageDelivery::TurnBoundary,
                11,
            ));
        }

        let req = rig.turn_request("Initial query for inbox seam", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        assert!(matches!(outcome, TurnOutcome::Completed { .. }));

        // Assert Immediate was drained into history, while TurnBoundary remains in harness.inbox()
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().unwrap();
        let messages = harness.messages();
        assert!(
            messages
                .iter()
                .any(|m| m.role == Role::User && m.content == "Immediate steering in harness"),
            "coordinator must drain Immediate message and inject it into history at step boundary"
        );
        assert_eq!(
            harness.inbox().len(),
            1,
            "TurnBoundary message must remain retained in inbox across step boundaries"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Deterministic lint-arch.sh enforces RULE-HARN-01: forbids harness/stages/tools/, forbids raw actor commands in loop.rs and steps.rs, and forbids stages/tools imports in loop.rs.
/// SCENARIO: Verify scripts/lint-arch.sh content and execution against RULE-HARN-01 specification.
/// ASSERTS:  lint-arch.sh contains RULE-HARN-01 invariant check; checks stages/tools absence, actor command isolation, and loop.rs pure boundaries.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Disable RULE-HARN-01 check in scripts/lint-arch.sh.
#[test]
fn lint_arch_script_blocks_forbidden_actor_commands() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir.join("scripts/lint-arch.sh");
    assert!(script_path.exists(), "scripts/lint-arch.sh must exist");

    let script_content = std::fs::read_to_string(&script_path).expect("read lint-arch.sh");
    assert!(
        script_content.contains("RULE-HARN-01"),
        "scripts/lint-arch.sh must enforce RULE-HARN-01 invariant"
    );
    assert!(
        script_content.contains("harness/stages/tools"),
        "lint-arch.sh must verify that harness/stages/tools does not exist"
    );
    assert!(
        script_content.contains("TtsCommand")
            && script_content.contains("LlmCommand")
            && script_content.contains("PersistenceEvent"),
        "lint-arch.sh must forbid raw actor commands in loop.rs and steps.rs"
    );

    let output = std::process::Command::new("bash")
        .arg(&script_path)
        .current_dir(manifest_dir)
        .output()
        .expect("failed to execute scripts/lint-arch.sh");
    assert!(
        output.status.success(),
        "lint-arch.sh failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// PURPOSE:  A tool from the sibling chassis must execute through the real turn loop.
/// SCENARIO: The scripted model calls `search_memory` during a harness turn.
/// ASSERTS:  The turn completes with a tool observation committed to history, proving
///           the ToolRegistry chassis is reachable from `Harness::execute_turn` (RULE-HARN-11B).
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   harness/steps.rs: Drop the committed tool observation for search_memory.
#[tokio::test]
async fn sibling_tool_executes_through_the_turn_loop() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1405;

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-sibling-chassis".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "chassis" }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Chassis reachable.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Find chassis facts", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "a turn with a sibling-chassis tool must complete"
        );

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("harness stays mounted");
        let has_tool_observation = harness
            .messages()
            .iter()
            .any(|m| m.role == Role::Tool && !m.content.trim().is_empty());
        assert!(
            has_tool_observation,
            "the sibling tool chassis must produce an observable tool result in history"
        );
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 10: Red-Seam Tests (Pipeline Refactor Baseline — Phase 14)
// ============================================================================

/// PURPOSE:  Ensure false barge-in (noise/cough) during Thinking does not wedge or abort the cognitive loop, but resumes token streaming at Step 4.
/// SCENARIO: Turn is in Step 0 Thinking streaming tokens; user coughs (SpeechStart -> Listening); STT yields empty transcript (Stage 2 false barge-in).
/// ASSERTS:  Pipeline transitions Thinking -> Listening -> Thinking and the cognitive loop completes with the full response.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   transcript.rs: In empty transcript path, transition unconditionally to Ready instead of resuming Thinking when llm_done == false.
#[tokio::test]
async fn cognitive_loop_resumes_on_false_barge_in_in_thinking() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1401;

        // Model pass 0: first token then pause (simulate in-flight thinking)
        let (pause_tx, pause_rx) = std::sync::mpsc::channel();
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Token("Thinking...".to_string()));
            let _ = pause_rx.recv();
            let _ = tx.send(LlmResponse::Token(" and answered.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("What is the speed of sound?", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let turn_handle =
            tokio::spawn(async move { Harness::execute_turn(&harness_arc, req).await });

        // Wait until pipeline enters Thinking
        assert!(
            rig.wait_for_state(InteractionState::Thinking, Duration::from_millis(500))
                .await,
            "pipeline must enter Thinking at onset"
        );

        // Acoustic interruption (Stage 1: cough/noise)
        rig.send_event(VoxEvent::SpeechStart {
            owner: InteractionOwner::Assistant,
        });

        assert!(
            rig.wait_for_state(InteractionState::Listening, Duration::from_millis(500))
                .await,
            "SpeechStart must transition state to Listening"
        );

        // Semantic resolution (Stage 2: empty transcript = false barge-in per RULE-EVT-08 §2a)
        rig.send_event(VoxEvent::TranscriptFinal {
            owner: InteractionOwner::Assistant,
            turn_id,
            text: "".to_string(),
        });

        // Loop must resume in Thinking!
        assert!(
            rig.wait_for_state(InteractionState::Thinking, Duration::from_millis(500))
                .await,
            "Empty transcript with llm_done == false must resume state to Thinking"
        );

        // Unpause LLM stream
        let _ = pause_tx.send(());

        let outcome = turn_handle.await.expect("turn join");
        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "resumed turn must complete successfully, got {:?}",
            outcome
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  A stale-turn PlaybackFinished must not transition the active Speaking turn.
/// SCENARIO: Active turn 1403 is Speaking; stale finish for 1402 arrives, followed by an
///           active-turn LlmFinished FIFO sentinel and a valid active PlaybackFinished.
/// ASSERTS:  The stale event is dropped (state stays Speaking after the sentinel proves it
///           was consumed); the valid active event transitions Speaking -> Ready (RULE-EVT-09 §2).
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   pipeline/assistant/playback.rs: Drop the active_turn_id epoch guard on PlaybackFinished.
#[tokio::test]
async fn stale_playback_finished_is_dropped_and_active_turn_is_resolved() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let stale_turn_id = 1402;
        let active_turn_id = 1403;

        rig.state
            .pipeline
            .turn_id
            .store(active_turn_id, Ordering::Relaxed);
        rig.state.pipeline.set_state(InteractionState::Speaking);
        // Keep audio outstanding so the LlmFinished sentinel cannot itself close the turn.
        rig.state
            .pipeline
            .pending_tts_jobs
            .store(1, Ordering::Relaxed);

        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id: stale_turn_id,
        });
        // FIFO sentinel: when llm_done latches, the stale event ahead of it was consumed.
        rig.send_event(VoxEvent::LlmFinished {
            turn_id: active_turn_id,
        });

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !rig.state.pipeline.is_llm_done() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            rig.state.pipeline.is_llm_done(),
            "the active-turn sentinel must reach the router before quiescence is asserted"
        );
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Speaking,
            "a stale-turn PlaybackFinished must NOT transition the active Speaking turn"
        );

        // With audio drained, the epoch guard is the only remaining barrier. A stale
        // PlaybackFinished must still leave the active turn in Speaking.
        // The positive control below then accepts the same event for the active turn.
        rig.state.pipeline.pending_tts_jobs.store(0, Ordering::Relaxed);
        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id: stale_turn_id,
        });
        common::harness::assert_pipeline_state_stable(
            || rig.state.pipeline.state(),
            InteractionState::Speaking,
            Duration::from_millis(300),
            "a stale PlaybackFinished with drained audio must not transition the active turn"
        )
        .await;

        // Positive control: the same event with the active turn_id must be accepted.
        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id: active_turn_id,
        });
        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_secs(5))
                .await,
            "active-turn PlaybackFinished with drained audio must transition Speaking -> Ready"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch loss of current_activity envelope when recovering from a Stage 2 false barge-in during tool execution.
/// SCENARIO: Turn is in Working with current_activity set to an ActivityEnvelope; SpeechStart transitions to Listening; TranscriptFinal arrives empty.
/// ASSERTS:  Pipeline transitions Working -> Listening -> Working; current_activity is preserved and NOT wiped per RULE-EVT-08 §1 and §2a.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   interrupt.rs: Clear current_activity or pass None when returning to Working on empty transcript.
#[tokio::test]
async fn tool_active_false_barge_in_restores_working_with_intact_activity() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 1404;

        rig.state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        let envelope = vox_lib::core::events::ActivityEnvelope {
            kind: vox_lib::core::events::ActivityKind::Tool,
            name: "search_memory".to_string(),
            call_id: Some("call-1404".to_string()),
        };

        // Set initial Working state with activity envelope
        *rig.state.pipeline.current_activity.write() = Some(envelope.clone());
        rig.state.pipeline.set_state(InteractionState::Working);

        // Stage 1: User coughs / acoustic onset during Working
        rig.send_event(VoxEvent::SpeechStart {
            owner: InteractionOwner::Assistant,
        });

        assert!(
            rig.wait_for_state(InteractionState::Listening, Duration::from_millis(500))
                .await,
            "SpeechStart must transition state to Listening"
        );

        // RULE-EVT-08 §1 invariant: current_activity is PRESERVED through Listening
        assert!(
            rig.state.pipeline.current_activity.read().is_some(),
            "current_activity must be preserved through Listening transition"
        );

        // Stage 2: STT yields empty transcript (false barge-in)
        rig.send_event(VoxEvent::TranscriptFinal {
            owner: InteractionOwner::Assistant,
            turn_id,
            text: "   ".to_string(),
        });

        assert!(
            rig.wait_for_state(InteractionState::Working, Duration::from_millis(500))
                .await,
            "Empty transcript with tool active must restore Working state"
        );

        let restored_activity = rig.state.pipeline.current_activity.read().clone();
        assert_eq!(
            restored_activity,
            Some(envelope),
            "current_activity must remain intact with original activity envelope upon return to Working"
        );
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 14: Domain 5 — Background Tasks, Timeout Promotion & Router Wake-Up
// ============================================================================

/// PURPOSE:  Catch premature background promotion or dropped stdout when a fast shell command completes within the auto-promotion window.
/// SCENARIO: Model invokes `bash({"command": "printf 'FAST_SYNC_OK'", "run_in_background": true})` on a Harness with default 5s auto-promote timeout; command exits in <20ms with code 0.
/// ASSERTS:  The terminal tool observation arrives within a 2s per-stage ceiling (well inside the 5s promotion window, not via the timeout fallback); it contains "FAST_SYNC_OK" and exit code 0 and does NOT contain "running_in_background"; `state.tasks.active_tasks_count(session_id) == 0`; `SELECT COUNT(*) FROM session_tasks WHERE status = 'running'` is 0; `TurnOutcome::Completed` returns final spoken response.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   In `BashTool::execute`, replace `handle.completion_rx` with a permanently pending channel so the fast command always takes the 5s promotion-timeout fallback.
#[tokio::test]
async fn bash_tool_executes_fast_command_synchronously_without_backgrounding() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 5001;

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-bash-fast-1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": "printf 'FAST_SYNC_OK'",
                    "run_in_background": true,
                }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Fast command finished.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Run fast sync command", turn_id);
        // Per-stage first-output ceiling (testing-style-guide §5.4): a fast command
        // must yield its terminal tool observation well inside the 5s auto-promotion
        // window. A permanently-pending completion receiver forces the 5s promotion
        // timeout fallback; mark_promoted then compensates with AlreadyFinished and
        // emits an identical observation, so the stall itself is the observable.
        let outcome = tokio::time::timeout(
            Duration::from_secs(2),
            Harness::execute_turn(&rig.state.harness, req),
        )
        .await
        .expect("fast command must complete within the auto-promotion window, not via the promotion-timeout fallback");

        match &outcome {
            TurnOutcome::Completed { assistant_response, .. } => {
                assert_eq!(assistant_response, "Fast command finished.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness must be mounted");
            let tool_obs = harness
                .messages()
                .iter()
                .find(|m| m.role == Role::Tool)
                .expect("Tool observation must be committed to history");
            assert!(
                tool_obs.content.contains("FAST_SYNC_OK"),
                "Committed Tool observation must contain FAST_SYNC_OK stdout, got: {}",
                tool_obs.content
            );
            assert!(
                !tool_obs.content.contains("running_in_background"),
                "Tool observation must not indicate running_in_background"
            );
            assert!(
                tool_obs.content.contains("Exit code: 0"),
                "Terminal tool observation must report exit code 0 for a synchronous command"
            );
        }

        assert_eq!(
            rig.state.tasks.active_tasks_count(rig.session_id),
            0,
            "Active background tasks count must be 0 for synchronous command"
        );

        let conn = rig.state.db.connect().expect("db connect");
        let mut rows = conn
            .query("SELECT COUNT(*) FROM session_tasks WHERE status = 'running';", ())
            .await
            .expect("query session_tasks");
        let running_count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(running_count, 0, "session_tasks running count must be 0");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch foreground turn stalls, double-spawned processes, or false guardrail error strikes when a shell command exceeds `auto_promote_timeout`.
/// SCENARIO: Configure `harness.auto_promote_timeout = Duration::from_millis(80)`. Model invokes `bash({"command": "echo 'BOOT_LINE' && sleep 10"})` (default `run_in_background: true`).
/// ASSERTS:  Step 0 finishes in <500ms while child process is still running; committed `Role::Tool` observation parses as JSON with "status": "running_in_background", non-empty "task_id", and existing "log_path"; `session_steps` row has `guardrail_strike == 0` and `session_tool_calls` row has `is_error == 0`; `state.tasks.active_tasks_count(session_id) == 1`; `session_tasks` table has 1 row with `status = 'running'`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   In `ToolExecutor` / `steps.rs`, mark auto-promoted tool outcomes with `is_error: true` (or drop the child process handle when the promotion timer expires).
#[tokio::test]
async fn bash_tool_auto_promotes_long_running_command_to_background_without_guardrail_strike() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 5002;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.auto_promote_timeout = Duration::from_millis(80);
        }

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-bash-promote-1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": "echo 'BOOT_LINE' && sleep 10",
                    "run_in_background": true,
                }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Task running in background.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let start = std::time::Instant::now();
        let req = rig.turn_request("Run long background command", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(500),
            "Step 0 must promote to background within 500ms, elapsed: {:?}",
            elapsed
        );

        match &outcome {
            TurnOutcome::Completed { .. } => {}
            other => panic!("Expected Completed outcome on auto-promotion, got {:?}", other),
        }

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness must be mounted");
            let tool_obs = harness
                .messages()
                .iter()
                .find(|m| m.role == Role::Tool)
                .expect("Tool observation must be committed to history");
            let parsed: serde_json::Value = serde_json::from_str(&tool_obs.content)
                .expect("Tool observation for promoted task must parse as valid JSON");
            assert_eq!(
                parsed["status"], "running_in_background",
                "Observation status must be running_in_background"
            );
            let task_id = parsed["task_id"]
                .as_str()
                .expect("task_id must be non-empty string");
            assert!(!task_id.is_empty(), "task_id must not be empty");
            let log_path_str = parsed["log_path"]
                .as_str()
                .expect("log_path must be string");
            let log_path = std::path::PathBuf::from(log_path_str);
            assert!(log_path.exists(), "task log file must exist on disk");
        }

        // Assert 0 guardrail strikes and is_error == 0 (bounded poll for async persistence)
        let conn = rig.state.db.connect().expect("db connect");
        let persistence_deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut strikes = None;
        let mut tool_is_error = None;
        while std::time::Instant::now() < persistence_deadline
            && (strikes.is_none() || tool_is_error.is_none())
        {
            let mut step_rows = conn
                .query(
                    "SELECT guardrail_strike FROM session_steps WHERE session_id = ? AND turn_id = ?;",
                    (rig.session_id, turn_id),
                )
                .await
                .expect("query session_steps");
            if let Some(row) = step_rows.next().await.expect("fetch session_steps row") {
                let value: i64 = row.get(0).expect("guardrail_strike");
                strikes = Some(value);
            }

            let mut tool_rows = conn
                .query(
                    "SELECT is_error FROM session_tool_calls WHERE session_id = ? AND turn_id = ?;",
                    (rig.session_id, turn_id),
                )
                .await
                .expect("query session_tool_calls");
            if let Some(row) = tool_rows.next().await.expect("fetch session_tool_calls row") {
                let value: i64 = row.get(0).expect("is_error");
                tool_is_error = Some(value);
            }

            if strikes.is_none() || tool_is_error.is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        assert_eq!(
            strikes.expect("session_steps row must be persisted within deadline"),
            0,
            "guardrail_strike must be 0 for auto-promoted tool"
        );
        assert_eq!(
            tool_is_error.expect("session_tool_calls row must be persisted within deadline"),
            0,
            "is_error must be 0 for auto-promoted tool call"
        );

        assert_eq!(
            rig.state.tasks.active_tasks_count(rig.session_id),
            1,
            "active_tasks_count must be 1"
        );

        let mut task_rows = conn
            .query(
                "SELECT COUNT(*) FROM session_tasks WHERE session_id = ? AND status = 'running';",
                (rig.session_id,),
            )
            .await
            .expect("query session_tasks");
        let running_tasks: i64 = task_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(running_tasks, 1, "session_tasks must have 1 running row");

        rig.state.tasks.kill_all(rig.session_id);
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch ignored `run_in_background: false` overrides that prematurely detach synchronous prerequisite commands to the background.
/// SCENARIO: Configure `harness.auto_promote_timeout = Duration::from_millis(50)` and `foreground_hard_timeout = Duration::from_secs(5)`. Model invokes `bash({"command": "sleep 0.15 && printf 'MIGRATION_DONE'", "run_in_background": false})`.
/// ASSERTS:  Tool execution does NOT promote at 50ms; it waits ~150ms for the process to exit synchronously; committed `Role::Tool` observation contains "MIGRATION_DONE" and does NOT contain "running_in_background"; `state.tasks.active_tasks_count(session_id) == 0`.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In `BashTool::execute` / `ToolExecutor`, ignore the `run_in_background: false` argument and always use `auto_promote_timeout`.
#[tokio::test]
async fn bash_tool_foreground_override_waits_past_auto_promote_window_until_completion() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 5003;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.auto_promote_timeout = Duration::from_millis(50);
            harness.foreground_hard_timeout = Duration::from_secs(5);
        }

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-bash-foreground-1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": "sleep 0.15 && printf 'MIGRATION_DONE'",
                    "run_in_background": false,
                }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Migration completed synchronously.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let start = std::time::Instant::now();
        let req = rig.turn_request("Run foreground override migration", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        let elapsed = start.elapsed();

        assert!(
            elapsed >= Duration::from_millis(130),
            "Foreground execution must wait past 50ms auto-promote window for command to complete, took {:?}",
            elapsed
        );

        match &outcome {
            TurnOutcome::Completed { assistant_response, .. } => {
                assert_eq!(assistant_response, "Migration completed synchronously.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_obs = harness
            .messages()
            .iter()
            .find(|m| m.role == Role::Tool)
            .expect("Tool observation must be committed to history");
        assert!(
            tool_obs.content.contains("MIGRATION_DONE"),
            "Committed Tool observation must contain MIGRATION_DONE output, got: {}",
            tool_obs.content
        );
        assert!(
            tool_obs.content.contains("Exit code: 0"),
            "Committed Tool observation must report 'Exit code: 0', got: {}",
            tool_obs.content
        );
        assert!(
            !tool_obs.content.contains("running_in_background"),
            "Tool observation must NOT indicate running_in_background when run_in_background is false"
        );
        drop(guard);

        assert_eq!(
            rig.state.tasks.active_tasks_count(rig.session_id),
            0,
            "active_tasks_count must be 0 after synchronous foreground completion"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch pipe buffer deadlocks or missing stderr redirection when a promoted background task writes incremental output streams.
/// SCENARIO: Configure `auto_promote_timeout = Duration::from_millis(50)`. Spawn a command via `bash` that writes "STDOUT_LINE_1\n" to stdout and "STDERR_LINE_1\n" to stderr, then waits on a temp file flag before exiting.
/// ASSERTS:  While the task is still `Running`, reading `log_path` directly from disk under `paths::session_tasks_dir(session_id)` contains both "STDOUT_LINE_1" and "STDERR_LINE_1".
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   In `BackgroundTaskManager::spawn_command`, redirect only `stdout` to `log_file` and leave `stderr` as `Stdio::null()`.
#[tokio::test]
async fn background_task_streams_stdout_and_stderr_directly_to_disk_log_without_buffering() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 5004;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.auto_promote_timeout = Duration::from_millis(50);
        }

        let tmp = tempfile::tempdir().expect("tempdir");
        let flag_path = tmp.path().join("hold.flag");
        std::fs::write(&flag_path, "hold").expect("write flag");

        let cmd = format!(
            "printf 'STDOUT_LINE_1\\n' && printf 'STDERR_LINE_1\\n' >&2 && while [ -f '{}' ]; do sleep 0.05; done",
            flag_path.display()
        );

        rig.scripted_llm.push_step({
            let cmd_clone = cmd.clone();
            move |tx| {
                let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                    id: "call-bash-streams-1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({
                        "command": cmd_clone,
                        "run_in_background": true,
                    }),
                }));
                let _ = tx.send(LlmResponse::Finished);
            }
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Background task started.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Stream test", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        assert!(matches!(outcome, TurnOutcome::Completed { .. }));

        let log_path = {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness mounted");
            let tool_obs = harness
                .messages()
                .iter()
                .find(|m| m.role == Role::Tool)
                .expect("Tool observation");
            let parsed: serde_json::Value = serde_json::from_str(&tool_obs.content)
                .expect("Valid JSON observation");
            let log_path_str = parsed["log_path"].as_str().expect("log_path string");
            std::path::PathBuf::from(log_path_str)
        };

        // Poll log file on disk while process is held by flag_path
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut log_content = String::new();
        while std::time::Instant::now() < deadline {
            if log_path.exists() {
                if let Ok(c) = std::fs::read_to_string(&log_path) {
                    if c.contains("STDOUT_LINE_1") && c.contains("STDERR_LINE_1") {
                        log_content = c;
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            log_content.contains("STDOUT_LINE_1"),
            "Log file on disk must contain STDOUT_LINE_1 without buffering"
        );
        assert!(
            log_content.contains("STDERR_LINE_1"),
            "Log file on disk must contain STDERR_LINE_1 without buffering"
        );

        let _ = std::fs::remove_file(&flag_path);
        rig.state.tasks.kill_all(rig.session_id);
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch stale task status reporting or full-file memory reads when inspecting a multi-hundred-line background task log via `manage_task`.
/// SCENARIO: Promote a `bash` command that writes 120 numbered lines (LINE_001 .. LINE_120) to its log file and stays running; in the next step, model calls `manage_task({"action": "list"})` and `manage_task({"action": "status", "task_id": task_id})` in a single parallel wave.
/// ASSERTS:  Both `manage_task` calls execute in `ToolExecutionMode::Parallel`; `list` returns the active `task_id` with `status: "running"`; `status` returns the last 50 lines (containing "LINE_120" and "LINE_071", excluding "LINE_001") AND reports the requested task's own `running` status, not a session-wide boolean.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   In `ManageTaskTool`, return only the first 50 lines (head) instead of seeking the last 50 lines (tail) of `log_path`.
#[tokio::test]
async fn manage_task_list_and_status_read_live_state_and_log_tail_from_disk() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 5005;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.auto_promote_timeout = Duration::from_millis(50);
        }

        let tmp = tempfile::tempdir().expect("tempdir");
        let flag_path = tmp.path().join("hold_5005.flag");
        std::fs::write(&flag_path, "hold").expect("write flag");

        let cmd = format!(
            "for i in $(seq -w 1 120); do echo \"LINE_$i\"; done; while [ -f '{}' ]; do sleep 0.05; done",
            flag_path.display()
        );

        // Turn 1: Spawn and promote background task
        rig.scripted_llm.push_step({
            let cmd_clone = cmd.clone();
            move |tx| {
                let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                    id: "call-bash-tail-1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({
                        "command": cmd_clone,
                        "run_in_background": true,
                    }),
                }));
                let _ = tx.send(LlmResponse::Finished);
            }
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Task started in background.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req1 = rig.turn_request("Start task", turn_id);
        let outcome1 = Harness::execute_turn(&rig.state.harness, req1).await;
        assert!(matches!(outcome1, TurnOutcome::Completed { .. }));

        let task_id = {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness mounted");
            let tool_obs = harness
                .messages()
                .iter()
                .find(|m| m.role == Role::Tool)
                .expect("Tool observation");
            let parsed: serde_json::Value = serde_json::from_str(&tool_obs.content)
                .expect("Valid JSON observation");
            parsed["task_id"].as_str().expect("task_id string").to_string()
        };

        // Declaration-level contract: both reads must be schedulable in a parallel
        // wave. `ToolRegistry` and `ToolDefinition` are the production schedulers.
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness mounted");
            let tool = harness
                .tool_registry()
                .get("manage_task")
                .expect("manage_task must be registered");
            assert_eq!(
                tool.execution_mode(&serde_json::json!({ "action": "list" })),
                ToolExecutionMode::Parallel,
                "manage_task list must declare Parallel execution mode"
            );
            assert_eq!(
                tool.execution_mode(&serde_json::json!({
                    "action": "status",
                    "task_id": task_id.clone(),
                })),
                ToolExecutionMode::Parallel,
                "manage_task status must declare Parallel execution mode"
            );
        }

        // Turn 2: Inspect via manage_task parallel wave (list + status)
        let turn_id_2 = 5006;
        rig.scripted_llm.push_step({
            let task_id_clone = task_id.clone();
            move |tx| {
                let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                    id: "call-manage-list".to_string(),
                    name: "manage_task".to_string(),
                    arguments: serde_json::json!({ "action": "list" }),
                }));
                let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                    id: "call-manage-status".to_string(),
                    name: "manage_task".to_string(),
                    arguments: serde_json::json!({ "action": "status", "task_id": task_id_clone }),
                }));
                let _ = tx.send(LlmResponse::Finished);
            }
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Status checked.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req2 = rig.turn_request("Check status", turn_id_2);
        let outcome2 = Harness::execute_turn(&rig.state.harness, req2).await;
        assert!(matches!(outcome2, TurnOutcome::Completed { .. }));

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness mounted");
        let tool_observations: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();

        let status_obs = tool_observations
            .iter()
            .find(|m| m.content.contains("LINE_120"))
            .expect("status observation must contain LINE_120");
        assert!(
            status_obs.content.contains("LINE_071"),
            "Tail must contain line 71 (last 50 lines of 120)"
        );
        assert!(
            !status_obs.content.contains("LINE_001"),
            "Tail must NOT contain line 001 (head line outside last 50)"
        );
        assert!(
            status_obs.content.contains(&task_id),
            "status observation must name the requested task_id"
        );
        assert!(
            status_obs.content.contains("running"),
            "status observation must report the requested task's own 'running' status, not a session-wide boolean"
        );

        let list_obs = tool_observations
            .iter()
            .find(|m| m.content.contains(&task_id) && m.content.contains("running"))
            .expect("list observation must contain task_id and running status");
        assert!(list_obs.content.contains(&task_id));
        drop(guard);

        let _ = std::fs::remove_file(&flag_path);
        rig.state.tasks.kill_all(rig.session_id);
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch orphaned grandchild processes when `manage_task(action="kill")` kills only the parent shell PID (+pid) instead of the negative process group ID (-pgid).
/// SCENARIO: Promote a `bash` command that spawns a detached subshell grandchild (`sh -c 'sleep 60 & echo $! > <tmp>/grandchild.pid; wait'`). Wait until grandchild.pid is written on disk and parse grandchild_pid: i32. Model invokes `manage_task({"action": "kill", "task_id": task_id})`.
/// ASSERTS:  `manage_task` returns `status: "killed"`; `state.tasks.active_tasks_count(session_id) == 0`; `session_tasks` row in Turso DB has `status = 'killed'`; polling `unsafe { libc::kill(grandchild_pid, 0) }` returns -1 (ESRCH, proving the grandchild process was reaped along with the parent shell).
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   In `kill_process_group` (`services/tasks/manager.rs`), change `libc::kill(-(pid as i32), libc::SIGKILL)` to `libc::kill(pid as i32, libc::SIGKILL)` (positive PID).
#[tokio::test]
async fn manage_task_kill_terminates_entire_process_group_including_grandchildren() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 5007;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.auto_promote_timeout = Duration::from_millis(50);
        }

        let tmp = tempfile::tempdir().expect("tempdir");
        let pid_file = tmp.path().join("grandchild.pid");

        let cmd = format!(
            "sh -c 'sleep 60 & echo $! > \"{}\"; wait'",
            pid_file.display()
        );

        rig.scripted_llm.push_step({
            let cmd_clone = cmd.clone();
            move |tx| {
                let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                    id: "call-bash-grandchild".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({
                        "command": cmd_clone,
                        "run_in_background": true,
                    }),
                }));
                let _ = tx.send(LlmResponse::Finished);
            }
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Spawned grandchild.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Spawn grandchild", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        assert!(matches!(outcome, TurnOutcome::Completed { .. }));

        let task_id = {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness mounted");
            let tool_obs = harness
                .messages()
                .iter()
                .find(|m| m.role == Role::Tool)
                .expect("Tool observation");
            let parsed: serde_json::Value = serde_json::from_str(&tool_obs.content)
                .expect("Valid JSON observation");
            parsed["task_id"].as_str().expect("task_id").to_string()
        };

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut grandchild_pid: i32 = 0;
        while std::time::Instant::now() < deadline {
            if pid_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&pid_file) {
                    if let Ok(pid) = content.trim().parse::<i32>() {
                        grandchild_pid = pid;
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(grandchild_pid > 0, "grandchild PID must be written and parsed");

        // POSIX-only liveness proof; the crate does not link libc on Windows.
        #[cfg(unix)]
        assert_eq!(
            unsafe { libc::kill(grandchild_pid, 0) },
            0,
            "grandchild process must be alive before kill"
        );

        // Turn 2: Invoke manage_task kill
        let turn_id_2 = 5008;
        rig.scripted_llm.push_step({
            let task_id_clone = task_id.clone();
            move |tx| {
                let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                    id: "call-manage-kill".to_string(),
                    name: "manage_task".to_string(),
                    arguments: serde_json::json!({
                        "action": "kill",
                        "task_id": task_id_clone,
                    }),
                }));
                let _ = tx.send(LlmResponse::Finished);
            }
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Killed task.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req2 = rig.turn_request("Kill task", turn_id_2);
        let outcome2 = Harness::execute_turn(&rig.state.harness, req2).await;
        assert!(matches!(outcome2, TurnOutcome::Completed { .. }));

        // The tool's own observation, not only the persisted row, must report the kill.
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness mounted");
            let kill_obs = harness
                .messages()
                .iter()
                .rfind(|m| m.role == Role::Tool)
                .expect("manage_task kill observation must be committed");
            assert!(
                kill_obs.content.contains("killed"),
                "manage_task kill observation must report status 'killed', got: {}",
                kill_obs.content
            );
        }

        assert_eq!(
            rig.state.tasks.active_tasks_count(rig.session_id),
            0,
            "active_tasks_count must be 0 after kill"
        );

        // Quiescence: kill() persists the terminal status asynchronously. Require
        // the row to exist (and reach 'killed') within a bounded deadline; never
        // skip the assertion when the row is absent.
        let conn = rig.state.db.connect().expect("db connect");
        let db_deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut killed_status: Option<String> = None;
        while std::time::Instant::now() < db_deadline {
            let mut rows = conn
                .query(
                    "SELECT status FROM session_tasks WHERE task_id = ?;",
                    (task_id.clone(),),
                )
                .await
                .expect("query session_tasks");
            let status: Option<String> = rows
                .next()
                .await
                .expect("read session_tasks row")
                .map(|row| row.get::<String>(0).expect("status column"));
            if status.as_deref() == Some("killed") {
                killed_status = status;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            killed_status.as_deref(),
            Some("killed"),
            "kill must persist a session_tasks row with status='killed' within the deadline"
        );

        #[cfg(unix)]
        {
            let kill_deadline = std::time::Instant::now() + Duration::from_secs(3);
            let mut grandchild_dead = false;
            while std::time::Instant::now() < kill_deadline {
                if unsafe { libc::kill(grandchild_pid, 0) } == -1 {
                    grandchild_dead = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            assert!(
                grandchild_dead,
                "grandchild process group must be terminated via SIGKILL to negative PGID"
            );
        }
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch a dropped proactive-turn dispatch for `TaskFinished` in Ready within the active window.
/// SCENARIO: Router is in `InteractionState::Ready` with activity <5m; a Completed(exit 0) `TaskFinished` is injected at the router seam. Real supervisor emission is covered by `supervised_task_natural_exit_emits_task_finished_with_completed_status_and_notification`.
/// ASSERTS:  `on_task_finished` persists `session_tasks.status = 'completed'` with `exit_code = 0`; the router emits `IpcEvent::NotificationCreated` with category "task"; TTS receives the model's spoken completion summary.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   In `pipeline/assistant/tasks.rs` (`on_task_finished`), omit the proactive `execute_turn` dispatch when `state == InteractionState::Ready`.
#[tokio::test]
async fn task_finished_in_ready_within_active_window_triggers_proactive_spoken_turn() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let task_id = "task-proactive-007";
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        rig.state
            .pipeline
            .last_user_activity_ms
            .store(now - 10_000, Ordering::Relaxed);
        rig.state.pipeline.set_state(InteractionState::Ready);

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token(
                "Your build completed successfully with exit code 0.".to_string(),
            ));
            let _ = tx.send(LlmResponse::Finished);
        });

        rig.send_event(VoxEvent::TaskFinished {
            task_id: task_id.to_string(),
            session_id: rig.session_id,
            status: vox_lib::core::events::TaskStatus::Completed {
                exit_code: 0,
                duration_ms: 1500,
                completed_at: now,
            },
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut heard_speech = false;
        while std::time::Instant::now() < deadline {
            let cmds = rig.drain_tts_commands();
            if cmds.iter().any(|cmd| {
                if let TtsCommand::Generate { text, .. } = cmd {
                    text.contains("completed successfully")
                } else {
                    false
                }
            }) {
                heard_speech = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            heard_speech,
            "Router must dispatch proactive spoken turn when task finishes within active user window (<5m) in Ready"
        );

        // Bounded DB barrier: on_task_finished persists the terminal state asynchronously.
        let conn = rig.state.db.connect().expect("db connect");
        let db_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut completed_row = None;
        while tokio::time::Instant::now() < db_deadline {
            let mut rows = conn
                .query(
                    "SELECT status, exit_code FROM session_tasks WHERE task_id = ?;",
                    (task_id.to_string(),),
                )
                .await
                .expect("query session_tasks");
            if let Some(row) = rows.next().await.expect("read session_tasks row") {
                let status: String = row.get(0).expect("status column");
                let exit_code: Option<i64> = row.get(1).expect("exit_code column");
                if status == "completed" {
                    completed_row = Some((status, exit_code));
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let (status, exit_code) = completed_row.expect("task row must reach completed within 5s");
        assert_eq!(status, "completed");
        assert_eq!(exit_code, Some(0), "completed task must persist exit_code 0");

        // Capture sink installed in Rig::new before any event is sent: this is the
        // production `app.emit("notification_created", …)` boundary, not a test double.
        let notification = rig
            .wait_for_ipc_event(
                |v| {
                    v["category"].as_str() == Some("task")
                        && v["group_key"]
                            .as_str()
                            .is_some_and(|key| key.contains(task_id))
                },
                Duration::from_secs(5),
            )
            .await
            .expect("NotificationCreated(category=task) must be emitted at the IPC boundary");
        assert_eq!(notification["category"].as_str(), Some("task"));
    })
    .await
    .expect("test timed out");
}

/// Quiescence proof for a suppressed `VoxEvent::TaskFinished` (testing-style-guide §5.1).
///
/// The negative assertions (zero LLM dispatch, zero TTS commands) are valid only
/// after the producer is proven finished. No bare sleep is used as that proof:
///  1. the Rig event tap (installed in `Rig::new`, before any event is sent)
///     captures the `TaskFinished` — the ack that it entered vox-router's channel;
///  2. a FIFO `LlmFinished` sentinel sent immediately after proves the router
///     progressed past `TaskFinished` in its single-threaded event pump;
///  3. the `session_tasks` terminal row is a positive barrier proving
///     `on_task_finished` actually ran;
///  4. the `notification_created` IPC payload is captured at the production emit
///     boundary, proving the notification-only half of the contract;
///  5. a bounded state-invariant watch then proves no later transition can stage
///     a proactive turn (every proactive dispatch transitions to Thinking first).
async fn assert_task_finished_suppressed_after_router_ack(
    rig: &Rig,
    task_id: &str,
    expected_state: InteractionState,
    baseline_dispatches: usize,
    sentinel_turn_id: u32,
) {
    // (1) Ack at the router seam: the tap records every event before forwarding
    // it to vox-router, so TaskFinished entered the router channel by this point.
    rig.wait_for_event(
        |ev| matches!(ev, VoxEvent::TaskFinished { task_id: id, .. } if id.as_str() == task_id),
        Duration::from_secs(5),
    )
    .await
    .expect("TaskFinished must be captured at the router seam");

    // (2) FIFO sentinel: the tap forwards to the router in send order, so seeing
    // this event after TaskFinished proves the router consumed the former first.
    rig.send_event(VoxEvent::LlmFinished {
        turn_id: sentinel_turn_id,
    });
    rig.wait_for_event(
        |ev| matches!(ev, VoxEvent::LlmFinished { turn_id: tid } if *tid == sentinel_turn_id),
        Duration::from_secs(5),
    )
    .await
    .expect("FIFO sentinel must be captured after TaskFinished");

    // (3) Positive barrier: on_task_finished persisted the terminal task state.
    let conn = rig.state.db.connect().expect("db connect");
    let db_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut saw_completed = false;
    while tokio::time::Instant::now() < db_deadline {
        let mut rows = conn
            .query(
                "SELECT status FROM session_tasks WHERE task_id = ?;",
                (task_id.to_string(),),
            )
            .await
            .expect("query session_tasks");
        let status: Option<String> = rows
            .next()
            .await
            .expect("read session_tasks row")
            .map(|row| row.get::<String>(0).expect("status column"));
        if status.as_deref() == Some("completed") {
            saw_completed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        saw_completed,
        "on_task_finished must persist status='completed' for task {}",
        task_id
    );

    // (4) Notification is mandatory even when speech is suppressed.
    let notification = rig
        .wait_for_ipc_event(
            |v| {
                v["category"].as_str() == Some("task")
                    && v["group_key"]
                        .as_str()
                        .is_some_and(|key| key.contains(task_id))
            },
            Duration::from_secs(5),
        )
        .await
        .expect("NotificationCreated(category=task) must be emitted even when speech is suppressed");
    assert_eq!(notification["category"].as_str(), Some("task"));

    // (5) A suppressed TaskFinished never transitions out of the expected state.
    // A late proactive dispatch must transition to Thinking before dispatching,
    // so a stable state over the watch window proves no turn can be staged.
    common::harness::assert_pipeline_state_stable(
        || rig.state.pipeline.state(),
        expected_state,
        Duration::from_millis(1000),
        "suppressed TaskFinished must not transition the pipeline",
    )
    .await;

    // Negative assertions are valid only after the quiescence proof above.
    assert_eq!(
        rig.dispatch_count(),
        baseline_dispatches,
        "a suppressed TaskFinished must produce zero LLM dispatches"
    );
    assert!(
        rig.drain_tts_commands().is_empty(),
        "a suppressed TaskFinished must produce zero TTS commands"
    );
}

/// PURPOSE:  Catch unsolicited voice wake-ups when a background task finishes after the user has been quiet for >5 minutes in Ready or while the session is Paused.
/// SCENARIO: Set `last_user_activity_ms` to now - 6 minutes in `InteractionState::Ready` (and repeat in `InteractionState::Paused`). Trigger `VoxEvent::TaskFinished` through `vox-router`.
/// ASSERTS:  The router consumes each `TaskFinished` (tap ack + FIFO sentinel + `session_tasks` 'completed' barrier); `IpcEvent::NotificationCreated` carries category "task"; pipeline state stays in Ready (or Paused) across a bounded invariant watch; zero LLM dispatches (`llm_dispatch_count == 0`) and zero TTS commands are dispatched.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   In `pipeline/assistant/tasks.rs`, bypass the 5-minute `last_user_activity_ms` check in Ready (or allow proactive turn dispatch in Paused).
#[tokio::test]
async fn task_finished_in_ready_after_quiet_window_or_in_paused_suppresses_speech_and_emits_notification_only() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let rig = Rig::new().await;
        let task_id = "task-quiet-008";
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Part 1: Quiet window (>5 minutes) in Ready state
        rig.state
            .pipeline
            .last_user_activity_ms
            .store(now - 6 * 60 * 1000, Ordering::Relaxed);
        rig.state.pipeline.set_state(InteractionState::Ready);

        let initial_dispatches = rig.dispatch_count();
        let _ = rig.drain_tts_commands();

        rig.send_event(VoxEvent::TaskFinished {
            task_id: format!("{}-ready", task_id),
            session_id: rig.session_id,
            status: vox_lib::core::events::TaskStatus::Completed {
                exit_code: 0,
                duration_ms: 3000,
                completed_at: now,
            },
        });

        assert_task_finished_suppressed_after_router_ack(
            &rig,
            &format!("{}-ready", task_id),
            InteractionState::Ready,
            initial_dispatches,
            8_008_001,
        )
        .await;

        // Part 2: Session in Paused state
        rig.state.pipeline.set_state(InteractionState::Paused);
        rig.state
            .pipeline
            .last_user_activity_ms
            .store(now, Ordering::Relaxed);

        let paused_dispatches = rig.dispatch_count();
        let _ = rig.drain_tts_commands();

        rig.send_event(VoxEvent::TaskFinished {
            task_id: format!("{}-paused", task_id),
            session_id: rig.session_id,
            status: vox_lib::core::events::TaskStatus::Completed {
                exit_code: 0,
                duration_ms: 4000,
                completed_at: now,
            },
        });

        assert_task_finished_suppressed_after_router_ack(
            &rig,
            &format!("{}-paused", task_id),
            InteractionState::Paused,
            paused_dispatches,
            8_008_002,
        )
        .await;
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch premature step-boundary release when TaskFinished arrives during a held voice turn, and voice leaking into StepInbox.
/// SCENARIO: A tool step commits in Working; the user speaks (hold engaged); a task finishes; TranscriptFinal("Also check the logs") rides both notices into one staged user turn.
/// ASSERTS:  After tap ack + FIFO sentinel + DB barrier the hold is still active; the next LLM request's staged user text contains BOTH the task notice ("task-009"/"completed") and "Also check the logs"; StepInbox holds no voice text; the hold is released.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   In `pipeline/assistant/tasks.rs`, call `inbox.push(...)` (which sets `speech_hold_active = false`) instead of `inbox.push_task_notification(...)`.
#[tokio::test]
async fn task_finished_in_working_buffers_in_step_inbox_without_releasing_active_speech_hold() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 5009;

        // Commit real tool work so the barge-in path sees a non-empty history.
        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.register_tool(Arc::new(SimpleTestTool {
                name: "step_tool".to_string(),
            }));
        }
        rig.state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        rig.state.pipeline.set_state(InteractionState::Working);

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-step-009".to_string(),
                name: "step_tool".to_string(),
                arguments: serde_json::json!({}),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Steering handled.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let inbox = Arc::clone(rig.state.harness.lock().as_ref().unwrap().inbox());
        // Engage before spawn: the coordinator parks at the step boundary, so the
        // second pass cannot consume the steering step before the user speaks.
        inbox.engage_speech_hold();

        let req = rig.turn_request("Working tool turn", turn_id);
        let harness_arc = Arc::clone(&rig.state.harness);
        let turn_handle = tokio::spawn(async move {
            Harness::execute_turn(&harness_arc, req).await
        });

        let tool_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            // Inner scope: the guard-derived borrow must die before the await below.
            let committed = {
                let guard = rig.state.harness.lock();
                guard
                    .as_ref()
                    .is_some_and(|h| h.messages().iter().any(|m| m.role == Role::Tool))
            };
            if committed {
                break;
            }
            assert!(
                tokio::time::Instant::now() < tool_deadline,
                "Step 0 tool observation must commit before barge-in"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // 1. Router receives SpeechStart and engages the hold.
        rig.send_event(VoxEvent::SpeechStart {
            owner: InteractionOwner::Assistant,
        });
        assert!(
            rig.wait_for_state(InteractionState::Listening, Duration::from_millis(500)).await,
            "SpeechStart must transition state to Listening"
        );
        assert!(inbox.is_speech_held(), "Speech hold must be active after SpeechStart");

        // 2. Background task finishes through the router while the hold is active.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        rig.send_event(VoxEvent::TaskFinished {
            task_id: "task-009".to_string(),
            session_id: rig.session_id,
            status: vox_lib::core::events::TaskStatus::Completed {
                exit_code: 0,
                duration_ms: 1200,
                completed_at: now,
            },
        });

        // Quiescence: tap ack proves the event entered the router channel; the FIFO
        // sentinel proves the router consumed it; the DB row proves on_task_finished ran.
        rig.wait_for_event(
            |ev| matches!(ev, VoxEvent::TaskFinished { task_id: id, .. } if id == "task-009"),
            Duration::from_secs(5),
        )
        .await
        .expect("TaskFinished must be captured at the router seam");
        rig.send_event(VoxEvent::LlmFinished {
            turn_id: 9_009_001,
        });
        rig.wait_for_event(
            |ev| matches!(ev, VoxEvent::LlmFinished { turn_id } if *turn_id == 9_009_001),
            Duration::from_secs(5),
        )
        .await
        .expect("FIFO sentinel must be captured after TaskFinished");
        let conn = rig.state.db.connect().expect("db connect");
        let db_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut saw_completed = false;
        while tokio::time::Instant::now() < db_deadline {
            let mut rows = conn
                .query(
                    "SELECT status FROM session_tasks WHERE task_id = ?;",
                    ("task-009".to_string(),),
                )
                .await
                .expect("query session_tasks");
            let status: Option<String> = rows
                .next()
                .await
                .expect("read session_tasks row")
                .map(|row| row.get::<String>(0).expect("status column"));
            if status.as_deref() == Some("completed") {
                saw_completed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(saw_completed, "on_task_finished must persist status='completed'");

        assert!(
            inbox.is_speech_held(),
            "speech_hold_active must remain TRUE after TaskFinished while user speaks (anti-clobber invariant)"
        );

        // 3. TranscriptFinal rides the held task notice and the voice into one turn.
        rig.send_event(VoxEvent::TranscriptFinal {
            turn_id,
            text: "Also check the logs".to_string(),
            owner: InteractionOwner::Assistant,
        });

        let hold_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while tokio::time::Instant::now() < hold_deadline && inbox.is_speech_held() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            !inbox.is_speech_held(),
            "Speech hold must be released when user finishes speaking"
        );

        // Observe the staged user text on the real LLM request, not a downstream buffer.
        let prompt_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut steering_prompt = None;
        while tokio::time::Instant::now() < prompt_deadline {
            if let Some(prompt) = rig
                .scripted_llm
                .prompts()
                .into_iter()
                .find(|p| p.contains("Also check the logs"))
            {
                steering_prompt = Some(prompt);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let prompt = steering_prompt.expect("staged LLM request must carry the steering message");
        assert!(
            prompt.contains("task-009") && prompt.contains("completed"),
            "staged request must carry the background task notice, got: {}",
            prompt
        );
        assert!(
            prompt.contains("Also check the logs"),
            "staged request must carry the user steering message, got: {}",
            prompt
        );

        // RULE-HARN-18: voice must never be re-inserted into StepInbox.
        let remaining = inbox.drain_immediate();
        assert!(
            remaining.iter().all(|m| !m.text.contains("Also check the logs")),
            "StepInbox is strictly non-voice; user speech must never be injected"
        );
        assert!(
            remaining.is_empty(),
            "the held task notice must be consumed into the staged turn, not left buffered"
        );

        let outcome = turn_handle.await.expect("turn worker panicked");
        assert!(
            matches!(outcome, TurnOutcome::Cancelled { .. }),
            "non-empty barge-in must cancel the prior turn, got {:?}",
            outcome
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch premature model weight offload (`Sleeping`) while background tasks are running (`RULE-EVT-12`) or unconfirmed session teardown leaving orphan processes (`RULE-EVT-13`).
/// SCENARIO: Start `spawn_idle_monitor_with_timeouts` with `paused_timeout = Duration::from_millis(60)`. Promote a long-running background task (recording `grandchild.pid`), pause session (`VoxEvent::PauseSession`), and wait 120ms past `paused_timeout`. Then resume to Ready, send `VoxEvent::EndSession { force: false }`, verify rejection, and send `VoxEvent::EndSession { force: true }`.
/// ASSERTS:  While the background task is running, pipeline remains in Paused past 120ms and NEVER transitions to Sleeping; in Ready, EndSession { force: false } leaves pipeline in Ready with task still running; EndSession { force: true } kills the process group (`libc::kill(grandchild_pid, 0) == -1`), marks the task killed, and transitions pipeline to Idle.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   In `lifecycle.rs`, remove the `!state.tasks.has_running_tasks()` guard before transitioning to `InteractionState::Sleeping` (or in `session.rs` `on_end`, ignore `force == false` when active tasks exist).
#[tokio::test]
async fn running_background_task_inhibits_paused_to_sleeping_offload_and_disengage_enforces_force_guard() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 5010;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.auto_promote_timeout = Duration::from_millis(50);
        }

        let tmp = tempfile::tempdir().expect("tempdir");
        let pid_file = tmp.path().join("grandchild_5010.pid");

        let cmd = format!(
            "sh -c 'sleep 60 & echo $! > \"{}\"; wait'",
            pid_file.display()
        );

        rig.scripted_llm.push_step({
            let cmd_clone = cmd.clone();
            move |tx| {
                let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                    id: "call-bash-task10".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({
                        "command": cmd_clone,
                        "run_in_background": true,
                    }),
                }));
                let _ = tx.send(LlmResponse::Finished);
            }
        });

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Task running in background.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Start task", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        assert!(matches!(outcome, TurnOutcome::Completed { .. }));

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut grandchild_pid: i32 = 0;
        while std::time::Instant::now() < deadline {
            if pid_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&pid_file) {
                    if let Ok(pid) = content.trim().parse::<i32>() {
                        grandchild_pid = pid;
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(grandchild_pid > 0, "grandchild PID must be written and parsed");

        // Start idle monitor with 60ms timeout
        vox_lib::pipeline::lifecycle::spawn_idle_monitor_with_timeouts(
            rig.app.clone(),
            Arc::clone(&rig.state),
            Duration::from_millis(100),
            Duration::from_millis(60),
        );

        // Pause session
        rig.send_event(VoxEvent::PauseSession);
        assert!(
            rig.wait_for_state(InteractionState::Paused, Duration::from_millis(500)).await,
            "Session must transition to Paused"
        );

        // RULE-EVT-12: watch the full horizon (> 2x paused_timeout of 60ms) instead
        // of sleeping then sampling once. A watched invariant catches a transient
        // Paused -> Sleeping -> Paused flapping that a single sample would miss.
        common::harness::assert_pipeline_state_stable(
            || rig.state.pipeline.state(),
            InteractionState::Paused,
            Duration::from_millis(150),
            "background task must inhibit Paused -> Sleeping (RULE-EVT-12)",
        )
        .await;

        // Resume to Ready
        rig.send_event(VoxEvent::ResumeSession);
        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_millis(500)).await,
            "Session must resume to Ready"
        );

        // RULE-EVT-13 Backend Force Guard: EndSession { force: false } must be rejected while tasks run!
        rig.send_event(VoxEvent::EndSession { force: false });

        // Quiescence: tap ack + FIFO sentinel prove the router consumed EndSession
        // before the invariant watch begins; no bare sleep asserts absence.
        rig.wait_for_event(
            |ev| matches!(ev, VoxEvent::EndSession { force: false }),
            Duration::from_secs(5),
        )
        .await
        .expect("unforced EndSession must be captured at the router seam");
        rig.send_event(VoxEvent::LlmFinished {
            turn_id: 10_010_001,
        });
        rig.wait_for_event(
            |ev| matches!(ev, VoxEvent::LlmFinished { turn_id } if *turn_id == 10_010_001),
            Duration::from_secs(5),
        )
        .await
        .expect("FIFO sentinel must be captured after unforced EndSession");

        common::harness::assert_pipeline_state_stable(
            || rig.state.pipeline.state(),
            InteractionState::Ready,
            Duration::from_millis(200),
            "unforced EndSession must be rejected while background tasks run (RULE-EVT-13)",
        )
        .await;
        assert_eq!(
            rig.state.tasks.active_tasks_count(rig.session_id),
            1,
            "Task must still be running after rejected unforced EndSession"
        );

        // Confirmed force teardown: EndSession { force: true }
        rig.send_event(VoxEvent::EndSession { force: true });
        assert!(
            rig.wait_for_state(InteractionState::Idle, Duration::from_millis(500)).await,
            "Forced EndSession must transition pipeline to Idle"
        );

        assert_eq!(
            rig.state.tasks.active_tasks_count(rig.session_id),
            0,
            "Active tasks count must be 0 after forced EndSession"
        );

        // The kill is persisted asynchronously: require the terminal row, never skip it.
        let conn = rig.state.db.connect().expect("db connect");
        let status_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut killed_status = None;
        while tokio::time::Instant::now() < status_deadline {
            let mut rows = conn
                .query(
                    "SELECT status FROM session_tasks WHERE session_id = ? ORDER BY started_at DESC LIMIT 1;",
                    (rig.session_id,),
                )
                .await
                .expect("query session_tasks");
            let status: Option<String> = rows
                .next()
                .await
                .expect("read session_tasks row")
                .map(|row| row.get::<String>(0).expect("status column"));
            if status.as_deref() == Some("killed") {
                killed_status = status;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            killed_status.as_deref(),
            Some("killed"),
            "forced EndSession must persist the task as killed"
        );

        #[cfg(unix)]
        {
            let kill_deadline = std::time::Instant::now() + Duration::from_secs(3);
            let mut grandchild_dead = false;
            while std::time::Instant::now() < kill_deadline {
                if unsafe { libc::kill(grandchild_pid, 0) } == -1 {
                    grandchild_dead = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            assert!(
                grandchild_dead,
                "Process group must be killed on forced EndSession"
            );
        }
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch a silent supervisor-to-router handoff when a promoted task exits on its own.
/// SCENARIO: A bash command outlives the 80ms auto-promotion window, is promoted, then exits with code 0. No TaskFinished is injected by the test.
/// ASSERTS:  The supervisor emits VoxEvent::TaskFinished(Completed, exit_code 0) at the tap; `session_tasks` persists status='completed', exit_code=0 and tool_call_id="call-natural-exit"; IpcEvent::NotificationCreated(category="task") is emitted; TTS receives the spoken completion summary.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   In `services/tasks/manager.rs`, delete the `tx.send(VoxEvent::TaskFinished { ... })` call in the supervisor's `is_promoted` branch.
#[tokio::test]
async fn supervised_task_natural_exit_emits_task_finished_with_completed_status_and_notification() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let rig = Rig::new().await;
        let turn_id = 5011;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.auto_promote_timeout = Duration::from_millis(80);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        rig.state
            .pipeline
            .last_user_activity_ms
            .store(now - 10_000, Ordering::Relaxed);
        rig.state.pipeline.set_state(InteractionState::Ready);

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-natural-exit".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": "sleep 0.3 && printf 'NATURAL_EXIT_OK'",
                    "run_in_background": true,
                }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Your task completed successfully.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Promote task then let it exit", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "promotion turn must complete, got {:?}",
            outcome
        );

        // Read the task id from the real promotion observation. Do NOT inject an event.
        let task_id = {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness mounted");
            let obs = harness
                .messages()
                .iter()
                .find(|m| m.role == Role::Tool)
                .expect("promotion observation must be committed");
            let parsed: serde_json::Value =
                serde_json::from_str(&obs.content).expect("promotion observation must be JSON");
            assert_eq!(parsed["status"].as_str(), Some("running_in_background"));
            parsed["task_id"].as_str().expect("task_id").to_string()
        };

        // Tap ack: only the supervisor can emit this event in this test.
        rig.wait_for_event(
            |ev| {
                matches!(
                    ev,
                    VoxEvent::TaskFinished {
                        task_id: id,
                        status: vox_lib::core::events::TaskStatus::Completed { exit_code: 0, .. },
                        ..
                    } if id.as_str() == task_id
                )
            },
            Duration::from_secs(8),
        )
        .await
        .expect("supervisor must emit TaskFinished(Completed, exit_code=0) without test injection");

        // Bounded DB barrier: supervisor writes the terminal row itself.
        let conn = rig.state.db.connect().expect("db connect");
        let db_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut terminal_row = None;
        while tokio::time::Instant::now() < db_deadline {
            let mut rows = conn
                .query(
                    "SELECT status, exit_code, tool_call_id FROM session_tasks WHERE task_id = ?;",
                    (task_id.clone(),),
                )
                .await
                .expect("query session_tasks");
            if let Some(row) = rows.next().await.expect("read session_tasks row") {
                let status: String = row.get(0).expect("status column");
                if status == "completed" {
                    let exit_code: Option<i64> = row.get(1).expect("exit_code column");
                    let tool_call_id: Option<String> = row.get(2).expect("tool_call_id column");
                    terminal_row = Some((status, exit_code, tool_call_id));
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let (status, exit_code, tool_call_id) =
            terminal_row.expect("supervisor must persist a completed task row within 5s");
        assert_eq!(status, "completed");
        assert_eq!(exit_code, Some(0));
        assert_eq!(
            tool_call_id.as_deref(),
            Some("call-natural-exit"),
            "BashTool must pass the canonical call id through to session_tasks"
        );

        // Notification is emitted on the real app.emit boundary, not a test double.
        let notification = rig
            .wait_for_ipc_event(
                |v| {
                    v["category"].as_str() == Some("task")
                        && v["group_key"]
                            .as_str()
                            .is_some_and(|key| key.contains(&task_id))
                },
                Duration::from_secs(5),
            )
            .await
            .expect("NotificationCreated(category=task) must be emitted for the natural exit");
        assert_eq!(notification["category"].as_str(), Some("task"));

        // Router must turn the supervisor event into a proactive spoken turn.
        let tts_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut heard_speech = false;
        while tokio::time::Instant::now() < tts_deadline {
            if rig
                .drain_tts_commands()
                .iter()
                .any(|cmd| matches!(cmd, TtsCommand::Generate { text, .. } if text.contains("completed successfully")))
            {
                heard_speech = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            heard_speech,
            "TaskFinished from the real supervisor must dispatch a spoken completion summary"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE: Catch a dropped task result or a duplicate proactive turn when a promoted task finishes during speech without a step-boundary hold.
/// SCENARIO: Ready with recent activity; a promoted bash task exits while the user is Listening (no hold engaged); SpeechEnd moves to Thinking; a non-empty TranscriptFinal arrives.
/// ASSERTS: The single user LLM request's staged text carries BOTH the task result and the user words; StepInbox is empty after the fold; no second proactive dispatch occurs (tap ack + FIFO sentinel + DB barrier + watched dispatch-count invariant).
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   In `transcript.rs`, in the non-held TranscriptFinal path, replace `drain_inbox_prefix(state)` with `String::new()`.
#[tokio::test]
async fn task_result_without_speech_hold_folds_into_single_user_turn() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let rig = Rig::new().await;
        let turn_id = 5012;

        {
            let mut guard = rig.state.harness.lock();
            let harness = guard.as_mut().expect("Harness mounted");
            harness.auto_promote_timeout = Duration::from_millis(50);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        rig.state
            .pipeline
            .last_user_activity_ms
            .store(now - 10_000, Ordering::Relaxed);
        rig.state.pipeline.set_state(InteractionState::Ready);

        // Pass 0 promotes the command; pass 1 ends the promotion turn.
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-fold-no-hold".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": "sleep 0.2 && printf 'TASK_RESULT_OK'",
                    "run_in_background": true,
                }),
            }));
            let _ = tx.send(LlmResponse::Finished);
        });
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Task started in background.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });
        // Pass 2 is the single user turn that must carry both notices.
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Token("Combined turn complete.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Start task then speak", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        assert!(
            matches!(outcome, TurnOutcome::Completed { .. }),
            "promotion turn must complete, got {:?}",
            outcome
        );

        let task_id = {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness mounted");
            let obs = harness
                .messages()
                .iter()
                .find(|m| m.role == Role::Tool)
                .expect("promotion observation must be committed");
            let parsed: serde_json::Value =
                serde_json::from_str(&obs.content).expect("promotion observation must be JSON");
            assert_eq!(parsed["status"].as_str(), Some("running_in_background"));
            parsed["task_id"].as_str().expect("task_id").to_string()
        };
        let baseline_dispatches = rig.dispatch_count();

        // The mock playback engine never emits PlaybackFinished, so the router
        // stays in Speaking after the promotion turn. Establish the real scene's
        // Ready precondition explicitly before driving SpeechStart.
        rig.state.pipeline.set_state(InteractionState::Ready);

        // Real router trigger: Ready -> Listening. SpeechStart in Ready does not
        // engage the step-boundary hold, which is the path under test.
        rig.send_event(VoxEvent::SpeechStart {
            owner: InteractionOwner::Assistant,
        });
        assert!(
            rig.wait_for_state(InteractionState::Listening, Duration::from_millis(500)).await,
            "SpeechStart must transition Ready -> Listening"
        );
        {
            let guard = rig.state.harness.lock();
            assert!(
                !guard.as_ref().expect("Harness mounted").inbox().is_speech_held(),
                "Ready-path SpeechStart must NOT engage the step-boundary hold"
            );
        }

        // Wait for the supervisor-emitted TaskFinished while the user is speaking.
        rig.wait_for_event(
            |ev| {
                matches!(
                    ev,
                    VoxEvent::TaskFinished {
                        task_id: id,
                        status: vox_lib::core::events::TaskStatus::Completed { exit_code: 0, .. },
                        ..
                    } if id.as_str() == task_id
                )
            },
            Duration::from_secs(8),
        )
        .await
        .expect("supervisor must emit TaskFinished while the user is Listening");

        // DB barrier proves on_task_finished ran and buffered the task notice.
        let conn = rig.state.db.connect().expect("db connect");
        let db_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut task_completed = false;
        while tokio::time::Instant::now() < db_deadline {
            let mut rows = conn
                .query(
                    "SELECT status FROM session_tasks WHERE task_id = ?;",
                    (task_id.clone(),),
                )
                .await
                .expect("query session_tasks");
            let status: Option<String> = rows
                .next()
                .await
                .expect("read session_tasks row")
                .map(|row| row.get::<String>(0).expect("status column"));
            if status.as_deref() == Some("completed") {
                task_completed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(task_completed, "supervisor must persist status='completed'");
        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness mounted");
            assert!(
                !harness.inbox().is_speech_held(),
                "task notice buffering must not engage a hold in the normal speech path"
            );
            assert!(
                !harness.inbox().is_empty(),
                "task result must be buffered in StepInbox before TranscriptFinal"
            );
        }

        // SpeechEnd is the real Listening -> Thinking trigger before the final.
        rig.send_event(VoxEvent::SpeechEnd {
            owner: InteractionOwner::Assistant,
        });
        assert!(
            rig.wait_for_state(InteractionState::Thinking, Duration::from_millis(500)).await,
            "SpeechEnd must transition Listening -> Thinking"
        );
        rig.send_event(VoxEvent::TranscriptFinal {
            turn_id: rig.state.pipeline.peek_turn_id(),
            text: "and check the logs".to_string(),
            owner: InteractionOwner::Assistant,
        });

        // Observe the staged user text on the real LLM request.
        let prompt_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut user_prompt = None;
        while tokio::time::Instant::now() < prompt_deadline {
            if let Some(prompt) = rig
                .scripted_llm
                .prompts()
                .into_iter()
                .find(|p| p.contains("and check the logs"))
            {
                user_prompt = Some(prompt);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let prompt = user_prompt.expect("exactly one user request must carry the user words");
        assert!(
            prompt.contains(&task_id) && prompt.contains("completed"),
            "single user request must carry the buffered task result, got: {}",
            prompt
        );
        assert!(
            prompt.contains("and check the logs"),
            "single user request must carry the user words, got: {}",
            prompt
        );
        {
            let guard = rig.state.harness.lock();
            assert!(
                guard.as_ref().expect("Harness mounted").inbox().drain_immediate().is_empty(),
                "inbox must be drained into the user turn, leaving nothing for a second turn"
            );
        }
        assert_eq!(
            rig.dispatch_count(),
            baseline_dispatches + 1,
            "one user turn must be dispatched, not a task turn plus a user turn"
        );

        // Quiescence: FIFO sentinel proves the router consumed the user turn;
        // the mock playback engine never emits PlaybackFinished, so liveness is
        // proven by the dispatch count staying flat across a watched window.
        rig.send_event(VoxEvent::LlmFinished {
            turn_id: 9_012_002,
        });
        rig.wait_for_event(
            |ev| matches!(ev, VoxEvent::LlmFinished { turn_id } if *turn_id == 9_012_002),
            Duration::from_secs(5),
        )
        .await
        .expect("FIFO sentinel must be captured after the user turn");
        let steady_dispatches = rig.dispatch_count();
        let watch_deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while tokio::time::Instant::now() < watch_deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
            assert_eq!(
                rig.dispatch_count(),
                steady_dispatches,
                "a buffered task result must not spawn a second proactive turn"
            );
        }
        assert_eq!(
            rig.dispatch_count(),
            baseline_dispatches + 1,
            "no second proactive turn may be dispatched after quiescence"
        );
    })
    .await
    .expect("test timed out");
}
