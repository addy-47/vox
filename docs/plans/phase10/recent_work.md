# Phase 10 — Recent Work & Architectural Ledger

This document tracks detailed architectural refactorings, milestone completions, sprint histories, and implementation logs for Phase 10. For high-level invariants and active guidelines, refer to [AGENTS.md](file:///home/addy/projects/apps/vox/AGENTS.md).

---

## Chronological Storyline

> UT Layer & Spec $\to$ Backend/Frontend Discovery $\to$ Uncalled Code Purge $\to$ STT Consolidation $\to$ 188-Sprint Review $\to$ Subsystem Decoupling $\to$ Turn ID / PTT Boundaries $\to$ LLM Engine Consolidation $\to$ Memory 4-Pillar Refactor $\to$ Pipeline Seam Hardening $\to$ State-Event Orchestration Purge & Sprints S01–S11.

---

## Complete Phase 10 Refactor & Architectural Ledger

### 1. Initial Test Suite & Architectural Discovery
- Built unit test suite and authored Integration Test Spec (`docs/plans/phase10/integration_test_spec.md`, Seams 1–8).
- Mutation testing revealed widespread dead code, uncalled methods, and tangled audio/LLM routing across backend and frontend.
- Paused Seams 9–14 to execute a foundational codebase overhaul.

### 2. Backend Refactor & Uncalled Functions Resolution
- **Decoupled Pipelines:** Extracted `services/pipeline/modular/` and `realtime/` orchestrated via central `router.rs` (`VoxEvent` pump).
- **Uncalled Code Resolution:** Wired 11 critical paths (`prepare_turn_context`, opportunistic compaction, monotonic turn IDs, transliteration) and purged 30 dead functions across 7 deleted legacy files.
- **Quality Gate:** 45 tests across 9 binaries green via `cargo-nextest --release --test-threads=1`; 0 clippy warnings.

### 3. Frontend Standardization & Dead Code Purge
- **7-State Alignment:** Unified UI across 7 canonical states (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`) with standardized `vad_backend` configuration.
- **Cleanup:** Purged 26 unused listeners/services via `knip`. All 10 suites (98 tests) and `pnpm build` verified green.
- **Ledger:** [`docs/features/performance-memory-optimizations.md`](file:///home/addy/projects/apps/vox/docs/features/performance-memory-optimizations.md) and [`docs/frontend.md`](file:///home/addy/projects/apps/vox/docs/frontend.md).

### 4. STT Streaming Benchmark & Engine Consolidation
- **Harness:** Built 256-sample streaming benchmark CLI (`app/src-tauri/benches/stt_bench.rs`) evaluating 10 canonical audio clips.
- **Sherpa-ONNX 1.13.6:** Standardized all STT/VAD/TTS on `sherpa-onnx 1.13.6` using multilingual Nemotron-3.5 transducer (`0.497x RTF`, `97.1% accuracy`, `~71MB RSS`), completely removing `parakeet-rs`.

### 5. 188-Sprint Second-Pass Review & Architectural Specs
- Authored and completed all 188 implementation sprints across 11 modules (`docs/plans/phase10/backend_review_sprints.md`).
- Established 5 standalone SSOT architecture specs: LLM/TTS Streaming, LLM Provider Consolidation, Monotonic Turn IDs, Memory `<user_profile>` Assembly, and Audio Ownership.

### 6. Subsystem Decoupling & Engine Lifecycle Migration
- **VAD 3-Role Decoupling:** Restricted `VadActor` to generic modes (`ContinuousSegmentation`, `WindowedValidation`, `StreamPassthrough`) with zero upward pipeline imports; extracted telemetry and math utilities.
- **Engine Relocation:** Moved application lifecycle orchestration (`VoxEngine`, startup/shutdown) to `core/engine.rs`, scoping `services/audio/` strictly to CPAL streams and playback draining.
- **Actor Decoupling:** Isolated dictation hotkeys, output routing, audio suppression atomics, and async TTS voice reference resolution.

### 7. Turn ID Synchronization & PTT Boundary Trimming
- **Monotonic Turn IDs:** Enforced atomic fetch-and-add increment across all PTT/dictation start events, eliminating `turn_id: 0` resets.
- **Speech Boundary Trimming:** `VadCommand::StartWindowValidation` / `StopWindowValidation` trims audio strictly to `[speech_start..speech_end]`, automatically discarding silence/accidental clicks with 0 STT/cloud emissions.
- **Jitter Buffer:** Added 250ms (12,000 samples @ 48kHz) pre-roll buffer in playback engine before opening audio output.

### 8. LLM Consolidation & Empirical Capability Discovery
- **Unified Engine (`services/llm/`):** Unified `ConnectionConfig` mapping 13 standard providers to unified `RemoteTransport` (streaming SSE line decoder, `/chat/completions`, `/responses`, `/api/chat`) and in-process `EmbeddedProvider` (`llama.cpp`).
- **Empirical Micro-Probing (`probe.rs`):** Replaced static catalog guessing with live tool schema and multilingual streaming TTFT/TPS micro-probes; purged heuristic token floor assumptions.

### 9. Memory Spec Consolidation & 4-Pillar Architecture Spec
- Consolidated memory requirements into a definitive 2-in-1 spec (`docs/plans/phase10/memory_formatting_context_assembly_spec.md` v2.0).
- Locked 4-pillar design: **Harness** (buffering, accounting, prompt building), **Retrieval** (waterfall search, scope classification), **Compaction** (async summarization), and **Ingestion** (4-stage offline queue pipeline).

### 10. Memory 4-Pillar Implementation & Pipeline Refactor
- Restructured `services/memory/` across 24 modular files conforming to the 4-pillar layout.
- Decomposed `ConversationManager` into `buffer.rs`, `accountant.rs`, `prompt_builder.rs`, `manager.rs`, and the unified `prepare_turn_context` public facade.
- Locked SSOT timing split: Critical inline compaction (`>= 0.85`), opportunistic soft compaction (`0.65 <= util < 0.85` in `{Ready, Paused}` with 20s debounce), and background queue ingestion (30s idle).

### 11. Pipeline Memory Seams & Quality Hardening
- **W1 (Pre-Compaction Filler):** Dispatches TTS transition filler before executing critical compaction, removing dead silence.
- **W2 (Cached LLM Provider):** Cached active `Arc<dyn LlmProvider>` in `AppState`, eliminating per-turn disk I/O and ORT reload during compaction.
- **W3 & R3 (Realtime & Fact Dispatch):** Guarded realtime turns on engagement/pause, routed compaction facts through `PersonalFactsReady` worker channel, and offloaded SQLite writes.
- **R2 & R4 (Event Router & Latencies):** Fixed `router.rs` so only `PlaybackFinished`/`Cancelled` emit `PipelineIdle`, preventing ingestion during active generation; wired real STT/TTFT metrics to `TurnCompleted`.
- **Quality Gate:** Clean `cargo clippy --all-targets` (0 warnings), clean `cargo check --all-targets` (0 errors), 40/40 tests green in release mode.

### 12. State, Event & Flag Bag Orchestration Purge
- **Synthetic Flags Eradicated:** Purged `is_sleeping`, `is_engaged`, `is_recording`, `is_speech_detected`, `is_earshot`, and loose `state_atomic` duplicates across `AppState`, `RuntimeSnapshot`, `collector.rs`, `telemetry_emitter.rs`, `VadActor`, `PlaybackEngine`, and TS frontend.
- **Single Source of Truth:** Replaced derived flags with direct queries on `InteractionState` (`state.pipeline.state() == InteractionState::Paused`) and polymorphic `VadBackend::is_above_noise_gate()`.

### 13. State-Event Remediation & Turn Cancellation Hardening
- **Turn ID Monotonic Invariants:** Replaced fragmented `fetch_add` calls with SSOT atomic helpers (`next_turn_id()`, `peek_turn_id()`, `next_turn()`, `cancel_current_turn()`) in `PipelineAtomics`, completely eliminating `turn_id: 0` resets across modular PTT, dictation, and duplex realtime providers (Gemini Live & Deepgram).
- **Tokio CancellationToken Standard:** Migrated all LLM provider abstractions (`LlmProvider`, `LlmEngine`, `EmbeddedProvider`, `RemoteTransport`, `LlamaCppEngine`), compaction harness (`manager.rs`, `facade.rs`, `runner.rs`), and modular pipeline dispatch to `tokio_util::sync::CancellationToken`, eliminating sleep-polling loops and bridging shims.
- **Memory Ingestion / Compaction Invariant:** Preserved strict `InteractionState::Idle` requirement for ONNX background ingestion worker with non-blocking opportunistic soft compaction execution on `{Ready, Paused}`.
- **Dead Variant Purge & Strict Style Alignment:** Removed dead IPC event variants (`WarmUp`, `SettingsUpdated`, `VadCommand::SetAudioSink`, `TtsCommand::UpdateQualitySteps`, `TtsCommand::UpdateSpeed`), resolved all `_`-prefixed variables/imports, and verified zero compiler/linter warnings across Rust and TypeScript.
- **Quality Gate:** `cargo check --all-targets` (0 errors, 0 warnings), `pnpm build` clean.

### 14. S01 Test Suite Refactor Alignment & Seam Hardening
- **S01 Seam Realignment:** Completely removed deprecated static buffer helpers (`ingest_audio`, `get_buffer_len`) across all integration tests (`modular_ptt_test.rs`, `dictation_ptt_test.rs`, `realtime_ptt_test.rs`), aligning tests directly with production capture seams (`setup_vad_actor` SPSC ring buffer producer via `stream_audio_to_ring_buffer`).
- **Strict Verification Discipline:** Enforced explicit top-level test timeouts, negative assertions for empty buffer guards and cancellations, and verified clean thread handle joining via `.join().expect(...)` across all worker threads.
- **Dictation & Realtime Invariants:** Verified Levenshtein transcription similarity >= 0.99 against ground truth fixtures, validated zero LLM execution during dictation completion, and verified ghost audio gate non-speech rejection in realtime PTT.
- **Quality Gate:** `cargo check --all-targets` (0 errors, 0 warnings), `cargo clippy --all-targets -- -D warnings` (0 warnings), all 40 tests passing in release mode (`cargo nextest run --release --test-threads=1`).

### 15. S02-S04 Concurrency & Dead Surface Remediation
- **S02 (Dead `audio_sink` Purged):** Completely removed unused `audio_sink: Option<Sender>` field, initialization, and loop checks from `VadActorState` (`services/vad/actor.rs`).
- **S03 (PTT Tokio Worker Non-Blocking):** Promoted `modular::ptt::ptt_stop` and `realtime::ptt::ptt_stop` to `pub async fn`, replacing `state.engine.blocking_lock()` with `state.engine.lock().await` and bridging to async Tauri IPC in `ipc/pipeline/assistant.rs`.
- **S04 (Monotonic Turn-ID Timing Alignment):** Standardized turn ID allocation across all PTT modes (`modular`, `realtime`, `dictation`) to allocate via `next_turn_id()` on `ptt_start` and inspect via `peek_turn_id()` on `ptt_stop`.
- **Quality Gate:** `cargo check --all-targets` (0 errors, 0 warnings), `cargo clippy --all-targets -- -D warnings` (0 warnings), release mode compilation clean.

### 16. S05 Realtime Activity Signal Wiring & Dead Surface Purge
- **S05 (Realtime Activity Signal Wiring):** Wired `rt_engine.activity_start()` into `realtime/ptt.rs::ptt_start` and `rt_engine.activity_end()` into `realtime/ptt.rs::ptt_stop` (triggered upon speech window validation), providing explicit turn segmentation signals to manual-VAD duplex providers (Gemini Live).
- **Dead Surface Purge:** Completely removed unused `is_connected` and `last_activity_time` methods from `RealtimeSession` trait and `RealtimeEngine`, and purged unread `ws_connected` atomics from session structs.
- **Server Turn Cursor Disambiguation:** Renamed `current_server_turn_id` $\to$ `server_turn_cursor` in duplex providers (`gemini_live.rs`, `deepgram_live.rs`) to prevent ambiguity with the global pipeline turn ID SSOT.
- **Pipeline Turn Helpers Adoption:** Adopted `state.pipeline.next_turn()` and `cancel_current_turn()` in realtime PTT.
- **Quality Gate:** `cargo check --all-targets` (0 errors, 0 warnings), `cargo clippy --all-targets -- -D warnings` (0 warnings), 40/40 tests green in release mode (`cargo nextest run --release --test-threads=1`).

### 17. S07, S09, S11 Lifecycle, Idempotency & Turn Accumulator Refactor
- **S11 (Transition Idempotency & Barge-In Invariant):** Added idempotency guard `if state.pipeline.state() == new_state { return; }` to `services/pipeline/mod.rs::transition()`, preventing redundant state broadcasts. Implemented `handle_barge_in()` on `MemoryManager` to cancel speculative compaction and pop trailing uncompleted User turns across modular/realtime passive and PTT handlers.
- **S07 (Dead Speech Event Handlers & Masking Purge):** Eradicated unhandled `VoxEvent::SpeechStart` / `SpeechEnd` branches and dead handlers across modular and realtime pipelines. Realtime passive now switches `Ready -> Listening` on first `TranscriptPartial`, cancels in-flight turns on barge-in (`Thinking/Speaking`), and transitions to `Thinking` on `TranscriptFinal`.
- **S09 (TurnAccumulator Encapsulation):** Replaced module-level statics (`CURRENT_ASSISTANT_RESPONSE`, `CURRENT_USER_TRANSCRIPT`, `CHUNKER`) with domain-scoped `TurnAccumulator` structs in all pipeline domains (`modular/passive.rs`, `modular/ptt.rs`, `realtime/passive.rs`, `realtime/ptt.rs`), ensuring clean deterministic lifecycle resets.
- **Quality Gate:** `cargo check --all-targets` (0 errors, 0 warnings), `cargo clippy --all-targets -- -D warnings` (0 warnings), all 40 tests passing in release mode (`cargo nextest run --release --test-threads=1`).

### 18. SSOT Model Residency Refactor & State Audit
- **Model Residency Single Source of Truth:** Eliminated 9 redundant `is_*_loaded: Arc<AtomicBool>` fields from `AppState`, `LlmWarmUpHandles`, `TtsWarmUpHandles`, `SttActorHandles`, and `VadActorHandles`.
- **Derived Residency Telemetry:** Updated `monitoring::collector::collect_snapshot()` to derive residency states directly from engine worker channels (`engine.llm_tx.is_some()`, etc.) and ML model static locks (`services::memory::is_embedder_loaded()`, etc.) without lock contention or stale flags.
- **State & Atomics Audit:** Verified all fields in `AppState`, `PipelineAtomics`, `TelemetryState`, `RuntimeSnapshot`, and `ProfilerSnapshot`. Confirmed all remaining atomics and variables have active readers and writers across backend pipelines, persistence workers, telemetry aggregator, and frontend monitoring.
- **Quality Gate:** `cargo check --all-targets` (0 errors, 0 warnings), `cargo clippy --all-targets -- -D warnings` (0 warnings).

### 19. Realtime Architecture & Lifecycle Refactor
- **Session Lifecycle Centralization:** Lifted all common session initialization (`conv_id` generation, `SessionStarted`, `ActiveSessionChanged`, and `init_new_session` with DB identity preloading) and termination (`SessionEnded`, `SessionEnd`, state transition to `Idle`) into `ipc/pipeline/assistant.rs`, stripping duplicated boilerplate across all 4 pipeline domains (`modular/passive.rs`, `modular/ptt.rs`, `realtime/passive.rs`, `realtime/ptt.rs`).
- **Resumption Cache & TTL Enforced:** Added 2-hour TTL validation on startup in `pipeline/realtime/session.rs` to read cached tokens from `realtime_session.json` into `GeminiRealtimeConfig` and purge expired files; implemented graceful cache eviction (`purge_session_cache()`) upon manual session exit.
- **Assembled System Prompt for Realtime:** Sourced the complete assembled prompt (`assemble_system_prompt()`) containing active `<user_profile>` identity facts directly into `create_realtime_provider` for Gemini Live and Deepgram Voice Agent handshakes.
- **Speaking Onset Transition:** Passed `pipeline_tx` and `turn_id` down through `RealtimeEngine` into `PlaybackBridge`, emitting `VoxEvent::PlaybackStarted` on the arrival of the first audio chunk of a turn.
- **Guaranteed Turn Persistence & Boundary Decoupling:** Completely purged leaky `handle_barge_in()` and `on_speech_start()` wrappers from `ConversationManager`; ensured barge-in interruptions in both modular and realtime modes commit partial assistant text and user prompts to SQLite via `TurnCompleted` without discarding turns; guarded background soft compaction in `facade.rs` strictly to `PipelineMode::Modular`.
- **Quality Gate:** `cargo check --all-targets` (0 errors, 0 warnings), `cargo clippy --all-targets -- -D warnings` (0 warnings), all 40 tests passing in release mode (`cargo nextest run --release --test-threads=1`).

### 20. Realtime Passive Review & Hardening
- **VAD Audio Passthrough Disconnect on Pause:** Implemented explicit `VadCommand::StopRealtime` dispatch during `realtime::passive::pause_session`, shutting down microphone frame passthrough to prevent ghost buffer accumulation while audio engine remains warm.
- **Unified Hindi/Hinglish Transliteration:** Wired `crate::services::translit::transliterate_if_hi` on both partial and final transcripts in `realtime/passive.rs`, ensuring Romanized transliteration conforms to user settings.
- **Barge-In Turn ID Alignment:** Fixed turn ID assignment during barge-in SQLite persistence to bind the interrupted turn to its actual `interrupted_turn_id` (`peek_turn_id()`) rather than the incoming turn ID.
- **Unified Idle Monitor:** Promoted `spawn_idle_monitor` to `pipeline/mod.rs` and wired it into `ipc/pipeline/assistant.rs`, auto-pausing any active assistant pipeline upon 7 minutes (420s) of continuous `Ready` state.
- **Quality Gate:** `cargo check --all-targets` (0 errors, 0 warnings), `cargo clippy --all-targets -- -D warnings` (0 warnings), all 40 tests passing in release mode (`cargo nextest run --release --test-threads=1`).

### 22. Canonical Event Architecture Refactor & Registry Ownership
- **Strictly Registry-Owned Event SSOT (`core/events.rs`):** Established `core/events.rs` as the single source of truth for all cross-boundary events, introducing the strongly typed `IpcEvent` enum with typed payload structs (`StateChangedPayload`, `TranscriptPayload`, `LlmTokenPayload`, `VoiceErrorPayload`, `ModelSetupStatus`, `TelemetryData`, `SystemStatsPayload`) and safe `emit_ipc` / `emit_ipc_to` dispatchers.
- **Zero Raw Strings Invariant:** Eliminated all raw event string literals (`"state_changed"`, `"voice_error"`, `"transcript_partial"`, `"transcript_final"`, `"llm_token"`, `"telemetry"`, `"system_stats"`, `"toggle_tray"`, `"settings-updated"`) from backend emit sites.
- **Consolidation of Remote Setup Stream:** Completely pruned legacy `remote_setup_status`; remote GPU server installation now emits canonical `IpcEvent::ModelProgress` with `model_id: "chatterbox_remote_server"`.
- **Frontend IPC Contract Mirroring (`eventsService.ts`):** Created `IpcEventMap` mirroring Rust payloads with generic compile-time checked `eventsService.on<K extends keyof IpcEventMap>(event, handler)`.
- **Quality Gate:** `cargo clippy --all-targets -- -D warnings` (0 warnings), `cargo check --release` (0 errors), Vitest suite 10/10 files (99/99 tests passed), and `pnpm build` (clean Vite bundle in 10.36s).

### 23. Architectural Invariants Formalization
- **Backend Style Guide (`.agents/rules/backend-style-guide.md`):** Added Section 7.5 ("Event Contracts") codifying strict registry ownership, typed payloads, explicit producer/consumer contracts, and the command vs event distinction.
- **AGENTS.md Critical Invariants (`AGENTS.md` Section 4.1):** Formulated the 8 non-negotiable architectural and logical invariants across backend and frontend (State SSOT, Registry-Owned Events, Sacred Audio Hot Path, Actor-Engine Separation, Frontend Service Boundary, Context Memoization & Selector Discipline, Monotonic Turn IDs, Single-Consumer Audio Stream).

### 24. Playback Consolidation & TTS Event Bus Pruning
- **Playback Consolidation (`services/audio/playback.rs`):** Consolidated `PlaybackBridge` into `PlaybackEngine` with native `spawn_pcm_stream_worker` and `ingest_chunk_i16` supporting dynamic resampling, deleting `services/realtime/playback_bridge.rs`.
- **Event Bus Pruning (`core/events.rs`):** Completely removed raw data transport events `VoxEvent::TtsChunk` and `VoxEvent::TtsFinished` from the pipeline event bus.
- **Direct TTS Delivery:** Updated TTS providers (Chatterbox local, Chatterbox remote, Supertonic, EdgeTTS) to feed audio chunks directly to `PlaybackEngine::ingest_chunk` and record RTF metrics directly to telemetry atomics.
- **Quality Gate:** `cargo clippy --all-targets -- -D warnings` (0 warnings), `cargo check --all-targets` (0 errors), Vitest suite 10/10 files (99/99 tests passed), and `pnpm build` (clean Vite bundle).

### 25. Playback State Guard Alignment & Persistence SSOT
- **Domain State Precondition Guards:** Added strict state verification guards across all 4 voice assistant pipeline domains (`modular/passive.rs`, `modular/ptt.rs`, `realtime/passive.rs`, `realtime/ptt.rs`):
  - `on_playback_started`: Strictly requires `state.pipeline.state() == InteractionState::Thinking` before transitioning to `InteractionState::Speaking`.
  - `on_playback_finished`: Strictly requires `state.pipeline.state() == InteractionState::Speaking` before transitioning to `InteractionState::Ready`.
- **Persistence SSOT (`VoxEvent::LlmFinished`):** Canonicalized `VoxEvent::LlmFinished` as the universal trigger across modular and realtime modes for consuming the assistant response from `TurnAccumulator`, pushing the turn into `ConversationManager`, and sending `PersistenceEvent::TurnCompleted` to SQLite. Playback completion is strictly reserved for the audio output state transition (`Speaking` $\to$ `Ready`).
- **VoiceError Payload Consistency:** Fixed `VoiceErrorPayload` field structure in realtime passive and PTT domain error handlers to adhere to `{ message, source, owner }`.
- **Quality Gate:** `cargo clippy --all-targets -- -D warnings` (0 warnings), `cargo check --all-targets` (0 errors).

### 26. Kokoro Multi-Lang v1.1 TTS Integration
- **Engine Implementation (`services/tts/providers/kokoro.rs`):** Built `KokoroEngine` using `sherpa-onnx`'s `OfflineTtsKokoroModelConfig` with dynamic speaker voice embeddings, `espeak-ng-data` phonemization, and direct 24kHz audio stream delivery into `PlaybackEngine`.
- **Model Directory & Path SSOT:** Defined `KOKORO_MODEL_DIR = "tts/kokoro"` in `services/tts/mod.rs` and registered the model group `kokoro_multi_lang_v1_1` in `manifests/models_manifest.json`.
- **Settings & IPC Sync:** Added `TtsActiveProvider::Kokoro`, `TtsKokoroConfig`, and `TtsProviderConfig::Kokoro` variants across `core/settings.rs`, `ipc/settings/health.rs`, `ipc/settings/mutation.rs`, `services/tts/actor.rs`, and frontend `settingsStore.ts` / `settingsCopy.ts`.
- **Quality Gate:** `cargo clippy --all-targets -- -D warnings` (0 warnings), `cargo check --all-targets` (0 errors), `pnpm build` (clean Vite bundle).

### 27. Universal `Listening` $\to$ `Thinking` on `TranscriptFinal` & Realtime PTT Activity Alignment
- **`TranscriptFinal` as Single Source of Truth for `Thinking`:**
  - In `modular/passive.rs`: Removed state transition from `on_speech_end` (which previously forced `Thinking` on raw VAD boundaries). In `on_transcript_final`, if `processed_text` is empty, returns cleanly to `Ready`; if valid, transitions `Listening` $\to$ `Thinking` right before context preparation and LLM generation.
  - In `modular/ptt.rs`: Removed premature `Thinking` transition on key release (`ptt_stop`). The pipeline remains in `Listening` while STT processes; `on_transcript_final` performs the transition to `Thinking` (valid text) or `Ready` (empty text).
  - In `realtime/passive.rs`: Added empty transcript check in `on_transcript_final` to transition to `Ready` on silence/noise instead of `Thinking`.
  - In `realtime/ptt.rs`: `on_transcript_final` transitions to `Thinking` upon receipt of server transcript, or `Ready` if empty.
- **Realtime PTT Activity Stream Alignment:**
  - Removed premature `rt_engine.activity_start()` dispatch from key-down (`ptt_start`).
  - In `ptt_stop`, queries VAD window validation: if non-speech, discards audio and transitions to `Ready` with zero network frames sent; if speech is detected, streams `activity_start()` $\to$ `push_audio(&i16_samples)` $\to$ `activity_end()` in sequence while keeping the pipeline in `Listening` until the server returns `TranscriptFinal`.
- **Quality Gate:** `cargo clippy --all-targets -- -D warnings` (0 warnings), `cargo check --all-targets` (0 errors), `pnpm build` (clean Vite bundle).

### 28. TTS Dynamic Voice Selection (`set_voice`) & Hot-Swap Architecture
- **`TtsProvider` Trait Enhancement (`services/tts/providers/mod.rs`):** Added `fn set_voice(&self, _voice: i32) {}` default method to the `TtsProvider` trait.
- **Atomic Dynamic Voice in Kokoro & Supertonic:**
  - `KokoroEngine` (`services/tts/providers/kokoro.rs`): Converted `voice` to `AtomicI32`, implemented `set_voice` (clamped $\ge 0$), and dynamically loads `sid` per chunk.
  - `SupertonicEngine` (`services/tts/providers/supertonic.rs`): Converted `voice` to `AtomicI32`, implemented `set_voice` (clamped 0..9), and dynamically loads speaker ID per chunk.
- **Actor & Settings Integration:**
  - Added `TtsCommand::SetVoice(i32)` to `TtsCommand` enum and worker dispatch loop in `services/tts/actor.rs`.
  - Updated `get_setting_reload_policy` in `core/settings.rs` to treat `tts.voice_index` and `tts.voice` as `SettingReloadPolicy::WorkerCommand`.
  - Wired `dispatch_worker_command` in `ipc/settings/mutation.rs` to forward `TtsCommand::SetVoice(voice)` directly to the active TTS worker channel, eliminating full audio engine reloads on voice switching.
- **Quality Gate:** `cargo clippy --all-targets -- -D warnings` (0 warnings), `cargo check --all-targets` (0 errors), `pnpm build` (clean Vite bundle).

### 29. Realtime Actor-Engine Decoupling & Provider Framing Encapsulation
- **Architecture & Service Layer Refactor:**
  - **`RealtimeActor` (`services/realtime/actor.rs`):** Replaced legacy `RealtimeEngine` wrapper with a proper `RealtimeActor`. The actor now owns the `event_tx: Sender<VoxEvent>` pipeline stream, the `playback_engine: Arc<PlaybackEngine>`, and a dedicated async Tokio event loop that consumes typed internal provider events.
  - **`RealtimeProviderEvent` Enum (`services/realtime/mod.rs`):** Established an internal typed communication channel between providers and `RealtimeActor` (`AudioChunk`, `TranscriptPartial`, `TranscriptFinal`, `LlmToken`, `LlmFinished`, `Interrupted`, `Error`, `SessionResumptionHandle`). Providers are now strictly transport drivers and no longer have references to `VoxEvent` or `event_tx`.
  - **Semantic Trait Protocol (`RealtimeSession`):** Replaced Gemini wire-protocol leaked methods (`activity_start`/`activity_end`) with a single semantic method `commit_speech_turn(&self, pcm: &[i16])` on `RealtimeSession`.
  - **Provider Enveloping:**
    - `GeminiLiveSession`: Internally maps `commit_speech_turn` to `ActivityStart` $\to$ PCM chunks $\to$ `ActivityEnd` wire frames.
    - `DeepgramVoiceAgentSession`: Enqueues PCM chunks directly without manual framing.
  - **Domain Cleansing (`pipeline/realtime/ptt.rs` & `passive.rs`):** Domain logic now invokes `rt_actor.signal_speech_committed(&i16_samples)` and `rt_actor.signal_interrupt()`, with 0 awareness of provider protocol or wire command mechanics.
  - **Non-Blocking Session Cache:** Relocated `realtime_session.json` cache persistence from blocking synchronous file I/O inside provider message handlers to non-blocking `tokio::fs` within `RealtimeActor`.
### 30. IPC Command Realignment, Service Layer Extraction & LLM Subsystem Consolidation
- **Runtime Defect Repairs & SSOT Invariants:**
  - Standardized `get_settings` as the single SSOT query returning `BootState` (snapshot, models dir presence, and settings path).
  - Pruned dead `getCachedCapabilities` in favor of `probeModelCapabilities` / `probeModelCapabilitiesFull`.
  - Consolidated queue retries to `retry_failed_queue_items(item_ids: Option<Vec<i64>>)` across backend and frontend `memoryService.ts`.
  - Pruned dead function definitions in Rust `ipc/memory/graph.rs` and `ipc/memory/ingestion.rs`.
- **Thin Handler IPC & Service Layer Separation:**
  - Extracted provider health checks into dedicated domain module `services/health.rs`.
  - Extracted SSH runner and setup script streaming into `setup/remote_server.rs`.
  - Extracted model task runner, file verification, and manifest syncing into `setup/manager_ops.rs`.
  - Enforced strict $\le 30$-line thin routing envelopes across `ipc/setup.rs` and `ipc/settings/health.rs`.
- **LLM Subsystem Consolidation & Defragmentation:**
  - Integrated `types.rs` structs and error types into `services/llm/mod.rs`.
  - Merged capability probing and model listing into `services/llm/probe.rs`.
  - Pruned micro-fragmented files (`probing.rs`, `types.rs`).
- **Quality Gate:** Verified `cargo check --all-targets` (0 errors), `pnpm test` (96/96 tests passed across all 10 suites), `pnpm run build` (0 TypeScript errors in 5.46s).

### 31. Backend Comprehensive Review — 7-Sprint Audit (§31)

- **Scope & Method:** Full `app/src-tauri/src/` audit — 147 `*.rs`, 32k LOC — via 7 parallel `explore` subagents (`very thorough`) against `backend-style-guide.md §1-10` + `backend-engineer.md` invariants + `AGENTS.md §4.1` 8 invariants. `tests/`/`benches/`/`examples/` excluded; `submodules/` not audited.
- **Gates:** `cargo check --all-targets` `0 errors` `app/src-tauri/Cargo.toml:1` (28.5s) + `cargo clippy --all-targets -- -D warnings` `0 warnings` (27.6s) — **clippy green ≠ style compliant** (style gates strictly tighter). Snapshot greps: `unwrap()` 13, `let _ =` 16 (`remote_server.rs:102` ×8), `#[allow` 0, `is_dictation_enabled` `state.rs:278` + `is_private_mode` `state.rs:325`/`worker.rs:15`.
- **Totals (de-duplicated):** **~283 issues** — 🔴 Critical 14, 🟠 High 52, 🟡 Medium 78, 🔵 Low 31, 📏 Style-Guide 108 (see `docs/plans/phase10/backend_review_report.md:1` ledger).
  - **Critical (14):** Sacred audio hot-path `log!/send/clone` on CPAL I/O + VAD loop `device.rs:135` `playback.rs:402` `vad/actor.rs:288`/`earshot_vad.rs:52`; `engine.lock().await` held across `recv_timeout(500ms)` in 3 PTT paths `dictation.rs:75` `modular/ptt.rs:196` `realtime/ptt.rs:250`; banned `is_dictation_enabled`/`is_private_mode` `state.rs:278` `worker.rs:15`; provider-local `fetch_add` turn fragments `gemini_live.rs:761` `deepgram_live.rs:602`.
  - **High (52):** `Result<T,String>` 58 sites typed-error violation `§5` (`ipc/audio.rs:22` `history.rs:8` `health.rs:16`); concrete `AppHandle` 29 sites `§10` (`tray.rs:53` `assistant.rs:12`); thread-priority gaps (`stt/actor.rs:300` `tts/actor.rs:213` missing `Max`); `std::fs` blocking in async `realtime/session.rs:22` `setup/model_manager.rs:212`; `parking_lot::Mutex` in Tokio `gemini_live.rs:140`.
  - **Style (108):** `§2` file ceiling 5 (`llama_cpp.rs:717` `probe.rs:658` `gemini_live.rs:1036` `mutation.rs:1000`); `§2.1` grammar 38 (missing `//!` 20/22 `ipc/*` + import inversion 18/29 `llm/*`); `§4` caps 48 (48 fns >50L, top `facade.rs:prepare_turn_context 226` `gemini_live.rs:connect 476` `llama_cpp.rs:generate 344`); `§3` hierarchy 27 (11 VAD thresholds not in `vad/mod.rs:13`).
- **Sprint Ledgers:** S1 Core 43 issues (3🔴 4🟠 8🟡 6🔵 22📏) — `is_*` flags + `lib.rs:51` 566-line monolith; S2 IPC 87 — 58 string errors + 29 `AppHandle` + dictation orphan 3 cmds + `fetch_manifest` S5 violate; S3 Pipeline 6 critical — `try_lock` silent drops `modular/passive.rs:67` + `blocking_lock` on router + filler dead code `modular/passive.rs:352`; S4 Audio/VAD 4 sacred violations + 11 constant leaks; S5 Inference 2 ceiling + 14 thread-priority/blocking-HTTP; S6 Realtime/Memory 3 crit — `ws_connected` flag + `fetch_add` + `block_in_place`; S7 Persistence 18 error/visibility + `Box::leak` `db.rs:18`.
- **Artifact:** Full per-sprint `File:Line | Cat | § | Evidence | Fix` tables in [`docs/plans/phase10/backend_review_report.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/backend_review_report.md). **Next P0 stack:** (1) sacred path `log/send/clone` removal + `utterance_buffer` pre-alloc 160k, (2) drop guard before `recv_timeout` ×3, (3) delete `is_*` flags + centralize `AppState::next_turn_id`, (4) `<R:Runtime>` generics 29 sites — gates `tauri::test::mock_app`. P1 `thiserror` 58 sites, P2 split 5 ceiling files + 48 caps.
