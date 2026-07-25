# Vox Cognitive Memory Subsystem Architecture Ledger — v0.9.0

> **Authoritative Technical Architecture Document**  
> **Scope:** Authoritative end-to-end technical documentation of the Vox Cognitive Memory Subsystem, ML models, Turso/libSQL database integration, background worker threads, deduplication algorithms, relationship graphs, and two-tier budgeted retrieval.

---

## 1. Subsystem Architecture Overview

Vox implements a **3-Tier Human-Centric Cognitive Memory System** designed to capture everyday human trace data (routines, language learning progress, social dynamics, personal emotions, tasks, and hobbies) with zero pipeline latency stalls during active voice interaction.

The system decouples real-time conversation from memory ingestion via an asynchronous Write-Ahead Log (WAL) queue, executing heavy ML models (dense embeddings and NLI cross-encoders) exclusively during idle states.

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ Vox Memory Subsystem Architecture                                                           │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                             │
│ 1. WORKING MEMORY (`services/memory/working_memory.rs`)                                     │
│    - Transient FIFO turn history in RAM (`ConversationManager`).                            │
│    - Manages token estimation, turn limits, and prompt updating.                            │
│    - Point-of-idle compactions and Critical maintenance shifts (>85% token window).         │
│                                                                                             │
│ 2. INGESTION & COMPACTION (`services/memory/ingestion.rs`)                                  │
│    - Differential LLM extraction using `<known_facts>` context comparison.                 │
│    - Ephemeral staging contract (`Context` & `Tasks` staged, `Goals` & `Semantic` pending). │
│    - Session End Consolidation Sweep on pipeline idle timeout.                              │
│                                                                                             │
│ 3. BACKGROUND ORCHESTRATION (`persistence/memory_worker.rs`, `services/memory/orchestrator.rs`)│
│    - Dedicated OS thread (`vox-memory-worker`) with 30-second continuous idle debounce.     │
│    - Cooperative yielding: AtomicBool `cancel_flag` interrupts ONNX loops on `PipelineActive`.│
│    - Phase 1: Dual-Defense Fast Hard Deduplication (Cosine ≥ 0.98 or Jaccard = 1.0 Merge).  │
│    - Phase 2: Multi-Tier NLI Routing (Similarity ≥ 0.95 `SIMILAR` edge; 0.65–0.95 DeBERTa).  │
│    - Phase 3: Atomic Multi-Table Transaction Persistence (`repository.rs`).               │
│                                                                                             │
│ 4. TWO-TIER BUDGETED RETRIEVAL (`services/memory/retrieval.rs`)                             │
│    - Strict 15% overall context window budget allocation.                                  │
│    - Tier 1 (7% cap): Foundational (`Identity`, `Constraints`) & Operational (`Tasks`, `Goals`).│
│      + Time-Windowed Context Chaining (12h window & Distant Memory Fallback).               │
│    - Tier 2 (8% cap): Semantic Profiles (`Preferences`, `Relationships`, `Skills`, `Projects`, │
│      `Experiences`) via single-query SQL Window Function partitioning.                      │
│      + Step 2A: Guaranteed Anchor Floor (Top K_base per collection).                        │
│      + Step 2B: Global Similarity Competitive Pool (similarity ≥ cutoff).                   │
│    - 3-Pass Edge Resolution (`USER_SUPERSEDES` swaps, `SUPPORTS` pulls, `CONFLICTS` events).│
│    - Active `<memory_manifest>` header + Chronological relative timestamp formatting.      │
│                                                                                             │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Architectural Taxonomy & Cognitive Collections

The memory subsystem categorizes all facts into **10 PascalCase collections** mapped across **3 structural types** (`core/constants.rs`):

