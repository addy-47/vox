# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: Automatic Documentation & AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK DOCUMENTATION HOOK (NON-NEGOTIABLE):**
> Every time code, architecture, candidate thresholds, system prompts, or LLM judge models are modified, or a task/phase is completed:
>
> 1. You **MUST** automatically update `AGENTS.md` to reflect the exact current implementation, model configuration, and threshold matrix.
> 2. You **MUST** automatically update any relevant feature, component, design, or architecture documentation in docs/ to match the actual code state, key files include `backend.md`, `models.md`, `frontend.md`and `docs/features/*`.
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

## 2.1 Execution & Testing Invariants (All Agents)

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

## 3. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
>
> - **WRITE TASK (Backend Rust):** You MUST read `.agents/rules/backend-style-guide.md` AND `.agents/rules/backend-engineer.md` BEFORE modifying Rust backend code.
> - **WRITE TASK (Frontend React/TS):** You MUST read `.agents/rules/frontend-style-guide.md` AND `.agents/rules/frontend-engineer.md` BEFORE modifying frontend code.
> - **WRITE TASK (Tests/Benches/Evals):** You MUST read `.agents/rules/testing-style-guide.md` AND `.agents/rules/test-engineer.md` BEFORE authoring tests or benchmarks.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.

---

## 4. Agent Roles

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

> **Chronological Storyline:** UT Layer & Spec $\to$ Backend/Frontend Discovery $\to$ Uncalled Code Purge $\to$ STT Consolidation $\to$ 188-Sprint Review $\to$ Subsystem Decoupling $\to$ Turn ID / PTT Boundaries $\to$ LLM Engine Consolidation $\to$ Memory 4-Pillar Refactor $\to$ Pipeline Seam Hardening.

### 5.1 Initial Test Suite & Architectural Discovery
- Built unit test suite and authored Integration Test Spec (`docs/plans/phase10/integration_test_spec.md`, Seams 1–8).
- Mutation testing revealed widespread dead code, uncalled methods, and tangled audio/LLM routing across backend and frontend.
- Paused Seams 9–14 to execute a foundational codebase overhaul.

### 5.2 Backend Refactor & Uncalled Functions Resolution
- **Decoupled Pipelines:** Extracted `services/pipeline/modular/` and `realtime/` orchestrated via central `router.rs` (`VoxEvent` pump).
- **Uncalled Code Resolution:** Wired 11 critical paths (`prepare_turn_context`, opportunistic compaction, monotonic turn IDs, transliteration) and purged 30 dead functions across 7 deleted legacy files.
- **Quality Gate:** 45 tests across 9 binaries green via `cargo-nextest --release --test-threads=1`; 0 clippy warnings.

