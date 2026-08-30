# Architecture Spec: DB-Backed Pipelined Memory Orchestration Subsystem

**Spec Type**: Architecture Spec  
**Governing Domain**: Asynchronous Memory Transformation, DB-Driven Pipeline, & Parallel Domain Orchestration  
**Version**: 7.0 (Production Master Specification)  

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
       │   (Phase 1 Exact String / Jaccard = 1.0 Dedup against active facts)
       │
       ├─► [Stage 2: Embedding Worker (Batch Size 16)]
       │   UPDATE status = 'processing_embed' ──► UPDATE status = 'embedded', vector = BLOB
       │   (Phase 2 MiniLM-L12 Embedding + Soft Vector Dedup at cos >= 0.95 ──► 'superseded')
       │
       ├─► [Stage 3: Unified Edge & State Evaluation Stage (Batch Size 16 Facts / 16-32 Pairs)]
       │   UPDATE status = 'processing_eval'
       │   │
       │   ├── Sub-Branch A Model Worker: NLI Engine (DeBERTa-v3 ONNX)
       │   │   - Evaluates intra-domain entailment & supersessions ('SUPPORTS', 'SUPERSEDES', 'CONFLICTS')
       │   │   - Writes forward AND inverse edges atomically
       │   │
       │   ├── Sub-Branch B Model Worker: Edge Classifier Engine (ModernBERT INT8 ONNX)
       │   │   - Classifies 7 cross-domain graph edge pairs ('SHAPES', 'DEPENDS_ON', 'CONFLICTS_WITH')
       │   │   - Writes forward AND inverse edges atomically
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

## Detailed Pipeline Stage Specifications

### Stage 1: Exact String / Jaccard Deduplication Worker (Batch Ceiling 128)
- **Input status**: `staged_pending`
- **Output status**: `deduped` (unique) or `superseded` (exact duplicate)
- **Execution**: Claims up to 128 `staged_pending` queue items (`UPDATE ... SET status = 'processing_dedup', claimed_at = ?`).
- **Algorithm**: For each claimed item, computes Jaccard word-set overlap (`JACCARD_EXACT_MATCH_THRESHOLD = 1.0`) against active facts in `memory_facts` within the same collection.
- **Result**: If an exact duplicate exists, marks the item `superseded`. Otherwise, advances to `deduped`.

### Stage 2: Embedding & Soft Vector Deduplication Worker (Batch Size 16)
- **Input status**: `deduped`
- **Output status**: `embedded` (unique) or `superseded` (soft vector duplicate)
- **Execution**: Claims up to 16 `deduped` queue items (`UPDATE ... SET status = 'processing_embed', claimed_at = ?`).
- **Algorithm**:
  1. Computes 384d MiniLM-L12 dense float vector and encodes it as a `F32_BLOB`.
  2. Runs Phase 2 **Soft Vector Deduplication** (`soft_vector_dedup_threshold = 0.95`) by comparing the vector against active facts in `memory_facts` within the same collection using Turso `vector_distance_cos`.
- **Result**: If cosine similarity $\ge 0.95$, marks item status `superseded` with `relations_json` containing a `SUPERSEDES` edge to the matching fact. Otherwise, stores vector BLOB and advances to `embedded`.

