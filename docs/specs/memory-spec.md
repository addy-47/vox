# Target Spec: Vox Minimal Memory & Session Continuation (v2)

## 1. Scope & System Concept
Vox is a voice orchestrator delegating execution to CLI agents. The memory subsystem provides:
1. Persistent user personalization via a single evolving, user-editable text document (**Personal Memory**).
2. Continuous context compression (**Working Memory** via rolling compaction-of-compactions).
3. Session continuity & grouping (**Persistent Sessions, Projects & Automatic Titles**).
4. On-demand task context retrieval via explicit tool calling (**Episodic Memory Index** — *deferred to LLD*).

---

## 2. Stage 0: Raw Turns & Working Memory Buffer

### 2.1 Turn Data Invariant
A completed turn in the voice pipeline consists of:
```
(turn_id: u32, session_id: i64, user_text: String, assistant_text: String, timestamp_ms: u64)
```
- A turn is finalized only after assistant speech playback completes or dictation finishes.
- Finalized turns are offloaded to persistence via `PersistenceEvent::TurnCompleted` and appended to the in-memory FIFO conversation buffer (`ChatMessage` history).

### 2.2 In-Memory Buffer Structure
- In-memory working buffer retains:
  1. `system_prompt`: Static system instructions + injected Personal Memory document.
  2. `session_context`: The entire structured output of the latest compaction pass (containing both Bucket 1: personal facts, and Bucket 2: working session state [objective, workdone, blocker, next_step, pitfall]).
  3. `messages`: FIFO sliding window of uncompacted recent `ChatMessage` turns (`[User, Assistant, ...]`).

---

## 3. Stage 1: LLM Harness & Working Memory Compaction

### 3.1 Normalized Egress Stream & Tool Interception Architecture
To prevent orchestration logic from stalling speech synthesis or leaking into speech audio:
- **Normalized Event Stream**: The LLM layer emits strongly typed events (`TextDelta`, `ToolCall`, `Finished`, `Error`).
- **Audio Hot Path Isolation**:
  - `TextDelta` tokens stream directly through clause normalization and chunking into TTS (`TtsClauseChunker` $\to$ `TtsActor`).
  - `ToolCall` events bypass the audio pipeline and are intercepted directly by the `Harness` orchestrator.
- **First-Turn Session Titles via Agentic Tool**:
  - When `session.title.is_none()` and title generation has not yet been attempted for the mounted session, the harness injects the `respond_and_set_title` tool schema into the candidate tool list.
  - The model calls `respond_and_set_title` as a `Terminal` tool call, providing both the 3-5 word title and its complete `spoken_response` in a single pass.
  - The spoken response is delivered immediately to TTS, while the title is persisted via an awaited database write in the persistence layer, followed immediately by emitting `IpcEvent::SessionsChanged` (`sessions_changed`) to trigger frontend session rail refresh. Legacy XML tag parsing (`<title>...</title>`) is decommissioned.

### 3.2 Compaction Output Contract (The Session Context)
Every compaction pass (Critical, Soft, or Manual) produces a single unified JSON output containing two distinct buckets across 6 generic human/conversational memory categories:
```json
{
  "personal": ["<durable user traits, preferences, identity, lifestyle, or habits>"],
  "objective": ["<active goals, ongoing endeavors, projects, or topics the user is focusing on>"],
  "workdone": ["<accomplished tasks, completed actions, reached decisions, or historical milestones>"],
  "blocker": ["<current obstacles, open questions, roadblocks, or frustrations>"],
  "next_step": ["<planned future actions, commitments, scheduled tasks, or intentions>"],
  "pitfall": ["<expressed dislikes, things to avoid, lessons learned, or negative experiences>"]
}
```
- **Dual-Destination Architecture**:
  1. **In-Memory Session Context (Working Memory Continuity)**: The **entire output** of this compaction (both Bucket 1: `personal` and Bucket 2: `objective`, `workdone`, `blocker`, `next_step`, `pitfall`) is what constitutes the `session_context`. The harness prunes all compacted raw turns from `ChatMessage` history and injects this entire structured compaction output into `<session_context>...</session_context>` inside the root system prompt for subsequent turns.
  2. **Turso Database (Long-Term Episodic Ingestion & Provenance)**: The raw JSON output string is stored directly in `session_compactions(compaction_output)`. Extracted facts are inserted into `memory_ingestion_queue` with `status = 'pending'` and linked to the `compaction_id` for background consolidation into the durable Personal Memory document.
