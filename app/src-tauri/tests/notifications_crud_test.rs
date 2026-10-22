//! ============================================================================
//! notifications_crud_test.rs — notification records are created, updated, resolved, filtered and routed to the right delivery channel without duplicating rows
//! ============================================================================
//! Category  : Integration (Seam 18 — notification centre)
//! Component : src/services/notifications/{service,router,actions,types}.rs, src/persistence/notifications.rs, src/persistence/compactions.rs
//! Binary    : notifications_crud_test
//! Prereqs   : none (isolated temporary VOX_HOME per test)
//! Run       : cargo nextest run --test notifications_crud_test --release --test-threads=1
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

/// PURPOSE:  A notification handed to the store must be readable back with
///           its group key, severity and action type intact; the frontend
///           routes cards on those fields.
/// SCENARIO: Create an interactive task notification and a passive receipt
///           notification, then list the active records.
/// ASSERTS:  Both rows exist, the task card keeps its `group_key`,
///           `severity`, `action_type` and `status`, and there are exactly two
///           active records.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop `group_key` from the insert in `create_notification` — the
///           `group_key` assertion would fail.
#[tokio::test]
async fn created_notification_is_persisted_with_its_fields() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        let active = fetch_active_notifications(&conn)
            .await
            .expect("Failed to fetch active");
        assert!(active.is_empty(), "Initial notifications must be empty");

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
    })
    .await
    .expect("created_notification_is_persisted_with_its_fields timed out");
}

/// PURPOSE:  A compaction card that grows must be updated in place; creating
///           a second card for the same group would spam the drawer with
///           duplicates.
/// SCENARIO: Create an interactive card, then update it through
///           `update_interactive_notification` and read it back by group.
/// ASSERTS:  The message is the updated one and the card is still the only
///           active interactive card in that group.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make `update_interactive_notification` insert a new row instead
///           of updating — the message assertion would fail.
#[tokio::test]
async fn interactive_notification_updates_in_place() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

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
        create_notification(&conn, &task_notif)
            .await
            .expect("Failed to create task notification");

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
        assert_eq!(
            updated_task.id, "notif_task_1",
            "the update must not create a second card in the same group"
        );
    })
    .await
    .expect("interactive_notification_updates_in_place timed out");
}

/// PURPOSE:  Resolving a card in place must flip the metadata resolution and
///           keep the record unread, so the user still sees the outcome.
/// SCENARIO: Resolve an interactive card through
///           `resolve_notification_in_place` and inspect the result.
/// ASSERTS:  The message is the resolution text, the metadata carries
///           `"resolution":"resolved"` and the status stays `unread`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Set the resolved record's status to `read` — the status
///           assertion would fail.
#[tokio::test]
async fn notification_resolution_preserves_unread_status() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        let session_id = create_session(&conn, None)
            .await
            .expect("Failed to create session");
        let task_notif = NewNotification {
            id: "notif_task_1".to_string(),
            group_key: format!("session_compaction:{}", session_id),
            category: "session_compaction".to_string(),
            severity: Severity::Info,
            action_type: "interactive".to_string(),
            action_payload: "{}".to_string(),
            title: "Session Finished".to_string(),
            message: "Session has 5 uncompacted turns".to_string(),
            status: "unread".to_string(),
            session_id: Some(session_id),
            metadata: "{\"resolution\": \"pending\"}".to_string(),
        };
        create_notification(&conn, &task_notif)
            .await
            .expect("Failed to create task notification");

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
    })
    .await
    .expect("notification_resolution_preserves_unread_status timed out");
}

/// PURPOSE:  "Mark as read" is applied per tab; marking the interactive tab
///           must not silently mark the receipt tab as read too.
/// SCENARIO: Create one interactive and one receipt notification, then mark
///           only the interactive ones read.
/// ASSERTS:  The interactive record becomes `read` and the receipt record
///           stays `unread`.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Ignore the `action_type` filter in `mark_notifications_read` —
///           the receipt assertion would fail.
#[tokio::test]
async fn mark_read_applies_only_to_the_filtered_action_type() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        let task_notif = NewNotification {
            id: "notif_task_1".to_string(),
            group_key: "session_compaction:1".to_string(),
            category: "session_compaction".to_string(),
            severity: Severity::Info,
            action_type: "interactive".to_string(),
            action_payload: "{}".to_string(),
            title: "Session Finished".to_string(),
            message: "Session has 5 uncompacted turns".to_string(),
            status: "unread".to_string(),
            session_id: Some(1),
            metadata: "{}".to_string(),
        };
        create_notification(&conn, &task_notif)
            .await
            .expect("Failed to create task notification");

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
    })
    .await
    .expect("mark_read_applies_only_to_the_filtered_action_type timed out");
}

