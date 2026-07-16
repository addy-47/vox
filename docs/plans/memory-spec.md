# Vox Memory Specification

> **Status:** Draft · Each subsystem is designed, implemented, benchmarked, and finalized independently before the next is introduced. Future subsystems are not discussed until their design phase.

## Design Principles

- Build one subsystem at a time; each independently testable with measurable latency/memory budgets.
- No subsystem enters the default runtime until benchmarked.
- The memory pipeline must never block the real-time voice pipeline.
- Architecture must support dynamic degrade/upgrade across hardware tiers (Tier 2 is recommended default).

## Hardware Tiers

| Tier | Configuration | Memory Systems |
|------|---------------|---------------|
| **1A** | 8GB Pure Local (no GPU) | Working + small Personal · no episodic retrieval |
| **1B** | Pure Local (GPU) · *Recommended* | Working + Personal + Episodic |
| **2A** | Hybrid: Remote LLM + Local Audio · *Recommended/No-cost* | Working + Personal + Episodic · remote extraction permitted |
| **2B** | Hybrid: Cloud LLM + Local Audio · *Recommended/Default* | Working + Personal + Episodic · cloud models do Memory Extraction during compaction |
| **3** | Realtime S2S WebSocket · *Best performance* | Provider Working + Personal + Episodic · provider tool calls update Personal immediately |

## Memory Taxonomy

Vox separates memory into three independent cognitive systems, each implemented separately:

| System | Status | Role |
|--------|--------|------|
| **Working Memory** | 🟢 Completed | Active conversation |
| **Episodic Memory** | 🟢 Completed | Historical conversations |
| **Personal Memory** | 🟡 Implemented (v1) | Evolving user model |

Working Memory handles the active conversation. Episodic Memory preserves historical conversations. Personal Memory builds an evolving user model — it is **not** a knowledge graph, and its storage is intentionally abstract/unspecified.

## Development Process

Each subsystem follows: Define → Design → Implement → Benchmark → Validate → Freeze → Next. No future subsystem influences the current one unless a hard architectural dependency exists.

---

# Working Memory

**Status:** 🟢 Completed · Transient, session-scoped; maintains a valid context window without exceeding the model limit while preserving real-time voice responsiveness. Not responsible for long-term persistence, retrieval, embeddings, or knowledge graphs.

## Responsibilities

**Is responsible for:** maintaining the active conversation · tracking token usage · managing the context budget · constructing the LLM prompt · maintaining provider-specific context state · performing context maintenance.

**Is not responsible for:** Episodic Memory · Personal Memory · embedding generation · Memory Extraction · persistent storage.

## Conversation Manager

A dedicated `ConversationManager` is the single source of truth for the active conversation. Responsibilities: maintain history · track tokens · monitor budget · select provider strategy · perform maintenance · build the final LLM request. No other subsystem may modify the active conversation directly.

## Conversation State

Maintained entirely in runtime memory:

```text
Conversation
├── System Prompt
├── Conversation History
├── Tool Results
├── Runtime Metadata
└── Provider State
```

Storage is provider-independent; synchronization is provider-specific.

## Provider Strategy

| Mode | Examples | Behavior |
|------|----------|----------|
| **Stateless** | OpenAI, Gemini, Anthropic, OpenAI-compatible | Manager builds the full prompt each request; no state in provider |
| **Stateful** | llama.cpp, future embedded engines | Provider holds KV cache; manager owns logical conversation + sync of provider context state |

Working Memory must never assume all providers behave identically.

## Context Budget

Before every inference the manager computes: current usage · max context · reserved generation budget · remaining context. The **runtime** (not the LLM) enforces these limits; thresholds are runtime-configurable.

## Context Maintenance

| Policy | Trigger | Characteristics |
|--------|---------|----------------|
| **Threshold** (high) | Exceeds critical threshold | Mandatory · synchronous · blocks next inference · guarantees budget validity |
| **Opportunistic** (low) | Pipeline idle + exceeds soft threshold | Optional · background · cancelable · never blocks voice |

The current user request is never processed until Threshold Maintenance completes; the response is always generated from the updated conversation. Threshold always takes precedence; if interrupted by new user activity the opportunistic task is cancelled without modifying the conversation.

## Transition State & Speech

During Threshold Maintenance Vox enters the global `ContextManaging` state and immediately plays a deterministic transition message (e.g. *"Give me a moment while I organize our conversation."*). Messages are runtime assets, **never LLM-generated**, guaranteeing zero added latency, determinism, localization, and consistent UX.

