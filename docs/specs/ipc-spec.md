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

### 2.2 History & Persistence Domain (`ipc/persistence.rs`) — [ALIGNED]
Manages conversational history queries, turn records, and session metadata.

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
Manages the single evolving Personal Memory semantic model. The canonical representation is structured JSON; every command below that touches the document returns **rendered Markdown**, so the frontend never sees the canonical format.

#### `PersonalMemoryRecord` (storage row and IPC wire type)

One struct serves both roles. The DB column `content` holds the canonical semantic JSON; the record
additionally carries `markdown`, rendered from `content` at read time. `content` is marked
`#[serde(skip_serializing)]`, so serialization physically cannot emit the canonical form — the only
representation the frontend receives is `markdown`.

#### `get_personal_memory(projectId: Option<String>) -> PersonalMemoryRecord`
- **Purpose**: Retrieves the active Personal Memory rendered as Markdown.
- **Behavior**: Reads the active `personal_memory` row, deserializes `content` into the semantic model, and returns `render_to_markdown()` output with the revision version and last consolidated timestamp.

#### `save_personal_memory(content: String, expectedVersion: u64, projectId: Option<String>) -> PersonalMemoryRecord`
- **Purpose**: Saves direct manual edits made to the rendered Markdown document.
- **Behavior**: Parses `content` back into the semantic model with the deterministic Markdown→JSON converter (`## Title` → section, each paragraph → prose block, application assigns fresh persistent IDs), validates via `PersonalMemory::validate()`, then persists the canonical JSON under an optimistic concurrency check: the new version is written only if `version == expectedVersion`, otherwise `VoxIpcError::Conflict`. Broadcasts `IpcEvent::PersonalMemoryUpdated`.

#### `consolidate_personal_memory(comments: Option<Vec<String>>, projectId: Option<String>, forced: Option<bool>) -> ConsolidateOutcome` — [UNIFIED]
- **Purpose**: Integrates active personal observations into the semantic model, or stages comment-directed LLM edits.
```rust
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConsolidateOutcome {
    Completed { record: PersonalMemoryRecord },
    ConfirmationRequired { reason: ConfirmationReason, pending_count: i64 },
}
#[serde(rename_all = "snake_case")]
pub enum ConfirmationReason { CompactionInProgress, PendingQueueItems }
```
- **Behavior**:
  - If `comments` is provided: runs the comment-directed LLM pass over the current memory in handle format, stages resolved semantic operations into `personal_memory_revisions`, and returns `Completed`. This path is never gated by compaction state, queue state, or `forced`.
  - If `comments` is None:
    - If any `session_compactions.status = 'in_progress'` row exists, returns `ConfirmationRequired { reason: CompactionInProgress, .. }` with no side effects. The frontend disables the control in this state rather than offering a confirm affordance.
    - If `forced` is false/absent and unfinished `memory_ingestion_queue` items exist, returns `ConfirmationRequired { reason: PendingQueueItems, pending_count }` with no side effects. The frontend surfaces a confirm toast; confirming re-issues the command with `forced = true`.
    - With `forced = true`, candidate observations are snapshotted once (INVARIANT 5.3-C) and pending queue items are left `pending` to drain via the normal quiet observer. No inline ingestion cycle runs.
    - If no active memory exists: runs the cold-generation pass, synthesizes the complete semantic model from the snapshot, saves version 1 (`is_active = 1`), and marks exactly the snapshot `'integrated'`.
    - If an active memory exists: runs the incremental pass, resolves per-request handles to persistent IDs, and either stages the resolved operations as pending revisions (`suggestion_policy = "manual_review"`) or auto-commits non-destructive operations while holding deletions for confirmation (`suggestion_policy = "auto_apply"`). Marks exactly the snapshot `'integrated'`.
  - Broadcasts `IpcEvent::PersonalMemoryUpdated` on `Completed`.

