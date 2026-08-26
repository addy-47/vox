---
title: "Vox Backend Architecture"
audience: "Internal — backend (Rust) contributors, system architects, agents"
last_updated: 2026-08-25
owners: "backend-engineer role"
related_docs:
  - "docs/frontend.md — Frontend consumes the IPC event contract (§8)"
  - "docs/models.md — Model inventory & specs"
  - "docs/features/* — Deep dives (memory, dictation, voice-flow, ...)"
  - "AGENTS.md §2, §5 — Workspace map & invariants"
---

# Vox — Backend Architecture

> **A realtime, event-driven native audio processing system** built in Rust with C++ inference backends (ONNX Runtime, llama.cpp). Runs entirely on-device with sub-200ms perceived pipeline latency on 8GB RAM systems. Phase 10 is domain-partitioned: a central non-blocking router dispatches to 5 dedicated handlers instead of a monolithic God loop.

---

## 0. How to read this doc

- **Audience:** backend (Rust) contributors, system architects, and agents needing accurate context on the native runtime.
- **Scope:** the Rust 4-layer architecture, provider/trait system, threading model, lifecycle, and the Tauri IPC event contract.
- **Convention:** claims use `path/file.rs` pointers; schemas are linked, not pasted.
- **Non-goals:** not the frontend (→ `docs/frontend.md`), not model specs (→ `docs/models.md`). The IPC event list in §8 is the contract the frontend consumes.
- **SSOT:** event payloads (§8), settings reload policies (§10), and hardware tiers (§2) are authoritative here. Orchestration topology is SSOT in `docs/plans/phase10/pipeline_orchestration_spec.md`.

## 1. Architecture Stack

The backend follows a strict 4-layer design. Each layer has a single responsibility and communicates via lock-free channels or atomic flags.

