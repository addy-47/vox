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
