//! ============================================================================
//! tests/personal_memory_test.rs — Personal Memory & Session Continuation v2 Integration Tests
//! ============================================================================
//! Category     : Integration Test (Seam 14)
//! Component    : services/memory/personal, persistence/personal_memory, persistence/sessions, persistence/facts
//! Prerequisites: Turso SQLite engine (vox.db)
//! Execution    : cargo nextest run --test personal_memory_test --release --nocapture --test-threads=1
//! Metrics      : Optimistic concurrency control, fact consolidation status transitions, quiescence gating, disk portability, continuation turn slicing
//! ============================================================================

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{harness::get_test_app_and_state, paths::TempPathsGuard};
use vox_lib::{
    config::PersonalMemorySettings,
    persistence::{
        compactions::{commit_compaction_output, record_compaction_start},
        facts::{
            fetch_active_observations_by_type, insert_observation, insert_vector,
            mark_observations_integrated, ObservationRecord,
        },
        has_in_progress_compaction,
        personal_memory::{
            fetch_pending_revisions, get_personal_memory, list_personal_memory_versions,
            save_personal_memory, set_active_personal_memory_version,
        },
        queue::enqueue_observation,
        sessions::{create_session_with_id, fetch_session_continuation},
        RevisionDecision,
    },
    services::{
        llm::{ConnectionConfig, RemoteTransport},
        memory::personal::{
            batch_resolve_memory_revisions, consolidate_personal_memory,
            save_personal_memory_from_markdown, stage_revisions, ConfirmationReason,
            ConsolidateOutcome, ConsolidationRequest, ResolvedOp,
        },
    },
};

const DUMMY_ENDPOINT_URL: &str = "http://127.0.0.1:11434/v1";
const DUMMY_ENDPOINT_MODEL: &str = "gemma3:12b";

/// Seeds sequential turns into the database with proper session association.
async fn seed_turns(conn: &turso::Connection, session_id: i64, count: u32) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO sessions (id, project_id, title, created_at, updated_at) \
         VALUES (?, 'default', 'Session Continuation Test', ?, ?)",
        (session_id, now, now),
    )
    .await
    .unwrap();

    for turn_id in 1..=count {
        conn.execute(
            "INSERT OR REPLACE INTO turns (session_id, turn_id, user_text, assistant_text, created_at) \
             VALUES (?, ?, ?, ?, ?)",
            (
                session_id,
                turn_id as i64,
                format!("User message {turn_id}"),
                format!("Assistant response {turn_id}"),
                now + turn_id as i64,
            ),
        )
        .await
        .unwrap();
    }
}

// ============================================================================
// Subtest 1: Optimistic Concurrency Control (expected_version)
// ============================================================================
/// Entry Seam A: `save_personal_memory(conn, project_id, content, expected_version)`
///
/// Verifies that manual edits enforce strict optimistic concurrency:
///   - Matching expected_version increments version and persists content.
///   - Stale expected_version fails with an optimistic version conflict error.
///   - Data is preserved intact upon conflict.
#[tokio::test]
async fn test_personal_memory_optimistic_concurrency() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let fact_1 = "User lives in Chicago.";
        let fact_2 = "User prefers Rust for systems programming.";

        // 1. Initial blank memory record starts at version 1
        let initial = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(initial.version, 1);

        // 2. Successful update with matching expected_version (1) increments to version 2
        let doc_v2 = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{fact_1}"}}]}}]}}"#
        );
        let updated = save_personal_memory(&conn, None, &doc_v2, 1).await.unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.content, doc_v2);

        // 3. Concurrent/stale update attempt with old version (1) MUST fail
        let stale_attempt = save_personal_memory(&conn, None, "# Hijacked Content", 1).await;
        assert!(
            stale_attempt.is_err(),
            "Stale expected_version must be rejected"
        );
        let err_msg = stale_attempt.err().unwrap().to_string();
        assert!(
            err_msg.contains("Optimistic version conflict") || err_msg.contains("expected version"),
            "Error message must specify version conflict, got: {}",
            err_msg
        );

        // 4. Verify content was NOT overwritten by stale attempt
        let current = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(current.version, 2);
        assert_eq!(current.content, doc_v2);

        // 5. Successful update with expected_version (2) increments to version 3
        let doc_v3 = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{fact_1}"}},{{"id":"blk_t2","text":"{fact_2}"}}]}}]}}"#
        );
        let updated_v3 = save_personal_memory(&conn, None, &doc_v3, 2).await.unwrap();
        assert_eq!(updated_v3.version, 3);
        assert_eq!(updated_v3.content, doc_v3);
    })
    .await
    .expect("test_personal_memory_optimistic_concurrency timed out");
}

