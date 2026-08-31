---
title: "Vox Voice Pipeline Flow"
audience: "Internal — backend (Rust) contributors, system architects, agents"
last_updated: 2026-08-31
owners: "backend-engineer role"
related_docs:
  - "docs/backend.md §3, §7, §9 — Module layout, threading model, concurrency primitives"
  - "docs/plans/phase10/pipeline_orchestration_spec.md — SSOT for routing & invariants"
  - "docs/features/memory-architecture.md — Memory subsystem internals"
  - "docs/frontend.md §9 — Frontend event consumers"
---

# Vox — End-to-End Voice Pipeline Flow & Architecture (Technical Reference)

> **Purpose:** A self-contained, implementation-accurate reference for the Vox voice runtime. After
> reading this a backend contributor should understand: how a `VoxEvent` travels from the cpal
> capture thread to the speaker, where every **actor**, **engine**, **thread**, **mutex/Arc/atomic**,
> **channel**, and **streaming / blocking** boundary lives, and how the memory subsystem is wired
> into the live turn. All claims cite `path/file.rs:line` against the current tree (post Phase-10
> refactor: `modular/`, `realtime/`, `dictation.rs` split out of the old flat `modular_*` files;
> `services/audio/router.rs`, `services/dictation/controller.rs`, `services/utils.rs`,
> `core/metrics.rs`, `services/llm/capabilities.rs`, `services/llm/probe.rs` deleted).

---

## 1. High-Level Architecture

Vox uses a **spec-first, domain-partitioned pipeline**. Raw audio and all internal events flow
through a lock-free `mpsc` channel to a single non-blocking **Central Event Router** (`vox-router`
OS thread, `pipeline/router.rs:33`), which dispatches each `VoxEvent` to one of the
domain handlers under `modular/`, `realtime/`, or `dictation.rs`. There is **no monolithic loop**
and **no `AudioRouter` OS thread** (the old `services/audio/router.rs` was deleted — its PTT
routing role moved into the VAD actor, see §4).

```
                         ┌──────────────────────────────────────────────┐
                         │  Audio Capture Tier (cpal 16kHz f32)          │
                         │  device.rs → SPSC ring buffer (64k/4s)        │
                         └───────────────────────┬──────────────────────┘
                                                  │ 256-sample frames
                                                  ▼
                         ┌──────────────────────────────────────────────┐
                         │  VAD Actor  (vox-vad-worker, OS thread)       │
                         │  services/vad/actor.rs — blocks on ONNX       │
                         │  emits VoxEvent::SpeechStart / SpeechEnd       │
                         │  PTT audio: dispatches directly to domain      │
                         │    ingest_audio() (no router round-trip)       │
                         └───────────────────────┬──────────────────────┘
                                                  │ VoxEvent (mpsc)
                                                  ▼
                         ┌──────────────────────────────────────────────┐
                         │  Central Event Router (vox-router, OS thread)  │
                         │  router.rs: spawn_router → route_event        │
                         │  snapshots RoutingContext once per event      │
                         └───────────────────────┬──────────────────────┘
                  ┌──────────────────────────────┼──────────────────────────────┐
                  ▼                              ▼                              ▼
         ┌─────────────────┐           ┌──────────────────┐           ┌──────────────────┐
         │ Modular Domains │           │ Realtime Domains │           │ Dictation Domain │
         │ modular/        │           │ realtime/        │           │ dictation.rs     │
         │  passive.rs     │           │  passive.rs      │           │ (0ms LLM/TTS)    │
         │  ptt.rs         │           │  ptt.rs          │           │ Paste/Clipboard/ │
         │  context.rs     │           │  session.rs      │           │ Tray output      │
         └─────────────────┘           └──────────────────┘           └──────────────────┘
```

Each domain handler is itself a **state machine** driven by the router. The handlers send commands
to inference **actors** over `std::sync::mpsc` and receive streaming results back as `VoxEvent`s.

---

## 2. The Canonical 7-State Turn Machine

Both Rust (`core/state.rs:InteractionState`) and TS (`services/eventsService.ts`) align on 7 states.
Ownership is binary: `InteractionOwner::Assistant (1)` vs `Dictation (0)` (`core/state.rs:10`).

