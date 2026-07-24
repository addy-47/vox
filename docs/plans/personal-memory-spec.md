# Master Cognitive Memory Specification: Personal & Working Memory Subsystems

**Document ID:** SPEC-COG-MEM-V5-DECOUPLED  
**Status:** SPECIFICATION FROZEN / VALIDATED (V5 DECOUPLED PIPELINE ERA)  
**Audience:** System Architects, Cognitive Engineers, and Developer Agents  

---

## 1. Concept & Scope

This specification governs the behavioral contract of the **Vox Hybrid Cognitive Memory System**. 

The memory system is responsible for maintaining context across real-time voice conversations and building a durable, self-healing model of the user. It is comprised of two coordinated subsystems:
1.  **Working Memory:** A transient, active memory system in RAM that manages real-time conversation turns, enforces token budgets, and executes low-latency context compactions.
2.  **Personal Memory:** A long-term, structured memory system that extracts, stores, relates, and retrieves personal traits, preferences, experiences, active tasks, goals, and session contexts.

### 1.1 Out of Scope
*   **Static World Knowledge:** General knowledge querying is delegated entirely to the foundational LLM; this spec only governs knowledge related directly to the user.
*   **Physical Vector Database Administration:** The low-level configuration of indexing algorithms or storage engine paging is handled by downstream database drivers and is excluded from this contract.
*   **Audio DSP & ASR/TTS Pipelines:** The transcription of voice to text (STT) and text to voice (TTS) are treated as clean, external stream interfaces.

---

## 2. Historical Evolution (The Cognitive Lineage)

To prevent regressions, maintain deep technical traceability, and provide future agents with a step-by-step roadmap to reproduce or test any version of the system, all implementations must respect the design decisions, failures, and architectural lessons accumulated across historical iterations:

```text
  ┌───────────────────────────────────────────────────────────────┐
  │ v1 Cognitive Era: Working + Episodic (Vector) + Semantic (NLP)│
  └───────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼ [Morph: NLP deprecated]
  ┌───────────────────────────────────────────────────────────────┐
  │ v2 Cognitive Era: Working + Episodic (Vector) + Personal (LLM)│
  └───────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼ [Absorption: Episodic deprecated]
  ┌───────────────────────────────────────────────────────────────┐
  │ v3 Cognitive Era: Working + Personal (10 Categorized Lists)   │
  └───────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼ [Dual-Defense: Relations & Self-Healing]
  ┌───────────────────────────────────────────────────────────────┐
  │ v4 Cognitive Era: Directed Relations Graph (HITL)             │
  └───────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼ [Clean Decoupled Architecture & Ephemerality]
  ┌───────────────────────────────────────────────────────────────┐
  │ v5 Cognitive Era (Active): Clean Decoupled 4-Phase Pipeline  │
  └───────────────────────────────────────────────────────────────┘
```

### 2.1 The v1 Cognitive Era (Pure Subsystems)
*   **Subsystem Configuration:** 
    *   **Working Memory:** A transient RAM-based rolling FIFO list of conversation messages.
    *   **Episodic Memory:** A separate chunk-based persistent vector store. Utilized `all-MiniLM-L6-v2` (384-dimensional dense vector) to embed 5-turn session chunks. Query similarity matching score threshold was set to `0.55`.
    *   **Semantic Memory:** A separate, rigid rule-based NLP entity extraction pipeline.
*   **Failures & Latency Bottlenecks:**
    *   *Coordinate Centroid Drift:* Searching dense multi-topic episode summaries using short, sparse turn-level user queries caused a dimensional mismatch. Cosine similarity scores hovered near the mean centroid, failing to fetch relevant episodes.
    *   *Brittle NLP:* Rule-based NLP entity extraction failed completely on natural, loose human dialogue, causing high processing latencies ($>400$ms) on CPU.