### Stage 3: Unified Edge & State Evaluation Worker (Batch Size 16)
- **Input status**: `embedded`
- **Output status**: `evaluated` or `superseded`
- **Execution**: Claims up to 16 `embedded` queue items (`UPDATE ... SET status = 'processing_eval', claimed_at = ?`).
- **Candidate Resolution Union**: Candidates are fetched by unioning persistent DB active facts from `memory_facts` WITH in-flight queue items in the current batch (`items` in `personal_memory_queue`). This guarantees intra-batch NLI and edge evaluation operates seamlessly on cold starts.
- **Concurrent Sub-Branches**:
  - **Sub-Branch A (DeBERTa-v3 NLI Engine)**: Evaluates intra-domain candidate pairs ($\text{cos} \ge 0.40$) for stateful domains (`Identity`, `Directives`, `Constraints`).
    - `Identity` & `Directives`: `ENTAILMENT` ($\ge 0.85$) $\rightarrow$ `SUPPORTS` edge; `CONTRADICTION` ($\ge 0.85$) $\rightarrow$ `SUPERSEDES` edge.
    - `Constraints`: `ENTAILMENT` ($\ge 0.85$) $\rightarrow$ `SUPPORTS` edge; `CONTRADICTION` ($\ge 0.85$) $\rightarrow$ `CONFLICTS` edge.
    - Atomically generates **both forward and inverse edges** (`SUPPORTS`/`supported_by`, `SUPERSEDES`/`superseded_by`, `CONFLICTS`/`conflicts_with`).
  - **Sub-Branch B (ModernBERT Edge Classifier Engine)**: Evaluates inter-domain candidate pairs across the 7 collection pairs defined in v7 spec §4.2 (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE`) with confidence threshold $\ge 0.80$.
    - Atomically generates **both forward and inverse edges** (`SHAPES`/`shaped_by`, `DEPENDS_ON`/`dependency_of`, `CONFLICTS_WITH`/`conflicts_with`).
- **Execution Pattern**: Dispatched concurrently using Rust `tokio::join!(spawn_blocking(SubBranchA), spawn_blocking(SubBranchB))`.
- **Result**: Aggregates relations into `relations_json` and updates status to `evaluated` (or `superseded` if an incoming `SUPERSEDES` edge targets this item).

### Stage 4: Commit & Prune Worker (Batch Size 32)
- **Input status**: `evaluated` or `superseded`
- **Output status**: Deleted from queue; active/superseded in `memory_facts` and relations in `memory_relations`.
- **Execution**: Claims up to 32 `evaluated` or `superseded` items (`UPDATE ... SET status = 'processing_commit', claimed_at = ?`).
- **Transaction**: Inside a single atomic Turso transaction (`BEGIN TRANSACTION` ... `COMMIT`):
  1. Inserts record into `memory_facts` (`status = 'active'` for `evaluated` items, `status = 'superseded'` for `superseded` items).
  2. Inserts vector into `memory_facts_vectors` (if vector present).
  3. Inserts forward and inverse edges into `memory_relations` (`INSERT OR IGNORE`) for all relations in `relations_json` (including Stage 2 soft-vector `SUPERSEDES` edges).
  4. For any `SUPERSEDES` relation edge, updates target fact `status = 'inactive'` in `memory_facts`.
  5. Deletes processed rows from `personal_memory_queue` (`DELETE FROM personal_memory_queue WHERE id = ?`).

---

## Batch Size Rationale & Execution Sweet Spot

| Pipeline Stage | Max Batch Size | Primary Workload | Rationale & Performance Impact |
| :--- | :---: | :--- | :--- |
| **Stage 1: Dedup** | **128 Facts** | Token set $O(N \cdot \|T\|)$ string comparison | In-memory code-only comparison; high batch size maximizes SQL throughput. |
| **Stage 2: Embedding** | **16 Facts** | Batched MiniLM-L12 ONNX 384d tensor + soft dedup | 16 facts $\times$ 384d tensor forward pass = ~29ms on CPU. Ideal ONNX throughput. |
| **Stage 3: Unified Eval** | **16 Facts (~16–32 Pairs)** | Batched DeBERTa + ModernBERT ONNX forward pass | 16 facts yield ~16–32 threshold-filtered candidate pairs. ONNX tensor batching evaluates all pairs in ~65ms, comfortably below the <50ms audio preemption SLA. |
| **Stage 4: Commit & Prune** | **32 Records** | Turso SQL `INSERT` + `DELETE` transaction | Groups database transactions to minimize WAL write lock overhead. |

---

## Data Taxonomy & Ephemeral Staging Queue Design

### 1. Ephemeral Staging Queue vs. Permanent Memory Store
- **`personal_memory_queue` is an ephemeral staging queue**, NOT a permanent database table.
- Intermediate columns (`vector`, `relations_json`, `retry_count`, `claimed_at`) exist solely for **idempotent crash recovery**: if Stage 3 panics mid-pass, Stage 2's computed 384d embedding is preserved on disk, preventing wasted ONNX re-computations upon restart.
- **Queue Pruning**: Stage 4 writes active facts to `memory_facts` and graph edges to `memory_relations`, then immediately **deletes** completed/superseded rows (`DELETE FROM personal_memory_queue WHERE status IN ('evaluated', 'superseded')`). The queue table stays lightweight (< 100 active rows).

### 2. Consolidated Queue Status Lifecycle (`personal_memory_queue.status`)

| Status Value | Meaning / Stage State | Processing Worker / Owner | Transition Outputs / Behavior |
| :--- | :--- | :--- | :--- |
| `staged_pending` | Initial compaction LLM extraction enqueued. | Enqueued by Compaction LLM. | Initial fact text and session context. |
| `processing_dedup` | Claimed by Stage 1 for Phase 1 String/Jaccard dedup. | Stage 1 Dedup Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `deduped` | Phase 1 exact string deduplication completed. | Stage 1 output state. | Unique facts advance to `deduped`. |
| `processing_embed` | Claimed by Stage 2 for MiniLM-L12 dense vector embedding. | Stage 2 Embedding Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `embedded` | 384d vector generated and soft vector dedup verified. | Stage 2 output state. | Dense float vector saved to `vector` BLOB. |
| `processing_eval` | Claimed by Stage 3 for Unified Edge & State Evaluation. | Stage 3 Evaluation Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `evaluated` | NLI state resolution and ModernBERT cognitive edges generated. | Stage 3 output state. | Graph edges saved to `relations_json`. |
| `processing_commit`| Claimed by Stage 4 for atomic persistence. | Stage 4 Commit Worker. | Sets `claimed_at = CURRENT_TIMESTAMP`. |
| `completed` | Active in `memory_facts`, pruned from queue. | Stage 4 Commit Worker. | Deleted from `personal_memory_queue`. |
| `superseded` | Fact deactivated by exact duplicate, soft vector dedup, or NLI supersession. | Terminal inactive state. | Pruned by Stage 4. |
| `failed` | Processing failed after maximum retry attempts (`retry_count >= 3`). | Terminal error state. | Quarantined for developer inspection. |

---

## Retry & Error Recovery Behavior (`mark_job_failed`)

When any pipeline stage worker encounters a failure during item processing:
1. Calls `mark_job_failed(conn, queue_id, error_message)`:
   ```sql
   UPDATE personal_memory_queue
   SET retry_count = retry_count + 1,
       error_msg = ?,
       status = CASE WHEN retry_count + 1 >= 3 THEN 'failed' ELSE status END
   WHERE id = ?
   ```
2. If `retry_count >= 3`, status moves to `'failed'` (quarantined).
3. If `retry_count < 3`, status is reset back to its pre-processing state (`staged_pending`, `deduped`, `embedded`) to allow retry on the next worker sweep cycle.

---

## System Behavioral Invariants (Must Be True)

1. **2 Concurrent Model Workers in Stage 3**: Sub-Branch A (NLI Worker) and Sub-Branch B (Edge Classifier Worker) MUST run concurrently via `tokio::join!`.
2. **Unified Write Handler**: Stage 3 MUST aggregate outputs in Rust memory and perform a single atomic SQL write per fact.
3. **Pure Threshold Candidate Selection**: Candidate selection for Stage 3 MUST be purely threshold-based (`cos >= cutoff`) without $K$-capping.
4. **4-Stage Pipeline Structure**: The ingestion pipeline MUST consist of exactly 4 stages (Dedup $\rightarrow$ Embedding + Soft Dedup $\rightarrow$ Unified Evaluation $\rightarrow$ Commit & Prune).
5. **Dual Forward & Inverse Edge Persistence**: Both Stage 3 sub-branches MUST output both forward and inverse edges for every valid relation into `relations_json`.
