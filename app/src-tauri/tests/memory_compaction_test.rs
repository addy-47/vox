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

use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
        harness::stages::compaction::{CompactionStage, MIN_MESSAGES_FOR_COMPACTION},
        memory::compaction::coordinator::CompactionCoordinator,
    },
    utils::json::parse_unified_compaction_json,
};

const REMOTE_OLLAMA_URL: &str = "http://100.67.98.126:11434/v1";
const REMOTE_OLLAMA_MODEL: &str = "gemma3:12b";

#[derive(Debug, Deserialize)]
struct DatasetTurn {
    turn: u32,
    user: String,
    assistant: String,
}

fn load_dataset(max_turns: usize) -> Vec<DatasetTurn> {
    let mut turns: Vec<DatasetTurn> =
        common::paths::load_json_dataset("legacy/dataset_session-2.json");
    turns.truncate(max_turns);
    turns
}

async fn seed_turns(conn: &turso::Connection, session_id: i64, turns: &[DatasetTurn]) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO sessions (id, project_id, title, created_at, updated_at) VALUES (?, 'default', 'Test', ?, ?)",
        (session_id, now, now),
    ).await.unwrap();

    for t in turns {
        conn.execute(
            "INSERT OR REPLACE INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (?, ?, ?, ?, ?)",
            (session_id, t.turn as i64, t.user.clone(), t.assistant.clone(), now),
        ).await.unwrap();
    }
}

// ============================================================================
// Subtest 1: Live Server Compaction (100 Turns -> Remote Ollama gemma3:12b -> Turso DB)
// ============================================================================
#[tokio::test]
#[ignore = "Requires remote GPU server with Ollama gemma3:12b running"]
async fn test_memory_compaction_100_turns_live_server() {
    let _guard = TempPathsGuard::new();
    let (app, state) = get_test_app_and_state().await;

    // 1. Point to remote GPU Ollama server with gemma3:12b
    {
        let mut settings = state.settings.write().unwrap();
        settings.llm.active = LlmActiveProvider::Server;
        settings.llm.server = LlmRemoteConfig {
            base_url: REMOTE_OLLAMA_URL.to_string(),
            model: REMOTE_OLLAMA_MODEL.to_string(),
            api_key: None,
            provider_name: Some("ollama".to_string()),
        };
        settings.llm.context_window = 32768;
    }

    // 2. Seed 100 turns from dataset into Turso SQLite
    let session_id = 9101;
    let turns = load_dataset(100);
    assert_eq!(turns.len(), 100, "Must seed exactly 100 turns from dataset");
    let conn = state.db.connect().unwrap();
    seed_turns(&conn, session_id, &turns).await;
    state.pipeline.set_state(InteractionState::Ready);

    // 3. Execute compaction slice via CompactionCoordinator
    let start = Instant::now();
    let cancel = CancellationToken::new();
    let summary = CompactionCoordinator::run_compaction_slice(
        &app,
        &state,
        session_id,
        "manual",
        Some(&cancel),
    )
    .await
    .expect("run_compaction_slice must succeed")
    .expect("Must return compaction summary");

    eprintln!(
        "[Live Compaction] Sliced 100 turns via gemma3:12b in {:.2}s",
        start.elapsed().as_secs_f64()
    );

    // 4. Assert summary & Turso SQLite state
    assert_eq!(summary.from_turn_id, 1);
    assert_eq!(summary.to_turn_id, 100);
    assert!(
        summary.facts_enqueued > 0,
        "Expected facts extracted from 100 turns"
    );

    let run = fetch_latest_compaction_run(&conn, session_id)
        .await
        .unwrap()
        .expect("Compaction record must exist");
    assert_eq!(run.status, "completed");
    assert_eq!(run.from_turn_id, 1);
    assert_eq!(run.to_turn_id, 100);

    let parsed =
        parse_unified_compaction_json(&run.compaction_output).expect("Must be valid 6-key JSON");
    let total_facts = parsed.personal.len()
        + parsed.objective.len()
        + parsed.workdone.len()
        + parsed.blocker.len()
        + parsed.next_step.len()
        + parsed.pitfall.len();
    assert!(total_facts > 0, "6-key schema must contain extracted facts");

    // 5. Assert memory_ingestion_queue staging
    let mut q_rows = conn.query(
        "SELECT COUNT(*) FROM memory_ingestion_queue WHERE session_id = ? AND status = 'pending'",
        (session_id,),
    ).await.unwrap();
    let count: i64 = q_rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(count as u32, summary.facts_enqueued);
}

