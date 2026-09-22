//! ============================================================================
//! tests/database_persistence_boundary_test.rs — Turso Database Boundary & MVCC Integration Tests
//! ============================================================================
//! Category     : Integration Test (Seam 20)
//! Component    : persistence/{db,schema,worker,sessions,projects,facts,compactions}
//! Prerequisites: Turso SQLite engine (vox.db), isolated temporary database
//! Execution    : cargo nextest run --test database_persistence_boundary_test --release --nocapture --test-threads=1
//! Metrics      : Schema migrations user_version = 5, foreign key RESTRICT / CASCADE / SET NULL,
//!                unique partial index concurrency enforcement, multi-threaded MVCC read/write,
//!                F32_BLOB 384-dimensional vector float precision
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
    db::VoxDb,
    decode_f32_blob, encode_f32_blob,
    facts::{insert_fact, insert_vector, FactRecord},
    notifications::{create_notification, NewNotification, Severity},
    personal_memory::get_personal_memory,
    projects::{delete_project, get_project_by_id},
    schema::run_migrations,
    sessions::{create_session_with_id, delete_session},
    worker::spawn_persistence_worker,
    PersistenceEvent,
};

// ============================================================================
// Subtest 1: Database Initialization, Schema Migration & Seed Data
// ============================================================================
/// Entry Seam A: `VoxDb::open` and `run_migrations`
///
/// Verifies:
///   - Database opens and applies pragmas (foreign_keys = ON, busy_timeout).
///   - `run_migrations` transitions database to schema version 5 (`PRAGMA user_version = 5`).
///   - Seed project `'default'` is created.
///   - Seed global personal memory document (project_id = NULL, version = 1) is created.
///   - Core v2 tables exist in `sqlite_master`.
#[tokio::test]
async fn test_schema_migration_and_seed_data() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_migration.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");

        // Run migrations
        run_migrations(&conn)
            .await
            .expect("Failed to execute schema migrations");

        // 1. Verify PRAGMA user_version = 5
        let mut rows = conn
            .query("PRAGMA user_version;", ())
            .await
            .expect("Failed to query user_version");
        let version: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            version, 5,
            "Database user_version must be 5 after migration"
        );

        // 2. Verify foreign_keys = ON
        let mut fk_rows = conn
            .query("PRAGMA foreign_keys;", ())
            .await
            .expect("Failed to query foreign_keys");
        let fk_enabled: i64 = fk_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(fk_enabled, 1, "PRAGMA foreign_keys must be enabled (1)");

        // 3. Verify seed project 'default' exists
        let default_project = get_project_by_id(&conn, "default")
            .await
            .expect("Failed to query default project");
        assert!(default_project.is_some(), "Default project must be seeded");
        assert_eq!(default_project.unwrap().id, "default");

        // 4. Verify seed global personal memory exists (version = 1)
        let pm = get_personal_memory(&conn, None)
            .await
            .expect("Failed to query personal memory");
        assert_eq!(pm.version, 1, "Seed personal memory must be version 1");
        assert!(pm.project_id.is_none());

        // 5. Verify core v2 tables exist
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
    .expect("test_schema_migration_and_seed_data timed out");
}

