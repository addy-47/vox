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
    paired_devices::{
        delete_paired_device, get_paired_device, insert_paired_device, list_paired_devices,
        mark_paired_device_seen, update_paired_device_hints, NewPairedDevice, PairedDeviceRole,
    },
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
        let _paths = TempPathsGuard::new();
        vox_lib::utils::paths::init();
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
            "INSERT INTO session_tool_calls (id, session_id, turn_id, tool_name, arguments, result, is_error, duration_ms, created_at) \
             VALUES ('call_cascade_test', ?, 1, 'set_title', '{}', 'ok', 0, 5, 1000);",
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
        let _paths = TempPathsGuard::new();
        vox_lib::utils::paths::init();
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

        conn.execute(
            "DELETE FROM memory_facts WHERE id = ?;",
            (observation_id.clone(),),
        )
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

/// PURPOSE:  Only one compaction may be in progress across the whole database (db-spec §5.4/§8).
/// SCENARIO: Record an in-progress compaction, then try a second for the same session and for a different session.
/// ASSERTS:  Both calls fail with a unique-constraint error naming the partial index.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Scope `idx_compactions_one_in_progress` by session_id — the cross-session assertion would fail.
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

        // db-spec §5.4 requires GLOBAL exclusion, not per-session exclusion.
        // A compaction for a different session must also be rejected while run_1 is active.
        let other_session = 20302i64;
        create_session_with_id(&conn, other_session, Some("default"))
            .await
            .expect("Failed to create second session");
        let cross_session_res = record_compaction_start(&conn, other_session, "soft", 1, 10).await;
        assert!(
            cross_session_res.is_err(),
            "db-spec §5.4: only one in_progress compaction may exist across all sessions"
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
        vox_lib::utils::paths::init();
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
        vox_lib::utils::paths::init();
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
        let _paths = TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_tool_calls.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn)
            .await
            .expect("Failed to run migrations");

        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let is_private = Arc::new(AtomicBool::new(false));

        let persist_tx =
            spawn_persistence_worker(Arc::new(db), is_healthy, rate, Arc::clone(&is_private));

        let orphan_sid = 999111222i64;
        let call_id = "call_test_123".to_string();
        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: call_id.clone(),
                session_id: orphan_sid,
                turn_id: 1,
                step_id: None,
                step_index: 0,
                tool_name: "set_title".to_string(),
                arguments: serde_json::json!({
                    "title": "Self Healing Session"
                }),
                result: "Session title updated".to_string(),
                is_error: false,
                duration_ms: 45,
                truncated: false,
                spillover_path: None,
                created_at: 1700000000100,
            })
            .expect("Failed to send ToolCallExecuted");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut tool_call_found = false;
        while std::time::Instant::now() < deadline {
            let mut rows = conn
                .query(
                    "SELECT tool_name, duration_ms FROM session_tool_calls WHERE id = ?;",
                    (call_id.clone(),),
                )
                .await
                .expect("Failed to query session_tool_calls");

            if let Ok(Some(row)) = rows.next().await {
                let name: String = row.get(0).unwrap();
                let dur: i64 = row.get(1).unwrap();
                assert_eq!(name, "set_title");
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
        let _paths = TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_tool_calls_private.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn)
            .await
            .expect("Failed to run migrations");

        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let is_private = Arc::new(AtomicBool::new(false));

        let persist_tx =
            spawn_persistence_worker(Arc::new(db), is_healthy, rate, Arc::clone(&is_private));

        let orphan_sid = 999111222i64;

        // 1. A public tool call proves the worker is alive and the table works.
        let public_call_id = "call_public_001".to_string();
        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: public_call_id.clone(),
                session_id: orphan_sid,
                turn_id: 1,
                step_id: None,
                step_index: 0,
                tool_name: "set_title".to_string(),
                arguments: serde_json::json!({ "title": "Public" }),
                result: "ok".to_string(),
                is_error: false,
                duration_ms: 10,
                truncated: false,
                spillover_path: None,
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
        //
        //    Quiescence mechanism: the worker's own acknowledgement. When the
        //    persistence worker consumes an event in private mode it logs
        //    "Private Mode active: skipping tool call record". Waiting for
        //    that line proves the worker has handled the private event, so
        //    the absence assertion below is not a race against the worker.
        common::log_capture::install();
        let _ = common::log_capture::take();

        is_private.store(true, Ordering::Relaxed);
        let private_call_id = "call_private_456".to_string();
        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: private_call_id.clone(),
                session_id: orphan_sid,
                turn_id: 2,
                step_id: None,
                step_index: 0,
                tool_name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "secret" }),
                result: "None".to_string(),
                is_error: false,
                duration_ms: 20,
                truncated: false,
                spillover_path: None,
                created_at: 1700000000200,
            })
            .expect("Failed to send private ToolCallExecuted");

        let ack_deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut captured = String::new();
        loop {
            captured.push_str(&common::log_capture::take_text());
            if captured.contains("skipping tool call record") {
                break;
            }
            assert!(
                std::time::Instant::now() < ack_deadline,
                "the persistence worker never acknowledged the private event; captured log:\n{}",
                captured
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

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

// ============================================================================
// Subtest 12: Paired Devices Schema & Storage Constraints
// ============================================================================

/// PURPOSE:  paired_devices table must enforce role allow-list, 32-byte key constraint, and FK independence.
/// SCENARIO: Execute migrations on isolated database; verify table info, check constraints, and FK absence.
/// ASSERTS:  Table exists with 8 documented columns; invalid role rejected; 31-byte key rejected; 0 FKs.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Remove CHECK (length(public_key) = 32) constraint from migration.
#[tokio::test]
async fn paired_devices_table_schema_constraints_and_migration() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let dir = tempdir().expect("tempdir");
        let db = VoxDb::open(dir.path().join("paired_devices_schema.db"))
            .await
            .expect("open db");
        let conn = db.connect().expect("connect");
        run_migrations(&conn).await.expect("migrations");

        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='paired_devices';",
                (),
            )
            .await
            .expect("table count");
        let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 1);

        // Columns check
        let mut columns = Vec::new();
        let mut col_rows = conn
            .query("PRAGMA table_info(paired_devices);", ())
            .await
            .expect("table_info");
        while let Some(row) = col_rows.next().await.expect("table_info row") {
            let name: String = row.get(1).unwrap();
            let decl_type: String = row.get(2).unwrap();
            let not_null: i64 = row.get(3).unwrap();
            let pk: i64 = row.get(5).unwrap();
            columns.push((name, decl_type, not_null, pk));
        }

        let expected = [
            ("device_id", "TEXT", 0, 1),
            ("public_key", "BLOB", 1, 0),
            ("display_name", "TEXT", 1, 0),
            ("role", "TEXT", 1, 0),
            ("trust_level", "TEXT", 1, 0),
            ("hints", "TEXT", 1, 0),
            ("paired_at", "INTEGER", 1, 0),
            ("last_seen_at", "INTEGER", 0, 0),
        ];
        assert_eq!(columns.len(), expected.len());
        for (actual, wanted) in columns.iter().zip(expected.iter()) {
            assert_eq!(actual.0, wanted.0);
            assert_eq!(actual.1, wanted.1);
            assert_eq!(actual.2, wanted.2);
            assert_eq!(actual.3, wanted.3);
        }

        // Invalid role rejected
        let bad_role = conn
            .execute(
                "INSERT INTO paired_devices \
                 (device_id, public_key, display_name, role, trust_level, hints, paired_at) \
                 VALUES ('bad-role-device', ?, 'Spectator', 'spectator', 'full', '[]', 1);",
                (vec![0u8; 32],),
            )
            .await;
        assert!(bad_role.is_err());

        // Short key rejected
        let bad_key = conn
            .execute(
                "INSERT INTO paired_devices \
                 (device_id, public_key, display_name, role, trust_level, hints, paired_at) \
                 VALUES ('bad-key-device', ?, 'Short Key', 'controller', 'full', '[]', 1);",
                (vec![0u8; 31],),
            )
            .await;
        assert!(bad_key.is_err());

        // Zero FK dependencies
        let mut fk_rows = conn
            .query("PRAGMA foreign_key_list(paired_devices);", ())
            .await
            .expect("fk query");
        let mut fk_count = 0;
        while fk_rows.next().await.expect("fk iter").is_some() {
            fk_count += 1;
        }
        assert_eq!(fk_count, 0);
    })
    .await
    .expect("timeout");
}

