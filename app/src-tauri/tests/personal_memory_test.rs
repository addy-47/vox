//! ============================================================================
//! personal_memory_test.rs — personal memory is versioned, gated against concurrent writes, hydrated into session continuations, and mutated only through reviewed revisions
//! ============================================================================
//! Category  : Integration (Seam 14 — personal memory)
//! Component : src/services/memory/personal.rs, src/persistence/personal_memory.rs, src/persistence/sessions.rs, src/persistence/facts.rs
//! Binary    : personal_memory_test
//! Prereqs   : none (isolated temporary VOX_HOME and Turso database per test)
//! Run       : cargo nextest run --test personal_memory_test --release --test-threads=1
//! ============================================================================

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{harness::get_test_app_and_state, paths::TempPathsGuard};
use vox_lib::{
    config::MemorySettings,
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

/// Builds a remote transport that is never actually used: every consolidation
/// scenario below is gated before the network is touched.
fn dummy_provider() -> RemoteTransport {
    let conn_cfg = ConnectionConfig::new(
        DUMMY_ENDPOINT_URL,
        DUMMY_ENDPOINT_MODEL,
        None,
        Some("ollama"),
    );
    RemoteTransport::new(conn_cfg)
}

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

/// PURPOSE:  A manual edit that matches the current document version must be
///           accepted and must bump the version; losing the version bump
///           would let a later stale edit overwrite the newer document.
/// SCENARIO: Save a document with `expected_version == 1` on a fresh record.
/// ASSERTS:  The returned record is version 2 and carries the saved content.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make `save_personal_memory` store the content without bumping
///           the version — the version assertion would fail.
#[tokio::test]
async fn personal_memory_save_with_matching_version_increments_version() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let fact_1 = "User lives in Chicago.";
        let initial = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(initial.version, 1);

        let doc_v2 = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{fact_1}"}}]}}]}}"#
        );
        let updated = save_personal_memory(&conn, None, &doc_v2, 1).await.unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.content, doc_v2);
    })
    .await
    .expect("personal_memory_save_with_matching_version_increments_version timed out");
}

/// PURPOSE:  A write based on a stale version must be refused; accepting it
///           would silently discard the newer document and lose the user's
///           memory edits.
/// SCENARIO: Save with `expected_version == 1` after the document is already
///           at version 2.
/// ASSERTS:  The call fails and the error message names the version conflict.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Remove the `expected_version` check in `save_personal_memory` —
///           the error assertion would fail.
#[tokio::test]
async fn personal_memory_save_with_stale_version_is_rejected() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let fact_1 = "User lives in Chicago.";
        let doc_v2 = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{fact_1}"}}]}}]}}"#
        );
        let updated = save_personal_memory(&conn, None, &doc_v2, 1).await.unwrap();
        assert_eq!(updated.version, 2);

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
    })
    .await
    .expect("personal_memory_save_with_stale_version_is_rejected timed out");
}

/// PURPOSE:  A rejected stale write must not damage the stored document; the
///           user's memory has to survive the failed edit intact.
/// SCENARIO: Attempt a stale write, then read the document back.
/// ASSERTS:  The stored version is still 2 and the content is byte-identical
///           to the accepted document.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Write the content before validating `expected_version` — the
///           content assertion would fail.
#[tokio::test]
async fn personal_memory_rejected_stale_save_preserves_stored_content() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let fact_1 = "User lives in Chicago.";
        let fact_2 = "User prefers Rust for systems programming.";
        let doc_v2 = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{fact_1}"}}]}}]}}"#
        );
        save_personal_memory(&conn, None, &doc_v2, 1).await.unwrap();

        let stale_attempt = save_personal_memory(&conn, None, "# Hijacked Content", 1).await;
        assert!(stale_attempt.is_err());

        let current = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(current.version, 2);
        assert_eq!(current.content, doc_v2);

        // The next accepted write must build on version 2.
        let doc_v3 = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{fact_1}"}},{{"id":"blk_t2","text":"{fact_2}"}}]}}]}}"#
        );
        let updated_v3 = save_personal_memory(&conn, None, &doc_v3, 2).await.unwrap();
        assert_eq!(updated_v3.version, 3);
        assert_eq!(updated_v3.content, doc_v3);
    })
    .await
    .expect("personal_memory_rejected_stale_save_preserves_stored_content timed out");
}