```text
                    ┌──────────────────────────────────────────────┐
                    │           10 COGNITIVE COLLECTIONS           │
                    └──────┬──────────────────┬─────────────────┬──┘
                           │                  │                 │
            ┌──────────────▼──────┐   ┌───────▼─────────────┐   │
            │     FOUNDATIONAL    │   │     OPERATIONAL     │   │
            │ Identity,Constraints│   │Context, Tasks, Goals│   │
            └─────────────────────┘   └─────────────────────┘   │
                                                                │
                                            ┌───────────────────▼─┐
                                            │       SEMANTIC      │
                                            │Prefs, Relationships,│
                                            │Skills, Projects,    │
                                            │    Experiences      │
                                            └─────────────────────┘
```

| Structural Type | Collections | Embedding Requirement | Retrieval Style | Description |
| :--- | :--- | :--- | :--- | :--- |
| **`foundational`** | `Identity`, `Constraints` | **Never Embedded** | Unconditional Vectorless Load | Core persona, biological traits, and safety rules loaded unconditionally into Tier 1. |
| **`operational`** | `Context`, `Tasks`, `Goals` | **Never Embedded** | Deterministic SQL / Time-Windowed | Session summaries and active/pending action items loaded into Tier 1. |
| **`semantic`** | `Preferences`, `Relationships`, `Skills`, `Projects`, `Experiences` | **Embedded (BGE-M3 1024-dim)** | Dense Vector Search + Anchor Floor | Long-term user profiles and personal traits searched semantically into Tier 2. |

---

## 3. Storage Layer & Database Architecture

### 3.1 Turso / libSQL Concurrency & Configuration (`persistence/db.rs`, `schema.rs`)
Vox utilizes the pure-Rust **Turso/libSQL Engine** (`turso` crate) configured with:
- **Write-Ahead Logging (WAL):** `journal_mode = WAL` enables concurrent database reads during background worker writes.
- **Busy Timeout:** `busy_timeout = 5000`ms prevents database locking deadlocks.

### 3.2 Schema Definition
The database schema (`persistence/schema.rs`) consists of **four core memory tables**:

```sql
-- 1. Core Facts Table (Houses all 10 collections under 3 structural types)
CREATE TABLE IF NOT EXISTS memory_facts (
    id           TEXT PRIMARY KEY,              -- UUID v4 (format: 'mem_{timestamp}_{uuid}')
    type         TEXT NOT NULL,                 -- 'foundational', 'operational', 'semantic'
    collection   TEXT NOT NULL,                 -- Identity, Constraints, Preferences, Relationships, Skills, Projects, Experiences, Context, Tasks, Goals
    fact         TEXT NOT NULL,
    source       TEXT NOT NULL DEFAULT 'LLM',   -- 'LLM', 'User', 'Import'
    status       TEXT NOT NULL DEFAULT 'active',-- 'active', 'superseded', 'deleted'
    session_id   TEXT NOT NULL DEFAULT '',      -- Provenance tracking
    turn_id      TEXT NOT NULL DEFAULT '',      -- Provenance tracking
    created_at   INTEGER NOT NULL               -- Millisecond epoch timestamp
);

-- 2. Separate Vectors Table (SQLite Page-Loading Performance Optimization - SEMANTIC ONLY)
CREATE TABLE IF NOT EXISTS memory_facts_vectors (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    fact_id     TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    collection  TEXT NOT NULL,
    embedding   F32_BLOB(1024) NOT NULL         -- 1024-dimensional BGE-M3 dense vector (F32_BLOB)
);

-- 3. Directed Relations Graph Table (SUPPORTS / CONFLICTS / USER_SUPERSEDES / SIMILAR / MERGED)
CREATE TABLE IF NOT EXISTS memory_relations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id     TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    to_id       TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    relation    TEXT NOT NULL,                  -- 'SUPPORTS', 'CONFLICTS', 'USER_SUPERSEDES', 'SIMILAR', 'MERGED'
    created_at  INTEGER NOT NULL,
    UNIQUE(from_id, to_id, relation)
);

-- 4. Unified Ingestion Queue & Staging WAL Table
CREATE TABLE IF NOT EXISTS personal_memory_queue (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    fact         TEXT NOT NULL,
    collection   TEXT NOT NULL,
    source       TEXT NOT NULL DEFAULT 'LLM',
    session_id   TEXT NOT NULL DEFAULT '',
    status       TEXT NOT NULL DEFAULT 'pending',-- 'pending' (to embed), 'staged' (active session WAL), 'processing', 'completed', 'failed'
    attempts     INTEGER NOT NULL DEFAULT 0,
    error_msg    TEXT,
    created_at   INTEGER NOT NULL,
    processed_at INTEGER
);

-- Performance Indices
CREATE INDEX IF NOT EXISTS idx_mf_type_status ON memory_facts(type, status);
CREATE INDEX IF NOT EXISTS idx_mf_collection_status ON memory_facts(collection, status);
CREATE INDEX IF NOT EXISTS idx_mf_created ON memory_facts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_mfv_collection ON memory_facts_vectors(collection);
CREATE INDEX IF NOT EXISTS idx_mr_from ON memory_relations(from_id, relation);
CREATE INDEX IF NOT EXISTS idx_mr_to ON memory_relations(to_id, relation);
CREATE INDEX IF NOT EXISTS idx_pmq_status ON personal_memory_queue(status, created_at ASC);
```

