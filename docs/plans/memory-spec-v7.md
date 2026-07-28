# Vox v7 Cognitive Memory Subsystem Architecture Specification

**Status**: Frozen Master Architectural Specification  
**Version**: 7.6 (Validated Architecture, Precision Retrieval & 4-Stage Pipeline Specification)  
**Target Systems**: `app/src-tauri/src/services/memory/` (Rust Backend)  

---

## 1. Architectural Paradigm Shift & Core Principles

The v7 memory architecture provides a unified, deterministic, and domain-agnostic memory engine for real-time voice AI. It resolves critical scaling and domain-coupling flaws:

1. **Unbounded Deterministic Fetch Elimination**: `Identity` facts are fetched as bounded invariants (`WHERE status = 'active'`). All active identity facts are retrieved directly without arbitrary token truncation.
2. **Operational State & Agent Agenda (`Directives`)**: `Directives` represent agent tasks and operational goals. They act as top-level parent seeds **ONLY on Turn 1 of a new session**, preventing prompt pollution across ongoing turns.
3. **Integrated Constraint Search**: `Constraints` are removed from direct SQL recency dumps. They are indexed in **Semantic Vector Search** and pulled dynamically as child nodes via graph expansion edges (`RESTRICTS` / `CONFLICTS`).
4. **4-Stage Pipeline with 2 Concurrent Model Workers in Stage 3**: Pipeline stages are consolidated into 4 clean stages (Dedup $\rightarrow$ Embedding $\rightarrow$ Unified Edge & State Evaluation $\rightarrow$ Commit & Prune). In Stage 3, **2 dedicated model workers** (Sub-Branch A: DeBERTa NLI and Sub-Branch B: ModernBERT Edge Classifier) execute concurrently via `tokio::join!`, merging results in memory before a single atomic SQL write.

### 1.1 Non-Deletion Provenance Mandate
**Zero hard deletions (`DELETE FROM memory_facts`) are permitted during pipeline execution.**
- When the ingestion pipeline invalidates an old fact (via deduplication or NLI supersession), it updates `memory_facts.status = 'inactive'`.
- When a user manually deletes a fact, the system updates `memory_facts.status = 'deleted'`.
- Only facts with `memory_facts.status = 'active'` are eligible for active RAG context retrieval. Inactive facts remain in Turso DB with full `memory_relations` provenance intact.

---

## 2. Hardware Tier & Memory Capability Matrix

| Tier | Hardware Profile | Ingestion | Retrieval | Operating Mode |
| :--- | :--------------- | :-------: | :-------: | :------------- |
| **1A** | 8GB CPU-only | ❌ | ✅ FIFO only | Working Memory context buffer only; background worker dormant. |
| **1B** | 8GB+ with GPU | ✅ Full | ✅ Full | Async ingestion via `memory_worker.rs` during idle (`PipelineIdle`). |
| **2A** | Remote LLM + Local Audio | ✅ Full | ✅ Full | Same as 1B; LLM offloaded to remote server. |
| **2B** | Cloud LLM + Local Audio | ✅ Full | ✅ Full | Recommended default; tool calling native. |
| **3** | Realtime S2S (WebSocket) | ✅ Managed | ✅ Tool calls | Provider owns voice loop; memory injected as system context. |

---

