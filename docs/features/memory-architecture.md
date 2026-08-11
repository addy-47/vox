# Vox v7 Memory Architecture — Current Implementation

**Last Updated:** 2026-08-01  
**Scope:** End-to-end description of what the code currently does.  
**Location:** `app/src-tauri/src/services/memory/` and `app/src-tauri/src/persistence/`

---

## 1. Overview

The Vox v7 memory subsystem is a database-backed, 4-stage pipeline that transforms raw LLM-extracted facts into active, graph-linked memory records in Turso (SQLite). A pre-retrieval scope classifier (`query-sieve-rs`) prunes irrelevant collections before vector search, and a 4-step dynamic waterfall budgets token usage at retrieval time. A full-screen interactive 2D Force-Directed Cognitive Memory Graph (`Memory.tsx`) visualizes 1,100+ memory nodes, 250+ inter-fact graph edges, and real-time background pipeline ingestion monitoring.

---

## 2. Fact Generation — LLM Compaction (`ingestion.rs`)

When `ConversationManager` context utilization crosses the critical threshold (default `0.85`), compaction is triggered via `run_compaction()`.

**Process:**
1. Sends conversation history to the active LLM provider using `COMPACTION_SYSTEM_PROMPT`.
2. The prompt instructs the LLM to extract facts into exactly 6 collections: `Identity`, `Directives`, `Narrative`, `Profile`, `Entities`, `Constraints`.
3. The LLM responds with a JSON object. `parse_compaction_json()` extracts a `HashMap<String, Vec<String>>` (except `Narrative` which is a single string).
4. Retried up to **2 attempts** on parse failure.

**Output:** `CompactionResult { context_summary, personal_memory, diff_to_enqueue }` where `diff_to_enqueue == personal_memory`.

**Next step:** `enqueue_personal_facts()` inserts each fact as a `staged_pending` row in `personal_memory_queue`. No pre-insertion dedup occurs.

---

## 3. 4-Stage Pipeline (`pipeline/`)

The pipeline runs sequentially: Dedup → Embedding → Evaluation → Commit & Prune. Each stage claims items atomically via TOCTOU-safe `UPDATE WHERE status = ?` queries.

### 3.1 Stage 1 — Dedup (`stage1_dedup.rs`)

| Parameter | Value |
|---|---|
| Batch ceiling | 128 items |
| Input status | `staged_pending` |
| Output statuses | `deduped` or `superseded` |

**Logic:**
1. SELECT up to 128 `staged_pending` rows, ordered by `created_at ASC`.
2. Atomically claim each with `UPDATE ... WHERE status = 'staged_pending'`.
3. Empty-fact items → immediately `superseded`.
4. For each remaining item: prefetch active facts across the 5 core factual collections (`Identity: 6 > Constraints: 5 > Directives: 4 > Profile: 3 > Entities: 2`) from `memory_facts` AND in-flight queue items.
5. Compute **Jaccard word-set similarity** (`JACCARD_EXACT_MATCH_THRESHOLD = 1.0`) against candidate facts across all 5 factual collections.
6. **5-Collection Priority Resolution:** On exact Jaccard match:
   - If `incoming_priority <= matched_priority`: Incoming item marked `superseded` (`duplicate_dropped`).
   - If `incoming_priority > matched_priority`: Existing lower-priority DB fact marked `superseded`, incoming item proceeds as `deduped`.

**Constants:**
- `STAGE1_BATCH_CEILING = 128`
- `JACCARD_EXACT_MATCH_THRESHOLD = 1.0` (in `deduplication.rs`)
- `COSINE_HARD_MATCH_THRESHOLD = 0.98` (in `deduplication.rs`, used by `is_exact_duplicate` but not in Stage 1)

### 3.2 Stage 2 — Embedding & Soft Vector Dedup (`stage2_embed.rs`)

| Parameter | Value |
|---|---|
| Batch size | 16 items |
| Input status | `deduped` |
| Output statuses | `embedded`, `superseded`, or `failed` |

