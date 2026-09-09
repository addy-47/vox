# LLM Agent Harness & Dual-Stream Demuxer Specification (Vox v2)

## 1. System Concept & Scope
The **LLM Agent Harness** is the unified runtime bridge between the voice interaction pipeline, language model providers, and backend task execution.
Its primary responsibilities are:
1. **Context Assembly & Budget Accounting**: Managing sliding-window conversational history, context window utilization thresholds, and system prompt formatting (injecting `personal_memory`).
2. **Dual-Stream Token Demuxing**: Splitting a single LLM generation stream into immediate user-facing voice/text vs. background orchestration instructions (e.g. titles, async agent tasks) without stalling voice latency or leaking orchestration tags to speech.
3. **Approach B Autonomous Task Delegation**: Coordinating background agent jobs (e.g. CLI tool executions, critical compactions) with the central voice pipeline state machine via autonomous completion callbacks.

---

## 2. Invariants & Preserved Logic

### 2.1 Preserved Budget Accounting Thresholds
From `services/harness/accountant.rs`:
- `CONTEXT_CRITICAL_THRESHOLD = 0.85`: At $\ge 85\%$ context utilization, critical compaction is triggered.
- `CONTEXT_SOFT_THRESHOLD = 0.65`: At $65\% \le \text{util} < 85\%$, opportunistic soft compaction is armed.
- `RESERVED_GENERATION_TOKENS = 512`: Reserved output headroom subtracted from `context_window` to compute usable context budget.

### 2.2 Preserved Core Data Structures
- `MessageBuffer`: In-memory FIFO queue of `ChatMessage` turns (`role`, `content`, `timestamp_ms`).
- `TokenAccountant`: Exact token count tracking, synchronization against active buffer, and utilization calculation.

### 2.3 Decommissioned Legacy Structures
- **Decommissioned**: Legacy ModernBERT scope classifier (`classify_scope`) and waterfall profile injection (`retrieve_turn_profile`) are completely removed from `prepare_turn_context`.
- **Replaced**: Context is assembled strictly from:
  1. Base system prompt + active `personal_memory` markdown.
  2. Session compaction context summary (if continuing or restored).
  3. FIFO uncompacted conversational turns.

---

## 3. Streaming Dual-Routing Demuxer (The Wire Engine)

### 3.1 Tagged Demuxing Architecture
To prevent orchestration logic from delaying speech playback or leaking into synthesized speech:
- Prompt instructions format conversational speech inside `<response>...</response>`, followed by trailing orchestration tags:
```xml
<response>I am running the test suite in the background now.</response>
<title>Test Suite Verification</title>
<task type="cli_agent" id="task_101">cargo test -p vox_lib</task>
```

### 3.2 Streaming Demuxer Finite State Machine (FSM)
The streaming token pump in `LlmActor` replaces raw token forwarding with a streaming character buffer running a 3-state machine:

```
                  ┌──────────────────────┐
                  │   InResponseText     │ ──(Token streams to TTS & UI)
                  └──────────────────────┘
                             │
                             ▼ (Encounter '<')
                  ┌──────────────────────┐
                  │    TagBuffering      │ ──(Buffer characters in memory)
                  └──────────────────────┘
                    │                  │
   (Matches '</response>')    (Matches '<task ...>' / '<title>')
                    │                  │
                    ▼                  ▼
          [Close TTS & Emit     ┌──────────────────────┐
           VoxEvent::LlmFinished]│ InOrchestrationTag   │ ──(Route to Consumer)
                                └──────────────────────┘
```

1. **`InResponseText`**:
   - Streamed characters are pushed immediately to `TurnAccumulator`.
   - Complete clauses stream directly to `TtsActor` (`TtsCommand::Generate`).
   - Streamed text emits `IpcEvent::LlmToken` to the active frontend window.
2. **`TagBuffering`**:
   - The moment `<` is encountered, tokens are buffered in an internal lookahead scratch string instead of streaming to TTS.
   - If the buffered text does not match any recognized tag prefix, the buffer flushes back to `InResponseText` and flows to TTS (handles literal `<` characters in prose safely).