// ============================================================================
// Subtest 13: Paired Devices CRUD & Unrecognized Hint Roundtrip
// ============================================================================

/// PURPOSE:  Paired devices persistence module must support complete CRUD without losing unrecognised hint kinds.
/// SCENARIO: Insert pairing; lookup; list; update hints with unknown json; mark last_seen; then delete.
/// ASSERTS:  All operations succeed; unrecognised hint roundtrips untouched; deleted row is gone.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Strip unrecognized fields when updating hints in update_paired_device_hints.
#[tokio::test]
async fn paired_devices_access_layer_crud_and_unrecognized_hints_roundtrip() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let dir = tempdir().expect("tempdir");
        let db = VoxDb::open(dir.path().join("paired_devices_crud.db"))
            .await
            .expect("open db");
        let conn = db.connect().expect("connect");
        run_migrations(&conn).await.expect("migrations");

        let device_id = "9f8c1d2b3a4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8";
        let public_key = vec![0u8; 32];

        // 1. Insert
        let inserted = insert_paired_device(
            &conn,
            &NewPairedDevice {
                device_id: device_id.to_string(),
                public_key: public_key.clone(),
                display_name: "Ada's Phone".to_string(),
                role: PairedDeviceRole::Controller,
                trust_level: "full".to_string(),
                hints: Vec::new(),
            },
        )
        .await
        .expect("insert");

        assert_eq!(inserted.device_id, device_id);
        assert_eq!(inserted.role, PairedDeviceRole::Controller);
        assert!(inserted.paired_at > 0);
        assert!(inserted.last_seen_at.is_none());

        // 2. Lookup & List
        let fetched = get_paired_device(&conn, device_id).await.unwrap().unwrap();
        assert_eq!(fetched, inserted);

        let listed = list_paired_devices(&conn).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].device_id, device_id);

        // 3. Hints update with unknown hint kind
        let known = serde_json::json!({ "kind": "lan", "addr": "192.168.1.24", "port": 51234 });
        let unknown = serde_json::json!({ "kind": "relay", "relay_id": "eu-west-3" });
        let hints = vec![known, unknown.clone()];
        assert_eq!(
            update_paired_device_hints(&conn, device_id, &hints)
                .await
                .unwrap(),
            1
        );
        let after_hints = get_paired_device(&conn, device_id).await.unwrap().unwrap();
        assert_eq!(after_hints.hints[1], unknown);

        // 4. Mark seen
        assert_eq!(mark_paired_device_seen(&conn, device_id).await.unwrap(), 1);
        let after_seen = get_paired_device(&conn, device_id).await.unwrap().unwrap();
        assert!(after_seen.last_seen_at.is_some());

        // 5. Delete
        assert!(delete_paired_device(&conn, device_id).await.unwrap());
        assert!(get_paired_device(&conn, device_id).await.unwrap().is_none());
        assert!(list_paired_devices(&conn).await.unwrap().is_empty());
        assert!(!delete_paired_device(&conn, device_id).await.unwrap());
    })
    .await
    .expect("timeout");
}

