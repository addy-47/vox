//! ============================================================================
//! session_lifecycle_test.rs — Voice Session Lifecycle Integration Test
//! ============================================================================
//! Category     : Integration Test (Seam 11)
//! Component    : pipeline/assistant/session.rs + pipeline/mod.rs + core/state.rs + services/vad/VadOperationalMode
//! Prerequisites: Local DB paths initialized, mock audio engine / real channels
//! Execution    : cargo nextest run --test session_lifecycle_test --release --nocapture --test-threads=1
//! Metrics      : State machine transitions, monotonic conversation IDs, event emissions, CPAL gate
//! ============================================================================

mod common;

use std::{
    sync::{atomic::Ordering, mpsc},
    time::Duration,
};

use common::harness::attach_lifecycle_mock_engine;
use vox_lib::{
    core::{
        settings::{DictationInteractionMode, InteractionMode, PipelineMode},
        state::{AppState, InteractionOwner, InteractionState},
    },
    persistence::events::{MemoryWorkerEvent, PersistenceEvent},
    pipeline::{
        assistant::session::{on_end, on_pause, on_resume, on_session_start},
        dictation::transition_dictation,
        RoutingContext,
    },
    services::vad::{VadCommand, VadOperationalMode},
};

/// Helper: Sets up channels on `state.persist_tx` and `state.memory_tx` to capture lifecycle events.
fn setup_lifecycle_channels(
    state: &AppState,
) -> (
    crossbeam_channel::Receiver<PersistenceEvent>,
    crossbeam_channel::Receiver<MemoryWorkerEvent>,
) {
    let (persist_tx, persist_rx) = crossbeam_channel::bounded::<PersistenceEvent>(32);
    let (memory_tx, memory_rx) = crossbeam_channel::bounded::<MemoryWorkerEvent>(32);

    *state.persist_tx.lock() = Some(persist_tx);
    *state.memory_tx.lock() = Some(memory_tx);

    (persist_rx, memory_rx)
}

/// Helper: Seeds active Identity facts in the SQLite database to verify preloading during `on_session_start`.
async fn seed_test_identity_facts(db_path: &std::path::Path) -> anyhow::Result<()> {
    let conn = vox_lib::persistence::db::VoxDb::open(db_path).await?;
    vox_lib::persistence::schema::run_migrations(&conn).await?;

    // Clear existing memory facts to make assertion deterministic
    conn.execute("DELETE FROM memory_facts;", ()).await?;

    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
         VALUES ('id_1', 'Fact', 'Identity', 'User is an advanced systems engineer.', 'User', 'active', 1000);",
        (),
    )
    .await?;

    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
         VALUES ('id_2', 'Fact', 'Identity', 'Preferred language is Rust.', 'User', 'active', 2000);",
        (),
    )
    .await?;

    Ok(())
}

