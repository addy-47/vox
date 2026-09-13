//! ============================================================================
//! notifications_crud_test.rs — Notifications & Compactions SQLite CRUD Integration Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : persistence/{notifications,compactions,schema,db}
//! Prerequisites: SQLite in-memory / tempdir
//! Execution    : cargo nextest run --test notifications_crud_test --release --nocapture --test-threads=1
//! Metrics      : Schema migrations, CRUD lifecycle, compaction status tracking
//! ============================================================================

use std::{collections::HashMap, time::Duration};

use tempfile::tempdir;
use vox_lib::persistence::{
    compactions::{
        commit_compaction_results, fetch_latest_compaction_run, fetch_turns_for_compaction,
        fetch_uncompacted_sessions, record_compaction_finish, record_compaction_start,
    },
    db::VoxDb,
    notifications::{
        create_notification, dismiss_interactive_by_entity, dismiss_notification,
        dismiss_notifications, fetch_active_notifications, find_active_interactive_by_group,
        find_notification_by_group, mark_all_notifications_read, mark_notifications_read,
        resolve_notification_in_place, NewNotification, NotificationFilter, Severity,
        update_interactive_notification,
    },
    schema::run_migrations,
    sessions::{create_session, create_session_with_id},
};

#[tokio::test]
async fn test_notifications_crud_lifecycle() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_notifs_crud.db");

        let db = VoxDb::open(&db_path)
            .await
            .expect("Failed to open database connection");
        let conn = db.connect().expect("Failed to vend connection");

        run_migrations(&conn)
            .await
            .expect("Failed to run schema migrations");

        // 1. Initially empty
        let active = fetch_active_notifications(&conn)
            .await
            .expect("Failed to fetch active");
        assert!(active.is_empty(), "Initial notifications should be empty");

        // 2. Create notification with Schema v4 fields
        let session_id = create_session(&conn, None)
            .await
            .expect("Failed to create session");
        let group_key = format!("compaction:{}", session_id);
        let notif1 = NewNotification {
            id: "notif_1".to_string(),
            group_key: group_key.clone(),
            category: "compaction".to_string(),
            severity: Severity::Info,
            action_type: "interactive".to_string(),
            action_payload: format!("{{\"action\":\"compact\",\"sessionId\":{session_id}}}"),
            title: "Session Finished".to_string(),
            message: "Session #1 has 5 uncompacted turns".to_string(),
            status: "unread".to_string(),
            session_id: Some(session_id),
            metadata: "{\"uncompacted_turns\": 5}".to_string(),
        };
        let rec1 = create_notification(&conn, &notif1)
            .await
            .expect("Failed to create notification 1");
        assert_eq!(rec1.id, "notif_1");
        assert_eq!(rec1.group_key, group_key);
        assert_eq!(rec1.status, "unread");
        assert_eq!(rec1.severity, Severity::Info);
        assert_eq!(rec1.action_type, "interactive");

        let notif2 = NewNotification {
            id: "notif_2".to_string(),
            group_key: "device_change".to_string(),
            category: "device".to_string(),
            severity: Severity::Warning,
            action_type: "dismiss".to_string(),
            action_payload: "{}".to_string(),
            title: "Audio Device Changed".to_string(),
            message: "Switched to Headset".to_string(),
            status: "unread".to_string(),
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
        assert!(active.iter().all(|n| n.status == "read"));

        // 5. In-place interactive idempotency update
        let active_task = find_active_interactive_by_group(&conn, &group_key)
            .await
            .expect("Failed to query active interactive task");
        assert!(active_task.is_some());
        assert_eq!(active_task.as_ref().unwrap().id, "notif_1");

        update_interactive_notification(
            &conn,
            "notif_1",
            "Session #1 has 8 uncompacted turns",
            "{\"uncompacted_turns\": 8}",
        )
        .await
        .expect("Failed to update interactive notification in place");

        let updated_task = find_active_interactive_by_group(&conn, &group_key)
            .await
            .expect("Failed to query updated interactive task")
            .expect("Interactive task missing");
        assert_eq!(updated_task.message, "Session #1 has 8 uncompacted turns");
        assert_eq!(updated_task.metadata, "{\"uncompacted_turns\": 8}");

        // 6. Dismiss interactive by entity group key
        dismiss_interactive_by_entity(&conn, &group_key)
            .await
            .expect("Failed to dismiss interactive by entity");
        let active_after_entity_dismiss = find_active_interactive_by_group(&conn, &group_key)
            .await
            .expect("Failed to query after dismiss");
        assert!(active_after_entity_dismiss.is_none());

        // 7. Dismiss remaining notification by ID
        dismiss_notification(&conn, "notif_2")
            .await
            .expect("Failed to dismiss notif_2");
        let remaining = fetch_active_notifications(&conn)
            .await
            .expect("Failed to fetch active");
        assert!(remaining.is_empty(), "All active notifications should be dismissed");
    })
    .await
    .expect("test_notifications_crud_lifecycle timed out");
}