// ============================================================================
// Subtest 2: Relational Cascades & Foreign Key Restrictions
// ============================================================================
/// Entry Seam C: Relational foreign key cascade and restriction enforcement
///
/// Verifies:
///   - `ON DELETE RESTRICT`: Attempting to delete a project containing active sessions fails.
///   - `ON DELETE CASCADE`: Deleting a session cascades to turns, notifications, and compactions.
///   - `ON DELETE SET NULL`: Deleting a session decouples memory facts (`session_id` becomes NULL)
///     without deleting the historical facts.
///   - `ON DELETE CASCADE` (Facts to Vectors): Deleting a fact cascades to `memory_facts_vectors`.
#[tokio::test]
async fn test_relational_cascades_and_foreign_key_restrictions() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_cascades.db");

        let db = VoxDb::open(&db_path).await.expect("Failed to open VoxDb");
        let conn = db.connect().expect("Failed to vend connection");
        run_migrations(&conn).await.expect("Failed migrations");

        // Create a provenance source session for compaction so compaction survives session_id deletion
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

        // 1. RESTRICT Check: Cannot delete project 'default' while session exists
        let delete_proj_res = delete_project(&conn, "default").await;
        assert!(
            delete_proj_res.is_err(),
            "delete_project must fail due to ON DELETE RESTRICT when child sessions exist"
        );

        // 2. Populate child entities belonging to session_id
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

        // Insert tool call belonging to session_id
        conn.execute(
            "INSERT INTO session_tool_calls (id, session_id, turn_id, tool_name, tool_kind, arguments, result, is_error, duration_ms, created_at) \
             VALUES ('call_cascade_test', ?, 1, 'respond_and_set_title', 'terminal', '{}', 'ok', 0, 5, 1000);",
            (session_id,),
        )
        .await
        .expect("Failed to insert tool call");

        // Insert fact associated with session_id
        let fact_id = "fact_cascade_20201".to_string();
        let fact_rec = FactRecord {
            id: fact_id.clone(),
            session_id: Some(session_id),
            compaction_id,
            fact_type: "objective".to_string(),
            text: "User is building integration tests".to_string(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &fact_rec)
            .await
            .expect("Failed to insert fact");

        // Insert vector for fact
        let test_vec = vec![0.5f32; 384];
        insert_vector(
            &conn,
            &fact_id,
            "objective",
            "active",
            Some("default"),
            &test_vec,
        )
        .await
        .expect("Failed to insert vector");

        // 3. CASCADE Check: Hard delete session
        delete_session(&conn, session_id, true)
            .await
            .expect("Failed to delete session");

        // Turns must be deleted (CASCADE)
        let mut turn_rows = conn
            .query(
                "SELECT COUNT(*) FROM turns WHERE session_id = ?;",
                (session_id,),
            )
            .await
            .unwrap();
        let turn_count: i64 = turn_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            turn_count, 0,
            "Turns must be deleted on session delete (CASCADE)"
        );

        // Notifications must be deleted (CASCADE)
        let mut notif_rows = conn
            .query(
                "SELECT COUNT(*) FROM notifications WHERE session_id = ?;",
                (session_id,),
            )
            .await
            .unwrap();
        let notif_count: i64 = notif_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            notif_count, 0,
            "Notifications must be deleted on session delete (CASCADE)"
        );

        // Compactions must be deleted (CASCADE)
        let mut comp_rows = conn
            .query(
                "SELECT COUNT(*) FROM session_compactions WHERE session_id = ?;",
                (session_id,),
            )
            .await
            .unwrap();
        let comp_count: i64 = comp_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            comp_count, 0,
            "Compactions must be deleted on session delete (CASCADE)"
        );

        // Tool calls must be deleted (CASCADE)
        let mut tc_rows = conn
            .query(
                "SELECT COUNT(*) FROM session_tool_calls WHERE session_id = ?;",
                (session_id,),
            )
            .await
            .unwrap();
        let tc_count: i64 = tc_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            tc_count, 0,
            "Tool calls must be deleted on session delete (CASCADE)"
        );

        // 4. SET NULL Check: Fact must survive with session_id = NULL
        let mut fact_rows = conn
            .query(
                "SELECT session_id FROM memory_facts WHERE id = ?;",
                (fact_id.clone(),),
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

        // Vector must still exist while fact exists
        let mut vec_rows = conn
            .query(
                "SELECT COUNT(*) FROM memory_facts_vectors WHERE fact_id = ?;",
                (fact_id.clone(),),
            )
            .await
            .unwrap();
        let vec_count: i64 = vec_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(vec_count, 1, "Vector must still exist while fact exists");

        // 5. CASCADE Check: Deleting fact cascades to vector
        conn.execute("DELETE FROM memory_facts WHERE id = ?;", (fact_id.clone(),))
            .await
            .expect("Failed to delete fact");

        let mut vec_after = conn
            .query(
                "SELECT COUNT(*) FROM memory_facts_vectors WHERE fact_id = ?;",
                (fact_id,),
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
    .expect("test_relational_cascades_and_foreign_key_restrictions timed out");
}

// ============================================================================
// Subtest 3: Unique Partial Index Enforces Single In-Progress Compaction
// ============================================================================
/// Entry Seam C: Engine-level constraint via `idx_compactions_one_in_progress`
///
/// Verifies:
///   - Starting a second compaction in `in_progress` status on the same session
///     fails with a unique constraint violation at the SQLite engine level.
///   - Once the active compaction transitions to `completed` or `failed`, a new
///     `in_progress` compaction on that session succeeds.
#[tokio::test]
async fn test_unique_partial_index_one_in_progress_compaction() {
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

        // 1. Record initial in_progress compaction
        let run_1 = record_compaction_start(&conn, session_id, "manual", 1, 10)
            .await
            .expect("First in_progress compaction must succeed");
        assert!(run_1 > 0);

        // 2. Attempt to record a second concurrent in_progress compaction on same session
        let run_2_res = record_compaction_start(&conn, session_id, "soft", 11, 20).await;
        assert!(
            run_2_res.is_err(),
            "Second in_progress compaction on same session must be rejected by partial index"
        );
        let err_str = run_2_res.unwrap_err().to_string();
        assert!(
            err_str.contains("UNIQUE constraint failed") || err_str.contains("conflict") || err_str.contains("idx_compactions_one_in_progress"),
            "Error must indicate unique constraint failure, got: {}",
            err_str
        );

        // 3. Transition run_1 to completed
        conn.execute(
            "UPDATE session_compactions SET status = 'completed', finished_at = 2000 WHERE id = ?;",
            (run_1,),
        )
        .await
        .expect("Failed to complete compaction");

        // 4. Now a new in_progress compaction on same session must succeed
        let run_3 = record_compaction_start(&conn, session_id, "soft", 11, 20)
            .await
            .expect("New in_progress compaction must succeed after previous one completed");
        assert!(run_3 > run_1);

        // 5. Multiple completed compactions on same session are allowed (WHERE status = 'in_progress')
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
        assert_eq!(completed_count, 2, "Multiple completed compactions must exist without index conflicts");
    })
    .await
    .expect("test_unique_partial_index_one_in_progress_compaction timed out");
}

