//! ============================================================================
//! tests/memory_compaction_test.rs — Memory Compaction Coordinator & Plugin v2 Integration Tests
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/memory/compaction, services/harness/plugins/compaction, persistence/compactions
//! Prerequisites: Turso SQLite engine (vox.db), remote GPU server (http://100.67.98.126:11434/v1, gemma3:12b)
//! Execution    : cargo nextest run --test memory_compaction_test --release --nocapture --test-threads=1
//!                cargo nextest run --test memory_compaction_test --release --nocapture --test-threads=1 -- --ignored
//! Metrics      : Compaction latency, facts extracted, watermark intervals, mutual exclusion
//! ============================================================================

mod common;

use std::{
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use common::{harness::get_test_app_and_state, paths::TempPathsGuard};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use vox_lib::{
    core::{
        settings::{LlmActiveProvider, LlmRemoteConfig},
        state::InteractionState,
    },
    persistence::{
        compactions::{
            commit_compaction_output, fetch_latest_compaction_run, fetch_turns_for_compaction,
            record_compaction_start,
        },
        notifications::find_notification_by_group,
    },
    services::{
        harness::plugins::compaction::{CompactionPlugin, MIN_MESSAGES_FOR_COMPACTION},
        memory::compaction::coordinator::CompactionCoordinator,
    },
    utils::json::parse_unified_compaction_json,
};

const REMOTE_OLLAMA_BASE_URL: &str = "http://100.67.98.126:11434/v1";
const REMOTE_OLLAMA_MODEL: &str = "gemma3:12b";

#[derive(Debug, Deserialize)]
struct DatasetTurn {
    turn: u32,
    user: String,
    assistant: String,
}

/// Locates and loads the 100-turns test dataset from candidate relative paths.
fn load_100_turns_dataset() -> Vec<DatasetTurn> {
    let candidates = [
        PathBuf::from("sandbox/datasets/100-turns/dataset_session-2.json"),
        PathBuf::from("../../sandbox/datasets/100-turns/dataset_session-2.json"),
        PathBuf::from("../sandbox/datasets/100-turns/dataset_session-2.json"),
        PathBuf::from("sandbox/datasets/dataset_session1.json"),
        PathBuf::from("../../sandbox/datasets/dataset_session1.json"),
    ];

    for c in &candidates {
        if c.exists() {
            let content = std::fs::read_to_string(c)
                .unwrap_or_else(|e| panic!("Failed to read dataset file {:?}: {}", c, e));
            let turns: Vec<DatasetTurn> = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("Failed to parse JSON dataset {:?}: {}", c, e));
            return turns;
        }
    }

    panic!(
        "Could not find 100-turns dataset file in candidate paths: {:?}",
        candidates
    );
}

/// Seeds a session row and a sequence of turns into the Turso SQLite database.
async fn seed_session_with_turns(
    conn: &turso::Connection,
    session_id: i64,
    project_id: &str,
    turns: &[DatasetTurn],
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO sessions (id, project_id, title, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
        (
            session_id,
            project_id.to_string(),
            format!("Compaction Test Session #{}", session_id),
            now,
            now,
        ),
    )
    .await
    .expect("Failed to seed session into database");

    for (idx, turn) in turns.iter().enumerate() {
        let turn_time = now + (idx as i64 * 1000);
        conn.execute(
            "INSERT OR REPLACE INTO turns (session_id, turn_id, user_text, assistant_text, created_at)
             VALUES (?, ?, ?, ?, ?)",
            (
                session_id,
                turn.turn as i64,
                turn.user.clone(),
                turn.assistant.clone(),
                turn_time,
            ),
        )
        .await
        .expect("Failed to seed turn into database");
    }
}

// ============================================================================
// Subtest 1: Live Server Compaction (100 Turns, Remote Ollama gemma3:12b)
// ============================================================================

