# State-Event Orchestration Architecture Map

> **Purpose:** Single Source of Truth (SSOT) mapping the exact relationship between Events, State Transitions, and Reactive Side-Effects across all 6 Pipeline Domains.

---

## 1. The 6 Interaction Domains & Their Ownership

Vox operates across **6 distinct operational pipelines**, divided by Owner and Mode:

| Domain ID | Owner | Mode | Description / Subsystem | Primary I/O |
|---|---|---|---|---|
| **D1: Modular-Passive** | Assistant | Passive | Hands-free continuous voice conversation (Local VAD $\to$ STT $\to$ LLM $\to$ TTS $\to$ Playback) | Mic $\to$ Audio Out |
| **D2: Modular-PTT** | Assistant | PTT | Push-To-Talk voice conversation (PTT hold $\to$ VAD window $\to$ STT $\to$ LLM $\to$ TTS $\to$ Playback) | Mic $\to$ Audio Out |
| **D3: Realtime-Passive** | Assistant | Passive | Hands-free cloud WebSocket audio/audio streaming (Local VAD $\to$ Cloud duplex) | Mic $\to$ Audio Out |
| **D4: Realtime-PTT** | Assistant | PTT | Push-To-Talk cloud WebSocket audio streaming | Mic $\to$ Audio Out |
| **D5: Dictation-PTT** | Dictation | PTT | Push-To-Talk OS text typing (Hotkey hold $\to$ VAD window $\to$ STT $\to$ OS Keyboard/Paste) | Mic $\to$ OS Focused App |
| **D6: Dictation-Passive**| Dictation | Passive | Hands-free background OS text transcription (VAD $\to$ STT $\to$ OS Keyboard/Paste) | Mic $\to$ OS Focused App |

---

## 2. The Canonical States & Invariants

```
[ Idle ] ──(start_session / engage)──► [ Ready ] ──(speech_start / ptt_press)──► [ Listening ]
   ▲                                       ▲                                            │
   │                                       │                                   (speech_end / ptt_release)
   │                                       │                                            ▼
   │                                (playback_finish)                            [ Thinking ]
   │                                       │                                            │
   │                                       │                                    (playback_start)
   │                                       ▼                                            ▼
   └──────────(end_session)───────── [ Speaking ] ◄─────────────────────────────────────┘
```

Auxiliary control states:
- **`Paused`**: Entered on `pause_session` IPC from `{Ready, Listening, Thinking, Speaking}`. Mic is muted / VAD frames dropped. Resumes to `Ready`.
- **`Error`**: Entered on fatal pipeline error. Resets to `Ready` on new turn or `Idle` on session exit.

---

## 3. Master Decision Tree & Alignment Decisions

### Area 1: Dictation vs Assistant State Machine Isolation
- **Decision:** **Option A (Clean Dual State Machines)**
  - **`InteractionState`** (Assistant): `Idle` | `Ready` | `Listening` | `Thinking` | `Speaking` | `Paused` | `Error`
  - **`DictationState`** (Dictation): `Idle` | `Recording` | `Transcribing` | `Error`
  - **Rationale:** Dictation is an ephemeral OS typing tool. It has no conversation session, no memory compaction, no LLM inference, and no TTS audio. Separating the state machines eliminates 100% of the cross-talk bugs where dictation hotkeys inadvertently reset assistant turn state.

---

### Realtime Pipeline Special Invariants
- **Server-Driven Speech Detection (Realtime Passive):** In realtime duplex mode, speech detection is strictly authoritative from the server (Gemini Live / Deepgram). `Ready -> Listening` transitions **only upon receiving the first input transcription / speech marker from the server**, avoiding false-positive desync between local Ten/Earshot VAD and cloud VAD.
- **Realtime 10-Minute Idle Timeout:** While in `Ready` state (in passive or PTT realtime), if no user utterance or server interaction occurs for 10 minutes (`REALTIME_IDLE_TIMEOUT = 600s`), the WebSocket connection is automatically closed to prevent billing/resource leaks and the state transitions `Ready -> Paused`. Resuming (`resume_session`) reconnects the WebSocket and returns to `Ready`.

---

### Area 2: State Transitions vs Streaming Payload Events
- **Decision:** **Strict Architectural Separation**
  - **State Control Events (Triggers):** Only explicit lifecycle triggers (`SpeechStart`, `SpeechEnd`, `PlaybackStarted`, `PlaybackFinished`, `Cancelled`, `Error`, `Pause`, `Resume`) can transition `InteractionState` via `transition(new_state)`.
  - **Payload Data Streams:** `LlmToken`, `TranscriptPartial`, `TtsChunk` are purely *data carriers* routed to their consumer (UI display or Audio Ring Buffer). They NEVER trigger state changes or background worker toggles.
  - **Impact:** Eradicates race conditions where tokens arriving slightly out of order corrupt the state machine.

