//! ============================================================================
//! transcript_ingestion_test.rs — Transcript ingestion seam
//! ============================================================================
//! Category  : Integration (Seam 6 — TranscriptFinal → accumulator → LLM request)
//! Component : pipeline/assistant/transcript.rs + pipeline/assistant/interrupt.rs
//!             + pipeline/assistant/{speech,mod}.rs + services/harness/steps.rs
//! Binary    : transcript_ingestion_test
//! Prereqs   : none. The LLM is a recording stub; no model is loaded.
//! Run       : cargo nextest run --test transcript_ingestion_test --release --test-threads=1
//! ============================================================================
//!
//! WHAT THIS SEAM OWNS
//! What the pipeline does with a finalized transcript: store it, merge a
//! stashed continuation prefix into it, suppress a duplicate turn, and dispatch
//! exactly one generation request. The decoder is another seam.
//! All events are delivered through the real `spawn_router` thread; no test
//! calls `pipeline::assistant::*` handlers directly.

mod common;

use std::{
    sync::{atomic::Ordering, mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use vox_lib::{
    core::{
        events::{InteractionMode, PipelineMode, VoxEvent},
        state::{AppState, InteractionOwner, InteractionState},
    },
    pipeline::router::spawn_router,
    services::{
        harness::{Harness, LlmExecutionParams, Role},
        llm::{actor::LlmResponse, LlmCommand},
    },
};


const DEADLINE: Duration = Duration::from_secs(10);

/// Requests seen by the LLM stub: (turn id, last user message).
type Recorded = Arc<Mutex<Vec<(u32, String)>>>;

struct Rig {
    state: Arc<AppState>,
    recorded: Recorded,
    event_tx: mpsc::Sender<VoxEvent>,
    llm_tx: mpsc::Sender<LlmCommand>,
    llm_join: Option<JoinHandle<()>>,
    router_join: Option<JoinHandle<()>>,
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
            ..
        } = common::harness::setup_pipeline_channels();

        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();
        common::harness::attach_mock_engine_with_llm_tts_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx.clone()),
            Some(tts_tx),
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

        let router_join = spawn_router(app.clone(), vox_event_rx).expect("router thread");
        drop(app);

        Self {
            state,
            recorded,
            event_tx: vox_event_tx,
            llm_tx,
            llm_join: Some(llm_join),
            router_join: Some(router_join),
            _paths: paths,
        }
    }

    /// Sends an event into the router's channel (the production entry seam).
    fn send(&self, event: VoxEvent) {
        self.event_tx.send(event).expect("router must accept the event");
    }

    /// Polls until `expected` state appears or the deadline elapses.
    fn wait_state(&self, expected: InteractionState, timeout: Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if self.state.pipeline.state() == expected {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// Polls until the accumulator holds `expected` (or the deadline elapses).
    fn wait_accumulator(&self, expected: &str, timeout: Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if self.state.pipeline_accumulator.lock().user_transcript() == expected {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// The user text of the generation request recorded for `turn_id`.
    fn recorded_query(&self, turn_id: u32) -> Option<String> {
        self.recorded
            .lock()
            .expect("recording lock")
            .iter()
            .find(|(t, _)| *t == turn_id)
            .map(|(_, text)| text.clone())
    }

    /// Waits until a generation request for `turn_id` is recorded.
    fn await_query(&self, turn_id: u32) -> String {
        let deadline = std::time::Instant::now() + DEADLINE;
        while std::time::Instant::now() < deadline {
            if let Some(text) = self.recorded_query(turn_id) {
                return text;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "no generation request for turn {} within the deadline",
            turn_id
        );
    }

    /// Waits until the assistant response of the running turn is populated.
    fn await_assistant_response(&self) -> String {
        let deadline = std::time::Instant::now() + DEADLINE;
        while std::time::Instant::now() < deadline {
            let text = self
                .state
                .pipeline_accumulator
                .lock()
                .assistant_response
                .clone();
            if !text.is_empty() {
                return text;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the turn did not complete within the deadline");
    }

    fn request_count(&self) -> usize {
        self.recorded.lock().expect("recording lock").len()
    }

    fn assert_no_generation(&self, label: &str) {
        assert_eq!(
            self.request_count(),
            0,
            "{label}: no generation request may be dispatched"
        );
    }

    /// Ordered stop for negative assertions. Shutdown is processed by the router and
    /// the stub LLM in FIFO order, so joining both proves every earlier event was
    /// fully handled before the caller checks for absent side effects.
    fn quiesce(&mut self) {
        let _ = self.event_tx.send(VoxEvent::Shutdown);
        if let Some(join) = self.router_join.take() {
            join.join().expect("router thread panicked");
        }
        let _ = self.llm_tx.send(LlmCommand::Shutdown);
        if let Some(join) = self.llm_join.take() {
            join.join().expect("llm thread panicked");
        }
    }

    fn accumulator(&self) -> String {
        self.state.pipeline_accumulator.lock().user_transcript()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.event_tx.send(VoxEvent::Shutdown);
        if let Some(join) = self.router_join.take() {
            join.join().expect("router thread panicked");
        }
        let _ = self.llm_tx.send(LlmCommand::Shutdown);
        if let Some(join) = self.llm_join.take() {
            join.join().expect("llm thread panicked");
        }
    }
}

/// Stub LLM worker: records the request and answers with one token.
fn stub_llm_worker(llm_rx: mpsc::Receiver<LlmCommand>, recorded: Recorded) {
    while let Ok(cmd) = llm_rx.recv() {
        match cmd {
            LlmCommand::Generate {
                request,
                turn_id,
                cancel,
                response_tx,
            } => {
                let user_text = request
                    .input
                    .messages
                    .iter()
                    .filter(|m| m.role == Role::User)
                    .map(|m| m.content.clone())
                    .next_back()
                    .unwrap_or_default();
                recorded
                    .lock()
                    .expect("recording lock")
                    .push((turn_id, user_text));

                if cancel.is_cancelled() {
                    let _ = response_tx.send(LlmResponse::Cancelled);
                } else {
                    let _ = response_tx.send(LlmResponse::Token("ack".to_string()));
                    let _ = response_tx.send(LlmResponse::Finished);
                }
            }
            LlmCommand::Shutdown => break,
            LlmCommand::Warmup { .. } => {}
        }
    }
}

/// PURPOSE:  A finalized transcript must become exactly one generation request that
///           carries the user's words.
/// SCENARIO: A transcript arrives while the pipeline is Thinking.
/// ASSERTS:  The accumulator holds the text and the LLM receives one request whose last
///           user message is that text.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Dispatch the trimmed text instead of the query.
#[tokio::test]
async fn transcript_is_stored_and_dispatched_to_the_llm() {
    let rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Thinking);

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: 7,
        text: "what is the capital of France".to_string(),
        owner: InteractionOwner::Assistant,
    });

    assert!(
        rig.wait_accumulator("what is the capital of France", DEADLINE),
        "the accumulator must hold the text"
    );
    assert_eq!(
        rig.await_query(7),
        "what is the capital of France",
        "the request must carry the user's words"
    );
}

/// PURPOSE:  An empty transcript is not a turn and must not reach the LLM.
/// ASSERTS:  The pipeline returns to Ready and nothing is dispatched.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `text.trim().is_empty()` guard.
#[tokio::test]
async fn empty_transcript_returns_to_ready_without_dispatch() {
    let rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Thinking);

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: 8,
        text: "   \n\t ".to_string(),
        owner: InteractionOwner::Assistant,
    });

    assert!(
        rig.wait_state(InteractionState::Ready, DEADLINE),
        "empty transcript must return the pipeline to Ready"
    );
    rig.assert_no_generation("empty transcript");
}

