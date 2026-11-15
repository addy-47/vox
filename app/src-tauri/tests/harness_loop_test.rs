//! ============================================================================
//! harness_loop_test.rs — Canonical Harness Turn Loop & Pipeline Seam
//! ============================================================================
//! Category  : Integration (Seam 12 — Harness turn loop → dispatch → commit)
//! Component : services/harness/loop.rs + services/harness/steps.rs
//!             + services/harness/chassis.rs + services/harness/stages/streaming/router.rs
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
        events::{AudioIntent, PipelineMode, VoxEvent},
        state::InteractionState,
    },
    services::{
        harness::{
            chassis::Harness,
            r#loop::MAX_TOOL_STEPS,
            stages::tools::{
                ToolDefinition, ToolError, ToolExecutionContext, ToolFilter, ToolResult,
                TruncationPolicy,
            },
            Role, TurnOutcome,
        },
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
            vox_lib::services::harness::LlmExecutionParams {
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
    rig.state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);

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
    let execute_handle = tokio::spawn(async move {
        Harness::execute_turn(&harness_arc, req).await
    });

    let tts_rx = Arc::clone(&rig.tts_rx);
    let filler_cmd = tokio::task::spawn_blocking(move || {
        tts_rx.lock().recv_timeout(Duration::from_secs(10))
    })
    .await
    .expect("join")
    .expect("Expected filler TtsCommand::Generate on critical threshold");

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
        rig.state.pipeline.state(),
        InteractionState::Working,
        "state must transition to Working when critical compaction filler is dispatched"
    );
    assert!(
        rig.state.pipeline.pending_synthesis_jobs.load(Ordering::Relaxed) >= 1,
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
/// ASSERTS:  tts_rx receives only the spoken clauses without thinking text; accumulator holds only assistant response text.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   router.rs: Route Thinking deltas directly into handle_token.
#[tokio::test]
async fn zero_tool_turn_demuxes_thinking_tokens_into_quarantine_without_tts_or_ipc() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 101;

        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::Thinking("Step 1: Analyzing user query.\nStep 2: Formulating response.".to_string()));
            let _ = tx.send(LlmResponse::Token("The weather today is sunny and clear.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("How is the weather today?", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed { turn_id: tid, assistant_response } => {
                assert_eq!(tid, turn_id);
                assert_eq!(assistant_response, "The weather today is sunny and clear.");
                assert!(!assistant_response.contains("Analyzing user query"), "thinking tokens leaked into assistant_response");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let tts_cmds = rig.drain_tts_commands();
        assert!(!tts_cmds.is_empty(), "Spoken text must be dispatched to TTS");
        for cmd in tts_cmds {
            if let TtsCommand::Generate { text, intent, .. } = cmd {
                assert!(!text.contains("Analyzing user query"), "Thinking text leaked into TTS: {}", text);
                assert!(!text.contains("Step 1"), "Reasoning leaked into TTS: {}", text);
                assert_eq!(intent, AudioIntent::InterimFiller, "Preamble/initial clauses in allow_tools have InterimFiller intent");
            }
        }

        let acc_text = rig.state.pipeline_accumulator.lock().assistant_response.clone();
        assert_eq!(acc_text, "The weather today is sunny and clear.");
        assert!(!acc_text.contains("Step 1"), "Accumulator must quarantine thinking tokens");
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

        rig.state.pipeline.set_state(InteractionState::Thinking);
        rig.state.pipeline.set_turn_open(true);
        rig.state.pipeline.pending_synthesis_jobs.store(1, Ordering::Relaxed);

        // Signal LlmFinished via authentic production routing while 1 pending synthesis job is active
        rig.send_event(VoxEvent::LlmFinished { turn_id });
        rig.wait_for_event(|ev| matches!(ev, VoxEvent::LlmFinished { turn_id: tid } if *tid == turn_id), Duration::from_secs(5)).await.expect("LlmFinished routed");

        assert!(
            rig.wait_for_state(InteractionState::Speaking, Duration::from_secs(5)).await,
            "State must transition to Speaking while audio synthesis is in flight"
        );
        assert!(!rig.state.pipeline.is_turn_open(), "Key 1: turn_open must be false after completion");

        // Calling playback_finished via authentic VoxEvent while 1 job is pending must defer Ready
        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id,
            intent: AudioIntent::InterimFiller,
        });
        rig.wait_for_event(|ev| matches!(ev, VoxEvent::PlaybackFinished { turn_id: tid, .. } if *tid == turn_id), Duration::from_secs(5)).await.expect("PlaybackFinished routed");
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Speaking,
            "State must remain Speaking while pending_synthesis_jobs > 0"
        );

        // Now clear pending jobs (Key 2 satisfied) and signal sound card playback finished
        rig.state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);
        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id,
            intent: AudioIntent::InterimFiller,
        });

        assert!(
            rig.wait_for_state(InteractionState::Ready, Duration::from_secs(5)).await,
            "Two-Key Latch must transition deterministically to Ready when audio drains"
        );
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Prevent sound card buffer underrun from kicking the pipeline to Ready while the turn is open.
/// SCENARIO: A clause finishes playing on the sound card (PlaybackFinished) while turn_open == true.
/// ASSERTS:  Router returns early without transitioning to Ready; pipeline state remains stable in Thinking/Working.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   playback.rs: Remove 'if state.pipeline.is_turn_open() { return; }'.
#[tokio::test]
async fn playback_finished_during_open_turn_does_not_prematurely_transition_to_ready() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 103;

        rig.state.pipeline.set_state(InteractionState::Thinking);
        rig.state.pipeline.set_turn_open(true);
        rig.state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);

        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id,
            intent: AudioIntent::InterimFiller,
        });
        rig.wait_for_event(|ev| matches!(ev, VoxEvent::PlaybackFinished { turn_id: tid, .. } if *tid == turn_id), Duration::from_secs(5)).await.expect("PlaybackFinished routed");

        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Thinking,
            "Pipeline state must NOT transition to Ready while turn_open is true"
        );
        assert!(rig.state.pipeline.is_turn_open());
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
/// SCENARIO: Preamble playback fires PlaybackStarted and PlaybackFinished while turn_open == true.
/// ASSERTS:  Pipeline remains stably in Working state throughout interim audio playback.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   playback.rs: Remove intent == InterimFiller guard in on_playback_started.
#[tokio::test]
async fn preamble_audio_playback_retains_working_state_during_tool_execution() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 202;

        rig.state.pipeline.set_state(InteractionState::Working);
        rig.state.pipeline.set_turn_open(true);

        // Onset of interim filler audio must keep state in Working
        rig.send_event(VoxEvent::PlaybackStarted {
            turn_id,
            intent: AudioIntent::InterimFiller,
        });
        rig.wait_for_event(|ev| matches!(ev, VoxEvent::PlaybackStarted { turn_id: tid, .. } if *tid == turn_id), Duration::from_secs(5)).await.expect("PlaybackStarted routed");
        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Working,
            "PlaybackStarted with InterimFiller must NOT transition state out of Working"
        );

        // Completion of interim filler audio while turn_open == true must remain in Working
        rig.send_event(VoxEvent::PlaybackFinished {
            turn_id,
            intent: AudioIntent::InterimFiller,
        });
        rig.wait_for_event(|ev| matches!(ev, VoxEvent::PlaybackFinished { turn_id: tid, .. } if *tid == turn_id), Duration::from_secs(5)).await.expect("PlaybackFinished routed");
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

        let req = rig.turn_request("Find my test facts", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed { assistant_response, .. } => {
                assert_eq!(assistant_response, "Memory retrieved successfully.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let dispatches = rig.requests();
        assert_eq!(dispatches.len(), 2, "Pass 1 tool call, Pass 2 response");
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
            let _ = tx.send(LlmResponse::Token("I have checked all three items.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Check three different memory queries", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed { assistant_response, .. } => {
                assert_eq!(assistant_response, "I have checked all three items.");
            }
            other => panic!("Expected Completed outcome, got {:?}", other),
        }

        let dispatches = rig.requests();
        assert_eq!(dispatches.len(), 2, "Must be exactly 2 passes (batch pass + final synthesis)");

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
/// ASSERTS:  Session title is persisted in DB; title_set is latched so set_title is suppressed on Turn 2.
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
            let _ = tx.send(LlmResponse::Token("Session initialized with concise title.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Initialize session title please", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        assert!(matches!(outcome, TurnOutcome::Completed { .. }));

        {
            let guard = rig.state.harness.lock();
            let harness = guard.as_ref().expect("Harness must be mounted");
            assert!(harness.title_set(), "harness.title_set must be latched to true");

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

        let title = rig.wait_for_session_title("Cognitive Architecture Spec", Duration::from_secs(5)).await;
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
/// ASSERTS:  TurnOutcome is Cancelled; turn_open is cleared to false.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   steps.rs: Ignore cancellation token check prior to tool execution.
#[tokio::test]
async fn turn_cancellation_during_tool_execution_aborts_cleanly_and_clears_turn_open() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 401;

        // Initialize turn_open to true to verify that cancellation resets it on the router thread
        rig.state.pipeline.set_turn_open(true);

        let cancel_token = rig.state.pipeline.turn_token();
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

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline && rig.state.pipeline.is_turn_open() {
            tokio::task::yield_now().await;
        }

        assert!(!rig.state.pipeline.is_turn_open(), "turn_open must be reset to false on cancellation");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Barge-in with spoken partial speech must commit user query and partial assistant text.
/// SCENARIO: User cancels turn after assistant has emitted partial speech.
/// ASSERTS:  Assistant text is recorded in DB turns table; history is committed with partial response.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   loop.rs: Drop assistant partial text on cancellation when partial_text is non-empty.
#[tokio::test]
async fn turn_cancellation_with_partial_spoken_text_commits_interrupted_turn() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 402;
        let cancel_token = rig.state.pipeline.turn_token();
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
            let _ = tx.send(LlmResponse::Token("Partial sentence before interruption.".to_string()));
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

        let recorded_text = rig.wait_for_turn_assistant_text(Duration::from_secs(5)).await;
        assert!(
            recorded_text.as_deref().map(|t| t.contains("Partial sentence")).unwrap_or(false),
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

        rig.state.pipeline.turn_token().cancel();

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
            .query("SELECT COUNT(*) FROM turns WHERE session_id = ? AND turn_id = ?;", (rig.session_id, turn_id as i64))
            .await
            .expect("query turns");
        let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 0, "Zero turns must be persisted when cancelled before speech");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Verify stream errors release turn_open latch and dispatch VoxEvent::Error.
/// SCENARIO: Model returns LlmResponse::Error due to upstream failure.
/// ASSERTS:  TurnOutcome is Error; turn_open is reset to false; VoxEvent::Error is emitted on event channel.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   router.rs: Do not reset turn_open or omit VoxEvent::Error on stream error.
#[tokio::test]
async fn stream_error_clears_turn_open_and_dispatches_error_event() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 404;

        // Initialize turn_open to true to verify that error handling resets it on the router thread
        rig.state.pipeline.set_turn_open(true);

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

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline && rig.state.pipeline.is_turn_open() {
            tokio::task::yield_now().await;
        }

        assert!(!rig.state.pipeline.is_turn_open(), "turn_open must be false after error");
    })
    .await
    .expect("test timed out");
}

// ============================================================================
// Section 7: Domain 2 — Cognitive Loop, Concurrency & Immediate Step Commits
// ============================================================================

/// PURPOSE:  Catch loss of completed tool observations or accidental deletion of the user prompt when a turn is cancelled after Step 1 completes.
/// SCENARIO: Step 1 executes [set_title, search_memory] and commits; during Step 2 tool execution, `cancel_token.cancel()` fires.
/// ASSERTS:  TurnOutcome is Cancelled; `harness.messages()` retains the User message, Step 1 Assistant(tool_calls) + 2 Tool observations, and Step 2 Assistant(tool_calls) + synthesized cancelled Tool observation (zero orphaned tool_call_ids); `state.pipeline.is_turn_open()` is false after router processes VoxEvent::Cancelled.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   In `ConversationHistoryStage::rollback_last_user_turn` (`history.rs`), remove the `if last.role == Role::User` guard so it pops unconditionally, OR move `commit_tool_step` back to `step7_commit_completed`.
#[tokio::test]
async fn mid_turn_cancellation_on_step_two_preserves_step_one_history_and_forbids_user_rollback() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 501;

        rig.state.pipeline.set_turn_open(true);
        let cancel_token = rig.state.pipeline.turn_token();
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
            cancel_clone.cancel();
            let _ = tx.send(LlmResponse::Finished);
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

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline && rig.state.pipeline.is_turn_open() {
            tokio::task::yield_now().await;
        }
        assert!(!rig.state.pipeline.is_turn_open(), "turn_open must be false after cancellation");

        // Assert conversation history invariants
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let messages = harness.messages();

        // 1. User message must be preserved (rollback forbidden after step 1 execution!)
        let user_msg = messages.iter().find(|m| m.role == Role::User);
        assert!(
            user_msg.is_some(),
            "User query must remain in conversation history; rollback_last_user_turn must NOT pop answered turns!"
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

/// PURPOSE:  Catch stale unanswered Role::User turns polluting history when the user barges in during Step 0 thinking before any preamble speech or tool execution.
/// SCENARIO: Model streams Thinking tokens in Step 0 and `cancel_token.cancel()` triggers before any Token or ToolCall is emitted.
/// ASSERTS:  TurnOutcome is Cancelled; `rollback_last_user_turn` pops the unanswered Role::User message leaving only Role::System in `harness.messages()`; `turn_open` is false.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   In `step7_handle_cancelled` (`steps.rs`), delete the call to `harness.rollback_last_user_turn()`.
#[tokio::test]
async fn step_zero_cancellation_before_speech_or_tools_rolls_back_unanswered_user_turn() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let rig = Rig::new().await;
        let turn_id = 502;

        rig.state.pipeline.set_turn_open(true);
        let cancel_token = rig.state.pipeline.turn_token();
        let cancel_clone = cancel_token.clone();

        // Step 0: Model streams thinking tokens and gets cancelled before any token or tool call
        rig.scripted_llm.push_step(move |tx| {
            let _ = tx.send(LlmResponse::Thinking("Thinking deeply in Step 0...".to_string()));
            cancel_clone.cancel();
            let _ = tx.send(LlmResponse::Finished);
        });

        let query = "Unanswered query cancelled during Step 0 thinking";
        let req = rig.turn_request(query, turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Cancelled { .. } => {}
            other => panic!("Expected Cancelled outcome on step 0 cancel, got {:?}", other),
        }

        // Wait for router thread to process VoxEvent::Cancelled
        let _ = rig
            .wait_for_event(
                |ev| matches!(ev, VoxEvent::Cancelled { turn_id: tid, .. } if *tid == turn_id),
                Duration::from_secs(5),
            )
            .await
            .expect("VoxEvent::Cancelled must be emitted on event channel");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline && rig.state.pipeline.is_turn_open() {
            tokio::task::yield_now().await;
        }
        assert!(!rig.state.pipeline.is_turn_open(), "turn_open must be false after cancellation");

        // Assert unanswered user turn was rolled back
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let messages = harness.messages();

        let has_user_msg = messages.iter().any(|m| m.role == Role::User);
        assert!(
            !has_user_msg,
            "Unanswered user turn must be rolled back on Step 0 cancellation before speech or tools!"
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
                vox_lib::services::harness::ToolExecutionMode::Parallel,
                Some(barrier.clone()),
                trace.clone(),
            )
            .with_yields(4),
        );

        let p2 = Arc::new(
            common::harness::BarrierTraceTool::new(
                "P2",
                vox_lib::services::harness::ToolExecutionMode::Parallel,
                Some(barrier.clone()),
                trace.clone(),
            )
            .with_yields(2),
        );

        let p3 = Arc::new(
            common::harness::BarrierTraceTool::new(
                "P3",
                vox_lib::services::harness::ToolExecutionMode::Parallel,
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
            let _ = tx.send(LlmResponse::Token("All parallel tools executed.".to_string()));
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
        assert_eq!(t.len(), 6, "Expected 3 starts and 3 ends in trace log, got {:?}", t);
        assert!(t[0].starts_with("start:"), "Entry 0 must be start:*, got {}", t[0]);
        assert!(t[1].starts_with("start:"), "Entry 1 must be start:*, got {}", t[1]);
        assert!(t[2].starts_with("start:"), "Entry 2 must be start:*, got {}", t[2]);

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
        assert_eq!(tool_msgs.len(), 3, "All 3 tools must be committed to history");
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
            vox_lib::services::harness::ToolExecutionMode::Parallel,
            Some(wave1_barrier.clone()),
            trace.clone(),
        ));
        let p2 = Arc::new(common::harness::BarrierTraceTool::new(
            "P2",
            vox_lib::services::harness::ToolExecutionMode::Parallel,
            Some(wave1_barrier.clone()),
            trace.clone(),
        ));
        let e1 = Arc::new(common::harness::BarrierTraceTool::new(
            "E1",
            vox_lib::services::harness::ToolExecutionMode::Exclusive,
            None,
            trace.clone(),
        ));
        let p3 = Arc::new(common::harness::BarrierTraceTool::new(
            "P3",
            vox_lib::services::harness::ToolExecutionMode::Parallel,
            Some(wave2_barrier.clone()),
            trace.clone(),
        ));
        let p4 = Arc::new(common::harness::BarrierTraceTool::new(
            "P4",
            vox_lib::services::harness::ToolExecutionMode::Parallel,
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
            let _ = tx.send(LlmResponse::Token("Batch with exclusive barrier executed.".to_string()));
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
        assert!(p1_end < e1_start, "P1 must complete before E1 starts (p1_end={}, e1_start={})", p1_end, e1_start);
        assert!(p2_end < e1_start, "P2 must complete before E1 starts (p2_end={}, e1_start={})", p2_end, e1_start);

        // end:E1 is recorded before either start:P3 or start:P4
        assert!(e1_end < p3_start, "E1 must complete before P3 starts (e1_end={}, p3_start={})", e1_end, p3_start);
        assert!(e1_end < p4_start, "E1 must complete before P4 starts (e1_end={}, p4_start={})", e1_end, p4_start);

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
/// ASSERTS:  LLM worker receives 4 requests total; requests 0..=2 have non-empty `tools`, request 3 has `tools.is_empty()` (or `None`); TurnOutcome is Completed with the final summary text dispatched to TTS as `AudioIntent::TurnResponse`; all 3 tool steps (6 messages) + final Assistant message are in `harness.messages()`.
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
            let _ = tx.send(LlmResponse::Token("Final synthesized summary after reaching step limit.".to_string()));
            let _ = tx.send(LlmResponse::Finished);
        });

        let req = rig.turn_request("Exhaust tool step limit", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;

        match outcome {
            TurnOutcome::Completed { ref assistant_response, .. } => {
                assert_eq!(
                    assistant_response,
                    "Final synthesized summary after reaching step limit."
                );
            }
            other => panic!("Expected Completed outcome after forced summary pass, got {:?}", other),
        }

        // Assert: LLM worker received 4 requests total; requests 0..=2 have non-empty tools, request 3 has false (tool-free)
        let requests = rig.requests();
        assert_eq!(requests.len(), 4, "Expected 4 dispatches (3 tool steps + 1 summary pass), got {}", requests.len());
        assert!(requests[0].1, "Pass 0 must offer tools");
        assert!(requests[1].1, "Pass 1 must offer tools");
        assert!(requests[2].1, "Pass 2 must offer tools");
        assert!(!requests[3].1, "Pass 3 (summary pass) must be tool-free (allow_tools == false)");

        // Assert: TTS received AudioIntent::TurnResponse
        let tts_cmds = rig.drain_tts_commands();
        let turn_response_cmd = tts_cmds.iter().find(|cmd| {
            matches!(cmd, TtsCommand::Generate { intent: AudioIntent::TurnResponse, text, .. }
                if text.contains("Final synthesized summary"))
        });
        assert!(turn_response_cmd.is_some(), "Summary pass must dispatch TurnResponse to TTS");

        // Assert: Conversation history holds all 3 tool steps (3 Assistant + 3 Tool) + final Assistant summary
        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert_eq!(tool_msgs.len(), 3, "All 3 tool observations must be committed");
        let assistant_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .collect();
        assert_eq!(assistant_msgs.len(), 4, "3 intermediate tool calls + 1 final summary pass");
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
            let _ = tx.send(LlmResponse::Token("Synthesized summary after circuit break.".to_string()));
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
        assert!(!requests[4].1, "Pass 4 (summary pass) must be tool-free (allow_tools == false)");

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
            let _ = tx.send(LlmResponse::Token("Summary after ping pong break.".to_string()));
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
        assert!(requests[0..=4].iter().all(|(_, tools)| *tools), "Steps 0..=4 must offer tools");
        assert!(!requests[5].1, "Pass following Strike 2 must have allow_tools == false");

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert!(tool_msgs.len() >= 4, "At least 4 tool observations must be committed");
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
            let _ = tx.send(LlmResponse::Token("Summary after consecutive error circuit break.".to_string()));
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
        assert!(requests[0..=3].iter().all(|(_, tools)| *tools), "Steps 0..=3 must offer tools");
        assert!(!requests[4].1, "Pass 4 must have allow_tools == false");

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let tool_msgs: Vec<_> = harness
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert!(tool_msgs.len() >= 3, "At least 3 tool observations must be committed");
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
/// ASSERTS:  Committed observation in history is <= max_observation_tokens (+ marker overhead), contains "HEAD_SENTINEL_LINE_1", "TAIL_SENTINEL_LINE_500", "Output truncated", and does NOT contain "MIDDLE_SECRET_PAYLOAD". The file at paths::session_tool_outputs_dir(rig.session_id).join("<call_id>.txt") exists and its full UTF-8 content equals the original 10,000-token string.
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
            let _ = tx.send(LlmResponse::Token("Turn completed after tool execution.".to_string()));
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
        assert!(obs.contains("HEAD_SENTINEL_LINE_1"), "Observation must preserve head sentinel");
        assert!(obs.contains("TAIL_SENTINEL_LINE_500"), "Observation must preserve tail sentinel");
        assert!(obs.contains("Output truncated"), "Observation must contain truncation notice marker");
        assert!(!obs.contains("MIDDLE_SECRET_PAYLOAD"), "Observation must NOT contain stripped middle payload");

        // Verify Tier 4 spillover file exists on disk
        let spillover_file = vox_lib::utils::paths::session_tool_outputs_dir(rig.session_id)
            .join(format!("{}.txt", call_id));
        assert!(
            spillover_file.exists(),
            "Spillover cache file must exist at {:?}",
            spillover_file
        );
        let spilled_content = std::fs::read_to_string(&spillover_file).expect("read spillover file");
        assert!(spilled_content.contains("HEAD_SENTINEL_LINE_1"));
        assert!(spilled_content.contains("MIDDLE_SECRET_PAYLOAD"));
        assert!(spilled_content.contains("TAIL_SENTINEL_LINE_500"));
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Ensure tools declaring TruncationPolicy::None bypass truncation.
/// SCENARIO: Tool with TruncationPolicy::None returns payload larger than max_observation_tokens.
/// ASSERTS:  Committed observation equals the exact unmodified string; zero files created under paths::session_tool_outputs_dir(rig.session_id); session_tool_calls records truncated = 0.
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
            assert!(!obs.contains("Output truncated"), "Truncation notice must NOT appear for Policy::None");
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
        assert_eq!(file_count, 0, "Zero spillover files must be created for TruncationPolicy::None");

        // Provenance assert: session_tool_calls must record truncated = 0
        let conn = rig.state.db.connect().expect("db connect");
        let mut rows = conn
            .query(
                "SELECT truncated, spillover_path FROM session_tool_calls WHERE id = ?;",
                (call_id,),
            )
            .await
            .expect("query session_tool_calls truncation columns");
        if let Some(row) = rows.next().await.expect("fetch row") {
            let truncated: i64 = row.get(0).expect("truncated");
            let spillover_path: Option<String> = row.get(1).expect("spillover_path");
            assert_eq!(truncated, 0, "session_tool_calls must record truncated = 0 for TruncationPolicy::None");
            assert!(spillover_path.is_none(), "spillover_path must be NULL for TruncationPolicy::None");
        } else {
            panic!("Expected session_tool_calls row for tool call");
        }
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch mid-loop context overflow and prevent erasure of in-flight active turn steps during inline compaction.
/// SCENARIO: Seed prior turns to ~80% context budget; on Turn N Step 0, model executes tool pushing context >= 85% (Critical); before Step 1 LLM dispatch, inline compaction triggers.
/// ASSERTS:  State transition/activity records ActivityKind::Compaction; older prior turns compacted into <session_context>, while Turn N's Role::User query, Step 0 Role::Assistant(tool_calls), and Step 0 Role::Tool observation remain intact in harness.messages() for Step 1's request.
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
                vox_lib::services::harness::LlmExecutionParams {
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
            for i in 0..90 {
                harness.push_user_turn(format!(
                    "Turn {} historical context question about distributed cognitive loops and state machines.",
                    i
                ));
                harness.push_assistant_turn(format!(
                    "Turn {} historical answer explaining pipeline stages, token budgets, and memory compaction.",
                    i
                ));
            }
        }

        // Turn N Step 0: model calls a tool returning an observation that pushes total tokens >= 85% (Critical)
        rig.scripted_llm.push_step(|tx| {
            let _ = tx.send(LlmResponse::ToolCall(CanonicalToolCall {
                id: "call-step0-mid".to_string(),
                name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "in-flight investigation" }),
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

        let guard = rig.state.harness.lock();
        let harness = guard.as_ref().expect("Harness must be mounted");
        let messages = harness.messages();

        // 1. Verify older prior turns were compacted into session_context prompt
        let has_old_turn_0 = messages.iter().any(|m| m.content.contains("Turn 0 historical"));
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
    })
    .await
    .expect("test timed out");
}



