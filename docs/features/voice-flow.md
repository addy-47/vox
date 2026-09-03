---
title: "Vox Voice Pipeline Flow"
audience: "Internal — backend (Rust) contributors, system architects, agents"
last_updated: 2026-09-03
owners: "backend-engineer role"
related_docs:
  - "docs/backend.md §3, §7, §9 — Module layout, threading model, concurrency primitives"
  - "docs/features/memory-architecture.md — Memory subsystem internals"
  - "docs/frontend.md §9 — Frontend event consumers"
---

# Vox — End-to-End Voice Pipeline Flow & Architecture (Technical Reference)

> **Purpose:** Implementation-accurate reference for the Vox voice runtime. After reading this a backend contributor should understand: how a `VoxEvent` travels from the cpal capture thread to the speaker, where every **handler**, **actor**, **engine**, **thread**, **mutex/Arc/atomic**, **channel**, and **streaming / blocking** boundary lives, and how the memory subsystem is wired into the live turn. All claims cite `path/file.rs:line` against the current tree (handler event-driven architecture: `pipeline/handlers/*` + `pipeline/dictation.rs` + `pipeline/router.rs`; no `modular/` or `realtime/` domain directories).

---

## 1. High-Level Architecture

Vox uses a **handler event-driven pipeline**. Raw audio and all internal events flow through a lock-free `std::sync::mpsc` channel to a single **Central Event Router** (`vox-router` OS thread, `pipeline/router.rs:72`), which calls `route_event` — snapshotting a `RoutingContext` once per event and dispatching to a flat set of **handler functions** under `pipeline/handlers/` or `pipeline/dictation.rs`. There is no domain-partitioned directory tree and no monolithic loop.

```
                         ┌──────────────────────────────────────────────┐
                         │  Audio Capture Tier (cpal 16kHz f32)          │
                         │  services/audio/device.rs → SPSC ring (64k/4s)│
                         └───────────────────────┬──────────────────────┘
                                                  │ 256-sample frames
                                                  ▼
                         ┌──────────────────────────────────────────────┐
                         │  VAD Actor  (vox-vad-actor, OS thread)        │
                         │  services/vad/actor.rs — blocks on Earshot   │
                         │  emits VoxEvent::SpeechStart / SpeechEnd      │
                         │  PTT window: Start/StopWindowValidation cmds  │
                         └───────────────────────┬──────────────────────┘
                                                  │ VoxEvent (mpsc)
                                                  ▼
                         ┌──────────────────────────────────────────────┐
                         │  Central Event Router (vox-router, OS thread) │
                         │  router.rs: spawn_router → route_event        │
                         │  RoutingContext { owner, pipeline_mode,       │
                         │    interaction_mode } snapshotted per event   │
                         └───────────────────────┬──────────────────────┘
                    ┌─────────────────────────────┼─────────────────────────────┐
                    ▼                             ▼                             ▼
           ┌─────────────────┐          ┌──────────────────┐          ┌──────────────────┐
           │ Dictation fast- │          │ Assistant handlers│          │ Session / PTT /  │
           │ path (owner==   │          │ speech.rs         │          │ Error handlers   │
           │ Dictation)      │          │ transcript.rs     │          │ session.rs       │
           │ dictation.rs    │          │ llm.rs            │          │ ptt.rs           │
           │ 0 LLM/TTS hops  │          │ playback.rs       │          │ error.rs         │
           └─────────────────┘          │ accumulator.rs    │          │ interrupt.rs     │
                                        └──────────────────┘          └──────────────────┘
```

Each handler is a **pure function** invoked synchronously on the router thread. Handlers send commands to inference **actors** over `std::sync::mpsc` and receive streaming results back as `VoxEvent`s re-entering the router.

---

## 2. The Canonical 7-State Turn Machine

Both Rust (`core/state.rs:39 InteractionState`) and TS (`services/eventsService.ts`) align on 7 states. Ownership is binary: `InteractionOwner::Assistant (1)` vs `Dictation (0)` (`core/state.rs:10`). Dictation has its own 4-state mirror `DictationState` (`core/state.rs:69 Idle/Recording/Transcribing/Error`) surfaced to the tray window.

