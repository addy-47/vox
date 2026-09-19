# Architectural Specification: LLM Agent Harness Runtime

> **Document Type:** System Architectural Specification  
> **Target Subsystem:** `services/harness/` and `services/llm/actor.rs`  
> **Status:** Approved Target Architecture  
> **Format Rule:** Pure architectural specification containing zero code snippets. All behaviors, state transitions, domain invariants, thresholds, and subsystem responsibilities are specified with rigorous technical precision for planning agents.

---

## 1. Scope, Philosophy & Core Invariants

#### 1.1 Scope Boundaries
This specification defines the architectural design of the Vox LLM Agent Harness. The scope is strictly bounded to:
1. **Single-Orchestrator Turn Execution**: Consolidating all conversational turn sequencing, state evaluation, and actor coordination into a single runtime orchestrator (`Harness` in `services/harness/orchestrator.rs`).
2. **Narrow-Interface Stages**: Decoupling memory, budgeting, prompt assembly, and compaction into isolated, pure input-in/output-out stages (`stages/`).
3. **Decoupled Egress Stream Processing**: Elevating token stream consumption, grammatical clause chunking, prosody punctuation morphing, TTS dispatch, and UI subtitle emissions out of both the LLM actor and the TTS actor into the Harness egress stream layer (`streaming/`).
4. **Architectural Future-Proofing**: Establishing designated integration slots for the Tagged Streaming Demuxer and On-Demand Episodic Tool Calling without implementing unapproved runtime features in this phase.

### 1.2 The Foundational Axiom: Agent = Model + Harness
Vox treats conversational intelligence as a two-tier system:
1. **The Model Layer (`LlmActor`)**: A pure, stateless compute engine. It accepts a structured conversation payload and produces an asynchronous stream of raw tokens. It possesses zero knowledge of audio devices, text-to-speech actors, user interface windows, clause chunking heuristics, or conversational state machines.
2. **The Harness Layer (`Harness`)**: The conversational orchestrator and runtime environment. It manages dialog memory, enforces token budgets, injects personalization, drives multi-step cognitive loops, routes output tokens to audio synthesis, and dictates pipeline state transitions.

### 1.3 The Six Architectural Invariants
Every component and stage within the harness must strictly satisfy six invariants:
1. **Single Orchestrator Authority**: Exactly one orchestrator (`Harness`) owns turn sequencing and lifecycle state. No other component or pipeline helper may make sequencing decisions.
2. **Narrow, Typed Stage Interfaces**: Each stage accepts a typed input struct and returns a typed output struct. A stage never reaches into another stage's internals.
3. **Zero Inter-Stage Communication**: Stages never import, call, or communicate with each other. The orchestrator mediates all data flow.
4. **Exclusive State Ownership**: Each stage owns its state exclusively. There is zero shared mutable state or simultaneous multi-actor mutation.
5. **Strict Boundary Encapsulation**: The orchestrator is an opaque black box. External callers cannot clone, borrow, or invoke internal stages directly.
6. **Domain-Pure Layer Placement**: Text processing (token accumulation, clause chunking, prosody morphing) belongs strictly in the harness streaming layer. Audio processing (voice conditioning, sample synthesis, buffer playback) belongs strictly in the TTS and audio subsystems.

---

## 2. System Boundaries & Integration Contracts

The voice pipeline router treats `Harness` as the single cognitive front door:
```
[Audio In] ──► [VAD] ──► [STT] ──► [Harness (Orchestrator)] ──► [TTS Actor] ──► [PlaybackEngine]
                                              ▲ │
                           Duplex Session Pipe│ │Token Stream
                                              │ ▼
                                        [LlmActor (Pure Model)]
```

### 2.1 Upstream Integration Contract (Caller $\to$ Harness)
- **Sole Entry Point**: Upstream speech recognition stages (`pipeline::assistant::transcript`) interact with the cognitive layer exclusively via a single unified turn method: `Harness::execute_turn(query, turn_id, cancel)`.
- **Zero External Stage Slicing**: Upstream callers must never call partial preparation methods, clone internal streaming plugins, or directly execute compaction on the database. The orchestrator owns all sequencing decisions internally.
- **Pipeline Adapter Role**: The upstream pipeline layer acts strictly as a thin event adapter: receiving `VoxEvent::TranscriptFinal`, handing it to `Harness::execute_turn`, and translating the returned typed `TurnOutcome` into pipeline state transitions:
  - `TurnOutcome::Completed { assistant_response: String, turn_id: u64 }`: Turn succeeded and committed; pipeline transitions to `Speaking` (driven by playback) or `Ready`.
  - `TurnOutcome::DuplicateIgnored`: Echo turn discarded; pipeline transitions back to `Ready`.
  - `TurnOutcome::Cancelled`: Turn was aborted mid-stream; history rolls back user query; pipeline transitions to `Ready` (or remains `Idle`).
  - `TurnOutcome::Error(String)`: Turn execution or model stream failed; history rolls back; pipeline emits error event and transitions to `Ready`.

