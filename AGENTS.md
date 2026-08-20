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

### 5.1 Voice Pipeline & Test Invariants
- **Deadlock prevention**: `engage()` drops the `state.engine` lock before calling `stop_engine()`.
- Guarded by `pipeline_lifecycle_invariants_test.rs` (15/15) + `useHomePage.test.ts` (9/9) — `handleEnd` routes to `testClipCancel()`/`engage()`/`stopRealtimeSession()`.
- No unbuffered stderr prints from `edge_tts.rs` (IPC spam).

### 5.2 Decoupled Realtime Dictation Subsystem (Phase 9)
- **Decoupled Architecture**: Dictation backend is decoupled from Tray UI (`services/dictation/`). `InteractionOwner::Dictation = 0` is a first-class citizen.
- **Two Independent Axes**:
  - `interaction_mode`: `Passive` (Continuous) vs `Ptt` (Push-To-Talk via global hotkey `Alt+Space`).
  - `output_mode`: `Paste` (Simulated keystroke injection via X11/Wayland with clipboard backup & 350ms restore), `Clipboard` (Clipboard copy only), and `Tray` (Desktop floating HUD window).
- **Zero Idle RAM & Lazy Warming**: In PTT mode, 0 ONNX models loaded on boot; `DictationController` lazily initializes audio/STT pipeline on-demand when the hotkey is triggered.
- **Transliteration & Recovery Invariants**: Spoken Hindi/Devanagari text is transliterated before output dispatch across all modes. Last transcript is cached in `AppState.dictation_last_transcript` for recovery (`get_last_dictation_transcript`, `copy_last_dictation_transcript`).
- **UI System**: `InteractionCard.tsx` provides clean layman switching between `Assistant` and `Dictation` views, mounting `DictationConfigDesk.tsx` with a matching minimal ribbon header, full-width extending SVG connector arrow, underline tabs (`Paste | Clipboard | Tray`), and hotkey trigger rebinding.
- **Dictation Serialization & Error Safeguards**: Dictation interaction mode normalized to `"passive" | "ptt"` matching Rust `serde(rename_all = "snake_case")`. ErrorBoundary covers all critical interactive canvases (`HistoryStage`, `MemoryGraphCanvas`, `LiquidChamber`, `TrayAppContent`, `MemoryProfilerTabs`). `DictationConfigDesk` features interactive keyboard shortcut recording with Liquid Space aesthetics.

### 5.3 Zero-Noise Testing & Dictation Benchmark Standards
- **Zero-Noise Testing Policy (`code-style-guide.md`)**: Banned trivial tests that only verify struct field assignments, derive serde roundtrips, or isolated local mutexes. Integration tests in `tests/` must validate real contracts, state machines, platform adapter resolutions, and error fallback recovery.
- **End-to-End Dictation Benchmark (`dictation_bench.rs`)**: Replaced shallow microbenchmarks with a CLI-driven tool (`--mode`, `--clip`, `--engine`, `--transliterate`) that ingests real audio WAV clips, runs acoustic STT inference, measures per-stage and $T_{\text{e2e}}$ latency, and verifies physical OS output dispatch (clipboard readback, keystroke fallback).
