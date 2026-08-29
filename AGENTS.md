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

## 5. Current Phase 10: Architecture & Orchestration Refactor

> **Status:** Phase 10 is in progress, all major refactor work is complete. The two SSOT specs are `docs/plans/phase10/integration_test_spec.md` (Seams 1–14) and `docs/plans/phase10/wiring_memory_pipeline_refactor_spec.md` (Sprints 01–44 + Special).

### 5.1 How Phase 10 Started 

- Built the **unit test suite** for Vox (UT layer).
- Authored the **Integration Test Spec** (`docs/plans/phase10/integration_test_spec.md`, Seams 1–8) and implemented those integration tests.
- Ran **mutation testing** on the suite — results in `docs/benchmarks/test_suite_bench.md`.
- Deferred seams 9-14 for later due to the discovery of uncalled functions and tangled backend code and frontend code.
- Discovered the backend was in **horrible shape** (uncalled functions, dead code, tangled audio/LLM routing).
- Refactored the **full backend** per `docs/plans/phase10/wiring_memory_pipeline_refactor_spec.md`.
- Refactored the **frontend** (see §5.4 and `docs/features/performance-memory-optimizations.md`).

### 5.2 Backend Refactor

The `wiring_memory_pipeline_refactor_spec.md` is the checklist; this subsection summarizes the completed work.

- **Decoupled Pipeline Architecture:** Domain modules fully decoupled under `services/pipeline/modular/` (`context.rs`, `passive.rs`, `ptt.rs`) and `services/pipeline/realtime/` (`session.rs`, `passive.rs`, `ptt.rs`), orchestrated by central `router.rs` (`spawn_router` / `route_event`) and thin IPC dispatchers (`ipc/pipeline/assistant.rs`). Deleted flat files: `services/audio/router.rs`, `services/ptt.rs`, `services/dictation/controller.rs`, `services/utils.rs`, `core/metrics.rs`, `services/llm/{capabilities,probe}.rs`, `utils/bench_reporter.rs`.
- **Central Router + VAD-actor PTT routing:** cpal audio → `VadActor` (OS thread) → domain `ingest_audio`. **No AudioRouter thread** exists.
- **Capability Probing SSOT:** `services/llm/capability_probe.rs` replaces deleted `probe.rs` + `capabilities.rs`.
- **Dynamic Memory Retrieval & Working Memory:** `classify_scope` (ModernBERT) → `generate_embedding` (MiniLM) → `retrieve_personal_context` (Turso hybrid SQL + vector + BFS graph) → `ConversationManager::assemble_system_prompt` (`<user_profile>` injection) → opportunistic background compaction on `vox-memory-worker`.
- **Uncalled Functions Resolution (Sprints 01–44 + Special) — COMPLETE:**
  - **11 Retained & Wired:** `set_max_context_tokens`, `load_identity_into_system_prompt`, `new_session`, `update_system_prompt`, `push_assistant_turn`, `build_context` (dynamic memory retrieval + non-blocking filler queue), `try_trigger_opportunistic`, `commit_opportunistic`, `barge_in` (client + server interrupts), `transliterate_if_hi`, `stitch_transcripts`.
  - **5 Eval / Test Seams:** `l2_normalize`, `set_speech_detected`, `is_speech_detected`, `fetch_intra_subfloor_candidates`, `fetch_inter_subfloor_candidates` relocated to `evals/` or converted to black-box event assertions.
  - **30 Dead Code Purges (7 files deleted):** `services/utils.rs`, `services/dictation/controller.rs`, `services/audio/router.rs`, `services/llm/capabilities.rs`, `services/llm/probe.rs`, `core/metrics.rs`, `utils/bench_reporter.rs`.
- **Quality Gate:** 45 tests across 9 binaries green via `cargo-nextest --release --test-threads=1`; clean `cargo clippy --all-targets` (0 warnings).

### 5.4 Frontend Refactor — Summary

- **7-State Alignment & UI Standardization:** Unified state machine (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`), standardized canonical `vad_backend` field across IPC and store, redesigned navigation/cards (`LlmConfigDesk`, `MemoryConfigDesk`, `TtsVoiceManager`), calibrated responsive viewport layouts.
- **Dead-Code Purge:** Cleaned 26 uncalled listeners, unused primitives, and stale services; `knip` `lint:dead` wired. 10 test suites (98 tests) + `pnpm build` green.
- **Ledger:** [`docs/features/performance-memory-optimizations.md`](file:///home/addy/projects/apps/vox/docs/features/performance-memory-optimizations.md) and [`docs/frontend.md`](file:///home/addy/projects/apps/vox/docs/frontend.md).

### 5.5 STT Streaming Benchmark & Engine Consolidation — Summary

- **Streaming Benchmark Runner (`app/src-tauri/benches/stt_bench.rs`):** CLI harness (`--model`, `--clip`, `--input-dir`, `--output-dir`, `--min-similarity`) simulating 256-sample streaming through `VadActor` $\to$ `SttWorker` across 10 canonical test clips, persisting reports to `benches/results/stt_bench/<run_id>/report.json` + `latest.json`.
- **Sherpa-ONNX 1.13.6 Consolidation:** Replaced `parakeet-rs` with `sherpa-onnx` `OnlineRecognizer` using multilingual Nemotron-3.5 transducer (`0.497x RTF`, `97.1% accuracy` [EN: 99.0%, HI: 95.2%], `~71 MB RSS`, 10/10 clips passing `>= 0.90` gate). Removed `parakeet-rs` crate; all STT, TTS, and VAD standardized on `sherpa-onnx 1.13.6`.
- **Ledger:** [`docs/benchmarks/stt_benchmark.md`](file:///home/addy/projects/apps/vox/docs/benchmarks/stt_benchmark.md).



