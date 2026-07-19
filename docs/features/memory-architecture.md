# Memory Architecture Ledger — Vox v0.9.0

> **Authoritative Technical Architecture Document**  
> **Scope:** Authoritative record of the finalized Vox V3 Human-Centric Cognitive Memory Subsystem architecture, ML models, Turso/libSQL database integration, background workers, relationship graph, and strict NLI edge resolution.

---

## 1. Subsystem Architecture Overview

Vox implements a **V3 Human-Centric Cognitive Memory System** designed to capture everyday human trace data (daily routines, culinary efforts, language learning progress, social dynamics, personal emotions, tasks, and hobbies) with zero pipeline latency stalls. The system operates on a hybrid architecture combining RAM-based Working Memory and a libSQL/Turso-backed Personal Memory Graph. 

The V2 episodic vector RAG (`episodes` table, keyword-vector RRF fusion, and FTS indexing) is completely deprecated and removed. It is replaced with **Time-Windowed Context Chaining** over raw text paragraphs.

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ Vox Memory Subsystem Architecture (v3)                                                      │
│                                                                                             │
│ 1. WORKING MEMORY (`ConversationManager` in `services/memory/working_memory.rs`)           │
│    - Transient FIFO turn history in RAM.                                                    │
│    - Manages context window allocation, token estimation, and system prompt updating.      │
│    - Point-of-idle compactions and Critical maintenance shifts.                             │
│                                                                                             │
│ 2. TIME-WINDOWED CONTEXT CHAINING (`services/memory/personal_memory.rs`)                   │
│    - Solves the micro-session context erasure bug.                                         │
│    - Loads raw text session context paragraphs chronologically within a 12-hour window.     │
│    - Distant memory fallback loads the latest context older than 7 days if none in window.  │
│    - Avoids vector search coordinate centroid drift and I/O overhead.                       │
│                                                                                             │
│ 3. PERSONAL MEMORY GRAPH (`services/memory/personal_memory.rs`)                             │
│    - Directed graph structure stored in `memory_facts`, `memory_facts_vectors`, and         │
│      `memory_relations` Turso/libSQL tables.                                                │
│    - Asynchronous background processing queue (`personal_memory_queue`).                    │
│    - Local ONNX-based Natural Language Inference (DeBERTa-v3) contradiction classifier.    │
│    - 3-Pass Edge Resolution (Pointer Swaps, Context Pulls, and Conflict Shadowing).         │
│    - Chronological sorting of retrieved semantic facts grouped by collection.               │
│                                                                                             │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Integrated Machine Learning Models

### 2.1 Hot-Path Query Classifier (`query-sieve`)
- **Model:** `query-sieve` (DistilBERT fine-tuned for voice query classification).
- **Location:** Integrated via native Rust bindings in `services/memory/classifier.rs`.
- **Purpose:** Short-circuits generic chatter turns (*"hello"*, *"thanks"*, *"okay"*), saving 100% of ONNX query embedding generation and database search latency.
- **Performance:** $< 1.5$ ms CPU inference overhead.

### 2.2 Multilingual Dense Embedder (`BGE-M3`)
- **Model ID:** `BAAI/bge-m3` (`Xenova/bge-m3` ONNX quantized INT8).
- **Location:** `~/.vox/models/embedding/bge-m3/model_quantized.onnx` (544 MB).
- **Vector Dimensions:** 1,024 float dimensions.
- **Normalization:** L2 Unit Normalization ($||v|| = 1.0$) applied to output embeddings.
- **Scope:** Computed strictly for the **Semantic** type collections (Preferences, Relationships, Skills, Projects, Experiences). Foundational and Operational items are never embedded.

### 2.3 Pairwise NLI Classifier (`deberta-v3-xsmall-nli`)
- **Model:** `deberta-v3-xsmall-nli` (Quantized ONNX model running locally via `ort` v2).
- **Location:** `~/.vox/models/nli/deberta-v3-xsmall-nli/model.onnx`.
- **Purpose:** Performs pairwise semantic comparison of newly extracted facts against existing historical facts to classify relationship edges (`entailment`, `contradiction`, or `neutral`).
- **Safety:** Thread-safe Mutex wrapper enforces sequential session access to align with ONNX session thread-safety constraints.

---

## 3. Storage Layer & Database Architecture

