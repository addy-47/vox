# Vox Backend v2 Implementation Checklist

> **Tracking SSOT** for Database & Persistence Spec (v2), Minimal Cognitive Memory Spec (v2), and IPC Command & Event Spec (v2).
> **Methodology**: Synthesis of `create-plan` (blast radius & dependency clustering) and `create-sprints` (persistent binary checklist).

---

## Batch 1: Schema Overhaul, Turso Initialization & DB Fixtures
**Dependencies**: None (Root foundation).  
**Expected Build State**: 100% Green (`cargo check` passes cleanly; new unit test in `schema.rs` validates DB creation and seed integrity).

- [x] `app/src-tauri/src/persistence/schema.rs`
  - [x] Rewrite `run_migrations`: Drop legacy obsolete tables (`memory_relations`, `personal_memory_queue`, `memory_pipeline_metrics`, old `memory_facts_vectors`, old `memory_facts`, old `session_compactions`, old `sessions`, old `turns`, old `notifications`).
  - [x] Add DDL statements for `projects` table (`id`, `name`, `created_at`, `updated_at`).
  - [x] Add seed statement for default project (`INSERT OR IGNORE INTO projects (id, name, created_at, updated_at) VALUES ('default', 'Default', ?, ?)`).
  - [x] Add DDL statements for `sessions` table (`id`, `project_id`, `title`, `is_pinned`, `deleted_at`, `created_at`, `updated_at`) and indexes (`idx_sessions_project_updated`, `idx_sessions_active`).
  - [x] Add DDL statements for `turns` table (`id`, `session_id`, `turn_id`, `user_text`, `assistant_text`, `created_at`) and index (`idx_turns_session_turn`).
  - [x] Add DDL statements for `session_compactions` table (`id`, `session_id`, `trigger_kind`, `from_turn_id`, `to_turn_id`, `compaction_output`, `status`, `error_msg`, `created_at`, `finished_at`) and index (`idx_compactions_session_status`).
  - [x] Add DDL statements for `personal_memory` table (`id`, `project_id`, `content`, `version`, `last_consolidated_at`, `updated_at`) and index (`idx_personal_memory_project`).
  - [x] Add DDL statements for `memory_ingestion_queue` table (`id`, `session_id`, `compaction_id`, `type`, `text`, `status`, `retry_count`, `error_msg`, `created_at`, `processed_at`) and index (`idx_queue_status_type`).
  - [x] Add DDL statements for `memory_facts` table (`id`, `session_id`, `compaction_id`, `type`, `text`, `status`, `created_at`, `updated_at`) and indexes (`idx_facts_status_type`, `idx_facts_session`).
  - [x] Add DDL statements for `memory_facts_vectors` table (`fact_id`, `type`, `status`, `project_id`, `created_at`, `embedding F32_BLOB(384)`) and index (`idx_vectors_filter`).
  - [x] Add DDL statements for `notifications` table (`id`, `category`, `title`, `message`, `status`, `session_id`, `metadata`, `is_read`, `created_at`) and index (`idx_notifications_status_cat`).
  - [x] Preserve `voices` table DDL (`id`, `name`, `source_kind`, `wav_path`, `voice_dir`, `created_at`, `preview_wav`) and index (`idx_voices_created`).
  - [x] Implement `pub async fn recreate_schema(conn: &Connection) -> Result<()>` helper for clean reset.
  - [x] Implement `pub async fn rebuild_fixture_db(path: &std::path::Path) -> Result<()>` to generate fresh fixtures.
  - [x] Add unit test `test_v2_schema_initialization` verifying table existence, foreign keys, and default project seed.
- [x] `app/src-tauri/src/persistence/db.rs`
  - [x] Verify `PRAGMA foreign_keys = ON;`, `PRAGMA busy_timeout = 5000;`, and `PRAGMA journal_mode = WAL;` (via `.query()`).
- [x] `app/src-tauri/tests/assets/test_vox.db`
  - [x] Rebuild binary fixture with fresh v2 schema and default seed data.