**Logic:**
1. SELECT up to 16 `deduped` rows, claim atomically.
2. Load MiniLM-L12 ONNX embedder if not already loaded.
3. For each item, generate a 384-dim float vector via `generate_embedding()`.
4. **Phase 2 Cross-Collection Soft Vector Dedup:** Query candidate facts (cos ≥ `SOFT_VECTOR_DEDUP_THRESHOLD` 0.95) across the 5 core factual collections in `memory_facts` AND in-flight queue items.
5. **Priority Resolution:**
   - If `incoming_priority <= matched_priority`: Incoming item marked `superseded`, pushing a `SUPERSEDES` edge from `match_id` (surviving fact) → `item_X` (dropped fact).
   - If `incoming_priority > matched_priority`: Existing lower-priority DB fact marked `superseded`, incoming item proceeds to `embedded`.
6. On embedding failure → call `mark_job_failed()` (increments `retry_count`, resets to `staged_pending`; transitions to `failed` after 3 attempts).

**Constants:**
- `STAGE2_BATCH_SIZE = 16`
- `SOFT_VECTOR_DEDUP_THRESHOLD = 0.95`
- `EMBEDDING_DIM = 384`
- `PRIMARY_MODEL_DIR = "minilm-l12-v2"`
- `PRIMARY_MODEL_FILENAME = "model_int8.onnx"`


### 3.3 Stage 3 — Unified Edge & State Evaluation (`stage3_eval.rs`)

| Parameter | Value |
|---|---|
| Batch size | 16 items |
| Input status | `embedded` |
| Output statuses | `evaluated` or `superseded` |

**Logic:**
1. SELECT up to 16 `embedded` rows, claim atomically.
2. For each item, fetch two candidate sets concurrently:
   - **Sub-Branch A (NLI):** Intra-collection candidates with cosine ≥ `SAME_COLLECTION_CANDIDATE_SEARCH` (0.40), no K-cap. Only for items in NLI domains: `Identity`, `Directives`, `Constraints`.
   - **Sub-Branch B (Edge Classifier):** Inter-collection candidates with cosine ≥ `INTER_COLLECTION_CANDIDATE_SEARCH` (0.55), no K-cap. Only for pairs where `inter_collection_edge()` returns a valid policy pair.
   - **Candidate Resolution Union:** Candidates are fetched by unioning persistent DB active facts from `memory_facts` WITH in-flight queue items in the current batch (`items` in `personal_memory_queue`). This guarantees intra-batch NLI and edge evaluation operates seamlessly on cold starts.

> [!NOTE]
> **Future Consideration — Candidate Retrieval Trade-offs & Hybrid Search:**
> Pure vector cosine search captures direct topic contradictions well (high cosine similarity), but can face candidate window saturation ("crowding") or vocabulary drift across disjoint contexts. Future hybrid options under evaluation:
> 1. *Entity/Topic Keying:* Filtering candidates via explicit category tags (e.g. `[Diet]`, `[Location]`) alongside vector distance.
> 2. *Graph-Assisted Candidate Traversal:* 1-hop graph neighbor expansion from top vector seed nodes.

3. Both sub-branches run concurrently via `tokio::task::spawn_blocking` + `tokio::join!`.
4. Results merged into a single `BatchEvaluationResult` and written as one atomic `UPDATE`.

**Sub-Branch A — NLI State Resolution (`nli.rs`):**

| Condition | Edge Produced | Effect on Old Fact |
|---|---|---|
| Identity/Directives: `contradiction ≥ 0.85` | `SUPERSEDES` + inverse `superseded_by` | Old fact → `inactive` |
| Identity/Directives: `entailment ≥ 0.85` | `SUPPORTS` + inverse `supported_by` | Both remain active |
| Identity/Directives: otherwise | No edge | Both remain active |
| Constraints: `contradiction ≥ 0.85` | `CONFLICTS` + inverse `conflicts_with` | Both remain active |
| Constraints: `entailment ≥ 0.85` | `SUPPORTS` + inverse `supported_by` | Both remain active |
| Constraints: otherwise | No edge | Both remain active |

**NLI thresholds:**
- `NLI_CONTRADICTION_THRESHOLD = 0.85`
- `NLI_ENTAILMENT_THRESHOLD = 0.85`
- Model: `nli-deberta-v3-base` INT8 ONNX at `~/.vox/models/nli/nli-deberta-v3-base/model_quantized.onnx`

