# Memory Architecture Ledger — Vox v0.9.0

> **Authoritative Technical Architecture Document**  
> **Scope:** Authoritative record of the current Vox Memory Subsystem architecture, ML models, Turso/libSQL database integration, background workers, relationship graph, and NLI edge resolution.

---

## 1. Subsystem Architecture Overview

Vox implements a 3-tier cognitive memory architecture designed for real-time voice interaction without pipeline latency stalls. The system operates on a hybrid architecture combining RAM-based Working Memory, dense vector/keyword Episodic RAG, and an NLI-driven Personal Memory Graph.

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ Vox Memory Subsystem Architecture                                                           │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                             │
│ 1. WORKING MEMORY (`ConversationManager` in `services/memory/working_memory.rs`)           │
│    - Transient FIFO turn history in RAM.                                                    │
│    - Handles context window allocation, token estimation, and system prompt updating.      │
│    - Point-of-idle compactions & Critical maintenance shifts.                               │
│                                                                                             │
│ 2. EPISODIC MEMORY (`services/memory/retrieval.rs`, `persistence/memory_worker.rs`)        │
│    - Hot-path Query Classification (`query-sieve` DistilBERT model).                        │
│    - Dense Multilingual Vector Embeddings (BGE-M3 1024-dim ONNX model).                     │
│    - Bullet-Chunk Compaction Ingestion (splits compactions into discrete fact chunks).       │
│    - Turso Database Storage (`episodes` table with `F32_BLOB(1024)` vectors).               │
│    - Hybrid Search combining native SQLite FTS5 keyword matching and dense vector search.   │
│    - Blended ranking using Reciprocal Rank Fusion (RRF) with dynamic token budgeting.       │
│                                                                                             │
│ 3. PERSONAL MEMORY GRAPH (`services/memory/personal_memory.rs`)                             │
│    - Directed graph structure stored in `memory_facts`, `memory_facts_vectors`, and         │
│      `memory_relations` Turso/libSQL tables.                                                │
│    - Asynchronous background processing queue (`personal_memory_queue`).                    │
│    - Local ONNX-based Natural Language Inference (DeBERTa-v3) contradiction classifier.    │
│    - 3-Pass Edge Resolution (Pointer Swaps, Context Pulls, and Conflict Shadowing).         │
│    - Injected `<user_profile>` prompt context block updated dynamically at each turn.       │
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
- **Multilingual Alignment:** Native cross-lingual semantic alignment across 100+ languages (English, Hindi Devanagari, Hinglish).

### 2.3 Pairwise NLI Classifier (`deberta-v3-xsmall-nli`)
- **Model:** `deberta-v3-xsmall-nli` (Quantized ONNX model running locally via `ort` v2).
- **Location:** `~/.vox/models/nli/deberta-v3-xsmall-nli/model.onnx`.
- **Purpose:** Performs pairwise semantic comparison of newly extracted facts against existing historical facts to classify relationship edges (`entailment`, `contradiction`, or `neutral`).
- **Concurrency Safety:** Thread-safe Mutex wrapper enforces sequential session access to align with ONNX session thread-safety constraints.

---

## 3. Storage Layer & Database Architecture

### 3.1 Turso / libSQL Concurrency & Configuration
Vox utilizes the pure-Rust **Turso/libSQL Engine** (`turso` crate) configured with:
- **Write-Ahead Logging (WAL):** `journal_mode = WAL` enables concurrent database reads while the background memory worker writes facts.
- **Busy Timeout:** `busy_timeout = 5000`ms prevents database locking and transaction deadlocks.

### 3.2 Schema Definition

