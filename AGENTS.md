# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: Automatic Documentation & AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK DOCUMENTATION HOOK (NON-NEGOTIABLE):**
> Every time code, architecture, candidate thresholds, system prompts, or LLM judge models are modified, or a task/phase is completed:
> 1. You **MUST** automatically update `AGENTS.md` to reflect the exact current implementation, model configuration, and threshold matrix. 
> 2. You **MUST** automatically update any relevant feature, component, design, or architecture documentation to match the actual code state.
> 3. This is a **mandatory post-task completion hook** — do NOT wait for the user to explicitly remind you to sync documentation.

---

## 1. Project Context

Vox is a **realtime voice AI desktop app** (Tauri v2 / Rust / TypeScript). Constraint: 8GB RAM, CPU-first inference, sub-200ms perceived pipeline latency.

**Crate structure:** Single Rust library crate `vox_lib` at `app/src-tauri/`. `main.rs` is 1 line. `lib.rs` is module declarations + Tauri assembly only. All logic lives in modules.

---

## 2. Workspace Directory Map

| Path | Purpose | Rules |
|---|---|---|
| `app/src-tauri/src/` | Purpose Rust source | No test logic. No benchmarks. |
| `app/src-tauri/tests/` | Integration tests (`cargo test --tests`) | Named `<feature>_test.rs`. Tests public API only. |
| `app/src-tauri/benches/` | Performance benchmarks (`cargo test --benches`) | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`. |
| `app/src-tauri/examples/` | Utility CLI tools (`cargo run --example <name>`) | Standalone tools. No `#[test]`. No assertions. |
| `.agents/rules/` | Role-specific agent instruction files | Read relevant file before acting in that role. |
| `docs/plans/` | Architecture specs and phase plans | Source of truth for specs. Do not contradict. |
| `docs/features/` | Implemented feature ledgers | Update after completing features. |
| `sandbox/` | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`. |
| `temp/` | Ephemeral runtime files: logs, raw LLM outputs | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned. |
| `submodules/` | Git submodules | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`. Do not edit directly. |
| `~/.vox/models/` | Local model weights | Canonical manifest: `~/.vox/models/models_manifest.json`. |

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
> - **WRITE TASK (Adding/editing code, refactoring, fixing bugs):** You MUST read `.agents/rules/code-style-guide.md` AND the relevant role rule file (e.g. `.agents/rules/backend-engineer.md` or `frontend-engineer.md`) BEFORE modifying code.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.

---

## 4. Agent Roles

| Role | Rule File | Scope |
|---|---|---|
| System Architect | `.agents/rules/system-architect.md` | Strategy, gates, plan approval |
| Backend Engineer | `.agents/rules/backend-engineer.md` | `app/src-tauri/src/` implementation |
| Frontend Engineer | `.agents/rules/frontend-engineer.md` | `app/src/` implementation |
| QA Engineer | `.agents/rules/qa-engineer.md` | Test audit, benchmark validation |
| ML Research Engineer | `.agents/rules/ml-research-engineer.md` | ML model research, evaluation, and fine-tuning dataset curation |
| Test Engineer | `.agents/rules/test-engineer.md` | Test case design, benchmark validation, and performance analysis |

---

## 5. Recent Work & Critical System Invariants

### 5.1 Architecture & Performance Invariants
- **UI & 3D Graph**: Banned sci-fi jargon and nested pills in HUD. Memory graph renders all 10,000+ nodes in 1 `InstancedMesh` GPU call (<15MB RAM) with stable refs across prop updates.
- **ONNX Lifecycle & Zero Idle RAM**: 0 ONNX models loaded on boot. Evict pipeline sessions (`unload_all_onnx_models()`) on barge-in (`PipelineActive`), disengage, or batch completion.
- **ModernBERT Edge Triggering**: Bidirectional candidate evaluation (`has_inter_collection_relationship`) enforcing canonical order `[Source] [SEP] [Target]` before inference.
- **Benchmark Rules**: Sequential runs only (4 CPU threads, release mode). Evaluated objectively via programmatic QA boolean parsers without inner-loop sampler allocation.

### 5.2 Default Local Conversational LLM (Qwen3.5-0.8B)
- **Model Registration**: `qwen_3_5_0_8b` (Q4_K_M GGUF, 508MB) in `~/.vox/models/llm/qwen/` registered in `manifests/models_manifest.json` and `defaults.rs`.
- **Sampling & Pipeline**: Non-thinking ChatML template (`<|im_start|>...`) with `presence_penalty=2.0`, `repetition_penalty=1.0`, `top_k=20`, `temperature=1.0`.

### 5.3 Voice Pipeline Lifecycle & Test Invariants
- **Concurrency & Deadlock Prevention**: `engage()` disengage drops `state.engine` lock before calling `stop_engine()`, preventing async mutex deadlock.
- **State Machine Integration Tests**: `pipeline_lifecycle_invariants_test.rs` (15/15 tests) guards state transitions, test clip cancel contracts, and owner handoffs.
- **Frontend Hook Contracts**: `useHomePage.ts` explicitly routes `handleEnd` to `testClipCancel()`, `engage()`, or `stopRealtimeSession()` (covered in `useHomePage.test.ts`).
- **Telemetry & Audio Logs**: Cleaned up unbuffered `edge_tts.rs` stderr prints to prevent IPC spam.

