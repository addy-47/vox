//! ============================================================================
//! llm_to_tts_test.rs — Cognitive Stage: HarnessSession ◄──► LlmActor ──► TTS
//! ============================================================================
//! Category     : Integration Test (Seam 6)
//! Component    : services/harness/session.rs + services/harness/plugins/stream.rs + services/llm/actor.rs + services/llm/embedded + services/tts/actor.rs + pipeline/assistant/llm.rs
//! Prerequisites: Local Qwen 3.5 GGUF model in ~/.vox/models/llm/qwen/
//! Execution    : cargo nextest run --test llm_to_tts_test --release --nocapture --test-threads=1
//! Metrics      : Real token generation, clause chunking boundaries, atomic pending_synthesis_jobs accounting, remainder flush, HarnessSession turn commit lifecycle
//! ============================================================================

mod common;

use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    time::Duration,
};

use vox_lib::{
    core::{
        events::{AudioIntent, VoxEvent},
        settings::{AudioOutputMode, InteractionMode, LlmActiveProvider, PipelineMode},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::{assistant::llm::on_llm_finished, RoutingContext},
    services::{
        harness::{HarnessSession, StreamRoutingHandles, TurnPreparation},
        llm::{
            actor::{spawn_llm_worker, LlmCommand},
            EmbeddedProvider,
        },
        tts::TtsCommand,
    },
};

#[tokio::test]
async fn test_harness_cognitive_stage_to_tts_matrix() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();

        let (app, state) = common::harness::get_test_app_and_state().await;

        let qwen_model_path = common::paths::get_qwen_model_path();
        assert!(
            qwen_model_path.exists(),
            "Qwen GGUF model must exist at {:?}",
            qwen_model_path
        );

        // 1. Configure settings for Assistant Modular LLM with real local EmbeddedProvider
        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::Passive;
            settings.interaction.pipeline_mode = PipelineMode::Modular;
            settings.audio.output_mode = AudioOutputMode::Headset;
            settings.llm.active = LlmActiveProvider::Embedded;
            settings.llm.context_window = 2048;
            settings.llm.max_output_tokens = 60;
            settings.llm.temperature = 0.1;
            settings.memory.context_retrieval_enabled = false;
        }
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        // 2. Instantiate real local EmbeddedProvider (ctx_size=2048, n_threads=4)
        let provider = Arc::new(
            EmbeddedProvider::new(&qwen_model_path, 2048, 4)
                .expect("Failed to load local Qwen GGUF model via EmbeddedProvider"),
        );
        *state.llm_provider.write() = Some(provider.clone());

        // 3. Channels for capturing STT, VAD, LLM, TTS clauses, and pipeline events
        let (stt_tx, _) = mpsc::channel();
        let (vad_tx, _) = mpsc::channel();
        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
        let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
        let (pipeline_tx, pipeline_rx) = mpsc::channel::<VoxEvent>();

        common::harness::attach_mock_engine_with_pipeline_tx_to_state(
            &app,
            &state,
            stt_tx,
            vad_tx,
            Some(llm_tx.clone()),
            Some(tts_tx.clone()),
            pipeline_tx.clone(),
        );

        state.pipeline.set_state(InteractionState::Thinking);

        // 4. Mount production modular HarnessSession
        {
            let settings = state.settings.read().unwrap().clone();
            let prompt = "You are a concise voice assistant. Reply in exactly two short sentences.".to_string();
            *state.harness.lock() = Some(HarnessSession::new_modular(
                Some(1),
                prompt,
                None,
                &settings,
                llm_tx.clone(),
            ));
        }

        // 5. Spawn real background LLM worker thread using production EmbeddedProvider
        let worker_provider = Arc::clone(&provider);
        let worker_handle = std::thread::spawn(move || {
            spawn_llm_worker(llm_rx, worker_provider);
        });

        let cancel = state.pipeline.turn_token();
        let cancel_flag = Arc::clone(&state.pipeline.cancel_flag);
        let accumulator = Arc::clone(&state.pipeline_accumulator);
        let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);

        let turn_id = 201;
        let user_query = "Hello! State your name and your purpose.".to_string();

        // ---------------------------------------------------------------------
        // Entry Seam: HarnessSession::prepare_turn generates request via chassis
        // ---------------------------------------------------------------------
        let prep = {
            let mut guard = state.harness.lock();
            let harness = guard.as_mut().expect("HarnessSession must be mounted");
            harness.prepare_turn(&user_query, turn_id)
        };

        let request = match prep {
            TurnPreparation::Ready(req) => req,
            other => panic!("Expected TurnPreparation::Ready, got {:?}", other),
        };

        // Transmit GenerationRequest over duplex dialogue pipe to LlmActor
        let (response_tx, response_rx) = mpsc::channel();
        llm_tx
            .send(LlmCommand::Generate {
                request: Box::new(request),
                turn_id,
                cancel: cancel.clone(),
                response_tx,
            })
            .expect("Failed to send Generate to LLM worker");

        // Route token stream via HarnessSession's active StreamRoutingPlugin
        let handles = StreamRoutingHandles {
            turn_id,
            owner: InteractionOwner::Assistant,
            accumulator: Arc::clone(&accumulator),
            tts_tx: Some(tts_tx.clone()),
            pending_synthesis_jobs: Arc::clone(&pending_jobs),
            cancel: Arc::clone(&cancel_flag),
            event_tx: pipeline_tx.clone(),
            app: app.clone(),
        };

        let route_res = {
            let guard = state.harness.lock();
            let harness = guard.as_ref().unwrap();
            harness.route_stream(handles, response_rx)
        };
        assert!(
            route_res.is_ok(),
            "Stream routing must succeed: {:?}",
            route_res
        );

        // ---------------------------------------------------------------------
        // Observable Exit 1: Real tokens streamed and VoxEvent::LlmFinished emitted
        // ---------------------------------------------------------------------
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let mut finished_received = false;
        while tokio::time::Instant::now() < deadline {
            match pipeline_rx.try_recv() {
                Ok(VoxEvent::LlmFinished { turn_id: tid }) => {
                    assert_eq!(tid, turn_id, "LlmFinished turn_id must match turn under test");
                    finished_received = true;
                    break;
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        assert!(
            finished_received,
            "Real Qwen model must complete generation and emit VoxEvent::LlmFinished within 30s"
        );

        // ---------------------------------------------------------------------
        // Observable Exit 2: Assistant response accumulated
        // ---------------------------------------------------------------------
        let full_text = accumulator.lock().assistant_response.clone();
        assert!(
            !full_text.trim().is_empty(),
            "Accumulator must capture real generated tokens from Qwen"
        );

        // ---------------------------------------------------------------------
        // Observable Exit 3: Real streaming TTS clauses emitted during generation
        // ---------------------------------------------------------------------
        let mut streaming_clauses = Vec::new();
        while let Ok(cmd) = tts_rx.try_recv() {
            if let TtsCommand::Generate {
                turn_id: tid,
                text,
                intent,
            } = cmd
            {
                assert_eq!(tid, turn_id, "TTS command turn_id must match turn under test");
                assert_eq!(
                    intent,
                    AudioIntent::TurnResponse,
                    "LLM streaming clauses must have TurnResponse intent"
                );
                streaming_clauses.push(text);
            }
        }

        assert!(
            !streaming_clauses.is_empty(),
            "Real LLM token streaming must chunk and dispatch at least 1 clause BEFORE LlmFinished: {:?}",
            streaming_clauses
        );

        // ---------------------------------------------------------------------
        // Observable Exit 4: Simulate router on_llm_finished for pre-roll & persistence
        // ---------------------------------------------------------------------
        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Modular,
            interaction_mode: InteractionMode::PTT,
            owner: InteractionOwner::Assistant,
        };
        on_llm_finished(turn_id, &state, &ctx);

        // Collect any flushed remainder clauses
        let mut all_clauses = streaming_clauses;
        while let Ok(cmd) = tts_rx.try_recv() {
            if let TtsCommand::Generate {
                turn_id: tid,
                text,
                intent,
            } = cmd
            {
                assert_eq!(tid, turn_id, "TTS command turn_id must match turn under test");
                assert_eq!(
                    intent,
                    AudioIntent::TurnResponse,
                    "Flushed tail remainder must have TurnResponse intent"
                );
                all_clauses.push(text);
            }
        }

        // ---------------------------------------------------------------------
        // Observable Exit 5: pending_synthesis_jobs accounting matches clauses
        // ---------------------------------------------------------------------
        let final_pending = pending_jobs.load(Ordering::Relaxed);
        assert_eq!(
            final_pending,
            all_clauses.len() as u32,
            "pending_synthesis_jobs ({}) must exactly equal dispatched clause count ({})",
            final_pending,
            all_clauses.len()
        );

        for (idx, clause) in all_clauses.iter().enumerate() {
            assert!(
                !clause.trim().is_empty(),
                "Dispatched clause {} must not be empty",
                idx
            );
        }

        // ---------------------------------------------------------------------
        // Observable Exit 6: HarnessSession commits turn to history & watcher
        // ---------------------------------------------------------------------
        {
            let mut guard = state.harness.lock();
            let harness = guard.as_mut().expect("HarnessSession must be mounted");
            harness.history_mut().push_assistant_turn(full_text.clone());
            harness.on_turn_completed(Arc::clone(&state), Arc::clone(&state.harness));

            let msgs = harness.history().messages();
            assert!(
                msgs.iter().any(|m| m.content == user_query),
                "History must contain the user query"
            );
            assert!(
                msgs.iter().any(|m| m.content == full_text),
                "History must contain the committed assistant response"
            );
        }

        // =====================================================================
        // Teardown
        // =====================================================================
        let _ = llm_tx.send(LlmCommand::Shutdown);
        let join_res = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                worker_handle.join().expect("LLM worker thread panicked");
            }),
        )
        .await;
        assert!(join_res.is_ok(), "LLM worker thread join timed out");
    })
    .await
    .expect("test_harness_cognitive_stage_to_tts_matrix timed out");
}
