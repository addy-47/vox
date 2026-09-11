# Implementation Plan — Unified Notification & Toast System (Vox v2)

Translate the approved [Notification & Toast Unified Architectural Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md) and [IPC Command & Event Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md) into code. This refactor cleanly replaces legacy `ToastLevel`, `Actionability`, ad-hoc `show_toast` calls, and loose notification writes across the entire Vox application with:
1. Pure `PipelineError` in `core/error.rs` with zero notification bleed into core.
2. A modular Notification Service (`services/notifications/`: `mod.rs`, `router.rs`, `actions.rs`) owning the 3D routing engine (`resolve_channel`), single `notify()` front door, and polymorphic action executor (`execute_notification_action`).
3. Schema v4 SQLite persistence with task idempotency (entity-scoped interactive updates) and append-only receipt auditing in `persistence/notifications.rs`.
4. Foreground focus suppression and overlay alignment in `toast.rs` and `ToastApp.tsx`.
5. Frontend service and store rollup feed with `(×N)` occurrence counters and dynamic action controls in `NotificationPanel.tsx`.

---

## Target End-State

1. **Pure Engine Error Boundary (`core/error.rs`)**:
   - `core/error.rs` contains strictly engine runtime error definitions:
     ```rust
     pub struct PipelineError {
         pub turn_id: u32,
         pub message: String,
         pub source: String,
         pub impact: PipelineImpact,
     }
     ```
   - `Actionability` is completely deleted. Zero notification types exist in `core/error.rs`.
   - `core/` has zero dependencies on `services/`.
   - Worker actors (`LlmActor`, `TtsActor`, `RealtimeActor`, `VadActor`) emit runtime errors with `PipelineImpact` only.
2. **Notification Service Architecture (`services/notifications/`)**:
   - Logically structured into 3 files following standard Vox patterns:
     - `services/notifications/mod.rs`: Service front door, enums and service-level structs (`Action`, `ActionPayload`, `NotificationCategory`, `NotificationParams`, `DeliveryChannel`), and the `notify()` orchestrator function.
     - `services/notifications/router.rs`: Deterministic 3D routing engine (`resolve_channel(impact, severity, action) -> DeliveryChannel`) implementing the spec truth table.
     - `services/notifications/actions.rs`: Polymorphic backend action dispatcher (`execute_notification_action(id)`), executing `CompactSession`, `ConsolidateMemory`, or `Retry`, and auto-dismissing active interactive cards on completion.
3. **IPC Event Contracts (`core/events.rs`)**:
   - `ToastLevel` decommissioned and deleted.
   - `Severity` defined in `core/events.rs` as the universal 3-tier visual priority enum (`Info`, `Warning`, `Critical`).
   - `ToastPayload` updated to use `severity: Severity`.
   - `NotificationRecord` mirrors Schema v4 (`id`, `group_key`, `category`, `severity`, `action_type`, `action_payload`, `title`, `message`, `status`, `session_id`, `metadata`, `created_at`, `updated_at`). Legacy `is_read` column dropped.
4. **Schema v4 Database (`persistence/schema.rs` & `persistence/notifications.rs`)**:
   - `SCHEMA_VERSION` bumped to 4.
   - `notifications` table has columns: `id`, `group_key`, `category`, `severity`, `action_type`, `action_payload`, `title`, `message`, `status`, `session_id`, `metadata`, `created_at`, `updated_at`.
   - Indexes: `(status, created_at DESC)`, `(group_key, status)`, `(session_id)`.
   - Task idempotency: interactive notifications use entity-scoped `group_key` (e.g. `"session_compaction:12"`) and execute in-place `UPDATE` on active cards.
   - Append-only receipt auditing: receipts use stream-level `group_key` (e.g. `"session_compaction:receipts"`) and always append new rows.
5. **Overlay Channel Alignment (`toast.rs` & `ToastApp.tsx`)**:
   - `toast.rs` takes `Severity`.
   - Foreground focus suppression: floating HUD suppressed when Vox main window is focused.
   - Graceful elevation fallback: if toast window creation fails, automatically falls back to persistent drawer (`NotificationOnly`).
