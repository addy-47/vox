# Vox v7 Cognitive Memory Subsystem Architecture Specification

**Status**: Frozen Master Architectural Specification  
**Version**: 7.11 (Functional Memory Architecture Specification)  
**Target Systems**: `app/src-tauri/src/services/memory/` (Rust Backend)  

---

## 1. Architectural Paradigm Shift & Core Principles

The v7 memory architecture provides a unified, deterministic, and domain-agnostic memory engine for real-time voice AI.

1. **Pre-Retrieval Memory Scope Classification (`query-sieve-rs`)**: All retrieval and vector candidate selection flows downstream from scope classification. Turn queries are classified into 4 discrete scope categories (`ChitChat`, `User`, `Domain`, `Temporal`). Irrelevant collections are pruned prior to vector search to eliminate context pollution. `Domain` serves as the primary fallback default.
2. **Two-Class Collection Taxonomy**: Memory collections are partitioned into **Special State Collections** (deterministic SQL/chaining fetch: `Identity`, `Directives`, `Narrative`) and **Semantic Graph Collections** (vector search + graph traversal: `Profile`, `Entities`, `Constraints`).
3. **Provider-Agnostic System Prompt Identity Prefilling**: `Identity` facts represent static user/agent invariants. Active `Identity` facts (`WHERE collection = 'Identity' AND status = 'active'`) are pre-loaded at session boot and baked directly into the System Prompt template across all providers (local LLM, remote GPU server, or cloud APIs). Identity context is universally available across all turns (including `ChitChat`) with 0ms per-turn SQL overhead.
4. **Dynamic Waterfall Token Allocation Under 15% Cap**: Personal memory context prompt rendering is capped at `max_personal_memory_share = 0.15` (15% of total LLM Context Window). Token allocation flows dynamically through a scope-specific waterfall hierarchy.

### 1.1 System Invariable Rules & Provenance Mandate
1. **15% Context Share Hard Cap Rule**: Personal memory prompt rendering MUST NOT exceed `max_personal_memory_share = 0.15` (15% of total LLM context window).
2. **Non-Deletion Provenance Mandate**: Zero `DELETE FROM memory_facts` during pipeline execution. Inactive facts set `status = 'inactive'`, soft-deleted user facts set `status = 'deleted'`. All relations remain preserved in Turso DB for auditability.
3. **Deterministic System Prompt Identity Rule**: Active `Identity` facts MUST be pre-loaded at session startup into the System Prompt template across all LLM inference providers. Dynamic RAG waterfalls exclude dynamic `Identity` SQL queries unless ephemeral mid-session identity deltas exist.

---

## 2. Collection Taxonomy: Special State vs. Semantic Graph Collections