### 3.3 Storage Performance Optimizations
1. **$O(1)$ Status Column:** Filtering by `WHERE status = 'active'` directly on `memory_facts` bypasses complex graph traversals during hot-path retrieval.
2. **Vector Table Separation:** Keeping heavy 4KB vector blobs in `memory_facts_vectors` leaves `memory_facts` row sizes minimal, maximizing SQLite page cache efficiency.
3. **Binary Float Encoding:** Vectors are serialized via little-endian float byte arrays (`encode_f32_blob` / `decode_f32_blob`).

---

## 4. Integrated Machine Learning Models

### 4.1 Hot-Path Query Classifier (`services/memory/classifier.rs`)
- **Model:** `query-sieve` (DistilBERT fine-tuned for voice query classification).
- **Path:** `~/.vox/models/classifier/distilbert-query-classifier/model_quantized.onnx`.
- **Purpose:** Short-circuits generic chatter turns (*"hello"*, *"thanks"*, *"okay"*), saving 100% of ONNX embedding and database search overhead.
- **Latency:** $< 1.5$ ms CPU inference time.

### 4.2 Multilingual Dense Embedder (`services/memory/embedder.rs`)
- **Primary Model:** `paraphrase-multilingual-MiniLM-L12-v2` (384-dim INT8 ONNX quantized).
- **Primary Path:** `~/.vox/models/embedding/paraphrase-multilingual-MiniLM-L12-v2/model_int8.onnx`.
- **Fallback Path:** `~/.vox/models/embedding/bge-m3/model_quantized.onnx` (1024-dim fallback).
- **Vector Dimensions:** 384 float dimensions with L2 unit normalization (1,536 bytes per SQLite vector row).
- **Performance:** 10.06 ms CPU inference latency (8.6x faster than BGE-M3), 118 MB RAM footprint.
- **Cutoff Floor (`semantic_similarity_cutoff`):** Set to `0.40` to align with MiniLM-L12 vector geometry (Noise baseline: 0.04-0.23, Margin: 0.34).
- **Scope:** Computed **strictly for `semantic` type facts**. Foundational and Operational collections are vectorless.

### 4.3 Pairwise NLI Classifier (`services/memory/nli.rs`)
- **Model:** `deberta-v3-xsmall-nli` (Quantized ONNX running via `ort` v2).
- **Path:** `~/.vox/models/nli/deberta-v3-xsmall-nli/model_quantized.onnx`.
- **Configuration:** Graph optimization level 3, intra-threads = 1, tokenizer truncation clamped to 512 tokens.
- **Dynamic Startup Calibration (`calibrate()`):** Runs dummy premise/hypothesis pairs at boot to dynamically map logit output indices to `Contradiction`, `Entailment`, and `Neutral` labels.
- **Thresholds:**
  - `Contradiction ≥ 0.85` $\rightarrow$ `CONFLICTS` edge.
  - `Entailment ≥ 0.85` $\rightarrow$ `SUPPORTS` edge.

