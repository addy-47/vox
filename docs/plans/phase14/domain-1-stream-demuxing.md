# Domain 1 Deep Dive: 3-Stream Demuxing & Interim Speech Architecture (v2)

This document provides the authoritative engineering analysis, line-grounded code trace, blast radius ledger, and specification contracts for **Domain 1** of the Harness v2 migration.

---

## 1. Executive Summary & Why This Domain Exists

In Vox, conversational turn latency must remain under 800ms to feel real-time. In v1, tool handling severely compromised this goal:

1. **Dropped Preamble Speech:** When an LLM emitted introductory words before invoking a tool (e.g., *"Let me search that for you"* followed by `web_search`), the v1 stream router cleared the chunker and deleted those spoken characters from memory.
2. **Schema-Bloated Spoken Fillers:** To work around dropped text, v1 forced every tool JSON schema to include a `spoken_filler: String` parameter. This bloated token prompts, increased generation latency, and forced models to speak unnatural, static phrases.
3. **No Reasoning Token Stream:** Frontier models (e.g., DeepSeek R1, Qwen 2.5, Claude 3.7) emit thinking tokens (`<think>` or `reasoning_content`). In v1, there was no dedicated `Thinking` stream. Reasoning tokens risked leaking into the audio hot-path or breaking JSON parsers.
4. **Single Tool Call Limit:** The v1 router returned immediately upon receiving the first tool call, silently dropping any subsequent tool calls emitted in the same pass.

**Domain 1** replaces this model with a **3-Stream Demuxer** that natively separates thinking tokens, spoken content tokens, and parallel tool call batches in a single pass.

---

## 2. Line-Grounded Code Trace: Current v1 Runtime Flow

This section reverse-engineers the exact v1 execution flow from the live codebase using the trace protocol mindset. Every step records what occurs, the initiating actor, the triggering condition, exact file-and-line grounding, and module ownership.

### Trace Scope & Boundaries
- **Entry Point:** `step6_run_stream_pass` in `app/src-tauri/src/services/harness/steps.rs#L418`.
- **Exit Points:**
  - Success Path: `StreamPassOutcome::Completed` or `StreamPassOutcome::ToolCallReceived`.
  - Abort Path: `StreamPassOutcome::Cancelled` or `StreamPassOutcome::Error`.
- **Boundaries Crossed:** Harness Async Task $\leftrightarrow$ OS Blocking Thread $\leftrightarrow$ TTS Actor Thread $\leftrightarrow$ CPAL Audio Output Engine.

---

### Phase A: Current v1 Token Streaming and Preamble Dropping Flow