```text
Idle → Listening → Thinking → Speaking → ContextManaging
```

## Context Maintenance Flow

```text
STT Final → ConversationManager → Critical Threshold?
                                  ├─ No → Continue
                                  └─ Yes → Enter ContextManaging → Play Transition Speech
                                          → Perform Maintenance → Rebuild Conversation
                                          → Generate Response → TTS
```

The original user request is preserved throughout; the LLM response is generated only after maintenance completes.

## Barge-in During Context Management

On a new `SpeechStart` during `ContextManaging`: the utterance is never discarded, the conversation is never mutated concurrently, and maintenance completes atomically. A temporary hold queue buffers the turn until maintenance finishes, then appends it. Guarantees: no dropped speech · no concurrent mutation · deterministic under interruption.

## Design Constraints

Never exceed the context budget · never reject a request for context exhaustion · never corrupt state under concurrent events · never block voice except mandatory Threshold Maintenance · always respond from the maintained (not pre-maintenance) conversation · always preserve the original request.

## Out of Scope

Episodic Memory · Personal Memory · embedding generation · retrieval · vector search · Memory Extraction · background memory consolidation.

---

# Episodic Memory Specification

> **Status:** Draft · Persistent across sessions; extends Working Memory by recalling past conversations after compaction/forgetting. Completely independent from Personal Memory.

**Purpose:** answers *"What have we talked about before?"* — stores historical summaries, retrieves when relevant. Not responsible for durable facts or user profiles.

**Responsibilities — is:** persist summaries · maintain chronological history · retrieve relevant sessions · supply LLM context.
**Responsibilities — is not:** Working Memory · context-window management · Memory Extraction · user profile construction.

**Design Principles:** never store raw conversations (summaries only) · retrieval never blocks realtime audio · retrieval within a fixed token budget · one memory per historical session · architecture adapts to runtime tier.

## Storage Unit & Record

```text
Session → Working Memory Compaction → Summary → Embedding → Vector Database
```
Only finalized compaction summaries are embedded (never raw turns).

```text
Episode ── Session ID ── Summary ── Embedding ── Timestamp ── Metadata
```
Metadata: duration · summary token count · creation timestamp. No extracted facts stored here.

## Ingestion Pipeline

Only **meaningful** conversations are stored; generic ones are discarded.

```text
Conversation Completed → Query Classifier ─┬─ Generic → Skip
                                           └─ Meaningful → Compaction Summary → Embedding → Store Episode
```

## Retrieval

```text
Current User Query → Query Embedding → Vector Search → Diversify By Session → Token Budget Filter → Inject Into Prompt
```

**Session Diversification** (not plain Top-K): retrieve a larger candidate set → group by Session ID → keep only the highest-scoring summary per session → return Top-K. Prevents one long conversation from dominating.

**Context Budget:** Episodic has an independent hard budget (e.g. ≤20% of context, below Working Memory priority). If retrieved summaries exceed it, keep highest relevance and discard the rest.

## Runtime Behavior

| Tier | Behavior |
|------|----------|
| 1A | No Episodic; Working Memory only |
| 1B | Automatic retrieval: `User Query → Retrieve Episodes → Inject → LLM` |
| 2A / 2B | Same as 1B; remote/cloud LLM with local embeddings + local vector DB |
| 3 | Tool-driven retrieval: `LLM decides needed → Episode Retrieval Tool → Relevant Sessions → Continue` (avoids per-turn retrieval during streaming) |

**Retrieval Tool (Tier 3):** input = natural-language query; output = Episode list. Respects max episodes · max token budget · one summary per session.

**Failure Behavior:** on retrieval failure, continue normally, no synchronous retry, no blocked generation — the LLM simply receives Working Memory.

## Design Constraints

Never store raw conversations · never exceed budget · never return multiple summaries per session · never block realtime audio · never modify Working Memory · never duplicate Personal Memory responsibilities.

## Out of Scope

User facts · preferences · user profiles · Memory Extraction · profile generation · long-term knowledge storage — these belong to **Personal Memory**.

---

# Memory Philosophy

- Vox does **not** try to remember everything — only what improves future conversations.
- Memory exists to make Vox feel continuous: one relationship across sessions, not disconnected chats.
- The **user**, not the world's knowledge, is the center of the memory system.
- Personal Memory stores *evolving knowledge about the user*, not static facts about the world.

This is the guiding principle for all future memory work.

---

# Personal Memory

