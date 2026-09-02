# Target Event-Domain Architectural Specification (Ground Truth)

---

## 1. Executive Summary & Core Architectural Invariants

This document establishes the **target behavioral and concurrency specification** for the upcoming `pipeline/` refactor across all 4 interaction domains (**Modular Passive**, **Modular PTT**, **Realtime Passive**, **Realtime PTT**).

### Architectural Invariants

1. **Single-Writer Serialization Point:**
   All state-mutating commands (including user IPC commands like `session_start`, `pause_session`, `resume_session`, `end_session`, `ptt_start`, `ptt_stop`, `ptt_cancel`, and `idle_monitor` timeouts) push strongly-typed events onto the central `mpsc::Sender<VoxEvent>` queue. The central Router OS thread is the **sole writer / sole serialization point** for `InteractionState` mutations and turn lifecycles, guaranteeing strict FIFO ordering and eliminating multi-threaded transition races.

2. **Decoupled UI Streaming Streams:**
   High-frequency streaming text tokens (`LlmToken`) and live interim subtitles (`TranscriptPartial`) carry **zero FSM transition responsibility**. They bypass the central Router queue and are emitted directly to the frontend webview by worker actors (`SttActor`, `LlmActor`, `RealtimeActor`) via Tauri IPC.

3. **Immediate Speech-End Responsiveness (`SpeechEnd` / `PttStop` $\to$ `Thinking`):**
   When user speech concludes (VAD silence threshold reached in passive mode, or PTT key released in PTT mode), the pipeline transitions `Listening` $\to$ `Thinking` immediately. STT decodes in the background and emits `TranscriptFinal`, which acts as the validation gate: if speech is recognized, LLM generation proceeds; if text is empty, the pipeline recovers `Thinking` $\to$ `Ready` and displays an Info Toast (*"No speech recognized"*).

4. **Two-Layer Concurrency Defense:**
   - **Layer 1 (Dumb Emitters):** Low-level hardware and worker actors emit raw facts unconditionally without knowing assistant FSM state.
   - **Layer 2 (FSM Handler Guards):** Canonical handlers in `pipeline/handlers/` evaluate incoming facts against current `InteractionState`. In-flight turn events arriving while `Idle` or `Paused` are dropped with zero side effects.

5. **Two-Stage Error Recovery Protocol:**
   When an error occurs, state transitions to `Error` and broadcasts `voice_error`.
   - **Stage 1 (Resume Attempt):** The user triggers `resume_session`. The handler attempts to re-arm hardware, restart VAD, or reconnect the WebSocket. If successful, state transitions `Error` $\to$ `Ready`.
   - **Stage 2 (Fallback on Resumption Failure):** If resumption fails, state returns to `Error` and an explicit UI Toast is displayed: *"Resumption failed: [Reason]. Please end session and start a new session."* The user must then execute `end_session` $\to$ `Idle` followed by `session_start`.

---

## 2. Event Enumerations & Channel Contracts

### Central Router Event Queue (`VoxEvent`)

- **`SessionStart { owner }`**: Enqueued by IPC `session_start`. Initializes session memory, identity facts, and transitions `Idle` $\to$ `Ready`.
- **`PauseSession`**: Enqueued by IPC `pause_session` or `idle_monitor`. Cancels playback, trips cancellation flags, and transitions active states $\to$ `Paused`.
- **`ResumeSession`**: Enqueued by IPC `resume_session`. Re-arms audio/VAD/network from `Paused` or `Error` $\to$ `Ready`.
- **`EndSession`**: Enqueued by IPC `end_session`. Teardown hardware and remote connections, persists end metrics, transitions any state $\to$ `Idle`.
- **`PttStart`**: Enqueued by IPC `ptt_start`. Evaluates barge-in vs fresh onset, starts window capture, transitions `Ready`/`Thinking`/`Speaking` $\to$ `Listening`.
- **`PttStop`**: Enqueued by IPC `ptt_stop`. Stops window validation. If speech detected $\to$ transitions `Listening` $\to$ `Thinking` and dispatches audio to STT/provider; if silence $\to$ transitions `Listening` $\to$ `Ready`.
- **`PttCancel`**: Enqueued by IPC `ptt_cancel`. Aborts current PTT recording window, transitions `Listening` $\to$ `Ready`.
- **`SpeechStart { turn_id }`**: Emitted by local VAD (modular) or provider actor (realtime). Evaluates barge-in vs onset, transitions `Ready`/`Thinking`/`Speaking` $\to$ `Listening`.
- **`SpeechEnd { turn_id }`**: Emitted by local VAD (modular) or provider actor (realtime). Signals speech boundary, transitions `Listening` $\to$ `Thinking`, and dispatches full audio to STT.
- **`TranscriptFinal { turn_id, text }`**: Emitted by STT worker or provider. If valid text $\to$ stays in `Thinking` and spawns LLM generation; if empty $\to$ transitions `Thinking` $\to$ `Ready` with Info Toast.
- **`LlmFinished { turn_id }`**: Emitted when LLM completes text generation (modular) or server finishes synthesis (realtime). Persists turn context to database and flushes audio pre-roll.
- **`PlaybackStarted { turn_id }`**: Emitted by PlaybackEngine when ring buffer samples satisfy pre-roll threshold or flush. Transitions `Thinking` $\to$ `Speaking`.
- **`PlaybackFinished { turn_id }`**: Emitted by PlaybackEngine when output ring buffer drains to empty and no synthesis jobs remain. Transitions `Speaking` $\to$ `Ready`.
- **`Error { turn_id, message, source }`**: Emitted on subsystem failures. Transitions state to `Error` and broadcasts `voice_error` IPC event.
- **`Cancelled { turn_id }`**: Emitted on turn aborts. Transitions state to `Ready`.
- **`Shutdown`**: Router loop termination sentinel.

