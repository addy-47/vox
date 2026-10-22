//! ============================================================================
//! session_lifecycle_test.rs — the assistant session starts, continues, pauses, resumes and ends with the state, ownership, VAD mode, persistence and working memory all in agreement
//! ============================================================================
//! Category  : Integration (Seam 11 — voice session lifecycle)
//! Component : src/pipeline/assistant/session.rs, src/pipeline/router.rs, src/persistence/worker.rs, src/services/harness/session.rs
//! Binary    : session_lifecycle_test
//! Prereqs   : none (isolated temporary VOX_HOME, Turso database and mock audio engine per test)
//! Run       : cargo nextest run --test session_lifecycle_test --release --test-threads=1
//! ============================================================================

mod common;

use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    time::{Duration, Instant},
};

use common::harness::attach_lifecycle_mock_engine;
use vox_lib::{
    core::{
        events::{PipelineMode, VoxEvent},
        state::{AppState, InteractionOwner, InteractionState},
    },
    pipeline::{
        dictation::{transition_dictation, DictationInteractionMode},
        router::spawn_router,
    },
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

    let seed_json = r#"{"sections":[{"id":"sec_seed","title":"Profile","blocks":[{"id":"blk_seed","text":"User is an advanced systems engineer. Preferred language is Rust."}]}]}"#;
    conn.execute(
        "UPDATE personal_memory SET content = ? WHERE project_id IS NULL;",
        (seed_json,),
    )
    .await?;

    Ok(())
}