// ============================================================================
// Subtest 1: test_session_start_modular_sets_ready_and_identity
// ============================================================================
#[tokio::test]
async fn test_session_start_modular_sets_ready_and_identity() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let _paths_guard = common::paths::TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state();

        let db_path = vox_lib::utils::paths::db_path();
        seed_test_identity_facts(&db_path)
            .await
            .expect("Failed to seed identity facts");

        let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);
        let (persist_rx, memory_rx) = setup_lifecycle_channels(&state);

        // State starts Idle
        state.pipeline.set_state(InteractionState::Idle);
        assert_eq!(state.pipeline.state(), InteractionState::Idle);

        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Modular,
            interaction_mode: InteractionMode::Passive,
            owner: InteractionOwner::Assistant,
        };

        // Execute SUT: on_session_start
        let app_clone = app.clone();
        let state_clone = state.clone();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            on_session_start(
                InteractionOwner::Assistant,
                &app_clone,
                &state_clone,
                &ctx_clone,
            );
        })
        .join()
        .expect("on_session_start thread panicked");

        // 1. Assert state transitioned to Ready
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "State must transition to Ready on session start"
        );

        // 2. Assert monotonic conversation ID generated and non-zero
        let conv_id = state.conversation_id.load(Ordering::Relaxed);
        assert!(conv_id > 0, "Conversation ID must be positive timestamp");

        // 3. Assert SessionStarted dispatched to persistence
        let persist_ev = persist_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("Persistence event must be received on session start");
        match persist_ev {
            PersistenceEvent::SessionStarted { id, .. } => {
                assert_eq!(
                    id, conv_id,
                    "Persistence event ID must match conversation ID"
                );
            }
            _ => panic!("Expected PersistenceEvent::SessionStarted"),
        }

        // 4. Assert ActiveSessionChanged dispatched to memory
        let memory_ev = memory_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("Memory event must be received on session start");
        match memory_ev {
            MemoryWorkerEvent::ActiveSessionChanged { session_id } => {
                assert_eq!(
                    session_id, conv_id,
                    "Memory session ID must match conversation ID"
                );
            }
            _ => panic!("Expected MemoryWorkerEvent::ActiveSessionChanged"),
        }

        // 5. Assert identity facts seeded from DB into Working Memory system prompt
        let assembled_prompt = state.conversation_manager.lock().assemble_system_prompt();
        assert!(
            assembled_prompt.contains("advanced systems engineer"),
            "Working memory system prompt must contain seeded identity fact: {}",
            assembled_prompt
        );

        // 6. Assert VAD operational mode configured for Passive mode (ContinuousSegmentation)
        let vad_cmd = vad_cmd_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("VAD command must be sent on session start");
        match vad_cmd {
            VadCommand::SetOperationalMode(mode) => {
                assert_eq!(
                    mode,
                    VadOperationalMode::ContinuousSegmentation,
                    "Passive interaction mode must set VAD to ContinuousSegmentation"
                );
            }
            _ => panic!("Expected VadCommand::SetOperationalMode"),
        }

        // 7. Negative / Idempotency check: Calling on_session_start again while Ready is ignored
        let app_clone = app.clone();
        let state_clone = state.clone();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            on_session_start(
                InteractionOwner::Assistant,
                &app_clone,
                &state_clone,
                &ctx_clone,
            );
        })
        .join()
        .expect("on_session_start second call thread panicked");
        assert_eq!(
            state.conversation_id.load(Ordering::Relaxed),
            conv_id,
            "Second on_session_start while Ready must be a no-op guard"
        );
    })
    .await
    .expect("test_session_start_modular_sets_ready_and_identity timed out");
}

// ============================================================================
// Subtest 2: test_session_pause_resume_transitions
// ============================================================================
#[tokio::test]
async fn test_session_pause_resume_transitions() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let _paths_guard = common::paths::TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state();

        let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        // Configure dictation interaction mode to PTT in settings
        {
            let mut settings = state.settings.write().unwrap();
            settings.dictation.interaction_mode = DictationInteractionMode::Ptt;
        }

        // Start in Ready as Assistant
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);
        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Modular,
            interaction_mode: InteractionMode::Passive,
            owner: InteractionOwner::Assistant,
        };

        // Verify initial turn token state
        let token_before_pause = state.pipeline.turn_token();
        assert!(!token_before_pause.is_cancelled());

        // Execute SUT: on_pause
        on_pause(&app, &state, &ctx);

        // 1. Assert state transitioned to Paused
        assert_eq!(state.pipeline.state(), InteractionState::Paused);

        // 2. Assert cancel flag set and turn token cancelled
        assert!(state.pipeline.cancel_flag.load(Ordering::Relaxed));
        assert!(
            token_before_pause.is_cancelled(),
            "Turn token must be cancelled on pause"
        );

        // 3. Assert owner surrendered to Dictation
        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Dictation as u32,
            "Owner must yield to Dictation on pause"
        );

        // 4. Assert VAD mode set to dictation's configured mode (WindowedValidation for PTT)
        let vad_cmd = vad_cmd_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("VAD command must be sent on pause");
        match vad_cmd {
            VadCommand::SetOperationalMode(mode) => {
                assert_eq!(
                    mode,
                    VadOperationalMode::WindowedValidation,
                    "Pause must sync VAD to dictation PTT mode (WindowedValidation)"
                );
            }
            _ => panic!("Expected VadCommand::SetOperationalMode"),
        }

        // Execute SUT: on_resume
        on_resume(&app, &state, &ctx);

        // 5. Assert state restored to Ready
        assert_eq!(state.pipeline.state(), InteractionState::Ready);

        // 6. Assert owner restored to Assistant
        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Assistant as u32,
            "Owner must be restored to Assistant on resume"
        );

        // 7. Assert turn token re-armed (new token not cancelled)
        assert!(!state.pipeline.turn_token().is_cancelled());
        assert!(!state.pipeline.cancel_flag.load(Ordering::Relaxed));

        // 8. Assert VAD mode restored to assistant mode (ContinuousSegmentation for Passive)
        let vad_cmd_resume = vad_cmd_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("VAD command must be sent on resume");
        match vad_cmd_resume {
            VadCommand::SetOperationalMode(mode) => {
                assert_eq!(
                    mode,
                    VadOperationalMode::ContinuousSegmentation,
                    "Resume must restore VAD mode to ContinuousSegmentation"
                );
            }
            _ => panic!("Expected VadCommand::SetOperationalMode"),
        }
    })
    .await
    .expect("test_session_pause_resume_transitions timed out");
}