### 3.1 Turso / libSQL Concurrency & Configuration
Vox utilizes the pure-Rust **Turso/libSQL Engine** (`turso` crate) configured with:
- **Write-Ahead Logging (WAL):** `journal_mode = WAL` enables concurrent database reads while the background memory worker writes facts.
- **Busy Timeout:** `busy_timeout = 5000`ms prevents database locking and transaction deadlocks.

### 3.2 Schema Definition
The database schema consists of **exactly four core tables** representing the personal memory graph nodes, vectors, edges, and ingestion queue:

```sql
-- 1. Core Facts Table (Houses all 10 collections under 3 structural types)
CREATE TABLE IF NOT EXISTS memory_facts (
    id           TEXT PRIMARY KEY,              -- UUID v4
    type         TEXT NOT NULL,                 -- 'foundational', 'operational', 'semantic'
    collection   TEXT NOT NULL,                 -- Context, Constraints, Identity, Preferences, Relationships, Skills, Projects, Tasks, Goals, Experiences
    fact         TEXT NOT NULL,
    source       TEXT NOT NULL DEFAULT 'LLM',   -- 'LLM', 'User', or 'Import'
    status       TEXT NOT NULL DEFAULT 'active',-- 'active', 'superseded', 'deleted'
    session_id   TEXT NOT NULL DEFAULT '',      -- Provenance tracking
    turn_id      TEXT NOT NULL DEFAULT '',      -- Provenance tracking
    created_at   INTEGER NOT NULL               -- Millisecond epoch timestamp
);

-- 2. Separate Vectors Table (SQLite Page-loading performance optimization - SEMANTIC ONLY)
CREATE TABLE IF NOT EXISTS memory_facts_vectors (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    fact_id     TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    collection  TEXT NOT NULL,
    embedding   BLOB NOT NULL                   -- 1024-dimensional BGE-M3 dense vector (F32_BLOB)
);

-- 3. Directed Relations Graph Table (SUPPORTS / CONFLICTS / USER_SUPERSEDES)
CREATE TABLE IF NOT EXISTS memory_relations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id     TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    to_id       TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
    relation    TEXT NOT NULL,                  -- 'SUPPORTS', 'CONFLICTS', 'USER_SUPERSEDES'
    created_at  INTEGER NOT NULL,
    UNIQUE(from_id, to_id, relation)
);

-- 4. Unified Ingestion Queue & Staging Table (Acts as Queue AND Crash Recovery WAL)
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
CREATE INDEX IF NOT EXISTS idx_pmq_status ON personal_memory_queue(status, created_at ASC);
```

### 3.3 Graph Schema Optimizations
1. **$O(1)$ Status Optimization:** The `status` column directly in `memory_facts` is used to filter out superseded or deleted facts. Prompt retrieval queries require simple `WHERE status = 'active'` checks, bypassing complex graph traversals.
2. **Page-loading Separation:** The heavy 1024-dimensional float vector blobs are separated into the `memory_facts_vectors` table, keeping the primary `memory_facts` rows tiny and highly queryable.

---

## 4. Ingestion & Compaction Processing Paths

```
    In-Session Compaction (Working Memory Token Limit Triggered)
                         │
                         ▼
        LLM Extracts Flat JSON (COMPACTION_SYSTEM_PROMPT)
            ├── Ephemeral Tasks/Goals ──► Queue (status = 'staged') [WAL]
            └── Durable Semantic Facts ──► Queue (status = 'pending')
                                                 │
                                                 ▼
                                    Background Worker Sweep (Idle State)
                                                 │
                                                 ▼
                                     Compute BGE-M3 Embeddings
                                                 │
                                                 ▼
                                   Prune: Cosine Sim < 0.82?
                                         ├── Yes ──► Skip NLI (Neutral)
                                         └── No  ──► Execute local DeBERTa-v3 NLI
                                                           │
                                                           ▼
                                               Write Nodes & Relations
```

### 4.1 In-Memory Compaction (Single Master Prompt)
- When active token count in working memory exceeds 85% of the context window (e.g. 4096 tokens), an in-memory compaction occurs.
- The system prompts the LLM using a static `COMPACTION_SYSTEM_PROMPT` containing no JSON examples (preventing overfitting). It returns a flat JSON payload containing:
  - `summary`: A raw rolling paragraph of the conversational context flow.
  - `personal_memory`: Flat dictionary of facts categorized by collection name.

