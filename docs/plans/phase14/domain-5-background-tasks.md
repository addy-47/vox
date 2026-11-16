# Domain 5: Background Tasks, Timeout Promotion & Router Wake-Up
## Hybrid Specification, Architectural Trace & Verification Blueprint

---

## 1. Executive Summary & Lane Definition

In voice-driven agent runtimes, long-running operations (>5s) cause dead air and break conversational fluency.
Domain 5 establishes the background task execution architecture for Vox Harness v2:
1. **`BackgroundTaskManager`**: Supervises asynchronous OS child processes detached from the ReAct loop inside the Tokio runtime.
2. **Timeout Auto-Promotion**: Replaces monolithic hard timeouts with tool-declared execution profiles and automatic background promotion.
3. **OS Kernel Stream Redirection**: Streams standard output and standard error directly to disk via kernel file descriptors with zero userspace memory overhead.
4. **Router Wake-Up & 8-State Handling Matrix**: Serializes background task completion events through `vox-router` across all 8 interaction states.
5. **Sleep Standby Inhibition (`RULE-EVT-12`)**: Inhibits the idle sleep timer while background tasks remain active.
6. **Two-Step Disengage UX (`ConfirmModal`)**: Guards session termination with modal confirmation before killing active child process groups.
7. **Built-in `bash` & `manage_task` Tools**: Minimal, robust execution primitives providing authentic agent capabilities and black-box verification.
8. **Turso DB Schema v19**: Relational persistence of task lifecycle and step provenance in `session_tasks`.

---

## 2. Ground-Truth Recon: Spec vs. Code vs. Reality Audit

| Subsystem | Existing Spec / Assumption | Codebase Reality | Architectural Verdict |
| :--- | :--- | :--- | :--- |
| **Tool Execution Timeout** | 13-second monolithic timeout (`TOOL_EXECUTION_TIMEOUT`) hardcoded in `loop.rs`. | Commit `f2a3e17` added a 13s constant to survive slow Nexus searches. Fast commands block the audio thread; slow commands fail abruptly. | **Replace with configurable execution profiles**. Fast commands wait up to `wait_ms_before_async`; slow commands promote to background. |
| **Process Execution** | No native OS command tool exists in `services/harness/tools/`. | Only mock tools (`SetTitleTool`, `MemorySearchTool`, `WebSearchTool`) exist. No tool can execute shell commands or manage background jobs. | **Implement minimal `bash` and `manage_task` tools** with exclusive barrier scheduling and process group ownership. |
| **Output Buffering** | High memory usage if command logs stream into userspace memory buffers. | Userspace channel streaming risks heap allocation and UI queue flood on verbose builds. | **OS Kernel File Redirection**: Point stdout and stderr file descriptors directly to disk (`~/.vox/cache/tasks/{session_id}/{task_id}.log`). |
| **Turn Boundary Ingestion** | `StepInbox` holds steering messages for the active turn. | `StepInbox` supports `Immediate` and `TurnBoundary` delivery, but only drains user speech. | **Extend router-to-inbox dispatch**: Inject background task completion messages into `StepInbox` as `Immediate` during `Working`, `Thinking`, and `Listening`. |
| **Sleep Standby** | Inactivity monitor auto-sleeps after 5m of paused state (`lifecycle.rs`). | `spawn_idle_monitor` transitions unconditionally to `Sleeping` after 5m in `Paused`, offloading LLM/TTS models from RAM. | **Enforce `RULE-EVT-12`**: Inhibit transition to `Sleeping` when `BackgroundTaskManager::has_running_tasks(session_id)` is true. |
| **Session Disengage** | `end_session` IPC command destroys harness immediately. | Unmounting harness drops active turn tokens, but orphan child processes continue executing indefinitely. | **Two-step disengage contract**: Intercept in UI if tasks run; backend `kill_all` sends `SIGKILL` to entire process groups on session end. |
| **Database Schema** | Current schema version is 18 (`schema.rs`). | Relational tables exist for `sessions`, `turns`, `session_steps`, and `session_tool_calls`, but no table records background tasks. | **Migrate to Schema v19**: Create `session_tasks` table with foreign key to `sessions(id)`. |

---

## 3. The 5 Mandatory Pivot Gates

