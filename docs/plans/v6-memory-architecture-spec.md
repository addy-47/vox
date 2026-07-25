# Vox v6 Hybrid Cognitive Memory Subsystem Architecture Specification

**Status**: Frozen Architectural Specification  
**Version**: 6.2  
**Target Systems**: `app/src-tauri/src/services/memory/` (Rust Backend)  

---

## 1. Master Pipeline Flow Architecture

Every query entering the Vox memory subsystem follows a single unified, linear processing pipeline:

```
                  User Query
                      │
                      ▼
┌─────────────────────────────────────────────────────────┐
│ 1. Unified Seed Generation Phase                        │
│    ├─► Deterministic SQL Seeds (Class A)                │
│    ├─► Deterministic SQL Seeds (Class B, top_k_facts)   │
│    └─► MiniLM-L12 Dense Vector Seeds (Class C, 384d)   │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 2. Unified Global Seed Pool Assembly                    │
│    └─► Deduplicate seeds by fact_id                     │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 3. Seed-and-Expand Graph Traversal (max_hops = 2)       │
│    ├─► Cycle Breaking via visited_fact_ids HashSet     │
│    └─► Dynamic Fair-Share Parent Budget Allocation     │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 4. Edge Resolution & Exclusions                         │
│    ├─► Hard Exclude SUPERSEDES target nodes             │
│    └─► Render CONFLICTS as prompt warning blocks       │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 5. Dynamic Context Budgeting & Truncation               │
│    └─► Compute 5% Operational & 10% Semantic from ctx  │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 6. Reranking & Context Assembly                         │
│    ├─► Rank by Recency + Proximity + Cosine Score       │
│    └─► Render Clean Prompt Context (<user_profile>)     │
└─────────────────────────┬───────────────────────────────┘
```

### 1.1 Ingestion Pipeline & Parallel Worker Pool Architecture

Background memory ingestion processes enqueued facts extracted during session maintenance through a decoupled 5-step pipeline:

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                             5-Step Async Ingestion Pipeline                                 │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 1: O(1) String Deduplication                                                           │
│         Check exact Jaccard match (1.0). If duplicate, update timestamp in 0ms & complete. │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 2: Parallel Embedding Pool (Multi-Worker)                                              │
│         Dispatch fact text across N worker threads (MiniLM-L12 384d INT8 ONNX, ~30MB/worker).│
│         Generates 384-dimensional dense float vector concurrently across CPU cores.         │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 3: Taxonomy Class Dispatch & Candidate Search                                          │
│         • Class A (Identity, Context): Zero candidate search. Bypasses NLI/LLM.             │
│         • Class B (Constraints, Tasks, Goals): Intra-collection vector search (cos >= 0.40).  │
│         • Class C (Skills, Projects, etc.): Inter-collection Policy Matrix search (cos >= 0.55).│
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 4: Parallel NLI / LLM Classification                                                   │
│         • Class B: ONNX DeBERTa-v3-xsmall NLI model (SUPERSEDES, SUPPORTS, CONFLICTS).      │
│         • Class C: Local LFM2.5-230M GGUF edge classifier + deterministic inverse edge map.  │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 5: Atomic Persistence (Turso Engine MVCC Transaction)                                  │
│         Atomic BEGIN TRANSACTION ... COMMIT writes memory_facts (status = 'active'),        │
│         memory_facts_vectors, and memory_relations (source provenance), marking queue item. │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Core Architectural Principles

1. **Retrieval-First Ingestion**: The memory system exists solely to improve future retrieval context. Ingestion creates ONLY edges that improve retrieval precision. If an edge is never useful during retrieval, it is never created.
2. **Class-Agnostic Strict Phase 1 Hard Deduplication**: Before any NLI or LLM pass, Phase 1 O(1) hard deduplication (`deduplication.rs`) runs on **100% of enqueued facts across ALL collections (Class A, B, and C)**.
   - **Strict Deduplication Criteria**: Exact String Match (`1.0`) OR (**Cosine Similarity $\ge 0.98$ AND Jaccard Similarity $= 1.0$**).
   - Duplicate facts are hard-merged or deleted in 0ms. This strictness prevents accidental deletion of distinct facts. No `SIMILAR` graph edges exist in the database.
