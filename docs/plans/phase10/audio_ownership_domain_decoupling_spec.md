# Push-To-Talk, Realtime Audio Ownership & VAD Boundary Specification

---

## 1. Executive Summary & Purpose

This specification defines the decoupled architecture, audio ownership boundaries, state transitions, and functional roles of the Voice Activity Detection (VAD) subsystem across all Vox interaction pipelines (`modular`, `realtime`, and `dictation`).

### Core Objectives:
1. **Zero Upstream Dependency Leakage in VAD:** Eliminate all direct references, conditional matching (`if realtime_ptt`, `if modular_ptt`), and domain imports (`crate::services::pipeline::*`) from `services/vad/`. Low-level services must have zero knowledge of callers.
2. **Deduplicated Functional VAD Roles:** Refactor VAD into 3 generic primitives:
   - `ContinuousSegmentation` (Autonomous speech onset/offset detection with utterance dispatch)
   - `WindowedValidation` (Binary speech presence validation for caller-owned time windows)
   - `StreamPassthrough` (Low-latency audio chunk routing)
3. **True Server-Driven Realtime Passive Lifecycle:** Realtime S2S (Gemini Live / Deepgram) bypasses local VAD for turn segmentation. State transitions are driven 100% by server-side turn signals and playback DAC events.
4. **Standardized PTT State Machine:** In PTT modes, button press transitions `Ready` -> `Listening`. On release, speech detection transitions `Listening` -> `Thinking`, while silence/ghost-audio transitions `Listening` -> `Ready`.
5. **Continuous UI Telemetry:** Mic energy and FFT subband telemetry for 60 FPS Orb animation are computed continuously, completely decoupled from turn segmentation.
6. **Mandatory State Transition Docstrings:** Every pipeline domain function must explicitly document its state transitions in docstrings.

---

## 2. The 3 Deduplicated Functional Roles of VAD

Low-level audio processing in `services/vad/` is decoupled into three generic operational roles:

```
                               ┌──────────────────────────────┐
                               │       CPAL Audio Feed        │
                               └──────────────┬───────────────┘
                                              │
                         ┌────────────────────┴────────────────────┐
                         │                                         │
                         ▼                                         ▼
         ┌───────────────────────────────┐         ┌───────────────────────────────┐
         │     UI Telemetry Generator    │         │     VAD Functional Engine     │
         │   (Computes Energy & FFT)     │         │   (Configured in 1 of 3 Modes)│
         │   • Emits 60 FPS for Orb      │         └───────────────┬───────────────┘
         └───────────────────────────────┘                         │
                                    ┌──────────────────────────────┼──────────────────────────────┐
                                    ▼                              ▼                              ▼
                     ┌─────────────────────────────┐┌─────────────────────────────┐┌─────────────────────────────┐
                     │ 1. ContinuousSegmentation   ││   2. WindowedValidation     ││    3. StreamPassthrough     │
                     │  (Autonomous Speech Bounds) ││    (PTT Speech Presence)    ││     (Direct Audio Route)    │
                     └──────────────┬──────────────┘└──────────────┬──────────────┘└──────────────┬──────────────┘
                                    │                              │                              │
                                    ▼                              ▼                              ▼
                             Emits SpeechStart/End       Returns bool to Caller        Direct Chunk Forwarding
                             Dispatches Utterance       (Zero state/turn events)       (Zero state/turn events)
```

### Role 1: `ContinuousSegmentation`
- **Used by:** `modular/passive.rs`, `dictation/passive.rs`
- **Contract:**
  - Evaluates neural ONNX VAD model on every incoming audio frame.
  - Detects `SpeechStart` -> emits event to pipeline router.
  - Detects `SpeechEnd` -> segments PCM utterance buffer and dispatches to configured STT channel.
  - Manages speech debouncing, pre-roll buffering, and silence timeout.

### Role 2: `WindowedValidation`
- **Used by:** `modular/ptt.rs`, `realtime/ptt.rs`, `dictation/ptt.rs`
- **Contract:**
  - The caller domain defines the recording window (`handle_ptt_start` -> `handle_ptt_stop`).
  - Evaluates voice energy/VAD probability strictly to answer: *Did human speech occur during this window?*
  - On window close, returns `is_speech_detected: bool` to the caller.
  - **Emits zero turn events, zero state transitions, and zero STT commands.** Audio routing ownership remains with the caller domain.