6. **Frontend Rollup & Polymorphic Actions**:
   - `NotificationPanel.tsx` visually rolls up cards by `group_key` with an occurrence badge `(×N)`.
   - Dynamic primary action buttons for `Interactive` cards (`CompactSession`, `ConsolidateMemory`, `Retry`, `Navigate`).
   - Client-side navigation: `Navigate` actions route via React router (`navigate(payload.target)`) with zero backend IPC invocation.

---

## User Review & Clarifications Resolved

> [!NOTE]
> **Resolved Architectural Discipline**:
> 1. **Pure PipelineError in `core/error.rs`**: `PipelineError` contains only `(turn_id, message, source, impact)`. `Actionability` is pruned. `core/` contains zero notification logic or types, preserving architectural layering.
> 2. **Modular `services/notifications/` Structure**: Structured cleanly into 3 cohesive files (`mod.rs`, `router.rs`, `actions.rs`) following Vox service conventions, avoiding micro-file oversaturation while completely avoiding monolithic dumps.
> 3. **Error Classification at the Boundary**: Runtime error boundaries (`pipeline/assistant/error.rs` and `pipeline/dictation/error.rs`) drive the state machine by `impact` and map `PipelineError` into `NotificationParams` for `notify()`.
> 4. **Interactive Task Dismissal on Success**: When an interactive action completes successfully, the backend automatically marks the active interactive card `status = 'dismissed'` in SQLite, maintaining card attention isolation while the new `Receipt` card logs the completion audit trail.
> 5. **Realtime Subsystem Alignment**: `services/realtime/mod.rs` (`RealtimeProviderEvent::Error`), `transport/connection.rs`, and `providers/deepgram/session.rs` are updated so `Actionability` deletion compiles 100% cleanly.
> 6. **Dictation Output Router Branches**: Clipboard copy is silent (zero toast). OS Paste success emits preview snippet toast. OS Paste-blocked error branch routes through `notify()` with `Severity::Warning`.
> 7. **Compaction Coordinator Scope**: Both `notify_uncompacted_session()` and `run_compaction_slice()` are updated to route through `notify()`.

---

## Specification Cross-Reference Matrix (Full Coverage Audit)

| Spec Section | Topic & Mandate | Target Subsystem / File | Mapped Batch |
| :--- | :--- | :--- | :---: |
| **§1.1 – 1.3** | 3D Orthogonal Taxonomy (`PipelineImpact`, `Severity`, `Action`) | `core/error.rs`, `core/events.rs`, `services/notifications/mod.rs` | **Batch 2A & 2B** |
| **§2** | Decommission Ledger (`ToastLevel`, `Actionability`, `show_toast`, `is_read`, `UNIQUE(group_key)`) | Entire workspace | **Batches 1–4** |
| **§3** | Notification Service front door (`notify()`), error boundary separation | `services/notifications/mod.rs`, `pipeline/assistant/error.rs` | **Batch 2B & 3B** |
| **§4.1 – 4.5** | Canonical contracts: `Severity`, `Action`, `ActionPayload`, closed 7 categories | `core/events.rs`, `services/notifications/mod.rs`, `eventsService.ts` | **Batch 2A, 2B & 4A** |
| **§4.6, §7.1** | Schema v4: `notifications` table DDL, indexes, dropping `is_read` | `persistence/schema.rs`, `persistence/notifications.rs` | **Batch 1** |
| **§5.1 – 5.2** | 3D Truth Table & Deterministic Router (`resolve_channel`) | `services/notifications/router.rs` | **Batch 2B** |
| **§6.1** | Pipeline Error Translation & State Machine Invariant (`PipelineImpact` drives FSM; `notify()` drives alerts) | `pipeline/assistant/error.rs`, `pipeline/dictation/error.rs` | **Batch 3B** |
| **§7.2** | Storage Invariants: Receipt append-only, Transient exclusion, Card attention isolation, Interactive task idempotency (in-place update) | `persistence/notifications.rs`, `services/notifications/mod.rs` | **Batch 1 & 2B** |
| **§8.1 – 8.4** | Overlay channel: Foreground focus suppression, text formatting contracts (paste snippet vs copy silent vs paste-blocked), HUD exclusivity & graceful elevation | `toast.rs`, `services/notifications/mod.rs`, `services/dictation/output_router.rs` | **Batch 2B & 3B** |
| **§9.1** | IPC Endpoints: `get_notifications`, `mark_notifications_read`, `dismiss_notifications`, `execute_notification_action` with `NotificationFilter` | `ipc/notifications.rs`, `notificationService.ts` | **Batch 3C & 4B** |
| **§9.2** | Drawer Presentation & Rollup Contracts (`(×N)` rollup by `group_key`, dynamic action button, inline spinner) | `notificationStore.ts`, `NotificationPanel.tsx` | **Batch 4B & 4C** |
| **§10** | Domain Call-Site Inventory (Dictation, Pipeline STT/TTS/RAG/Context/Auth, Hardware, Models, Compaction, Scheduler) | Upstream domain files | **Batch 3A, 3B, 3C** |
| **§11** | Negative Constraints: No ad-hoc toasts, no DB for transient, no HUD for receipts, no status mutation by jobs, no raw enums in UI | Entire codebase | **All Batches** |