- [x] `app/src-tauri/benches/assets/bench_vox.db`
  - [x] Rebuild binary fixture with fresh v2 schema and default seed data.

---

## Batch 2: Legacy Subsystem Decommissioning & Pruning
**Dependencies**: Batch 1.  
**Expected Build State**: Intermediate Red during file removal $\to$ 100% Green once unlinked and stubbed.

- [x] [DELETE] `app/src-tauri/src/persistence/graph.rs`
  - [x] Purge `fetch_memory_graph`, `fetch_fact_detail`, `fetch_memory_conflicts`, `MemoryGraphPayload`, `MemoryConflictItem`, `MemoryEdgeTopology`, `MemoryNodeTopology`.
- [x] [DELETE] `app/src-tauri/src/persistence/memory_mutations.rs`
  - [x] Purge `enqueue_personal_facts`, `update_memory_fact`, `delete_memory_fact`, `reassign_memory_fact`, `supersede_user_fact`, `resolve_fact_conflict`.
- [x] [DELETE] `app/src-tauri/src/persistence/memory_queries.rs`
  - [x] Purge `fetch_memory_queue_status`, `fetch_facts_for_consolidation`, `MemoryQueueItem`, `MemoryQueueSummary`.
- [x] [DELETE] `app/src-tauri/src/persistence/memory_worker.rs`
  - [x] Purge `spawn_memory_worker`, legacy crossbeam worker loop, and automatic trigger passes.
- [x] [DELETE] `app/src-tauri/src/services/memory/ml/nli.rs`
  - [x] Purge DeBERTa NLI engine (`init_nli_engine`, `classify_batch`, `NliLabel`, `NliRelation`).
- [x] [DELETE] `app/src-tauri/src/services/memory/ml/edge_classifier.rs`
  - [x] Purge ModernBERT edge classifier (`init_edge_classifier`, `classify_edge`).
- [x] [DELETE] `app/src-tauri/src/services/memory/ml/scope_classifier.rs`
  - [x] Purge legacy query scope router (`init_scope_classifier`, `classify_scope`).
- [x] [DELETE] `app/src-tauri/src/services/memory/retrieval/`
  - [x] Delete `scope.rs` (query classification routing).
  - [x] Delete `search.rs` (waterfall 6-collection vector search).
  - [x] Delete `mod.rs` (retrieval facade).
- [x] `app/src-tauri/src/persistence/mod.rs`
  - [x] Remove `pub mod graph;`, `pub mod memory_mutations;`, `pub mod memory_queries;`, `pub mod memory_worker;`.
  - [x] Remove legacy re-exports: `MemoryConflictItem`, `MemoryEdgeTopology`, `MemoryFactDetail`, `MemoryGraphPayload`, `MemoryGraphQueryFilter`, `MemoryNodeTopology`, `mutations`, `queries`, `MemoryQueueItem`, `MemoryQueueSummary`.
- [x] `app/src-tauri/src/services/memory/ml/mod.rs`
  - [x] Remove submodules `edge_classifier`, `nli`, `scope_classifier`.
  - [x] Retain `embedder.rs` and `tokenizer.rs` for MiniLM 384-dim Stage 2 dedup.
- [x] `app/src-tauri/src/services/memory/mod.rs`
  - [x] Remove `pub mod retrieval;`.
  - [x] Remove obsolete exports: `classify_edge`, `init_edge_classifier`, `classify_batch`, `init_nli_engine`, `classify_scope`, `init_scope_classifier`, `retrieve_turn_profile`, `RetrievedProfile`, `MemoryFact`.
  - [x] Purge legacy thresholds (`NLI_*`, `EDGE_CLASSIFIER_*`, `SAME_COLLECTION_*`, `INTER_COLLECTION_*`, `SUBFLOOR_*`).