**Sub-Branch B — Inter-Collection Edge Classifier (`classifiers/inter_edge_classifier.rs`):**

Classifies cross-domain candidate pairs using ModernBERT INT8 ONNX. Returns `Some(forward_edge)` if calibrated prediction score ≥ `EDGE_CLASSIFIER_THRESHOLD` (0.80) and matches the policy matrix's forward edge label. Otherwise returns `None`.

**Edge classifier threshold:** `EDGE_CLASSIFIER_THRESHOLD = 0.80`  
**Model:** `~/.vox/models/classifier/modernbert_edge_creation/model_quantized.onnx`

**Inter-collection edge policy matrix** (`inter_collection_edge()` in `constants.rs`):

| Source → Target | Forward Edge | Inverse Edge |
|---|---|---|
| `Profile → Entities` | `SHAPES` | `shaped_by` |
| `Profile → Constraints` | `restricted_by` | `restricts` |
| `Entities → Constraints` | `DEPENDS_ON` | `constrains` |

Note: `Narrative` never originates inter-collection edges. Special state collections (`Identity`, `Directives`, `Narrative`) do not participate in Sub-Branch B.

**Constants:**
- `STAGE3_BATCH_SIZE = 16`
- `SAME_COLLECTION_CANDIDATE_SEARCH = 0.60` (Intra-collection NLI requires high similarity)
- `INTER_COLLECTION_CANDIDATE_SEARCH = 0.40` (Inter-collection cross-domain edges have lower similarity)
- `SUBFLOOR_CANDIDATE_FLOOR = 0.25`

### 3.4 Stage 4 — Commit & Prune (`stage4_commit.rs`)

| Parameter | Value |
|---|---|
| Batch size | 32 items |
| Input statuses | `evaluated` or `superseded` |

**Logic:**
1. SELECT up to 32 `evaluated` or `superseded` rows, claim atomically.
2. For each item:
   - Generate a fact ID: `mem_{timestamp}_{uuid}`.
   - `INSERT INTO memory_facts` with `status = 'active'` (for `evaluated` items) or `status = 'superseded'` (for `superseded` items from Stage 2 soft-dedup or Stage 3 NLI).
   - If vector present → `INSERT INTO memory_facts_vectors`.
   - For each relation in `relations_json` (including Stage 2 soft-vector `SUPERSEDES` edges) → `INSERT OR IGNORE INTO memory_relations`.
   - If any relation is `SUPERSEDES` → `UPDATE memory_facts SET status = 'inactive' WHERE id = to_id`.
3. All operations wrapped in a single `BEGIN TRANSACTION / COMMIT` with `ROLLBACK` on error.
4. `DELETE FROM personal_memory_queue WHERE id = ?` for all processed rows.

**Constants:**
- `STAGE4_BATCH_SIZE = 32`

> **Execution & Evaluation Rule:** All benchmark probes and evaluation scripts MUST be executed using `--release` mode (`cargo run --release --example <eval_name>`). Debug profile builds omit SIMD/LTO/ONNX optimizations and produce unrepresentative latency metrics.

---

## 4. Scope Routing Matrix (`scope_router.rs`)

The `route_scope()` function maps each `MemoryScope` variant to its SQL and vector collection targets:

| Scope | `sql_collections` | `vector_collections` |
|---|---|---|
| `ChitChat` | *(empty)* | *(empty)* |
| `User` | *(empty)* | `Profile`, `Constraints` |
| `Domain` | *(empty)* | `Entities`, `Directives`, `Constraints` |
| `Temporal` | `Directives`, `Narrative` | `Constraints` |

**Key design decisions:**
- `Identity` is **never** in `sql_collections` — it's pre-loaded into the system prompt at session boot.
- `Directives` is in `vector_collections` for `Domain` (semantic search) and in `sql_collections` for `Temporal` (recency-based fetch).
- `Narrative` is only in `sql_collections` for `Temporal`.

---

## 5. Retrieval Waterfall (`retrieval.rs`)

`retrieve_personal_context_v7()` implements a 4-step dynamic budget allocation capped at `max_personal_memory_share` (default `0.15`, i.e., 15% of the LLM context window).

