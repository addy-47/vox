# Vox v7 Cognitive Memory Subsystem Architecture Specification

**Status**: Frozen Architectural Specification  
**Version**: 7.2  
**Target Systems**: `app/src-tauri/src/services/memory/` (Rust Backend)  

---

## 1. Architectural Paradigm Shift (v6 to v7)

The v6 memory architecture suffered from four critical scaling and domain-coupling flaws:
1. **Unbounded Class A Retrieval Flaw**: In v6, `Identity` facts were retrieved via 100% deterministic SQL (`WHERE status = 'active'`). As a user's memory accumulates over months, fetching 100% of identity facts blows the LLM context window.
2. **User-Centric Bias & Missing Agent State**: The v6 taxonomy cataloged user trivia but provided zero structured state for the agent's operational agenda (active tasks, workflow steps, promises made).
3. **Misapplication of NLI to Non-Logical Domains**: v6 attempted NLI processing broadly. In reality, NLI (`deberta-v3-xsmall`) evaluates formal premise-hypothesis entailment/contradiction. Applying NLI to multi-faceted user profile traits causes false supersessions and invalid deletions.
4. **Dead-End Profile Seeds & Missing Intra-Profile Graph Topology**: Profile traits were isolated from graph expansion, preventing the agent from linking user constraints to user preferences.

### 1.1 Non-Deletion Provenance Mandate
**Zero hard deletions (`DELETE FROM memory_facts`) are permitted during pipeline execution.**
- When the ingestion pipeline invalidates an old fact (via deduplication or NLI supersession), it updates `memory_facts.status = 'inactive'`.
- When a user manually deletes a fact, the system updates `memory_facts.status = 'deleted'`.
- Only facts with `memory_facts.status = 'active'` are eligible for active RAG context retrieval. Inactive facts remain in Turso DB with full `memory_relations` provenance intact.

---

## 2. Universal Domain-Agnostic Cognitive Taxonomy (6 Collections)

Memory is partitioned into 6 distinct cognitive domains:

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

### 2.1 Domain Specification Table

| Domain | Cognitive Purpose | Extraction Gating | Evaluation Pipeline | Retrieval Policy |
| :--- | :--- | :--- | :--- | :--- |
| **`Identity`** | **Core User Identity.** Name, age, primary occupation, native language, baseline self-descriptors. | **Prompt-Gated** (Instructs LLM to extract only foundational core descriptors) | Step 1 String Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **Intra NLI (State Supersession)** | Deterministic SQL (`WHERE status = 'active'`). Fixed ~100 token cap. |
| **`Directives`** | **Agent Operational State.** Active user requests, agent promises, pending tasks, workflow steps, sub-goals. | Unrestricted | Step 1 String Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **Intra NLI (State Supersession)** | Dynamic Hybrid Core Budget (8% Cap) |
| **`Constraints`** | **Hard Invariants & Boundaries.** Non-negotiable rules, safety limits, forbidden actions, explicit bans. | Unrestricted | Step 1 String Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **Intra NLI (Conflict Detection)** | Dynamic Hybrid Core Budget (8% Cap) |
| **`Profile`** | **User Persona & Tastes.** Secondary attributes, skills, preferences, habits, style choices, background. | Unrestricted | Step 1 String Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **LLM Edge Classifier** | Semantic Vector Search (ANN) + Graph Traversal |
| **`Entities`** | **External Knowledge Graph.** Codebases, tools, APIs, third-party people, services, devices. | Unrestricted | Step 1 String Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ **LLM Edge Classifier** | Semantic Vector Search (ANN) + Graph Traversal |
| **`Narrative`** | **Session History Flow.** Ephemeral chronological summary of conversation turns within active session. | Extracted per compaction turn | **Bypasses Async Pipeline Entirely** (In-Memory Working Memory state) | Backward Prepending Context Chain (5% Cap) |

---

## 3. Frozen Thresholds & Calibration Matrix

