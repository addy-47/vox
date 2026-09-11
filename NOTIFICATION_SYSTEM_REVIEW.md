# Production Code Review: Unified Notification & Alert System Refactor

- **Scope Evaluated**: Backend services (`services/notifications/`, `persistence/notifications.rs`, `pipeline/`), IPC layer (`ipc/notifications.rs`), Frontend store and UI (`notificationStore.ts`, `NotificationPanel.tsx`, `ToastApp.tsx`), and schema migrations (`schema.rs`).
- **Calibration**: Reviewing this as **production code** for an 8GB RAM, CPU-first, low-latency desktop AI application. Concurrency safety, race-free SQLite transactions, thread placement boundaries, and memory leak prevention are evaluated under real desktop runtime conditions.

---

## Review Summary

The backend agent successfully translated the 3D routing matrix (`PipelineImpact`, `Severity`, `Action`), dropped the legacy `ToastLevel` and `Actionability` abstractions, implemented the closed 7-category enum, established client-side `Navigate` handling, and wired the `(×N)` frontend rollup. The architecture is significantly cleaner than the pre-refactor state.

However, an adversarial review of the actual implementation diff surfaces **one blocking test regression**, **one serious state desynchronization bug on action failure**, **two concurrency/lifecycle traps**, and **one UI rollup race condition**.

---

## 🔴 Will Break

### 1. Schema Version Unit Test Out of Sync (`schema.rs`)
- **What it is**: `app/src-tauri/src/persistence/schema.rs:322` asserts `assert_eq!(version, 3, "Schema version must be 3");`. However, `SCHEMA_VERSION` was bumped to `4` in line 15 as part of Schema v4.
- **Why it matters**: `cargo test` / `cargo nextest run` fails immediately on `persistence::schema::tests::test_v2_schema_initialization`. Any CI or developer running the unit suite will be blocked.
- **The replacement**:
```rust
// app/src-tauri/src/persistence/schema.rs:322
assert_eq!(version, 4, "Schema version must be 4");
```
- **Severity**: 🔴 Will break (CI / test suite failure).

---