## 3. Universal Domain-Agnostic Cognitive Taxonomy (6 Collections)

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                   Vox v7 Cognitive Taxonomy                                      │
├──────────────────────────────────┬────────────────────────────────┬──────────────────────────────┤
│ Deterministic & Bounded State    │ Declarative Semantic Graph     │ Session Ephemera             │
│ (SQL Fetch & NLI State)          │ (ANN Vector Search + LLM Edges)│ (In-Memory Prepending)       │
├──────────────────────────────────┼────────────────────────────────┼──────────────────────────────┤
│ • Identity (Gated Core Persona)  │ • Profile (User Persona/Tastes)│ • Narrative (Session Chain)  │
│ • Directives (Agent State/Tasks) │ • Entities (Projects/Tools/Etc)│                              │
│ • Constraints (Hard Boundaries)  │                                │                              │
└──────────────────────────────────┴────────────────────────────────┴──────────────────────────────┘
```

### 3.1 Domain Specification & Retrieval Policy Matrix

| Domain | Cognitive Purpose | Ingestion Dispatch Pipeline | Turn Retrieval Policy | Budget Cap |
| :--- | :--- | :--- | :--- | :--- |
| **`Identity`** | Core User Identity (name, age, language, baseline role). | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **Stage 3 Unified Eval** | Deterministic SQL (`WHERE status = 'active'`). Fetch ALL active identity facts. | Dynamic ~2% Context Window. |
| **`Directives`** | Agent Operational State (active tasks, workflow steps, promises). | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **Stage 3 Unified Eval** | **Parent Seed on Turn 1 ONLY**; Child Graph Node on Turns 2+. | Capped within Operational Budget. |
| **`Constraints`** | Hard Invariants & Boundaries (rules, safety limits, explicit bans). | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **Stage 3 Unified Eval** | **Semantic Vector Search + Graph Traversal** (`RESTRICTS` / `CONFLICTS`). | Capped within Semantic Budget. |
| **`Profile`** | User Persona & Tastes (skills, habits, secondary preferences). | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **Stage 3 Unified Eval** | Semantic Vector Search (ANN) + Graph Traversal. | Part of Semantic Budget (`semantic_budget_share`). |
| **`Entities`** | External Knowledge Graph (codebases, tools, APIs, services). | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **Stage 3 Unified Eval** | Semantic Vector Search (ANN) + Graph Traversal. | Part of Semantic Budget (`semantic_budget_share`). |
| **`Narrative`** | Session History Flow (ephemeral turn summaries). | Compaction summary generator | Backward Prepending Context Chain (`context_chaining_window_hours`). | 5% Context Window Cap. |

---

## 4. Frozen Calibration Thresholds & Candidate Selection Rules

| Threshold Constant | Value | Config / Source Location | Target System / Purpose |
| :--- | :---: | :--- | :--- |
| `primary_embedding_model` | **`MiniLM-L12`** | `~/.vox/models/embedding/` | 384d INT8 ONNX dense vector engine (~10ms CPU). |
| `edge_classifier_model` | **`ModernBERT-base`** | `~/.vox/models/classifier/` | 1-Pass INT8 ONNX Sequence Classifier (~35ms CPU, <120MB RAM). |
| `soft_vector_dedup_threshold` | **`0.95`** | Frozen Ingestion Rule | Soft vector deduplication threshold (Gate 1 calibrated: 0.0% false inactivations). |
| `nli_candidate_search_cutoff` | **`0.40`** | Frozen Ingestion Rule | Pre-filter cutoff to select candidate facts for Stage 3 NLI evaluation. |
| `edge_candidate_search_cutoff`| **`0.50 - 0.65`**| Frozen Connection Matrix | Domain-pair specific pre-filter cutoffs for Stage 3 Edge Classification. |
| `NLI_CONTRADICTION_THRESHOLD` | **`0.85`** | `nli-deberta-v3-base` ONNX | Minimum probability required for NLI `SUPERSEDES` / `CONFLICTS` classification. |
| `NLI_ENTAILMENT_THRESHOLD` | **`0.85`** | `nli-deberta-v3-base` ONNX | Minimum probability required for NLI `SUPPORTS` / `SUPERSEDES` classification. |
| `semantic_similarity_cutoff` | **`0.40`** | `MemorySettings.semantic_similarity_cutoff` | Cutoff floor for Turn Query RAG vector retrieval. |
| `top_k_facts` | **`5`** | `MemorySettings.top_k_facts` | **Turn Query RAG retrieval limit per semantic collection.** |
| `max_hops` | **`2`** | `MemorySettings.max_hops` | Maximum graph traversal expansion depth during Seed-and-Expand. |

---

## 5. Master 4-Stage Ingestion Pipeline Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                           4-Stage Modular Ingestion Pipeline                                │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Stage 1: O(1) String & Jaccard Exact Deduplication (Batch Ceiling = 128)                    │
│          Exact string match OR Jaccard == 1.0. Set old fact status = 'inactive'; advance new.  │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Stage 2: Dense Vector Embedding & Soft Vector Deduplication (Batch Size = 16)               │
│          Generate 384d vector via MiniLM-L12 INT8 ONNX. Query existing same-collection facts.│
│          If Cosine >= 0.95, mark old fact status = 'inactive', advance new fact with vector. │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Stage 3: Unified Edge & State Evaluation (Batch Size = 16 Facts / 16-32 Pairs)              │
│          Runs 2 concurrent model workers via tokio::join!:                                  │
│          • Sub-Branch A Model Worker (DeBERTa-v3 ONNX): Intra-Domain NLI supersessions.      │
│          • Sub-Branch B Model Worker (ModernBERT ONNX): Inter-Domain cross-domain edges.     │
│          Aggregates results in memory and executes a single atomic update (`status='evaluated'`).│
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Stage 4: Atomic Persistence & Queue Pruning (Batch Size = 32)                               │
│          Writes active facts to `memory_facts` and graph edges to `memory_relations`.       │
│          Executes `DELETE FROM personal_memory_queue WHERE status IN ('evaluated', 'superseded')`.│
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. System Behavioral Invariants (Preserved Invariants)

1. **2 Concurrent Model Workers in Stage 3**: Sub-Branch A (NLI Worker) and Sub-Branch B (Edge Classifier Worker) MUST run concurrently via `tokio::join!`.
2. **4-Stage Pipeline Structure**: The ingestion pipeline MUST consist of exactly 4 stages (Dedup $\rightarrow$ Embedding $\rightarrow$ Unified Evaluation $\rightarrow$ Commit & Prune).
3. **Unified Write Handler**: Stage 3 MUST aggregate Intra-Domain NLI and Inter-Domain Edge Classifier outputs in Rust memory and perform a single atomic SQL write per fact.
4. **All Active Identity Fetch**: All active `Identity` facts MUST be retrieved without arbitrary token limits.
5. **First-Turn Directives Rule**: `Directives` MUST act as top-level parent seeds ONLY on Turn 1 of a session.
6. **Vector-Indexed Constraints Rule**: `Constraints` MUST be retrieved via Semantic Vector Search and Graph Traversal, NOT direct SQL recency dumps.