// ============================================================================
// Subtest 2: Coordinator Slicing, Watermark Progression & Ingestion Ledger
// ============================================================================
#[tokio::test]
async fn test_memory_compaction_coordinator_slicing_and_ledger() {
    let _guard = TempPathsGuard::new();
    let (_app, state) = get_test_app_and_state().await;
    let conn = state.db.connect().unwrap();
    let session_id = 9102;

    // 1. Seed initial 5 turns (turns 1..5)
    let turns_1 = (1..=5)
        .map(|i| DatasetTurn {
            turn: i,
            user: format!("User {}", i),
            assistant: format!("Bot {}", i),
        })
        .collect::<Vec<_>>();
    seed_turns(&conn, session_id, &turns_1).await;

    // 2. Query pending turns for initial slice
    let pending_1 = fetch_turns_for_compaction(&conn, session_id, 1, u32::MAX)
        .await
        .unwrap();
    assert_eq!(pending_1.len(), 5);

    // 3. Record run 1 and commit with 2 facts
    let run1_id = record_compaction_start(&conn, session_id, "soft", 1, 5)
        .await
        .unwrap();
    let facts1 = vec![(
        "personal".to_string(),
        "User prefers concise answers.".to_string(),
    )];
    commit_compaction_output(
        &conn,
        run1_id,
        r#"{"personal": ["User prefers concise answers."]}"#,
        &facts1,
        session_id,
    )
    .await
    .unwrap();

    let run1 = fetch_latest_compaction_run(&conn, session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run1.status, "completed");
    assert_eq!(run1.to_turn_id, 5);

    // 4. Seed next 5 turns (turns 6..10)
    let turns_2 = (6..=10)
        .map(|i| DatasetTurn {
            turn: i,
            user: format!("User {}", i),
            assistant: format!("Bot {}", i),
        })
        .collect::<Vec<_>>();
    seed_turns(&conn, session_id, &turns_2).await;

    // 5. Query turns starting strictly after last compacted turn (5 + 1 = 6)
    let pending_2 = fetch_turns_for_compaction(&conn, session_id, run1.to_turn_id + 1, u32::MAX)
        .await
        .unwrap();
    assert_eq!(pending_2.len(), 5);
    assert_eq!(pending_2.first().unwrap().turn_id, 6);
    assert_eq!(pending_2.last().unwrap().turn_id, 10);

    // 6. Record run 2 and commit
    let run2_id = record_compaction_start(&conn, session_id, "manual", 6, 10)
        .await
        .unwrap();
    let facts2 = vec![(
        "workdone".to_string(),
        "Implemented compaction slice.".to_string(),
    )];
    commit_compaction_output(
        &conn,
        run2_id,
        r#"{"workdone": ["Implemented compaction slice."]}"#,
        &facts2,
        session_id,
    )
    .await
    .unwrap();

    let run2 = fetch_latest_compaction_run(&conn, session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run2.status, "completed");
    assert_eq!(run2.from_turn_id, 6);
    assert_eq!(run2.to_turn_id, 10);

    // 7. Verify ingestion queue holds all staged facts
    let mut q_rows = conn.query(
        "SELECT COUNT(*) FROM memory_ingestion_queue WHERE session_id = ? AND status = 'pending'",
        (session_id,),
    ).await.unwrap();
    let count: i64 = q_rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(
        count, 2,
        "Ingestion queue must hold exactly 2 facts across both runs"
    );
}