#[tokio::test]
async fn test_compaction_ledger_queries_and_mutations() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_compaction_ledger.db");

    let db = VoxDb::open(&db_path)
        .await
        .expect("Failed to open database connection");
    let conn = db.connect().expect("Failed to vend connection");

    run_migrations(&conn)
        .await
        .expect("Failed to run schema migrations");

    // Insert a dummy session and 3 turns
    create_session_with_id(&conn, 100, Some("default"))
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
    let turns = fetch_turns_for_compaction(&conn, 100, 0, u32::MAX)
        .await
        .expect("Failed to fetch turns");
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].user_text, "Hello");
    assert_eq!(turns[1].user_text, "My name is Alice");
    assert_eq!(turns[2].user_text, "I love Rust");

    // Record compaction start
    let run_id = record_compaction_start(&conn, 100, "manual", 1, 3)
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
    record_compaction_finish(&conn, fail_run_id, "", "failed", Some("LLM timed out"))
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

#[tokio::test]
async fn test_notifications_inplace_resolution_and_tab_filtering() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let db_path = dir.path().join("test_notifs_inplace.db");
        let db = VoxDb::open(&db_path)
            .await
            .expect("Failed to open db");
        let conn = db.connect().expect("Failed to connect");
        run_migrations(&conn).await.expect("Failed migrations");

        // Create 1 interactive task card and 1 passive receipt card
        let task = NewNotification {
            id: "task_1".to_string(),
            group_key: "session_compaction:42".to_string(),
            category: "session_compaction".to_string(),
            severity: Severity::Info,
            action_type: "interactive".to_string(),
            action_payload: "{\"action\":\"compact\"}".to_string(),
            title: "Session #42 Ready to Compact".to_string(),
            message: "5 uncompacted turns".to_string(),
            status: "unread".to_string(),
            session_id: Some(42),
            metadata: "{\"uncompacted_turns\": 5, \"resolution\": \"pending\"}".to_string(),
        };
        create_notification(&conn, &task).await.expect("Failed creating task");

        let receipt = NewNotification {
            id: "receipt_1".to_string(),
            group_key: "memory_consolidation:daily".to_string(),
            category: "memory_consolidation".to_string(),
            severity: Severity::Info,
            action_type: "receipt".to_string(),
            action_payload: "{}".to_string(),
            title: "Memory Consolidated".to_string(),
            message: "Daily profile updated".to_string(),
            status: "unread".to_string(),
            session_id: None,
            metadata: "{}".to_string(),
        };
        create_notification(&conn, &receipt).await.expect("Failed creating receipt");

        // Verify find_notification_by_group works
        let found = find_notification_by_group(&conn, "session_compaction:42")
            .await
            .expect("Query failed")
            .expect("Should find task");
        assert_eq!(found.id, "task_1");

        // Test in-place resolution (zero ghost receipts)
        let resolved = resolve_notification_in_place(
            &conn,
            "task_1",
            "resolved",
            Some("Session #42 compacted: extracted 3 facts."),
        )
        .await
        .expect("Resolution failed")
        .expect("Record should exist");

        assert_eq!(resolved.message, "Session #42 compacted: extracted 3 facts.");
        assert!(resolved.metadata.contains("\"resolution\":\"resolved\""));
        assert_eq!(resolved.status, "unread"); // Invariant: Attention status untouched by background task!

        // Test tab-scoped mark as read (interactive only)
        mark_notifications_read(
            &conn,
            Some(&NotificationFilter {
                action_type: Some("interactive".to_string()),
                ..Default::default()
            }),
        )
        .await
        .expect("Failed mark read");

        let active = fetch_active_notifications(&conn).await.expect("Fetch failed");
        let task_rec = active.iter().find(|n| n.id == "task_1").unwrap();
        let receipt_rec = active.iter().find(|n| n.id == "receipt_1").unwrap();
        assert_eq!(task_rec.status, "read");
        assert_eq!(receipt_rec.status, "unread"); // Receipt untouched!

        // Test tab-scoped dismiss (receipt only)
        dismiss_notifications(
            &conn,
            Some(&NotificationFilter {
                action_type: Some("receipt".to_string()),
                ..Default::default()
            }),
        )
        .await
        .expect("Failed dismiss");

        let remaining = fetch_active_notifications(&conn).await.expect("Fetch failed");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "task_1"); // Task still active in Tasks/Updates!

        // User dismissal sovereignty: task is dismissed
        dismiss_notification(&conn, "task_1").await.expect("Failed dismiss task");
        let found_dismissed = find_notification_by_group(&conn, "session_compaction:42")
            .await
            .expect("Query failed")
            .expect("Should find task even if dismissed");
        assert_eq!(found_dismissed.status, "dismissed");
    })
    .await
    .expect("test_notifications_inplace_resolution_and_tab_filtering timed out");
}