**Status:** 🟡 Designing (v2) · Purpose: *Maintain a temporal, user-centric memory system that continuously learns about the user without blocking the real-time voice pipeline.* It answers *"What do I know about this user?"* rather than *"What facts exist?"*

## Design Principles

* **User-Centricity:** Memory is about the user, not the conversation.
* **Immutability:** The runtime is strictly append-only; it never modifies or deletes existing memory records.
* **User Sovereignty:** The user has the final authority and can inspect, edit, merge, or delete any memory via the UI.
* **Zero Voice Latency:** Personal memory extraction and processing are fully asynchronous; the voice pipeline never blocks.
* **Extraction Efficiency:** Memory extraction runs during Working Memory compaction and introduces zero additional LLM calls.
* **Separation of Concerns:** Extraction, similarity search, semantic classification (NLI), and graph operations are decoupled.

---

## Memory Architecture

```text
Conversation ──► Working Memory ──► Compaction
                                        ├─► Episodic Summary ──► Vector Database
                                        └─► Personal Facts ──► SQLite Job Queue
                                                                   │
                                                                   ▼ (Asynchronous)
  Memory Graph ◄── Runtime ◄── NLI Model ◄── Candidate Retrieval ◄── BGE-M3 Embedder
```

---

## Components & Decoupled Responsibilities

| Component | Responsibility | Performance Target |
| :--- | :--- | :--- |
| **LLM Extractor** | Extract raw facts and assign them to fixed collections during compaction | $0$ additional runtime calls |
| **BGE-M3 Embedder** | Generate 1024-dim Unit-L2 normalized dense embeddings for new facts | $<5$ ms per fact |
| **Candidate Retriever** | Query Turso DB for Top-K candidates within the same collection | $<10$ ms per query |
| **NLI Cross-Encoder** | Perform logical classification (Entailment/Contradiction/Neutral) between pairs | $<10$ ms per pair (CPU) |
| **Runtime Engine** | Process queue, manage SQLite graph relations, select current values | $<2$ ms overhead |
| **Settings & UI** | Configure thresholds, inspect collections, resolve conflicts manually | Client-side reactive |

---

## Memory Collections

The memory taxonomy consists of ten fixed, predefined categories. The runtime never creates or mutates collections:

`Identity` · `Preferences` · `Experiences` · `Projects` · `Goals` · `Tasks` · `Relationships` · `Skills` · `Devices` · `Locations`

---

## Immutable Memory Record Schema

Every memory fact is an immutable record. Once created, the backend never alters it.

| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | TEXT (PK) | Unique identifier (e.g. `mem_143`) |
| `collection` | TEXT | Predefined collection category (e.g., `Preferences`) |
| `fact` | TEXT | Plain English extracted fact |
| `source` | TEXT | Origin of the fact: `LLM`, `User`, or `Import` |
| `created_at` | INTEGER | Millisecond epoch timestamp |
| `session_id` | TEXT | Source conversation session ID |
| `turn_id` | TEXT | Source turn ID where the fact was spoken |
| `embedding_id` | TEXT | Associated vector embedding ID in `episodes` or personal index |
| `metadata` | TEXT (JSON) | Optional structured metadata |

---

## Graph Relations & Immutability

### A. Runtime Graph Relations
Since raw records are immutable, the state of the user's mind is represented as a directed graph. The runtime maintains relations between records:

| Relation | Description | Graph Meaning |
| :--- | :--- | :--- |
| `SUPPORTS` | Two facts reinforce the same information | Directed: $B \longrightarrow A$ (Increases confidence) |
| `CONFLICTS` | Two facts logically disagree | Bidirectional: $A \longleftrightarrow B$ (Triggers conflict flag) |
| `USER_SUPERSEDES` | User-edited version of a memory | Directed: $B_{\text{new}} \longrightarrow A_{\text{old}}$ (Shadows older version) |

### B. User Edits Flow
If a user edits a memory, a **new immutable record** is written, and the runtime writes a `USER_SUPERSEDES` edge from the new record to the old. The historical record remains intact.

---

## Fact Ingestion & Asynchronous Pipeline

Personal memory updates are triggered exclusively during **Working Memory Compaction** to avoid extraneous LLM API expenses.

### 1. Extractor Schema
During compaction, the LLM processes history using `COMPACTION_SYSTEM_PROMPT_V2` and returns a structured JSON payload:
```json
{
  "summary": "...",
  "personal_memory": {
    "Identity": ["Lives in San Francisco."],
    "Preferences": ["Prefers dark mode UI."],
    "Projects": ["Working on a Rust desktop voice engine."]
  }
}
```