### 2.2 Downstream Integration Contracts (Harness $\to$ Subsystems)
- **Model Layer (`LlmActor`)**: The harness transmits `GenerationRequest` over the duplex session pipe and receives an asynchronous stream of `LlmResponse` tokens for conversational turn responses.
- **Speech Synthesis (`TtsActor`)**: The harness egress stream stage dispatches finished speakable clauses to the TTS worker thread via `TtsCommand::Generate { text, intent, turn_id }`. The TTS actor is strictly an audio worker; it contains zero clause-chunking, punctuation, or text-parsing logic.
- **Audio Playback (`PlaybackEngine`)**: Receives synthesized PCM samples from the TTS actor and coordinates physical speaker playback, emitting state transition events (`Speaking`, `Ready`).
- **Frontend IPC (`IpcEvent`)**: The harness egress stream stage emits clean subtitle tokens (`IpcEvent::LlmToken`) directly to the active UI window.
- **Persistence (`Turso DB`)**: The compaction stage records compaction runs and staged memory facts to the local database ledger.

### 2.3 Duplex Dialogue Pipe vs. Compaction Transport
- **Conversational Duplex Dialogue Pipe (`Harness` $\leftrightarrow$ `LlmActor`)**: A persistent, session-scoped command channel (`llm_tx`) with per-turn one-shot response channels dedicated exclusively to conversational generation with low-latency token streaming, clause chunking, and TTS dispatch. The channel is established at session mount and terminated at session unmount.
- **Compaction Inference Transport**: Compaction summarization is a non-streaming, single-shot structured JSON extraction call that runs directly against `Arc<dyn LlmProvider>`. Crucially, to prevent blocking the Tokio async runtime during local/embedded model compaction, compaction inference must be executed with dedicated executor isolation (via `tokio::task::spawn_blocking` or dedicated compute threads). It never contends with or blocks the conversational duplex pipe.

---

## 3. Session Lifecycle & Domain Gating

### 3.1 1:1 Lifecycle Mapping (`session_start` to `session_end`)
The lifecycle of `Harness` maps directly to user engagement:
- **Session Initiation (`start_session` IPC / `VoxEvent::SessionStart`)**:
  - The voice pipeline transitions out of `InteractionState::Idle`.
  - The `Harness` is instantiated and mounted into global state (`services/harness/orchestrator.rs`).
  - Base persona prompts and the latest Personal Memory document are retrieved from persistent storage and assembled into working memory.
  - If continuing an existing session, uncompacted turns and the latest rolling summary are loaded from the database into the working history buffer.
  - Domain-specific stages are instantiated and bound to the orchestrator.
  - The bidirectional dialogue pipe to `LlmActor` is established.
- **Session Termination (`end_session` IPC / `VoxEvent::EndSession`)**:
  - The voice pipeline transitions back to `InteractionState::Idle`.
  - The active `Harness` is unmounted and deconstructed via RAII.
  - All pending one-shot timers, debounce watchers, and background tasks are immediately cancelled via session-scoped cancellation tokens.
  - Uncommitted session metadata is flushed to the database.
  - Zero harness background loops or mutex-protected memory structures persist in memory during the `Idle` state.

### 3.2 In-Session Continuation vs. Idle Session Selection
The command contracts strictly separate browsing sessions from mounting active runtimes:
- **Browsing Past Sessions (`continue_session(sessionId: i64)`)**:
  - Fetches and returns historical session metadata and turns for desktop UI rendering.
  - The pipeline remains `InteractionState::Idle`.
  - The `Harness` is **not** booted or seeded in memory prematurely.
- **Engaging the Assistant (`start_session(sessionId: Option<i64>)`)**:
  - If `sessionId == Some(id)`: The user is engaging to continue a past conversation. The `Harness` boots, queries Turso DB for continuation turns and the latest compaction summary for session `id`, and seeds working memory.
  - If `sessionId == None`: The user is engaging for a fresh session. The `Harness` boots with a fresh prompt (`session_id = 0`, lazy DB row created upon the first spoken turn).
  - Clicking "+ New Session" (`create_session`) simply clears the active session selection in the UI; subsequent engagement sends `start_session(None)`.nt engagement sends `start_session(None)`.

### 3.3 Domain Gating Configuration
Different interaction domains mount specific subsets of the harness stages:

| Subsystem Component | Modular Assistant (`PipelineMode::Modular`) | Realtime S2S (`PipelineMode::Realtime`) | Dictation (`InteractionState::Sleeping`) |
| :--- | :---: | :---: | :---: |
| **ConversationHistoryStage** | **Active**: Full FIFO dialog buffer | **Active**: History logging for UI rail | **None**: No conversational memory |
| **PromptBuilderStage** | **Active**: Persona + Personal Memory | **Active**: Session persona initialization | **None**: No prompt construction |
| **ContextBudgetStage** | **Active**: 65% soft / 85% critical checks | **None**: Context managed server-side | **None**: No token budgeting |
| **CompactionStage** | **Active**: Inline & Opportunistic | **None**: Context managed server-side | **None**: No local compaction |
| **StreamRouter & Chunker** | **Active**: Token clause chunking $\to$ TTS | **None**: Provider outputs PCM audio | **None**: STT transcript $\to$ OS typing |

---

## 4. The Functional Stages Taxonomy

## 4. The Functional Stages Taxonomy

The `Harness` orchestrator coordinates five distinct, decoupled functional stages:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                  HARNESS ORCHESTRATOR                                  │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 1. ConversationHistoryStage (`stages/history.rs`)                                      │
│    • In-memory message sequence (System, User, Assistant)                              │
│    • Trailing user turn deduplication and interruption rollback                        │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 2. ContextBudgetStage (`stages/budget.rs`)                                             │
│    • Real-time token consumption tracking against model context window                 │
│    • Threshold classification: Nominal (<65%), Soft (65-85%), Critical (≥85%)          │
│    • Deterministic FIFO Sliding Window shift execution                                 │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 3. PromptBuilderStage (`stages/prompt.rs`)                                             │
│    • Pure text assembly: Persona Prompt + <user_identity> + <session_context>          │
│    • Personal memory token budget enforcement (20% context window share ceiling)       │
│    • GenerationRequest payload assembly from history slice and active user query       │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 4. CompactionStage (`stages/compaction.rs` & `watcher.rs`)                             │
│    • Structured cognitive summarization execution via isolated provider compute        │
│    • Universal 6-bucket JsonSchema enforcement & fact staging to Turso DB              │
│    • Reactive 20-second debounced soft compaction watcher                              │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 5. Egress Stream Stage (`streaming/chunker.rs` & `streaming/router.rs`)                │
│    • Clause chunking accumulator and punctuation prosody boundary enforcement          │
│    • Text-to-Speech audio command dispatch tagged with AudioIntent                     │
│    • Frontend subtitle streaming via IPC token events                                  │
│    • Emission of VoxEvent::LlmFinished on full turn completion                         │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.1 ConversationHistoryStage (`stages/history.rs`)
- **Responsibility**: Maintains the ordered in-memory message history buffer (`System`, `User`, `Assistant`).
- **Input**: User utterance text, assistant reply text, or seeded historical turn rows.
- **Output**: Snapshot slice of message turns (`&[ChatMessage]`).
- **Invariants**:
  - Message at index 0 is always the active root system prompt.
  - Interrupted turns with zero or empty assistant text are rolled back to prevent dangling user turns.
  - Trailing duplicate detection: If an incoming user query matches the preceding user query within a short temporal window, it is flagged as a duplicate.

### 4.2 ContextBudgetStage (`stages/budget.rs`)
- **Responsibility**: Tracks token consumption and determines whether context capacity is healthy or requires compaction.
- **Utilization Formula**:
  $$\text{Context Utilization} = \frac{\text{Tracked In-Memory Tokens}}{\text{Maximum Context Window Tokens} - \text{Reserved Generation Tokens}}$$
  - *Tracked In-Memory Tokens*: Sum of system prompt, injected Personal Memory document, active rolling context summary, uncompacted turns, and current user query.
  - *Reserved Generation Tokens*: Sourced from active configuration `settings.llm.max_output_tokens` (clamped between 256 and 2048 tokens; defaults to 512 for local embedded models, 1024 for cloud providers).
- **Classification Thresholds**:
  - **Nominal ($< 65\%$)**: Context is healthy; turn proceeds directly.
  - **Soft Warning ($65\% \le \text{Utilization} < 85\%$)**: Context is elevated; qualifies for debounced post-turn background compaction.
  - **Critical Threshold ($\ge 85\%$)**: Context is saturated; triggers immediate in-turn cognitive maintenance.

### 4.3 PromptBuilderStage (`stages/prompt.rs`)
- **Responsibility**: Assembles the static persona and dynamic memory documents into the canonical root system prompt string (Message 0), and shapes the complete `GenerationRequest` payload.
- **Memory Share Ceiling**: Injected `<user_identity>` Personal Memory must not exceed 20% of the total context window. Any excess is truncated deterministically at character boundaries.
- **Tag Formatting**: Opening and closing delimiters are generated strictly via the typed `PromptTag` enum.
- **Payload Assembly Authority**: `PromptBuilderStage` owns `build_generation_request(&self, history: &[ChatMessage], query: &str) -> GenerationRequest`. The `Harness` delegates all prompt assembly and token structuring directly to this stage.