/// PURPOSE:  A transcript that arrives outside Thinking belongs to no turn.
/// ASSERTS:  The state is untouched, nothing is stored, nothing is dispatched.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `current_state != Thinking` guard.
#[tokio::test]
async fn transcript_outside_thinking_is_dropped() {
    let mut rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Listening);

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: 9,
        text: "speech during listening".to_string(),
        owner: InteractionOwner::Assistant,
    });
    rig.quiesce();

    assert_eq!(rig.state.pipeline.state(), InteractionState::Listening);
    assert!(rig.accumulator().is_empty());
    rig.assert_no_generation("transcript outside Thinking");
}

/// PURPOSE:  Realtime mode owns its own turn; the modular LLM must not see it.
/// ASSERTS:  The query is stored, the state stays Thinking, and nothing is dispatched.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Dispatch the harness turn in every pipeline mode.
#[tokio::test]
async fn realtime_mode_keeps_the_query_but_does_not_dispatch() {
    let rig = Rig::new().await;
    {
        let mut settings = rig.state.settings.write().unwrap();
        settings.interaction.pipeline_mode = PipelineMode::Realtime;
    }
    rig.state.pipeline.set_state(InteractionState::Thinking);

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: 10,
        text: "realtime user query".to_string(),
        owner: InteractionOwner::Assistant,
    });

    assert!(
        rig.wait_accumulator("realtime user query", DEADLINE),
        "realtime mode must still store the query"
    );

    assert_eq!(rig.state.pipeline.state(), InteractionState::Thinking);
    assert_eq!(rig.accumulator(), "realtime user query");
    rig.assert_no_generation("realtime mode");
}