// ============================================================================
// Subtest 4: Multi-Threaded MVCC Concurrency Under WAL
// ============================================================================
/// Entry Seam B: Persistence Worker & Concurrent Readers
///
/// Verifies:
///   - Dedicated background persistence worker thread processing write events
///     never blocks or fails concurrent reader threads querying sessions and facts.
///   - Zero `database locked` or SQLite busy errors occur under concurrent read/write load.
#[tokio::test]
async fn test_concurrent_mvcc_wal_readers_and_persistence_worker() {
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

        // Spawn persistence worker on dedicated connection
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
        tokio::time::sleep(Duration::from_millis(100)).await;

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

        // Verify turns were written
        let verify_conn = db.connect().unwrap();
        let mut final_turns = verify_conn
            .query(
                "SELECT COUNT(*) FROM turns WHERE session_id = ?;",
                (session_id,),
            )
            .await
            .unwrap();
        let final_count: i64 = final_turns.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(final_count, 25, "All 25 turns must be persisted by worker");
    })
    .await
    .expect("test_concurrent_mvcc_wal_readers_and_persistence_worker timed out");
}

// ============================================================================
// Subtest 5: F32_BLOB 384-Dimensional Vector Precision Roundtrip
// ============================================================================
/// Entry Seam C: Dense vector serialization and raw SQLite storage
///
/// Verifies:
///   - `encode_f32_blob` encodes 384 floating-point numbers into 1536 bytes.
///   - Storing into Turso `F32_BLOB(384)` column preserves exact IEEE 754 float representation.
///   - `decode_f32_blob` reproduces the original vector with bit-for-bit equality across all 384 dimensions.
#[tokio::test]
async fn test_f32_blob_vector_precision_roundtrip() {
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

        let fact_id = "fact_vector_precision_test";
        let fact_rec = FactRecord {
            id: fact_id.to_string(),
            session_id: Some(session_id),
            compaction_id,
            fact_type: "objective".to_string(),
            text: "Vector precision test fact".to_string(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &fact_rec)
            .await
            .expect("Failed to insert parent fact");

        // 4. Insert vector via production helper
        insert_vector(&conn, fact_id, "objective", "active", None, &original_vec)
            .await
            .expect("Failed to insert vector");

        // 5. Select raw BLOB from SQLite
        let mut rows = conn
            .query(
                "SELECT embedding FROM memory_facts_vectors WHERE fact_id = ?;",
                (fact_id,),
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
    .expect("test_f32_blob_vector_precision_roundtrip timed out");
}
