# Architecture Spec: DB-Backed Pipelined Memory Orchestration Subsystem

**Spec Type**: Architecture Spec  
**Governing Domain**: Asynchronous Memory Transformation, DB-Driven Pipeline, & Parallel Domain Orchestration  
**Version**: 5.2 (Production Verified Specification)  

---

## Name & Concept

A database-backed, status-driven batch pipeline architecture governing the asynchronous lifecycle transformation of raw extracted facts into active, graph-linked memory records via atomic state transitions in Turso (`personal_memory_queue`).

---

## Purpose

To provide a fully asynchronous, database-driven queue pipeline that processes memory ingestion during idle periods (`PipelineIdle`). By storing explicit intermediate stage statuses and temporary payload blobs in `personal_memory_queue` (an ephemeral staging queue), different stages execute in parallel across successive batches without holding unpersisted in-memory state. Stage 3 (NLI) and Stage 4 (ModernBERT Edge Classifier) run in **parallel domain-specific evaluation branches** fed by Stage 2 (`embedded`), ensuring zero audio hot-path latency impact (< 200ms perceived voice latency) and instant preemption (< 50ms SLA) upon live user speech detection.

---

## Database State Machine & Parallel Domain Branching

```
                     Compaction Output (~100 Facts)
                                   │
                                   ▼ (Single SQL INSERT Transaction)
┌──────────────────────────────────────────────────────────────────────────────────┐
│ personal_memory_queue (Turso DB - Ephemeral Staging Queue)                       │
├──────────────────────────────────────────────────────────────────────────────────┤
│ status: 'staged_pending'                                                         │
└──────┬───────────────────────────────────────────────────────────────────────────┘
       │
       ├─► [Stage 1: Dedup Worker (Batch Ceiling 128)] ──► UPDATE status = 'deduped' (or 'superseded')
       │
       ├─► [Stage 2: Embedding Worker]               ──► UPDATE status = 'embedded', vector = BLOB
       │                                                 │
       │                        ┌────────────────────────┴────────────────────────┐
       │                        ▼ (Domain Route)                                  ▼ (Domain Route)
       │           [Domains: Identity, Directives, Constraints]         [Domains: Profile, Entities, Narrative, Fallback]
       │                        │                                                 │
       ├─► [Stage 3: Vector Candidate Search + NLI Worker]  ├─► [Stage 4: Vector Candidate Search + ModernBERT Edge Classifier]
       │   UPDATE status = 'processing_nli'                    │   UPDATE status = 'processing_llm'
       │   UPDATE status = 'evaluated', relations = JSON       │   UPDATE status = 'evaluated', relations = JSON
       │   (or 'superseded')                                   │   (or 'superseded')
       │                        └────────────────────────┬────────────────────────┘
       │                                                 │
       └─► [Stage 5: Commit Worker]                  ────┴──► INSERT memory_facts / memory_relations
                                                               DELETE completed/superseded queue rows
```

---

## Data Taxonomy & Ephemeral Staging Queue Design

### 1. Ephemeral Staging Queue vs. Permanent Memory Store
- **`personal_memory_queue` is NOT a permanent "god table"**. It is an **ephemeral staging queue** used exclusively during transformation and crash recovery.
- Intermediate columns (`vector`, `relations_json`, `claimed_at`) exist in the queue solely to ensure **idempotent crash recovery**: if Stage 3 or 4 crashes mid-pass, Stage 2's computed 384d embedding is already saved on disk, preventing wasted ONNX re-computations.
- **Queue Pruning**: Upon Stage 5 terminal commit, active records and vectors are written to permanent tables (`memory_facts`, `memory_facts_vectors`, `memory_relations`), and completed/superseded rows are **pruned/deleted** via `DELETE FROM personal_memory_queue WHERE status IN ('evaluated', 'superseded')`. The queue table remains lightweight (< 100 active rows).

### 2. Expanded Queue Status Lifecycle (`personal_memory_queue.status`)

| Status Value | Meaning / Stage State | Processing Worker / Owner | Transition Outputs / Behavior |
| :--- | :--- | :--- | :--- |
| `staged_pending` | Initial compaction LLM extraction enqueued. | Enqueued by Compaction LLM. | Initial fact text and session context. |
| `processing_dedup` | Claimed by Stage 1 for Phase 1 String/Jaccard dedup. | Stage 1 Dedup Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `deduped` | Phase 1 exact string deduplication completed. | Stage 1 output state. | Unique facts advance to `deduped`. |
| `processing_embed` | Claimed by Stage 2 for MiniLM-L12 dense vector embedding. | Stage 2 Embedding Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `embedded` | 384d dense vector embedding generated and stored in column. | Stage 2 output state. | Dense float vector saved to `vector` BLOB. |
| `processing_nli` | Claimed by Stage 3 (NLI) for domain state evaluation. | Stage 3 NLI Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `processing_llm` | Claimed by Stage 4 for ModernBERT Edge Classification. | Stage 4 Edge Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `evaluated` | NLI supersession or ModernBERT cognitive graph relations generated. | Stage 3 / Stage 4 output state.| Relation graph edges saved to `relations_json`. |
| `processing_commit`| Claimed by Stage 5 for atomic persistence. | Stage 5 Commit Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `completed` | Active in `memory_facts`, pruned from queue. | Stage 5 Commit Worker. | Deleted from `personal_memory_queue`. |
| `superseded` | Fact deactivated by exact duplicate or NLI supersession. | Terminal inactive state. | Pruned by Stage 5. |
| `failed` | Processing failed after maximum retry attempts (`retry_count >= 3`). | Terminal error state. | Quarantined for developer inspection. |