/// PURPOSE:  The same question twice must not start a second turn.
/// SCENARIO: The harness history already holds the user turn, then the identical
///           transcript arrives again.
/// ASSERTS:  No generation request is dispatched, the accumulator is cleared and the
///           pipeline returns to Ready.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `is_duplicate_user_turn` pre-dispatch check.
///
/// NOTE: the filter compares against the last message of the harness history. A
/// completed turn leaves an assistant message last, so the duplicate window is the
/// time between the user turn and the answer.
#[tokio::test]
async fn duplicate_user_turn_is_suppressed_before_dispatch() {
    let rig = Rig::new().await;
    {
        let mut guard = rig.state.harness.lock();
        let harness = guard.as_mut().expect("Harness must be mounted");
        harness.push_user_turn("same question".to_string());
    }
    let turn = rig.state.pipeline.next_turn().0;
    rig.state.pipeline.set_state(InteractionState::Thinking);

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: turn,
        text: "same question".to_string(),
        owner: InteractionOwner::Assistant,
    });

    assert!(
        rig.wait_state(InteractionState::Ready, DEADLINE),
        "a duplicate suppressed turn must return to Ready"
    );

    assert_eq!(
        rig.request_count(),
        0,
        "a duplicate turn must not reach the LLM"
    );
    assert!(
        rig.accumulator().is_empty(),
        "a suppressed duplicate must not stay in the accumulator"
    );
    assert_eq!(rig.state.pipeline.state(), InteractionState::Ready);
}

/// PURPOSE:  A user who keeps talking after a mid-thought interruption must end up
///           with one combined query (RULE-EVT-08A).
/// SCENARIO: Turn N speaks and completes, the user interrupts, turn N+1 speaks.
/// ASSERTS:  The second request carries "first part second part".
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the prefix in `resolve_merged_transcript`.
#[tokio::test]
async fn continuation_prefix_is_merged_into_the_next_query() {
    let rig = Rig::new().await;
    let first_turn = rig.state.pipeline.next_turn().0;
    rig.state.pipeline.set_state(InteractionState::Thinking);

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: first_turn,
        text: "first part".to_string(),
        owner: InteractionOwner::Assistant,
    });
    rig.await_query(first_turn);
    // Wait for the turn to finish: a turn that completes late clears the prefix.
    rig.await_assistant_response();

    // The user starts speaking again while the assistant is still thinking.
    rig.send(VoxEvent::SpeechStart {
        owner: InteractionOwner::Assistant,
    });
    assert!(
        rig.wait_state(InteractionState::Listening, DEADLINE),
        "SpeechStart must switch Thinking -> Listening"
    );
    rig.send(VoxEvent::SpeechEnd {
        owner: InteractionOwner::Assistant,
    });
    assert!(
        rig.wait_state(InteractionState::Thinking, DEADLINE),
        "SpeechEnd must switch Listening -> Thinking"
    );
    let second_turn = rig.state.pipeline.peek_turn_id();

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: second_turn,
        text: "second part".to_string(),
        owner: InteractionOwner::Assistant,
    });

    assert_eq!(
        rig.await_query(second_turn),
        "first part second part",
        "the stashed prefix and the new words must be one query"
    );
}