#[tokio::test]
#[ignore = "Requires remote GPU server with Ollama gemma3:12b running"]
async fn test_memory_compaction_100_turns_live_server() {
    let _guard = TempPathsGuard::new();
    let (app, state) = get_test_app_and_state().await;

    // 1. Configure settings to point to the remote GPU Ollama server
    {
        let mut settings = state.settings.write().unwrap();
        settings.llm.active = LlmActiveProvider::Server;
        settings.llm.server = LlmRemoteConfig {
            base_url: REMOTE_OLLAMA_BASE_URL.to_string(),
            model: REMOTE_OLLAMA_MODEL.to_string(),
            api_key: None,
            provider_name: Some("ollama".to_string()),
        };
        settings.llm.context_window = 32768;
    }

    // 2. Load 100 turns from the dataset and seed into Turso SQLite
    let dataset = load_100_turns_dataset();
    let turns_to_seed = if dataset.len() > 100 {
        &dataset[0..100]
    } else {
        &dataset[..]
    };
    assert_eq!(turns_to_seed.len(), 100, "Must load exactly 100 turns for compaction evaluation");

    let session_id = 9101;
    let conn = state.db.connect().expect("Failed to open connection");
    seed_session_with_turns(&conn, session_id, "default", turns_to_seed).await;

    // Set pipeline state to Ready so soft/manual compaction proceeds
    state.pipeline.set_state(InteractionState::Ready);

    // 3. Execute compaction slice via CompactionCoordinator
    let start_time = Instant::now();
    let cancel = CancellationToken::new();
    let summary_res = CompactionCoordinator::run_compaction_slice(
        &app,
        &state,
        session_id,
        "manual",
        Some(&cancel),
    )
    .await;

    let elapsed = start_time.elapsed();
    eprintln!(
        "[Live Compaction] Executed 100-turn compaction on gemma3:12b in {:.2}s",
        elapsed.as_secs_f64()
    );

    let summary = summary_res
        .expect("Compaction coordinator run_compaction_slice must not error")
        .expect("Compaction must return Some(CompactionExecutionSummary)");

    // 4. Assert summary watermark coverage and facts
    assert_eq!(summary.session_id, session_id);
    assert_eq!(summary.from_turn_id, 1, "Compaction slice must start from turn 1");
    assert_eq!(summary.to_turn_id, 100, "Compaction slice must cover through turn 100");
    assert!(
        summary.facts_enqueued > 0,
        "Compacting 100 rich conversational turns must extract at least 1 durable fact"
    );
    assert!(
        !summary.session_context.trim().is_empty(),
        "Compaction must return non-empty session context"
    );

    // 5. Query Turso SQLite: session_compactions record
    let latest_run = fetch_latest_compaction_run(&conn, session_id)
        .await
        .expect("Database query for latest compaction run must succeed")
        .expect("Compaction record must exist in session_compactions table");

    assert_eq!(latest_run.status, "completed", "Compaction status must be 'completed'");
    assert_eq!(latest_run.from_turn_id, 1);
    assert_eq!(latest_run.to_turn_id, 100);
    assert!(latest_run.finished_at.is_some(), "Finished timestamp must be recorded");

    // 6. Assert JSON payload contains valid 6-key structure
    let parsed_json = parse_unified_compaction_json(&latest_run.compaction_output)
        .expect("Compaction output stored in DB must be valid unified compaction JSON");

    eprintln!(
        "[Live Compaction] Facts Extracted: personal={}, objective={}, workdone={}, blocker={}, next_step={}, pitfall={}",
        parsed_json.personal.len(),
        parsed_json.objective.len(),
        parsed_json.workdone.len(),
        parsed_json.blocker.len(),
        parsed_json.next_step.len(),
        parsed_json.pitfall.len(),
    );

    // 7. Query Turso SQLite: memory_ingestion_queue staged facts
    let mut rows = conn
        .query(
            "SELECT COUNT(*), COUNT(DISTINCT type) FROM memory_ingestion_queue WHERE session_id = ? AND status = 'pending'",
            (session_id,),
        )
        .await
        .expect("Failed to query memory_ingestion_queue");

    if let Some(row) = rows.next().await.unwrap() {
        let queued_count: i64 = row.get(0).unwrap();
        let distinct_types: i64 = row.get(1).unwrap();
        assert_eq!(
            queued_count as u32, summary.facts_enqueued,
            "Queued facts in database must match facts_enqueued summary count"
        );
        assert!(distinct_types >= 1, "Must enqueue at least one distinct memory category");
    } else {
        panic!("No result returned for memory_ingestion_queue count query");
    }
}