// ============================================================================
// Subtest 3: Concurrency Mutual Exclusion & Partial Unique Index Guard
// ============================================================================
#[tokio::test]
async fn test_memory_compaction_concurrency_and_partial_unique_index() {
    let _guard = TempPathsGuard::new();
    let (app, state) = get_test_app_and_state().await;
    let conn = state.db.connect().unwrap();
    let session_id = 9103;

    let turns = (1..=2)
        .map(|i| DatasetTurn {
            turn: i,
            user: format!("User {}", i),
            assistant: format!("Bot {}", i),
        })
        .collect::<Vec<_>>();
    seed_turns(&conn, session_id, &turns).await;
    state.pipeline.set_state(InteractionState::Ready);

    // 1. Manually start an in_progress compaction run
    let run_id = record_compaction_start(&conn, session_id, "manual", 1, 2)
        .await
        .unwrap();
    assert!(run_id > 0);

    // 2. Direct DB constraint: Partial unique index idx_compactions_one_in_progress MUST reject duplicate
    let dup_insert = record_compaction_start(&conn, session_id, "manual", 1, 2).await;
    assert!(
        dup_insert.is_err(),
        "Partial unique index must reject duplicate in_progress run"
    );
    assert!(dup_insert
        .err()
        .unwrap()
        .to_string()
        .contains("UNIQUE constraint failed"));

    // 3. Coordinator mutual exclusion check: run_compaction_slice must return Ok(None)
    let slice_res =
        CompactionCoordinator::run_compaction_slice(&app, &state, session_id, "manual", None).await;
    assert!(slice_res.is_ok());
    assert!(
        slice_res.unwrap().is_none(),
        "Coordinator must return Ok(None) while compaction is in progress"
    );
}

// ============================================================================
// Subtest 4: Harness CompactionStage Invariants & Context Injection
// ============================================================================
#[test]
fn test_compaction_plugin_preemptive_fifo_and_context_injection() {
    // 1. Invariant: Minimum 4 messages required for inline compaction
    let plugin = CompactionStage::new(8192, false, true);
    assert_eq!(MIN_MESSAGES_FOR_COMPACTION, 4);
    assert!(!plugin.can_perform_inline_compaction(0));
    assert!(!plugin.can_perform_inline_compaction(3));
    assert!(plugin.can_perform_inline_compaction(4));

    // 2. Invariant: Embedded models at standard context window (>=8192) perform inline compaction
    let embedded_model = CompactionStage::new(8192, true, true);
    assert!(!embedded_model.can_perform_inline_compaction(3));
    assert!(embedded_model.can_perform_inline_compaction(4));

    let remote_model = CompactionStage::new(8192, false, true);
    assert!(!remote_model.can_perform_inline_compaction(3));
    assert!(remote_model.can_perform_inline_compaction(4));

    // 3. Invariant: prune_history_with_summary injects <session_context> and prunes to [System, User]
    let mut active_plugin = CompactionStage::new(8192, false, true);
    let sample_context = "User is building Vox with Turso SQLite and Gemma3 LLM.";
    active_plugin.apply_session_context(sample_context);

    let base_prompt = "You are Vox, a helpful assistant.";
    let user_turn = "What database am I using?";
    let pruned = active_plugin.prune_history_with_summary(base_prompt, user_turn);

    assert_eq!(pruned.len(), 2);
    assert_eq!(pruned[0].role, vox_lib::services::harness::Role::System);
    assert_eq!(pruned[1].role, vox_lib::services::harness::Role::User);
    assert_eq!(pruned[1].content, user_turn);

    let system_text = &pruned[0].content;
    assert!(system_text.contains("<session_context>"));
    assert!(system_text.contains("</session_context>"));
    assert!(system_text.contains(sample_context));
    assert!(system_text.contains(base_prompt));
}

// ============================================================================
// Subtest 5: Notification Card In-Place Resolution Lifecycle
// ============================================================================
#[tokio::test]
async fn test_compaction_notification_lifecycle() {
    let _guard = TempPathsGuard::new();
    let (app, state) = get_test_app_and_state().await;
    let conn = state.db.connect().unwrap();
    let session_id = 9105;

    // 1. Emit interactive card for uncompacted session
    let notif_id =
        CompactionCoordinator::notify_uncompacted_session(&app, &state.db, session_id, 15)
            .await
            .unwrap()
            .expect("Must emit notification ID");

    let group_key = format!("session_compaction:{}", session_id);
    let card = find_notification_by_group(&conn, &group_key)
        .await
        .unwrap()
        .expect("Card must exist in DB");
    assert_eq!(card.group_key, group_key);
    assert_eq!(card.action_type, "interactive");
    let card_meta: serde_json::Value =
        serde_json::from_str(&card.metadata).expect("Must be valid JSON");
    assert_eq!(card_meta["resolution"], "pending");

    // 2. Resolve card in-place upon compaction completion
    let resolved = vox_lib::persistence::notifications::resolve_notification_in_place(
        &conn,
        &notif_id,
        "resolved",
        Some("Compacted 15 turns into 3 facts."),
    )
    .await
    .unwrap()
    .expect("Card must resolve in-place");

    assert_eq!(resolved.id, notif_id);
    assert_eq!(resolved.status, "unread");
    let resolved_meta: serde_json::Value =
        serde_json::from_str(&resolved.metadata).expect("Must be valid JSON");
    assert_eq!(resolved_meta["resolution"], "resolved");
    assert!(resolved.message.contains("Compacted 15 turns"));
}