---

## Dependency & Blast-Radius Batching

```
  ┌────────────────────────────────────────────────────────┐
  │ Batch 1: Persistence Foundation (Schema v4)           │
  │ • schema.rs: Schema v4 table DDL, drop is_read         │
  │ • persistence/notifications.rs: CRUD & idempotency     │
  │ • tests/notifications_crud_test.rs: Schema v4 verified │
  └──────────────────────────┬─────────────────────────────┘
                             │
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Batch 2: Contracts & Notification Service Chassis      │
  │ • Sub-batch 2A: Pure PipelineError & core/events.rs    │
  │ • Sub-batch 2B: services/notifications/ & toast.rs     │
  └──────────────────────────┬─────────────────────────────┘
                             │
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Batch 3: Upstream Call-Site Migration & IPC Handlers   │
  │ • Sub-batch 3A: Worker error emission & realtime files │
  │ • Sub-batch 3B: Pipeline error boundaries & dictation  │
  │ • Sub-batch 3C: Compaction, Scheduler & IPC adapter   │
  └──────────────────────────┬─────────────────────────────┘
                             │
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Batch 4: Frontend Services, Store & Feed Rollup UI     │
  │ • Sub-batch 4A: eventsService.ts & ToastApp.tsx        │
  │ • Sub-batch 4B: notificationService.ts & store rollup  │
  │ • Sub-batch 4C: NotificationPanel.tsx UI & actions     │
  └────────────────────────────────────────────────────────┘
```

---

## Detailed Batch Breakdown

### Batch 1: Persistence Foundation (Schema v4 & Notification Storage Invariants)

- **Target Files**:
  - `app/src-tauri/src/persistence/schema.rs`
  - `app/src-tauri/src/persistence/notifications.rs`
  - `app/src-tauri/tests/notifications_crud_test.rs`
- **Dependencies**: None.
- **Build Health Expectation**: Builds green independently; passes test suite.
- **Scope & Changes**:
  1. `schema.rs`:
     - Bump `SCHEMA_VERSION: u32 = 4;`.
     - Update `notifications` table statement in `V2_TABLE_STATEMENTS`:
       ```sql
       CREATE TABLE IF NOT EXISTS notifications (
           id TEXT PRIMARY KEY,
           group_key TEXT NOT NULL,
           category TEXT NOT NULL,
           severity TEXT NOT NULL DEFAULT 'info',
           action_type TEXT NOT NULL,
           action_payload TEXT NOT NULL DEFAULT '{}',
           title TEXT NOT NULL,
           message TEXT NOT NULL,
           status TEXT NOT NULL DEFAULT 'unread',
           session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
           metadata TEXT NOT NULL DEFAULT '{}',
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL
       );
       CREATE INDEX IF NOT EXISTS idx_notifications_status_created ON notifications(status, created_at DESC);
       CREATE INDEX IF NOT EXISTS idx_notifications_group_status ON notifications(group_key, status);
       CREATE INDEX IF NOT EXISTS idx_notifications_session ON notifications(session_id);
       ```
     - Remove legacy `is_read` column and old `idx_notifications_status_cat` index.
  2. `persistence/notifications.rs`:
     - Update `NewNotification` to include `group_key`, `severity`, `action_type`, `action_payload`, `updated_at`.
     - Update `create_notification` to insert all Schema v4 fields and set `status = 'unread'`, `updated_at = now`.
     - Add `find_active_interactive_by_group(conn, group_key)` to check for active (`status != 'dismissed'`) cards.
     - Add `update_interactive_notification(conn, id, message, metadata)` for task idempotency in-place updates (`SET message = ?, metadata = ?, updated_at = ?`).
     - Update `fetch_active_notifications` to query `WHERE status != 'dismissed' ORDER BY created_at DESC` and map Schema v4 columns.
     - Update `mark_all_notifications_read` -> `mark_notifications_read(conn, filter: Option<&NotificationFilter>)` to update `SET status = 'read', updated_at = ? WHERE status = 'unread'`.
     - Update `dismiss_notification` -> `dismiss_notifications(conn, filter: Option<&NotificationFilter>)` to update `SET status = 'dismissed', updated_at = ? WHERE status != 'dismissed'`.
     - Add `dismiss_interactive_by_entity(conn, group_key)` to dismiss interactive task cards upon successful completion.
  3. `tests/notifications_crud_test.rs`:
     - Update integration tests to verify Schema v4 insertion, group query, in-place update idempotency, filter-based read/dismiss, and attention status isolation.