// ============================================================================
// Subtest 3: test_session_resume_from_sleeping_and_error
// ============================================================================
#[tokio::test]
async fn test_session_resume_from_sleeping_and_error() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let _paths_guard = common::paths::TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state();

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Modular,
            interaction_mode: InteractionMode::Passive,
            owner: InteractionOwner::Assistant,
        };

        // Case A: Resume from Sleeping (idle monitor background offload)
        state.pipeline.set_state(InteractionState::Sleeping);
        assert_eq!(state.pipeline.state(), InteractionState::Sleeping);

        on_resume(&app, &state, &ctx);
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "on_resume must transition Sleeping -> Ready"
        );
        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Assistant as u32
        );

        // Case B: Resume from Error
        state.pipeline.set_state(InteractionState::Error);
        assert_eq!(state.pipeline.state(), InteractionState::Error);

        on_resume(&app, &state, &ctx);
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "on_resume must transition Error -> Ready"
        );
        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Assistant as u32
        );

        // Case C: Resume from Idle (must be dropped / guarded)
        state.pipeline.set_state(InteractionState::Idle);
        on_resume(&app, &state, &ctx);
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Idle,
            "on_resume must drop when called from Idle"
        );
    })
    .await
    .expect("test_session_resume_from_sleeping_and_error timed out");
}

// ============================================================================
// Subtest 4: test_session_end_dictation_gate_keeps_engine
// ============================================================================
#[tokio::test]
async fn test_session_end_dictation_gate_keeps_engine() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let _paths_guard = common::paths::TempPathsGuard::new();

        // --------------------------------------------------------------------
        // Scenario 1: Dictation is enabled (dictation_state == Ready)
        // Ending assistant session MUST preserve CPAL engine and switch VAD to dictation mode
        // --------------------------------------------------------------------
        {
            let (app, state) = common::harness::get_test_app_and_state();
            let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
            let (_stt_tx, _pipeline_rx, _pipeline_tx) =
                attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

            // Configure dictation enabled and Ready
            {
                let mut settings = state.settings.write().unwrap();
                settings.dictation.enabled = true;
                settings.dictation.interaction_mode = DictationInteractionMode::Ptt;
            }
            transition_dictation(InteractionState::Ready, &app, &state);
            state.pipeline.set_state(InteractionState::Ready);
            state
                .owner
                .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

            let ctx = RoutingContext {
                pipeline_mode: PipelineMode::Modular,
                interaction_mode: InteractionMode::Passive,
                owner: InteractionOwner::Assistant,
            };

            // Execute SUT: on_end
            let app_clone = app.clone();
            let state_clone = state.clone();
            let ctx_clone = ctx.clone();
            let join_res = std::thread::spawn(move || {
                on_end(&app_clone, &state_clone, &ctx_clone);
            })
            .join();
            join_res.expect("on_end thread panicked");

            // Assertions for Scenario 1:
            assert_eq!(state.pipeline.state(), InteractionState::Idle);
            assert_eq!(
                state.owner.load(Ordering::Relaxed),
                InteractionOwner::Dictation as u32,
                "Owner must yield to Dictation on session end"
            );

            // CRITICAL CPAL GATE: Engine must NOT be dropped because dictation is Ready
            assert!(
                state.engine.lock().await.is_some(),
                "CPAL engine must remain active when dictation is Ready"
            );

            // VAD operational mode must be synced to dictation mode (WindowedValidation for PTT)
            let vad_cmd = vad_cmd_rx
                .recv_timeout(Duration::from_millis(500))
                .expect("VAD command must be sent on session end when dictation is active");
            match vad_cmd {
                VadCommand::SetOperationalMode(mode) => {
                    assert_eq!(
                        mode,
                        VadOperationalMode::WindowedValidation,
                        "VAD mode must be switched to dictation's PTT mode"
                    );
                }
                _ => panic!("Expected VadCommand::SetOperationalMode"),
            }
        }

        // --------------------------------------------------------------------
        // Scenario 2: Dictation is disabled (dictation_state == Idle)
        // Ending assistant session MUST tear down CPAL audio engine (stop_audio_engine_sync)
        // --------------------------------------------------------------------
        {
            let (app, state) = common::harness::get_test_app_and_state();
            let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
            let (_stt_tx, _pipeline_rx, _pipeline_tx) =
                attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

            // Configure dictation disabled and Idle
            {
                let mut settings = state.settings.write().unwrap();
                settings.dictation.enabled = false;
            }
            transition_dictation(InteractionState::Idle, &app, &state);
            state.pipeline.set_state(InteractionState::Ready);
            state
                .owner
                .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

            let ctx = RoutingContext {
                pipeline_mode: PipelineMode::Modular,
                interaction_mode: InteractionMode::Passive,
                owner: InteractionOwner::Assistant,
            };

            // Execute SUT: on_end
            let app_clone = app.clone();
            let state_clone = state.clone();
            let ctx_clone = ctx.clone();
            let join_res = std::thread::spawn(move || {
                on_end(&app_clone, &state_clone, &ctx_clone);
            })
            .join();
            join_res.expect("on_end thread panicked");

            // Assertions for Scenario 2:
            assert_eq!(state.pipeline.state(), InteractionState::Idle);

            // CRITICAL CPAL GATE: Engine must be None after stop_audio_engine_sync
            assert!(
                state.engine.lock().await.is_none(),
                "CPAL audio engine must be stopped when dictation is Idle"
            );
        }
    })
    .await
    .expect("test_session_end_dictation_gate_keeps_engine timed out");
}

