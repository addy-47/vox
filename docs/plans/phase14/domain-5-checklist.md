# Domain 5 Implementation Checklist: Background Tasks, Timeout Promotion & Router Wake-Up

---

### Phase 1: Minimal Signatures Stub (Backend Engineer)
*Depends on: None*
*Commit Gate: `git commit -m "feat(tasks): minimal signatures stub for domain 5"`*
- [ ] `app/src-tauri/src/core/events.rs`
  - Add `TaskStatus` enum (`Running`, `Completed`, `Failed`, `Killed`).
  - Add `TaskDto` struct.
  - Add `VoxEvent::TaskFinished { task_id: String, session_id: i64, status: TaskStatus }`.
  - Update `VoxEvent::EndSession { force: bool }`.
- [ ] `app/src-tauri/src/core/state.rs`
  - Add `pub tasks: Arc<BackgroundTaskManager>` field on `AppState`.
- [ ] `app/src-tauri/src/pipeline/atomics.rs`
  - Add `pub last_user_activity_ms: AtomicU64` field to `PipelineAtomics`.
- [ ] `app/src-tauri/src/harness/tools/mod.rs`
  - Add `fn supports_background_promotion(&self) -> bool { false }` to `ToolDefinition`.
- [ ] `app/src-tauri/src/harness/chassis.rs`
  - Add `pub auto_promote_timeout: Duration` and `pub foreground_hard_timeout: Duration` to `Harness`.
- [ ] `app/src-tauri/src/harness/stages/inbox.rs`
  - Add `pub fn push_task_notification(&self, message: InboxMessage)` method on `StepInbox`.
- [ ] `app/src-tauri/src/ipc/pipeline.rs`
  - Add stubs for `get_active_tasks_count`, `list_tasks`, and update `end_session(force: Option<bool>)`.

---

### Phase 2: Black-Box Seam Tests (RED) (Test Engineer)
*Depends on: Phase 1*
*Gate: Run `cargo nextest run --test harness_loop_test --test database_persistence_boundary_test --release` -> Confirm ALL 12 SUBTESTS FAIL (RED)*
*Commit Gate: `git commit -m "test(tasks): add red seam tests for domain 5 background tasks"`*
- [ ] `app/src-tauri/tests/harness_loop_test.rs`
  - [ ] Subtest 1: `bash_tool_executes_fast_command_synchronously_without_backgrounding`
  - [ ] Subtest 2: `bash_tool_auto_promotes_long_running_command_to_background_without_guardrail_strike`
  - [ ] Subtest 3: `bash_tool_foreground_override_waits_past_auto_promote_window_until_completion`
  - [ ] Subtest 4: `background_task_streams_stdout_and_stderr_directly_to_disk_log_without_buffering`
  - [ ] Subtest 5: `manage_task_list_and_status_read_live_state_and_log_tail_from_disk`
  - [ ] Subtest 6: `manage_task_kill_terminates_entire_process_group_including_grandchildren`
  - [ ] Subtest 7: `task_finished_in_ready_within_active_window_triggers_proactive_spoken_turn`
  - [ ] Subtest 8: `task_finished_in_ready_after_quiet_window_or_in_paused_suppresses_speech_and_emits_notification_only`
  - [ ] Subtest 9: `task_finished_in_working_buffers_in_step_inbox_without_releasing_active_speech_hold`
  - [ ] Subtest 10: `running_background_task_inhibits_paused_to_sleeping_offload_and_disengage_enforces_force_guard`
- [ ] `app/src-tauri/tests/database_persistence_boundary_test.rs`
  - [ ] Subtest 11: `schema_v19_persists_session_tasks_lifecycle_and_enforces_status_check_constraint`
  - [ ] Subtest 12: `session_hard_delete_kills_running_tasks_cascades_session_tasks_rows_and_purges_log_cache_dir`

---

### Phase 3: Atomic Production Implementation (GREEN) (Backend & Frontend Engineers)
*Forbidden from modifying `tests/**`!*

#### Batch 1: Schema v19, Tier 4 Log Purge, Paths & Atomics (Backend Engineer)
*Depends on: Phase 2*
*Commit Gate: `git commit -m "feat(tasks): batch 1 schema v19, paths and tier 4 purge"`*
- [ ] `app/src-tauri/src/utils/paths.rs`
  - Add `pub fn tasks_dir() -> PathBuf`.
  - Add `pub fn session_tasks_dir(session_id: i64) -> PathBuf`.
  - Add `pub fn task_log_path(session_id: i64, task_id: &str) -> PathBuf`.
- [ ] `app/src-tauri/src/persistence/schema.rs`
  - Update `SCHEMA_VERSION` to 19.
  - Add `session_tasks` DDL to `SCHEMA_TABLE_STATEMENTS`.
  - Add migration block in `migrate_schema` for `current_version < 19`.