### 2. The Asynchronous Processing Pipeline
Every extracted fact is transformed into an independent, asynchronous background task:

```text
Raw Fact ──► SQLite Job Queue ──► BGE-M3 Embedder ──► Candidate Retrieval ──► NLI Classification ──► Graph Update
```

1. **Persistent SQLite Queue:** To prevent memory loss due to sudden app shutdown, jobs are appended to a `personal_memory_queue` table in SQLite.
2. **BGE-M3 Embedding:** Generates a 1024-dim vector for the new fact.
3. **Collection-Restricted Retrieval:** Queries the database for candidates *only within the same collection* using cosine similarity. To prevent $O(N)$ computational blowup in the NLI stage, the candidate set is capped at **$K \le 5$** (ordered by highest similarity).
4. **NLI Classification:** The new fact and each candidate are evaluated as a pair using a local NLI model.
5. **Runtime Resolution & Graph Update:** The runtime inserts the new fact, establishes edges based on NLI output, and updates the queue job status.

---

## The Natural Language Inference (NLI) Layer

The NLI layer determines the exact logical connection between a newly extracted fact and existing candidate records.

### A. Model Specification
* **Model:** `cross-encoder/nli-MiniLM2-L6-H768` (Quantized to `INT8` ONNX)
* **Parameters & Footprint:** ~33 Million parameters; **~30 MB** on disk.
* **Performance:** $<8$ ms CPU inference per pair.
* **Accuracy:** ~88.4% SNLI/MNLI average.
* **Input Structure:** Tokenized pair `[CLS] New Fact [SEP] Candidate Memory [SEP]` processed as a single sequence.
* **Output Logits:** 3-class classification probabilities:
  * **Index 0 (Contradiction):** Threshold $\ge 0.85$ triggers a `CONFLICTS` relation.
  * **Index 1 (Entailment):** Threshold $\ge 0.85$ triggers a `SUPPORTS` relation.
  * **Index 2 (Neutral):** Triggers no logical relation (considered a distinct, independent fact).

### B. Hardware Tier Fallbacks (Dynamic Degrade)
* **Tier 1B / CPU Throttled:** If local NLI execution latency spikes or causes CPU heating, the runtime dynamically degrades to high-confidence Cosine Similarity threshold matching:
  * Similarity $\ge 0.90$: Auto-supports.
  * Similarity $[0.75, 0.90)$: Assumed Neutral/Distinct.
  * Similarity $<0.75$: Ignored.

---

## Conflict & Current Value Resolution

Conflicting records are never auto-merged or deleted by the machine.

```text
"Lives in Delhi."  ◄───[ CONFLICTS ]───►  "Lives in Bangalore."
```

### 1. Runtime Selection Policy
When formatting context for the LLM, if a `CONFLICTS` relation exists and remains unresolved:
1. Prefer user-supervised or user-superseded records.
2. If both are LLM-extracted, resolve using the **most recent** timestamp.
3. Suppress conflicting alternatives from the active prompt to prevent LLM hallucinations.

### 2. UI Exposure
The client UI highlights active conflicts, allowing the user to:
* **Select One:** Mark one as authoritative (creates a `USER_SUPERSEDES` edge).
* **Merge:** Edit both into a unified fact (e.g. *"Lives in Bangalore but frequently visits Delhi."*).
* **Delete:** Prune the inaccurate record.

---

## Retrieval Strategy

Personal Memory context injection employs collection-aware hybrid retrieval to ensure high semantic accuracy and token efficiency.

### A. Collection Strategies
Each collection possesses a configurable retrieval strategy:

| Collection | Strategy | Injection Behavior |
| :--- | :--- | :--- |
| **Identity** | `always` | Formatted into system prompt on every turn. |
| **Preferences** | `vector` | Searched dynamically using BGE-M3 query embedding. |
| **Experiences** | `vector` | Searched dynamically using BGE-M3 query embedding. |
| **Projects** | `vector` | Searched dynamically using BGE-M3 query embedding. |
| **Goals`** | `vector` | Searched dynamically using BGE-M3 query embedding. |
| **Tasks** | `tool` | Excluded from default turns; retrieved only if LLM executes `get_tasks` tool. |
| **Relationships** | `vector` | Searched dynamically using BGE-M3 query embedding. |
| **Skills** | `vector` | Searched dynamically using BGE-M3 query embedding. |
| **Devices** | `tool` | Retrieved only via system `get_user_devices` tool calling. |
| **Locations** | `tool` | Retrieved only via system `get_user_location` tool calling. |

### B. Collection-Aware Vector Retrieval Algorithm
To prevent a single collection (e.g. `Experiences`) from crowding out other vital user preferences, vector retrieval applies a strict round-robin interleaved schema:

```text
User Query ──► Local BGE-M3 Embedder (1024-dim Vector)
                  │
                  ├──► Preference Vector Search ──► Top X Candidates
                  ├──► Project Vector Search    ──► Top X Candidates
                  │
                  ▼
          Merge Candidates ──► Restrict to Top Y Overall ──► Sort by Timestamp (Recency) ──► Inject Top K Final
