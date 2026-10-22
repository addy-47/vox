//! ============================================================================
//! database_persistence_boundary_test.rs — the Turso schema migrates to the current version, enforces its foreign-key and unique constraints, and stores vectors without loss
//! ============================================================================
//! Category  : Integration (Seam 20 — persistence boundary)
//! Component : src/persistence/{db,schema,worker,sessions,projects,facts,compactions,notifications}.rs
//! Binary    : database_persistence_boundary_test
//! Prereqs   : none (temporary Turso database per test)
//! Run       : cargo nextest run --test database_persistence_boundary_test --release --test-threads=1
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use common::paths::TempPathsGuard;
use tempfile::tempdir;
use vox_lib::persistence::{
    compactions::record_compaction_start,
    decode_f32_blob, encode_f32_blob,
    facts::{insert_observation, insert_vector, ObservationRecord},
    notifications::{create_notification, NewNotification, Severity},
    personal_memory::get_personal_memory,
    projects::{delete_project, get_project_by_id},
    schema::{run_migrations, SCHEMA_VERSION},
    sessions::{create_session_with_id, delete_session},
    worker::spawn_persistence_worker,
    PersistenceEvent, VoxDb,
};

/// PURPOSE:  A freshly opened database must be migrated to the current schema
///           version with foreign keys enabled; a stale `user_version` makes
///           the app write into columns that do not exist yet.
/// SCENARIO: Open a temporary database, run the migrations, and read the
///           connection pragmas.
/// ASSERTS:  `PRAGMA user_version` equals `SCHEMA_VERSION` and
///           `PRAGMA foreign_keys` is 1.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make `run_migrations` a no-op — the `user_version` assertion
///           would fail.
#[tokio::test]
async fn migrations_set_user_version_and_enable_foreign_keys() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_migration.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");

        run_migrations(&conn)
            .await
            .expect("Failed to execute schema migrations");

        let mut rows = conn
            .query("PRAGMA user_version;", ())
            .await
            .expect("Failed to query user_version");
        let version: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            version, SCHEMA_VERSION as i64,
            "Database user_version must match SCHEMA_VERSION after migration",
        );

        let mut fk_rows = conn
            .query("PRAGMA foreign_keys;", ())
            .await
            .expect("Failed to query foreign_keys");
        let fk_enabled: i64 = fk_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(fk_enabled, 1, "PRAGMA foreign_keys must be enabled (1)");
    })
    .await
    .expect("migrations_set_user_version_and_enable_foreign_keys timed out");
}

/// PURPOSE:  The migrations must seed the default project and the global
///           personal memory document; without them the first session has no
///           project to belong to and personal memory reads fail.
/// SCENARIO: Run the migrations on an empty database and read the seed rows.
/// ASSERTS:  Project `default` exists and the global personal memory is at
///           version 1 with no project id.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the seed inserts in `run_migrations` — the project
///           assertion would fail.
#[tokio::test]
async fn migrations_seed_default_project_and_global_personal_memory() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_seed.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        let default_project = get_project_by_id(&conn, "default")
            .await
            .expect("Failed to query default project");
        assert!(default_project.is_some(), "Default project must be seeded");
        assert_eq!(default_project.unwrap().id, "default");

        let pm = get_personal_memory(&conn, None)
            .await
            .expect("Failed to query personal memory");
        assert_eq!(pm.version, 1, "Seed personal memory must be version 1");
        assert!(pm.project_id.is_none());
    })
    .await
    .expect("migrations_seed_default_project_and_global_personal_memory timed out");
}

/// PURPOSE:  Every table the persistence layer writes to must exist after the
///           migration; a missing table turns a write into a runtime error
///           deep inside a worker.
/// SCENARIO: Run the migrations and query `sqlite_master` for each expected
///           table.
/// ASSERTS:  All twelve core tables exist exactly once.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Drop the `session_tool_calls` table from the migration — the
///           table-existence assertion would fail.
#[tokio::test]
async fn migrations_create_every_core_table() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_tables.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        let expected_tables = [
            "projects",
            "sessions",
            "turns",
            "notifications",
            "session_compactions",
            "memory_facts",
            "memory_facts_vectors",
            "memory_ingestion_queue",
            "personal_memory",
            "voices",
            "session_tool_calls",
            "personal_memory_revisions",
        ];

        for table in &expected_tables {
            let mut tbl_rows = conn
                .query(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?;",
                    (table.to_string(),),
                )
                .await
                .expect("Failed to check table existence");
            let count: i64 = tbl_rows.next().await.unwrap().unwrap().get(0).unwrap();
            assert_eq!(count, 1, "Expected table '{}' to exist in database", table);
        }
    })
    .await
    .expect("migrations_create_every_core_table timed out");
}