---

## 5. Ingestion, Compaction & Staging Pipeline

```text
    In-Memory Compaction Shift (Working Memory Token Limit > 85%)
                                 │
                                 ▼
         LLM Differential Extraction (`COMPACTION_SYSTEM_PROMPT` + `<known_facts>`)
                 ├── `Context` & `Tasks`  ──► Queue (`status = 'staged'`) [WAL]
                 └── `Goals` & `Semantic` ──► Queue (`status = 'pending'`)
                                                   │
                                                   ▼
                                      Background Worker Sweep (Idle State)
                                                   │
                                                   ▼
                                     Compute BGE-M3 Embeddings (Semantic Only)
                                                   │
                                                   ▼
                                 Phase 1: Dual-Defense Hard Deduplication
                                     ├── Cosine ≥ 0.98 OR Jaccard = 1.0 ──► O(1) Merge
                                     └── Distinct Fact ──► Phase 2 Multi-Tier NLI Routing
                                                                ├── Cosine > 0.95 ──► SIMILAR Edge (Skip NLI)
                                                                ├── Cosine 0.65–0.95 ──► DeBERTa NLI
                                                                └── Cosine < 0.65 ──► Neutral (Skip NLI)
                                                                            │
                                                                            ▼
                                                                Phase 3 Persistence Transaction
```

### 5.1 Differential Compaction Extraction (`services/memory/ingestion.rs`)
- Triggered during point-of-idle turns or when working memory exceeds 85% of its token limit.
- **Differential Extraction:** Passes currently known active facts (`<known_facts>`) into the LLM prompt, instructing it to extract ONLY brand-new facts or explicit updates introduced in `<conversation_history>`.
- Parses structured JSON into `CompactionResult { context_summary, personal_memory, diff_to_enqueue }`.

### 5.2 Ephemerality & Staging WAL Contract (`persistence/repository.rs`)
- **Active Session Staging:**
  - `Context` and `Tasks` are enqueued into `personal_memory_queue` with `status = 'staged'`.
  - `Goals` and all `semantic`/`foundational` collections are enqueued with `status = 'pending'`.
- **Session End Consolidation Sweep (`session_end_consolidation`):**
  - Triggered on session end or pipeline idle timeout (`auto_sleep_timeout`).
  - Promotes staged `Tasks` in the queue from `staged` $\rightarrow$ `pending`.
  - Deletes intermediate staged `Context` entries from the queue.
  - Writes the finalized session `Context` paragraph directly into `memory_facts` (`type = 'operational'`, `collection = 'Context'`, `status = 'active'`). **It is never embedded.**

---

## 6. Background Worker & Multi-Tier Orchestration

### 6.1 Worker Threading & Debounce (`persistence/memory_worker.rs`)
- Runs in a dedicated OS thread (`vox-memory-worker`).
- **30-Second Idle Debounce:** Requires 30 seconds of continuous pipeline idle time (`MIN_IDLE_DEBOUNCE_SECS = 30`) before commencing queue sweeps.
- **Private Mode Guard:** If `is_private_mode` is enabled, all memory events and sweeps are bypassed.
- **Cooperative Yielding:** When the pipeline becomes active (`MemoryWorkerEvent::PipelineActive`), the worker sets `cancel_flag = true` (`AtomicBool`), immediately aborting ONNX embedding or NLI inference loops to prevent UI/audio stuttering.

### 6.2 3-Phase Queue Orchestration (`services/memory/orchestrator.rs`)

#### Phase 1: Dual-Defense Fast Hard Deduplication
1. Generates 1024-dimensional BGE-M3 embedding for the enqueued fact.
2. Fetches existing active candidate vectors in the same collection.
3. Computes Cosine similarity and Jaccard token set overlap (`jaccard_similarity`).
4. **Exact Duplicate Match (`is_exact_duplicate`):** If `cosine ≥ 0.98` OR `jaccard == 1.0`, performs $O(1)$ Merge (`insert_exact_merged_fact`):
   - Inserts incoming fact into `memory_facts` with `status = 'superseded'`.
   - Inserts vector into `memory_facts_vectors`.
   - Inserts `MERGED` relation edge (new fact $\rightarrow$ existing candidate).
   - Updates existing candidate's `created_at` timestamp to `now`.
   - Marks queue job `completed`.

