# Memory Architecture Ledger — Vox v0.9.0

> **Authoritative Technical Architecture Document**  
> **Scope:** Complete record of Vox Memory Subsystem architecture, ML models, Turso vector database integration, bullet-chunk ingestion, zero-magic-number dynamic retrieval, comparative benchmarks, and future recommendations.

---

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
│ 3. SEMANTIC MEMORY & PROFILE LAYER (Planned Future Architecture)                            │
│    - Structured Key-Value User Profile Store (`user_profile` table in Turso DB).            │
│    - Hybrid Search: Turso `FTS5` BM25 Lexical Search + BGE-M3 Dense Vector RRF.              │
│    - Turso Native Vector Indexing (`libsql_vector_idx` / DiskANN).                           │
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

### 4.1 Ingestion Pipeline (`persistence/memory_worker.rs`)
1. Active session finishes or transitions to `PipelineIdle`.
2. `SessionReadyForIngestion { session_id, summary }` event sent to `vox-memory-worker` OS thread.
3. Worker checks `current_session_id` guard (rejects active pipeline session to prevent race conditions).
4. `summary` is split into **Bullet-Chunks** (individual lines $\ge 15$ characters).
5. Each bullet chunk is embedded via BGE-M3 (1024-dim) and written to `episodes` table.
6. `sessions.embedding_status` updated to `'embedded'`.

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

1. **Structured User Profile Key-Value Store (`user_profile` table):**
   - Create `user_profile (key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER)` in Turso DB.
   - Background worker automatically extracts core identity facts (`user_name`, `user_role`, `favorite_color`, `app_name`).
   - Injected into system prompt as a fixed `<user_profile>` block (~50 tokens), guaranteeing **100% identity recall across infinite sessions**.
2. **Hybrid Search (Turso FTS5 + BGE-M3 RRF):**
   - Combine Turso `FTS5` BM25 full-text keyword search with BGE-M3 dense vector search using Reciprocal Rank Fusion (RRF).
   - Ensures exact entity matches (`teal`, `Vox`, `Alex`) shoot to Rank #1.
3. **Turso Native Vector Indexing (`libsql_vector_idx`):**
   - Replace in-memory vector loop in Rust with Turso's native `libsql` vector search extensions and DiskANN indexing to support 100+ sessions at sub-10ms latency.