### 5.3 Frontend Standardization & Dead Code Purge
- **7-State Alignment:** Unified UI across 7 canonical states (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`) with standardized `vad_backend` configuration.
- **Cleanup:** Purged 26 unused listeners/services via `knip`. All 10 suites (98 tests) and `pnpm build` verified green.
- **Ledger:** [`docs/features/performance-memory-optimizations.md`](file:///home/addy/projects/apps/vox/docs/features/performance-memory-optimizations.md) and [`docs/frontend.md`](file:///home/addy/projects/apps/vox/docs/frontend.md).

### 5.4 STT Streaming Benchmark & Engine Consolidation
- **Harness:** Built 256-sample streaming benchmark CLI (`app/src-tauri/benches/stt_bench.rs`) evaluating 10 canonical audio clips.
- **Sherpa-ONNX 1.13.6:** Standardized all STT/VAD/TTS on `sherpa-onnx 1.13.6` using multilingual Nemotron-3.5 transducer (`0.497x RTF`, `97.1% accuracy`, `~71MB RSS`), completely removing `parakeet-rs`.

### 5.5 188-Sprint Second-Pass Review & Architectural Specs
- Authored and completed all 188 implementation sprints across 11 modules (`docs/plans/phase10/backend_review_sprints.md`).
- Established 5 standalone SSOT architecture specs: LLM/TTS Streaming, LLM Provider Consolidation, Monotonic Turn IDs, Memory `<user_profile>` Assembly, and Audio Ownership.

### 5.6 Subsystem Decoupling & Engine Lifecycle Migration
- **VAD 3-Role Decoupling:** Restricted `VadActor` to generic modes (`ContinuousSegmentation`, `WindowedValidation`, `StreamPassthrough`) with zero upward pipeline imports; extracted telemetry and math utilities.
- **Engine Relocation:** Moved application lifecycle orchestration (`VoxEngine`, startup/shutdown) to `core/engine.rs`, scoping `services/audio/` strictly to CPAL streams and playback draining.
- **Actor Decoupling:** Isolated dictation hotkeys, output routing, audio suppression atomics, and async TTS voice reference resolution.

### 5.7 Turn ID Synchronization & PTT Boundary Trimming
- **Monotonic Turn IDs:** Enforced atomic fetch-and-add increment across all PTT/dictation start events, eliminating `turn_id: 0` resets.
- **Speech Boundary Trimming:** `VadCommand::StartWindowValidation` / `StopWindowValidation` trims audio strictly to `[speech_start..speech_end]`, automatically discarding silence/accidental clicks with 0 STT/cloud emissions.
- **Jitter Buffer:** Added 250ms (12,000 samples @ 48kHz) pre-roll buffer in playback engine before opening audio output.

### 5.8 LLM Consolidation & Empirical Capability Discovery
- **Unified Engine (`services/llm/`):** Unified `ConnectionConfig` mapping 13 standard providers to unified `RemoteTransport` (streaming SSE line decoder, `/chat/completions`, `/responses`, `/api/chat`) and in-process `EmbeddedProvider` (`llama.cpp`).
- **Empirical Micro-Probing (`probe.rs`):** Replaced static catalog guessing with live tool schema and multilingual streaming TTFT/TPS micro-probes; purged heuristic token floor assumptions.

### 5.9 Memory Spec Consolidation & 4-Pillar Architecture Spec
- Consolidated memory requirements into a definitive 2-in-1 spec (`docs/plans/phase10/memory_formatting_context_assembly_spec.md` v2.0).
- Locked 4-pillar design: **Harness** (buffering, accounting, prompt building), **Retrieval** (waterfall search, scope classification), **Compaction** (async summarization), and **Ingestion** (4-stage offline queue pipeline).

### 5.10 Memory 4-Pillar Implementation & Pipeline Refactor
- Restructured `services/memory/` across 24 modular files conforming to the 4-pillar layout.
- Decomposed `ConversationManager` into `buffer.rs`, `accountant.rs`, `prompt_builder.rs`, `manager.rs`, and the unified `prepare_turn_context` public facade.
- Locked SSOT timing split: Critical inline compaction (`>= 0.85`), opportunistic soft compaction (`0.65 <= util < 0.85` in `{Ready, Paused}` with 20s debounce), and background queue ingestion (30s idle).

### 5.11 Pipeline Memory Seams & Quality Hardening
- **W1 (Pre-Compaction Filler):** Dispatches TTS transition filler before executing critical compaction, removing dead silence.
- **W2 (Cached LLM Provider):** Cached active `Arc<dyn LlmProvider>` in `AppState`, eliminating per-turn disk I/O and ORT reload during compaction.
- **W3 & R3 (Realtime & Fact Dispatch):** Guarded realtime turns on engagement/pause, routed compaction facts through `PersonalFactsReady` worker channel, and offloaded SQLite writes.
- **R2 & R4 (Event Router & Latencies):** Fixed `router.rs` so only `PlaybackFinished`/`Cancelled` emit `PipelineIdle`, preventing ingestion during active generation; wired real STT/TTFT metrics to `TurnCompleted`.
- **Quality Gate:** Clean `cargo clippy --all-targets` (0 warnings), clean `cargo check --all-targets` (0 errors), 40/40 tests green in release mode.

