# Domain 1 Deep Dive: 3-Stream Demuxing & Interim Speech Architecture (v2)

This document provides the authoritative engineering analysis, code trace, blast radius ledger, and specification contracts for **Domain 1**.
It is fully aligned with the Phase 14 pipeline refactor.

---

## 1. Executive Summary & Why This Domain Exists

In Vox, conversational turn latency must remain under 800ms to feel real-time. In v1, tool handling compromised this goal:

1. **Dropped Preamble Speech:** The v1 router cleared the chunker when an LLM emitted words before a tool call. It deleted spoken words from memory.
2. **Schema-Bloated Spoken Fillers:** Tools carried a `spoken_filler: String` parameter. This bloated prompts, increased latency, and forced unnatural phrases.
3. **No Reasoning Token Stream:** Thinking tokens (`<think>` or `reasoning_content`) leaked into the audio hot-path or broke parsers.
4. **Single Tool Call Limit:** The v1 router returned on the first tool call. It silently dropped subsequent tool calls in the same pass.

**Domain 1** replaces this model with a **3-Stream Demuxer**. It separates thinking tokens, content tokens, and parallel tool calls in one pass.

---

## 2. Line-Grounded Code Trace: Historical v1 Runtime Flow

This section documents the historical v1 execution flow. Every step records the actor, trigger, file grounding, and ownership.

### Trace Scope & Boundaries
- **Entry Point:** `step6_run_stream_pass` in `app/src-tauri/src/harness/steps.rs`.
- **Exit Points:**
  - Success Path: `StreamPassOutcome::Completed` or `StreamPassOutcome::ToolCallsReceived`.
  - Abort Path: `StreamPassOutcome::Cancelled`, `StreamPassOutcome::PausedForSpeech`, or `StreamPassOutcome::Error`.
- **Boundaries Crossed:** Harness Async Task $\leftrightarrow$ OS Blocking Thread $\leftrightarrow$ TTS Actor Thread $\leftrightarrow$ CPAL Audio Output Engine.

---

### Phase A: Historical v1 Token Streaming and Preamble Dropping Flow

1. **What happens:** The async turn harness spawns a blocking OS task to consume the LLM response stream.
   **Who does it:** `steps.rs` initiates by calling `tokio::task::spawn_blocking` and delegating to `StreamRoutingStage::route_stream`.
   **What triggers it:** The turn loop reaches Step 6 after dispatching the generation request.
   **Grounding:** Historical — `harness/steps.rs`.
   **Owner:** `harness::steps`

2. **What happens:** The router enters an event loop polling the channel receiver (`response_rx`) with a 100ms timeout up to 30s.
   **Who does it:** `router.rs` inside `consume_stream_events`.
   **What triggers it:** Receipt of `response_rx.recv_timeout(timeout_interval)`.
   **Grounding:** Historical — `harness/stages/streaming/router.rs`.
   **Owner:** `harness::stages::streaming::router`

3. **What happens:** When the LLM emits a token, the router emits an IPC event and pushes the token into the chunker. If punctuated, clauses are dispatched to TTS.
   **Who does it:** `router.rs` (`handle_token` and `dispatch_clauses`).
   **What triggers it:** Match on `LlmResponse::Token(token)`.
   **Grounding:** Historical — `harness/stages/streaming/router.rs`.
   **Owner:** `harness::stages::streaming::router`

4. **What happens:** For each clause, text is normalized, `pending_tts_jobs` is incremented, and `TtsCommand::Generate` is sent to TTS.
   **Who does it:** `router.rs` (`dispatch_clauses`).
   **What triggers it:** Non-empty clauses returned by `push_token`.
   **Grounding:** Historical — `harness/stages/streaming/router.rs`.
   **Owner:** `harness::stages::streaming::router`

5. **What happens:** The LLM emits a tool call. The historical router wiped the chunker (`acc.chunker.clear()`) and returned immediately.
   **Who does it:** Historical `router.rs` inside `consume_stream_events`.
   **What triggers it:** Match on `LlmResponse::ToolCall(call)`.
   **Grounding:** Historical bug — `router.rs:152-167`.
   **Owner:** `harness::stages::streaming::router`