// Subtest 14: History Search matches titles and turn content with snippets
//
// PURPOSE:  search_history must find matching sessions by title or turn speech
//           and return accurate snippet extracts without client-side turn scanning.
// SCENARIO: Create sessions and turns, search for words in title and utterance.
// ASSERTS:  search_history returns matched session, match_type, and snippet.
// CLASS:    happy-path
// REQUIRES: none (temporary Turso database)
// MUTANT:   Break LIKE query or extract_snippet logic in search_history.
#[tokio::test]
async fn search_history_matches_titles_and_turns() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test_search.db");
        let db = VoxDb::open(&db_path).await.expect("open Turso database");
        let conn = db.connect().expect("connect");
        run_migrations(&conn).await.expect("run_migrations");

        // Seed session 1: title match
        let s1_id = 1001i64;
        conn.execute(
            "INSERT INTO sessions (id, project_id, title, is_pinned, created_at, updated_at)
             VALUES (?, 'default', 'Architecture Discussion', 0, 1000, 1000)",
            (s1_id,),
        )
        .await
        .unwrap();

        // Seed session 2: turn match
        let s2_id = 1002i64;
        conn.execute(
            "INSERT INTO sessions (id, project_id, title, is_pinned, created_at, updated_at)
             VALUES (?, 'default', 'Regular Sync', 0, 2000, 2000)",
            (s2_id,),
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at)
             VALUES (?, 1, 'Can you help me configure the Turso database engine?', 'Sure, Turso is pure Rust.', 2000)",
            (s2_id,),
        )
        .await
        .unwrap();

        // 1. Search title keyword
        let hits_title = vox_lib::persistence::search_history(&conn, "Architecture", None, 10)
            .await
            .expect("search_history title");
        assert_eq!(hits_title.len(), 1);
        assert_eq!(hits_title[0].session.id, s1_id);
        assert_eq!(hits_title[0].match_type, "title");

        // 2. Search turn keyword in user utterance
        let hits_turn = vox_lib::persistence::search_history(&conn, "configure the Turso", None, 10)
            .await
            .expect("search_history turn");
        assert_eq!(hits_turn.len(), 1);
        assert_eq!(hits_turn[0].session.id, s2_id);
        assert_eq!(hits_turn[0].match_type, "turn");
        assert_eq!(hits_turn[0].matched_role.as_deref(), Some("user"));
        assert!(hits_turn[0].matched_snippet.as_ref().unwrap().contains("Turso"));

        // 3. Search non-existent keyword
        let hits_none = vox_lib::persistence::search_history(&conn, "nonexistent_keyword_xyz", None, 10)
            .await
            .expect("search_history empty");
        assert!(hits_none.is_empty());
    })
    .await
    .expect("timeout");
}

