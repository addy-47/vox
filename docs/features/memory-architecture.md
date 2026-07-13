# Memory Architecture Ledger — Vox v0.9.0

> **Authoritative Technical Architecture Document**  
> **Scope:** Complete record of Vox Memory Subsystem architecture, ML models, Turso vector database integration, bullet-chunk ingestion, zero-magic-number dynamic retrieval, comparative benchmarks, and future recommendations.

---

> **Terminology Update (v1):** This document previously described a "Semantic Memory" / "Knowledge Graph" layer and "Entity Extraction (NER)". Those terms are **retired**. The implemented system uses **Personal Memory** (a structured key/value user profile) and **Memory Extraction** (compaction-based profile extraction that reuses the chat LLM). The sections below reflect the current v1 implementation.

## 1. Subsystem Architecture Overview

Vox implements a 3-tier cognitive memory architecture designed for real-time voice interaction without pipeline latency stalls.

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ Vox Memory Subsystem Architecture                                                           │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                             │
│ 1. WORKING MEMORY (`ConversationManager` in `services/memory/working_memory.rs`)           │
│    - Transient FIFO turn history in RAM.                                                    │
│    - Handles context window allocation, token estimation, and system prompt updating.      │
│    - Opportunistic background compactions & Critical maintenance shifts.                    │
│                                                                                             │
│ 2. EPISODIC MEMORY (`services/memory/retrieval.rs`, `persistence/memory_worker.rs`)        │
│    - Hot-path Query Classification (`query-sieve` distilbert model).                        │
│    - Dense Multilingual Vector Embeddings (BGE-M3 1024-dim ONNX model).                     │
│    - Bullet-Chunk Compaction Ingestion (splits compactions into discrete fact chunks).       │
│    - Turso Database Storage (`episodes` table with `F32_BLOB(1024)` vectors).               │
│    - Zero-Magic-Number Dynamic Round-Robin Retrieval & Session Budgeting.                   │
│                                                                                             │
│ 3. PERSONAL MEMORY (Implemented v1)                                                             │
│    - Turso KV store: `personal_memory` (current values) + `personal_memory_history` (append-only log). │
│    - `<user_profile>` block injected into the system prompt every turn via structured key/value lookup. │
│    - NOT semantic search, NOT FTS5. Key normalization collapses synonyms to canonical keys/categories. │
│    - Extraction via `COMPACTION_SYSTEM_PROMPT_V2`: LLM emits a single `profile_updates` array (items carry a `category`). │
│    - LIVE PATH: retrieval/injection is wired into the pipeline; extraction runs only when an LLM provider is passed │
│      (currently benchmarks / test paths) — not yet active in the live pipeline (FIFO maintenance only). │
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

---

## 3. Storage Layer & Turso Native Vector Capabilities Analysis

### 3.1 Turso Database Architecture (libsql Engine)
Vox uses the pure-Rust **Turso Database Engine** (`turso` crate v0.7.0-pre, built on `libsql` / Limbo) for thread-safe asynchronous WAL-mode persistence.

### 3.2 Current Implementation vs. Turso Native Vector Capabilities

| Dimension | Current Vox Implementation | Native Turso (`libsql`) Vector Engine Capabilities | Scalability Impact |
|---|---|---|---|
| **Vector Storage Format** | Custom `F32_BLOB(1024)` byte slice in Turso DB | Native `F32_BLOB(1024)` vector column type | Standardized binary format |
| **Vector Search Method** | In-memory loop in Rust decoding blobs & calling `cosine_similarity()` | Native SQL `vector_distance_cos(embedding, ?)` function & `vector_top_k()` | Offloads SIMD vector math to database engine |
| **Indexing Structure** | Sequential table scan over `episodes` | Native **DiskANN / vector index** (`CREATE INDEX idx ON episodes(libsql_vector_idx(embedding))`) | Replaces $O(N)$ linear scans with $O(\log N)$ ANN graph traversal |
| **Scale Target** | 5 to 20 Sessions (~300 to 1,000 chunks) | **100 to 1,000+ Sessions (100,000+ chunks)** | Enables sub-10ms vector search across thousands of sessions |

---

## 4. Ingestion Write Path & Bullet-Chunk Architecture

### 4.1 Episodic Ingestion Path (`persistence/memory_worker.rs`)

**Live path (idle sweep):** When the pipeline transitions to `PipelineIdle`, the `vox-memory-worker` OS thread runs `sweep_next_pending_session`, which selects the oldest session with `embedding_status = 'pending'` (excluding the current active session) and ingests its **last assistant turn text** (from `turns.assistant_text`) as the compaction summary. That summary is split into **Bullet-Chunks** (individual lines $\ge 15$ characters), each embedded via BGE-M3 (1024-dim) and written to the `episodes` table, after which `sessions.embedding_status` is set to `'embedded'`.

**Intended / test path (`SessionReadyForIngestion`):** The worker also handles a `SessionReadyForIngestion { session_id, summary }` event (used by benchmarks and tests). It enforces a hard invariant — it rejects ingestion for the *active* pipeline session to prevent race conditions — then runs the same bullet-chunk embedding + `episodes` write. This event is **not emitted on the live runtime path** today; the idle sweep above is what drives live Episodic ingestion.

### 4.2 Personal Memory Extraction (`services/memory/working_memory.rs` → `persistence/memory_worker.rs`)

Personal Memory facts are extracted during **Working Memory compaction** (LLM-driven context compaction in `ConversationManager::perform_compaction_maintenance`):

