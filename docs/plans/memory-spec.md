# Vox Memory Specification

> **Status:** Draft
>
> This document defines the architecture of Vox's memory system.
>
> The specification is developed incrementally. Each memory subsystem is designed, implemented, benchmarked, and finalized independently before the next subsystem is introduced.
>
> This document intentionally avoids discussing future subsystems until their design phase begins.

---

# Design Principles

* Build one memory subsystem at a time.
* Every subsystem must be independently testable.
* Every subsystem must have measurable latency and memory budgets.
* No subsystem becomes part of the default runtime until it has been benchmarked.
* The memory pipeline must never block the real-time voice pipeline.

## The Hardware Mapping

- Architecture decisons are decided based on feasibility with recommended tiers - hence vox must suport dynamic degrade and upgrade of architecture based on tier
where Tier 2 is recommended for users and is set as default

* **Tier 1A: 8GB Pure Local (no gpu):** Working Memory FIFO variation only (Simple buffer to manage context window)

* **Tier 1B: [RECOMMENDED] Pure Local (with gpu):** Working Memory + Episodic Memory + Semantic Memory(requires tool_calling hence depends on runtime capability) .

* **Tier 2A: [RECOMMENDED/NO-COST] Hybrid Stack ( Remote LLM + Local Audio ):** Working Memory + Episodic + Semantic(requires tool_calling hence depends on runtime capability) .

* **Tier 2B: [RECOMMENDED/DEFAULT] Hybrid Stack ( Cloud LLM + Local Audio ):** Working Memory + Episodic + Semantic(tool_calling is natively supported by all cloud models). 

* **Tier 3: [BEST-PERFORMANCE] Realtime S2S (WebSocket):** Provider-managed Working Memory + Episodic & Semantic (managed via early tool calls like prompt must force to use tool calls at start before generating reponse to avoid interruptions) . 

---

# Memory Taxonomy

Vox separates memory into independent cognitive systems.

These systems serve different purposes and are implemented independently.

## 1. Working Memory

**Status:** 🟢 Completed

Purpose:

* Maintain the current conversation.
* Provide context to the LLM.
* Manage context window growth.
* Handle context compression.

This is the only memory subsystem currently under design.

No assumptions are made yet regarding implementation details.

---

## 2. Episodic Memory

**Status:** ⚪ Not Started

Purpose:

* Record historical interactions.
* Preserve chronological events.
* Enable retrieval of previous conversations.

Implementation intentionally deferred.

---

## 3. Semantic Memory

**Status:** ⚪ Not Started

Purpose:

* Store durable facts.
* Maintain persistent knowledge.
* Track entities and relationships.

Implementation intentionally deferred.

---

# Development Process

Each memory subsystem follows the same lifecycle:

1. Define requirements.
2. Design architecture.
3. Implement.
4. Benchmark.
5. Validate.
6. Freeze.
7. Proceed to the next subsystem.

No future subsystem should influence the implementation of the current one unless a hard architectural dependency exists.



# Working Memory

**Status:** 🟢 Completed 

Working Memory is the runtime subsystem responsible for managing the active conversation presented to the LLM.

It is transient, session-scoped, and exists only while the conversation is active.

Its purpose is to maintain a valid, high-quality context window without exceeding the active model's context limit while preserving real-time voice responsiveness.

Working Memory is not responsible for long-term persistence, retrieval, embeddings, or knowledge graphs.

---

# Responsibilities

Working Memory is responsible for:

* Maintaining the active conversation.
* Tracking token usage.
* Managing the available context budget.
* Constructing the prompt presented to the LLM.
* Maintaining provider-specific context state.
* Performing context maintenance when required.

Working Memory is **not** responsible for:

* Episodic Memory
* Semantic Memory
* Embedding generation
* Entity extraction
* Knowledge graph construction
* Persistent storage

---

# Conversation Manager

Working Memory is implemented by a dedicated `ConversationManager`.

The ConversationManager is the single source of truth for the active conversation.

Responsibilities include:

* Maintaining conversation history.
* Tracking token usage.
* Monitoring context budget.
* Selecting the provider strategy.
* Performing context maintenance.
* Building the final LLM request.

No other subsystem may directly modify the active conversation.

---

# Conversation State

The active conversation is maintained entirely in runtime memory.

Conceptually:

```text
Conversation
├── System Prompt
├── Conversation History
├── Tool Results
├── Runtime Metadata
└── Provider State
```

The storage mechanism is provider-independent.

The synchronization mechanism is provider-specific.

---

# Provider Strategy

Working Memory supports two execution strategies depending on the active runtime.

## Stateless Providers

Examples:

* OpenAI
* Gemini
* Anthropic
* OpenAI-compatible APIs

The ConversationManager constructs the complete prompt for every request.

No conversational state exists inside the provider.

---

## Stateful Providers

Examples:

* llama.cpp
* Future embedded inference engines

The provider maintains an active KV Cache.

The ConversationManager owns the logical conversation while the provider owns the synchronization of its internal context state.

Working Memory must never assume all providers behave identically.

---

# Context Budget

Before every inference request the ConversationManager calculates:

* Current context usage
* Maximum supported context
* Reserved generation budget
* Remaining available context

The runtime—not the LLM—is responsible for enforcing these limits.

Thresholds are runtime configurable.

---

# Context Maintenance

Working Memory supports two independent maintenance policies.

## 1. Threshold Maintenance (High Priority)

Triggered when the conversation exceeds the configured critical context threshold.

Characteristics:

* Mandatory
* Synchronous
* Blocks the next inference request
* Guarantees the context budget remains valid

The current user request is never processed until maintenance has successfully completed.

The response is always generated using the updated conversation.

---

## 2. Opportunistic Maintenance (Low Priority)

Triggered only when:

* the pipeline is idle
* conversation usage exceeds a configurable soft threshold

Characteristics:

* Optional
* Background task
* Cancelable
* Never blocks the voice pipeline

If interrupted by new user activity the task is immediately cancelled without modifying the active conversation.

Threshold Maintenance always takes precedence.

---

# Transition State

During Threshold Maintenance Vox enters a dedicated runtime state.

```text
Idle

Listening

Thinking

Speaking

ContextManaging
```

`ContextManaging` is a global runtime state.

Its purpose is to clearly communicate that Working Memory maintenance is occurring.

---

# Transition Speech

Upon entering the `ContextManaging` state Vox immediately plays a deterministic transition message.

Messages are selected randomly from a predefined set.

Examples include:

* "Give me a moment while I organize our conversation."
* "One moment while I reorganize everything we've discussed."

These messages are runtime assets.

They are **never generated by the LLM**.

This guarantees:

* zero additional LLM latency
* deterministic behavior
* localization support
* consistent UX

---

# Context Maintenance Flow

Threshold Maintenance follows the sequence below.

```text
STT Final
        │
        ▼
ConversationManager
        │
        ▼
Critical Threshold Reached?
        │
   ┌────┴────┐
   │         │
  No        Yes
   │         │
   ▼         ▼
 Continue   Enter ContextManaging
                │
                ▼
      Play Transition Speech
                │
                ▼
      Perform Context Maintenance
                │
                ▼
     Rebuild Active Conversation
                │
                ▼
 Generate Response To Original User Input
                │
                ▼
              TTS
```

The original user request is preserved throughout the maintenance process.

The LLM response is generated only after maintenance completes.

---

# Barge-in During Context Management

Working Memory must support user interruption while maintenance is active.

If VAD detects a new `SpeechStart` event during `ContextManaging`:

* the new utterance must never be discarded
* the active conversation must never be modified concurrently
* the maintenance task must complete atomically

To prevent race conditions the runtime maintains a temporary hold queue.

Flow:

```text
SpeechStart

↓

Temporary Hold Queue

↓

Context Maintenance Completes

↓

Append Buffered Turns

↓

Generate Response
```

This guarantees:

* no dropped speech
* no concurrent mutation of conversation state
* deterministic behavior under interruption

---

# Design Constraints

Working Memory must satisfy the following constraints:

* Never exceed the configured context budget.
* Never reject a request due to context exhaustion.
* Never corrupt conversation state during concurrent events.
* Never block the voice pipeline except during mandatory Threshold Maintenance.
* Always generate the final response from the maintained conversation rather than the pre-maintenance context.
* Always preserve the user's original request during maintenance.

---

# Out of Scope

The following systems are intentionally excluded from Working Memory and will be specified independently:

* Episodic Memory
* Semantic Memory
* Embedding generation
* Retrieval
* Vector search
* Knowledge graph
* Entity extraction
* Background memory consolidation


# Episodic Memory Specification

> **Status:** Draft

This document defines the Episodic Memory subsystem of Vox.

Episodic Memory extends Working Memory by allowing Vox to recall relevant past conversations after the active context has been compacted or forgotten.

Unlike Working Memory, Episodic Memory is persistent across sessions.

It is designed to remain completely independent from Semantic Memory.

---

# Purpose

Episodic Memory exists to answer:

> **"What have we talked about before?"**

It stores historical conversation summaries and retrieves them when they are relevant to the current conversation.

It is **not** responsible for storing durable facts or user profiles.

---

# Responsibilities

Episodic Memory is responsible for:

* Persisting conversation summaries.
* Maintaining chronological conversation history.
* Retrieving relevant historical sessions.
* Supplying additional context to the LLM.

It is **not** responsible for:

* Working Memory
* Context window management
* Fact extraction
* Entity extraction
* Knowledge graphs
* User profile construction

---

# Design Principles

* Never store raw conversations.
* Store compacted summaries only.
* Retrieval must never block the realtime voice pipeline.
* Retrieval must operate within a fixed token budget.
* Every retrieved memory must represent a different historical session.
* Memory architecture dynamically adapts based on runtime tier.

---

# Storage Unit

The storage unit of Episodic Memory is a completed Working Memory compaction.

Conceptually:

```text
Session
    ↓
Working Memory Compaction
    ↓
Summary
    ↓
Embedding
    ↓
Vector Database
```

Raw turns are never embedded.

Only finalized compaction summaries are embedded.

---

# Memory Record

Each Episodic Memory record contains:

```text
Episode
├── Session ID
├── Summary
├── Embedding
├── Timestamp
├── Metadata
```

Metadata may include:

* conversation duration
* summary token count
* creation timestamp

No extracted facts are stored here.

---

# Ingestion Pipeline

Only semantic conversations are stored.

```text
Conversation Completed
        │
        ▼
Query Classifier
        │
 ┌──────┴──────┐
 │             │
Generic     Semantic
 │             │
Skip      Compaction Summary
               │
               ▼
        Generate Embedding
               │
               ▼
        Store Episode
```

Generic conversations are discarded.

Only summaries classified as semantic are embedded.

---

# Retrieval

Retrieval begins only after the user's current query is available.

```text
Current User Query
        │
        ▼
Generate Query Embedding
        │
        ▼
Vector Search
        │
        ▼
Diversify By Session
        │
        ▼
Token Budget Filter
        │
        ▼
Inject Into Prompt
```

---

# Session Diversification

Standard Top-K retrieval is not used.

Instead:

1. Retrieve a larger candidate set.
2. Group candidates by Session ID.
3. Keep only the highest scoring summary from each session.
4. Return the final Top-K.

Example:

```text
Raw Results

S1 (0.94)
S1 (0.92)
S1 (0.89)
S2 (0.87)
S3 (0.84)

↓

Diversified

S1
S2
S3
```

This prevents one long conversation from dominating retrieval.

---

# Context Budget

Retrieved memories have an independent context budget.

Example allocation:

```text
Context Window
├── System Prompt
├── Working Memory
├── Episodic Memory (≤20%)
└── Generation Reserve
```

The Episodic Memory budget is a hard runtime limit.

If the retrieved summaries exceed the configured budget:

1. Highest relevance summaries are kept.
2. Remaining summaries are discarded.

Working Memory always has priority.

---

# Runtime Behavior

Episodic Memory behaves differently depending on runtime tier.

## Tier 1A

No Episodic Memory.

Working Memory only.

---

## Tier 1B

Automatic retrieval.

```text
User Query
    ↓
Retrieve Episodes
    ↓
Inject Context
    ↓
LLM
```

---

## Tier 2A

Same architecture as Tier 1B.

Remote LLM.

Local embeddings.

Local vector database.

---

## Tier 2B

Same architecture as Tier 2A.

Cloud LLM.

Local embeddings.

Local vector database.

---

## Tier 3

Realtime speech requires a different strategy.

Rather than retrieving memories every turn, retrieval becomes tool-driven.

```text
Realtime Session
        │
        ▼
User Query
        │
        ▼
LLM decides memory is needed
        │
        ▼
Episode Retrieval Tool
        │
        ▼
Return Relevant Sessions
        │
        ▼
Continue Response
```

This avoids unnecessary retrieval during continuous streaming.

---

# Retrieval Tool (Tier 3)

The retrieval tool returns only historical summaries.

Input:

```text
Natural language query
```

Output:

```text
Episode 1
Episode 2
Episode 3
```

The tool must respect:

* maximum number of returned episodes
* maximum token budget
* one summary per session

---

# Failure Behavior

If retrieval fails:

* continue normally
* do not retry synchronously
* do not block response generation

The LLM simply receives Working Memory.

---

# Design Constraints

Episodic Memory must:

* never store raw conversations
* never exceed its configured context budget
* never return multiple summaries from the same session
* never block realtime audio
* never modify Working Memory
* never duplicate Semantic Memory responsibilities

---

# Out of Scope

The following are intentionally excluded:

* user facts
* preferences
* entity graphs
* relationship extraction
* profile generation
* long-term knowledge storage

These belong to Semantic Memory and will be specified independently.