- [x] `app/src-tauri/src/monitoring/collector.rs`
  - [x] In `collect_profiler_snapshot`, hardcode `is_intra_edge_classifier_loaded: false` and `is_inter_edge_classifier_loaded: false`.
- [x] `app/src-tauri/src/services/harness/facade.rs`
  - [x] Remove lines 70-90 (automatic per-turn scope classification and profile retrieval).
  - [x] Remove unused `PrepareTurnParams.memory_tx`.
  - [x] Stub `enqueue_personal_facts` call sites as no-ops pending Batch 5.
- [x] `app/src-tauri/src/ipc/memory.rs`
  - [x] Replace decommissioned handlers (`get_graph_version`, `get_memory_graph_topology`, `get_memory_fact_detail`, `get_unresolved_conflicts`, `resolve_memory_conflict`, `manage_memory_fact`, `get_memory_queue_status`, `retry_failed_queue_items`, `toggle_pipeline_processing`) with minimal stubs returning `Err(VoxIpcError::Database("Decommissioned".into()))` so `lib.rs` and `ipc/` compile cleanly without depending on deleted files.
- [x] `app/src-tauri/src/lib.rs`
  - [x] Remove `spawn_memory_worker` bootstrap in setup (lines 352-358).
  - [x] Remove shutdown hook taking `state.memory_tx` (lines 746-747).
- [x] `app/src-tauri/src/core/state.rs` & `app/src-tauri/src/pipeline/assistant/`
  - [x] Cleanly decouple `memory_tx` from `AppState` and assistant session lifecycle.
- [x] `app/src-tauri/tests/session_lifecycle_test.rs`
  - [x] Align test assertions with removed `MemoryWorkerEvent` and legacy identity seeding.

---

## Batch 3: Persistence Facade & Hot-Path Offload (`persistence/`)
**Dependencies**: Batch 1 & 2.  
**Expected Build State**: 100% Green (`cargo check` passes cleanly).

- [x] `app/src-tauri/src/persistence/mod.rs`
  - [x] Update `PersistenceEvent` enum to target spec §1.4 (`SessionStarted`, `SessionEnded`, `TurnCompleted`, `UpdateSessionMetadata`, `Shutdown`).
  - [x] Register new domain modules: `projects`, `personal_memory`, `queue`, `facts`.
  - [x] Export strongly-typed domain structs and public functions.
- [x] [NEW] `app/src-tauri/src/persistence/projects.rs`
  - [x] Implement `ProjectRow` struct (`id`, `name`, `created_at`, `updated_at`).
  - [x] Implement `get_projects(conn: &Connection) -> Result<Vec<ProjectRow>>`.
  - [x] Implement `create_project(conn: &Connection, id: &str, name: &str) -> Result<ProjectRow>`.
  - [x] Implement `rename_project(conn: &Connection, project_id: &str, new_name: &str) -> Result<()>`.
  - [x] Implement `delete_project(conn: &Connection, project_id: &str) -> Result<()>` with 0-session check.
- [x] `app/src-tauri/src/persistence/sessions.rs`
  - [x] Update `SessionRow` struct (`id`, `project_id`, `title`, `is_pinned`, `deleted_at`, `created_at`, `updated_at`, `turn_count`, `first_message`).
  - [x] Update `TurnRow` struct (`id`, `session_id`, `turn_id`, `user_text`, `assistant_text`, `created_at`).
  - [x] Implement `create_session(conn: &Connection, project_id: Option<&str>) -> Result<i64>`.
  - [x] Implement `fetch_sessions(conn: &Connection, project_id: Option<&str>) -> Result<Vec<SessionRow>>` (filtering `deleted_at IS NULL`).
  - [x] Implement `fetch_session_by_id(conn: &Connection, session_id: i64) -> Result<Option<SessionRow>>`.
  - [x] Implement `fetch_turns(conn: &Connection, session_id: i64) -> Result<Vec<TurnRow>>`.
  - [x] Implement `update_session_metadata(conn: &Connection, session_id: i64, title: Option<&str>, is_pinned: Option<bool>, project_id: Option<&str>) -> Result<()>`.
  - [x] Implement `delete_session(conn: &Connection, session_id: i64, hard: bool) -> Result<()>` (soft delete vs hard delete cascade).