/// PURPOSE:  Consolidation must not run while a compaction is in progress;
///           running both at once would compact turns that the consolidation
///           is about to rewrite.
/// SCENARIO: Record an in-progress compaction and request a forced
///           consolidation.
/// ASSERTS:  The outcome is `ConfirmationRequired` with reason
///           `CompactionInProgress` and a pending count of zero.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove the `has_in_progress_compaction` gate in
///           `consolidate_personal_memory` — the outcome assertion would fail.
#[tokio::test]
async fn consolidation_is_gated_by_an_in_progress_compaction() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        let session_id = create_session_with_id(&conn, 14301, Some("default"))
            .await
            .unwrap();

        let provider = dummy_provider();

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
            memory_settings: &MemorySettings::default(),
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

        // Clearing the compaction block must unblock a later consolidation.
        commit_compaction_output(
            &conn,
            run_id,
            r#"{"context_summary": "Compaction committed"}"#,
            &[],
            session_id,
        )
        .await
        .unwrap();
    })
    .await
    .expect("consolidation_is_gated_by_an_in_progress_compaction timed out");
}

/// PURPOSE:  Consolidation must not run while ingestion queue items are still
///           unfinished; those facts have not reached the fact store yet and
///           would be lost by the rewrite.
/// SCENARIO: Enqueue one observation and request a non-forced consolidation.
/// ASSERTS:  The outcome is `ConfirmationRequired` with reason
///           `PendingQueueItems` and a pending count of exactly one.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Report `pending_count: 0` for the pending-queue gate — the count
///           assertion would fail.
#[tokio::test]
async fn consolidation_is_gated_by_pending_queue_items() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        let session_id = create_session_with_id(&conn, 14302, Some("default"))
            .await
            .unwrap();
        let provider = dummy_provider();

        let run_id = record_compaction_start(&conn, session_id, "manual", 1, 10)
            .await
            .unwrap();
        commit_compaction_output(
            &conn,
            run_id,
            r#"{"context_summary": "Compaction committed"}"#,
            &[],
            session_id,
        )
        .await
        .unwrap();

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
            memory_settings: &MemorySettings::default(),
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
    })
    .await
    .expect("consolidation_is_gated_by_pending_queue_items timed out");
}

/// PURPOSE:  A consolidation request on a quiescent pipeline with no active
///           facts must succeed as a clean no-op instead of erroring, or the
///           user cannot trigger a consolidation at all when there is nothing
///           to consolidate.
/// SCENARIO: Request a forced consolidation with no compaction running, no
///           queue items and no active facts.
/// ASSERTS:  The call returns `Ok`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Return an error when there are no active facts — the `is_ok`
///           assertion would fail.
#[tokio::test]
async fn consolidation_succeeds_when_quiescent_without_active_facts() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        let provider = dummy_provider();

        let quiescent_res = consolidate_personal_memory(ConsolidationRequest {
            conn: &conn,
            llm_provider: &provider,
            comments: None,
            project_id: None,
            memory_settings: &MemorySettings::default(),
            llm_settings: None,
            forced: true,
        })
        .await;

        assert!(
            quiescent_res.is_ok(),
            "Consolidation must succeed (no-op) when pipeline is quiescent and no active facts exist"
        );
    })
    .await
    .expect("consolidation_succeeds_when_quiescent_without_active_facts timed out");
}

/// PURPOSE:  A continuation must carry the personal memory document, or the
///           resumed session loses everything the assistant knew about the
///           user.
/// SCENARIO: Save a personal memory document, commit a compaction and fetch
///           the continuation for that session.
/// ASSERTS:  The continuation's personal memory markdown equals the document
///           rendered as markdown.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop `personal_memory_markdown` from the continuation payload —
///           the assertion would fail.
#[tokio::test]
async fn session_continuation_hydrates_personal_memory_markdown() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let session_id = 14501i64;
        seed_turns(&conn, session_id, 10).await;

        let fact_1 = "User prefers Rust for systems programming.";
        let fact_2 = "User lives in Chicago.";
        let personal_doc = format!(
            r#"{{"sections":[{{"id":"sec_t1","title":"Profile","blocks":[{{"id":"blk_t1","text":"{fact_1}"}},{{"id":"blk_t2","text":"{fact_2}"}}]}}]}}"#
        );
        save_personal_memory(&conn, None, &personal_doc, 1)
            .await
            .unwrap();

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

        let continuation = fetch_session_continuation(&conn, session_id).await.unwrap();

        let expected_markdown = vox_lib::services::memory::personal::PersonalMemory::from_json(
            &personal_doc,
        )
        .unwrap()
        .render_to_markdown();
        assert_eq!(
            continuation.personal_memory_markdown.as_deref(),
            Some(expected_markdown.as_str()),
            "Continuation must contain personal memory document rendered as markdown"
        );
    })
    .await
    .expect("session_continuation_hydrates_personal_memory_markdown timed out");
}

