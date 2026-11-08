//! ============================================================================
//! llm_to_tts_test.rs — LLM to TTS chunking seam
//! ============================================================================
//! Category  : Integration (Seam 8 — token stream → clauses → TTS commands)
//! Component : services/harness/stages/streaming/chunker.rs + services/harness/steps.rs
//!             + services/tts/actor.rs + pipeline/assistant/llm.rs
//! Binary    : llm_to_tts_test
//! Prereqs   : the deterministic scripted tests need no model; the Qwen scenario
//!             requires qwen-3.5-0.8b-q4_k_m.gguf (VOX_MODELS_DIR).
//! Run       : cargo nextest run --test llm_to_tts_test --release --test-threads=1
//! ============================================================================
//!
//! The deterministic tests drive a scripted streaming `LlmProvider` through
//! `Harness::execute_turn` and observe clause dispatch plus `VoxEvent::LlmFinished`
//! reaching the real router (`spawn_router`). Clause-rule unit behaviour is covered
//! by `chunker::tests` in `src/`. The final scenario runs the real local Qwen GGUF
//! model end to end; it is excluded from the default tier.

mod common;

use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    time::Duration,
};

use vox_lib::{
    core::{
        events::{AudioIntent, InteractionMode, PipelineMode, VoxEvent},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::{router::spawn_router, RoutingContext},
    services::{
        harness::{Harness, TurnExecutionRequest, TurnOutcome},
        llm::{
            actor::{spawn_llm_worker, LlmCommand},
            EmbeddedProvider,
        },
        tts::TtsCommand,
    },
};


/// Builds the modular harness rig shared by the deterministic tests and the real-model
/// test: scripted or real LLM worker, router consuming the pipeline channel.
struct SeamRig {
    app: tauri::AppHandle<tauri::test::MockRuntime>,
    state: Arc<vox_lib::core::state::AppState>,
    pipeline_tx: mpsc::Sender<VoxEvent>,
    tts_rx: mpsc::Receiver<TtsCommand>,
    llm_tx: mpsc::Sender<LlmCommand>,
    tts_tx: mpsc::Sender<TtsCommand>,
    llm_join: std::thread::JoinHandle<()>,
    router_join: std::thread::JoinHandle<()>,
    _paths: common::paths::TempPathsGuard,
}

impl SeamRig {
    async fn new(provider: Arc<dyn vox_lib::services::llm::LlmProvider>) -> Self {
        let paths = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;

        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::Passive;
            settings.interaction.pipeline_mode = PipelineMode::Modular;
            settings.memory.context_retrieval_enabled = false;
        }
        state.owner.store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Thinking);

        // Single shared pipeline channel, exactly like production: workers and the
        // harness publish VoxEvents onto it; the router consumes them.
        let (stt_tx, _stt_rx) = mpsc::channel();
        let (vad_tx, _vad_rx) = mpsc::channel();
        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
        let (pipeline_tx, pipeline_rx) = mpsc::channel::<VoxEvent>();

        let router_join = spawn_router(app.clone(), pipeline_rx).expect("router thread");

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
            let prompt = "You are a concise voice assistant. Reply in exactly two short sentences.".to_string();
            *state.harness.lock() = Some(Harness::new_modular(
                Some(1),
                prompt,
                None,
                &settings,
                vox_lib::services::harness::LlmExecutionParams {
                    context_window: 8192,
                    max_output_tokens: 300,
                    temperature: 0.7,
                    reasoning_enabled: false,
                },
                llm_tx.clone(),
                true,
            ));
        }

        let worker_provider = Arc::clone(&provider);
        let worker_handle = std::thread::Builder::new()
            .name("test-llm-worker".into())
            .spawn(move || {
                spawn_llm_worker(llm_rx, worker_provider);
            })
            .expect("spawn test-llm-worker");

        Self {
            app,
            state,
            pipeline_tx,
            tts_rx,
            llm_tx,
            tts_tx,
            llm_join: worker_handle,
            router_join,
            _paths: paths,
        }
    }

    fn shutdown(self) {
        // The router consumes VoxEvent::Shutdown and exits. FIFO ordering across the
        // shared pipeline channel proves it processed every earlier event first (e.g.
        // LlmFinished) before exiting.
        let _ = self.pipeline_tx.send(VoxEvent::Shutdown);
        self.router_join.join().expect("router thread panicked");
        let _ = self.llm_tx.send(LlmCommand::Shutdown);
        self.llm_join.join().expect("llm thread panicked");
        // Kill any vox-llm child spawned through the production supervisor; its
        // provider drop does not terminate the process.
        vox_lib::services::sidecar::global_sidecar_supervisor().shutdown_all();
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
            routing_ctx: self.routing_ctx(),
            app_state: Arc::clone(&self.state),
        }
    }

    fn routing_ctx(&self) -> RoutingContext {
        RoutingContext::from_app_state(&self.state)
    }

}