### 3.1 Gate 1: Lifecycle & Command Orthogonality Check
Three distinct operational lifecycles must remain decoupled:
1. **Foreground Turn ReAct Loop**: Owned by `Harness` coordinator (`loop.rs`). Identified by `turn_id`. Lifecycle ends when model speaks or awaits user.
2. **Background Task Lifecycle**: Owned by `BackgroundTaskManager`. Identified by `task_id`. Outlives individual turns; runs as long as the OS process executes.
3. **Session Lifecycle**: Owned by `AppState` / `session.rs`. Identified by `session_id`. Persists conversation context and models in RAM.

**Invariants**:
- Finishing or aborting a foreground turn (`stop_turn`) MUST NOT terminate background tasks.
- Background task promotion MUST return an observation with `is_error: false`. It MUST NOT trigger guardrail strikes or turn failures.
- Ending a session (`end_session`) terminates all background tasks associated with that session to prevent orphan process leaks.

### 3.2 Gate 2: State Space Bijection (Zero-State & Role Occupancy Audit)

#### Canonical `TaskStatus` Enum
A task transitions only across bijective states. It is born in `Running`:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TaskStatus {
    Running {
        pid: u32,
        started_at: u64,
    },
    Completed {
        exit_code: i32,
        duration_ms: u64,
        completed_at: u64,
    },
    Failed {
        error: String,
        duration_ms: u64,
        failed_at: u64,
    },
    Killed {
        killed_at: u64,
    },
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        !matches!(self, TaskStatus::Running { .. })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running { .. } => "running",
            Self::Completed { .. } => "completed",
            Self::Failed { .. } => "failed",
            Self::Killed { .. } => "killed",
        }
    }
}
```

#### Task DTO for UI & Remote Projections
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDto {
    pub taskId: String,
    pub sessionId: i64,
    pub command: String,
    pub status: String,
    pub exitCode: Option<i32>,
    pub error: Option<String>,
    pub logPath: String,
    pub startedAt: u64,
    pub completedAt: Option<u64>,
    pub durationMs: Option<u64>,
}
```

#### Single Projection Rule
The frontend NEVER queries SQLite or joins local arrays to determine if tasks are active. `BackgroundTaskManager::active_tasks_count(session_id)` provides the canonical count.

---

### 3.3 Gate 3: The 4-Boundary End-to-End Traces

```
[Boundary 1: UI -> Backend IPC]
[Boundary 2: Backend Internal -> Event Router / Harness]
[Boundary 3: Backend -> OS Kernel / Process Group]
[Boundary 4: Backend -> Turso MVCC Storage]
```

#### Trace 1: Fast Command Execution (`wait_ms_before_async = 5000`)
1. **Boundary 1**: User asks assistant to run `echo "hello"`.
2. **Boundary 2**: Model emits `bash(command="echo 'hello'", wait_ms_before_async=5000)`.
3. **Boundary 3**: `bash` opens log file descriptor and spawns child process with `setpgid(0, 0)`.
4. **Boundary 3**: Child process exits in 15ms with code 0.
5. **Boundary 2**: `bash` reads stdout tail synchronously from log file.
6. **Boundary 2**: `bash` returns `ToolResult` with exit code and output inline. Turn proceeds to answer user.

#### Trace 2: Long Command Execution & Auto-Promotion
1. **Boundary 1**: User asks assistant to run `cargo test --release`.
2. **Boundary 2**: Model emits `bash(command="cargo test --release", wait_ms_before_async=5000)`.
3. **Boundary 3**: `bash` spawns child process in new process group. Kernel streams output directly to log file.
4. **Boundary 2**: 5000ms timer expires while child process still executes.
5. **Boundary 4**: Task registered in `session_tasks` table as `running`.
6. **Boundary 2**: Child process handle transferred to `BackgroundTaskManager`. Background Tokio task spawned to await exit.
7. **Boundary 2**: `bash` returns immediate success observation:
   ```json
   {
     "status": "running_in_background",
     "task_id": "task_1001_a9f1",
     "command": "cargo test --release",
     "log_path": "/home/user/.vox/cache/tasks/1001/task_1001_a9f1.log",
     "message": "Process running in background. Use manage_task to monitor or kill."
   }
   ```
8. **Boundary 2**: ReAct loop treats observation as normal tool output (`is_error: false`, no guardrail strike).
9. **Boundary 1**: Model speaks: *"I started the tests in the background. I will notify you when finished."* State transitions `Working` $\to$ `Speaking` $\to$ `Ready`.

