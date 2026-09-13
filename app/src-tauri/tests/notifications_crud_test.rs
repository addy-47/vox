//! ============================================================================
//! tests/notifications_crud_test.rs — Notification Center 3D Matrix Routing, Aggregation & Actions Integration Tests
//! ============================================================================
//! Category     : Integration Test (Seam 18)
//! Component    : services/notifications/{service,router,actions,types}, persistence/notifications, persistence/compactions
//! Prerequisites: Turso SQLite engine (vox.db), isolated TempPathsGuard
//! Execution    : cargo nextest run --test notifications_crud_test --release --nocapture --test-threads=1
//! Metrics      : 3D routing channel resolution, Zero-DB invariant for transient alerts,
//!                group-key rollup deduplication, action execution engine, SQLite CRUD & resolution
//! ============================================================================

mod common;

use std::{collections::HashMap, time::Duration};

use vox_lib::{
    core::{error::PipelineImpact, events::Severity},
    persistence::{
        compactions::{
            commit_compaction_results, fetch_latest_compaction_run, fetch_turns_for_compaction,
            fetch_uncompacted_sessions, record_compaction_finish, record_compaction_start,
        },
        notifications::{
            create_notification, dismiss_interactive_by_entity, dismiss_notifications,
            fetch_active_notifications, fetch_notification_by_id, find_active_interactive_by_group,
            mark_notifications_read, resolve_notification_in_place,
            update_interactive_notification, NewNotification, NotificationFilter,
        },
        sessions::{create_session, create_session_with_id},
    },
    services::notifications::{
        execute_notification_action, notify, resolve_channel, Action, ActionPayload,
        DeliveryChannel, NotificationCategory, NotificationParams,
    },
};

// ============================================================================
// Subtest 1: Persistence CRUD, In-Place Resolution & Tab Filtering
// ============================================================================
/// Verifies full SQLite CRUD lifecycle, Schema v4 fields (group_key, action_type, severity),
/// in-place resolution metadata mutations, tab-scoped mark as read, and scoped dismissal.
#[tokio::test]
async fn test_notifications_persistence_crud_and_in_place_resolution() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        // 1. Initially empty
        let active = fetch_active_notifications(&conn)
            .await
            .expect("Failed to fetch active");
        assert!(active.is_empty(), "Initial notifications must be empty");

        // 2. Create interactive task notification
        let session_id = create_session(&conn, None)
            .await
            .expect("Failed to create session");
        let group_key = format!("session_compaction:{}", session_id);
        let task_notif = NewNotification {
            id: "notif_task_1".to_string(),
            group_key: group_key.clone(),
            category: "session_compaction".to_string(),
            severity: Severity::Info,
            action_type: "interactive".to_string(),
            action_payload: format!("{{\"action\":\"compact\",\"sessionId\":{session_id}}}"),
            title: "Session Finished".to_string(),
            message: "Session has 5 uncompacted turns".to_string(),
            status: "unread".to_string(),
            session_id: Some(session_id),
            metadata: "{\"uncompacted_turns\": 5, \"resolution\": \"pending\"}".to_string(),
        };
        let rec1 = create_notification(&conn, &task_notif)
            .await
            .expect("Failed to create task notification");
        assert_eq!(rec1.id, "notif_task_1");
        assert_eq!(rec1.group_key, group_key);
        assert_eq!(rec1.status, "unread");
        assert_eq!(rec1.severity, Severity::Info);
        assert_eq!(rec1.action_type, "interactive");

        // 3. Create passive receipt notification
        let receipt_notif = NewNotification {
            id: "notif_receipt_1".to_string(),
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
        create_notification(&conn, &receipt_notif)
            .await
            .expect("Failed to create receipt notification");

        let active = fetch_active_notifications(&conn)
            .await
            .expect("Failed to fetch active");
        assert_eq!(active.len(), 2, "Must have exactly 2 active notifications");

        // 4. In-place interactive update
        let active_task = find_active_interactive_by_group(&conn, &group_key)
            .await
            .expect("Failed to query active interactive task");
        assert!(active_task.is_some());
        assert_eq!(active_task.as_ref().unwrap().id, "notif_task_1");

        update_interactive_notification(
            &conn,
            "notif_task_1",
            "Session has 8 uncompacted turns",
            "{\"uncompacted_turns\": 8, \"resolution\": \"pending\"}",
        )
        .await
        .expect("Failed to update interactive notification in place");

        let updated_task = find_active_interactive_by_group(&conn, &group_key)
            .await
            .expect("Failed to query updated interactive task")
            .expect("Interactive task missing");
        assert_eq!(updated_task.message, "Session has 8 uncompacted turns");

        // 5. In-place resolution (preserves unread status for user attention)
        let resolved = resolve_notification_in_place(
            &conn,
            "notif_task_1",
            "resolved",
            Some("Session compacted: extracted 4 facts."),
        )
        .await
        .expect("Resolution failed")
        .expect("Record should exist");
        assert_eq!(resolved.message, "Session compacted: extracted 4 facts.");
        assert!(resolved.metadata.contains("\"resolution\":\"resolved\""));
        assert_eq!(resolved.status, "unread");

        // 6. Tab-scoped mark read (interactive only)
        mark_notifications_read(
            &conn,
            Some(&NotificationFilter {
                action_type: Some("interactive".to_string()),
                ..Default::default()
            }),
        )
        .await
        .expect("Failed mark read");

        let active = fetch_active_notifications(&conn)
            .await
            .expect("Fetch failed");
        let task_rec = active.iter().find(|n| n.id == "notif_task_1").unwrap();
        let receipt_rec = active.iter().find(|n| n.id == "notif_receipt_1").unwrap();
        assert_eq!(task_rec.status, "read");
        assert_eq!(
            receipt_rec.status, "unread",
            "Receipt status must be untouched"
        );

        // 7. Tab-scoped dismiss (receipt only)
        dismiss_notifications(
            &conn,
            Some(&NotificationFilter {
                action_type: Some("receipt".to_string()),
                ..Default::default()
            }),
        )
        .await
        .expect("Failed dismiss receipts");

        let remaining = fetch_active_notifications(&conn)
            .await
            .expect("Fetch failed");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "notif_task_1");

        // 8. Dismiss interactive by entity group key
        dismiss_interactive_by_entity(&conn, &group_key)
            .await
            .expect("Failed to dismiss interactive by entity");
        let remaining_after = fetch_active_notifications(&conn)
            .await
            .expect("Fetch failed");
        assert!(
            remaining_after.is_empty(),
            "All notifications must now be dismissed"
        );
    })
    .await
    .expect("test_notifications_persistence_crud_and_in_place_resolution timed out");
}