/// PURPOSE:  Catch foreign key constraint failures (`step_id REFERENCES session_steps(id)`) or missing step trajectories in Turso DB during multi-step tool execution.
/// SCENARIO: Execute a 2-step turn through `PersistenceWorker` where Step 0 runs 2 tools (`search_memory`, `web_search`) with preamble "Checking notes." and Step 1 runs 1 tool (`set_title`).
/// ASSERTS:  `session_steps` contains 2 rows for `(session_id, turn_id)` with `step_index = 0` (`tool_count = 2`, `preamble_text = "Checking notes."`) and `step_index = 1` (`tool_count = 1`); `session_tool_calls` contains 3 rows whose `step_id` foreign keys match the respective `session_steps.id` rows.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   In `step6_execute_tool_batch` (`steps.rs`), dispatch `PersistenceEvent::ToolCallExecuted` before `PersistenceEvent::StepCompleted` so the foreign key lookup fails.
#[tokio::test]
async fn multi_step_turn_persists_session_steps_and_foreign_key_linked_tool_calls() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _paths = TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test_steps.db");

        let db = VoxDb::open(&db_path).await.expect("open db");
        let conn = db.connect().expect("connect");
        run_migrations(&conn).await.expect("migrations");

        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let is_private = Arc::new(AtomicBool::new(false));

        let persist_tx = spawn_persistence_worker(
            Arc::new(db),
            is_healthy,
            rate,
            Arc::clone(&is_private),
        );

        let session_id = 888001i64;
        let turn_id = 1u32;
        let step0_id = "step-0-uuid".to_string();
        let step1_id = "step-1-uuid".to_string();

        let now = 1700000000000u64;
        persist_tx
            .send(PersistenceEvent::SessionStarted {
                session_id,
                timestamp_ms: now,
            })
            .expect("SessionStarted");

        // Step 0: 2 tools (search_memory, web_search), preamble "Checking notes."
        persist_tx
            .send(PersistenceEvent::StepCompleted {
                id: step0_id.clone(),
                session_id,
                turn_id,
                step_index: 0,
                content: Some("Checking notes.".to_string()),
                tool_count: 2,
                duration_ms: Some(120),
                prompt_tokens: None,
                completion_tokens: None,
                guardrail_strike: 0,
                created_at: now + 10,
            })
            .expect("StepCompleted 0");

        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: "call-search-0".to_string(),
                session_id,
                turn_id,
                step_id: Some(step0_id.clone()),
                step_index: 0,
                tool_name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "notes" }),
                result: "Found note".to_string(),
                is_error: false,
                duration_ms: 50,
                truncated: false,
                spillover_path: None,
                created_at: now + 20,
            })
            .expect("ToolCallExecuted 0a");

        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: "call-web-0".to_string(),
                session_id,
                turn_id,
                step_id: Some(step0_id.clone()),
                step_index: 0,
                tool_name: "web_search".to_string(),
                arguments: serde_json::json!({ "query": "current status" }),
                result: "Web result".to_string(),
                is_error: false,
                duration_ms: 60,
                truncated: false,
                spillover_path: None,
                created_at: now + 30,
            })
            .expect("ToolCallExecuted 0b");

        // Step 1: 1 tool (set_title)
        persist_tx
            .send(PersistenceEvent::StepCompleted {
                id: step1_id.clone(),
                session_id,
                turn_id,
                step_index: 1,
                content: None,
                tool_count: 1,
                duration_ms: Some(40),
                prompt_tokens: None,
                completion_tokens: None,
                guardrail_strike: 0,
                created_at: now + 100,
            })
            .expect("StepCompleted 1");

        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: "call-title-1".to_string(),
                session_id,
                turn_id,
                step_id: Some(step1_id.clone()),
                step_index: 1,
                tool_name: "set_title".to_string(),
                arguments: serde_json::json!({ "title": "New Title" }),
                result: "Title updated".to_string(),
                is_error: false,
                duration_ms: 35,
                truncated: false,
                spillover_path: None,
                created_at: now + 110,
            })
            .expect("ToolCallExecuted 1");

        // Quiescence: wait until worker has processed all events
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut steps_persisted = false;
        while std::time::Instant::now() < deadline {
            if let Ok(mut rows) = conn
                .query(
                    "SELECT COUNT(*) FROM session_steps WHERE session_id = ? AND turn_id = ?;",
                    (session_id, turn_id),
                )
                .await
            {
                if let Ok(Some(row)) = rows.next().await {
                    let count: i64 = row.get(0).unwrap_or(0);
                    if count == 2 {
                        steps_persisted = true;
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            steps_persisted,
            "session_steps must contain 2 rows for session {} turn {}",
            session_id, turn_id
        );

        // Verify Step 0 details
        let mut s0_rows = conn
            .query(
                "SELECT tool_count, content FROM session_steps WHERE id = ?;",
                (step0_id.clone(),),
            )
            .await
            .expect("query step 0");
        let s0_row = s0_rows.next().await.unwrap().expect("step 0 row");
        let s0_tool_count: i64 = s0_row.get(0).unwrap();
        let s0_content: Option<String> = s0_row.get(1).unwrap();
        assert_eq!(s0_tool_count, 2);
        assert_eq!(s0_content.as_deref(), Some("Checking notes."));

        // Verify Step 1 details
        let mut s1_rows = conn
            .query(
                "SELECT tool_count, content FROM session_steps WHERE id = ?;",
                (step1_id.clone(),),
            )
            .await
            .expect("query step 1");
        let s1_row = s1_rows.next().await.unwrap().expect("step 1 row");
        let s1_tool_count: i64 = s1_row.get(0).unwrap();
        assert_eq!(s1_tool_count, 1);

        // Verify session_tool_calls links to session_steps.id
        let mut tc_rows = conn
            .query(
                "SELECT step_id FROM session_tool_calls WHERE session_id = ? ORDER BY created_at ASC;",
                (session_id,),
            )
            .await
            .expect("query tool calls");
        let mut linked_step_ids = Vec::new();
        while let Ok(Some(row)) = tc_rows.next().await {
            let sid: Option<String> = row.get(0).unwrap();
            linked_step_ids.push(sid);
        }
        assert_eq!(linked_step_ids.len(), 3);
        assert_eq!(linked_step_ids[0].as_deref(), Some(step0_id.as_str()));
        assert_eq!(linked_step_ids[1].as_deref(), Some(step0_id.as_str()));
        assert_eq!(linked_step_ids[2].as_deref(), Some(step1_id.as_str()));
    })
    .await
    .expect("multi_step_turn_persists_session_steps_and_foreign_key_linked_tool_calls timed out");
}