```
┌─────────────────────────────────────────────────────────────────────┐
│  1. CORE — Shared state, settings, events, constants, error types   │
│     (core/)                                                         │
├─────────────────────────────────────────────────────────────────────┤
│  2. SERVICES — Domain-specific inference + orchestration            │
│     Audio → VAD → STT → LLM → TTS → Playback                       │
│     + Pipeline Router (5 domains) + Realtime S2S + Memory (async)  │
├─────────────────────────────────────────────────────────────────────┤
│  3. INFRASTRUCTURE — Persistence, monitoring, IPC command handlers  │
│     (persistence/, monitoring/, ipc/)                                │
├─────────────────────────────────────────────────────────────────────┤
│  4. SETUP & BOOT — Model management, onboarding, update checking    │
│     (setup/, wizard/, tray.rs, window_main.rs, utils/)              │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Hardware Tiers & Feature Mapping

Architecture capabilities are gated by hardware tier. Vox must dynamically degrade or upgrade based on what the user's system supports. **Tier 2 is the recommended baseline.**

| Tier | Hardware | Pipeline Mode | Memory Ingestion | Memory Retrieval | Tool Calling |
| :--- | :------- | :-----------: | :--------------: | :--------------: | :----------: |
| **1A** | 8GB, CPU-only, no GPU | Modular (Local) | ❌ None (FIFO only) | ✅ Working Memory context window only | ❌ Unavailable |
| **1B** ⭐ | 8GB+, dedicated GPU | Modular (Local) | ✅ Full async ingestion | ✅ Full retrieval (episodic + semantic) | ⚠️ Depends on local LLM capability |
| **2A** ⭐ | Hybrid (Remote LLM + Local Audio) | Modular (Remote LLM) | ✅ Full async ingestion | ✅ Full retrieval | ⚠️ Depends on remote LLM capability |
| **2B** ⭐ default | Hybrid (Cloud LLM + Local Audio) | Modular (Cloud LLM) | ✅ Full async ingestion | ✅ Full retrieval | ✅ All cloud models support tool calling |
| **3** | Any (Realtime S2S) | Realtime (WebSocket) | ✅ Provider-managed | ✅ Via early tool calls in provider | ✅ Via early tool calls |

### Tier Implications

- **Tier 1A** — Strictly a conversation buffer. No persistent memory ingestion runs (no background worker). Working Memory uses a simple FIFO to manage the context window. This is the floor — every system supports at minimum this.
- **Tier 1B+** — Full memory subsystem enabled. The background memory worker (`persistence/memory_worker.rs`) runs async ingestion (embedding, NLI, edge classification) during idle cycles. Retrieval uses the full two-tier budgeted system against the Turso database. Gated in `lib.rs:213-228` on `pipeline_processing_enabled && has_gpu`.
- **Tier 2B** — Recommended default. Cloud LLM handles all reasoning and tool calling; local models handle audio capture, VAD, STT, and TTS. Memory ingestion runs locally during idle.
- **Tier 3** — Cloud provider owns the full voice loop. Memory is provider-managed via tool calls during the S2S session. Local memory subsystem supplements with episodic/semantic context injected as system context.

> **Design principle**: All features degrade gracefully. Every tier supports real-time voice interaction — higher tiers add persistent memory and tool capabilities. The pipeline never hard-fails due to missing memory models.

---

## 3. Module Layout

```
src/
├── lib.rs                  # Tauri app assembly, engine lifecycle, tray, window events
├── main.rs                 # Binary entry (1 line: calls vox_lib::run())
├── core/                   # Shared infrastructure
│   ├── constants.rs        # Model paths, system prompts, timing, memory taxonomy
│   ├── defaults.rs         # Centralized default values for all 13 settings domains
│   ├── error.rs            # Unified VoxError + domain-specific errors
│   ├── events.rs           # VoxEvent enum (14 variants)
│   ├── metrics.rs          # PipelineMetrics, MetricField
│   ├── settings.rs         # VoxSettings (13 domains: appearance, audio, vad, stt, llm, tts, realtime, interaction, dictation, history, memory, persona, system)
│   └── state.rs            # AppState, VoxEngine, PipelineAtomics, InteractionState, InteractionOwner (Dictation=0, Assistant=1)
├── services/
│   ├── audio/              # engine (start/stop, spawn_router), device (cpal), playback (Cubic Hermite 2× upsample), router (VAD/Realtime routing), decode
│   ├── vad/                # VadEngine trait + VadBackend enum dispatch (Earshot / TenVAD) + actor
│   ├── stt/                # SttEngine trait + EmbeddedSttProvider (Nemotron-3.5 / Qwen3-ASR) + actor
│   ├── llm/                # LlmProvider trait (Embedded / OpenAiCompat / Ollama / LMStudio), actor, capability_probe, policy
│   ├── tts/                # TtsProvider trait (EdgeTTS / Supertonic3 / Chatterbox / ChatterboxRemote), actor, voice
│   ├── realtime/           # RealtimeVoiceProvider + RealtimeSession traits, engine, audio_bridge, playback_bridge (Gemini Live, Deepgram)
│   ├── dictation/          # controller, clipboard (with_clipboard_safe), input (per-OS enigo adapters), output_router, hotkey
│   ├── memory/             # classifiers/ (intra/inter edge, query), deduplication, embedder, formatter, ingestion, retrieval, scope_router, tokenizer, working_memory, pipeline/{runner,stage1-4}
│   ├── pipeline/           # router (central VoxEvent pump), modular_passive, modular_ptt, realtime_passive, realtime_ptt, dictation, mod (RoutingContext, transition, target_window)
│   ├── translit.rs         # Devanagari→Roman ONNX encoder-decoder (evictable singleton)
│   └── utils.rs            # should_flush, count_words, is_devanagari, transliterate_if_hi, stitch_transcripts
├── ipc/                    # Tauri command handlers
│   ├── pipeline/           # assistant (start/end/pause/resume/ptt_* + engine), dictation (settings, recovery, clipboard copy), test_clip
│   ├── settings/           # catalog, health (probe, validate_token_cap, hardware), mutation (update_setting, dispatch_worker_command)
│   ├── memory/             # graph, conflicts, ingestion, mutations
│   ├── audio.rs, history.rs, memory_profiler.rs, monitoring.rs, setup.rs, tray.rs, voices.rs
├── persistence/            # VoxDb (Turso/libSQL), schema, queries, mutations, voices, worker, memory_worker, events
├── monitoring/             # aggregator (crossbeam bounded 4096), collector, system_monitor (/proc/stat), snapshot, runtime_state, telemetry_emitter
├── setup/                  # manifest (AppManifest/VoxManifest fetch+cache), model_manager, runtime_check, update_check
├── utils/                  # paths singleton, logging (tracing + file rotation), audio_filters, bench_reporter, hardware (detect_local_gpu)
├── tray.rs                 # Tray menu, overlay window management, Linux virtual layer
├── wizard.rs               # Setup wizard window config + model health checks
├── window_main.rs          # ensure_main_window (lazy recreate after crash)
└── window_customizer.rs    # PinchZoomDisablePlugin
```

---

## 3. Streaming Pipeline

```
audio(cpal 16kHz f32 SPSC ring 4s) → VAD actor (256-sample frames) → VoxEvent::SpeechStart/SpeechEnd
        │
        ▼
  Central Router (services/pipeline/router.rs — spawn_router, route_event)
        │  RoutingContext { owner, pipeline_mode, interaction_mode } derived once per event
        ├── Assistant / Modular / Passive → STT actor → WorkingMemory + RAG → LLM actor → TTS clause chunker → Playback (24kHz→48kHz 2× Hermite)
        ├── Assistant / Modular / PTT     → PTT gated buffer → STT → LLM → TTS → Playback
        ├── Assistant / Realtime / Passive→ Realtime S2S WebSocket (Gemini Live / Deepgram)
        ├── Assistant / Realtime / PTT    → Gated Realtime buffer (ghost-audio suppressed) → WS
        └── Dictation (Passive+PTT unified) → STT → output_router (Paste/Clipboard/Tray) — 0 LLM/TTS