---

## Domain Routing & Candidate Retrieval Mechanics

### 1. Threshold-Based Ingestion Candidate Selection
In both Stage 3 and Stage 4, candidate selection is **PURELY THRESHOLD-BASED** (`cos >= cutoff`). Workers retrieve **ALL matching candidate facts** meeting or exceeding the similarity threshold across active facts and uncommitted same-session queue facts (`vector IS NOT NULL`). Candidate selection is **NEVER limited by $K$ / `top_k_facts`**.

### 2. Parallel Domain Evaluation Branches (Stage 3 & Stage 4)
Stage 3 (NLI) and Stage 4 (ModernBERT Edge Classifier) both depend ONLY on Stage 2 (`embedded`). They run **in parallel on separate domain branches**:

- **Stage 3 (NLI Branch)**: Processes facts where `domain IN ('Identity', 'Directives', 'Constraints')`.
  1. **Threshold Candidate Search**: Performs ANN vector cosine search using the fact's 384d `vector` BLOB (`cos >= 0.40`).
  2. **DeBERTa-v3 NLI Pass**: Evaluates Candidate Pairs for entailment ($\ge 0.85$), contradiction (supersession $\rightarrow$ `superseded`), or neutral.
  3. Updates row to `status = 'evaluated'` (or `superseded`).

- **Stage 4 (Edge Classifier Branch)**: Processes facts where `domain IN ('Profile', 'Entities', 'Narrative')` as well as fallback domains.
  1. **Threshold Candidate Search**: Queries ALL semantically relevant nodes matching the permitted domain pair threshold (`cos >= cutoff`).
  2. **ModernBERT 1-Pass ONNX Classifier**: Classifies edge labels (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE`) in a single forward pass (~35ms CPU).
  3. Updates row to `status = 'evaluated'` with `relations_json`.

---

## Batch Pipeline Execution & Worker Allocations

| Stage | Component / Role | Worker Allocation | Max Batch Size | Threading Rules | Primary Workload / Engine |
| :--- | :--- | :---: | :---: | :---: | :--- |
| **Stage 1** | String & Jaccard Dedup | 1 Worker | **128 Facts** | `nice(10)` | Fast in-memory token set comparison ($O(N \cdot \|T\|)$). |
| **Stage 2** | Dense Vector Embedding | 1 Model Worker | 16 Facts | `intra_op = 1`, `nice(10)` | Batched ONNX MiniLM-L12 384d tensor inference. |
| **Stage 3** | Candidate Search + NLI | 1 Model Worker | 16 Pairs | `intra_op = 1`, `nice(10)` | Vector ANN (Threshold) + Batched ONNX DeBERTa-v3 NLI pass. |
| **Stage 4** | Candidate Search + Edge | 1 Model Worker | 16 Pairs | `intra_op = 1`, `nice(10)` | Vector ANN (Threshold) + 1-Pass INT8 ONNX ModernBERT Classifier. |
| **Stage 5** | Terminal Commit & Prune | 1 DB Worker | 32 Records | Async DB Task | Turso SQL transaction + `DELETE` completed rows. |

---

## Crash Recovery, Lease Timeout, & Preemption

### 1. Live Audio Preemption (< 50ms Cooperative SLA)
Upon detecting live user voice activity (`PipelineActive`), the Preemption Controller MUST:
1. **ONNX Workers (Stage 2, 3, & 4)**: Invoke native ONNX C++ session termination handles (`Ort::RunOptions::SetTerminate()`).
2. **Non-Blocking Hot-Path Isolation**: Background workers catch cancellation exceptions locally, immediately abort their C++ inference passes within < 50ms, and allow the 60s Lease Timeout Sweeper to lazily reset DB statuses.

---

## System Behavioral Invariants (Must Be True)

1. **Pure Threshold Ingestion Candidate Selection**: Candidate selection for Stage 3 (NLI) and Stage 4 (Edge Classifier) MUST be purely threshold-based (`cos >= cutoff`) and MUST NOT be constrained by `top_k_facts`.
2. **First-Turn Directives Rule**: `Directives` MUST act as top-level parent seeds ONLY on Turn 1 of a session. On Turns 2+, they are reached strictly via graph traversal.
3. **Vector-Indexed Constraints Rule**: `Constraints` MUST be retrieved via Semantic Vector Search and Graph Traversal, NOT direct SQL recency dumps.
4. **1-Pass Sequence Classifier**: Stage 4 Edge Classifier MUST use a 1-pass sequence classifier (`ModernBERT-base` INT8 ONNX).
5. **Ephemeral Queue Pruning**: Completed and superseded items MUST be pruned/deleted from `personal_memory_queue` upon Stage 5 commit.