| State | `state != Idle` | Audio Ingestion | Owner window | Meaning |
|---|:---:|---|---|---|
| `Idle` | `false` | Dormant (bg dictation) | main/tray | No conversational turn active. |
| `Ready` | `true` | Active (Passive) / Standby (PTT) | main | Engaged, engines warm, awaiting speech/PTT. |
| `Listening` | `true` | Streaming | main | User speaking; mic buffered. |
| `Thinking` | `true` | Gated | main | STT → harness context → LLM dispatch. |
| `Speaking` | `true` | Ducked (Speaker) / Active (Headset, PTT) | main | Playback draining to speakers. |
| `Paused` | `true` | Discarded | main | Explicit pause (Passive only). |
| `Error` | current | Discarded | main/tray | Surfaced via `voice_error`. |

Transitions go through `pipeline/mod::transition` (`pipeline/mod.rs:59`), which calls `state.pipeline.set_state(...)` and emits `state_changed` to `target_window(owner)` (`pipeline/mod.rs:52`: `Dictation → "tray"`, `Assistant → "main"`). State is stored as a lock-free `AtomicU32` mirror (`PipelineAtomics::current_state_atomic` at `core/state.rs:117`) broadcast via `tokio::sync::watch` (`state_tx/state_rx`) — the old `parking_lot::Mutex<InteractionState>` was removed.

Dictation transitions use a parallel path: `pipeline/dictation.rs:emit_dictation_state` maps `DictationState::Recording/Transcribing` to tray `state_changed` strings (`Recording→"Recording"`, `Transcribing→"Thinking"` — the tray reuses the same UI mood).

---

## 3. Actor vs Engine vs Handler vs Thread — the concurrency core

Vox separates **synchronous C/C++ inference** (llama.cpp, ONNX Runtime) from async orchestration by running each inference domain on its **own dedicated OS thread** with a `std::sync::mpsc` command loop. The "engine" is the bundle of shared handles; the "actor" is the thread that owns the model and blocks on it; the "handler" is the stateless function the router calls.

### 3.1 Actors (dedicated OS thread + mpsc command loop)

| Actor | File | Command enum | Thread name | How it blocks |
|---|---|---|---|---|
| VAD | `services/vad/actor.rs` | `VadCommand` (`services/vad/mod.rs:29`) | `vox-vad-actor` (Max prio) | Consumes ring buffer; runs Earshot/TenVAD synchronously. Also handles `StartWindowValidation` / `StopWindowValidation` for PTT. |
| STT | `services/stt/actor.rs` | `SttCommand {ResetStream, Final(turn_id,audio), Shutdown}` | `vox-stt-worker` | `provider.transcribe_chunk` blocks on ONNX. Emits `TranscriptFinal` via `pipeline_tx`. |
| LLM | `services/llm/actor.rs` | `LlmCommand {Generate{request,turn_id,cancel,accumulator,tts_tx}, Shutdown}` | `vox-llm-persistent` | Builds a **tokio current-thread runtime** and `runtime.block_on(provider.generate(...))` — llama.cpp / HTTP blocks on this OS thread. Streams tokens via accumulator → TTS. |
| TTS | `services/tts/actor.rs` | `TtsCommand {Generate{turn_id,text}, …}` | `vox-tts-persistent` | `provider.synthesize_chunk` blocks on ONNX. |

> **Realtime S2S is the exception:** it runs on the Tokio runtime, not dedicated OS threads. `RealtimeActor` (`services/realtime/mod.rs`) owns a `tokio::sync::mpsc` audio sender; VAD forwards PCM via `VadCommand::StartRealtime/StopRealtime`.

### 3.2 Engines (shared handle bundles)