#### Phase 2: Multi-Tier NLI Routing
1. Takes top candidates up to `NLI_CANDIDATE_LIMIT = 5`.
2. **Near-Duplicates (`cosine > 0.95` i.e. `SIMILAR_EDGE_THRESHOLD`):** Directly writes a `SIMILAR` relation edge without executing NLI.
3. **NLI Classification Candidate Pool (`0.65 ≤ cosine ≤ 0.95`):** Passes pairs to DeBERTa-v3 NLI:
   - `Contradiction ≥ 0.85` $\rightarrow$ `CONFLICTS` relation edge.
   - `Entailment ≥ 0.85` $\rightarrow$ `SUPPORTS` relation edge.
   - `Neutral` $\rightarrow$ No relation edge written.
4. **Distant Candidates (`cosine < 0.65`):** Bypasses NLI (classified as `Neutral`).

#### Phase 3: Atomic Persistence Transaction
- Executes `insert_fact_with_vector_and_relations` inside a single `BEGIN TRANSACTION`:
  - Inserts node into `memory_facts` (`status = 'active'`).
  - Inserts vector into `memory_facts_vectors`.
  - Inserts relation edges into `memory_relations`.
  - Updates queue job status to `completed`.

---

## 7. Two-Tier Budgeted Retrieval & Prompt Assembly

### 7.1 Master Master Toggles (`core/settings.rs`)
- `context_retrieval_enabled` (Toggle 1): Controls whether retrieved memory is injected into live LLM system prompts.
- `pipeline_processing_enabled` (Toggle 2): Controls whether the background worker processes queue items.

### 7.2 Strict 15% Budget Split (`services/memory/retrieval.rs`)
Enforces a strict **15% hard cap of the total context window** for memory injection:

```text
                  ┌──────────────────────────────────────────────┐
                  │          TOTAL PERSONAL MEMORY (15%)         │
                  └──────┬────────────────────────────────┬──────┘
                         │                                │
        ┌────────────────▼────────────────┐     ┌─────────▼──────────────────────┐
        │   TIER 1: FOUNDATIONAL (7%)     │     │     TIER 2: SEMANTIC (8%)      │
        │   Identity, Constraints,        │     │  Preferences, Relationships,   │
        │   Tasks, Goals, Context Chain   │     │   Skills, Projects, Experiences│
        └─────────────────────────────────┘     └────────────────────────────────┘
```

#### Tier 1: Foundational & Operational Core (7% hard cap)
1. **Identity & Constraints:** Loaded unconditionally if active.
2. **Tasks & Goals:** Loaded deterministically where `type = 'operational'` AND `status = 'active'`.
3. **Time-Windowed Context Chaining:**
   - Queries `Context` facts within `context_chaining_window_hours` (default 12h) sorted `created_at DESC`.
   - Preps relative timeline `[Past Contexts within the Last 12 Hours]` until remaining Tier 1 budget is filled.
   - **Distant Memory Fallback:** If no contexts exist within the window, queries the single latest context older than the window (`created_at < window_start ORDER BY created_at DESC LIMIT 1`) and formats it inside a `[Recollection (Distant Memory)]` container.