```

### Pipeline State Machine (7 Canonical Turn States)

| State | Definition | `is_engaged` | Audio Ingestion | Description |
|-------|------------|:---:|:---:|---|
| `Idle` | Dormant / unengaged (`is_engaged = false`) | `false` | Standby (or background dictation) | No conversational turns active. |
| `Ready` | Warm / awaiting speech or PTT hold | `true` | Active (VAD) or PTT standby | Session engaged, engines warm. |
| `Listening` | User is actively speaking; Vox is capturing voice | `true` | Streaming | Mic audio buffered/streamed. |
| `Thinking` | Turn complete; LLM inference or RAG compaction active | `true` | Gated | STT→RAG→LLM reasoning active. |
| `Speaking` | System audio playback actively streaming through speakers | `true` | Ducked (Speaker) or Active (Headset/PTT) | Playback engine draining. |
| `Paused` | User explicitly paused session | `true` | Discarded | Audio muted, pipeline halted. |
| `Error` | Recoverable or unrecoverable subsystem error | current | Discarded | Surfaced via `pipeline_error`. |

States are defined in `core/state.rs:InteractionState` and mirrored in `services/eventsService.ts:InteractionState`. Ownership is binary: `InteractionOwner::Dictation (0)` vs `Assistant (1)` (`core/state.rs:10-28`).

### Sub-Sentence Chunking (should_flush)

Tokens flush to TTS via a fully dynamic algorithm in `services/utils.rs` — all thresholds are continuous functions of observed TPS (tokens/sec), not hardcoded categories:

| Condition | Slow TPS (1) | Medium (3.5) | Fast (6) |
|-----------|:---:|:---:|:---:|
| Sentence boundary (`.!?।`) | Always | Always | Always |
| Clause boundary (`,;—`) | 3 words | 4 words | Disabled |
| Time gate | 1.0s / 3 words | 2.2s / 5 words | 3.5s / 8 words |
| Word-count fallback | 5 words | 12 words | 20 words |

No mid-word splits. Word-boundary safety enforced via `ends_at_word_boundary()`.

---

## 4. Provider Architecture

Every AI domain uses a **trait-based provider system** — the pipeline dispatches through `Box<dyn Provider>` without knowing the concrete backend.

### 4.1 VAD — Voice Activity Detection

| Backend | Type | Latency | Threshold | Notes |
|---------|------|--------:|-----------|-------|
| **Earshot** (default dispatch, TenVad still default setting) | Rust-native, embedded weights | ~1ms | 0.5 (hot-reloadable via `VadCommand::UpdateThreshold`) | ~20× faster than TenVAD, zero ONNX overhead |
| **TenVAD** (legacy) | ONNX via sherpa-onnx | ~15ms | 0.5 | Requires `ten_vad.onnx` model file |

Runtime selection via `VadBackendOption` in `core/settings.rs:VadSettings`; actor is `services/vad/actor.rs` (high-priority OS thread, emits `VoxEvent::SpeechStart/SpeechEnd`).

### 4.2 STT — Speech-to-Text

| Engine | Type | Memory | RTF | Strategy |
|--------|------|-------:|:---:|----------|
| **Nemotron-3.5** (primary, `nvidia_nemotron`) | ONNX INT8, parakeet-rs | ~2.5 GB | 0.02–0.35× | 8960-sample windows, stateful FastConformer-RNNT |
| **Qwen3-ASR-0.6B** (legacy) | ONNX INT8, sherpa-onnx | ~800 MB | 0.38–4.63× | Rolling overlap window |
| **Cloud** (`stt.cloud` — Google Chirp3 default) | HTTP | 0 MB local | network | `SttProviderConfig::Cloud` via `services/stt/providers/mod.rs` |

Throttling: partials capped at 1 per 800ms (`STT_THROTTLE_MS` in `core/constants.rs`). Provider construction at `services/stt/providers/embedded.rs:ensure_loaded` (lazy).

### 4.3 LLM — Language Model

| Provider | Backend | Routing | Memory |
|----------|---------|---------|:------:|
| **EmbeddedProvider** | `llama.cpp` (GGUF) via `llama-cpp-4` | Local CPU inference | ~750 MB–1.4 GB |
| **OpenAiCompatProvider** | `reqwest` (streaming) | `provider_name` → URL mapping (openai, gemini, anthropic, nvidia, groq, openrouter, together) | 0 MB (local) |
| **Ollama / LMStudio** | `reqwest` | Local server `http://localhost:11434` | 0 MB (local) |

