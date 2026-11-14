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
use vox_lib::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::{AudioIntent, PipelineMode, VoxEvent},
        state::InteractionState,
    },
    services::{
        harness::{
            chassis::Harness, r#loop::MAX_TOOL_STEPS, stages::tools::ToolFilter,
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