- [x] `app/src-tauri/src/persistence/compactions.rs`
  - [x] Implement `CompactionRecord` struct (`id`, `session_id`, `trigger_kind`, `from_turn_id`, `to_turn_id`, `compaction_output`, `status`, `error_msg`, `created_at`, `finished_at`).
  - [x] Implement `record_compaction_start(conn: &Connection, session_id: i64, trigger_kind: &str, from_turn_id: u32, to_turn_id: u32) -> Result<i64>`.
  - [x] Implement `record_compaction_finish(conn: &Connection, compaction_id: i64, compaction_output: &str, status: &str, error_msg: Option<&str>) -> Result<()>`.
  - [x] Implement `fetch_latest_compaction_run(conn: &Connection, session_id: i64) -> Result<Option<CompactionRecord>>`.
  - [x] Implement `fetch_turns_for_compaction(conn: &Connection, session_id: i64, from_turn_id: u32, to_turn_id: u32) -> Result<Vec<TurnRow>>`.
  - [x] Implement `commit_compaction_output(conn: &Connection, compaction_id: i64, output_json: &str, facts: &[(String, String)], session_id: i64) -> Result<()>`.
- [x] [NEW] `app/src-tauri/src/persistence/personal_memory.rs`
  - [x] Implement `PersonalMemoryRecord` struct (`id`, `project_id`, `content`, `version`, `last_consolidated_at`, `updated_at`).
  - [x] Implement `get_personal_memory(conn: &Connection, project_id: Option<&str>) -> Result<PersonalMemoryRecord>`.
  - [x] Implement `save_personal_memory(conn: &Connection, project_id: Option<&str>, content: &str, expected_version: i64) -> Result<PersonalMemoryRecord>` (optimistic locking).
  - [x] Implement `update_consolidated_memory(conn: &Connection, project_id: Option<&str>, new_content: &str) -> Result<PersonalMemoryRecord>`.
- [x] [NEW] `app/src-tauri/src/persistence/queue.rs`
  - [x] Implement `QueueItem` struct and `QueueStatus` enum.
  - [x] Implement `claim_pending_queue_batch(conn: &Connection, target_status: &str, next_status: &str, limit: usize) -> Result<Vec<QueueItem>>`.
  - [x] Implement `update_queue_item_status(conn: &Connection, id: i64, new_status: &str, error_msg: Option<&str>) -> Result<()>`.
  - [x] Implement `reconcile_crashed_queue_on_boot(conn: &Connection) -> Result<usize>`.
- [x] [NEW] `app/src-tauri/src/persistence/facts.rs`
  - [x] Implement `FactRecord` struct.
  - [x] Implement `insert_fact(conn: &Connection, fact: &FactRecord) -> Result<()>`.
  - [x] Implement `fetch_active_facts_by_type(conn: &Connection, fact_type: &str) -> Result<Vec<FactRecord>>`.
  - [x] Implement `deactivate_fact(conn: &Connection, fact_id: &str) -> Result<()>`.
  - [x] Implement `mark_facts_consolidated(conn: &Connection, fact_ids: &[String]) -> Result<()>`.
  - [x] Implement `insert_vector(conn: &Connection, fact_id: &str, fact_type: &str, status: &str, project_id: Option<&str>, embedding: &[f32]) -> Result<()>`.
  - [x] Implement `fetch_active_vectors_by_type(conn: &Connection, fact_type: &str) -> Result<Vec<(String, Vec<f32>)>>`.
- [x] `app/src-tauri/src/persistence/notifications.rs`
  - [x] Refactor notification CRUD to borrow `&Connection` instead of calling `open_readonly`.
