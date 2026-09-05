//! ============================================================================
//! notifications_crud_test.rs — Notifications & Compactions SQLite CRUD Integration Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : persistence/{notifications,compactions,schema,db}
//! Prerequisites: SQLite in-memory / tempdir
//! Execution    : cargo nextest run --test notifications_crud_test --release --nocapture --test-threads=1
//! Metrics      : Schema migrations, CRUD lifecycle, compaction status tracking
//! ============================================================================

use std::time::Duration;
use std::collections::HashMap;
use tempfile::tempdir;
use vox_lib::persistence::compactions::{
    commit_compaction_results, fetch_latest_compaction_run, fetch_turns_for_compaction,
    fetch_uncompacted_sessions, record_compaction_finish, record_compaction_start,
};
use vox_lib::persistence::db::VoxDb;
use vox_lib::persistence::notifications::{
    create_notification, dismiss_notification, fetch_active_notifications,
    find_active_notification_by_session, mark_all_notifications_read, update_notification_status,
    NewNotification,
};
use vox_lib::persistence::schema::run_migrations;

#[tokio::test]
async fn test_notifications_crud_lifecycle() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_notifs_crud.db");

    let conn = VoxDb::open(&db_path)
        .await
        .expect("Failed to open database connection");

    run_migrations(&conn)
        .await
        .expect("Failed to run schema migrations");

    // 1. Initially empty
    let active = fetch_active_notifications(&conn)
        .await
        .expect("Failed to fetch active");
    assert!(active.is_empty(), "Initial notifications should be empty");

    // 2. Create notification
    let notif1 = NewNotification {
        id: "notif_1".to_string(),
        category: "session_compaction".to_string(),
        title: "Session Finished".to_string(),
        message: "Session #1 has 5 uncompacted turns".to_string(),
        status: "pending".to_string(),
        session_id: Some(1),
        metadata: "{\"uncompacted_turns\": 5}".to_string(),
    };
    let rec1 = create_notification(&conn, &notif1)
        .await
        .expect("Failed to create notification 1");
    assert_eq!(rec1.id, "notif_1");
    assert!(!rec1.is_read);
    assert_eq!(rec1.status, "pending");

    let notif2 = NewNotification {
        id: "notif_2".to_string(),
        category: "system_alert".to_string(),
        title: "Audio Device Changed".to_string(),
        message: "Switched to Headset".to_string(),
        status: "pending".to_string(),
        session_id: None,
        metadata: "{}".to_string(),
    };
    create_notification(&conn, &notif2)
        .await
        .expect("Failed to create notification 2");

    // 3. Fetch active: should have 2 notifications
    let active = fetch_active_notifications(&conn)
        .await
        .expect("Failed to fetch active");
    assert_eq!(active.len(), 2);

    // 4. Mark all read
    mark_all_notifications_read(&conn)
        .await
        .expect("Failed to mark read");
    let active = fetch_active_notifications(&conn)
        .await
        .expect("Failed to fetch active");
    assert!(active.iter().all(|n| n.is_read));

    // 5. Update status
    update_notification_status(&conn, "notif_1", "in_progress")
        .await
        .expect("Failed to update status");
    let found = find_active_notification_by_session(&conn, 1, "session_compaction")
        .await
        .expect("Failed to find by session");
    assert!(found.is_some());
    assert_eq!(found.unwrap().status, "in_progress");

    // 6. Dismiss
    dismiss_notification(&conn, "notif_2")
        .await
        .expect("Failed to dismiss");
    let active = fetch_active_notifications(&conn)
        .await
        .expect("Failed to fetch active");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "notif_1");
    })
    .await
    .expect("test_notifications_crud_lifecycle timed out");
}

#[tokio::test]
async fn test_compaction_ledger_queries_and_mutations() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_compaction_ledger.db");

    let conn = VoxDb::open(&db_path)
        .await
        .expect("Failed to open database connection");

    run_migrations(&conn)
        .await
        .expect("Failed to run schema migrations");

    // Insert a dummy session and 3 turns
    conn.execute(
        "INSERT INTO sessions (id, started_at, turn_count) VALUES (100, 1000, 3)",
        (),
    )
    .await
    .expect("Failed to insert session");

    conn.execute(
        "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (100, 1, 'Hello', 'Hi there', 1001)",
        (),
    )
    .await
    .expect("Failed to insert turn 1");

    conn.execute(
        "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (100, 2, 'My name is Alice', 'Nice to meet you Alice', 1002)",
        (),
    )
    .await
    .expect("Failed to insert turn 2");

    conn.execute(
        "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (100, 3, 'I love Rust', 'Rust is awesome', 1003)",
        (),
    )
    .await
    .expect("Failed to insert turn 3");

    // Verify session 100 is detected as uncompacted
    let uncompacted = fetch_uncompacted_sessions(&conn)
        .await
        .expect("Failed to fetch uncompacted");
    assert_eq!(uncompacted.len(), 1);
    assert_eq!(uncompacted[0].session_id, 100);
    assert_eq!(uncompacted[0].turn_count, 3);
    assert_eq!(uncompacted[0].last_compacted_turn_id, 0);

    // Fetch turns for compaction
    let turns = fetch_turns_for_compaction(&conn, 100, 0)
        .await
        .expect("Failed to fetch turns");
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].user_text, "Hello");
    assert_eq!(turns[1].user_text, "My name is Alice");
    assert_eq!(turns[2].user_text, "I love Rust");

    // Record compaction start
    let run_id = record_compaction_start(&conn, 100, "session_end", 1, 3)
        .await
        .expect("Failed to record start");
    assert!(run_id > 0);

    // Commit compaction results
    let mut facts = HashMap::new();
    facts.insert(
        "Identity".to_string(),
        vec!["User's name is Alice".to_string()],
    );
    facts.insert(
        "Preferences".to_string(),
        vec!["User loves Rust".to_string()],
    );

    let committed_count = commit_compaction_results(
        &conn,
        run_id,
        "100",
        "Conversation about user name and programming language preference.",
        facts,
        true,
    )
    .await
    .expect("Failed to commit compaction results");
    assert_eq!(committed_count, 2);

    // Verify session 100 is no longer uncompacted!
    let uncompacted_after = fetch_uncompacted_sessions(&conn)
        .await
        .expect("Failed to fetch uncompacted");
    assert!(
        uncompacted_after.is_empty(),
        "Session 100 should now be fully compacted"
    );

    // Record a failed compaction attempt and verify error is recorded
    let fail_run_id = record_compaction_start(&conn, 100, "manual", 4, 5)
        .await
        .expect("Failed to record start");
    record_compaction_finish(&conn, fail_run_id, "failed", 0, Some("LLM timed out"))
        .await
        .expect("Failed to record finish");

    let latest = fetch_latest_compaction_run(&conn, 100)
        .await
        .expect("Failed to fetch latest")
        .expect("Expected latest run");
    assert_eq!(latest.status, "failed");
    assert_eq!(latest.error_msg.as_deref(), Some("LLM timed out"));
    })
    .await
    .expect("test_compaction_ledger_queries_and_mutations timed out");
}