#### `regenerate_personal_memory(projectId: Option<String>) -> PersonalMemoryRecord`
- **Purpose**: Re-synthesizes Personal Memory from all currently integrated facts.
- **Behavior**: Fetches all already-integrated facts (`type = 'personal' AND status = 'integrated'`), passes them to the whole-memory synthesis pass (`PERSONAL_COLD_GENERATION_SYSTEM_PROMPT`), assigns fresh `sec_*`/`blk_*` IDs to every entity, bulk-rejects any pending revisions that targeted superseded IDs, and inserts a new `personal_memory` record with `version = max_version + 1`, `is_active = 1`. Operates strictly over integrated facts, without touching active facts. Broadcasts `IpcEvent::PersonalMemoryUpdated`.

#### `get_personal_memory_versions(projectId: Option<String>) -> Vec<PersonalMemoryView>`
- **Purpose**: Lists all historical versions for carousel browsing.
- **Behavior**: Queries `personal_memory WHERE project_id IS ? ORDER BY version DESC`, renders each row's canonical JSON to Markdown, and returns `Vec<PersonalMemoryRecord>` including `version` and `is_active` flags.

#### `set_active_personal_memory_version(version: i64, projectId: Option<String>) -> PersonalMemoryRecord`
- **Purpose**: Restores a historical version as the active personal memory profile.
- **Behavior**: Inside a transaction, updates `is_active = 0` for all versions of that project and sets `is_active = 1` for the specified version, then renders the newly active row to Markdown. Broadcasts `IpcEvent::PersonalMemoryUpdated`.

#### `get_memory_revisions(projectId: Option<String>) -> Vec<MemoryRevisionView>` — [RENAMED from `get_memory_suggestions`]
- **Purpose**: Lists all pending semantic operations awaiting review.
- **Behavior**: Queries `personal_memory_revisions WHERE project_id IS ? AND status = 'pending' ORDER BY created_at ASC`. Returns `Vec<MemoryRevisionView>`, which carries `id`, `op` (`create_section` | `create_block` | `update_block` | `delete_block`), `target_id`, `status`, `created_at`, and a human-readable `preview` string resolved against the active semantic model (for example, `Create block in 'Vox Development': Addy is building…`).

#### `resolve_memory_revisions(request: ResolveRevisionsRequest) -> PersonalMemoryRecord` — [REFACTORED BATCH]
- **Purpose**: Atomically resolves a batch of pending memory revisions (accept and/or reject) in a single IPC roundtrip.
- **Parameters**:
  ```rust
  pub struct RevisionDecision {
      pub id: String,
      pub action: String, // "accept" | "reject"
  }
  pub struct ResolveRevisionsRequest {
      pub project_id: Option<String>,
      pub decisions: Vec<RevisionDecision>,
  }
  ```
- **Behavior**:
  - Validates that each item in `decisions` has `action == "accept"` or `action == "reject"`, else `VoxIpcError::InvalidArgument`. A decision targeting a non-pending revision yields `VoxIpcError::NotFound`.
  - Deserializes each accepted revision's `ResolvedOp` from its JSON payload and applies it to the active semantic model. Because operations address persistent semantic IDs rather than positional indices, **no re-anchoring of remaining pending revisions occurs and no descending-index sort is required**.
  - Per-operation rejection: an operation whose target ID no longer exists (deleted by another accepted operation in the same batch, or by a prior accept) is auto-rejected with a logged reason, and the rest of the batch still applies.
  - Validates the result with `PersonalMemory::validate()`. A violating result commits nothing — no version bump, no row status change — and returns `VoxIpcError::Engine`.
  - Inserts a new `personal_memory` row with `version = max_version + 1`, `is_active = 1`, and `last_consolidated_at = now()`; the previous version flips to `is_active = 0`.
  - Marks accepted revisions `'accepted'` and rejected revisions `'rejected'`, both with `resolved_at = now()`.
  - Commits all document and revision updates in a single atomic database transaction.
  - Broadcasts `IpcEvent::PersonalMemoryUpdated`.

#### `get_observations(projectId: Option<String>, status: Option<String>, limit: Option<u32>, offset: Option<u32>) -> Vec<ObservationRecord>`
- **Purpose**: Returns personal observations (`type = 'personal'`) from `memory_facts` across all or filtered statuses (`active`, `integrated`, `deactivated`), optionally scoped by `project_id`, with optional pagination (`limit`, `offset`).
- **Behavior**: Scopes queries to `type = 'personal'`. If `status` is supplied (e.g. `'active'`), queries rows matching that status. If omitted or null, returns all personal observations regardless of status ordered by `created_at DESC`. Applies `LIMIT` and `OFFSET` when provided for efficient windowed scrolling. Read-only; no working memory mutation.

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

