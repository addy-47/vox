# IPC Command & Event Specification (Vox v2)

## 1. Scope & Architectural Invariants

### 1.1 Strict Service Boundary
- **Frontend Discipline**: React components and hooks must NEVER invoke `@tauri-apps/api/core` (`invoke`) or `@tauri-apps/api/event` (`listen`) directly. All calls route through strongly-typed singleton service modules in `src/services/` (e.g. `historyService.ts`, `memoryService.ts`, `eventsService.ts`).
- **Backend Discipline**: IPC handlers in `app/src-tauri/src/ipc/` are transport adapters only. They:
  1. Validate incoming parameters.
  2. Borrow shared state (`State<'_, Arc<AppState>>`) and connection (`AppState.db`).
  3. Delegate business and persistence logic to `persistence/` or `services/`.
  4. Write **zero raw SQL queries**.

### 1.2 Serialization & Parameter Contracts
- All command arguments use standard Tauri v2 `camelCase` deserialization: Rust signatures declare `snake_case` params (e.g. `session_id: i64`) and frontend invokes pass `camelCase` keys (e.g. `{ sessionId }`). Tauri maps between the two; sending the Rust `snake_case` spelling from the frontend misses the required key.
- All return payloads are strongly-typed Rust structs serializing to camelCase JSON.
- Handlers return `Result<T, VoxIpcError>` with explicit error categories (`Database`, `InvalidArgument`, `NotFound`, `Pipeline`, `Conflict`). `delete_project` maps guard failures to `InvalidArgument`, missing rows to `NotFound`; only genuine storage failures surface as `Database`.

---

## 2. Frontend-to-Backend Commands (`tauri::command`)

### 2.1 Projects Domain (`ipc/projects.rs`) — [NEW]
Manages workspace session grouping folders.

#### `get_projects()`
- **Purpose**: Returns all workspace projects ordered newest first.
- **Behavior**: Queries Turso `projects` table. Returns display name, ID slug, and timestamps. Always includes the default project (`id: 'default'`).

#### `create_project(name: String)`
- **Purpose**: Creates a new project category.
- **Behavior**: Validates non-empty name, generates a unique project slug/UUID, commits to Turso `projects`, and returns the created project record.

#### `rename_project(projectId: String, newName: String)`
- **Purpose**: Renames an existing project folder.
- **Behavior**: Validates non-empty name, updates display name in Turso `projects`, and returns success.

#### `delete_project(projectId: String)`
- **Purpose**: Permanently deletes an empty project.
- **Behavior**: Rejects the `'default'` project and any project with session count > 0 with `VoxIpcError::InvalidArgument`. Returns `VoxIpcError::NotFound` for unknown IDs. Otherwise executes hard delete on Turso `projects`.

---

### 2.2 History & Sessions Domain (`ipc/history.rs`) — [ALIGNED]
Manages conversational sessions, turns, and session continuation.

#### `create_session(projectId: Option<String>)` — [ALIGNED]
- **Purpose**: Prepares UI and backend state for a fresh session (triggered by user clicking "+ New Session").
- **Behavior**: If an active voice session is running (`InteractionState != Idle`), disengages the audio pipeline cleanly (`VoxEvent::EndSession`). Clears `conversation_id` to 0. Follows **lazy session persistence**: zero empty database rows are created upfront; Turso `sessions` row insertion is deferred until the first spoken turn. While `Idle`, **zero harness instances exist in memory**. When the user subsequently engages, frontend passes `sessionId: null` to `start_session(None)`.

#### `continue_session(sessionId: i64)` — [ALIGNED]
- **Purpose**: Fetches historical session turns and metadata to display a past session in the conversation view.
- **Behavior**: 
  1. Queries session metadata and turns from Turso database via `persistence::sessions`.
  2. If an active voice session is currently running, disengages it cleanly.
  3. Sets `current_session_id = sessionId`.
  4. Returns `{ session, turns }` to the frontend.
  5. **Zero Idle Harness Footprint**: Does NOT instantiate or seed working memory while `Idle`. When the user subsequently clicks "Engage", the frontend passes `sessionId` to `start_session(Some(sessionId))`, which boots the `HarnessSession` and seeds continuation context.
  6. Does not emit echo events.

#### `get_sessions(projectId: Option<String>)`
- **Purpose**: Returns sessions for a specific project or all active sessions.
- **Behavior**: Queries `sessions WHERE deleted_at IS NULL` (optionally filtered by `project_id`), ordered `is_pinned DESC, updated_at DESC`. Returns monotonic IDs, titles, timestamps, and pin flags, plus derived `turn_count` and `first_message` (first user turn text) powering rail ordering and the title→first-message→untitled fallback.