3. **Dynamic Un-Hardcoded Token Budgeting**: No token numbers or character budgets are ever hardcoded in source code or specifications. The active LLM context window size (`ctx_window_size`) is dynamically passed at runtime. Operational (5%) and Semantic (10%) budgets are computed dynamically:
   $$B_{\text{operational}} = \lfloor \text{ctx\_window\_size} \times 0.05 \rfloor$$
   $$B_{\text{semantic}} = \lfloor \text{ctx\_window\_size} \times 0.10 \rfloor$$
4. **Decoupled Seed-and-Expand Pipeline**: Seeds are gathered into a Unified Global Seed Pool (deduplicated by `fact_id`) before running bi-directional graph expansion up to `max_hops = 2`.
5. **Automatic Deterministic Inverse Edge Mapping**: Forward inter-collection edges generated by Class B LLM automatically trigger runtime creation of deterministic inverse edges in SQLite.
6. **Ultra-Low Latency Multilingual Vector Embedding**: Memory vector embeddings are generated using `paraphrase-multilingual-MiniLM-L12-v2` (384-dim INT8 ONNX, 10.06 ms CPU inference speed, 118 MB RAM allocation).

---

## 3. Frozen Config Variables & Thresholds

| Variable Name | Config Value | Scope / Type | Description |
| :--- | :---: | :--- | :--- |
| `primary_embedding_model` | **`MiniLM-L12`** | Primary / Architecture | `paraphrase-multilingual-MiniLM-L12-v2` (384-dim INT8 ONNX, 10.06ms CPU latency). |
| `semantic_similarity_cutoff` | **`0.40`** | Primary / User-Facing | Retrieval cutoff threshold for vector similarity search using MiniLM-L12 vector geometry. |
| `same_collection_candidate_search` | **`0.40`** | Internal / Ingestion | Candidate pre-filter threshold for intra-collection Class B NLI processing. |
| `inter_collection_candidate_search` | **`0.55`** | Internal / Ingestion | Candidate pre-filter threshold for inter-collection Class C LLM processing. |
| `top_k_facts` | **`5`** | Primary / User-Facing | Top-$K$ facts limit per collection (used for Class A, B, and C). |
| `max_hops` | **`2`** | Primary / User-Facing | Maximum graph traversal expansion depth during Seed-and-Expand. |
| `NLI_CONTRADICTION_THRESHOLD` | **`0.85`** | Internal / NLI | Minimum probability required for NLI `CONFLICTS` classification. |
| `NLI_ENTAILMENT_THRESHOLD` | **`0.85`** | Internal / NLI | Minimum probability required for NLI `SUPPORTS` classification. |

### Model Threshold Calibration Matrix
Cosine similarity distributions vary by embedding model vector geometry:
* **`paraphrase-multilingual-MiniLM-L12-v2` (384d INT8 ONNX):** Cutoff = **`0.40`** (Noise floor = 0.04-0.23, Cosine Margin = 0.34)
* **`bge-m3` (1024d INT8 ONNX):** Cutoff = **`0.65`** (Noise floor = 0.58-0.74, Cosine Margin = 0.14)
* **`multilingual-e5-small` / `base` (384d/768d ONNX):** Cutoff = **`0.75`** (Noise floor = 0.75-0.85, Cosine Margin = 0.05)

---