| State | `state != Idle` | Audio Ingestion | Owner window | Meaning |
|---|:---:|---|---|---|
| `Idle` | `false` (`state == Idle`) | Dormant (bg dictation) | main/tray | No conversational turn active. |
| `Ready` | `true` (`state == Ready`) | Active (Passive) / Standby (PTT) | main | Engaged, engines warm, awaiting speech/PTT. |
| `Listening` | `true` | Streaming | main/tray | User speaking; mic buffered. |
| `Thinking` | `true` | Gated | main | Speech ended; STT → dynamic memory retrieval → LLM. |
| `Speaking` | `true` | Ducked (Speaker) / Active (Headset, PTT) | main | Playback draining to speakers. |
| `Paused` | `true` | Discarded | main | Explicit pause (Passive only). |
| `Error` | current | Discarded | main/tray | Surfaced via `voice_error`. |

Transitions go through `pipeline/mod::transition` (`mod.rs:79`), which calls
`state.pipeline.set_state(...)` and emits `state_changed` to `target_window(owner)` (`mod.rs:71`:
`Dictation → "tray"`, `Assistant → "main"`). State is stored as both a `parking_lot::Mutex<InteractionState>`
and a lock-free `AtomicU32` mirror for the audio hot path (see §5).

---

## 3. Actor vs Engine vs Thread — the concurrency core

Vox separates **synchronous C/C++ inference** (llama.cpp, ONNX Runtime) from async orchestration by
running each inference domain on its **own dedicated OS thread** with a `std::sync::mpsc` command
loop. The "engine" is the bundle of shared handles; the "actor" is the thread that owns the model
and blocks on it.

### 3.1 Actors (dedicated OS thread + mpsc command loop)

| Actor | File | Command enum | Thread name | How it blocks |
|---|---|---|---|---|
| VAD | `services/vad/actor.rs` | `VadCommand` (`services/vad/mod.rs:VadCommand`) | `vox-vad-worker` (`actor.rs:248`, Max prio) | Consumes ring buffer; runs Earshot/TenVAD ONNX synchronously. |
| STT | `services/stt/actor.rs` | `SttCommand {Partial,Final,ResetStream,Shutdown}` (`:12`) | `vox-stt-worker` (`:309`) | `provider.transcribe_chunk` blocks on ONNX. |
| LLM | `services/llm/actor.rs` | `LlmCommand {Generate{turn_id,cancel_flag}, Shutdown}` (`:13`) | `vox-llm-persistent` (`:149`) | Builds a **tokio current-thread runtime** and `runtime.block_on(provider.generate(...))` (`:38–51`) — llama.cpp / HTTP blocks on this OS thread. |
| TTS | `services/tts/actor.rs` | `TtsCommand {Generate{turn_id,text}, …}` (`:14`) | `vox-tts-persistent` (`:197`) | `provider.synthesize_chunk` blocks on ONNX. |

> **Realtime S2S is the exception:** it runs on **tokio tasks**, not dedicated OS threads
> (`gemini_live.rs:376`, `deepgram_live.rs:313`). The cloud owns STT/LLM/TTS; Vox only bridges audio.

### 3.2 Engines (shared handle bundles)

- **`VoxEngine`** (`core/state.rs:70`) holds the cpal `audio_stream`, cloned `mpsc::Sender`s
  (`stt_tx`, `vad_tx`, `pipeline_tx`), `Option<mpsc::Sender<LlmCommand>>` (`llm_tx`, `:74`),
  `Option<mpsc::Sender<TtsCommand>>` (`tts_tx`, `:75`), `telemetry_tx` (crossbeam),
  `Arc<PlaybackEngine>`, and the worker `JoinHandle`s.
- **`AppState.engine`** = `tokio::sync::Mutex<Option<VoxEngine>>` (`state.rs:165`). Accessed via
  `state.engine.lock().await`; command senders are cloned out of the locked guard (e.g.
  `modular/passive.rs:242`).
- **`RealtimeEngine`** (`services/realtime/engine.rs:12`) = `Box<dyn RealtimeVoiceProvider>` +
  `Option<Arc<dyn RealtimeSession>>` + audio/playback bridges. **`AppState.realtime_engine`** =
  `tokio::sync::Mutex<Option<RealtimeEngine>>` (`state.rs:166`).

### 3.3 Modular worker spawning

`ensure_modular_workers` (`pipeline/modular/mod.rs:14`) locks `state.engine`, then calls
`warm_up_llm` (`llm/actor.rs:113`) and `warm_up_tts` (`tts/actor.rs:174`), storing the cloned
`llm_tx`/`tts_tx` back into the engine. STT + VAD actors are spawned earlier inside
`start_audio_engine` (`core/engine.rs:209,:247`), so they exist before the LLM/TTS workers warm.

---