- **Compaction Input Isolation Contract**:
  - The compaction LLM operates strictly as an isolated summarizer. It NEVER receives the conversational session's system prompt, TTS instructions, or personal memory profile blob.
  - The compaction input consists strictly of:
    1. Internal Compaction System Prompt containing:
       - `<role>`: Frames the model as a session-state and memory extraction engine and states that the entire JSON output is injected into `<session_context>`.
       - `<schema>`: The canonical 6-bucket JSON schema (shared SSOT with the wire `OutputConstraint::JsonSchema`).
       - `<category_definitions>`: Per-bucket semantics for the 6 generic categories.
       - `<rules>`: Concise, dense, third-person extraction rules; ignore chit-chat and pleasantries; deduplicate; never nest user facts behind prefixes in other buckets; output only the raw JSON object.
    2. User Message containing:
       - `<prior_summary>`: The prior session context string from the last compaction run, if any.
         - *In-session runs* (Critical, Soft): Extracted from the `<session_context>` tag of the root `Role::System` message in the active harness history.
         - *Boundary runs* (Manual, Auto, Boot): Extracted from `session_compactions.compaction_output` of the latest completed run (`fetch_latest_compaction_run`) and injected as a `Role::System` message wrapped in `<session_context>` prepended to the uncompacted turn slice.
         - Omitted only on the very first compaction run of a session where no prior completed compaction exists.
       - `<dialogue>`: The uncompacted conversation slice wrapped as `<turn speaker="user">` and `<turn speaker="assistant">` elements (stripping any `Role::System` and `Role::Tool` messages).
       - `<task>`: Extraction instructions directing analysis of `<dialogue>` in light of `<prior_summary>` and emission of only the raw JSON object starting with `{` and ending with `}`.
- **Output Token Budget**: Determined strictly in code as `min(slice, probed_max_output_tokens)` where `slice = (context_window as f32 * 0.15) as u32`. If the 15% slice exceeds what the provider physically supports (`probed_max_output_tokens`), it clamps strictly to the provider ceiling; otherwise it uses the 15% slice. Zero arbitrary magic numbers.
- **Buffer Pruning**: Compacted raw turns are pruned completely from the in-memory FIFO buffer upon successful compaction.
- **Lenient Parse Fallback**: If the model returns non-empty text that fails JSON parsing, the raw text is preserved directly inside `<session_context>` with zero staged DB facts rather than dropping context.
### 3.2.1 Compaction LLM Parameter & Settings Invariants