Memory is partitioned into two distinct structural collection classes:

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                   Vox v7 Collection Taxonomy                                     │
├──────────────────────────────────────────────────┬───────────────────────────────────────────────┤
│ Class 1: Special State Collections               │ Class 2: Semantic Graph Collections           │
│ (Deterministic System Prompt & SQL Chaining)     │ (Vector Similarity Search + Graph Traversal)  │
├──────────────────────────────────────────────────┼───────────────────────────────────────────────┤
│ • Identity   (Pre-loaded into System Prompt)     │ • Profile     (User Persona/Tastes/Skills)    │
│ • Directives (Fetch Latest K active items)       │ • Entities    (Projects/Codebases/Tools)      │
│ • Narrative  (Backward Context Chaining window)  │ • Constraints (Hard System Rules & Boundaries)│
└──────────────────────────────────────────────────┴───────────────────────────────────────────────┘
```

### 2.1 Collection Specification & Retrieval Matrix

| Collection Name | Collection Class | Primary Retrieval Engine | Retrieval Behavior |
| :--- | :--- | :--- | :--- |
| **`Identity`** | Special State | System Prompt Prefill | Pre-loaded at session startup into base System Prompt (`WHERE collection = 'Identity' AND status = 'active'`). Available across 100% of turns (including `ChitChat`) across all providers with 0ms SQL overhead. Excluded from dynamic per-turn RAG SQL queries unless ephemeral mid-session deltas exist. |
| **`Directives`** | Special State | Vector Search (Domain) / Recency SQL (Temporal) | **Domain Scope**: Vector search ($\text{cos} \ge 0.40$, max `top_k_facts` = 5) alongside `Entities` & `Constraints`, participating in BFS seed graph expansion.<br>**Temporal Scope**: Recency SQL (`WHERE collection = 'Directives' AND status = 'active' ORDER BY created_at DESC LIMIT 5`). |
| **`Narrative`** | Special State | Context Chaining | Prepending session summary chain (`context_chaining_window_hours`) inside `retrieve_personal_context_v7()` waterfall under 15% budget cap. Triggered exclusively in `Temporal` scope. |
| **`Profile`** | Semantic Graph | Vector Search + Graph | Vector search ($\text{cos} \ge 0.40$), truncated to `top_k_facts` (5). Triggered exclusively in `User` scope. |
| **`Entities`** | Semantic Graph | Vector Search + Graph | Vector search ($\text{cos} \ge 0.40$), truncated to `top_k_facts` (5). Triggered exclusively in `Domain` scope. |
| **`Constraints`** | Semantic Graph | Vector Search + Graph | Vector search ($\text{cos} \ge 0.40$), truncated to `top_k_facts` (5). Triggered in `User`, `Domain`, and `Temporal` scopes. |

---

## 3. Calibration Settings & System Constants

| Threshold Constant | Value | Config / Source Location | Target System / Purpose |
| :--- | :---: | :--- | :--- |
| `primary_embedding_model` | **`MiniLM-L12`** | `~/.vox/models/embedding/` | 384d INT8 ONNX dense vector engine (~10ms CPU). |
| `soft_vector_dedup_threshold` | **`0.95`** | Frozen Ingestion Rule | Soft vector deduplication threshold in Stage 2 (`SOFT_VECTOR_DEDUP_THRESHOLD`). |
| `SAME_COLLECTION_CANDIDATE_SEARCH` | **`0.60`** | Frozen Ingestion Rule | Pre-filter cutoff to select candidate facts for intra-collection NLI state resolution. |
| `INTER_COLLECTION_CANDIDATE_SEARCH`| **`0.40`**| Connection Policy Matrix | Pre-filter cutoff for inter-collection directed Edge Classification. |
| `NLI_CONTRADICTION_THRESHOLD` | **`0.85`** | `nli-deberta-v3-base` ONNX | Minimum probability required for NLI `SUPERSEDES` / `CONFLICTS` classification. |
| `NLI_ENTAILMENT_THRESHOLD` | **`0.85`** | `nli-deberta-v3-base` ONNX | Minimum probability required for NLI `SUPPORTS` classification. |
| `EDGE_CONFIDENCE_THRESHOLD` | **`0.80`** | `modernbert-base` INT8 ONNX | Minimum positive edge probability required for graph relation creation (below 0.80 defaults to `NONE`). |
| `semantic_similarity_cutoff` | **`0.40`** | `MemorySettings.semantic_similarity_cutoff` | Cutoff floor for Turn Query RAG vector retrieval. |
| `top_k_facts` | **`5`** | `MemorySettings.top_k_facts` | **Turn Query RAG vector retrieval seed limit per target collection (Profile, Entities, Directives, Constraints).** |
| `max_hops` | **`2`** | `MemorySettings.max_hops` | Maximum graph expansion depth during Seed-and-Expand BFS. |
| `max_personal_memory_share` | **`0.15`** | `MemorySettings.max_personal_memory_share` | **Hard context window budget cap (15% of total LLM prompt window).** |

---

## 4. Edge Connection Matrix

### 4.1 Targeted NLI State Resolution Engine

NLI processing evaluates formal logical relationships strictly within stateful/invariant domains (`Identity`, `Directives`, `Constraints`). Note: `Narrative` is a Special State collection that bypasses vector embedding in Stage 2 and is excluded from Stage 3 NLI and Edge Classifier evaluation.

1. **`Identity` & `Directives` Domains**:
   - Candidate facts selected via threshold filtering (`SAME_COLLECTION_CANDIDATE_SEARCH = 0.60`).
   - **`ENTAILMENT` (>= 0.85)**: New fact *refines/extends* the existing fact. Writes `SUPPORTS` edge (`new_fact → SUPPORTS → existing_fact`). **Both facts remain `status = 'active'`**. The existing fact (parent) pulls the new fact (child) alongside it during RAG retrieval.
   - **`CONTRADICTION` (>= 0.85)**: New fact *contradicts/replaces* the existing fact. Writes `SUPERSEDES` edge (`new_fact → SUPERSEDES → existing_fact`). Existing fact `status` updated to `'inactive'`.
   - **`NEUTRAL`**: No edge written. Both facts remain active.

2. **`Constraints` Domain**:
   - **`ENTAILMENT` (>= 0.85)**: New constraint *refines* existing constraint. Writes `SUPPORTS` edge (`new_fact → SUPPORTS → existing_fact`). Both remain `status = 'active'`; new constraint (child) is rendered indented under its parent constraint during RAG retrieval.
   - **`CONTRADICTION` (>= 0.85)**: Conflict detected between hard constraints. Writes `CONFLICTS` edge (`new_fact → CONFLICTS → existing_fact`). **Neither constraint is deactivated**. Both remain `status = 'active'` and trigger an `[Unresolved Conflicts]` warning block in prompt context.
   - **`NEUTRAL`**: No edge written. Both facts remain active.

### 4.2  Inter-Domain Connection Policy Matrix (Stage 3B)

Cross-domain graph connections generated by the ModernBERT INT8 ONNX sequence classifier dynamically evaluate output prediction logits across 4 operational edge labels (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE`) across 7 sanctioned collection pairs:

| Source Domain | Target Domain | Pre-Filter Threshold (cos >= cutoff) | Allowed Operational Predictions | Deterministic Traversal Behavior |
| :--- | :--- | :---: | :--- | :--- |
| **`Identity`** | `Profile` | `>= 0.40` | `SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE` | Bridge from core identity into user profile traits. |
| **`Directives`** | `Constraints` | `>= 0.40` | `SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE` | Connects active tasks to hard system boundaries. |
| **`Directives`** | `Entities` | `>= 0.40` | `SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE` | Core agent work $\rightarrow$ tool/codebase project dependency. |
| **`Entities`** | `Constraints` | `>= 0.40` | `SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE` | Entity/tool specific hard boundary link. |
| **`Entities`** | `Profile` | `>= 0.40` | `SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE` | Connects codebase/tool experience to user profile skills. |
| **`Entities`** | `Entities` | `>= 0.40` | `SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE` | Inter-tool & inter-codebase dependency graph. |
| **`Profile`** | `Profile` | `>= 0.40` | `SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE` | Intra-user trait topology & preference constraints. |

### 4.3 Model Output → Edge Created → Inverse Edge Reference

This section provides a definitive 3-column lookup for every model signal produced by Stage 3's two sub-branches, what edge label is written into `memory_relations`, and what the inverse edge label is when both directions are stored.

#### 4.3.1 Sub-Branch A: NLI Engine (DeBERTa-v3) — Intra-Domain State Resolution

| NLI Model Output | Collection Domain | Edge Written (`new → existing`) | Inverse Edge (`existing → new`) | Downstream Behavior |
| :--- | :--- | :--- | :--- | :--- |
| **`ENTAILMENT`** (>= 0.85) | `Identity`, `Directives` | `SUPPORTS` | `supported_by` | Both facts remain `active`. Parent (existing) pulls child (new) alongside during RAG retrieval. |
| **`CONTRADICTION`** (>= 0.85) | `Identity`, `Directives` | `SUPERSEDES` | `superseded_by` | Existing (old) fact set to `status = 'inactive'`. New fact is the replacement. |
| **`ENTAILMENT`** (>= 0.85) | `Constraints` | `SUPPORTS` | `supported_by` | Both facts remain `active`. New (child) constraint rendered indented under existing (parent) in prompt context. |
| **`CONTRADICTION`** (>= 0.85) | `Constraints` | `CONFLICTS` | `conflicts_with` | Both facts remain `active`. Triggers `[Unresolved Conflicts]` warning block in retrieval output. |
| **`NEUTRAL`** | All | *(no edge)* | *(no edge)* | No status change. No edge written. |

> **Note on inverse edges for NLI:** The NLI sub-branch writes only the forward edge (`new_fact → relation → existing_fact`). The inverse label above is the conceptual reverse relationship readable from the `existing_fact`'s perspective and may be used during bidirectional BFS graph traversal in retrieval.

#### 4.3.2 Sub-Branch B: Edge Classifier (ModernBERT INT8 ONNX) — Inter-Domain Edge Creation

The Edge Classifier outputs one of 4 labels. When a positive edge is predicted (confidence >= `EDGE_CONFIDENCE_THRESHOLD = 0.80`), **both the forward and inverse edges are written** into `memory_relations` in a single atomic write.

| Classifier Output Label | Forward Edge Written (`source → target`) | Inverse Edge Written (`target → source`) | Symmetric? | Semantic Meaning |
| :--- | :--- | :--- | :---: | :--- |
| **`SHAPES`** | `SHAPES` | `shaped_by` | No | Source collection fact shapes/influences the target fact. |
| **`DEPENDS_ON`** | `DEPENDS_ON` | `dependency_of` | No | Source collection fact depends on or is constrained by the target fact. |
| **`CONFLICTS_WITH`** | `CONFLICTS_WITH` | `conflicts_with` | Yes | Mutual conflict between facts in different domains. Both directions use the same label. |
| **`NONE`** | *(no edge written)* | *(no edge written)* | — | Classifier found no meaningful cross-domain relationship. |

