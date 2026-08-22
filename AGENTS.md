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

### 5.1 Voice Pipeline & Subsystems
- **Deadlock prevention**: `engage()` drops `state.engine` lock before calling `stop_engine()`. Guarded by `pipeline_lifecycle_invariants_test.rs` (15/15) + `useHomePage.test.ts` (9/9).
- **Decoupled Dictation (Phase 9)**: Independent `interaction_mode` (`passive` vs `ptt`) and `output_mode` (`paste`, `clipboard`, `tray`). On-demand ONNX warming and HUD WebViews (~490MB RAM saved on boot). Transliteration + transcript cache in `AppState`.
- **Realtime Voice Desk**: Unified interaction ribbon (`RealtimeConfigDesk`), 2-column balanced grid with provider memory, and auto-engine warming on launch.

### 5.2 Performance, Memory & Glass Rendering
- **Global Drawer Portal**: `Drawer.tsx` (`position="global"`) portals to `document.body` above `EdgeNav` (`z-50`).
- **Physics Settlement & 0 FPS**: Graph physics freezes after settlement (`ticks < 100`) to eliminate WebKit GPU allocations. `LiquidChamber` throttled to 30 FPS.
- **Heap Trimming**: Platform-specific hooks (`malloc_trim` on Linux, `EmptyWorkingSet` on Windows).
- **Settings Store Reactivity & Auto-Save**: Granular selectors eliminate fan-out; direct CSS variable buffering for color pickers; hybrid auto-save lifecycle with 600ms debounced auto-sync + auto-hiding "Changes Saved" toast for Hot/WorkerCommand settings, and an explicit "Apply & Reload" restart gate for heavy model/compute swaps that safely discards unapplied drafts on navigation.

### 5.3 Settings Architecture, Topology & Input Design
- **13 Canonical Domains**: Zero alias shims across Rust, TypeScript, and UI (`audio`, `vad`, `stt`, `llm`, `tts`, `realtime`, `interaction`, `dictation`, `history`, `appearance`, `memory`, `persona`, `system`).
- **Parallel Provider Memory**: `stt`, `llm`, `tts`, and `realtime` maintain independent state (`embedded`, `server`, `cloud`) across switches.
- **Persistent Capability Cache**: Atomic probes stored at `~/.vox/cache/model_capabilities.json` with unified left-side tooltip trigger.
- **Unified Glass Desk Standard**: Shared `rounded-xl p-3 border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)]` surface container applied across `ModelsCard`, `InteractionCard`, `RealtimeCard`, `LlmSettingsView`, and `HistoryCard`.
- **ToggleTile & Rotary Standardization**: Unified `ToggleTile.tsx` and `RotaryKnob.tsx` across `MemoryCard`, `InteractionCard`, and `HistoryCard`.
- **Underline Input & Carousel Standardization**: Unified `UnderlineInput.tsx`, `ApiKeyField.tsx`, and borderless/backgroundless `CarouselSelector.tsx` across all credential, URL, provider, and value fields.
- **Model Hub Hierarchy & Scrolling**: Clean flat workspace hierarchy in `ModelsCard`; responsive vertical scrolling (`custom-scrollbar`) across `AuxiliaryWorkspace` (Support), `AsrWorkspace`, `TtsModelWorkspace`, and `VadWorkspace`.
- **LlmSettings & Category Ribbon Polish**: Non-scrollable flat sub-tabs (`Compute`, `Tokens`, `Context`, `Temp`), baseline-aligned `CategorySelector` chevrons/connector, clean typographic status indicators (no badge backgrounds/borders), and natural top-to-bottom vertical rhythm grouping.