- [ ] `app/src-tauri/src/persistence/sessions.rs`
  - In `delete_session`, delete `paths::session_tasks_dir(session_id)` if it exists.
- [ ] `app/src-tauri/src/persistence/worker.rs`
  - In `run_startup_sweeps`, sweep `paths::tasks_dir()` for orphaned session folders.
- [ ] `app/src-tauri/src/pipeline/atomics.rs`
  - Implement atomic methods to get and record `last_user_activity_ms`.

#### Batch 2: BackgroundTaskManager & Process Group Supervisor (Backend Engineer)
*Depends on: Batch 1*
*Commit Gate: `git commit -m "feat(tasks): batch 2 background task manager and supervisor"`*
- [ ] `app/src-tauri/src/services/tasks/mod.rs`
  - Create and export module.
- [ ] `app/src-tauri/src/services/tasks/manager.rs`
  - Implement `BackgroundTaskManager`:
    - `resolve_shell_command`: maps to `sh` on Android and `cmd /C` on Windows.
    - `spawn_supervised_task`: spawns process group (`setpgid(0, 0)`), hooks stdout/stderr to disk file, `.stdin(Stdio::null())`.
    - Tokio supervisor: awaits `child.wait().await` to reap zombies, updates DB, emits `VoxEvent::TaskFinished`.
    - `mark_promoted`: inserts row in `session_tasks`, sets `is_promoted = true`.
    - `prune_unpromoted`: cleans up unpromoted tasks without emitting event.
    - `kill_task`: sends `SIGKILL` to negative PGID, marks `killed`.
    - `kill_all`: terminates all process groups for a session.
    - `read_log_tail`: seeks and reads last $N$ lines from disk.
    - `has_running_tasks` & `active_tasks_count`: fast O(1) in-memory checks.
- [ ] `app/src-tauri/src/core/state.rs`
  - Instantiate `tasks: Arc<BackgroundTaskManager>` in `AppState::new`.

#### Batch 3: Tool Chassis: bash, manage_task & Promotion Engine (Backend Engineer)
*Depends on: Batch 2*
*Commit Gate: `git commit -m "feat(tasks): batch 3 bash and manage_task tools"`*
- [ ] `app/src-tauri/src/harness/tools/mod.rs`
  - Add `supports_background_promotion(&self) -> bool { false }` to `ToolDefinition`.
  - Add `ToolExecutionContext::with_promotion_timeouts(auto_promote, hard_ceiling)`.
- [ ] `app/src-tauri/src/harness/tools/bash.rs`
  - Implement `BashTool`:
    - Schema with `command: string` and `run_in_background: boolean` (default `true`).
    - `supports_background_promotion(&self) -> bool { true }`.
    - Single-spawn supervisor execution: spawns once, races `completion_rx` against timeout, promotes without dropping future.
- [ ] `app/src-tauri/src/harness/tools/manage_task.rs`
  - Implement `ManageTaskTool`:
    - Actions: `list`, `status`, `kill`, `kill_all`.
    - Parallel execution for `list`/`status`; Exclusive execution for `kill`/`kill_all`.
    - `status` backward seek tail read.
- [ ] `app/src-tauri/src/harness/tools/registry.rs`
  - Register `BashTool` and `ManageTaskTool` in default tools.
- [ ] `app/src-tauri/src/harness/tools/executor.rs`
  - Scope `run_in_background` handling to promotable tools; non-promotable tools retain 13s hard timeout.
- [ ] `app/src-tauri/src/harness/steps.rs`
  - Propagate `auto_promote_timeout` and `foreground_hard_timeout` to `ToolExecutionContext`.

#### Batch 4: Event Router 8-State Matrix & StepInbox Integration (Backend Engineer)
*Depends on: Batch 3*
*Commit Gate: `git commit -m "feat(tasks): batch 4 router 8-state matrix and step inbox"`*
- [ ] `app/src-tauri/src/harness/stages/inbox.rs`
  - Implement `push_task_notification(&self, message: InboxMessage)` without touching `speech_hold_active`.
- [ ] `app/src-tauri/src/pipeline/router.rs`
  - Route `VoxEvent::TaskFinished` to `super::assistant::tasks::on_task_finished`.
- [ ] `app/src-tauri/src/pipeline/assistant/tasks.rs` (NEW)
  - Implement 8-state handling matrix:
    - `Idle`: Updates DB, emits Toast + Drawer notification.
    - `Ready`: If `<5m` since `last_user_activity_ms`: dispatches in 0ms if `active_count == 0` (or coalesces if >0) to proactive spoken turn. If `>5m`: Toast + Drawer only.
    - `Listening`, `Thinking`, `Working`: Calls `inbox.push_task_notification()`.
    - `Speaking`: Calls `inbox.push_task_notification()`; evaluated when playback drains.
    - `Paused`, `Sleeping`: Updates DB, Toast + Drawer only.
  - Proactive turn prompt assembly with `task_id={task_id}` prefix.
