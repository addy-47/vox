# Domain 5 Implementation Plan: Background Tasks, Timeout Promotion & Router Wake-Up

---

## 1. Target End-State

Domain 5 establishes production-grade background task execution, timeout promotion, and router wake-up for Vox Harness v2:
1. **`BackgroundTaskManager` Mounted on `AppState`**:
   - Manages child process groups detached from the foreground ReAct loop.
   - Spawns child processes in isolated process groups (`setpgid(0, 0)` on Unix, `CREATE_NEW_PROCESS_GROUP` on Windows).
   - Directs stdout and stderr to disk file descriptors with `.stdin(Stdio::null())`.
   - Reaps exited children to eliminate `<defunct>` zombie processes.
2. **Harness-Owned Timeout Promotion Engine**:
   - `BashTool` supports `run_in_background: boolean` (default `true`).
   - Automatically promotes tasks taking longer than 5 seconds (`HARNESS_AUTO_PROMOTE_TIMEOUT`) to the background, returning `is_error: false`.
   - Single-spawn supervisor pattern ensures the child process is never dropped or double-spawned on promotion.
   - Foreground override (`run_in_background: false`) executes up to a 120-second hard ceiling (`HARNESS_FOREGROUND_HARD_TIMEOUT`).
3. **Router Wake-Up & 8-State Handling Matrix**:
   - Central `vox-router` routes `VoxEvent::TaskFinished` across all 8 states.
   - `Ready`: Checks `last_user_activity_ms < 5m`; dispatches proactive spoken announcement immediately (0ms delay) if no other tasks run.
   - `Listening`, `Thinking`, `Working`: Calls `inbox.push_task_notification()` without resetting `speech_hold_active`.
   - `Speaking`: Queued until playback drains to `Ready`.
   - `Paused`, `Idle`: Updates database and emits `NotificationCreated` (Toast + Drawer) with zero voice audio.
4. **Sleep Standby Inhibition (`RULE-EVT-12`)**:
   - `spawn_idle_monitor` inhibits transition to `Sleeping` while background tasks run.
   - Re-arms the sleep timer upon task completion without deadlocking.
5. **Two-Step Disengage UX & Backend Guard (`RULE-EVT-13`)**:
   - `end_session(force: bool)` rejects unconfirmed teardown while tasks run.
   - Frontend `ConfirmModal` warns user before terminating process groups.
6. **Turso DB Schema v19 & Tier 4 Cache Parity (`RULE-HARN-15`)**:
   - `session_tasks` table persists task lifecycle with foreign key cascade to `sessions(id)`.
   - Session hard delete and startup sweeps purge `~/.vox/cache/tasks/<session_id>/`.

---

## 2. Resolved Architectural Decisions (Clarification Ledger)

1. **Single-Spawn Supervisor vs. Future-Drop (P0 Defect 1 Fix)**:
   - *Problem:* Splitting `execute()` and `promote_to_background()` in `tokio::select!` dropped the in-flight child future at 5 seconds, causing double spawns or lost processes.
   - *Resolution:* `BashTool::execute` spawns the process and immediately registers it with `BackgroundTaskManager::spawn_supervised_task`. The supervisor holds `tokio::process::Child`. `execute` awaits an exit receiver or promotion timer. When 5s elapses, it marks the task promoted without touching the process.
2. **Universal Parameter Scoped Strictly to Promotable Tools (P0 Defect 2 Fix)**:
   - *Problem:* Adding `run_in_background` to sub-millisecond tools like `set_title` or fast queries like `search_memory` polluted schemas and created a broken 120-second fallback in `executor.rs`.
   - *Resolution:* Only tools returning `supports_background_promotion() == true` (`bash`) include `run_in_background`. Non-promotable tools retain the 13s hard timeout in `executor.rs`.
3. **Speech-Hold Latch Isolation on `StepInbox` (P0 Defect 3 Fix)**:
   - *Problem:* Calling `inbox.push()` when a background task finished cleared `speech_hold_active`, waking the coordinator while the user was speaking in a held `Working` turn.
   - *Resolution:* Added `inbox.push_task_notification()`, which pushes messages to the queue without clearing `speech_hold_active`. Added drainage check in `evaluate_synthesis_latch` when entering `Ready`.