### Step 1: Identity — NOOP
Active Identity facts are pre-loaded into the system prompt at session boot via `ConversationManager::load_identity_into_system_prompt()`. No per-turn SQL fetch.

### Step 2: SQL Branch — Narrative & Directives Seeds
- **Narrative** (Temporal scope only): Fetches latest 3 active Narrative facts via `fetch_narrative_history(conn, 3)`, ordered by `created_at DESC`. Rendered as `<narrative>` block.
- **Directives** (Temporal scope only): Fetches latest 5 active Directives facts via `fetch_latest_directives(conn, 5)`, ordered by `created_at DESC`. Rendered as `<directives>` block.

Both are token-budgeted — facts are added until `remaining_budget` is exhausted.

### Step 3: Vector Seeds & BFS Graph Expansion
- `fetch_inter_collection_candidates()` with `semantic_similarity_cutoff = 0.40`, no K-cap.
- Seed facts rendered as `"- [{collection}] {fact_text}"`.
- **BFS expansion** up to `max_hops = 2` via `fetch_graph_neighbors()` (bidirectional: `from_id IN ... OR to_id IN ...`).
- Children rendered as `"  ↳ --[{relation}]--> [{collection}] {fact_text}"`.
- `parent_quota = max(30, remaining_budget / seed_count)` per seed group.
- All wrapped in `<semantic_graph>` block.

### Final Output
```
<user_profile>
<narrative>...</narrative>       ← Temporal scope only
<directives>...</directives>     ← Temporal scope only
<semantic_graph>...</semantic_graph>  ← all non-ChitChat scopes
</user_profile>
```

**Constants:**
- `max_personal_memory_share = 0.15` (in `MemorySettings`)
- `semantic_similarity_cutoff = 0.40`
- `top_k_facts = 5` (used for vector search seed limit)
- `max_hops = 2`

---

## 6. Identity Pre-loading (`working_memory.rs`)

`ConversationManager::load_identity_into_system_prompt()` fetches all active Identity facts and appends them as a `<user_profile>` block to the system prompt content.

**Idempotency fix (current code):** Before appending, strips any existing `<user_profile>` block from the system prompt content to prevent duplicate blocks on repeated calls.

**Scope:** All LLM providers (local, remote GPU, cloud) — operates on `ChatMessage.content` strings before provider dispatch.

---

## 7. Intra-Collection Edge Classifier Engine (`classifiers/intra_edge_classifier.rs`)

| Parameter | Value |
|---|---|
| Model | `nli-deberta-v3-base` INT8 ONNX |
| Path | `~/.vox/models/nli/nli-deberta-v3-base/model_quantized.onnx` |
| Contradiction threshold | 0.85 |
| Entailment threshold | 0.85 |
| Intra-op threads | 1 |
| Graph optimization | Level 3 |
| Tokenizer truncation | 512 tokens |

**Calibration:** On load, runs a calibration pair (entailment + contradiction) to determine output logit index mapping. Falls back to default order `[Contradiction, Entailment, Neutral]` if calibration indices collide.

---

## 8. Inter-Collection Edge Classifier Engine (`classifiers/inter_edge_classifier.rs`)

| Parameter | Value |
|---|---|
| Model | ModernBERT INT8 ONNX |
| Path | `~/.vox/models/classifier/modernbert_edge_creation/model_quantized.onnx` |
| Confidence threshold | 0.80 |
| Intra-op threads | 1 |
| Graph optimization | Level 3 |
| Tokenizer truncation | 512 tokens |

**Input format:** `[{src_collection}] {src_fact} [SEP] [{tgt_collection}] {tgt_fact}`  
**Output:** Label index 0..N-1 = positive relations, last index = NONE. Prediction accepted only if `max_prob >= 0.80` and `max_idx < len - 1`.

---

## 9. Embedding Engine (`embedder.rs`)

| Parameter | Value |
|---|---|
| Primary model | MiniLM-L12 INT8 ONNX |
| Path | `~/.vox/models/embedding/minilm-l12-v2/model_int8.onnx` |
| Embedding dimension | 384 |
| Fallback model | `bge-m3` (`model_quantized.onnx`, 1024-dim) |
| Intra-op threads | 1 |
| Graph optimization | Level 3 |