#### Trace 3: Background Task Finishes in `Ready` (Proactive Announcement)
1. **Boundary 3**: Child process exits with code 0.
2. **Boundary 2**: Tokio supervisor detects exit. Computes duration.
3. **Boundary 4**: Updates `session_tasks` table: `status = 'completed'`, `exit_code = 0`, `duration_ms = 42150`.
4. **Boundary 2**: Removes task from active memory registry.
5. **Boundary 2**: Emits `VoxEvent::TaskFinished { task_id, session_id, status }` to central event queue.
6. **Boundary 1**: Router emits `IpcEvent::NotificationCreated` (OS toast + drawer card).
7. **Boundary 2**: Router inspects `InteractionState`. State is `Ready`. Quiet debounce window (<5m) is valid.
8. **Boundary 2**: Router initiates proactive turn: transitions `Ready` $\to$ `Thinking`.
9. **Boundary 1**: Model streams spoken response: *"Your background cargo test has finished successfully with zero errors."* State transitions `Thinking` $\to$ `Speaking` $\to$ `Ready`.

#### Trace 4: Session Disengage with Active Tasks (Two-Step Protection)
1. **Boundary 1**: User clicks Disengage or presses `Esc` while assistant is `Ready`.
2. **Boundary 1**: Frontend checks `active_tasks_count`. Count is 2 (>0).
3. **Boundary 1**: Frontend opens `ConfirmModal` (*"Disengage Session? You have 2 background processes running. Disengaging will terminate them."*).
4. **Boundary 1**: User clicks *"Terminate & Disengage"*.
5. **Boundary 1**: Frontend invokes `end_session(force=true)`.
6. **Boundary 2**: Router receives `VoxEvent::EndSession`.
7. **Boundary 2**: Router calls `BackgroundTaskManager::kill_all(session_id)`.
8. **Boundary 3**: Manager sends `libc::kill(-pgid, SIGKILL)` to all active process groups.
9. **Boundary 4**: Manager marks records as `killed` in `session_tasks`.
10. **Boundary 2**: Router unmounts harness, cools down models, transitions to `Idle`.

---

### 3.4 Gate 4: Asymmetry & Worst-Case Failure Matrix

| Failure Mode | Root Cause | Architectural Defense |
| :--- | :--- | :--- |
| **Grandchild Process Leak** | `bash` spawns subshells or child binaries (`cargo` -> `rustc` -> `linker`). Killing parent PID leaves children orphaned. | **Process Group Isolation**: Spawns child with `setpgid(0, 0)`. Termination sends `libc::SIGKILL` to negative PGID (`-pid`), nuking the entire tree. |
| **Pipe Buffer Deadlock** | Child process fills 64KB OS stdout pipe buffer because userspace is not actively reading. Process freezes. | **Kernel Stream Redirection**: Process stdout/stderr write directly to disk file descriptors via `Stdio::from(file)`. Zero pipe buffers in userspace. |
| **Simultaneous Task Exits** | Multiple background tasks exit within milliseconds of each other. | **Single-Writer MPSC Queue**: All completions funnel through `vox-router` channel. Router handles one exit at a time with a 2-second speech coalescing window. |
| **Barge-in on Proactive Speech** | User begins speaking (`SpeechStart`) just as assistant announces task completion. | **Standard Barge-in Protocol (`RULE-EVT-08`)**: Assistant audio mutes immediately (<30ms). Turn token cancels. State transitions cleanly to `Listening`. |
| **Sustained Idle Sleep Drift** | System attempts to offload models from RAM while a background task compiles code. | **Sleep Invalidation (`RULE-EVT-12`)**: `spawn_idle_monitor` checks `BackgroundTaskManager::has_running_tasks()`. Transition to `Sleeping` is inhibited. |
| **Log Disk Exhaustion** | Runaway command writes gigabytes of infinite loop output to log file. | **100MB File Ceiling & Startup LRU Sweep (`RULE-HARN-15`)**: Task log writes snap to session cache ceiling. Wiped on session deletion. |

---

### 3.5 Gate 5: Spec-Wide Referential Integrity Sweep
- No legacy 13-second constants in harness loop.
- No direct database queries from React components.
- State machine aligns with `events-spec.md` 8-state model.
- Tool schemas conform to `harness-spec.md §4.5`.
- Database schema matches `db-spec.md` conventions.

---

## 4. Part I: `BackgroundTaskManager` Architecture