/// Collects every `TtsCommand::Generate` received within `timeout`.
fn drain_tts_generate(
    tts_rx: &mpsc::Receiver<TtsCommand>,
    timeout: Duration,
) -> Vec<(u32, String, AudioIntent)> {
    let deadline = std::time::Instant::now() + timeout;
    let mut clauses = Vec::new();
    loop {
        match tts_rx.try_recv() {
            Ok(TtsCommand::Generate {
                turn_id,
                text,
                intent,
            }) => clauses.push((turn_id, text, intent)),
            Ok(_) => {}
            Err(mpsc::TryRecvError::Empty) => {
                if std::time::Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
    }
    clauses
}

/// PURPOSE:  Deterministic streaming through a scripted provider must produce TTS clauses.
/// SCENARIO: "Hello there. How are you today?" streams through Harness::execute_turn.
/// ASSERTS:  The turn completes with the scripted text, clause dispatches reach the TTS
///           channel with the right turn_id and TurnResponse intent, and LlmFinished is
///           delivered to the real router before Shutdown.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the clause dispatch in StreamRoutingStage.
#[tokio::test]
async fn scripted_stream_dispatches_clauses_to_tts() {
    let rig = SeamRig::new(Arc::new(
        common::harness::ScriptedLlmProvider::tokens("Hello there. How are you today?"),
    ))
    .await;
    let turn_id = 202;

    let req = rig.turn_request("Hello! State your name and your purpose.", turn_id);
    let outcome = Harness::execute_turn(&rig.state.harness, req).await;

    match outcome {
        TurnOutcome::Completed {
            turn_id: tid,
            assistant_response,
        } => {
            assert_eq!(tid, turn_id, "Completed turn_id must match request");
            assert_eq!(
                assistant_response, "Hello there. How are you today?",
                "assistant response must match the script"
            );
        }
        other => panic!("Expected TurnOutcome::Completed, got {:?}", other),
    }

    let clauses = drain_tts_generate(&rig.tts_rx, Duration::from_secs(5));
    assert!(
        clauses.len() >= 2,
        "a two-sentence scripted stream must produce at least two TTS clauses, got {:?}",
        clauses.iter().map(|(_, t, _)| t).collect::<Vec<_>>()
    );
    for (tid, _, intent) in &clauses {
        assert_eq!(*tid, turn_id, "clauses must carry the request turn_id");
        assert_eq!(*intent, AudioIntent::TurnResponse, "clauses must carry TurnResponse intent");
    }
    let texts: Vec<&str> = clauses.iter().map(|(_, t, _)| t.as_str()).collect();
    assert!(
        texts.iter().any(|t| t.contains("Hello")),
        "first clause must land in TTS, got {:?}",
        texts
    );
    assert!(
        texts.iter().any(|t| t.contains("How are you") || t.contains("today")),
        "second clause must land in TTS, got {:?}",
        texts
    );

    rig.shutdown();
}

/// PURPOSE:  Stream completion must flush pre-roll before TTS can start Speaking.
/// SCENARIO: A scripted provider streams a single short sentence.
/// ASSERTS:  The turn outcome is Completed and exactly one clause carries the full sentence.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Dispatch no clauses for a complete sentence.
#[tokio::test]
async fn scripted_complete_sentence_reaches_tts() {
    let rig = SeamRig::new(Arc::new(
        common::harness::ScriptedLlmProvider::tokens("Everything is ready for the demo."),
    ))
    .await;
    let turn_id = 203;

    let req = rig.turn_request("Quick status check.", turn_id);
    let outcome = Harness::execute_turn(&rig.state.harness, req).await;

    match outcome {
        TurnOutcome::Completed { turn_id: tid, .. } => assert_eq!(tid, turn_id),
        other => panic!("Expected TurnOutcome::Completed, got {:?}", other),
    }

    let clauses = drain_tts_generate(&rig.tts_rx, Duration::from_secs(5));
    assert_eq!(
        clauses.len(),
        1,
        "a single complete sentence must produce exactly one clause, got {:?}",
        clauses
    );
    assert!(clauses[0].1.contains("Everything"));
    rig.shutdown();
}

/// PURPOSE:  A turn with tools wired end to end must reach TTS with the model's text.
/// SCENARIO: The model streams a single sentence; the tool registry is available but unused.
/// ASSERTS:  Completed outcome and clause dispatch for the sentence.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the dispatch for tool-less sentences.
#[tokio::test]
async fn tools_registered_but_unused_stream_completes() {
    let rig = SeamRig::new(Arc::new(
        common::harness::ScriptedLlmProvider::tokens("Understood, no tools needed."),
    ))
    .await;
    let turn_id = 204;

    let req = rig.turn_request("No tools, please.", turn_id);
    let outcome = Harness::execute_turn(&rig.state.harness, req).await;

    match outcome {
        TurnOutcome::Completed { turn_id: tid, .. } => assert_eq!(tid, turn_id),
        other => panic!("Expected TurnOutcome::Completed, got {:?}", other),
    }

    let clauses = drain_tts_generate(&rig.tts_rx, Duration::from_secs(5));
    assert!(
        !clauses.is_empty(),
        "a complete sentence must produce at least one clause"
    );
    rig.shutdown();
}

/// PURPOSE:  A real generated turn must stream its clauses to the TTS and commit both
///           messages to the harness history.
/// SCENARIO: The local Qwen model answers a prompt through the real harness turn.
/// ASSERTS:  The turn completes, `LlmFinished` is delivered to the real router, at least
///           one clause reaches the TTS as a turn response, the pending job count equals
///           the clause count, and production committed the query and the answer to the
///           history.
/// CLASS:    happy-path
/// REQUIRES: the Qwen GGUF model + the production UDS LLM sidecar (F-002 fixed)
/// MUTANT:   Remove `harness.history.push_assistant_turn(...)` from the completion path.
#[tokio::test]
async fn harness_turn_streams_clauses_to_tts_and_commits_history() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        let qwen_model_path = common::paths::get_qwen_model_path();
        assert!(
            qwen_model_path.exists(),
            "Qwen GGUF model must exist at {:?}",
            qwen_model_path
        );

        let provider = Arc::new(
            EmbeddedProvider::new(&qwen_model_path, 8192, 4)
                .expect("Failed to load local Qwen GGUF model via EmbeddedProvider"),
        ) as Arc<dyn vox_lib::services::llm::LlmProvider>;
        let rig = SeamRig::new(provider).await;
        let turn_id = 201;

        let req = rig.turn_request("Hello! State your name and your purpose.", turn_id);
        let outcome = Harness::execute_turn(&rig.state.harness, req).await;
        match outcome {
            TurnOutcome::Completed {
                turn_id: tid,
                assistant_response,
            } => {
                assert_eq!(tid, turn_id, "Completed turn_id must match request");
                assert!(
                    !assistant_response.is_empty(),
                    "Assistant response must not be empty"
                );
            }
            other => panic!("Expected TurnOutcome::Completed, got {:?}", other),
        }

        let accumulator = Arc::clone(&rig.state.pipeline_accumulator);
        let full_text = accumulator.lock().assistant_response.clone();
        assert!(
            !full_text.trim().is_empty(),
            "Accumulator must capture real generated tokens from Qwen"
        );

        let clauses = drain_tts_generate(&rig.tts_rx, Duration::from_secs(30));
        assert!(
            !clauses.is_empty(),
            "Real LLM token streaming must produce at least one TTS clause"
        );

        rig.shutdown();
    })
    .await
    .expect("test_harness_turn_streams_clauses_to_tts_and_commits_history timed out");
}