1. When the context crosses the critical threshold and an LLM provider is available, the conversation history is compressed using `COMPACTION_SYSTEM_PROMPT_V2`.
2. The LLM returns JSON: `{ "summary": "...", "memory_updates": [ { "category": "...", "key": "...", "value": "...", "confidence": "..." } ] }`. The `memory_updates` field is parsed (alias `profile_updates`) into a `Vec<ProfileUpdate>` via `CompactionResponse`.
3. The extracted `profile_updates` are forwarded as a `MemoryWorkerEvent::ProfileUpdatesReady { updates }` to the memory worker.
4. `apply_profile_updates` normalizes each update (collapsing synonym keys/categories — e.g. `name|username|user_name → (Identity, name)`, `current_project|project → (Projects, current_project)`, `role|job|occupation → (Identity, role)`) and writes it to **both** tables:
   - `personal_memory` via `INSERT OR REPLACE` (current value, keyed by canonical `key`).
   - `personal_memory_history` via `INSERT` (append-only log of every change — the "Temporal Memory" concept).

> **Live-path gap:** In the live pipeline (`services/pipeline.rs`), `build_context` is invoked with `None` for the LLM provider, so the LLM-compaction / profile-extraction branch does **not** run at runtime — only FIFO maintenance runs. Extraction therefore currently works only when a provider is passed (the benchmark binaries `src/bin/vox_sim_bench.rs` and `src/bin/vox_multi_session_bench.rs`, and unit tests). Personal Memory **retrieval/injection** is fully wired into the live pipeline; **extraction** is not yet active on the live path.

---

## 5. Retrieval Pipeline & Zero-Magic-Number Dynamic Budgeting

### 5.1 Retrieval Flow (`services/memory/retrieval.rs`)
```text
User Query -> Query Classifier (GENERIC -> Bypass RAG | SEMANTIC -> Proceed)
   │
   ▼
BGE-M3 Query Embedding (1024-dim normalized vector)
   │
   ▼
Turso DB Query (SELECT candidates WHERE session_id != current AND similarity >= 0.65)
   │
   ▼
Zero-Magic-Number Dynamic Round-Robin Budgeting
   │
   ▼
System Prompt Context Update (`conv_mgr.update_system_prompt`)
```

### 5.2 Zero-Magic-Number Dynamic Budgeting Algorithm
- **Input Budget Calculation:** `max_token_budget = context_size * max_context_share` (e.g. 20% of 4096 = 819 tokens; 20% of 1M = 200,000 tokens).
- **Per-Session Share Cap:** `per_session_token_budget = max_token_budget / num_active_sessions`. Prevents recent sessions from flooding the context window and starving older user profile facts.
- **Round-Robin Interleaving:** Interleaves candidate facts across sessions ordered by similarity score until `max_token_budget` is reached.

---

## 6. Model Benchmark & Comparative Findings

### 6.1 Head-to-Head Embedding Model Comparison (`bge_m3_vs_minilm_bench`)

| Scenario / Test Pair | Query Language | Summary Type | MiniLM Cosine Sim | BGE-M3 Cosine Sim | MiniLM @ `0.55` | BGE-M3 @ `0.55` |
|---|---|---|---|---|---|---|
| **Short Query vs Summary** | English | Monolithic (200 words) | `0.2890` | **`0.7311`** | ❌ FAIL | ✅ **PASS** |
| **Fact Query vs Summary** | English | Monolithic (200 words) | `0.3228` | **`0.7562`** | ❌ FAIL | ✅ **PASS** |
| **Bullet Chunk (Color)** | English | Focused Bullet | `0.5852` | **`0.8382`** | ✅ PASS | ✅ **PASS** |
| **Bullet Chunk (Lang)** | English | Focused Bullet | `0.3059` | **`0.7667`** | ❌ FAIL | ✅ **PASS** |
| **Multilingual Hindi Query** | Devanagari (`नमस्ते...`) | English Summary | `0.2016` | **`0.7012`** | ❌ FAIL | ✅ **PASS** |
| **Multilingual Hinglish** | Hinglish (`Mera color...`) | English Summary | `0.3018` | **`0.6678`** | ❌ FAIL | ✅ **PASS** |
| **Multilingual Hindi Pair** | Hindi (`भारत की राजधानी...`)| Hindi Summary | `0.5598` | **`0.8524`** | ✅ PASS | ✅ **PASS** |
| **Pass Rate @ Strict 0.55** | — | — | **28.6%** (2/7) | **100.0%** (7/7) | ❌ FAIL | 🏆 **PASS** |

---

## 7. Recommendations & Future Architectural Roadmap

1. **Structured User Profile Key-Value Store — ✅ IMPLEMENTED (v1) as `personal_memory` / `personal_memory_history`:**
    - `personal_memory (key TEXT PRIMARY KEY, category TEXT, value TEXT, updated_at INTEGER)` holds the current value per canonical key (written via `INSERT OR REPLACE`).
    - `personal_memory_history (id, key, category, value, recorded_at)` is an append-only log of every change (Temporal Memory).
    - `load_user_profile` performs a structured `SELECT category, key, value FROM personal_memory ORDER BY category, key` and formats a `<user_profile>` block (truncated to ~120 tokens) injected into the system prompt every turn.
    - Extraction (Working Memory compaction → `apply_profile_updates`) is implemented but **not yet active on the live pipeline path** (see §4.2).
2. **Hybrid Search (Turso FTS5 + BGE-M3 RRF) — future recommendation:**
    - Combine Turso `FTS5` BM25 full-text keyword search with BGE-M3 dense vector search using Reciprocal Rank Fusion (RRF).
    - Ensures exact entity matches (`teal`, `Vox`, `Alex`) shoot to Rank #1.
3. **Turso Native Vector Indexing (`libsql_vector_idx`) — future recommendation:**
    - Replace the current in-memory linear cosine scan in Rust with Turso's native `libsql` vector search extensions and DiskANN indexing to support 100+ sessions at sub-10ms latency.
