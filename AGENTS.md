# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: Automatic Documentation & AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK DOCUMENTATION HOOK (NON-NEGOTIABLE):**
> 1. You **MUST** automatically update `AGENTS.md` (specifically Section 5) directly with all recent work, milestones, implementation changes, model configurations, and threshold updates.
>    - **Line-Count Threshold (175 Lines Max):** All recent work is recorded in `AGENTS.md` first. When `AGENTS.md` approaches or exceeds the 175-line ceiling, you MUST compact Section 5 into high-level critical milestones, move the uncompacted detailed history into `docs/plans/<current_phase>/recent_work.md` (e.g. [`recent_work.md`](file:///home/addy/projects/apps/vox/docs/plans/<current_phase>/recent_work.md)), and keep a deep link to it.
> 2. You **MUST** automatically update any relevant feature, component, design, or architecture documentation in docs/ to match the actual code state, key files include `backend.md`, `models.md`, `frontend.md` and `docs/features/*`.
> 3. This is a **mandatory post-task completion hook** — do NOT wait for the user to explicitly remind you to sync documentation.

---

## 1. Project Context

Vox is a **realtime voice AI desktop app** (Tauri v2 / Rust / TypeScript). Constraint: 8GB RAM, CPU-first inference, sub-200ms perceived pipeline latency.

**Crate structure:** Single Rust library crate `vox_lib` at `app/src-tauri/`. `main.rs` is 1 line. `lib.rs` is module declarations + Tauri assembly only. All logic lives in modules.

---

## 2. Workspace Directory Map

| Path                      | Purpose                                             | Rules                                                                                                 |
| ------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `app/src-tauri/src/`      | Purpose Rust source                                 | No test logic. No benchmarks.                                                                         |
| `app/src-tauri/tests/`    | Integration tests (`cargo test --tests`)            | Named `<feature>_test.rs`. Tests public API only.                                                     |
| `app/src-tauri/benches/`  | Performance benchmarks (`cargo test --benches`)     | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`.                                   |
| `app/src-tauri/examples/` | Utility CLI tools (`cargo run --example <name>`)    | Standalone tools. No `#[test]`. No assertions.                                                        |
| `.agents/rules/`          | Role-specific agent instruction files               | Read relevant file before acting in that role.                                                        |
| `docs/plans/`             | Architecture specs and phase plans                  | Source of truth for specs. Do not contradict.                                                         |
| `docs/features/`          | Implemented feature ledgers                         | Update after completing features.                                                                     |
| `sandbox/`                | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`.                  |
| `temp/`                   | Ephemeral runtime files: logs, raw LLM outputs      | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned.                   |
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`. Do not edit directly. |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |

**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

1. **Sequential Execution:** Run performance-sensitive tasks (benchmarks, evals, test suites) strictly one at a time to prevent CPU, memory, and I/O contention.
2. **Release / Optimized Mode:** Always run performance measurements and benchmarks under release mode (`--release`). Debug builds produce invalid metrics.
3. **Isolated Test Runner (`cargo-nextest`):** Always use `cargo-nextest run` with explicit thread pool allocation and single-thread isolation:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1
   ```
   _Single test:_ `cargo nextest run --test <test_file> --release --nocapture --test-threads=1`  
   _Timeouts:_ 60s per individual test, 90s full suite (baseline runtime is ~28.8s).
4. **External API Keys (`#[ignore]`):** Cloud provider tests (Nvidia, Gemini Live, Deepgram, OpenAI, ElevenLabs) must be marked `#[ignore]` and run manually only with explicit user approval: `cargo nextest -- --ignored`.

---

## 4. Invariants and Workflow Gates

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)

1. **Single Source of Truth for State:** `InteractionState` (`Idle, Ready, Listening, Thinking, Speaking, Paused, Error`) and `DictationState` (`Idle, Recording, Transcribing, Error`) are the sole sources of truth. Synthetic lifecycle booleans (`is_engaged`, `is_recording`, `is_connected`, `is_speaking`, `is_sleeping` are strictly banned across Rust and TypeScript.
2. **Registry-Owned Event Contracts:** `core/events.rs` is the SSOT for all cross-boundary events. Internal pipeline events belong to `VoxEvent`; IPC events belong to `IpcEvent` with strongly typed payloads. Raw string event literals are forbidden at emit and listen sites; frontend mirrors the registry via `IpcEventMap`.
3. **Sacred Audio Hot Path:** Zero dynamic memory allocations, zero lock acquisitions (`Mutex`/`RwLock`), and zero blocking I/O on the CPAL audio thread and VAD inference loop. Ring buffers must be lock-free and pre-allocated.
4. **Actor-Engine Separation & Thread Isolation:** CPU/GPU-heavy model inference (Whisper STT, ONNX VAD, Llama LLM, Chatterbox TTS) runs strictly on dedicated background OS threads (`std::thread`). Tokio runtime is reserved strictly for async I/O, IPC routing, and network WebSockets.
5. **Strict Frontend Service Boundary:** React components and hooks must never directly call `@tauri-apps/api/core` (`invoke`) or `@tauri-apps/api/event` (`listen`). All backend interactions must route through strongly-typed singleton service modules in `src/services/`.
6. **React 19 Context Memoization & Selector Discipline:** Provider values must be wrapped in `useMemo`. Zustand store state must be queried via fine-grained atomic selectors (`(s) => s.field`) rather than consuming entire store snapshots, preventing cascading render loops.
7. **Centralized Monotonic Turn ID Progression:** Monotonic turn IDs must be generated exclusively at turn boundaries via `AppState::next_turn_id()`. Turn IDs must never be reset to 0, fragmented across parallel actors, or fabricated with dummy values.
8. **Single-Consumer Audio Stream Invariant:** Audio ring buffers and input channels must have exactly one consumer (`VadActor`). Never attach secondary or ad-hoc readers to production audio streams.

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
>
> - **WRITE TASK (Backend Rust):** You MUST read `.agents/rules/backend-style-guide.md` AND `.agents/rules/backend-engineer.md` BEFORE modifying Rust backend code.
> - **WRITE TASK (Frontend React/TS):** You MUST read `.agents/rules/frontend-style-guide.md` AND `.agents/rules/frontend-engineer.md` BEFORE modifying frontend code.
> - **WRITE TASK (Tests/Benches/Evals):** You MUST read `.agents/rules/testing-style-guide.md` AND `.agents/rules/test-engineer.md` BEFORE authoring tests or benchmarks.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.

---

### 4.3 Agent Roles

| Role                 | Rule File                               | Scope                                                            |
| -------------------- | --------------------------------------- | ---------------------------------------------------------------- |
| System Architect     | `.agents/rules/system-architect.md`     | Strategy, gates, plan approval                                   |
| Backend Engineer     | `.agents/rules/backend-engineer.md`     | `app/src-tauri/src/` implementation                              |
| Frontend Engineer    | `.agents/rules/frontend-engineer.md`    | `app/src/` implementation                                        |
| QA Engineer          | `.agents/rules/qa-engineer.md`          | Test audit, benchmark validation                                 |
| ML Research Engineer | `.agents/rules/ml-research-engineer.md` | ML model research, evaluation, and fine-tuning dataset curation  |
| Test Engineer        | `.agents/rules/test-engineer.md`        | Test case design, benchmark validation, and performance analysis |

---

## 5. Phase 10 Architecture & Orchestration Refactor Ledger

> 📖 **Full History & Sprint Details:** See [`docs/plans/phase10/recent_work.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md) for full chronological storyline and sprint breakdowns.

- **State & Flag Purge:** Purged synthetic flags (`is_sleeping`, `is_engaged`, `is_recording`, etc.); switched to SSOT state enum queries.
- **Monotonic Turn ID & Tokio standard:** Centralized turn IDs in `PipelineAtomics`; unified cancellation tokens with `tokio_util::sync::CancellationToken`.
- **Test Suite Realignment (S01):** Production capture seam integration across all test suites (`cargo nextest run --release` 40/40 green).
- **Concurrency & Dead Code Purge (S02-S05):** Removed dead `audio_sink` and unread atomics; promoted PTT workers to async locks; wired realtime activity signals.
- **Lifecycle & TurnAccumulator (S07-S11):** Idempotent transitions, barge-in memory cleanup, and domain-scoped `TurnAccumulator` structs.
- **Subsystem Promotion & Path Alignment:** Promoted `services/pipeline/` $\to$ `src/pipeline/` (`vox_lib::pipeline::*`) and `services/memory/harness/` $\to$ `services/harness/` (`vox_lib::services::harness::*`), with all integration tests and benchmarks aligned.
- **SSOT Model Residency & Dead State Audit:** Eliminated 9 redundant `is_*_loaded` AtomicBools across `AppState` and actor handles; derived residency directly from engine channels & static locks in `monitoring::collector`; verified all state atomics across backend and frontend.
- **Realtime Architecture & Lifecycle Refactor:** Decoupled session boot/teardown into IPC SSOT orchestrators; unified 7-min idle monitor (`REALTIME_IDLE_TIMEOUT=420s`) across all assistant pipelines in `pipeline/mod.rs`; enforced 2-hr session resumption cache validation and graceful eviction; wired `PlaybackStarted` on first audio chunk; ensured guaranteed turn persistence on barge-in across modular and realtime modes with zero context-compaction leakage.
- **Realtime Passive Review & Hardening:** Fixed audio passthrough leak on `pause_session` by dispatching `VadCommand::StopRealtime`; wired Hindi/Hinglish `transliterate_if_hi` on partial/final realtime transcripts before UI emit; resolved barge-in turn ID desync to persist interrupted turns under their actual turn ID.
- **ConversationManager & ContextHarness Decoupling:** Cleanly separated pure dialog turn & prompt state (`ConversationManager`) from modular LLM token budgeting, sliding-window FIFO, and compaction state (`ContextHarness`), ensuring Realtime S2S domains have zero coupling or overhead to context compaction machinery. Added `#![recursion_limit = "256"]` to `lib.rs` for deep async Tauri handler state machines.
- **Canonical Interruption & VAD Encapsulation Refactor:** Decoupled `VadCommand::SetOperationalMode` out of assistant IPC `start_session` directly into domain starters (`modular::passive`, `modular::ptt`); retained `realtime_session.json` cache on network retries exhaustion for UI reconnect; purged legacy fragmented `realtime_barge_in()` helpers; introduced canonical `VoxEvent::Interrupted` dispatched by Gemini (`serverContent.interrupted`) and Deepgram (`UserStartedSpeaking`); implemented isolated `on_interrupt` across all 4 voice domains with guaranteed SQLite turn persistence and zero nominal handler entanglement.
- **Event Architecture Refactor & Dead Event Pruning (Sprints 1–5):** Consolidated Tauri IPC and internal actor busses to strict SSOT streams; renamed `toggle_hud` $\to$ `toggle_tray` and `pipeline_error` $\to$ `voice_error`; unified dictation state machine emissions into universal `state_changed({ owner, state, turn_id })`; consolidated all model setup events into `model_progress`; pruned 15+ dead and redundant events (`theme-changed`, `mode_changed`, `dictation_success`, `runtime_booting`, `model_loading`, etc.); fixed Setup Wizard microphone energy meter by replacing phantom `audio_energy` with canonical `telemetry.energy`; dynamically re-polled CPU governor in monitoring collector; full Vitest suite (99/99) and Vite build verified green.
- **Strictly Registry-Owned Event SSOT & Invariants Formalization:** Established `core/events.rs` as the single source of truth for all cross-boundary events with `IpcEvent` enum and typed payloads (`StateChangedPayload`, `TranscriptPayload`, `LlmTokenPayload`, `VoiceErrorPayload`, `ModelSetupStatus`, `TelemetryData`, `SystemStatsPayload`); eliminated all raw event strings from backend emit sites; consolidated remote GPU server setup into `ModelProgress`; mirrored IPC contract in TypeScript via `IpcEventMap` with generic compile-time checked `eventsService.on<K>()`; codified Event Contracts in backend style guide and synthesized 8 non-negotiable critical invariants into `AGENTS.md` Section 4.1.
- **Playback Consolidation & TTS Event Bus Pruning:** Consolidated `PlaybackBridge` into `PlaybackEngine` (`services/audio/playback.rs`) with native PCM i16 stream worker and resampling; purged raw data events `VoxEvent::TtsChunk` and `VoxEvent::TtsFinished` from the canonical event bus; wired TTS providers (Chatterbox local/remote, Supertonic, EdgeTTS) to feed `PlaybackEngine` directly with direct RTF telemetry recording.
- **Playback State Guard Alignment & Realtime SSOT:** Strict precondition guards across all 4 voice domains (`Thinking` $\to$ `Speaking` on `on_playback_started`; `Speaking` $\to$ `Ready` on `on_playback_finished`); canonicalized `VoxEvent::LlmFinished` as the universal persistence trigger for SQLite and `ConversationManager`.
- **Kokoro Multi-Lang v1.1 TTS Integration:** Implemented `KokoroEngine` using `sherpa-onnx`'s `OfflineTtsKokoroModelConfig` in `tts/kokoro/` with dynamic user voices, native 24kHz stream output, IPC settings schema sync, and manifest registration (`manifests/models_manifest.json`). Verified 0 compile errors and 0 clippy warnings.
- **Universal Listening $\to$ Thinking on TranscriptFinal:** Canonicalized `TranscriptFinal` across all 4 voice assistant domains as the SSOT for entering `Thinking` (valid text) or cleanly returning to `Ready` (empty text/silence); purged premature `Thinking` transitions from VAD `on_speech_end` and PTT key release (`ptt_stop`); aligned Realtime PTT to stream `activity_start` $\to$ audio $\to$ `activity_end` only upon validated speech, preventing ghost turns and session stalls.
- **TTS Dynamic Voice Hot-Swapping (`set_voice`):** Added `set_voice(&self, voice: i32)` to `TtsProvider` trait, backed by `AtomicI32` in `KokoroEngine` and `SupertonicEngine`; added `TtsCommand::SetVoice(i32)` to `TtsActor` and mapped `tts.voice_index` to `SettingReloadPolicy::WorkerCommand`, enabling instant, zero-allocation runtime voice switching without restarting or reloading ONNX weights.
- **Full-Stack IPC Command Architecture & Thin-Handler Overhaul (Sprints 1–7):** Audited and streamlined backend `#[tauri::command]` handlers from 87 down to 54 registered commands. Decomposed large handlers into thin routing envelopes ($\le 30$ lines) delegating to service helpers in `ipc/settings/health.rs`, `ipc/setup.rs`, and `ipc/memory/mutations.rs`. Consolidated fragmented provider health checks into `check_provider_health(kind, provider)` and unified model operations into `manage_models(payload)` and `check_updates(scope)`. Pruned dead dictation & monitoring history commands, redundant capability/queue queries (`get_cached_capabilities`, `retry_failed_queue`), and unified memory mutations into `manage_memory_fact(payload)`. Encapsulated all frontend invokes strictly within singleton services (`services/*Service.ts`). All 10 Vitest suites (96/96 tests) and Vite production bundle verified green.
- **Realtime Actor-Engine Decoupling & Provider Framing Encapsulation:** Decoupled `RealtimeActor` from providers; introduced `RealtimeProviderEvent` typed result channel so providers no longer directly emit `VoxEvent`s or touch `PlaybackEngine`; encapsulated wire protocol envelopes (`activityStart`/`activityEnd`) inside provider sessions via semantic `commit_speech_turn(pcm)` on `RealtimeSession` trait; cleaned domain layer `realtime/ptt.rs` and `realtime/passive.rs` of provider leaks; shifted session cache writing to non-blocking `tokio::fs` within actor task loop.
- **IPC Command Realignment, Service Extraction & LLM Consolidation (§30):** Standardized `get_settings` as SSOT; resolved all breaking frontend invoke call sites; pruned dead `getCachedCapabilities` and unreferenced memory graph/queue definitions; retained `fetch_manifest` for onboarding setup wizard; extracted SSH remote server runner to `setup/remote_server.rs`, model ops to `setup/manager_ops.rs`, and provider health checks to `services/health.rs`; streamlined IPC files (`ipc/setup.rs`, `ipc/settings/health.rs`) to thin routing envelopes ($\le 30$ lines); consolidated `types.rs` into `services/llm/mod.rs` and capability probing into `services/llm/probe.rs`; wired custom voice inline renaming (`renameVoice` / `rename_voice`) in `VoiceCarousel.tsx`. All 10 Vitest suites (96/96) and Vite production build verified green.
- **Backend Comprehensive Review — 7-Sprint Audit (§31):** Full `app/src-tauri/src/` audit (147 `*.rs`, 32k LOC) via 7 parallel explore subagents against `backend-style-guide.md §1-10` + `AGENTS.md §4.1` invariants. Result: ~283 issues — 🔴 Critical 14 (sacred audio `log!/send/clone` on CPAL/VAD `device.rs:135` `playback.rs:402` `vad/actor.rs:288`, `engine.lock` held across `recv_timeout(500ms)` in 3 PTT paths, banned `is_dictation_enabled`/`is_private_mode` `state.rs:278`, turn `fetch_add` fragments `gemini_live.rs:761`), 🟠 High 52 (58 `Result<T,String>` typed errors, 29 concrete `AppHandle`, thread-priority gaps), 📏 Style 108 (§2.1 grammar 38, §4 caps 48, §3 hierarchy 27). `cargo check` + `clippy -- -D warnings` 0 warnings; greps: `unwrap()` 13, `let _ =` 16. Artifact: [`docs/plans/phase10/backend_review_report.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/backend_review_report.md) with per-sprint ledgers and P0 remediation stack.