6. **What happens:** Subsequent tool calls in the same pass were dropped because the router exited early.
   **Who does it:** Historical `router.rs`.
   **What triggers it:** Early return on first tool call.
   **Grounding:** Historical bug — `router.rs:166`.
   **Owner:** `harness::stages::streaming::router`

7. **What happens:** The historical async coordinator wiped `assistant_response` completely, discarding the preamble.
   **Who does it:** Historical `steps.rs`.
   **What triggers it:** Execution of non-terminal tool branch.
   **Grounding:** Historical bug — `steps.rs:540-545`.
   **Owner:** `harness::steps`

8. **What happens:** To replace lost speech, `steps.rs` read `"spoken_filler"` from arguments or picked a synthetic phrase.
   **Who does it:** Historical `steps.rs`.
   **What triggers it:** Tool arguments inspection.
   **Grounding:** Historical bug — `steps.rs:546-577`.
   **Owner:** `harness::steps`

---

### Phase B: Historical v1 Playback State Transitions

9. **What happens:** TTS synthesized audio and pushed PCM samples to playback. When pre-roll filled, playback emitted `PlaybackStarted`.
   **Who does it:** `AudioPlaybackEngine` inside `services/audio/playback.rs`.
   **What triggers it:** Ring buffer fill level satisfies pre-roll cushion.
   **Grounding:** `services/audio/playback.rs`.
   **Owner:** `services::audio::playback`

10. **What happens:** The router received `PlaybackStarted` and called `on_playback_started`.
    **Who does it:** `pipeline/assistant/mod.rs` routing to `playback.rs`.
    **What triggers it:** Arrival of `VoxEvent::PlaybackStarted`.
    **Grounding:** `pipeline/assistant/mod.rs`.
    **Owner:** `pipeline::assistant`

11. **What happens:** In v1, `PlaybackStarted` forced an immediate transition to `InteractionState::Speaking`.
    **Who does it:** Historical `playback.rs`.
    **What triggers it:** `PlaybackStarted` arrival.
    **Grounding:** Historical bug — `pipeline/assistant/playback.rs:21-41`.
    **Owner:** `pipeline::assistant::playback`

12. **What happens:** While tools ran, audio drained. Once drained, `sink.rs` emitted `PlaybackFinished`.
    **Who does it:** `AudioSink` inside `services/audio/sink.rs`.
    **What triggers it:** Ring buffer underrun with zero pending jobs.
    **Grounding:** `services/audio/sink.rs`.
    **Owner:** `services::audio::sink`

13. **What happens:** In v1, `PlaybackFinished` immediately transitioned the pipeline to `Ready`, abandoning running background tools.
    **Who does it:** Historical `playback.rs` (`on_playback_finished`).
    **What triggers it:** `PlaybackFinished` arrival with zero jobs.
    **Grounding:** Historical bug — `pipeline/assistant/playback.rs:58-124`.
    **Owner:** `pipeline::assistant::playback`

14. **What happens:** When the LLM actor finished, `on_llm_finished` forced a transition to `Ready` if audio was silent.
    **Who does it:** Historical `llm.rs`.
    **What triggers it:** Receipt of `VoxEvent::LlmFinished`.
    **Grounding:** Historical bug — `pipeline/assistant/llm.rs:69-134`.
    **Owner:** `pipeline::assistant::llm`

---

### Historical Defects Eradicated
1. **Preamble Destruction:** Chunker clearing and response wiping were deleted.
2. **State Machine Inversion:** Playback callbacks no longer control cognitive phase transitions.
3. **Multi-Tool Truncation:** Multi-tool batching accumulates all tool calls until `Finished`.
4. **Missing Reasoning Separation:** `Thinking` stream isolation keeps chain-of-thought out of TTS and IPC.
5. **AudioIntent Crutch:** `AudioIntent` enum is completely purged.

---

## 3. Production v2 Architecture: 3-Stream Demuxer & Duplex Quiescence Law

Domain 1 eliminates synthetic fillers. It preserves natural preamble speech. It establishes that the Harness coordinator and router thread own state transitions.

### 3.1 Stream Delivery & Duplex Quiescence Law (Purged AudioIntent)
Tokens arrive before tool calls during streaming. The router cannot know in advance if tool calls follow.
Most sentences end with punctuation, emptying the chunker before stream finish.

The system does NOT use intent tags. The **Duplex Quiescence Law** governs state transitions:

```
Key 1: LLM Complete? (Router: is_llm_done() == true) ─────────────┐
                                                                  ├────► Both True? ───► Transition to Ready
Key 2: Sound Card Drained? (pending_tts_jobs == 0 && !sink_occ) ──┘
```

**The Production Rules:**
1. **Continuous Clause Streaming:**
   - Streamed clauses from `ClauseChunker` are dispatched to TTS as plain speech (`TtsCommand::Generate`).
   - The user hears the spoken preamble immediately with sub-800ms latency.
2. **State Preservation on Playback Started (`on_playback_started`):**
   - When playback starts, `on_playback_started` transitions `Thinking` or `Working` to `Speaking`.
   - It strictly preserves `current_activity` (active tool context is retained).
   - Fast turns in `Ready` accept `PlaybackStarted` to record TTFA metrics.
3. **Audio Drain While Tools Execute (`on_playback_finished`):**
   - When audio drains while tools are active (`is_llm_done() == false` and `current_activity.is_some()`), `on_playback_finished` transitions to `Working`.
   - It restores the active tool envelope. The pipeline does NOT drop to `Ready`.
   - If `is_llm_done() == false` and `current_activity.is_none()`, it transitions to `Thinking`.
4. **Tool Calls Received (`!tool_calls.is_empty()`):**
   - The router flushes any unpunctuated tail to TTS.
   - It returns `StreamPassOutcome::ToolCallsReceived { preamble_text, thinking_text, calls }`.
   - The coordinator sets `current_activity` and transitions to `InteractionState::Working`.
   - Tools execute in parallel while audio continues playing.
5. **Final Generation Pass (Zero Tool Calls):**
   - When `LlmResponse::Finished` arrives with zero tool calls, the router flushes any clause tail to TTS.
   - It returns `StreamPassOutcome::Completed { assistant_text, thinking_text }`.
   - The router thread receives `VoxEvent::LlmFinished { turn_id }`.
   - `on_llm_finished` calls epoch-guarded `mark_llm_finished(turn_id)` to set `llm_done = true` (**Key 1**).
   - If audio is active (`pending_tts_jobs > 0` or `sink_occupied == true`), the router remains in current state until audio drains.
   - If audio is drained, it transitions immediately to `InteractionState::Ready`.
6. **Final Audio Drain (`on_playback_finished`):**
   - When the sound card drains and `is_llm_done() == true` (**Key 2**), `on_playback_finished` transitions to `InteractionState::Ready`.
7. **Zero-Audio Fallback:**
   - When a turn produces zero audio, the TTS worker flushes pre-roll.
   - If the sink is unarmed and empty, it emits fallback `PlaybackFinished`.
   - `on_playback_finished` unblocks the quiescence latch and transitions to `Ready`.

---

### 3.2 Non-Blocking Parallel TTS Execution Timeline

Tool execution never blocks for audio playback. Audio playback never blocks tool execution:

```
Time (ms)  LLM Provider Channel            TTS Actor (OS Thread)              Harness Coordinator (Tokio)
──────────────────────────────────────────────────────────────────────────────────────────────────────────
0ms        LlmResponse::Thinking("Analyze")  -                                  Pipeline in Thinking
80ms       LlmResponse::Token("Let")         -                                  Accumulating clause...
140ms      LlmResponse::Token(" me check.")  Chunked -> "Let me check."         -
145ms      -                                 Synthesizing audio clause 0...     -
180ms      LlmResponse::ToolCall("read_dir") Collected in batch (call 0)        -
210ms      LlmResponse::ToolCall("read_log") Collected in batch (call 1)        -
250ms      LlmResponse::Finished             PCM pushed to ring buffer          Pass complete: 2 calls
252ms      -                                 🔊 "Let me check." starts playing  Transitions to Speaking (preserves activity)
255ms      -                                 🔊 Speaker: "...check."            Spawns parallel tool tasks!
280ms      -                                 🔊 Speaker: "...the logs."         read_dir & read_log executing
350ms      -                                 🔊 Speaker finishes                Transitions to Working (tools running)
400ms      -                                 Silent                             read_dir & read_log finish
410ms      -                                 -                                  Step 0 commits to history;
                                                                                Step 1 generation dispatches!
```

---

### 3.3 Thinking Token Isolation Contract

