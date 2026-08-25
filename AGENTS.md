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

## 2.1 Benchmark & Latency Execution Rules (MANDATORY)

1. **NEVER RUN BENCHMARK PROBES IN PARALLEL**:
   - Running multiple GGUF or ONNX inference commands concurrently causes CPU thread contention and invalidates per-pair latency metrics.
   - Always execute benchmark probes **strictly sequentially, one model at a time**.

2. **NEVER RUN BENCHMARKS OR EVALUATION SCRIPTS IN DEBUG MODE**:
   - Debug builds (`dev` profile without `--release`) omit SIMD vectorization, ONNX graph optimizations, and LTO, producing invalid latency metrics (up to 7x slower).
   - Always execute evaluation scripts and benchmarks using `--release` mode (e.g. `cargo run --release --example <eval_name>`).

---

## 3. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
>
> - **WRITE TASK (Adding/editing code, refactoring, fixing bugs):** You MUST read `.agents/rules/code-style-guide.md` AND the relevant role rule file (e.g. `.agents/rules/backend-engineer.md` or `frontend-engineer.md`) BEFORE modifying code.
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

### 5.1 Architecture & Design Specifications (SSOT)
All state machines, routing topologies, data flows, and IPC schemas are documented in:
- **Backend Orchestration & Routing SSOT:** [`docs/plans/phase10/pipeline_orchestration_spec.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/pipeline_orchestration_spec.md) (7 canonical states, 4 pipeline domains, silence gating, barge-in, ownership rules).
- **Frontend Integration SSOT:** [`docs/plans/phase10/frontend_orchestration_spec.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/frontend_orchestration_spec.md) (Discrete IPC action mapping, root `VoiceSessionContext`, mode-adaptive UI controls).
- **Core Architecture & Feature Ledgers:**
  - [`docs/features/backend.md`](file:///home/addy/projects/apps/vox/docs/features/backend.md) (Domain modules, actor lifecycle, audio streaming).
  - [`docs/features/voice-flow.md`](file:///home/addy/projects/apps/vox/docs/features/voice-flow.md) (Modular Passive, Modular PTT, Realtime Passive, Realtime PTT, Dictation).
  - [`docs/features/models.md`](file:///home/addy/projects/apps/vox/docs/features/models.md) (STT/LLM/TTS models manifest & tier allocation).
  - [`docs/features/frontend.md`](file:///home/addy/projects/apps/vox/docs/features/frontend.md) (Component layout & interaction states).

---

### 5.2 Refactor Standards & Invariants
1. **Thin IPC Adapters (`ipc/pipeline/assistant.rs` & `dictation.rs`):** IPC handlers are pure 1-line dispatchers. Zero business logic in IPC files.
2. **Soft 50-Line Function Cap & Docstrings:** Functions must not exceed 50 lines without documented justification. Exactly one function-level `///` docstring per function. Zero per-line body comments.
3. **No Toggle Functions:** Discrete single-purpose functions only (e.g. separate `start_session()` and `end_session()`).
4. **Canonical Mutex Lock Order:** Strictly acquire `state.engine` before `state.realtime_engine`. Lock inversion is banned.
5. **No Silent Error Swallows or Fallbacks:** Zero `let _ = tx.send(...)`. All channel sends and fallible operations must log warnings on error or propagate.
6. **Zero Lint Suppressions & Clean Signatures:** `#[allow(...)]` is strictly banned. Parameter lists with >5 arguments are bundled into typed structs. Masking unused parameters with `_` is banned except for genuine RAII drop guards (`_stream`, `_log_guard`, `_thread_handle`).

---

### 5.3 Completed Refactor Summary
- **Domain Modules:** Decoupled legacy God loop into dedicated domain handlers (`modular_passive.rs`, `modular_ptt.rs`, `realtime_passive.rs`, `realtime_ptt.rs`, `dictation.rs`) driven by a central non-blocking `router.rs`.
- **Decoupled Actors & Lifecycle:** Extracted `services/audio/engine.rs`, `services/vad/actor.rs`, `services/llm/actor.rs`, and `services/tts/actor.rs` with dedicated warm-up/cool-down lifecycles.
- **Frontend State Alignment:** Aligned all TypeScript stores, hooks, and UI components to canonical 7 states (`Idle`, `Listening`, `UserSpeaking`, `Thinking`, `AssistantSpeaking`, `Paused`, `Error`) with non-toggle discrete Tauri IPC commands.
- **Code Quality Baseline:** Clean slate on testing; zero `#[allow(...)]` suppressions; zero dead `_` fields; 100% clean compilation on `cargo clippy --all-targets` (0 warnings) and `pnpm build`.


