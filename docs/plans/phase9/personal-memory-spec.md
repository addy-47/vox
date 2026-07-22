# Master Cognitive Memory Specification: Personal & Working Memory Subsystems

**Document ID:** SPEC-COG-MEM-V4-FINAL  
**Status:** SPECIFICATION FROZEN / VALIDATED (V4 COGNITIVE ERA)  
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
  │ v3 Cognitive Era: Working + Personal (10 Categorized Flat Lists)
  └───────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼ [Dual-Defense: Relations & Self-Healing]
  ┌───────────────────────────────────────────────────────────────┐
  │ v4 Cognitive Era (Active): Directed Relations Graph (HITL)    │
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
*   **Subsystem Configuration:** Introduces a **Directed Relations Graph** where facts are linked via explicit relationship edges: `SUPPORTS`, `CONFLICTS`, `USER_SUPERSEDES`, and the newly specified `SIMILAR` and `MERGED` edges.
*   It implements **Multi-Tier Cosine Similarity Range Routing `[0.65 - 0.95]`** to bypass performance bottlenecks and exact-match NLI failures.
*   **Automatic Merge:** If the BGE-M3 cosine similarity between a new fact and an existing fact is exactly `1.0` (or has a Jaccard similarity of `1.0`), they are automatically merged, bypassing downstream NLI and similarity checks entirely.
*   It establishes **Conversational Self-Healing via Explicit Context Injection**, presenting unresolved similarities and contradictions directly to the active prompt RAG context so that the LLM can resolve them during natural user turns (Human-in-the-Loop).
*   It optimizes the compaction harness by stripping redundant system-level tasks and passing structured JSON blocks during differential extractions to prevent duplication.

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
    *   Play a local, deterministic transition audio asset (e.g., *"Give me a moment to organize our conversation"*). This asset must be loaded locally and never generated by an LLM, ensuring zero added latency.
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

## 5. Personal Memory Behavioral Contract

Personal Memory builds, relates, and retrieves structured user facts across 10 defined collections:
`Identity` · `Constraints` · `Preferences` · `Relationships` · `Skills` · `Projects` · `Experiences` · `Context` · `Tasks` · `Goals`

### 5.1 The 3-Tier Collection Taxonomy
To minimize embedding generation overhead and maximize database retrieval efficiency, the 10 collections are divided into three operational categories:

1.  **Foundational Core (`Identity`, `Constraints`):**
    *   *Contract:* Slow-moving, critical biological or behavioral constraints.
    *   *Embedding:* **Never embedded.**
    *   *Retrieval:* Loaded unconditionally and injected into every conversation turn.
2.  **Operational Core (`Context`, `Tasks`, `Goals`):**
    *   *Contract:* Active, dynamic session histories and actionable state trackers.
    *   *Embedding:* **Never embedded.**
    *   *Retrieval:* Loaded deterministically via vectorless chronological and status queries.
3.  **Semantic Profiles (`Preferences`, `Relationships`, `Skills`, `Projects`):**
    *   *Contract:* Long-term traits and social/project contexts.
    *   *Embedding:* **Individually embedded** using dense vectors.
    *   *Retrieval:* Retrieved semantically using user-query similarity searches.

### 5.2 Dynamic Compaction prompt Contract
To completely eliminate duplicate facts during extraction, subsequent compactions in a session must use a differential prompt harness:
1.  **First Compaction in Session:** The extraction task is "extract all." The LLM parses the raw turns and extracts all stated user facts into a structured JSON block.
2.  **Subsequent Compactions:** 
    *   The previously extracted facts are injected as a structured JSON block inside a `<current_personal_memory>` tag.
    *   The extraction task is dynamically mutated to a "differential" extraction.
    *   The LLM must compare the new raw turns against `<current_personal_memory>` and output **only** unique additions, updates, or explicit contradictions, emitting the finalized state of the JSON.
3.  **Narrative Chaining:** Session-level summaries are compiled into a cumulative narrative ledger (Ledger Summary $N = \text{Ledger Summary } N-1 + \text{Additions } N$) to preserve temporal progression.

### 5.3 Directed Relations Graph and Multi-Tier Cosine Range Routing
When a new semantic fact $F_{\text{new}}$ is extracted, it must be aligned against all active existing facts $F_{\text{existing}}$ within the same collection. To solve logical contradiction loops and cross-encoder performance bottlenecks, alignment must execute a **Multi-Tier Cosine Range Filter**:

1.  **The $O(1)$ Semantic Identity Match:**
    *   If the cosine similarity score $S$ between the BGE-M3 embedding of $F_{\text{new}}$ and $F_{\text{existing}}$ is exactly `1.0`, or if the normalized lexical Jaccard similarity score is exactly `1.0`, the facts are deemed identical.
    *   *Action:* The new fact is **automatically merged** into the existing record. The system updates the `created_at` timestamp of the existing fact to represent reinforcement, bypasses NLI entirely, and writes a `MERGED` relation edge. No duplicate fact is inserted.

2.  **Multi-Tier Routing Matrix:**

```text
               Cosine Similarity Score (S) between F_new and F_existing
                                     │
         ┌───────────────────────────┼───────────────────────────┐
         ▼                           ▼                           ▼
      S < 0.65                  0.65 <= S <= 0.95             S > 0.95
   [Neutral Zone]               [NLI Candidate Zone]      [Near-Duplicate Zone]
         │                           │                           │
         ▼                           ▼                           ▼
     Skip NLI.                 Run DeBERTa-v3 NLI.            Skip NLI.
     No Edge.              Contradiction >= 0.85?        Write SIMILAR Edge.
                           ├── Yes -> Write CONFLICTS    Both facts stay active.
                           └── No -> Check Entailment
                                     (>= 0.85 -> SUPPORTS)
```