Reasoning models emit chain-of-thought tokens. These tokens must never leak into spoken audio or user chats:
1. **TTS Hot-Path Quarantine:** `LlmResponse::Thinking(thought)` tokens append to `thinking_buffer`. They never pass to `ClauseChunker`. They never trigger TTS synthesis.
2. **Backend Memory Containment:** Thinking tokens stay in turn-local memory and logs. They do not emit unthrottled per-token IPC events (complying with `events-spec.md RULE-EVT-02`).
3. **Database Quarantine:** Thinking text is excluded from `turns.assistant_text` and `session_tool_calls.arguments`.
4. **Context Budget Protection:** Thinking tokens stay in memory only during the active turn. They are not injected into `ChatMessage::content` for future turns.
5. **Provider Adapter Ingress Demuxing:** Concrete adapters (`chat_completions.rs`, `ollama.rs`, `sidecar.rs`) parse `reasoning_content` deltas or strip inline `<think>...</think>` tags at the network boundary.

---

## 4. Blast Radius & Zero Backward Compatibility (ZBC) Ledger

Vox enforces Zero Backward Compatibility (ZBC). No compatibility shims or deprecated branches exist.

### 4.1 Structural Diff Matrix

| File Path | Nature | Structural Modifications |
| :--- | :--- | :--- |
| `app/src-tauri/src/services/llm/provider.rs` | Contract | Add `LlmStreamEvent::Thinking(String)`. Remove `ToolFlow` enum. Remove `flow` from `CanonicalToolDefinition`. |
| `app/src-tauri/src/services/llm/actor.rs` | Actor | Add `LlmResponse::Thinking(String)`. Remove `ToolFlow` matching. |
| `app/src-tauri/src/services/llm/mod.rs` | Export | Delete `pub use provider::ToolFlow;`. |
| `app/src-tauri/src/services/llm/transport/chat_completions.rs` | Provider Adapter | Parse `delta.reasoning_content` / `delta.thinking` and emit `LlmStreamEvent::Thinking(String)`. |
| `app/src-tauri/src/services/llm/transport/ollama.rs` | Provider Adapter | Parse `thinking` field in streaming JSON and emit `LlmStreamEvent::Thinking(String)`. |
| `app/src-tauri/src/services/llm/sidecar.rs` | Provider Adapter | Add state-machine tag filter stripping `<think>...</think>` tags and routing to `Thinking`. |
| `app/src-tauri/src/harness/stages/streaming/router.rs` | Replacement | 3-Stream Demuxer: isolate `Thinking`, collect multi-tool `Vec<CanonicalToolCall>`, preserve preamble, handle pause for speech. |
| `app/src-tauri/src/harness/tools/mod.rs` | Trait | Remove `fn flow(&self) -> ToolFlow` from `ToolDefinition` trait. |
| `app/src-tauri/src/harness/tools/search_memory.rs` | Cleanup | Delete `spoken_filler` from parameters schema; remove `fn flow(&self)`. |
| `app/src-tauri/src/harness/tools/web_search.rs` | Cleanup | Delete `spoken_filler` from parameters schema; remove `fn flow(&self)`. |
| `app/src-tauri/src/harness/tools/respond_and_set_title.rs` | Removal (ZBC) | Delete legacy `respond_and_set_title.rs` and its `spoken_response` parameter. |
| `app/src-tauri/src/harness/tools/set_title.rs` | Addition (ZBC) | Add universal `SetTitleTool` taking `{"title": "..."}` without flow or speech parameters. |
| `app/src-tauri/src/harness/tools/executor.rs` | Tool Executor | Remove `flow: ToolFlow` parameter from `dispatch_persistence`. |
| `app/src-tauri/src/persistence/mod.rs` | Persistence | Remove `tool_flow: ToolFlow` from `PersistenceEvent::ToolCallExecuted`. |
| `app/src-tauri/src/persistence/tool_calls.rs` | Persistence | Remove `tool_flow` matching and remove `tool_kind` column insert. |
| `app/src-tauri/src/persistence/schema.rs` | Database (ZBC) | Remove `tool_kind TEXT NOT NULL` from `session_tool_calls` table schema. |
| `app/src-tauri/src/harness/loop.rs` | Coordinator | Multi-tool batch execution (`step6_execute_tool_batch`); speech hold parking; cancel bridge. |
| `app/src-tauri/src/harness/steps.rs` | Steps | Universal tool batch execution; `transition_harness_phase` with Epoch guards; query staging. |
| `app/src-tauri/src/pipeline/assistant/playback.rs` | Playback Engine | Duplex Quiescence Law: check `is_llm_done()`, `current_activity()`, and audio drain. |
| `app/src-tauri/src/pipeline/assistant/llm.rs` | Latch Evaluation | Epoch-guarded `mark_llm_finished(turn_id)`; audio drain checks on `pending_tts_jobs` and `sink_occupied`. |
| `app/src-tauri/src/pipeline/atomics.rs` | Atomics | Added `hardware_cancel_flag`, `stream_cancel_token`, `llm_done`, `utterance_id`. Purged shadow flags. |
| `app/src-tauri/src/services/audio/playback.rs` | Audio Playback | Pre-roll occupancy publishing before job decrement; buffer flush on cancel. |