// ============================================================================
// Subtest 2: Compaction Ledger Queries, Watermarks & Cascades
// ============================================================================
/// Verifies session turn association, uncompacted session queries, compaction start/finish
/// tracking, and error recording.
#[tokio::test]
async fn test_compaction_ledger_queries_and_cascades() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        let session_id = 18201i64;
        create_session_with_id(&conn, session_id, Some("default"))
            .await
            .expect("Failed to insert session");

        conn.execute(
            "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES \
             (?, 1, 'Hello', 'Hi there', 1001), \
             (?, 2, 'My name is Alice', 'Nice to meet you Alice', 1002), \
             (?, 3, 'I love Rust', 'Rust is awesome', 1003)",
            (session_id, session_id, session_id),
        )
        .await
        .expect("Failed to insert turns");

        // 1. Verify session is detected as uncompacted
        let uncompacted = fetch_uncompacted_sessions(&conn)
            .await
            .expect("Failed to fetch uncompacted");
        assert_eq!(uncompacted.len(), 1);
        assert_eq!(uncompacted[0].session_id, session_id);
        assert_eq!(uncompacted[0].turn_count, 3);
        assert_eq!(uncompacted[0].last_compacted_turn_id, 0);

        // 2. Fetch turns for compaction
        let turns = fetch_turns_for_compaction(&conn, session_id, 0, u32::MAX)
            .await
            .expect("Failed to fetch turns");
        assert_eq!(turns.len(), 3);

        // 3. Record compaction start
        let run_id = record_compaction_start(&conn, session_id, "manual", 1, 3)
            .await
            .expect("Failed to record start");
        assert!(run_id > 0);

        // 4. Commit compaction results
        let mut facts = HashMap::new();
        facts.insert(
            "personal".to_string(),
            vec![
                "User's name is Alice".to_string(),
                "User loves Rust".to_string(),
            ],
        );

        let committed_count = commit_compaction_results(
            &conn,
            run_id,
            &session_id.to_string(),
            "Conversation about user name and programming language preference.",
            facts,
            true,
        )
        .await
        .expect("Failed to commit compaction results");
        assert_eq!(committed_count, 2);

        // 5. Session is no longer uncompacted
        let uncompacted_after = fetch_uncompacted_sessions(&conn)
            .await
            .expect("Failed to fetch uncompacted");
        assert!(
            uncompacted_after.is_empty(),
            "Session should now be fully compacted"
        );

        // 6. Record failed compaction attempt and verify error tracking
        let fail_run_id = record_compaction_start(&conn, session_id, "manual", 4, 5)
            .await
            .expect("Failed to record start");
        record_compaction_finish(&conn, fail_run_id, "", "failed", Some("LLM timed out"))
            .await
            .expect("Failed to record finish");

        let latest = fetch_latest_compaction_run(&conn, session_id)
            .await
            .expect("Failed to fetch latest")
            .expect("Expected latest run");
        assert_eq!(latest.status, "failed");
        assert_eq!(latest.error_msg.as_deref(), Some("LLM timed out"));
    })
    .await
    .expect("test_compaction_ledger_queries_and_cascades timed out");
}