### 4.1 System Placement & Threading Model
Per `architectural-invariants.md §3.3`, background process supervision is asynchronous I/O.
`BackgroundTaskManager` lives inside the **Tokio async runtime** within `AppState`:
```rust
pub struct BackgroundTaskManager {
    active_tasks: Arc<parking_lot::RwLock<HashMap<String, ActiveTaskRecord>>>,
    db: Arc<VoxDb>,
    event_tx: parking_lot::Mutex<Option<std::sync::mpsc::Sender<VoxEvent>>>,
}
```

```rust
pub struct ActiveTaskRecord {
    pub task_id: String,
    pub session_id: i64,
    pub tool_call_id: Option<String>,
    pub command: String,
    pub pid: u32,
    pub log_path: PathBuf,
    pub started_at: u64,
    pub cancel_token: tokio_util::sync::CancellationToken,
}
```

### 4.2 Lifecycle Methods
1. **`spawn_background_task`**:
   - Takes `session_id`, `tool_call_id`, `command`, `log_path`.
   - Opens file descriptor with `OpenOptions::new().create(true).append(true)`.
   - Spawns child process in dedicated process group (`setpgid(0, 0)` on Unix, `CREATE_NEW_PROCESS_GROUP` on Windows).
   - Records task in `session_tasks` table with status `'running'`.
   - Inserts record into `active_tasks`.
   - Spawns Tokio supervisor task awaiting child exit.
2. **`kill_task(task_id)`**:
   - Sends `SIGKILL` to `-pgid` on Unix (or `taskkill /pid {pid} /t /f` on Windows).
   - Cancels supervisor cancellation token.
   - Marks status as `'killed'` in `session_tasks`.
   - Removes task from `active_tasks`.
   - Sends `VoxEvent::TaskFinished` with `TaskStatus::Killed`.
3. **`kill_all(session_id)`**:
   - Iterates through all tasks matching `session_id` in `active_tasks`.
   - Terminates process groups and updates DB records to `'killed'`.
4. **`has_running_tasks(session_id) -> bool`**:
   - O(1) in-memory check against `active_tasks`.
5. **`active_tasks_count(session_id) -> usize`**:
   - Returns count of active tasks for `session_id`.
6. **`read_log_tail(log_path, max_lines) -> Result<String>`**:
   - Reads the last $N$ lines from disk using backward seek.

---

## 5. Part II: Minimal `bash` & `manage_task` Tools

### 5.1 Minimal `bash` Tool
- **Canonical Identifier**: `"bash"`
- **Domain**: `ToolDomain::Modular`
- **Concurrency Mode**: `ToolExecutionMode::Exclusive` (system mutation barrier)
- **Handles Own Timeout**: `true`
- **Truncation Policy**: `TruncationPolicy::TailHeavy { tail_ratio: 0.8 }`
- **Parameters Schema**:
  ```json
  {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "The exact shell command line string to execute."
      },
      "wait_ms_before_async": {
        "type": "integer",
        "description": "Milliseconds to wait synchronously before promoting to background. Defaults to 5000. Set to 0 to background immediately."
      }
    },
    "required": ["command"]
  }
  ```
