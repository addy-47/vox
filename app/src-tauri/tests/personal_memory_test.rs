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
    persistence::{
        compactions::{commit_compaction_output, record_compaction_start},
        facts::{fetch_active_facts_by_type, insert_fact, FactRecord},
        has_in_progress_compaction,
        personal_memory::{get_personal_memory, save_personal_memory},
        queue::enqueue_fact,
        sessions::{create_session_with_id, fetch_session_continuation},
    },
    services::{
        llm::{ConnectionConfig, RemoteTransport},
        memory::personal::consolidate_personal_memory,
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
    fact_type: String,
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
        .filter(|f| f.fact_type == category)
        .take(limit)
        .collect()
}

/// Loads up to `limit` conversation turns from `sandbox/datasets/100-turns/dataset_session-2.json`.
fn load_dataset_turns(limit: usize) -> Vec<DatasetTurn> {
    let mut turns: Vec<DatasetTurn> =
        common::paths::load_json_dataset("100-turns/dataset_session-2.json");
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
            let fact_rec = FactRecord {
                id: f.id.clone(),
                session_id: Some(session_id),
                compaction_id,
                fact_type: "personal".to_string(),
                text: f.text.clone(),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            };
            insert_fact(&conn, &fact_rec).await.unwrap();
        }

        // Finish the initial compaction so quiescence check succeeds
        conn.execute(
            "UPDATE session_compactions SET status = 'completed', finished_at = ? WHERE id = ?",
            (now, compaction_id),
        )
        .await
        .unwrap();

        let active_before = fetch_active_facts_by_type(&conn, "personal")
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
        let consolidated = consolidate_personal_memory(&conn, &provider, None, None, None)
            .await
            .expect("consolidate_personal_memory must succeed against remote Ollama server");

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
        let active_after = fetch_active_facts_by_type(&conn, "personal")
            .await
            .unwrap();
        assert_eq!(
            active_after.len(),
            0,
            "Zero active personal facts must remain after consolidation"
        );

        let mut count_rows = conn
            .query(
                "SELECT COUNT(*) FROM memory_facts WHERE status = 'consolidated' AND type = 'personal'",
                (),
            )
            .await
            .unwrap();
        let consolidated_count: i64 = count_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            consolidated_count, 100,
            "All 100 facts must be marked status='consolidated'"
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
        let doc_v2 = format!("# Profile\n- {}\n", dataset_facts[0].text);
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
            "# Profile\n- {}\n- {}\n",
            dataset_facts[0].text, dataset_facts[1].text
        );
        let updated_v3 = save_personal_memory(&conn, None, &doc_v3, 2).await.unwrap();
        assert_eq!(updated_v3.version, 3);
        assert_eq!(updated_v3.content, doc_v3);
    })
    .await
    .expect("test_personal_memory_optimistic_concurrency timed out");
}

// ============================================================================
// Subtest 3: Consolidation Quiescence Precondition Gating
// ============================================================================
/// Entry Seam B: `consolidate_personal_memory(conn, provider, comments, project_id)`
///
/// Verifies the two-arm quiescence gate:
///   - Precondition failure if compaction is actively `in_progress`.
///   - Precondition failure if ingestion queue has unfinished items.
///   - Precondition passes and returns current record (no-op) when quiescent and facts are empty.
#[tokio::test]
async fn test_consolidation_quiescence_precondition_gating() {
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

        let compaction_blocked =
            consolidate_personal_memory(&conn, &provider, None, None, None).await;
        assert!(
            compaction_blocked.is_err(),
            "Consolidation must be blocked when compaction is in progress"
        );
        let err_msg = compaction_blocked.err().unwrap().to_string();
        assert!(
            err_msg.contains("active compaction is in progress"),
            "Error must mention active compaction, got: {}",
            err_msg
        );

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

        let q_id = enqueue_fact(
            &conn,
            Some(session_id),
            run_id,
            "personal",
            fact_text,
        )
        .await
        .unwrap();
        assert!(q_id > 0);

        let queue_blocked = consolidate_personal_memory(&conn, &provider, None, None, None).await;
        assert!(
            queue_blocked.is_err(),
            "Consolidation must be blocked when items are pending in ingestion queue"
        );
        let err_msg2 = queue_blocked.err().unwrap().to_string();
        assert!(
            err_msg2.contains("pending items in memory ingestion queue"),
            "Error must mention pending items, got: {}",
            err_msg2
        );

        // Clear queue item by marking it completed
        conn.execute(
            "UPDATE memory_ingestion_queue SET status = 'completed' WHERE id = ?",
            (q_id,),
        )
        .await
        .unwrap();

        // --- Gate Arm 3: Quiescent & No Active Facts -> clean no-op Ok ---
        let quiescent_res = consolidate_personal_memory(&conn, &provider, None, None, None).await;
        assert!(
            quiescent_res.is_ok(),
            "Consolidation must succeed (no-op) when pipeline is quiescent and no active facts exist"
        );
    })
    .await
    .expect("test_consolidation_quiescence_precondition_gating timed out");
}

// ============================================================================
// Subtest 4: Document Direct Edit Persistence
// ============================================================================
/// Entry Seams C: `save_personal_memory` / `get_personal_memory`
///
/// Verifies:
///   - Direct manual edits save cleanly and increment version.
///   - Database reflects updated content accurately.
#[tokio::test]
async fn test_manual_edit_persistence() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let dataset_facts = load_dataset_facts("personal", 4);
        assert!(
            dataset_facts.len() >= 4,
            "Need at least 4 dataset facts for manual edit test"
        );

        let initial_doc = format!(
            "# Personal Profile\n\n- {}\n- {}\n",
            dataset_facts[0].text, dataset_facts[1].text
        );

        // 1. Save initial document to DB (version 1 -> 2)
        let saved = save_personal_memory(&conn, None, &initial_doc, 1)
            .await
            .unwrap();
        assert_eq!(saved.version, 2);
        assert_eq!(saved.content, initial_doc);

        // 2. Overwrite document directly (simulating user pasting/importing external content)
        let updated_doc = format!(
            "# Personal Profile Updated\n\n- {}\n- {}\n- {}\n",
            dataset_facts[0].text, dataset_facts[2].text, dataset_facts[3].text
        );
        let updated = save_personal_memory(&conn, None, &updated_doc, 2)
            .await
            .unwrap();
        assert_eq!(
            updated.version, 3,
            "Manual edit must increment personal memory version"
        );
        assert_eq!(updated.content, updated_doc);

        // 3. Verify database reflects the updated content
        let current = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(current.version, 3);
        assert_eq!(current.content, updated_doc);
    })
    .await
    .expect("test_manual_edit_persistence timed out");
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
            "# Personal Profile\n\n- {}\n- {}\n",
            personal_facts[0].text, personal_facts[1].text
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
            continuation.personal_memory.as_deref(),
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
