# Vox — Backend Architecture

> **A realtime, event-driven native audio processing system** built in Rust with C++ inference backends (ONNX Runtime, llama.cpp). Runs entirely on-device with sub-200ms perceived pipeline latency on 8GB RAM systems.

---

## 1. Architecture Stack

The backend follows a strict 4-layer design. Each layer has a single responsibility and communicates via lock-free channels or atomic flags.

```
┌─────────────────────────────────────────────────────────────────────┐
│  1. CORE — Shared state, settings, events, constants, error types   │
│     (core/)                                                         │
├─────────────────────────────────────────────────────────────────────┤
│  2. SERVICES — Domain-specific inference + orchestration            │
│     (services/)                                                     │
│     Audio → VAD → STT → LLM → TTS → Playback                       │
│     + Realtime S2S (WebSocket) + Memory (async ingestion)          │
├─────────────────────────────────────────────────────────────────────┤
│  3. INFRASTRUCTURE — Persistence, monitoring, IPC command handlers  │
│     (persistence/, monitoring/, ipc/)                                │
├─────────────────────────────────────────────────────────────────────┤
│  4. SETUP & BOOT — Model management, onboarding, update checking    │
│     (setup/, wizard/, tray/, utils/)                                │
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
- **Tier 1B+** — Full memory subsystem enabled. The background memory worker (`persistence/memory_worker.rs`) runs async ingestion (embedding, NLI, edge classification) during idle cycles. Retrieval uses the full two-tier budgeted system against the Turso database.
- **Tier 2B** — Recommended default. Cloud LLM handles all reasoning and tool calling; local models handle audio capture, VAD, STT, and TTS. Memory ingestion runs locally during idle.
- **Tier 3** — Cloud provider owns the full voice loop. Memory is provider-managed via tool calls during the S2S session. Local memory subsystem supplements with episodic/semantic context injected as system context.

> **Design principle**: All features degrade gracefully. Every tier supports real-time voice interaction — higher tiers add persistent memory and tool capabilities. The pipeline never hard-fails due to missing memory models.

---

## 3. Module Layout

```
src/
├── lib.rs                  # Tauri app assembly, engine lifecycle
├── main.rs                 # Binary entry (1 line: calls vox_lib::run())
├── core/                   # Shared infrastructure
│   ├── constants.rs        # Model paths, system prompts, timing, memory taxonomy
│   ├── error.rs            # Unified VoxError + domain-specific errors (Audio, STT, LLM, TTS, Memory, Persistence)
│   ├── events.rs           # VoxEvent enum (14 pipeline signal variants)
│   ├── metrics.rs          # PipelineMetrics, MetricField
│   ├── settings.rs         # VoxSettings (12 sub-settings: UI, Audio, VAD, ASR, LLM, TTS, Interaction, Telemetry, Persistence, Memory, Assistant, Realtime, Setup)
│   └── state.rs            # AppState, VoxEngine, PipelineAtomics, PttState, InteractionState, VadCommand
├── services/
│   ├── audio/              # Capture (cpal ring buffer), Playback (Cubic Hermite upsample, jitter buffer, underrun fade), Router (VAD/Realtime routing), Decode
│   ├── vad/                # VadEngine trait + VadBackend enum dispatch (Earshot / TenVAD)
│   ├── stt/                # SttEngine trait + EmbeddedSttProvider (Nemotron-3.5 / Qwen3-ASR)
│   ├── llm/                # LlmProvider trait (Embedded / OpenAiCompat), LlmEngine, capability probe
│   ├── tts/                # TtsProvider trait (Edge TTS / Supertonic 3 / Chatterbox / ChatterboxRemote)
│   ├── realtime/           # RealtimeVoiceProvider + RealtimeSession traits (Gemini Live, Deepgram Voice Agent)
│   ├── memory/             # 11 modules: query_classifier, deduplication, embedder, formatter, ingestion, nli, edge_classifier, retrieval, scope_router, tokenizer, working_memory
│   ├── pipeline.rs         # Pipeline orchestrator (LLM→TTS→Playback coordination, ~1888 lines)
│   ├── ptt.rs              # Push-to-talk mode (VAD gate, realtime support, speech_detected)
│   ├── translit.rs         # Devanagari→Roman ONNX encoder-decoder
│   └── utils.rs            # should_flush, count_words, is_devanagari, transliterate, stitch
├── ipc/                    # Tauri command handlers (pipeline, settings, tray, history, audio, voices, monitoring, memory, setup)
├── persistence/            # VoxDb (Turso/SQLite), schema migrations, session CRUD, memory_worker, mutations, queries
├── monitoring/             # TelemetryAggregator (crossbeam), system monitor (/proc/stat/meminfo), snapshot, emitter
├── setup/                  # AppManifest, VoxManifest, ModelManager (download, SHA256 verify), runtime check, update check
├── utils/                  # Paths singleton, logging (tracing + file rotation), audio_filters, bench_reporter
├── tray.rs                 # Tray icon, overlay window management
└── wizard.rs               # Setup wizard window config + model health checks
```

---

## 3. Streaming Pipeline

```
audio → VAD → STT → (Transliteration) → LLM → (Tag Stripping) → TTS → Playback → speaker
       ↑                                                                           ↓
   Audio Capture (cpal, 16kHz)                                         Playback Engine (48kHz)