// ============================================================================
// Subtest 5: test_session_end_purges_and_idles
// ============================================================================
#[tokio::test]
async fn test_session_end_purges_and_idles() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let _paths_guard = common::paths::TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state();

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);
        let (persist_rx, memory_rx) = setup_lifecycle_channels(&state);

        // Populate session cache file
        let cache_dir = vox_lib::utils::paths::cache_dir();
        std::fs::create_dir_all(&cache_dir).ok();
        let cache_file = cache_dir.join(vox_lib::services::realtime::SESSION_CACHE_FILENAME);
        std::fs::write(&cache_file, b"{\"handle\":\"test-resumption-handle\"}")
            .expect("Failed to write test cache file");
        assert!(cache_file.exists(), "Cache file must exist before on_end");

        // Set state to Ready with a known conversation ID
        let conv_id = 99887766u64;
        state.conversation_id.store(conv_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        // Put some dummy tokens in accumulator
        state
            .pipeline_accumulator
            .lock()
            .push_token("residual token");
        assert!(!state
            .pipeline_accumulator
            .lock()
            .assistant_response
            .is_empty());

        let ctx = RoutingContext {
            pipeline_mode: PipelineMode::Realtime,
            interaction_mode: InteractionMode::PTT,
            owner: InteractionOwner::Assistant,
        };

        // Execute SUT: on_end
        let app_clone = app.clone();
        let state_clone = state.clone();
        let ctx_clone = ctx.clone();
        let join_res = std::thread::spawn(move || {
            on_end(&app_clone, &state_clone, &ctx_clone);
        })
        .join();
        join_res.expect("on_end thread panicked");

        // 1. Assert state transitioned to Idle
        assert_eq!(state.pipeline.state(), InteractionState::Idle);

        // 2. Assert accumulator cleared
        assert!(
            state
                .pipeline_accumulator
                .lock()
                .assistant_response
                .is_empty(),
            "Accumulator response must be cleared on session end"
        );
        assert!(
            state.pipeline_accumulator.lock().user_transcript.is_empty(),
            "Accumulator transcript must be cleared on session end"
        );

        // 3. Assert SessionEnded persistence event dispatched
        let persist_ev = persist_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("Persistence event must be received on session end");
        match persist_ev {
            PersistenceEvent::SessionEnded { id, .. } => {
                assert_eq!(id, conv_id, "Persistence event id must match conv_id");
            }
            other => panic!("Unexpected persistence event: {:?}", other),
        }

        // 4. Assert SessionEnd memory event dispatched
        let mem_ev = memory_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("Memory worker event must be received on session end");
        match mem_ev {
            MemoryWorkerEvent::SessionEnd { session_id, .. } => {
                assert_eq!(session_id, conv_id.to_string());
            }
            other => panic!("Unexpected memory event: {:?}", other),
        }

        // 5. Assert realtime resumption cache purged from disk
        assert!(
            !cache_file.exists(),
            "Realtime session cache file must be purged after on_end in Realtime mode"
        );

        // 6. Assert turn token cancelled
        assert!(state.pipeline.turn_token().is_cancelled());
        assert!(state.pipeline.cancel_flag.load(Ordering::Relaxed));
    })
    .await
    .expect("test_session_end_purges_and_idles timed out");
}