*   **Range 1: $S < 0.65$ (Neutral Zone):** The facts are semantically disjoint. The system must skip NLI evaluation. No relationship edge is written.
*   **Range 2: $0.65 \le S \le 0.95$ (Candidate Zone):** The facts represent a potential logical overlap or contradiction. The pair must be passed to the DeBERTa-v3 Natural Language Inference (NLI) model:
    *   An NLI contradiction score $\ge 0.85$ must write an explicit `CONFLICTS` edge in the relationship graph.
    *   An NLI entailment score $\ge 0.85$ must write an explicit `SUPPORTS` edge in the relationship graph.
*   **Range 3: $S > 0.95$ (Near-Duplicate Zone):** The facts are lexically or semantically nearly identical. The system must bypass NLI evaluation (preventing false negatives on minor word reorderings or punctuation shifts) and automatically write a `SIMILAR` relationship edge. Both facts remain active in the database.

---

## 6. Prompt Assembly & Conversational Self-Healing

Prompt Assembly is responsible for dynamically reconstructing the active system prompt context block for each new turn.

### 6.1 Priority-Based Token Budgeting (15% Hard Cap)
The total injected personal memory context must never exceed a strict **15% hard cap of the overall context window**. This budget is split into two prioritized, isolated tiers:

1.  **Tier 1: Foundational & Operational Core (7% hard cap):**
    *   Loaded vectorlessly: unconditional `Identity` and `Constraints`, active `Tasks` and `Goals`, and Time-Windowed Chained `Context` paragraphs.
    *   *Overfill Guard:* If the core text exceeds the 7% threshold, chronological FIFO pruning must be applied to keep only the most recently updated operational facts, guaranteeing it never overflows the 7% cap.
2.  **Tier 2: Semantic Profiles (8% hard cap):**
    *   Loaded via semantic search interleaved using **Interleaved Round-Robin Selection**:
        *   Generate a query embedding and fetch the top $K=5$ candidates independently from `Preferences`, `Relationships`, `Skills`, and `Projects`.
        *   Select candidates sequentially: Preference 1, Relationship 1, Skill 1, Project 1, Preference 2, Relationship 2...
        *   Stop selecting the moment the cumulative token footprint hits the 8% limit.
        *   Sort the selected facts chronologically by their creation timestamp before injection.

### 6.2 Relative Time-Windowed Context Chaining (Trap 1 Prevention)
When starting a brand-new session, the system must automatically carry forward all raw text context summaries from previous sessions within a configurable time window (defaults to **12 hours**):
1.  **Dynamic Budget Loading:** Calculate the remaining tokens in the Tier 1 (7%) budget after loading Identity, Constraints, Tasks, and Goals.
2.  **Timeline Formatting:** Retrieve active context paragraphs within the window, sort them chronologically, and inject them into the prompt as a relative chronological timeline:
    ```markdown
    [Past Contexts within the Last 12 Hours]

    - 5 minutes ago (Session 2):
      User requested to set a 10-minute baking timer.

    - 1 hour ago (Session 1):
      Sarah prepared a gluten-free French apple tart. She discussed her grandmother Evelyn, noting it was lovely. She left to clean up.
    ```
3.  **Distant Memory Container:** If no contexts exist in the last 12 hours, but a prior context exists older than 7 days, retrieve only that single latest context and format it inside a `"Recollection (Distant Memory)"` container to let the LLM transition gracefully.

### 6.3 Conversational Self-Healing (HITL)
To resolve semantic overlaps and logical contradictions without silently destroying data, the memory system must execute the following Human-in-the-Loop (HITL) self-healing contract:

1.  **Dual Active State Maintenance:** Facts participating in unresolved `CONFLICTS` or `SIMILAR` relationships must remain active in the database and be retrieved by semantic search.
2.  **Metadata Injection Formatting:** During RAG context assembly, retrieved facts carrying active conflict or similarity edges must be formatted inside the `<user_profile>` system prompt with explicit relation headers:
    *   `- [Unresolved Conflict] "Fact A" CONFLICTS WITH "Fact B"`
    *   `- [Unresolved Similarity] "Fact A" is SIMILAR TO "Fact B"`
3.  **Conversational Awareness:** Surfacing these relations ensures the active conversational LLM has complete situational awareness, allowing it to naturally prompt the user for clarification during dialogue (e.g., *"Hey, I noticed you mentioned studying Spanish, but earlier you said you were studying Japanese. Are you doing both, or should we correct that?"*).
4.  **Graph Healing Commit:** When the user provides clarification, the resulting in-session compaction emits the resolution. The worker writes a `USER_SUPERSEDES` (for conflict resolution) or `MERGED` (for similarity merging) edge, and marks the losing fact status as `'superseded'`, cleanly resolving the graph and removing the superseded fact from future active turn retrievals.

---

## 8. Rules & Negative Constraints (Must Not Happen)

*   **No Synchronous Model Sweeps during Conversational Turns:** Under no circumstances may expensive embedding generation (BGE-M3) or NLI evaluation run synchronously during active user speech turns, ensuring voice pipeline latency is never impacted.
*   **No Direct Overwrites of Prior Facts:** To preserve chronological lineage, the system must never directly overwrite or hard-delete prior facts in the database. Updates must soft-delete or write a relationship edge, keeping the timeline fully auditable.
*   **No Silent Subsystem State Changes:** No background thread may modify active Working Memory states concurrently without completing atomically, preventing conversational context corruption under interruption.