4. **`last_user_activity_ms` Tracking & Smart Coalescing (P0 Defect 4 Fix)**:
   - *Problem:* 5-minute quiet window in `Ready` could not be evaluated without tracking, and a flat 2-second delay added dead air to single tasks.
   - *Resolution:* Added `pub last_user_activity_ms: AtomicU64` to `PipelineAtomics`. When a task finishes in `Ready`, if `active_tasks_count == 0`, dispatch is immediate (0ms). Proactive query formats include `task_id={task_id}` to avoid duplicate turn drops.
5. **Cross-Platform Shell Resolution, Null Stdin & Zombie Reaping (P1 Defect 5 Fix)**:
   - *Problem:* Android target crashed on missing `/bin/bash`, interactive commands hung on stdin, and killed processes became OS zombies.
   - *Resolution:* Shell helper maps to `/system/bin/sh` on Android and `cmd /C` on Windows. Added `.stdin(Stdio::null())`. Supervisor actively awaits `child.wait().await` after `SIGKILL`.
6. **Sleep Inactivity Monitor Re-Arm Loop (P1 Defect 6 Fix)**:
   - *Problem:* Skipping `Sleeping` when tasks were active in `Paused` fell through to `state_rx.changed().await`, deadlocking the monitor.
   - *Resolution:* The monitor continues its loop and re-arms `tokio::time::sleep(paused_timeout)` without blocking on state changes.
7. **Backend Force Guard & Tier 4 Log Purge (P1 Defect 7 Fix)**:
   - *Problem:* Frontend modal existed without a backend guard, and task log directories leaked on session deletion.
   - *Resolution:* `ipc::pipeline::end_session` and `session::on_end` require `force: true` to kill tasks and unmount. `delete_session` and `run_startup_sweeps` remove `session_tasks_dir`.

---

## 3. Subagent Orchestration Workflow & Incremental Commit Protocol

Implementation follows **Pattern A (Red-Seam First / TDD)**.
Every batch is implemented sequentially with its own atomic git commit.

### Orchestration Roles & Boundaries
- **Backend Engineer**: Implements production types, `BackgroundTaskManager`, DB Schema v19, `bash` and `manage_task` tools, router handlers, and lifecycle hooks. Strictly forbidden from modifying `tests/**`.
- **Frontend Engineer**: Implements `ConfirmModal` integration, `active_tasks_count` tracking, and `end_session(force)` invocation.
- **Test Engineer**: Authors the 12 black-box seam tests in Phase 2 (verifies RED). Exclusively owns Phase 4 **Mutation Testing**: seeds mutants from the False-Green Table into production code, confirms tests turn RED, reverts mutants, and confirms green.
- **Independent Reviewer**: Evaluates full git diff with clean-room fresh eyes against specifications and invariants.