### 2.2 The v2 Cognitive Era (The Personal Memory Morph)
*   **Subsystem Configuration:**
    *   **Episodic Memory:** Shifted embedding model to `BGE-M3` (1024-dimensional dense vectors with Unit-L2 normalization). Similarity threshold raised to `0.65`.
    *   **Personal Memory (The Morph):** Rule-based NLP Semantic Memory was deprecated. Personal Memory was introduced. Rather than using a separate NLP model, extraction was delegated directly to the LLM during Working Memory compactions (via `COMPACTION_SYSTEM_PROMPT_V2`), emitting structured updates.
    *   **NLI Verification Engine:** Introduced a local, quantized INT8 local ONNX Natural Language Inference model (`cross-encoder/nli-MiniLM2-L6-H768` ~33M parameters, ~30MB footprint on disk) to evaluate logical connections (Entailment/Contradiction/Neutral) between new facts and candidates.
*   **Failures & Latency Bottlenecks:**
    *   *NLI CPU Pinning:* Processing every single extracted fact against $K=5$ candidates via local cross-encoder NLI pinned CPU cores, causing audio stuttering during live pipeline playback.
    *   *Fact Duplication:* No upstream filtering existed. The LLM continuously re-extracted identical or slightly reworded facts, bloating prompt context.
    *   *Micro-Session Context Erasure:* A naive temporal chaining rule that fetched only the immediate prior session summary meant a 10-second micro-session (e.g., "Set a timer") completely erased the rich, multi-topic context of a 20-minute discussion that occurred just minutes prior.

### 2.3 The v3 Cognitive Era (Episodic Absorption)
*   **Subsystem Configuration:**
    *   **Episodic Memory:** Fully deprecated as an independent subsystem. Long-term session histories were absorbed directly into Personal Memory under a dedicated operational `Context` collection. Linear vector searches of context summaries were completely replaced by **Time-Windowed Context Chaining** (vectorless chronological SQL queries).
    *   **Personal Memory (Category Consolidation):** The memory model was consolidated into a flat collection model of 10 explicit categories (`Identity`, `Constraints`, `Preferences`, `Relationships`, `Skills`, `Projects`, `Experiences`, `Context`, `Tasks`, `Goals`).
    *   **Two-Tier Budgeted Retrieval:** Introduced a strict 15% hard cap of the overall LLM context window, splitting context into Tier 1 (7% Foundational Core) and Tier 2 (8% Semantic Profiles).
    *   **NLI Cosine Pruning:** To resolve NLI CPU pinning, BGE-M3 cosine similarity was calculated first. If the score was $<0.82$, the NLI cross-encoder was bypassed, and the relationship was classified as `Neutral` immediately. This cut CPU pinning during sweeps by over 85%, reducing latency to $<150$ms on CPU.
*   **Failures & Latency Bottlenecks:**
    *   *Semantic Lossiness:* Compactions flattened previously extracted multi-dimensional categories into a single, lossy summary sentence, causing subsequent compactions to lose historical granularity.
    *   *Upstream Redundancy:* The compaction template repeated instructions across system and user turns, wasting tokens.
    *   *Automatic Suppression Defects:* In conflicts, the backend automatically marked the older fact as inactive behind the user's back. This caused silent context loss and left the user with no visibility or control over why their profile state shifted.

### 2.4 The v4 Cognitive Era (The Self-Healing Graph)
*   **Subsystem Configuration:** Introduces a **Directed Relations Graph** where facts are linked via explicit relationship edges: `SUPPORTS`, `CONFLICTS`, `USER_SUPERSEDES`, and `SIMILAR` and `MERGED` edges.
*   It implements **Multi-Tier Cosine Similarity Range Routing `[0.65 - 0.95]`** to bypass performance bottlenecks and exact-match NLI failures.
*   **Automatic Merge:** If the cosine similarity between a new fact and an existing fact is exactly `1.0` (or has a Jaccard similarity of `1.0`), they are automatically merged, bypassing downstream NLI and similarity checks entirely.
*   It establishes **Conversational Self-Healing via Explicit Context Injection**, presenting unresolved similarities and contradictions directly to the active prompt RAG context so that the LLM can resolve them during natural user turns (Human-in-the-Loop).

---

## 3. Cognitive Hardware Tiers

To ensure portability across hardware configurations, the memory system dynamically adapts its logical capability based on the active hardware tier:

| Tier | Name | Working Memory Behavior | Personal Memory Behavior |
| :--- | :--- | :--- | :--- |
| **Tier 1A** | Pure Local (Low RAM) | Strict transient RAM-based FIFO message truncation. | **Disabled.** No background worker, no durable database, no extraction, and no context injection. |
| **Tier 1B** | Pure Local (GPU) | Transient RAM with threshold-based compaction. | **Enabled.** Durable category database, local embedding generation, and local contradiction/entailment sweeps. |
| **Tier 2A/2B** | Hybrid (Remote LLM) | Transient RAM with threshold-based compaction. | **Enabled.** Durable category database, utilizes remote cloud models for structured extraction during compaction, and local embedding/NLI sweeps. |
| **Tier 3** | Stateful / Realtime | Stateful provider-managed KV cache syncing. | **Enabled.** Real-time tool-driven updates; LLM decisions dynamically trigger targeted graph reads and writes. |

---

## 4. Working Memory Behavioral Contract

Working Memory governs the active conversation context. It must ensure the system remains responsive, stays within safety budgets, and never drops user speech.

### 4.1 Message History and Token Budgeting
1.  **Strict Token Budget Enforcement:** Before any LLM turn, the system must compute the current active message token footprint.
2.  **Transition State Invocation:** When the active conversation token footprint exceeds a configured `critical_threshold`, the system must:
    *   Immediately suspend standard turn execution.
    *   Enter a dedicated context management state.
    *   Play a local, deterministic transition audio asset (e.g., *"Give me a moment to organize our conversation"*).
    *   Trigger the compaction process.
3.  **Barge-in Safety Contract:** If the user speaks while the system is in the context management state, the system must:
    *   Never discard the user's speech.
    *   Buffer the incoming transcription in a temporary queue.
    *   Once compaction completes, append the buffered turn to the newly compacted conversation history and proceed with standard turn execution.

### 4.2 The Compaction Contract
During compaction, the conversation history is condensed. The contract requires that:
1.  **The original user request is preserved:** Compaction must process the history up to the second-to-last turn, leaving the user's latest active prompt uncompacted so that it is responded to directly.
2.  **No concurrent mutations:** The active conversation history must remain read-only during the compaction thread run.

---

## 5. Version 5.0 (v5 Clean Decoupled Pipeline & Architecture)

To resolve logic leakage, fragmented deduplication, and code pollution across working memory, personal memory, and worker threads, the **v5 Cognitive Architecture** strictly enforces single-responsibility modules and a clean 4-phase orchestrated execution pipeline.

### 5.1 Single-Responsibility Module Boundaries

Every file in the memory subsystem must govern exactly one domain. Logic mixing across boundaries is strictly forbidden:

```text
app/src-tauri/src/
├── core/
│   └── constants.rs       <-- Pure Constants: Prompt templates, schema JSON keys, static role strings.
└── services/memory/
    ├── embedder.rs        <-- Pure Vector Generator: Text string -> Float vector embeddings (384-dim).
    ├── nli.rs             <-- Pure NLI Model: Cross-encoder ONNX evaluation, raw probabilities, NLI thresholds.
    ├── deduplication.rs   <-- Pure Deduplication Utility: Jaccard similarity & Cosine hard-delete logic.
    ├── retrieval.rs       <-- Pure Retrieval: Candidate fact lookups & User query RAG context assembly.
    ├── ingestion.rs       <-- Pure Extraction: Working memory compaction triggers & raw LLM JSON parsing.
    ├── orchestrator.rs    <-- Pure Pipeline Orchestrator: Sequentially drives Phase 1 -> Phase 2 -> Phase 3 -> Phase 4.
    └── persistence/       <-- Pure Database Access (SQLite/Turso):
        ├── schema.rs      <-- Tables: memory_facts, memory_facts_vectors, memory_relations.
        └── repository.rs  <-- Atomic SQL transactions: Reads, inserts, soft-deletes, hard-deletes.
```