**Aggregation:** Mean pooling over attention-masked tokens, followed by L2 normalization.

---

## 10. Retry Mechanism (`mutations.rs`)

`mark_job_failed()` implements a 3-strike retry policy:

```sql
UPDATE personal_memory_queue
SET retry_count = retry_count + 1,
    error_msg = ?,
    status = CASE WHEN retry_count + 1 >= 3 THEN 'failed' ELSE 'staged_pending' END
WHERE id = ?
```

- Attempts 1–2: `retry_count` increments, status resets to `staged_pending` for re-processing.
- Attempt 3+: status transitions to `failed`, item is quarantined.

---

## 11. Complete Constants Reference

### Pipeline Constants
| Constant | Value | Location |
|---|---|---|
| `STAGE1_BATCH_CEILING` | 128 | `stage1_dedup.rs` |
| `STAGE2_BATCH_SIZE` | 16 | `stage2_embed.rs` |
| `STAGE3_BATCH_SIZE` | 16 | `stage3_eval.rs` |
| `STAGE4_BATCH_SIZE` | 32 | `stage4_commit.rs` |
| `SOFT_VECTOR_DEDUP_THRESHOLD` | 0.95 | `stage2_embed.rs` |
| `SAME_COLLECTION_CANDIDATE_SEARCH` | 0.40 | `stage3_eval.rs` |
| `INTER_COLLECTION_CANDIDATE_SEARCH` | 0.55 | `stage3_eval.rs` |

### NLI Constants
| Constant | Value | Location |
|---|---|---|
| `NLI_CONTRADICTION_THRESHOLD` | 0.85 | `nli.rs` |
| `NLI_ENTAILMENT_THRESHOLD` | 0.85 | `nli.rs` |
| `NLI_CONTRADICTION_CONFIDENCE_THRESHOLD` | 0.85 | `stage3_eval.rs` |
| `NLI_CONTRADICTION_MARGIN_THRESHOLD` | 0.20 | `stage3_eval.rs` |
| `NLI_ENTAILMENT_CONFIDENCE_THRESHOLD` | 0.85 | `stage3_eval.rs` |
| `NLI_MODEL_DIR` | `"nli-deberta-v3-base"` | `classifiers/intra_edge_classifier.rs` |


### Edge Classifier Constants
| Constant | Value | Location |
|---|---|---|
| `EDGE_CLASSIFIER_THRESHOLD` | 0.80 | `classifiers/inter_edge_classifier.rs` |
| `EDGE_CLASSIFIER_MODEL_DIR` | `"classifier/modernbert_edge_creation"` | `classifiers/inter_edge_classifier.rs` |

### Embedding Constants
| Constant | Value | Location |
|---|---|---|
| `EMBEDDING_DIM` | 384 | `embedder.rs` |
| `PRIMARY_MODEL_DIR` | `"minilm-l12-v2"` | `embedder.rs` |
| `FALLBACK_MODEL_DIR` | `"bge-m3"` | `embedder.rs` |

### Dedup Constants
| Constant | Value | Location |
|---|---|---|
| `COSINE_HARD_MATCH_THRESHOLD` | 0.98 | `deduplication.rs` |
| `JACCARD_EXACT_MATCH_THRESHOLD` | 1.0 | `deduplication.rs` |

### Memory Settings (`MemorySettings`)
| Setting | Default | Description |
|---|---|---|
| `context_retrieval_enabled` | `true` | Toggle for retrieval injection |
| `pipeline_processing_enabled` | `true` | Toggle for background pipeline |
| `max_personal_memory_share` | `0.15` | 15% context window cap |
| `context_chaining_window_hours` | `12` | Narrative lookback window |
| `top_k_facts` | `5` | Vector retrieval seed limit |
| `max_hops` | `2` | BFS graph expansion depth |
| `semantic_similarity_cutoff` | `0.40` | Cosine similarity floor for vector retrieval |

