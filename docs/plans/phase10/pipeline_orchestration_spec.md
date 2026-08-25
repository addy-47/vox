# Pipeline Orchestration Specification (Root Blueprint)

> **Status:** ACTIVE Socratic Definition (Finalized Architecture Blueprint)  
> **Location:** `docs/plans/phase10/pipeline_orchestration_spec.md`  
> **Scope:** Full architecture of `app/src-tauri/src/ipc/` and `app/src-tauri/src/services/`  
> **Standard:** Soft 50-line cap per function, single `///` docstring, zero silent swallows, deterministic paths only.

---

## 1. Domain Model: The Two-Tier Control Architecture

We reject treating all buttons and events as flat peer actions in a single file. Voice interactions operate on two distinct control planes:

```
┌────────────────────────────────────────────────────────────────────────┐
│                        TIER 1: SESSION LIFECYCLE                       │
│  Controls the existence of the voice environment (Models/Engines/WS)   │
│                                                                        │
│   [ Start Session ] ───► ( Session Active ) ───► [ End Session ]      │
│                                 │   ▲                                  │
│                                 ▼   │                                  │
│                          ( Paused State )                              │
│                      [ Pause ]     [ Resume ]                          │
└────────────────────────────────────┬───────────────────────────────────┘
                                     │
                                     ▼
┌────────────────────────────────────────────────────────────────────────┐
│                         TIER 2: TURN CONTROL                           │
│     Controls audio gating & turn execution inside an active session    │
│                                                                        │
│   PASSIVE MODE:                                                        │
│     • Continuous streaming / Local VAD onset OR Server-VAD (Gemini)    │
│     • No manual button clicks needed during speech                     │
│                                                                        │
│   PTT (PUSH-TO-TALK) MODE:                                             │
│     • PointerDown / KeyDown  ──► ptt_start (Gate Open / Buffer Ingest) │
│     • PointerUp / KeyUp      ──► ptt_stop  (Gate Close / Dispatch)     │
│     • Escape / Lost Focus    ──► ptt_cancel (Flush Buffer / No-op)     │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. The 3 Canonical Settings Matrix

All downstream execution paths are completely determined by these 3 orthogonal settings:

```
┌───────────────────────────────┐
│       1. pipeline_mode        │ ──► Modular (Local/Cloud STT+LLM+TTS) OR Realtime (Gemini S2S)
└───────────────────────────────┘
┌───────────────────────────────┐
│      2. interaction_mode      │ ──► Passive (Autonomous VAD) OR PTT (User Gated)
└───────────────────────────────┘
┌───────────────────────────────┐
│     3. dictation.enabled      │ ──► Background OS utility active OR Disabled
└───────────────────────────────┘
```

This yields **4 distinct conversational execution pipelines** + **1 unified dictation engine**:

1. **`modular_passive.rs`**: Local VAD → Embedded STT → LLM (Local/Cloud) → TTS (Local/Remote)
2. **`modular_ptt.rs`**: PTT Ringbuffer → Embedded STT → LLM (Local/Cloud) → TTS (Local/Remote)
3. **`realtime_passive.rs`**: Unconditional PCM Stream → Gemini Live WebSocket S2S (Server-VAD)
4. **`realtime_ptt.rs`**: PTT Gated PCM Stream → Gemini Live WebSocket S2S (Gated audio chunks)
5. **`dictation.rs`**: Unified Passive & PTT OS text ingestion → Output Router (Paste/Clipboard/Tray)

---

## 3. Interaction State Machine (Canonical 7 Turn States)

Tier 1 Session Lifecycle (`AppState.is_engaged: AtomicBool`) governs whether a conversational session exists (`Active` vs `Dormant`). Tier 2 Turn Control defines the discrete turn state (`InteractionState` enum):

| State | Definition | Audio Ingestion | Model / WS State | Target Window |
|---|---|---|---|---|
| `Idle` | Session active in PTT mode; mic gate closed, waiting for user PTT hold. | Gated / Buffering only on PTT | Warm & Ready | `"main"` |
| `Listening` | Session active in Passive mode; waiting for user speech onset. | Continuous (Passive VAD) | Warm & Ready | `"main"` |
| `UserSpeaking` | User is actively talking (VAD active or PTT held). | Buffering / Streaming to Provider | Ingesting audio | `"main"` / `"tray"` |
| `Thinking` | User finished speaking; LLM inference or RAG context compaction active. | Standby | Inferring / Compacting | `"main"` |
| `AssistantSpeaking`| System audio playback is active. | Ducked (Speaker) or Active (Headset / PTT) | Streaming playback | `"main"` |
| `Paused` | User explicitly paused session. | Discarded | Paused (Audio muted) | `"main"` |
| `Error` | Recoverable or unrecoverable subsystem error. | Halted | Logged with reason | `"main"` / `"tray"` |

*Note on Session Dormancy:* When `is_engaged == false`, the session is cold (`Dormant`). The UI renders cold unlit state, and audio hardware is stopped unless background Dictation is active.

### State Machine Invariants:
1. **Context Maintenance / Compaction:** Runs strictly inside the `Thinking` state. Transition speech plays while in `Thinking`, before final LLM response generation.
2. **Acoustic Barge-In vs Speaker Echo Protection:**
   - **`AudioOutputMode::Headset`:** Full acoustic barge-in. While in `AssistantSpeaking`, user speech onset immediately sets `cancel_flag = true`, halts playback, and transitions directly to `UserSpeaking`.
   - **`AudioOutputMode::Speaker`:** Open speaker audio bleeds into the microphone. To prevent self-interruption acoustic feedback loops, mic frames are ducked/muted during `AssistantSpeaking`. Passive barge-in is suppressed; user can manually interrupt via PTT key or UI tap.
   - **PTT Mode:** Pressing the PTT button (`ptt_start`) always triggers instantaneous barge-in regardless of output mode.

---

## 4. Directory & Module Architecture Plan

```
app/src-tauri/src/
├── ipc/
│   ├── audio.rs                 <-- Audio devices & hardware configuration
│   ├── history.rs               <-- Conversation history queries
│   ├── memory.rs                <-- RAG memory queries & management
│   ├── memory_profiler.rs       <-- RAM/VRAM inspection
│   ├── monitoring.rs            <-- Latency & RTF metrics
│   ├── settings.rs              <-- Settings management
│   ├── setup.rs                 <-- Onboarding setup IPC
│   ├── tray.rs                  <-- System tray interactions
│   ├── voices.rs                <-- Voice model selection
│   └── pipeline/
│       ├── mod.rs               <-- Module declarations & Tauri command registration
│       ├── assistant.rs         <-- Voice Assistant IPC dispatcher (start_session, end_session, pause_session, resume_session, ptt_start, ptt_stop, ptt_cancel)
│       ├── dictation.rs         <-- Unified Dictation IPC (settings, recovery, clipboard copy, toggles)
│       └── test_clip.rs         <-- Isolated QA / Dev audio clip testing utility
└── services/
    ├── utils.rs                 <-- SHARED SSOT for token buffering (should_flush), script detection, prompt formatting
    ├── audio/
    │   ├── engine.rs            <-- SSOT for VoxEngine hardware launch (CPAL mic, playback, VAD/STT threads)
    │   ├── stream.rs            <-- CPAL input audio capture stream
    │   └── playback.rs          <-- CPAL output playback engine & jitter buffer
    ├── llm/
    │   ├── actor.rs             <-- LLM worker thread spawn, warm_up_llm, cool_down_llm
    │   └── mod.rs               <-- LLM providers and capabilities exports
    ├── tts/
    │   ├── actor.rs             <-- TTS worker thread spawn, warm_up_tts, cool_down_tts
    │   └── mod.rs               <-- TTS providers exports
    ├── vad/
    │   └── actor.rs             <-- Dedicated OS audio thread: Ringbuffer pop, RMS telemetry, VAD boundary detection
    └── pipeline/
        ├── router.rs            <-- Central lock-free VoxEvent pump & domain dispatcher (~150 LOC)
        ├── context.rs           <-- RoutingContext struct (derived ONCE from settings snapshot)
        ├── state_machine.rs     <-- InteractionState transitions & targeted window broadcast
        ├── modular_passive.rs   <-- Modular + Passive implementation (~380 LOC)
        ├── modular_ptt.rs       <-- Modular + PTT implementation (~350 LOC)
        ├── realtime_passive.rs  <-- Realtime + Passive implementation (~350 LOC)
        ├── realtime_ptt.rs      <-- Realtime + PTT implementation (~300 LOC)
        └── dictation.rs         <-- Unified Passive & PTT Dictation (~300 LOC)