- [x] `app/src-tauri/src/persistence/worker.rs`
  - [x] Update `PersistenceEvent` matching to handle `SessionStarted`, `SessionEnded`, `TurnCompleted`, and `UpdateSessionMetadata` (updating `title`, `project_id`, or `is_pinned`).
- [x] `app/src-tauri/src/core/state.rs` & `app/src-tauri/src/lib.rs`
  - [x] Expose `pub db: Arc<turso::Connection>` on `AppState`.
  - [x] Initialize `db` once during bootstrap in `lib.rs` and inject into `AppState`.

---

## Batch 4: Ingestion Queue & 2-Stage Deduplication Engine
**Dependencies**: Batch 3.  
**Expected Build State**: 100% Green (`cargo check` passes cleanly).

- [x] [DELETE] `app/src-tauri/src/services/memory/ingestion/stage3_eval.rs`
  - [x] Purge NLI and ModernBERT candidate evaluation and relation edge generation.
- [x] [DELETE] `app/src-tauri/src/services/memory/ingestion/stage4_commit.rs`
  - [x] Purge legacy 4th stage transaction commit and relational edge insertion.
- [x] `app/src-tauri/src/services/memory/ingestion/stage1_dedup.rs`
  - [x] Retain `jaccard_similarity(s1: &str, s2: &str) -> f32`.
  - [x] Implement `run_stage1_exact_dedup(conn: &Connection) -> Result<Stage1Summary>`.
  - [x] Atomically claim up to `STAGE1_BATCH_CEILING = 128` items with `status = 'pending'` (`status -> 'stage1_processing'`).
  - [x] Query active facts from `memory_facts` with matching `type`.
  - [x] Apply Winner-Takes-All: If Jaccard == 1.0, deactivate older fact in `memory_facts` (`status = 'inactive'`); advance incoming item to `status = 'stage1_done'`.
  - [x] If no exact match, advance incoming item to `status = 'stage1_done'`.
- [x] `app/src-tauri/src/services/memory/ingestion/stage2_embed.rs`
  - [x] Implement `run_stage2_cosine_dedup(conn: &Connection) -> Result<Stage2Summary>`.
  - [x] Atomically claim up to `STAGE2_BATCH_SIZE = 16` items with `status = 'stage1_done'` (`status -> 'stage2_processing'`).
  - [x] Generate 384-dim embedding via `generate_embedding(&item.text)` (MiniLM-L12 ONNX).
  - [x] Query active vectors from `memory_facts_vectors` with matching `type`.
  - [x] Apply Winner-Takes-All: If cosine similarity >= 0.95, deactivate older matching fact and vector (`status = 'inactive'`).
  - [x] Insert incoming fact into `memory_facts` (`status = 'active'`) and vector into `memory_facts_vectors` (`status = 'active'`).
  - [x] On success, transition queue item to `status = 'completed'`, `processed_at = now()`.
  - [x] On error, increment `retry_count`. If `retry_count >= 3`, transition to `status = 'failed'`, `error_msg = err`.
- [x] `app/src-tauri/src/services/memory/ingestion/runner.rs`
  - [x] Implement `run_ingestion_cycle(conn: &Connection) -> Result<IngestionCycleSummary>` executing Stage 1 followed by Stage 2.
  - [x] Implement `reconcile_crashed_queue_on_boot(conn: &Connection) -> Result<usize>` resetting in-flight items.
- [x] `app/src-tauri/src/services/memory/ingestion/mod.rs`
  - [x] Prune legacy structs (`RelationEdge`, `CandidateAuditLog`, `BatchEvaluationResult`).
  - [x] Export `run_ingestion_cycle`, `reconcile_crashed_queue_on_boot`, `Stage1Summary`, `Stage2Summary`, `IngestionCycleSummary`.

---

## Batch 5: Compaction Coordinator, Personal Memory & Session Continuation
**Dependencies**: Batch 3 & 4.  
**Expected Build State**: 100% Green (`cargo check` passes cleanly).

