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

mod common;

use std::{
    sync::{
        atomic::Ordering,
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use vox_lib::{
    core::{
        events::{InteractionMode, PipelineMode, VoxEvent},
        state::{AppState, InteractionOwner, InteractionState},
    },
    pipeline::{
        assistant::{
            interrupt::on_interrupt, speech::on_speech_end, speech::on_speech_start,
            transcript::on_transcript_final,
        },
        RoutingContext,
    },
    services::{
        harness::{Harness, LlmExecutionParams, Role},
        llm::{
            actor::LlmResponse,
            LlmCommand,
        },
    },
};

type MockApp = tauri::AppHandle<tauri::test::MockRuntime>;

const DEADLINE: Duration = Duration::from_secs(10);

/// Requests seen by the LLM stub: (turn id, last user message).
type Recorded = Arc<Mutex<Vec<(u32, String)>>>;

struct Rig {
    app: MockApp,
    state: Arc<AppState>,
    recorded: Recorded,
    pipeline_rx: mpsc::Receiver<VoxEvent>,
    ctx: RoutingContext,
    llm_tx: mpsc::Sender<LlmCommand>,
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
            pipeline_rx,
        } = common::harness::setup_pipeline_channels();

        common::harness::attach_mock_engine_with_pipeline_tx_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx.clone()),
            Some(tts_tx),
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

        let recorded: Recorded = Arc::new(Mutex::new(Vec::new()));
        let worker_recorded = recorded.clone();
        let llm_join = std::thread::spawn(move || {
            stub_llm_worker(llm_rx, worker_recorded);
        });

        let ctx = RoutingContext::from_app_state(&state);
        Self {
            app,
            state,
            recorded,
            pipeline_rx,
            ctx,
            llm_tx,
            llm_join: Some(llm_join),
            _paths: paths,
        }
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
        panic!("no generation request for turn {} within the deadline", turn_id);
    }

    /// Waits until the assistant response of the running turn is populated.
    fn await_assistant_response(&self) -> String {
        let deadline = std::time::Instant::now() + DEADLINE;
        while std::time::Instant::now() < deadline {
            let text = self.state.pipeline_accumulator.lock().assistant_response.clone();
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

    fn accumulator(&self) -> String {
        self.state.pipeline_accumulator.lock().user_transcript()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.llm_tx.send(LlmCommand::Shutdown);
        if let Some(join) = self.llm_join.take() {
            let _ = join.join();
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

    on_transcript_final(
        7,
        "what is the capital of France".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

    assert_eq!(rig.accumulator(), "what is the capital of France");
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

    on_transcript_final(
        8,
        "   \n\t ".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

    assert_eq!(rig.state.pipeline.state(), InteractionState::Ready);
    rig.assert_no_generation("empty transcript");
}

/// PURPOSE:  A transcript that arrives outside Thinking belongs to no turn.
/// ASSERTS:  The state is untouched, nothing is stored, nothing is dispatched.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `current_state != Thinking` guard.
#[tokio::test]
async fn transcript_outside_thinking_is_dropped() {
    let rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Listening);

    on_transcript_final(
        9,
        "speech during listening".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

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
    rig.state.pipeline.set_state(InteractionState::Thinking);
    let mut realtime_ctx = rig.ctx.clone();
    realtime_ctx.pipeline_mode = PipelineMode::Realtime;

    on_transcript_final(
        10,
        "realtime user query".to_string(),
        &rig.app,
        &rig.state,
        &realtime_ctx,
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

    on_transcript_final(
        turn,
        "same question".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
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

    on_transcript_final(
        first_turn,
        "first part".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );
    rig.await_query(first_turn);
    // Wait for the turn to finish: a turn that completes late clears the prefix.
    rig.await_assistant_response();

    // The user starts speaking again while the assistant is still thinking.
    on_speech_start(&rig.app, &rig.state, &rig.ctx);
    assert_eq!(rig.state.pipeline.state(), InteractionState::Listening);
    on_speech_end(&rig.app, &rig.state, &rig.ctx);
    assert_eq!(rig.state.pipeline.state(), InteractionState::Thinking);
    let second_turn = rig.state.pipeline.peek_turn_id();

    on_transcript_final(
        second_turn,
        "second part".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

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
    on_speech_start(&rig.app, &rig.state, &rig.ctx);
    assert_eq!(rig.state.pipeline.state(), InteractionState::Listening);
    assert!(
        rig.state
            .pipeline_accumulator
            .lock()
            .awaiting_continuation,
        "the accumulator must await the late final"
    );

    // The late final belongs to the interrupted turn, one before the current one.
    on_transcript_final(
        rig.state.pipeline.peek_turn_id() - 1,
        "late words".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

    assert_eq!(
        rig.state.pipeline_accumulator.lock().continuation_prefix,
        "late words",
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

    on_speech_start(&rig.app, &rig.state, &rig.ctx);
    on_transcript_final(
        rig.state.pipeline.peek_turn_id() - 1,
        "late words".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );
    on_speech_end(&rig.app, &rig.state, &rig.ctx);
    assert_eq!(rig.state.pipeline.state(), InteractionState::Thinking);
    let current_turn = rig.state.pipeline.peek_turn_id();

    on_transcript_final(
        current_turn,
        "   ".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

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

    on_speech_start(&rig.app, &rig.state, &rig.ctx);
    assert_eq!(rig.state.pipeline.state(), InteractionState::Listening);
    on_speech_end(&rig.app, &rig.state, &rig.ctx);
    let current_turn = rig.state.pipeline.peek_turn_id();

    on_transcript_final(
        current_turn,
        "after a stale cancel".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

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

    on_transcript_final(
        19,
        "नमस्ते वोक्स".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

    let query = rig.await_query(19);
    let has_devanagari = query.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
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

    let new_turn = on_interrupt(&rig.app, &rig.state, &rig.ctx);

    assert_ne!(new_turn, interrupted);
    assert_eq!(rig.state.pipeline.state(), InteractionState::Listening);
    assert_eq!(rig.state.pipeline.peek_turn_id(), new_turn);
}

/// PURPOSE:  A dropped transcript must not emit a transcript event to the UI.
/// ASSERTS:  The pipeline event channel stays empty.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Emit the TranscriptFinal IPC before the state guard.
#[tokio::test]
async fn dropped_transcript_emits_no_pipeline_event() {
    let rig = Rig::new().await;
    rig.state.pipeline.set_state(InteractionState::Listening);

    on_transcript_final(
        20,
        "dropped".to_string(),
        &rig.app,
        &rig.state,
        &rig.ctx,
    );

    common::harness::assert_channel_empty(&rig.pipeline_rx, "dropped transcript");
}
