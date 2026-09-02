# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target.
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **165 lines** (the warning threshold before the 175-line ceiling):
> 1. Write the **full, uncompacted** current Section 5 content to `docs/plans/<current_phase>/recent_work.md`.
> 2. Replace Section 5 in `AGENTS.md` with a compact 3–5 bullet summary of only the highest-level milestones.
> 3. Add a deep link at the top of Section 5: `📖 Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/<current_phase>/recent_work.md)`.
>
> **This is the complete hook. Nothing else is mandatory on every task.**

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
7. **Centralized Monotonic Turn Lifecycle:** Monotonic turn IDs must be generated exclusively at turn boundaries via `PipelineAtomics::next_turn()`, which atomically advances the turn ID, renews the `CancellationToken`, and returns `(turn_id, token)` as a bundle. Turn IDs must never be reset to 0, fragmented across parallel actors, or fabricated with dummy values. Subsystems receive `(turn_id, token)` at the turn boundary — they never own or directly advance the underlying `AtomicU32`.
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

> 📖 **Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md)**

- **Foundation & Lifecycle (Purge → SSOT):** Purged synthetic `is_*` flags across frontend and backend; unified turn lifecycles strictly under monotonic `PipelineAtomics::next_turn()` (`(turn_id, token)` bundle) with poison-safe `parking_lot` snapshots.
- **Audio Hot Path & Realtime Hardening:** Eliminated all allocations, logging, and lock acquisitions from sacred CPAL output/VAD loops; converted realtime streaming and control queues to bounded channels (`capacity = 100`) with non-blocking drops.
- **IPC & Error Standard:** Standardized all 37 Tauri IPC handlers to strongly-typed `VoxIpcError` with tagged JSON serialization and parameterized command services with generic runtime `<R: tauri::Runtime>`.
- **Constant Hierarchy & Traversal Security:** Centralized all subsystem timeouts, buffer sizes, window identifiers, and URLs into owning `mod.rs`/`constants.rs` roots; secured archive extractions against Zip-Slip/Tar-Slip traversal.
- **Structural Decoupling & Module Hierarchy (2026-09-01):** Flattened `services/stt/` removing `super::super::*` escapes; relocated `AudioResampler` to `services/audio/resampler.rs` fixing lateral audio $\to$ realtime inversion; centralized voice profiles in `services/tts/voice.rs`; unified `asr` $\to$ `stt` frontend/backend catalog; decoupled `services/llm/` into symmetric `embedded/` (`family.rs`, `worker.rs`, `generate.rs` with `GenerationLimits` soft caps) & `transport/` (`config.rs`, `chat_completions.rs`, `ollama.rs`, `responses.rs`, `sse.rs`), relocated compaction token math to `memory/compaction/`, merged policy into `actor.rs`. Zero clippy warnings across all targets.
- **Structure Audit (2026-09-01):** 5-sprint review-only flag audit (154 files, 32,950 LOC, 2487 audit lines, soft 120/600 caps) → `docs/plans/phase10/audits/{audit-llm.md, audit-stt-vad-audio.md, audit-tts-realtime.md, audit-memory-harness.md, audit-core-pipeline.md, INDEX.md}` (Checklist: 154/154).
- **Realtime Transport & Driver Refactor (2026-09-02):** Extracted shared WebSocket reconnect harness (`services/realtime/transport/{connection.rs, health.rs, mod.rs}`) with single-FIFO `OutboundCommand` channel, eliminating wire-ordering framing races; decomposed `gemini_live.rs` (1061 LOC) and `deepgram_live.rs` (827 LOC) into modular `ProviderDriver` drivers under `providers/gemini/` and `providers/deepgram/` (all files ≤ 312 LOC); enforced monotonic turn ID SSOT by removing provider-level `fetch_add` bypass; zero clippy warnings, 100% release test suite green (34/34 passed).
- **Playback Triggers & Realtime Stream Guard (2026-09-02):** Aligned `PlaybackStarted` and `PlaybackFinished` across all 4 domains in `domain_event_matrix.md`; implemented `PlaybackEngine::flush_pre_roll()` to eliminate short-utterance (<250ms) deadlocks; introduced tuned `REALTIME_PREROLL_THRESHOLD_SAMPLES` (80ms vs 250ms) for low-latency S2S; guarded realtime multi-packet playback against network jitter cutoffs via in-flight `pending_synthesis_jobs` lifecycle until `LlmFinished`. Zero clippy warnings.
- **Comprehensive Backend Audit (2026-09-02):** 8-sprint domain-bounded audit (154 files, ~32,700 LOC) → `docs/plans/phase10/audits/{audit-core-engine.md, audit-audio-vad.md, audit-stt-tts.md, audit-llm.md, audit-realtime.md, audit-memory.md, audit-ipc-pipeline.md, audit-persistence-monitoring.md, INDEX.md}`. Found 12 critical findings (VAD hot-path allocations, LLM `max_output_tokens` ignored, SSE unbounded buffer, thread leak, memory pipeline blocking inference + panic swallowing, Edge TTS nested runtime block) and 20 high findings (silent error swallowing at 15+ sites, `unwrap()` in src/ at 14 sites, blocking I/O on tokio at 5 sites, unbounded channels at 3 sites). 4 false positives eliminated via feedback-review pass. (Checklist: 154/154).
- **4-Domain Event Pipeline & Lifecycle Refactor (2026-09-02):**[event-domain-matrix.md](file:///home/addy/projects/apps/vox/docs/specs/event-domain-matrix.md): Unified and decomposed pipeline event routing across all 4 interaction domains (Modular Passive, Modular PTT, Realtime Passive, Realtime PTT) under single-FIFO elevated router (`pipeline/router.rs`) and modular handlers (`pipeline/handlers/{session, speech, transcript, llm, playback, ptt, interrupt, error}.rs`); direct actor-to-actor audio hot-path streaming; centralized monotonic turn ID SSOT; fully encapsulated barge-in in `on_interrupt`; 100% zero clippy warnings across all targets.
- **Frontend Pipeline Alignment & Home Controls Refactor (2026-09-02):** Aligned frontend interaction state machine and Home page controls across all 4 domains: single Engage button for `Idle`, dynamic Pause/Play toggle + Disengage for `PASSIVE`, Mic hold-to-talk + Disengage for `PTT`, and single Reconnect button (calling `resume_session` for Stage 1 error recovery) with detailed action banner for `Error`; fixed `VoiceSessionContext::resume` to support resumption from `Error` state and filtered `owner: "Dictation"` transitions; extracted zero-hardcoding copy to `src/data/homeCopy.ts`; 100% clean TypeScript typecheck and Vite production build.
- **Unified Audit — Single Source (2026-09-02):** Re-read all 8 sprint audits + pipeline `handlers/` refactor at HEAD vs `specs/event-domain-matrix.md` + logic SSOT (ignored stale `plans/`). Consolidated into sole artifact `AUDIT_REPORT.md` (root) with own taxonomy: 16 Real Bugs / 14 Edge Cases / 12 Guards; downgraded dictation/test_clip ephemeral `turn_id` to Low; removed 8 sprint files + `audits/INDEX.md` + `audits/` dir; single-FIFO router, barge-in, warm-pause, streaming bypass all verified.
- **Audit Defect Remediations & Audio Hot-Path Allocations Purge (2026-09-02):** Executed full `/feedback-review` of `AUDIT_REPORT.md` and remediated confirmed bugs and performance bottlenecks: wired `max_output_tokens` enforcement across embedded LLM generation (R1); added `MAX_SSE_BUFFER_BYTES` bounds check (R2); moved embedding inference to `spawn_blocking` with strict candidate query error propagation (R4, R5, R9); added 30s timeout on Edge TTS MP3 collection (R6) and 180s timeout on remote LLM transport (R7); added queue count error checks (R8); logged malformed JSON on Gemini/Deepgram handshakes (R10); made settings snapshot poison-safe (R11); hardened archive parent paths (R12); implemented zero-allocation recycle pools for VAD 62.5Hz passthrough and partial STT streaming (R14, R15); avoided buffer copies in playback ingestion (E1); bounded CPAL input buffers (E2); added 10s reconnect timeout (E9); chunked SQLite IN queries $\le 400$ (E4) and wrapped queue enqueue in transactions (E8); used `mem::take` for voice recording buffers (P11); unified multi-threaded runtime in LLM worker actor. Zero clippy warnings across all targets.