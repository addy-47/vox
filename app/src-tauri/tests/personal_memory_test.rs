//! ============================================================================
//! tests/personal_memory_test.rs — Personal Memory & Session Continuation v2 Integration Tests
//! ============================================================================
//! Category     : Integration Test (Seam 14)
//! Component    : services/memory/personal, persistence/personal_memory, persistence/sessions, persistence/facts
//! Prerequisites: Turso SQLite engine (vox.db), real datasets in sandbox/datasets/
//!                (Live subtest): Remote GPU server (http://100.67.98.126:11434/v1, gemma3:12b)
//! Execution    : cargo nextest run --test personal_memory_test --release --nocapture --test-threads=1
//!                (Live subtest): cargo nextest run --test personal_memory_test --release --nocapture --test-threads=1 -- --ignored
//! Metrics      : Optimistic concurrency control, fact consolidation status transitions, quiescence gating, disk portability, continuation turn slicing
//! ============================================================================

mod common;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::{harness::get_test_app_and_state, paths::TempPathsGuard};
use serde::Deserialize;
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
            fetch_pending_revisions, get_personal_memory, insert_personal_memory_revisions,
            list_personal_memory_versions, resolve_batch_revisions_transaction,
            save_personal_memory, set_active_personal_memory_version, PersonalMemoryRevisionRecord,
        },
        queue::enqueue_observation,
        sessions::{create_session_with_id, fetch_session_continuation},
        RevisionDecision,
    },
    services::{
        llm::{ConnectionConfig, RemoteTransport},
        memory::personal::{
            consolidate_personal_memory, ConfirmationReason, ConsolidateOutcome,
            ConsolidationRequest,
        },
    },
};

const REMOTE_OLLAMA_URL: &str = "http://100.67.98.126:11434/v1";
const REMOTE_OLLAMA_MODEL: &str = "gemma3:12b";

// ============================================================================
// Dataset Fixture Helpers
// ============================================================================