/// PURPOSE:  Deleting a project that still owns sessions must be refused;
///           cascading the delete would destroy the user's conversation
///           history without warning.
/// SCENARIO: Create a session under project `default`, then delete the
///           project.
/// ASSERTS:  `delete_project` returns an error.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Change the `sessions.project_id` foreign key to `ON DELETE
///           CASCADE` — the error assertion would fail.
#[tokio::test]
async fn project_delete_is_restricted_while_sessions_exist() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_restrict.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        create_session_with_id(&conn, 20201, Some("default"))
            .await
            .expect("Failed to create session");

        let delete_proj_res = delete_project(&conn, "default").await;
        assert!(
            delete_proj_res.is_err(),
            "delete_project must fail due to ON DELETE RESTRICT when child sessions exist"
        );
    })
    .await
    .expect("project_delete_is_restricted_while_sessions_exist timed out");
}

/// PURPOSE:  Deleting a session must delete its dependent rows; leaving
///           orphaned turns, notifications, compactions or tool calls behind
///           corrupts the history view.
/// SCENARIO: Populate a session with a turn, a notification, a compaction and
///           a tool call, then hard-delete the session.
/// ASSERTS:  All four child tables report zero rows for that session.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Change the `turns.session_id` foreign key to `ON DELETE SET
///           NULL` — the turn-count assertion would fail.
#[tokio::test]
async fn session_delete_cascades_to_its_child_rows() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_cascades.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        let session_id = 20201i64;
        create_session_with_id(&conn, session_id, Some("default"))
            .await
            .expect("Failed to create session");

        conn.execute(
            "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES \
             (?, 1, 'Question', 'Answer', 1000);",
            (session_id,),
        )
        .await
        .expect("Failed to insert turn");

        let notif = NewNotification {
            id: "notif_cascade_test".to_string(),
            group_key: format!("session:{}", session_id),
            category: "pipeline".to_string(),
            severity: Severity::Info,
            action_type: "receipt".to_string(),
            action_payload: "{}".to_string(),
            title: "Turn Finished".to_string(),
            message: "Turn completed".to_string(),
            status: "unread".to_string(),
            session_id: Some(session_id),
            metadata: "{}".to_string(),
        };
        create_notification(&conn, &notif)
            .await
            .expect("Failed to insert notification");

        let session_compaction_id = record_compaction_start(&conn, session_id, "critical", 1, 1)
            .await
            .expect("Failed to record session-specific compaction");
        assert!(session_compaction_id > 0);

        conn.execute(
            "INSERT INTO session_tool_calls (id, session_id, turn_id, tool_name, tool_kind, arguments, result, is_error, duration_ms, created_at) \
             VALUES ('call_cascade_test', ?, 1, 'respond_and_set_title', 'terminal', '{}', 'ok', 0, 5, 1000);",
            (session_id,),
        )
        .await
        .expect("Failed to insert tool call");

        delete_session(&conn, session_id, true)
            .await
            .expect("Failed to delete session");

        for (table, label) in [
            ("turns", "Turns must be deleted on session delete (CASCADE)"),
            (
                "notifications",
                "Notifications must be deleted on session delete (CASCADE)",
            ),
            (
                "session_compactions",
                "Compactions must be deleted on session delete (CASCADE)",
            ),
            (
                "session_tool_calls",
                "Tool calls must be deleted on session delete (CASCADE)",
            ),
        ] {
            let mut rows = conn
                .query(
                    &format!("SELECT COUNT(*) FROM {} WHERE session_id = ?;", table),
                    (session_id,),
                )
                .await
                .unwrap();
            let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
            assert_eq!(count, 0, "{}", label);
        }
    })
    .await
    .expect("session_delete_cascades_to_its_child_rows timed out");
}