---

### 4.2 Exact Type & Signature Modifications

#### 1. `LlmStreamEvent` & `LlmResponse`
```rust
// In services/llm/provider.rs
pub enum LlmStreamEvent {
    Thinking(String),            // Isolated chain-of-thought stream
    Token(String),               // Spoken conversational content
    ToolCall(CanonicalToolCall), // Structured tool invocation
    Finished,                    // Stream generation complete
}

// In services/llm/actor.rs
pub enum LlmResponse {
    Thinking(String),
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

pub struct CanonicalToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    // REMOVED: pub flow: ToolFlow,
}

// In harness/tools/mod.rs
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
// In harness/stages/streaming/router.rs
#[derive(Debug, Clone)]
pub enum StreamPassOutcome {
    Completed {
        assistant_text: String,
        thinking_text: String,
    },
    ToolCallsReceived {
        preamble_text: String,
        thinking_text: String,
        calls: Vec<CanonicalToolCall>,
    },
    Cancelled {
        partial_text: String,
    },
    PausedForSpeech {
        partial_text: String,
    },
    Superseded {
        stale: u32,
        current: u32,
    },
    Error(String),
}
```

#### 4. Turso Database Migration (ZBC)
```sql
-- Migration (v16): Drop legacy tool_kind column directly
ALTER TABLE session_tool_calls DROP COLUMN tool_kind;
```

---

## 5. Authoritative Specification Contracts (Locked in `docs/specs/harness-spec.md`)

These contracts are formally locked in [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md) and supersede all legacy logic:

### RULE-HARN-09 (3-Stream Demuxing & Thinking Isolation)
1. The streaming router MUST demux incoming LLM responses into three channels in a single pass:
   - **Thinking Channel (`LlmStreamEvent::Thinking`)**: Captured into isolated memory; never sent to TTS; never emitted as unthrottled per-token IPC events (complying with `RULE-EVT-02`); never persisted to `turns.assistant_text`.
   - **Content Token Channel (`LlmStreamEvent::Token`)**: Eagerly chunked into speakable clauses and emitted to `IpcEvent::LlmToken`.
   - **Tool Call Channel (`LlmStreamEvent::ToolCall`)**: Buffered until `LlmResponse::Finished` to assemble complete multi-tool batches.
2. Concrete adapters (`chat_completions.rs`, `ollama.rs`, `sidecar.rs`) MUST parse structured reasoning deltas or strip inline `<think>...</think>` tags at the network boundary.
3. The router MUST NOT terminate stream consumption on the first tool call. It MUST continue reading until `Finished`, `Cancelled`, `PausedForSpeech`, or `Error`.

### RULE-HARN-10A (Preamble Speech Preservation)
1. Spoken words emitted prior to tool calls in a generation pass MUST NOT be dropped, chunker-cleared, or overwritten.
2. The complete preamble string MUST be preserved in `StreamPassOutcome::ToolCallsReceived` and recorded in conversation history as the assistant message initiating the tool calls.