#### Tier 2: Semantic Profiles (8% hard cap)
1. **Single-Query SQL Window Function:** Fetches ranked candidates across all 5 semantic collections in 1 DB round-trip:
   ```sql
   WITH Ranked AS (
       SELECT mf.id, mf.type, mf.collection, mf.fact, mf.source, mf.status, mf.created_at,
              (1.0 - vector_distance_cos(mfv.embedding, ?)) as similarity,
              ROW_NUMBER() OVER (
                  PARTITION BY mfv.collection
                  ORDER BY vector_distance_cos(mfv.embedding, ?) ASC
              ) as rank
       FROM memory_facts mf
       JOIN memory_facts_vectors mfv ON mfv.fact_id = mf.id
       WHERE mfv.collection IN ('Preferences', 'Relationships', 'Skills', 'Projects', 'Experiences')
         AND mf.status = 'active'
         AND (mf.session_id = '' OR mf.session_id != ?)
   )
   SELECT id, type, collection, fact, source, status, created_at, similarity
   FROM Ranked
   ```
2. **Step 2A (Guaranteed Anchor Floor):** Selects top `K_base` (`personal_top_k_per_semantic_collection`, default 5) candidates per collection to preserve user identity anchors across topic shifts.
3. **Step 2B (Global Similarity Competitive Pool):** Sorts remaining candidates by similarity descending and includes facts above `semantic_similarity_cutoff` (default 0.65) until Tier 2 budget is consumed.
4. Sorts selected semantic facts chronologically (`created_at` ascending).

### 7.3 Unresolved Relations & 3-Pass Edge Resolution
1. **Unresolved Relations Header:** Checks `memory_relations` for selected semantic facts and outputs:
   - `[Unresolved Contradictions]` (`CONFLICTS`)
   - `[Unresolved Near-Duplicates]` (`SIMILAR`)
2. **3-Pass Edge Resolution (`resolve_edges`):**
   - **Pass 1 (Pointer Swaps):** Traverses `USER_SUPERSEDES` edges up to depth 10, replacing superseded nodes with their newest descendants.
   - **Pass 2 (Context Pull):** Pulls single-hop supporting facts linked by `SUPPORTS` edges.
   - **Pass 3 (Conflict Notification):** Identifies `CONFLICTS` pairs and emits Tauri UI event `memory:conflict_detected`.

### 7.4 Prompt Context Injection Layout
Retrieved context is injected into LLM system prompts formatted inside `<user_profile>`:

```markdown
<user_profile>
<memory_manifest total_active_facts="14">
  Preferences: 4 | Relationships: 3 | Skills: 2 | Projects: 3 | Experiences: 2
</memory_manifest>

[Unresolved Contradictions]
- [Unresolved Conflict] "User prefers dark roast coffee." CONFLICTS WITH "User prefers tea over coffee."

[Identity]
- Name is Alex.

[Constraints]
- User is allergic to peanuts.

[Active Tasks]
- Complete Vox v0.9.0 documentation.

[Active Goals]
- Train for half-marathon in October.

[Past Contexts within the Last 12 Hours]
- 2 hours ago:
  Alex discussed the Vox memory subsystem architecture and planned to refactor retrieval.

[Preferences]
- [Yesterday] User prefers oat milk over soy milk.
- [3 hours ago] User prefers dark mode interfaces.

</user_profile>
```

---

## 8. Implementation & Verification Status

- **Working Memory & Turn FIFO:** Active in RAM (`ConversationManager`).
- **Differential Compaction & Staging WAL:** Active. Staged tasks promoted on session end, staged context purged.
- **Background Worker & Cooperative Yielding:** Active. OS thread debounces 30s during idle and yields immediately when pipeline becomes active.
- **3-Phase Ingestion Orchestration:** Active. Dual-defense hard deduplication ($O(1)$ Merge), multi-tier NLI routing (DeBERTa-v3), and atomic multi-table transaction persistence.
- **Two-Tier Budgeted Retrieval:** Active. 15% hard cap split (7% foundational/operational, 8% semantic with single-query SQL window partitioning), Time-Windowed Context Chaining, 3-pass edge resolution, `<memory_manifest>` header, and `<user_profile>` injection.
- **Test Coverage:** Idempotent database migrations, worker processing, retrieval budgeting, and LLM compaction quality evaluation verified by test suite (`memory_v3_schema_test`, `memory_v3_retrieval_test`, `memory_v3_worker_test`).