// ============================================================================
// Subtest 6: Boundary Multi-Slice Compaction Seeds Prior Summary Invariant
// ============================================================================
#[tokio::test]
async fn test_compaction_boundary_multi_slice_preserves_prior_summary() {
    let _guard = TempPathsGuard::new();
    let (_app, state) = get_test_app_and_state().await;
    let conn = state.db.connect().unwrap();
    let session_id = 9106;

    // 1. Seed initial turns (1..3)
    let turns_1 = (1..=3)
        .map(|i| DatasetTurn {
            turn: i,
            user: format!("User question {}", i),
            assistant: format!("Bot answer {}", i),
        })
        .collect::<Vec<_>>();
    seed_turns(&conn, session_id, &turns_1).await;

    // 2. Commit completed compaction run for slice 1..3
    let run1_id = record_compaction_start(&conn, session_id, "soft", 1, 3)
        .await
        .unwrap();
    let prior_json =
        r#"{"objective": ["Build rolling compaction pipeline"], "personal": ["Engineer"]}"#;
    commit_compaction_output(
        &conn,
        run1_id,
        prior_json,
        &[(
            "objective".to_string(),
            "Build rolling compaction pipeline".to_string(),
        )],
        session_id,
    )
    .await
    .unwrap();

    // 3. Seed slice 4..5
    let turns_2 = (4..=5)
        .map(|i| DatasetTurn {
            turn: i,
            user: format!("User question {}", i),
            assistant: format!("Bot answer {}", i),
        })
        .collect::<Vec<_>>();
    seed_turns(&conn, session_id, &turns_2).await;

    // 4. Verify fetch_session_continuation recovers formatted prior summary
    let continuation =
        vox_lib::persistence::sessions::fetch_session_continuation(&conn, session_id)
            .await
            .unwrap();
    assert!(continuation.latest_summary.is_some());
    let summary_str = continuation.latest_summary.unwrap();
    assert!(summary_str.contains("Build rolling compaction pipeline"));

    // 5. Query turns for next slice (turn 4+) and verify build_compaction_request receives prior summary
    let next_turns = fetch_turns_for_compaction(&conn, session_id, 4, u32::MAX)
        .await
        .unwrap();
    assert_eq!(next_turns.len(), 2);

    let mut history_messages = Vec::new();
    history_messages.push(vox_lib::services::harness::ChatMessage::new(
        vox_lib::services::harness::Role::System,
        vox_lib::services::harness::PromptTag::SessionContext.wrap(&summary_str),
    ));
    for t in next_turns {
        history_messages.push(vox_lib::services::harness::ChatMessage::new(
            vox_lib::services::harness::Role::User,
            t.user_text,
        ));
        history_messages.push(vox_lib::services::harness::ChatMessage::new(
            vox_lib::services::harness::Role::Assistant,
            t.assistant_text,
        ));
    }

    let request = vox_lib::services::memory::compaction::prompt::build_compaction_request(
        &history_messages,
        None,
    );
    let user_msg = request
        .input
        .messages
        .iter()
        .find(|m| m.role == vox_lib::services::harness::Role::User)
        .expect("Must have user message in compaction request");
    assert!(
        user_msg.content.contains("<prior_summary>"),
        "Compaction request must include <prior_summary> tag"
    );
    assert!(
        user_msg
            .content
            .contains("Build rolling compaction pipeline"),
        "Compaction request must include content from prior compaction"
    );
}