```

### Deprecations, Relocations & Simplifications:
- **Central Event Pump (`services/pipeline/router.rs`)**: Replaces the monolithic `event_loop.rs`. A thin, non-blocking actor loop that consumes `VoxEvent` frames from the mpsc channel and dispatches discrete calls to the active domain module based on `RoutingContext`.
- **`ipc/dictation.rs` CONSOLIDATED into `ipc/pipeline/dictation.rs`**: All dictation IPC commands are located in `ipc/pipeline/dictation.rs` alongside `ipc/pipeline/assistant.rs`.
- **Isolated QA Utility (`ipc/pipeline/test_clip.rs`)**: Retained as a dedicated developer and QA test clip injection tool without polluting core session IPC handlers.
- **`ipc/pipeline/engine_launch.rs` RELOCATED to `services/audio/engine.rs`**: Audio hardware I/O, CPAL streams, and worker thread spawning belong exclusively to the audio service layer.
- **`services/ptt.rs` DELETED**: Completely deconstructed. PTT IPC commands move to `ipc/pipeline/assistant.rs`, and PTT domain logic is partitioned into `modular_ptt.rs`, `realtime_ptt.rs`, and `dictation.rs`.
- **No Standalone `lifecycle.rs` Files in `llm/` and `tts/`**: `warm_up_llm`/`cool_down_llm` and `warm_up_tts`/`cool_down_tts` live directly inside `services/llm/actor.rs` and `services/tts/actor.rs`.
- **`services/vad/actor.rs` Streamlined**: Stripped of UI state machine manipulation and inline settings mutexes. It is strictly the dedicated high-priority audio processing actor that calculates RMS telemetry, performs VAD prediction, and emits `VoxEvent` frames over lock-free channels.
- **Combined `dictation.rs`**: Passive and PTT dictation workflows are unified into a single clean `services/pipeline/dictation.rs` module.

---

## 5. Ownership Model & Invariants

### 5.1 The Two Canonical Owners
We eliminate the conflation between owner and interaction mode (`MainWindow`, `Ptt`, `Wizard`). There are strictly **two** logical owners:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InteractionOwner {
    Assistant, // The primary conversational voice AI
    Dictation, // The background OS transcription utility
}
```