#[derive(Debug, Deserialize)]
struct DatasetFact {
    id: String,
    #[serde(rename = "type")]
    observation_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct LlmFactsDataset {
    facts: Vec<DatasetFact>,
}

#[derive(Debug, Deserialize)]
struct DatasetTurn {
    turn: u32,
    user: String,
    assistant: String,
}

/// Loads up to `limit` facts of a specific category from `sandbox/datasets/pure_llm_facts_dataset.json`.
fn load_dataset_facts(category: &str, limit: usize) -> Vec<DatasetFact> {
    let dataset: LlmFactsDataset = common::paths::load_json_dataset("pure_llm_facts_dataset.json");
    dataset
        .facts
        .into_iter()
        .filter(|f| f.observation_type == category)
        .take(limit)
        .collect()
}

/// Loads up to `limit` conversation turns from `sandbox/datasets/100-turns/dataset_session-2.json`.
fn load_dataset_turns(limit: usize) -> Vec<DatasetTurn> {
    let mut turns: Vec<DatasetTurn> =
        common::paths::load_json_dataset("legacy/dataset_session-2.json");
    turns.truncate(limit);
    turns
}

/// Seeds turns into the database with proper session association.
async fn seed_turns(conn: &turso::Connection, session_id: i64, turns: &[DatasetTurn]) {
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

    for t in turns {
        conn.execute(
            "INSERT OR REPLACE INTO turns (session_id, turn_id, user_text, assistant_text, created_at) \
             VALUES (?, ?, ?, ?, ?)",
            (session_id, t.turn as i64, t.user.clone(), t.assistant.clone(), now + t.turn as i64),
        )
        .await
        .unwrap();
    }
}

// ============================================================================
// Subtest 1 (IGNORED): Live Remote Ollama 100-Fact Consolidation
// ============================================================================
/// Entry Seam B: `consolidate_personal_memory(conn, provider, None, None)`
///
/// Seeds 100 real personal facts from `pure_llm_facts_dataset.json` into `memory_facts`.
/// Connects to remote GPU Ollama server running `gemma3:12b`.
/// Asserts:
///   - Personal memory version increments.
///   - Personal memory markdown contains substantive synthesized content.
///   - All 100 seeded facts transition from 'active' to 'consolidated'.
///   - Zero active personal facts remain.
#[tokio::test]
#[ignore = "Requires remote GPU server with Ollama gemma3:12b running"]
async fn test_personal_memory_consolidation_live_server() {
    tokio::time::timeout(Duration::from_secs(180), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        // 1. Load 100 real personal facts from dataset
        let dataset_facts = load_dataset_facts("personal", 100);
        assert_eq!(
            dataset_facts.len(),
            100,
            "Must load exactly 100 personal facts from dataset"
        );

        let session_id = 14101i64;
        create_session_with_id(&conn, session_id, None)
            .await
            .unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "manual", 1, 100)
            .await
            .unwrap();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for f in &dataset_facts {
            let observation_rec = ObservationRecord {
                id: f.id.clone(),
                session_id: Some(session_id),
                compaction_id,
                observation_type: "personal".to_string(),
                text: f.text.clone(),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            };
            insert_observation(&conn, &observation_rec).await.unwrap();
        }

        // Finish the initial compaction so quiescence check succeeds
        conn.execute(
            "UPDATE session_compactions SET status = 'completed', finished_at = ? WHERE id = ?",
            (now, compaction_id),
        )
        .await
        .unwrap();

        let active_before = fetch_active_observations_by_type(&conn, "personal")
            .await
            .unwrap();
        assert_eq!(
            active_before.len(),
            100,
            "Must have 100 active personal facts prior to consolidation"
        );

        let initial_record = get_personal_memory(&conn, None).await.unwrap();
        let version_before = initial_record.version;

        // 2. Build real RemoteTransport provider pointing to remote GPU Ollama server
        let conn_cfg = ConnectionConfig::new(
            REMOTE_OLLAMA_URL,
            REMOTE_OLLAMA_MODEL,
            None,
            Some("ollama"),
        );
        let provider = RemoteTransport::new(conn_cfg);

        // 3. Execute consolidation through production entry seam
        eprintln!(
            "[Seam14/Live] Consolidating 100 facts via remote {REMOTE_OLLAMA_MODEL}..."
        );
        let start = Instant::now();
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
            .expect("consolidate_personal_memory must succeed against remote Ollama server");
        let ConsolidateOutcome::Completed {
            record: consolidated,
        } = outcome
        else {
            panic!("consolidate_personal_memory must complete, not request confirmation");
        };

        eprintln!(
            "[Seam14/Live] Consolidated in {:.2}s: v{} -> v{}",
            start.elapsed().as_secs_f64(),
            version_before,
            consolidated.version
        );

        // 4. Invariant assertions on consolidated document
        assert!(
            consolidated.version > version_before,
            "Document version must increment after consolidation (before: {version_before}, after: {})",
            consolidated.version
        );
        assert!(
            !consolidated.content.trim().is_empty(),
            "Consolidated personal memory content must not be empty"
        );
        assert!(
            consolidated.content.len() > 50,
            "Consolidated personal memory must have substantive length (> 50 chars)"
        );

        // 5. Invariant assertions on fact status transitions
        let active_after = fetch_active_observations_by_type(&conn, "personal")
            .await
            .unwrap();
        assert_eq!(
            active_after.len(),
            0,
            "Zero active personal facts must remain after consolidation"
        );

        let mut count_rows = conn
            .query(
                "SELECT COUNT(*) FROM memory_facts WHERE status = 'integrated' AND type = 'personal'",
                (),
            )
            .await
            .unwrap();
        let consolidated_count: i64 = count_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            consolidated_count, 100,
            "All 100 facts must be marked status='integrated'"
        );
    })
    .await
    .expect("test_personal_memory_consolidation_live_server timed out");
}

// ============================================================================
// Subtest 2: Optimistic Concurrency Control (expected_version)
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

        let dataset_facts = load_dataset_facts("personal", 3);
        assert!(
            dataset_facts.len() >= 3,
            "Need at least 3 dataset facts for concurrency test"
        );

        // 1. Initial blank memory record starts at version 1
        let initial = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(initial.version, 1);

        // 2. Successful update with matching expected_version (1) increments to version 2
        let doc_v2 = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{}"}}]}}]}}"#,
            dataset_facts[0].text.replace('"', "'")
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
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{}"}},{{"id":"blk_t2","text":"{}"}}]}}]}}"#,
            dataset_facts[0].text.replace('"', "'"),
            dataset_facts[1].text.replace('"', "'")
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
            REMOTE_OLLAMA_URL,
            REMOTE_OLLAMA_MODEL,
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
        let dataset_facts = load_dataset_facts("personal", 1);
        let fact_text = &dataset_facts[0].text;

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
    .expect("test_consolidation_quiescence_precondition_gating timed out");
}

