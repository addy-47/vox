# Notification & Toast System Refactor Checklist

---

## Batch 1: Persistence Foundation (Schema v4 & Notification Storage Invariants)
*Depends on: None*

- [x] `app/src-tauri/src/persistence/schema.rs`
  - `SCHEMA_VERSION`: Bump from 3 to 4.
  - `V2_TABLE_STATEMENTS`: Replace `notifications` table DDL with Schema v4 schema (`group_key`, `severity`, `action_type`, `action_payload`, `updated_at`, drop `is_read`).
  - `V2_TABLE_STATEMENTS`: Add indexes `idx_notifications_status_created`, `idx_notifications_group_status`, `idx_notifications_session`.
- [x] `app/src-tauri/src/persistence/notifications.rs`
  - `NewNotification`: Add `group_key`, `severity`, `action_type`, `action_payload`, `updated_at`.
  - `create_notification()`: Insert all Schema v4 fields and set `status = 'unread'`, `updated_at = now`.
  - `find_active_interactive_by_group()`: Add query for active (`status != 'dismissed'`) card matching `group_key`.
  - `update_interactive_notification()`: Add in-place update for `message`, `metadata`, and `updated_at`.
  - `fetch_active_notifications()`: Map Schema v4 columns from `notifications WHERE status != 'dismissed'`.
  - `mark_notifications_read()`: Support optional `NotificationFilter` and update `status = 'read', updated_at = now`.
  - `dismiss_notifications()`: Support optional `NotificationFilter` and update `status = 'dismissed', updated_at = now`.
  - `dismiss_interactive_by_entity()`: Mark active interactive notification matching entity group as `dismissed`.
- [x] `app/src-tauri/tests/notifications_crud_test.rs`
  - `test_notifications_crud_lifecycle()`: Update test assertions for Schema v4 fields, group queries, and in-place idempotency.

---

## Batch 2: Unified Contracts & Notification Service Chassis
*Depends on: Batch 1*

### Sub-batch 2A: Pure PipelineError & Universal IPC Contracts
*Depends on: Batch 1*

- [x] `app/src-tauri/src/core/error.rs`
  - `PipelineImpact`: Add `None` variant.
  - `PipelineError`: Prune `actionability` field so struct contains strictly `(turn_id, message, source, impact)`.
  - `Actionability`: Decommission and delete enum permanently.
  - Zero notification imports or types in `core/error.rs`.
- [x] `app/src-tauri/src/core/events.rs`
  - `Severity`: Define universal 3-tier visual priority enum (`Info`, `Warning`, `Critical`) with serde snake_case.
  - `ToastLevel`: Decommission and delete enum permanently.
  - `ToastPayload`: Replace `level: ToastLevel` with `severity: Severity`.
  - `NotificationRecord`: Add `group_key`, `severity: Severity`, `action_type`, `action_payload`, `updated_at`; drop `is_read`.

### Sub-batch 2B: Notification Service Chassis & Toast Overlay Alignment
*Depends on: Sub-batch 2A*

- [x] `app/src-tauri/src/services/notifications/mod.rs` [NEW]
  - Register submodules: `pub mod router; pub mod actions; pub use router::*; pub use actions::*;`.
  - `NotificationCategory`: Define closed 7-variant enum `SessionCompaction`, `MemoryConsolidation`, `Pipeline`, `Dictation`, `Hardware`, `Models`, `Storage`.
  - `Action`: Define enum `Transient`, `Receipt`, `Interactive(ActionPayload)`.
  - `ActionPayload`: Define enum `CompactSession`, `ConsolidateMemory`, `Retry`, `Navigate`.
  - `NotificationParams`: Define input parameters struct for `notify()`.
  - `DeliveryChannel`: Define `ToastOnly`, `NotificationOnly`, `ToastAndNotification`.
  - `notify()`: Implement single entry point orchestrating channel delivery, task idempotency, and DB persistence.
- [x] `app/src-tauri/src/services/notifications/router.rs` [NEW]
  - `resolve_channel()`: Implement deterministic 3D routing truth table (`impact`, `severity`, `action`).
- [x] `app/src-tauri/src/services/notifications/actions.rs` [NEW]
  - `execute_notification_action()`: Implement polymorphic backend action dispatcher for `CompactSession`, `ConsolidateMemory`, and `Retry`.
  - On action success, invoke `dismiss_interactive_by_entity` to mark active interactive card dismissed.