- [ ] `app/src-tauri/src/pipeline/assistant/llm.rs`
  - In `evaluate_synthesis_latch` / transition to `Ready`: check undrained inbox task notifications and trigger proactive turn if present.
- [ ] `app/src-tauri/src/pipeline/assistant/speech.rs`, `transcript.rs`, `text.rs`, `ptt.rs`
  - Record `last_user_activity_ms` on user interactions.

#### Batch 5: Inactivity Sleep Inhibition (RULE-EVT-12) (Backend Engineer)
*Depends on: Batch 4*
*Commit Gate: `git commit -m "feat(tasks): batch 5 sleep standby inhibition rule-evt-12"`*
- [ ] `app/src-tauri/src/pipeline/lifecycle.rs`
  - In `spawn_idle_monitor`: while in `Paused`, check `state.tasks.has_running_tasks()`.
  - When running: log inhibition and `continue` loop to re-arm sleep timer without deadlocking on `state_rx.changed()`.
  - Extract `spawn_idle_monitor_with_timeouts` for deterministic test verification.

#### Batch 6: Two-Step Disengage UX (ConfirmModal) & Backend Force Guard (Backend & Frontend Engineers)
*Depends on: Batch 5*
*Commit Gate: `git commit -m "feat(tasks): batch 6 two-step disengage ux and force guard"`*
- [ ] `app/src-tauri/src/ipc/pipeline.rs`
  - Implement `get_active_tasks_count(sessionId?: number) -> number`.
  - Implement `list_tasks(sessionId?: number) -> Array<TaskDto>`.
  - Update `end_session(force: Option<bool>)` passing `force.unwrap_or(false)`.
- [ ] `app/src-tauri/src/pipeline/assistant/session.rs`
  - Update `on_end(app, state, ctx, force: bool)`:
    - If `!force && state.tasks.active_tasks_count(session_id) > 0`: log warning and return early (stay in `Ready`).
    - If `force || active_count == 0`: call `state.tasks.kill_all(session_id)`, unmount harness, transition to `Idle`.
- [ ] `app/src/services/pipelineService.ts`
  - Export `getActiveTasksCount(sessionId?: number): Promise<number>`.
  - Update `endSession(force?: boolean): Promise<void>`.
  - Update `disengageSession(notifyState, timeoutMs, force?: boolean)`.
- [ ] `app/src/shared/context/VoiceSessionContext.tsx`
  - In `disengage()`: query `getActiveTasksCount()`. If `> 0`, render `ConfirmModal`.
  - On confirm: call `disengageSession(notifyState, 8000, true)`.
- [ ] `app/src/store/sessionStore.ts`
  - Add `activeTasksCount: number` to state.

---

### Phase 4: Mutation Proof & Verification (Test Engineer)
*Depends on: Phase 3*
*Gate: Confirm all 10 mutants in False-Green table turn RED, then revert to GREEN*
*Commit Gate: `git commit -m "test(tasks): verify mutation coverage domain 5"`*
- [ ] Mutant 1: In `BashTool::execute`, promote all commands unconditionally.
- [ ] Mutant 2: In `BashTool::execute` / `executor.rs`, mark auto-promoted outcomes with `is_error: true`.
- [ ] Mutant 3: In `BashTool::execute`, ignore `run_in_background: false` and use auto-promote timer.
- [ ] Mutant 4: In `BackgroundTaskManager::spawn_supervised_task`, redirect only `stdout` and leave `stderr` as `Stdio::null()`.
- [ ] Mutant 5: In `ManageTaskTool`, return first 50 lines (head) instead of last 50 lines (tail).
- [ ] Mutant 6: In `kill_process_group`, change `-(pid as i32)` to `pid as i32` (positive PID).
- [ ] Mutant 7: In `pipeline/assistant/tasks.rs`, omit proactive turn dispatch in `Ready`.
- [ ] Mutant 8: In `pipeline/assistant/tasks.rs`, bypass 5-minute activity check in `Ready`.
- [ ] Mutant 9: In `pipeline/assistant/tasks.rs`, call `inbox.push()` instead of `push_task_notification()`.
- [ ] Mutant 10: In `lifecycle.rs`, remove `!state.tasks.has_running_tasks()` guard.

---

### Phase 5: Adversarial Review & Sign-Off (Independent Reviewer)
*Depends on: Phase 4*
*Gate: Full diff review against specs and style guides with ZERO prior context*
- [ ] Review full git diff.
- [ ] Verify `verify-targets.sh` and `cargo nextest` clean pass.
- [ ] Verify zero compiler warnings, zero deadlocks, and strict boundary adherence.
- [ ] Issue formal review report.