// ============================================================================
// Subtest 3: Consolidation Confirmation Gating (memory-spec.md §5.3)
// ============================================================================
/// Entry Seam B: `consolidate_personal_memory(ConsolidationRequest)`
///
/// Verifies user-controlled gating: a compaction in progress returns
/// `ConfirmationRequired` with no side effects; unfinished queue items return
/// `ConfirmationRequired` unless `forced` is true.
#[tokio::test]
async fn test_consolidation_confirmation_gating() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        let session_id = create_session_with_id(&conn, 14301, Some("default"))
            .await
            .unwrap();

        // Use real RemoteTransport constructor (no mock; network is never touched due to early gating)
        let conn_cfg = ConnectionConfig::new(
            DUMMY_ENDPOINT_URL,
            DUMMY_ENDPOINT_MODEL,
            None,
            Some("ollama"),
        );
        let provider = RemoteTransport::new(conn_cfg);

        // --- Gate Arm 1: In-Progress Compaction ---
        let run_id = record_compaction_start(&conn, session_id, "manual", 1, 10)
            .await
            .unwrap();
        assert!(
            has_in_progress_compaction(&conn).await.unwrap(),
            "Setup: in_progress compaction must be recorded"
        );

        let outcome = consolidate_personal_memory(ConsolidationRequest {
            conn: &conn,
            llm_provider: &provider,
            comments: None,
            project_id: None,
            memory_settings: &PersonalMemorySettings::default(),
            llm_settings: None,
            forced: true,
        })
        .await
        .expect("gating check must not error");
        match outcome {
            ConsolidateOutcome::ConfirmationRequired {
                reason: ConfirmationReason::CompactionInProgress,
                pending_count: 0,
            } => {}
            other => panic!(
                "In-progress compaction must yield ConfirmationRequired(CompactionInProgress), got {:?}",
                std::mem::discriminant(&other)
            ),
        }

        // Clear compaction block by committing it
        commit_compaction_output(
            &conn,
            run_id,
            r#"{"context_summary": "Compaction committed"}"#,
            &[],
            session_id,
        )
        .await
        .unwrap();

        // --- Gate Arm 2: Pending Ingestion Queue Items ---
        let fact_text = "User lives in Chicago.";

        let q_id = enqueue_observation(
            &conn,
            Some(session_id),
            run_id,
            "personal",
            fact_text,
        )
        .await
        .unwrap();
        assert!(q_id > 0);

        let outcome = consolidate_personal_memory(ConsolidationRequest {
            conn: &conn,
            llm_provider: &provider,
            comments: None,
            project_id: None,
            memory_settings: &PersonalMemorySettings::default(),
            llm_settings: None,
            forced: false,
        })
        .await
        .expect("gating check must not error");
        match outcome {
            ConsolidateOutcome::ConfirmationRequired {
                reason: ConfirmationReason::PendingQueueItems,
                pending_count,
            } => assert_eq!(
                pending_count, 1,
                "confirmation must report the unfinished queue depth"
            ),
            other => panic!(
                "Pending queue items must yield ConfirmationRequired(PendingQueueItems), got {:?}",
                std::mem::discriminant(&other)
            ),
        }

        // Clear queue item by marking it completed
        conn.execute(
            "UPDATE memory_ingestion_queue SET status = 'completed' WHERE id = ?",
            (q_id,),
        )
        .await
        .unwrap();

        // --- Gate Arm 3: Quiescent & No Active Facts -> clean no-op Ok ---
        let quiescent_res =
            consolidate_personal_memory(ConsolidationRequest {
            conn: &conn,
            llm_provider: &provider,
            comments: None,
            project_id: None,
            memory_settings: &PersonalMemorySettings::default(),
            llm_settings: None,
            forced: true,
        }).await;
        assert!(
            quiescent_res.is_ok(),
            "Consolidation must succeed (no-op) when pipeline is quiescent and no active facts exist"
        );
    })
    .await
    .expect("test_consolidation_confirmation_gating timed out");
}