### 5.2 Deterministic Transition Rules
1. **Start Assistant Session (`start_assistant_session`):**
   - Atomically sets `owner = InteractionOwner::Assistant`.
   - If audio engine is cold (Dictation was disabled), launches audio engine.
   - If audio engine is already warm (Dictation was active), retains engine and transitions routing to Assistant.
2. **End Assistant Session (`end_assistant_session`):**
   - If `dictation.enabled == true`: Sets `owner = InteractionOwner::Dictation`. Keeps audio engine running in background.
   - If `dictation.enabled == false`: Sets `owner = None` / Dormant. Stops audio engine to conserve memory & CPU.
3. **Dictation Hotkey Collision:**
   - When `owner == InteractionOwner::Assistant`, Assistant has exclusive mic priority. Global dictation hotkey presses are suppressed/ignored with a debug log.
4. **Changing Dictation Settings in UI:**
   - If Assistant session is active, settings are saved to disk without modifying active `owner`.
   - If Assistant is dormant and Dictation is toggled ON, launches engine and sets `owner = InteractionOwner::Dictation`.
   - If Assistant is dormant and Dictation is toggled OFF, stops engine.
### 5.3 Legacy Purge Inventory & Bug/Fallback Catalog

To eliminate cognitive load and prevent regressions during backend implementation, the following legacy patterns and bugs must be strictly purged:

#### A. `InteractionOwner` Purge Checklist (Reduced to Exactly 3 SSOT Locations)
In the legacy codebase, `InteractionOwner` was conflated across 22 files with interaction modes (`Ptt`) and UI windows (`MainWindow`, `Wizard`). In the refactored architecture, `InteractionOwner` exists in **strictly 3 locations**:
1. **`AppState.owner` (`core/state.rs`)**: Single atomic/RwLock designating active subsystem (`Assistant` vs `Dictation`).
2. **Session Start/End IPC Handlers (`ipc/pipeline/assistant.rs` & `dictation.rs`)**: Where ownership transitions are initiated.
3. **VAD Audio Event Tagging (`VoxEvent::SpeechStart` / `SpeechEnd`)**: Where audio boundaries are tagged with the active owner for event routing.

**Legacy locations to purge:**
- **`core/state.rs`**: Delete variants `MainWindow`, `Ptt`, `Wizard`. Delete `From<u32>` and `From<u8>` conversion hacks (`1 => MainWindow`, `2 => Ptt`, etc.).
- **`services/pipeline/event_loop.rs` & `handlers.rs`**: Purge all `match owner { MainWindow | Ptt => "main", Wizard => "wizard" }` routing logic. Routing is based on `state_machine.rs`.
- **`services/vad/actor.rs`**: Purge `VadCommand::UpdateOwner` buffer mutation logic. VAD strictly tags `VoxEvent` frames.
- **`services/ptt.rs`**: Delete entire file and its inline owner mutations.
- **`ipc/pipeline/lifecycle.rs`, `engine_launch.rs`, `realtime.rs`**: Purge all 8 scattered `state.owner.store(...)` calls.
- **`ipc/settings/mutation.rs` & `ipc/setup.rs`**: Purge `InteractionOwner::Wizard` state overrides.
- **`monitoring/telemetry_emitter.rs` & `collector.rs`**: Purge owner-to-window string mapping matches.
- **`services/realtime/providers/*.rs`**: Purge owner checks (realtime WebSocket providers are strictly Assistant engines).
- **`services/dictation/controller.rs` & `output_router.rs`**: Purge internal owner checking (dictation is invoked directly by domain router).

#### B. Lock Inversions to Eliminate
- **`state.engine` vs `state.realtime_engine` Inversion**:
  - Legacy `ipc/pipeline/realtime.rs` (lines 52, 224) and `services/ptt.rs` (lines 84, 215) acquired `realtime_engine.lock()` before `engine.lock()`.
  - Legacy `event_loop.rs` (line 451) acquired `engine.lock()` before `realtime_engine.lock()`.
  - **Rule:** Strictly acquire `state.engine` before `state.realtime_engine`.

#### C. Silent Error Swallows & Fallback Chains to Eliminate
- **Zero Silent `let _ = tx.send(...)`**: All cross-thread channel sends (`stt_tx`, `llm_tx`, `tts_tx`, `vox_event_tx`, `telemetry_tx`) must log `log::warn!` on failure or return an explicit `VoxError`.
- **No Silent Fallback Chains**: Purge silent model fallbacks (`if cloud fails silently try local`). If a selected model/provider fails, emit an explicit `EVENT_MODEL_FAILED` / `Error` state to UI so the user is informed.
- **Eliminate Event Loop Polling**: Purge `recv_timeout(Duration::from_millis(150))` in `event_loop.rs`. `PlaybackEngine` emits `PlaybackStarted` and `PlaybackDrained` events to drive state transitions deterministically.

---

## 6. Interaction UX & Control Paradigms (Passive vs PTT)

### 6.1 UX Differences

```
PASSIVE UX (Autonomous):
[ Start Session ] ──► State: Listening (Mic Open)
                      ├── UI Buttons: [ Pause / Resume ] & [ End Session ]
                      └── Flow: Listening ──► (VAD) ──► UserSpeaking ──► Thinking ──► AssistantSpeaking ──► Listening

PTT UX (User Gated):
[ Start Session ] ──► State: Idle (Mic Closed, Engines Warm)
                      ├── UI Buttons: [ 🎙️ Hold to Talk (Mic Button) ] & [ End Session ]
                      │   (No "Pause" button needed — not holding mic is already paused)
                      └── Flow: Idle ──► (ptt_start) ──► UserSpeaking ──► (ptt_stop) ──► Thinking ──► AssistantSpeaking ──► Idle
```

### 6.2 The 3 PTT IPC Commands