#### `launch_engine()`, `stop_engine()`
- **Purpose**: Initializes or completely shuts down the 3-tier audio engine, VAD, and model workers. (Engine restarts are backend-owned via `restart_engine_inner` and triggered automatically by settings mutations).
- **Behavior**: 
  - `launch_engine()`: Bootstraps CPAL streams, model weights, and hotkey listeners in `Idle` state.
  - `stop_engine()`: Cleanly joins worker threads, terminates the central router pump, and releases mic hardware.

#### `start_session(sessionId: Option<i64>)` — [ALIGNED]
- **Purpose**: Transitions assistant from `Idle` to `Ready`, mounting the `HarnessSession`.
- **Behavior**: 
  - If `sessionId == Some(id)`: Mounts the `HarnessSession` continuing session `id`, loading continuation turns and the latest summary from Turso.
  - If `sessionId == None`: Mounts a fresh `HarnessSession` (minted epoch timestamp `conv_id`, with `SessionStarted` persisted on boot; zero-turn sessions swept on clean exit or restart).
  - Starts audio engine and dispatches `VoxEvent::SessionStart { owner: Assistant, session_id }` to `event_tx`.

#### `create_session(projectId: Option<String>)` — [ALIGNED]
- **Purpose**: Prepares UI and backend state for a fresh session (triggered by user clicking "+ New Session").
- **Behavior & Scenario Flow**:
  - **Scenario 1 (User is Idle)**: Clears `conversation_id` to 0 and clears `pipeline_accumulator`. While `Idle`, zero harness instances exist in memory. Upon subsequent voice or text interaction, `start_session(None)` mints an epoch monotonic session ID and dispatches `SessionStarted`.
  - **Scenario 2 (User is Not Idle)**: The frontend or `createSession` helper disengages the running session first (`disengageSession` / `endSession`), cleanly transitioning to `Idle`, recording `SessionEnded` in persistence, and resetting working memory for the fresh session.

#### `continue_session(sessionId: i64)` — [ALIGNED]
- **Purpose**: Fetches historical session turns and metadata to restore a past session into active working memory.
- **Behavior & Scenario Flow**:
  1. Sets `state.conversation_id = sessionId`.
  2. Queries session metadata (`SessionRow`) and all historical turns (`TurnRow[]`) from Turso SQLite (`~/.vox/vox.db`).
  3. Returns `{ session, turns }` to the frontend.
  - **Orchestration Contract (`selectSession` in Frontend)**:
    - **Scenario 1 (Currently Active Session)**: If `sessionId === activeSessionId`, immediate no-op.
    - **Scenario 2 (User is Idle)**: Fetches historical context (`continueSession`), populates `dialogueHistory`, and immediately auto-engages (`engageSession(sessionId)`) into `Ready` state so the user can speak or type right away.
    - **Scenario 3 (User is Not Idle)**: Disengages old session first (`disengageSession`), flushing accumulators and finalizing DB records, fetches new session context (`continueSession`), and immediately auto-engages (`engageSession(sessionId)`) into `Ready` state.

#### `pause_session()`, `resume_session()`, `end_session()`
- **Purpose**: Transitions high-level assistant session states.
- **Behavior**: Dispatches strongly-typed commands (`PauseSession`, `ResumeSession`, `EndSession`) to the central FIFO `event_tx` Router. `end_session` unmounts and drops `HarnessSession`.

#### `ptt_start()`, `ptt_stop()`, `ptt_cancel()`
- **Purpose**: Controls Push-To-Talk voice windows.
- **Behavior**: Dispatches `PttStart`, `PttStop`, or `PttCancel` to `event_tx` for window validation and barge-in evaluation.