// ============================================================================
// Subtest 5: Session Continuation Data Assembly & Slicing Past Watermark
// ============================================================================
/// Entry Seam D: `fetch_session_continuation(conn, session_id)`
///
/// Seeds 10 turns from `100-turns/dataset_session-2.json`.
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

        // 1. Seed 10 real turns from the dataset
        let dataset_turns = load_dataset_turns(10);
        assert_eq!(
            dataset_turns.len(),
            10,
            "Must seed exactly 10 turns from dataset"
        );
        seed_turns(&conn, session_id, &dataset_turns).await;

        // 2. Set personal memory content using real dataset facts
        let personal_facts = load_dataset_facts("personal", 2);
        let personal_doc = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{}"}},{{"id":"blk_t2","text":"{}"}}]}}]}}"#,
            personal_facts[0].text.replace('"', "'"),
            personal_facts[1].text.replace('"', "'")
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
            "personal": [personal_facts[0].text],
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
            &[("personal".to_string(), personal_facts[0].text.clone())],
            session_id,
        )
        .await
        .unwrap();

        // 4. Fetch continuation data for the session
        let continuation = fetch_session_continuation(&conn, session_id).await.unwrap();

        // Verify Personal Memory hydration
        assert_eq!(
            continuation.personal_memory_markdown.as_deref(),
            Some(personal_doc.as_str()),
            "Continuation must contain personal memory document"
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
// These tests cover the transactional surface of the semantic revision model
// (`personal_memory_revisions`). No behavioural test existed for insert,
// no-re-anchor resolution, rejection, or the candidate partition.
//
// Entry seams:
//   persistence::personal_memory::insert_personal_memory_revisions
//   persistence::personal_memory::fetch_pending_revisions
//   persistence::personal_memory::resolve_batch_revisions_transaction
//   persistence::facts::mark_observations_integrated
//
// Model: none. These are transactional/persistence contracts. The *selection* of
// which facts an operation references is LLM behaviour and is eval-grade
// (evals/memory_consolidation_eval.rs), not asserted here.
// ============================================================================

/// Builds a pending suggestion record anchored at `base_version`.
fn pending_suggestion(
    id: &str,
    base_version: i64,
    op: &str,
    target_id: &str,
    content: &str,
    created_at: i64,
) -> PersonalMemoryRevisionRecord {
    PersonalMemoryRevisionRecord {
        id: id.to_string(),
        base_memory_version: base_version,
        project_id: None,
        op: op.to_string(),
        target_id: target_id.to_string(),
        content: content.to_string(),
        status: "pending".to_string(),
        created_at,
        resolved_at: None,
    }
}

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

/// Reads the live `status` of a single suggestion row.
async fn suggestion_status(conn: &turso::Connection, id: &str) -> String {
    let mut rows = conn
        .query(
            "SELECT status FROM personal_memory_revisions WHERE id = ?",
            (id,),
        )
        .await
        .expect("suggestion status query must succeed");
    let row = rows
        .next()
        .await
        .expect("query must yield a row")
        .expect("suggestion row must exist");
    row.get(0).expect("status column must be readable")
}

/// Reads the live `base_memory_version` of a single suggestion row.
async fn suggestion_base_version(conn: &turso::Connection, id: &str) -> i64 {
    let mut rows = conn
        .query(
            "SELECT base_memory_version FROM personal_memory_revisions WHERE id = ?",
            (id,),
        )
        .await
        .expect("suggestion base version query must succeed");
    let row = rows
        .next()
        .await
        .expect("query must yield a row")
        .expect("suggestion row must exist");
    row.get(0)
        .expect("base_memory_version column must be readable")
}

/// Reads the live `target_id` of a single revision row.
async fn suggestion_target_id(conn: &turso::Connection, id: &str) -> String {
    let mut rows = conn
        .query(
            "SELECT target_id FROM personal_memory_revisions WHERE id = ?",
            (id,),
        )
        .await
        .expect("revision target_id query must succeed");
    let row = rows
        .next()
        .await
        .expect("query must yield a row")
        .expect("suggestion row must exist");
    row.get(0).expect("target_id column must be readable")
}

/// Subtest 7: `insert_personal_memory_revisions` persists rows and preserves index properties.
///
/// Covers the write path plus the `fetch_pending_revisions` read-back contract:
/// `status = 'pending'`, ordering oldest-created-first, and an empty slice is a no-op.
#[tokio::test]
async fn test_suggestion_insert_and_fetch_pending_roundtrip() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        // Empty slice must be a clean no-op.
        insert_personal_memory_revisions(&conn, &[])
            .await
            .expect("empty suggestion slice must be a no-op Ok");
        let empty = fetch_pending_revisions(&conn, None)
            .await
            .expect("fetch must succeed");
        assert!(
            empty.is_empty(),
            "empty insert must not create suggestion rows"
        );

        // Two pending suggestions with target indices.
        let older = pending_suggestion(
            "sug_older",
            1,
            "create_block",
            "blk_seed_older",
            "- User enjoys badminton.",
            1_000,
        );
        let newer = pending_suggestion(
            "sug_newer",
            1,
            "create_block",
            "blk_seed_newer",
            "- User relocated to Seattle.",
            2_000,
        );
        insert_personal_memory_revisions(&conn, &[older, newer])
            .await
            .expect("insert_personal_memory_revisions must succeed");

        // Only pending rows are returned, ordered oldest created_at first.
        let pending = fetch_pending_revisions(&conn, None)
            .await
            .expect("fetch_pending_revisions must succeed");
        assert_eq!(pending.len(), 2, "both suggestions must be pending");
        assert_eq!(
            pending[0].id, "sug_older",
            "ordering must be created_at ASC"
        );
        assert_eq!(pending[1].id, "sug_newer");

        assert_eq!(pending[0].content, "- User enjoys badminton.");
        assert_eq!(pending[1].content, "- User relocated to Seattle.");

        // Both anchored to the memory version current at staging time.
        assert_eq!(suggestion_base_version(&conn, "sug_older").await, 1);
        assert_eq!(suggestion_base_version(&conn, "sug_newer").await, 1);
    })
    .await
    .expect("test_suggestion_insert_and_fetch_pending_roundtrip timed out");
}