| Threshold Constant | Value | Target Model / System | Purpose |
| :--- | :---: | :--- | :--- |
| `primary_embedding_model` | **`MiniLM-L12`** | 384d INT8 ONNX (`paraphrase-multilingual-MiniLM-L12-v2`, ~10ms CPU) | Primary dense vector embedding engine. |
| `semantic_similarity_cutoff` | **`0.40`** | MiniLM-L12 Vector Search | Minimum cosine similarity threshold for RAG vector retrieval. |
| `soft_vector_dedup_threshold` | **`0.95`** | MiniLM-L12 Step 2 Ingestion | Soft vector deduplication threshold. Calibrated via Gate 1 benchmark (0.0% false inactivations across 500 pairs). |
| `nli_candidate_search_cutoff` | **`0.40`** | MiniLM-L12 Candidate Search | Pre-filter cutoff to select candidate facts for NLI evaluation (`Identity`, `Directives`, `Constraints`). |
| `edge_candidate_search_cutoff`| **`0.55`** | MiniLM-L12 Candidate Search | Pre-filter cutoff for cross-domain / intra-profile 230M LLM edge classification. |
| `NLI_CONTRADICTION_THRESHOLD` | **`0.85`** | `deberta-v3-xsmall` ONNX | Minimum probability required for NLI `CONFLICTS` / `SUPERSEDES` classification. |
| `NLI_ENTAILMENT_THRESHOLD` | **`0.85`** | `deberta-v3-xsmall` ONNX | Minimum probability required for NLI `SUPPORTS` classification. |
| `max_hops` | **`2`** | Seed-and-Expand Traversal | Maximum graph expansion depth from root seeds. |

---

## 4. Master Ingestion Pipeline Architecture

Memory ingestion operates asynchronously via a 5-step worker queue (`personal_memory_queue`):

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                             5-Step Modular Ingestion Pipeline                               │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 1: O(1) String & Jaccard Exact Deduplication                                           │
│         Exact string match OR Jaccard == 1.0. Set old fact status = 'inactive'; write new.    │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 2: Dense Embedding & Step 2 Soft Vector Deduplication                                 │
│         1. Generate 384-dimensional float vector via MiniLM-L12 INT8 ONNX.                  │
│         2. Query existing facts in same collection. If Cosine >= 0.98, mark old fact status │
│            = 'inactive', insert new fact ID & vector.                                       │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 3: Domain-Targeted Evaluator Dispatch                                                  │
│         • If domain in [`Identity`, `Directives`, `Constraints`]: Dispatch to Step 4A (NLI).│
│         • If domain in [`Profile`, `Entities`]: Dispatch to Step 4B (LLM Edge Classifier). │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 4: Domain-Specific Evaluator Execution                                                 │
│         • 4A (NLI Engine): DeBERTa-v3 evaluates candidate pairs within same domain.         │
│         • 4B (LLM Edge Classifier): LFM2.5-230M classifies edges according to Policy Matrix.│
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Step 5: Atomic Persistence (Turso Engine MVCC Transaction)                                  │
│         BEGIN CONCURRENT ... COMMIT writes memory_facts, memory_facts_vectors, and          │
│         memory_relations. Auto-generates deterministic inverse graph edges in Turso DB.     │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Targeted NLI State Resolution Engine

NLI processing runs on stateful/invariant domains (`Identity`, `Directives`, `Constraints`).

### 5.1 Resolution Logic Rules

1. **`Identity` & `Directives` Domains**:
   - Candidate pairs pre-filtered by `nli_candidate_search_cutoff = 0.40`.
   - **`ENTAILMENT` (>= 0.85)**: New fact refines/subsumes old fact. New fact writes `SUPERSEDES` edge to old fact in `memory_relations`. Old fact `status` updated to `'inactive'`.
   - **`CONTRADICTION` (>= 0.85)**: New fact contradicts old fact (e.g. role change or task scope change). New fact writes `SUPERSEDES` edge to old fact; old fact `status` updated to `'inactive'`.
   - **`NEUTRAL`**: Both facts remain `status = 'active'`.

2. **`Constraints` Domain**:
   - Candidate pairs pre-filtered by `nli_candidate_search_cutoff = 0.40`.
   - **`ENTAILMENT` (>= 0.85)**: New constraint refines existing constraint. Writes `SUPPORTS` edge (`refined_by`). Both remain active; child rendered indented under parent.
   - **`CONTRADICTION` (>= 0.85)**: Conflict detected between hard constraints. Writes `CONFLICTS` edge in `memory_relations`. **Neither constraint is set to inactive**. Both remain `status = 'active'` and trigger an `[Unresolved Contradictions]` warning block in prompt context.

---

## 6. Cognitive Edge Classifier Engine & Connection Matrix

Inter-domain and intra-profile graph connections are generated using the local 230M model (`LFM2.5-230M` GGUF) operating as a 4-label classifier.

### 6.1 Cognitive Edge Ontology (4 Directed Edge Types)

| Edge Label (Stored in `memory_relations`) | Semantic Meaning | Forward Edge Sign | Inverse Edge Label (Derived at Traversal) |
| :--- | :--- | :--- | :--- |
| **`REQUIRES`** | Source Fact has a hard prerequisite on Target Fact. | `A -> REQUIRES -> B` | `required_by` (`B -> required_by -> A`) |
| **`RESTRICTS`** | Target Fact imposes a boundary/limit on Source Fact. | `B -> RESTRICTS -> A` | `restricted_by` (`A -> restricted_by -> B`) |
| **`ENABLES`** | Target Fact provides capability/tools that enable Source Fact. | `B -> ENABLES -> A` | `enabled_by` (`A -> enabled_by -> B`) |
| **`RELATES_TO`** | General conceptual or semantic relationship. | `A -> RELATES_TO -> B` | `related_to` (`B -> related_to -> A`) |