- [x] `app/src-tauri/src/services/memory/compaction/prompt.rs`
  - [x] Replace 6-collection prompt with the target unified JSON schema (`context_summary`, `personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`).
- [x] `app/src-tauri/src/services/memory/compaction/runner.rs`
  - [x] Refactor `CompactionResult` struct (`raw_json: String`, `context_summary: String`, `facts: Vec<(String, String)>`).
  - [x] Update `run_compaction` to parse and validate unified JSON.
- [x] `app/src-tauri/src/services/memory/compaction/coordinator.rs`
  - [x] Implement single-execution lock per session (`status = 'in_progress'`).
  - [x] Critical inline compaction (`CONTEXT_CRITICAL_THRESHOLD = 0.85`):
    - [x] Dispatches organic language transition filler phrase (`TRANSITION_MESSAGES_EN`, `TRANSITION_MESSAGES_HI`) to `TtsActor`.
    - [x] Binds to turn `CancellationToken` with 45s timeout.
    - [x] On failure after 2 retries, triggers FIFO fallback popping oldest turns until utilization < 85%.
  - [x] Opportunistic soft compaction (`CONTEXT_SOFT_THRESHOLD = 0.65`):
    - [x] Triggers on `0.65 <= util < 0.85` with 20-second quiet debounce in `Ready`/`Paused`.
    - [x] Aborts debounce immediately on speech onset or state transition.
  - [x] Session-end boundary / manual compaction:
    - [x] Creates persistent notification (`category: "session_compaction"`).
    - [x] Automatically executes if `auto_compaction == true` or awaits manual click if `false`.
  - [x] Commits raw JSON to `session_compactions(compaction_output)` and stages extracted facts into `memory_ingestion_queue`.
- [x] [NEW] `app/src-tauri/src/services/memory/personal.rs`
  - [x] Implement `consolidate_personal_memory(conn: &Connection, llm_provider: &dyn LlmProvider, comments: Option<Vec<String>>, project_id: Option<&str>) -> Result<PersonalMemoryRecord>`.
  - [x] Implement comment-driven regeneration when `comments` is provided.
  - [x] Implement facts merge when `comments` is None (verifying ingestion queue is quiet, fetching active personal facts, merging via LLM, and updating facts to `status = 'consolidated'`).
  - [x] Implement `export_personal_memory(conn: &Connection, target_path: &Path, project_id: Option<&str>) -> Result<()>`.
  - [x] Implement `import_personal_memory(conn: &Connection, source_path: &Path, project_id: Option<&str>) -> Result<PersonalMemoryRecord>`.
- [x] `app/src-tauri/src/services/harness/manager.rs` & `facade.rs`
  - [x] Update `ConversationManager` to hold Personal Memory markdown document and inject it into `assemble_system_prompt`.
  - [x] Enforce `MAX_SYSTEM_PROMPT_SHARE` token budget guard.
  - [x] Implement session continuation context restoration: load base prompt + personal memory + latest compaction context summary + uncompacted turns.
- [x] `app/src-tauri/src/services/memory/mod.rs` & Pipeline State Observer
  - [x] Implement 30-second debounced quiet idle observer triggering `run_ingestion_cycle(conn)` when pipeline is in `InteractionState::Ready` or `Paused`.

---

## Batch 6: IPC Transport Adapters & Event Registry Alignment
**Dependencies**: Batch 3, 4, 5.  
**Expected Build State**: 100% Green (`cargo check --release` and test suite pass).

- [ ] [NEW] `app/src-tauri/src/ipc/projects.rs`
  - [ ] Implement `get_projects(state: State<'_, Arc<AppState>>) -> Result<Vec<ProjectRow>, VoxIpcError>`.
  - [ ] Implement `create_project(name: String, state: State<'_, Arc<AppState>>) -> Result<ProjectRow, VoxIpcError>`.
  - [ ] Implement `rename_project(projectId: String, newName: String, state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError>`.
  - [ ] Implement `delete_project(projectId: String, state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError>`.