/// PURPOSE:  Catch orphaned rows in `session_steps` or `session_tool_calls` when a parent session is hard-deleted (`RULE-DB-10`).
/// SCENARIO: Persist a session with 1 turn, 2 `session_steps`, and 3 `session_tool_calls`, then execute hard session deletion.
/// ASSERTS:  `SELECT COUNT(*) FROM session_steps WHERE session_id = ?` is 0, and `SELECT COUNT(*) FROM session_tool_calls WHERE session_id = ?` is 0.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   In `persistence/schema.rs`, omit `ON DELETE CASCADE` from `session_steps.session_id` or `session_tool_calls.step_id`.
#[tokio::test]
async fn session_hard_delete_cascades_to_session_steps_and_session_tool_calls() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _paths = TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test_cascade.db");

        let db = VoxDb::open(&db_path).await.expect("open db");
        let conn = db.connect().expect("connect");
        run_migrations(&conn).await.expect("migrations");

        let session_id = 999222i64;
        let now = 1700000000000i64;

        conn.execute(
            "INSERT INTO sessions (id, project_id, title, created_at, updated_at) VALUES (?, 'default', 'Test Cascade', ?, ?);",
            (session_id, now, now),
        )
        .await
        .expect("insert session");

        conn.execute(
            "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (?, 1, 'Hello', 'World', ?);",
            (session_id, now),
        )
        .await
        .expect("insert turn");

        // Insert 2 session_steps
        conn.execute(
            "INSERT INTO session_steps (id, session_id, turn_id, step_index, content, tool_count, duration_ms, created_at)
             VALUES ('step-cas-0', ?, 1, 0, 'Preamble', 2, 100, ?);",
            (session_id, now),
        )
        .await
        .expect("insert step 0");

        conn.execute(
            "INSERT INTO session_steps (id, session_id, turn_id, step_index, content, tool_count, duration_ms, created_at)
             VALUES ('step-cas-1', ?, 1, 1, NULL, 1, 50, ?);",
            (session_id, now),
        )
        .await
        .expect("insert step 1");

        // Insert 3 session_tool_calls referencing session_steps
        conn.execute(
            "INSERT INTO session_tool_calls (id, session_id, turn_id, step_id, step_index, tool_name, arguments, result, is_error, duration_ms, created_at)
             VALUES ('tc-1', ?, 1, 'step-cas-0', 0, 'search_memory', '{}', 'ok', 0, 40, ?);",
            (session_id, now),
        )
        .await
        .expect("insert tc 1");

        conn.execute(
            "INSERT INTO session_tool_calls (id, session_id, turn_id, step_id, step_index, tool_name, arguments, result, is_error, duration_ms, created_at)
             VALUES ('tc-2', ?, 1, 'step-cas-0', 0, 'web_search', '{}', 'ok', 0, 50, ?);",
            (session_id, now),
        )
        .await
        .expect("insert tc 2");

        conn.execute(
            "INSERT INTO session_tool_calls (id, session_id, turn_id, step_id, step_index, tool_name, arguments, result, is_error, duration_ms, created_at)
             VALUES ('tc-3', ?, 1, 'step-cas-1', 1, 'set_title', '{}', 'ok', 0, 30, ?);",
            (session_id, now),
        )
        .await
        .expect("insert tc 3");

        // Delete session (hard delete)
        delete_session(&conn, session_id, true).await.expect("delete_session");

        // Assert session_steps rows are 0
        let mut step_rows = conn
            .query("SELECT COUNT(*) FROM session_steps WHERE session_id = ?;", (session_id,))
            .await
            .expect("query session_steps count");
        let step_count: i64 = step_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(step_count, 0, "session_steps rows must cascade delete with session");

        // Assert session_tool_calls rows are 0
        let mut tc_rows = conn
            .query("SELECT COUNT(*) FROM session_tool_calls WHERE session_id = ?;", (session_id,))
            .await
            .expect("query session_tool_calls count");
        let tc_count: i64 = tc_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(tc_count, 0, "session_tool_calls rows must cascade delete with session");
    })
    .await
    .expect("session_hard_delete_cascades_to_session_steps_and_session_tool_calls timed out");
}