### Realtime Provider Internal Stream (`RealtimeProviderEvent`)

- **`AudioChunk(Vec<i16>)`**: Binary PCM audio stream routed directly to playback worker (bypasses Router).
- **`SpeechStart { turn_id }`**: Inbound speech onset detected by server VAD $\to$ translated to `VoxEvent::SpeechStart`.
- **`SpeechEnd { turn_id }`**: Inbound speech completion detected by server VAD $\to$ translated to `VoxEvent::SpeechEnd`.
- **`TranscriptPartial { turn_id, text }`**: Interim user speech transcription $\to$ emitted directly to UI via IPC (bypasses Router).
- **`TranscriptFinal { turn_id, text }`**: Finalized user speech transcription $\to$ translated to `VoxEvent::TranscriptFinal`.
- **`LlmToken { turn_id, token }`**: Streamed assistant text tokens $\to$ emitted directly to UI via IPC (bypasses Router).
- **`LlmFinished { turn_id }`**: Model generation complete marker $\to$ translated to `VoxEvent::LlmFinished`.
- **`Error { turn_id, message }`**: Provider network/protocol error $\to$ translated to `VoxEvent::Error`.
- **`SessionResumptionHandle { handle, model }`**: Session cache token $\to$ written to disk non-blocking.

---

## 3. Comprehensive 4-Domain Target Specification Matrix