// ============================================================================
// Subtest 3: Session Continuation Data Assembly & Slicing Past Watermark
// ============================================================================
/// Entry Seam D: `fetch_session_continuation(conn, session_id)`
///
/// Seeds 10 sequential turns.
/// Seeds completed compaction covering turns 1..5.
/// Asserts:
///   - `turns` strictly contains uncompacted turns (turns 6..10).
///   - Compacted turns (turns 1..5) are excluded (negative assertion).
///   - `latest_summary` contains context_summary from the completed compaction.
///   - `personal_memory` is hydrated with the personal memory document.
#[tokio::test]
async fn test_session_continuation_data_assembly() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let session_id = 14501i64;

        // 1. Seed 10 sequential turns
        seed_turns(&conn, session_id, 10).await;

        // 2. Set personal memory content using inline facts
        let fact_1 = "User prefers Rust for systems programming.";
        let fact_2 = "User lives in Chicago.";
        let personal_doc = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{fact_1}"}},{{"id":"blk_t2","text":"{fact_2}"}}]}}]}}"#
        );
        save_personal_memory(&conn, None, &personal_doc, 1)
            .await
            .unwrap();

        // 3. Record completed compaction covering turns 1..5
        let run_id = record_compaction_start(&conn, session_id, "manual", 1, 5)
            .await
            .unwrap();
        let expected_summary =
            "User discussed software architecture and async pipeline invariants.";
        let compaction_json = serde_json::json!({
            "context_summary": expected_summary,
            "personal": [fact_1],
            "objective": [],
            "workdone": [],
            "blocker": [],
            "next_step": [],
            "pitfall": []
        })
        .to_string();
        commit_compaction_output(
            &conn,
            run_id,
            &compaction_json,
            &[("personal".to_string(), fact_1.to_string())],
            session_id,
        )
        .await
        .unwrap();

        // 4. Fetch continuation data for the session
        let continuation = fetch_session_continuation(&conn, session_id).await.unwrap();

        // Verify Personal Memory hydration
        let expected_markdown = vox_lib::services::memory::personal::PersonalMemory::from_json(&personal_doc)
            .unwrap()
            .render_to_markdown();
        assert_eq!(
            continuation.personal_memory_markdown.as_deref(),
            Some(expected_markdown.as_str()),
            "Continuation must contain personal memory document rendered as markdown"
        );

        // Verify Compaction Context Summary
        assert_eq!(
            continuation.latest_summary.as_deref(),
            Some(expected_summary),
            "Continuation must extract context_summary from latest completed compaction"
        );

        // Verify Slicing: Only turns 6..10 (strictly uncompacted turns past watermark)
        assert_eq!(
            continuation.turns.len(),
            5,
            "Continuation turns must strictly contain uncompacted turns (turns 6..10), got {}",
            continuation.turns.len()
        );
        let turn_ids: Vec<u32> = continuation.turns.iter().map(|t| t.turn_id).collect();
        assert_eq!(
            turn_ids,
            vec![6, 7, 8, 9, 10],
            "Continuation turns must be exactly [6, 7, 8, 9, 10], got {:?}",
            turn_ids
        );

        // Negative assertion: Compacted turns (1..5) must NOT appear
        for t in &continuation.turns {
            assert!(
                t.turn_id >= 6,
                "Compacted turn {} must not appear in continuation payload",
                t.turn_id
            );
        }
        assert!(
            !continuation.turns.iter().any(|t| t.turn_id == 5),
            "Turn 5 (already compacted) must NOT appear in continuation turns"
        );
    })
    .await
    .expect("test_session_continuation_data_assembly timed out");
}

// ============================================================================
// Subtest 6: Personal Memory Version History & Compaction Preemption
// ============================================================================
#[tokio::test]
async fn test_personal_memory_versions() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");
        vox_lib::persistence::schema::run_migrations(&conn)
            .await
            .unwrap();

        // 1. Initial version is created (v1)
        let v1 = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(v1.version, 1);
        assert_eq!(v1.is_active, 1);

        // 2. Save v2 and v3
        let v2 = save_personal_memory(&conn, None, r#"{"sections":[{"id":"sec_t1","title":"V2","blocks":[{"id":"blk_t1","text":"User likes coffee."}]}]}"#, 1)
            .await
            .unwrap();
        assert_eq!(v2.version, 2);
        assert_eq!(v2.is_active, 1);

        let v3 = save_personal_memory(&conn, None, r#"{"sections":[{"id":"sec_t1","title":"V3","blocks":[{"id":"blk_t1","text":"User likes tea."}]}]}"#, 2)
            .await
            .unwrap();
        assert_eq!(v3.version, 3);
        assert_eq!(v3.is_active, 1);

        // 3. List versions
        let versions = list_personal_memory_versions(&conn, None).await.unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version, 3);
        assert_eq!(versions[0].is_active, 1);
        assert_eq!(versions[1].version, 2);
        assert_eq!(versions[1].is_active, 0);
        assert_eq!(versions[2].version, 1);
        assert_eq!(versions[2].is_active, 0);

        // 4. Switch active version back to v2
        let restored = set_active_personal_memory_version(&conn, None, 2)
            .await
            .unwrap();
        assert_eq!(restored.version, 2);
        assert_eq!(restored.is_active, 1);

        // Verify active memory is now v2
        let active = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(active.version, 2);
        assert!(active.markdown.contains("User likes coffee"));
    })
    .await
    .expect("test_personal_memory_versions timed out");
}