- **Execution Flow**:
  1. Validates non-empty command.
  2. Resolves `wait_ms = args.get("wait_ms_before_async").unwrap_or(5000)`.
  3. Mints `task_id` (`format!("task_{}_{}", ctx.turn_id, nanoid))`.
  4. Resolves log path: `paths::task_log_path(ctx.session_id, &task_id)`.
  5. If `wait_ms == 0`: registers immediately in `BackgroundTaskManager`, detaches future, and returns `status: "running_in_background"`.
  6. If `wait_ms > 0`: races child process against `wait_ms` timeout:
     - **Process Exits Inside Window**: Reads output tail from log file on disk. Returns synchronous completion with exit code and output inline.
     - **Process Exceeds Window**: Transfers child handle to `BackgroundTaskManager`. Detaches supervisor. Returns `status: "running_in_background"` with `is_error: false`.

### 5.2 `manage_task` Tool
- **Canonical Identifier**: `"manage_task"`
- **Domain**: `ToolDomain::Modular`
- **Concurrency Mode**:
  - `ToolExecutionMode::Parallel` for read-only actions (`list`, `status`).
  - `ToolExecutionMode::Exclusive` for mutating actions (`kill`, `kill_all`).
- **Truncation Policy**:
  - `TruncationPolicy::TailHeavy { tail_ratio: 0.8 }` for `status`.
  - `TruncationPolicy::None` for `list`, `kill`, `kill_all`.
- **Parameters Schema**:
  ```json
  {
    "type": "object",
    "properties": {
      "action": {
        "type": "string",
        "enum": ["list", "status", "kill", "kill_all"],
        "description": "Action to perform on background tasks."
      },
      "task_id": {
        "type": "string",
        "description": "Unique task identifier (required for 'status' and 'kill')."
      }
    },
    "required": ["action"]
  }
  ```
- **Execution Flow**:
  - `"list"`: Returns all active tasks from in-memory registry plus recent historical tasks from `session_tasks` for `ctx.session_id`.
  - `"status"`: Reads status from manager (if running) or database (if terminal). Reads last 50 lines from log path on disk.
  - `"kill"`: Calls `BackgroundTaskManager::kill_task(task_id)`. Returns confirmation.
  - `"kill_all"`: Calls `BackgroundTaskManager::kill_all(ctx.session_id)`. Returns count of terminated tasks.

---

## 6. Part III: Turso DB Schema (Migration v19)

### 6.1 DDL Migration
```sql
CREATE TABLE IF NOT EXISTS session_tasks (
    task_id TEXT PRIMARY KEY,
    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    tool_call_id TEXT,
    command TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'killed')),
    exit_code INTEGER,
    error TEXT,
    log_path TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    duration_ms INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_session_tasks_session_id ON session_tasks(session_id);
CREATE INDEX IF NOT EXISTS idx_session_tasks_status ON session_tasks(session_id, status);
```

### 6.2 Schema Constants & Persistence Integration
- Update `pub const SCHEMA_VERSION: u32 = 19;` in `persistence/schema.rs`.
- Add table creation statement to `SCHEMA_TABLE_STATEMENTS`.
- Add conditional migration in `migrate_schema` for `current_version < 19`.
- Direct async updates from Tokio supervisor to ensure zero race conditions on fast task completions.

---

## 7. Part IV: Event Router & The Complete 8-State Matrix

### 7.1 `VoxEvent::TaskFinished`
```rust
pub enum VoxEvent {
    // ...
    TaskFinished {
        task_id: String,
        session_id: i64,
        status: TaskStatus,
    },
}
```

### 7.2 The 8-State Handling Matrix
All events pass through the single-writer `vox-router` channel.

| State | Router Action on `TaskFinished` | Audio / Voice Action | Alerts (`ToastAndNotification`) |
| :--- | :--- | :--- | :--- |
| **`Idle`** | Updates Turso DB record. | Zero audio. Assistant remains `Idle`. | Emits OS toast and Notification Drawer card. |
| **`Ready`** | Evaluates 5m quiet debounce window. | If user active (<5m): transitions `Ready` $\to$ `Thinking` $\to$ `Speaking` to speak status. If quiet (>5m): audio suppressed. | Emits OS toast and Notification Drawer card. |
| **`Listening`** | Injects event into `StepInbox` as `Immediate`. | Zero audio interruption. Does not interrupt user speech. | Emits OS toast and Notification Drawer card. |
| **`Thinking`** | Injects event into `StepInbox` as `Immediate`. | Zero audio interruption. Does not abort in-flight model stream. | Emits OS toast and Notification Drawer card. |
| **`Working`** | Injects event into `StepInbox` as `Immediate`. | Active tools continue running. Injected into context at step boundary. | Emits OS toast and Notification Drawer card. |
| **`Speaking`** | Holds event in router queue or coalesces. | Zero audio interruption. Never interrupts assistant voice. Evaluated when playback completes. | Emits OS toast and Notification Drawer card. |
| **`Paused`** | Updates Turso DB record. | Zero audio. Never speaks while session is paused by user. | Emits OS toast and Notification Drawer card. |
| **`Sleeping`** | State invalid under `RULE-EVT-12`. Fallback: updates DB. | Zero audio. | Emits OS toast and Notification Drawer card. |

### 7.3 Sleep Invalidation Rule (`RULE-EVT-12`)
- **RULE-EVT-12 (Sleep Standby Inhibition)**: While `BackgroundTaskManager::has_running_tasks(session_id)` is `true`, transition from `Paused` to `Sleeping` is strictly inhibited. The pipeline MUST NOT offload language model or speech synthesis weights from RAM while background processes are running.

### 7.4 Two-Second Speech Coalescing Window in `Ready`
When multiple background tasks finish simultaneously (e.g. parallel batch jobs), the router coalesces completions arriving within 2000ms into a single spoken announcement rather than spawning competing turn loops.

---

## 8. Part V: Two-Step Disengage UX (`ConfirmModal`)

### 8.1 User Interaction Flow
1. User presses `Esc` or clicks the Disengage button in `Ready` state.
2. Frontend queries `active_tasks_count` (tracked in store or via `get_active_tasks_count`).
3. If `active_tasks_count == 0`:
   - Frontend invokes `end_session()`.
   - Pipeline transitions immediately to `Idle`.
4. If `active_tasks_count > 0`:
   - Frontend renders `ConfirmModal`:
     - **Title**: *"Disengage Session?"*
     - **Description**: *"You have {N} background process(es) running. Disengaging will terminate them."*
     - **Confirm Label**: *"Terminate & Disengage"* (destructive styling)
     - **Cancel Label**: *"Keep Running"*
5. User clicks *"Terminate & Disengage"*:
   - Frontend invokes `end_session(force=true)`.
   - Backend calls `BackgroundTaskManager::kill_all(session_id)` sending `SIGKILL` to all process groups.
   - Pipeline unmounts harness and transitions to `Idle`.
6. User clicks *"Keep Running"*:
   - Modal closes. Session remains in `Ready`. Tasks continue executing.

---

## 9. Verification & Seam Test Matrix (Pattern A / TDD)

The Test Engineer validates Domain 5 using 10 black-box seam tests in `tests/harness_loop_test.rs`:

| Test ID | Seam Test Function Name | Tested Assertion | Expected Output |
| :--- | :--- | :--- | :--- |
| **TEST-D5-01** | `bash_tool_executes_fast_command_synchronously_without_backgrounding` | Command `< 5s` completes inside synchronous window. | Exit code 0, stdout returned inline, task not registered in background. |
| **TEST-D5-02** | `bash_tool_auto_promotes_long_running_command_to_background` | Command `> 5s` auto-promotes to background without error. | `status: "running_in_background"`, valid `task_id`, `is_error: false`. |
| **TEST-D5-03** | `background_task_streams_output_directly_to_disk_log` | Child writes stdout to kernel file descriptor. | Log file contains real-time stdout lines; zero userspace heap leaks. |
| **TEST-D5-04** | `manage_task_status_reads_log_tail_from_disk` | `manage_task(action="status")` reads backward seek from log file. | Returns last 50 lines of log file and current status. |
| **TEST-D5-05** | `manage_task_kill_terminates_entire_process_group` | Child spawns grandchild; `kill` terminates entire process tree. | Negative PGID receives `SIGKILL`; both parent and grandchild terminate; status `Killed`. |
| **TEST-D5-06** | `task_finished_in_ready_triggers_proactive_spoken_announcement` | Task completes while pipeline is `Ready` within active window. | Pipeline transitions `Ready` $\to$ `Thinking` $\to$ `Speaking`; model speaks completion. |
| **TEST-D5-07** | `task_finished_in_working_buffers_in_step_inbox_and_injects_at_step_boundary` | Task completes while tools run in `Working`. | Active tools complete; completion injected into context for Step $K+1$. |
| **TEST-D5-08** | `task_finished_in_paused_updates_db_and_emits_toast_without_audio` | Task completes while session is `Paused`. | DB updated; Toast + Drawer emitted; zero audio playback. |
| **TEST-D5-09** | `running_background_task_inhibits_sleep_state_transition` | Session paused with active background task for >5 minutes. | `RULE-EVT-12` prevents `Sleeping`; models remain warm in RAM. |
| **TEST-D5-10** | `disengage_with_running_tasks_requires_confirmation_and_kills_process_group` | Session disengage triggered while tasks run. | Rejects unconfirmed disengage; on confirm kills all process groups and reaches `Idle`. |

---

## 10. Human Sign-Off Gate

🛑 **ATTENTION DOMAIN 5 LEAD**:
This enriched Hybrid Specification & Architectural Trace document captures all architectural decisions, state transitions, boundary traces, and edge-case failure modes for Domain 5.

Per the `/architect` protocol, System Architect work halts here.
**Awaiting your formal review and sign-off before proceeding to `/create-plan` (Phase decomposition & mutation test specification).**