### 4.4 CompactionStage (`stages/compaction.rs` & `watcher.rs`)
- **Responsibility**: Compresses older dialog history into structured, categorized facts and a rolling narrative summary.
- **Universal Contract**: Compaction requests enforce the canonical 6-bucket JsonSchema (`personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`) with reasoning disabled.
- **Compaction Settings Parameter Isolation**: Compaction operates with its own dedicated parameters, inheriting ONLY the active provider/model and context window ceiling (`effective_ctx_size()`) from user settings:
  1. *Always JSON Mode*: `OutputConstraint::JsonSchema` (with fallback to `JsonObject` baseline only if model lacks schema support).
  2. *Reasoning Always Disabled*: `ReasoningMode::Disabled`, regardless of user conversational reasoning settings.
  3. *Deterministic Low Temperature*: Hardcoded constant `DEFAULT_LLM_COMPACTION_TEMPERATURE = 0.2` (user conversation temperature is ignored).
  4. *Autonomous Output Budget*: Computed strictly via `calculate_compaction_max_tokens` (`(effective_ctx_size * 0.15).clamp(256, 16384)`), independent of conversation `max_output_tokens`.
- **Direct Provider Inference & Isolation**: Executes directly against `Arc<dyn LlmProvider>` with dedicated executor isolation (`tokio::task::spawn_blocking` for local embedded inference), ensuring the Tokio async reactor threads are never blocked.
- **Persistence**: Records the compaction run in Turso DB (`compactions` ledger) and stages newly extracted facts into the ingestion queue.
- **Pruning**: Replaces the pruned historical turns in memory with the structured `<session_context>` block.

### 4.5 Egress Stream Processing (`streaming/chunker.rs` & `streaming/router.rs`)
- **ClauseChunker (`chunker.rs`)**:
  - Accumulates incoming token fragments into text.
  - Splits on primary terminators (`\n`, `?`, `!`) and sub-clause boundaries (`,`, `;`, `:`, `—`, `–`) once minimum word thresholds are satisfied.
  - Enforces adaptive word counts based on clause index: clause 0 targets 5–12 words for low TTFA; clause 1 targets 10–20 words; steady state targets 16–32 words.
  - Applies prosody morphing: premature periods (`< 5` words) are rewritten to commas so speech engines maintain rising pitch contours.
  - Prevents splits across standard abbreviations and decimals.
- **StreamRouter (`router.rs`)**:
  - Routes finished clauses to `TtsActor` as `TtsCommand::Generate`.
  - Dispatches `IpcEvent::LlmToken` subtitle events to the UI.
  - Flushes tail remainder and emits `VoxEvent::LlmFinished`.

---

## 5. Pipeline State Lifecycle & Audio Intent Rules

### 5.1 The Generic Non-Terminal Phase Contract & `Working` State
`InteractionState::Working` represents any intermediate, non-terminal cognitive phase where the assistant has acknowledged the user's utterance but is actively performing intermediate work before the finalized conversational response is ready. 

To prevent the `Harness` from hardcoding one-off special cases for every cognitive operation, all non-terminal behaviors are governed by a single generic contract:

1. **Stage Declaration**: Any stage within `stages/` (e.g., `CompactionStage` today, episodic memory retrieval or agentic tool execution tomorrow) can return an intermediate operation signal (`NonTerminalPhase`) to the `Harness`.
2. **Uniform Transition**: When an intermediate operation is triggered, the `Harness` immediately transitions the voice pipeline from `InteractionState::Thinking` to `InteractionState::Working`.
3. **Uniform Audio Cue**: If the intermediate operation provides an interim phrase (e.g., *"One moment while I organize our conversation..."*), the `Harness` dispatches it to `TtsActor` tagged as `AudioIntent::InterimFiller` to eliminate dead air.
4. **Uniform Execution**: The `Harness` executes the operation asynchronously (guaranteeing thread/executor isolation for local inference or I/O).
5. **State Locking**: The pipeline remains locked in `InteractionState::Working` throughout the intermediate execution. Playback of interim filler audio does not transition the pipeline to `Speaking` or `Ready`.
6. **Resumption**: Once the intermediate operation completes, the `Harness` feeds the output into subsequent generation without leaving `Working` until the finalized response audio begins physical playback (`Working` $\to$ `Speaking` $\to$ `Ready`).
7. **Terminal Error & Abort Recovery**: If any intermediate operation fails, encounters an error, or is cancelled, the `Harness` executes cleanup, rolls back the user turn if appropriate, and transitions the pipeline cleanly back to `InteractionState::Ready` (or `InteractionState::Idle` on fatal session halt), ensuring the pipeline is never stranded in `Working`.
8. **Dropped-Finish Hazard Resolution**: The pipeline event handler `on_llm_finished` (`pipeline::assistant::llm`) explicitly accepts `InteractionState::Working` alongside `Thinking` and `Speaking`. When LLM token streaming completes before the first packet of TTS response audio reaches physical playback, `VoxEvent::LlmFinished` must not be dropped. Conversation and database records must be reliably committed.

