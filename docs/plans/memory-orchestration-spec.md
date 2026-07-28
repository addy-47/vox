# Architecture Spec: DB-Backed Pipelined Memory Orchestration Subsystem

**Spec Type**: Architecture Spec  
**Governing Domain**: Asynchronous Memory Transformation, DB-Driven Pipeline, & Parallel Domain Orchestration  
**Version**: 6.1 (Production Verified Specification)  

---

## Name & Concept

A database-backed, status-driven 4-stage batch pipeline architecture governing the asynchronous lifecycle transformation of raw extracted facts into active, graph-linked memory records via atomic state transitions in Turso (`personal_memory_queue`).

---

## Purpose & Architectural Validation

Resolves race conditions, double-write lock contention, and domain overlap flaws present in split-worker architectures. Instead of partitioning domain routing into separate database workers, the pipeline consolidates evaluation into a **4-Stage Pipeline with a Unified Edge & State Evaluation Stage (Stage 3)**.

Inside Stage 3, **2 dedicated model workers** (1 for Sub-Branch A: NLI and 1 for Sub-Branch B: ModernBERT Edge Classifier) execute concurrently via Rust `tokio::join!`. Their outputs are merged into an in-memory `BatchEvaluationResult` and committed to `personal_memory_queue` in a single atomic SQL transaction (`status = 'evaluated'`, `relations_json = [...]`).

---

## Database State Machine & 4-Stage Pipeline Diagram

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
       ├─► [Stage 1: Dedup Worker (Batch Ceiling 128)]
       │   UPDATE status = 'processing_dedup' ──► UPDATE status = 'deduped' (or 'superseded')
       │
       ├─► [Stage 2: Embedding Worker (Batch Size 16)]
       │   UPDATE status = 'processing_embed' ──► UPDATE status = 'embedded', vector = BLOB
       │
       ├─► [Stage 3: Unified Edge & State Evaluation Stage (Batch Size 16 Facts / 16-32 Pairs)]
       │   UPDATE status = 'processing_eval'
       │   │
       │   ├── Sub-Branch A Model Worker: NLI Engine (DeBERTa-v3 ONNX)
       │   │   - Evaluates intra-domain entailment & supersessions ('SUPERSEDES', 'CONFLICTS')
       │   │
       │   ├── Sub-Branch B Model Worker: Edge Classifier Engine (ModernBERT INT8 ONNX)
       │   │   - Classifies cross-domain graph edges ('SHAPES', 'DEPENDS_ON', 'CONFLICTS_WITH')
       │   │   (Executes concurrently via tokio::join!)
       │   │
       │   └── In-Memory Result Aggregator & Single DB Update
       │       UPDATE status = 'evaluated', relations_json = JSON (or status = 'superseded')
       │
       └─► [Stage 4: Commit Worker (Batch Size 32)]
           UPDATE status = 'processing_commit' ──► INSERT memory_facts / memory_relations
                                                   DELETE FROM personal_memory_queue WHERE status IN ('evaluated', 'superseded')