## 4. Audio Capture & VAD Tier (and why there is no AudioRouter)

- **Capture** (`services/audio/device.rs`): cpal callback writes 16 kHz mono `f32` PCM into a
  **SPSC lock-free ring buffer** (64 000 samples / 4 s, `RING_BUFFER_SIZE`). This is the only
  allocation on the capture hot path — **zero locks**.
- **VAD actor** (`services/vad/actor.rs`) consumes the ring buffer in 256-sample (16 ms) frames on
  `vox-vad-worker`. It runs Earshot (Rust-native, ~1 ms) or TenVAD (ONNX) synchronously and emits
  `VoxEvent::SpeechStart{SpeechEnd{turn_id,audio_buffer}}`.
- **PTT audio routing (replaces old AudioRouter):** for push-to-talk, the VAD actor does **not**
  emit an event round-trip — it calls the domain `ingest_audio` directly (`vad/actor.rs:506–544`):
  `pipeline::dictation::ingest_audio`, `realtime::ptt::ingest_audio`, or `modular::ptt::ingest_audio`
  depending on `owner`. Passive audio instead flows as `VoxEvent`s through `vox-router`.

---

## 5. Concurrency Primitives (exact locations)

| Mechanism | Type | Location | Use |
|---|---|---|---|
| Pipeline turn state | `Arc<parking_lot::Mutex<InteractionState>>` + `Arc<AtomicU32>` mirror | `core/state.rs:93,98` | `state` for transitions; `current_state_atomic` read lock-free on audio hot path |
| Engagement / cancel / flags | `Arc<AtomicBool>` ×8 | `core/state.rs:87–99` | `cancel_flag`, `is_paused`, `playback_active`, `llm_generating`, `tts_generating`, `is_assistant_speaking`, `engine_shutdown` (+ `state` atomics for engagement) |
| Turn id | `Arc<AtomicU32>` | `state.rs:92` | monotonic turn counter |
| Transcript history | `Arc<parking_lot::Mutex<VecDeque<String>>>` | `state.rs:95` | recent turns |
| Settings | `Arc<RwLock<VoxSettings>>` | `state.rs:170` | read-heavy; `RoutingContext::from_app_state` does one `read()` per event (`mod.rs:49`) |
| Engine / Realtime engine | `tokio::sync::Mutex<Option<…>>` | `state.rs:165–166` | async IPC lifecycle |
| Conversation manager | `Arc<parking_lot::Mutex<ConversationManager>>` | `state.rs:227` | sync; `build_context` etc. |
| ONNX singletons (evictable) | `parking_lot::RwLock<Option<T>>` | `translit.rs:232`, `embedder.rs:19`, `query_classifier.rs:37`, `inter_edge_classifier.rs:18`, `intra_edge_classifier.rs:52` | lazy-load on use, `*write() = None` to evict + free RAM |
| Actor command channels | `std::sync::mpsc::Sender<T>` | VAD/STT/LLM/TTS actors | dispatch to blocking inference threads |
| Router / events | `std::sync::mpsc::Receiver<VoxEvent>` | `router.rs:33` | `vox-router` `recv()` pump |
| Telemetry / persistence | `crossbeam_channel::bounded(4096)` | `state.rs:175`, `persistence/worker.rs` | high-throughput fan-out |
| Realtime audio bridge | `tokio::sync::mpsc` | `realtime/audio_bridge.rs` | WS ↔ cpal on tokio |

**Lock order invariant (AGENTS.md §5.2):** acquire `state.engine` strictly **before**
`state.realtime_engine`. Enforced at `realtime/passive.rs:24→37` and `realtime/ptt.rs:48→61`.
`realtime/session.rs:37` uses `try_lock()` for barge-in to avoid inversion. All sync mutexes are
`parking_lot` (no poisoning). **Rule:** no lock on the audio capture hot path; settings are
snapshotted once into `RoutingContext` per event.

---

## 6. Domain 1 — Modular Passive (end-to-end thread trace)

`pipeline/modular/passive.rs`. This is the canonical Assistant turn; every step names the
thread that owns it.