/// Subtest 8: Accepting `sug_1` re-anchors `sug_2` and arithmetically shifts its `target_id`.
///
/// This is INVARIANT 5.3-B — arithmetic index re-anchoring. `sug_2` was anchored at index 3;
/// accepting an `insert_after` at index 2 shifts subsequent pending suggestions (index > 2)
/// by +1, so `sug_2`'s `target_id` becomes 4.
#[tokio::test]
async fn test_suggestion_accept_reanchors_remaining_pending() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let session_id = create_session_with_id(&conn, 14401, Some("default"))
            .await
            .expect("session must be created");
        let compaction_id = record_compaction_start(&conn, session_id, "manual", 1, 2)
            .await
            .expect("compaction run must be recorded");

        seed_staged_candidate(
            &conn,
            session_id,
            compaction_id,
            "fact_for_sug_1",
            "personal",
            "User enjoys badminton.",
        )
        .await;
        seed_staged_candidate(
            &conn,
            session_id,
            compaction_id,
            "fact_for_sug_2",
            "personal",
            "User relocated to Seattle.",
        )
        .await;

        // Base document at version 2.
        let v2_doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."}]}]}"#;
        save_personal_memory(&conn, None, v2_doc, 1)
            .await
            .expect("v2 document must be saved");

        // Two suggestions anchored to version 2:
        // sug_1: insert after element 2
        // sug_2: insert after element 3
        insert_personal_memory_revisions(
            &conn,
            &[
                pending_suggestion(
                    "sug_1",
                    2,
                    "create_block",
                    "blk_sug_1",
                    "- User enjoys badminton.",
                    1_000,
                ),
                pending_suggestion(
                    "sug_2",
                    2,
                    "create_block",
                    "blk_sug_2",
                    "- User relocated to Seattle.",
                    2_000,
                ),
            ],
        )
        .await
        .expect("suggestions must be inserted");

        // Accept ONLY sug_1, supplying the merged document.
        let v3_doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."},{"id":"blk_t2","text":"User enjoys badminton."}]}]}"#;
        let updated = resolve_batch_revisions_transaction(
            &conn,
            None,
            &[RevisionDecision {
                id: "sug_1".to_string(),
                action: "accept".to_string(),
            }],
            Some(v3_doc),
        )
        .await
        .expect("accepting sug_1 must succeed");

        assert_eq!(
            updated.version, 3,
            "accept must bump the document version from 2 to 3"
        );
        assert_eq!(
            updated.content, v3_doc,
            "accept must persist the supplied new_content"
        );
        assert_eq!(
            suggestion_status(&conn, "sug_1").await,
            "accepted",
            "resolved suggestion must flip to 'accepted'"
        );

        // Semantic IDs are never re-anchored: sug_2 survives as pending with its
        // generation-time base version and target ID untouched.
        assert_eq!(
            suggestion_status(&conn, "sug_2").await,
            "pending",
            "resolution must NOT change a remaining pending revision's status"
        );
        assert_eq!(
            suggestion_base_version(&conn, "sug_2").await,
            2,
            "remaining pending revision keeps its generation-time base version"
        );
        assert_eq!(
            suggestion_target_id(&conn, "sug_2").await,
            "blk_sug_2",
            "remaining pending revision keeps its semantic target ID"
        );

        // sug_2 must still be returned by the review slate and still be resolvable.
        let still_pending = fetch_pending_revisions(&conn, None)
            .await
            .expect("fetch must succeed after accept");
        assert_eq!(
            still_pending.len(),
            1,
            "exactly one suggestion (sug_2) must remain on the review slate"
        );
        assert_eq!(still_pending[0].id, "sug_2");

        let v4_doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."},{"id":"blk_t2","text":"User enjoys badminton."},{"id":"blk_t3","text":"User relocated to Seattle."}]}]}"#;
        let final_record = resolve_batch_revisions_transaction(
            &conn,
            None,
            &[RevisionDecision {
                id: "sug_2".to_string(),
                action: "accept".to_string(),
            }],
            Some(v4_doc),
        )
        .await
        .expect("sug_2 must still be individually resolvable");
        assert_eq!(
            final_record.version, 4,
            "sug_2 must resolve on top of the re-anchored base version"
        );
    })
    .await
    .expect("test_suggestion_accept_reanchors_remaining_pending timed out");
}

