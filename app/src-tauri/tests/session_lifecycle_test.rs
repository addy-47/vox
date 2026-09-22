//! ============================================================================
//! session_lifecycle_test.rs — Voice Session Lifecycle Integration Test
//! ============================================================================
//! Category     : Integration Test (Seam 11)
//! Component    : pipeline/assistant/session.rs + pipeline/router.rs +
//!                persistence/worker.rs + services/harness/session.rs
//! Prerequisites: Local DB paths initialized, mock audio engine / real channels
//! Execution    : cargo nextest run --test session_lifecycle_test --release --nocapture --test-threads=1
//! Metrics      : State machine transitions, monotonic conversation IDs, real Turso
//!                persistence insertion, continuation hydration, CPAL engine gate
//! ============================================================================

mod common;

use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    time::{Duration, Instant},
};

use common::harness::attach_lifecycle_mock_engine;
use vox_lib::{
    core::{
        events::VoxEvent,
        settings::{DictationInteractionMode, PipelineMode},
        state::{AppState, InteractionOwner, InteractionState},
    },
    pipeline::{dictation::transition_dictation, router::spawn_router},
    services::vad::{VadCommand, VadOperationalMode},
};

/// Helper: Wires the real persistence worker to write lifecycle events directly into Turso SQLite.
fn wire_persistence_worker(state: &AppState) {
    let persist_tx = vox_lib::persistence::worker::spawn_persistence_worker(
        Arc::clone(&state.db),
        state.telemetry.is_db_healthy.clone(),
        state.telemetry.latest_persistence_rate.clone(),
        state.telemetry.is_private_mode.clone(),
    );
    *state.persist_tx.lock() = Some(persist_tx);
}

/// Helper: Seeds active Identity facts in personal_memory to verify preloading during session start.
async fn seed_test_identity_facts(db_path: &std::path::Path) -> anyhow::Result<()> {
    let db = vox_lib::persistence::VoxDb::open(db_path).await?;
    let conn = db.connect()?;
    vox_lib::persistence::schema::run_migrations(&conn).await?;

    conn.execute(
        "UPDATE personal_memory SET content = 'User is an advanced systems engineer. Preferred language is Rust.' WHERE project_id IS NULL;",
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
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let db_path = vox_lib::utils::paths::db_path();
        seed_test_identity_facts(&db_path)
            .await
            .expect("Failed to seed identity facts");

        wire_persistence_worker(&state);

        let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        // State starts Idle
        state.pipeline.set_state(InteractionState::Idle);
        assert_eq!(state.pipeline.state(), InteractionState::Idle);

        // Spawn central production router
        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        // Production Entry Seam: Send SessionStart over event_tx into spawn_router
        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        // 1. Assert state transitioned to Ready
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "State must transition to Ready on session start via router"
        );

        // 2. Assert monotonic conversation ID generated and non-zero
        let conv_id = state.conversation_id.load(Ordering::Relaxed);
        assert!(conv_id > 0, "Conversation ID must be positive timestamp");

        // 3. Assert SessionStarted was persisted to Turso SQLite database
        let db_conn = state.db.connect().expect("Failed to connect to db");
        let poll_deadline = Instant::now() + Duration::from_secs(5);
        let mut row_found = false;
        while Instant::now() < poll_deadline {
            let mut rows = db_conn
                .query(
                    "SELECT COUNT(*) FROM sessions WHERE id = ?;",
                    (conv_id as i64,),
                )
                .await
                .expect("Failed to query sessions table");
            if let Ok(Some(row)) = rows.next().await {
                let count: i64 = row.get(0).unwrap_or(0);
                if count > 0 {
                    row_found = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            row_found,
            "Turso SQLite must contain inserted session row from persistence worker"
        );

        // 4. Assert identity facts seeded from DB into Working Memory system prompt
        let assembled_prompt = state
            .harness
            .lock()
            .as_ref()
            .expect("HarnessSession must be mounted")
            .assembled_system_prompt();
        assert!(
            assembled_prompt.contains("advanced systems engineer"),
            "Working memory system prompt must contain seeded identity fact: {}",
            assembled_prompt
        );

        // 5. Assert VAD operational mode configured for Passive mode (ContinuousSegmentation)
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

        // 6. Negative / Idempotency check: Sending SessionStart again while Ready is ignored
        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to send duplicate SessionStart");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            state.conversation_id.load(Ordering::Relaxed),
            conv_id,
            "Second SessionStart while Ready must be a no-op guard"
        );

        // Teardown router
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_session_start_modular_sets_ready_and_identity timed out");
}

// ============================================================================
// Subtest 2: test_session_continuation_seeds_harness
// ============================================================================
#[tokio::test]
async fn test_session_continuation_seeds_harness() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let (_paths_guard, app, state) =
            common::harness::setup_isolated_app_state().await;

        vox_lib::persistence::schema::run_migrations(&state.db.connect().unwrap())
            .await
            .expect("Failed to run migrations");

        wire_persistence_worker(&state);

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        // Seed existing session with turns and compaction in Turso DB
        let existing_sid = 987654321i64;
        let now = 1700000000000i64;
        let db_conn = state.db.connect().expect("Failed to connect to db");
        db_conn
            .execute(
                "INSERT INTO sessions (id, created_at, updated_at, project_id) VALUES (?, ?, ?, 'default');",
                (existing_sid, now, now),
            )
            .await
            .expect("Failed to insert existing session");

        db_conn
            .execute(
                "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (?, ?, ?, ?, ?);",
                (
                    existing_sid,
                    2,
                    "Prior query on neural architecture",
                    "Acknowledged neural architecture discussion",
                    now,
                ),
            )
            .await
            .expect("Failed to insert existing turn");

        db_conn
            .execute(
                "INSERT INTO session_compactions (session_id, trigger_kind, from_turn_id, to_turn_id, compaction_output, status, created_at) VALUES (?, 'periodic', ?, ?, ?, 'completed', ?);",
                (
                    existing_sid,
                    0,
                    1,
                    r#"{"context_summary": "Prior context summary of neural architecture"}"#,
                    now,
                ),
            )
            .await
            .expect("Failed to insert existing compaction");

        // State starts Idle
        state.pipeline.set_state(InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle = spawn_router(app.clone(), event_rx)
            .expect("Failed to spawn router thread");

        // Resume existing session via SessionStart with session_id
        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: Some(existing_sid),
            })
            .expect("Failed to dispatch continuation SessionStart");

        // Wait for state transition to Ready
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(state.pipeline.state(), InteractionState::Ready);
        assert_eq!(
            state.conversation_id.load(Ordering::Relaxed),
            existing_sid as u64
        );

        // Verify HarnessSession mounted and continuation turns hydrated
        {
            let guard = state.harness.lock();
            let harness = guard.as_ref().expect("HarnessSession must be mounted");
            assert_eq!(harness.session_id(), Some(existing_sid));

            let messages = harness.messages();
            assert!(
                messages
                    .iter()
                    .any(|m| m.content.contains("Prior query on neural architecture")),
                "Seeded database turn must be hydrated into working memory history"
            );
        }

        // Teardown router
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_session_continuation_seeds_harness timed out");
}