```

### Pipeline State Machine

| State | Description |
|-------|-------------|
| `Idle` | Waiting for speech |
| `Listening` | VAD active, no speech detected |
| `UserSpeaking` | Speech in progress, STT streaming |
| `Thinking` | LLM generating response |
| `AssistantSpeaking` | TTS playback active |
| `Interrupted` | Barge-in triggered |
| `MaintainingContext` | Compaction running, transition speech plays |

### Sub-Sentence Chunking (should_flush)

Tokens flush to TTS via a fully dynamic algorithm in `utils.rs` — all thresholds are continuous functions of observed TPS (tokens/sec), not hardcoded categories:

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
| **Earshot** (default) | Rust-native, embedded weights | ~1ms | 0.5 (config) | ~20× faster than TenVAD, zero ONNX overhead |
| **TenVAD** (legacy) | ONNX via sherpa-onnx | ~15ms | 0.45 (config) | Requires `ten_vad.onnx` model file |

### 4.2 STT — Speech-to-Text

| Engine | Type | Memory | RTF | Strategy |
|--------|------|-------:|:---:|----------|
| **Nemotron-3.5** (primary) | ONNX INT8, parakeet-rs | ~2.5 GB | 0.02–0.35× | 8960-sample windows, stateful FastConformer-RNNT |
| **Qwen3-ASR-0.6B** (legacy) | ONNX INT8, sherpa-onnx | ~800 MB | 0.38–4.63× | Rolling overlap window |

### 4.3 LLM — Language Model

| Provider | Backend | Routing | Memory |
|----------|---------|---------|:------:|
| **EmbeddedProvider** | `llama.cpp` (GGUF) via `llama-cpp-4` crate | Local CPU inference | ~750 MB–1.4 GB |
| **OpenAiCompatProvider** | `reqwest` HTTP (streaming) | `provider_name` → URL mapping (openai, gemini, anthropic) | 0 MB (local) |

Cloud routing is automatic: `provider_name = "openai"` → `api.openai.com`, `"gemini"` → `generativelanguage.googleapis.com/v1beta/openai`, `"anthropic"` → `api.anthropic.com` (with `x-api-key` + `anthropic-version` headers).

### 4.4 TTS — Text-to-Speech

| Provider | Type | Params | Memory | Output | Feature |
|----------|------|-------:|:------:|--------|---------|
| **Edge TTS** (default) | Pure Rust WebSocket (`tokio-tungstenite`) | Remote | **0 MB** | 24kHz f32 | Free Microsoft Bing ReadAloud, 3.3× RTF, sub-200ms latency |
| **Supertonic 3** (local) | ONNX INT8, sherpa-onnx | 99M | ~144 MB | 24kHz f32 | 31 languages, 10 local voices |
| **Chatterbox** (local clone) | GGML, chatterbox-rs | 340M Q4 | ~1.1 GB | 24kHz native | Voice cloning from 5s reference |
| **Chatterbox Remote** | reqwest blocking HTTP | 340M | 0 MB (local) | 24kHz | Offloads to remote CUDA GPU |

### 4.5 Realtime S2S — Speech-to-Speech

| Provider | Input SR | Output SR | Status |
|----------|:--------:|:---------:|:------:|
| **Gemini Live** | 16 kHz | 24 kHz | ✅ Implemented (910 lines) |
| **Deepgram Voice Agent** | 16 kHz | configurable | ✅ Implemented (657 lines) |
| **OpenAI Realtime** | 24 kHz | 24 kHz | ⏳ Config defined |
| **ElevenLabs ConvAI** | 16 kHz | 44.1 kHz | ⏳ Config defined |

All realtime providers follow `RealtimeVoiceProvider` + `RealtimeSession` traits with hybrid sync/async threading (tokio for WebSocket I/O, OS threads for audio).

---

## 5. Memory Subsystem

> **Status: Active Development (WIP)** — See `docs/plans/memory-spec-v7.md` for the complete architecture specification.

Vox implements a **cognitive memory subsystem** that operates asynchronously via a background worker (`persistence/memory_worker.rs`), decoupled from the live voice pipeline. The architecture is organized into 4 cognitive scopes: `ChitChat`, `User`, `Domain` (primary default), `Temporal`.

A pre-retrieval **MemoryScope classifier** (ModernBERT INT8 ONNX, 4-class) routes each user query to the appropriate memory collection before embedding generation and vector search. This prunes irrelevant collections early, saving ~30ms of embedding inference and ~10–50ms of vector DB search per chit-chat turn.

The ingestion pipeline runs as a 4-stage async queue: **Dedup(128) → Embed(16) → Eval(16, concurrent NLI+Edge) → Commit(32)**. Full implementation details and benchmark gate results in `docs/features/memory-architecture.md`.

Key files: `services/memory/` (11 modules), `persistence/memory_worker.rs`, `persistence/mutations.rs`, `persistence/queries.rs`. See [`docs/features/memory-architecture.md`](features/memory-architecture.md) for the current v7 architecture reference.

---

## 6. Persistence Layer

- **Database**: Turso/libSQL (`turso` crate), WAL mode, `busy_timeout = 5000ms`
- **Tables**: `sessions`, `turns`, `voice_library`, `memory_facts`, `memory_facts_vectors`, `memory_relations`, `personal_memory_queue`
- **Workers**: Dedicated OS thread for session persistence (`persistence/worker.rs`), dedicated OS thread for background memory ingestion (`persistence/memory_worker.rs`)
- **Events**: `SessionStarted`, `SessionEnded`, `TurnCompleted`, `TurnCancelled`
- **Private mode**: Atomic `is_private_mode` check before each write

---

## 7. Threading Model

### Allocation

```
Total cores = N
LLM threads  = N - 2
Remaining: audio (Tier 1, Max priority), VAD (Tier 2, high priority)
```

### Thread Priorities

| Thread | Priority | Type |
|--------|----------|------|
| Audio capture (cpal) | `ThreadPriority::Max` | OS thread |
| AudioRouter | `ThreadPriority::Max` | OS thread |
| VAD worker | Crossplatform(80) | OS thread |
| STT worker | Crossplatform(80) | OS thread |
| LLM worker | Default | OS thread |
| TTS worker | Default | OS thread |
| Playback | Default | OS thread |
| Persistence worker | Default | OS thread |
| Memory worker | Default | OS thread |
| Realtime WS send/recv | — | tokio tasks |
| IPC handlers | — | tokio tasks |

### Why OS Threads for Inference

`llama.cpp` and `onnxruntime` C++ calls are synchronous and block for seconds. All inference runs on **dedicated OS threads** — never on tokio workers. The exception is the Realtime S2S engine, which uses tokio for non-blocking WebSocket I/O.

---

## 8. Event System

### Internal VoxEvent (mpsc channels)

```
VAD:        SpeechStart, SpeechEnd
STT:        TranscriptPartial, TranscriptFinal
Pipeline:   WarmUp, Cancelled, Error, Shutdown
LLM:        LlmToken, LlmFinished, Error
TTS:        TtsChunk, TtsFinished
Playback:   PlaybackStarted, PlaybackFinished
```

### Tauri IPC Events (frontend-bound)

| Event | Payload | Description |
|-------|---------|-------------|
| `state_changed` | `InteractionState` | Pipeline state |
| `audio_energy` | `{ energy: f32 }` | Mic level for visualization |
| `ptt_status` | `"IDLE"\|"RECORDING"\|"PROCESSING"` | PTT button state |
| `pipeline_paused/resumed` | — | Audio halt/resume |
| `realtime_session_started/ended` | — | S2S session lifecycle |
| `realtime_interrupted` | — | Barge-in confirmed |
| `realtime_idle_warning` | `{ seconds_remaining }` | Timeout countdown |
| `pipeline_error` | `String` | Error message |

---

## 9. Concurrency Patterns

| Mechanism | Usage | Location |
|-----------|-------|----------|
| SPSC lock-free ring buffer | Audio transport | `services/audio/device.rs` |
| `Arc<AtomicBool>` | Cancellation, playback, engagement flags | `PipelineAtomics` |
| `mpsc::channel` | Inter-thread events (VoxEvent) | All workers |
| `crossbeam_channel::bounded` | High-throughput telemetry | `monitoring/aggregator.rs` |
| `parking_lot::RwLock<VoxSettings>` | Read-heavy settings | `core/state.rs` |
| `parking_lot::Mutex<Option<VoxEngine>>` | Engine lifecycle | `core/state.rs` |
| `tokio::sync::Mutex` | Async IPC state | `ipc/` handlers |

> **Note:** All sync mutexes use `parking_lot` (not `std::sync::Mutex`) for lower overhead, no poisoning, and better performance under contention. The switch was made in v0.8.6 to eliminate lock-poisoning risks in the audio pipeline.

**Rule**: Zero locks on audio hot path — settings are snapshotted, VAD updates arrive via channel.

---

## 10. Settings & Hot-Reloading

```
VoxSettings → 12 sub-settings (Ui, Audio, Vad, Asr, Llm, Tts, Interaction, Telemetry, Persistence, Memory, Assistant, Setup, Realtime)
```

### Reload Policies

| Policy | Effect | Examples |
|--------|--------|---------|
| `Hot` | Apply immediately | UI theme, private mode, prompts |
| `WorkerCommand` | Send via channel | VAD threshold, TTS speed, quality steps |
| `Restart` | Full pipeline restart | Model changes, provider switches, engine config |

Every agent domain has a `ProviderConfig` tagged enum (`LlmProviderConfig`, `TtsProviderConfig`, `SttProviderConfig`) for provider selection at worker construction time.

---

## 11. Lifecycle Management

```
Cold ──(engage)──→ Warm ──(auto-sleep timeout)──→ Cold
  ↑                                                     |
  └───────────────────(re-engage)───────────────────────┘
```

- **Cold state**: All models unloaded, minimal RAM (~50 MB)
- **Warm state**: LLM + TTS loaded, ready for interaction (~2.5 GB peak)
- **Auto-sleep**: Offloads LLM/TTS after inactivity timeout
- **Shutdown**: Signal via channels + atomics → join threads → persistence flush

---

## 12. Monitoring & Telemetry

- **TelemetryAggregator**: Dedicated OS thread, `crossbeam_channel::bounded(4096)`, collects audio energy, VAD probability, system health
- **SystemMonitor**: Spawns every 30s, reads `/proc/stat` + `/proc/meminfo`, filters Linux thread sub-tasks to prevent RSS double-counting
- **RuntimeSnapshot**: Exposed via IPC for frontend monitoring dashboard

---

**Last Updated:** 2026-08-02