### Collection Taxonomy
| Constant | Value |
|---|---|
| `PM_COLLECTIONS` | `["Identity", "Directives", "Narrative", "Profile", "Entities", "Constraints"]` |
| `PM_SPECIAL_STATE_COLLECTIONS` | `["Identity", "Directives", "Narrative"]` |
| `PM_SEMANTIC_GRAPH_COLLECTIONS` | `["Profile", "Entities", "Constraints"]` |
| `PM_TYPE_SPECIAL_STATE` | `"special_state"` |
| `PM_TYPE_SEMANTIC_GRAPH` | `"semantic_graph"` |

### Graph Relations
| Constant | Value |
|---|---|
| `PM_RELATION_SUPPORTS` | `"SUPPORTS"` |
| `PM_RELATION_CONFLICTS` | `"CONFLICTS"` |
| `PM_RELATION_SUPERSEDES` | `"SUPERSEDES"` |
| `PM_RELATION_SHAPES` | `"SHAPES"` |
| `PM_RELATION_DEPENDS_ON` | `"DEPENDS_ON"` |

### Queue Status Lifecycle
| Status | Meaning |
|---|---|
| `staged_pending` | Initial enqueued state |
| `processing_dedup` | Claimed by Stage 1 |
| `deduped` | Stage 1 complete, no duplicate |
| `processing_embed` | Claimed by Stage 2 |
| `embedded` | Stage 2 complete, vector stored |
| `processing_eval` | Claimed by Stage 3 |
| `evaluated` | Stage 3 complete, relations stored |
| `processing_commit` | Claimed by Stage 4 |
| `completed` | Written to permanent store (deleted from queue) |
| `superseded` | Deactivated by dedup/NLI/vector dedup |
| `failed` | Max retries exceeded |

---

## 12. Database Schema

### `memory_facts`
| Column | Type | Notes |
|---|---|---|
| `id` | TEXT PK | `mem_{timestamp}_{uuid}` |
| `type` | TEXT | `foundational`, `operational`, `semantic` |
| `collection` | TEXT | 6 collection names |
| `fact` | TEXT | The fact text |
| `source` | TEXT | `LLM`, `User`, `Import`, `NLI` |
| `status` | TEXT | `active`, `inactive`, `superseded`, `deleted` |
| `session_id` | TEXT | Provenance |
| `turn_id` | TEXT | Provenance |
| `created_at` | INTEGER | Millisecond epoch |

### `memory_facts_vectors`
| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | Autoincrement |
| `fact_id` | TEXT FK | References `memory_facts(id)` |
| `collection` | TEXT | Denormalized for query filtering |
| `embedding` | F32_BLOB(384) | 384-dim MiniLM-L12 vector |

### `memory_relations`
| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | Autoincrement |
| `from_id` | TEXT FK | |
| `to_id` | TEXT FK | |
| `relation` | TEXT | `SUPPORTS`, `CONFLICTS`, `SUPERSEDES`, `SHAPES`, `DEPENDS_ON` |
| `source` | TEXT | `NLI`, `LLM`, `USER` |
| `created_at` | INTEGER | |
| UNIQUE | `(from_id, to_id, relation)` | |

### `personal_memory_queue`
| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | Autoincrement |
| `fact` | TEXT | Raw fact text |
| `collection` | TEXT | Target collection |
| `source` | TEXT | Always `LLM` for pipeline |
| `session_id` | TEXT | |
| `status` | TEXT | Status lifecycle (see above) |
| `attempts` | INTEGER | Legacy counter |
| `retry_count` | INTEGER | Retry counter (3-strike policy) |
| `error_msg` | TEXT | Last error |
| `created_at` | INTEGER | |
| `processed_at` | INTEGER | |
| `claimed_at` | INTEGER | |
| `vector` | F32_BLOB(384) | Embedding (intermediate) |
| `relations_json` | TEXT | JSON array of `RelationEdge` (intermediate) |
| `dedup_match_json` | TEXT | JSON object of `DedupAuditLog` (Stage 1 Jaccard / Stage 2 Soft Vector match) |
| `audit_json` | TEXT | JSON array of `CandidateAuditLog` (Stage 3 NLI & ModernBERT logits/scores, rejection reasons, candidate sources) |