/// PURPOSE:  A final that lands after the interruption must be preserved, not dropped
///           (RULE-EVT-08A, late-arriving final).
/// SCENARIO: The user is interrupted before the first final arrived; the late final then
///           lands while the pipeline is Listening.
/// ASSERTS:  The late text is stashed as the continuation prefix and nothing is dispatched.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Drop the `awaiting_continuation` capture branch.
#[tokio::test]
async fn late_arriving_final_is_captured_as_a_continuation_prefix() {
    let rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Thinking);

    // No user transcript yet: the previous final is still in flight.
    rig.send(VoxEvent::SpeechStart {
        owner: InteractionOwner::Assistant,
    });
    assert!(
        rig.wait_state(InteractionState::Listening, DEADLINE),
        "SpeechStart must switch Thinking -> Listening"
    );
    assert!(
        rig.state.pipeline_accumulator.lock().awaiting_continuation,
        "the accumulator must await the late final"
    );

    // The late final belongs to the interrupted turn, one before the current one.
    rig.send(VoxEvent::TranscriptFinal {
        turn_id: rig.state.pipeline.peek_turn_id() - 1,
        text: "late words".to_string(),
        owner: InteractionOwner::Assistant,
    });

    let deadline = std::time::Instant::now() + DEADLINE;
    let mut prefix_stashed = false;
    while std::time::Instant::now() < deadline {
        if rig.state.pipeline_accumulator.lock().continuation_prefix == "late words" {
            prefix_stashed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        prefix_stashed,
        "the late final must be stashed as the continuation prefix"
    );
    rig.assert_no_generation("late-arriving final");
}

/// PURPOSE:  An empty next turn must promote the stashed prefix instead of aborting
///           (RULE-EVT-08A).
/// SCENARIO: A prefix is stashed, then the next turn has no words.
/// ASSERTS:  The prefix is dispatched as the query and the pipeline does not go Ready.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Drop the prefix promotion in the empty-transcript branch.
#[tokio::test]
async fn empty_transcript_promotes_the_stashed_prefix() {
    let rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Thinking);

    rig.send(VoxEvent::SpeechStart {
        owner: InteractionOwner::Assistant,
    });
    assert!(
        rig.wait_state(InteractionState::Listening, DEADLINE),
        "SpeechStart must switch Thinking -> Listening"
    );
    rig.send(VoxEvent::TranscriptFinal {
        turn_id: rig.state.pipeline.peek_turn_id() - 1,
        text: "late words".to_string(),
        owner: InteractionOwner::Assistant,
    });
    rig.send(VoxEvent::SpeechEnd {
        owner: InteractionOwner::Assistant,
    });
    assert!(
        rig.wait_state(InteractionState::Thinking, DEADLINE),
        "SpeechEnd must switch Listening -> Thinking"
    );
    let current_turn = rig.state.pipeline.peek_turn_id();

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: current_turn,
        text: "   ".to_string(),
        owner: InteractionOwner::Assistant,
    });

    assert_eq!(
        rig.await_query(current_turn),
        "late words",
        "the stashed prefix must be dispatched"
    );
}