- **`VoxEngine`** (`core/state.rs:95`) holds the cpal `AudioStream`, cloned `mpsc::Sender`s (`stt_tx`, `vad_tx`, `pipeline_tx`), `Option<mpsc::Sender<LlmCommand>>` (`llm_tx`), `Option<mpsc::Sender<TtsCommand>>` (`tts_tx`), `telemetry_tx` (crossbeam), `Arc<PlaybackEngine>`, and the worker `JoinHandle`s.
- **`AppState.engine`** = `tokio::sync::Mutex<Option<VoxEngine>>` (`core/state.rs:268`). `AppState.event_tx` (`core/state.rs:299`) caches the `pipeline_tx` for idle-monitor and IPC use.
- **`RealtimeActor`** is stored as `Mutex<Option<RealtimeActor>>` (`core/state.rs:269`).

### 3.3 Handler layer (router-thread functions, `pipeline/handlers/`)

| Handler file | VoxEvents handled | Key responsibility |
|---|---|---|
| `speech.rs` | `SpeechStart`, `SpeechEnd` | Passive-only gate; barge-in via `interrupt::on_interrupt`; `Transcription` → `Listening→Thinking` |
| `transcript.rs` | `TranscriptFinal` | Validate non-empty; `transliterate_if_hi`; emit `transcript_final` IPC; `spawn_modular_llm_task` (harness → LLM dispatch) or set `pending_synthesis_jobs=1` for Realtime |
| `llm.rs` | `LlmFinished` | Flush `TurnAccumulator` remainder to TTS; `flush_pre_roll`; persist turn via `PersistenceEvent::TurnCompleted` |
| `playback.rs` | `PlaybackStarted`, `PlaybackFinished` | `Listening`/`Speaking`/`Ready` transitions; opportunistic harness compaction trigger |
| `ptt.rs` | `PttStart`, `PttStop`, `PttCancel` | PTT-only gate; `StartWindowValidation` / `StopWindowValidation` round-trip to VAD; dispatch to STT or `RealtimeActor::signal_speech_committed` |
| `session.rs` | `SessionStart`, `PauseSession`, `ResumeSession`, `EndSession` | `ensure_modular_workers_sync` or `RealtimeActor::start`; VAD `SetOperationalMode`; persistence + memory lifecycle; `spawn_idle_monitor` (7m auto-pause / 5m offload) |
| `error.rs` | `Error`, `Cancelled` | Map to `InteractionState::Error` or back to `Ready`/`Idle` |
| `interrupt.rs` | (helper) | `cancel_flag + turn_token.cancel() + playback.cancel() + next_turn()` |
| `accumulator.rs` | (state) | `TurnAccumulator { chunker: TtsClauseChunker, assistant_response, user_transcript }` — owned by `AppState::pipeline_accumulator` |

A thin second layer — **IPC adapters** (`ipc/pipeline/mod.rs`) — snapshots `RoutingContext` and sends the corresponding `VoxEvent` into `event_tx`. They contain zero business logic.

---

## 4. Audio Capture & VAD Tier

- **Capture** (`services/audio/device.rs`): cpal callback writes 16 kHz mono `f32` PCM into a **SPSC lock-free ring buffer** (64 000 samples / 4 s, `RING_BUFFER_SIZE` in `core/constants.rs:4`). Zero locks on the capture hot path.
- **VAD actor** (`services/vad/actor.rs`): consumes the ring buffer in 256-sample (16 ms) frames on `vox-vad-actor`. Runs Earshot (Rust-native, ~1 ms) or TenVAD (ONNX) synchronously. In `ContinuousSegmentation` mode it emits `VoxEvent::SpeechStart/SpeechEnd`; in `WindowedValidation` mode it buffers audio between `StartWindowValidation` and `StopWindowValidation` and returns a `VadValidationResult { is_speech_detected, audio }`.
- **Realtime passthrough** (`VadOperationalMode::StreamPassthrough`): VAD forwards 16 kHz PCM chunks directly to the `RealtimeActor` audio sender without segmentation.

---

## 5. Concurrency Primitives (exact locations)