Selection is `LlmActiveProvider::{Embedded, Server, Cloud}` (`core/settings.rs:398-405`). Cloud routing is automatic: `provider_name = "openai"` → `api.openai.com`, `"gemini"` → `generativelanguage.googleapis.com/v1beta/openai`, `"nvidia"` → `integrate.api.nvidia.com/v1`, etc. Default local model is `qwen_3_5_0_8b` (`core/defaults.rs:30`); defaults for server/cloud are `gemma3:4b` / `meta/llama-3.1-8b-instruct`.

#### Capability Probing & Settings Engine (`services/llm/capability_probe.rs`)

- **Multi-Phase Streaming Probe (`probe_capabilities`)**:
  - **Phase 1: Streaming Latency & Script**: Dispatches streaming `/v1/chat/completions` request measuring true Time-to-First-Token (`ttft_ms`) on initial byte arrival, pure inter-token generation throughput (`tps` = tokens / duration), and multi-lingual script output for Unicode Devanagari (`U+0900..U+097F`) and Latin scripts.
  - **Phase 2: Structured Tool Calling**: Sends `lookup_user(user_id: integer)` JSON schema with `tool_choice: "auto"` to verify structured `tool_calls` object generation without guessing.
  - **Phase 3: Hardware & Context Attribution**: Automatically tags cloud providers as Cloud GPU clusters. Local servers probe `/api/show` and `/api/ps` for Ollama VRAM allocations and exact context lengths.
  - **Zero-Guessing Context Policy**: When exact context metadata is unexposed, context length returns `None` (`Provider Managed`) rather than artificially clamping to a guessed number.
  - **URL Normalization (`resolve_chat_url`)**: Seamlessly normalizes base URLs ending in `/v1`, `/chat/completions`, or root hostnames, preventing double-path 404 errors.