1. **What happens:** The async turn harness spawns a blocking OS task to consume the LLM provider's response stream.
   **Who does it:** `steps.rs` initiates by calling `tokio::task::spawn_blocking` and delegating to `StreamRoutingStage::route_stream`.
   **What triggers it:** The turn loop reaches Step 6 after dispatching the generation request to the LLM duplex channel.
   **Grounding:** Traced — [steps.rs:418-428](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/steps.rs#L418-L428).
   **Owner:** `services::harness::steps`

2. **What happens:** The router enters an event loop polling the channel receiver (`response_rx`) with a 100ms timeout up to a 30s idle ceiling.
   **Who does it:** `router.rs` inside `consume_stream_events`.
   **What triggers it:** Receipt of `response_rx.recv_timeout(timeout_interval)`.
   **Grounding:** Traced — [router.rs:107-145](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs#L107-L145).
   **Owner:** `services::harness::stages::streaming::router`

3. **What happens:** When the LLM emits a text token (e.g. *"Let me look"*), the router emits an IPC event to the frontend and pushes the token into the sentence accumulator chunker. If a punctuation boundary is met, clauses are extracted and dispatched to TTS with intent hardcoded to `AudioIntent::TurnResponse`.
   **Who does it:** `router.rs` (`handle_token` and `dispatch_clauses`).
   **What triggers it:** Match on `LlmResponse::Token(token)`.
   **Grounding:** Traced — [router.rs:148-151](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs#L148-L151) and [router.rs:214-220](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs#L214-L220).
   **Owner:** `services::harness::stages::streaming::router`

4. **What happens:** For each extracted clause, text is normalized, `pending_synthesis_jobs` is incremented atomically, and `TtsCommand::Generate` is sent to the TTS actor channel with `AudioIntent::TurnResponse`.
   **Who does it:** `router.rs` (`dispatch_clauses`).
   **What triggers it:** Non-empty `clauses` returned by `push_token`.
   **Grounding:** Traced — [router.rs:235-280](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs#L235-L280).
   **Owner:** `services::harness::stages::streaming::router`

5. **What happens:** The LLM finishes emitting preamble words and emits a tool invocation (e.g. `web_search`). The router receives `LlmResponse::ToolCall(call)`. It locks the turn accumulator, **wipes the clause chunker completely** (`acc.chunker.clear()`), extracts accumulated string length, logs dropped prefix characters, and **immediately returns** `StreamPassOutcome::ToolCallReceived { partial_text, call }`.
   **Who does it:** `router.rs` inside `consume_stream_events`.
   **What triggers it:** Match on `LlmResponse::ToolCall(call)`.
   **Grounding:** Traced — [router.rs:152-167](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs#L152-L167).
   **Owner:** `services::harness::stages::streaming::router`

6. **What happens:** Any subsequent tool calls emitted in the same generation pass are abandoned because `consume_stream_events` returned on the first tool call; the channel receiver `response_rx` is dropped, terminating stream reading.
   **Who does it:** `router.rs` (`route_stream` exits).
   **What triggers it:** Early return on line 166.
   **Grounding:** Traced — [router.rs:90-99](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs#L90-L99) and [router.rs:166](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs#L166).
   **Owner:** `services::harness::stages::streaming::router`

7. **What happens:** The async coordinator in `steps.rs` receives `ToolCallReceived`. It locks the accumulator and **wipes `assistant_response` completely** (`acc.assistant_response.clear()`), discarding any remaining record of the preamble.
   **Who does it:** `steps.rs` inside `step6_handle_non_terminal_tool`.
   **What triggers it:** Execution of non-terminal tool branch in `loop.rs:335`.
   **Grounding:** Traced — [steps.rs:540-545](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/steps.rs#L540-L545).
   **Owner:** `services::harness::steps`

8. **What happens:** To replace the discarded spoken text, `steps.rs` inspects the tool call arguments for a `"spoken_filler"` string. If present, it passes it to `enter_non_terminal_phase`; if absent, it calls `select_filler_phrase` to pick a synthetic phrase (e.g. *"Checking that now..."*). It transitions the pipeline to `Working` and dispatches this synthetic phrase to TTS as `AudioIntent::InterimFiller`.
   **Who does it:** `steps.rs` (`enter_non_terminal_phase`).
   **What triggers it:** Tool arguments inspection in `step6_handle_non_terminal_tool`.
   **Grounding:** Traced — [steps.rs:546-577](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/steps.rs#L546-L577) and [steps.rs:173-213](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/steps.rs#L173-L213).
   **Owner:** `services::harness::steps`

---

### Phase B: Current v1 Playback State Transitions and AudioIntent Handling

9. **What happens:** The TTS actor synthesizes audio for the preamble clause dispatched in Step 4 and pushes 24kHz PCM samples to `AudioPlaybackEngine`. When samples pass the pre-roll cushion threshold, `AudioPlaybackEngine` emits `VoxEvent::PlaybackStarted` carrying the intent of that chunk (`AudioIntent::TurnResponse`).
   **Who does it:** `AudioPlaybackEngine` inside `services/audio/playback.rs`.
   **What triggers it:** Ring buffer fill level satisfies pre-roll cushion.
   **Grounding:** Traced — [services/audio/playback.rs:206-222](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/playback.rs#L206-L222).
   **Owner:** `services::audio::playback`

10. **What happens:** The centralized pipeline router receives `VoxEvent::PlaybackStarted` and routes it to `on_playback_started`.
    **Who does it:** `pipeline/assistant/mod.rs` routing to `playback.rs`.
    **What triggers it:** Arrival of `VoxEvent::PlaybackStarted` on event channel.
    **Grounding:** Traced — [pipeline/assistant/mod.rs:228-230](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/mod.rs#L228-L230).
    **Owner:** `pipeline::assistant`

11. **What happens:** Because the preamble clause was tagged with `AudioIntent::TurnResponse`, `on_playback_started` bypasses the `InterimFiller` check. It confirms the current state is `Thinking` (or `Working`) and **immediately transitions the entire system to `InteractionState::Speaking`**.
    **Who does it:** `playback.rs` (`on_playback_started`).
    **What triggers it:** `intent == AudioIntent::TurnResponse`.
    **Grounding:** Traced — [pipeline/assistant/playback.rs:21-41](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/playback.rs#L21-L41).
    **Owner:** `pipeline::assistant::playback`

12. **What happens:** Meanwhile, tool execution begins in an async task. While the tool is running, the CPAL sound card callback drains the preamble audio from the output ring buffer. Once drained and pending jobs reach zero, `sink.rs` emits `VoxEvent::PlaybackFinished` carrying `AudioIntent::TurnResponse`.
    **Who does it:** `AudioSink` inside `services/audio/sink.rs`.
    **What triggers it:** Ring buffer underrun with `turn_armed == true` and `pending_jobs == 0`.
    **Grounding:** Traced — [services/audio/sink.rs:219-239](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/sink.rs#L219-L239).
    **Owner:** `services::audio::sink`

13. **What happens:** The pipeline router receives `VoxEvent::PlaybackFinished` and calls `on_playback_finished`. Because intent is `TurnResponse`, it checks whether `turn_open` is false and `pending_synthesis_jobs == 0`. If true, it **immediately transitions the pipeline to `InteractionState::Ready`**, even though tool execution is still running in the background!
    **Who does it:** `playback.rs` (`on_playback_finished`).
    **What triggers it:** `intent == AudioIntent::TurnResponse` with no remaining synthesis jobs.
    **Grounding:** Traced — [pipeline/assistant/playback.rs:58-124](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/playback.rs#L58-L124).
    **Owner:** `pipeline::assistant::playback`

14. **What happens:** When the LLM actor finishes a pass, `on_llm_finished` clears `turn_open` and calls `evaluate_synthesis_latch`. If `pending_synthesis_jobs == 0` and `current_state != InteractionState::Speaking`, it forces an immediate transition to `InteractionState::Ready`, cutting off any remaining coordination.
    **Who does it:** `llm.rs` (`evaluate_synthesis_latch`).
    **What triggers it:** Receipt of `VoxEvent::LlmFinished`.
    **Grounding:** Traced — [pipeline/assistant/llm.rs:69-134](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/llm.rs#L69-L134).
    **Owner:** `pipeline::assistant::llm`

---

### Structural & Flow-Level Findings
1. **Preamble Destruction:** Lines `router.rs:156` (`acc.chunker.clear()`) and `steps.rs:544` (`acc.assistant_response.clear()`) deliberately dropped the LLM's natural speech to patch around state-machine leakage.
2. **State Machine Inversion:** Audio playback callbacks in `playback.rs` were driving cognitive pipeline transitions (`Thinking` $\to$ `Speaking` $\to$ `Ready`), usurping control from the cognitive coordinator (`loop.rs`).
3. **Multi-Tool Truncation:** Line `router.rs:166` returned on the first `ToolCall`, making multi-tool passes physically impossible.
4. **Missing Reasoning Separation:** `LlmStreamEvent` ([provider.rs:209](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/provider.rs#L209)) and `LlmResponse` ([actor.rs:29](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/actor.rs#L29)) had no `Thinking` variant, risking prompt corruption and TTS speech leakage.

---

## 3. Proposed v2 Architecture: 3-Stream Demuxer & Audio Intent Resolution

Domain 1 eliminates synthetic fillers, preserves natural preamble speech, and establishes the strict invariant: **The Harness coordinator (`loop.rs`) owns state transitions; audio playback does not.**

### 3.1 Resolving Stream Audio Intent: The Two-Key Quiescence Latch
During streaming, tokens arrive *before* tool calls. The router cannot know in advance whether a tool call will follow. Furthermore, 99% of spoken sentences end with terminal punctuation (`.`, `?`, `!`), leaving the sentence chunker empty upon pass completion.

Therefore, the system MUST NOT rely on an audio tag on a final clause tail to guess turn quiescence. State transitions are governed by a **Two-Key Quiescence Latch**:

```
Key 1: Cognitive Turn Closed? (Harness: turn_open == false) ─────┐
                                                                 ├────► Both True? ───► Transition to Ready
Key 2: Sound Card Audio Drained? (AudioSink: PlaybackFinished) ──┘
```

**The Authoritative Rules:**
1. **Tool-Enabled Steps (`allow_tools == true`):**
   - Streamed clauses from `ClauseChunker` are dispatched to TTS with **`AudioIntent::InterimFiller`**.
   - `playback.rs` (`on_playback_started`) does NOT transition to `Speaking`. The UI stays stably in `Thinking`.
   - The user hears the spoken preamble immediately with sub-800ms time-to-first-audio (TTFA).
2. **Intermediate Step Latch Protection (`turn_open == true`):**
   - While tools are executing, `turn_open` remains `true` across all intermediate steps (`0..N`) of the ReAct turn.
   - When preamble audio finishes playing on the speakers, `sink.rs` emits `PlaybackFinished`.
   - `on_playback_finished` checks `if state.pipeline.is_turn_open() { return; }`. Because `turn_open == true`, **it CANNOT transition to `Ready` while tools are still running**. The pipeline stays in `Working`.
3. **Tool Invocations Received (`!tool_calls.is_empty()`):**
   - Returns `StreamPassOutcome::ToolCallsReceived { preamble_text, thinking_text, calls }`.
   - Any remaining clause tail is flushed as `AudioIntent::InterimFiller`.
   - The coordinator transitions the pipeline from `Thinking` $\to$ `InteractionState::Working` with the active `ActivityEnvelope`.
   - Tools execute concurrently in background tasks while preamble audio continues playing on the speakers.
4. **Final Response Step (0 Tool Calls — `StreamPassOutcome::Completed`):**
   - During streaming (when `allow_tools == true`), the router does not yet know that zero tool calls will be emitted, so punctuated clauses emitted mid-stream were already dispatched as `AudioIntent::InterimFiller` (or `AudioIntent::TurnResponse` if `allow_tools == false`).
   - Once `LlmResponse::Finished` arrives with `tool_calls.is_empty()`, any remaining unpunctuated tail in `ClauseChunker` is flushed as `AudioIntent::TurnResponse`.
   - `loop.rs` immediately emits `VoxEvent::LlmFinished { turn_id }` to close the cognitive turn.
   - In `llm.rs` (`on_llm_finished`), the router sets **`turn_open = false`** (**Key 1 is turned**) and evaluates the synthesis/playback state:
     - If TTS synthesis jobs are still pending (`pending_synthesis_jobs > 0`) or the audio sink is actively playing, `on_llm_finished` transitions the pipeline to **`InteractionState::Speaking`** (if not already `Speaking`). Furthermore, in `playback.rs` (`on_playback_started`), any clause playback starting while `turn_open == false` (or carrying `AudioIntent::TurnResponse`) transitions `Thinking` -> `InteractionState::Speaking`.
     - If `pending_synthesis_jobs == 0` and the audio sink has already drained, `on_llm_finished` transitions directly to **`InteractionState::Ready`**.
   - In `playback.rs` (`on_playback_finished`), when the final audio sample drains from the sound card buffer (**Key 2 is turned**), `on_playback_finished` checks `!state.pipeline.is_turn_open() && pending_synthesis_jobs == 0` **regardless of the chunk's `AudioIntent` tag**. Because `turn_open == false`, it deterministically transitions **`Speaking` -> `InteractionState::Ready`** even when all sentences in the final pass ended with terminal punctuation and were streamed as `InterimFiller`.

---

### 3.2 Non-Blocking Parallel TTS Execution Timeline

Tool execution **never** blocks for audio playback, and audio playback **never** blocks tool execution:

```
Time (ms)  LLM Provider Channel            TTS Actor (OS Thread)              Harness Coordinator (Tokio)
──────────────────────────────────────────────────────────────────────────────────────────────────────────
0ms        LlmResponse::Thinking("Analyze")  -                                  Pipeline in Thinking
80ms       LlmResponse::Token("Let")         -                                  Accumulating clause...
140ms      LlmResponse::Token(" me check.")  Chunked -> "Let me check." (Filler) -
145ms      -                                 Synthesizing audio clause 0...     -
180ms      LlmResponse::ToolCall("read_dir") Collected in batch (call 0)        -
210ms      LlmResponse::ToolCall("read_log") Collected in batch (call 1)        -
250ms      LlmResponse::Finished             PCM pushed to ring buffer          Pass complete: 2 calls
252ms      -                                 🔊 "Let me check." starts playing  Transitions to Working
255ms      -                                 🔊 Speaker: "...check."            Spawns parallel tool tasks!
280ms      -                                 🔊 Speaker: "...the logs."         read_dir & read_log executing
350ms      -                                 🔊 Speaker finishes (Filler)       read_log completes (95ms)
400ms      -                                 Silent (stays in Working)          read_dir completes (145ms)
410ms      -                                 -                                  Step 0 commits to history;
                                                                                Step 1 generation dispatches!
```

---

### 3.3 Thinking Token Isolation Contract

Reasoning models emit chain-of-thought tokens that must never leak into spoken audio or user-facing chat responses:
1. **TTS Hot-Path Quarantine:** `LlmResponse::Thinking(thought)` tokens are appended to `thinking_buffer`. They are never passed to `ClauseChunker`, never sent to `tts_tx`, and never trigger audio synthesis.
2. **Backend Memory Containment (Zero Unthrottled IPC):** Thinking tokens are captured in turn-local memory (`thinking_buffer`) and internal logs. They do NOT emit raw, unthrottled per-token IPC events across Tauri IPC, strictly complying with `events-spec.md RULE-EVT-02: High-Frequency Stream Isolation` to protect the 8GB hardware ceiling from WebKitGTK IPC churn.
3. **Database Quarantine:** Thinking text is excluded from `turns.assistant_text` and `session_tool_calls.arguments`.
4. **Context Budget Protection:** Thinking tokens are stored in memory only during the active turn for logging and telemetry. They are never injected into `ChatMessage::content` for subsequent turns, preserving context budget.
5. **Provider Adapter Ingress Demuxing:** Concrete provider stream adapters (`chat_completions.rs`, `ollama.rs`, `sidecar.rs`) parse structured `reasoning_content` deltas or state-machine strip inline `<think>...</think>` tags at the network boundary, ensuring `router.rs` only receives clean, normalized tokens.

---

## 4. Blast Radius & Zero Backward Compatibility (ZBC) Ledger

Vox enforces **Zero Backward Compatibility (ZBC)**. No compatibility shims, dummy enums, or deprecated branches are permitted.

### 4.1 Structural Diff Matrix

| File Path | Nature | Structural Modifications |
| :--- | :--- | :--- |
| `app/src-tauri/src/services/llm/provider.rs` | Contract | Add `LlmStreamEvent::Thinking(String)`. Remove `ToolFlow` enum completely. Remove `flow: ToolFlow` from `CanonicalToolDefinition`. |
| `app/src-tauri/src/services/llm/actor.rs` | Actor | Add `LlmResponse::Thinking(String)`. Remove `ToolFlow` matching from generation loop. |
| `app/src-tauri/src/services/llm/mod.rs` | Export | Delete `pub use provider::ToolFlow;`. |
| `app/src-tauri/src/services/llm/transport/chat_completions.rs` | Provider Adapter | Parse `delta.reasoning_content` / `delta.thinking` and emit `LlmStreamEvent::Thinking(String)`. |
| `app/src-tauri/src/services/llm/transport/ollama.rs` | Provider Adapter | Parse `thinking` field in streaming JSON and emit `LlmStreamEvent::Thinking(String)`. |
| `app/src-tauri/src/services/llm/sidecar.rs` | Provider Adapter | Add state-machine tag filter stripping `<think>...</think>` tags and routing them to `LlmStreamEvent::Thinking(String)`. |
| `app/src-tauri/src/services/harness/stages/streaming/router.rs` | Replacement | Replace with 3-Stream Demuxer: isolate `Thinking`, collect multi-tool `Vec<CanonicalToolCall>`, preserve preamble, tag clauses with `InterimFiller`. |
| `app/src-tauri/src/services/harness/stages/tools/mod.rs` | Trait | Remove `fn flow(&self) -> ToolFlow` from `ToolDefinition` trait and `to_canonical`. |
| `app/src-tauri/src/services/harness/stages/tools/search_memory.rs` | Cleanup | Delete `spoken_filler` from parameters schema; remove `fn flow(&self)`. |
| `app/src-tauri/src/services/harness/stages/tools/web_search.rs` | Cleanup | Delete `spoken_filler` from parameters schema; remove `fn flow(&self)`. |
| `app/src-tauri/src/services/harness/stages/tools/respond_and_set_title.rs` | Removal (ZBC) | Delete legacy `respond_and_set_title.rs` and its `spoken_response` parameter. |
| `app/src-tauri/src/services/harness/stages/tools/set_title.rs` | Addition (ZBC) | Add clean universal `SetTitleTool` taking only `{"title": "..."}` without flow or speech parameters. |
| `app/src-tauri/src/services/harness/stages/tools/executor.rs` | Tool Executor | Remove `flow: ToolFlow` parameter from `dispatch_persistence`. |
| `app/src-tauri/src/persistence/mod.rs` | Persistence | Remove `tool_flow: ToolFlow` from `PersistenceEvent::ToolCallExecuted`. |
| `app/src-tauri/src/persistence/tool_calls.rs` | Persistence | Remove `tool_flow` matching and remove `tool_kind` column insert. |
| `app/src-tauri/src/persistence/schema.rs` | Database (ZBC) | Remove `tool_kind TEXT NOT NULL` from `session_tool_calls` table schema; author clean Turso table migration. |
| `app/src-tauri/src/services/harness/loop.rs` | Coordinator | Replace single-tool branching with multi-tool batch execution (`step6_execute_tool_batch`); manage `turn_open` across full ReAct turn. |
| `app/src-tauri/src/services/harness/steps.rs` | Steps | Remove `step6_handle_terminal_tool` and `step6_handle_non_terminal_tool`. Implement `step6_execute_tool_batch`. |
| `app/src-tauri/src/pipeline/assistant/playback.rs` | Playback Engine | Update `on_playback_finished` to check `is_turn_open()`. Guarantee clean transition to `Ready` when `turn_open == false` and audio drains. |
| `app/src-tauri/src/pipeline/assistant/llm.rs` | Latch Evaluation | Update `on_llm_finished` and `evaluate_synthesis_latch` to coordinate with `turn_open` and audio draining. |
| `app/src-tauri/src/utils/paths.rs` | Paths | Add `tool_outputs_dir()` and `tasks_dir()` disposable cache roots. |

---

### 4.2 Exact Type & Signature Modifications

#### 1. `LlmStreamEvent` & `LlmResponse`
```rust
// In services/llm/provider.rs
pub enum LlmStreamEvent {
    Thinking(String),            // NEW: Isolated chain-of-thought stream
    Token(String),               // Spoken conversational content
    ToolCall(CanonicalToolCall), // Structured tool invocation
    Finished,                    // Stream generation complete
}

// In services/llm/actor.rs
pub enum LlmResponse {
    Thinking(String),            // NEW
    Token(String),
    ToolCall(super::CanonicalToolCall),
    Finished,
    Cancelled,
    Error(PipelineError),
}
```

#### 2. Clean Removal of `ToolFlow`
```rust
// In services/llm/provider.rs
// REMOVED: pub enum ToolFlow { Terminal, NonTerminal }

// In services/llm/provider.rs
pub struct CanonicalToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    // REMOVED: pub flow: ToolFlow,
}

// In services/harness/stages/tools/mod.rs
pub trait ToolDefinition: Send + Sync {
    fn name(&self) -> &str;
    fn domain(&self) -> ToolDomain { ToolDomain::All }
    fn description(&self, mode: PipelineMode) -> &str;
    fn parameters_schema(&self, mode: PipelineMode) -> Value;
    // REMOVED: fn flow(&self) -> ToolFlow;
    fn execute<'a>(...);
}
```

#### 3. Updated `StreamPassOutcome`
```rust
// In services/harness/stages/streaming/router.rs
#[derive(Debug, Clone)]
pub enum StreamPassOutcome {
    Completed {
        assistant_text: String,
        thinking_text: String,
    },
    ToolCallsReceived {
        preamble_text: String,
        thinking_text: String,
        calls: Vec<CanonicalToolCall>, // Full batch collected
    },
    Cancelled {
        partial_text: String,
    },
    Error(String),
}
```

#### 4. Turso Database Migration (ZBC)
```sql
-- Migration (v16): Drop legacy tool_kind column directly (supported in Turso/libsql SQLite 3.35+)
ALTER TABLE session_tool_calls DROP COLUMN tool_kind;
```

---

## 5. Authoritative Specification Contracts (Locked in `docs/specs/harness-spec.md`)

These contracts are now formally locked and integrated into [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md) (§4.4 and §4.5) and [tools-specs/lld.md](file:///home/addy/projects/apps/vox/docs/specs/tools-specs/lld.md), completely superseding all legacy v1 logic:

### RULE-HARN-09 (3-Stream Demuxing & Thinking Isolation)
1. The streaming router MUST demux incoming LLM responses into three distinct channels in a single pass:
   - **Thinking Channel (`LlmStreamEvent::Thinking`)**: Captured into an isolated turn memory buffer; never sent to TTS chunkers; never emitted as unthrottled per-token IPC events to the UI (strictly complying with `events-spec.md RULE-EVT-02`); never persisted to `turns.assistant_text`.
   - **Content Token Channel (`LlmStreamEvent::Token`)**: Eagerly chunked into speakable clauses and emitted to `IpcEvent::LlmToken`.
   - **Tool Call Channel (`LlmStreamEvent::ToolCall`)**: Buffered until `LlmResponse::Finished` to assemble complete multi-tool batches.
2. Concrete provider stream adapters (`chat_completions.rs`, `ollama.rs`, `sidecar.rs`) MUST parse structured reasoning deltas or state-machine strip inline `<think>...</think>` tags at the network ingress boundary so raw thinking tags never enter the content token stream.
3. The router MUST NOT terminate stream consumption on the first tool call. It MUST continue reading until `Finished`, `Cancelled`, or `Error`.

### RULE-HARN-10A (Preamble Speech Preservation)
1. If the model emits spoken words prior to tool calls in a generation pass (e.g. *"Let me search the files for that"*), those words MUST NOT be dropped, chunker-cleared, or overwritten.
2. The complete preamble string MUST be preserved in `StreamPassOutcome::ToolCallsReceived` and recorded in conversation history as the assistant message initiating the tool calls.

### RULE-HARN-10B (Stream Audio Intent & Two-Key Quiescence Latch)
1. In any step where tools are enabled (`allow_tools == true`), mid-stream clauses extracted before `LlmResponse::Finished` MUST be dispatched to TTS with `AudioIntent::InterimFiller` (since the router cannot know in advance whether tool calls will follow). If `allow_tools == false`, all clauses are dispatched as `AudioIntent::TurnResponse`.
2. While `turn_open == true` (the cognitive ReAct loop is still active), audio playback onset (`on_playback_started`) for `AudioIntent::InterimFiller` MUST NOT transition the pipeline to `Speaking`, and audio playback completion (`on_playback_finished`) MUST return early (`if state.pipeline.is_turn_open() { return; }`) so the pipeline remains stably in `Thinking` or `Working`.
3. Preamble audio playback on the sound card and background tool execution MUST execute concurrently. The coordinator MUST NOT await audio draining before launching tool futures.
4. When a generation pass finishes with zero tool calls (`StreamPassOutcome::Completed`), the cognitive turn is complete: any unpunctuated tail is flushed as `AudioIntent::TurnResponse`, and `loop.rs` emits `VoxEvent::LlmFinished { turn_id }`.
5. Upon receiving `VoxEvent::LlmFinished`, `on_llm_finished` sets `turn_open = false` (**Key 1**). If audio synthesis or playback is still in progress (`pending_synthesis_jobs > 0` or sink active), `on_llm_finished` (or `on_playback_started` when `turn_open == false`) transitions the pipeline to `InteractionState::Speaking`.
6. Transition to `InteractionState::Ready` is governed strictly by the **Two-Key Quiescence Latch** (`turn_open == false` AND `pending_synthesis_jobs == 0` with sound card buffer drained), evaluated in both `on_llm_finished` and `on_playback_finished` **regardless of whether the draining audio chunks were originally dispatched with `InterimFiller` or `TurnResponse`**.

### RULE-HARN-11 (ZBC Universal Tool Interface & Zero Filler Schemas)
1. Cognitive tool definitions MUST NOT declare execution flow categories (`Terminal` vs `NonTerminal`).
2. Cognitive tool JSON schemas MUST NOT include synthetic speech parameters (`spoken_filler` or `spoken_response`).
3. State transitions to `InteractionState::Working` MUST be executed solely by the Harness coordinator upon inspecting tool batch presence, never by downstream tool wrappers.
4. The coordinator supports iterative ReAct execution up to `MAX_TOOL_STEPS = 20`. If step 20 is reached without a terminal answer, the Harness forces a final generation pass with tools disabled (`allow_tools = false`) to produce a complete spoken summary.

---

## 6. Edge-Case, Asymmetry & Failure Matrix (Pivot Gates)

| Scenario / Trigger | Failure Mode / Physics | Deterministic v2 Resolution |
| :--- | :--- | :--- |
| **Silent Wi-Fi Blackhole** | Provider socket hangs mid-stream after emitting preamble tokens. | `recv_timeout(100ms)` accumulates idle time. At 30s (`max_idle`), returns `Err("LLM provider response timed out")`. `loop.rs` catches error, transitions to `Ready`, and purges turn tokens. |
| **Abrupt Socket RST** | Remote provider drops connection during tool invocation. | `recv_timeout` returns `RecvTimeoutError::Disconnected`. Router logs error and returns `Err("LLM stream disconnected prematurely")`. No hang. |
| **Zero-Preamble Model** | Local GGUF/C++ model immediately emits tool calls with zero spoken words. | `accumulator` remains empty. Router flushes nothing. `step6_execute_tool_batch` transitions immediately to `Working`. System remains silent with zero dead-air filler. |
| **User Barge-In During Preamble** | User speaks while preamble *"Let me search..."* is playing. | Microphone detects VAD speech. Event router emits `VoxEvent::SpeechStart`. `cancel` atomic is set. TTS and playback engine mute immediately. Router exits with `StreamPassOutcome::Cancelled`. Active tool tasks are aborted via cancellation tokens. |
| **Reasoning Model Context Overflow** | Model emits 4,000 thinking tokens before preamble. | `thinking_buffer` accumulates in RAM. Context budget evaluator tracks prompt + completion tokens against ceiling. If ceiling exceeded, provider returns `ContextLimitExceeded` error; router exits cleanly to `Error`. |
| **Multi-Tool Mixed Outcome** | Batch contains 3 tools; Tool 1 succeeds, Tool 2 fails with timeout, Tool 3 succeeds. | Barrier scheduler captures each outcome independently. All 3 tool results are committed into history as separate `tool` role messages. The LLM receives full observation context in the next step. |