---

### Area 3: Subsystem Reactive Observers (Ingestion, Compaction, Echo Suppression)
- **Decision:** **Unified `tokio::sync::watch` State Observer Bus**
  - **Single Bus:** The central state machine maintains a `tokio::sync::watch::Sender<InteractionState>`.
  - **Memory Ingestion Worker:** Subscribes to `watch::Receiver<InteractionState>`. Enters idle mode when `state ∈ {Ready, Paused, Idle}` for $\ge 30\text{s}$. Immediately pauses when `state ∈ {Listening, Thinking, Speaking}`.
  - **Background Compaction:** Triggered when `state ∈ {Ready, Paused}` and $20\text{s}$ debounce elapses. Cancelled immediately via cancellation token when `state` leaves `{Ready, Paused}`.
  - **VAD Echo Suppression:** Derives suppression state directly from `state == Speaking` without separate atomic flags.
  - **Eliminated:** Ad-hoc `PipelineActive` and `PipelineIdle` channel messages are deleted entirely.

---

### Area 4: Elimination of Redundant Atomic Flags & Pure Enum Matching
- **Decision:** **Complete Purge of Derived/Dead Atomics & Zero Helper Booleans**
  - **Deleted Atomics:** `is_paused`, `playback_active`, `is_assistant_speaking`, `llm_generating`, `tts_generating`.
  - **No Synthetic Helper Booleans:** No wrapper methods (`is_speaking()`, `is_paused()`). Code everywhere compares directly against the single enum:
    ```rust
    if state == InteractionState::Speaking { ... }
    match state {
        InteractionState::Ready | InteractionState::Paused => { ... }
        _ => { ... }
    }
    ```
  - **Single Source of Truth:** `InteractionState` read from `state.pipeline.state()` or observer watch channel is the sole check across the entire codebase.

---

### Area 5: Barge-In, Cancellation, and Turn Invalidation
- **Decision:** **Turn-Scoped Monotonic Epochs & Cancellation**
  - **Barge-In Behavior:** On `SpeechStart` while state $\in \{\text{Thinking}, \text{Speaking}\}$:
    1. Monotonic `turn_id` increments atomically (`fetch_add(1) + 1`).
    2. Active audio playback buffer is immediately cleared/drained.
    3. Old turn's scoped cancellation token triggers, halting prior in-flight LLM/TTS generation cleanly.
    4. State transitions directly to `Listening`.
  - **Overloaded Global Flag Replaced:** Removes the overloaded multi-purpose `cancel_flag` in favor of clean per-turn cancellation tokens.

---

### Area 6: Frontend Synchronization & Telemetry Fixes
- **Decision:** **Unified `state_changed` Master IPC Stream**
  - **Telemetry Fix:** Corrected `monitoring/collector.rs:34-44` discriminant mapping to match the exact canonical 7 enum values (`Idle=0`, `Ready=1`, `Listening=2`, `Thinking=3`, `Speaking=4`, `Paused=5`, `Error=6`).
  - **Frontend UI Reactive Alignment:** The Frontend Orb, HUD, and status visualizer consume `state_changed` as the single authoritative visual state driver.
  - **Redundant IPC Events Purged:** Redundant lifecycle IPC events (`speech_start`, `speech_end`, `playback_started`, `playback_finished`) that duplicate `state_changed` are removed. Streaming payload events (`transcript_partial`, `transcript_final`, `llm_token`) remain focused on data payload delivery.

---

## 4. Complete State Transition Matrix (SSOT)

