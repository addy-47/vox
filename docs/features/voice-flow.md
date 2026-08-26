---
title: "Vox Voice Pipeline Flow"
audience: "Internal — backend & frontend contributors, system architects"
last_updated: 2026-08-25
owners: "backend-engineer role"
related_docs:
  - "docs/backend.md §3, §8 — Module layout & event contract"
  - "docs/plans/phase10/pipeline_orchestration_spec.md — SSOT for routing & invariants"
  - "docs/frontend.md §9 — Frontend event consumers"
---

# Vox — End-to-End Voice Pipeline Flow & Architecture

> **Purpose:** Canonical reference and trace for all voice interaction flows in Vox, documenting every domain's lifecycle, state transitions, audio routing, data formats, and latency characteristics. Phase 10 domain-partitioned architecture — router + 5 handlers, not a God loop.

---

## 1. High-Level Architecture Overview

Vox operates on a **Spec-First, Domain-Partitioned Pipeline Architecture** (Phase 10). Rather than routing audio and events through a monolithic God loop, all incoming audio, VAD events, STT transcripts, and LLM tokens flow through a non-blocking **Central Event Router** (`services/pipeline/router.rs`) which dispatches to one of **5 dedicated domain handlers**:

```
                              ┌──────────────────────────────────┐
                              │    Audio Capture & VAD Tier      │
                              │  (cpal 16kHz PCM + Earshot VAD)  │
                              └────────────────┬─────────────────┘
                                               │
                                               ▼
                                  ┌────────────────────────┐
                                  │  Central Event Router  │
                                  │      (router.rs)       │
                                  └────────────┬───────────┘
                                               │
                     ┌─────────────────────────┼─────────────────────────┐
                     ▼                         ▼                         ▼
        ┌─────────────────────────┐ ┌────────────────────┐    ┌────────────────────┐
        │     Modular Domains     │ │  Realtime Domains  │    │  Dictation Domain  │
        │ ┌─────────────────────┐ │ │ ┌────────────────┐ │    │ ┌────────────────┐ │
        │ │ modular_passive.rs  │ │ │ │realtime_passive│ │    │ │  dictation.rs  │ │
        │ ├─────────────────────┤ │ │ ├────────────────┤ │    │ └────────────────┘ │
        │ │   modular_ptt.rs    │ │ │ │  realtime_ptt  │ │    │   (0ms LLM/TTS     │
        │ └─────────────────────┘ │ │ └────────────────┘ │    │   Direct OS Paste) │
        └─────────────────────────┘ └────────────────────┘    └────────────────────┘
```

---

## 2. The Canonical 7-State Turn Machine

All surfaces (Main Window, Ambient Orb, Tray HUD, Status Capsules) and both Rust backend (`core/state.rs`) and TypeScript frontend (`services/eventsService.ts`) align strictly on the **Canonical 7-State Turn Machine**:

```
        ┌────────────────────────────────────────────────────────┐
        │                                                        │
        ▼                                                        │
    ┌────────┐    start_session()    ┌────────┐   speech_start   │
    │  Idle  ├──────────────────────►│ Ready  │◄─────────────────┤
    └────────┘                       └───┬────┘                  │
                                         │                       │
                                         ▼                       │
                                   ┌───────────┐                 │
                                   │ Listening │                 │
                                   └─────┬─────┘                 │
                                         │ speech_end            │
                                         ▼                       │
                                   ┌───────────┐                 │
                                   │ Thinking  │                 │
                                   └─────┬─────┘                 │
                                         │ playback_started      │
                                         ▼                       │
                                   ┌───────────┐                 │
                                   │ Speaking  ├─────────────────┘
                                   └───────────┘ playback_finished
```

### State Semantics

| State | Canonical Role | `is_engaged` | VAD Audio Capture | Owner window | Description |
|---|---|:---:|:---:|:---:|---|
| **`Idle`** | Cold / Standby | `false` | Dormant (or background dictation) | `main`/`tray` | Session dormant; no conversational turns active. |
| **`Ready`** | Warm / Standby | `true` | Active (Passive) or Standby (PTT) | `main` | Session engaged, engines warm, awaiting speech or PTT hold. |
| **`Listening`** | Active Ingestion | `true` | Streaming | `main`/`tray` | User is actively speaking; mic audio is buffered. |
| **`Thinking`** | Processing | `true` | Gated | `main` | Speech ended; STT / RAG context compaction / LLM reasoning active. |
| **`Speaking`** | System Playback | `true` | Ducked (Speaker) / Active (Headset, PTT) | `main` | Assistant audio playback actively streaming through speakers. |
| **`Paused`** | Explicit Hold | `true` | Discarded | `main` | User paused; audio muted. Only valid in Passive mode. |
| **`Error`** | Failure State | current | Discarded | `main`/`tray` | Recoverable or unrecoverable error; surfaced via `pipeline_error`. |