// ============================================================================
// Subtest 3: test_session_pause_resume_transitions
// ============================================================================
#[tokio::test]
async fn test_session_pause_resume_transitions() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

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

        let token_before_pause = state.pipeline.turn_token();
        assert!(!token_before_pause.is_cancelled());

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        // Production Entry Seam: Send PauseSession to router
        event_tx
            .send(VoxEvent::PauseSession)
            .expect("Failed to send PauseSession");

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Paused {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

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

        // Production Entry Seam: Send ResumeSession to router
        event_tx
            .send(VoxEvent::ResumeSession)
            .expect("Failed to send ResumeSession");

        let resume_deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < resume_deadline {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

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

        // Teardown router
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_session_pause_resume_transitions timed out");
}

// ============================================================================
// Subtest 4: test_session_resume_from_sleeping_and_error
// ============================================================================
#[tokio::test]
async fn test_session_resume_from_sleeping_and_error() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        // Case A: Resume from Sleeping
        state.pipeline.set_state(InteractionState::Sleeping);
        assert_eq!(state.pipeline.state(), InteractionState::Sleeping);

        event_tx
            .send(VoxEvent::ResumeSession)
            .expect("Failed to send ResumeSession");

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "Resume must transition Sleeping -> Ready"
        );
        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Assistant as u32
        );

        // Case B: Resume from Error
        state.pipeline.set_state(InteractionState::Error);
        assert_eq!(state.pipeline.state(), InteractionState::Error);

        event_tx
            .send(VoxEvent::ResumeSession)
            .expect("Failed to send ResumeSession");

        let deadline2 = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline2 {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "Resume must transition Error -> Ready"
        );
        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Assistant as u32
        );

        // Case C: Resume from Idle (must be dropped / guarded)
        state.pipeline.set_state(InteractionState::Idle);
        event_tx
            .send(VoxEvent::ResumeSession)
            .expect("Failed to send ResumeSession");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Idle,
            "Resume must drop when called from Idle"
        );

        // Teardown router
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_session_resume_from_sleeping_and_error timed out");
}