#### `submit_text_input(query: String)`
- **Purpose**: Submits a typed user query directly into the active conversational pipeline, bypassing audio input, VAD, and STT.
- **Behavior**: Dispatches `VoxEvent::TextInput { text }` to the central `event_tx` Router. If the pipeline is `Paused`, the router auto-resumes (`ResumeSession` shared FX: `cancel_flag=false`, renewed turn token, `owner=Assistant`, VAD re-arm) and then processes the query as a `Ready`-state turn, so typed input is never silently dropped. `Idle`/`Sleeping` still drop. If the pipeline is in `Thinking`, `Speaking`, or `Working`, it invokes `on_interrupt()` to halt previous playback and vend a new turn before dispatching to LLM generation. TTS is synthesized and played back normally through the standard lifecycle.

#### `set_playback_muted(muted: bool)`
- **Purpose**: Toggles speaker audio output muting at the CPAL output sink layer.
- **Behavior**: Atomically updates `state.pipeline.is_playback_muted`. When true, CPAL hardware output buffer is filled with silence (`0.0`) while synthesis frame consumption and pipeline event timing proceed normally.

#### `set_mic_muted(muted: bool)`
- **Purpose**: Toggles microphone audio input gating at the CPAL input ingestion layer.
- **Behavior**: Atomically updates `state.pipeline.is_mic_muted` and recomputes `ingestion_gate`. When true, microphone audio frames are dropped before VAD.

#### `set_session_private_mode(enabled: bool)`
- **Purpose**: Toggles ephemeral temporary session mode (incognito / private mode) in memory without modifying `settings.json`.
- **Behavior**: Atomically updates `state.telemetry.is_private_mode`. While enabled, the persistence worker drops disk write events for turns and sessions, leaving conversations strictly in memory.

#### `list_audio_devices()`
- **Purpose**: Enumerates available host input microphones.
- **Behavior**: Queries CPAL host for device names and supported sample rates.

---

### 2.6 Settings & Setup Domain (`ipc/settings/{catalog,core,mutation}.rs` & `ipc/setup.rs`)
Manages configuration and model assets.

#### `get_settings()` & `update_setting(key: String, value: Value)` & `reset_settings()`
- **Purpose**: Reads, mutates, or resets application configuration.
- **Behavior**: Atomically updates `settings.json` with schema validation, hot-reloads live workers (VAD thresholds, speech rate, compute threads), and emits `IpcEvent::SettingsUpdated`. Supports provider-specific TTS sub-struct keys: `tts.chatterbox`, `tts.chatterbox_remote`, and `tts.zipvoice` (`{ voice_id: Option<String> }`).
- **Unknown Keys Are Rejected:** An unrecognised `(domain, key)` pair MUST return `InvalidArgument`. It MUST NOT return a success payload with `applied: false` — a rejected write is otherwise indistinguishable from an applied one, and the value is silently dropped. The frontend MUST surface a rejection to the user; a green "Saved" indicator for a value the backend refused is a correctness defect.
- **Reload Policy Is Backend-Owned and Must Be Executed:** Every accepted `(domain, key)` is classified by `config::get_setting_reload_policy` as `Hot`, `WorkerCommand`, or `Restart`. The classification is the SSOT; the frontend MUST NOT carry its own copy of the key list.
  - `Hot` — read from live state on each use; nothing further to do.
  - `WorkerCommand` — forwarded over the worker's command channel.
  - `Restart` — MUST actually rebuild the engine, not merely be reported. Providers cache their wiring (model path, credentials, thread pool, reference voice) for the engine's entire lifetime, so a `Restart`-classified write that is only reported leaves the running engine disagreeing with the persisted settings.
  - Every key `apply_setting_mutation` accepts MUST have an explicit arm. A key reaching the catch-all MUST be treated as a defect, not as a silent `Restart` default; `is_explicitly_classified` exists to make that testable.
- **Response Contract:** `update_setting` and `reset_settings` return `reload_policy: String` plus `restart_scheduled: bool`. `restart_scheduled` is `true` when the backend has taken responsibility for the rebuild. The frontend MUST render its restart affordance from these fields and MUST NOT infer reload need by comparing draft against saved settings.
- **Restarts Are Coalesced:** A single settings commit writes every dirty key in parallel, so multiple `Restart`-classified keys can land at once. Exactly one restart MUST be performed per burst; requests arriving during an in-flight rebuild MUST be absorbed by that rebuild rather than queued into a second one.
- **No Dead Policy State:** A policy classification the frontend cannot act on is dead weight. If a field is returned, it MUST have a consuming site, and a consuming site MUST NOT carry a duplicate of the backend table.