/// PURPOSE:  A continuation must carry only the turns that were not already
///           compacted; replaying compacted turns duplicates history and
///           wastes the context window.
/// SCENARIO: Seed ten turns, compact turns 1..5, then fetch the continuation.
/// ASSERTS:  The continuation holds exactly turns 6..10, excludes turn 5, and
///           carries the compaction's context summary.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Change the watermark query to start at turn 0 — the turn-id
///           assertion would fail.
#[tokio::test]
async fn session_continuation_excludes_compacted_turns() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let session_id = 14502i64;
        seed_turns(&conn, session_id, 10).await;

        let fact_1 = "User prefers Rust for systems programming.";
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

        let continuation = fetch_session_continuation(&conn, session_id).await.unwrap();

        assert_eq!(
            continuation.latest_summary.as_deref(),
            Some(expected_summary),
            "Continuation must extract context_summary from latest completed compaction"
        );

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
    .expect("session_continuation_excludes_compacted_turns timed out");
}

/// PURPOSE:  The version history must list every saved revision newest first
///           and mark exactly one of them active; otherwise the UI cannot
///           offer a rollback target.
/// SCENARIO: Save three versions and list them.
/// ASSERTS:  Three versions are listed, ordered 3, 2, 1, and only version 3
///           is active.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Sort the version list ascending — the ordering assertion would
///           fail.
#[tokio::test]
async fn personal_memory_versions_are_listed_newest_first() {
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

        let v1 = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(v1.version, 1);
        assert_eq!(v1.is_active, 1);

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

        let versions = list_personal_memory_versions(&conn, None).await.unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version, 3);
        assert_eq!(versions[0].is_active, 1);
        assert_eq!(versions[1].version, 2);
        assert_eq!(versions[1].is_active, 0);
        assert_eq!(versions[2].version, 1);
        assert_eq!(versions[2].is_active, 0);
    })
    .await
    .expect("personal_memory_versions_are_listed_newest_first timed out");
}

/// PURPOSE:  Switching the active version must make that version the one the
///           app reads; otherwise the rollback button reports success while
///           the old document stays active.
/// SCENARIO: Save two versions, switch the active version back to the first
///           one and read the active document.
/// ASSERTS:  The restored record is version 2 and active, and the active
///           memory contains the first version's content.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Update the version row without clearing the other versions'
///           active flags — the `is_active` assertion would fail.
#[tokio::test]
async fn active_personal_memory_version_can_be_switched_back() {
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

        save_personal_memory(&conn, None, r#"{"sections":[{"id":"sec_t1","title":"V2","blocks":[{"id":"blk_t1","text":"User likes coffee."}]}]}"#, 1)
            .await
            .unwrap();
        save_personal_memory(&conn, None, r#"{"sections":[{"id":"sec_t1","title":"V3","blocks":[{"id":"blk_t1","text":"User likes tea."}]}]}"#, 2)
            .await
            .unwrap();

        let restored = set_active_personal_memory_version(&conn, None, 2)
            .await
            .unwrap();
        assert_eq!(restored.version, 2);
        assert_eq!(restored.is_active, 1);

        let active = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(active.version, 2);
        assert!(active.markdown.contains("User likes coffee"));
    })
    .await
    .expect("active_personal_memory_version_can_be_switched_back timed out");
}

/// PURPOSE:  Staging an empty operation set must be a clean no-op; creating
///           revision rows for nothing would make the review queue show
///           phantom changes.
/// SCENARIO: Stage an empty slice and list the pending revisions.
/// ASSERTS:  Zero rows are staged and no pending revision exists.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Insert one placeholder row for an empty slice — the emptiness
///           assertion would fail.
#[tokio::test]
async fn staging_an_empty_revision_set_is_a_noop() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let initial_record = get_personal_memory(&conn, None).await.unwrap();

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
    })
    .await
    .expect("staging_an_empty_revision_set_is_a_noop timed out");
}

/// PURPOSE:  A staged revision must be pending and anchored to the memory
///           version it was computed against; a wrong anchor would apply the
///           change on top of a document the user never reviewed.
/// SCENARIO: Stage two block creations against the current record and read
///           them back.
/// ASSERTS:  Two pending revisions exist, both with op `create_block`, and
///           both anchored to the record's version.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Store `base_memory_version` as 0 — the anchor assertion would
///           fail.
#[tokio::test]
async fn staged_revisions_are_pending_and_anchored_to_the_base_version() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let initial_record = get_personal_memory(&conn, None).await.unwrap();

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
    .expect("staged_revisions_are_pending_and_anchored_to_the_base_version timed out");
}