- **Runtime Token Smoke Validator (`validate_token_cap`)**: By default, remote API requests omit `max_tokens` (`null`) for full uncapped model capacity. Custom token inputs are validated on-demand via a 1-token smoke probe.
- **Hardware-Aware CPU Profiles**: Local GGUF allocates CPU cores dynamically: `Auto` (`max(2, cores - 2)` headroom), `Power Saver` (`max(1, cores / 2)`), `Maximum`.
- **Persistent Capability Cache (`~/.vox/cache/model_capabilities.json`)**: Probed TTFT/TPS written directly to an isolated cache file; loaded via `get_cached_capabilities` without dirtying `settings.json`.
- **Flat 13-Domain Configuration**: Replaces nested models key with 13 domains without erasing settings during mode toggles.
- **UI Decoupling**: `LlmCatalogView.tsx` with `fzf` fuzzy search (`shared/lib/fuzzy.ts`), inline benchmarking, and `LlmSettingsView.tsx` flat underline sub-tabs.

### 4.4 TTS — Text-to-Speech

| Provider | Type | Params | Memory | Output | Feature |
|----------|------|-------:|:------:|--------|---------|
| **Edge TTS** (default) | Pure Rust WebSocket (`tokio-tungstenite`) | Remote | **0 MB** | 24kHz f32 | Free Microsoft Bing ReadAloud, 3.3× RTF, sub-200ms latency |
| **Supertonic 3** (local) | ONNX INT8, sherpa-onnx | 99M | ~144 MB | 24kHz f32 | 31 languages, 10 local voices |
| **Chatterbox** (local clone) | GGML, chatterbox-rs | 340M Q4 | ~1.1 GB | 24kHz native | Voice cloning from 5s reference |
| **Chatterbox Remote** | reqwest blocking HTTP | 340M | 0 MB (local) | 24kHz | Offloads to remote CUDA GPU |

Selection is `TtsActiveProvider::{EdgeTts,Supertonic,Chatterbox,ChatterboxRemote}` (`core/settings.rs:541-549`). Quality steps and speed are `WorkerCommand` hot-reloadable.

### 4.5 Realtime S2S — Speech-to-Speech

| Provider | Input SR | Output SR | Status |
|----------|:--------:|:---------:|:------:|
| **Gemini Live** (default) | 16 kHz | 24 kHz | ✅ Implemented (`gemini_live.rs:945`) |
| **Deepgram Voice Agent** | 16 kHz | configurable | ✅ Implemented (`deepgram_live.rs:698`) |
| **OpenAI Realtime** | 24 kHz | 24 kHz | ⏳ Config defined |
| **ElevenLabs ConvAI** | 16 kHz | 44.1 kHz | ⏳ Config defined |

All realtime providers follow `RealtimeVoiceProvider` + `RealtimeSession` traits with hybrid sync/async threading (tokio for WebSocket I/O, OS threads for audio). Bridged via `services/realtime/{engine,audio_bridge,playback_bridge}.rs`.

---

## 5. Memory Subsystem

> **Status: Active Development** — See `docs/features/memory-architecture.md` for the complete architecture.

Vox implements a **cognitive memory subsystem** that operates asynchronously via a background worker (`persistence/memory_worker.rs`), decoupled from the live voice pipeline. The architecture is organized into 6 collections across 2 structural classes: special-state (`Identity`, `Directives`, `Narrative`) and semantic-graph (`Profile`, `Entities`, `Constraints`) (`core/constants.rs:223-237`).

A pre-retrieval **MemoryScope classifier** (ModernBERT INT8 ONNX, 4-class) routes each user query to the appropriate memory collection before embedding generation and vector search. This prunes irrelevant collections early, saving ~30ms of embedding inference and ~10–50ms of vector DB search per chit-chat turn.

The ingestion pipeline runs as a 4-stage async queue: **Dedup(128) → Embed(16) → Eval(16, concurrent NLI+Edge) → Commit(32)** (`services/memory/pipeline/{stage1..stage4}.rs`). All 3 pipeline ONNX models (Embedder, NLI Engine, Edge Classifier) use an **evictable singleton pattern** (`parking_lot::RwLock<Option<T>>`). They are lazy-loaded only when `personal_memory_queue` has pending items during 30s idle sweeps, and evicted immediately on voice engagement (`PipelineActive`), disengage, or batch completion.