- [ ] `app/src-tauri/src/ipc/history.rs`
  - [ ] Implement `create_session(projectId: Option<String>, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<i64, VoxIpcError>`.
  - [ ] Implement `continue_session(sessionId: i64, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<Vec<TurnRow>, VoxIpcError>`.
  - [ ] Implement `get_sessions(projectId: Option<String>, state: State<'_, Arc<AppState>>) -> Result<Vec<SessionRow>, VoxIpcError>`.
  - [ ] Implement `get_turns(sessionId: i64, state: State<'_, Arc<AppState>>) -> Result<Vec<TurnRow>, VoxIpcError>`.
  - [ ] Implement `update_session(sessionId: i64, title: Option<String>, isPinned: Option<bool>, projectId: Option<String>, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError>`.
  - [ ] Implement `delete_session(sessionId: i64, hard: bool, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError>`.
  - [ ] Retain `get_transcript_history(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, VoxIpcError>`.
  - [ ] Purge `commit_session_to_history` permanently.
- [ ] `app/src-tauri/src/ipc/memory.rs`
  - [ ] Implement `get_personal_memory(projectId: Option<String>, state: State<'_, Arc<AppState>>) -> Result<PersonalMemoryRecord, VoxIpcError>`.
  - [ ] Implement `save_personal_memory(content: String, expectedVersion: i64, projectId: Option<String>, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<PersonalMemoryRecord, VoxIpcError>`.
  - [ ] Implement `consolidate_personal_memory(comments: Option<Vec<String>>, projectId: Option<String>, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<PersonalMemoryRecord, VoxIpcError>`.
  - [ ] Implement `export_personal_memory(targetPath: String, projectId: Option<String>, state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError>`.
  - [ ] Implement `import_personal_memory(sourcePath: String, projectId: Option<String>, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<PersonalMemoryRecord, VoxIpcError>`.
  - [ ] Purge all 9 decommissioned handlers and legacy types permanently.
- [ ] `app/src-tauri/src/ipc/notifications.rs`
  - [ ] Implement `get_notifications(state: State<'_, Arc<AppState>>) -> Result<Vec<NotificationRecord>, VoxIpcError>`.
  - [ ] Implement `mark_notifications_read(state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError>`.
  - [ ] Implement `dismiss_notification(id: String, state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError>`.
  - [ ] Implement `trigger_session_compaction(sessionId: i64, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError>`.
- [ ] `app/src-tauri/src/core/events.rs`
  - [ ] Add `PersonalMemoryPayload` struct (`content: String`, `version: i64`, `updated_at: i64`).
  - [ ] Add `SessionsChangedPayload` struct (`session_id: Option<i64>`, `action: String`, `title: Option<String>`).
  - [ ] Add `IpcEvent::PersonalMemoryUpdated(PersonalMemoryPayload)` (`"personal_memory_updated"`).
  - [ ] Add `IpcEvent::SessionsChanged(SessionsChangedPayload)` (`"sessions_changed"`).
  - [ ] Purge decommissioned events: `NotificationDismissed`, `NotificationsMarkedRead`.
- [ ] `app/src-tauri/src/ipc/mod.rs`
  - [ ] Register `pub mod projects;`.
- [ ] `app/src-tauri/src/lib.rs`
  - [ ] Register in `generate_handler!`:
    - `get_projects`, `create_project`, `rename_project`, `delete_project`,
    - `create_session`, `continue_session`, `update_session`,
    - `get_personal_memory`, `save_personal_memory`, `consolidate_personal_memory`, `export_personal_memory`, `import_personal_memory`.
  - [ ] Unregister from `generate_handler!`:
    - `get_graph_version`, `get_memory_graph_topology`, `get_memory_fact_detail`, `get_unresolved_conflicts`, `resolve_memory_conflict`, `manage_memory_fact`, `get_memory_queue_status`, `retry_failed_queue_items`, `toggle_pipeline_processing`, `commit_session_to_history`.
