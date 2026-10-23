//! ============================================================================
//! memory_compaction_test.rs — compaction slices only the turns past the watermark, refuse to run twice for one session, and inject the prior summary into the next request
//! ============================================================================
//! Category  : Integration (Seam 12 — memory compaction)
//! Component : src/services/memory/compaction/coordinator.rs, src/services/harness/stages/compaction.rs, src/persistence/compactions.rs
//! Binary    : memory_compaction_test
//! Prereqs   : none (the live-server scenario additionally needs the remote Ollama host)
//! Run       : cargo nextest run --test memory_compaction_test --release --test-threads=1
//! ============================================================================

mod common;

use std::time::{SystemTime, UNIX_EPOCH};

use common::{harness::get_test_app_and_state, paths::TempPathsGuard};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use vox_lib::{
    core::state::InteractionState,
    persistence::{
        compactions::{
            commit_compaction_output, fetch_latest_compaction_run, fetch_turns_for_compaction,
            record_compaction_start,
        },
        notifications::find_notification_by_group,
    },
    services::{
        harness::stages::compaction::{CompactionStage, MIN_MESSAGES_FOR_COMPACTION},
        llm::LlmCustomServer,
        memory::compaction::coordinator::{build_history_messages, CompactionCoordinator},
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

/// PURPOSE:  A compaction must cover only the turns after the last compacted
///           turn; re-compacting old turns re-writes the same facts and burns
///           the LLM budget on history that is already summarised.
/// SCENARIO: Seed five turns, commit a compaction for turns 1..5, then seed
///           five more turns and query the pending compaction range.
/// ASSERTS:  The pending range starts at turn 6 and ends at turn 10, and the
///           committed run is marked `completed` up to turn 5.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Start the pending range at turn 1 regardless of the watermark —
///           the first-turn-id assertion would fail.
#[tokio::test]
async fn compaction_slice_covers_only_the_turns_past_the_watermark() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().unwrap();
        let session_id = 9102;

        let turns_1 = (1..=5)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User {}", i),
                assistant: format!("Bot {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_1).await;

        let pending_1 = fetch_turns_for_compaction(&conn, session_id, 1, u32::MAX)
            .await
            .unwrap();
        assert_eq!(pending_1.len(), 5);

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

        let turns_2 = (6..=10)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User {}", i),
                assistant: format!("Bot {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_2).await;

        let pending_2 =
            fetch_turns_for_compaction(&conn, session_id, run1.to_turn_id + 1, u32::MAX)
                .await
                .unwrap();
        assert_eq!(pending_2.len(), 5);
        assert_eq!(pending_2.first().unwrap().turn_id, 6);
        assert_eq!(pending_2.last().unwrap().turn_id, 10);
    })
    .await
    .expect("compaction_slice_covers_only_the_turns_past_the_watermark timed out");
}

/// PURPOSE:  Every fact extracted by a compaction must land in the ingestion
///           queue; a fact that never reaches the queue is never stored and
///           the user's memory silently loses it.
/// SCENARIO: Commit two compaction runs with one fact each and count the
///           pending queue rows for the session.
/// ASSERTS:  Exactly two facts are queued as `pending`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the queue insert in `commit_compaction_output` — the count
///           assertion would fail.
#[tokio::test]
async fn compaction_commit_stages_every_fact_in_the_ingestion_queue() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().unwrap();
        let session_id = 9102;

        let turns_1 = (1..=5)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User {}", i),
                assistant: format!("Bot {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_1).await;

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

        let turns_2 = (6..=10)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User {}", i),
                assistant: format!("Bot {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_2).await;

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

        let mut q_rows = conn.query(
            "SELECT COUNT(*) FROM memory_ingestion_queue WHERE session_id = ? AND status = 'pending'",
            (session_id,),
        ).await.unwrap();
        let count: i64 = q_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            count, 2,
            "Ingestion queue must hold exactly 2 facts across both runs"
        );
    })
    .await
    .expect("compaction_commit_stages_every_fact_in_the_ingestion_queue timed out");
}

/// PURPOSE:  Two compactions of the same session must not run at once; the
///           second one would duplicate every extracted fact.
/// SCENARIO: Record an in-progress compaction and try to record a second one
///           for the same session.
/// ASSERTS:  The second call fails with a `UNIQUE constraint failed` error.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Drop the partial unique index from the migration — the error
///           assertion would fail.
#[tokio::test]
async fn second_in_progress_compaction_on_a_session_is_rejected() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
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

        let run_id = record_compaction_start(&conn, session_id, "manual", 1, 2)
            .await
            .unwrap();
        assert!(run_id > 0);

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
    })
    .await
    .expect("second_in_progress_compaction_on_a_session_is_rejected timed out");
}