| IPC Command | Physical Trigger | Exact Action Taken |
|---|---|---|
| `ptt_start` | `PointerDown` / `KeyDown` (Space or Hotkey) | Sets `is_recording = true`, resets audio buffer and `speech_detected`. If Assistant is speaking, immediately cuts off playback (barge-in) and switches state to `UserSpeaking`. |
| `ptt_stop` | `PointerUp` / `KeyUp` (Normal release) | Sets `is_recording = false`. If `speech_detected == true`, transitions to `Thinking` and sends buffer to STT (`SttCommand::Final`). If silence, discards buffer silently and returns to `Idle`. |
| `ptt_cancel` | `Escape` key / Pointer drag outside / Window lost focus | Immediately drops and clears the buffer without sending anything to STT or LLM. Resets state to `Idle`. |

---

## 7. Domain Pipeline Step-by-Step Function Inventory

### 7.1 `modular_passive.rs` (Estimated LOC: ~380)

```rust
// ─── EVENT 1: Start Session (Dormant -> Listening) ───────────────────────────
pub async fn start_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Ensure engine is launched (CPAL stream + VAD + STT worker active)
    // Step 2: Set active owner to InteractionOwner::Assistant
    // Step 3: Lazy-load MemoryScope classifier in background thread
    // Step 4: Generate new epoch conversation_id & record SessionStarted in DB
    // Step 5: Notify MemoryWorker of active session change
    // Step 6: Warm up LLM and TTS workers
    // Step 7: Transition InteractionState to Listening & broadcast to "main" window
}

// ─── EVENT 2: Pause Session (Listening / Thinking -> Paused) ─────────────────
pub async fn pause_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Set is_paused = true and cancel_flag = true (abort playback & inference)
    // Step 2: Send SttCommand::ResetStream to reset STT decoder
    // Step 3: Transition InteractionState to Paused & emit "pipeline_paused" to UI
}

// ─── EVENT 3: Resume Session (Paused -> Listening) ───────────────────────────
pub async fn resume_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Set is_paused = false and cancel_flag = false
    // Step 2: Send VoxEvent::WarmUp to ensure worker readiness
    // Step 3: Transition InteractionState to Listening & emit "pipeline_resumed" to UI
}

// ─── EVENT 4: Speech Start (Listening / AssistantSpeaking -> UserSpeaking) ────
pub fn on_speech_start(turn_id: u32, app: &AppHandle, state: &AppState) {
    // Step 1: Set cancel_flag = true to interrupt any ongoing LLM/TTS/Playback (Barge-in)
    // Step 2: Send SttCommand::ResetStream to STT worker
    // Step 3: Transition InteractionState to UserSpeaking & broadcast to "main" window
}

// ─── EVENT 5: Speech End (UserSpeaking -> Thinking) ──────────────────────────
pub fn on_speech_end(turn_id: u32, app: &AppHandle, state: &AppState, audio_buffer: Vec<f32>) {
    // Step 1: Send SttCommand::Final(turn_id, audio_buffer) to STT worker
    // Step 2: Transition InteractionState to Thinking & broadcast to "main" window
}

// ─── EVENT 6: Transcript Final (Thinking) ────────────────────────────────────
pub async fn on_transcript_final(turn_id: u32, text: String, app: &AppHandle, state: &AppState) {
    // Step 1: Check for empty/silence transcript -> if empty, reset to Listening
    // Step 2: Run MemoryScope classifier on transcript (ChitChat vs Domain vs Temporal)
    // Step 3: If not ChitChat, retrieve RAG memory facts from Turso DB
    // Step 4: Push user turn to ConversationManager & resolve system prompt
    // Step 5: If FIFO context maintenance triggered transition speech, send to TTS
    // Step 6: Dispatch LlmCommand::Generate with resolved prompt to LLM worker
}

// ─── EVENT 7: LLM Token & Dynamic TTS Flush (Thinking -> AssistantSpeaking) ───
pub fn on_llm_token(turn_id: u32, token: String, state: &AppState) {
    // Step 1: Append token to sub-sentence token accumulator buffer
    // Step 2: Evaluate should_flush(buffer, word_count, elapsed_ms, tps) from services/utils.rs
    // Step 3: If should_flush == true:
    //         - Pop chunk from buffer
    //         - Dispatch TtsCommand::Generate(chunk) to TTS worker
}

// ─── EVENT 8: TTS Chunk Audio Playback (AssistantSpeaking) ───────────────────
pub fn on_tts_chunk(turn_id: u32, samples: Vec<f32>, state: &AppState, app: &AppHandle) {
    // Step 1: Upsample 24kHz -> 48kHz via Cubic Hermite interpolation
    // Step 2: Push upsampled audio to CPAL PlaybackEngine jitter buffer
    // Step 3: Transition InteractionState to AssistantSpeaking (if not already)
}

// ─── EVENT 9: Playback Finished (AssistantSpeaking -> Listening) ─────────────
pub fn on_playback_finished(turn_id: u32, app: &AppHandle, state: &AppState) {
    // Step 1: Record turn completion metrics (STT RTF, LLM TPS, TTFT, TTFA) in DB
    // Step 2: Transition InteractionState back to Listening
}

// ─── EVENT 10: End Session (Listening / Paused -> Dormant) ───────────────────
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Abort any active turn (cancel_flag = true, stop playback)
    // Step 2: Emit PersistenceEvent::SessionEnded & trigger MemoryWorker consolidation
    // Step 3: Evaluate Dictation settings:
    //         - If dictation.enabled == true -> Hand ownership to Dictation, keep engine warm
    //         - If dictation.enabled == false -> Stop engine, evict ONNX models, trim heap
    // Step 4: Transition InteractionState to Dormant & broadcast to UI
}
```

