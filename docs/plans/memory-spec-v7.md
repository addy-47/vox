# Vox v7 Cognitive Memory Subsystem Architecture Specification

**Status**: Frozen Master Architectural Specification  
**Version**: 7.5 (Validated Architecture, Precision Retrieval & 4-Stage Pipeline Specification)  
**Target Systems**: `app/src-tauri/src/services/memory/` (Rust Backend)  

---

## 1. Architectural Paradigm Shift & Core Principles

The v7 memory architecture provides a unified, deterministic, and domain-agnostic memory engine for real-time voice AI. It resolves critical scaling and domain-coupling flaws:

1. **Unbounded Deterministic Fetch Elimination**: `Identity` facts are fetched as bounded invariants (`WHERE status = 'active'`). All active identity facts are retrieved directly without arbitrary token truncation.
2. **Operational State & Agent Agenda (`Directives`)**: `Directives` represent agent tasks and operational goals. They act as top-level parent seeds **ONLY on Turn 1 of a new session**, preventing prompt pollution across ongoing turns.
3. **Integrated Constraint Search**: `Constraints` are removed from direct SQL recency dumps. They are indexed in **Semantic Vector Search** and pulled dynamically as child nodes via graph expansion edges (`RESTRICTS` / `CONFLICTS`).
4. **4-Stage Pipeline with Unified Evaluation**: Pipeline stages are consolidated into 4 clean stages (Dedup $\rightarrow$ Embedding $\rightarrow$ Unified Edge & State Evaluation $\rightarrow$ Commit & Prune). Stage 3 aggregates Intra-Domain NLI (DeBERTa-v3) and Inter-Domain Edge Classification (ModernBERT) in memory, executing a single atomic write to eliminate database lock contention and race conditions.

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

Memory ingestion operates asynchronously via a 4-stage database worker queue (`personal_memory_queue`):

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                           4-Stage Modular Ingestion Pipeline                                │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Stage 1: O(1) String & Jaccard Exact Deduplication (Batch Ceiling = 128)                    │
│          Exact string match OR Jaccard == 1.0. Set old fact status = 'inactive'; advance new.  │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Stage 2: Dense Vector Embedding & Soft Vector Deduplication                                 │
│          Generate 384d vector via MiniLM-L12 INT8 ONNX. Query existing same-collection facts.│
│          If Cosine >= 0.95, mark old fact status = 'inactive', advance new fact with vector. │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Stage 3: Unified Edge & State Evaluation (Intra-Domain NLI + Inter-Domain Edge Classifier) │
│          • Sub-Branch A (Intra-Domain NLI): DeBERTa-v3 ONNX evaluates NLI supersessions.   │
│          • Sub-Branch B (Inter-Domain Edge): ModernBERT ONNX classifies cross-domain edges.  │
│          Aggregates results in memory and executes a single atomic update (`status='evaluated'`).│
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Stage 4: Atomic Persistence & Queue Pruning (Turso MVCC Transaction)                        │
│          Writes active facts to `memory_facts` and graph edges to `memory_relations`.       │
│          Executes `DELETE FROM personal_memory_queue WHERE status IN ('evaluated', 'superseded')`.│
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Targeted NLI State Resolution Sub-Branch (Stage 3A)

NLI processing evaluates formal logical relationships strictly within stateful/invariant domains (`Identity`, `Directives`, `Constraints`).

1. **`Identity` & `Directives` Domains**:
   - Candidate facts selected via threshold filtering (`nli_candidate_search_cutoff = 0.40`).
   - **`ENTAILMENT` (>= 0.85)**: New fact refines/subsumes old fact. Writes `SUPERSEDES` edge to old fact. Old fact `status` set to `'superseded'`.
   - **`CONTRADICTION` (>= 0.85)**: New fact contradicts old fact. Writes `SUPERSEDES` edge; old fact `status` set to `'superseded'`.
   - **`NEUTRAL`**: Both facts remain active.