#### Module Responsibility Matrix:
1. **`constants.rs`**: Contains all static system prompts (`COMPACTION_SYSTEM_PROMPT`), schema definitions, and constant configuration defaults.
2. **`embedder.rs`**: Converts text strings into dense float vector embeddings. Contains zero database or NLI logic.
3. **`nli.rs`**: Manages the local ONNX DeBERTa-v3 cross-encoder. Evaluates string pairs and outputs raw classification probabilities (`Entailment`, `Contradiction`, `Neutral`). Contains zero SQL or retrieval code.
4. **`deduplication.rs`**: Stateless utility module that executes **Jaccard Word-Set Overlap Similarity** and **Cosine Similarity Threshold Checks** ($>0.999$).
5. **`retrieval.rs`**: Responsible for all retrieval operations:
   * *Candidate Retrieval*: Fetching existing active facts in a collection for deduplication and NLI scoring.
   * *RAG Query Retrieval*: Tier 1 Foundational/Operational loading, Tier 2 Semantic Profile vector search, Interleaved Round-Robin selection, and Edge Resolution (`resolve_edges`).
6. **`ingestion.rs`**: Parses incoming user conversation turns, evaluates working memory compaction thresholds, builds compaction prompts, and parses raw LLM JSON responses.
7. **`persistence/`**: The exclusive layer for permanent database interactions. Executes atomic SQLite transactions (`BEGIN TRANSACTION ... COMMIT`) for facts, vectors, and relation graph edges.
8. **`orchestrator.rs`**: The master memory pipeline manager that executes the 4 sequential phases of memory processing.

---

### 5.2 Ephemerality Contract (`Tasks` & `Context` with Staged DB WAL)

A critical architectural flaw in prior versions was treating intermediate `Tasks` and `Context` entries as permanent facts during intra-session compactions, leading to database pollution and duplicate task entries.

**The v5.1 Ephemerality & Staging Contract:**
1. **Intra-Session Ephemerality with Crash-Resilient Staging**:
   * During active conversation sessions, intermediate `Tasks` and `Context` summaries produced by LLM compactions are written to the database queue (`personal_memory_queue`) with `status = 'staged'`.
   * Staged items act as a Write-Ahead Log (WAL) for crash safety. They are strictly **gated** and must **NOT** enter the embedding generator, NLI cross-encoder, or active context retrieval during intra-session turns.
   * On subsequent intra-session compactions, newly extracted task lists overwrite or update prior staged entries for that session.
2. **Goals Processing (Non-Ephemeral)**:
   * `Goals` are treated as long-lived operational facts. Unlike `Tasks` and `Context`, `Goals` bypass staging and are enqueued directly as `status = 'pending'`, undergoing immediate deduplication, vector embedding, and relation mapping.
3. **Session-End Finalization**:
   * **Only when a session explicitly terminates** (`SessionEnd` event), the final state of `Tasks` for that session transitions from `status = 'staged'` to `status = 'pending'`, triggering embedding, deduplication, and persistence into `memory_facts`.
   * All intermediate, superseded staged task entries from earlier turns in the session are purged without polluting `memory_facts`.
   * The final narrative `Context` paragraph for the session is written directly to `memory_facts` (`type = 'operational'`, `collection = 'Context'`, `status = 'active'`) without vector embedding.

---

### 5.3 The 4-Phase Memory Processing Pipeline

All extracted facts must flow sequentially through the 4 phases managed by `orchestrator.rs`:

```text
   [ Raw Extracted Facts from Ingestion ]
                    │
                    ▼
┌────────────────────────────────────────────────────────┐
│ PHASE 1: Dual-Defense Fast Hard Deduplication         │
│  - Step 1A: Parallel Jaccard Overlap (Score = 1.0)     │
│             -> Delete older fact permanently.          │
│  - Step 1B: Generate Vector Embeddings (embedder.rs)   │
│  - Step 1C: Cosine Hard Match Check (Score > 0.999)    │
│             -> Delete older fact permanently.          │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ PHASE 2: Candidate Retrieval & NLI Classification      │
│  - Step 2A: Fetch Candidates in Collection (retrieval)│
│  - Step 2B: Route Candidate Zone (0.65 <= S <= 0.95)   │
│  - Step 2C: Run Cross-Encoder Classification (nli.rs) │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ PHASE 3: Relation Mapping & Graph Persistence         │
│  - Step 3A: Map NLI Probabilities to Graph Relations   │
│             (Entailment -> SUPPORTS)                   │
│             (Contradiction -> CONFLICTS)               │
│             (S > 0.95 -> SIMILAR)                      │
│  - Step 3B: Atomic Database Write Transaction         │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ PHASE 4: RAG Retrieval & Edge Resolution (Query Time)  │
│  - Step 4A: Tier 1 + Tier 2 Budgeted Context Search    │
│  - Step 4B: Resolve Supersedes & Pull Supporting Facts │
│  - Step 4C: Format <user_profile> with Self-Healing    │
└────────────────────────────────────────────────────────┘
```