Compaction execution operates with dedicated, deterministic generation parameters isolated from user conversational settings. Compaction derives ONLY the active provider/model and context window ceiling (`effective_ctx_size()`) from user settings:
1. **JSON Output Mode Enforced Always**: Compaction strictly enforces `OutputConstraint::JsonSchema` with the canonical 6-bucket memory schema (falling back to `OutputConstraint::JsonObject` baseline only when the model catalog explicitly lacks structured output support).
2. **Reasoning Always Disabled**: Compaction reasoning is strictly disabled (`ReasoningMode::Disabled`), even if reasoning is enabled for conversation turns, avoiding latency overhead and unpredictable reasoning tags.
3. **Hardcoded Temperature Constant**: Compaction strictly uses `DEFAULT_LLM_COMPACTION_TEMPERATURE = 0.2` for deterministic, low-hallucination extraction. User conversation temperature settings are ignored.
4. **Autonomous Output Budget**: Max output tokens are calculated autonomously via `calculate_compaction_max_tokens(effective_ctx_size, probed_max_output)` (`(ctx * 0.15).clamp(256, 16384)`), completely independent of the user's conversational `max_output_tokens` setting.
5. **Explicit Context Propagation**: The compaction request MUST set `GenerationRequest.options.context_window = effective_ctx_size()`. The provider receives the same context window used for threshold and output-budget calculation; an unset context field is forbidden.
6. **Grounded Attribution**: Personal facts require explicit user assertions. Questions, topics, assistant suggestions, and inferred relationships are not user facts. Locations, identities, and preferences are never promoted from a mention or request without an explicit user statement.
7. **Modality Preservation**: Planned, promised, or intended actions belong in `next_step`; they may not be promoted to `workdone`. A completed external action requires explicit evidence of successful execution in the dialogue or persisted action result.
8. **No Unsupported Inference**: When attribution or completion is uncertain, omit the claim or use the narrower supported statement. Empty category arrays are valid and preferred over speculation.

### 3.3 Compaction Triggers & Behavioral Rules

#### A. Critical Inline Compaction (`CONTEXT_CRITICAL_THRESHOLD = 0.85`)
- **Trigger**: Fired synchronously in `prepare_turn_context` prior to LLM generation when context utilization reaches or exceeds 85% of `context_window`.
- **Speech Transition Filler**: An organic transition phrase is randomly selected based on language (English: `TRANSITION_MESSAGES_EN`, Hindi: `TRANSITION_MESSAGES_HI`) and dispatched immediately to `TtsActor` for playback to prevent dead air while compaction executes.
- **Ledger & Staging**: The critical path records a `session_compactions` run (`trigger_kind = 'critical'`, from/to resolved as min/max uncompacted turn at slice time) and stages extracted facts via the same atomic commit as every other trigger, so critical-path facts enter deduplication with full provenance.
- **Eligibility Gate**: Requires `message_count >= MIN_MESSAGES_FOR_COMPACTION` (4 messages). No provider or model size bypasses compaction; preemptive FIFO is completely eliminated.
- **Single Attempt & Fast FIFO Fallback**: Dispatches a single compaction attempt with a 45-second timeout (`MAX_COMPACTION_ATTEMPTS = 1`), bound to the turn's `CancellationToken`. User speech onset (`InteractionState::Listening`) aborts compaction immediately.
- **Failure Policy**: If the attempt times out, fails generation, or returns empty text:
  1. Immediately falls back to raw FIFO truncation: pops oldest turn pair so utilization drops and voice response is never blocked.
  2. Emits `VoxEvent::Error(PipelineError)` with `impact: PipelineImpact::Degraded` and `actionability: Actionability::Actionable { category: "compaction_failure", hint: "Context compaction failed; fell back to FIFO" }`.
  3. Does NOT retry repeatedly or halt the user. The next turn will re-evaluate compaction cleanly if context remains critical.

#### Global Minimum Context Window Invariant
- Vox enforces a strict minimum context window of 8,192 tokens across all providers (Embedded, Server, Cloud) via `MIN_LLM_CONTEXT_WINDOW = 8192`. Context window configurations below 8,192 tokens are invalid and rejected at IPC mutation and deserialization.

#### B. Opportunistic Soft Compaction (`CONTEXT_SOFT_THRESHOLD = 0.65`)
- **Trigger Condition**: Context utilization is between $65\%$ and $85\%$ (`0.65 <= util < 0.85`).
- **Gating** (all required):
  1. `settings.history.auto_compaction` must be `true`.
  2. Pipeline mode must be `Modular`.
  3. Pipeline state must be in `InteractionState::Ready` or `InteractionState::Paused`.