---

### 7.2 `modular_ptt.rs` (Estimated LOC: ~350)

```rust
// ─── EVENT 1: Start Session (Dormant -> Idle) ────────────────────────────────
pub async fn start_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Ensure engine is launched (CPAL stream + VAD + STT worker active)
    // Step 2: Set active owner to InteractionOwner::Assistant
    // Step 3: Warm up LLM and TTS workers
    // Step 4: Transition InteractionState to Idle (Waiting for user to hold mic button)
}

// ─── EVENT 2: PTT Press / Start (Idle / AssistantSpeaking -> UserSpeaking) ───
pub fn handle_ptt_start(app: &AppHandle, state: &AppState) {
    // Step 1: Atomic CAS is_recording -> true (prevent double press)
    // Step 2: If Assistant was speaking, cancel playback & LLM immediately (Barge-in)
    // Step 3: Reset PTT audio buffer & speech_detected = false
    // Step 4: Bump turn_id
    // Step 5: Transition InteractionState to UserSpeaking & emit "RECORDING" status
}

// ─── EVENT 3: PTT Ingestion & Waveform (UserSpeaking) ────────────────────────
pub fn handle_audio_frame(samples: &[f32], state: &AppState) {
    // Step 1: Append audio chunk to ptt.audio_buffer
    // Step 2: Calculate RMS energy & check noise gate -> if speech, set speech_detected = true
    // Step 3: Every 60ms emit audio_energy telemetry for waveform animation
    // Step 4: If hard limit reached (10 min), auto-trigger ptt_stop
}

// ─── EVENT 4: PTT Release / Stop (UserSpeaking -> Thinking / Idle) ────────────
pub fn handle_ptt_stop(app: &AppHandle, state: &AppState) {
    // Step 1: Atomic CAS is_recording -> false
    // Step 2: Check speech_detected atomic:
    //         - If false (silence hold): Discard buffer, transition to Idle
    //         - If true (speech detected):
    //             • Transition InteractionState to Thinking & emit "PROCESSING" status
    //             • Send full buffer to STT (SttCommand::Final)
}

// ─── EVENT 5: PTT Cancel (UserSpeaking -> Idle) ──────────────────────────────
pub fn handle_ptt_cancel(app: &AppHandle, state: &AppState) {
    // Step 1: Clear audio buffer & reset flags
    // Step 2: Transition InteractionState to Idle & emit "IDLE" status
}

// ─── EVENT 6: Turn Completion (AssistantSpeaking -> Idle) ────────────────────
pub fn on_playback_finished(turn_id: u32, app: &AppHandle, state: &AppState) {
    // Step 1: Record turn completion metrics
    // Step 2: Transition InteractionState back to Idle (NOT Listening — waiting for next PTT hold)
}

// ─── EVENT 7: End Session (Idle -> Dormant) ──────────────────────────────────
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Clear any lingering PTT buffer
    // Step 2: Finalize persistence session in DB
    // Step 3: Stop or maintain engine based on dictation.enabled
    // Step 4: Transition InteractionState to Dormant
}
```