// ============================================================================
// Subtest 2: Coordinator Slicing, Watermark Progression & Ingestion Ledger
// ============================================================================

#[tokio::test]
async fn test_memory_compaction_coordinator_slicing_and_ledger() {
    let _guard = TempPathsGuard::new();
    let (_app, state) = get_test_app_and_state().await;
    let conn = state.db.connect().expect("Failed to open connection");
    let session_id = 9102;

    // 1. Seed initial 5 turns (turns 1..5)
    let turns_batch_1 = vec![
        DatasetTurn { turn: 1, user: "Turn 1".into(), assistant: "Reply 1".into() },
        DatasetTurn { turn: 2, user: "Turn 2".into(), assistant: "Reply 2".into() },
        DatasetTurn { turn: 3, user: "Turn 3".into(), assistant: "Reply 3".into() },
        DatasetTurn { turn: 4, user: "Turn 4".into(), assistant: "Reply 4".into() },
        DatasetTurn { turn: 5, user: "Turn 5".into(), assistant: "Reply 5".into() },
    ];
    seed_session_with_turns(&conn, session_id, "default", &turns_batch_1).await;

    // 2. Fetch turns pending compaction — initially from turn 1 to u32::MAX
    let pending_1 = fetch_turns_for_compaction(&conn, session_id, 1, u32::MAX)
        .await
        .expect("fetch_turns_for_compaction must succeed");
    assert_eq!(pending_1.len(), 5);
    assert_eq!(pending_1.first().unwrap().turn_id, 1);
    assert_eq!(pending_1.last().unwrap().turn_id, 5);

    // 3. Record compaction run 1 initiation
    let run1_id = record_compaction_start(&conn, session_id, "soft", 1, 5)
        .await
        .expect("record_compaction_start must succeed");
    assert!(run1_id > 0);

    // 4. Commit compaction output with 2 facts
    let output1 = r#"{"personal": ["User prefers concise answers."], "objective": ["Build Vox"]}"#;
    let facts1 = vec![
        ("personal".to_string(), "User prefers concise answers.".to_string()),
        ("objective".to_string(), "Build Vox".to_string()),
    ];
    commit_compaction_output(&conn, run1_id, output1, &facts1, session_id)
        .await
        .expect("commit_compaction_output must succeed");

    // Verify run 1 recorded as completed with watermark [1..5]
    let run1 = fetch_latest_compaction_run(&conn, session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run1.status, "completed");
    assert_eq!(run1.from_turn_id, 1);
    assert_eq!(run1.to_turn_id, 5);

    // 5. Seed next batch of 5 turns (turns 6..10)
    let turns_batch_2 = vec![
        DatasetTurn { turn: 6, user: "Turn 6".into(), assistant: "Reply 6".into() },
        DatasetTurn { turn: 7, user: "Turn 7".into(), assistant: "Reply 7".into() },
        DatasetTurn { turn: 8, user: "Turn 8".into(), assistant: "Reply 8".into() },
        DatasetTurn { turn: 9, user: "Turn 9".into(), assistant: "Reply 9".into() },
        DatasetTurn { turn: 10, user: "Turn 10".into(), assistant: "Reply 10".into() },
    ];
    seed_session_with_turns(&conn, session_id, "default", &turns_batch_2).await;

    // 6. Query uncompacted turns starting strictly AFTER last compacted turn (5 + 1 = 6)
    let last_compacted_turn = run1.to_turn_id;
    let pending_2 = fetch_turns_for_compaction(&conn, session_id, last_compacted_turn + 1, u32::MAX)
        .await
        .expect("fetch_turns_for_compaction must succeed");
    assert_eq!(pending_2.len(), 5, "Must only fetch new uncompacted turns 6..10");
    assert_eq!(pending_2.first().unwrap().turn_id, 6);
    assert_eq!(pending_2.last().unwrap().turn_id, 10);

    // 7. Record compaction run 2 initiation and commit
    let run2_id = record_compaction_start(&conn, session_id, "manual", 6, 10)
        .await
        .expect("record_compaction_start must succeed");
    let output2 = r#"{"workdone": ["Implemented compaction slice."]}"#;
    let facts2 = vec![("workdone".to_string(), "Implemented compaction slice.".to_string())];
    commit_compaction_output(&conn, run2_id, output2, &facts2, session_id)
        .await
        .expect("commit_compaction_output must succeed");

    // 8. Verify watermark progression and ingestion queue total
    let run2 = fetch_latest_compaction_run(&conn, session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run2.status, "completed");
    assert_eq!(run2.from_turn_id, 6);
    assert_eq!(run2.to_turn_id, 10);

    // Ingestion queue must contain exactly 3 facts across the two runs
    let mut q_rows = conn
        .query(
            "SELECT COUNT(*) FROM memory_ingestion_queue WHERE session_id = ? AND status = 'pending'",
            (session_id,),
        )
        .await
        .unwrap();
    let count: i64 = q_rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(count, 3, "Total staged facts in ingestion queue must be 3");

    // All turns are now compacted: next fetch must return 0
    let pending_3 = fetch_turns_for_compaction(&conn, session_id, 11, u32::MAX)
        .await
        .unwrap();
    assert_eq!(pending_3.len(), 0, "No pending turns after full compaction");
}

