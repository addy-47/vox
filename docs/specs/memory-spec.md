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

### 3.1 Streaming Dual-Routing Harness (Wire Format Benchmarking)
To prevent orchestration logic from stalling speech synthesis or leaking into speech audio:
- **Wire Format Benchmark Gate**: Streaming XML tags vs. structured JSON token demuxing must be empirically benchmarked for parser latency and TTS streaming throughput before locking the production wire format.
- **Tagged Streaming Architecture**:
  - The conversational speech response is framed inside `<response>...</response>`.
  - Tokens within `<response>` stream immediately into TTS (`TtsClauseChunker` $\to$ `TtsActor`).
  - The closing tag `</response>` immediately signals completion to the speech pipeline: TTS chunking finishes and `VoxEvent::LlmFinished` is emitted.
  - Trailing orchestration tags (e.g. `<title>...</title>`, `<action>...</action>`) stream to their registered backend consumers off the voice hot path.
- **First-Turn Session Titles**: On Turn 1 of a new session, the harness appends a prompt directive: *"At the end of your response, output a concise 3-5 word title in `<title>...</title>`."* The harness consumes this tag and dispatches `PersistenceEvent::UpdateSessionMetadata { session_id, key: "title", value }`.

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
    1. Internal Compaction System Prompt: Instructs concise, dense, third-person extraction into the 6 generic categories and summary, ignoring conversational chit-chat.
    2. User Message containing:
       - `<prior_summary>`: The prior session context string from the last compaction run, if any.
       - `<dialogue>`: The uncompacted conversation slice wrapped as `<turn speaker="user">` and `<turn speaker="assistant">` elements (stripping any `Role::System` messages).
       - `<schema>` and `<instructions>`.
- **Output Token Budget**: Determined strictly in code as `min(slice, probed_max_output_tokens)` where `slice = (context_window as f32 * 0.15) as u32`. If the 15% slice exceeds what the provider physically supports (`probed_max_output_tokens`), it clamps strictly to the provider ceiling; otherwise it uses the 15% slice. Zero arbitrary magic numbers.
- **Buffer Pruning**: Compacted raw turns are pruned completely from the in-memory FIFO buffer upon successful compaction.
- **Lenient Parse Fallback**: If the model returns non-empty text that fails JSON parsing, the raw text is preserved directly inside `<session_context>` with zero staged DB facts rather than dropping context.

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

### 5.1 Personal Memory Document
- An evolving markdown document capturing consolidated knowledge about the user.
- Stored in the `personal_memory` table in Turso DB, injected directly into the LLM system prompt for conversational awareness.
- Pre-structured for future project scoping via `project_id NULLABLE`.

### 5.2 User Interaction Modes
1. **View, Export & Import**: User views markdown in the UI, exports to disk, or imports an external file to overwrite or initialize.
2. **Direct Manual Edit**: User directly edits markdown text and saves changes.
3. **Comment-Driven Regeneration**: User leaves directive comments. The backend triggers an LLM pass taking `[Current Document] + [User Comments]` to regenerate the document.

### 5.3 Background Consolidation Pipeline
Merges newly accumulated personal facts into the existing document:
1. **Candidate Query**:
   `SELECT * FROM memory_facts WHERE type = 'personal' AND status = 'active'`
2. **Execution Gating & Preconditions**:
   Consolidation MUST NOT run if:
   - An active compaction run is in progress (`session_compactions.status = 'in_progress'`).
    - Unprocessed items exist in the ingestion queue (`memory_ingestion_queue.status != 'completed'`).
    This gate applies equally to fact-merge consolidation and comment-driven regeneration.
3. **Consolidation Prompt & Merge**:
   - The LLM receives `[Current Personal Memory] + [Active Personal Facts]`.
   - Reorganizes sections and resolves contradictions using model reasoning (zero NLI or secondary classifier models).
4. **State Transition on Success**:
    - On successful merge, the document is saved with `last_consolidated_at` stamped to now, and merged personal facts transition from `status = 'active'` to `status = 'consolidated'`.
5. **UI Lock**:
   - While consolidation or regeneration executes, the UI locks the editor to prevent concurrent edit collisions.

### 5.4 Consolidation Cadence
Configurable in `settings.memory.consolidation_cadence` (`"manual"` default, `"daily"` with `settings.memory.consolidation_time` as `"HH:MM"`):
- **Manual**: Runs strictly when triggered by `[Consolidate Now]` or comment regeneration (today's behavior, the default).
- **Scheduled Time**: Runs daily at the configured time. The scheduler sleeps until the next scheduled time and wakes once per run (no polling); a run deferred by the §5.3 gate retries at the next scheduled time. Boot reconciliation detects runs missed while the app was down.
- **Missed & Failed Runs**: A run due while the app was down emits a persistent `personal_consolidation` notification card (`pending`, tap-to-run) instead of running silently. A failed run flips its card to `failed` with the error; successes complete silently.

### 5.5 Project Scope (Current)
Memory is global: the merge folds all `status = 'active'` personal facts into the single document regardless of `project_id` (which is reserved scaffolding for future project-specific memory). Import replaces only the document text and leaves waiting facts active by design.

---

## 6. Stage 3B: Episodic Memory Retrieval & Session Continuation

### 6.1 Episodic Memory Retrieval (Deferred as LLD)
- Active episodic facts (`objective`, `workdone`, `blocker`, `next_step`, `pitfall`) are stored in `memory_facts` with denormalized metadata in `memory_facts_vectors`.
- **Zero Automatic Turn Injection**: No scope classification, no per-turn injection.
- **On-Demand Tool Call**: Retrieved exclusively when the model invokes `search_memory(...)`. Exact tool parameters, vector thresholding, and hybrid search options are deferred to Low-Level Design (LLD).

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