/// PURPOSE:  A deleted session must not delete the facts it produced; the
///           facts are long-lived memory and their provenance is decoupled,
///           not destroyed.
/// SCENARIO: Insert a fact tied to a session, delete the session and read
///           the fact's `session_id` and vector count.
/// ASSERTS:  The fact survives with a NULL `session_id` and its vector row
///           is still present.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Change the `memory_facts.session_id` foreign key to `ON DELETE
///           CASCADE` — the survival assertion would fail.
#[tokio::test]
async fn session_delete_preserves_fact_and_its_vector() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_setnull.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        let source_session_id = 20200i64;
        create_session_with_id(&conn, source_session_id, Some("default"))
            .await
            .expect("Failed to create source session");
        let compaction_id = record_compaction_start(&conn, source_session_id, "manual", 1, 1)
            .await
            .expect("Failed to record compaction");

        let session_id = 20201i64;
        create_session_with_id(&conn, session_id, Some("default"))
            .await
            .expect("Failed to create session");

        let observation_id = "fact_cascade_20201".to_string();
        let observation_rec = ObservationRecord {
            id: observation_id.clone(),
            session_id: Some(session_id),
            compaction_id,
            observation_type: "objective".to_string(),
            text: "User is building integration tests".to_string(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_observation(&conn, &observation_rec)
            .await
            .expect("Failed to insert fact");

        insert_vector(
            &conn,
            &observation_id,
            "objective",
            "active",
            Some("default"),
            &vec![0.5f32; 384],
        )
        .await
        .expect("Failed to insert vector");

        delete_session(&conn, session_id, true)
            .await
            .expect("Failed to delete session");

        let mut fact_rows = conn
            .query(
                "SELECT session_id FROM memory_facts WHERE id = ?;",
                (observation_id.clone(),),
            )
            .await
            .unwrap();
        let row = fact_rows
            .next()
            .await
            .unwrap()
            .expect("Fact must survive session deletion");
        let fact_session: Option<i64> = row.get(0).unwrap();
        assert!(
            fact_session.is_none(),
            "Fact session_id must be SET NULL upon session deletion to preserve history"
        );

        let mut vec_rows = conn
            .query(
                "SELECT COUNT(*) FROM memory_facts_vectors WHERE fact_id = ?;",
                (observation_id,),
            )
            .await
            .unwrap();
        let vec_count: i64 = vec_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(vec_count, 1, "Vector must still exist while fact exists");
    })
    .await
    .expect("session_delete_preserves_fact_and_its_vector timed out");
}

/// PURPOSE:  Deleting a fact must delete its embedding; an orphan vector
///           pollutes the similarity search with a fact that no longer
///           exists.
/// SCENARIO: Insert a fact and its vector, delete the fact and count the
///           remaining vector rows.
/// ASSERTS:  Zero vector rows remain for that fact.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Change the `memory_facts_vectors.fact_id` foreign key to `ON
///           DELETE SET NULL` — the zero-count assertion would fail.
#[tokio::test]
async fn fact_delete_cascades_to_its_vector() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_fact_cascade.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        let source_session_id = 20200i64;
        create_session_with_id(&conn, source_session_id, Some("default"))
            .await
            .expect("Failed to create source session");
        let compaction_id = record_compaction_start(&conn, source_session_id, "manual", 1, 1)
            .await
            .expect("Failed to record compaction");

        let observation_id = "fact_cascade_20201".to_string();
        insert_observation(
            &conn,
            &ObservationRecord {
                id: observation_id.clone(),
                session_id: Some(source_session_id),
                compaction_id,
                observation_type: "objective".to_string(),
                text: "User is building integration tests".to_string(),
                status: "active".to_string(),
                created_at: 1000,
                updated_at: 1000,
            },
        )
        .await
        .expect("Failed to insert fact");

        insert_vector(
            &conn,
            &observation_id,
            "objective",
            "active",
            Some("default"),
            &vec![0.5f32; 384],
        )
        .await
        .expect("Failed to insert vector");

        conn.execute("DELETE FROM memory_facts WHERE id = ?;", (observation_id.clone(),))
            .await
            .expect("Failed to delete fact");

        let mut vec_after = conn
            .query(
                "SELECT COUNT(*) FROM memory_facts_vectors WHERE fact_id = ?;",
                (observation_id,),
            )
            .await
            .unwrap();
        let vec_count_after: i64 = vec_after.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            vec_count_after, 0,
            "Vector must be deleted on fact delete (CASCADE)"
        );
    })
    .await
    .expect("fact_delete_cascades_to_its_vector timed out");
}