- **True Idle Debounce (`SOFT_COMPACTION_DEBOUNCE_SECS = 20s`)**:
  - Entering `Ready` or `Paused` arms a 20-second debounce timer.
  - Any voice interaction, speech onset, or state transition aborts the timer.
  - Compaction executes in background only after 20 continuous seconds of quiet state.
- **Ledger Watermark**: Soft runs record the real compacted turn range (resolved as min/max uncompacted turn at commit time), never message counts, so later slices resume after the true watermark.

#### C. Manual & Session-End Boundary Compaction
- **Trigger Condition**: Session terminates, app restarts, or user switches sessions while uncompacted turns remain. Trigger kinds: `'manual'` (user button), `'auto'` (session-end with auto-compaction on), `'boot_auto'` (boot reconciliation with auto-compaction on).
- **Notification Record**: A persistent notification (`category: "session_compaction"`) is always created in Turso DB and emitted via IPC — including when the backend compacts automatically, so every boundary run has a visible receipt that flips to `'completed'`/`'failed'`.
- **Execution Routing**:
  - If `settings.history.auto_compaction == true`: The backend automatically executes the background compaction slice against the pre-created notification. On completion, notification status transitions to `'completed'` (or `'failed'` with the error).
  - If `settings.history.auto_compaction == false`: The notification waits for user action via the Compact action button in the notification drawer or session rail.
- **Mutual Exclusion**: Exactly one compaction run may execute per session at any time, enforced by a partial unique index (`one in_progress run per session_id`); concurrent duplicate runs are rejected at insert time, not just by pre-check.
- **Prior Summary Seeding**: Boundary runs must seed the turn slice with the latest completed run's `compaction_output` formatted as `<session_context>` in a `Role::System` message, ensuring subsequent slices update the cumulative session state rather than compacting in a vacuum.

---

## 4. Stage 2: Fact Ingestion & 2-Stage Deduplication

### 4.1 Flat Fact Types
Facts are tagged with a flat `type` column without nested hierarchies:
- `'personal'`, `'objective'`, `'workdone'`, `'blocker'`, `'next_step'`, `'pitfall'`

### 4.2 Queue Status Lifecycle (Strongly-Typed)
Items in `memory_ingestion_queue` transition through a strongly-typed enum (`QueueStatus`):
`Pending` $\to$ `Stage1Processing` $\to$ `Stage1Done` $\to$ `Stage2Processing` $\to$ `Completed` (or `Failed` after 3 errors on the same stage).
A per-item failure requeues the item at the same stage's input (`pending` after a Stage 1 failure, `stage1_done` after a Stage 2 failure) with `retry_count + 1`; only the third failure on the same stage moves it to `Failed`.
On application boot, crash reconciliation resets any `Stage1Processing` or `Stage2Processing` items back to `Pending` or `Stage1Done`.

### 4.3 Deduplication Workflow & Batching Logic
Deduplication runs via a background quiet ingestion observer task (`spawn_quiet_ingestion_observer`):
- **Setting Gate**: Gated strictly by `settings.memory.pipeline_processing_enabled == true`. When disabled, the background observer suppresses all deduplication cycles.
- **Quiet State Contract**: The observer watches the pipeline state and triggers only after a sustained 30-second quiet debounce window (`QUIET_INGESTION_DEBOUNCE_SECS = 30`).
  - **Quiet States (Eligible)**: `InteractionState::Idle`, `InteractionState::Ready`, `InteractionState::Paused`, `InteractionState::Sleeping`.
  - **Active States (Ineligible / Abort)**: `Listening`, `Thinking`, `Speaking`, `Working`, `Error`. Transitioning into any active state immediately aborts or resets the debounce window to protect the audio/inference path.
- **Quiescence Pre-Check**: Before executing Stage 1 and Stage 2 deduplication, the observer queries `has_unfinished_items(conn)`. If `memory_ingestion_queue` contains 0 pending or processing items, the cycle returns cleanly without log spam or compute allocation.