## 4. The 3-Class Collection Taxonomy & Ingestion Rules

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                  Vox v6 3-Class Taxonomy                                        │
├──────────────────────────────────┬────────────────────────────────┬─────────────────────────────┤
│ Class A: Direct Isolation        │ Class B: Operational State     │ Class C: Semantic Knowledge │
│ (Deterministic SQL Only)         │ (Deterministic + Intra NLI)    │ (Vector Search + Inter LLM) │
├──────────────────────────────────┼────────────────────────────────┼─────────────────────────────┤
│ • Identity                       │ • Constraints                  │ • Skills                    │
│ • Context                        │ • Tasks                        │ • Preferences               │
│                                  │ • Goals                        │ • Projects                  │
│                                  │                                │ • Experiences               │
│                                  │                                │ • Relationships             │
└──────────────────────────────────┴────────────────────────────────┴─────────────────────────────┘
```

### Ingestion & Seed Generation Rules

#### Class A (Direct Isolation)
* **Collections**: `Identity`, `Context`
* **Ingestion**: Step 1 strict hard-deduplication ONLY. Zero NLI, Zero LLM, Zero edge creation. Isolated from graph traversal.
* **Retrieval Policy**:
  - `Identity`: 100% active facts fetched deterministically via SQL (`WHERE status = 'active'`).
  - `Context`: Time-window chaining fetched deterministically via SQL (`WHERE created_at >= window_start`).

#### Class B (Operational State)
* **Collections**: `Constraints`, `Tasks`, `Goals`
* **Ingestion**: Step 1 strict hard-deduplication $\rightarrow$ Candidate search within same collection (`same_collection_candidate_search = 0.40`) $\rightarrow$ **Intra-collection NLI ONLY** (`deberta-v3-xsmall`).
* **Retrieval Policy**: Deterministic SQL (`Latest K = top_k_facts (5)` facts for `Tasks`, `Goals`, and `Constraints`). Seeds enter Unified Global Seed Pool.

#### Class C (Semantic Knowledge Graph)
* **Collections**: `Skills`, `Preferences`, `Projects`, `Experiences`, `Relationships`
* **Ingestion**:
  - Intra-collection: Step 1 strict hard deduplication in 0ms. **Zero LLM passes.**
  - Inter-collection: Candidate search across connected Policy Matrix collections (`inter_collection_candidate_search = 0.55`) $\rightarrow$ **Inter-collection LLM ONLY** (`LFM2.5-230M` GGUF).
* **Retrieval Policy**: MiniLM-L12 Dense Vector Search (`cosine_similarity >= 0.40`, `top_k_facts = 5`). Seeds enter Unified Global Seed Pool and trigger bi-directional Seed-and-Expand graph traversal up to `max_hops = 2`.

---

## 5. Class C Edge Creation LLM Specification (`LFM2.5-230M`)

### 5.1 Conceptual Purpose
The Edge Creation LLM is a dedicated, ultra-lightweight local model (`LiquidAI/LFM2.5-230M` GGUF, ~230 million parameters) whose sole task is to determine whether a newly ingested fact in Source Collection A has a semantic relationship to an existing fact in Target Collection B across distinct memory domains.

It operates as a constrained, single-token binary/multi-class classifier during background memory ingestion candidate processing (`inter_collection_candidate_search >= 0.55`).

### 5.2 Provided Input Context
To evaluate relationship ground truth accurately without hallucination, the model receives **both the fact content AND session context** for each candidate:

1. **Source Fact**: Content + Collection Name + Session Context (e.g. Session ID / Session Summary).
2. **Target Fact**: Content + Collection Name + Session Context (e.g. Session ID / Session Summary).
3. **Allowed Edge Vocabulary**: Defined strictly by the Connection Policy Matrix (`[{Allowed Edge Type}, NONE]`).

### 5.3 System Prompt & Format
The prompt enforces single-label output with zero chain-of-thought overhead:

```text
<|im_start|>system
You are a memory graph edge classifier for a cognitive AI system.
Your task is to classify the semantic relationship between Fact 1 (Source Collection) and Fact 2 (Target Collection).
Allowed edge types for {Source Collection} -> {Target Collection}: [{Allowed Edge Type}, NONE].
Respond with ONLY the exact edge label name. Do not output explanations or punctuation.<|im_end|>
<|im_start|>user
Fact 1 ({Source Collection}) [Session Context: {Session 1 Context}]: {Fact 1 Content}
Fact 2 ({Target Collection}) [Session Context: {Session 2 Context}]: {Fact 2 Content}
Relationship:<|im_end|>
<|im_start|>assistant
```

#### Example Prompt Trace:
```text
<|im_start|>system
You are a memory graph edge classifier for a cognitive AI system.
Your task is to classify the semantic relationship between Fact 1 (Projects) and Fact 2 (Tasks).
Allowed edge types for Projects -> Tasks: [contains_task, NONE].
Respond with ONLY the exact edge label name.<|im_end|>
<|im_start|>user
Fact 1 (Projects) [Session Context: Session 12 - Building memory engine architecture]: Vox Event-Driven Cognitive Memory Subsystem engine.
Fact 2 (Tasks) [Session Context: Session 14 - Gate 1 benchmarking strategy]: Execute automated benchmark regression sweep on release build.
Relationship:<|im_end|>
<|im_start|>assistant
contains_task
```

### 5.4 Edge Creation Logic & Deterministic Inverse Mapping
When the LLM outputs a valid forward edge label (e.g., `contains_task`), the Rust SQLite runtime automatically creates two edges in the graph without calling the LLM again:

$$\text{(Fact 1: Project)} \xrightarrow{\quad\text{contains\_task [LLM Output]}\quad} \text{(Fact 2: Task)}$$
$$\text{(Fact 2: Task)} \xrightarrow{\quad\text{belongs\_to\_project [Runtime Auto-Created]}\quad} \text{(Fact 1: Project)}$$

---

## 6. Impactful Inter-Collection Policy Matrix

| Source Collection (Class) | Allowed Target Collection (Class) | Forward Edge Type (LLM Generated) | Deterministic Inverse Edge Type (Runtime Created) |
| :--- | :--- | :--- | :--- |
| **`Projects` (Class B)** | `Constraints` (Class A) | `constrained_by` | `restricts_project` |
| **`Projects` (Class B)** | `Skills` (Class B) | `requires_skill` | `used_in_project` |
| **`Projects` (Class B)** | `Tasks` (Class A) | `contains_task` | `belongs_to_project` |
| **`Projects` (Class B)** | `Goals` (Class A) | `drives_goal` | `supported_by_project` |
| **`Preferences` (Class B)** | `Constraints` (Class A) | `restricted_by` | `shapes_preference` |
| **`Preferences` (Class B)** | `Experiences` (Class B) | `shaped_by` | `influenced_preference` |
| **`Skills` (Class B)** | `Experiences` (Class B) | `acquired_in` | `demonstrated_skill` |
| **`Relationships` (Class B)** | `Experiences` (Class B) | `involved_in` | `included_relationship` |
| **`Relationships` (Class B)** | `Projects` (Class B) | `collaborates_on` | `project_contributor` |

*(Note: Class C `Identity` and `Context` are excluded from the Connection Policy Matrix. No edges connect to Class C).*

---

## 7. Intra-Collection Edge Resolution & Prompt Rendering Matrix

NLI runs **strictly on Class B intra-collection pairs** (`same_collection_candidate_search = 0.40`). Edge Resolution Logic during context assembly is collection-specific:

| Collection | Edge Type | Resolution & Retrieval Action | Prompt Context Formatting |
| :--- | :--- | :--- | :--- |
| **`Tasks`** | `SUPERSEDES` | **Auto-Resolved**. Task B supersedes Task A. Task A is hard-excluded from context. | Only Task B is rendered. |
| **`Tasks`** | `SUPPORTS` | **Sub-Task Link**. Task B is a sub-task of Task A. | Indented as child sub-task under Task A. |
| **`Goals`** | `SUPERSEDES` | **Auto-Resolved**. Goal B supersedes Goal A. Goal A is hard-excluded from context. | Only Goal B is rendered. |
| **`Goals`** | `SUPPORTS` | **Sub-Goal / Milestone Link**. Goal B is a sub-goal of Goal A. | Indented as child milestone under Goal A. |
| **`Constraints`** | `SUPERSEDES` | **User-Resolved Only** (`source = 'USER'`). Constraint A is hard-excluded. | Only Constraint B is rendered. |
| **`Constraints`** | `CONFLICTS` | **PRESERVED**. Rendered as an active ambiguity warning block. | Rendered under `[Unresolved Contradictions]`. |
| **`Constraints`** | `SUPPORTS` | **Constraint Refinement**. Constraint B refines/reinforces Constraint A. | Indented as refinement under Constraint A. |

---

## 8. Graph Traversal Algorithms: Cycle Breaking & Dynamic Fair-Share Budgeting

### 8.1 Cycle Breaking Algorithm
During Seed-and-Expand traversal, infinite loops and duplicate node visits are prevented via in-memory set tracking:
1. Maintain `visited_fact_ids: HashSet<String>` initialized with all seed `fact_id`s.
2. For each seed parent $P$, query bi-directional connected neighbors $C$.
3. If $C \in \text{visited\_fact\_ids}$, skip $C$ immediately.
4. If `current_hop > max_hops` (2 hops), halt traversal for that branch.
5. Insert newly visited child $C$ into `visited_fact_ids`.

### 8.2 Dynamic Fair-Share Parent Budget Allocation
To prevent a single seed parent with many child edges (e.g. Project "Vox Engine" with 30 tasks) from consuming 100% of the $B_{\text{semantic}}$ token budget:
1. Compute dynamic $B_{\text{semantic}} = \lfloor \text{ctx\_window\_size} \times 0.10 \rfloor$.
2. Let $P$ be the number of active seed parents in the Global Seed Pool attempting expansion.
3. **Dynamic Parent Quota**: Each parent receives a fair-share token allocation:
   $$Q_{\text{parent}} = \left\lfloor \frac{B_{\text{semantic}}}{P} \right\rfloor$$
4. **Child Ranking Within Parent**: If a parent has $M$ child edges exceeding $Q_{\text{parent}}$, children are ranked by:
   - Edge Type Priority (`constrained_by` > `contains_task` > `requires_skill` > `shaped_by`)
   - Recency (`created_at` timestamp)
   - Cosine similarity to user query
5. **Budget Redistribution**: Unused tokens from parents with few children are dynamically redistributed to remaining unexpanded parents.

---

## 9. Prompt Context Formatting & Provenance

Retrieved facts in `<user_profile>` **never contain internal database `fact_id`s**. They feature clean relative timestamps (`[2 weeks ago]`, `[Yesterday at 4 PM]`) and structural graph tree indentation:

```xml
<user_profile>
  [Identity]
  - Senior Staff Engineer located in Chicago, IL
  - Works remotely for a robotics startup

  [Constraints]
  - [2 weeks ago] Severe tree nut allergy (anaphylactic to walnuts and cashews)
  - [1 month ago] Strictly avoids refined sugars
    └─ (refined_by) -> [2 weeks ago] Diagnosed pre-diabetic, zero sugar intake

  [Unresolved Contradictions]
  - [Unresolved Conflict] "Strictly avoids refined sugars" CONFLICTS WITH "Enjoys sourdough bakery cake"

  [Active Tasks]
  - [Yesterday at 4 PM] Complete Gate 1 benchmark script
    └─ (sub_task) -> [Today at 9 AM] Create synthetic test datasets in sandbox/

  [Semantic Knowledge Context]
  - [3 days ago] Project: Vox Memory Subsystem
    └─ (constrained_by) -> [2 weeks ago] Constraint: Local CPU/ONNX execution only (<50ms)
    └─ (requires_skill) -> [1 week ago] Skill: Rust ONNX & GGUF
    └─ (contains_task)  -> [Yesterday] Task: Complete Gate 1 benchmark script