// ============================================================================
// Subtest 5: test_session_end_dictation_gate_keeps_engine
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
            let (app, state) = common::harness::get_test_app_and_state().await;
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

            let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
            let router_handle =
                spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

            // Dispatch EndSession through router
            event_tx
                .send(VoxEvent::EndSession)
                .expect("Failed to send EndSession");

            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if state.pipeline.state() == InteractionState::Idle {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }

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

            // VAD operational mode must be switched to dictation's PTT mode
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

            // Teardown router
            let _ = event_tx.send(VoxEvent::Shutdown);
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::task::spawn_blocking(move || router_handle.join()),
            )
            .await
            .expect("Router thread join timed out");
        }

        // --------------------------------------------------------------------
        // Scenario 2: Dictation is disabled (dictation_state == Idle)
        // Ending assistant session MUST tear down CPAL audio engine (stop_audio_engine_sync)
        // --------------------------------------------------------------------
        {
            let (app, state) = common::harness::get_test_app_and_state().await;
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

            let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
            let router_handle =
                spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

            // Dispatch EndSession through router
            event_tx
                .send(VoxEvent::EndSession)
                .expect("Failed to send EndSession");

            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if state.pipeline.state() == InteractionState::Idle {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }

            // Assertions for Scenario 2:
            assert_eq!(state.pipeline.state(), InteractionState::Idle);

            // CRITICAL CPAL GATE: Engine must be None after stop_audio_engine_sync
            assert!(
                state.engine.lock().await.is_none(),
                "CPAL audio engine must be stopped when dictation is Idle"
            );

            // Teardown router
            let _ = event_tx.send(VoxEvent::Shutdown);
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::task::spawn_blocking(move || router_handle.join()),
            )
            .await
            .expect("Router thread join timed out");
        }
    })
    .await
    .expect("test_session_end_dictation_gate_keeps_engine timed out");
}