#### Phase 1: Dual-Defense Fast Hard Deduplication
1. **Step 1A — Parallel Jaccard Word-Set Overlap**:
   * Before calling any embedding model or GPU/ONNX resource, run parallel Jaccard word similarity across all extracted facts and existing active facts in all collections.
   * Formula: $J(A, B) = \frac{|A \cap B|}{|A \cup B|}$ on alphanumeric token sets.
   * **Rule**: If $J(A, B) == 1.0$ (100% word overlap), the older fact is **permanently deleted (`hard delete`)** from memory. No vector embedding or NLI is generated for the duplicate.
2. **Step 1B — Vector Embedding Generation**:
   * For facts surviving Step 1A, call `embedder.rs` to generate 384-dimensional dense float vectors.
3. **Step 1C — Cosine Hard Deduplication**:
   * Compute Cosine Similarity between new embeddings and existing candidate embeddings.
   * **Rule**: If Cosine Similarity score $S > 0.999$, the older fact is **permanently deleted (`hard delete`)**.

#### Phase 2: Candidate Retrieval & NLI Classification
1. **Candidate Retrieval**: For each surviving fact, call `retrieval.rs` to fetch existing active facts in the same collection.
2. **Multi-Tier Range Routing**:
   * **$S < 0.65$ (Neutral Zone)**: Bypassed immediately. No NLI evaluation.
   * **$0.65 \le S \le 0.95$ (Candidate Zone)**: Passed to `nli.rs` for DeBERTa-v3 cross-encoder classification.
   * **$S > 0.95$ (Near-Duplicate Zone)**: Bypasses NLI to prevent false negatives. Automatically marked for `SIMILAR` relation mapping.
3. **NLI Evaluation (`nli.rs`)**:
   * Evaluates candidate pairs and outputs raw probabilities for `Entailment`, `Contradiction`, and `Neutral`.

#### Phase 3: Relation Mapping & Graph Persistence
1. **Relation Mapping**:
   * `Entailment` ($\ge 0.80$ probability) $\rightarrow$ Maps to `SUPPORTS` edge.
   * `Contradiction` ($\ge 0.80$ probability) $\rightarrow$ Maps to `CONFLICTS` edge.
   * Near-Duplicate ($S > 0.95$) $\rightarrow$ Maps to `SIMILAR` edge.
   * Merged Duplicate ($S == 1.0$ or $J == 1.0$) $\rightarrow$ Maps to `MERGED` edge.
2. **Atomic Persistence (`persistence/repository.rs`)**:
   * All surviving facts, vector embeddings, and mapped relation edges are written to SQLite inside a single atomic transaction:
     ```sql
     BEGIN TRANSACTION;
     INSERT INTO memory_facts ...;
     INSERT INTO memory_facts_vectors ...;
     INSERT INTO memory_relations ...;
     COMMIT;
     ```

#### Phase 4: RAG Retrieval, 2-Tier Allocation & Edge Resolution (User Query Time)
1. **Active Session Exclusion Guard**:
   * All Tier 1 and Tier 2 SQL queries strictly enforce `AND (session_id = '' OR session_id != ?)`.
   * Context retrieval **strictly fetches facts from PREVIOUS sessions**, as working memory is the sole authoritative context for the active live session. This eliminates 100% of context duplication between system prompt `<user_profile>` and working memory turns.
2. **System Prompt `<memory_manifest>` Record Header**:
   * Prepend an active record count manifest at the top of `<user_profile>`:
     ```xml
     <memory_manifest total_active_facts="142">
       Identity: 2 | Constraints: 3 | Preferences: 24 | Skills: 45 | Projects: 18 | Experiences: 50
     </memory_manifest>
     ```
   * Signal to downstream agentic tool-calling modules that additional un-injected profile facts exist in SQLite.