// ============================================================================
// Revision Lifecycle — memory-spec.md §5.3 INVARIANT 5.3-A
// ---------------------------------------------------------------------------
// These tests cover the transactional and domain service surface of the semantic
// revision model (`personal_memory_revisions`).
//
// Entry seams:
//   services::memory::personal::stage_revisions
//   persistence::personal_memory::fetch_pending_revisions
//   services::memory::personal::batch_resolve_memory_revisions
//   persistence::facts::mark_observations_integrated
//   services::memory::personal::save_personal_memory_from_markdown
//   services::memory::personal::consolidate_personal_memory
// ============================================================================

/// Seeds one active fact plus its 384-dim vector row so status transitions are observable.
async fn seed_staged_candidate(
    conn: &turso::Connection,
    session_id: i64,
    compaction_id: i64,
    observation_id: &str,
    observation_type: &str,
    text: &str,
) {
    insert_observation(
        conn,
        &ObservationRecord {
            id: observation_id.to_string(),
            session_id: Some(session_id),
            compaction_id,
            observation_type: observation_type.to_string(),
            text: text.to_string(),
            status: "active".to_string(),
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
        },
    )
    .await
    .expect("insert_observation must succeed");

    insert_vector(
        conn,
        observation_id,
        observation_type,
        "active",
        None,
        &vec![0.25f32; 384],
    )
    .await
    .expect("insert_vector must succeed");
}

/// Reads the live `status` of a single fact row.
async fn fact_status(conn: &turso::Connection, observation_id: &str) -> String {
    let mut rows = conn
        .query(
            "SELECT status FROM memory_facts WHERE id = ?",
            (observation_id,),
        )
        .await
        .expect("fact status query must succeed");
    let row = rows
        .next()
        .await
        .expect("query must yield a row")
        .expect("fact row must exist");
    row.get(0).expect("status column must be readable")
}

/// Reads the live `status` of a single revision row.
async fn revision_status(conn: &turso::Connection, id: &str) -> String {
    let mut rows = conn
        .query(
            "SELECT status FROM personal_memory_revisions WHERE id = ?",
            (id,),
        )
        .await
        .expect("revision status query must succeed");
    let row = rows
        .next()
        .await
        .expect("query must yield a row")
        .expect("revision row must exist");
    row.get(0).expect("status column must be readable")
}

/// Reads the live `base_memory_version` of a single revision row.
async fn revision_base_version(conn: &turso::Connection, id: &str) -> i64 {
    let mut rows = conn
        .query(
            "SELECT base_memory_version FROM personal_memory_revisions WHERE id = ?",
            (id,),
        )
        .await
        .expect("revision base version query must succeed");
    let row = rows
        .next()
        .await
        .expect("query must yield a row")
        .expect("revision row must exist");
    row.get(0)
        .expect("base_memory_version column must be readable")
}