- **Verification Command**:
  ```bash
  cargo nextest run --test notifications_crud_test --release --nocapture --test-threads=1
  ```

---

### Batch 2: Unified Contracts & Notification Service Chassis

#### Sub-batch 2A: Pure PipelineError & Universal IPC Contracts
- **Target Files**:
  - `app/src-tauri/src/core/error.rs`
  - `app/src-tauri/src/core/events.rs`
- **Dependencies**: Batch 1.
- **Build Health Expectation**: Intermediate; will be made green across crate in Batch 2B and Batch 3.
- **Scope & Changes**:
  1. `core/error.rs`:
     - Keep `core/error.rs` 100% pure engine runtime definitions.
     - Add `PipelineImpact::None` (for non-voice background tasks).
     - Update `PipelineError`:
       ```rust
       pub struct PipelineError {
           pub turn_id: u32,
           pub message: String,
           pub source: String,
           pub impact: PipelineImpact,
       }
       ```
     - Decommission and delete `Actionability` permanently.
     - Zero notification imports or types in `core/error.rs`.
  2. `core/events.rs`:
     - Define `Severity` enum (`Info`, `Warning`, `Critical`) with serde snake_case (universal visual styling).
     - Decommission and delete `ToastLevel` permanently.
     - Update `ToastPayload` to use `pub severity: Severity`.
     - Update `NotificationRecord` to reflect Schema v4:
       ```rust
       pub struct NotificationRecord {
           pub id: String,
           pub group_key: String,
           pub category: String,
           pub severity: Severity,
           pub action_type: String,
           pub action_payload: String,
           pub title: String,
           pub message: String,
           pub status: String,
           pub session_id: Option<i64>,
           pub metadata: String,
           pub created_at: i64,
           pub updated_at: i64,
       }
       ```
     - Drop legacy `is_read`.

#### Sub-batch 2B: Notification Service Chassis & Toast Overlay Alignment
- **Target Files**:
  - `app/src-tauri/src/services/notifications/mod.rs` [NEW]
  - `app/src-tauri/src/services/notifications/router.rs` [NEW]
  - `app/src-tauri/src/services/notifications/actions.rs` [NEW]
  - `app/src-tauri/src/toast.rs`
  - `app/src-tauri/src/services/mod.rs`
