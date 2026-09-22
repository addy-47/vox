# Architectural Specification: LLM Agent Harness Runtime

> **Document Type:** System Architectural Specification  
> **Target Subsystem:** `services/harness/` and `services/llm/actor.rs`  
> **Status:** Approved Target Architecture  
> **Format Rule:** Pure architectural specification containing zero code snippets. All behaviors, state transitions, domain invariants, thresholds, and subsystem responsibilities are specified with rigorous technical precision for planning agents.

---

## 1. Scope, Philosophy & Core Invariants

### 1.1 Scope Boundaries
This specification defines the architectural design of the Vox LLM Agent Harness. The scope is strictly bounded to:
1. **Single-Orchestrator Turn Execution**: Consolidating all conversational turn sequencing, state evaluation, and actor coordination into a single runtime orchestrator (`Harness` in `services/harness/orchestrator.rs`).
2. **Narrow-Interface Stages**: Decoupling memory, budgeting, prompt assembly, and compaction into isolated, pure input-in/output-out stages (`stages/`).
3. **Decoupled Egress Stream Processing**: Elevating token stream consumption, grammatical clause chunking, prosody punctuation morphing, TTS dispatch, and UI subtitle emissions out of both the LLM actor and the TTS actor into the Harness egress stream layer (`streaming/`).
4. **Agentic Tool Execution Chassis**: Integrating the cognitive tool registry and multi-turn reentrant execution loop into `Harness::execute_turn`, supporting single-pass Terminal tools and reentrant Non-Terminal tools with strict audio isolation.

### 1.2 The Foundational Axiom: Agent = Model + Harness
Vox treats conversational intelligence as a two-tier system:
1. **The Model Layer (`LlmActor`)**: A pure, stateless compute engine. It accepts a structured conversation payload and produces an asynchronous stream of raw tokens. It possesses zero knowledge of audio devices, text-to-speech actors, user interface windows, clause chunking heuristics, or conversational state machines.
2. **The Harness Layer (`Harness`)**: The conversational orchestrator and runtime environment. It manages dialog memory, enforces token budgets, injects personalization, drives multi-step cognitive loops, routes output tokens to audio synthesis, and dictates pipeline state transitions.