/// Subtest 7: `stage_revisions` persists rows and preserves ordering contract.
///
/// Covers the write path plus the `fetch_pending_revisions` read-back contract:
/// `status = 'pending'`, ordering oldest-created-first, and an empty slice is a no-op.
#[tokio::test]
async fn test_revision_stage_and_fetch_pending_roundtrip() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let initial_record = get_personal_memory(&conn, None).await.unwrap();

        // Empty slice must be a clean no-op returning 0.
        let staged_zero = stage_revisions(&conn, &initial_record, vec![])
            .await
            .expect("empty revision slice must be a no-op Ok");
        assert_eq!(staged_zero, 0);

        let empty = fetch_pending_revisions(&conn, None)
            .await
            .expect("fetch must succeed");
        assert!(
            empty.is_empty(),
            "empty insert must not create revision rows"
        );

        // Stage two operations
        let op1 = ResolvedOp::CreateBlock {
            section_id: "sec_seed".to_string(),
            text: "User enjoys badminton.".to_string(),
        };
        let op2 = ResolvedOp::CreateBlock {
            section_id: "sec_seed".to_string(),
            text: "User relocated to Seattle.".to_string(),
        };

        let staged = stage_revisions(&conn, &initial_record, vec![op1, op2])
            .await
            .expect("stage_revisions must succeed");
        assert_eq!(staged, 2);

        // Only pending rows are returned, ordered oldest created_at first.
        let pending = fetch_pending_revisions(&conn, None)
            .await
            .expect("fetch_pending_revisions must succeed");
        assert_eq!(pending.len(), 2, "both revisions must be pending");
        assert_eq!(pending[0].op, "create_block");
        assert_eq!(pending[1].op, "create_block");

        // Both anchored to the memory version current at staging time.
        assert_eq!(
            revision_base_version(&conn, &pending[0].id).await,
            initial_record.version
        );
        assert_eq!(
            revision_base_version(&conn, &pending[1].id).await,
            initial_record.version
        );
    })
    .await
    .expect("test_revision_stage_and_fetch_pending_roundtrip timed out");
}

/// Subtest 8: `batch_resolve_memory_revisions` applies operations via production seam and bumps version.
#[tokio::test]
async fn test_batch_resolve_revisions_applies_operations_and_bumps_version() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        // Base document at version 2.
        let v2_doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."}]}]}"#;
        let v2_record = save_personal_memory(&conn, None, v2_doc, 1)
            .await
            .expect("v2 document must be saved");
        assert_eq!(v2_record.version, 2);

        // Stage two operations targeting sec_t1
        let op1 = ResolvedOp::CreateBlock {
            section_id: "sec_t1".to_string(),
            text: "User enjoys badminton.".to_string(),
        };
        let op2 = ResolvedOp::CreateBlock {
            section_id: "sec_t1".to_string(),
            text: "User relocated to Seattle.".to_string(),
        };

        stage_revisions(&conn, &v2_record, vec![op1, op2])
            .await
            .expect("stage_revisions must succeed");

        let pending = fetch_pending_revisions(&conn, None)
            .await
            .expect("fetch_pending_revisions must succeed");
        assert_eq!(pending.len(), 2);
        let rev1_id = pending[0].id.clone();
        let rev2_id = pending[1].id.clone();

        // 1. Accept ONLY rev1 through the production service seam
        let v3_record = batch_resolve_memory_revisions(
            &conn,
            None,
            &[RevisionDecision {
                id: rev1_id.clone(),
                action: "accept".to_string(),
            }],
        )
        .await
        .expect("batch_resolve_memory_revisions must succeed for rev1");

        assert_eq!(
            v3_record.version, 3,
            "Accepting a revision must increment document version to 3"
        );
        assert!(
            v3_record.content.contains("User enjoys badminton."),
            "Memory content must contain newly created block text"
        );
        assert_eq!(
            revision_status(&conn, &rev1_id).await,
            "accepted",
            "Accepted revision status must flip to 'accepted'"
        );
        assert_eq!(
            revision_status(&conn, &rev2_id).await,
            "pending",
            "Unresolved revision must remain 'pending'"
        );

        // 2. Accept rev2 on top of the newly committed active document
        let v4_record = batch_resolve_memory_revisions(
            &conn,
            None,
            &[RevisionDecision {
                id: rev2_id.clone(),
                action: "accept".to_string(),
            }],
        )
        .await
        .expect("batch_resolve_memory_revisions must succeed for rev2");

        assert_eq!(
            v4_record.version, 4,
            "Accepting second revision must increment version to 4"
        );
        assert!(v4_record.content.contains("User enjoys badminton."));
        assert!(v4_record.content.contains("User relocated to Seattle."));
        assert_eq!(
            revision_status(&conn, &rev2_id).await,
            "accepted",
            "Second revision status must flip to 'accepted'"
        );

        let final_pending = fetch_pending_revisions(&conn, None)
            .await
            .expect("fetch must succeed");
        assert!(final_pending.is_empty(), "Zero pending revisions should remain");
    })
    .await
    .expect("test_batch_resolve_revisions_applies_operations_and_bumps_version timed out");
}