- **Dependencies**: Sub-batch 2A.
- **Scope & Changes**:
  1. `services/notifications/mod.rs`:
     - Register submodules: `pub mod router; pub mod actions; pub use router::*; pub use actions::*;`.
     - Define closed `NotificationCategory` enum (`SessionCompaction`, `MemoryConsolidation`, `Pipeline`, `Dictation`, `Hardware`, `Models`, `Storage`).
     - Define `Action` enum (`Transient`, `Receipt`, `Interactive(ActionPayload)`).
     - Define `ActionPayload` enum (`CompactSession { session_id: i64 }`, `ConsolidateMemory`, `Retry { operation: String, resource_id: Option<String> }`, `Navigate { target: String }`).
     - Define `NotificationParams`:
       ```rust
       pub struct NotificationParams {
           pub group_key: String,
           pub category: NotificationCategory,
           pub impact: PipelineImpact,
           pub severity: Severity,
           pub action: Action,
           pub title: String,
           pub message: String,
           pub session_id: Option<i64>,
           pub metadata: Option<String>,
       }
       ```
     - Define `DeliveryChannel` (`ToastOnly`, `NotificationOnly`, `ToastAndNotification`).
     - Implement single entry point `notify<R: tauri::Runtime>(app: &AppHandle<R>, db: &turso::Connection, params: NotificationParams) -> Result<Option<String>>`:
       - Zero DB queries for `Transient`.
       - For `Interactive`: check `find_active_interactive_by_group`. If present, update in-place (`update_interactive_notification`) and emit `IpcEvent::NotificationUpdated`. If absent, create row and emit `IpcEvent::NotificationCreated`.
       - For `Receipt`: append new record and emit `IpcEvent::NotificationCreated`.
       - If channel has toast: check foreground suppression. If not suppressed, invoke `toast::show_toast`.
       - Graceful elevation fallback: if toast window creation fails, fall back to SQLite drawer insert.
  2. `services/notifications/router.rs`:
     - Implement `resolve_channel(impact: PipelineImpact, severity: Severity, action: &Action) -> DeliveryChannel` per the 3D truth table:
       - `Action::Transient` $\to$ `ToastOnly`
       - `Action::Receipt` $\to$ `NotificationOnly`
       - `Action::Interactive`:
         - If `impact == SessionHalted || severity == Critical` $\to$ `ToastAndNotification`
         - If `impact == TurnAborted && severity == Warning` $\to$ `ToastAndNotification`
         - Otherwise $\to$ `NotificationOnly`
  3. `services/notifications/actions.rs`:
     - Implement `execute_notification_action(app, db, id, action)`:
       - Polymorphic backend dispatcher resolving `ActionPayload` and executing `CompactSession`, `ConsolidateMemory`, or `Retry`.
       - On successful execution of interactive actions, call `dismiss_interactive_by_entity` to mark the interactive card dismissed.
  4. `toast.rs`:
     - Update `show_toast` signature to take `severity: Severity`.
     - Implement foreground window focus suppression: check `app.get_webview_window("main")` and check `is_focused()`. If main window is focused, suppress floating HUD overlay.
     - Emit `IpcEvent::ShowToast(ToastPayload { title, message, severity, duration_ms: Some(3000) })`.

---

### Batch 3: Upstream Call-Site Migration & IPC Adapter

#### Sub-batch 3A: Worker Error Emission & Realtime Call Sites
- **Target Files**:
  - `app/src-tauri/src/services/llm/actor.rs`
  - `app/src-tauri/src/services/tts/actor.rs`
  - `app/src-tauri/src/services/tts/providers/kokoro.rs`
  - `app/src-tauri/src/services/tts/providers/edge_tts.rs`
  - `app/src-tauri/src/services/realtime/mod.rs`
  - `app/src-tauri/src/services/realtime/actor.rs`
  - `app/src-tauri/src/services/realtime/transport/connection.rs`
  - `app/src-tauri/src/services/realtime/providers/deepgram/session.rs`
  - `app/src-tauri/src/pipeline/dictation/ptt.rs`
- **Dependencies**: Batch 2.
- **Scope & Changes**:
  1. `services/llm/actor.rs`:
     - Update `classify_llm_error` to construct pure `PipelineError { turn_id, message: err_str, source: "LlmActor".into(), impact }`. Remove `Actionability`.
  2. `services/tts/` (`actor.rs`, `kokoro.rs`, `edge_tts.rs`):
     - Update all error emissions to construct pure `PipelineError { turn_id, message, source, impact: PipelineImpact::Degraded }`. Remove `Actionability`.
  3. `services/realtime/`:
     - `services/realtime/mod.rs`: Update `RealtimeProviderEvent::Error`: remove `actionability`.
     - `services/realtime/transport/connection.rs`: Update `RealtimeProviderEvent::Error` emission without `actionability`.
     - `services/realtime/providers/deepgram/session.rs`: Update error emissions without `actionability`.
     - `services/realtime/actor.rs`: Update translation of `RealtimeProviderEvent::Error` to `VoxEvent::Error(PipelineError { turn_id, message, source, impact })`.
  4. `pipeline/dictation/ptt.rs`:
     - When dictation is disabled in Settings: emit `PipelineError` with `turn_id: 0, message: "Dictation is disabled in Settings.".into(), source: "DictationPtt".into(), impact: PipelineImpact::TurnAborted`. Remove `Actionability`.

