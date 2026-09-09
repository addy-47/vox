# Vox Backend v2: Minimal Memory, Persistence & IPC Implementation Plan

**Target Specifications**:
1. [Database & Persistence Spec (v2)](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)
2. [Minimal Cognitive Memory Spec (v2)](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)
3. [IPC Command & Event Spec (v2)](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md)

**Accompanying Persistent Checklist**: [`CHECKLIST.md`](file:///home/addy/projects/apps/vox/CHECKLIST.md)

---

## 1. Target End-State & Architectural Principles

### 1.1 Target End-State
The Vox backend persistence, memory, and IPC layers are fully consolidated into a unified, zero-overhead, production-grade native pipeline:
1. **Embedded Database**: A pure-Rust, embedded Turso database with 9 normalized tables (`projects`, `sessions`, `turns`, `session_compactions`, `personal_memory`, `memory_ingestion_queue`, `memory_facts`, `memory_facts_vectors`, `notifications`) plus `voices`, operating under WAL mode with explicit foreign key cascades and zero C SQLite dependencies.
2. **Strict Persistence Boundary**: All database queries are isolated behind the `persistence/` facade. Direct calls to `VoxDb::open` outside bootstrap/fixtures are eradicated. The application borrows a single shared `Arc<turso::Connection>` held in `AppState.db`.
3. **Hot-Path Asynchronous Offload**: Turns and session metadata mutations stream asynchronously via `mpsc::Sender<PersistenceEvent>` to a background worker thread, ensuring the sub-200ms voice pipeline is never blocked by disk I/O.
4. **2-Stage Deduplication Engine**: A high-efficiency background fact dedup engine operating only when the system is quiet (30-second quiet debounce on `InteractionState::Ready`/`Paused`):
   - Stage 1: Exact word-set match via Jaccard 1.0 (batch ceiling 128) with Winner-Takes-All deactivation.
   - Stage 2: Semantic cosine match via MiniLM-L12 ONNX 384-dim embeddings (batch size 16, threshold 0.95) with Winner-Takes-All deactivation and denormalized vector table population.
   - Boot Crash Recovery: Automatically reconciles in-flight queue items on startup.
5. **Compaction Coordinator & Personal Memory Document**:
   - Working memory context compression via unified JSON output schema (`context_summary`, `personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`).
   - Three robust triggers: Critical (0.85 util with speech transition filler + FIFO fallback), Soft (0.65 util with 20s quiet debounce), and Session-End/Manual (with persistent notifications).
   - Single evolving markdown document (`personal_memory`) with optimistic concurrency version checking, comment-driven regeneration, and fact-merge consolidation.
   - Zero-overhead session continuation: seeds `ConversationManager` with Base Prompt + Personal Memory + Latest Compaction Summary + Uncompacted Turns.
6. **Unified IPC Transport**:
   - New `projects` domain for workspace folder grouping.
   - Aligned `history` domain with soft/hard delete and session continuation.
   - Refactored `memory` domain managing personal memory documents and disk export/import.
   - Purged 10 decommissioned commands, 2 echo events, and all legacy relational graph/NLI runtime code.

### 1.2 Non-Negotiable Constraints & Methodology
- **Backend Only**: Zero frontend code changes and zero new tests. Existing integration tests continue to compile and pass.
- **Zero Backward Compatibility (ZBC)**: Breaking interfaces is prioritized over transitional compatibility shims.
- **Simple DROP & CREATE**: Schema initialization applies clean DROP & CREATE for target tables; static test and bench database fixtures (`tests/assets/test_vox.db` and `benches/assets/bench_vox.db`) are regenerated.
- **Synthesis Methodology**: Blending `create-plan` (blast-radius and dependency clustering) with `create-sprints` (persistent binary checklist).

---

## 2. Grill-Me Inquiries & Resolved Architecture Decisions

During the architectural alignment pass, the following critical decisions were established:

1. **Deduplication Execution Cadence**:
   - *Decision*: Deduplication MUST run on a **debounced idle timer** when the pipeline is in `InteractionState::Ready` or `InteractionState::Paused` (30-second continuous quiet debounce).
   - *Rationale*: Even though MiniLM embedding is fast, running ONNX tensor math immediately during an active turn or mid-conversation risks audio buffer underruns on CPU-constrained machines (8GB RAM target).
2. **LLM Invocation for Compaction & Consolidation**:
   - *Decision*: Route through `create_llm_provider_from_llm_settings(&settings.llm, &model_path)` (or the shared provider in `AppState`), running as an isolated async task using `tokio::time::timeout(45s, ...)`, bound to a `CancellationToken`.
3. **Tagged Streaming Demuxer Scope**:
   - *Decision*: `harness-spec.md` is DRAFT / under discussion. This plan only implements the consumer hook (`PersistenceEvent::UpdateSessionMetadata { session_id, key: "title", value }` and the helper in `persistence/sessions.rs`). The streaming tag parser FSM in `actor.rs` is decoupled.
4. **Default Project Initialization**:
   - *Decision*: Schema bootstrap in `persistence/schema.rs` will automatically insert the default project (`id = 'default'`, `name = 'Default'`) immediately upon table creation to ensure foreign key integrity.
5. **Fixture Recreation Tooling**:
   - *Decision*: Provide reusable helper functions `recreate_schema` and `rebuild_fixture_db` in `persistence/schema.rs` to rebuild `test_vox.db` and `bench_vox.db` with seeded defaults.

---

## 3. Blast Radius & Dependency Topology

```
[Batch 1: Schema Overhaul & DB Fixtures]
        │
        ├──> [Batch 2: Legacy Subsystem Decommissioning]
        │            │
        └────────────┴───> [Batch 3: Persistence Facade & Hot-Path Offload]
                                  │
                 ┌────────────────┴────────────────┐
                 ▼                                 ▼
    [Batch 4: Ingestion & 2-Stage Dedup]   [Batch 5: Compaction & Personal Memory]
                 │                                 │
                 └────────────────┬────────────────┘
                                  ▼
             [Batch 6: IPC Transport & Event Alignment]
```

### Dependency Reasoning
- **Batch 1** is the structural foundation: tables, foreign key constraints, indexes, and fixture DBs must exist before any code queries them.
- **Batch 2** eliminates dead code (relational graph, NLI, ModernBERT, legacy memory worker crossbeam loop) that would otherwise conflict with the new persistence interfaces. Call sites in `ipc/memory.rs` and `collector.rs` are cleanly stubbed to preserve a green build.
- **Batch 3** implements the strict data access layer across all domain modules and wires `AppState.db`.
- **Batch 4** builds the 2-stage dedup engine upon the queue and fact tables provided by Batch 3.
- **Batch 5** builds compaction, personal memory, and session continuation upon Batches 3 & 4.
- **Batch 6** finalizes the public boundary by mapping Tauri commands and IPC events to the completed persistence and memory subsystems.

---

## 4. Batch Execution Sequence

### Batch 1: Schema Overhaul, Turso Initialization & DB Fixtures
- **Primary Goal**: Implement clean DROP & CREATE DDL for all target tables, indexes, and pragmas, and rebuild static fixtures.
- **Files Touched**:
  - `app/src-tauri/src/persistence/schema.rs`
  - `app/src-tauri/src/persistence/db.rs`
  - `app/src-tauri/tests/assets/test_vox.db`
  - `app/src-tauri/benches/assets/bench_vox.db`
- **Tables Initialized**:
  1. `projects` (`id`, `name`, `created_at`, `updated_at`) + default `'default'` seed.
  2. `sessions` (`id`, `project_id`, `title`, `is_pinned`, `deleted_at`, `created_at`, `updated_at`) + indexes `idx_sessions_project_updated`, `idx_sessions_active`.
  3. `turns` (`id`, `session_id`, `turn_id`, `user_text`, `assistant_text`, `created_at`) + index `idx_turns_session_turn`.
  4. `session_compactions` (`id`, `session_id`, `trigger_kind`, `from_turn_id`, `to_turn_id`, `compaction_output`, `status`, `error_msg`, `created_at`, `finished_at`) + index `idx_compactions_session_status`.
  5. `personal_memory` (`id`, `project_id`, `content`, `version`, `last_consolidated_at`, `updated_at`) + index `idx_personal_memory_project`.
  6. `memory_ingestion_queue` (`id`, `session_id`, `compaction_id`, `type`, `text`, `status`, `retry_count`, `error_msg`, `created_at`, `processed_at`) + index `idx_queue_status_type`.
  7. `memory_facts` (`id`, `session_id`, `compaction_id`, `type`, `text`, `status`, `created_at`, `updated_at`) + indexes `idx_facts_status_type`, `idx_facts_session`.
  8. `memory_facts_vectors` (`fact_id`, `type`, `status`, `project_id`, `created_at`, `embedding F32_BLOB(384)`) + index `idx_vectors_filter`.
  9. `notifications` (`id`, `category`, `title`, `message`, `status`, `session_id`, `metadata`, `is_read`, `created_at`) + index `idx_notifications_status_cat`.
  10. `voices` (Preserved for Kokoro/Sherpa TTS).
- **Pragmas**: `journal_mode = WAL` (via `.query()`), `busy_timeout = 5000`, `foreign_keys = ON`.
- **Expected Build State**: **100% Green**.
- **Definition of Done**: `cargo check --release` passes; in-crate unit test verifies table creation, foreign key cascade/restrict rules, and default seed rows; `test_vox.db` and `bench_vox.db` are regenerated.

---

### Batch 2: Legacy Subsystem Decommissioning & Pruning
- **Primary Goal**: Eradicate legacy 6-collection taxonomy, DeBERTa NLI ONNX runtime, ModernBERT edge classifier, and crossbeam memory polling loop.
- **Files Deleted**:
  - `persistence/graph.rs`
  - `persistence/memory_mutations.rs`
  - `persistence/memory_queries.rs`
  - `persistence/memory_worker.rs`
  - `services/memory/ml/nli.rs`
  - `services/memory/ml/edge_classifier.rs`
  - `services/memory/ml/scope_classifier.rs`
  - `services/memory/retrieval/{scope.rs, search.rs, mod.rs}`
- **Files Unlinked / Stubbed**:
  - `persistence/mod.rs`: Remove module declarations and re-exports.
  - `services/memory/ml/mod.rs` & `services/memory/mod.rs`: Retain only `embedder.rs` and `tokenizer.rs`.
  - `monitoring/collector.rs`: Hardcode `is_intra_edge_classifier_loaded: false`, `is_inter_edge_classifier_loaded: false`.
  - `services/harness/facade.rs`: Remove automatic per-turn scope classification and profile retrieval; stub `enqueue_personal_facts`.
  - `ipc/memory.rs`: Stub 9 decommissioned handlers returning `Err(VoxIpcError::Database("Decommissioned".into()))` to prevent breaking `lib.rs`.
  - `lib.rs`: Unlink `spawn_memory_worker` startup and shutdown.
  - `core/state.rs` & `pipeline/assistant/`: Decouple `memory_tx`.
  - `tests/session_lifecycle_test.rs`: Reconcile assertions with removed `MemoryWorkerEvent`.
- **Expected Build State**: **Intermediate Red** during file deletion $\to$ **100% Green** once unlinked and stubbed.
- **Definition of Done**: `cargo check --release` passes; ripgrep confirms 0 matches for DeBERTa, ModernBERT, or `fetch_memory_graph` in `src/`.

---

### Batch 3: Persistence Facade & Hot-Path Offload (`persistence/`)
- **Primary Goal**: Implement strongly-typed async Rust CRUD methods for all 9 v2 entities, update the asynchronous hot-path offload worker, and wire `AppState.db`.
- **Domain Modules**:
  - `persistence/projects.rs` [NEW]: `ProjectRow`, `get_projects`, `create_project`, `rename_project`, `delete_project` (with 0-session restriction check).
  - `persistence/sessions.rs`: `SessionRow`, `TurnRow`, `create_session`, `fetch_sessions` (filtering `deleted_at IS NULL`), `fetch_session_by_id`, `fetch_turns`, `update_session_metadata`, `delete_session` (soft delete vs hard delete cascade).
  - `persistence/compactions.rs`: `CompactionRecord`, `record_compaction_start`, `record_compaction_finish`, `fetch_latest_compaction_run`, `fetch_turns_for_compaction`, `commit_compaction_output`.
  - `persistence/personal_memory.rs` [NEW]: `PersonalMemoryRecord`, `get_personal_memory`, `save_personal_memory` (optimistic version locking), `update_consolidated_memory`.
  - `persistence/queue.rs` [NEW]: `QueueItem`, `QueueStatus` enum, `claim_pending_queue_batch`, `update_queue_item_status`, `reconcile_crashed_queue_on_boot`.
  - `persistence/facts.rs` [NEW]: `FactRecord`, `insert_fact`, `fetch_active_facts_by_type`, `deactivate_fact`, `mark_facts_consolidated`, `insert_vector`, `fetch_active_vectors_by_type`.
  - `persistence/notifications.rs`: Refactored to borrow `&Connection` cleanly.
- **Hot-Path Worker (`persistence/worker.rs`)**:
  - Modernized `PersistenceEvent`: `SessionStarted`, `SessionEnded`, `TurnCompleted`, `UpdateSessionMetadata`, `Shutdown`.
  - Updates `pipeline/assistant/{session.rs, llm.rs, interrupt.rs}` dispatch sites.
- **Connection Lifecycle**:
  - `AppState.db: Arc<turso::Connection>` initialized once during bootstrap in `lib.rs`.
  - All operations borrow `&state.db`; ad-hoc `open_readonly` calls removed.
- **Expected Build State**: **100% Green**.
- **Definition of Done**: `cargo check --release` passes; unit tests verify CRUD methods and transaction integrity.

---

### Batch 4: Ingestion Queue & 2-Stage Deduplication Engine
- **Primary Goal**: Refactor ingestion into the high-performance 2-stage dedup engine with Winner-Takes-All deactivation.
- **Components**:
  - `stage1_dedup.rs`: Exact Match Dedup using Jaccard 1.0 threshold across batch ceiling of 128 items with Winner-Takes-All deactivation of older matching facts.
  - `stage2_embed.rs`: Semantic Cosine Match using MiniLM-L12 ONNX 384-dim embedding across batch size 16 items with 0.95 threshold, Winner-Takes-All deactivation, vector insertion, and max-3 retry handling.
  - `runner.rs`: Clean `run_ingestion_cycle(conn: &Connection) -> Result<IngestionCycleSummary>` and boot crash recovery `reconcile_crashed_queue_on_boot(conn: &Connection) -> Result<usize>`.
  - `mod.rs`: Clean domain re-exports.
- **Deleted Files**: `stage3_eval.rs`, `stage4_commit.rs`.
- **Expected Build State**: **100% Green**.
- **Definition of Done**: `cargo check --release` passes; unit tests verify Jaccard exact match, cosine vector thresholding, Winner-Takes-All deactivation, and crash reconciliation.

---

### Batch 5: Compaction Coordinator, Personal Memory & Session Continuation
- **Primary Goal**: Implement working memory compaction, personal memory document lifecycle, and session continuation context loading.
- **Components**:
  - `services/memory/compaction/prompt.rs`: Targets the unified JSON contract (`context_summary`, `personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`).
  - `services/memory/compaction/runner.rs`: Returns `CompactionResult { raw_json, context_summary, facts }`.
  - `services/memory/compaction/coordinator.rs`: Single execution lock per session (`status = 'in_progress'`), Critical compaction (85% threshold with filler phrases + FIFO fallback), Soft compaction (65% threshold with 20s quiet debounce), and Session-End/Manual boundary compaction with persistent notifications.
  - `services/memory/personal.rs` [NEW]: `consolidate_personal_memory` (comment-driven regeneration vs facts merge with quiet ingestion preconditions), and disk export/import.
  - `services/harness/manager.rs`: `personal_memory` injection into system prompt with `MAX_SYSTEM_PROMPT_SHARE` token budget guard.
  - Session continuation context loading: Base Prompt + Personal Memory + Latest Compaction Summary + Uncompacted Turns.
  - Quiet Idle Observer: 30-second debounced quiet timer in `Ready`/`Paused` triggering background `run_ingestion_cycle`.
- **Expected Build State**: **100% Green**.
- **Definition of Done**: `cargo check --release` passes; unit tests verify compaction JSON parsing, lock mutual exclusion, FIFO fallback, and personal memory consolidation state transitions.

---

### Batch 6: IPC Transport Adapters & Event Registry Alignment
- **Primary Goal**: Align Tauri invoke handlers, purge decommissioned endpoints, and update backend-to-frontend event contracts.
- **Components**:
  - `ipc/projects.rs` [NEW]: `get_projects`, `create_project`, `rename_project`, `delete_project`.
  - `ipc/history.rs`: `create_session`, `continue_session`, `get_sessions`, `get_turns`, `update_session`, `delete_session` (with `hard: bool`), `get_transcript_history`. (Purges `commit_session_to_history`).
  - `ipc/memory.rs`: `get_personal_memory`, `save_personal_memory`, `consolidate_personal_memory`, `export_personal_memory`, `import_personal_memory`. (Purges 9 decommissioned handlers).
  - `ipc/notifications.rs`: `get_notifications`, `mark_notifications_read`, `dismiss_notification`, `trigger_session_compaction`.
  - `core/events.rs`: Adds `IpcEvent::PersonalMemoryUpdated` and unified `IpcEvent::SessionsChanged`. Purges `NotificationDismissed` and `NotificationsMarkedRead`.
  - `lib.rs`: Registers all new commands in `generate_handler!` and unregisters decommissioned handlers.
- **Expected Build State**: **100% Green**.
- **Definition of Done**: `cargo check --release` passes; existing integration test suite passes under release mode (`cargo nextest run --release --test-threads=1`).

---

## 5. Verification & Test Plan

1. **Static Build Verification**:
   - Run `cargo check --release` at the end of each batch to guarantee zero compilation errors.
2. **Schema & Integrity Testing**:
   - Execute in-crate unit test `test_v2_schema_initialization` verifying all 10 tables, indexes, default project seed, and foreign key cascades/restricts.
3. **Deduplication & Compaction Testing**:
   - Run dedicated unit tests for Jaccard similarity, Winner-Takes-All deactivation, cosine similarity matching, and compaction JSON parsing.
4. **Full Regression Suite**:
   - Run the complete existing test suite via `cargo-nextest`:
     ```bash
     RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1
     ```
   - Target baseline: 100% pass rate across all untouched unit and integration tests.
5. **Checklist Completion**:
   - Confirm 100% of items in [`CHECKLIST.md`](file:///home/addy/projects/apps/vox/CHECKLIST.md) are checked off.

---

## 6. Risk Analysis & Mitigations

| Risk | Impact | Mitigation Strategy |
|---|---|---|
| **Audio Thread Contention during Dedup** | Latency spikes / audio dropouts | Dedup strictly gated on a 30-second continuous quiet debounce timer in `Ready`/`Paused`. Any speech onset aborts the timer immediately. |
| **Dead Air during Critical Compaction** | Poor conversational user experience | Immediate playback of organic language filler phrase (`TRANSITION_MESSAGES_EN`/`HI`) dispatched to `TtsActor` before initiating the 45s compaction call. |
| **LLM Compaction Failure / Timeout** | Blocked voice response | Bound to turn `CancellationToken` with max 2 retries; immediate FIFO truncation fallback pops oldest turns if compaction fails, ensuring voice generation proceeds. |
| **Concurrent Memory Edits Collision** | Lost user edits | Optimistic concurrency locking via monotonic `version` counter in `personal_memory`. Drifted saves reject with `VoxIpcError::Conflict`. |
| **Database Contention / WAL Locking** | Write latency | Turso pure-Rust async engine with WAL mode and `PRAGMA busy_timeout = 5000`. Hot-path writes offloaded via async `mpsc::Sender<PersistenceEvent>`. |
| **Orphaned State from App Crash** | Ingestion pipeline stalls | `reconcile_crashed_queue_on_boot` automatically resets in-flight items (`stage1_processing` $\to$ `pending`, `stage2_processing` $\to$ `stage1_done`) at startup. |