1. **Stage 1 — Exact Match Dedup (`STAGE1_BATCH_CEILING = 128`)**:
   - Atomically claims up to 128 `pending` items (`UPDATE ... WHERE status = 'pending' RETURNING ...`).
   - Computes Jaccard word-set similarity (threshold = 1.0) against existing active facts in Turso DB with the same `type`.
   - **Winner-Takes-All Policy**:
     - On exact match: The **incoming fact becomes active**; the existing older matching fact in `memory_facts` is updated to `status = 'inactive'`.
     - Unique facts proceed to `Stage1Done`.

2. **Stage 2 — Semantic Cosine Match (`STAGE2_BATCH_SIZE = 16`)**:
   - Atomically claims up to 16 `stage1_done` items to prevent ONNX CPU/RAM contention.
   - Generates 384-dim embedding via MiniLM-L12 ONNX engine.
   - Queries active vectors in `memory_facts_vectors` with the same `type`.
   - **Winner-Takes-All Policy**:
     - If cosine similarity $\ge 0.95$: Incoming fact is inserted with `status = 'active'`, and the older matching fact is updated to `status = 'inactive'`.
     - If cosine similarity $< 0.95$: Incoming fact is inserted with `status = 'active'`.
   - On completion, queue item transitions to `Completed`.

3. **Status Invariant**:
   - Facts are never hard-deleted during dedup. Superseded or duplicate facts transition to `status = 'inactive'`.
   - Queries for consolidation and tool retrieval exclusively filter `WHERE status = 'active'`.

---

## 5. Stage 3A: Personal Memory & Consolidation Lifecycle

### 5.1 Personal Memory Document & Historical Versioning
- An evolving markdown document capturing consolidated knowledge about the user.
- Stored in the `personal_memory` table in Turso DB (schema governed by `db-spec.md §2.5`) with versioning and `is_active` status.
- **Historical Immutability**: New consolidations or manual saves insert a new record with `version = max_version + 1` and `is_active = 1`, setting previous versions to `is_active = 0`. Older versions remain permanently accessible in the database.
- **Version Navigation & Activation**: The Memory Drawer UI provides an interactive version carousel (`[ < ] v{X} [ > ]`) allowing users to inspect older archived versions and promote any historical version back to `is_active = 1` via `set_active_personal_memory_version`.
- **System Prompt Injection**: Only the currently active version (`is_active = 1`) is injected into the conversational system prompt.

### 5.2 User Interaction Modes
1. **View & Copy**: User views formatted markdown in the UI and can copy the raw markdown text directly to their clipboard.
2. **Direct Manual Edit**: User directly edits markdown text in the UI and saves changes (modal exclusive: disabled while uncommitted patch suggestions are pending review).
3. **Comment-Driven Structured Edits**: User leaves directive comments on specific lines/quotes. The backend triggers a structured patch LLM pass taking `[Current Document] + [User Comments]` to generate targeted delta suggestions (`replace`, `insert`, `delete`) displayed on the staging slate for user review.
4. **Version Carousel Navigation**: User flips between previous versions of personal memory to inspect changes over time or restore an earlier version as the active document.

### 5.3 Structured Delta Consolidation Pipeline
Merges newly accumulated personal facts into the existing document via an audited patch protocol rather than full-document regeneration:
1. **Candidate Query**:
   `SELECT * FROM memory_facts WHERE type = 'personal' AND status = 'active'`