/// Helper: Waits until the production log contains `needle`.
///
/// This is the acknowledgement used by the negative scenarios: the router's own
/// log line proves that the event was consumed, so the following absence
/// assertion is not a race against an unprocessed event.
async fn wait_for_log(needle: &str, deadline: Duration) -> String {
    let deadline_at = Instant::now() + deadline;
    let mut captured = String::new();
    loop {
        captured.push_str(&common::log_capture::take_text());
        if captured.contains(needle) {
            return captured;
        }
        if Instant::now() >= deadline_at {
            panic!(
                "log line `{needle}` never appeared within the deadline; captured log:\n{}",
                captured
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Helper: Polls the pipeline state until it equals `expected` or the deadline expires.
async fn wait_for_state(state: &AppState, expected: InteractionState, deadline: Duration) {
    let deadline_at = Instant::now() + deadline;
    while Instant::now() < deadline_at {
        if state.pipeline.state() == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        state.pipeline.state(),
        expected,
        "pipeline state must reach {expected:?} within the deadline"
    );
}

/// Helper: Stops the router thread and asserts that it exited cleanly.
async fn shutdown_router(event_tx: mpsc::Sender<VoxEvent>, handle: std::thread::JoinHandle<()>) {
    let _ = event_tx.send(VoxEvent::Shutdown);
    let joined = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || handle.join()),
    )
    .await
    .expect("Router thread join timed out")
    .expect("Router join task panicked");
    joined.expect("Router thread panicked");
}

/// PURPOSE:  Starting a session must move the pipeline to Ready and mint a
///           non-zero conversation id; without a conversation id the turn
///           history cannot be written back.
/// SCENARIO: Dispatch `SessionStart` through the production router.
/// ASSERTS:  The pipeline state is `Ready` and the conversation id is
///           positive.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the `state.pipeline.set_state(InteractionState::Ready)` in
///           `on_session_start` — the state assertion would fail.
#[tokio::test]
async fn session_start_transitions_to_ready_and_assigns_a_conversation_id() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        state.pipeline.set_state(InteractionState::Idle);
        assert_eq!(state.pipeline.state(), InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;

        let conv_id = state.conversation_id.load(Ordering::Relaxed);
        assert!(conv_id > 0, "Conversation ID must be positive timestamp");

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_start_transitions_to_ready_and_assigns_a_conversation_id timed out");
}

/// PURPOSE:  The started session must be written to the database; an
///           in-memory-only session disappears when the app restarts.
/// SCENARIO: Dispatch `SessionStart` with the real persistence worker wired
///           and then read the sessions table.
/// ASSERTS:  A row exists for the minted conversation id.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the `SessionStarted` event in `on_session_start` — the row
///           assertion would fail.
#[tokio::test]
async fn session_start_persists_the_session_row() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        wire_persistence_worker(&state);

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        state.pipeline.set_state(InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;
        let conv_id = state.conversation_id.load(Ordering::Relaxed);

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

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_start_persists_the_session_row timed out");
}

/// PURPOSE:  The session must preload the user's identity facts into the
///           working memory prompt; a session that starts blank makes the
///           assistant forget who it is talking to.
/// SCENARIO: Seed an identity fact, start a session and read the assembled
///           system prompt from the mounted harness.
/// ASSERTS:  The assembled prompt contains the seeded identity text.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the identity preload when the harness is created — the
///           prompt assertion would fail.
#[tokio::test]
async fn session_start_loads_identity_facts_into_the_working_prompt() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let db_path = vox_lib::utils::paths::db_path();
        seed_test_identity_facts(&db_path)
            .await
            .expect("Failed to seed identity facts");

        wire_persistence_worker(&state);

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        state.pipeline.set_state(InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;

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

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_start_loads_identity_facts_into_the_working_prompt timed out");
}

/// PURPOSE:  A passive session must put the VAD into continuous
///           segmentation; the wrong VAD mode means the assistant never
///           hears the user.
/// SCENARIO: Start a session and read the VAD command the router emits.
/// ASSERTS:  A `VadCommand::SetOperationalMode` arrives carrying
///           `ContinuousSegmentation`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Set the VAD to `WindowedValidation` on session start — the mode
///           assertion would fail.
#[tokio::test]
async fn session_start_switches_vad_to_continuous_segmentation() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        state.pipeline.set_state(InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        let mut got_operational_mode = false;
        while let Ok(vad_cmd) = vad_cmd_rx.recv_timeout(Duration::from_millis(500)) {
            match vad_cmd {
                VadCommand::SetOperationalMode(mode) => {
                    assert_eq!(
                        mode,
                        VadOperationalMode::ContinuousSegmentation,
                        "Passive interaction mode must set VAD to ContinuousSegmentation"
                    );
                    got_operational_mode = true;
                    break;
                }
                VadCommand::UpdateAudioMode(_) => continue,
                _ => break,
            }
        }
        assert!(
            got_operational_mode,
            "Expected VadCommand::SetOperationalMode on session start"
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_start_switches_vad_to_continuous_segmentation timed out");
}

/// PURPOSE:  A second `SessionStart` while the session is already running
///           must be ignored; honouring it would mint a new conversation id
///           and orphan the turns of the live session.
/// SCENARIO: Start a session, dispatch a duplicate `SessionStart` and wait
///           for the router's own "cannot start" acknowledgement.
/// ASSERTS:  The conversation id is unchanged and the pipeline stays `Ready`.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `current_state != InteractionState::Idle` guard in
///           `on_session_start` — the conversation-id assertion would fail.
#[tokio::test]
async fn duplicate_session_start_while_ready_is_ignored() {
    tokio::time::timeout(Duration::from_secs(10), async {
        common::log_capture::install();

        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        state.pipeline.set_state(InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;
        let conv_id = state.conversation_id.load(Ordering::Relaxed);
        let _ = common::log_capture::take_text();

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to send duplicate SessionStart");

        // The router's rejection line is the acknowledgement that the duplicate
        // was consumed and dropped.
        wait_for_log("Cannot start session", Duration::from_secs(5)).await;

        assert_eq!(
            state.conversation_id.load(Ordering::Relaxed),
            conv_id,
            "Second SessionStart while Ready must be a no-op guard"
        );
        assert_eq!(state.pipeline.state(), InteractionState::Ready);

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("duplicate_session_start_while_ready_is_ignored timed out");
}

/// PURPOSE:  Resuming a stored session must hydrate the harness with the
///           earlier turns; an empty harness makes the assistant forget the
///           conversation it is continuing.
/// SCENARIO: Seed a session with one turn and a completed compaction, then
///           start the session with that session id.
/// ASSERTS:  The mounted harness carries the seeded session id and the seeded
///           turn text in its message history.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the continuation hydration in `on_session_start` — the
///           message assertion would fail.
#[tokio::test]
async fn session_continuation_hydrates_the_harness_with_prior_turns() {
    tokio::time::timeout(Duration::from_secs(10), async {
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

        state.pipeline.set_state(InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle = spawn_router(app.clone(), event_rx)
            .expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: Some(existing_sid),
            })
            .expect("Failed to dispatch continuation SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;
        assert_eq!(
            state.conversation_id.load(Ordering::Relaxed),
            existing_sid as u64
        );

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

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_continuation_hydrates_the_harness_with_prior_turns timed out");
}

/// PURPOSE:  The turn counter of a continued session must resume past the
///           highest persisted turn id; reusing an id collides with the
///           UNIQUE constraint and drops the turn.
/// SCENARIO: Continue a seeded session whose highest turn id is 2 and read
///           the turn counter and the next allocated turn.
/// ASSERTS:  The counter reads 2 and the next allocated turn id is 3.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Initialise the turn counter from zero on continuation — the
///           counter assertion would fail.
#[tokio::test]
async fn session_continuation_advances_the_turn_counter_past_the_persisted_max() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) =
            common::harness::setup_isolated_app_state().await;

        vox_lib::persistence::schema::run_migrations(&state.db.connect().unwrap())
            .await
            .expect("Failed to run migrations");

        wire_persistence_worker(&state);

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        let existing_sid = 987654322i64;
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

        state.pipeline.set_state(InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle = spawn_router(app.clone(), event_rx)
            .expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: Some(existing_sid),
            })
            .expect("Failed to dispatch continuation SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;

        assert_eq!(
            state.pipeline.peek_turn_id(),
            2,
            "Turn counter must synchronize to max persisted turn_id (2)"
        );
        let next_bundle = state.pipeline.next_turn();
        assert_eq!(
            next_bundle.0, 3,
            "Next allocated turn must be 3, avoiding UNIQUE constraint collision"
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_continuation_advances_the_turn_counter_past_the_persisted_max timed out");
}

/// PURPOSE:  A compaction whose output is empty must not be treated as a
///           valid summary; the continuation would lose every earlier turn.
/// SCENARIO: Seed a session with two raw turns and a `completed` compaction
///           whose output is `{}`, then fetch the continuation.
/// ASSERTS:  The continuation has no summary, carries both raw turns and
///           reports the maximum turn id of 2.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Accept an empty `compaction_output` as a summary — the summary
///           assertion would fail.
#[tokio::test]
async fn empty_compaction_output_falls_back_to_the_raw_turns() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let existing_sid = 999_888_777i64;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let db_conn = state.db.connect().expect("Failed to connect to db");
        db_conn
            .execute(
                "INSERT INTO sessions (id, project_id, is_pinned, created_at, updated_at) VALUES (?, 'default', 0, ?, ?);",
                (existing_sid, now, now),
            )
            .await
            .expect("Failed to insert existing session");

        db_conn
            .execute(
                "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (?, ?, ?, ?, ?);",
                (
                    existing_sid,
                    1,
                    "First raw turn before mock compaction",
                    "Acknowledged first turn",
                    now,
                ),
            )
            .await
            .expect("Failed to insert existing turn 1");

        db_conn
            .execute(
                "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (?, ?, ?, ?, ?);",
                (
                    existing_sid,
                    2,
                    "Second raw turn before mock compaction",
                    "Acknowledged second turn",
                    now,
                ),
            )
            .await
            .expect("Failed to insert existing turn 2");

        // Compaction with status='completed' but compaction_output='{}' (the exact mock bug)
        db_conn
            .execute(
                "INSERT INTO session_compactions (session_id, trigger_kind, from_turn_id, to_turn_id, compaction_output, status, created_at) VALUES (?, 'periodic', ?, ?, ?, 'completed', ?);",
                (existing_sid, 0, 100, "{}", now),
            )
            .await
            .expect("Failed to insert empty compaction");

        let continuation =
            vox_lib::persistence::sessions::fetch_session_continuation(&db_conn, existing_sid)
                .await
                .expect("fetch_session_continuation must succeed");

        assert!(continuation.latest_summary.is_none());
        assert_eq!(continuation.turns.len(), 2, "Must fall back and load all raw turns");
        assert_eq!(continuation.max_turn_id, 2, "Max turn id must be 2");
    })
    .await
    .expect("empty_compaction_output_falls_back_to_the_raw_turns timed out");
}

/// PURPOSE:  Pausing must stop the current turn and hand the microphone to
///           dictation; a pause that leaves the turn running makes the
///           assistant talk over the user's dictation.
/// SCENARIO: Dispatch `PauseSession` through the router and read the state,
///           the cancel flag, the turn token and the owner.
/// ASSERTS:  The state is `Paused`, the cancel flag is set, the turn token is
///           cancelled and the owner is `Dictation`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip `state.pipeline.cancel_flag.store(true)` in the pause
///           handler — the cancel-flag assertion would fail.
#[tokio::test]
async fn pause_transitions_to_paused_cancels_the_turn_and_yields_ownership() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        {
            let mut settings = state.settings.write().unwrap();
            settings.dictation.interaction_mode = DictationInteractionMode::Ptt;
        }

        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);

        let token_before_pause = state.pipeline.turn_token();
        assert!(!token_before_pause.is_cancelled());

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::PauseSession)
            .expect("Failed to send PauseSession");

        wait_for_state(&state, InteractionState::Paused, Duration::from_secs(5)).await;

        assert!(state.pipeline.cancel_flag.load(Ordering::Relaxed));
        assert!(
            token_before_pause.is_cancelled(),
            "Turn token must be cancelled on pause"
        );
        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Dictation as u32,
            "Owner must yield to Dictation on pause"
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("pause_transitions_to_paused_cancels_the_turn_and_yields_ownership timed out");
}

/// PURPOSE:  Pausing must put the VAD into the mode dictation is configured
///           for; the wrong mode means the pause actually changes how the
///           microphone behaves.
/// SCENARIO: Configure dictation for PTT, dispatch `PauseSession` and read
///           the VAD command.
/// ASSERTS:  A `VadCommand::SetOperationalMode` arrives carrying
///           `WindowedValidation`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Leave the VAD mode untouched on pause — the mode assertion
///           would fail.
#[tokio::test]
async fn pause_switches_the_vad_to_the_dictation_mode() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        {
            let mut settings = state.settings.write().unwrap();
            settings.dictation.interaction_mode = DictationInteractionMode::Ptt;
        }

        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::PauseSession)
            .expect("Failed to send PauseSession");

        wait_for_state(&state, InteractionState::Paused, Duration::from_secs(5)).await;

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

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("pause_switches_the_vad_to_the_dictation_mode timed out");
}