### 6.2 Minimal Cognitive Connection Policy Matrix

**Design principle:** Only create edges that are not already reachable within `max_hops = 2` through other edges in the matrix. Redundant edge pairs add ingestion LLM calls with zero retrieval benefit — the traversal engine handles transitive paths automatically.

**Redundant edges removed via 2-hop transitivity:**
- `Identity -> Entities`: reachable via `Identity -> Profile` (hop 1) + `Entities -> Profile` inverse (hop 2).
- `Identity -> Directives`: reachable via `Identity -> Profile` (hop 1) + `Directives -> Profile` inverse (hop 2).
- `Identity -> Constraints`: reachable via `Identity -> Profile` (hop 1) + `Entities -> Constraints` inverse (hop 2).
- `Directives -> Profile`: reachable via `Directives -> Entities` (hop 1) + `Entities -> Profile` (hop 2).

The 230M Edge Classifier is invoked **ONLY** when a newly enqueued fact meets the candidate search threshold against an existing fact in a permitted domain pair:

| Source Domain | Target Domain | Pre-Filter Threshold (cos >= cutoff) | Allowed Output Labels | Deterministic Inverse Edge Behavior |
| :--- | :--- | :---: | :--- | :--- |
| **`Identity`** | `Profile` | `>= 0.50` | `ENABLES`, `RESTRICTS`, `RELATES_TO`, `NONE` | Auto-creates deterministic inverse edge. Single bridge from deterministic tier into semantic graph. |
| **`Directives`** | `Constraints` | `>= 0.50` | `RESTRICTS`, `NONE` | Auto-creates `restricted_by`. Not reachable via Profile path in 2 hops. |
| **`Directives`** | `Entities` | `>= 0.55` | `REQUIRES`, `ENABLES`, `RELATES_TO`, `NONE` | Auto-creates `required_by`, `enabled_by`, `related_to`. Core agent work → tool/knowledge link. |
| **`Entities`** | `Constraints` | `>= 0.50` | `RESTRICTS`, `NONE` | Auto-creates `restricts_entity`. Tool/entity hard-boundary link. |
| **`Entities`** | `Profile` | `>= 0.55` | `ENABLES`, `RELATES_TO`, `NONE` | Auto-creates `enabled_for_user`, `related_profile`. Enables Profile traversal from Entity seeds. |
| **`Entities`** | `Entities` | `>= 0.55` | `REQUIRES`, `ENABLES`, `RELATES_TO`, `NONE` | Auto-creates deterministic inverse edge. Inter-tool dependency graph. |
| **`Profile`** | `Profile` | `>= 0.65` | `RESTRICTS`, `ENABLES`, `RELATES_TO`, `NONE` | Auto-creates deterministic inverse edge within `Profile`. Intra-user trait topology. |

---

## 7. Database Provenance Schema & Table Ownership Matrix

### 7.1 Table & Column Ownership Table

| Table | Column | Valid Values | Writing Component / Owner | Lifecycle & Rules |
| :--- | :--- | :--- | :--- | :--- |
| `personal_memory_queue` | `status` | `'pending'`, `'processing'`, `'completed'`, `'staged'`, `'failed'` | `orchestrator.rs` (Ingestion Worker) | Compaction LLM enqueues `'pending'`. Worker locks item to `'processing'`. On success, updates to `'completed'`. If pipeline disabled, remains `'staged'`. |
| `memory_facts` | `status` | `'active'`, `'inactive'`, `'deleted'` | `mutations.rs` (Memory System) | `'active'`: Default live fact eligible for RAG retrieval. `'inactive'`: Fact deactivated by pipeline dedup or NLI supersession. `'deleted'`: Fact soft-deleted by user. |
| `memory_facts` | `source` | `'LLM'`, `'User'`, `'Import'` | `ingestion.rs` / `mutations.rs` | Provenance origin of the fact text. |
| `memory_relations` | `relation` | `'REQUIRES'`, `'RESTRICTS'`, `'ENABLES'`, `'RELATES_TO'`, `'SUPERSEDES'`, `'CONFLICTS'`, `'SUPPORTS'` | `orchestrator.rs` / `mutations.rs` | Directed graph relation label. Upper case standard. |
| `memory_relations` | `source` | `'NLI'`, `'LLM'`, `'USER'` | `orchestrator.rs` | Provenance mechanism that generated the graph edge. |