// ============================================================================
// Subtest 3: 3D Matrix Routing & Zero-DB Invariant
// ============================================================================
/// Entry Seam A: `services::notifications::notify` & `services::notifications::resolve_channel`
///
/// Verifies:
///   - 3D truth table channel resolution across (Impact, Severity, Action)
///   - Zero-DB Invariant: Action::Transient dispatches to toast overlay without persisting ANY row to SQLite.
#[tokio::test]
async fn test_notifications_3d_routing_and_zero_db_invariant() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        // 1. Truth Table Tests for resolve_channel
        // Transient always resolves to ToastOnly regardless of impact/severity
        assert_eq!(
            resolve_channel(None, Severity::Info, &Action::Transient),
            DeliveryChannel::ToastOnly
        );
        assert_eq!(
            resolve_channel(
                Some(PipelineImpact::SessionHalted),
                Severity::Critical,
                &Action::Transient
            ),
            DeliveryChannel::ToastOnly
        );

        // Receipt always resolves to NotificationOnly
        assert_eq!(
            resolve_channel(None, Severity::Info, &Action::Receipt),
            DeliveryChannel::NotificationOnly
        );

        // Interactive resolves conditionally based on impact / severity
        let interactive_action =
            Action::Interactive(ActionPayload::CompactSession { session_id: 10 });
        assert_eq!(
            resolve_channel(None, Severity::Info, &interactive_action),
            DeliveryChannel::NotificationOnly
        );
        assert_eq!(
            resolve_channel(None, Severity::Critical, &interactive_action),
            DeliveryChannel::ToastAndNotification
        );
        assert_eq!(
            resolve_channel(
                Some(PipelineImpact::SessionHalted),
                Severity::Info,
                &interactive_action
            ),
            DeliveryChannel::ToastAndNotification
        );
        assert_eq!(
            resolve_channel(
                Some(PipelineImpact::TurnAborted),
                Severity::Warning,
                &interactive_action
            ),
            DeliveryChannel::ToastAndNotification
        );

        // 2. Zero-DB Invariant Execution: Dispatch Action::Transient through universal front door
        let transient_params = NotificationParams {
            group_key: Some("transient_alert"),
            category: NotificationCategory::Hardware,
            severity: Severity::Info,
            impact: None,
            action: Action::Transient,
            title: "Microphone Switched",
            message: "Switched to Headset Microphone",
            session_id: None,
            metadata: None,
            duration_ms: Some(3000),
        };

        let result = notify(&app, &state.db, transient_params)
            .await
            .expect("notify must succeed for transient action");

        // Transient notifications do not generate persistent drawer IDs
        assert!(
            result.is_none(),
            "Transient notification must return None for persistent ID"
        );

        // Critical Assertion: Database MUST remain completely empty (Zero-DB Invariant)
        let active_rows = fetch_active_notifications(&conn)
            .await
            .expect("Failed to fetch notifications");
        assert_eq!(
            active_rows.len(),
            0,
            "Zero-DB Invariant Violated: Transient notification was persisted to SQLite"
        );

        let mut all_rows = conn
            .query("SELECT COUNT(*) FROM notifications", ())
            .await
            .expect("Query failed");
        let total_count: i64 = all_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(
            total_count, 0,
            "Total notifications table row count must be 0 after transient alert"
        );
    })
    .await
    .expect("test_notifications_3d_routing_and_zero_db_invariant timed out");
}