// ============================================================================
// Subtest 3: Concurrency Mutual Exclusion & Partial Unique Index Guard
// ============================================================================

#[tokio::test]
async fn test_memory_compaction_concurrency_and_partial_unique_index() {
    let _guard = TempPathsGuard::new();
    let (app, state) = get_test_app_and_state().await;
    let conn = state.db.connect().expect("Failed to open connection");
    let session_id = 9103;

    // Seed session and turns
    let turns = vec![
        DatasetTurn { turn: 1, user: "Hello".into(), assistant: "Hi".into() },
        DatasetTurn { turn: 2, user: "Work".into(), assistant: "Done".into() },
    ];
    seed_session_with_turns(&conn, session_id, "default", &turns).await;
    state.pipeline.set_state(InteractionState::Ready);

    // 1. Manually start an in_progress compaction run
    let run_id = record_compaction_start(&conn, session_id, "manual", 1, 2)
        .await
        .expect("Initial record_compaction_start must succeed");
    assert!(run_id > 0);

    // 2. Direct database constraint check:
    // Turso partial unique index `idx_compactions_one_in_progress` MUST reject a second in_progress run
    let duplicate_start_res = record_compaction_start(&conn, session_id, "manual", 1, 2).await;
    assert!(
        duplicate_start_res.is_err(),
        "Partial unique index must reject concurrent in_progress compaction for same session"
    );
    let err_str = duplicate_start_res.err().unwrap().to_string();
    assert!(
        err_str.contains("UNIQUE constraint failed"),
        "Error message must indicate UNIQUE constraint failure: {}",
        err_str
    );

    // 3. Coordinator check:
    // Calling CompactionCoordinator::run_compaction_slice when an in_progress run exists must return Ok(None)
    let coordinator_res = CompactionCoordinator::run_compaction_slice(
        &app,
        &state,
        session_id,
        "manual",
        None,
    )
    .await;

    assert!(
        coordinator_res.is_ok(),
        "Coordinator must gracefully handle concurrent run without error"
    );
    assert!(
        coordinator_res.unwrap().is_none(),
        "Coordinator must return Ok(None) when compaction is already in progress"
    );
}

// ============================================================================
// Subtest 4: Harness CompactionPlugin Invariants & System Prompt Context Injection
// ============================================================================