### 7.2 Rationale for `SUPERSEDES` Dual-Representation
`SUPERSEDES` is represented i┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                           v7 Pre-Implementation Benchmark Gates                             │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ [PASSED] Gate 1: MiniLM-L12 Soft Vector Deduplication Calibration                           │
│          Harness: `benches/dedup_bench.rs` (subcommand `batch-pair-score`)                 │
│          Dataset: 500 synthetic pairs (`sandbox/datasets/gate1_dedup_500_pairs.json`)       │
│          Result: Calibrated threshold = 0.95. 0.0% false inactivations across 150 hard       │
│                  negatives (max hard neg cos = 0.9074). 28.0% exact reworded duplicate       │
│                  recall. Average pair latency: 29.7 ms. Report: `docs/benchmarks/dedup-bench.md`.│
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Gate 2: DeBERTa-v3 NLI Domain Precision Audit                                               │ domain = 'Directives' AND status = 'active' ORDER BY created_at DESC LIMIT K`
   - Guarantees zero task amnesia regardless of user prompt topic shifts.

2. **`Constraints` & `Profile` / `Entities` (Hybrid Vector + Recency Reranking)**:
   `Score(f) = 0.70 * CosineSimilarity(q, f) + 0.30 * exp(-lambda * delta_t)`
   where `delta_t` is fact age in hours, and `lambda = 0.005` (half-life approx 5.8 days).

---

## 9. Core Invariable Rules (Preserved Invariants)

1. **Edge-LLM Narrative Context Rule**: The 230M Edge Classifier MUST receive the active session `Narrative` summary alongside candidate facts so it has full situational context when classifying relationships.
2. **Working Memory vs RAG Boundary Rule**: RAG retrieval excludes ONLY facts currently present in Working Memory's *active uncompacted turn window* (preventing double-prompting). Facts from earlier compacted turns of the active session are fully eligible for RAG retrieval.
3. **Non-Deletion Provenance Mandate**: Zero `DELETE FROM memory_facts` in ingestion pipeline execution. Deactivated facts set `status = 'inactive'`, soft-deleted user facts set `status = 'deleted'`. All relations remain preserved in Turso DB for auditability.

---

## 10. Pre-Implementation Benchmark Gate Matrix

Before any production code changes are written for v7, the following 3 benchmark gates must pass with concrete empirical metrics.

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                           v7 Pre-Implementation Benchmark Gates                            │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ [PASSED] Gate 1: MiniLM-L12 Soft Vector Deduplication Calibration                           │
│          Harness: `benches/dedup_bench.rs` (subcommand `batch-pair-score`)                 │
│          Dataset: 500 synthetic pairs (`sandbox/datasets/gate1_dedup_500_pairs.json`)       │
│          Result: Calibrated threshold = 0.95. 0.0% false inactivations across 150 hard       │
│                  negatives (max hard neg cos = 0.9074). 28.0% exact reworded duplicate       │
│                  recall. Average pair latency: 29.7 ms. Report: `docs/benchmarks/dedup-bench.md`.│
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ [FAILED] Gate 2: DeBERTa-v3 NLI Domain Precision Audit                                      │
│          Harness: `benches/nli_bench.rs` (subcommand `batch-nli-score`)                     │
│          Dataset: 450 synthetic pairs (`sandbox/datasets/gate2_nli_400_pairs.json`)          │
│          Result: Overall 78.67% accuracy. Directives = 98.67% (PASSED). Identity = 76.00%    │
│                  (FAILED), Constraints = 65.00% (FAILED). Failure mode: Entailment          │
│                  misclassified as Neutral due to MNLI strict formal logic bias.             │
│                  Report: `docs/benchmarks/nli-precision-bench.md`.                         │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│ Gate 3: LFM2.5-230M Edge Classifier Capabilities & Batching Probe                           │
│         Harness: `examples/edge_classifier_probe.rs`                                        │
│         Target: Probe single-pair vs multi-pair batching classification precision & latency. │
│         Pass Criteria: >= 85% edge precision vs gold reference; < 100ms per pair inference.   │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.1 Fine-Tuning Contingency Plan
If `LFM2.5-230M` GGUF falls below the 85% edge classification precision target on Gate 3:
1. **Dataset Creation**: Generate a 1,000-pair synthetic dataset (`sandbox/datasets/edge_classifier_v7.jsonl`) covering all allowed pairs in the Cognitive Connection Policy Matrix.
2. **LoRA / Unsloth Fine-Tuning**: Fine-tune `LFM2.5-230M` or `Qwen2.5-0.5B` on remote GPU server (`hypr4@100.86.62.14`).
3. **Quantization & Export**: Export GGUF (`Q8_0` / `Q4_K_M`) to `~/.vox/models/llm/` for local CPU inference.
