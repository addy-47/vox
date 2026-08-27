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

## 2.1 Sequential Execution & Build Mode (All Agents)

These rules apply to any long-running task — benchmarks, evaluations, inference probes, test suites, or any task that measures or depends on resource-sensitive performance.

1. **Run long-running tasks sequentially, one at a time.** Concurrent execution causes resource contention (CPU threads, memory bandwidth, I/O) that corrupts timing and accuracy measurements. Complete each task fully before starting the next.

2. **Use optimized builds for any performance-sensitive task.** Unoptimized or debug builds omit critical compiler and runtime optimizations, producing metrics that are invalid by up to an order of magnitude. Any task whose output informs a performance-based decision must run in release or optimized mode.

3. **Tests requiring external API keys (Nvidia, Gemini, Deepgram, OpenAI, ElevenLabs, etc.) MUST be annotated with `#[ignore]` by default.** They must never run in the automated test loop. They require explicit user approval before running. To run them manually, load credentials from `temp/.env` and execute with `cargo test -- --ignored`.

4. **Explicit Thread Pool Allocation for Inference Commands (Critical Pitfall):** Subshells spawned in automated agent environments do not inherit terminal thread defaults, causing ONNX Runtime, OpenMP, and Rayon to fall back to single-core execution (1 core instead of all available cores). Always prefix test and benchmark execution commands with explicit thread allocation:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo test --test <test_file> --release -- --nocapture --test-threads=1
   ```

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

## 5. Current Phase 10: Architecture & Orchestration Refactor

### 5.1 Architecture & Specifications (SSOT)

- **Integration Test Plan:** [`docs/plans/phase10/integration_test_spec.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/integration_test_spec.md) (Seams 1–8 integration matrix).

---

### 5.2 Core Refactor Standards & Invariants
1. **Thin IPC Adapters:** IPC handlers are pure 1-line dispatchers. Zero business logic.
2. **Soft 50-Line Cap & Docstrings:** Max 50 lines per function. Exactly one `///` docstring per function. Zero body comments.
3. **Discrete Actions:** No toggle functions (`start_session()` / `end_session()`).
4. **Mutex Lock Order:** Acquire `state.engine` strictly before `state.realtime_engine`.
5. **No Silent Swallows:** Zero `let _ = tx.send(...)`. All errors logged or propagated.
6. **Zero Suppressions & Testability Seams:** Zero `#[allow(...)]`. Generics `<R: tauri::Runtime>` on all actors/routers for `MockRuntime` testing. Audio ingestion seams and engine/sender injection across all domains.

---

### 5.3 Completed Work Summary
- **Domain Modules & Lifecycle Extraction:** Decoupled God loop into `modular_passive.rs`, `modular_ptt.rs`, `realtime_passive.rs`, `realtime_ptt.rs`, and `dictation.rs` with central `router.rs` pump and decoupled actor lifecycles (`audio`, `vad`, `llm`, `tts`).
- **Frontend 7-State Realignment:** Converted all stores, hooks, and UI components to canonical 7 states (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`) with non-toggle discrete IPC.
- **Backend Testability Seams (Seams 2, 3, 6, 8):** Added `ingest_audio`, buffer inspectors, and fallback sender/engine injection (`handle_ptt_stop_with_sender`, `handle_ptt_stop_with_engine`, `handle_hotkey_release_with_sender`) across Modular PTT, Realtime PTT, and Dictation to decouple live audio/network hardware in tests.
- **Runtime Generics (`<R: tauri::Runtime>`):** Generified `pipeline`, `router`, `modular_ptt`, `realtime_ptt`, `modular_passive`, `realtime_passive`, `dictation`, `output_router`, `llm::actor`, `tts::actor`, and `tray` to support Tauri `MockRuntime` in test harnesses.
- **VAD & STT Queue Fixes:** Fixed unbuffered audio dropping in VAD actor PTT mode; routed frames to domain buffers with speech boundary detection for ghost audio gating; fixed `drain_reset_stream` in `stt/actor.rs` to preserve pending `SttCommand::Final` items.
- **Backend Style Guide Enhancement:** Added Section 9 (Testability Seams, Inversion of Control & Runtime Generics) in `.agents/rules/backend-style-guide.md` to prevent mock/hardware blockers in future implementations.
- **Integration Test Suite Delivery (Seams 1–8):** Built, audited, and verified 6 integration test suites (`passive_streaming_test.rs`, `modular_ptt_test.rs`, `vad_ducking_test.rs`, `dictation_ptt_test.rs`, `realtime_ptt_test.rs`, `tts_test.rs`, and `llm_test.rs`) covering all 8 seams. Extracted test harness & helpers into `tests/common/` (`harness.rs`, `audio.rs`, `scoring.rs`, `paths.rs`).
- **Quality & Verification Baseline:** 49 passing tests (33 unit tests + 16 integration tests in release mode), 0 clippy warnings (`cargo clippy --all-targets`), 1.0000 STT similarity, and verified zero-leak resource lifecycles across VAD, STT, LLM, TTS, and Realtime engines.