---

## 5. Pre-Retrieval `MemoryScope` Classification (`query-sieve-rs`)

Before executing RAG search, `query-sieve-rs` classifies the turn query into an idiomatic Rust `MemoryScope` enum to prune irrelevant vector candidate collections:

```rust
pub enum MemoryScope {
    ChitChat,       // Zero RAG search (greetings, banter)
    User,           // Identity + Profile + Constraints
    Domain,         // Identity + Entities + Directives + Constraints (PRIMARY DEFAULT)
    Temporal,       // Identity + Narrative + Directives + Constraints
}
```

### 5.1 Scope Variant Execution & Pruning Matrix

| `MemoryScope` Variant | Triggers & Intent | System Prompt Identity | Deterministic / SQL Fetches | Vector Search Collections | Pruning & Exclusions |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`ChitChat`** | Casual greetings, banter (*"hello"*, *"thanks"*). | **Inherited (Pre-loaded)** | **None** | **None** | All dynamic memory RAG retrieval skipped. 0 per-turn SQL overhead. System prompt identity inherited automatically. |
| **`User`** | Persona, identity, preferences, personal rules (*"my role"*, *"I prefer Python"*). | **Inherited (Pre-loaded)** | Ephemeral `Identity` deltas (if any) | `Profile` + `Constraints` | `Entities`, `Directives`, and `Narrative` pruned from vector search. |
| **`Domain`** *(Primary Default)* | Projects, codebase, tools, tasks, technical Q&A (*"Vox"*, *"Rust error"*). | **Inherited (Pre-loaded)** | Ephemeral `Identity` deltas | `Entities` + `Directives` + `Constraints` | `Profile` and `Narrative` pruned from search. |
| **`Temporal`** | Session continuity, temporal recap (*"yesterday"*, *"where were we"*). | **Inherited (Pre-loaded)** | Ephemeral `Identity` deltas + `Narrative` (Chaining) + `Directives` (Recency SQL, limit 5) | `Constraints` | `Profile` and `Entities` pruned from vector search. |

---

## 6. Dynamic Waterfall Token Budgeting Subsystem

Memory prompt rendering is capped by a single setting: **`max_personal_memory_share = 0.15`** (15% of total LLM Context Window). Token allocation is executed step-by-step per scope via a **Dynamic Waterfall Hierarchy**. Primary active `Identity` facts are pre-loaded into the System Prompt template at session startup.

```
                  TOTAL MEMORY SHARED BUDGET (15% Context Window Cap)
                                         │
                                         ▼
                 [Step 1: Ephemeral Mid-Session Identity Delta Deduction]
                 - Deducts un-consolidated ephemeral Identity deltas (if any exist)
                 - Remaining Budget = Total Budget - Identity Delta Tokens
                                         │
                                         ▼
                 [Step 2: Scope Target Entrypoint Seeds + Intra-Edges]
                 - Fetches target seeds & resolves intra-edge links for active scope
                 - Remaining Budget = Remaining Budget - Seed & Intra-Edge Tokens
                                         │
                                         ▼
                 [Step 3: Bi-Directional Graph Traversal Expansion]
                 - Expands child nodes along `memory_relations` edges
                   (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `SUPPORTS`) up to `max_hops` (2)
                 - Remaining Budget = Remaining Budget - Child Node Tokens
                                         │
                                         ▼
                 [Step 4: Fair-Share Dynamic Token Redistribution]
                 - Unused token quota from smaller parent trees dynamically
                   redistributes to expand subsequent parent trees
```

### 6.1 Step-by-Step Waterfall Execution Per Scope

#### A. Scope: `User`
1. **Step 1 (Ephemeral Identity Delta Deduction)**: Render and deduct ephemeral mid-session `Identity` deltas (if any exist).
2. **Step 2 (Scope Target Entrypoint Seeds + Intra-Edges)**:
   - Perform vector search on `Profile` and `Constraints` ($\text{cos} \ge 0.40$, max `top_k_facts` = 5 per collection).
   - Resolve intra-edge links (e.g. `Identity` $\rightarrow$ `SHAPES`/`SUPPORTS` $\rightarrow$ `Profile`, or `Constraints` intra-supports).
   - Deduct seed and intra-edge tokens from remaining budget.