/// PURPOSE:  Only one compaction may be in progress per session; a second
///           one would compact the same turns twice and double-write the
///           facts.
/// SCENARIO: Record an in-progress compaction and then try to record a
///           second one for the same session.
/// ASSERTS:  The second call fails with a unique-constraint error naming the
///           partial index.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Drop `idx_compactions_one_in_progress` from the migration — the
///           error assertion would fail.
#[tokio::test]
async fn second_in_progress_compaction_is_rejected_by_partial_index() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_partial_index.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        let session_id = 20301i64;
        create_session_with_id(&conn, session_id, Some("default"))
            .await
            .expect("Failed to create session");

        let run_1 = record_compaction_start(&conn, session_id, "manual", 1, 10)
            .await
            .expect("First in_progress compaction must succeed");
        assert!(run_1 > 0);

        let run_2_res = record_compaction_start(&conn, session_id, "soft", 11, 20).await;
        assert!(
            run_2_res.is_err(),
            "Second in_progress compaction on same session must be rejected by partial index"
        );
        let err_str = run_2_res.unwrap_err().to_string();
        assert!(
            err_str.contains("UNIQUE constraint failed")
                || err_str.contains("conflict")
                || err_str.contains("idx_compactions_one_in_progress"),
            "Error must indicate unique constraint failure, got: {}",
            err_str
        );
    })
    .await
    .expect("second_in_progress_compaction_is_rejected_by_partial_index timed out");
}

/// PURPOSE:  Once a compaction completes, the next slice must be allowed;
///           a permanent block would stop the session from ever compacting
///           again.
/// SCENARIO: Complete the first run, record a second one, complete it, then
///           count the completed runs.
/// ASSERTS:  The second run is created with a higher id and two completed
///           runs exist for the session.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Change the partial index predicate to `status IN ('in_progress',
///           'completed')` — the second-record assertion would fail.
#[tokio::test]
async fn new_in_progress_compaction_is_allowed_after_previous_completes() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_partial_index2.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        let session_id = 20301i64;
        create_session_with_id(&conn, session_id, Some("default"))
            .await
            .expect("Failed to create session");

        let run_1 = record_compaction_start(&conn, session_id, "manual", 1, 10)
            .await
            .expect("First in_progress compaction must succeed");

        conn.execute(
            "UPDATE session_compactions SET status = 'completed', finished_at = 2000 WHERE id = ?;",
            (run_1,),
        )
        .await
        .expect("Failed to complete compaction");

        let run_3 = record_compaction_start(&conn, session_id, "soft", 11, 20)
            .await
            .expect("New in_progress compaction must succeed after previous one completed");
        assert!(run_3 > run_1);

        conn.execute(
            "UPDATE session_compactions SET status = 'completed', finished_at = 3000 WHERE id = ?;",
            (run_3,),
        )
        .await
        .expect("Failed to complete compaction 3");

        let mut count_rows = conn
            .query(
                "SELECT COUNT(*) FROM session_compactions WHERE session_id = ? AND status = 'completed';",
                (session_id,),
            )
            .await
            .unwrap();
        let completed_count: i64 = count_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            completed_count, 2,
            "Multiple completed compactions must exist without index conflicts"
        );
    })
    .await
    .expect("new_in_progress_compaction_is_allowed_after_previous_completes timed out");
}