Key files: `services/memory/` (11 modules), `persistence/memory_worker.rs`, `persistence/mutations.rs`, `persistence/queries.rs`. See [`docs/features/memory-architecture.md`](features/memory-architecture.md) for the current v7 architecture reference.

---

## 6. Persistence Layer

- **Database**: Turso/libSQL (`turso` crate), WAL mode, `busy_timeout = 5000ms`
- **Tables**: `sessions`, `turns`, `voice_library`, `memory_facts`, `memory_facts_vectors`, `memory_relations`, `personal_memory_queue`, `memory_pipeline_metrics`, `voice_*`
- **Workers**: Dedicated OS thread for session persistence (`persistence/worker.rs`), dedicated OS thread for background memory ingestion (`persistence/memory_worker.rs`)
- **Events**: `SessionStarted`, `SessionEnded`, `TurnCompleted`, `TurnCancelled`, `Shutdown`
- **Private mode**: Atomic `is_private_mode` check before each write (`AppState::is_private_mode`)
- **Channels**: `crossbeam_channel::bounded` for persistence events with drop counters (`dropped_persistence_events`)

---

## 7. Threading Model

### Allocation

```
Total cores = N
LLM threads  = N - 2 (capped at DEFAULT_LLM_THREADS=4 effective for embedded; cloud offloads)
Remaining: audio (Tier 1, Max priority), VAD (Tier 2, high priority)
```

### Thread Inventory

| Thread | Priority | Type |
|--------|----------|------|
| Audio capture (cpal callback) | `ThreadPriority::Max` | OS callback |
| AudioRouter | `ThreadPriority::Max` | OS thread |
| VAD actor | Crossplatform(80) | OS thread |
| STT worker | Crossplatform(80) | OS thread |
| LLM worker | Default | OS thread |
| TTS worker | Default | OS thread |
| Playback engine | Default | OS thread |
| Central Router (`vox-router`) | Default | OS thread |
| Persistence worker | Default | OS thread |
| Memory worker | Default | OS thread |
| Realtime WS send/recv | — | tokio tasks |
| IPC handlers | — | tokio tasks |

### Why OS Threads for Inference

`llama.cpp` and `onnxruntime` C++ calls are synchronous and block for seconds. All inference runs on **dedicated OS threads** — never on tokio workers. The exception is the Realtime S2S engine, which uses tokio for non-blocking WebSocket I/O. The central router is a blocking `mpsc::Receiver::recv()` loop on its own OS thread (`services/pipeline/router.rs:34-56`).

---

## 8. Event System

### Internal VoxEvent (mpsc channel, `core/events.rs:2-49`)

```
VAD:        SpeechStart { turn_id }, SpeechEnd { turn_id, audio_buffer }
STT:        TranscriptPartial { turn_id, text }, TranscriptFinal { turn_id, text }
LLM:        LlmToken { turn_id, token }, LlmFinished { turn_id }
TTS:        TtsChunk { turn_id, samples }, TtsFinished { turn_id, rtf }
Playback:   PlaybackStarted { turn_id }, PlaybackFinished { turn_id }
Control:    WarmUp, Shutdown, SettingsUpdated(Box<VoxSettings>)
Flow:       Cancelled { turn_id }, Error { turn_id, message }
```

### Tauri IPC Events (frontend-bound, via `app.emit_to`)