/// PURPOSE:  While a compaction is already running the coordinator must
///           decline the request instead of starting a second pass.
/// SCENARIO: Start an in-progress compaction, then ask the coordinator to run
///           a compaction slice for the same session.
/// ASSERTS:  The coordinator returns `Ok(None)`.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove the `has_in_progress_compaction` guard in
///           `CompactionCoordinator::run_compaction_slice` — the `is_none`
///           assertion would fail.
#[tokio::test]
async fn coordinator_declines_a_slice_while_a_compaction_is_in_progress() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
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

        record_compaction_start(&conn, session_id, "manual", 1, 2)
            .await
            .unwrap();

        let slice_res =
            CompactionCoordinator::run_compaction_slice(&app, &state, session_id, "manual", None)
                .await;
        assert!(slice_res.is_ok());
        assert!(
            slice_res.unwrap().is_none(),
            "Coordinator must return Ok(None) while compaction is in progress"
        );
    })
    .await
    .expect("coordinator_declines_a_slice_while_a_compaction_is_in_progress timed out");
}

/// PURPOSE:  Inline compaction must not fire on a short conversation; the
///           compaction call would cost an LLM round-trip for a summary of
///           nothing.
/// SCENARIO: Ask the compaction stage whether it can compact conversations of
///           0, 3 and 4 messages.
/// ASSERTS:  The minimum is four messages and only four or more qualifies.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Lower `MIN_MESSAGES_FOR_COMPACTION` to 1 — the boundary
///           assertion would fail.
#[test]
fn inline_compaction_requires_the_minimum_message_count() {
    let plugin = CompactionStage::new(true);

    assert_eq!(MIN_MESSAGES_FOR_COMPACTION, 4);
    assert!(!plugin.can_perform_inline_compaction(0));
    assert!(!plugin.can_perform_inline_compaction(3));
    assert!(plugin.can_perform_inline_compaction(4));
}

/// PURPOSE:  Pruning the history must leave exactly the system prompt and the
///           current user turn; keeping older messages defeats the purpose of
///           compaction.
/// SCENARIO: Apply a session context and prune the history to a base prompt
///           plus one user turn.
/// ASSERTS:  The pruned history has two messages with roles System and User,
///           and the user content is the current turn.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Keep the last assistant message during pruning — the length
///           assertion would fail.
#[test]
fn pruned_history_keeps_only_the_system_and_the_current_user_turn() {
    let mut active_plugin = CompactionStage::new(true);
    active_plugin.apply_session_context("User is building Vox with Turso SQLite and Gemma3 LLM.");

    let base_prompt = "You are Vox, a helpful assistant.";
    let user_turn = "What database am I using?";
    let pruned = active_plugin.prune_history_with_summary(base_prompt, user_turn);

    assert_eq!(pruned.len(), 2);
    assert_eq!(pruned[0].role, vox_lib::services::harness::Role::System);
    assert_eq!(pruned[1].role, vox_lib::services::harness::Role::User);
    assert_eq!(pruned[1].content, user_turn);
}

/// PURPOSE:  The pruned system prompt must carry the session context and the
///           base prompt inside the `<session_context>` envelope; without it
///           the model loses the compacted summary.
/// SCENARIO: Apply a session context, prune the history and inspect the
///           system message.
/// ASSERTS:  The system text contains the opening and closing
///           `<session_context>` tags, the session context text and the base
///           prompt.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the session context from the pruned system prompt — the tag
///           assertion would fail.
#[test]
fn pruned_system_prompt_carries_the_session_context() {
    let mut active_plugin = CompactionStage::new(true);
    let sample_context = "User is building Vox with Turso SQLite and Gemma3 LLM.";
    active_plugin.apply_session_context(sample_context);

    let base_prompt = "You are Vox, a helpful assistant.";
    let user_turn = "What database am I using?";
    let pruned = active_plugin.prune_history_with_summary(base_prompt, user_turn);

    let system_text = &pruned[0].content;
    assert!(system_text.contains("<session_context>"));
    assert!(system_text.contains("</session_context>"));
    assert!(system_text.contains(sample_context));
    assert!(system_text.contains(base_prompt));
}