- [x] `app/src-tauri/src/services/mod.rs`
  - Register `pub mod notifications;`.
- [x] `app/src-tauri/src/toast.rs`
  - `show_toast()`: Update signature to accept `severity: Severity`.
  - `show_toast()`: Add foreground focus suppression using `main_window.is_focused()`.
  - `show_toast()`: Add fallback to SQLite drawer when window construction or display fails.

---

## Batch 3: Upstream Call-Site Migration & IPC Adapter
*Depends on: Batch 2*

### Sub-batch 3A: Worker Error Emission & Realtime Call Sites
*Depends on: Batch 2*

- [x] `app/src-tauri/src/services/llm/actor.rs`
  - `classify_llm_error()`: Update to construct pure `PipelineError { turn_id, message: err_str, source: "LlmActor".into(), impact }`. Remove `Actionability`.
- [x] `app/src-tauri/src/services/tts/actor.rs`
  - Update `PipelineError` construction to provide `(turn_id, message, source, impact: PipelineImpact::Degraded)`.
- [x] `app/src-tauri/src/services/tts/providers/kokoro.rs`
  - Update `PipelineError` construction to provide `(turn_id, message, source, impact: PipelineImpact::Degraded)`.
- [x] `app/src-tauri/src/services/tts/providers/edge_tts.rs`
  - Update all 5 `PipelineError` emissions to provide `(turn_id, message, source, impact: PipelineImpact::Degraded)`.
- [x] `app/src-tauri/src/services/realtime/mod.rs`
  - `RealtimeProviderEvent::Error`: Remove `actionability` field.
- [x] `app/src-tauri/src/services/realtime/transport/connection.rs`
  - Update `RealtimeProviderEvent::Error` emission without `actionability`.
- [x] `app/src-tauri/src/services/realtime/providers/deepgram/session.rs`
  - Update error emissions without `actionability`.
- [x] `app/src-tauri/src/services/realtime/actor.rs`
  - Update translation of `RealtimeProviderEvent::Error` to pure `VoxEvent::Error(PipelineError { turn_id, message, source, impact })`.
- [x] `app/src-tauri/src/pipeline/dictation/ptt.rs`
  - `on_ptt_start()`: Emit pure `PipelineError` with `turn_id: 0, message, source, impact: PipelineImpact::TurnAborted`.

### Sub-batch 3B: Pipeline Error Boundaries & Dictation Router
*Depends on: Sub-batch 3A*

- [x] `app/src-tauri/src/pipeline/assistant/error.rs`
  - `on_error()`: Drive state machine transitions strictly by `err.impact` (`Degraded`, `TurnAborted`, `SessionHalted`).
  - `on_error()`: Classify `PipelineError` into `NotificationParams` and delegate alerting strictly to `services::notifications::notify()`.
- [x] `app/src-tauri/src/pipeline/dictation/error.rs`
  - `on_error()`: Transition dictation state machine by `err.impact` and delegate alert to `notify()`.
- [x] `app/src-tauri/src/pipeline/assistant/transcript.rs`
  - `on_transcript_final()`: Route empty recognition alert through `notify()` (`category: Pipeline`, `impact: None`, `severity: Info`, `action: Transient`).
- [x] `app/src-tauri/src/pipeline/dictation/transcript.rs`
  - `on_transcript_final()`: Route empty recognition alert through `notify()` (`category: Dictation`, `impact: None`, `severity: Info`, `action: Transient`).
- [x] `app/src-tauri/src/pipeline/assistant/session.rs`
  - `on_resume()`: Route resumption failure through `notify()` (`category: Pipeline`, `impact: SessionHalted`, `severity: Critical`, `action: Interactive(Navigate("settings/ai"))`).
- [x] `app/src-tauri/src/services/dictation/output_router.rs`
  - `dispatch_to_paste()`: Route paste success through `notify()` with snippet preview (`category: Dictation`, `impact: None`, `severity: Info`, `action: Transient`).
  - `dispatch_to_paste()`: Route paste-blocked error branch through `notify()` (`category: Dictation`, `impact: None`, `severity: Warning`, `action: Transient`, title `"Paste Blocked by OS"`).
  - `dispatch_to_clipboard()`: Remove toast overlay (silent per spec §8.1).