// ============================================================================
// Subtest 6: test_session_end_purges_and_unmounts_harness
// ============================================================================
#[tokio::test]
async fn test_session_end_purges_and_unmounts_harness() {
    let test_timeout = Duration::from_secs(10);
    tokio::time::timeout(test_timeout, async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        wire_persistence_worker(&state);

        // Populate session cache file
        let cache_dir = vox_lib::utils::paths::cache_dir();
        std::fs::create_dir_all(&cache_dir).ok();
        let cache_file = cache_dir.join(vox_lib::services::realtime::SESSION_CACHE_FILENAME);
        std::fs::write(&cache_file, b"{\"handle\":\"test-resumption-handle\"}")
            .expect("Failed to write test cache file");
        assert!(
            cache_file.exists(),
            "Cache file must exist before EndSession"
        );

        // Start in Ready with a known conversation ID and mounted HarnessSession
        let conv_id = 99887766u64;
        state.conversation_id.store(conv_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        {
            let mut settings = state.settings.write().unwrap();
            settings.interaction.pipeline_mode = PipelineMode::Realtime;
        }

        let harness = vox_lib::services::harness::Harness::new_realtime(
            Some(conv_id as i64),
            "System prompt".to_string(),
            None,
            &state.settings.read().unwrap(),
        );
        *state.harness.lock() = Some(harness);
        assert!(state.harness.lock().is_some());

        // Put residual tokens in accumulator
        state
            .pipeline_accumulator
            .lock()
            .push_token("residual token");
        assert!(!state
            .pipeline_accumulator
            .lock()
            .assistant_response
            .is_empty());

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        // Dispatch EndSession via router
        event_tx
            .send(VoxEvent::EndSession)
            .expect("Failed to send EndSession");

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Idle {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // 1. Assert state transitioned to Idle
        assert_eq!(state.pipeline.state(), InteractionState::Idle);

        // 2. Assert Zero Harness Instances in Memory Invariant
        assert!(
            state.harness.lock().is_none(),
            "HarnessSession must be unmounted (None) on session end"
        );

        // 3. Assert accumulator cleared
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

        // 4. Assert realtime resumption cache purged from disk
        assert!(
            !cache_file.exists(),
            "Realtime session cache file must be purged after on_end"
        );

        // 5. Assert turn token cancelled
        assert!(state.pipeline.turn_token().is_cancelled());
        assert!(state.pipeline.cancel_flag.load(Ordering::Relaxed));

        // Teardown router
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_session_end_purges_and_unmounts_harness timed out");
}

// ============================================================================
// Subtest 7: test_session_boot_capability_probe_and_cache_lifecycle
// ============================================================================
/// Verifies Seam 11 Phase 12 Capability Discovery & Cache Flow:
/// 1. Cold boot with unprobed model: resolve_model_tool_support initiates background probe.
/// 2. If probe times out / model is unsupported: dispatches model_tool_unsupported notification.
/// 3. Cached model capability hit: reads model_capabilities.json from cache directory
///    and initializes Harness with supports_tools immediately with zero probe delay.
#[tokio::test]
async fn test_session_boot_capability_probe_and_cache_lifecycle() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        // Initialize TOKIO_HANDLE so background probe tasks spawned from the
        // router thread execute on the test's tokio runtime (not the undriven
        // fallback current-thread runtime).
        let _ = vox_lib::persistence::TOKIO_HANDLE.set(tokio::runtime::Handle::current());

        let db_path = vox_lib::utils::paths::db_path();
        seed_test_identity_facts(&db_path)
            .await
            .expect("Failed to seed identity facts");

        wire_persistence_worker(&state);

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        let cache_dir = vox_lib::utils::paths::get().cache.clone();
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .expect("Failed to create cache dir");
        let cache_file = cache_dir.join("model_capabilities.json");

        // Ensure cache starts blank
        if cache_file.exists() {
            let _ = tokio::fs::remove_file(&cache_file).await;
        }

        // Configure active model to an unprobed remote model
        let test_model = "test-model-404".to_string();
        {
            let mut settings = state.settings.write().unwrap();
            settings.llm.active = vox_lib::core::settings::LlmActiveProvider::Server;
            settings.llm.server.model = test_model.clone();
            settings.llm.server.base_url = "http://127.0.0.1:9999".to_string(); // unresponsive dummy port
        }

        // Spawn central production router
        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        // ---------------------------------------------------------------------
        // Part A: Cold Boot with Unprobed Model -> Enters Ready, triggers background probe
        // ---------------------------------------------------------------------
        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "Pipeline must enter Ready on session start without blocking on probe"
        );

        // Wait for background probe timeout / resolution (up to 4.5s)
        let poll_probe = Instant::now() + Duration::from_secs(6);
        let mut unsupported_notification_found = false;
        let db_conn = state.db.connect().expect("Failed to connect to db");
        while Instant::now() < poll_probe {
            let mut rows = db_conn
                .query(
                    "SELECT COUNT(*) FROM notifications WHERE group_key = 'model_tool_unsupported';",
                    (),
                )
                .await
                .expect("Failed to query notifications");
            if let Ok(Some(row)) = rows.next().await {
                let count: i64 = row.get(0).unwrap_or(0);
                if count > 0 {
                    unsupported_notification_found = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            unsupported_notification_found,
            "Unresponsive probe must emit model_tool_unsupported notification toast"
        );

        // ---------------------------------------------------------------------
        // Part B: Populate Cache & Verify Warm Cache Hit on Second Session
        // ---------------------------------------------------------------------
        // End first session
        event_tx
            .send(VoxEvent::EndSession)
            .expect("Failed to dispatch EndSession");

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Idle {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(state.pipeline.state(), InteractionState::Idle);

        // Seed model_capabilities.json with supported model
        let supported_model = "test-agentic-model".to_string();
        let key = format!("server:{}", supported_model);
        let mut caps_map = std::collections::HashMap::new();
        caps_map.insert(
            key,
            vox_lib::core::settings::ModelCapabilities {
                model_id: supported_model.clone(),
                provider_kind: "server".to_string(),
                supports_tools: true,
                supports_latin: true,
                supports_devanagari: true,
                context_window: Some(8192),
                max_output_tokens: Some(512),
                provenance: Some("test_cache".to_string()),
                tps: Some(50.0),
                ttft_ms: Some(120),
                server_has_gpu: false,
                is_gpu_accelerated: false,
                gpu_status: "Test".to_string(),
                vram_bytes: None,
                parameter_size: None,
                quantization: None,
                family: Some("qwen2.5".to_string()),
                tested_at_epoch: 1700000000,
            },
        );
        let json_content = serde_json::to_string_pretty(&caps_map).unwrap();
        tokio::fs::write(&cache_file, json_content)
            .await
            .expect("Failed to write test cache");

        // Update settings to use the cached model
        {
            let mut settings = state.settings.write().unwrap();
            settings.llm.server.model = supported_model.clone();
        }

        // Re-attach mock engine — EndSession calls stop_audio_engine which
        // takes the engine out of AppState. The second SessionStart needs a
        // live engine to pass ensure_modular_workers.
        state
            .pipeline
            .engine_shutdown
            .store(false, Ordering::Relaxed);
        let (vad_cmd_tx2, _vad_cmd_rx2) = mpsc::channel::<VadCommand>();
        let (_stt_tx2, _pipeline_rx2, _pipeline_tx2) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx2);

        // Start session again
        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart for warm session");

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(state.pipeline.state(), InteractionState::Ready);

        // Assert Harness was immediately mounted with supports_tools = true
        let harness_supports_tools = state
            .harness
            .lock()
            .as_ref()
            .map(|h| h.supports_tools())
            .unwrap_or(false);

        assert!(
            harness_supports_tools,
            "Cached model capabilities must immediately set harness.supports_tools = true"
        );

        // Teardown router
        let _ = event_tx.send(VoxEvent::Shutdown);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || router_handle.join()),
        )
        .await
        .expect("Router thread join timed out");
    })
    .await
    .expect("test_session_boot_capability_probe_and_cache_lifecycle timed out");
}