| Event | Payload | Source | Description |
|-------|---------|--------|-------------|
| `state_changed` | `InteractionState` | `services/pipeline/mod::transition` | Pipeline state (7 variants) to `target_window(owner)` |
| `transcript_partial` | `TranscriptPayload { turn_id, text, owner }` | STT actor | Streaming partial transcript |
| `transcript_final` | `TranscriptPayload` | STT actor | Final transcript |
| `llm_token` | `string` | LLM actor | Streaming LLM token |
| `ptt_status` | `PttStatusPayload { state: IDLE\|RECORDING\|PROCESSING }` | PTT domains | PTT button state |
| `audio_energy` | `{ energy: f32 }` | VAD actor | Mic level for Orb waveform |
| `telemetry` | `TelemetryData` | aggregator | Full telemetry tick |
| `pipeline_paused` / `pipeline_resumed` | — | `modular_passive` | Audio halt/resume (Passive only) |
| `realtime_session_started/ended/resumed` | — + reason | `realtime_*` | S2S session lifecycle |
| `realtime_idle_warning` | `{ seconds_remaining }` | Realtime | Timeout countdown |
| `realtime_interrupted` | — | Realtime | Barge-in confirmed |
| `pipeline_error` | `String` | any domain | Error message |
| `speech_start` / `speech_end` | `SpeechEventPayload` | VAD | Low-level VAD boundaries |
| `mode_changed_main` / `mode_changed_tray` / `mode_changed` | `String` | `ipc/tray.rs` | Cross-surface mode sync |
| `pipeline_mode_changed` | `String` | settings mutation | Pipeline mode sync |
| `cpu_governor_warning` | `{ governor, optimal }` | `lib.rs:321` | Linux governor advisory |
| `model_setup_status` / `model_setup_complete` | `ModelSetupStatusPayload` | `setup/model_manager` | Wizard + download progress |
| `settings-updated` / `theme-changed` | — | `ipc/settings/mutation` | Settings hot-reload |

Full consumer map is in `docs/frontend.md:§9` and typed wrappers in `services/eventsService.ts:164-280`.

---

## 9. Concurrency Patterns

| Mechanism | Usage | Location |
|-----------|-------|----------|
| SPSC lock-free ring buffer | Audio transport (64k samples / 4s) | `services/audio/device.rs`, `core/constants.rs:RING_BUFFER_SIZE` |
| `Arc<AtomicBool>` / `AtomicU32` / `AtomicU64` | Cancellation, playback, engagement, turn_id, state, sleep, health flags | `PipelineAtomics`, `AppState` |
| `Arc<parking_lot::Mutex<InteractionState>>` | Turn state (sync, no poisoning) | `PipelineAtomics::state` |
| `Arc<AtomicU32>` (state as u32) + `is_assistant_speaking` | Lock-free state read on audio hot path | `PipelineAtomics::current_state_atomic` |
| `std::sync::mpsc::channel` | Inter-thread VoxEvent + STT/LLM/TTS commands | All workers, `VoxEngine::{stt,llm,tts,pipeline}_tx` |
| `crossbeam_channel::bounded(4096)` | High-throughput telemetry + persistence events | `monitoring/aggregator.rs`, `persistence/worker.rs` |
| `parking_lot::RwLock<VoxSettings>` | Read-heavy settings | `AppState::settings` |
| `parking_lot::RwLock<Option<T>>` | Evictable ONNX model singletons | `translit.rs`, `query_classifier.rs`, `embedder.rs`, `intra_edge_classifier.rs`, `inter_edge_classifier.rs` |
| `tokio::sync::Mutex<Option<VoxEngine>>` + `Mutex<Option<RealtimeEngine>>` | Engine lifecycle (async IPC) | `AppState::{engine,realtime_engine}` |
| `parking_lot::Mutex<Option<CheckMenuItem>>` | Tray menu handle | `AppState::hud_menu_item` |

> **Note:** All sync mutexes use `parking_lot` (not `std::sync::Mutex`) for lower overhead, no poisoning, and better performance under contention. The switch was made in v0.8.6 to eliminate lock-poisoning risks in the audio pipeline. Canonical lock order is `state.engine` before `state.realtime_engine` (AGENTS.md §5.2).

**Rule**: Zero locks on audio hot path — settings are snapshotted into `RoutingContext::from_app_state` once per event, VAD updates arrive via channel.

---

## 10. Settings & Hot-Reloading

```
VoxSettings → 13 domains (appearance, audio, vad, stt, llm, tts, realtime, interaction, dictation, history, memory, persona, system)
```

### Reload Policies (`core/settings.rs:171-190`)