/// PURPOSE:  Dismissal is applied per tab; dismissing the receipt tab must
///           leave the interactive card in the drawer.
/// SCENARIO: Create one interactive and one receipt notification, then dismiss
///           only the receipts.
/// ASSERTS:  Exactly one active record remains and it is the interactive
///           task card.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Ignore the `action_type` filter in `dismiss_notifications` — the
///           remaining-count assertion would fail.
#[tokio::test]
async fn dismiss_applies_only_to_the_filtered_action_type() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        let task_notif = NewNotification {
            id: "notif_task_1".to_string(),
            group_key: "session_compaction:1".to_string(),
            category: "session_compaction".to_string(),
            severity: Severity::Info,
            action_type: "interactive".to_string(),
            action_payload: "{}".to_string(),
            title: "Session Finished".to_string(),
            message: "Session has 5 uncompacted turns".to_string(),
            status: "unread".to_string(),
            session_id: Some(1),
            metadata: "{}".to_string(),
        };
        create_notification(&conn, &task_notif)
            .await
            .expect("Failed to create task notification");

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
    })
    .await
    .expect("dismiss_applies_only_to_the_filtered_action_type timed out");
}

/// PURPOSE:  Dismissing by entity group must remove the interactive card for
///           that entity and leave the drawer empty; a no-op here leaves
///           resolved cards visible forever.
/// SCENARIO: Dismiss the interactive notifications of one group key and list
///           the remaining active records.
/// ASSERTS:  No active notifications remain.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make `dismiss_interactive_by_entity` a no-op — the emptiness
///           assertion would fail.
#[tokio::test]
async fn dismiss_interactive_by_entity_clears_the_group() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

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
            action_payload: "{}".to_string(),
            title: "Session Finished".to_string(),
            message: "Session has 5 uncompacted turns".to_string(),
            status: "unread".to_string(),
            session_id: Some(session_id),
            metadata: "{}".to_string(),
        };
        create_notification(&conn, &task_notif)
            .await
            .expect("Failed to create task notification");

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
    .expect("dismiss_interactive_by_entity_clears_the_group timed out");
}

/// PURPOSE:  A transient alert must never reach the notification drawer; it
///           is a toast with a duration, not a record.
/// SCENARIO: Resolve the delivery channel for a transient action at two
///           different impact/severity combinations.
/// ASSERTS:  Both resolve to `DeliveryChannel::ToastOnly`.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Let `resolve_channel` fall through to the interactive branch for
///           `Action::Transient` — the channel assertion would fail.
#[test]
fn transient_action_resolves_to_toast_only() {
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
}

/// PURPOSE:  A receipt is a persistent record with no user action; routing it
///           to a toast would make it disappear unread.
/// SCENARIO: Resolve the delivery channel for a receipt action.
/// ASSERTS:  The channel is `DeliveryChannel::NotificationOnly`.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Return `ToastOnly` for `Action::Receipt` — the channel
///           assertion would fail.
#[test]
fn receipt_action_resolves_to_notification_only() {
    assert_eq!(
        resolve_channel(None, Severity::Info, &Action::Receipt),
        DeliveryChannel::NotificationOnly
    );
}

/// PURPOSE:  An interactive card only earns a toast when it is urgent; a
///           routine card must stay in the drawer without interrupting.
/// SCENARIO: Resolve the delivery channel for an interactive action at
///           several impact/severity combinations.
/// ASSERTS:  Routine `Info` resolves to `NotificationOnly`, while critical
///           severity, a halted session and an aborted turn all resolve to
///           `ToastAndNotification`.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Always return `NotificationOnly` for interactive actions — the
///           critical-severity assertion would fail.
#[test]
fn interactive_action_routes_to_toast_only_when_urgent() {
    let interactive_action = Action::Interactive(ActionPayload::CompactSession { session_id: 10 });

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
}

/// PURPOSE:  The Zero-DB invariant: a transient alert must not create a row
///           in the notifications table; persistent toasts would grow the
///           database without bound.
/// SCENARIO: Dispatch a transient notification through the universal front
///           door and then count the rows.
/// ASSERTS:  `notify` returns no persistent ID and the notifications table
///           has zero rows.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Persist transient notifications before deciding the channel —
///           the row-count assertion would fail.
#[tokio::test]
async fn transient_notification_writes_no_database_row() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

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

        assert!(
            result.is_none(),
            "Transient notification must return None for persistent ID"
        );

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
    .expect("transient_notification_writes_no_database_row timed out");
}

