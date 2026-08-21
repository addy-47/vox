# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: Automatic Documentation & AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK DOCUMENTATION HOOK (NON-NEGOTIABLE):**
> Every time code, architecture, candidate thresholds, system prompts, or LLM judge models are modified, or a task/phase is completed:
> 1. You **MUST** automatically update `AGENTS.md` to reflect the exact current implementation, model configuration, and threshold matrix. 
> 2. You **MUST** automatically update any relevant feature, component, design, or architecture documentation in docs/ to match the actual code state, key files include `backend.md`, `models.md`, `frontend.md`and `docs/features/*`.
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
> Full specification: [`docs/features/dictation.md`](docs/features/dictation.md)
- **Decoupled Architecture**: Dictation backend is decoupled from Tray UI (`services/dictation/`). `InteractionOwner::Dictation = 0` is a first-class citizen with two independent axes: `interaction_mode` (`passive` continuous vs `ptt` hotkey `Alt+Space`) and `output_mode` (`paste` keystroke injection via platform adapters with clipboard backup, `clipboard`, and `tray` HUD window).
- **Zero Idle RAM & Lazy Warming**: In PTT mode, 0 ONNX models loaded on boot; audio/STT pipeline initializes on-demand when the hotkey triggers. Tray HUD and Setup Wizard WebViews are dynamically constructed strictly on-demand and destroyed when inactive (~490MB RAM saved on cold boot).
- **Transliteration & Recovery**: Hindi/Devanagari text transliterated before dispatch; last transcript cached in `AppState.dictation_last_transcript`.
- **UI & Cascading Sync**: `InteractionCard.tsx` provides clean switching to `DictationConfigDesk.tsx`. When Voice Typing is disabled or output is non-Tray, child controls dim and system tray `Vox Live` menu item is disabled (`dictation.enabled && output_mode == Tray`).

### 5.3 Performance, Memory Management & WebGL Annealing
> Full specification: [`docs/features/performance-memory-optimizations.md`](docs/features/performance-memory-optimizations.md)
- **Global Drawer Portal Mounting**: `Drawer.tsx` (`position="global"`) renders via `createPortal(..., document.body)` to escape CSS layout containment and float above `<EdgeNav />` (`z-50`).
- **Markdown Fast-Path Rendering**: `DetailPanel.tsx` uses memoized plain text regex fast-path (`/[*_#`\[\]]/`), bypassing heavy ReactMarkdown AST initialization.
- **Memory Graph Physics Settlement & 0 FPS Equilibrium**: Layout physics calculates during initial settlement (`ticks < 100`) and halts completely once stable, freezing nodes in natural equilibrium to eliminate WebKit GPU allocations and expansion drift. Re-arms dynamically on graph prop updates.
- **Cross-Platform Heap Trimming (`trim_heap`)**: Eviction hooks invoke platform-specific heap trimming: Linux $\to$ `libc::malloc_trim(0)`, Windows $\to$ `EmptyWorkingSet(GetCurrentProcess())`, macOS $\to$ no-op.
- **UI Throttling & Profiler**: `LiquidChamber` throttled to 30 FPS; centroid badge updates throttled to ~8Hz with $O(1)$ node lookup; theme switching executes zero-teardown WebGL color remapping. Memory Profiler WebProcess attribution inspects active window handles (`has_main`, `has_tray`, `has_wizard`).

### 5.4 LLM Capability Probing, Provider Differentiation & Settings Decoupling
> Full specification: [`docs/backend.md#43-llm--language-model`](docs/backend.md#43-llm--language-model)
- **Unified Single LLM Model**: Local GGUF, local GPU servers (Ollama/vLLM), and Cloud APIs share identical universal controls: Response Limit (`Voice Concise ~300`, `Conversational ~1,000`, `Native Full`), Creativity (`Precise 0.2`, `Balanced 0.7`, `Creative 1.0`), and Context Window.
- **Zero-Guessing Context Transparency**: Transparently displays context window: selectable RAM budget (`2k`, `4k`, `8k`, `16k`) for local models, or `Provider Managed (Full Capacity)` for cloud endpoints with zero artificial client-side clamping.
- **Streaming Capability Probe Engine (`capability_probe.rs`)**: Uses SSE streaming to measure true TTFT and TPS, validates structured JSON tool calling schema (`lookup_user`), and normalizes URLs (`resolve_chat_url`) to support root and `/v1` endpoints.
- **Runtime Token Smoke Validator (`validate_llm_token_cap`)**: 1-token smoke probe catching HTTP 400 server ceiling errors with 1-click auto-clamping.
- **Flat Underline Sub-Tabs & Decoupling**: Organized into non-scrollable flat tabs (`Performance`, `Tokens & Context`, `Creativity`) in `LlmSettingsView.tsx`, decoupled from `LlmCatalogView.tsx` (`fzf` fuzzy search).



