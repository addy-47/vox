---
title: "Notification System Rework — Typed Categories, Central notify(), Schema v4"
status: "READY — architecture aligned with notifications-spec.md"
last_updated: 2026-09-09
owners: "backend-engineer role, frontend-engineer role"
related_docs:
  - "docs/specs/notifications-spec.md — Ground truth behavioral and interface contract"
  - "docs/specs/db-spec.md — §2.9 notifications table schema"
  - "docs/specs/ipc-spec.md — §2.4 notifications IPC domain and events"
  - "docs/specs/events-spec.md — §2.3 VoxEvent scope (voice-turn lifecycle)"
---

# Notification System Rework Plan

## Goal

Replace today's loose-string, three-sender notification code with one typed front door (`notify()`), a closed category set, explicit severity levels, and a clean schema where card status is strictly `unread | read | dismissed`. Job execution progress (e.g. compaction running/succeeded/failed) is cleanly decoupled from user read/attention state.

The system adopts the standard desktop notification architecture:
- **Append in storage**: Each event inserts a distinct record with a `group_key TEXT NOT NULL` (no brittle DB-level `UNIQUE` constraint destroying historical event logs).
- **Producer idempotency for tasks**: Actionable task cards (e.g., `session_compaction:{session_id}`) check for an active card before insert to prevent duplicate action buttons.
- **Rollup in presentation**: The frontend renders a unified chronological timeline, rolling up repeated event alerts sharing a `group_key` with a `(×N)` occurrence badge.

---

## Current State

- Table `notifications` (`persistence/schema.rs:116`): id/category/title/message/status/session_id(+FK cascade)/metadata/is_read/created_at.
- Three independent writers, each hand-rolling DB write + IPC emit:
  1. `pipeline/assistant/error.rs::on_error` — actionable errors only; toast always, card on `Actionable`.
  2. `services/memory/compaction/coordinator.rs::notify_uncompacted_session` — one pending card per session.
  3. `services/memory/scheduler.rs::ensure_card` — missed/failed consolidation cards.
- 8 loose category strings across 8 files: `session_compaction`, `personal_consolidation`, `compaction_failure`, `context_overflow`, `auth_failure`, `llm_panic`, `tts_unsupported_language`, `network_error`, `dictation_disabled`.
- `VoxEvent` enum carries no explicit scope rule distinguishing internal voice pipeline events from app-level notifications.

---

## Target Design

### 1. Types (Rust, compiler-checked; DB stores strings at boundary)

- `NotificationCategory` (closed enum):
  - `SessionCompaction`
  - `MemoryConsolidation`
  - `ModelManagement`
  - `PipelineError`
  - `StorageHealth`
- `NotificationSeverity`: `Info`, `Warning`, `Critical`. Drives icon visual accents and priority styling.
- `NotificationStatus`: `Unread`, `Read`, `Dismissed`. This is the ONLY card status.
- Contract: `title` and `message` are plain user-facing language; `category` is an internal enum never rendered as raw text.

### 2. Central `notify()` & Idempotency Rules

- **Signature**:
  ```rust
  pub async fn notify(
      conn: &Connection,
      app: &AppHandle<R>,
      category: NotificationCategory,
      severity: NotificationSeverity,
      group_key: String,
      title: String,
      message: String,
      session_id: Option<i64>,
      metadata: Option<serde_json::Value>,
  ) -> Result<NotificationRecord>
  ```
- **Task Idempotency**: If `group_key` represents an entity task (e.g. `session_compaction:{id}` or `consolidation_missed:{date}`), `notify()` checks if an active (`unread` or `read`) row exists with that `group_key`. If so, it returns the existing row without inserting a duplicate.
- **Alert Appends**: For recurring system events (e.g. `error:llm:timeout`), a new row is appended with the same `group_key`, preserving audit trail and recency timestamps.
- **Emits**: Automatically emits `IpcEvent::NotificationCreated` on new insert.

### 3. Schema v4 (`notifications`)