### 5.2 Audio Intent Classification
Every audio synthesis request dispatched to `TtsActor` and `PlaybackEngine` is explicitly tagged with an intent category:
1. **`AudioIntent::InterimFiller`**: A short, transitional phrase spoken to eliminate dead air while background cognitive work executes (e.g., *"Give me a moment while I organize our conversation"*).
2. **`AudioIntent::TurnResponse`**: The finalized conversational reply that answers the user's utterance.

### 5.3 Playback Engine Gating Rules
The audio playback engine enforces state transitions based on audio intent:
- **Interim Filler Playback**:
  - The pipeline state remains in `InteractionState::Working`.
  - The playback engine **does not** transition the state to `Speaking`.
  - When interim filler playback finishes, the playback engine **does not** emit a completion event that triggers `Ready`. The pipeline remains locked in `InteractionState::Working`.
- **Turn Response Playback**:
  - The playback engine transitions the state from `Working` (or `Thinking`) to `InteractionState::Speaking`.
  - When the final response audio finishes playing, the playback engine transitions the state to `InteractionState::Ready`.
- **Sequential Intent Boundary**: Physical audio playback ring buffer writes are sequenced such that interim filler audio drains before turn response audio samples hit the ring buffer, preventing atomic intent race conditions during overlapping synthesis.

---

---

## 6. Authoritative Turn Execution Contract (`execute_turn`)

The turn lifecycle is executed as an unbroken, deterministic sequence owned entirely by `Harness::execute_turn` in `services/harness/orchestrator.rs`:

### Phase 1: Intake & Deduplication Gate
1. ⬆️ **[UPSTREAM]**  
   - **What**: The voice pipeline adapter (`pipeline::assistant::transcript`) invokes `Harness::execute_turn(query, turn_id, cancel)`.  
   - **Input**: User utterance text `&str`, monotonic `turn_id: u64`, single-turn `cancel: CancellationToken`.  
   - **Owner**: `pipeline::assistant::transcript` $\to$ `services/harness/orchestrator.rs`.
2. 🔌 **[STAGE_CALL]**  
   - **What**: Deduplication check against trailing turn.  
   - **Owner**: `stages/history.rs` (`ConversationHistoryStage::is_duplicate_user_turn`).  
   - **Action**: Compares incoming query against the preceding user turn in working memory.  
   - **Branch**: If duplicate detected, early return `TurnOutcome::DuplicateIgnored` without advancing pipeline state.

### Phase 2: User Turn Staging & Token Budget Evaluation
3. 🔌 **[STAGE_CALL]**  
   - **What**: Stage active user query into working memory.  
   - **Owner**: `stages/history.rs` (`ConversationHistoryStage::push_user_turn`).  
   - **Invariant**: The user turn is staged *before* any cognitive maintenance, guaranteeing that fallback truncation never drops the active query.
4. 🔌 **[STAGE_CALL]**  
   - **What**: Real-time context capacity evaluation.  
   - **Owner**: `stages/budget.rs` (`ContextBudgetStage::evaluate_utilization`).  
   - **Calculation**: Sums tracked in-memory tokens across system prompt, injected memory, active summary, and staged history against usable window (`max_window - reserve_tokens`).  
   - **Branch**: Classifies state as `ContextStatus::Nominal` ($<85\%$) or `ContextStatus::Critical` ($\ge 85\%$).