### Role 3: `StreamPassthrough`
- **Used by:** `realtime/passive.rs`
- **Contract:**
  - Forwards incoming PCM audio chunks directly to the destination sender channel (e.g. Cloud WebSocket).
  - **Emits zero turn events, zero state transitions, and performs zero local segmentation.**

---

## 3. Domain State Transitions & Audio Ownership

### 3.1 Realtime Passive Mode (Hands-Free S2S)

Realtime Passive is a full-duplex conversational session where turn detection and semantic completion are owned by the server.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                         REALTIME PASSIVE STATE MACHINE                           │
├──────────────────────────────────────────────────────────────────────────────────┤
│ 1. Session Start:               Idle ───────────────> Listening (Skips Ready!)   │
│                                                                                  │
│ 2. Server Finalizes User Turn:  Listening ──────────> Thinking                   │
│    (TranscriptFinal / TurnComplete from Server)                                  │
│                                                                                  │
│ 3. Server Audio Arrives:        Thinking ───────────> Speaking                   │
│    (PlaybackStarted / First PCM chunk)                                           │
│                                                                                  │
│ 4. Server Barge-In / Interrupt: Speaking ───────────> Listening                  │
│    (Server sends 'interrupted' -> DAC cancelled)                                 │
│                                                                                  │
│ 5. Assistant Audio Finishes:    Speaking ───────────> Listening                  │
│    (PlaybackFinished on local DAC)                                               │
│                                                                                  │
│ 6. Session End:                 Listening ──────────> Idle                       │
└──────────────────────────────────────────────────────────────────────────────────┘
```

- **Audio Flow:** Microphone audio is passed directly via `StreamPassthrough` to the server WebSocket.
- **Client VAD:** Zero event emissions. Runs only raw energy/FFT calculation for Orb animation.

---

### 3.2 Realtime & Modular Push-To-Talk (PTT) Modes

PTT interaction boundaries are governed by the user's physical button press.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                            PUSH-TO-TALK STATE MACHINE                            │
├──────────────────────────────────────────────────────────────────────────────────┤
│ 1. Session Start:          Idle ───────────────> Ready                           │
│                                                                                  │
│ 2. Button Pressed:         Ready ──────────────> Listening                       │
│    (handle_ptt_start -> arms buffer, interrupts ongoing playback)                │
│                                                                                  │
│ 3. Button Released:                                                              │
│    ├── Speech Detected:    Listening ──────────> Thinking                        │
│    │   (Dispatches buffer to Cloud Realtime WebSocket or Local STT)              │
│    └── Silence / Ghost:    Listening ──────────> Ready                           │
│        (Discards buffer, resets UI without cloud/STT request)                    │
│                                                                                  │
│ 4. Audio Playback Starts:  Thinking ───────────> Speaking                        │
│                                                                                  │
│ 5. Playback Finishes:      Speaking ───────────> Ready                           │
│                                                                                  │
│ 6. Session End:            Ready ──────────────> Idle                            │
└──────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Mandatory Code Structure & Documentation Rules

### 4.1 Zero Domain Imports in Lower-Level Services
The following imports and any equivalent paths are strictly forbidden in `services/vad/`, `services/stt/`, `services/tts/`, and `services/audio/`:
```rust
// FORBIDDEN IN LOWER-LEVEL SERVICES:
use crate::services::pipeline::modular::*;
use crate::services::pipeline::realtime::*;
use crate::services::pipeline::dictation::*;
```

### 4.2 State Transition Function Headers
Every function in `services/pipeline/{modular,realtime,dictation}/` must include an explicit state transition docstring.

Example:
```rust
/// State Transition: Ready -> Listening
/// Initiates Push-To-Talk recording and interrupts ongoing playback.
pub fn handle_ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> { ... }