Ownership is binary: `InteractionOwner::Assistant (1)` vs `Dictation (0)` (`core/state.rs:10`). Transitions go through `services/pipeline/mod::transition` which sets `PipelineAtomics::current_state_atomic` and emits `state_changed` to `target_window(owner)` (`services/pipeline/mod.rs:46-70`).

---

## 3. Audio Capture & VAD Tier

### 3.1 Audio Ingestion (`services/audio/`)
- **Library:** `cpal` (cross-platform audio I/O unified across Linux PipeWire/PulseAudio, macOS CoreAudio, Windows WASAPI).
- **Format:** 16 kHz mono PCM, 32-bit floating point (`f32`), normalized `[-1.0, 1.0]`.
- **Buffer:** Lock-free Single-Producer Single-Consumer (SPSC) ring buffer (64,000 samples / 4.0s depth).
- **Resampling:** Dynamic linear interpolation in callback if the hardware input rate is non-16kHz.

### 3.2 Decoupled VAD Actor (`services/vad/actor.rs`)
- **Frame Size:** 256 samples (16ms @ 16kHz).
- **Backends:**
  - **Earshot (Default):** Native Rust energy/spectral analysis, ~1ms latency, 0MB model overhead.
  - **Ten VAD:** ONNX runtime neural voice activity detection (~15ms frame latency).
- **Decoupled Architecture:** Evaluates frames strictly in its dedicated worker loop without acquiring global locks or querying `app.state()`. Emits `VoxEvent::SpeechStart` and `VoxEvent::SpeechEnd { turn_id, audio_buffer }`.

---

## 4. Domain 1: Modular Passive Pipeline (`services/pipeline/modular_passive.rs`)

Designed for hands-free, continuous ambient voice conversations.

```
[User Speaks] ──► VAD SpeechStart ──► State: Listening ──► Stream to STT
      │
[Speech Finishes] ──► VAD SpeechEnd ──► State: Thinking
      │
[STT Final] ──► Ingest to Working Memory & Context Compaction
      │
[LLM Streaming] ──► Tokens stream into TtsClauseChunker
      │
[First Sentence Ready] ──► Synthesize via TTS ──► State: Speaking
      │
[Playback Finishes] ──► State: Ready (Awaiting next user speech)
```

### Discrete Execution Steps:
1. **Session Start (`start_session`):**
   - Ensures audio engine is running (`services/audio::start_audio_engine`).
   - Sets `pipeline.is_engaged = true`, `pipeline.is_paused = false`.
   - Transitions state to `InteractionState::Ready`.
2. **Speech Onset (`on_speech_start`):**
   - Cancels any existing playback (barge-in).
   - Transitions state to `InteractionState::Listening`.
   - Emits `speech_start` event to frontend.
3. **Speech Offset (`on_speech_end`):**
   - Buffers captured PCM audio and transitions state to `InteractionState::Thinking`.
   - Dispatches `SttCommand::Final(turn_id, buffer)` to STT worker.
4. **Transcription & Memory Ingestion (`on_transcript_final`):**
   - If empty/silence: transitions state back to `InteractionState::Ready`.
   - Pushes user turn into `ConversationManager`.
   - Executes RAG retrieval / context compaction if token limit threshold is exceeded.
   - Dispatches prompt to LLM provider via `spawn_llm_stream`.
5. **Token Generation & Chunking (`on_llm_token`):**
   - Tokens are fed to `TtsClauseChunker`.
   - Complete clauses or sentences trigger `TtsCommand::SynthesizeChunk`.
6. **Playback & Completion (`on_playback_started` / `on_playback_finished`):**
   - When playback starts: state transitions to `InteractionState::Speaking`.
   - When audio finishes: state transitions back to `InteractionState::Ready`.

---

## 5. Domain 2: Modular Push-To-Talk Pipeline (`services/pipeline/modular_ptt.rs`)