| From State | Event / Trigger | Guard Condition | To State | Side-Effects & Observers Notified |
|---|---|---|---|---|
| **`Idle`** | `start_session` / `engage` | None | **`Ready`** | Preloads Identity facts, resets turn counters, emits `state_changed(Ready)` |
| **`Ready`** | `SpeechStart` (Modular VAD) / PTT Press | Not Paused | **`Listening`** | Scoped turn cancellation active, Ingestion paused, emits `state_changed(Listening)` |
| **`Ready`** | First Input Transcription (Realtime Server) | Realtime Passive Mode | **`Listening`** | Server-driven detection confirmed, emits `state_changed(Listening)` |
| **`Ready`** | 10-Minute Idle Timeout (`REALTIME_IDLE_TIMEOUT`) | Realtime Mode + 600s elapsed | **`Paused`** | Closes WebSocket connection to prevent billing leaks, emits `state_changed(Paused)` |
| **`Listening`** | `SpeechEnd` (VAD) / PTT Release | Valid speech window | **`Thinking`** | Dispatches audio to STT, emits `state_changed(Thinking)` |
| **`Listening`** | PTT Release | No speech detected (silence) | **`Ready`** | Discards buffer, emits `state_changed(Ready)` with 0 STT calls |
| **`Thinking`** | `PlaybackStarted` (first TTS samples) | None | **`Speaking`** | Starts CPAL audio drain, enables echo suppression, emits `state_changed(Speaking)` |
| **`Thinking`** | `SpeechStart` (Barge-In) | None | **`Listening`** | Cancels in-flight LLM/TTS generation, increments `turn_id`, emits `state_changed(Listening)` |
| **`Speaking`** | `PlaybackFinished` (buffer drained) | None | **`Ready`** | Soft compaction evaluated, 30s idle timer started, emits `state_changed(Ready)` |
| **`Speaking`** | `SpeechStart` (Barge-In) | None | **`Listening`** | Flushes playback ring buffer, cancels TTS, increments `turn_id`, emits `state_changed(Listening)` |
| **`Paused`** | `resume_session` | State is `Paused` | **`Ready`** | Resumes VAD listening / reconnects Realtime WS, emits `state_changed(Ready)` |
| **Any non-Idle State** | `pause_session` | State != `Idle` | **`Paused`** | Suspends voice processing, frees VAD/mic focus, emits `state_changed(Paused)` |
| **Any non-Idle State** | `end_session` | State != `Idle` | **`Idle`** | Full engine teardown/standby, closes WS, emits `state_changed(Idle)` |
| **Any State** | `pipeline_error` | None | **`Error`** | Logs error, notifies UI, emits `state_changed(Error)` |
| **`Error`** | New Turn / `reset` | None | **`Ready`** | Resets error state, emits `state_changed(Ready)` |

---

## 5. Granular 12-Sprint Implementation Plan

To ensure rigorous execution without cutting corners or taking shortcuts, the refactoring is decomposed into **12 targeted sprints**:
where  there are three more docs that this backend md should refer
- **Sprint 01:** Delete dead atomics (`llm_generating`, `tts_generating`) across `core/state.rs` and `monitoring/collector.rs`.
- **Sprint 02:** Remove `is_assistant_speaking` and replace all read sites (`playback.rs`, `telemetry_emitter.rs`) with direct `state == InteractionState::Speaking` checks.
- **Sprint 03:** Remove `playback_active` and replace all read sites (`vad/actor.rs`, `engine.rs`, `services/audio/`) with direct `state == InteractionState::Speaking` checks.
- **Sprint 04:** Remove `is_paused` and replace all read sites (`passive.rs`, `ptt.rs`, `facade.rs`) with direct `state == InteractionState::Paused` checks.
- **Sprint 05:** Implement the `tokio::sync::watch` State Observer Bus in `core/state.rs` and wire `PipelineAtomics::set_state` to broadcast state changes.
- **Sprint 06:** Refactor `persistence/memory_worker.rs` to subscribe to the state watch channel, removing `MemoryWorkerEvent::PipelineActive` and `PipelineIdle`.
- **Sprint 07:** Refactor background soft compaction in `facade.rs` to subscribe to the state watch channel, eliminating manual triggers on `PlaybackFinished`.
- **Sprint 08:** Create dedicated `DictationState` (`Idle`, `Recording`, `Transcribing`, `Error`) in `core/state.rs` and migrate `services/pipeline/dictation.rs` to use it exclusively.
- **Sprint 09:** Implement turn-scoped `CancellationToken` generation and refactor barge-in handling in `modular/passive.rs` and `modular/ptt.rs`.
- **Sprint 10:** Implement Realtime Passive server-driven `Ready -> Listening` transition and 10-minute idle WebSocket timeout in `realtime/engine.rs` and `realtime/passive.rs`.
- **Sprint 11:** Fix `monitoring/collector.rs` state discriminant mapping bug and remove redundant frontend lifecycle IPC events (`speech_start`, `playback_started`, etc.), standardizing UI on `state_changed`.
- **Sprint 12:** Full test suite execution, mutation checks, and documentation synchronization (`docs/features/`, `docs/backend.md`, `AGENTS.md`).