#### `get_turns(sessionId: i64)`
- **Purpose**: Retrieves all finalized dialog turns for a given session.
- **Behavior**: Queries `turns WHERE session_id = ? ORDER BY turn_id ASC`. Returns user transcripts and assistant replies in original sequence.

#### `update_session(sessionId: i64, title: Option<String>, isPinned: Option<bool>, projectId: Option<String>)` — [NEW]
- **Purpose**: Updates session metadata (rename, pin/unpin, move between projects).
- **Behavior**: Updates specified fields on `sessions` in Turso. A call with all fields `None` is a no-op returning success without emitting. Otherwise broadcasts `IpcEvent::SessionsChanged`.

#### `delete_session(sessionId: i64, hard: Option<bool>)` — [UNIFIED]
- **Purpose**: Deletes a session (supports soft trash delete and permanent hard delete).
- **Behavior**: 
  - If `hard == Some(true)` (permanent purge): executes `DELETE FROM sessions WHERE id = ?`. Cascades strictly to `turns` and `session_compactions` (`ON DELETE CASCADE`). Extracted facts in `memory_facts` remain intact with `session_id` set to `NULL`.
  - Otherwise (soft delete, the default when `hard` is omitted or `false`): sets `deleted_at = now()` on the session. Preserves all child rows for trash recovery.
  - Broadcasts `IpcEvent::SessionsChanged`.

#### `get_transcript_history()`
- **Purpose**: Retrieves the transient in-memory transcript buffer for the tray window.
- **Behavior**: Reads the FIFO `transcript_history` deque from `AppState.pipeline` without touching disk.

---

### 2.3 Personal Memory Domain (`ipc/memory.rs`) — [REFACTORED]
Manages the single evolving Personal Memory markdown document.

#### `get_personal_memory(projectId: Option<String>)`
- **Purpose**: Retrieves the consolidated Personal Memory markdown document.
- **Behavior**: Queries `personal_memory WHERE project_id IS ?` (or default if None). Returns raw markdown text, revision version number, and last consolidated timestamp.

#### `save_personal_memory(content: String, expectedVersion: u64, projectId: Option<String>)`
- **Purpose**: Saves direct manual text edits made to the Personal Memory document.
- **Behavior**: Optimistic concurrency check: updates content and increments version only if `version == expectedVersion`. If version drifted, rejects with `VoxIpcError::Conflict`. Broadcasts `IpcEvent::PersonalMemoryUpdated`.

#### `consolidate_personal_memory(comments: Option<Vec<String>>, projectId: Option<String>)` — [UNIFIED]
- **Purpose**: Merges active personal facts into the document or performs comment-driven LLM regeneration.
- **Behavior**: 
  - If `comments` provided: triggers LLM regeneration taking `[Current Document] + [User Comments]`.
  - If `comments` None: verifies ingestion queue is quiet, fetches all `status = 'active'` personal facts, merges them via LLM, marks facts `'consolidated'`, and updates the document.
  - Broadcasts `IpcEvent::PersonalMemoryUpdated`.

#### `get_active_facts(projectId: Option<String>)` — [NEW]
- **Purpose**: Returns all `status = 'active'` facts from `memory_facts` for memory graph visualization.
- **Behavior**: Queries all active fact rows (all `fact_type` values: `personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`), optionally scoped by `project_id` via the session join. Returns `Vec<FactRecord>` ordered by `created_at DESC`. Read-only; no working memory mutation.

---


### 2.4 Notifications Domain (`ipc/notifications.rs`)
Manages actionable system notifications and historical alerts (governed by `notifications-spec.md`).

#### Parameter Types: `NotificationFilter`
```rust
pub struct NotificationFilter {
    pub ids: Option<Vec<String>>,
    pub group_key: Option<String>,
    pub category: Option<String>,
    pub action_type: Option<String>,
}
```

#### `get_notifications()`
- **Purpose**: Returns all active (non-dismissed) notifications ordered newest first.
- **Behavior**: Queries `notifications WHERE status != 'dismissed' ORDER BY created_at DESC`. Returns full records with `group_key`, `category`, `severity`, `action_type`, `action_payload`, `title`, `message`, `status` (`'unread'` or `'read'`), `session_id`, `metadata`, `created_at`, and `updated_at`.

#### `mark_notifications_read(filter: Option<NotificationFilter>)`
- **Purpose**: Marks matching unread notifications as read.
- **Behavior**: Evaluates filter:
  - If `ids` provided: updates target IDs.
  - Else if `group_key` provided: updates all matching that correlation group.
  - Else if `category` provided: updates all matching that category.
  - Else if `action_type` provided: updates all matching that interaction type (`'interactive'` or `'receipt'`).
  - Else (omitted or empty): updates all rows `WHERE status = 'unread'`.
  Updates `SET status = 'read', updated_at = ? WHERE status = 'unread'`. Clears unread badge counts in frontend.

