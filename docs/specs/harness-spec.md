# Architectural Specification: Plugin-Based LLM Agent Harness Runtime

> **Document Type:** System Architectural Specification  
> **Target Subsystem:** `services/harness/` and `services/llm/actor.rs`  
> **Status:** Final Proposed Architecture  
> **Format Rule:** Pure architectural specification containing zero code snippets. All behaviors, state transitions, domain invariants, thresholds, and subsystem responsibilities are specified with rigorous technical precision for planning agents.

---

## 1. Scope, Purpose & Clean-Slate Mandate

### 1.1 Scope Boundaries
This specification defines the architectural overhaul of the Vox LLM Agent Harness. The scope is strictly bounded to:
1. **Hardening Current Core Capabilities**: Conversation History Management, Prompt & Persona Assembly, Context Window Budgeting, and Rolling Working Memory Compaction.
2. **Decoupling the LLM Actor**: Separating the low-level inference model into a pure token engine and elevating stream processing, clause chunking, TTS dispatch, and IPC token emissions into the Harness.
3. **Establishing a Plugin-Based Session Chassis**: Structuring the harness runtime around modular, domain-configurable plugins with explicit lifecycle hooks.
4. **Architectural Future-Proofing**: Providing designated integration slots for the Tagged Streaming Demuxer and On-Demand Episodic Tool Calling without implementing unapproved runtime features in this phase.

### 1.2 The Clean-Slate Deletion Mandate
There will be zero patching, wrapping, or legacy preservation of the existing harness implementation. The existing files in `services/harness/`—totaling over 1,600 lines of entangled procedural logic, duplicate proxy structs (`ContextHarness` vs. `TokenAccountant`, `ConversationManager` vs. `MessageBuffer`), dead Memory v1 XML formatting artifacts, and leaky facade dumping grounds—will be deleted entirely via clean removal. The new subsystem is authored completely fresh from this specification.

---

## 2. Core Architectural Philosophy: The Agent Chassis

### 2.1 The Foundational Axiom: Agent = Model + Harness
Vox treats conversational intelligence as a two-tier system:
1. **The Model Layer (`LlmActor`)**: A pure, stateless compute engine. It accepts a structured conversation payload and produces an asynchronous stream of raw tokens. It possesses zero knowledge of audio devices, text-to-speech actors, user interface windows, clause chunking heuristics, or conversational state machines.
2. **The Harness Layer (`HarnessSession`)**: The conversational orchestrator and runtime environment. It manages dialog memory, enforces token budgets, injects personalization, drives multi-step cognitive loops, routes output tokens to audio synthesis, and dictates pipeline state transitions.

### 2.2 Spatial & Temporal Composability
Following the DeepSeek Harness and Cordis meta-framework principles:
- **Spatial Composability (Swap Points)**: The harness is a chassis hosting swappable plugins. Operational domains mount only the plugins they require. The Modular Assistant domain mounts all conversational, budgeting, and compaction plugins. The Realtime Speech-to-Speech domain mounts only history logging and persona plugins, completely omitting local token budgeting and compaction because context is maintained remotely over WebSocket.
- **Temporal Composability (Lifecycle Scoping)**: The harness runtime is strictly bound 1:1 to the active voice session (`session_start` to `session_end`). It does not exist as an unmanaged, ambient background singleton during idle states.

---

## 3. Session Lifecycle & Ownership Model

### 3.1 1:1 Lifecycle Mapping (`session_start` to `session_end`)
The lifecycle of the `HarnessSession` maps directly to user engagement:
- **Session Initiation (`start_session` IPC / `VoxEvent::SessionStart`)**:
  - The voice pipeline transitions out of the `Idle` state.
  - The `HarnessSession` is instantiated and mounted.
  - Base persona prompts and the latest Personal Memory document are retrieved from persistent storage and assembled into working memory.
  - If continuing an existing session, uncompacted turns and the latest rolling summary are loaded from the database into the working history buffer.
  - Domain-specific plugins are instantiated and registered to the session chassis.
  - The bidirectional dialogue pipe to the `LlmActor` is established.