</user_profile>
```

---

## 10. Database Provenance Schema (`source` Column)

```sql
-- Provenance enum values for memory_relations
-- 'NLI'  : Automatically generated by deberta-v3-xsmall NLI model during Class B ingestion.
-- 'LLM'  : Automatically generated by LFM2.5-230M GGUF model during Class C ingestion.
-- 'USER' : Explicitly created or resolved by user via UI interaction (highest precedence).
```

---

## 11. Queue Staging & Crash Resilience Lifecycle (`personal_memory_queue`)

Extracted facts from LLM session compaction are decoupled from active memory storage via `personal_memory_queue`:

```sql
-- Queue Status Lifecycle
-- 'pending'   : Fact enqueued during LLM compaction, awaiting worker pick-up.
-- 'staged'    : Fact queued while pipeline_processing_enabled setting is false (held without execution).
-- 'processing': Locked status applied when worker thread starts processing queue item.
-- 'completed' : Fact successfully embedded, graph-classified, and persisted to memory_facts.
-- 'failed'    : Error occurred during embedding/ONNX/GGUF execution; records error_msg and attempts.
```

### Crash Resilience Contract
1. **Isolated Active Memory**: A fact is inserted into `memory_facts` (`status = 'active'`) **only at Step 5 after full pipeline completion**.
2. **Zero Memory Corruption**: If an application crash occurs mid-pipeline (during Step 2 embedding or Step 4 classification), no partial or un-embedded facts are written to `memory_facts`.
3. **Automatic Restart Recovery**: Upon Vox app startup, any queue items remaining in `'processing'` status are automatically reset back to `'pending'` and re-processed cleanly from Step 1.