### 2. Phantom Card Dismissal on Action Launch (`services/notifications/actions.rs`)
- **What it is**: In [`execute_notification_action`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/notifications/actions.rs#L38-L47), when the user clicks an action like `[Tidy Now]`, the card is immediately deleted from the database and marked `dismissed` in the UI **before** the asynchronous task even starts:
```rust
// actions.rs:38
dismiss_notification(&state.db, id).await?;
let mut updated_record = record.clone();
updated_record.status = "dismissed".to_string();
let _ = emit_ipc(app, IpcEvent::NotificationUpdated(updated_record));

tauri::async_runtime::spawn(async move {
    match CompactionCoordinator::run_compaction_slice(...).await { ... }
});
```
- **Why it matters**: If `run_compaction_slice` is deferred (e.g. pipeline state is `Thinking` or `Speaking`, or another compaction is running), `Ok(None)` is returned. The interactive card is already permanently destroyed, but compaction never ran. The user is never re-prompted to compact those turns, leaving uncompacted turns orphaned until session teardown. Furthermore, if the spawn crashes or fails to lock, the remediation card is lost forever.
- **The replacement**: Keep the interactive notification in state and dismiss it **only upon verified completion**:
```rust
// Dismiss ONLY on success inside the async task:
match CompactionCoordinator::run_compaction_slice(&app_handle, &app_state, session_id, "manual", None).await {
    Ok(Some(summary)) => {
        let _ = dismiss_notification(&db, id).await;
        let mut updated = record.clone();
        updated.status = "dismissed".to_string();
        let _ = emit_ipc(&app_handle, IpcEvent::NotificationUpdated(updated));
        // emit receipt...
    }
    Ok(None) => {
        log::info!("[Notifications::Action] Compaction deferred for session {}", session_id);
        // Do NOT dismiss card; let user trigger again when ready
    }
    Err(e) => {
        log::error!("[Notifications::Action] Compaction failed for session {}: {}", session_id, e);
        // Do NOT dismiss card; emit warning receipt or allow retry
    }
}
```
- **Severity**: 🔴 Will break user recovery flow on deferred/failed actions.

---

### 3. Duplicate Receipt Emission on Successful Action (`coordinator.rs` & `actions.rs`)
- **What it is**: When `execute_notification_action` runs compaction, both `CompactionCoordinator::run_compaction_slice` (lines 183 & 262) AND `execute_notification_action` (lines 63–82) independently emit success receipts (`"Session #X Tidied"`).
- **Why it matters**: A single click on `[Tidy Now]` writes **two identical success receipts** to SQLite and broadcasts two events to the frontend, cluttering the drawer with duplicate receipt rows.
- **The replacement**: `run_compaction_slice` should be the sole SSOT for emitting the completion receipt, or `actions.rs` should not re-emit it if the coordinator already does.
- **Severity**: 🔴 Logic bug / data duplication.

---

## 🟠 Real Cost at Production Scale

### 4. O(N) Unbatched SQLite Writes in Mark Read / Dismiss Filters (`persistence/notifications.rs`)
- **What it is**: In [`mark_notifications_read`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/notifications.rs#L160-L167) and [`dismiss_notifications`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/notifications.rs#L203-L210), when a list of `ids` is supplied, it loops sequentially over every ID, issuing a separate `UPDATE` query per row:
```rust
if let Some(ref ids) = f.ids {
    for id in ids {
        conn.execute("UPDATE notifications SET status = 'read', updated_at = ? WHERE id = ? AND status = 'unread'", (now, id.clone())).await?;
    }
    return Ok(());
}
```
- **Why it matters**: Over Turso/SQLite, N sequential roundtrips acquire and release database write locks N times. If a user dismisses or reads 20 rolled-up notifications, this causes write serialization contention with background memory ingestion and session logging.
- **The replacement**: Issue a single batched `WHERE id IN (...)` query or wrap in a transaction:
```rust
if let Some(ref ids) = f.ids {
    if !ids.is_empty() {
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("UPDATE notifications SET status = 'read', updated_at = ? WHERE id IN ({}) AND status = 'unread'", placeholders);
        // bind now followed by all ids
    }
}
```
- **Severity**: 🟠 Real performance cost at scale.

---

### 5. Client-Side Rollup Sort Order Inversion (`notificationStore.ts`)
- **What it is**: In [`selectRolledUpNotifications`](file:///home/addy/projects/apps/vox/app/src/store/notificationStore.ts#L173-L202):
```typescript
for (const notif of state.notifications) {
  if (notif.status === "dismissed") continue;
  const groupKey = notif.group_key || notif.id;
  const existing = groups.get(groupKey);
  if (!existing) {
    groups.set(groupKey, { key: groupKey, latest: notif, count: 1, hasUnread: notif.status === "unread" });
  } else {
    existing.count += 1;
    if (notif.created_at > existing.latest.created_at) {
      existing.latest = notif;
    }
  }
}
return Array.from(groups.values());
```
- **Why it matters**: `state.notifications` starts newest-first. When an existing group key receives an older notification (or when an updated record arrives), the Map's insertion order is preserved from the *first* time that key was encountered, which may no longer be the newest group. The returned array is not sorted by `latest.created_at DESC`, causing rolled-up groups to jump or appear out of chronological order in the panel.
- **The replacement**: Sort the final rolled-up array by timestamp:
```typescript
return Array.from(groups.values()).sort((a, b) => b.latest.created_at - a.latest.created_at);
```
- **Severity**: 🟠 UI visual regression / jitter.

---

## 🟡 Stylistic / Minor Findings

1. **`ToastApp.tsx` Unused Lucide Icons**:
   - `CheckCircle2` is still imported on line 3 even though the `success` level was decommissioned in favor of 3 tiers (`info`, `warning`, `critical`).
2. **Missing Duration Customization in `notify()`**:
   - `NotificationParams` defines `duration_ms: Option<u64>`, but `dispatch_toast` calls `show_toast(app, title, message, severity)` which hardcodes duration to `None` (falling back to 3400ms).
3. **Hardcoded Strings in Error Sources**:
   - `pipeline/assistant/error.rs:65-71` relies on substring matching (`err.source.contains("Audio")`) to map to `NotificationCategory::Hardware`. While functional, error sources should ideally be an enum in `PipelineError` rather than string matching.

---

## What's Actually Fine & Solid

- **Architectural Boundary Adherence**: The 3D truth table in `services/notifications/router.rs` is concise, deterministic, and 100% faithful to the specification.
- **Thread Placement Invariant**: Audio threads and hot paths emit zero notifications. All alerts route through Tokio async tasks via `notify()`.
- **Client-Side Navigate Separation**: `ActionPayload::Navigate` is intercepted directly in React (`notificationStore.ts:98`) and routed with zero backend IPC round-trips.
- **Schema v4 Data Model**: Dropped `is_read`, added `group_key` indexes, and strictly enforced task idempotency via `find_active_interactive_by_group`.
- **Build Health**: Backend compiles cleanly with `cargo clippy --release --all-targets -- -D warnings` (0 warnings), and frontend builds cleanly with `pnpm build`.

---

## Bottom Line

The notification refactor is well-architected and adheres closely to the spec, but **it is not ready to ship until two issues are resolved**: 
1. Fix the `schema.rs:322` test assertion from `3` to `4` so tests pass.
2. In `actions.rs`, do not delete interactive cards *before* background compaction succeeds, and eliminate the duplicate receipt emission between `actions.rs` and `coordinator.rs`.