### Sub-batch 3C: Compaction, Scheduler & IPC Handlers
*Depends on: Sub-batch 3B*

- [x] `app/src-tauri/src/services/memory/compaction/coordinator.rs`
  - `notify_uncompacted_session()`: Route uncompacted turn alerts through `notify()` with `Interactive(CompactSession)` and entity-scoped idempotency.
  - `run_compaction_slice()`: On compaction success, dismiss interactive card for session via `persistence::notifications::dismiss_interactive_by_entity` and emit `Receipt` via `notify()`.
  - `run_compaction_slice()`: On compaction failure, emit `Receipt` via `notify()` without mutating card status to pseudo-job states.
- [x] `app/src-tauri/src/services/memory/compaction/mod.rs`
  - Ensure boot reconciliation caller of `notify_uncompacted_session` aligns with signature.
- [x] `app/src-tauri/src/services/memory/scheduler.rs`
  - `check_daily_consolidation()`: Route missed consolidation alert through `notify()` with `Interactive(ConsolidateMemory)`.
  - `check_daily_consolidation()`: Route successful consolidation through `notify()` with `Receipt`.
- [x] `app/src-tauri/src/ipc/notifications.rs`
  - `NotificationFilter`: Define filter struct (`ids`, `group_key`, `category`).
  - `get_notifications()`: Query non-dismissed records.
  - `mark_notifications_read()`: Support optional `NotificationFilter`.
  - `dismiss_notifications()`: Support optional `NotificationFilter`.
  - `execute_notification_action()`: Dispatch action execution to `services::notifications::execute_notification_action`.
- [x] `app/src-tauri/src/lib.rs`
  - Update Tauri invoke handlers: register `execute_notification_action`, `dismiss_notifications`.
  - Prune legacy dev 600s test poll loop with obsolete `ToastLevel`.

---

## Batch 4: Frontend Services, Store & Feed Rollup UI
*Depends on: Batch 3*

### Sub-batch 4A: Core Types, Event Registry & Toast App
*Depends on: Batch 3*

- [x] `app/src/services/eventsService.ts`
  - Remove `ToastLevel`.
  - Export `Severity` (`"info" | "warning" | "critical"`).
  - Update `ToastPayload`: `{ title: string; message: string; severity: Severity; duration_ms?: number }`.
  - Update `NotificationRecord`: Add `group_key`, `severity: Severity`, `action_type`, `action_payload`, `updated_at`; drop `is_read`.
- [x] `app/src/toast/ToastApp.tsx`
  - Update visual style configuration to 3 tiers (`info`, `warning`, `critical`).
  - Remove legacy `success` / `error` references.

### Sub-batch 4B: Notification Service & Store Feed Rollup
*Depends on: Sub-batch 4A*

- [x] `app/src/services/notificationService.ts`
  - `NotificationCategory`: Constrain to closed 7 categories.
  - `NotificationFilter`: Export filter interface.
  - `markNotificationsRead()`: Accept optional filter.
  - `dismissNotifications()`: Accept optional filter.
  - `executeNotificationAction()`: Add IPC invoke caller.
  - Decommission `triggerSessionCompaction` and legacy job status helpers.
- [x] `app/src/store/notificationStore.ts`
  - `selectRolledUpNotifications`: Implement grouping algorithm by `group_key` with `(×N)` counts and latest record.
  - `selectBadgeCount`: Compute unread count for interactive tasks.
  - `executeAction`: Handle client-side `Navigate` via React router or dispatch `executeNotificationAction`.

### Sub-batch 4C: Notification Panel UI Alignment & Actions
*Depends on: Sub-batch 4B*

- [x] `app/src/shared/components/common/NotificationPanel.tsx`
  - Render rolled-up notification groups using `selectRolledUpNotifications`.
  - Render `(×{count})` occurrence badge for grouped receipts.
  - Render card severity styling (violet neutral, amber, red).
  - Render primary action buttons dynamically when `latest.action_type === "interactive"`:
    - `[Tidy Now]`
    - `[Consolidate]`
    - `[Open Settings]`
    - `[Retry]`
  - Show inline spinner during action execution.
  - Group dismissal: clicking dismiss calls `dismissNotifications({ group_key })`.
- [x] `app/src/data/notificationCopy.ts`
  - Update copy dictionaries to match closed categories and action button labels.