2. **Execution Gating & Preconditions**:
   Consolidation is NEVER hard-blocked by an active compaction or pending queue items:
   - **Comment-Driven Edits**: Executes immediately regardless of ingestion queue state or ongoing compactions.
   - **Fact Integration ("Integrate Learned Facts" in UI)**:
     - If an active compaction is in progress (`session_compactions.status = 'in_progress'`), the UI provides a non-blocking resolution choice:
       1. **Pause / Preempt Compaction & Consolidate Now**: Signals cancellation on the active compaction task, resets its DB record status from `'in_progress'` back to `'pending'` (allowing auto-compaction to resume/pick it back up once consolidation completes), processes pending ingestion items, and immediately runs consolidation.
       2. **Queue Consolidation**: Registers the consolidation request in `PendingConsolidationState` to run automatically as soon as the ongoing compaction finishes.
   - **Headless Scheduled Runs**:
     - Headless scheduled runs never raise errors. If a compaction is in progress, the scheduled run is automatically queued in `PendingConsolidationState` and executes as soon as the active compaction and its ingestion cycle finish.
3. **Structured Patch LLM Pass**:
   - The LLM receives `[Current Personal Memory Document] + [Active Personal Facts]`.
   - The LLM acts strictly as a **change proposer** rather than a document re-writer. It emits a structured JSON object containing an array of atomic patch operations without synthetic line numbers:
     ```json
     {
       "operations": [
         {
           "op": "replace",
           "section": "## Personal Information",
           "target_text": "Lives in Chicago.",
           "proposed_text": "Lives in Austin.",
           "source_fact_ids": ["fact_101"]
         },
         {
           "op": "insert",
           "section": "## Technical Projects",
           "target_text": null,
           "proposed_text": "Building a voice orchestrator in Rust.",
           "source_fact_ids": ["fact_204"]
         }
       ]
     }
     ```
   - **Atomic Operators**:
     - `insert`: Appends `proposed_text` under the designated `section` heading. If `section` does not exist, it is created.
     - `replace`: Locates exact `target_text` within `section` and substitutes it with `proposed_text`.
     - `delete`: Locates exact `target_text` within `section` and removes it.
   - Any document text not explicitly targeted by an operation is mathematically immutable and preserved.
4. **Staging & Modal Isolation Lifecycle**:
   - Generated operations are inserted into `personal_memory_suggestions` with `status = 'pending'`.
   - Linked facts in `memory_facts` transition from `status = 'active'` to `status = 'staged'`.
   - The staging slate (`PersonalMemoryStagingCard`) enters an exclusive `"review"` mode displaying diff cards with `[✓]` (Accept) and `[✕]` (Reject) alongside `Accept All` and `Discard All`.
   - While pending suggestions exist, manual text editing, markdown import, and new comment submissions are locked to eliminate concurrent mutation races.
5. **Suggestion Resolution**:
   - Suggestions are resolved individually or in bulk via `resolve_memory_suggestion(id: Option<String>, action: String)`.
   - **Acceptance (`action = 'accept'`)**:
     1. Applies patch delta(s) to the active `personal_memory` markdown.
     2. Inserts a new record in `personal_memory` with `version = max_version + 1`, `is_active = 1`, and `last_consolidated_at = now()`. The previous version flips to `is_active = 0`.
     3. Suggestion rows flip to `status = 'accepted', resolved_at = now()`.
     4. Linked facts in `memory_facts` flip from `'staged'` to `'consolidated'`.
   - **Rejection (`action = 'reject'`)**:
     1. Suggestion rows flip to `status = 'rejected', resolved_at = now()`.
     2. Linked facts in `memory_facts` flip from `'staged'` to `'rejected'`, ensuring they are not repeatedly re-suggested in subsequent consolidation cycles.
     3. Active document remains unchanged.

### 5.4 Suggestion Policies & Cadence
- **Suggestion Policy** (`settings.memory.suggestion_policy`):
  - `"manual_review"` (default): All fact integration and comment edits land in `personal_memory_suggestions` for user review.
  - `"auto_apply"`: Non-conflicting `insert` and `replace` operations automatically commit into a new document version; deletions are held for user confirmation.