3. **`InOrchestrationTag`**:
   - When `</response>` is matched:
     - The accumulator remainder is flushed to TTS.
     - **`VoxEvent::LlmFinished` is immediately emitted to the central Voice Router**.
     - From this instant, the speech pipeline is completely detached from the LLM generation: audio plays out smoothly, and the turn prepares to return to `Ready`.
   - Characters inside subsequent tags (e.g. `<title>`, `<task>`) are buffered exclusively into an orchestration payload struct and suppressed from TTS and frontend subtitle IPC.

---

## 4. Approach B: Autonomous Background Task Lifecycle

### 4.1 The Core Problem & Solution
When the LLM yields a background task (e.g. CLI agent execution, or an inline critical compaction):
- The assistant's initial response (*"I'm kicking off the test suite now"*) plays aloud.
- Playback completes $\to$ pipeline transitions `Speaking` $\to$ `Ready`.
- **Approach B Invariant**: The voice pipeline does NOT lock or freeze waiting for the background task. The microphone remains open and the user stays in control. The background task executes asynchronously on an isolated worker pool.

### 4.2 Pipeline Coordination via `VoxEvent::TaskCompleted`
To bridge background task completion back into the voice pipeline without race conditions or speech collision:

1. **Event Registration in `VoxEvent`**:
   We add a single canonical completion event to `VoxEvent`:
   ```rust
   pub enum VoxEvent {
       // ... existing voice events ...
       TaskCompleted {
           task_id: String,
           task_type: TaskType, // CliAgent, Compaction, Tool
           result: TaskResult,  // Success(String), Failed(String)
       },
   }
   ```

2. **Router State Transition & Collision Matrix**:
   When `VoxEvent::TaskCompleted` arrives at the central FIFO Router, it evaluates the active `InteractionState`:

| Active State at Router | Collision Behavior & Pipeline Action |
|---|---|
| **`Ready`** (User is silent) | **Autonomous Spoken Report**: Assistant autonomously initiates a speech turn.<br>1. Advances turn ID via `next_turn()`.<br>2. Transitions `Ready` $\to$ `Thinking`.<br>3. Prompts the LLM with the task result: *"Task [id] completed: [result]. Formulate a concise 1-sentence voice update for the user."*<br>4. Speaks result aloud (`Thinking` $\to$ `Speaking` $\to$ `Ready`). |
| **`Listening`** (User is speaking) | **Defer & Enqueue**: The task result is appended to `ConversationManager` working buffer as a high-priority system observation message (`Role::System`).<br>It is NOT spoken aloud immediately to prevent talking over the user.<br>When the user finishes speaking, the upcoming LLM turn naturally incorporates the task result into its response. |
| **`Thinking`** (Assistant is generating) | **Context Merge**: Appended to active generation context if turn has not finished; otherwise enqueued for the next turn. |
| **`Speaking`** (Assistant is currently talking) | **Wait for Playback**: Held until `PlaybackFinished` transitions the state to `Ready`, then evaluated under the `Ready` rule. |
| **`Paused` / `Sleeping`** | **Silent Persistence**: Persists task results to Turso DB and creates an actionable desktop notification (`NotificationCreated`), without waking audio hardware or speaking aloud. |

### 4.3 Task Cancellation Invariant
- If the user explicitly cancels or interrupts via barge-in (`PttCancel`, fresh `SpeechStart` while assistant is reporting), the voice turn aborts cleanly to `Ready`.
- Background CLI tasks receive their own cancellable token. If the user explicitly commands *"Cancel the running test suite"*, the LLM emits `<task_cancel id="..."/>`, cancelling the background process.

---

## 5. First-Turn Dynamic Session Title Generation

1. **Prompt Injection on Turn 1**:
   When `turn_id == 0` (or if `session.title IS NULL`), `prepare_turn_context` appends:
   ```
   At the end of your response, output a concise 3-5 word title summarizing the session topic inside <title>...</title>.
   ```
2. **Harness Tag Consumer**:
   - The streaming demuxer catches `<title>...</title>`.
   - Validates non-empty string, trims whitespace, and caps length to 48 characters.
   - Dispatches `PersistenceEvent::UpdateSessionMetadata { session_id, key: "title".into(), value: title.clone() }`.
   - Emits `IpcEvent::SessionTitleUpdated { session_id, title }` directly to the frontend.
3. **Failure Invariant**:
   If the LLM fails to output `<title>`, the session remains with its untitled placeholder. No error is thrown and the voice response is unaffected.