/// State Transition: Listening -> Thinking (if speech detected) | Listening -> Ready (if ghost audio)
/// Finalizes Push-To-Talk recording and dispatches audio buffer if speech was present.
pub fn handle_ptt_stop<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> { ... }
```

---

## 5. Verification Invariants

1. **VAD Independence:** Compiling `services/vad/` with all `pipeline` domain modules removed/mocked must succeed without unresolved imports.
2. **Ghost Audio Rejection:** Releasing PTT without speaking must return the state to `Ready` with 0 bytes sent to STT or Cloud WebSockets.
3. **Realtime Passive Start State:** Calling `realtime::passive::start_session` must transition directly from `Idle` to `Listening`.
4. **Orb Animation Continuous:** Orb UI telemetry events (`EVENT_TELEMETRY`) must continue firing across all states (`Listening`, `Thinking`, `Speaking`, `Ready`) as long as audio input is active.

---

## 6. Comprehensive Subsystem Decoupling & Inverted Dependency Fixes

Based on the architectural audit across all lower-level services (`services/vad/`, `services/audio/`, `services/dictation/`, `services/stt/`, `services/tts/`), the following explicit fixes are mandated to eliminate upward domain leakage:

### 6.1 `services/vad/actor.rs` Decoupling
- **Current Violation:** Lines 509, 513, 515 import and directly call `pipeline::dictation::ingest_audio`, `pipeline::realtime::ptt::ingest_audio`, and `pipeline::modular::ptt::ingest_audio`. Lines 174–177 switch on `InteractionOwner` to emit UI events to `"main"` vs `"tray"`.
- **Mandated Fix:**
  1. Purge all direct domain calls. `VadActor` exposes the 3 clean functional modes (`ContinuousSegmentation`, `WindowedValidation`, `StreamPassthrough`).
  2. Audio chunks are dispatched exclusively through generic channels (`mpsc::Sender<AudioChunkEvent>` or pre-configured target channel).
  3. Window-targeted event emissions (`"main"` vs `"tray"`) are moved entirely to the central pipeline router (`services/pipeline/router.rs`).

### 6.2 `services/audio/engine.rs` Decoupling
- **Current Violation:** Lines 7, 313 import and call `spawn_router` from `services/pipeline/router.rs`, making the low-level audio module act as an application god-object. Lines 145–258 directly mutate `state.pipeline` atomics and states.
- **Mandated Fix:**
  1. Relocate application assembly and router lifecycle management up to `crate::core::engine` or `crate::services::pipeline::manager`.
  2. `services/audio/` is restricted strictly to CPAL hardware streams (`AudioStream`), output playback buffer draining (`PlaybackEngine`), and audio decoding (`decode.rs`).

### 6.3 `services/dictation/` Decoupling (`hotkey.rs`, `output_router.rs`)
- **Current Violation:** `hotkey.rs:42, 50` calls `pipeline::dictation::handle_hotkey_*`. `output_router.rs:35, 44` forces UI tray window creation and reads `state.pipeline.turn_id`.
- **Mandated Fix:**
  1. `register_global_hotkey` accepts an abstract command channel `Sender<DictationCommand>` or callback closure. The pipeline registers the hotkey and supplies the handler.
  2. `output_router.rs` is restricted purely to OS text output destinations (`Clipboard`, `SystemInputAdapter`). UI event emissions and `turn_id` attribution are owned by the pipeline dispatcher.

### 6.4 `services/stt/actor.rs` Decoupling
- **Current Violation:** Lines 171–176 hardcode `"main"` window targeting and emit PTT-specific UI events (`"ptt_status"`).
- **Mandated Fix:**
  1. `SttActor` emits generic `VoxEvent::SttDecoding { duration_ms }` over the internal pipeline channel.
  2. The central pipeline router routes events to active UI windows based on the current interaction context.

### 6.5 `services/tts/actor.rs` Decoupling
- **Current Violation:** Lines 71–81 spin up an ad-hoc single-thread Tokio runtime on the worker thread to query SQLite DB for voice reference audio paths.
- **Mandated Fix:**
  1. The caller sending `TtsCommand::Synthesize` pre-resolves the voice reference path (`reference_audio: Option<PathBuf>`) or provides a cached reference path in the command payload.
  2. `TtsActor` performs pure CPU/GPU audio inference with zero database access and zero Tokio runtime instantiation.