#### `dismiss_notifications(filter: Option<NotificationFilter>)`
- **Purpose**: Dismisses matching active notifications from the drawer.
- **Behavior**: Evaluates filter:
  - If `ids` provided: dismisses target IDs.
  - Else if `group_key` provided: dismisses all matching that correlation group.
  - Else if `category` provided: dismisses all matching that category.
  - Else if `action_type` provided: dismisses all matching that interaction type (`'interactive'` or `'receipt'`), enabling tab-scoped dismissal.
  - Else (omitted or empty): dismisses all rows `WHERE status != 'dismissed'`.
  Updates `SET status = 'dismissed', updated_at = ?`.

#### `execute_notification_action(id: String, action: Option<String>)`
- **Purpose**: Polymorphic executor for actionable notification cards.
- **Behavior**: Loads notification record by `id`. Inspects its `category`, `session_id`, and `metadata`. Dispatches execution to the corresponding backend subsystem:
  - `session_compaction`: Dispatches `CompactionCoordinator::run_compaction_slice(sessionId, trigger_kind="manual")`. On completion, updates the notification in-place (`metadata.resolution = "resolved"`, updated `message`) and emits `NotificationUpdated`.
  - `memory_consolidation`: Dispatches `run_consolidation_once()`. On completion, updates the notification in-place (`metadata.resolution = "resolved"`, updated `message`) and emits `NotificationUpdated`.
  - `pipeline_error`: Dispatches error recovery / retry handler.
  Returns `Ok(())` on successful task launch. Does not mutate the notification's attention status (`unread`/`read`), but resolves task status in-place.



---

### 2.5 Pipeline & Audio Domain (`ipc/pipeline.rs` & `ipc/audio.rs`)
Controls the voice interaction lifecycle and hardware devices.

#### `launch_engine()` & `stop_engine()`
- **Purpose**: Initializes or completely shuts down the audio engine, VAD, and model workers.
- **Behavior**: Bootstraps CPAL streams, model weights, and hotkey listeners, or cleanly joins threads and releases mic hardware.

#### `start_session(sessionId: Option<i64>)` — [ALIGNED]
- **Purpose**: Transitions assistant from `Idle` to `Ready`, mounting the `HarnessSession`.
- **Behavior**: 
  - If `sessionId == Some(id)`: Mounts the `HarnessSession` continuing session `id`, loading continuation turns and the latest summary from Turso.
  - If `sessionId == None`: Mounts a fresh `HarnessSession` (`session_id = 0`, lazy DB row created on first turn).
  - Starts audio engine and dispatches `VoxEvent::SessionStart { owner: Assistant, session_id }` to `event_tx`.

#### `pause_session()`, `resume_session()`, `end_session()`
- **Purpose**: Transitions high-level assistant session states.
- **Behavior**: Dispatches strongly-typed commands (`PauseSession`, `ResumeSession`, `EndSession`) to the central FIFO `event_tx` Router. `end_session` unmounts and drops `HarnessSession`.

#### `ptt_start()`, `ptt_stop()`, `ptt_cancel()`
- **Purpose**: Controls Push-To-Talk voice windows.
- **Behavior**: Dispatches `PttStart`, `PttStop`, or `PttCancel` to `event_tx` for window validation and barge-in evaluation.

#### `test_clip(path: String, isPrivateMode: bool)` & `test_clip_cancel()`
- **Purpose**: Runs synthetic developer test audio through the active pipeline.
- **Behavior**: Feeds WAV PCM frames into the pipeline without requiring physical microphone speech.

#### `list_audio_devices()`
- **Purpose**: Enumerates available host input microphones.
- **Behavior**: Queries CPAL host for device names and supported sample rates.

---

### 2.6 Settings & Setup Domain (`ipc/settings.rs` & `ipc/setup.rs`)
Manages configuration and model assets.

#### `get_settings()` & `update_setting(key: String, value: Value)` & `reset_settings()`
- **Purpose**: Reads, mutates, or resets application configuration.
- **Behavior**: Atomically updates `settings.json` with schema validation, hot-reloads live workers (VAD thresholds, speech rate, compute threads), and emits `IpcEvent::SettingsUpdated`.

#### `get_model_catalog()` & `get_provider_caps()`
- **Purpose**: Queries verified models and dynamic provider capabilities.
- **Behavior**: Reads canonical models manifest and inspects hardware acceleration support.