/// PURPOSE:  Resuming must restore the ready state, the assistant ownership
///           and an armed turn token; a resume that leaves the token
///           cancelled makes every following turn a no-op.
/// SCENARIO: Pause the session, then dispatch `ResumeSession` and read the
///           state, the owner, the cancel flag and the turn token.
/// ASSERTS:  The state is `Ready`, the owner is `Assistant`, the cancel flag
///           is cleared and the turn token is not cancelled.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip `state.pipeline.rearm_turn_token()` in the resume handler —
///           the token assertion would fail.
#[tokio::test]
async fn resume_restores_ready_and_the_assistant_ownership() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        {
            let mut settings = state.settings.write().unwrap();
            settings.dictation.interaction_mode = DictationInteractionMode::Ptt;
        }

        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Ready);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::PauseSession)
            .expect("Failed to send PauseSession");
        wait_for_state(&state, InteractionState::Paused, Duration::from_secs(5)).await;

        // Consume the pause's own VAD command so the next command read from
        // the channel is the one the resume emits.
        let _pause_vad = vad_cmd_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("VAD command must be sent on pause");

        event_tx
            .send(VoxEvent::ResumeSession)
            .expect("Failed to send ResumeSession");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;

        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Assistant as u32,
            "Owner must be restored to Assistant on resume"
        );
        assert!(!state.pipeline.turn_token().is_cancelled());
        assert!(!state.pipeline.cancel_flag.load(Ordering::Relaxed));

        // The VAD must be restored to the assistant's passive mode.
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

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("resume_restores_ready_and_the_assistant_ownership timed out");
}