- **Session Termination (`end_session` IPC / `VoxEvent::EndSession`)**:
  - The voice pipeline transitions back to `Idle`.
  - The active `HarnessSession` is unmounted and deconstructed.
  - All pending one-shot timers, debounce watchers, and background tasks are immediately cancelled via session-scoped cancellation tokens.
  - Uncommitted session metadata is flushed to the database.
  - Zero harness background loops or mutex-protected memory structures persist in memory during the `Idle` state.

### 3.2 In-Session Continuation vs. Idle Session Selection
The command contracts strictly separate browsing sessions from mounting active runtimes:
- **Browsing Past Sessions (`continue_session(sessionId: i64)`)**:
  - Fetches and returns historical session metadata and turns for desktop UI rendering.
  - The pipeline remains `Idle`.
  - The `HarnessSession` is **not** booted or seeded in memory prematurely.
- **Engaging the Assistant (`start_session(sessionId: Option<i64>)`)**:
  - If `sessionId == Some(id)`: The user is engaging to continue a past conversation. The `HarnessSession` boots, queries Turso DB for continuation turns and the latest compaction summary for session `id`, and seeds working memory.
  - If `sessionId == None`: The user is engaging for a fresh session. The `HarnessSession` boots with a fresh prompt (`session_id = 0`, lazy DB row created upon the first spoken turn).
  - Clicking "+ New Session" (`create_session`) simply clears the active session selection in the UI; subsequent engagement sends `start_session(None)`.

---

## 4. Pipeline Topology & The Cognitive Stage

### 4.1 Upstream Consumption Contract
The voice pipeline router treats the Harness as the single cognitive front door:
```
[Audio In] ──► [VAD] ──► [STT] ──► [Harness (The Cognitive Stage)] ──► [TTS] ──► [Playback]
                                            ▲ │
                         Duplex Session Pipe│ │Token Stream
                                            │ ▼
                                      [LlmActor (Pure Model)]
```

- When the Speech-to-Text stage finalizes user speech (`VoxEvent::TranscriptFinal`), the pipeline hands the turn query directly to the `HarnessSession`.
- The pipeline does not act as a middleman between the Harness and the LLM. The Harness owns the direct dialogue pipe to the `LlmActor`.
- The `HarnessSession` emits synthesized speech clauses directly into the Text-to-Speech stage via its internal stream routing plugin.
- The `HarnessSession` is the sole authority that determines when the entire cognitive stage has concluded, emitting `VoxEvent::LlmFinished` only after all multi-step inference, compaction, or tool loops for the turn are complete.

### 4.2 Duplex Dialogue Pipe (Harness $\leftrightarrow$ LLM Actor)
Communication between the `HarnessSession` and the `LlmActor` occurs over a persistent, session-scoped bidirectional communication channel:
- **Harness to LLM Actor**: Transmits generation requests, prompt payloads, sampling parameters, cancellation signals, and (in future phases) tool call execution results.
- **LLM Actor to Harness**: Streams raw output tokens, model completion signals, and low-level engine errors.
- **Lifecycle**: The channel is established at session mount and terminated at session unmount. Ad-hoc, per-turn provider construction and file-system model searches are strictly forbidden.

---

## 5. Pipeline State Transitions & The `Working` State

### 5.1 The `Working` State Invariant
A critical architectural flaw in previous designs was the assumption that a turn is always a direct linear sequence: `Thinking` $\to$ `Speaking` $\to$ `Ready`. 

When cognitive maintenance (compaction) or agentic execution (tool calling) occurs, the assistant must speak an immediate interim phrase to acknowledge the user, but the turn is **not complete**. To prevent premature transitions to `Ready`, the pipeline introduces the `InteractionState::Working` state.

### 5.2 Audio Intent Classification: Interim Filler vs. Turn Response
Every audio synthesis request dispatched to the Text-to-Speech and Playback subsystems is explicitly tagged with an intent category:
1. **`AudioIntent::InterimFiller`**: A short, transitional phrase spoken to eliminate dead air while background cognitive work executes (e.g., *"Give me a moment while I organize our conversation"*).
2. **`AudioIntent::TurnResponse`**: The finalized conversational reply that answers the user's utterance.