3. **2-Tier Semantic Memory Allocation Algorithm (`retrieval.rs`)**:
   * **Tier 2A — Guaranteed Anchor Floor ($K_{\text{base}}$ per collection)**:
     * For each of the 5 semantic collections (`Preferences`, `Relationships`, `Skills`, `Projects`, `Experiences`), select top $K_{\text{base}}$ candidates ($K_{\text{base}} = 5$).
     * Preserves the user's personal identity anchor across all 5 dimensions regardless of topic shifts.
   * **Tier 2B — Global Similarity Competitive Pool ($\theta \ge 0.65$)**:
     * Collect all remaining candidates across ALL 5 semantic collections having a similarity score $\ge 0.65$.
     * Sort globally by cosine similarity score descending.
     * Fill all remaining 8% Tier 2 context window budget dynamically based on pure relevance, allowing deep topic concentration.
4. **Edge Resolution (`resolve_edges`)**:
   * Recursively resolves `USER_SUPERSEDES` pointers to replace superseded facts with their newest active versions.
   * Pulls supporting details for active `SUPPORTS` edges.
5. **Self-Healing Context Assembly**:
   * Surfacing unresolved `CONFLICTS` and `SIMILAR` relations inside the `<user_profile>` system prompt so the active LLM can naturally clarify contradictions with the user during conversation turns.

---

## 6. Edge Cases, Optimizations & Worker Governance

1. **30-Second Minimum Idle Debounce Window (`MIN_IDLE_DEBOUNCE_SECS = 30`)**:
   * The background `vox-memory-worker` tracks continuous pipeline idle duration (`idle_since`).
   * Background queue orchestration is suppressed on short pauses (e.g. 1–5 second gaps between turns) and only triggers after 30 seconds of continuous pipeline idle.
2. **ONNX Interrupt Safety & CPU/GPU Contention Guard**:
   * If `MemoryWorkerEvent::PipelineActive` arrives while a queue item is being processed, an atomic `cancel_flag` immediately aborts in-flight BGE-M3 embedding generation or DeBERTa NLI cross-encoder inference.
   * Interrupted items revert status from `processing` back to `pending` to be re-tried during the next true idle window, completely eliminating CPU/GPU contention during speech capture.
3. **Multilingual Combining Mark (Matra) Preservation**:
   * Tokenization in `deduplication.rs` explicitly targets ASCII and Devanagari punctuation (`c.is_ascii_punctuation() || c == '।'`) instead of `!c.is_alphanumeric()`.
   * Preserves Devanagari vowel marks/matras (`ॉ`, `्`, `ा`, `े`, `ै`), guaranteeing accurate Jaccard token set overlap scores for Hindi and Hinglish.
4. **Parallel Jaccard Optimization**: Jaccard similarity is purely lexical and CPU-bound. Executing it in parallel across candidate sets reduces pre-embedding evaluation time to $< 2$ms, eliminating unnecessary ONNX embedding calls.
5. **NLI Fallback & Resilience**: If the local ONNX NLI model fails to load or encounters an inference exception, candidate pairs fall back gracefully to `Neutral` without crashing the application or corrupting SQLite state.
6. **Cycle Detection in Pointer Swaps**: When resolving `USER_SUPERSEDES` graph chains, depth is capped at 10 hops with a `HashSet` visited check to prevent infinite loops from malformed database state.
7. **Private Mode Isolation**: When Private Mode is active, all pipeline events (`PersonalFactsReady`, `SessionEnd`) are dropped in memory immediately. Zero disk writes or vector embeddings are performed.

---

## 7. Rules & Negative Constraints (Must Not Happen)

*   **No Synchronous Model Sweeps during Conversational Turns:** Under no circumstances may expensive embedding generation or NLI evaluation run synchronously during active user speech turns.
*   **No Intra-Session Task Pollution:** Intermediate tasks produced during active sessions must NEVER be written to disk or embedded until session end.
*   **No Active-Session RAG Duplication:** RAG context retrieval must NEVER fetch facts created during the active live session; active session context belongs exclusively to Working Memory.
*   **No Logic Mixing Across Files:** Functionality must strictly adhere to the single-responsibility module boundaries defined in Section 5.1.
*   **No Un-Transacted Graph Writes:** All database writes (facts + vectors + relations) must be wrapped in a single SQLite transaction to guarantee database consistency.