/// PURPOSE:  The "session needs compaction" card must exist in the drawer
///           with a pending resolution; without it the user never learns that
///           the session grew too large.
/// SCENARIO: Emit the notification for a session with 15 uncompacted turns
///           and read the card back from the database.
/// ASSERTS:  The card exists under the `session_compaction:<id>` group, is
///           interactive and its metadata resolution is `pending`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the notification insert in
///           `CompactionCoordinator::notify_uncompacted_session` — the card
///           assertion would fail.
#[tokio::test]
async fn uncompacted_session_notification_is_created_as_a_pending_card() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().unwrap();
        let session_id = 9105;

        let _notif_id =
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
    })
    .await
    .expect("uncompacted_session_notification_is_created_as_a_pending_card timed out");
}

/// PURPOSE:  When the compaction finishes the card must be resolved in place
///           so the user sees the outcome instead of a stale "pending" card.
/// SCENARIO: Resolve the emitted card through
///           `resolve_notification_in_place` and inspect the record.
/// ASSERTS:  The resolved record keeps the same id, stays `unread`, its
///           metadata resolution is `resolved` and its message names the
///           compacted turns.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Mark the card as `dismissed` instead of resolving it in place —
///           the status assertion would fail.
#[tokio::test]
async fn compaction_card_is_resolved_in_place_on_completion() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().unwrap();
        let session_id = 9105;

        let notif_id =
            CompactionCoordinator::notify_uncompacted_session(&app, &state.db, session_id, 15)
                .await
                .unwrap()
                .expect("Must emit notification ID");

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
    })
    .await
    .expect("compaction_card_is_resolved_in_place_on_completion timed out");
}

/// PURPOSE:  After a slice is compacted the next continuation must recover
///           the earlier summary; losing it means each slice starts blind and
///           the session loses its thread.
/// SCENARIO: Commit a compaction for turns 1..3, seed turns 4..5 and fetch
///           the session continuation.
/// ASSERTS:  The continuation carries a latest summary that contains the
///           earlier compaction's objective text.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Return `None` for `latest_summary` when a compaction exists —
///           the summary assertion would fail.
#[tokio::test]
async fn continuation_recovers_the_prior_compaction_summary() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().unwrap();
        let session_id = 9106;

        let turns_1 = (1..=3)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User question {}", i),
                assistant: format!("Bot answer {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_1).await;

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

        let turns_2 = (4..=5)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User question {}", i),
                assistant: format!("Bot answer {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_2).await;

        let continuation =
            vox_lib::persistence::sessions::fetch_session_continuation(&conn, session_id)
                .await
                .unwrap();
        assert!(continuation.latest_summary.is_some());
        let summary_str = continuation.latest_summary.unwrap();
        assert!(summary_str.contains("Build rolling compaction pipeline"));
    })
    .await
    .expect("continuation_recovers_the_prior_compaction_summary timed out");
}

/// PURPOSE:  The production history assembler must lead with a System message
///           that carries the session context; a request without it drops the
///           prior summary entirely.
/// SCENARIO: Feed the pending turns and the recovered summary to
///           `build_history_messages` and inspect the produced messages.
/// ASSERTS:  The first message has role System and the history holds one
///           system plus two user plus two assistant messages.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the System message in `build_history_messages` — the role
///           assertion would fail.
#[tokio::test]
async fn production_assembler_emits_a_system_message_carrying_the_prior_summary() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().unwrap();
        let session_id = 9106;

        let turns_1 = (1..=3)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User question {}", i),
                assistant: format!("Bot answer {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_1).await;

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

        let turns_2 = (4..=5)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User question {}", i),
                assistant: format!("Bot answer {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_2).await;

        let continuation =
            vox_lib::persistence::sessions::fetch_session_continuation(&conn, session_id)
                .await
                .unwrap();
        let summary_str = continuation.latest_summary.expect("summary must exist");

        let next_turns = fetch_turns_for_compaction(&conn, session_id, 4, u32::MAX)
            .await
            .unwrap();
        assert_eq!(next_turns.len(), 2);

        let history_messages = build_history_messages(&next_turns, Some(&summary_str));

        assert_eq!(
            history_messages.first().map(|m| m.role),
            Some(vox_lib::services::harness::Role::System),
            "production assembler must lead with a System message carrying session context"
        );
        assert_eq!(
            history_messages.len(),
            5,
            "assembler must emit 1 System + 2 User + 2 Assistant messages"
        );
    })
    .await
    .expect("production_assembler_emits_a_system_message_carrying_the_prior_summary timed out");
}