```
cpal capture (device.rs thread)
   → ring buffer (SPSC, 0 locks)
   → VAD actor (vox-vad-worker): SpeechStart ──► vox-router ──► on_speech_start → State: Listening
   → VAD actor: SpeechEnd{turn_id,audio_buffer}
        └─► STT actor (vox-stt-worker): SttCommand::Final → transcribe (BLOCKS on ONNX)
             └─► VoxEvent::TranscriptFinal ──► vox-router ──► on_transcript_final
                  ├─► (empty) → State: Ready
                  └─► on_transcript_final spawns tokio task → build_generation_request
                       • transliterate_if_hi (Devanagari→Roman, ONNX singleton)
                       • stitch_transcripts (stt/stitcher.rs)
                       • [MEMORY] classify_scope (ModernBERT) → generate_embedding (MiniLM)
                                  → retrieve_personal_context (Turso)  ← BLOCKS tokio worker
                       • ConversationManager::build_context → <user_profile> injected
                       • LlmCommand::Generate{turn_id, cancel_flag} ──► llm_tx (mpsc)
   → LLM actor (vox-llm-persistent): block_on generate → streams VoxEvent::LlmToken
        └─► vox-router ──► on_llm_token
             ├─► TtsClauseChunker (actor.rs:218) accumulates tokens
             └─► clause ready → TtsCommand::Generate{turn_id,text} ──► tts_tx (mpsc)
   → TTS actor (vox-tts-persistent): synthesize_chunk (BLOCKS on ONNX)
        └─► VoxEvent::TtsChunk ──► vox-router ──► on_tts_chunk → playback.ingest_chunk (ring buffer)
   → cpal playback thread drains ring buffer → State: Speaking → PlaybackFinished → State: Ready
```

**Discrete steps (`modular/passive.rs`):**

1. **`start_session`** — `start_audio_engine`; `ensure_modular_workers` (warm LLM+TTS); set
   `owner=Assistant`, `state → Ready`; → `Ready`.
2. **`on_speech_start`** — cancel any playback (barge-in); → `Listening`; emit `speech_start`.
3. **`on_speech_end`** — buffer PCM; → `Thinking`; `SttCommand::Final` to STT actor.
4. **`on_transcript_final`** — empty→`Ready`; else `transliterate_if_hi` + `stitch_transcripts`
   + **dynamic memory retrieval** via `build_generation_request` (`modular/context.rs:62`) then
   `LlmCommand::Generate`.
5. **`on_llm_token`** — feed `TtsClauseChunker`; per clause → `TtsCommand::Generate`.
6. **`on_tts_chunk` / `on_playback_*`** — `playback.ingest_chunk`; → `Speaking` on start,
   `Ready` on finish; `try_trigger_opportunistic` background compaction (`context.rs:145`).

---

## 7. Domain 2 — Modular PTT (`modular/ptt.rs`)

- `ptt_start` → `Listening` (waveform UI only; partials suppressed); immediate `playback_engine.cancel()`
  + `cancel_flag` set (barge-in).
- `ptt_stop` → if no speech: **discard buffer**, → `Ready` (no STT/LLM). If speech: → `Thinking` →
  same STT → memory retrieval → LLM → TTS → `Speaking` path as §6.
- Non-toggle IPC verbs `ptt_start` / `ptt_stop` / `ptt_cancel` (no toggle state functions).

---

## 8. Domain 3 — Realtime S2S Passive (`realtime/passive.rs`)

Cloud-owns-everything path. `RealtimeEngine::new` (`engine.rs:12`) over a tokio runtime; audio
streamed raw to the WebSocket, server-VAD drives turn boundaries. **Zero local STT/LLM/TTS weights.**
Barge-in: local speech onset cancels `playback_engine` immediately and signals the server
(`realtime/session.rs` manages bidirectional barge-in + keep-alives).

---

## 9. Domain 4 — Realtime S2S PTT (`realtime/ptt.rs`)

- `ptt_start` → `realtime_barge_in` (cancel local playback + send server interrupt) + buffer PCM locally.
- Client VAD sets `SPEECH_DETECTED` during hold.
- `ptt_stop`: if `SPEECH_DETECTED == false` → **purge buffer, 0 network calls** (ghost-audio
  rejection); else push buffer to WS → server responds.

---

## 10. Domain 5 — Unified Dictation (`dictation.rs`, 0 ms LLM/TTS)

`InteractionOwner::Dictation` routes all events away from Assistant domains (`router.rs` →
`dictation::handle_event`, `:229`). **No `controller.rs`** (deleted) — the unified handler is
`pipeline/dictation.rs`; reusable primitives live in `services/dictation/`
(`clipboard.rs`, `input.rs`, `output_router.rs`, `hotkey.rs`).