---

### 7.3 `realtime_passive.rs` (Estimated LOC: ~350)

```rust
// ─── EVENT 1: Start Session (Dormant -> Listening) ───────────────────────────
pub async fn start_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Ensure engine is launched
    // Step 2: Connect WebSocket session to Gemini Live (load cached resumption handle if present)
    // Step 3: Set active owner to InteractionOwner::Assistant
    // Step 4: Register realtime audio bridge with VAD actor
    // Step 5: Transition InteractionState to Listening
}

// ─── EVENT 2: Pause Session & Go-Away Protection (Listening -> Paused) ───────
pub async fn pause_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Signal activity_end to WebSocket provider, stop local playback
    // Step 2: Transition InteractionState to Paused (audio bridge mutes mic forwarding)
    // Step 3: Maintain warm WebSocket. If server sends goAway or idle timeout triggers,
    //         cache the latest sessionResumption token and close WebSocket gracefully.
}

// ─── EVENT 3: Resume Session & Resumption Recovery (Paused -> Listening) ──────
pub async fn resume_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Check if WebSocket connection is still alive
    // Step 2: If disconnected, reconnect using cached sessionResumption token to restore context
    // Step 3: Re-enable audio streaming and transition InteractionState to Listening
}

// ─── EVENT 4: Stream Continuous Audio Frame ──────────────────────────────────
pub fn stream_audio_chunk(samples: &[f32], state: &AppState) {
    // Step 1: Convert f32 PCM to i16 PCM
    // Step 2: Send PCM frame over realtime_tx to WebSocket channel
}

// ─── EVENT 5: Server VAD Speech Start (Listening -> UserSpeaking) ────────────
pub fn on_server_speech_start(app: &AppHandle, state: &AppState) {
    // Step 1: Cancel any local audio playback (Barge-in)
    // Step 2: Transition InteractionState to UserSpeaking
}

// ─── EVENT 6: Server Audio Frame Received (UserSpeaking -> AssistantSpeaking) 
pub fn on_server_audio_frame(samples: Vec<f32>, state: &AppState) {
    // Step 1: Stream samples to PlaybackEngine
    // Step 2: Transition InteractionState to AssistantSpeaking
}

// ─── EVENT 7: Server Turn Complete (AssistantSpeaking -> Listening) ──────────
pub fn on_server_turn_complete(app: &AppHandle, state: &AppState) {
    // Step 1: Transition InteractionState back to Listening
}

// ─── EVENT 8: End Session (Listening / Paused -> Dormant) ────────────────────
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Send session termination message to WebSocket & close connection
    // Step 2: Transition InteractionState to Dormant
}
```

---

### 7.4 `realtime_ptt.rs` (Estimated LOC: ~300)

```rust
// ─── EVENT 1: Start Session (Dormant -> Idle) ────────────────────────────────
pub async fn start_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Connect WebSocket session to Gemini Live
    // Step 2: Set active owner to InteractionOwner::Assistant
    // Step 3: Transition InteractionState to Idle
}

// ─── EVENT 2: PTT Press / Start (Idle -> UserSpeaking) ───────────────────────
pub fn handle_ptt_start(app: &AppHandle, state: &AppState) {
    // Step 1: Atomic CAS is_recording -> true
    // Step 2: Signal activity_start to realtime WebSocket provider
    // Step 3: Transition InteractionState to UserSpeaking
}

// ─── EVENT 3: Gated PCM Streaming (UserSpeaking) ─────────────────────────────
pub fn stream_gated_pcm(samples: &[f32], state: &AppState) {
    // Step 1: Client VAD gate check -> if speech_detected, stream i16 chunk to WebSocket
}

// ─── EVENT 4: PTT Release / Stop (UserSpeaking -> Thinking / Idle) ────────────
pub fn handle_ptt_stop(app: &AppHandle, state: &AppState) {
    // Step 1: Atomic CAS is_recording -> false
    // Step 2: Signal activity_end to realtime WebSocket provider
    // Step 3: Transition InteractionState to Thinking
}

// ─── EVENT 5: Turn Complete (AssistantSpeaking -> Idle) ──────────────────────
pub fn on_server_turn_complete(app: &AppHandle, state: &AppState) {
    // Step 1: Transition InteractionState to Idle
}

// ─── EVENT 6: End Session (Idle -> Dormant) ──────────────────────────────────
pub async fn end_session(app: &AppHandle, state: &AppState) -> Result<(), VoxError> {
    // Step 1: Disconnect WebSocket and return to Dormant
}
```