/// PURPOSE:  The compaction request must carry the prior summary inside a
///           `<prior_summary>` tag; the model can only use the earlier
///           context if it is explicitly handed over.
/// SCENARIO: Build the production compaction request from the assembled
///           history and inspect the user message.
/// ASSERTS:  The user message contains the `<prior_summary>` tag and the
///           earlier compaction's objective text.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the prior summary from `build_compaction_request` — the tag
///           assertion would fail.
#[tokio::test]
async fn compaction_request_includes_the_prior_summary_tag() {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().unwrap();
        let session_id = 9106;

        let turns_1 = (1..=3)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User question {}", i),
                assistant: format!("Bot answer {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_1).await;

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

        let turns_2 = (4..=5)
            .map(|i| DatasetTurn {
                turn: i,
                user: format!("User question {}", i),
                assistant: format!("Bot answer {}", i),
            })
            .collect::<Vec<_>>();
        seed_turns(&conn, session_id, &turns_2).await;

        let continuation =
            vox_lib::persistence::sessions::fetch_session_continuation(&conn, session_id)
                .await
                .unwrap();
        let summary_str = continuation.latest_summary.expect("summary must exist");

        let next_turns = fetch_turns_for_compaction(&conn, session_id, 4, u32::MAX)
            .await
            .unwrap();
        let history_messages = build_history_messages(&next_turns, Some(&summary_str));

        let request = vox_lib::services::memory::compaction::prompt::build_compaction_request(
            &history_messages,
            None,
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
    })
    .await
    .expect("compaction_request_includes_the_prior_summary_tag timed out");
}

/// PURPOSE:  A full 100-turn compaction against a live LLM must slice the
///           whole session, extract facts, persist the run and stage every
///           fact in the ingestion queue.
/// SCENARIO: Seed 100 dataset turns, point the settings at a remote Ollama
///           server and run the compaction slice.
/// ASSERTS:  The summary spans turns 1..100 and extracted facts, the persisted
///           run is `completed` over the same range, the output parses as the
///           six-key schema with facts, and the pending queue count equals
///           the number of enqueued facts.
/// CLASS:    happy-path
/// REQUIRES: a reachable Ollama server serving gemma3:12b at
///           http://100.67.98.126:11434/v1
/// MUTANT:   Change `fetch_turns_for_compaction` to cap the slice at 20 turns
///           — the turn-range assertions would fail.
#[tokio::test]
#[ignore = "Requires remote GPU server with Ollama gemma3:12b running"]
async fn compaction_slice_100_turns_against_a_remote_ollama_server() {
    tokio::time::timeout(std::time::Duration::from_secs(600), async {
        let _guard = TempPathsGuard::new();
        let (app, state) = get_test_app_and_state().await;

        {
            let mut settings = state.settings.write().unwrap();
            settings.llm.active = vox_lib::config::ActiveSelection {
                category: "server".to_string(),
                provider: Some("ollama".to_string()),
                model: REMOTE_OLLAMA_MODEL.to_string(),
            };
            settings.providers.llm.servers = vec![LlmCustomServer {
                id: "ollama-server".to_string(),
                name: "Ollama".to_string(),
                url: REMOTE_OLLAMA_URL.to_string(),
                model: REMOTE_OLLAMA_MODEL.to_string(),
                protocol: "ollama".to_string(),
                api_key: None,
            }];
        }

        let session_id = 9101;
        let turns = load_dataset(100);
        assert_eq!(turns.len(), 100, "Must seed exactly 100 turns from dataset");
        let conn = state.db.connect().unwrap();
        seed_turns(&conn, session_id, &turns).await;
        state.pipeline.set_state(InteractionState::Ready);

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

        let parsed = parse_unified_compaction_json(&run.compaction_output)
            .expect("Must be valid 6-key JSON");
        let total_facts = parsed.personal.len()
            + parsed.objective.len()
            + parsed.workdone.len()
            + parsed.blocker.len()
            + parsed.next_step.len()
            + parsed.pitfall.len();
        assert!(total_facts > 0, "6-key schema must contain extracted facts");

        let mut q_rows = conn.query(
        "SELECT COUNT(*) FROM memory_ingestion_queue WHERE session_id = ? AND status = 'pending'",
        (session_id,),
    ).await.unwrap();
        let count: i64 = q_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(count as u32, summary.facts_enqueued);
    })
    .await
    .expect("compaction_slice_100_turns_against_a_remote_ollama_server timed out");
}