Designed for explicit, intentional push-to-talk button or hotkey interactions.

```
[User Holds PTT] ──► handle_ptt_start() ──► State: Listening (Waveform UI)
      │
[User Releases PTT] ──► handle_ptt_stop()
      ├─► No Speech Detected (Silence) ──► Discard Buffer ──► State: Ready
      └─► Speech Detected ──► State: Thinking ──► STT ──► LLM ──► TTS ──► Speaking
```

### Key Invariants & Features:
- **Silence Gating & Ghost Discard:** If the user presses and releases PTT without speaking, the audio buffer is immediately dropped without invoking STT or LLM inference, resetting state to `Ready`.
- **Discrete PTT Verbs:** Handled via non-toggle IPC commands (`ptt_start`, `ptt_stop`, `ptt_cancel`).
- **Waveform-Only Capture:** During `RECORDING`, live partial transcripts are suppressed to eliminate visual jitter until the user finishes talking.

---

## 6. Domain 3: Realtime S2S Passive Pipeline (`services/pipeline/realtime_passive.rs`)

Designed for ultra-low latency direct Speech-to-Speech cloud streaming (Gemini Live WebSocket / Deepgram Voice Agent).

```
[Audio Ingestion] ──► Raw 16kHz PCM streaming to WebSocket
      │
[Server VAD / Speech] ──► on_server_speech_start ──► State: Listening
      │
[Server Audio Stream] ──► 24kHz PCM Chunks ──► Playback ──► State: Speaking
      │
[Server Turn Complete] ──► State: Ready
```

### Key Invariants:
- **Zero Local Model Latency:** STT, LLM reasoning, and TTS voice generation execute concurrently in the cloud.
- **Barge-In Protection:** Speech onset locally interrupts speaker playback immediately (`playback_engine.cancel()`) and informs the server.
- **Session Resumption & Idle Timeout:** Automatically manages WebSocket keep-alives and caches session tokens for transparent reconnection.

---

## 7. Domain 4: Realtime S2S Push-To-Talk Pipeline (`services/pipeline/realtime_ptt.rs`)

Designed for explicit user control over cloud Realtime S2S sessions with **Ghost Audio Hallucination Protection**.

```
[User Holds PTT] ──► Buffer PCM locally in REALTIME_PTT_BUFFER
      │
[Client VAD Evaluates] ──► Sets SPEECH_DETECTED = true if voice active
      │
[User Releases PTT]
      ├─► SPEECH_DETECTED == false ──► Clear Buffer (0 Network Calls) ──► State: Ready
      └─► SPEECH_DETECTED == true  ──► Push Buffer to WebSocket ──► Server Responds
```

### Ghost Audio Protection:
Gemini Live and other conversational WebSocket providers can hallucinate or output background noise when sent empty or ambient audio frames with turn completion signals. `realtime_ptt.rs` buffers audio locally in memory (`REALTIME_PTT_BUFFER`) and evaluates client-side VAD. If no speech was detected during the hold, the buffer is purged without dispatching anything over the WebSocket.

---

## 8. Domain 5: Unified Dictation Pipeline (`services/pipeline/dictation.rs`)

Designed for system-wide speech-to-text dictation across any application on the operating system. Passive and PTT dictation are unified in a single handler (`services/pipeline/dictation.rs`); `InteractionOwner::Dictation` routes events away from Assistant domains via `router::route_event` (`services/pipeline/router.rs:10-31`).

```
[Global Hotkey Press (Alt+Space)] ──► State: Listening (Tray HUD if output_mode=Tray)
      │
[User Releases Hotkey] ──► State: Thinking ──► Embedded STT (Nemotron/Qwen) → transliterate_if_hi
      │
[Final Transcript] ──► output_router::route_transcript (Paste/Clipboard/Tray) ──► State: Idle
```

### 0ms LLM / TTS Fast Path:
Dictation bypasses LLM prompting, context compilation, and TTS synthesis entirely. Final transcripts are sent directly to `services/dictation/output_router.rs` for immediate clipboard restoration and simulated keystrokes. The owner is `InteractionOwner::Dictation`; Assistant has exclusive mic priority — hotkey presses while `owner==Assistant` are suppressed with a debug log (`docs/plans/phase10/pipeline_orchestration_spec.md §5.2`).

---