/// PURPOSE:  A second notification for the same entity must roll up into the
///           existing card; inserting a duplicate floods the drawer with the
///           same task.
/// SCENARIO: Dispatch two interactive notifications with the same group key
///           and different content, then list the active records.
/// ASSERTS:  The second dispatch returns the first notification's ID, there
///           is still exactly one active record, and its message and metadata
///           carry the newest content.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Always insert a new row in `notify` for a group key — the
///           active-count assertion would fail.
#[tokio::test]
async fn group_key_rollup_updates_existing_notification_in_place() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        let group_key = "session_compaction:18401";

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
        assert_eq!(
            active_1.len(),
            1,
            "Must have exactly 1 active notification after first dispatch"
        );
        assert_eq!(active_1[0].id, id_1);
        assert_eq!(active_1[0].message, "Session has 5 uncompacted turns");

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

        assert_eq!(
            id_2, id_1,
            "Group-key rollup must return the existing notification ID"
        );

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
    .expect("group_key_rollup_updates_existing_notification_in_place timed out");
}

/// PURPOSE:  The compaction ledger must list sessions that still have
///           uncompacted turns; an empty list means the user never gets the
///           "this session needs compaction" card.
/// SCENARIO: Create a session with three turns and query the uncompacted
///           sessions and the pending compaction turns.
/// ASSERTS:  The session is listed with turn count 3 and last compacted turn
///           id 0, and three turns are pending for compaction.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Return an empty list from `fetch_uncompacted_sessions` — the
///           listing assertion would fail.
#[tokio::test]
async fn compaction_ledger_lists_uncompacted_session_and_its_turns() {
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

        let uncompacted = fetch_uncompacted_sessions(&conn)
            .await
            .expect("Failed to fetch uncompacted");
        assert_eq!(uncompacted.len(), 1);
        assert_eq!(uncompacted[0].session_id, session_id);
        assert_eq!(uncompacted[0].turn_count, 3);
        assert_eq!(uncompacted[0].last_compacted_turn_id, 0);

        let turns = fetch_turns_for_compaction(&conn, session_id, 0, u32::MAX)
            .await
            .expect("Failed to fetch turns");
        assert_eq!(turns.len(), 3);
    })
    .await
    .expect("compaction_ledger_lists_uncompacted_session_and_its_turns timed out");
}

/// PURPOSE:  After a compaction is committed the session must leave the
///           uncompacted list; otherwise the app nags the user about a
///           session it already compacted.
/// SCENARIO: Record a compaction run for the session, commit its facts, then
///           re-query the uncompacted sessions.
/// ASSERTS:  Two facts are committed and the session is no longer listed.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Do not advance the compaction watermark in
///           `commit_compaction_results` — the empty-list assertion would
///           fail.
#[tokio::test]
async fn committed_compaction_removes_session_from_uncompacted_list() {
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

        let run_id = record_compaction_start(&conn, session_id, "manual", 1, 3)
            .await
            .expect("Failed to record start");
        assert!(run_id > 0);

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

        let uncompacted_after = fetch_uncompacted_sessions(&conn)
            .await
            .expect("Failed to fetch uncompacted");
        assert!(
            uncompacted_after.is_empty(),
            "Session should now be fully compacted"
        );
    })
    .await
    .expect("committed_compaction_removes_session_from_uncompacted_list timed out");
}

/// PURPOSE:  A failed compaction must be recorded with its error message; a
///           run that is silently dropped hides the reason the user's
///           session never compacts.
/// SCENARIO: Record a compaction run, finish it with status `failed` and an
///           error message, then read the latest run.
/// ASSERTS:  The latest run has status `failed` and the recorded error
///           message.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Drop the `error_msg` write in `record_compaction_finish` — the
///           error-message assertion would fail.
#[tokio::test]
async fn failed_compaction_run_records_its_error_message() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, _app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

        let session_id = 18202i64;
        create_session_with_id(&conn, session_id, Some("default"))
            .await
            .expect("Failed to insert session");

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
    .expect("failed_compaction_run_records_its_error_message timed out");
}

/// PURPOSE:  Executing a notification action must resolve the card in place
///           and describe the retry; a card that stays `pending` makes the
///           user believe the action never ran.
/// SCENARIO: Create a retry card, execute the action through the production
///           entry seam, then read the record back.
/// ASSERTS:  The metadata resolution becomes `resolved` and the message names
///           the retried operation.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the in-place resolution in `execute_notification_action` —
///           the metadata assertion would fail.
#[tokio::test]
async fn retry_action_execution_resolves_the_card_in_place() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (_guard, app, state) = common::harness::setup_isolated_app_state().await;
        let conn = state.db.connect().expect("Failed to connect to test db");

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

        execute_notification_action(&app, &state, notif_id)
            .await
            .expect("execute_notification_action must succeed");

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
    .expect("retry_action_execution_resolves_the_card_in_place timed out");
}