/// PURPOSE:  A cancel left over from the previous turn must not block the next one.
/// SCENARIO: The turn token is cancelled, then a new turn starts and speaks.
/// ASSERTS:  The new turn is dispatched.
/// CLASS:    regression (F-5, stale cancel)
/// REQUIRES: none
/// MUTANT:   Drop the token re-arm in `next_turn`.
#[tokio::test]
async fn stale_cancel_does_not_block_the_next_turn() {
    let rig = Rig::new().await;
    rig.state.pipeline.turn_token().cancel();

    rig.send(VoxEvent::SpeechStart {
        owner: InteractionOwner::Assistant,
    });
    assert!(
        rig.wait_state(InteractionState::Listening, DEADLINE),
        "SpeechStart must reset the cancelled token and open Listening"
    );
    rig.send(VoxEvent::SpeechEnd {
        owner: InteractionOwner::Assistant,
    });
    assert!(
        rig.wait_state(InteractionState::Thinking, DEADLINE),
        "SpeechEnd must switch Listening -> Thinking"
    );
    let current_turn = rig.state.pipeline.peek_turn_id();

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: current_turn,
        text: "after a stale cancel".to_string(),
        owner: InteractionOwner::Assistant,
    });

    assert_eq!(
        rig.await_query(current_turn),
        "after a stale cancel",
        "the new turn must be dispatched"
    );
}

/// PURPOSE:  Devanagari must be transliterated before it reaches the model.
/// ASSERTS:  With the engine available the dispatched query is not Devanagari.
/// CLASS:    happy-path
/// REQUIRES: the transliteration model for the transliterated branch; the fallback branch
///            runs without it.
/// MUTANT:   Skip `transliterate_if_hi` before dispatch.
#[tokio::test]
async fn transcript_is_transliterated_before_dispatch() {
    let rig = Rig::new().await;
    {
        let mut settings = rig.state.settings.write().unwrap();
        settings.stt.transliterate_enabled = true;
    }
    rig.state.pipeline.set_state(InteractionState::Thinking);
    let engine_available = vox_lib::services::translit::init_transliteration_engine().is_ok();

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: 19,
        text: "नमस्ते वोक्स".to_string(),
        owner: InteractionOwner::Assistant,
    });

    let query = rig.await_query(19);
    let has_devanagari = query
        .chars()
        .any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
    if engine_available {
        assert!(
            !has_devanagari,
            "the dispatched query must be transliterated, got {:?}",
            query
        );
    } else {
        assert!(
            has_devanagari,
            "without the model the raw script must reach the model, got {:?}",
            query
        );
    }
}

/// PURPOSE:  An interrupt must cancel the running turn and open a new one.
/// ASSERTS:  A new turn id is allocated and the state becomes Listening.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Keep the old turn id after an interrupt.
#[tokio::test]
async fn interrupt_opens_a_new_turn() {
    let rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Thinking);
    let interrupted = rig.state.pipeline.peek_turn_id();

    rig.send(VoxEvent::SpeechStart {
        owner: InteractionOwner::Assistant,
    });

    assert!(
        rig.wait_state(InteractionState::Listening, DEADLINE),
        "SpeechStart in Thinking must interrupt and switch to Listening"
    );
    let new_turn = rig.state.pipeline.peek_turn_id();
    assert_ne!(new_turn, interrupted);
}

/// PURPOSE:  A transcript dropped in the wrong state is not stored or dispatched.
/// ASSERTS:  Nothing is stored, nothing is dispatched.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Store the text before the state guard.
#[tokio::test]
async fn dropped_transcript_is_not_dispatched_or_stored() {
    let mut rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Listening);

    rig.send(VoxEvent::TranscriptFinal {
        turn_id: 20,
        text: "dropped".to_string(),
        owner: InteractionOwner::Assistant,
    });
    rig.quiesce();

    rig.assert_no_generation("dropped transcript must not dispatch");
    assert!(
        rig.accumulator().is_empty(),
        "dropped transcript must not be stored"
    );
}