/// PURPOSE:  The background persistence writer must never block or fail the
///           readers; a "database is locked" error during a write would abort
///           the user's session.
/// SCENARIO: Run four reader tasks against the database while the
///           persistence worker writes 25 turns.
/// ASSERTS:  All four readers finish, at least 120 queries succeeded and the
///           worker still reports a healthy database.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make the persistence worker hold an exclusive lock across the
///           whole batch — the readers would fail with a busy error.
#[tokio::test]
async fn concurrent_readers_and_persistence_worker_complete_without_db_errors() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_mvcc.db");

        let db = Arc::new(VoxDb::open(&db_path).await.expect("Failed to open VoxDb"));
        let setup_conn = db.connect().expect("Failed to vend connection");
        run_migrations(&setup_conn)
            .await
            .expect("Failed migrations");

        let session_id = 20401i64;
        create_session_with_id(&setup_conn, session_id, Some("default"))
            .await
            .expect("Failed to create session");

        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let private_mode = Arc::new(AtomicBool::new(false));

        let persistence_tx = spawn_persistence_worker(
            db.clone(),
            is_healthy.clone(),
            rate.clone(),
            private_mode.clone(),
        );

        let finished_readers = Arc::new(AtomicU32::new(0));
        let total_reads = Arc::new(AtomicU32::new(0));

        // Spawn 4 concurrent reader tasks
        let mut reader_handles = Vec::new();
        for _r_id in 0..4 {
            let reader_db = db.clone();
            let finished = finished_readers.clone();
            let reads = total_reads.clone();

            let handle = tokio::spawn(async move {
                let conn = reader_db.connect().expect("Reader connection failed");
                for _ in 0..30 {
                    // Read 1: count turns
                    let mut turn_rows = conn
                        .query(
                            "SELECT COUNT(*) FROM turns WHERE session_id = ?;",
                            (session_id,),
                        )
                        .await
                        .expect("Reader turn query must never fail under MVCC");
                    let _: i64 = turn_rows.next().await.unwrap().unwrap().get(0).unwrap();

                    // Read 2: query personal memory
                    let pm = get_personal_memory(&conn, None)
                        .await
                        .expect("Reader personal memory query must never fail");
                    assert!(pm.version >= 1);

                    reads.fetch_add(2, Ordering::Relaxed);
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                finished.fetch_add(1, Ordering::SeqCst);
            });
            reader_handles.push(handle);
        }

        // Writer: send 25 turns to the background worker
        for turn in 1..=25 {
            persistence_tx
                .send(PersistenceEvent::TurnCompleted {
                    session_id,
                    turn_id: turn,
                    user_text: format!("Concurrent user text turn {}", turn),
                    assistant_text: format!("Concurrent assistant reply turn {}", turn),
                })
                .expect("Failed to enqueue turn write event");
            tokio::time::sleep(Duration::from_millis(8)).await;
        }

        // Wait for all readers to complete
        for h in reader_handles {
            h.await.unwrap();
        }

        // Shutdown persistence worker cleanly
        let _ = persistence_tx.send(PersistenceEvent::Shutdown);

        assert_eq!(
            finished_readers.load(Ordering::SeqCst),
            4,
            "All 4 reader tasks must complete without errors"
        );
        assert!(
            total_reads.load(Ordering::Relaxed) >= 120,
            "Readers must execute concurrent queries successfully"
        );
        assert!(
            is_healthy.load(Ordering::Relaxed),
            "Persistence worker must report healthy status (zero DB errors)"
        );
    })
    .await
    .expect("concurrent_readers_and_persistence_worker_complete_without_db_errors timed out");
}