#### Sub-batch 3B: Pipeline Error Boundaries & Dictation Router
- **Target Files**:
  - `app/src-tauri/src/pipeline/assistant/error.rs`
  - `app/src-tauri/src/pipeline/dictation/error.rs`
  - `app/src-tauri/src/pipeline/assistant/transcript.rs`
  - `app/src-tauri/src/pipeline/dictation/transcript.rs`
  - `app/src-tauri/src/pipeline/assistant/session.rs`
  - `app/src-tauri/src/services/dictation/output_router.rs`
- **Dependencies**: Sub-batch 3A.
- **Scope & Changes**:
  1. `pipeline/assistant/error.rs`:
     - Enforce error boundary invariant: state machine transition driven strictly by `err.impact` (`Degraded`, `TurnAborted`, `SessionHalted`).
     - Remove ad-hoc `show_toast` and raw `create_notification`.
     - Classify `PipelineError` into `NotificationParams`:
       - Context overflow: `category: Pipeline`, `severity: Warning`, `action: Interactive(Navigate("settings/ai"))`.
       - Auth failure: `category: Pipeline`, `severity: Critical`, `action: Interactive(Navigate("settings/ai"))`.
       - Audio hardware disconnect: `category: Hardware`, `severity: Critical`, `action: Interactive(Navigate("settings/audio"))`.
       - Missing model: `category: Models`, `severity: Critical`, `action: Interactive(Navigate("settings/models"))`.
       - TTS/Memory degraded: `category: Pipeline`, `severity: Warning`, `action: Transient`.
       - Other: `category: Pipeline`, `severity: Warning`, `action: Transient`.
     - Delegate alerting strictly to `services::notifications::notify()`.
  2. `pipeline/dictation/error.rs`:
     - Transition dictation state machine by `err.impact`.
     - Classify and delegate user notice to `services::notifications::notify()`.
  3. `pipeline/assistant/transcript.rs`:
     - Empty recognition: replace ad-hoc `show_toast` with `notify()`: `category: Pipeline`, `impact: None`, `severity: Info`, `action: Transient`, title `"Voice Assistant"`, message `"Speech detected, but no words recognized."`.
  4. `pipeline/dictation/transcript.rs`:
     - Empty recognition: replace ad-hoc `show_toast` with `notify()`: `category: Dictation`, `impact: None`, `severity: Info`, `action: Transient`, title `"Dictation"`, message `"Speech detected, but no words recognized."`.
  5. `pipeline/assistant/session.rs`:
     - Resumption failure: replace ad-hoc `show_toast` with `notify()`: `category: Pipeline`, `impact: SessionHalted`, `severity: Critical`, `action: Interactive(ActionPayload::Navigate { target: "settings/ai".into() })`.
  6. `services/dictation/output_router.rs`:
     - Paste success: emit `notify()`: `category: Dictation`, `impact: None`, `severity: Info`, `action: Transient`, title `"Dictation Pasted"`, message preview snippet up to 40 characters (`“<start> ... <end>”`).
     - Paste blocked error: emit `notify()`: `category: Dictation`, `impact: None`, `severity: Warning`, `action: Transient`, title `"Paste Blocked by OS"`, message `"Transcript saved to clipboard — paste manually with Ctrl+V."`.
     - Clipboard copy: remove toast overlay (silent per spec §8.1).

#### Sub-batch 3C: Compaction, Scheduler & IPC Handlers
- **Target Files**:
  - `app/src-tauri/src/services/memory/compaction/coordinator.rs`
  - `app/src-tauri/src/services/memory/compaction/mod.rs`
  - `app/src-tauri/src/services/memory/scheduler.rs`
  - `app/src-tauri/src/ipc/notifications.rs`
  - `app/src-tauri/src/lib.rs`