---

### 7.5 `dictation.rs` (Unified Passive & PTT Engine, Estimated LOC: ~300)

```rust
// ─── EVENT 1: Global Hotkey Press (Dormant -> UserSpeaking) ──────────────────
pub fn handle_hotkey_press(app: &AppHandle, state: &AppState) {
    // Step 1: Start buffering mic audio in dictation buffer
    // Step 2: Transition InteractionState to UserSpeaking & emit to "tray" window
}

// ─── EVENT 2: Global Hotkey Release (UserSpeaking -> Thinking -> Dormant) ─────
pub async fn handle_hotkey_release(app: &AppHandle, state: &AppState) {
    // Step 1: Send buffer to STT worker (SttCommand::Final)
    // Step 2: Receive transcript & paste directly into active focused window via OS input simulation
    // Step 3: Transition InteractionState to Dormant
}

// ─── EVENT 3: Passive Background Utterance Finalized ─────────────────────────
pub async fn on_passive_utterance(turn_id: u32, audio: Vec<f32>, app: &AppHandle, state: &AppState) {
    // Step 1: Send audio buffer to STT worker (SttCommand::Final)
    // Step 2: Await transcript result
    // Step 3: Route transcript to dictation/output_router.rs (Clipboard / Key Simulation / Tray)
    // Step 4: Zero LLM, zero TTS invocation
}
```

---

## 8. Architectural Trade-Offs & Invariants

| Dimension | Decision | Rationale |
|---|---|---|
| **Code Sharing vs. Isolation** | **Isolation > DRY coupling** | Keeping domain files separate guarantees that each voice mode can be modified, debugged, and benchmarked with 100% isolation. |
| **Dictation Engine Consolidation** | **Unified `dictation.rs`** | Passive and PTT dictation share identical STT finalization and OS output injection paths. Combining them into `services/pipeline/dictation.rs` prevents file sprawl while keeping LOC ~300. |
| **Central Event Router** | **Thin `router.rs` Event Pump** | Replaces 1145-line `event_loop.rs` with a single lock-free `mpsc::Receiver<VoxEvent>` loop that delegates directly to domain handlers. |
| **Function Length Cap** | **≤50 Lines Strict** | Every step within an event handler is a 1–3 line delegation to a specialized domain engine (`stt_tx`, `llm_tx`, `tts_tx`, `conversation_manager`, `db`). |
| **File LOC Budget** | **200–380 LOC (Hard limit: 500 LOC)** | By keeping functions under 50 LOC and delegating provider details to `llm/` and `tts/`, each domain file remains compact, clean, and highly readable. |
| **Acoustic Barge-In & Speaker Ducking** | **Headset Active / Speaker Muted** | In `Speaker` mode, mic frames are ducked/dropped during assistant playback to prevent self-interruption echo loops (barge-in via PTT/tap). In `Headset` mode, full acoustic barge-in is enabled via VAD. |
| **PTT Lock-Free Audio Pipeline** | **Bounded Channel Frame Stream** | In PTT mode, `vad/actor.rs` pushes 16ms audio frames over a bounded lock-free channel directly to the PTT accumulator, eliminating mutex locks on the high-priority audio thread. |
| **Realtime Telemetry Invariant** | **Local RMS Computation** | Even in Realtime S2S mode where Gemini handles VAD, local RMS calculations must run in `vad/actor.rs` to stream `audio_energy` for UI Orb visualizers. |
| **Gemini Resumption Lifecycle** | **Token Caching + Auto Reconnect** | During pause, Gemini sessions handle server `goAway` by caching `sessionResumptionUpdate` tokens to restore conversation state on resume. |