/// PURPOSE:  Every turn handed to the persistence worker must reach the
///           database; a dropped event silently loses the user's transcript.
/// SCENARIO: Enqueue 25 `TurnCompleted` events and then poll the turn count
///           until the drain completes.
/// ASSERTS:  Exactly 25 turns are persisted for the session.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the last event of the channel in the worker loop — the
///           final count assertion would fail.
#[tokio::test]
async fn persistence_worker_writes_every_queued_turn() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_worker_drain.db");

        let db = Arc::new(VoxDb::open(&db_path).await.expect("Failed to open VoxDb"));
        let setup_conn = db.connect().expect("Failed to vend connection");
        run_migrations(&setup_conn)
            .await
            .expect("Failed migrations");

        let session_id = 20402i64;
        create_session_with_id(&setup_conn, session_id, Some("default"))
            .await
            .expect("Failed to create session");

        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let private_mode = Arc::new(AtomicBool::new(false));

        let persistence_tx = spawn_persistence_worker(
            db.clone(),
            is_healthy.clone(),
            rate.clone(),
            private_mode.clone(),
        );

        for turn in 1..=25 {
            persistence_tx
                .send(PersistenceEvent::TurnCompleted {
                    session_id,
                    turn_id: turn,
                    user_text: format!("user turn {}", turn),
                    assistant_text: format!("assistant turn {}", turn),
                })
                .expect("Failed to enqueue turn write event");
        }
        let _ = persistence_tx.send(PersistenceEvent::Shutdown);

        // This previously slept a fixed 100ms after Shutdown and then counted rows.
        // That is a sleep-as-proxy-for-completion (testing-style-guide.md §5.4) and it
        // was systematically too short: the worker had only drained 19-21 of 25 queued
        // events in that window, so the test failed deterministically on unmodified
        // code as well as after any change. Replaced with a deadline poll: if the worker
        // is correct this converges in milliseconds, and if it genuinely drops events the
        // deadline expires and the assertion below still fails with the true count.
        let verify_conn = db.connect().unwrap();
        let drain_deadline = std::time::Instant::now() + Duration::from_secs(10);
        let final_count: i64 = loop {
            let mut rows = verify_conn
                .query(
                    "SELECT COUNT(*) FROM turns WHERE session_id = ?;",
                    (session_id,),
                )
                .await
                .unwrap();
            let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
            if count >= 25 || std::time::Instant::now() >= drain_deadline {
                break count;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert_eq!(
            final_count, 25,
            "All 25 turns must be persisted by the worker (waited up to 10s for the drain)"
        );
    })
    .await
    .expect("persistence_worker_writes_every_queued_turn timed out");
}

/// PURPOSE:  A 384-dimensional embedding must be stored and read back
///           bit-for-bit; a lossy round-trip changes the cosine similarity
///           and silently breaks semantic deduplication.
/// SCENARIO: Encode 384 floats, store them through the production helper,
///           read the raw BLOB back and decode it.
/// ASSERTS:  The BLOB is 1536 bytes, the decoded vector has 384 dimensions
///           and every float matches the original bit pattern.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Store the vector as f16 instead of f32 — the byte-length
///           assertion would fail.
#[tokio::test]
async fn f32_blob_roundtrip_preserves_every_float_bit() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_vectors.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        // 1. Generate 384 distinctive float values
        let original_vec: Vec<f32> = (0..384).map(|i| (i as f32 * 0.01234567).sin()).collect();
        assert_eq!(original_vec.len(), 384);

        // 2. Verify encoding size
        let encoded_bytes = encode_f32_blob(&original_vec);
        assert_eq!(
            encoded_bytes.len(),
            384 * 4,
            "Encoded 384-dim vector must be exactly 1536 bytes"
        );

        // 3. Insert parent fact record with valid compaction provenance
        let session_id = 20501i64;
        create_session_with_id(&conn, session_id, Some("default"))
            .await
            .expect("Failed to create session");
        let compaction_id = record_compaction_start(&conn, session_id, "manual", 1, 1)
            .await
            .expect("Failed to record compaction");

        let observation_id = "fact_vector_precision_test";
        insert_observation(
            &conn,
            &ObservationRecord {
                id: observation_id.to_string(),
                session_id: Some(session_id),
                compaction_id,
                observation_type: "objective".to_string(),
                text: "Vector precision test fact".to_string(),
                status: "active".to_string(),
                created_at: 1000,
                updated_at: 1000,
            },
        )
        .await
        .expect("Failed to insert parent fact");

        // 4. Insert vector via production helper
        insert_vector(
            &conn,
            observation_id,
            "objective",
            "active",
            None,
            &original_vec,
        )
        .await
        .expect("Failed to insert vector");

        // 5. Select raw BLOB from SQLite
        let mut rows = conn
            .query(
                "SELECT embedding FROM memory_facts_vectors WHERE fact_id = ?;",
                (observation_id,),
            )
            .await
            .expect("Query vector failed");

        let row = rows.next().await.unwrap().expect("Vector row must exist");
        let blob_data: Vec<u8> = row.get(0).expect("Failed to extract BLOB data");
        assert_eq!(blob_data.len(), 1536, "Raw BLOB must be exactly 1536 bytes");

        // 6. Decode and assert bit-for-bit float equality
        let decoded_vec = decode_f32_blob(&blob_data);
        assert_eq!(decoded_vec.len(), 384);

        for (idx, (&orig, &decoded)) in original_vec.iter().zip(decoded_vec.iter()).enumerate() {
            assert_eq!(
                orig.to_bits(),
                decoded.to_bits(),
                "Float bit-fidelity mismatch at dimension {}: original {}, decoded {}",
                idx,
                orig,
                decoded
            );
        }
    })
    .await
    .expect("f32_blob_roundtrip_preserves_every_float_bit timed out");
}