### 4.2 In-Session Queueing (Lightweight WAL)
- During compaction, facts are pushed to `personal_memory_queue`:
  - **Durable Core facts** (`type = 'semantic'`) are enqueued with `status = 'pending'`.
  - **Ephemeral Tasks and Goals** (`type = 'operational'`) are enqueued with `status = 'staged'`, serving as a crash-safe Write-Ahead Log.

### 4.3 Background Worker Sweep & NLI Pruning (`memory_worker.rs`)
1. Sweeps `personal_memory_queue` for `pending` jobs when the pipeline is idle (`state.is_idle`).
2. Calculates embeddings **only** for `type = 'semantic'` items.
3. Performs **NLI Cosine Pruning**: computes cosine similarity between the new fact and existing candidate facts in the database.
   - If similarity $< 0.82$ (`candidate_similarity_search_threshold`), NLI is skipped, and they are classified as `Neutral`.
   - If similarity $\ge 0.82$, it executes local DeBERTa NLI. Entailment ($\ge 0.85$) or Contradiction ($\ge 0.85$) write `SUPPORTS` or `CONFLICTS` relations.
4. If NLI fails, the error is bubbled and the job is marked `failed` (no silent error swallowing).

### 4.4 Session End Consolidation Sweep
On pipeline idle timeout (exceeding `auto_sleep_timeout`), the system:
1. Deletes all intermediate `'staged'` tasks and goals for the current session from the queue.
2. Writes the final session context paragraph directly to `memory_facts` as `Context` (`type = 'operational'`). **It is never embedded.**
3. Writes finalized tasks and goals directly to `memory_facts`. **These are never embedded.**

---

## 5. Retrieval Pipeline & Graph Edge Resolution

### 5.1 Token Budget Allocation (15% Hard Cap)
The system enforces a **15% hard cap of the overall context window** for memory injection. This is split into two prioritized, isolated tiers:

#### Tier 1: Foundational & Operational Core (7% hard cap)
- **Collections:** `Context` (time-windowed chained summaries), `Identity`, `Constraints`, `Tasks`, `Goals`.
- **Retrieval Style:** Vectorless SQL load.
  - *Identity & Constraints:* Unconditionally loaded if active.
  - *Tasks & Goals:* Loaded deterministically where `type = 'operational'` AND `status = 'active'`.
  - *Context Chaining:* Fetches raw text `Context` facts within the last `context_chaining_window_hours` (12h window) and formats them as a chronological relative timeline. Prepend until budget is filled.
  - *Distant Fallback:* If no contexts exist in the window, retrieves the latest single context older than 7 days inside a `[Recollection (Distant Memory)]` block.

#### Tier 2: Semantic Profiles (8% hard cap)
- **Collections:** `Preferences`, `Relationships`, `Skills`, `Projects`, `Experiences`.
- **Retrieval Style:** Vector search + **Interleaved Round-Robin Selection**:
  1. Fetch top $K=5$ candidates independently from each of these semantic vector collections.
  2. Select candidate 1 from each bucket, then candidate 2, and so on, until the 8% budget limit is reached. This prevents a single collection from crowding out others.
  3. Sort the resolved semantic facts chronologically by `created_at` before prompt injection.

### 5.2 Personal Memory 3-Pass Edge Resolution
Retrieval filters semantic candidates using an in-memory 3-pass resolution loop to ensure consistency:

```text
Fetched Candidate Nodes (Semantic Scan + Unconditional Identity Nodes)
  │
  ▼
Pass 1: Pointer Swap (USER_SUPERSEDES)
  │  - Recursively swaps superseded nodes with their newest descendants.
  │  - Cycle-protection guard limits maximum depth traversal to 10.
  ▼
Pass 2: Context Pull (SUPPORTS)
  │  - Pulls single-hop supporting facts linked by SUPPORTS edges.
  ▼
Pass 3: Conflict Shadowing (CONFLICTS)
  │  - Identifies CONFLICTS edges between active nodes.
  │  - Suppresses the older node based on its created_at timestamp.
  ▼
Final Injected <user_profile> Block
```

---

## 6. Implementation Status

- **Personal Memory Context Injection:** 100% complete and fully active. The `<user_profile>` block is dynamically assembled and injected into system prompts at every turn.
- **Asynchronous Fact Extraction & Graph Processing:** Active during compaction events. Facts are processed out-of-band by the memory worker thread, avoiding frontend thread blockage.
- **Compaction Quality Evaluation:** Verified via integration tests using LLM-as-a-judge dataset validations.