```

---

## Context Allocation Budget

Personal Memory limits conform to a dynamic, configurable **maximum upper bound (hard limit)** of the overall system prompt context allocation, rather than statically reserving or pre-allocating token slots. If the actual retrieved facts are smaller than the budget, only those tokens are consumed. If no facts are retrieved, the consumption is 0 tokens.

| Component | Default Max Share | Meaning |
| :--- | :--- | :--- |
| **System Prompt (Core)** | Max 10% | Core instruction set. Typically occupies 3k-5k tokens regardless of context window size. |
| **Personal Memory (Current Profile)** | Max 8% | Dynamic upper limit for personal facts injection. |
| **Episodic Memory (RAG)** | Max 15% | Dynamic upper limit for episodic summary chunks. |
| **Working Memory (Chat History)** | Max 67% | Balance of context window reserved for raw active dialog history. |

---

## Config-Driven Settings (`settings.rs` Integration)

All configuration options are defined in the Rust backend inside `app/src-tauri/src/core/settings.rs` within the `MemorySettings` struct (which nests under the central `VoxSettings` master config). This layout ensures standard JSON serialization/deserialization to `settings.json` and seamless exposure to the HTML/JS frontend UI via existing Tauri IPC commands.

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct MemorySettings {
    /// Enable Episodic Memory subsystem.
    pub episodic_enabled: bool,
    /// Enable background memory worker thread.
    pub bg_worker_enabled: bool,
    /// Number of top historical session summaries to retrieve (RAG parameter).
    pub top_k: u32,
    /// Cosine similarity threshold for vector search filtering (0.0 - 1.0).
    pub similarity_threshold: f32,
    /// Maximum share of context window allocated to retrieved episodic memory (0.0 - 1.0).
    pub max_context_share: f32,

    // --- Personal Memory v2 Additions ---
    /// Enable Personal Memory v2 processing and graph relations.
    pub personal_enabled: bool,
    /// Maximum share of context window allocated to personal facts (0.0 - 1.0).
    pub personal_max_context_share: f32,
    /// Maximum number of candidate facts retrieved for NLI logical comparison (K-limit).
    pub nli_candidate_limit: u32,
    /// Threshold above which an NLI prediction is classified as Contradiction (0.0 - 1.0).
    pub nli_contradiction_threshold: f32,
    /// Threshold above which an NLI prediction is classified as Entailment (0.0 - 1.0).
    pub nli_entailment_threshold: f32,
    /// Name or path of the local NLI ONNX model directory under ~/.vox/models/nli/
    pub nli_model_name: String,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            episodic_enabled: true,
            bg_worker_enabled: true,
            top_k: 3,
            similarity_threshold: 0.65,
            max_context_share: 0.20,
            
            // Personal Memory v2 Defaults
            personal_enabled: true,
            personal_max_context_share: 0.08,
            nli_candidate_limit: 5,
            nli_contradiction_threshold: 0.85,
            nli_entailment_threshold: 0.85,
            nli_model_name: "deberta-v3-small-nli".to_string(),
        }
    }
}
```

---

# Desired Win Scenarios

Success is defined by outcomes, not storage:

| Scenario | What Vox Remembers | Outcome |
|----------|-------------------|---------|
| **Entertainment** | Watched shows, liked/disliked, reasons | Recommendation based on prior experience, not generic taste |
| **Programming** | Previous Rust pain points (e.g. *"target dir hit 14 GB last time"*) | Proactively avoids repeating the mistake on the next Rust project |
| **Long-running Projects** | Current milestone, blockers, open decisions, architecture discussions | *"Continue Vox"* resumes without re-explaining context |
| **Personal Continuity** | Editor history `VSCode → Neovim → Zed` (append-only, nothing overwritten) | Full history stays available as the user evolves |