/// PURPOSE:  The tool-call table must carry the indexes the history view
///           queries; without them the drawer loads every tool call in the
///           database.
/// SCENARIO: Run the migrations and list the indexes on `session_tool_calls`.
/// ASSERTS:  `idx_tool_calls_session_turn` and `idx_tool_calls_created` both
///           exist.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Drop `idx_tool_calls_created` from the migration — the index
///           assertion would fail.
#[tokio::test]
async fn tool_call_indexes_exist_on_session_tool_calls() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_tool_call_indexes.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed to run migrations");

        let mut idx_rows = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'session_tool_calls';",
                (),
            )
            .await
            .expect("Failed to query indexes");

        let mut indexes = Vec::new();
        while let Ok(Some(row)) = idx_rows.next().await {
            let name: String = row.get(0).unwrap();
            indexes.push(name);
        }
        assert!(
            indexes.contains(&"idx_tool_calls_session_turn".to_string()),
            "Index idx_tool_calls_session_turn must exist"
        );
        assert!(
            indexes.contains(&"idx_tool_calls_created".to_string()),
            "Index idx_tool_calls_created must exist"
        );
    })
    .await
    .expect("tool_call_indexes_exist_on_session_tool_calls timed out");
}

/// PURPOSE:  A tool call for a session the database does not know yet must
///           still be stored; losing the record means the agent's actions
///           vanish from the history.
/// SCENARIO: Send a `ToolCallExecuted` event for an unregistered session id
///           and read the row back.
/// ASSERTS:  The row exists with the tool name, kind and duration that were
///           sent.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Remove the self-healing parent insert from the persistence
///           worker — the row would never appear.
#[tokio::test]
async fn tool_call_event_self_heals_a_missing_session() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_tool_calls.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed to run migrations");

        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let is_private = Arc::new(AtomicBool::new(false));

        let persist_tx = spawn_persistence_worker(
            Arc::new(db),
            is_healthy,
            rate,
            Arc::clone(&is_private),
        );

        let orphan_sid = 999111222i64;
        let call_id = "call_test_123".to_string();
        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: call_id.clone(),
                session_id: orphan_sid,
                turn_id: 1,
                tool_name: "respond_and_set_title".to_string(),
                tool_flow: vox_lib::services::llm::ToolFlow::Terminal,
                arguments: serde_json::json!({
                    "title": "Self Healing Session",
                    "spoken_response": "Hello world"
                }),
                result: "Session title updated".to_string(),
                is_error: false,
                duration_ms: 45,
                created_at: 1700000000100,
            })
            .expect("Failed to send ToolCallExecuted");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut tool_call_found = false;
        while std::time::Instant::now() < deadline {
            let mut rows = conn
                .query(
                    "SELECT tool_name, tool_kind, duration_ms FROM session_tool_calls WHERE id = ?;",
                    (call_id.clone(),),
                )
                .await
                .expect("Failed to query session_tool_calls");

            if let Ok(Some(row)) = rows.next().await {
                let name: String = row.get(0).unwrap();
                let kind: String = row.get(1).unwrap();
                let dur: i64 = row.get(2).unwrap();
                assert_eq!(name, "respond_and_set_title");
                assert_eq!(kind, "terminal");
                assert_eq!(dur, 45);
                tool_call_found = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            tool_call_found,
            "session_tool_calls record must be inserted with self-healed session"
        );
    })
    .await
    .expect("tool_call_event_self_heals_a_missing_session timed out");
}

