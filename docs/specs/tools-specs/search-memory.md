# Behavioral Specification: `search_memory` Tool

> **Document Type:** Tool Behavioral & Interface Specification  
> **Tool Identifier:** `search_memory`  
> **Classification:** Cognitive Observation (`ToolFlow::NonTerminal`)  
> **Target Subsystems:** `services/harness/stages/tools/search_memory.rs`, `services/memory/retrieval.rs`, `services/intelligence/embedding.rs`  
> **Status:** Approved Target Specification  
> **Format Rule:** Pure behavioral specification containing zero library-specific code snippets. All behaviors, ranking algorithms, scoring equations, parameters, and contracts are specified with rigorous, language-agnostic precision.

---

## 1. Tool Metadata & Scope

- **Identifier**: `search_memory`
- **Domain Availability**: `ToolDomain::All` (Adaptive parameters for Modular Assistant and Realtime S2S)
- **Flow Category**: `ToolFlow::NonTerminal` (Modular triggers `NonTerminalPhase` / `InteractionState::Working` and speech filler; Realtime yields tool result frame to remote model)
- **Description**: Searches episodic project memory for past factual decisions, completed work, blockers, next steps, or technical context across previous turns and sessions.

### 1.1 Search Scope Boundary
- **Included Categories**: Active episodic facts categorized as `objective`, `workdone`, `blocker`, `next_step`, or `pitfall`.
- **Excluded Categories**: Personal identity facts (`personal` category). Personal identity facts are governed exclusively by the static `<user_identity>` block assembled in `PromptBuilderStage` and must never be duplicated or retrieved through episodic tool search.

---

## 2. Parameter Schemas

### 2.1 Modular Assistant Schema (`PipelineMode::Modular`)
```json
{
  "name": "search_memory",
  "description": "Searches project memory for past factual decisions, completed work, blockers, next steps, or technical context.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "The semantic search phrase or keyword expression to look up in memory."
      },
      "spoken_filler": {
        "type": "string",
        "description": "A natural, brief 3 to 5 word spoken filler phrase to say aloud right now while searching (e.g., 'Checking your notes...')."
      }
    },
    "required": ["query", "spoken_filler"]
  }
}
```

### 2.2 Realtime S2S Schema (`PipelineMode::Realtime`)
```json
{
  "name": "search_memory",
  "description": "Searches project memory for past factual decisions, completed work, blockers, next steps, or technical context.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "The semantic search phrase or keyword expression to look up in memory."
      }
    },
    "required": ["query"]
  }
}
```
*Note: In Realtime mode, `spoken_filler` is omitted. The remote voice provider natively synthesizes filler audio over WebSocket.*

### 2.3 Threshold Authority Rule
The model is strictly prohibited from passing result limits (`k`), cutoff scores, or table targets. The harness controls retrieval volume and quality thresholds deterministically from user settings:
- `top_k_facts`: Clamped between 1 and 100 (Default: `10`).
- `semantic_similarity_cutoff`: Clamped between 0.0 and 1.0 (Default: `0.35`).

---

## 3. Retrieval Pipeline & Hybrid RRF Ranking

Memory retrieval executes a 2-stage hybrid search combining dense semantic vector search with sparse keyword full-text search, fused via Reciprocal Rank Fusion (RRF).

```
                 [Search Query: &str]
                          │
         ┌────────────────┴────────────────┐
         ▼                                 ▼
┌──────────────────┐             ┌──────────────────┐
│  Dense Vector    │             │  Sparse FTS5     │
│  Embedding       │             │  Keyword Match   │
│  (MiniLM ONNX)   │             │  (Turso SQLite)  │
└────────┬─────────┘             └────────┬─────────┘
         ▼                                 ▼
┌──────────────────┐             ┌──────────────────┐
│ Cosine Sim Top-N │             │ BM25 Rank Top-N  │
│ Scored Candidate │             │ Scored Candidate │
│ List (Rank_dense)│             │ List (Rank_sparse│
└────────┬─────────┘             └────────┬─────────┘
         └────────────────┬───────────────┘
                          ▼
             ┌─────────────────────────┐
             │ Reciprocal Rank Fusion  │
             │ RRF Score Calculation   │
             └────────────┬────────────┘
                          ▼
             ┌─────────────────────────┐
             │ Similarity Cutoff Guard │
             │ & Top-K Truncation      │
             └────────────┬────────────┘
                          ▼
             [Observation Serialization]
```

### 3.1 Dense Vector Search Path
1. **Query Embedding**: The search query string is encoded into a normalized 384-dimensional dense float vector using the local ONNX embedding model (`all-MiniLM-L6-v2`) via the non-blocking inference worker.
2. **Cosine Similarity Evaluation**: Dense similarity is computed against stored fact vector embeddings in the local Turso SQLite database:
   $$\text{CosineSim}(\vec{q}, \vec{d}) = \frac{\vec{q} \cdot \vec{d}}{\|\vec{q}\|_2 \|\vec{d}\|_2}$$
   Because embeddings are stored unit-normalized ($\|\vec{d}\|_2 = 1.0$) and the query vector is unit-normalized ($\|\vec{q}\|_2 = 1.0$), this simplifies to the dot product $\vec{q} \cdot \vec{d}$.