2. **`Constraints` Domain**:
   - **`ENTAILMENT` (>= 0.85)**: Writes `SUPPORTS` edge (`refined_by`). Both remain active.
   - **`CONTRADICTION` (>= 0.85)**: Writes `CONFLICTS` edge in `memory_relations`. **Neither constraint is deactivated**.

---

## 7. Inter-Domain Edge Classifier Sub-Branch (Stage 3B)

Cross-domain graph connections are generated using a 1-pass fine-tuned INT8 ONNX sequence classifier (`ModernBERT-base`).

| Edge Label | Semantic Meaning | Forward Edge Sign | Inverse Edge Label (Derived at Traversal) |
| :--- | :--- | :--- | :--- |
| **`SHAPES`** | Target Fact modifies or constrains how Source Fact is executed. | `A -> SHAPES -> B` | `shaped_by` (`B -> shaped_by -> A`) |
| **`DEPENDS_ON`** | Source Fact functionally requires Target Fact to exist first. | `A -> DEPENDS_ON -> B` | `required_by` (`B -> required_by -> A`) |
| **`CONFLICTS_WITH`** | Source Fact and Target Fact represent opposing goals or rules. | `A -> CONFLICTS_WITH -> B` | `conflicts_with` (`B -> conflicts_with -> A`) |
| **`NONE`** | No causal, dependency, or conflict relationship exists. | N/A | No edge created in `memory_relations`. |

---

## 8. Precision Turn Context Retrieval Subsystem

```
                                USER TURN QUERY
                                       │
                   ┌───────────────────┼───────────────────┐
                   ▼                   ▼                   ▼
           [Class A: Identity]   [Class B: Directives]   [Class C: Vector Search]
           (All Active Facts)    (Turn 1 ONLY / Recency) (Profile, Entities,
                   │                   │                  Constraints, Skills)
                   │                   │                   │
                   └───────────────────┼───────────────────┘
                                       ▼
                              [GLOBAL SEED POOL]
                                       │
                                       ▼
                       [Bi-Directional Graph Expansion]
                       (BFS max_hops = 2 via memory_relations)
                                       │
                                       ▼
                    [Dynamic Fair-Share Token Budgeting]
                    (Render XML <user_profile> Prompt)
```

---

## 9. Database Schema & Table Ownership Matrix

| Table | Column | Valid Values | Writing Component / Owner | Lifecycle & Rules |
| :--- | :--- | :--- | :--- | :--- |
| `personal_memory_queue` | `status` | `'staged_pending'`, `'processing_*'`, `'deduped'`, `'embedded'`, `'evaluated'`, `'completed'`, `'superseded'`, `'failed'` | `orchestrator.rs` (Ingestion Worker) | Ephemeral queue lifecycle. Completed and superseded items deleted by Stage 4. |
| `memory_facts` | `status` | `'active'`, `'inactive'`, `'deleted'` | `mutations.rs` (Memory System) | `'active'`: Live fact for RAG. `'inactive'`: Deactivated by dedup/NLI. `'deleted'`: Soft-deleted by user. |
| `memory_relations` | `relation` | `'SHAPES'`, `'DEPENDS_ON'`, `'CONFLICTS_WITH'`, `'SUPERSEDES'`, `'SUPPORTS'` | `orchestrator.rs` / `mutations.rs` | Directed graph relation label. |

---

## 10. System Behavioral Invariants (Preserved Invariants)

1. **4-Stage Pipeline Structure**: The ingestion pipeline MUST consist of exactly 4 stages (Dedup $\rightarrow$ Embedding $\rightarrow$ Unified Evaluation $\rightarrow$ Commit & Prune).
2. **Unified Write Handler**: Stage 3 MUST aggregate Intra-Domain NLI and Inter-Domain Edge Classifier outputs in Rust memory and perform a single atomic SQL write per fact.
3. **All Active Identity Fetch**: All active `Identity` facts MUST be retrieved without arbitrary token limits.
4. **First-Turn Directives Rule**: `Directives` MUST act as top-level parent seeds ONLY on Turn 1 of a session.
5. **Vector-Indexed Constraints Rule**: `Constraints` MUST be retrieved via Semantic Vector Search and Graph Traversal, NOT direct SQL recency dumps.