/// PURPOSE:  In private mode nothing about the user may reach the database;
///           a stored tool call leaks the conversation contents on disk.
/// SCENARIO: Enable private mode, send a tool call, then send a second
///           public tool call and wait for it to appear — because the worker
///           processes its channel in order, the private event has been
///           handled by then.
/// ASSERTS:  The private tool call has zero rows while the public one is
///           stored.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Ignore the private-mode flag in the persistence worker — the
///           zero-count assertion would fail.
#[tokio::test]
async fn tool_call_is_not_persisted_in_private_mode() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_tool_calls_private.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed to run migrations");

        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let is_private = Arc::new(AtomicBool::new(false));

        let persist_tx = spawn_persistence_worker(
            Arc::new(db),
            is_healthy,
            rate,
            Arc::clone(&is_private),
        );

        let orphan_sid = 999111222i64;

        // 1. A public tool call proves the worker is alive and the table works.
        let public_call_id = "call_public_001".to_string();
        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: public_call_id.clone(),
                session_id: orphan_sid,
                turn_id: 1,
                tool_name: "respond_and_set_title".to_string(),
                tool_flow: vox_lib::services::llm::ToolFlow::Terminal,
                arguments: serde_json::json!({ "title": "Public" }),
                result: "ok".to_string(),
                is_error: false,
                duration_ms: 10,
                created_at: 1700000000000,
            })
            .expect("Failed to send public ToolCallExecuted");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut public_found = false;
        while std::time::Instant::now() < deadline {
            let mut rows = conn
                .query(
                    "SELECT COUNT(*) FROM session_tool_calls WHERE id = ?;",
                    (public_call_id.clone(),),
                )
                .await
                .expect("Failed to query session_tool_calls");
            let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
            if count > 0 {
                public_found = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            public_found,
            "the public tool call must be stored — otherwise the test proves nothing"
        );

        // 2. Private mode on: the next tool call must not be written.
        is_private.store(true, Ordering::Relaxed);
        let private_call_id = "call_private_456".to_string();
        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: private_call_id.clone(),
                session_id: orphan_sid,
                turn_id: 2,
                tool_name: "search_memory".to_string(),
                tool_flow: vox_lib::services::llm::ToolFlow::NonTerminal,
                arguments: serde_json::json!({ "query": "secret" }),
                result: "None".to_string(),
                is_error: false,
                duration_ms: 20,
                created_at: 1700000000200,
            })
            .expect("Failed to send private ToolCallExecuted");

        // 3. Quiescence: a third, public tool call queued after the private one.
        //    The worker consumes its channel in order, so once this one is
        //    stored the private event has already been handled.
        let sentinel_call_id = "call_sentinel_789".to_string();
        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: sentinel_call_id.clone(),
                session_id: orphan_sid,
                turn_id: 3,
                tool_name: "respond_and_set_title".to_string(),
                tool_flow: vox_lib::services::llm::ToolFlow::Terminal,
                arguments: serde_json::json!({ "title": "Sentinel" }),
                result: "ok".to_string(),
                is_error: false,
                duration_ms: 10,
                created_at: 1700000000300,
            })
            .expect("Failed to send sentinel ToolCallExecuted");

        let sentinel_deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut sentinel_found = false;
        while std::time::Instant::now() < sentinel_deadline {
            let mut rows = conn
                .query(
                    "SELECT COUNT(*) FROM session_tool_calls WHERE id = ?;",
                    (sentinel_call_id.clone(),),
                )
                .await
                .expect("Failed to query session_tool_calls");
            let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
            if count > 0 {
                sentinel_found = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            sentinel_found,
            "the sentinel tool call must be stored to prove the private event was processed"
        );

        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM session_tool_calls WHERE id = ?;",
                (private_call_id,),
            )
            .await
            .expect("Query failed");
        let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            count, 0,
            "Tool call record must NOT be inserted into SQLite when private mode is active"
        );
    })
    .await
    .expect("tool_call_is_not_persisted_in_private_mode timed out");
}