- **Cadence** (`settings.memory.consolidation_cadence`):
  - `"manual"` (default): Triggered on-demand via the `"Integrate Learned Facts"` button or comment regeneration.
  - `"daily"` (with `settings.memory.consolidation_time` as `"HH:MM"`): Runs daily at configured time.
  - **Missed & Failed Runs**: Runs due while the app was down emit a persistent `personal_consolidation` notification card (`pending`, tap-to-run). A failed run flips its card to `failed` with the error; successes complete silently.

### 5.5 Project Scope (Current)
Memory is global: the merge folds all `status = 'active'` personal facts into the single document regardless of `project_id` (reserved scaffolding for future project-specific memory). Direct manual edits replace only the document text and leave waiting facts active by design.

---

## 6. Stage 3B: Episodic Memory Retrieval & Session Continuation

### 6.1 Episodic Memory Retrieval & Embedding Model Lifecycle
- Active episodic facts (`objective`, `workdone`, `blocker`, `next_step`, `pitfall`) are stored in `memory_facts` with denormalized metadata in `memory_facts_vectors`.
- **Zero Automatic Turn Injection**: No scope classification, no per-turn injection.
- **On-Demand Tool Call**: Retrieved exclusively when the model invokes the non-terminal tool `search_memory(query, spoken_filler)`. Thresholding (cosine cutoff, top-K facts) is governed by system configuration, not model arguments (see `tools-spec.md §7.2`).
- **Dynamic Embedding Model Lifecycle (`minilm-l12-v2`)**:
  1. *Session-Start Eager Pre-warming*: When a voice session starts (`ensure_modular_workers` in Modular mode, or `start_realtime_session` in Realtime mode), if `settings.personal_memory.context_retrieval_enabled == true`, the embedding model is loaded asynchronously in a non-blocking background task. This ensures zero cold-start delay (~50–120ms) when `search_memory` is first called by the LLM.
  2. *Mid-Session Dynamic Settings Toggle*:
     - Enabling retrieval (`context_retrieval_enabled = true`) while a session is active immediately warms the embedder in background and updates the active `Harness` (`harness.set_memory_retrieval_enabled(true)`), instantly exposing `search_memory` on subsequent turns.
     - Disabling retrieval (`context_retrieval_enabled = false`) mid-session immediately updates `harness.set_memory_retrieval_enabled(false)` to prune `search_memory` from candidate tools, evicts the ONNX model from memory via `unload_memory_pipeline_onnx_models()`, and invokes `trim_heap` to free memory back to the OS.
  3. *Zero-Idle Eviction Invariant*: When no session is active (`InteractionState::Idle`) or upon session teardown (`on_end`), all ONNX models are evicted via `stop_audio_engine` -> `unload_all_onnx_models()`. Model weights are never retained in RAM during idle state.

### 6.2 Session Continuation Context
When a user restores and continues an existing session from the conversation list:
- The context injected into the system prompt consists strictly of:
  1. Base system prompt + **Personal Memory Document**.
  2. The **latest persisted context summary** for that session (extracted from `compaction_output` in `session_compactions`).
  3. The uncompacted recent turns of that session.

---

## 7. Subsystem Boundaries & Invariants

1. **Elimination of Legacy Systems**:
   - 6-collection taxonomy, relational graph edges, DeBERTa NLI ONNX runtime, ModernBERT edge classifier, and Three.js graph UI are permanently decommissioned.
2. **Removal of `MemoryWorkerEvent`**:
   - No separate polling crossbeam event loop for memory. Memory processing tasks (dedup, consolidation) run as direct async jobs triggered by state observers and IPC commands.
3. **Strict Persistence Boundary Invariant**:
   - Zero SQL queries outside the `persistence/` directory. All database operations route through strongly-typed functions in `persistence/` using a single shared Turso connection in `AppState`.
   - Hot-path write operations from the voice pipeline use `PersistenceEvent`.
4. **Native Turso Engine Invariant**:
   - Persistence utilizes the native Rust `turso` crate with WAL/MVCC and native `F32_BLOB` vector storage.