- Columns:
  - `id TEXT PRIMARY KEY` (UUID or deterministic prefix)
  - `group_key TEXT NOT NULL`
  - `category TEXT NOT NULL`
  - `severity TEXT NOT NULL DEFAULT 'info'`
  - `title TEXT NOT NULL`
  - `message TEXT NOT NULL`
  - `status TEXT NOT NULL DEFAULT 'unread'` (`'unread'`, `'read'`, `'dismissed'`)
  - `session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE`
  - `metadata TEXT NOT NULL DEFAULT '{}'`
  - `created_at INTEGER NOT NULL`
  - `updated_at INTEGER NOT NULL`
- Indexes:
  - `idx_notifications_status_created`: `(status, created_at DESC)`
  - `idx_notifications_group_status`: `(group_key, status)`
- Migration mapping from v3:
  - Drop `is_read` column.
  - Map status: `pending/active/in_progress` $\to$ `unread`; `completed/failed` $\to$ `read`; `dismissed` $\to$ `dismissed`.
  - Backfill `group_key`: `session_compaction:{session_id}` if session_id is present, else `category:{id}`.
  - Backfill `severity`: `'info'`.
  - Set `updated_at = created_at`.

### 4. IPC Commands & Events (`ipc/notifications.rs`)

- `get_notifications()`: Returns all rows `WHERE status != 'dismissed' ORDER BY created_at DESC`.
- `mark_notifications_read(ids: Option<Vec<String>>)`: Marks specified rows (or all unread if `None`/empty) as `'read'`.
- `dismiss_notifications(ids: Option<Vec<String>>)`: Marks specified rows (or all active if `None`/empty) as `'dismissed'`.
- `trigger_session_compaction(sessionId: i64)`: Triggers background compaction slice. Does not touch notification card attention status (`unread`/`read`). Progress is tracked in frontend memory store / metadata.

### 5. Frontend Presentation & Rollup (`NotificationPanel.tsx`)

- **Unified Chronological Stream**: Chronological card feed sorted by latest activity.
- **Rollup by `group_key`**:
  - The notification store / selector groups un-dismissed notifications sharing the same `group_key`.
  - Renders a single card displaying the latest message, latest timestamp, and an `(×N)` occurrence badge when $N > 1$.
  - Separate sessions have distinct `group_key`s and always render as individual action cards.
- **Visuals**:
  - `severity` drives border and icon accent colors (`info` = violet/neutral, `warning` = amber, `critical` = rose).
  - Category drives icon selection and deep-link click routing (`session_id` $\to$ History, `memory` $\to$ Memory view, errors $\to$ Settings).

---

## Build Batches

### Batch 1: Specs & Backend Foundation
1. Verify spec alignment: `docs/specs/notifications-spec.md`, `db-spec.md`, and `ipc-spec.md`.
2. Add `NotificationCategory`, `NotificationSeverity`, and `NotificationStatus` enums in `core/events.rs`.
3. Implement Schema v4 migration in `persistence/schema.rs` (user_version bump).
4. Update `persistence/notifications.rs` with `group_key`, `severity`, `updated_at`, and new query helpers.
5. Create central `notify()` front door in `services/notifications/mod.rs` (or `persistence/notifications.rs`).

### Batch 2: Producer Migration & IPC Handlers
1. Update `services/memory/scheduler.rs` to route through `notify()`.
2. Update `services/memory/compaction/coordinator.rs` to route through `notify()`.
3. Update `pipeline/assistant/error.rs` (`on_error`) to route through `notify()`.
4. Update `ipc/notifications.rs` handlers (`get_notifications`, `mark_notifications_read`, `dismiss_notifications`).
5. Ensure `cargo test` and `cargo nextest` pass release verification.

### Batch 3: Frontend Service, Store & UI Rollup
1. Update `eventsService.ts` and `notificationService.ts` types with `group_key`, `severity`, `updated_at`.
2. Update `notificationStore.ts`:
   - Store active notifications.
   - Selector to roll up notifications sharing `group_key` into aggregated card view models with `count: number`.
3. Update `NotificationPanel.tsx`:
   - Render rolled-up cards with `(×N)` occurrence badge.
   - Bind Mark All Read / Dismiss actions to new IPC command signatures.
   - Ensure clean aesthetic styling, micro-animations, and empty states.
4. Run `pnpm check` / `pnpm build` to verify zero TypeScript or bundle regressions.