3. **Step 3 (Graph Traversal Expansion)**: Expand cross-collection child nodes along `memory_relations` edges (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`) up to `max_hops` (2) until budget is exhausted.
4. **Step 4 (Dynamic Redistribution)**: Unused token quota redistributes to subsequent seed trees.

#### B. Scope: `Domain` (Primary Fallback Default)
1. **Step 1 (Ephemeral Identity Delta Deduction)**: Render and deduct ephemeral mid-session `Identity` deltas (if any exist).
2. **Step 2 (Scope Target Entrypoint Seeds + Intra-Edges)**:
   - Perform vector search on `Entities`, `Directives`, and `Constraints` ($\text{cos} \ge 0.40$, max `top_k_facts` = 5 per collection). `Profile` is EXCLUDED.
   - All retrieved seeds (`Entities`, `Directives`, `Constraints`) are integrated into the seed set for intra-edge resolution. Deduct seed tokens from remaining budget.
3. **Step 3 (Graph Traversal Expansion)**: Expand connected child nodes (including linked `Profile` or `Identity` nodes reachable via edges) up to `max_hops` (2).
4. **Step 4 (Dynamic Redistribution)**: Unused token quota redistributes to subsequent seed trees.

#### C. Scope: `Temporal`
1. **Step 1 (Deterministic Identity Deduction)**: Deduct ephemeral mid-session `Identity` deltas (if any exist).
2. **Step 2 (Scope Target Entrypoint Seeds + Intra-Edges)**:
   - Fetch `Narrative` history via Backward Context Chaining (`context_chaining_window_hours`) inside `retrieve_personal_context_v7()` waterfall and deduct tokens from remaining budget.
   - Fetch Latest 5 active `Directives` via Recency SQL (`ORDER BY created_at DESC LIMIT 5`).
   - Perform vector search on `Constraints` ($\text{cos} \ge 0.40$, max `top_k_facts` = 5).
   - Integrate `Directives` and `Constraints` seeds into the seed set for intra-edge resolution. Deduct seed tokens from remaining budget.
3. **Step 3 (Graph Traversal Expansion)**: Expand connected child nodes along `memory_relations` edges up to `max_hops` (2).
4. **Step 4 (Dynamic Redistribution)**: Unused token quota redistributes to subsequent seed trees.

### 6.2 Dynamic Fair-Share Parent Formula
$$\text{parent\_quota\_tokens} = \max\left(30, \frac{\text{remaining\_scope\_budget}}{\max(1, \text{remaining\_parents})}\right)$$

---

## 7. Pre-Implementation Benchmark Gate Matrix

Before any production code changes are executed for v7, the following 3 benchmark gates serve as the empirical validation harness:

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                           v7 Pre-Implementation Benchmark Gates                            │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ [PASSED] Gate 1: MiniLM-L12 Soft Vector Deduplication Calibration                           │
│          Harness: `examples/dedup_bench.rs`                                                 │
│          Dataset: 500 synthetic pairs (`sandbox/datasets/gate1_dedup_500_pairs.json`)       │
│          Result: Calibrated threshold = 0.95. 0.0% false inactivations across 150 hard       │
│                  negatives (max hard neg cos = 0.9074). 28.0% exact reworded duplicate       │
│                  recall. Average pair latency: 29.7 ms. Report: `docs/benchmarks/dedup-bench.md`.│
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ [PASSED] Gate 2: DeBERTa-v3 NLI Domain Precision Audit                                      │
│          Harness: `examples/nli_precision_bench.rs`                                         │
│          Dataset: 450 synthetic pairs (`sandbox/datasets/gate2_nli_400_pairs.json`)          │
│          Result: `nli-deberta-v3-base` selected. Overall 85.11% accuracy across domains     │
│                  (Directives = 99.33%, Constraints = 75.50%). Average pair latency: 64.8 ms. │
│                  Report: `docs/benchmarks/nli-precision-bench.md`.                         │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ [PASSED] Gate 3: Cognitive Edge Classifier Calibration & ONNX Fine-Tuning                   │
│          Harness: `app/src-tauri/benches/edge_classifier_bench.rs`                          │
│          Dataset: 6,000 verified pairs (`sandbox/datasets/gate3_v7_ontology_6000p.json`)     │
│          Result: `ModernBERT-base` INT8 ONNX fine-tuned for 6 epochs. Test Acc = 87.50%,    │
│                  Test Macro F1 = 0.8722, Peak Val Acc = 88.17%. Calibrated graph threshold  │
│                  tau* = 0.80 achieving 86.67% Positive Edge Precision & 7.69% FP rate.       │
│                  Average CPU latency: ~28.4 ms/pair. Report: `docs/benchmarks/edge-classifier-bench.md`.│
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```