| Mechanism | Type | Location | Use |
|---|---|---|---|
| Pipeline turn state | `Arc<AtomicU32>` + `watch::Sender<InteractionState>` | `core/state.rs:117-119` | `current_state_atomic` read lock-free on audio hot path; `state_tx` fans out to idle monitor & `subscribe_state()` |
| Dictation state | `Arc<AtomicU32>` + `watch::Sender<DictationState>` | `core/state.rs:120-122` | `dictation_state_atomic`; `subscribe_dictation_state()` |
| Turn / cancel | `Arc<AtomicBool>` (cancel_flag) + `CancellationToken` + `AtomicU64` epoch | `core/state.rs:112,123-125` | `cancel_flag`, `turn_token` (parking_lot::Mutex), `turn_epoch`; `next_turn()` atomically advances both |
| Turn id | `Arc<AtomicU32>` | `core/state.rs:113` | `turn_id`; `next_turn_id()` / `peek_turn_id()` / `next_turn() -> (id, token)` |
| Turn accumulator | `Arc<parking_lot::Mutex<TurnAccumulator>>` | `core/state.rs:301` | `pipeline_accumulator`; `clear()` / `push_token()` / `flush_chunker()` |
| Settings | `Arc<RwLock<VoxSettings>>` | `core/state.rs:273` (std::sync::RwLock) | Read-heavy; `RoutingContext::from_app_state` does one `read()` per event |
| Engine / Realtime | `tokio::sync::Mutex<Option<…>>` | `core/state.rs:268-269` | Async IPC lifecycle |
| Conversation manager | `Arc<parking_lot::Mutex<ConversationManager>>` | `core/state.rs:297` | `new_session` / `build_context` |
| ONNX singletons (evictable) | `parking_lot::RwLock<Option<T>>` | `translit.rs:232`, harness/memory | Lazy-load on use, `*write()=None` to evict |
| Actor command channels | `std::sync::mpsc::Sender<T>` | VAD/STT/LLM/TTS actors | Dispatch to blocking inference threads |
| Router / events | `std::sync::mpsc::Receiver<VoxEvent>` | `pipeline/router.rs:72` | `vox-router` `recv()` pump; `event_tx` cached in `AppState` |
| Telemetry / persistence | `crossbeam_channel::bounded(4096)` | `core/state.rs:175`, `persistence/worker.rs` | High-throughput fan-out |
| Realtime audio bridge | `tokio::sync::mpsc` | `services/realtime/` | WS ↔ cpal on tokio |

**Lock order invariant:** acquire `state.engine` strictly **before** `state.realtime_engine` (enforced at `pipeline/handlers/session.rs`). All sync mutexes use `parking_lot` where possible (no poisoning). **Rule:** no lock on the audio capture hot path; settings are snapshotted once into `RoutingContext` per event. Router thread priority is `ThreadPriority::Max` (`pipeline/router.rs:80`).

---

## 6. End-to-End Traces

### 6.1 Modular Passive (canonical turn, `PipelineMode::Modular` + `InteractionMode::Passive`)

```
cpal capture (device.rs)
   → ring buffer (SPSC, 0 locks)
   → VAD actor (vox-vad-actor): SpeechStart ──► vox-router ──► speech::on_speech_start
        guards: PTT → ignore; Idle/Paused → drop; Thinking/Speaking → interrupt::on_interrupt
        else Ready → next_turn() + accumulator.clear() + playback.cancel() → Listening
        STT ResetStream if Modular
   → VAD actor: SpeechEnd ──► vox-router ──► speech::on_speech_end → Thinking
   → STT actor (vox-stt-worker): transcribes buffered PCM (BLOCKS on ONNX)
        └─► VoxEvent::TranscriptFinal ──► vox-router ──► transcript::on_transcript_final
             guards: Idle/Paused/!Thinking → drop
             transliterate_if_hi + empty→Ready+toast / else
             accumulator.set_user_transcript + emit transcript_final IPC to target_window(owner)
             if Modular: spawn_modular_llm_task (tokio::spawn)
                 • prepare_turn_context (harness::prepare_turn_context)
                   - scope classification → embedding → Turso retrieval (if enabled)
                   - ConversationManager::build_context → <user_profile> injected
                   - enqueue personal_memory facts to memory_tx
                 • LlmCommand::Generate{request, turn_id, cancel, accumulator, tts_tx, pending_jobs} ──► llm_tx
             if Realtime: pending_synthesis_jobs=1 (no LLM dispatch; server owns generation)
   → LLM actor (vox-llm-persistent): block_on generate → streams tokens
        accumulator.push_token(token) → TtsClauseChunker extracts clauses → TtsCommand::Generate per clause ──► tts_tx
        on completion: VoxEvent::LlmFinished ──► vox-router ──► llm::on_llm_finished
             flush_modular_tts_remainder → pending_jobs++ → TtsCommand::Generate
             playback.flush_pre_roll(); persist_assistant_turn via PersistenceEvent::TurnCompleted
   → TTS actor (vox-tts-persistent): synthesize_chunk (BLOCKS on ONNX)
        └─► PlaybackEngine ingests PCM ring buffer
             VoxEvent::PlaybackStarted ──► playback::on_playback_started (Thinking→Speaking)
             VoxEvent::PlaybackFinished ──► playback::on_playback_finished (Speaking→Ready, opportunistic compaction)
```