| Trigger | Event | Modular Passive | Modular PTT | Realtime Passive | Realtime PTT |
|---|---|---|---|---|---|
| **IPC** | **`SessionStart`** | **Pre:** `Idle`<br>**Trans:** `Idle` $\to$ `Ready`<br>**Shared FX:** `init_new_session`, prefetch identity, set `owner=Assistant`, `cancel_flag=false`, persist `SessionStarted`, spawn `idle_monitor`<br>**Domain FX:** Start audio engine, warm STT/LLM/TTS workers, VAD set `ContinuousSegmentation` | **Pre:** `Idle`<br>**Trans:** `Idle` $\to$ `Ready`<br>**Shared FX:** Same shared FX<br>**Domain FX:** Start audio engine, lazy worker pool, VAD set `WindowedValidation`, arm hotkey | **Pre:** `Idle`<br>**Trans:** `Idle` $\to$ `Ready`<br>**Shared FX:** Same shared FX<br>**Domain FX:** Start audio engine, `RealtimeActor::start(Passive)` (open WS, check session cache), VAD set `StreamPassthrough` | **Pre:** `Idle`<br>**Trans:** `Idle` $\to$ `Ready`<br>**Shared FX:** Same shared FX<br>**Domain FX:** Start audio engine, `RealtimeActor::start(PTT)` (open WS standby), VAD set `WindowedValidation`, arm hotkey |
| **IPC / IdleMonitor** | **`PauseSession`** | **Pre:** Any active state (if `Idle`/`Paused` $\to$ no-op)<br>**Trans:** `Current` $\to$ `Paused`<br>**Shared FX:** `cancel_flag=true`, `token.cancel()`, `PlaybackEngine::cancel()`, `ACCUMULATOR.clear()`<br>**Domain FX:** None (local workers observe token) | Same shared FX.<br>**Domain FX:** None | Same shared FX.<br>**Domain FX:** `VadCommand::StopRealtime` (halt mic passthrough; keep WS alive) | Same shared FX.<br>**Domain FX:** `VadCommand::StopRealtime` |
| **IPC** | **`ResumeSession`** | **Pre:** `Paused` or `Error`<br>**Trans:** `Paused`/`Error` $\to$ `Ready` (on success) \| `Error` $\to$ `Error` (on fail)<br>**Shared FX:** `cancel_flag=false`, renew `token`<br>**Domain FX:** VAD set `ContinuousSegmentation` | Same shared FX.<br>**Domain FX:** VAD set `WindowedValidation`, re-arm hotkey | Same shared FX.<br>**Domain FX:** `VadCommand::StartRealtime`. Attempt WS reconnect if dropped. If fail $\to$ `Error` + Toast: *"Resumption failed. Please restart session."* | Same shared FX.<br>**Domain FX:** `VadCommand::StartRealtime`. Attempt WS reconnect if dropped. If fail $\to$ `Error` + Toast |
| **IPC** | **`EndSession`** | **Pre:** Any state (idempotent)<br>**Trans:** `Current` $\to$ `Idle`<br>**Shared FX:** `cancel_flag=true`, `token.cancel()`, `PlaybackEngine::cancel()`, `ACCUMULATOR.clear()`, persist `SessionEnded`, notify memory worker, stop audio engine<br>**Domain FX:** None | Same shared FX.<br>**Domain FX:** Disarm hotkey | Same shared FX.<br>**Domain FX:** `RealtimeActor::stop()`, disconnect WS, purge session cache | Same shared FX.<br>**Domain FX:** `RealtimeActor::stop()`, disconnect WS, purge session cache, disarm hotkey |
| **IPC** | **`PttStart`** | **N/A** (Passive rejects PTT) | **Pre:** Any state<br>**Cond:** If `Idle`/`Paused`/`Listening` $\to$ drop; if `Thinking`/`Speaking` $\to$ invoke `on_interrupt()`; if `Ready` $\to$ direct onset<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** `next_turn()`, `ACCUMULATOR.clear()`, cancel playback<br>**Domain FX:** `VadCommand::StartWindowValidation` | **N/A** | Same pre-state guard, conditions, transitions, and shared FX.<br>**Domain FX:** `VadCommand::StartWindowValidation` (if barge-in, invokes `on_interrupt()` which owns all interrupt duties including provider outbound interrupt) |
| **IPC** | **`PttStop`** | **N/A** | **Pre:** `Listening` (else drop)<br>**Cond:** Local VAD speech detected and audio non-empty?<br>**Trans:** Speech $\to$ `Thinking` \| Silence $\to$ `Ready`<br>**Shared FX:** Drain VAD window buffer<br>**Domain FX:** If speech $\to$ `SttCommand::Final(turn_id, audio)` to STT worker; if silence $\to$ drop buffer | **N/A** | Same pre-state guard, condition, transition, and shared FX.<br>**Domain FX:** If speech $\to$ convert to PCM i16 and dispatch `RealtimeActor::commit_speech_turn()`; if silence $\to$ drop buffer (0 network frames) |
| **IPC** | **`PttCancel`** | **N/A** | **Pre:** `Listening` (else no-op)<br>**Trans:** `Listening` $\to$ `Ready`<br>**Shared FX:** Cancel turn token, `ACCUMULATOR.clear()`, cancel playback, `VadCommand::StopWindowValidation` (discard buffer)<br>**Domain FX:** None | **N/A** | Same pre-state guard, transition, shared FX, and domain FX (audio was never committed to provider) |
| **VAD / RealtimeActor** | **`SpeechStart`** | **Pre:** Any state<br>**Cond:** If `Idle`/`Paused` $\to$ drop; if `Thinking`/`Speaking` $\to$ invoke `on_interrupt()`; if `Ready` $\to$ direct onset<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** `next_turn()`, `ACCUMULATOR.clear()`, cancel playback<br>**Domain FX:** Sourced from local `VadActor`; dispatches `SttCommand::ResetStream` | **N/A** (`PttStart` owns PTT speech onset) | Same pre-state guard, conditions, transitions, and shared FX.<br>**Domain FX:** Sourced from `RealtimeActor` (server VAD onset / first inbound speech packet); if barge-in, server already detected onset | **N/A** (`PttStart` owns PTT speech onset) |
| **VAD / RealtimeActor** | **`SpeechEnd`** | **Pre:** `Listening` (else drop)<br>**Trans:** `Listening` $\to$ `Thinking`<br>**Shared FX:** None<br>**Domain FX:** Sourced from local `VadActor` (silence threshold reached); dispatches `SttCommand::Final(turn_id, utterance_buffer)` to STT worker | **N/A** (`PttStop` owns PTT speech completion) | **Pre:** `Listening` (else drop)<br>**Trans:** `Listening` $\to$ `Thinking`<br>**Shared FX:** None<br>**Domain FX:** Sourced from `RealtimeActor` (server VAD completion event, e.g. Deepgram `UserStoppedSpeaking`) | **N/A** (`PttStop` owns PTT speech completion) |
| **STT / RealtimeActor** | **`TranscriptFinal`** | **Pre:** `Thinking` (if `Idle`/`Paused` $\to$ drop)<br>**Cond:** Text non-empty or empty?<br>**Trans:** Valid text $\to$ Remains `Thinking` \| Empty text $\to$ `Thinking` $\to$ `Ready`<br>**Shared FX:** If Valid $\to$ store transcript in accumulator, emit `IpcEvent::TranscriptFinal`. If Empty $\to$ emit `IpcEvent::ShowToast("No speech recognized", Info)`, clear accumulator<br>**Domain FX:** If Valid $\to$ spawn async LLM task (`prepare_turn` $\to$ `LlmCommand::Generate`) | Same guard, condition, transition, shared FX, and domain FX | Same guard, condition, transition, and shared FX.<br>**Domain FX:** If Valid $\to$ assert `pending_synthesis_jobs=1` (server already generating audio in-flight) | Same as Realtime Passive |
| *(Internal Handler)* | **`on_interrupt`** | Invoked strictly inside `SpeechStart` guard when in `Thinking` or `Speaking`.<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Immediate `PlaybackEngine::cancel()`; `cancel_flag=true`; cancel & renew `token`; `pending_synthesis_jobs=0`; flush partial assistant response from accumulator and persist `PersistenceEvent::TurnCompleted` under interrupted turn ID; clear accumulator; emit `IpcEvent::StateChanged(Listening)`<br>**Domain FX:** None (local models only) | Invoked strictly inside `PttStart` guard when in `Thinking` or `Speaking`.<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Same shared FX.<br>**Domain FX:** None (fully local) | Invoked strictly inside `SpeechStart` guard when in `Thinking` or `Speaking`.<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Same shared FX.<br>**Domain FX:** None (server detected barge-in first) | Invoked strictly inside `PttStart` guard when in `Thinking` or `Speaking`.<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Same shared FX.<br>**Domain FX:** Also dispatch outbound `OutboundCommand::Interrupt` to remote provider |
| **LLM / RealtimeActor** | **`LlmFinished`** | **Pre:** `Thinking` or `Speaking`<br>**Trans:** None (Playback controls return to `Ready`)<br>**Shared FX:** Consume `ACCUMULATOR` assistant text, push turn to `ConversationManager`, dispatch `PersistenceEvent::TurnCompleted`<br>**Domain FX:** Flush `TtsClauseChunker` remainder to TTS worker, invoke `PlaybackEngine::flush_pre_roll()` | Same shared FX and domain FX | Same shared FX.<br>**Domain FX:** Reset `pending_synthesis_jobs=0`, invoke `PlaybackEngine::flush_pre_roll()` | Same as Realtime Passive |
| **PlaybackEngine** | **`PlaybackStarted`** | **Pre:** `Thinking` (if `!=Thinking` $\to$ drop)<br>**Trans:** `Thinking` $\to$ `Speaking`<br>**Shared FX:** Emit `IpcEvent::StateChanged(Speaking)`<br>**Domain FX:** Triggered when playback ring buffer reaches modular pre-roll threshold (12,000 samples @ 48kHz = 250ms) or on `flush_pre_roll()` | Same | **Pre:** `Thinking` (if `!=Thinking` $\to$ drop)<br>**Trans:** `Thinking` $\to$ `Speaking`<br>**Shared FX:** Emit `IpcEvent::StateChanged(Speaking)`<br>**Domain FX:** Triggered when playback ring buffer reaches realtime pre-roll threshold (3,840 samples @ 48kHz = 80ms) or on `flush_pre_roll()` | Same |
| **PlaybackEngine** | **`PlaybackFinished`** | **Pre:** `Speaking` (if `!=Speaking` $\to$ drop)<br>**Trans:** `Speaking` $\to$ `Ready`<br>**Shared FX:** Emit `IpcEvent::StateChanged(Ready)`<br>**Domain FX:** Fired when output ring buffer drains to empty and `pending_synthesis_jobs == 0` | Same | Same (guarded by `pending_synthesis_jobs == 0`, preventing mid-speech network jitter cutoffs) | Same |
| **Any Actor** | **`Error`** | **Pre:** Any state<br>**Trans:** `Current` $\to$ `Error`<br>**Shared FX:** Log error with source attribution, cancel `PlaybackEngine`, emit `IpcEvent::VoiceError`, display UI error toast. Session remains in `Error` until user resumes or ends | Same | Same | Same |
| **Any Actor** | **`Cancelled`** | **Pre:** Any state<br>**Trans:** `Current` $\to$ `Ready`<br>**Shared FX:** Clear `TurnAccumulator`, reset `pending_synthesis_jobs=0`, cancel audio playback | Same | Same | Same |

