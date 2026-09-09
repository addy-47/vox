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
- All command arguments use standard Tauri v2 `camelCase` deserialization in Rust signatures (e.g. `sessionId: i64`).
- All return payloads are strongly-typed Rust structs serializing to camelCase JSON.
- Handlers return `Result<T, VoxIpcError>` with explicit error categories (`Database`, `InvalidArgument`, `NotFound`, `Pipeline`).

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
- **Behavior**: Checks if any sessions reference `projectId`. If session count > 0, returns `VoxIpcError::InvalidArgument("Cannot delete project containing sessions")`. Otherwise executes hard delete on Turso `projects`.

---

### 2.2 History & Sessions Domain (`ipc/history.rs`) — [ALIGNED]
Manages conversational sessions, turns, and session continuation.

#### `create_session(projectId: Option<String>)` — [NEW]
- **Purpose**: Initializes a fresh conversational session (triggered by user clicking "+ New Session").
- **Behavior**: Creates a new row in Turso `sessions` with an untitled placeholder. Resets the in-memory working buffer (`ConversationManager`), loads the active `personal_memory` into the system prompt, sets `current_session_id`, and emits `IpcEvent::SessionsChanged`.

#### `continue_session(sessionId: i64)` — [NEW]
- **Purpose**: Restores a past session from the conversation list to continue conversation.
- **Behavior**: 
  1. Loads target session turns and the latest compaction summary from `session_compactions`.
  2. Seeds `ConversationManager` with: Base System Prompt + Personal Memory + Latest Compaction Summary + Uncompacted Turns.
  3. Updates `current_session_id`.
  4. Returns the restored turns and metadata to the frontend.

#### `get_sessions(projectId: Option<String>)`
- **Purpose**: Returns sessions for a specific project or all active sessions.
- **Behavior**: Queries `sessions WHERE deleted_at IS NULL` (optionally filtered by `project_id`), ordered `is_pinned DESC, updated_at DESC`. Returns monotonic IDs, titles, timestamps, and pin flags.

#### `get_turns(sessionId: i64)`
- **Purpose**: Retrieves all finalized dialog turns for a given session.
- **Behavior**: Queries `turns WHERE session_id = ? ORDER BY turn_id ASC`. Returns user transcripts and assistant replies in original sequence.

#### `update_session(sessionId: i64, title: Option<String>, isPinned: Option<bool>, projectId: Option<String>)` — [NEW]
- **Purpose**: Updates session metadata (rename, pin/unpin, move between projects).
- **Behavior**: Updates specified fields on `sessions` in Turso. Broadcasts `IpcEvent::SessionsChanged`.

#### `delete_session(sessionId: i64, hard: bool)` — [UNIFIED]
- **Purpose**: Deletes a session (supports soft trash delete and permanent hard delete).
- **Behavior**: 
  - If `hard == false` (soft delete): sets `deleted_at = now()` on the session. Preserves all child rows for trash recovery.
  - If `hard == true` (permanent purge): executes `DELETE FROM sessions WHERE id = ?`. Cascades strictly to `turns` and `session_compactions` (`ON DELETE CASCADE`). Extracted facts in `memory_facts` remain intact with `session_id` set to `NULL`.
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

#### `export_personal_memory(targetPath: String, projectId: Option<String>)` — [NEW]
- **Purpose**: Exports the current Personal Memory document to a local markdown file.
- **Behavior**: Reads document from Turso `personal_memory` and writes to specified file path on host disk.

#### `import_personal_memory(sourcePath: String, projectId: Option<String>)` — [NEW]
- **Purpose**: Overwrites or initializes the Personal Memory document from an external markdown file.
- **Behavior**: Reads markdown file from host disk, validates content, updates `personal_memory`, increments version counter, and broadcasts `IpcEvent::PersonalMemoryUpdated`.

#### `get_active_facts(projectId: Option<String>)` — [NEW]
- **Purpose**: Returns all `status = 'active'` facts from `memory_facts` for memory graph visualization.
- **Behavior**: Queries all active fact rows (all `fact_type` values: `personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`), optionally scoped by `project_id` via the session join. Returns `Vec<FactRecord>` ordered by `created_at DESC`. Read-only; no working memory mutation.

---


### 2.4 Notifications Domain (`ipc/notifications.rs`)
Manages actionable system notifications.

#### `get_notifications()`
- **Purpose**: Returns all active notifications ordered newest first.
- **Behavior**: Queries `notifications WHERE status IN ('pending', 'in_progress') ORDER BY created_at DESC`.

#### `mark_notifications_read()`
- **Purpose**: Clears unread notification badges.
- **Behavior**: Updates `notifications SET is_read = TRUE WHERE is_read = FALSE` in Turso.

#### `dismiss_notification(id: String)`
- **Purpose**: Dismisses an actionable notification card from the drawer.
- **Behavior**: Sets `status = 'dismissed'` in Turso for the target notification ID.

#### `trigger_session_compaction(sessionId: i64)`
- **Purpose**: Executes manual compaction from a session notification card action.
- **Behavior**: Updates notification `status = 'in_progress'`, invokes `CompactionCoordinator::run_compaction_slice(sessionId, trigger_kind="manual")`, and updates notification to `'completed'` on success.

---

### 2.5 Pipeline & Audio Domain (`ipc/pipeline.rs` & `ipc/audio.rs`)
Controls the voice interaction lifecycle and hardware devices.

#### `launch_engine()` & `stop_engine()`
- **Purpose**: Initializes or completely shuts down the audio engine, VAD, and model workers.
- **Behavior**: Bootstraps CPAL streams, model weights, and hotkey listeners, or cleanly joins threads and releases mic hardware.

#### `start_session()`, `pause_session()`, `resume_session()`, `end_session()`
- **Purpose**: Transitions high-level assistant session states.
- **Behavior**: Dispatches strongly-typed commands (`SessionStart`, `PauseSession`, `ResumeSession`, `EndSession`) to the central FIFO `event_tx` Router.

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
| `notification_created` | `NotificationRecord { id, category, title, message, status, ... }` | Emitted when a persistent actionable notification is created. |
| `notification_updated` | `NotificationRecord { id, category, title, message, status, ... }` | Emitted when an active notification status changes (e.g. `'in_progress'` $\to$ `'completed'`). |
| `personal_memory_updated`| `PersonalMemoryPayload { content, version, updated_at }` | **[NEW]** Emitted when Personal Memory is consolidated, edited, imported, or regenerated. |
| `sessions_changed` | `SessionsChangedPayload { session_id?, action, title? }` | **[UNIFIED]** Signals frontend when sessions are updated (`"title_updated"`, `"created"`, `"deleted"`, `"pinned"`). Allows surgical in-place title patches or list invalidation. |
| `settings-updated` | `void` | Signals frontend that application settings were hot-reloaded. |
| `toggle_tray` | `void` | Toggles tray drawer visibility. |

### Permanently Decommissioned IPC Events
The following 2 backend-to-frontend echo events are permanently deleted:
1. `notification_dismissed` (decommissioned; frontend updates its local store upon successful `dismiss_notification` invoke promise).
2. `notifications_marked_read` (decommissioned; frontend updates its local unread badges upon successful `mark_notifications_read` invoke promise).