### 6.2 Modular PTT (`InteractionMode::PTT`)

```
ipc/pipeline PttStart ──► VoxEvent::PttStart ──► ptt::on_ptt_start
     guards: Passive→drop; Idle/Paused→drop; Listening→drop (already held)
     Thinking/Speaking → interrupt (cancel + next_turn) else Ready → next_turn → Listening
     VAD StartWindowValidation (buffer from now)

ipc/pipeline PttStop ──► VoxEvent::PttStop ──► ptt::on_ptt_stop
     guards: Passive/!Listening → drop
     VAD StopWindowValidation round-trip (500ms timeout) → VadValidationResult
     !is_speech or empty → Ready (ghost-audio rejection, 0 network calls)
     else Thinking → dispatch_ptt_speech_audio:
         Modular: SttCommand::Final(turn_id, audio) ──► STT actor → same TranscriptFinal path as §6.1
         Realtime: encode f32→i16 + RealtimeActor::signal_speech_committed

PttCancel ──► ptt::on_ptt_cancel: clear accumulator; cancel_flag + turn_token.cancel(); playback.cancel();
              VAD StopWindowValidation (drain) → Ready
```

### 6.3 Realtime S2S (either Passive or PTT, `PipelineMode::Realtime`)

Cloud owns STT+LLM+TTS. `session::start_realtime_session` (`pipeline/handlers/session.rs:39`) creates a `RealtimeActor` (`services/realtime::RealtimeActor::new`), calls `rt_actor.start(mode, playback_engine, pipeline_tx, app)` (bridges audio + playback), and sends `VadCommand::StartRealtime { tx: audio_tx, is_ptt }`. On `PttStart/Stops` the PTT handler above routes to `signal_speech_committed` instead of STT. Barge-in in `interrupt::on_interrupt` cancels local playback and rotates the turn token before the server sees the new turn.

### 6.4 Dictation fast path (`InteractionOwner::Dictation`, 0 LLM/TTS hops)

```
Router sees ctx.owner==Dictation → dictation::handle_event (pipeline/dictation.rs:261)

Passive dictation:
  VAD ContinuousSegmentation → SpeechStart → dictation::on_speech_start → DictationState::Recording (tray: "Recording")
                         → SpeechEnd   → dictation::on_speech_end   → DictationState::Transcribing (tray: "Thinking")
  STT actor (same Nemotron/Qwen) → TranscriptFinal → dictation::on_transcript_final
     transliterate_if_hi → route_transcript(output_mode) spawned task:
         Paste      → clipboard::with_clipboard_safe + input::simulate_paste (Ctrl+V / Cmd+V)
         Clipboard  → clipboard::set_text
         Tray       → Ok(()) (HUD is the surface)
     → DictationState::Idle + emit transcript_final to WINDOW_TRAY

PTT dictation (global hotkey Alt+Space):
  services/dictation/hotkey::HotkeyAction::Press  → handle_hotkey_press  → Recording
  services/dictation/hotkey::HotkeyAction::Release→ handle_hotkey_release_with_sender
     VAD StopWindowValidation round-trip → !is_speech → Idle (discard)
     else Transcribing → SttCommand::Final → same on_transcript_final path
```