### 1.3 Architectural & Subsystem Invariants
Every component, stage, and actor within the harness runtime must strictly satisfy these foundational invariants:
1. **Single Orchestrator Authority & Encapsulation**: Exactly one orchestrator (`Harness` in `services/harness/orchestrator.rs`) owns turn sequencing, state transitions, and stage coordination. External callers (`pipeline::assistant::transcript`) interact exclusively via `Harness::execute_turn`. They must never inspect, clone, or invoke internal stages (`stages/`) or streaming components (`streaming/`) directly.
2. **Narrow, Typed Stage Interfaces & Zero Inter-Stage Communication**: Each stage accepts typed input structs and returns typed output structs. Stages never import, call, or communicate with each other; the orchestrator mediates all data flow.
3. **Exclusive State Ownership**: Each stage owns its state exclusively. There is zero shared mutable state or simultaneous multi-actor mutation across the harness runtime.
4. **Sacred Audio Hot Path**: Zero memory allocations, zero locks (`Mutex`/`RwLock`), and zero disk or database operations are permitted on the CPAL audio callback or real-time VAD processing threads. All harness operations execute on Tokio async tasks or dedicated background worker threads.
5. **Domain-Pure Layer Placement**: Text processing (token accumulation, clause chunking, prosody morphing) belongs strictly in the harness streaming layer (`streaming/`). Audio processing (voice conditioning, sample synthesis, buffer playback) belongs strictly in TTS and audio playback subsystems (`services/tts/actor.rs`, `PlaybackEngine`). Audio workers must never contain NLP text-splitting logic.
6. **Lock Discipline Across Await Points**: A `Mutex` or `RwLock` guard protecting conversational, stage, or orchestrator state must **never** be held across an `.await` boundary, particularly during LLM inference, tool execution, or database transactions.
7. **Decoupled Stateless Model Worker**: `LlmActor` is strictly a stateless compute engine accepting structured generation requests and emitting normalized token/event streams (`TextDelta`, `ToolCall`, `Finished`, `Error`). It possesses zero knowledge of audio devices, text-to-speech actors, user interface windows, clause chunking heuristics, or conversational state machines.
8. **Central Authority for Turn Completion**: `VoxEvent::LlmFinished` is emitted exclusively by `Harness` once after the **terminal** model pass concludes. Cancelled or aborted turns must never emit `VoxEvent::LlmFinished`.
9. **Single-Turn Cancellation Token Discipline**: Turn cancellation tokens (`CancellationToken`) are captured by value per turn and threaded through LLM generation, tool execution, stream routing, and TTS synthesis.
10. **Autonomous Reactive Quiet Watcher & History Pruning**: `QuietCompactionWatcher` is self-governing; it monitors pipeline state transitions via `state_rx` and `CancellationToken` directly, automatically aborting when the pipeline leaves `Ready`/`Paused` without imperative caller invocation. Upon successful completion of background soft compaction, working history must be pruned and replaced with structured `<session_context>` (both personal and working session buckets) to drop token utilization while retaining complete session fidelity.
11. **Pipeline Turn Accumulator Boundary**: `TurnAccumulator` (in `pipeline/assistant/accumulator.rs`) remains the pipeline-level turn accumulation container for interruption recovery and partial turn snapshotting. During turn execution, `StreamRouter` writes incoming speakable text into the accumulator.
12. **Turn-Local Ephemeral Scratchpad**: `Harness::execute_turn` maintains a turn-local scratchpad (`Vec<ChatMessage>`) for intermediate tool calls and observations during active multi-pass execution. At turn commit (or cancellation), the scratchpad is dropped; working `ConversationHistoryStage` and the Turso `turns` table store exclusively committed `User` and `Assistant` turns. Tool invocations are durably recorded in `session_tool_calls` for auditing and future rollback, guaranteeing 100% parity between active working memory and database-restored memory.
13. **Single-Writer Barge-In Persistence**: The `Harness::Cancelled` branch is the sole persistence writer for interrupted turns. It inspects pass-local partial text: if `!partial.trim().is_empty()`, it commits `(user_query, partial)` to history and emits `PersistenceEvent::TurnCompleted`; if empty, it rolls back the user query from history and skips persistence. The turn-local scratchpad is unconditionally discarded.
14. **Turn Synthesis Guard & Latch (`turn_open` & `drained_while_open`)**: The router sets `turn_open = true` at turn onset, and clears it on all terminal outcomes (`LlmFinished`, `Cancelled`, `Error`, interrupt, `End`, `Pause`). `on_playback_finished` sets `drained_while_open = true` if `turn_open == true` (instead of prematurely transitioning to `Ready`). `on_llm_finished` evaluates the latch and transitions to `Ready` if `pending_synthesis_jobs == 0`. On `PlaybackStarted`, the latch is cleared.
15. **Session Lifecycle & Self-Healing Foreign Keys**: `session_id` is minted at session engage as an epoch timestamp (`u32` monotonic turn IDs) and persisted via `SessionStarted`. All tool-call ledger writes and title updates execute idempotent `INSERT OR IGNORE INTO sessions` self-healing to eliminate FK failures if `SessionStarted` is dropped by channel backpressure. Title updates are awaited in the persistence layer before emitting `IpcEvent::SessionsChanged` (`sessions_changed`).
16. **Discovery Probe Non-Blocking FSM (`session_starting`)**: The capability discovery probe runs asynchronously off the router thread under a `session_starting = true` flag. The pipeline state remains `Idle` (not `Ready`), and `owner = Assistant` is set only after probe completion, ensuring dictation is never blocked.
17. **Terminal Tool Single-Pass Resolution**: Terminal tools declare a required `spoken_response` parameter. Upon invocation, the harness dispatches `spoken_response` directly to speech synthesis as `AudioIntent::TurnResponse` while executing the tool action in the background, resolving the turn in a single generation pass with zero second-pass LLM latency and zero ghost background tasks.
18. **Zero Backward Compatibility (ZBC)**: No legacy wrappers, transitional bridges, or compatibility shims will be retained. Redesign cleanly to the approved spec.

---

## 2. System Boundaries & Integration Contracts