#### `get_model_catalog()` & `get_provider_caps()`
- **Purpose**: Queries verified models and dynamic provider capabilities.
- **Behavior**: Reads canonical models manifest and inspects hardware acceleration support. Canonical TTS provider IDs: `supertonic`, `kokoro`, `chatterbox`, `chatterbox_remote`, `edge_tts`, and `zipvoice`.
- **Capability Contract**: Returns `ProviderCaps { voices: ProviderVoiceSource, clone: bool }`. For `zipvoice`, caps are `{ voices: Custom, clone: false }`. Unknown provider IDs MUST return an explicit error and never silently fall through to default/catalog capabilities.
- **A Capability Field Must Be Consumed:** `ProviderCaps` carries only facts that vary between providers and that the frontend acts on. A boolean that is uniformly true across every provider is not a capability and MUST NOT be added — a field with no consuming site is dead weight that reads as a contract. Speed is deliberately absent: every provider supports it, so a `speed: bool` was uniformly `true` and read by nobody. Per-provider speed *ranges* are a separate concern, declared beside the clamp that enforces them.
- **Diffusion Steps & Guidance Scale Are Not Settings:** Each TTS provider synthesises at a fixed, per-provider validated step count and guidance scale declared as constants beside that provider's engine (`zipvoice` steps 4, guidance scale 1.0 — flow-distilled; `supertonic` 12; `chatterbox` 10; `chatterbox_remote` 10). There is no `tts.quality_steps` or `tts.guidance_scale` key in the UI. ZipVoice is flow-distilled (arXiv 2506.13053), where distillation exists specifically to eliminate classifier-free guidance — guidance scale is fixed to `1.0` (1 forward pass/step), avoiding 2x computational slowdown and metallic over-saturation.
- **Parameter Ranges Are Per-Provider:** Where a provider supports a tunable range, the range is declared beside the clamp that enforces it, not as a UI constant. `speed` is `0.7..=2.0` for every provider except `edge_tts`, which is `0.5..=2.0`.

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
- **Purpose**: Custom voice profile CRUD for Chatterbox and Sherpa-ONNX (Kokoro, ZipVoice).
- **Behavior**: Manages reference WAV audio files and metadata in Turso `voices` table. The `provider` argument scopes server-side: `edge`/`edge_tts` returns the live remote Edge list; `zipvoice` returns packaged ZipVoice pack rows; `chatterbox`/`chatterbox_remote` returns user-cloned and packaged custom rows; `supertonic`/`kokoro` return empty (catalog voices come from the manifest, never the DB); omitted returns the full table; an unrecognised id returns `InvalidArgument`. ZipVoice reads packaged directory sidecars (`voices/<slug>/clip.wav` + `reference.txt`); pack voice ids ARE the slugs (`atlas`), and `tts.zipvoice.voice_id` carries a slug — the `zipvoice_voice_<slug>` prefix convention is retired, and schema v9 migrates stored ids and display names to the slug form; arbitrary user-uploaded clone clips are disabled (`clone: false`) until database schema migration support for sidecar transcripts is added.

---

## 3. Permanently Decommissioned IPC Commands

The following 11 legacy commands are permanently purged:
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
11. `resolve_memory_suggestion` (singular single-revision form; superseded by the batch `resolve_memory_revisions` command, and never consumed by the frontend)

---

## 4. Backend-to-Frontend Events (`IpcEvent`)

Every event emitted by the backend via `emit_ipc` or `emit_ipc_to` is mapped directly in `src/services/eventsService.ts` via `IpcEventMap`.