### 5.3 Playback Engine Gating Rules
The audio playback engine enforces state transitions based on audio intent:
- When playing an audio chunk tagged as `AudioIntent::InterimFiller`:
  - The pipeline state remains in `InteractionState::Working`.
  - The playback engine **does not** transition the state to `Speaking`.
  - When the interim filler audio playback completes, the playback engine **does not** emit a completion event that triggers a transition to `Ready`. The pipeline remains locked in `InteractionState::Working`.
- When playing an audio chunk tagged as `AudioIntent::TurnResponse`:
  - The playback engine transitions the state from `Working` (or `Thinking`) to `InteractionState::Speaking`.
  - When the final response audio finishes playing, the playback engine transitions the state to `InteractionState::Ready`.

---

## 6. Detailed Turn Execution & Compaction Mechanics

### 6.1 Token Accounting & Utilization Metrics
Context window utilization is calculated continuously across working memory:
$$\text{Context Utilization} = \frac{\text{Tracked In-Memory Tokens}}{\text{Maximum Context Window Tokens} - \text{Reserved Generation Tokens}}$$
- **Tracked In-Memory Tokens**: The sum of the system prompt, injected Personal Memory document, active rolling context summary, and uncompacted historical turns.
- **Reserved Generation Tokens**: A dedicated budget allocation (default: 512 tokens for local models, 1024 for cloud) reserved strictly for the model's reply, preventing out-of-memory context clipping.
- **System Prompt Share Cap**: The combined size of the base persona and Personal Memory markdown document must not exceed 20% of the total context window. Any excess Personal Memory is truncated with an informative warning.

### 6.2 The Nominal Path (Context Utilization $< 85\%$)
1. User speech concludes; pipeline enters `InteractionState::Thinking`.
2. Finalized transcript arrives at `HarnessSession`.
3. Context utilization is evaluated and found to be nominal ($< 85\%$).
4. Harness formats the dialog payload and transmits it across the duplex pipe to the `LlmActor`.
5. `LlmActor` streams raw tokens back to the Harness `StreamRoutingPlugin`.
6. `StreamRoutingPlugin` accumulates tokens into grammatical clauses, dispatches synthesized audio chunks tagged as `AudioIntent::TurnResponse` to Text-to-Speech, and emits token updates to the UI window.
7. Playback engine begins streaming audio and transitions pipeline to `InteractionState::Speaking`.
8. Once all tokens are received and the clause chunker is flushed, the Harness emits `VoxEvent::LlmFinished`.
9. Audio playback completes; playback engine transitions pipeline to `InteractionState::Ready`.
10. The finalized turn is appended to the working history buffer and dispatched to persistent storage.

### 6.3 The Critical Inline Compaction Path (Context Utilization $\ge 85\%$)
When the context reaches or exceeds 85% usable capacity prior to generation, immediate cognitive maintenance is required:
1. User speech concludes; pipeline enters `InteractionState::Thinking`.
2. Finalized transcript arrives at `HarnessSession`.
3. Context evaluation detects utilization $\ge 85\%$.
4. **Immediate Working Transition & Interim Filler Dispatch**:
   - The Harness instructs the pipeline to transition `Thinking` $\to$ `InteractionState::Working`.
   - The Harness evaluates the active user query string via Unicode scalar value inspection (checking for characters within the Devanagari block `U+0900`..=`U+097F` via `services::translit::is_devanagari`). If Devanagari characters are detected, the filler phrase is drawn from the Hindi filler catalog; otherwise, it is drawn from the English filler catalog.
   - The filler phrase is dispatched immediately to Text-to-Speech as `AudioIntent::InterimFiller`. The user hears immediate voice acknowledgement while dead air is eliminated.