```
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 1: Minimal Signatures Stub                                       │
│ Agent: Backend Engineer                                                │
│ Action: Add public types, enums, trait methods & stubs in production   │
│ Commit Gate: git commit -m "feat(tasks): minimal signatures stub D5"   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 2: Black-Box Seam Tests (RED)                                    │
│ Agent: Test Engineer                                                   │
│ Action: Write 12 Subtests in harness_loop_test & database_boundary     │
│ Gate: Run cargo nextest -> EMPIRICALLY CONFIRM ALL 12 TESTS FAIL (RED) │
│ Commit Gate: git commit -m "test(tasks): add red seam tests for D5"    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 3: Atomic Production Implementation (GREEN)                      │
│ Agent: Backend Engineer & Frontend Engineer [FORBIDDEN FROM tests/**]  │
│                                                                        │
│ Batch 1: Schema v19, Tier 4 Log Purge, Paths & Atomics                 │
│ Commit Gate: git commit -m "feat(tasks): batch 1 schema v19 & paths"   │
│                                                                        │
│ Batch 2: BackgroundTaskManager & Process Group Supervisor              │
│ Commit Gate: git commit -m "feat(tasks): batch 2 background manager"   │
│                                                                        │
│ Batch 3: Tool Chassis: bash, manage_task & Promotion Engine            │
│ Commit Gate: git commit -m "feat(tasks): batch 3 bash & manage_task"   │
│                                                                        │
│ Batch 4: Event Router 8-State Matrix & StepInbox Integration           │
│ Commit Gate: git commit -m "feat(tasks): batch 4 router 8-state matrix"│
│                                                                        │
│ Batch 5: Inactivity Sleep Inhibition (RULE-EVT-12) & Re-Arm Loop       │
│ Commit Gate: git commit -m "feat(tasks): batch 5 sleep inhibition"     │
│                                                                        │
│ Batch 6: Two-Step Disengage UX (ConfirmModal) & Backend Force Guard    │
│ Commit Gate: git commit -m "feat(tasks): batch 6 two-step disengage"   │
│                                                                        │
│ Gate: Run cargo nextest -> EMPIRICALLY CONFIRM ALL 12 TESTS PASS (GREEN│
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 4: Mutation Proof & Verification                                 │
│ Agent: Test Engineer                                                   │
│ Action: Seed deliberate mutants from False-Green table one by one.     │
│ Gate: Confirm nextest fails RED for each mutant, then revert to GREEN. │
│ Commit Gate: git commit -m "test(tasks): verify mutation coverage D5"  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 5: Adversarial Review & Sign-Off                                 │
│ Agent: Independent Reviewer Agent (ZERO prior context / fresh eyes)    │
│ Action: Audit full git diff against specs, invariants & style guides   │
│ Output: Formal Review Report (PASS or BLOCKED)                         │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Implementation Batches & Blast Radius

### Batch 1: Schema v19, Tier 4 Log Purge, Paths & Atomics
*Scope:* Turso DB migration v19 (`session_tasks` table, foreign keys, indices), task log directory helpers under `utils/paths.rs`, Tier 4 cache purge in `persistence/sessions.rs` and `persistence/worker.rs`, and `last_user_activity_ms: AtomicU64` on `PipelineAtomics`.
*Files:*
- `app/src-tauri/src/persistence/schema.rs`
- `app/src-tauri/src/persistence/sessions.rs`
- `app/src-tauri/src/persistence/worker.rs`
- `app/src-tauri/src/utils/paths.rs`
- `app/src-tauri/src/pipeline/atomics.rs`
*Build Expectation:* Compiles clean. `TEST-D5-11` and `TEST-D5-12` turn GREEN.
*Definition of Done:* Schema version is 19; `session_tasks` table is created; deleting a session purges `session_tasks_dir`.

---

### Batch 2: `BackgroundTaskManager` & Process Group Supervisor
*Scope:* Asynchronous process group manager in `services/tasks/manager.rs`, cross-platform shell resolver, kernel file descriptor redirection, `.stdin(Stdio::null())`, supervisor task awaiting `child.wait().await` to reap zombies, and `kill_process_group` with `libc::SIGKILL` to negative PGID. Mount `pub tasks: Arc<BackgroundTaskManager>` on `AppState`.
*Files:*
- `app/src-tauri/src/services/tasks/mod.rs`
- `app/src-tauri/src/services/tasks/manager.rs`
- `app/src-tauri/src/core/state.rs`
- `app/src-tauri/src/core/events.rs` (`VoxEvent::TaskFinished`, `TaskStatus`, `TaskDto`)
*Build Expectation:* Backend compiles clean (`./scripts/verify-targets.sh`).
*Definition of Done:* Can spawn, supervise, stream output to disk, read log tails, and kill process groups cleanly.

---

### Batch 3: Tool Chassis: `bash`, `manage_task` & Promotion Engine
*Scope:* Implement `bash` tool with `run_in_background: boolean` parameter (default `true`) and single-spawn supervisor integration. Implement `manage_task` with actions `list`, `status`, `kill`, `kill_all`. Update `ToolDefinition` with `supports_background_promotion(&self) -> bool`. Add configurable promotion timeouts (`auto_promote_timeout`, `foreground_hard_timeout`) to `Harness` and `ToolExecutionContext`. Register tools in `harness/tools/registry.rs`.
*Files:*
- `app/src-tauri/src/harness/tools/mod.rs`
- `app/src-tauri/src/harness/tools/bash.rs`
- `app/src-tauri/src/harness/tools/manage_task.rs`
- `app/src-tauri/src/harness/tools/registry.rs`
- `app/src-tauri/src/harness/tools/executor.rs`
- `app/src-tauri/src/harness/chassis.rs`
- `app/src-tauri/src/harness/steps.rs`
*Build Expectation:* Backend compiles clean. `TEST-D5-01`, `TEST-D5-02`, `TEST-D5-03`, `TEST-D5-04`, `TEST-D5-05`, `TEST-D5-06` turn GREEN.
*Definition of Done:* Fast commands finish synchronously in <5s; slow commands promote at 5s with `is_error: false`; `run_in_background: false` waits up to 120s; `manage_task` lists, tails, and kills process groups.

---

### Batch 4: Event Router 8-State Matrix & `StepInbox` Integration
*Scope:* Route `VoxEvent::TaskFinished` through `vox-router` to `pipeline/assistant/tasks.rs`. Implement complete 8-state matrix. Add `push_task_notification(&self, message: InboxMessage)` on `StepInbox` (preserves `speech_hold_active`). In `Ready`, check `last_user_activity_ms < 5m` and dispatch proactive turn in 0ms if `active_tasks_count == 0` (or coalesce if >0). Drain undrained inbox tasks in `evaluate_synthesis_latch` when entering `Ready`. Emit `IpcEvent::NotificationCreated` (Toast + Drawer).
*Files:*
- `app/src-tauri/src/pipeline/router.rs`
- `app/src-tauri/src/pipeline/assistant/mod.rs`
- `app/src-tauri/src/pipeline/assistant/tasks.rs` (NEW)
- `app/src-tauri/src/pipeline/assistant/llm.rs`
- `app/src-tauri/src/harness/stages/inbox.rs`
*Build Expectation:* Backend compiles clean. `TEST-D5-07`, `TEST-D5-08`, `TEST-D5-09` turn GREEN.
*Definition of Done:* Proactive voice announcement in `Ready` when user active; toast-only in `Paused` and when user quiet; `Working` buffers into `StepInbox` without clobbering speech hold.

---

### Batch 5: Inactivity Sleep Inhibition (`RULE-EVT-12`)
*Scope:* Update `spawn_idle_monitor` in `pipeline/lifecycle.rs` to check `state.tasks.has_running_tasks()`. When active tasks exist in `Paused`, log inhibition and re-arm sleep timer without deadlocking on `state_rx.changed().await`. Parameterize timeouts in `spawn_idle_monitor_with_timeouts` for deterministic test verification.
*Files:*
- `app/src-tauri/src/pipeline/lifecycle.rs`
*Build Expectation:* Backend compiles clean.
*Definition of Done:* Background tasks prevent transition from `Paused` to `Sleeping`; monitor re-arms cleanly.

---

### Batch 6: Two-Step Disengage UX (`ConfirmModal`) & Backend Force Guard (`RULE-EVT-13`)
*Scope:* Update `ipc::pipeline::end_session` and `session::on_end` to take `force: bool`. Reject unforced disengage while tasks run. Expose `get_active_tasks_count` and `list_tasks` IPC commands. Update `VoiceSessionContext.tsx` to check active tasks, show `ConfirmModal`, and issue `end_session(force=true)` on confirmation.
*Files:*
- `app/src-tauri/src/ipc/pipeline.rs`
- `app/src-tauri/src/pipeline/assistant/session.rs`
- `app/src/services/pipelineService.ts`
- `app/src/shared/context/VoiceSessionContext.tsx`
- `app/src/store/sessionStore.ts`
*Build Expectation:* Full workspace compiles clean (`./scripts/verify-targets.sh` & `pnpm build`). `TEST-D5-10` turns GREEN.
*Definition of Done:* Pressing `Esc` or clicking disengage with active tasks opens `ConfirmModal`; confirmation invokes `end_session(force=true)`, kills process groups, and reaches `Idle`.

---

## 5. Phase 2b False-Green Mutation Table (10 Deliberate Mutants)

| # | Seam Test Function | Seam Binary | Deliberate Mutant (Injected by Test Engineer) | Expected Mutation Failure (RED) |
| :--- | :--- | :--- | :--- | :--- |
| **1** | `bash_tool_executes_fast_command_synchronously_without_backgrounding` | `harness_loop_test` | In `BashTool::execute`, promote all commands unconditionally without awaiting synchronous completion window | Fails: observation contains `"running_in_background"` for `<20ms` command |
| **2** | `bash_tool_auto_promotes_long_running_command_to_background_without_guardrail_strike` | `harness_loop_test` | In `BashTool::execute` / `executor.rs`, mark auto-promoted tool outcomes with `is_error: true` | Fails: `guardrail_strike == 1` and `is_error == 1` on auto-promotion |
| **3** | `bash_tool_foreground_override_waits_past_auto_promote_window_until_completion` | `harness_loop_test` | In `BashTool::execute`, ignore `run_in_background: false` and always use 5s auto-promotion timer | Fails: command promoted to background at 50ms instead of waiting for exit |
| **4** | `background_task_streams_stdout_and_stderr_directly_to_disk_log_without_buffering` | `harness_loop_test` | In `BackgroundTaskManager::spawn_supervised_task`, redirect only `stdout` to log file and leave `stderr` as `Stdio::null()` | Fails: log file contains `"STDOUT_LINE_1"` but misses `"STDERR_LINE_1"` |
| **5** | `manage_task_list_and_status_read_live_state_and_log_tail_from_disk` | `harness_loop_test` | In `ManageTaskTool`, return the first 50 lines (head) instead of seeking the last 50 lines (tail) of `log_path` | Fails: status output contains `"LINE_001"` instead of `"LINE_120"` |
| **6** | `manage_task_kill_terminates_entire_process_group_including_grandchildren` | `harness_loop_test` | In `kill_process_group`, change `libc::kill(-(pid as i32), libc::SIGKILL)` to `libc::kill(pid as i32, libc::SIGKILL)` (positive PID) | Fails: grandchild process remains alive (`kill(grandchild_pid, 0) == 0`) |
| **7** | `task_finished_in_ready_within_active_window_triggers_proactive_spoken_turn` | `harness_loop_test` | In `pipeline/assistant/tasks.rs` (`on_task_finished`), omit proactive turn dispatch when `state == InteractionState::Ready` | Fails: pipeline remains in `Ready`; zero LLM dispatches and zero TTS commands |
| **8** | `task_finished_in_ready_after_quiet_window_or_in_paused_suppresses_speech_and_emits_notification_only` | `harness_loop_test` | In `pipeline/assistant/tasks.rs`, bypass the 5-minute `last_user_activity_ms` check in `Ready` | Fails: assistant synthesizes speech when user has been quiet for >5 minutes |
| **9** | `task_finished_in_working_buffers_in_step_inbox_without_releasing_active_speech_hold` | `harness_loop_test` | In `pipeline/assistant/tasks.rs`, call `inbox.push(...)` (which sets `speech_hold_active = false`) instead of `inbox.push_task_notification(...)` | Fails: step boundary advances prematurely while user is mid-speech |
| **10** | `running_background_task_inhibits_paused_to_sleeping_offload_and_disengage_enforces_force_guard` | `harness_loop_test` | In `lifecycle.rs`, remove `!state.tasks.has_running_tasks()` guard before transitioning to `Sleeping` | Fails: pipeline transitions to `Sleeping` while background task is running |

---

## 6. Definition of Done (DoD)

1. **Compilation Gate**: `cd app/src-tauri && ./scripts/verify-targets.sh` succeeds with zero errors across all cross-targets.
2. **Architecture Lint Gate**: `./scripts/lint-arch.sh` passes with zero violations.
3. **Atomic Git Commit Gate**: Every phase and batch is committed with a separate atomic commit.
4. **Deterministic Seam Test Gate**: Full test suite passes under `cargo-nextest`:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
5. **Empirical Mutation Proof**: Test Engineer validates all 10 deliberate mutants fail RED before being reverted to GREEN on remote GPU server (`temp/server.txt`).
6. **Two-Step Disengage Validated**: Frontend UI renders `ConfirmModal`; backend rejects unforced `end_session` while tasks run.
7. **Clean Tier 4 Lifecycle**: Session deletion purges task log folders on disk.
8. **Independent Review Approval**: Independent Reviewer subagent audits full git diff with fresh eyes and issues a formal PASS report.