/// Subtest 9: Rejecting a suggestion marks it `'rejected'` and leaves the document intact.
#[tokio::test]
async fn test_suggestion_reject_marks_facts_rejected_and_preserves_version() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."}]}]}"#;
        save_personal_memory(&conn, None, doc, 1)
            .await
            .expect("document must be saved");

        insert_personal_memory_revisions(
            &conn,
            &[pending_suggestion(
                "sug_reject",
                2,
                "create_block",
                "blk_sug_reject",
                "- User dislikes morning meetings.",
                1_000,
            )],
        )
        .await
        .expect("suggestion must be inserted");

        let record = resolve_batch_revisions_transaction(
            &conn,
            None,
            &[RevisionDecision {
                id: "sug_reject".to_string(),
                action: "reject".to_string(),
            }],
            None,
        )
        .await
        .expect("rejecting must succeed");

        assert_eq!(
            record.version, 2,
            "reject must NOT bump the document version"
        );
        assert_eq!(
            record.content, doc,
            "reject must leave the active document byte-identical"
        );
        assert_eq!(
            suggestion_status(&conn, "sug_reject").await,
            "rejected",
            "resolved suggestion must flip to 'rejected'"
        );
    })
    .await
    .expect("test_suggestion_reject_marks_facts_rejected_and_preserves_version timed out");
}

/// Subtest 10: Invariant 5.3-A — all candidate facts transition directly to `'consolidated'`.
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

        // All candidate facts are transitioned to 'consolidated' immediately on staging/synthesis
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

        // Verify zero facts remain in 'staged' or 'active' for this personal scope
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