5. **Inline Summarization Execution**:
   - The Harness temporarily extracts the latest user turn, isolating the uncompacted history slice.
   - The Harness sends a structured compaction task across the duplex pipe to the `LlmActor` (with a 45-second timeout and up to 2 attempts).
   - Universal compaction contract (provider-agnostic): every compaction/extraction request carries strict JSON-schema enforcement against the canonical 6-bucket schema wherever the backend supports it (with negotiated fallback to JSON-object then prompt-only on `unsupported_parameter` rejections), and LLM reasoning is always disabled (voice-native default; user-driven opt-in reserved for a future agentic phase).
   - The model generates a structured JSON summary containing a rolling narrative overview and newly extracted categorical facts (`personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`).
6. **Persistence Staging & Working Memory Pruning**:
   - Extracted facts are staged into the database ingestion queue linked to a new compaction record.
   - The working history buffer is pruned of older raw turns and rebuilt: `[System Prompt, Rolling Context Summary, Active User Turn]`.
   - Context utilization drops safely below the 65% soft threshold.
7. **Execution of the Actual User Query**:
   - With memory now compacted, the Harness immediately constructs the generation request for the active user turn and dispatches it to the `LlmActor`.
   - As tokens stream back, they are routed as `AudioIntent::TurnResponse`.
   - The playback engine transitions `Working` $\to$ `InteractionState::Speaking` as the actual answer begins playing.
   - On completion, `VoxEvent::LlmFinished` is emitted, playback completes, and the state transitions to `InteractionState::Ready`.

### 6.4 Degraded FIFO Fallback Policy
If the `LlmActor` is an embedded local model with a context window $\le 4096$ tokens, or if history holds three or fewer messages, or if inline compaction fails both retry attempts:
1. Inline LLM compaction is skipped or aborted.
2. The Harness executes a deterministic **FIFO Sliding Window Shift**: oldest historical User/Assistant turn pairs are dropped iteratively from index 1 until context utilization drops below 65%.
3. The Harness emits a degraded pipeline error event indicating fallback to FIFO truncation.
4. Turn execution proceeds immediately, guaranteeing that the user's voice response is never permanently blocked.

### 6.5 The Reactive Opportunistic Soft Compaction Path ($65\% \le \text{Utilization} < 85\%$)
To minimize the occurrence of critical in-turn compaction, background compaction runs opportunistically during idle intervals:
- **Zero Idle Polling Invariant**: There is no continuous background polling loop spawned at session boot.
- **Reactive Post-Turn Evaluation**:
  - At the completion of every turn, context utilization is re-evaluated.
  - If utilization is $< 65\%$, no background action is taken.
  - If utilization is between $65\%$ and $85\%$, and `auto_compaction` is enabled, the Harness arms a **20-second quiet debounce timer**.
- **Execution Conditions**:
  - If the user speaks, an interruption occurs, or the pipeline leaves `Ready` or `Paused` before 20 seconds elapse, the timer is aborted immediately.
  - If the pipeline remains continuously in `Ready` or `Paused` for the full 20 seconds, the Harness triggers background summarization across uncompacted history.
  - On completion, the working history buffer is pruned with the rolling summary (shedding older raw turns), and facts are staged to the database.

---

## 7. The Five-Plugin Component Taxonomy

The `HarnessSession` chassis manages five distinct, decoupled plugins:

```
                                HARNESS SESSION CHASSIS
  ┌─────────────────────────────────────────────────────────────────────────────────┐
  │ 1. ConversationHistoryPlugin                                                    │
  │    • In-memory message sequence (System, User, Assistant)                       │
  │    • KV-cache prefix synchronization tracking                                   │
  │    • Trailing user turn deduplication and interruption rollback                 │
  ├─────────────────────────────────────────────────────────────────────────────────┤
  │ 2. PromptBuilderPlugin                                                          │
  │    • Pure text assembly: Persona Prompt + <user_profile> Markdown Document      │
  │    • Personal memory token budget enforcement and deterministic truncation      │
  │    • Complete pruning of legacy Memory v1 XML formatting logic                  │
  ├─────────────────────────────────────────────────────────────────────────────────┤
  │ 3. ContextBudgetPlugin                                                          │
  │    • Real-time token consumption tracking (heuristics & model-specific counters) │
  │    • Context utilization metric calculation against usable window budget         │
  │    • Threshold state classification (Nominal, Soft Window, Critical)            │
  │    • Deterministic FIFO Sliding Window shift execution                          │
  ├─────────────────────────────────────────────────────────────────────────────────┤
  │ 4. CompactionPlugin                                                             │
  │    • Structured cognitive summarization execution via duplex model pipe         │
  │    • Reactive 20-second debounced soft compaction watcher                       │
  │    • Turso database ledger recording and fact queue staging                     │
  ├─────────────────────────────────────────────────────────────────────────────────┤
  │ 5. StreamRoutingPlugin                                                          │
  │    • Egress consumption of raw tokens from LlmActor duplex pipe                 │
  │    • Clause chunking accumulator and punctuation prosody boundary enforcement   │
  │    • Text-to-Speech audio command dispatch (tagged with AudioIntent)            │
  │    • Frontend subtitle streaming via IPC token events                           │
  │    • Emission of VoxEvent::LlmFinished on full turn completion                  │
  └─────────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Strongly-Typed Prompt XML Tag Schema
To eliminate ad-hoc string formatting, prompt tags are governed by a strictly typed enum rather than loose string literals:

1. **`PromptTag::UserIdentity` (`<user_identity>...</user_identity>`)**:
   - **Content**: The active Personal Memory markdown document retrieved from the database.
   - **Placement**: Injected into the root System Prompt message, trailing the base persona instructions.
   - **Budget Guard**: Bounded by the 20% system prompt share ceiling (`context_window * max_context_share`). Truncated deterministically if exceeded.
2. **`PromptTag::SessionContext` (`<session_context>...</session_context>`)**:
   - **Content**: The entire structured output of the latest compaction pass, containing both Bucket 1 (personal facts) and Bucket 2 (working session state: `objective`, `workdone`, `blocker`, `next_step`, `pitfall`).
   - **Placement**: Injected into the root System Prompt message, framing historical continuity for subsequent turns.
3. **`PromptTag::PastTurns` (`<past_turns>...</past_turns>`)**:
   - **Content**: Restored uncompacted historical dialog turns upon session continuation.
   - **Placement**: Encloses historical message turns preceding the active user query.
4. **Implementation Invariant**: All prompt assembly logic must utilize the strongly typed tag enum for opening and closing delimiters, guaranteeing syntactic determinism and preventing raw string drift.

---

## 8. Domain Configurations

Different interaction domains mount specific subsets of the plugin chassis, enforcing clean architectural boundaries:

| Architectural Component | Modular Assistant (`PipelineMode::Modular`) | Realtime S2S (`PipelineMode::Realtime`) | Dictation (`InteractionState::Sleeping`) |
| :--- | :---: | :---: | :---: |
| **ConversationHistoryPlugin** | **Active**: Full FIFO dialog buffer | **Active**: History logging for UI rail | **None**: No conversational memory |
| **PromptBuilderPlugin** | **Active**: Persona + Personal Memory | **Active**: Session persona initialization | **None**: No prompt construction |
| **ContextBudgetPlugin** | **Active**: 65% soft / 85% critical checks | **None**: Context managed server-side | **None**: No token budgeting |
| **CompactionPlugin** | **Active**: Inline & Opportunistic | **None**: Context managed server-side | **None**: No local compaction |
| **StreamRoutingPlugin** | **Active**: Token clause chunking $\to$ TTS | **None**: Provider outputs PCM audio | **None**: STT transcript $\to$ OS typing |

---

## 9. Future-Proofing: Tool Calling & Advanced Filler Strategies

While out of scope for implementation in this phase, the architecture formally accommodates future capabilities without interface breakage:

### 9.1 The On-Demand Agentic Tool Calling Loop
Future episodic memory retrieval and external CLI execution will operate strictly on-demand via the model's output stream:
1. The user asks a question requiring deep memory search.
2. The `LlmActor` generates an interim conversational tag followed by a tool request tag:
   ```
   <response>Searching through your project notes...</response>
   <tool name="search_episodic_memory">authentication refactor</tool>
   ```
3. The `StreamRoutingPlugin` demuxes the stream:
   - `<response>` tokens route immediately to Text-to-Speech as `AudioIntent::InterimFiller`.
   - The pipeline enters `InteractionState::Working`.
   - `<tool>` tokens are intercepted by the Harness.
4. The Harness pauses inference, executes the vector query against the database, and captures the retrieved facts.
5. The Harness appends the tool results as a context observation and transmits an augmented payload back across the duplex pipe to the `LlmActor`.
6. The `LlmActor` generates the final conversational response, which streams through `StreamRoutingPlugin` to TTS as `AudioIntent::TurnResponse`.
7. Audio plays; pipeline transitions `Working` $\to$ `Speaking` $\to$ `Ready`.

### 9.2 Transition Filler Evolution Matrix
The architecture identifies three progressive strategies for interim filler delivery:
1. **Approach 1 (Baseline / Active Scope)**: Hardcoded localized phrases selected by the Harness and dispatched directly to TTS. Deterministic, zero extra inference overhead, immediate response.
2. **Approach 2 (Tagged Streaming Demuxer — Future Exploration)**: A single LLM pass generates an immediate spoken filler tag while concurrently synthesizing compaction or tool payloads under separate tags. Requires benchmarking parsing latency and model tag compliance.
3. **Approach 3 (Sequential Multi-Turn Fallback)**: If streaming tag demuxing fails on small local models, a multi-prompt while-loop is executed: Prompt 1 requests a concise 5-word filler $\to$ streamed to TTS $\to$ Prompt 2 executes compaction $\to$ Prompt 3 answers the user query.

---

## 10. Architectural Invariants

1. **Zero Backward Compatibility (ZBC)**: No legacy wrappers or compatibility bridges will be retained. The old 13-parameter `prepare_turn_context` and dead v1 XML methods are deleted completely.
2. **Sacred Audio Hot Path**: Zero memory allocations, zero locks (`Mutex`/`RwLock`), and zero disk or database operations are permitted on the CPAL audio callback or real-time VAD processing threads. All harness operations occur on Tokio tasks or dedicated background workers.
3. **Lock Discipline Across Await Points**: A `Mutex` or `RwLock` guard protecting conversational state must **never** be held across an `.await` boundary, particularly during LLM inference or database transactions.
4. **Decoupled Actor Invariant**: The `LlmActor` must never import or interact with audio channels, clause accumulators, or UI IPC emitters. It is strictly a token-generating worker.
5. **Central Authority for Turn Completion**: `VoxEvent::LlmFinished` is emitted exclusively by the `StreamRoutingPlugin` upon complete conclusion of all turn activities.
6. **Strict Two-Door Chassis Encapsulation**: `HarnessSession` is an opaque black-box orchestrator. Its internal plugins (`ConversationHistoryPlugin`, `PromptBuilderPlugin`, `ContextBudgetPlugin`, `CompactionPlugin`, `StreamRoutingPlugin`, `QuietCompactionWatcher`) are strictly private. External callers must never inspect, borrow, or mutate plugin fields directly. The harness boundary is strictly constrained to two touchpoints: (1) Session Lifecycle (`on_session_start` mount, `on_end` unmount), and (2) Cognitive Turn Execution (`prepare_turn`, token stream routing, turn finalization).
7. **Autonomous Reactive Quiet Watcher & History Pruning**: The `QuietCompactionWatcher` is self-governing; it monitors pipeline state transitions via `state_rx` and `CancellationToken` directly, automatically aborting when the pipeline leaves `Ready`/`Paused` without imperative caller invocation. Upon successful completion of background soft compaction, working history must be pruned and replaced with the structured `<session_context>` containing the entire compaction output (both personal and working session buckets) to ensure token utilization drops while retaining complete session fidelity for subsequent turns.