/// PURPOSE:  Catch dropped v18 provenance columns across persistence worker boundary.
/// SCENARIO: Execute turn through PersistenceWorker with StepCompleted (prompt_tokens = Some(420), completion_tokens = Some(85), guardrail_strike = 1, is_compacted = true) and ToolCallExecuted (truncated = true, spillover_path = Some("/tmp/spill.txt")).
/// ASSERTS:  Querying session_steps returns (420, 85, 1, 1) for (prompt_tokens, completion_tokens, guardrail_strike, is_compacted). Querying session_tool_calls returns (1, "/tmp/spill.txt") for (truncated, spillover_path).
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   In persistence/steps.rs or tool_calls.rs, hardcode 0 or NULL for the v18 provenance columns.
#[tokio::test]
async fn schema_v18_persists_step_tokens_strikes_compaction_and_tool_spillover_metadata() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _paths = TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_schema_v18.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn)
            .await
            .expect("Failed to execute schema migrations");

        let session_id = 18001i64;
        create_session_with_id(&conn, session_id, None)
            .await
            .expect("Failed to create session");

        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let private_mode = Arc::new(AtomicBool::new(false));

        let persist_tx = spawn_persistence_worker(
            Arc::new(db),
            is_healthy,
            rate,
            private_mode,
        );

        let now = 1700000000000u64;
        let step_id = "step-v18-prov-001".to_string();
        let tool_call_id = "tc-v18-prov-001".to_string();

        persist_tx
            .send(PersistenceEvent::StepCompleted {
                id: step_id.clone(),
                session_id,
                turn_id: 1,
                step_index: 0,
                content: Some("Analyzing query".to_string()),
                tool_count: 1,
                duration_ms: Some(120),
                prompt_tokens: Some(420),
                completion_tokens: Some(85),
                guardrail_strike: 1,
                created_at: now,
            })
            .expect("Failed to send StepCompleted");

        persist_tx
            .send(PersistenceEvent::ToolCallExecuted {
                id: tool_call_id.clone(),
                session_id,
                turn_id: 1,
                step_id: Some(step_id.clone()),
                step_index: 0,
                tool_name: "search_memory".to_string(),
                arguments: serde_json::json!({ "query": "auth" }),
                result: "found auth data".to_string(),
                is_error: false,
                duration_ms: 50,
                truncated: true,
                spillover_path: Some("/tmp/spill.txt".to_string()),
                created_at: now + 50,
            })
            .expect("Failed to send ToolCallExecuted");

        let _ = persist_tx.send(PersistenceEvent::Shutdown);

        // Poll database for step and tool call provenance columns
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut step_verified = false;
        let mut tool_verified = false;

        while std::time::Instant::now() < deadline {
            if !step_verified {
                let mut rows = conn
                    .query(
                        "SELECT prompt_tokens, completion_tokens, guardrail_strike, content FROM session_steps WHERE id = ?;",
                        (step_id.clone(),),
                    )
                    .await
                    .expect("query session_steps provenance columns");

                if let Ok(Some(row)) = rows.next().await {
                    let prompt_tok: Option<i64> = row.get(0).expect("prompt_tokens");
                    let comp_tok: Option<i64> = row.get(1).expect("completion_tokens");
                    let strike: i64 = row.get(2).expect("guardrail_strike");
                    let text: Option<String> = row.get(3).expect("content");

                    assert_eq!(prompt_tok, Some(420), "prompt_tokens must be 420");
                    assert_eq!(comp_tok, Some(85), "completion_tokens must be 85");
                    assert_eq!(strike, 1, "guardrail_strike must be 1");
                    assert_eq!(text.as_deref(), Some("Analyzing query"));
                    step_verified = true;
                }
            }

            if !tool_verified {
                let mut tc_rows = conn
                    .query(
                        "SELECT truncated, spillover_path FROM session_tool_calls WHERE id = ?;",
                        (tool_call_id.clone(),),
                    )
                    .await
                    .expect("query session_tool_calls truncation columns");

                if let Ok(Some(row)) = tc_rows.next().await {
                    let truncated: i64 = row.get(0).expect("truncated");
                    let spillover_path: Option<String> = row.get(1).expect("spillover_path");

                    assert_eq!(truncated, 1, "truncated must be 1 (true)");
                    assert_eq!(spillover_path, Some("/tmp/spill.txt".to_string()), "spillover_path must match");
                    tool_verified = true;
                }
            }

            if step_verified && tool_verified {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(step_verified, "session_steps provenance columns must be verified");
        assert!(tool_verified, "session_tool_calls truncation columns must be verified");
    })
    .await
    .expect("test timed out");
}