3. **Candidate Ordering**: Facts are ordered descending by cosine similarity, establishing `rank_dense(f) \in [1, N]`.

### 3.2 Sparse Keyword Full-Text Search Path
1. **FTS5 Query Generation**: The search query is tokenized and sanitized into an SQLite FTS5 query matching the `memory_facts` text ledger.
2. **BM25 Evaluation**: Matches are ranked using native BM25 scoring over token frequencies:
   $$\text{Score}_{\text{BM25}}(D, Q) = \sum_{t \in Q} \text{IDF}(t) \cdot \frac{f(t, D) \cdot (k_1 + 1)}{f(t, D) + k_1 \cdot \left(1 - b + b \cdot \frac{|D|}{\text{avgdl}}\right)}$$
3. **Candidate Ordering**: Facts are ordered descending by BM25 match quality, establishing `rank_sparse(f) \in [1, N]`.

### 3.3 Reciprocal Rank Fusion (RRF) Formulation
To combine the disjoint score spaces of cosine similarity ($[0.0, 1.0]$) and BM25 ($[0.0, \infty)$) without fragile manual weight tuning, the candidates are merged using Reciprocal Rank Fusion:

$$\text{RRF\_Score}(f) = \sum_{m \in \{\text{dense}, \text{sparse}\}} \frac{I(f \in L_m)}{k_{\text{rrf}} + \text{rank}_m(f)}$$

- $k_{\text{rrf}} = 60$ (The standard smoothing constant balancing top-rank dominance against long-tail discovery).
- $I(f \in L_m) = 1$ if fact $f$ appears in candidate list $L_m$, else $0$.
- $\text{rank}_m(f)$ is the 1-based index of fact $f$ in list $m$.

### 3.4 Filtering, Pruning & Cutoff Guard
1. **Cosine Cutoff Guard**: A fact must possess a dense cosine similarity $\ge \text{semantic\_similarity\_cutoff}$ (default `0.35`) to be admitted into the final candidate set, even if sparse BM25 produced a superficial keyword hit. This prevents keyword-matching hallucinations on semantically unrelated facts.
2. **Top-K Truncation**: Candidates passing the cutoff are sorted descending by `RRF_Score` and truncated to `settings.top_k_facts` (default `10`).

---

## 4. Observation Serialization & Scratchpad Formatting

Retrieved facts are serialized into a clean, structured observation string appended to the turn-local ephemeral scratchpad (`Role::Tool`):

### 4.1 Nominal Match Format
```text
Memory search results for 'database migration strategy' (3 facts found):
- [workdone] (2026-09-18) Migrated SQLite turns table to normalized v2 schema with idempotent session foreign keys.
- [objective] (2026-09-19) Optimize database connection pool to eliminate blocking IO on harness worker threads.
- [pitfall] (2026-09-20) Avoid holding database transaction locks across tokio await boundaries.
```

### 4.2 Empty Result Format
When no facts pass the similarity cutoff or the database is unpopulated:
```text
Memory search completed for 'database migration strategy'. No relevant facts found.
```
*Invariant: An empty search result is an observation, not an error. The model must explain or acknowledge the absence of facts gracefully.*

---

## 5. Execution State Transitions & Audio Coordination

### 5.1 Modular Voice Pipeline (`PipelineMode::Modular`)
1. **Intake & Intent**: Model invokes `search_memory(query, spoken_filler)`.
2. **Interim Filler Dispatch**: Harness validates `spoken_filler`, dispatches text to `TtsActor` as `AudioIntent::InterimFiller`, and emits pipeline transition `Thinking` $\to$ `InteractionState::Working`.
3. **Execution Isolation**: Hybrid RRF search executes off the router thread within `TOOL_EXECUTION_TIMEOUT = 10s`.
4. **Scratchpad Commit**: `ToolResult` is appended to the turn-local `scratchpad: Vec<ChatMessage>`. Invocations are recorded in `session_tool_calls`.
5. **Reentrant Pass**: Harness re-enters Phase 2 Step 4 of `execute_turn` with the updated scratchpad, producing the finalized response.
6. **Playback Handshake**: Response audio transitions `Working` $\to$ `Speaking` $\to$ `Ready`.

### 5.2 Error & Timeout Recovery
- If database read fails, ONNX embedding fails, or the operation exceeds the 10-second deadline:
  - The harness returns `ToolResult { is_error: true, content: "Memory search unavailable: <sanitized reason>" }`.
  - The turn is **never** aborted.
  - The model verbally informs the user that memory retrieval was temporarily unavailable and answers from general knowledge.