#### `manage_models(action: String, modelId: String)` & `check_updates()`
- **Purpose**: Downloads, verifies SHA256 integrity, extracts, or deletes local model weight archives.
- **Behavior**: Manages background download workers and emits `model_progress` IPC events.

---

### 2.7 Tray & Voice Cloning Domain (`ipc/tray.rs` & `ipc/voices.rs`)
Window visibility and custom TTS voice management.

#### `toggle_tray_visibility_internal()` & `hide_tray_window()` & `show_main_window()`
- **Purpose**: Manages desktop floating tray and main window visibility.
- **Behavior**: Controls native WebKitGTK / platform window visibility and focus states.

#### `list_voices()`, `add_voice_from_file()`, `add_voice_from_recording()`, `delete_voice()`, `rename_voice()`
- **Purpose**: Custom voice profile CRUD for Sherpa-ONNX / Kokoro TTS.
- **Behavior**: Manages reference WAV audio files and metadata in Turso `voices` table.

---

## 3. Permanently Decommissioned IPC Commands

The following 10 legacy commands are permanently purged:
1. `get_memory_graph_topology` (scrapped with 3D canvas)
2. `get_graph_version` (scrapped with 3D canvas)
3. `get_memory_fact_detail` (scrapped with 3D canvas)
4. `get_unresolved_conflicts` (scrapped with NLI conflict model)
5. `resolve_memory_conflict` (scrapped with NLI conflict model)
6. `manage_memory_fact` (scrapped with 6-collection taxonomy)
7. `get_memory_queue_status` (replaced by internal async worker)
8. `retry_failed_queue_items` (replaced by automatic boot recovery)
9. `toggle_pipeline_processing` (subsumed by standard settings toggle)
10. `commit_session_to_history` (purged; backend pipeline owns transcript history directly)

---

## 4. Backend-to-Frontend Events (`IpcEvent`)

Every event emitted by the backend via `emit_ipc` or `emit_ipc_to` is mapped directly in `src/services/eventsService.ts` via `IpcEventMap`.

| Event Name | Payload Struct | Description |
|---|---|---|
| `state_changed` | `StateChangedPayload { owner, state, turn_id }` | SSOT for all pipeline & dictation FSM transitions (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`, `Sleeping`, `Working`). |
| `transcript_partial` | `TranscriptPayload { turn_id, text, owner? }` | Real-time interim streaming transcription for subtitle display. |
| `transcript_final` | `TranscriptPayload { turn_id, text, owner? }` | Finalized turn STT transcript. |
| `llm_token` | `LlmTokenPayload { turn_id, token }` | High-frequency streaming text token delta for live assistant response render. |
| `model_progress` | `ModelProgressPayload { model_id, step, progress, bytes_downloaded, total_bytes, error }` | Real-time download/extraction progress for model management. |
| `telemetry` | `TelemetryData { energy, vad_prob, low, mid, high }` | 60Hz audio frequency visualizer data. |
| `system_stats` | `SystemStatsPayload { system_cpu, system_ram_pct, vox_cpu, vox_ram_mb, threads, ... }` | CPU/RAM resource usage metrics for profiler drawer. |
| `show_toast` | `ToastPayload { title, message, level, duration_ms? }` | Ephemeral toast popups for user feedback. |
| `notification_created` | `NotificationRecord { id, group_key, category, severity, title, message, status, ... }` | Emitted when a persistent actionable notification or alert is created. |
| `notification_updated` | `NotificationRecord { id, group_key, category, severity, title, message, status, ... }` | Emitted when an active notification status changes (e.g. marked read or updated). |
| `personal_memory_updated`| `PersonalMemoryRecord { id, project_id, content, version, last_consolidated_at, updated_at }` | Emitted when Personal Memory is consolidated, edited, or regenerated. |
| `sessions_changed` | `void` | Signals frontend when sessions are updated asynchronously / out-of-band by the backend (e.g. background title generation or compaction cleanup). Frontend refetches the session list. |
| `settings-updated` | `void` | Signals frontend that application settings were hot-reloaded. |
| `toggle_tray` | `void` | Toggles tray drawer visibility. |

### Permanently Decommissioned IPC Events
The following backend-to-frontend echo events are permanently deleted:
1. `notification_dismissed` (decommissioned; frontend updates its local store upon successful `dismiss_notification` invoke promise).
2. `notifications_marked_read` (decommissioned; frontend updates its local unread badges upon successful `mark_notifications_read` invoke promise).
3. `sessions_changed` on `continue_session` and synchronous user-driven CRUD (decommissioned; frontend updates its state and triggers refetches upon awaiting the invoke promise without backend echo events).
