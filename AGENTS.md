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

## 5. Current Phase 10: IPC & Services Layer Refactor (Spec-First, 3-Layer Execution)

> **Phase status:** Phase 10 Layer 1 Audit & Behavioral Specification Complete (100% — 572 of 572 functions audited and classified into Production / Bad Code / Disaster). Ready for Layer 2 Spec-Driven Architecture Refactor.
> **SSOT for Behavioral Architecture:** [`docs/plans/phase10/pipeline_orchestration_spec.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/pipeline_orchestration_spec.md) (All routing, 7 canonical states, 6 domain pipeline step-by-step function inventories, ownership rules, and button orchestration live strictly in the spec).
> **Checklist Tracker:** [`docs/plans/phase10/function_inventory_checklist.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/function_inventory_checklist.md) (572 of 572 functions audited).

---

### 5.1 Refactor Standards & Invariants

1. **Thin IPC Adapters (`ipc/pipeline/assistant.rs` & `dictation.rs`):** IPC handlers are pure 1-line dispatchers. Zero orchestration or business logic in IPC files.
2. **Soft 50-Line Function Cap & Docstrings:** Functions must not exceed 50 lines without documented justification. Exactly one function-level `///` docstring per function. Zero per-line body comments.
3. **No Toggle Functions:** Discrete single-purpose functions only (e.g. separate `start_session()` and `end_session()`).
4. **Canonical Mutex Lock Order:** Strictly acquire `state.engine` before `state.realtime_engine`. Lock inversion is banned.
5. **No Silent Error Swallows or Fallbacks:** Zero `let _ = tx.send(...)`. All results must log warnings on failure or propagate errors. No fallback chains (`if path A fails try path B`).
6. **No Polling Where Events Suffice:** `PlaybackEngine` emits `PlaybackStarted`/`PlaybackDrained` events; eliminate `recv_timeout(150ms)` polling in `event_loop.rs`.
7. **Zero Domain Duplication in Rules:** `AGENTS.md` defines rules, standards, and workflow only. All domain-level state machine details and routing rules live in `pipeline_orchestration_spec.md`.

---

### 5.2 File Quality Categories & Triage Criteria

| Category | Definition | Action |
|---|---|---|
| **Production** | Clean structure, single responsibility, ≤50 line cap respected, single function docstring | Preserved as-is; reference baseline |
| **Cleanup** | Good structure, isolated violations (per-line comments, minor line overflow, missing docstring) | Fixed immediately in Layer 1 and promoted to Production |
| **Bad Code** | God functions, boundary leakages, repeated inline settings/owner checks, lock inversions | Flagged in checklist; queued untouched for Layer 2 refactor |
| **Disaster** | Unmaintainable multi-concern God files (e.g. 1145-line `run_event_loop()`, 17KB mixed `setup.rs`) | Flagged in checklist; queued untouched for Layer 2 rewrite |

---

### 5.3 Three-Layer Execution Workflow

The refactor executes in 3 distinct, gated layers:

#### Layer 1: Checklist Audit & Quick-Win Promotion (File by File)
- Walk through the 572 functions across `ipc/` and `services/`
- For every file audited:
  1. Explain in plain English what the file does and what each function does.
  2. If the file/function is in the **Cleanup** tier, fix it immediately (strip per-line comments, add docstrings, extract minor helpers) and promote it directly to **Production**.
  3. If the file/function is **Bad Code** or a **Disaster**, leave the code completely untouched. Classify it in the checklist and queue it for Layer 2.
- **End State of Layer 1:** Every function in the 572-function checklist is cleanly partitioned into only 3 states: `Production`, `Bad Code`, or `Disaster`.

#### Layer 2: Spec-Driven Architecture Refactor (Bad Code & Disaster Sprints)
- Refactor all `Bad Code` and `Disaster` components directory by directory.
- Backend implementation is strictly driven by and gated on the finalized behavioral specification ([`docs/plans/phase10/pipeline_orchestration_spec.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/pipeline_orchestration_spec.md)).
- Deconstruct God files into dedicated domain modules (`modular_passive.rs`, `modular_ptt.rs`, `realtime_passive.rs`, `realtime_ptt.rs`, per-event loop handlers).

#### Layer 3: System Wiring, Polish & Legacy Purge
- Wire thin IPC adapters (`orchestration.rs`) to the new domain handlers.
- Purge all obsolete shims, dead code, and legacy paths.
- Perform comprehensive end-to-end regression validation.

---

### 5.4 Applied Hotfixes & System Health
- **Audio Device Resolution Unification (`services/audio/device.rs` & `ipc/audio.rs`):** Removed hardcoded ALSA host override on Linux in `device.rs` to unify with `cpal::default_host()`, ensuring PipeWire/PulseAudio devices (including headsets) match correctly. Implemented and registered `list_output_devices` IPC command.
- **Frontend Voice Session Persistence (`shared/context/VoiceSessionContext.tsx`):** Lifted voice session state (`interactionState`, `isEngaged`, `dialogueHistory`, `pttStatus`, Tauri event listeners) to a root `<VoiceSessionProvider>` in `App.tsx`. Prevents unmounting and UI state resets when navigating between `/`, `/settings`, `/history`, and `/memory`.