// ============================================================================
// Subtest 4: Group-Key Rollup & In-Place Deduplication
// ============================================================================
/// Entry Seam A: `services::notifications::notify` with duplicate `group_key`
///
/// Verifies:
///   - Dispatching an initial interactive notification creates a persistent record.
///   - Dispatching a second interactive notification with the identical `group_key` updates the
///     existing record in-place without creating a duplicate row in SQLite.
#[tokio::test]
async fn test_notifications_group_key_rollup_and_deduplication() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, app, state) =
            common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        let group_key = "session_compaction:18401";

        // 1. First Dispatch: creates initial interactive task card
        let params_1 = NotificationParams {
            group_key: Some(group_key),
            category: NotificationCategory::SessionCompaction,
            severity: Severity::Info,
            impact: None,
            action: Action::Interactive(ActionPayload::CompactSession { session_id: 18401 }),
            title: "Session #18401 Needs Compaction",
            message: "Session has 5 uncompacted turns",
            session_id: Some(18401),
            metadata: Some("{\"uncompacted_turns\": 5, \"resolution\": \"pending\"}"),
            duration_ms: None,
        };

        let id_1 = notify(&app, &state.db, params_1)
            .await
            .expect("Initial notify must succeed")
            .expect("Must return notification ID for interactive action");

        let active_1 = fetch_active_notifications(&conn)
            .await
            .expect("Query failed");
        assert_eq!(active_1.len(), 1, "Must have exactly 1 active notification after first dispatch");
        assert_eq!(active_1[0].id, id_1);
        assert_eq!(active_1[0].message, "Session has 5 uncompacted turns");

        // 2. Second Dispatch: identical group_key with updated turn count
        let params_2 = NotificationParams {
            group_key: Some(group_key),
            category: NotificationCategory::SessionCompaction,
            severity: Severity::Warning,
            impact: None,
            action: Action::Interactive(ActionPayload::CompactSession { session_id: 18401 }),
            title: "Session #18401 Needs Compaction",
            message: "Session has 12 uncompacted turns (high memory)",
            session_id: Some(18401),
            metadata: Some("{\"uncompacted_turns\": 12, \"resolution\": \"pending\"}"),
            duration_ms: None,
        };

        let id_2 = notify(&app, &state.db, params_2)
            .await
            .expect("Second notify must succeed")
            .expect("Must return notification ID");

        // Invariant: returned ID must match the original ID (updated in-place)
        assert_eq!(id_2, id_1, "Group-key rollup must return the existing notification ID");

        // Critical Invariant: Total active notifications count must STILL be 1 (zero duplicate rows)
        let active_2 = fetch_active_notifications(&conn)
            .await
            .expect("Query failed");
        assert_eq!(
            active_2.len(),
            1,
            "Group-key rollup failed: duplicate notification row was inserted instead of in-place update"
        );
        assert_eq!(active_2[0].id, id_1);
        assert_eq!(
            active_2[0].message,
            "Session has 12 uncompacted turns (high memory)",
            "Notification message must be updated to newest content"
        );
        assert!(active_2[0].metadata.contains("\"uncompacted_turns\": 12"));
    })
    .await
    .expect("test_notifications_group_key_rollup_and_deduplication timed out");
}

// ============================================================================
// Subtest 5: Action Execution Engine
// ============================================================================
/// Entry Seam B: `services::notifications::execute_notification_action`
///
/// Verifies:
///   - Polymorphic interactive action execution dispatches targeted remediation.
///   - On completion of `Retry`, the interactive task card is resolved in-place with `resolution: "resolved"`.
#[tokio::test]
async fn test_notification_action_execution_engine() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        // 1. Ingest an interactive retry task card
        let notif_id = "notif_retry_18501";
        let retry_notif = NewNotification {
            id: notif_id.to_string(),
            group_key: "model_load:failed".to_string(),
            category: "models".to_string(),
            severity: Severity::Warning,
            action_type: "interactive".to_string(),
            action_payload: serde_json::to_string(&ActionPayload::Retry {
                operation: "load_model".to_string(),
                resource_id: Some("qwen_gguf".to_string()),
            })
            .unwrap(),
            title: "Model Load Failed".to_string(),
            message: "Failed to allocate memory for Qwen GGUF".to_string(),
            status: "unread".to_string(),
            session_id: None,
            metadata: "{\"resolution\": \"pending\"}".to_string(),
        };

        create_notification(&conn, &retry_notif)
            .await
            .expect("Failed to create retry notification");

        let before = fetch_notification_by_id(&conn, notif_id)
            .await
            .expect("Query failed")
            .expect("Notification must exist");
        assert!(before.metadata.contains("\"resolution\": \"pending\""));

        // 2. Execute the action through the production entry seam
        execute_notification_action(&app, &state, notif_id)
            .await
            .expect("execute_notification_action must succeed");

        // 3. Verify in-place resolution
        let after = fetch_notification_by_id(&conn, notif_id)
            .await
            .expect("Query failed")
            .expect("Notification must exist");

        assert!(
            after.metadata.contains("\"resolution\":\"resolved\""),
            "Action execution must mark resolution as 'resolved' in metadata, got: {}",
            after.metadata
        );
        assert!(
            after.message.contains("Retried operation: load_model"),
            "Action execution must update notification message, got: {}",
            after.message
        );
    })
    .await
    .expect("test_notification_action_execution_engine timed out");
}