```sql
-- 1. Compaction summaries/episodes search
CREATE TABLE IF NOT EXISTS episodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    summary TEXT NOT NULL,
    embedding BLOB NOT NULL, -- F32_BLOB (1024-dimensional normalized vector)
    created_at INTEGER NOT NULL,
    token_count INTEGER NOT NULL
);

-- 2. Personal Memory Graph: Nodes
CREATE TABLE IF NOT EXISTS memory_facts (
    id TEXT PRIMARY KEY,
    collection TEXT NOT NULL, -- Category (e.g. Identity, Preferences, Projects, Skills)
    fact TEXT NOT NULL,
    source TEXT NOT NULL,     -- LLM, User, Import
    session_id TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS memory_facts_vectors (
    fact_id TEXT PRIMARY KEY,
    collection TEXT NOT NULL,
    embedding BLOB NOT NULL, -- 1024-dimensional vector blob
    FOREIGN KEY(fact_id) REFERENCES memory_facts(id) ON DELETE CASCADE
);

-- 3. Personal Memory Graph: Edges
CREATE TABLE IF NOT EXISTS memory_relations (
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    relation TEXT NOT NULL,  -- SUPPORTS, CONFLICTS, USER_SUPERSEDES
    created_at INTEGER NOT NULL,
    PRIMARY KEY (source_id, target_id, relation),
    FOREIGN KEY(source_id) REFERENCES memory_facts(id) ON DELETE CASCADE,
    FOREIGN KEY(target_id) REFERENCES memory_facts(id) ON DELETE CASCADE
);

-- 4. Asynchronous Queue
CREATE TABLE IF NOT EXISTS personal_memory_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    fact_text TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed
    attempts INTEGER NOT NULL DEFAULT 0,
    error_msg TEXT,
    created_at INTEGER NOT NULL
);
```

---

## 4. Ingestion & Compaction Processing Paths

### 4.1 Episodic Ingestion Path (`persistence/memory_worker.rs`)
1. When the pipeline transitions to `PipelineIdle`, the memory worker picks up sessions marked as `pending` for ingestion.
2. The session summary is split into **Bullet-Chunks** (individual lines $\ge 15$ characters).
3. The worker generates 1024-dimensional BGE-M3 embeddings for each chunk and writes them to the `episodes` table.

### 4.2 Personal Memory Fact Queue Path
1. At the end of a session or during point-of-idle compactions, `ConversationManager` calls context compaction using `COMPACTION_SYSTEM_PROMPT_V2`.
2. The LLM returns a structured JSON payload containing personal memory facts keyed by category (`Identity`, `Preferences`, `Experiences`, `Projects`, etc.).
3. The extracted facts are pushed to `personal_memory_queue` in the database.
4. The background memory worker thread retrieves `pending` queue jobs. For each fact:
   - It generates the BGE-M3 vector embedding and writes it to `memory_facts` and `memory_facts_vectors`.
   - It queries same-collection candidate facts from the database.
   - It executes pairwise local DeBERTa-v3 NLI inferences to detect semantic overlap or contradictions.
   - It updates the graph structure in `memory_relations`:
     - **Entailment (score $\ge 0.85$):** Creates a `SUPPORTS` edge.
     - **Contradiction (score $\ge 0.85$):** Creates a `CONFLICTS` edge.
     - **User Direct Update:** Creates a `USER_SUPERSEDES` edge pointing from the old fact to the new fact.

---

## 5. Retrieval Pipeline & Graph Edge Resolution

### 5.1 Hybrid Episodic Retrieval & RRF (`services/memory/retrieval.rs`)
1. For every incoming user query, the system generates a BGE-M3 query embedding.
2. The query is executed simultaneously across:
   - **Dense Vector Search:** Distance scans comparing the query embedding against the `episodes` embeddings (filtering by `similarity_threshold = 0.65`).
   - **Native Keyword Search:** Fast FTS match queries over summaries.
3. The candidates from both channels are merged using **Reciprocal Rank Fusion (RRF)**.
4. Chunks are dynamically allocated across sessions using **Round-Robin Interleaving** to build the context prompt.

### 5.2 Personal Memory 3-Pass Edge Resolution (`services/memory/personal_memory.rs`)
During query retrieval, semantic candidates from categories other than `Identity` are fetched from `memory_facts_vectors` (using the user's query vector). The `Identity` facts are always loaded directly. These nodes are resolved through an in-memory 3-pass edge resolution loop:

```text
Fetched Candidate Nodes (Semantic Scan + Always-Inject Identity Nodes)
  │
  ▼
Pass 1: Pointer Swap (USER_SUPERSEDES)
  │  - Recursively replaces superseded nodes with their newest descendants.
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

## 6. Live Path Implementation Status

- **Episodic RAG Retrieval:** Fully wired and active on the live audio pipeline path.
- **Personal Memory Context Injection:** Fully wired and active. The `<user_profile>` block is dynamically resolved and injected into system prompts at every turn.
- **Asynchronous Fact Extraction & Graph Processing:** Active during compaction events. Facts are queued in `personal_memory_queue` and processed out-of-band by the memory worker thread, avoiding frontend thread blockage.