---

## 4. Idle Monitor Specification

The Assistant pipeline runs an asynchronous background observer task (`spawn_idle_monitor`):

1. **Observer Trigger:** Subscribes to atomic `InteractionState` broadcasts.
2. **Timer Condition:** If state enters `InteractionState::Ready`, arms a continuous 7-minute ($420\text{s}$) timeout timer (`REALTIME_IDLE_TIMEOUT`).
3. **Reset Condition:** If state transitions away from `Ready` before 7 minutes elapse (e.g. to `Listening`), the timer is cancelled and resets.
4. **Execution Action:** If the timer expires while the pipeline is still in `Ready`:
   - Enqueues `VoxEvent::PauseSession` into the central event channel (`event_tx.send(VoxEvent::PauseSession)`).
   - The central Router OS thread processes the pause event, transitions state `Ready` $\to$ `Paused`, disarms mic passthrough/audio, and notifies the UI.
   - Logs: `"[Pipeline] Auto-pausing session after 7 minutes of idle Ready state."`

---

## 5. Canonical Internal Handler: Barge-In (`on_interrupt`)

`on_interrupt` is a synchronous helper executed exclusively inside handler guards when user speech or hotkey interaction occurs while the assistant is in `Thinking` or `Speaking`:

1. **Immediate Audio Silence:** Immediately calls `PlaybackEngine::cancel()` to drain and mute the CPAL hardware audio output buffer.
2. **Cancellation & Token Renewal:** Asserts `cancel_flag = true`, cancels the active `CancellationToken`, resets `pending_synthesis_jobs = 0`, and renews the turn token for the incoming turn.
3. **Outbound Provider Notification:** If in Realtime mode and the interrupt was client-detected (e.g. user pressed PTT key in Realtime PTT), dispatches `OutboundCommand::Interrupt` to the remote WebSocket session.
4. **Partial Turn Persistence:** Extracts any partially generated assistant text from `TurnAccumulator`. If non-empty, saves the partial assistant turn to `ConversationManager` and dispatches `PersistenceEvent::TurnCompleted` under the interrupted turn ID so conversational context is never lost.
5. **Accumulator Reset:** Clears `TurnAccumulator` for the incoming user utterance.
6. **State Transition:** Executes `transition(InteractionState::Listening)`.

---

## 6. Target Code Organization for Refactored `pipeline/`

```
app/src-tauri/src/pipeline/
├── mod.rs               # RoutingContext, transition(), target_window(), state query helpers, spawn_idle_monitor
├── router.rs            # spawn_router(): Single-FIFO central event pump loop
│
└── handlers/            # [CANONICAL EVENT HANDLERS ONLY]
    ├── mod.rs           # Match router events & dispatch to handlers
    ├── session.rs       # on_session_start, on_pause, on_resume, on_end
    ├── ptt.rs           # on_ptt_start, on_ptt_stop, on_ptt_cancel
    ├── speech.rs        # on_speech_start, on_speech_end
    ├── transcript.rs    # on_transcript_final (LLM spawn or empty text recovery)
    ├── llm.rs           # on_llm_finished (DB persistence & pre-roll flush)
    ├── playback.rs      # on_playback_started, on_playback_finished
    ├── interrupt.rs     # on_interrupt (shared barge-in handler)
    └── error.rs         # on_error, on_cancelled, voice_error & toast dispatch
```