/// PURPOSE:  A sleeping or errored session must be resumable; a user whose
///           session is stuck in Error can never talk to the assistant again.
/// SCENARIO: Put the pipeline into `Sleeping`, resume, then put it into
///           `Error` and resume again.
/// ASSERTS:  Both resumes restore `Ready` and the `Assistant` owner.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Allow only `Paused` in the resume guard — the Sleeping case
///           would fail.
#[tokio::test]
async fn resume_from_sleeping_and_error_restores_ready() {
    tokio::time::timeout(Duration::from_secs(10), async {
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

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;
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

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;
        assert_eq!(
            state.owner.load(Ordering::Relaxed),
            InteractionOwner::Assistant as u32
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("resume_from_sleeping_and_error_restores_ready timed out");
}

/// PURPOSE:  A resume from `Idle` must be dropped; there is no session to
///           resume, and honouring the event would start a session behind the
///           user's back.
/// SCENARIO: Put the pipeline into `Idle`, dispatch `ResumeSession` and wait
///           for the router's own rejection acknowledgement.
/// ASSERTS:  The router reports it cannot resume and the pipeline is still
///           `Idle`.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Accept `Idle` in the resume guard — the state assertion would
///           fail.
#[tokio::test]
async fn resume_from_idle_is_ignored() {
    tokio::time::timeout(Duration::from_secs(10), async {
        common::log_capture::install();

        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        state.pipeline.set_state(InteractionState::Idle);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::ResumeSession)
            .expect("Failed to send ResumeSession");

        // The router's rejection line is the acknowledgement that the resume
        // was consumed and dropped.
        wait_for_log("Cannot resume session", Duration::from_secs(5)).await;

        assert_eq!(
            state.pipeline.state(),
            InteractionState::Idle,
            "Resume must drop when called from Idle"
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("resume_from_idle_is_ignored timed out");
}

/// PURPOSE:  Ending the assistant session must not tear down the audio
///           engine while dictation is ready; the user is still dictating
///           and losing the engine kills their dictation.
/// SCENARIO: Configure dictation as enabled and ready, dispatch `EndSession`
///           and inspect the engine and the VAD mode.
/// ASSERTS:  The engine is still present, the owner is `Dictation` and the
///           VAD was switched to the dictation PTT mode.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Always stop the audio engine on session end — the engine
///           assertion would fail.
#[tokio::test]
async fn session_end_keeps_the_audio_engine_while_dictation_is_ready() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let _paths_guard = common::paths::TempPathsGuard::new();

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

        event_tx
            .send(VoxEvent::EndSession)
            .expect("Failed to send EndSession");

        wait_for_state(&state, InteractionState::Idle, Duration::from_secs(5)).await;

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

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_end_keeps_the_audio_engine_while_dictation_is_ready timed out");
}

/// PURPOSE:  Ending the assistant session must stop the audio engine when
///           dictation is idle; keeping the engine alive pins the microphone
///           and the ONNX sessions for nothing.
/// SCENARIO: Configure dictation as disabled and idle, dispatch `EndSession`
///           and inspect the engine.
/// ASSERTS:  The pipeline is `Idle` and the engine has been removed from the
///           app state.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Never stop the audio engine on session end — the engine
///           assertion would fail.
#[tokio::test]
async fn session_end_stops_the_audio_engine_when_dictation_is_idle() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let _paths_guard = common::paths::TempPathsGuard::new();

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

        event_tx
            .send(VoxEvent::EndSession)
            .expect("Failed to send EndSession");

        wait_for_state(&state, InteractionState::Idle, Duration::from_secs(5)).await;

        // CRITICAL CPAL GATE: Engine must be None after stop_audio_engine_sync
        assert!(
            state.engine.lock().await.is_none(),
            "CPAL audio engine must be stopped when dictation is Idle"
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_end_stops_the_audio_engine_when_dictation_is_idle timed out");
}

/// PURPOSE:  Ending the session must unmount the harness, clear the
///           accumulator and purge the realtime cache; a leftover harness
///           keeps the previous session's context alive.
/// SCENARIO: Mount a harness with residual tokens and write a realtime cache
///           file, then dispatch `EndSession`.
/// ASSERTS:  The harness is unmounted, the accumulator is empty and the cache
///           file is gone from disk.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip unmounting the harness in the end handler — the
///           `harness.is_none()` assertion would fail.
#[tokio::test]
async fn session_end_unmounts_the_harness_clears_the_accumulator_and_purges_the_cache() {
    tokio::time::timeout(Duration::from_secs(10), async {
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

        event_tx
            .send(VoxEvent::EndSession)
            .expect("Failed to dispatch EndSession");

        wait_for_state(&state, InteractionState::Idle, Duration::from_secs(5)).await;

        // Zero Harness Instances in Memory Invariant
        assert!(
            state.harness.lock().is_none(),
            "HarnessSession must be unmounted (None) on session end"
        );

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

        assert!(
            !cache_file.exists(),
            "Realtime session cache file must be purged after on_end"
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect(
        "session_end_unmounts_the_harness_clears_the_accumulator_and_purges_the_cache timed out",
    );
}

/// PURPOSE:  Ending the session must cancel the in-flight turn; a token that
///           stays live lets the assistant answer a turn the user already
///           abandoned.
/// SCENARIO: Dispatch `EndSession` and inspect the turn token and the cancel
///           flag.
/// ASSERTS:  The turn token is cancelled and the cancel flag is set.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the token cancellation in the end handler — the token
///           assertion would fail.
#[tokio::test]
async fn session_end_cancels_the_in_flight_turn() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        wire_persistence_worker(&state);

        state.pipeline.set_state(InteractionState::Ready);
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::EndSession)
            .expect("Failed to dispatch EndSession");

        wait_for_state(&state, InteractionState::Idle, Duration::from_secs(5)).await;

        assert!(state.pipeline.turn_token().is_cancelled());
        assert!(state.pipeline.cancel_flag.load(Ordering::Relaxed));

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("session_end_cancels_the_in_flight_turn timed out");
}

/// PURPOSE:  A capability probe that cannot reach the model must leave the
///           tool support flag alone and must not warn the user; an
///           unreachable endpoint is not evidence that tools are broken.
/// SCENARIO: Point the active model at an unreachable server, start a session
///           and wait for the probe's own failure acknowledgement.
/// ASSERTS:  The harness still reports tool support and no
///           `model_tool_unsupported` notification was written.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Treat a failed probe as "unsupported" — the tool-support
///           assertion would fail.
#[tokio::test]
async fn failed_capability_probe_keeps_tools_enabled_and_emits_no_notification() {
    tokio::time::timeout(Duration::from_secs(30), async {
        common::log_capture::install();

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
        if cache_file.exists() {
            let _ = tokio::fs::remove_file(&cache_file).await;
        }

        // Configure active model to an unprobed remote model
        let test_model = "test-model-404".to_string();
        {
            let mut settings = state.settings.write().unwrap();
            settings.llm.active = vox_lib::config::ActiveSelection {
                category: "server".to_string(),
                provider: Some("ollama".to_string()),
                model: test_model.clone(),
            };
            settings.providers.llm.servers = vec![vox_lib::services::llm::LlmCustomServer {
                id: "test-server".to_string(),
                name: "Test Server".to_string(),
                url: "http://127.0.0.1:9999".to_string(),
                model: test_model.clone(),
                protocol: "ollama".to_string(),
                api_key: None,
            }];
        }

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;

        // The probe's own failure line is the acknowledgement that the probe
        // terminated; only a confirmed `unsupported` verdict can notify.
        let captured = wait_for_log("Background probe", Duration::from_secs(20)).await;
        assert!(
            captured.contains("Background probe failed")
                || captured.contains("Background probe timed out")
                || captured.contains("could not determine tool support"),
            "the probe must have failed, timed out or been unable to determine tool support; \
             captured log:\n{}",
            captured
        );

        let harness_supports_tools = state
            .harness
            .lock()
            .as_ref()
            .map(|h| h.supports_tools())
            .unwrap_or(false);
        assert!(
            harness_supports_tools,
            "Failed probe must leave harness.supports_tools untouched"
        );

        let db_conn = state.db.connect().expect("Failed to connect to db");
        let mut rows = db_conn
            .query(
                "SELECT COUNT(*) FROM notifications WHERE group_key = 'model_tool_unsupported';",
                (),
            )
            .await
            .expect("Failed to query notifications");
        let mut unsupported_notification_found = false;
        if let Ok(Some(row)) = rows.next().await {
            let count: i64 = row.get(0).unwrap_or(0);
            unsupported_notification_found = count > 0;
        }
        assert!(
            !unsupported_notification_found,
            "Unreachable probe must NOT emit model_tool_unsupported notification toast"
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("failed_capability_probe_keeps_tools_enabled_and_emits_no_notification timed out");
}

/// PURPOSE:  A cached "tools supported" verdict must enable the agentic tools
///           on the very first turn; waiting for a fresh probe would leave
///           the assistant without tools for a whole session.
/// SCENARIO: Write the capability cache with a supported verdict, start a
///           session and wait for the cache-hit acknowledgement.
/// ASSERTS:  The router reports the cached verdict and the harness reports
///           tool support enabled.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Ignore the capability cache in `resolve_model_tool_support` —
///           the tool-support assertion would fail.
#[tokio::test]
async fn cached_supported_capabilities_enable_tools_immediately() {
    tokio::time::timeout(Duration::from_secs(30), async {
        common::log_capture::install();

        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let _ = vox_lib::persistence::TOKIO_HANDLE.set(tokio::runtime::Handle::current());

        wire_persistence_worker(&state);

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        let cache_dir = vox_lib::utils::paths::get().cache.clone();
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .expect("Failed to create cache dir");
        let cache_file = cache_dir.join("model_capabilities.json");

        let supported_model = "test-agentic-model".to_string();
        let key = format!(
            "{}:{}",
            vox_lib::services::llm::catalog::CAP_KIND_SERVER,
            supported_model
        );
        let mut caps_map = std::collections::HashMap::new();
        caps_map.insert(
            key,
            vox_lib::services::llm::catalog::ModelCapabilities {
                model_id: supported_model.clone(),
                provider_kind: vox_lib::services::llm::catalog::CAP_KIND_SERVER.to_string(),
                supports_tools: vox_lib::services::llm::Support::Supported,
                supports_latin: vox_lib::services::llm::Support::Supported,
                supports_devanagari: vox_lib::services::llm::Support::Supported,
                context_window: Some(8192),
                max_output_tokens: Some(512),
                provenance: vox_lib::services::llm::catalog::CapabilityProvenance::CatalogBaseline,
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
                checks: Vec::new(),
            },
        );
        let json_content = serde_json::to_string_pretty(&caps_map).unwrap();
        tokio::fs::write(&cache_file, json_content)
            .await
            .expect("Failed to write test cache");

        {
            let mut settings = state.settings.write().unwrap();
            settings.llm.active = vox_lib::config::ActiveSelection {
                category: "server".to_string(),
                provider: Some("ollama".to_string()),
                model: supported_model.clone(),
            };
        }

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;
        wait_for_log("tools supported", Duration::from_secs(10)).await;

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

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("cached_supported_capabilities_enable_tools_immediately timed out");
}

/// PURPOSE:  A cached "tools unsupported" verdict must disable the agentic
///           tools and warn the user; silently running without tools leaves
///           the user waiting for actions that never happen.
/// SCENARIO: Write the capability cache with an unsupported verdict, start a
///           session and wait for the cache-hit acknowledgement.
/// ASSERTS:  The router reports the cached verdict, the harness reports tool
///           support disabled and a `model_tool_unsupported` notification
///           exists.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the notification in the cached-unsupported branch — the
///           notification assertion would fail.
#[tokio::test]
async fn cached_unsupported_capabilities_disable_tools_and_notify() {
    tokio::time::timeout(Duration::from_secs(30), async {
        common::log_capture::install();

        let (_paths_guard, app, state) = common::harness::setup_isolated_app_state().await;

        let _ = vox_lib::persistence::TOKIO_HANDLE.set(tokio::runtime::Handle::current());

        wire_persistence_worker(&state);

        let (vad_cmd_tx, _vad_cmd_rx) = mpsc::channel::<VadCommand>();
        let (_stt_tx, _pipeline_rx, _pipeline_tx) =
            attach_lifecycle_mock_engine(&app, &state, vad_cmd_tx);

        let cache_dir = vox_lib::utils::paths::get().cache.clone();
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .expect("Failed to create cache dir");
        let cache_file = cache_dir.join("model_capabilities.json");

        let supported_model = "test-agentic-model".to_string();
        let key = format!(
            "{}:{}",
            vox_lib::services::llm::catalog::CAP_KIND_SERVER,
            supported_model
        );
        let mut unsupported_map = std::collections::HashMap::new();
        unsupported_map.insert(
            key,
            vox_lib::services::llm::catalog::ModelCapabilities {
                model_id: supported_model.clone(),
                provider_kind: vox_lib::services::llm::catalog::CAP_KIND_SERVER.to_string(),
                supports_tools: vox_lib::services::llm::Support::Unsupported,
                supports_latin: vox_lib::services::llm::Support::Supported,
                supports_devanagari: vox_lib::services::llm::Support::Unknown,
                context_window: Some(8192),
                max_output_tokens: Some(512),
                provenance: vox_lib::services::llm::catalog::CapabilityProvenance::ProbedServer,
                tps: None,
                ttft_ms: None,
                server_has_gpu: false,
                is_gpu_accelerated: false,
                gpu_status: "Test".to_string(),
                vram_bytes: None,
                parameter_size: None,
                quantization: None,
                family: Some("qwen2.5".to_string()),
                tested_at_epoch: 1700000000,
                checks: Vec::new(),
            },
        );
        let json_content = serde_json::to_string_pretty(&unsupported_map).unwrap();
        tokio::fs::write(&cache_file, json_content)
            .await
            .expect("Failed to write test cache");

        {
            let mut settings = state.settings.write().unwrap();
            settings.llm.active = vox_lib::config::ActiveSelection {
                category: "server".to_string(),
                provider: Some("ollama".to_string()),
                model: supported_model.clone(),
            };
        }

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        let router_handle =
            spawn_router(app.clone(), event_rx).expect("Failed to spawn router thread");

        event_tx
            .send(VoxEvent::SessionStart {
                owner: InteractionOwner::Assistant,
                session_id: None,
            })
            .expect("Failed to dispatch SessionStart");

        wait_for_state(&state, InteractionState::Ready, Duration::from_secs(5)).await;
        wait_for_log("tools unsupported", Duration::from_secs(10)).await;

        let harness_supports_tools = state
            .harness
            .lock()
            .as_ref()
            .map(|h| h.supports_tools())
            .unwrap_or(true);
        assert!(
            !harness_supports_tools,
            "Confirmed-unsupported cache hit must set harness.supports_tools = false"
        );

        let db_conn = state.db.connect().expect("Failed to connect to db");
        let poll_unsupported = Instant::now() + Duration::from_secs(10);
        let mut unsupported_notification_found = false;
        while Instant::now() < poll_unsupported {
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
            "Confirmed-unsupported model must emit model_tool_unsupported notification toast"
        );

        shutdown_router(event_tx, router_handle).await;
    })
    .await
    .expect("cached_unsupported_capabilities_disable_tools_and_notify timed out");
}