### Phase 3: Generic Non-Terminal Phase Branch (Inline Compaction / Maintenance)
5. 🔀 **[COORDINATOR_DISPATCH]**  
   - **What**: Non-terminal operation evaluation.  
   - **Owner**: `Harness` orchestrator.  
   - **Branch A (Nominal $<85\%$ or History $<4$ messages)**: Skips maintenance; proceeds immediately to Phase 4.  
   - **Branch B (Critical $\ge 85\%$ & Eligible)**: Triggers intermediate non-terminal phase:
     - a. ⬇️ **[DOWNSTREAM]**: Emits pipeline transition `Thinking` $\to$ `InteractionState::Working`.
     - b. ⬇️ **[DOWNSTREAM]**: Dispatches localized filler phrase (Devanagari / English) to `TtsActor` as `AudioIntent::InterimFiller` to eliminate dead air.
     - c. 🔌 **[STAGE_CALL]**: Executes `stages/compaction.rs` directly via `Arc<dyn LlmProvider>` on an isolated blocking thread (`tokio::task::spawn_blocking`), enforcing the 6-bucket JsonSchema with 0 retries.
     - d. ⬇️ **[DOWNSTREAM]**: Persists extracted facts to Turso DB (`compactions` ledger) and stages facts to the memory ingestion queue.
     - e. 🔌 **[STAGE_CALL]**: Prunes older turn pairs in `stages/history.rs`, replacing them with structured `<session_context>`.
   - **Fail-Fast Degradation Branch (Compaction Failure or Ineligible)**:
     If the single compaction attempt fails or returns malformed output, the `Harness` fails fast:
     - a. Emits pipeline warning event (`PipelineImpact::Degraded`).
     - b. Executes deterministic FIFO sliding window shift (`stages/budget.rs` / `stages/history.rs`), dropping oldest turn pairs until context drops below $65\%$.
     - c. Retains the currently staged user query without data loss and proceeds immediately to Phase 4.

### Phase 4: Generation Request Assembly
6. 🔌 **[STAGE_CALL]**  
   - **What**: Root prompt synchronization and generation payload assembly.  
   - **Owner**: `stages/prompt.rs` (`PromptBuilderStage::build_generation_request`).  
   - **Input**: Persona prompt, bounded `<user_identity>` (20% ceiling), active `<session_context>`, and staged message history slice from `stages/history.rs`.  
   - **Output**: Fully structured `GenerationRequest` containing `[System Message 0, Historical Turns, Active User Query]`.  
   - **Invariant**: Message turns (1..N) are preserved intact; system prompt updates do not wipe active history.

### Phase 5: Duplex Model Pipe Dispatch
7. ⬇️ **[DOWNSTREAM]**  
   - **What**: Turn generation command dispatch.  
   - **Owner**: `Harness` $\to$ `services/llm/actor.rs`.  
   - **Action**: Creates per-turn one-shot response channel `(response_tx, response_rx)`. Captures single-turn `CancellationToken` by value.  
   - **Command**: Transmits `LlmCommand::Generate { request, response_tx, cancel_token }` across long-lived `llm_tx`.

### Phase 6: Egress Token Processing & Sentence Chunking
8. 🔌 **[STAGE_CALL]**  
   - **What**: Token stream demuxing, subtitle emission, and grammatical clause chunking.  
   - **Owner**: `streaming/router.rs` (`StreamRouter`) and `streaming/chunker.rs` (`ClauseChunker`).  
   - **Loop Actions**:
     - a. Reads raw tokens from `response_rx`.
     - b. Resolves leading tags in speculative demuxer buffer (or immediately flushes raw speakable text).
     - c. ⬇️ **[DOWNSTREAM]**: Emits clean subtitle tokens to active UI window via `IpcEvent::LlmToken`.
     - d. 🔌 **[STAGE_CALL]**: Accumulates tokens in `ClauseChunker`, enforcing adaptive word counts (clause 0: 5–12 words; clause 1: 10–20 words; steady state: 16–32 words) and applying prosody morphing (`.` $\to$ `,` under 5 words).
     - e. ⬇️ **[DOWNSTREAM]**: Dispatches complete clauses to `TtsActor` as `TtsCommand::Generate` tagged with `AudioIntent::TurnResponse`.
     - f. ⬇️ **[DOWNSTREAM]**: On first synthesized audio frame reaching hardware, `PlaybackEngine` transitions `Working`/`Thinking` $\to$ `InteractionState::Speaking`.

### Phase 7: Turn Finalization, Failure Recovery & Watcher Arming
9. 🔀 **[COORDINATOR_DISPATCH]**  
   - **What**: Stream conclusion, abort handling, and commit.  
   - **Owner**: `streaming/router.rs` and `Harness`.  
   - **Outcome Branches**:
     - **Branch A (Stream Disconnect / Provider Error without Finished)**:  
       Rolls back staged user query from `stages/history.rs`. Transitions `Working`/`Thinking` $\to$ `InteractionState::Ready`. Emits `VoxEvent::Error(PipelineError::TurnAborted)`. Never commits partial text as a complete turn. Returns `TurnOutcome::Error`.
     - **Branch B (Turn Cancelled via Token / Barge-In)**:  
       Rolls back staged user query. Transitions state to `Ready`. Never emits `VoxEvent::LlmFinished`. Returns `TurnOutcome::Cancelled`.
     - **Branch C (LlmResponse::Finished Succeeded)**:  
       - a. ⬇️ **[DOWNSTREAM]**: Flushes `ClauseChunker` remainder text to `TtsActor`.  
       - b. ⬇️ **[DOWNSTREAM]**: `StreamRouter` emits `VoxEvent::LlmFinished { turn_id, assistant_text }` onto `event_tx`, triggering Turso DB turn row persistence.  
       - c. 🔌 **[STAGE_CALL]**: Commits full assistant response into `stages/history.rs`.  
       - d. 🔌 **[STAGE_CALL]**: If post-turn utilization is in soft window ($65\% \le \text{utilization} < 85\%$), arms the 20-second quiet debounce watcher (`services/harness/watcher.rs`).  
       - e. ⬇️ **[DOWNSTREAM]**: Audio playback drains to empty $\to$ `PlaybackEngine` emits `PlaybackFinished`, transitioning `Speaking` $\to$ `InteractionState::Ready`.  
       - f. Returns `TurnOutcome::Completed { assistant_response, turn_id }` to caller.

