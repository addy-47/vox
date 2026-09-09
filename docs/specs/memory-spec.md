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
  2. `context_summary`: Rolling compaction summary of all turns compacted so far in the current session (extracted from `compaction_output`).
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

### 3.2 Compaction Output Contract
Every compaction pass (Critical, Soft, or Manual) produces a single unified JSON output string:
```json
{
  "context_summary": "<rolling conversational summary string>",
  "personal": ["<unstructured fact about user>"],
  "objective": ["<unstructured goal/intent>"],
  "workdone": ["<unstructured completed task/milestone>"],
  "blocker": ["<unstructured error/blocker>"],
  "next_step": ["<unstructured upcoming step>"],
  "pitfall": ["<unstructured edge case/lesson learned>"]
}
```
- **Provenance Retention**: The raw JSON output string is stored directly in `session_compactions(compaction_output)`.
- **Buffer Pruning**: Compacted turns are pruned from the in-memory FIFO buffer.
- **Staging**: Extracted facts are inserted into `memory_ingestion_queue` with `status = 'pending'` and linked to `compaction_id`.

### 3.3 Compaction Triggers & Behavioral Rules

#### A. Critical Inline Compaction (`CONTEXT_CRITICAL_THRESHOLD = 0.85`)
- **Trigger**: Fired synchronously in `prepare_turn_context` prior to LLM generation when context utilization reaches or exceeds 85% of `context_window`.
- **Speech Transition Filler**: An organic transition phrase is randomly selected based on language (English: `TRANSITION_MESSAGES_EN`, Hindi: `TRANSITION_MESSAGES_HI`) and dispatched immediately to `TtsActor` for playback to prevent dead air while compaction executes.
- **Execution & Timeout**: Dispatches compaction request with a 45-second timeout, bound to the turn's `CancellationToken`. User speech onset (`InteractionState::Listening`) aborts compaction immediately.
- **Fallback Policy**: Compaction attempts generation with up to 2 retries. If all retries fail:
  1. Falls back to raw FIFO truncation: pops oldest turns until utilization $< 85\%$ to ensure voice response is never blocked.
  2. Emits `VoxEvent::Error(PipelineError)` with `impact: PipelineImpact::Degraded` and `actionability: Actionability::Actionable { category: "compaction_failure", hint: "Context compaction failed; fell back to FIFO" }`.

#### B. Opportunistic Soft Compaction (`CONTEXT_SOFT_THRESHOLD = 0.65`)
- **Trigger Condition**: Context utilization is between $65\%$ and $85\%$ (`0.65 <= util < 0.85`).
- **Gating**:
  1. `settings.history.auto_compaction` must be `true`.
  2. Pipeline mode must be `Modular`.
  3. Pipeline state must be in `InteractionState::Ready` or `InteractionState::Paused`.
- **True Idle Debounce (`SOFT_COMPACTION_DEBOUNCE_SECS = 20s`)**:
  - Entering `Ready` or `Paused` arms a 20-second debounce timer.
  - Any voice interaction, speech onset, or state transition aborts the timer.
  - Compaction executes in background only after 20 continuous seconds of quiet state.

#### C. Manual & Session-End Boundary Compaction
- **Trigger Condition**: Session terminates, app restarts, or user switches sessions while uncompacted turns remain.
- **Notification Record**: A persistent notification (`category: "session_compaction"`) is always created in Turso DB and emitted via IPC.
- **Execution Routing**:
  - If `settings.history.auto_compaction == true`: The backend automatically executes the background compaction slice. On completion, notification status transitions to `'completed'`.
  - If `settings.history.auto_compaction == false`: Compaction waits for user action via `[Compact Now]` button in the notification drawer.
- **Mutual Exclusion**: Exactly one compaction run may execute per session at any time (`status = 'in_progress'`). Duplicate concurrent runs are rejected.

---

## 4. Stage 2: Fact Ingestion & 2-Stage Deduplication

### 4.1 Flat Fact Types
Facts are tagged with a flat `type` column without nested hierarchies:
- `'personal'`, `'objective'`, `'workdone'`, `'blocker'`, `'next_step'`, `'pitfall'`

### 4.2 Queue Status Lifecycle (Strongly-Typed)
Items in `memory_ingestion_queue` transition through a strongly-typed enum (`QueueStatus`):
`Pending` $\to$ `Stage1Processing` $\to$ `Stage1Done` $\to$ `Stage2Processing` $\to$ `Completed` (or `Failed` on 3 errors).
On application boot, crash reconciliation resets any `Stage1Processing` or `Stage2Processing` items back to `Pending` or `Stage1Done`.

### 4.3 Deduplication Workflow & Batching Logic
Deduplication runs in background cycles when the system is quiet:

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
3. **Consolidation Prompt & Merge**:
   - The LLM receives `[Current Personal Memory] + [Active Personal Facts]`.
   - Reorganizes sections and resolves contradictions using model reasoning (zero NLI or secondary classifier models).
4. **State Transition on Success**:
   - On successful merge, merged personal facts transition from `status = 'active'` to `status = 'consolidated'`.
5. **UI Lock**:
   - While consolidation or regeneration executes, the UI locks the editor to prevent concurrent edit collisions.

### 5.4 Consolidation Cadence
User-configurable in settings:
- **On Session Close**: Automatically runs after a session terminates (once compaction and dedup settle).
- **Scheduled Time**: E.g. daily at a user-specified time.
- **Manual Only**: Runs strictly when triggered by `[Consolidate Now]` or comment regeneration.

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