While `owner==Assistant`, the global hotkey channel keeps receiving but `Press` is a no-op (dictation yield — assistant has exclusive mic priority). See `docs/features/dictation.md`.

---

## 7. Sub-Sentence Streaming TTS Chunking

Clause chunking is **`TtsClauseChunker`** inside `services/tts/actor.rs` wrapped by `TurnAccumulator` (`pipeline/handlers/accumulator.rs:6`). LLM tokens stream in; `push_token` accumulates `assistant_response` and calls `chunker.push_str(token)` → `find_split_point` / `extract_chunks` decide clause boundaries from a dynamic TPS function (slow 1.0 → fast 6.0+):

| Condition | Slow (1.0) | Medium (3.5) | Fast (6.0+) |
|---|:---:|:---:|:---:|
| Sentence boundary (`.!?।`) | Flush | Flush | Flush |
| Clause boundary (`,;—`) | 3 words | 4 words | Disabled |
| Time gate | 1.0s/3w | 2.2s/5w | 3.5s/8w |
| Word fallback | 5 words | 12 words | 20 words |

`ends_at_word_boundary()` guarantees no mid-word split. Each completed chunk → `TtsCommand::Generate`. On `LlmFinished`, any remainder is flushed as one final `Generate` (`llm.rs:47 flush_modular_tts_remainder`) with `pending_synthesis_jobs` incremented before send and decremented on failure.

---

## 8. Memory Subsystem in the Live Turn

Per Assistant turn, inside `prepare_turn_context` (`services/harness/mod.rs`, invoked by `transcript::spawn_modular_llm_task:62`):

1. `scope` classification + embedding + `retrieve_personal_context` (Turso hybrid: SQL directives/narrative + vector search + 2-hop BFS graph expansion, capped at `memory.max_context_share` of `llm.context_window`).
2. `ConversationManager::push_user_turn` + `build_context` injects `<user_profile>` into the system prompt alongside the compacted history summary.
3. Extracted `personal_memory` facts are **enqueued async** to `memory_tx` (`MemoryWorkerEvent`) for the background pipeline.

**Background ingestion (decoupled, non-blocking to the turn):** the `vox-memory-worker` OS thread (`persistence/memory_worker.rs`) runs the 4-stage pipeline Dedup(128) → Embed(16) → Eval(16, concurrent NLI+Edge) → Commit(32). ONNX embed/NLI/edge models are evictable singletons, lazy-loaded only when the idle queue has pending items, evicted on voice engagement / disengage / batch completion.

The synchronous `prepare_turn_context` retrieval runs inside a `tauri::async_runtime::spawn` task (`transcript.rs:50`), so it occupies a tokio worker briefly but never blocks the `vox-router` or VAD threads. Full v7 schema: `docs/features/memory-architecture.md`.

---

## 9. Central Router & Ownership Invariants

- **`spawn_router`** (`pipeline/router.rs:72`): OS thread `vox-router`; blocking `recv()` loop, elevated to `ThreadPriority::Max`, exits on `VoxEvent::Shutdown`. No `recv_timeout` polling.
- **`route_event`** (`pipeline/router.rs:10`): `RoutingContext::from_app_state` (one `settings.read()` + `owner` atomic load) → `Dictation` → `dictation::handle_event`; `Assistant` → flat `match event` dispatching to `handlers::speech / transcript / llm / playback / ptt / session / error`.
- **Thin IPC adapters:** `ipc/pipeline/mod.rs` snapshots context and sends `VoxEvent::{SessionStart,PttStart,PttStop,PttCancel,EndSession,…}` via `AppState::event_tx`. Commands are `start_session`, `end_session`, `pause_session`, `resume_session`, `ptt_start`, `ptt_stop`, `ptt_cancel`, `launch_engine`, `stop_engine`, `check_engine_status`, `test_clip*`.
- **No silent sends:** every `tx.send(..)` logs `warn!` on failure (no `let _ =`).

---

**Last Updated:** 2026-09-03 — rewritten from domain-partitioned (`modular/`/`realtime/`/`dictation.rs`) to handler event-driven (`pipeline/handlers/*` + `pipeline/dictation.rs` + `pipeline/router.rs`) architecture.