/// Subtest 9: Rejecting a revision marks it `'rejected'` and leaves the document intact.
#[tokio::test]
async fn test_batch_resolve_revisions_rejection() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."}]}]}"#;
        let record = save_personal_memory(&conn, None, doc, 1)
            .await
            .expect("document must be saved");

        let op = ResolvedOp::CreateBlock {
            section_id: "sec_t1".to_string(),
            text: "User dislikes morning meetings.".to_string(),
        };
        stage_revisions(&conn, &record, vec![op])
            .await
            .expect("revision must be staged");

        let pending = fetch_pending_revisions(&conn, None).await.unwrap();
        assert_eq!(pending.len(), 1);
        let rev_id = pending[0].id.clone();

        let updated = batch_resolve_memory_revisions(
            &conn,
            None,
            &[RevisionDecision {
                id: rev_id.clone(),
                action: "reject".to_string(),
            }],
        )
        .await
        .expect("rejecting must succeed");

        assert_eq!(
            updated.version, 2,
            "reject must NOT bump the document version"
        );
        assert_eq!(
            updated.content, doc,
            "reject must leave the active document byte-identical"
        );
        assert_eq!(
            revision_status(&conn, &rev_id).await,
            "rejected",
            "resolved revision must flip to 'rejected'"
        );
    })
    .await
    .expect("test_batch_resolve_revisions_rejection timed out");
}

/// Subtest 10: Invariant 5.3-A — all candidate facts transition directly to `'integrated'`.
#[tokio::test]
async fn test_candidate_partition_leaves_no_fact_trapped_in_staged() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let session_id = create_session_with_id(&conn, 14403, Some("default"))
            .await
            .expect("session must be created");
        let compaction_id = record_compaction_start(&conn, session_id, "manual", 1, 3)
            .await
            .expect("compaction run must be recorded");

        let observation_ids = ["fact_alpha", "fact_beta", "fact_gamma"];
        for id in observation_ids {
            seed_staged_candidate(
                &conn,
                session_id,
                compaction_id,
                id,
                "personal",
                "Fact text",
            )
            .await;
        }

        // All candidate facts are transitioned to 'integrated' immediately on staging/synthesis
        let facts_to_consolidate = observation_ids
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        mark_observations_integrated(&conn, &facts_to_consolidate)
            .await
            .expect("facts must be marked integrated");

        for id in observation_ids {
            assert_eq!(
                fact_status(&conn, id).await,
                "integrated",
                "fact '{}' must be 'integrated', never left 'active'",
                id
            );
        }

        // Verify zero facts remain in 'active' for this personal scope
        let active = fetch_active_observations_by_type(&conn, "personal")
            .await
            .expect("active query must succeed");
        assert!(
            active.is_empty(),
            "zero candidate facts must remain 'active'"
        );
    })
    .await
    .expect("test_candidate_partition_leaves_no_fact_trapped_in_staged timed out");
}

/// Subtest 11: Manual Markdown save re-mints persistent IDs and bulk-rejects superseded pending revisions.
#[tokio::test]
async fn test_manual_markdown_save_bulk_rejects_superseded_revisions() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        // 1. Initial memory at v2
        let initial_doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."}]}]}"#;
        let v2_record = save_personal_memory(&conn, None, initial_doc, 1)
            .await
            .expect("document must be saved");

        // 2. Stage a pending revision targeting blk_t1
        let op = ResolvedOp::UpdateBlock {
            block_id: "blk_t1".to_string(),
            text: "User moved to Seattle.".to_string(),
        };
        stage_revisions(&conn, &v2_record, vec![op]).await.unwrap();

        let pending_before = fetch_pending_revisions(&conn, None).await.unwrap();
        assert_eq!(pending_before.len(), 1);
        let stale_rev_id = pending_before[0].id.clone();

        // 3. User saves manual markdown, re-minting all entity IDs
        let md = "## Personal Profile\nUser relocated to Denver.\n";
        let saved = save_personal_memory_from_markdown(&conn, None, md, 2)
            .await
            .expect("manual markdown save must succeed");

        assert_eq!(saved.version, 3);
        assert!(saved.markdown.contains("Personal Profile"));
        assert!(saved.markdown.contains("User relocated to Denver."));

        // 4. Invariant: the pending revision must be bulk-rejected
        let pending_after = fetch_pending_revisions(&conn, None).await.unwrap();
        assert!(
            pending_after.is_empty(),
            "Zero revisions must remain pending after manual save"
        );

        assert_eq!(
            revision_status(&conn, &stale_rev_id).await,
            "rejected",
            "Superseded revision must transition to 'rejected'"
        );
    })
    .await
    .expect("test_manual_markdown_save_bulk_rejects_superseded_revisions timed out");
}