## 9. Sub-Sentence Streaming TTS Chunking (`TtsClauseChunker`)

To achieve perceived pipeline latencies under 200ms in Modular mode, LLM tokens are streamed dynamically into `TtsClauseChunker` (`services/utils.rs`).

Thresholds are dynamically computed based on observed LLM Tokens Per Second (TPS):

| Condition | Slow TPS (1.0) | Medium TPS (3.5) | Fast TPS (6.0+) |
|---|:---:|:---:|:---:|
| **Sentence Boundary** (`. ! ? ।`) | Always flush | Always flush | Always flush |
| **Clause Boundary** (`, ; : —`) | 3 words | 4 words | Disabled (avoids choppy audio) |
| **Time Gate** | 1.0s / 3 words | 2.2s / 5 words | 3.5s / 8 words |
| **Word Fallback Cap** | 5 words | 12 words | 20 words |

*Note: Chunker enforces `ends_at_word_boundary()` to ensure words are never truncated across synthetic audio frames.*

---

## 10. Central Router & Ownership Invariants

- **Router** (`services/pipeline/router.rs:34-56`): `spawn_router` spawns a `vox-router` OS thread with a blocking `mpsc::Receiver::recv()` loop; `route_event` snapshots `RoutingContext::from_app_state` once per `VoxEvent` and dispatches to the active domain. No `recv_timeout` polling.
- **RoutingContext** (`services/pipeline/mod.rs:14-43`): `{ pipeline_mode, interaction_mode, owner }` derived from `settings.read()` + `owner.load()`. For dictation, `interaction_mode` is mapped from `dictation.interaction_mode` (`Passive`↔`Passive`, `Ptt`↔`PTT`).
- **Canonical lock order**: `state.engine` before `state.realtime_engine` (AGENTS.md §5.2). No silent `let _ = tx.send(..)` — all channel sends log `warn!` on failure.
- **Thin IPC adapters**: `ipc/pipeline/assistant.rs:8-136` and `ipc/pipeline/dictation.rs` are pure 1-line dispatchers that snapshot `RoutingContext` and delegate to the domain handler. Zero business logic in IPC files.

## 11. Tauri IPC Command Matrix

All interaction is dispatched via discrete non-toggle commands in [`ipc/pipeline/assistant.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/pipeline/assistant.rs) and [`ipc/pipeline/dictation.rs`] — thin adapters over `services/pipeline/*`:

| Command | Signature | Description |
|---|---|---|
| `start_session` | `() -> Result<(), String>` | Launches engine if needed, engages session, transitions to `Ready`. |
| `end_session` | `() -> Result<(), String>` | Disengages session, cancels audio, transitions to `Idle`. |
| `pause_session` | `() -> Result<(), String>` | Pauses active session, discards mic audio, transitions to `Paused`. |
| `resume_session` | `() -> Result<(), String>` | Resumes active session, transitions to `Ready`. |
| `ptt_start` | `() -> Result<(), String>` | Starts Push-To-Talk recording, transitions to `Listening`. |
| `ptt_stop` | `() -> Result<(), String>` | Stops Push-To-Talk recording, evaluates speech, transitions to `Thinking`/`Ready`. |
| `ptt_cancel` | `() -> Result<(), String>` | Cancels in-flight Push-To-Talk recording, resets to `Ready`. |
| `launch_engine` | `() -> Result<(), String>` | Starts audio stream for Setup Wizard / diagnostics (`services/audio/engine::start_audio_engine`). |
| `stop_engine` | `() -> Result<(), String>` | Stops hardware audio stream and unloads all ONNX models + `trim_heap` (`services/audio/engine::stop_audio_engine`). |
| `check_engine_status` | `() -> bool` | Returns whether audio engine is currently running (`state.engine.lock().await.is_some()`). |

Frontend mapping lives in `services/pipelineService.ts:63-97` (`startSession`→`start_session`, `pttStart`→`ptt_start`, etc.) and `services/eventsService.ts:164-280` (`onStateChanged`, `onTranscriptPartial`, etc.). Frontend never branches on `pipeline_mode` for lifecycle — same verbs regardless; backend `RoutingContext` resolves the domain. Dictation recovery commands (`get_last_dictation_transcript`, `copy_last_dictation_transcript`, `get_dictation_settings`) live in `ipc/pipeline/dictation.rs`.

---

**Last Updated:** 2026-08-25
