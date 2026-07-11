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
| **Personal Memory** | ⚪ Not Started | Evolving user model |

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

**Status:** ⚪ Not Started · Purpose: *maintain a continuously evolving model of the user.* It answers *"What do I know about this user?"* rather than *"What facts exist?"* Implementation is intentionally abstract.

## Memory Categories

Fixed categories — the extractor fills them; the runtime never invents memory types:

`Identity` · `Preferences` · `Experiences` · `Projects` · `Goals` · `Tasks` · `Relationships` · `Skills` · `Devices` · `Locations`

## Temporal Memory

Every durable memory is **append-only**; previous values are never overwritten. History accumulates; the current value is resolved at retrieval time.

```text
Preference → History → Current State

Favorite Language: Python → Go → Rust
```

History remains available even after the current value changes.

## Episodic vs Personal Memory

| | **Episodic** | **Personal** |
|--|--------------|--------------|
| Answers | "What happened?" | "What do I know about the user?" |
| Stores | conversation summaries, discussions, sessions | preferences, projects, identity, relationships, experiences, evolving profile |
| Retrieval | semantic similarity | structured lookups |

## Ingestion Pipeline

Old design (separate NLP entity pipeline):

```text
Conversation → Summary → Entity Extraction → Knowledge Graph
```

New design shares compaction and splits into two branches:

```text
Conversation
    │
    ▼
Working Memory
    │
    ▼
Compaction
    ├────────────► Episodic Summary ──► Vector Database
    │
    └────────────► Personal Memory Extraction ──► Personal Memory Store
```

One branch preserves *what happened* (Episodic); the other preserves *what was learned about the user* (Personal Memory). **Personal Memory Extraction** is performed by the compaction model and produces a structured object — there is no separate NLP pipeline, and Memory Extraction is **not** NLP entity extraction:

```json
{
  "summary": "...",
  "profile_updates": [],
  "project_updates": [],
  "experience_updates": [],
  "goal_updates": [],
  "task_updates": []
}
```

## Design Constraints

Personal Memory must **not** mandate a storage implementation — no graph database, Neo4j, RDF, or triples required. It may use any engine capable of representing temporal structured user knowledge, keeping the cognitive model stable while preserving implementation flexibility.

---

# Desired Win Scenarios

Success is defined by outcomes, not storage:

| Scenario | What Vox Remembers | Outcome |
|----------|-------------------|---------|
| **Entertainment** | Watched shows, liked/disliked, reasons | Recommendation based on prior experience, not generic taste |
| **Programming** | Previous Rust pain points (e.g. *"target dir hit 14 GB last time"*) | Proactively avoids repeating the mistake on the next Rust project |
| **Long-running Projects** | Current milestone, blockers, open decisions, architecture discussions | *"Continue Vox"* resumes without re-explaining context |
| **Personal Continuity** | Editor history `VSCode → Neovim → Zed` (append-only, nothing overwritten) | Full history stays available as the user evolves |