---

### 6.1 Degraded FIFO Fallback Policy
If history holds three or fewer messages, or if the single inline compaction attempt fails or returns malformed JSON:
1. Inline LLM compaction is skipped or aborted immediately without blocking the conversational turn.
2. The orchestrator executes a deterministic **FIFO Sliding Window Shift**: oldest historical User/Assistant turn pairs are dropped iteratively from index 1 until context utilization drops below 65%.
3. The currently staged user query is strictly preserved at the end of the history buffer.
4. The orchestrator emits a degraded pipeline error event indicating fallback to FIFO truncation.
5. Turn execution proceeds immediately to Phase 4, guaranteeing that the user's voice response is never permanently blocked.

### 6.2 Reactive Opportunistic Soft Compaction Path ($65\% \le \text{Utilization} < 85\%$)
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

## 7. Strongly-Typed Prompt XML Tag Schema

To eliminate ad-hoc string formatting, prompt tags are governed by a strictly typed enum rather than loose string literals:

1. **`PromptTag::UserIdentity` (`<user_identity>...</user_identity>`)**:
   - **Content**: The active Personal Memory markdown document retrieved from the database.
   - **Placement**: Injected into the root System Prompt message, trailing base persona instructions.
   - **Budget Guard**: Bounded by the 20% system prompt share ceiling (`context_window * max_context_share`). Truncated deterministically if exceeded.
2. **`PromptTag::SessionContext` (`<session_context>...</session_context>`)**:
   - **Content**: The structured output of the latest compaction pass, containing both Bucket 1 (personal facts) and Bucket 2 (working session state: `objective`, `workdone`, `blocker`, `next_step`, `pitfall`).
   - **Placement**: Injected into the root System Prompt message, framing historical continuity for subsequent turns.
3. **`PromptTag::PastTurns` (`<past_turns>...</past_turns>`)**:
   - **Content**: Restored uncompacted historical dialog turns upon session continuation.
   - **Placement**: Encloses historical message turns preceding the active user query.
4. **Implementation Invariant**: All prompt assembly logic must utilize the strongly typed tag enum for opening and closing delimiters, guaranteeing syntactic determinism and preventing raw string drift.

---

## 8. Future-Proofing: Tool Calling & Demuxer Slots

While deferred from the current refactoring phase, the architecture formally reserves slots for future streaming capabilities without interface breakage:

### 8.1 On-Demand Agentic Tool Calling Loop (Non-Normative / Conceptual Target)
Future episodic memory retrieval and external CLI execution will operate on-demand via the model's output stream without altering the outer `Harness::execute_turn` interface:
1. The model generates an interim conversational tag followed by a tool request tag:
   ```
   <response>Searching through your project notes...</response>
   <tool name="search_episodic_memory">authentication refactor</tool>
   ```
2. The egress stream layer demuxes the stream:
   - `<response>` tokens route immediately to Text-to-Speech as `AudioIntent::InterimFiller`.
   - The pipeline enters `InteractionState::Working`.
   - `<tool>` tokens are intercepted by the Harness.
3. The Harness pauses inference, executes the tool against the database, and captures retrieved facts.
4. The Harness appends tool results as a context observation and transmits an augmented payload back across the duplex pipe to the `LlmActor`.
5. The `LlmActor` generates the final conversational response, which streams to TTS as `AudioIntent::TurnResponse`.