```

---

## Stage 3 Concurrent Sub-Branches & Model Worker Allocations

### 1. Dedicated Model Workers per Sub-Branch
Inside Stage 3, evaluation is dispatched concurrently across **2 dedicated ONNX model workers**:

- **Sub-Branch A Model Worker (DeBERTa-v3 NLI Engine)**:
  - Holds `nli-deberta-v3-base` ONNX session in RAM (~233MB).
  - Evaluates candidate pairs in stateful domains (`Identity`, `Directives`, `Constraints`).
- **Sub-Branch B Model Worker (ModernBERT Edge Classifier Engine)**:
  - Holds `ModernBERT-base` INT8 ONNX session in RAM (~120MB).
  - Evaluates cross-domain candidate pairs across all collections.
- **Concurrent Rust Execution (`tokio::join!`)**:
  ```rust
  let (nli_results, edge_results) = tokio::join!(
      eval_subbranch_a_nli(&nli_worker, &candidate_pairs),
      eval_subbranch_b_edges(&edge_worker, &candidate_pairs)
  );
  ```
- **Latency Optimization**: Sub-Branch A (~65ms) and Sub-Branch B (~35ms) run concurrently on CPU, completing Stage 3 batch evaluation in $\max(65\text{ms}, 35\text{ms}) = \mathbf{\sim 65\text{ms total batch latency}}$.

---

## Batch Size Rationale & Execution Sweet Spot

| Pipeline Stage | Max Batch Size | Primary Workload | Rationale & Performance Impact |
| :--- | :---: | :--- | :--- |
| **Stage 1: Dedup** | **128 Facts** | Token set $O(N \cdot \|T\|)$ string comparison | In-memory code-only comparison; high batch size maximizes SQL throughput. |
| **Stage 2: Embedding** | **16 Facts** | Batched MiniLM-L12 ONNX 384d tensor | 16 facts $\times$ 384d tensor forward pass = ~29ms on CPU. Ideal ONNX throughput. |
| **Stage 3: Unified Eval** | **16 Facts (~16–32 Pairs)** | Batched DeBERTa + ModernBERT ONNX forward pass | 16 facts yield ~16–32 threshold-filtered candidate pairs. ONNX tensor batching evaluates all pairs in ~65ms, comfortably below the <50ms audio preemption SLA. |
| **Stage 4: Commit & Prune** | **32 Records** | Turso SQL `INSERT` + `DELETE` transaction | Groups database transactions to minimize WAL write lock overhead. |

---

## Data Taxonomy & Ephemeral Staging Queue Design

### 1. Ephemeral Staging Queue vs. Permanent Memory Store
- **`personal_memory_queue` is an ephemeral staging queue**, NOT a permanent database table.
- Intermediate columns (`vector`, `relations_json`, `claimed_at`) exist solely for **idempotent crash recovery**: if Stage 3 panics mid-pass, Stage 2's computed 384d embedding is preserved on disk, preventing wasted ONNX re-computations upon restart.
- **Queue Pruning**: Stage 4 writes active facts to `memory_facts` and graph edges to `memory_relations`, then immediately **deletes** completed/superseded rows (`DELETE FROM personal_memory_queue WHERE status IN ('evaluated', 'superseded')`). The queue table stays lightweight (< 100 active rows).

### 2. Consolidated Queue Status Lifecycle (`personal_memory_queue.status`)

| Status Value | Meaning / Stage State | Processing Worker / Owner | Transition Outputs / Behavior |
| :--- | :--- | :--- | :--- |
| `staged_pending` | Initial compaction LLM extraction enqueued. | Enqueued by Compaction LLM. | Initial fact text and session context. |
| `processing_dedup` | Claimed by Stage 1 for Phase 1 String/Jaccard dedup. | Stage 1 Dedup Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `deduped` | Phase 1 exact string deduplication completed. | Stage 1 output state. | Unique facts advance to `deduped`. |
| `processing_embed` | Claimed by Stage 2 for MiniLM-L12 dense vector embedding. | Stage 2 Embedding Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `embedded` | 384d dense vector embedding generated and stored in column. | Stage 2 output state. | Dense float vector saved to `vector` BLOB. |
| `processing_eval` | Claimed by Stage 3 for Unified Edge & State Evaluation. | Stage 3 Evaluation Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `evaluated` | NLI supersession and ModernBERT cognitive edges generated. | Stage 3 output state. | Graph edges saved to `relations_json`. |
| `processing_commit`| Claimed by Stage 4 for atomic persistence. | Stage 4 Commit Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `completed` | Active in `memory_facts`, pruned from queue. | Stage 4 Commit Worker. | Deleted from `personal_memory_queue`. |
| `superseded` | Fact deactivated by exact duplicate or NLI supersession. | Terminal inactive state. | Pruned by Stage 4. |
| `failed` | Processing failed after maximum retry attempts (`retry_count >= 3`). | Terminal error state. | Quarantined for developer inspection. |

---

## System Behavioral Invariants (Must Be True)

1. **2 Concurrent Model Workers in Stage 3**: Sub-Branch A (NLI Worker) and Sub-Branch B (Edge Classifier Worker) MUST run concurrently via `tokio::join!`.
2. **Unified Write Handler**: Stage 3 MUST aggregate outputs in Rust memory and perform a single atomic SQL write per fact.
3. **Pure Threshold Candidate Selection**: Candidate selection for Stage 3 MUST be purely threshold-based (`cos >= cutoff`) without $K$-capping.
4. **4-Stage Pipeline Structure**: The ingestion pipeline MUST consist of exactly 4 stages (Dedup $\rightarrow$ Embedding $\rightarrow$ Unified Evaluation $\rightarrow$ Commit & Prune).