/// PURPOSE:  Catch disk space leaks in tool output cache when sessions are deleted or orphaned on disk.
/// SCENARIO: Create spillover files under paths::session_tool_outputs_dir(session_a) and paths::session_tool_outputs_dir(orphan_b); call delete_session(&conn, session_a, true) and run_startup_sweeps(&conn).
/// ASSERTS:  Both directories no longer exist on disk.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   In delete_session (sessions.rs), omit the remove_dir_all call.
#[tokio::test]
async fn session_deletion_and_startup_sweep_purge_tier4_tool_output_cache_directories() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _paths = TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_cache_purge.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn)
            .await
            .expect("Failed to execute schema migrations");

        let session_a = 28001i64;
        let orphan_b = 28002i64;

        create_session_with_id(&conn, session_a, None)
            .await
            .expect("Failed to create session_a");
        // orphan_b is deliberately NOT created in db to simulate orphaned session folder

        let dir_a = vox_lib::utils::paths::session_tool_outputs_dir(session_a);
        let dir_b = vox_lib::utils::paths::session_tool_outputs_dir(orphan_b);

        std::fs::create_dir_all(&dir_a).expect("create dir_a");
        std::fs::write(dir_a.join("call_a.txt"), "spillover output for session_a")
            .expect("write file a");

        std::fs::create_dir_all(&dir_b).expect("create dir_b");
        std::fs::write(dir_b.join("call_b.txt"), "spillover output for orphan_b")
            .expect("write file b");

        assert!(dir_a.exists(), "dir_a must exist on disk before deletion");
        assert!(
            dir_b.exists(),
            "dir_b must exist on disk before startup sweep"
        );

        // 1. Delete session_a
        delete_session(&conn, session_a, true)
            .await
            .expect("Failed to hard delete session_a");

        assert!(
            !dir_a.exists(),
            "session_tool_outputs_dir for session_a must be purged upon delete_session"
        );

        // 2. Spawn persistence worker to run startup sweeps on boot
        let is_healthy = Arc::new(AtomicBool::new(true));
        let rate = Arc::new(AtomicU32::new(0));
        let private_mode = Arc::new(AtomicBool::new(false));

        let persist_tx = spawn_persistence_worker(Arc::new(db), is_healthy, rate, private_mode);

        let _ = persist_tx.send(PersistenceEvent::Shutdown);

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline && dir_b.exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            !dir_b.exists(),
            "orphaned tool outputs directory orphan_b must be purged by startup sweeps"
        );
    })
    .await
    .expect("test timed out");
}