```
Alt+Space (global hotkey) → ingest_audio / handle_hotkey_press
   → VAD → STT actor (Nemotron/Qwen) → transliterate_if_hi
   → on_transcript_final spawns task → output_router::route_transcript
        ├─ Paste   → input::create_input_adapter().simulate_paste (Ctrl+V / Cmd+V, clipboard-safe)
        ├─ Clipboard→ clipboard::set_text
        └─ Tray    → dispatch_to_tray (floating HUD)
   → State: Idle
```

Bypasses LLM/TTS entirely. Assistant has exclusive mic priority; hotkey while `owner==Assistant`
is suppressed. See `docs/features/dictation.md`.

---

## 11. Sub-Sentence Streaming TTS Chunking

Clause chunking is **`TtsClauseChunker`** inside `services/tts/actor.rs:218` (a
`static CHUNKER: LazyLock<Mutex<TtsClauseChunker>>` instantiated in `modular/passive.rs:17` and
`ptt.rs`) — **not** a separate `tts/chunker.rs`. LLM tokens stream in; `push_str` / `find_split_point`
/ `extract_chunks` decide clause boundaries from a dynamic TPS function (slow 1.0 → fast 6.0+):

| Condition | Slow (1.0) | Medium (3.5) | Fast (6.0+) |
|---|:---:|:---:|:---:|
| Sentence boundary (`.!?।`) | Flush | Flush | Flush |
| Clause boundary (`,;—`) | 3 words | 4 words | Disabled |
| Time gate | 1.0s/3w | 2.2s/5w | 3.5s/8w |
| Word fallback | 5 words | 12 words | 20 words |

`ends_at_word_boundary()` guarantees no mid-word split. Each completed chunk → `TtsCommand::Generate`.

---

## 12. Memory Subsystem in the Live Turn

Per Assistant turn, inside `build_generation_request` (`modular/context.rs:62`):

1. **`classify_scope`** (`memory::classify_scope`, `query_classifier.rs:100`) — ModernBERT INT8 ONNX,
   4-class scope (ChitChat/User/Domain/Temporal). ChitChat → skip retrieval.
2. **`generate_embedding`** (`embedder.rs:197`) — MiniLM-L12 384-dim ONNX vector.
3. **`retrieve_personal_context`** (`retrieval.rs`) — Turso hybrid: SQL directives/narrative +
   vector search + 2-hop BFS graph expansion, capped at `max_personal_memory_share` (0.15) of the
   LLM context window.
4. `ConversationManager::update_dynamic_user_profile` + `push_user_turn` + `build_context`
   (`working_memory.rs:398`) inject `<user_profile>` into the system prompt.
5. Extracted `personal_memory` facts are **enqueued async** to the Turso `personal_memory_queue`
   (`context.rs:112`, spawned task) for the background pipeline.

**Background ingestion (decoupled, non-blocking to the turn):** the `vox-memory-worker` OS thread
(`persistence/memory_worker.rs:43`) runs `run_pipeline_cycle` (`memory/pipeline/runner.rs:12`) —
Dedup(128) → Embed(16) → Eval(16, NLI + edge, `tokio::join!`) → Commit(32). ONNX embed/NLI/edge
models are evictable singletons, lazy-loaded only when the idle queue has pending items, evicted on
voice engagement / disengage / batch completion. **Note:** memory retrieval in step 1–3 is
*synchronous ONNX CPU work* executed inside a `tauri::async_runtime::spawn` task (`passive.rs:251`),
so it occupies a tokio worker briefly but never blocks the `vox-router` or VAD threads. See
`docs/features/memory-architecture.md` for the full v7 schema.

---

## 13. Central Router & Ownership Invariants

- **`spawn_router`** (`router.rs:33`): OS thread `vox-router`; blocking `recv()` loop, exits on
  `VoxEvent::Shutdown`. No `recv_timeout` polling.
- **`route_event`** (`router.rs:10`): `RoutingContext::from_app_state` (one `settings.read()` +
  `owner` atomic load) → `Dictation` → `dictation::handle_event`; `Assistant` → `modular` or
  `realtime` by `pipeline_mode`.
- **Thin IPC adapters:** `ipc/pipeline/assistant.rs` and `ipc/pipeline/dictation.rs` snapshot
  `RoutingContext` and delegate — zero business logic. Commands live in `assistant.rs`
  (no `lifecycle.rs`):
  `start_session`, `end_session`, `pause_session`, `resume_session`, `ptt_start`, `ptt_stop`,
  `ptt_cancel`, `launch_engine`, `stop_engine`, `check_engine_status`.
- **No silent sends:** every `tx.send(..)` logs `warn!` on failure (no `let _ =`).

---

**Last Updated:** 2026-08-31