| Policy | Effect | Examples |
|--------|--------|---------|
| `Hot` | Apply immediately (no restart) | UI theme, private mode, prompts, `vad.threshold`, `stt.transliterate_enabled`, `llm.temperature`, `interaction.auto_sleep_timeout`, `system.telemetry_enabled` |
| `WorkerCommand` | Send via channel | `tts.quality_steps`, `tts.speed` |
| `Restart` | Full pipeline restart (`stop_engine` → `start_engine`) | Model changes, provider switches, engine config, `vad.backend`, `tts.active`, `stt.active` |

Every agent domain has a `ProviderConfig` tagged enum (`LlmProviderConfig`, `TtsProviderConfig`, `SttProviderConfig`, `RealtimeProviderKind`) for provider selection at worker construction time. Dispatch is via `ipc/settings/mutation.rs:dispatch_worker_command`.

---

## 11. Lifecycle Management

```
Cold ──(dictation Passive)──→ Warm (VAD resident) ──(first speech turn)──→ Full (LLM+TTS warm)
Cold ──(dictation PTT)──────→ Cold (0 RAM) ──(first hotkey press)──────→ Warm
Cold ──(engage main window)─→ Warm (VAD+STT lazy) ──(first turn)──────→ Full
Full ──(auto_sleep_timeout 400s)──→ Cool (LLM+TTS evicted, VAD+STT stay) ──(new activity)──→ Full
Any ──(disengage + dictation off)──→ Cold (stop_engine, unload_all_onnx_models + trim_heap)
Any ──(realtime mode)──────────→ WS (no local LLM/STT/TTS weights)
```

- **Cold state**: 0 ONNX models loaded on boot (~50 MB base RAM) when dictation disabled. `is_dictation_enabled` false → 0 webviews beyond main, 0 engines until `engage()` (`lib.rs:360-395`).
- **PTT dictation warm**: `Alt+Space` first press lazily calls `ensure_engine_running` + `start_audio_engine`; VAD resident, STT lazy; stays warm for subsequent presses.
- **Passive dictation warm**: `start_audio_engine` at boot (`lib.rs:381-385`), VAD resident immediately, STT warms on first speech.
- **Main-window engaged warm**: `start_session` per domain (`modular_passive::start_session`, etc.) sets `owner=Assistant`, `is_engaged=true`, warms LLM+TTS via `spawn_llm_worker`/`spawn_tts_worker`, transitions to `Ready`.
- **Memory Pipeline Eviction**: Pipeline ONNX models (Embedder, NLI, Edge Classifier) lazy-load during 30s idle sweeps **only if pending queue items exist** (`memory_worker.rs`), and evict back to 0 MB RAM on voice engagement, disengage, or batch completion.
- **Auto-sleep**: Driven by `interaction.auto_sleep_timeout` (default 400s in `core/defaults.rs:51`). Router sets `is_sleeping` and runs tiered offload via `cool_down_llm`/`cool_down_tts`. VAD and STT stay resident.
- **Realtime S2S**: Audio capture + VAD routing without loading STT or LLM/TTS weights (0 MB local models); bridged via `services/realtime/{audio_bridge,playback_bridge}`.
- **Shutdown**: `lib.rs:529-560` on `RunEvent::Exit` — sends `VoxEvent::Shutdown` + `SttCommand::Shutdown` + `VadCommand::Shutdown` + memory/persistence `Shutdown`, then joins 150ms.

---

## 12. Monitoring & Telemetry

- **TelemetryAggregator**: Dedicated OS thread, `crossbeam_channel::bounded(4096)`, collects audio energy, VAD probability, system health; fan-out to atomics in `AppStateTelemetryHandles`.
- **SystemMonitor**: Spawns every 5s (`SYSTEM_STATS_INTERVAL`), reads `/proc/stat` + `/proc/meminfo`, filters Linux thread sub-tasks to prevent RSS double-counting (`monitoring/system_monitor.rs`).
- **MonitoringCollector** + **TelemetryEmitter**: `monitoring/collector.rs:spawn_monitoring_collector` + `telemetry_emitter.rs:spawn_telemetry_emitter` — periodic snapshot + `telemetry` event emit.
- **RuntimeSnapshot**: Exposed via IPC `get_runtime_snapshot/history` for frontend monitoring dashboard (`ipc/monitoring.rs:8-22`).

---

**Last Updated:** 2026-08-25