### `memory_pipeline_metrics`
| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | Autoincrement |
| `run_id` | TEXT | Unique pipeline execution run UUID |
| `stage_name` | TEXT | Stage identifier (`stage1_dedup`, `stage2_embed`, `stage3_eval`, `stage4_commit`) |
| `session_id` | TEXT | Provenance session ID |
| `batch_seq` | INTEGER | Batch sequence counter within execution run |
| `items_claimed` | INTEGER | Count of items claimed in stage execution |
| `error_count` | INTEGER | Count of failed items |
| `duration_ms` | INTEGER | Wall-clock execution duration in milliseconds |
| `created_at` | INTEGER | Epoch timestamp |

---

## 13. Query Classifier (`query_classifier.rs`)

Uses `query_sieve::MemoryScopeClassifier` (ModernBERT INT8 ONNX) to classify turn queries into 4 scopes: `ChitChat`, `User`, `Domain`, `Temporal`.

- Model: `~/.vox/models/classifier/modernbert_memory_scope/model_quantized.onnx`
- Calibrated threshold: `tau* = 0.81` — predictions below this confidence default to `Domain`.
- Fallback on error or missing model: `Domain`.

---

## 14. Integration Tests

| Test | File | Validates |
|---|---|---|
| Layer 2 4-stage pipeline | `memory_pipeline_test.rs` | Full pipeline from `staged_pending` → `completed`, queue empty after Stage 4 |
| Layer 3 retrieval + budget cap | `memory_retrieval_test.rs` | Scope routing, vector search, BFS expansion, 15% token budget |
| Layer 4 NLI state resolution | `memory_nli_edge_test.rs` | Real DeBERTa NLI inference, SUPERSEDES edge, old fact deactivation |

---

## 15. Memory Subsystem IPC & Data Access Architecture (`app/src-tauri/src/ipc/memory/`)

### 15.1 Module Structure
- `mod.rs`: Router and command re-exports.
- `graph.rs`: Lightweight topology retrieval (`get_memory_graph_topology`), atomic `graph_version` token (`get_graph_version`), lazy detail loading (`get_memory_fact_detail`), and combined `get_memory_stats`.
- `ingestion.rs`: Real-time queue summary (`get_memory_queue_status`), pipeline toggle control (`toggle_pipeline_processing`), queue retry (`retry_failed_queue`), manual consolidation (`trigger_memory_consolidation`).
- `mutations.rs`: Fact edits (`edit_fact_content`), collection re-assignment (`reassign_fact_collection`), soft deletes (`soft_delete_fact`).
- `conflicts.rs`: Single JOIN conflict discovery (`get_unresolved_conflicts`) and conflict resolution (`resolve_memory_conflict`).

### 15.2 Invariants & Consistency Rules
1. **Atomic Revision Token (`graph_version`)**: `MemoryAppState` holds `graph_version: Arc<AtomicU64>`. Incremented whenever fact text is edited, collections reassigned, facts soft deleted, conflicts resolved, or background pipeline cycles commit new facts. Exposed to the frontend for zero-cost cache validation.
2. **Transaction Boundaries**: All multi-statement mutations (`edit_fact_content`, `soft_delete_fact`, `resolve_memory_conflict`) execute inside explicit SQLite transactions (`BEGIN TRANSACTION;` ... `COMMIT;` with `ROLLBACK` on error).
3. **Vector Synchronization**: `edit_fact_content` updates raw text and executes an SQLite `UPSERT` on `memory_facts_vectors(fact_id)` (`ON CONFLICT(fact_id) DO UPDATE SET embedding = excluded.embedding`), preventing vector embedding drift.
4. **Status Invariant on Deletes & Conflicts**: `soft_delete_fact` and `resolve_memory_conflict` execute `UPDATE memory_facts SET status = 'superseded' WHERE id = ?` on target/loser nodes, ensuring retrieval queries (`WHERE status = 'active'`) never pull soft-deleted or conflict-losing facts into context windows.
5. **Database Indexes**: Schema defines `idx_mfv_fact_id` on `memory_facts_vectors(fact_id)` and `idx_pmq_session` on `personal_memory_queue(session_id)` to prevent $O(N)$ full table scans.