- **Dependencies**: Sub-batch 3B.
- **Build Health Expectation**: Full Rust backend builds 100% clean (`cargo check --release`, `cargo clippy --release --lib -- -D warnings`).
- **Scope & Changes**:
  1. `services/memory/compaction/coordinator.rs`:
     - Update `notify_uncompacted_session()`: route via `notify()`: `category: SessionCompaction`, `impact: None`, `severity: Info`, `action: Interactive(ActionPayload::CompactSession { session_id })`, group_key: `format!("session_compaction:{}", session_id)`, title `format!("Session #{} Ready to Tidy", session_id)`, message `format!("{} uncompacted turns.", uncompacted_turns)`.
     - `run_compaction_slice()`: On compaction success, dismiss interactive card for session via `persistence::notifications::dismiss_interactive_by_entity` and emit `Receipt` via `notify()`: `title: "Session #X Tidied"`, `message: "Extracted X memory facts."`, `group_key: "session_compaction:receipts"`.
     - `run_compaction_slice()`: On compaction failure, emit `Receipt` via `notify()`: `title: "Session #X Compaction Failed"` without mutating card status to pseudo-job states.
  2. `services/memory/compaction/mod.rs`:
     - Ensure callers of `CompactionCoordinator::notify_uncompacted_session` match updated signature.
  3. `services/memory/scheduler.rs`:
     - Missed scheduled consolidation: route via `notify()`: `category: MemoryConsolidation`, `impact: None`, `severity: Warning`, `action: Interactive(ActionPayload::ConsolidateMemory)`, group_key: `"memory_consolidation:missed"`.
     - Consolidation success: route via `notify()`: `category: MemoryConsolidation`, `impact: None`, `severity: Info`, `action: Receipt`, title `"Memory Consolidated"`, group_key: `"memory_consolidation:daily"`.
  4. `ipc/notifications.rs`:
     - Define `NotificationFilter` struct (`ids: Option<Vec<String>>`, `group_key: Option<String>`, `category: Option<String>`).
     - Update `get_notifications(state) -> Result<Vec<NotificationRecord>, VoxIpcError>`.
     - Update `mark_notifications_read(filter: Option<NotificationFilter>, state) -> Result<(), VoxIpcError>`.
     - Update `dismiss_notifications(filter: Option<NotificationFilter>, state) -> Result<(), VoxIpcError>`.
     - Implement `execute_notification_action(id: String, action: Option<String>, app: AppHandle, state) -> Result<(), VoxIpcError>`: dispatches to `services::notifications::execute_notification_action`.
     - Delete legacy `trigger_session_compaction`.
  5. `lib.rs`:
     - Register `execute_notification_action` in `tauri::generate_handler![]` and remove `trigger_session_compaction`.
     - Prune legacy dev 600s test poll loop with obsolete `ToastLevel`.
- **Verification Commands**:
  ```bash
  cargo check --release --all-targets
  cargo clippy --release --lib -- -D warnings
  cargo nextest run --test notifications_crud_test --release --nocapture --test-threads=1
  ```

---

### Batch 4: Frontend Services, Store & Feed Rollup UI

#### Sub-batch 4A: Core Types, Event Registry & Toast App
- **Target Files**:
  - `app/src/services/eventsService.ts`
  - `app/src/services/toastService.ts`
  - `app/src/toast/ToastApp.tsx`
- **Dependencies**: Batch 3.
- **Scope & Changes**:
  1. `eventsService.ts`:
     - Remove `ToastLevel` (`"success" | "warning" | "error" | "info"`).
     - Export `Severity` (`"info" | "warning" | "critical"`).
     - Update `ToastPayload`: `{ title: string; message: string; severity: Severity; duration_ms?: number }`.
     - Update `NotificationRecord`:
       ```typescript
       export interface NotificationRecord {
         id: string;
         group_key: string;
         category: string;
         severity: Severity;
         action_type: "receipt" | "interactive";
         action_payload: string;
         title: string;
         message: string;
         status: "unread" | "read" | "dismissed";
         session_id?: number | null;
         metadata: string;
         created_at: number;
         updated_at: number;
       }
       ```
     - Drop `is_read`.
  2. `ToastApp.tsx`:
     - Update severity visual mapping to 3 tiers:
       - `info`: Violet accent tile & subtle border.
       - `warning`: Amber accent tile & warning icon.
       - `critical`: High-contrast rose/red tile & alert icon.
     - Remove legacy `success` / `error` references.