### 8.2 Streaming Tag Demuxer Slot
The egress stream layer reserves an integration point (`streaming/demuxer.rs`) for a zero-allocation streaming state machine. When activated in a future phase, it will distinguish untagged raw text from structured tag streams (`<title>`, `<response>`, `<tool>`), routing subtitle metadata and audio clauses cleanly without tag leakage into speech synthesis.
- **Upstream Layer Placement**: The streaming demuxer sits strictly **upstream of both** the `ClauseChunker` (audio synthesis) and the UI subtitle emitter (`IpcEvent::LlmToken`). Output tokens never reach audio or subtitle channels prior to demuxer filtering.
- **Speculative Buffer Bounds**: The demuxer buffers initial incoming tokens speculatively up to a bounded maximum ceiling of 64 tokens (sufficient to resolve delimiter prefixes and tag names).
- **Leading Tag Resolution Invariant**: The streaming demuxer must buffer initial tokens speculatively and definitively resolve the leading tag before dispatching any textual payload downstream to the TTS clause chunker or conversation history. If the leading token sequence does not match a registered tag delimiter (e.g., the first non-whitespace character is not `<`, or the prefix fails match against registered tags within the bounded window), the speculative buffer is immediately flushed downstream to both the UI subtitle emitter and the TTS clause chunker as raw speakable text. This guarantees zero latency overhead for untagged responses while definitively preventing premature audio playback or tag leakage during tag evaluation.
- **Mid-Stream Stream End Handling**: If the token stream concludes (`LlmResponse::Finished`) or disconnects while tokens reside in the speculative buffer without resolving a registered tag, the buffer is flushed immediately to UI subtitles and conversation history as raw text.

---

## 9. Subsystem Organization & Architectural Invariants

### 9.1 Directory Structure
The `services/harness/` subsystem is organized strictly by role:

```
services/harness/
├── mod.rs                 # Domain constants, Role, ChatMessage, PromptTag, TurnOutcome
├── orchestrator.rs             # Harness: The SINGLE orchestrator (owns execute_turn)
├── stages/                # Pure input->output stages (never call each other)
│   ├── history.rs         # ConversationHistoryStage: in-memory FIFO buffer & dedup
│   ├── prompt.rs          # PromptBuilderStage: persona + memory + GenerationRequest assembly
│   ├── budget.rs          # ContextBudgetStage: token counting, limits, FIFO shifts
│   └── compaction.rs      # CompactionStage: inline summarization & DB fact staging
└── streaming/             # Egress stream processing
    ├── chunker.rs         # ClauseChunker: punctuation, prosody morphing, split boundaries
    ├── router.rs          # StreamRouter: TTS clause dispatch & UI subtitle IPC
    └── demuxer.rs         # (Deferred Slot) Streaming tag demuxer state machine
```

### 9.2 Durable Subsystem Invariants
1. **Zero Backward Compatibility (ZBC)**: No legacy wrappers or compatibility bridges will be retained.
2. **Sacred Audio Hot Path**: Zero memory allocations, zero locks (`Mutex`/`RwLock`), and zero disk or database operations are permitted on the CPAL audio callback or real-time VAD processing threads. All harness operations occur on Tokio tasks or dedicated background workers.
3. **Lock Discipline Across Await Points**: A `Mutex` or `RwLock` guard protecting conversational state must **never** be held across an `.await` boundary, particularly during LLM inference or database transactions.
4. **Decoupled Actor Invariant**: The `LlmActor` must never import or interact with audio channels, clause accumulators, or UI IPC emitters. It is strictly a token-generating worker.
5. **Central Authority for Turn Completion**: `VoxEvent::LlmFinished` is emitted exclusively by the `StreamRouter` upon complete conclusion of all turn token streaming.
6. **Strict Single-Orchestrator Encapsulation**: `Harness` is the sole sequencing orchestrator. External callers (`pipeline::assistant::transcript`) must interact exclusively through `execute_turn`. They must never inspect, clone, or invoke internal stages (`stages/`) or streaming components (`streaming/`) directly.
7. **Audio Layer Boundary Invariant**: `services/tts/actor.rs` owns physical audio synthesis only. Clause chunking, prosody punctuation morphing, and abbreviation guards belong exclusively in `services/harness/streaming/chunker.rs`. Audio workers must never contain NLP text-splitting logic.
8. **Autonomous Reactive Quiet Watcher & History Pruning**: The `QuietCompactionWatcher` is self-governing; it monitors pipeline state transitions via `state_rx` and `CancellationToken` directly, automatically aborting when the pipeline leaves `Ready`/`Paused` without imperative caller invocation. Upon successful completion of background soft compaction, working history must be pruned and replaced with the structured `<session_context>` containing the entire compaction output (both personal and working session buckets) to ensure token utilization drops while retaining complete session fidelity for subsequent turns.
9. **Single-Turn Cancellation Token Discipline**: Turn cancellation tokens (`CancellationToken`) are captured by value per turn and threaded through LLM generation, stream routing, and TTS synthesis. Cancelled turns must never emit `VoxEvent::LlmFinished`.
10. **Pipeline Turn Accumulator Boundary**: `TurnAccumulator` (in `pipeline/assistant/accumulator.rs`) remains the pipeline-level turn accumulation container for interruption recovery and partial turn snapshotting. During turn execution, `StreamRouter` writes incoming speakable text into the accumulator.