The voice pipeline router treats `Harness` as the single cognitive front door:
```
[Audio In] ──► [VAD] ──► [STT] ──► [Harness (Orchestrator)] ──► [TTS Actor] ──► [PlaybackEngine]
                                         ▲ │        │
                      Duplex Session Pipe│ │        ├─► [ToolExecutor] ──► [session_tool_calls DB]
                      Normalized Events  │ │        │         │
                                         │ ▼        │         └─► Reentrant Turn Loop (NonTerminal)
                                   [LlmActor]       ▼
                                            [StreamRouter] ──► [TextNormalizer] ──► [ClauseChunker]
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
- **Model Layer (`LlmActor`)**: The harness transmits `GenerationRequest` (including active tool schemas) over the duplex session pipe and receives an asynchronous normalized stream of `LlmResponse` events (`TextDelta`, `ToolCall`, `Finished`, `Error`).
- **Tool Execution & Persistence (`ToolExecutor` & `Turso DB`)**: The harness executes invoked tools, stages execution records to `session_tool_calls`, and drives multi-turn observation loops.
- **Speech Synthesis (`TtsActor`)**: The harness egress stream stage dispatches finished speakable clauses to the TTS worker thread via `TtsCommand::Generate { text, intent, turn_id }`. The TTS actor is strictly an audio worker; it contains zero clause-chunking, punctuation, or text-parsing logic.
- **Audio Playback (`PlaybackEngine`)**: Receives synthesized PCM samples from the TTS actor and coordinates physical speaker playback, emitting state transition events (`Speaking`, `Ready`).
- **Frontend IPC (`IpcEvent`)**: The harness egress stream stage emits clean subtitle tokens (`IpcEvent::LlmToken`) directly to the active UI window.
- **Persistence (`Turso DB`)**: Records compaction runs, memory facts, turns, and tool scratchpad entries to the local database ledger.

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
  - If `sessionId == None`: The user is engaging for a fresh session. The `Harness` boots with a fresh prompt (minted epoch timestamp `conv_id`, with `SessionStarted` persisted on engage; zero-turn sessions are swept on clean exit or restart).
  - Clicking "+ New Session" (`create_session`) simply clears the active session selection in the UI; subsequent engagement sends `start_session(None)`.

### 3.3 Domain Gating Configuration
Different interaction domains mount specific subsets of the harness stages:

| Subsystem Component | Modular Assistant (`PipelineMode::Modular`) | Realtime S2S (`PipelineMode::Realtime`) | Dictation (`InteractionState::Sleeping`) |
| :--- | :---: | :---: | :---: |
| **ConversationHistoryStage** | **Active**: Full FIFO dialog buffer | **Active**: History logging for UI rail | **None**: No conversational memory |
| **PromptBuilderStage** | **Active**: Persona + Personal Memory | **Active**: Session persona initialization | **None**: No prompt construction |
| **ContextBudgetStage** | **Active**: 65% soft / 85% critical checks | **None**: Context managed server-side | **None**: No token budgeting |
| **CompactionStage** | **Active**: Inline & Opportunistic | **None**: Context managed server-side | **None**: No local compaction |
| **ToolExecutionStage** | **Active**: Dual terminal/non-terminal loop $\to$ TTS | **Active**: Realtime registry projected $\to$ `RealtimeActor` WS | **None**: No tool execution |
| **StreamRouter & Chunker** | **Active**: Token clause chunking $\to$ TTS | **None**: Provider outputs PCM audio | **None**: STT transcript $\to$ OS typing |

---

## 4. The Functional Stages Taxonomy

The `Harness` orchestrator coordinates six distinct, decoupled functional stages:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                  HARNESS ORCHESTRATOR                                  │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 1. ConversationHistoryStage (`stages/history.rs`)                                      │
│    • In-memory message sequence (System, User, Assistant, Tool)                        │
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
│    • Dynamic tool schema injection from ToolRegistry based on session/turn policy      │
│    • GenerationRequest payload assembly from history slice and active user query       │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 4. CompactionStage (`stages/compaction.rs` & `watcher.rs`)                             │
│    • Structured cognitive summarization execution via isolated provider compute        │
│    • Universal 6-bucket JsonSchema enforcement & fact staging to Turso DB              │
│    • Reactive 20-second debounced soft compaction watcher                              │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 5. ToolExecutionStage (`stages/tools/{registry.rs, executor.rs}`)                      │
│    • Dual tool dispatch: Terminal (single-pass voice + action) vs NonTerminal (loop)   │
│    • Scratchpad persistence logging to session_tool_calls table                        │
│    • Ingress tool schema filtering and single non-terminal retrieval pass (Phase 12.1) │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 6. Egress Stream Stage (`streaming/chunker.rs`, `router.rs`, `normalizer.rs`)          │
│    • Live UI subtitle emission via IpcEvent::LlmToken                                  │
│    • Speech normalization: abbreviation expansion, symbol stripping (normalizer.rs)   │
│    • Clause chunking accumulator and punctuation prosody morphing (chunker.rs)         │
│    • Text-to-Speech audio command dispatch tagged with AudioIntent                     │
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

1. **Stage Declaration**: Any stage within `stages/` (e.g., `CompactionStage` or `ToolExecutionStage`) can declare a non-terminal cognitive phase (`NonTerminalPhase`). Crucially:
   - **`ToolFlow::NonTerminal`**: Triggers `NonTerminalPhase`.
   - **`ToolFlow::Terminal`**: Does NOT trigger `NonTerminalPhase`. Terminal tools include `spoken_response` in their parameter schema; the Harness executes the action and dispatches `spoken_response` directly to `TtsActor` as `AudioIntent::TurnResponse`, resolving the turn in a single pass.
2. **Uniform Transition**: When a non-terminal tool is triggered, the `Harness` immediately transitions the voice pipeline from `InteractionState::Thinking` to `InteractionState::Working`.
3. **Uniform Audio Cue**: The `Harness` extracts `spoken_filler` from the non-terminal tool arguments (with fallback to `select_filler_phrase`) and dispatches it to `TtsActor` tagged as `AudioIntent::InterimFiller` to eliminate dead air.
4. **Uniform Execution**: The `Harness` executes the operation asynchronously with executor isolation.
5. **State Locking**: The pipeline remains locked in `InteractionState::Working` throughout the intermediate execution. Playback of interim filler audio does not transition the pipeline to `Speaking` or `Ready`.
6. **Resumption & Reentrant Loop**: Once the non-terminal operation completes, the `Harness` stages the `ToolResult` into working memory and initiates a reentrant generation pass across the duplex pipe without leaving `Working` until finalized response audio begins physical playback (`Working` $\to$ `Speaking` $\to$ `Ready`).
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
   - **Input**: User utterance text `&str`, monotonic `turn_id: u32`, single-turn `cancel: CancellationToken`.  
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
   - **Invariant**: The user turn is staged *before* any cognitive maintenance, guaranteeing that fallback truncation never drops the active query. Initial turn allocates a fresh turn-local `scratchpad: Vec<ChatMessage>`.
4. 🔌 **[STAGE_CALL]**  
   - **What**: Real-time context capacity evaluation.  
   - **Owner**: `stages/budget.rs` (`ContextBudgetStage::evaluate_utilization`).  
   - **Loop Cycle Invariant**: Turn intake (Step 3 `push_user_turn`) executes exactly once per turn, outside the reentrant loop. The reentrant cognitive loop is a self-contained cycle — **budget (Step 4) $\to$ assembly (Phase 4) $\to$ dispatch (Phase 5) $\to$ stream (Phase 6) $\to$ tools (Phase 6 Case B)** — owned solely by `orchestrator/loop.rs`. Loop iterations re-enter at Step 4 and never re-stage the user turn.
   - **Calculation**: Sums tracked in-memory tokens across system prompt, injected memory, active summary, staged history, registered tool schemas, and active scratchpad against usable window (`max_window - reserve_tokens`).  
   - **Branch**: Classifies state as `ContextStatus::Nominal` ($<85\%$) or `ContextStatus::Critical` ($\ge 85\%$).

### Phase 3: Generic Non-Terminal Phase Branch (Inline Compaction / Maintenance)
5. 🔀 **[COORDINATOR_DISPATCH]**  
   - **What**: Non-terminal operation evaluation.  
   - **Owner**: `Harness` orchestrator.  
   - **Filler Policy**: Gated by `has_played_filler` for harness-owned compaction filler only; executed **at most once per turn**. Model-provided `spoken_filler` in tool calls (§8.2) is not gated by `has_played_filler`.
   - **Branch A (Nominal $<85\%$ or History $<4$ messages)**: Skips maintenance; proceeds immediately to Phase 4.  
   - **Branch B (Critical $\ge 85\%$ & Eligible)**: Triggers intermediate non-terminal phase:
     - a. ⬇️ **[DOWNSTREAM]**: Emits pipeline transition `Thinking` $\to$ `InteractionState::Working`.
     - b. ⬇️ **[DOWNSTREAM]**: Dispatches localized filler phrase (Devanagari / English) to `TtsActor` as `AudioIntent::InterimFiller` if not already spoken.
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
   - **What**: Root prompt synchronization, active tool schema injection, and generation payload assembly.  
   - **Owner**: `stages/prompt.rs` (`PromptBuilderStage::build_generation_request`).  
   - **Input**: Persona prompt, bounded `<user_identity>` (20% ceiling), active `<session_context>`, staged message history slice from `stages/history.rs`, candidate tool schemas from `tools/registry.rs`, and the turn-local ephemeral `scratchpad`.  
   - **Output**: Fully structured `GenerationRequest` containing `[System Message 0, Historical Turns, Active User Query, Scratchpad Tool Interactions, Registered Tools]`.

### Phase 5: Duplex Model Pipe Dispatch
7. ⬇️ **[DOWNSTREAM]**  
   - **What**: Turn generation command dispatch across long-lived session pipe.  
   - **Owner**: `Harness` $\to$ `services/llm/actor.rs`.  
   - **Action**: Creates per-turn one-shot response channel `(response_tx, response_rx)`. Captures single-turn `CancellationToken` by value.  
   - **Command**: Transmits `LlmCommand::Generate { request, response_tx, cancel_token }` across long-lived `llm_tx`.

### Phase 6: Egress Stream Demuxing, Normalization & Tool Interception
8. 🔌 **[STAGE_CALL]**  
   - **What**: Consumes normalized `LlmResponse` events from `response_rx`.  
   - **Owner**: `streaming/router.rs` (`StreamRouter`), `streaming/normalizer.rs` (`TextNormalizer`), and `streaming/chunker.rs` (`ClauseChunker`).  
   - **Event Routing Loop**:
     - **Case A: `LlmResponse::TextDelta(token)`**:
       - a. ⬇️ **[DOWNSTREAM]**: Emits clean subtitle tokens to active UI window via `IpcEvent::LlmToken`.
       - b. 🔌 **[STAGE_CALL]**: Appends token to pass-local accumulator.
       - c. 🔌 **[STAGE_CALL]**: Normalizes completed clause string via `TextNormalizer::normalize_for_speech`.
       - d. 🔌 **[STAGE_CALL]**: Feeds normalized text into `ClauseChunker`, enforcing adaptive word bounds and prosody morphing (`.` $\to$ `,` under 5 words).
       - e. ⬇️ **[DOWNSTREAM]**: Dispatches complete clauses to `TtsActor` as `TtsCommand::Generate` tagged with `AudioIntent::TurnResponse`.
       - f. ⬇️ **[DOWNSTREAM]**: On first synthesized audio frame reaching hardware, `PlaybackEngine` transitions `Working`/`Thinking` $\to$ `InteractionState::Speaking`.
     - **Case B: `LlmResponse::ToolCall(call)`**:
        - Bypasses standard streaming text path. Evaluates tool classification via `ToolRegistry`:
          - **Pre-Tool Partial Text Handling (Drop-All-Prefix)**:
            - If preceded by non-empty partial text in the same pass, the Harness discards it entirely: no chunker flush, no TTS dispatch for either `Terminal` or `NonTerminal` tools. Only `spoken_response` (`Terminal`) or `spoken_filler` (`NonTerminal`) may be synthesized.
            - **Buffering Requirement**: the egress stream layer must buffer clause dispatch within a pass until the pass outcome is known (`Completed` vs `ToolCallReceived`), so prefix text streamed before a tool call is never already in the TTS queue when the tool call arrives. Eager per-token TTS dispatch that cannot be recalled violates this contract.
          - **If `ToolFlow::Terminal` (e.g. `respond_and_set_title`)**:
            - Extracts required `spoken_response` parameter from arguments.
            - Immediately routes `spoken_response` through `TextNormalizer` $\to$ `ClauseChunker` $\to$ `TtsActor` as `AudioIntent::TurnResponse`. Conversational voice begins playback with optimal clause-level TTFA, zero delay, and zero 2-pass latency.
            - Pipeline transitions directly `Thinking` → `Speaking`. Zero state transition to `Working`.
            - Tool execution action runs asynchronously (e.g. persists title via awaited DB write, then emits `IpcEvent::SessionsChanged`).
            - Logs invocation and outcome to `session_tool_calls` scratchpad ledger.
            - Resolves the conversational turn in a single pass. The tool call and its response are excluded from subsequent turn prompt histories (only `spoken_response` text is committed to `ConversationHistoryStage`).
          - **If `ToolFlow::NonTerminal` (e.g. `search_memory`)**:
            - Pipeline transitions immediately from `Thinking` to `InteractionState::Working` (triggering `NonTerminalPhase`).
            - Extracts required `spoken_filler` parameter from arguments (e.g. *"Checking your notes on that..."*) and dispatches it to `TtsActor` as `AudioIntent::InterimFiller` to eliminate dead air (falling back to localized filler phrases if missing). Not subject to `has_played_filler` compaction suppression.
            - Executes tool action with a strict per-tool timeout (`TOOL_EXECUTION_TIMEOUT = 10s`).
            - Logs invocation and result to `session_tool_calls` scratchpad ledger.
            - Appends `ChatMessage { role: Role::Assistant, tool_calls: Some(vec![call]), ... }` and `ChatMessage { role: Role::Tool, tool_call_id, content: result }` to the **turn-local ephemeral scratchpad** (never to permanent working history).
            - **REENTRANT COGNITIVE LOOP**: Harness appends the tool observation to the turn-local ephemeral scratchpad and iterates the loop cycle defined in Phase 2 Step 4 (budget $\to$ assembly $\to$ dispatch $\to$ stream $\to$ tools).
            - **Recursion Guard**: Iterations bounded by `MAX_TOOL_ITERATIONS = 5`. On breach, executes one final generation pass with `tools = None` to produce a spoken summary rather than aborting.

### Phase 7: Turn Finalization, Commit & Watcher Arming
9. 🔀 **[COORDINATOR_DISPATCH]**  
   - **What**: Stream conclusion, abort handling, and commit.  
   - **Owner**: `streaming/router.rs` and `Harness`.  
   - **Outcome Branches**:
     - **Branch A (Stream Disconnect / Provider Error without Finished)**:  
       Rolls back staged user query from `stages/history.rs`. Discards turn-local scratchpad. Transitions `Working`/`Thinking` $\to$ `InteractionState::Ready`. Emits `VoxEvent::Error(PipelineError::TurnAborted)`. Returns `TurnOutcome::Error`.
     - **Branch B (Turn Cancelled via Token / Barge-In)**:  
       Inspects pass-local partial assistant text from `StreamPassOutcome::Cancelled { partial }`.
       - If `!partial.trim().is_empty()`: commits `(user_query, partial)` to `stages/history.rs` and dispatches `PersistenceEvent::TurnCompleted`.
       - If `partial.trim().is_empty()`: rolls back user query from `stages/history.rs` and skips persistence.
       - The turn-local ephemeral scratchpad is unconditionally discarded. Transitions state to `Ready` (or `Listening` if barge-in). Returns `TurnOutcome::Cancelled`.
     - **Branch C (LlmResponse::Finished Succeeded)**:  
       - a. ⬇️ **[DOWNSTREAM]**: Flushes `ClauseChunker` remainder text to `TtsActor`.  
       - b. ⬇️ **[DOWNSTREAM]**: If `assistant_text.trim().is_empty()`: rolls back user turn from `stages/history.rs`, skips `TurnCompleted`, and returns `TurnOutcome::Completed`.
       - c. ⬇️ **[DOWNSTREAM]**: If `!assistant_text.trim().is_empty()`: `Harness` emits `VoxEvent::LlmFinished { turn_id, assistant_text }` once, triggering Turso DB turn row persistence, and commits `(user_query, assistant_text)` into `stages/history.rs`. The turn-local ephemeral scratchpad is dropped.
       - d. 🔌 **[STAGE_CALL]**: If post-turn utilization is in soft window ($65\% \le \text{utilization} < 85\%$), arms the 20-second quiet debounce watcher (`services/harness/watcher.rs`).  
       - e. ⬇️ **[DOWNSTREAM]**: Audio playback drains to empty $\to$ `PlaybackEngine` emits `PlaybackFinished`, evaluating `drained_while_open` latch and transitioning `Speaking` $\to$ `InteractionState::Ready`.  
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

## 8. Agentic Tool Calling & Harness Consumption Model

The Harness acts as the single orchestrator and runtime consumer for all agentic tools specified in `docs/specs/tools-spec.md`.

### 8.1 Ingress Capability Gating & Notification
- **Discovery Check**: At session boot (`start_session`), the Harness reads the persistent `model_capabilities.json` cache.
- **Blocking Discovery Probe**: If unprobed, a synchronous capability probe runs with a strict 4.0-second timeout (`SESSION_BOOT_PROBE_TIMEOUT = 4s`).
- **State Boundary**: The pipeline remains in transition and does NOT emit `state_changed(Ready)` until capability resolution completes.
- **Degradation**: If the probe times out or errors, capability defaults to `supports_tools = false`, all tool schemas are suppressed from `PromptBuilderStage`, and a persistent system notification (`category: "system"`) is dispatched to inform the user that the model is running in text-only conversational mode.

### 8.2 Tool Consumption & Execution Routing
When `LlmActor` returns `LlmResponse::ToolCall(call)` over the duplex pipe, `StreamRouter` routes the event directly to `Harness::execute_turn`, completely bypassing the audio synthesis path:
1. **`ToolFlow::Terminal` (e.g. `respond_and_set_title`)**:
   - The pipeline transitions directly from `Thinking` to `Speaking`. Zero transition to `InteractionState::Working`.
   - `spoken_response` is extracted from arguments and immediately routed through `TextNormalizer` $\to$ `ClauseChunker` $\to$ `TtsActor` as `AudioIntent::TurnResponse` — optimal clause-level TTFA, no dead air, no 2-pass latency.
   - Tool side-effect (e.g. title persistence) runs asynchronously, then emits `IpcEvent::SessionsChanged`.
   - Invocation is logged to `session_tool_calls`. Excluded from subsequent turn prompts (only `spoken_response` is committed to history).
2. **`ToolFlow::NonTerminal` (e.g. `search_memory`)**:
   - The pipeline transitions from `Thinking` to `InteractionState::Working`.
   - `spoken_filler` is extracted from arguments and dispatched as `AudioIntent::InterimFiller` to eliminate dead air (falling back to localized filler phrases if missing). Not gated by `has_played_filler`.
   - The tool executes under `TOOL_EXECUTION_TIMEOUT = 10s` (applying user-configured `top_k_facts` and `semantic_similarity_cutoff` thresholds from settings).
   - Execution is logged to the `session_tool_calls` scratchpad.
   - The structured `ToolResult` is appended to the turn-local scratchpad, triggering a reentrant generation pass to produce the final conversational response.

### 8.3 Interim Filler Source (Non-Terminal Tools)
- `spoken_filler` is a required parameter in every `NonTerminal` tool schema. The model provides a context-aware 3–5 word filler phrase alongside the tool arguments (e.g. *"Checking your project notes..."*).
- The Harness dispatches the model-provided `spoken_filler` to `TtsActor` as `AudioIntent::InterimFiller` immediately upon tool invocation, before awaiting the result.
- If `spoken_filler` is empty or missing (e.g. model failure or compaction path), the Harness falls back to a localized filler phrase set (Devanagari / English).

### 8.4 Ingress Tag Demuxer Shim (Local & Non-Native Models)
- For cloud models with native tool streaming (OpenAI, Gemini), tool calls are parsed directly from structured streaming frames.
- For local GGUF models or models emitting textual syntax (e.g. `<tool_call>`), a streaming ingress filter resides strictly **within the provider adapter layer**. It intercepts raw tags and translates them into canonical `LlmStreamEvent::ToolCall` events before the stream reaches the Harness. The Harness egress layer never performs string-level tag parsing.

---

## 9. Subsystem Organization & Architectural Invariants

### 9.1 Directory Structure
The `services/harness/` subsystem is organized cleanly and flatly by role:

```
services/harness/
├── mod.rs                 # Domain constants, Role, ChatMessage, PromptTag, TurnOutcome, TurnExecutionRequest
├── chassis.rs             # Harness struct, constructors, session-scoped state. No turn sequencing.
├── loop.rs                # Sole turn-sequencing coordinator: execute_turn() + reentrant budget→assemble→dispatch→stream→tools cycle
├── steps.rs               # Sequential phase step functions: Step 1 (Intake), Step 2 (Budget), Step 3 (Compaction & Working), Step 4 (Assemble), Step 5 (Dispatch), Step 6 (Stream & Tools), Step 7 (Finalize)
└── stages/                # Pure domain stages (never call each other; consumed by steps.rs)
    ├── history.rs         # ConversationHistoryStage: in-memory FIFO buffer & dedup
    ├── prompt.rs          # PromptBuilderStage: persona + memory + GenerationRequest assembly
    ├── budget.rs          # ContextBudgetStage: token counting, limits, FIFO shifts
    ├── compaction.rs      # CompactionStage: inline summarization & DB fact staging
    ├── streaming/         # Egress stream processing (chunker, router, normalizer)
    └── tools/             # ToolExecutionStage: registry, executor, title, memory
        ├── mod.rs         # Tool trait, ToolDefinition, ToolFlow, ToolResult
        ├── registry.rs    # ToolRegistry: dynamic injection, name lookups, capability gate
        ├── executor.rs    # ToolExecutor: Terminal single-pass + NonTerminal reentrant dispatch
        ├── title.rs       # RespondAndSetTitleTool (Terminal implementation)
        └── memory.rs      # MemorySearchTool (NonTerminal implementation)