#### Sub-batch 4B: Notification Service & Store Feed Rollup
- **Target Files**:
  - `app/src/services/notificationService.ts`
  - `app/src/store/notificationStore.ts`
- **Dependencies**: Sub-batch 4A.
- **Scope & Changes**:
  1. `notificationService.ts`:
     - Constrain `NotificationCategory` to closed 7 categories: `"session_compaction" | "memory_consolidation" | "pipeline" | "dictation" | "hardware" | "models" | "storage"`.
     - Add `NotificationFilter` interface.
     - Export `markNotificationsRead(filter?: NotificationFilter): Promise<void>`.
     - Export `dismissNotifications(filter?: NotificationFilter): Promise<void>`.
     - Export `executeNotificationAction(id: string, action?: string): Promise<void>`.
     - Decommission `triggerSessionCompaction` and legacy job status helpers.
  2. `notificationStore.ts`:
     - Manage `notifications: NotificationRecord[]`.
     - Compute unread badge count: `notifications.filter(n => n.status === "unread" && n.action_type === "interactive").length` (or all unread interactive/receipts as specified).
     - Implement rollup grouping selector `selectRolledUpNotifications(state)`:
       - Group active notifications by `group_key`.
       - Return items with `{ groupKey, count, latest: NotificationRecord, ids: string[], hasUnread: boolean }`.
     - Manage execution working state `workingNotificationIds: Set<string>`.
     - Implement `executeAction(notif: NotificationRecord, navigate: NavigateFunction)`:
       - Parse `action_payload`. If `kind === "navigate"`, call `navigate(payload.target)`.
       - Otherwise, add ID to `workingNotificationIds`, invoke `executeNotificationAction(notif.id)`, and clear on settlement.

#### Sub-batch 4C: Notification Panel UI Alignment & Actions
- **Target Files**:
  - `app/src/shared/components/common/NotificationPanel.tsx`
  - `app/src/data/notificationCopy.ts`
- **Dependencies**: Sub-batch 4B.
- **Scope & Changes**:
  1. `NotificationPanel.tsx`:
     - Render rolled-up items using `selectRolledUpNotifications`.
     - If `count > 1`, display the occurrence badge `(×{count})`.
     - Render card severity badge (`Info` neutral, `Warning` amber, `Critical` red).
     - Render primary action buttons dynamically when `latest.action_type === "interactive"`:
       - `[Tidy Now]`
       - `[Consolidate]`
       - `[Open Settings]`
       - `[Retry]`
     - Show inline spinner during action execution.
     - Group dismissal: clicking dismiss calls `dismissNotifications({ group_key })`.
  2. `notificationCopy.ts`:
     - Align copy dictionaries to match closed categories and action button labels.
- **Verification Command**:
  ```bash
  pnpm build
  ```

---

## Verification Plan

### Automated Tests
1. **SQLite CRUD & Idempotency Integration Test**:
   ```bash
   cargo nextest run --test notifications_crud_test --release --nocapture --test-threads=1
   ```
   Validates Schema v4 table creation, index integrity, interactive in-place update idempotency, receipt append-only behavior, and filter-based dismissal/mark-read.
2. **Backend Syntax, Lints & All Seams**:
   ```bash
   cargo check --release --all-targets
   cargo clippy --release --lib -- -D warnings
   ```
   Validates all call sites compile cleanly with zero deprecated constructs.
3. **Frontend Type Check & Bundle Build**:
   ```bash
   pnpm build
   ```
   Validates complete TypeScript contract alignment, zero `any` slips, and green asset packaging.

### Manual Verification
1. Launch dev engine: trigger dictation PTT $\to$ verify transient toast displays snippet without adding rows to the notification drawer.
2. Trigger session compaction notification $\to$ verify interactive card renders with `[Tidy Now]` and clicking it initiates compaction without duplicating cards.
3. Complete session compaction $\to$ verify the interactive card is dismissed and an audit receipt `“Session #X Tidied”` is logged with rollup count `(×N)`.
4. Verify floating HUD is suppressed when Vox main window is focused.