| Event Name | Payload Struct | Description |
|---|---|---|
| `state_changed` | `StateChangedPayload { owner, state, turn_id, activity? }` | SSOT for all pipeline & dictation FSM transitions (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`, `Sleeping`, `Working`). The optional `activity` envelope identifies the non-terminal operation in progress — see §4.1. |
| `transcript_partial` | `TranscriptPayload { turn_id, text, owner? }` | Real-time interim streaming transcription for subtitle display. |
| `transcript_final` | `TranscriptPayload { turn_id, text, owner? }` | Finalized turn STT transcript. |
| `llm_token` | `LlmTokenPayload { turn_id, token }` | High-frequency streaming text token delta for live assistant response render. |
| `model_progress` | `ModelProgressPayload { model_id, step, progress, bytes_downloaded, total_bytes, error }` | Real-time download/extraction progress for model management. |
| `telemetry` | `TelemetryData { energy, vad_prob, low, mid, high }` | 60Hz audio frequency visualizer data. |
| `system_stats` | `SystemStatsPayload { system_cpu, system_ram_pct, vox_cpu, vox_ram_mb, threads, ... }` | One-second full launch-scope CPU and resident RAM usage. Release builds include the application and owned descendants; debug builds also include the `tauri dev` process tree. |
| `notification_created` | `NotificationRecord { id, group_key, category, severity, title, message, status, ... }` | Emitted when a persistent actionable notification or alert is created. |
| `notification_updated` | `NotificationRecord { id, group_key, category, severity, title, message, status, ... }` | Emitted when an active notification status changes (e.g. marked read or updated). |
| `personal_memory_updated`| `PersonalMemoryRecord { id, project_id, markdown, version, last_consolidated_at, updated_at }` | Emitted when Personal Memory is consolidated, edited, regenerated, or version-restored. Only `markdown` serializes; `content` never leaves the backend. |
| `turn_metrics` | `TurnMetricsPayload { turn_id, ttft_ms, ttfa_ms, total_voice_latency_ms, context_tokens_used, context_window }` | Key milestone latencies (TTFT, TTFA, end-to-end voice latency) and context utilization tokens emitted at the start of assistant turn playback. |
| `sessions_changed` | `void` | Signals frontend when sessions are updated asynchronously / out-of-band by the backend (e.g. session title assignment via `respond_and_set_title` or compaction cleanup). Frontend refetches the session list. |
| `settings-updated` | `void` | Signals frontend that application settings were hot-reloaded. |
| `toggle_tray` | `void` | Toggles tray drawer visibility. |

### 4.1 The `activity` Envelope

`StateChangedPayload.activity` identifies **which non-terminal operation** holds the pipeline in `Working`. It exists because `Working` covers more than tool calls: inline context compaction is a Harness stage with no `ToolFlow` and no `ToolRegistry` entry, so a `tool_name` field would be structurally wrong for it. The envelope is deliberately generic — a new non-terminal operation requires no schema change.

| Field | Type | Meaning |
|---|---|---|
| `kind` | `ActivityKind` = `"tool"` \| `"compaction"` | Discriminator for the operation class. `tool` for any registered `ToolFlow::NonTerminal` invocation; `compaction` for the Harness inline-compaction stage. |
| `name` | `String` | Free-form operation identity. For `kind: "tool"` this is the canonical `ToolDefinition::name()` (e.g. `web_search`, `search_memory`). For `kind: "compaction"` it is `compaction`. |
| `call_id` | `Option<String>` | The originating model tool-call id, present only for `kind: "tool"`. Omitted entirely when `None`. |

**Invariants:**

1. `activity` is `Some` **if and only if** `state === "Working"`. Every other state serializes it as absent.
2. `activity` is owned by the `InteractionOwner` that is `Working`. Dictation never enters `Working`, so dictation `state_changed` events always carry `None`.
3. Consumers MUST NOT branch on `name` alone. The display layer resolves activity through a three-tier cascade (`name` → `kind` → `interactionState`) so an unknown `name` degrades to its `kind` default rather than failing.
4. `kind` is additive. Adding a new member is permitted; consumers keyed on existing members are unaffected.

### Permanently Decommissioned IPC Events
The following backend-to-frontend echo events are permanently deleted:
1. `notification_dismissed` (decommissioned; frontend updates its local store upon successful `dismiss_notification` invoke promise).
2. `notifications_marked_read` (decommissioned; frontend updates its local unread badges upon successful `mark_notifications_read` invoke promise).
3. `sessions_changed` on `continue_session` and synchronous user-driven CRUD (decommissioned; frontend updates its state and triggers refetches upon awaiting the invoke promise without backend echo events).
4. `show_toast` (decommissioned; all floating ephemeral alerts are delivered via Native OS Desktop Notifications directly from the Rust backend across Linux, macOS, and Windows, completely retiring the custom webview toast window).