```

### 9.2 Layered Communication Hierarchy

Communication strictly obeys a 4-tier uni-directional hierarchy:
```
[Upstream Pipeline (transcript.rs / Session)]
                      │
                      ▼
[Harness Coordinator (`loop.rs`)]
   ├── Step 1 & 2: Intake & Budget
   ├── Step 3: Compaction
   └── Reentrant Cognitive Loop (Iterative passes up to MAX_TOOL_ITERATIONS)
                      │
                      ▼
[Sequential Step Functions (`steps.rs`)]
   ├── step1_intake
   ├── step3_execute_compaction & enter_non_terminal_phase
   ├── step4_assemble_request
   ├── step5_dispatch_llm
   ├── step6_run_stream_pass & tool handlers
   └── step7_finalize (commit / cancelled / error)
                      │
                      ▼
[Domain Stages (`stages/`)]
   ├── history, prompt, budget, compaction, streaming, tools
                      │
                      ▼
[Subsystems, Actors, Storage & IPC]
   ├── LlmActor, TtsActor, Turso SQLite, Frontend IpcEvent
```

1. **Top-to-Bottom Flow**: `loop.rs` contains the high-level turn coordinator and reentrant `while` loop without inlined helper clutter.
2. **Sequential Step Implementation**: `steps.rs` arranges concrete step helper functions in chronological order (Step 1 through Step 7) with step banners.
3. **Assembly Authority**: `step4_assemble_request` delegates exclusively to `PromptBuilderStage::build_generation_request`.
4. **NonTerminal Unity**: `enter_non_terminal_phase` in `steps.rs` is the single unified path for transitioning to `Working` and dispatching speech-normalized `AudioIntent::InterimFiller`. Both `spoken_response` and `spoken_filler` route through speech normalization before dispatching to TTS.
5. **Spoken Filler Delivery (Flagged)**: Currently `spoken_filler` is dispatched exclusively to audio synthesis (`AudioIntent::InterimFiller`) to eliminate dead air. Flagged for potential future UI message box rendering via an IPC token stream if visual display of interim filler is desired.


---

## 10. Future Capabilities & Evolution (Phase 12.2+)

The following harness-level capabilities are formally deferred. Their architecture must be considered when making Phase 12.1 decisions to avoid major refactors later.

### 10.1 Interactive Capability Discovery Probe
- **Current**: A synchronous, single-attempt probe with a strict 4.0-second timeout (`SESSION_BOOT_PROBE_TIMEOUT = 4s`). On timeout, falls back to `supports_tools = false`.
- **Future**: Replace with an interactive probe featuring retry and exponential backoff. The UI must surface probe progress and retry state to the user so the session boot does not appear frozen. On repeated failure, the user can choose to proceed without tool support or abort session initialization.

### 10.2 Adaptive Tool Recursion & Multi-Step Planning
- **Current**: Observation turns are bounded to `MAX_TOOL_ITERATIONS = 5` with a hard fallback error on breach.
- **Future**: Replace the hard iteration cap with a token-budget-aware stopping heuristic. The recursion guard evaluates remaining context headroom after each observation pass and terminates gracefully before overflow rather than crashing with a fallback error. Supports graph-based multi-step tool planning.

### 10.3 Multi-Turn Session Rollback Engine
- **Architecture must support this without major refactors**: The separated ledgers (`turns`, `session_tool_calls`, `session_compactions`) enable atomic rollback — pruning spoken turns, pruning tool traces, invalidating post-rollback compaction snapshots, and reconstructing context from the latest valid compaction.
- **Deferred**: The user-facing command, state machine, and context reconstruction execution logic is not implemented in Phase 12.1.

### 10.4 Compensating Side-Effect Rollback Actions
- **Deferred**: Inverse operations to undo external mutations (e.g., reverting a title write) when a turn is cancelled or rewound. Requires a compensating action registry on `ToolDefinition`.

### 10.5 MCP Multi-Server Daemon Lifecycle
- **Deferred**: Spawning, managing, and hot-reloading external sub-process MCP servers over standard I/O. The `ToolRegistry` interface is designed to accommodate external tool providers without harness changes.