/// PURPOSE:  Accepting a revision must apply its operation to the document
///           and bump the version; a rejected or ignored operation would
///           leave the accepted change invisible.
/// SCENARIO: Stage a block creation, accept it through the production service
///           seam and inspect the resulting document.
/// ASSERTS:  The document is version 3 and contains the new block text.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the operation application in `batch_resolve_memory_revisions`
///           — the content assertion would fail.
#[tokio::test]
async fn accepted_revision_applies_its_operation_and_bumps_the_version() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let v2_doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."}]}]}"#;
        let v2_record = save_personal_memory(&conn, None, v2_doc, 1)
            .await
            .expect("v2 document must be saved");
        assert_eq!(v2_record.version, 2);

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
    })
    .await
    .expect("accepted_revision_applies_its_operation_and_bumps_the_version timed out");
}

/// PURPOSE:  Accepting a second revision must build on the newly committed
///           document, not on the original one; applying against a stale base
///           would lose the first change.
/// SCENARIO: Stage two revisions, accept them one after the other and inspect
///           the final document and revision list.
/// ASSERTS:  The document reaches version 4 with both block texts, the
///           second revision is accepted, and no pending revision remains.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Apply the second revision against version 2 instead of version
///           3 — the content assertion would fail.
#[tokio::test]
async fn second_accepted_revision_bumps_the_version_again() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let v2_doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."}]}]}"#;
        let v2_record = save_personal_memory(&conn, None, v2_doc, 1)
            .await
            .expect("v2 document must be saved");

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
        let rev1_id = pending[0].id.clone();
        let rev2_id = pending[1].id.clone();

        batch_resolve_memory_revisions(
            &conn,
            None,
            &[RevisionDecision {
                id: rev1_id,
                action: "accept".to_string(),
            }],
        )
        .await
        .expect("first accept must succeed");

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
    .expect("second_accepted_revision_bumps_the_version_again timed out");
}

/// PURPOSE:  Rejecting a revision must leave the document untouched; a reject
///           that still applies the operation would change the user's memory
///           without their consent.
/// SCENARIO: Stage a block creation, reject it and inspect the document.
/// ASSERTS:  The version stays 2, the content is byte-identical, and the
///           revision status is `rejected`.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Apply the operation regardless of the decision — the content
///           assertion would fail.
#[tokio::test]
async fn rejected_revision_leaves_the_document_unchanged() {
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
    .expect("rejected_revision_leaves_the_document_unchanged timed out");
}

/// PURPOSE:  Every candidate fact must transition straight to `integrated`;
///           a fact left `active` is re-consolidated on the next cycle and
///           the user's memory accumulates duplicates.
/// SCENARIO: Seed three candidate facts, mark them integrated and read their
///           statuses back.
/// ASSERTS:  All three facts are `integrated` and no active personal fact
///           remains.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Make `mark_observations_integrated` a no-op — the status
///           assertion would fail.
#[tokio::test]
async fn integrated_facts_leave_no_active_personal_fact() {
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

        let active = fetch_active_observations_by_type(&conn, "personal")
            .await
            .expect("active query must succeed");
        assert!(
            active.is_empty(),
            "zero candidate facts must remain 'active'"
        );
    })
    .await
    .expect("integrated_facts_leave_no_active_personal_fact timed out");
}

/// PURPOSE:  A manual save re-mints every entity id, so any pending revision
///           that referenced the old ids must be rejected; a stale revision
///           would later re-apply a change the user already overwrote.
/// SCENARIO: Stage a revision, save a manual markdown document and read the
///           revision state back.
/// ASSERTS:  The document is version 3 with the new content, no revision is
///           pending, and the superseded revision is `rejected`.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Skip the bulk rejection in `save_personal_memory_from_markdown`
///           — the pending-revision assertion would fail.
#[tokio::test]
async fn manual_markdown_save_rejects_superseded_revisions() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("test db must connect");

        let initial_doc = r#"{"sections":[{"id":"sec_t1","title":"Profile","blocks":[{"id":"blk_t1","text":"User lives in Chicago."}]}]}"#;
        let v2_record = save_personal_memory(&conn, None, initial_doc, 1)
            .await
            .expect("document must be saved");

        let op = ResolvedOp::UpdateBlock {
            block_id: "blk_t1".to_string(),
            text: "User moved to Seattle.".to_string(),
        };
        stage_revisions(&conn, &v2_record, vec![op]).await.unwrap();

        let pending_before = fetch_pending_revisions(&conn, None).await.unwrap();
        assert_eq!(pending_before.len(), 1);
        let stale_rev_id = pending_before[0].id.clone();

        let md = "## Personal Profile\nUser relocated to Denver.\n";
        let saved = save_personal_memory_from_markdown(&conn, None, md, 2)
            .await
            .expect("manual markdown save must succeed");

        assert_eq!(saved.version, 3);
        assert!(saved.markdown.contains("Personal Profile"));
        assert!(saved.markdown.contains("User relocated to Denver."));

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
    .expect("manual_markdown_save_rejects_superseded_revisions timed out");
}