### RULE-HARN-10B (Duplex Quiescence Law & Precedence Latch)
1. Mid-stream clauses extracted before `LlmResponse::Finished` are dispatched to TTS as standard speech generations without intent tags.
2. When playback begins, `on_playback_started` transitions `Thinking` or `Working` to `Speaking` while preserving `current_activity`.
3. Preamble audio playback on the sound card and background tool execution MUST execute concurrently. The coordinator MUST NOT await audio draining before launching tool futures.
4. When audio playback drains while tools are active (`is_llm_done() == false` and `current_activity.is_some()`), `on_playback_finished` transitions to `Working` (restoring the tool envelope).
5. When a pass finishes with zero tool calls (`StreamPassOutcome::Completed`), `loop.rs` emits `VoxEvent::LlmFinished { turn_id }`.
6. Upon receiving `VoxEvent::LlmFinished`, `on_llm_finished` calls epoch-guarded `mark_llm_finished(turn_id)` to set `llm_done = true` (**Key 1**). If audio is active (`pending_tts_jobs > 0 || sink_occupied == true`), the router remains in current state until audio drains.
7. Transition to `InteractionState::Ready` is governed strictly by the **Duplex Quiescence Law** (`is_llm_done() == true` AND `pending_tts_jobs == 0` with sound card buffer drained).
8. If a turn produces zero audio, the TTS worker emits fallback `PlaybackFinished` once all jobs finish and the sink is empty.

### RULE-HARN-11 (ZBC Universal Tool Interface & Zero Filler Schemas)
1. Tool definitions MUST NOT declare execution flow categories (`Terminal` vs `NonTerminal`).
2. Tool JSON schemas MUST NOT include synthetic speech parameters (`spoken_filler` or `spoken_response`).
3. State transitions to `InteractionState::Working` MUST be executed solely by the coordinator upon inspecting tool batch presence.
4. The coordinator supports iterative ReAct execution up to `MAX_TOOL_STEPS = 20`. If step 20 is reached without a terminal answer, the Harness forces a final generation pass with tools disabled (`allow_tools = false`).

---

## 6. Edge-Case, Asymmetry & Failure Matrix (Pivot Gates)

| Scenario / Trigger | Failure Mode / Physics | Deterministic v2 Resolution |
| :--- | :--- | :--- |
| **Silent Wi-Fi Blackhole** | Provider socket hangs mid-stream after emitting preamble tokens. | `recv_timeout(100ms)` accumulates idle time. At 30s (`max_idle`), returns `Err("LLM provider response timed out")`. `loop.rs` catches error, transitions to `Ready`, and purges turn tokens. |
| **Abrupt Socket RST** | Remote provider drops connection during tool invocation. | `recv_timeout` returns `RecvTimeoutError::Disconnected`. Router logs error and returns `Err("LLM stream disconnected prematurely")`. No hang. |
| **Zero-Preamble Model** | Model immediately emits tool calls with zero spoken words. | `accumulator` remains empty. Router flushes nothing. `step6_execute_tool_batch` transitions immediately to `Working`. System remains silent with zero dead-air filler. |
| **User Barge-In During Preamble (Stage 1 & 2)** | User speaks while preamble is playing. | **Stage 1 (`SpeechStart`):** `playback_engine.cancel()` mutes sound card; `stream_cancel_token.cancel()` pauses stream; `utterance_id` allocated; router returns `PausedForSpeech`; loop parks at `inbox.wait_if_speech_held()`. `hardware_cancel_flag` is NOT set.<br>**Stage 2 (`TranscriptFinal`):** If empty, `rearm_hardware_cancel_flag()` and `release_speech_hold()` resume loop at Step 4. If non-empty, `cancel_active_turn()` finalizes turn, advances `next_turn()`, and starts new turn. |
| **Late-Final Speech Segment** | Delayed STT transcript arrives from prior speech segment while in `Listening`. | Evaluated via `RULE-EVT-01A`: if `bound_utterance_id < state.pipeline.peek_utterance_id()`, transcript appends to continuation prefix instead of starting rogue turn. |
| **Reasoning Model Context Overflow** | Model emits 4,000 thinking tokens before preamble. | `thinking_buffer` accumulates in RAM. Context budget evaluator tracks prompt + completion tokens against ceiling. If ceiling exceeded, provider returns error; router exits cleanly. |
| **Multi-Tool Mixed Outcome** | Batch contains 3 tools; Tool 1 succeeds, Tool 2 fails with timeout, Tool 3 succeeds. | Barrier scheduler captures each outcome independently. All 3 tool results commit into history as separate `tool` messages. The LLM receives full observation context in the next step. |