#[test]
fn test_compaction_plugin_preemptive_fifo_and_context_injection() {
    // 1. Threshold check: min messages invariant
    let plugin = CompactionPlugin::new(8192, false, true);

    assert_eq!(MIN_MESSAGES_FOR_COMPACTION, 4);
    assert!(!plugin.can_perform_inline_compaction(0));
    assert!(!plugin.can_perform_inline_compaction(1));
    assert!(!plugin.can_perform_inline_compaction(2));
    assert!(!plugin.can_perform_inline_compaction(3));
    assert!(plugin.can_perform_inline_compaction(4));
    assert!(plugin.can_perform_inline_compaction(10));

    // 2. Embedded model guard: context_window <= 4096 suppresses inline compaction
    let embedded_constrained = CompactionPlugin::new(4096, true, true);
    assert!(
        !embedded_constrained.can_perform_inline_compaction(10),
        "Embedded models at or below 4096 context window must suppress inline compaction"
    );

    let embedded_sufficient = CompactionPlugin::new(8192, true, true);
    assert!(
        embedded_sufficient.can_perform_inline_compaction(10),
        "Embedded models with >4096 context window may perform inline compaction"
    );

    // 3. Context application and system prompt injection
    let mut active_plugin = CompactionPlugin::new(8192, false, true);
    let sample_context = "User is building Vox with Turso SQLite and Gemma3 LLM.";
    active_plugin.apply_session_context(sample_context);

    assert_eq!(active_plugin.session_context(), Some(sample_context));

    let base_system_prompt = "You are Vox, a high-performance voice assistant.";
    let user_turn = "What database am I using?";

    let pruned_messages = active_plugin.prune_history_with_summary(base_system_prompt, user_turn);

    // Pruned history must have exactly 2 messages: System (with <session_context>) and User
    assert_eq!(pruned_messages.len(), 2);
    assert_eq!(pruned_messages[0].role, vox_lib::services::harness::Role::System);
    assert_eq!(pruned_messages[1].role, vox_lib::services::harness::Role::User);
    assert_eq!(pruned_messages[1].content, user_turn);

    let formatted_system = &pruned_messages[0].content;
    assert!(
        formatted_system.contains("<session_context>"),
        "Formatted system prompt must include opening <session_context> tag"
    );
    assert!(
        formatted_system.contains("</session_context>"),
        "Formatted system prompt must include closing </session_context> tag"
    );
    assert!(
        formatted_system.contains(sample_context),
        "Formatted system prompt must contain the applied session context"
    );
    assert!(
        formatted_system.contains(base_system_prompt),
        "Formatted system prompt must preserve the base system prompt instructions"
    );
}

// ============================================================================
// Subtest 5: Compaction Notification Lifecycle & In-Place Resolution
// ============================================================================

#[tokio::test]
async fn test_compaction_notification_lifecycle() {
    let _guard = TempPathsGuard::new();
    let (app, state) = get_test_app_and_state().await;
    let conn = state.db.connect().expect("Failed to open connection");
    let session_id = 9105;

    // 1. Alert user that session has uncompacted turns via interactive notification card
    let notif_id_res = CompactionCoordinator::notify_uncompacted_session(
        &app,
        &state.db,
        session_id,
        18,
    )
    .await;

    assert!(notif_id_res.is_ok(), "notify_uncompacted_session must succeed");
    let notif_id = notif_id_res.unwrap().expect("Must return notification ID");
    assert!(!notif_id.is_empty());

    // Verify notification was stored in database with interactive action
    let group_key = format!("session_compaction:{}", session_id);
    let card = find_notification_by_group(&conn, &group_key)
        .await
        .expect("Database query for notification must succeed")
        .expect("Notification card must exist in database");

    assert_eq!(card.group_key, group_key);
    assert_eq!(card.action_type, "interactive");
    assert!(card.metadata.contains("\"resolution\":\"pending\""));

    // 2. Simulate successful compaction run completion and receipt emission
    let run_id = record_compaction_start(&conn, session_id, "manual", 1, 18)
        .await
        .unwrap();
    let facts = vec![("personal".to_string(), "User lives in Seattle.".to_string())];
    commit_compaction_output(&conn, run_id, r#"{"personal": ["User lives in Seattle."]}"#, &facts, session_id)
        .await
        .unwrap();

    // 3. Verify in-place resolution:
    // Resolves the existing card from "pending" to "resolved" without creating duplicate cards
    let message = format!(
        "Successfully compacted session #{} and extracted {} memory facts.",
        session_id, facts.len()
    );
    let resolved_card = vox_lib::persistence::notifications::resolve_notification_in_place(
        &conn,
        &card.id,
        "resolved",
        Some(&message),
    )
    .await
    .expect("resolve_notification_in_place must succeed")
    .expect("Card must be returned after in-place resolution");

    assert_eq!(resolved_card.id, card.id, "Resolved card ID must match original card ID");
    assert!(
        resolved_card.metadata.contains("\"resolution\":\"resolved\""),
        "Card metadata resolution must be updated to 'resolved': {}",
        resolved_card.metadata
    );
    assert!(
        resolved_card.message.contains("extracted 1 memory facts"),
        "Resolved card message must state extracted fact count"
    );
}
