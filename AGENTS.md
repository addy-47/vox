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
- **Typography**: Display = `Sora`, Body/UI = `DM Sans`, Telemetry = `JetBrains Mono`. Font floor `>= 11px`. All user-facing copy is layman (no STT/LLM jargon; HUD pills read Thinking/Hearing/Speaking).
- **Tooltip**: `app/src/shared/ui/Tooltip.tsx` is the only sanctioned tooltip. Native `title` attrs are banned as tooltips.
- **ONNX / Zero Idle RAM**: 0 ONNX models loaded on boot; evict pipeline sessions on barge-in, disengage, or batch completion.
- **Memory graph**: 10,000+ nodes in 1 `InstancedMesh` GPU call (<15MB RAM).
- **Benchmarks**: sequential runs only (4 CPU threads, release mode), no inner-loop sampler allocation.
- **ModernBERT edge triggering**: bidirectional candidate eval enforcing canonical `[Source] [SEP] [Target]`.

### 5.2 Default Local LLM (Qwen3.5-0.8B)
`qwen_3_5_0_8b` (Q4_K_M GGUF, 508MB) in `~/.vox/models/llm/qwen/`, registered in `models_manifest.json` + `defaults.rs`. Non-thinking ChatML template, `presence_penalty=2.0`, `top_k=20`, `temperature=1.0`.

### 5.3 Voice Pipeline & Test Invariants
- **Deadlock prevention**: `engage()` drops the `state.engine` lock before calling `stop_engine()`.
- Guarded by `pipeline_lifecycle_invariants_test.rs` (15/15) + `useHomePage.test.ts` (9/9) — `handleEnd` routes to `testClipCancel()`/`engage()`/`stopRealtimeSession()`.
- No unbuffered stderr prints from `edge_tts.rs` (IPC spam).

### 5.4 UI Systems & View Invariants
- **Monitoring** (`Monitoring.tsx`): Zero work at idle; polling and canvas loops gated on `!document.hidden`. Subcomponents in `shared/components/monitoring/`.
- **History Orbit** (`OrbitCarousel.tsx`, `CentralClockNode.tsx`, `useHistory.ts`): CSS 3D perspective ellipse projection with imperative card positioning on refs. Self-stopping rAF loop (<1000ms momentum), quantized depth blurs (0/1.5/3px), 6 discrete z-index bands, and `no-blur` class toggled during rotation for 60fps drag. Session Hub disc features transparent frosted glassmorphism in light mode and dark obsidian in dark mode, segmented Day/Month toggle, date hero, and counter breakdowns. Mobile falls back to `HistoryListView.tsx`.
- **Monitoring & Liquid Chamber** (`Monitoring.tsx`, `LiquidChamber.tsx`): Zero work at idle; polling and canvas loops gated on `!document.hidden`. Liquid chamber 2D physics canvas dynamically computes transparent glass background, fluid gradients, specular reflections, and HUD pill cards based on active `data-theme` (light vs dark) via `MutationObserver`. Water level responds dynamically to real Vox RAM allocation (150MB baseline up to 3.5GB models loaded) and CPU load.
- **DetailPanel**: Vertically resizable between 35% and 85% vh via direct imperative ref dragging with commit on pointer up.
- **Memory Graph** (`MemoryGraph.tsx`, `MemoryNodeTooltip.tsx`, `SearchBar.tsx`): Single-pass `InstancedMesh` (10k nodes) and `LineSegments` (20k edges) with `frustumCulled = false` for seamless panning. Clamped pan boundaries (`maxPan = radius * 1.5`), dynamic zoom ceiling ($Z_{\text{max}} \approx R \times 2.165$ enforcing $\ge 80\%$ vh scale), crisp cluster badges and cards using `bg-[rgba(var(--card),0.92-0.96)]` with zero `backdrop-filter` over the 3D canvas (eliminating compositor Gaussian blur halos), and top-right `+` / `-` zoom dock.
- **Settings & Skeletons** (`Settings.tsx`, `SettingsCardSkeleton.tsx`, `settingsStore.ts`): Single uniform compact global skeleton card (`lg:w-[440px] min-h-[220px]`) with universal header, tab pill bar, 2-column card grid, and controls shimmer layout. `isDomainDirty` performs property-specific comparison avoiding phantom save footer triggers from background server capability probes or cosmetic metadata.
- **Boot Lifecycle & Window Reveal**: Dark theme injected into HTML head; `App.tsx` coordinates window reveal on double-rAF after initial setup resolution with smooth opacity fade-in.

### 5.5 UI Memory Attribution & Diagnostic Profiler
- **WebGL Teardown**: `MemoryGraph.tsx` guarantees explicit `.geometry.dispose()`, `.material.dispose()`, and `scene.clear()` on unmount before `renderer.dispose()`.
- **Node Position Cache**: Controlled via `clearCacheOnUnmount` prop (defaults to `false` to preserve spatial coordinates across navigation for UX).
- **Process-Tree Attribution**: `ipc/memory_profiler.rs` (`get_profiler_snapshot`) provides synchronous, non-blocking on-demand RSS measurement across Main Process, Main WebView, Tray WebView, and Network processes.
- **Diagnostic UI & Accuracy Floor**: `/memory-profiler` displays live per-route `Baseline → Peak → Retained` deltas, DOM node counts, loaded fonts, and component lifecycle traces with strict, honest accuracy labeling (`Measured`, `Estimated`, `Correlated`, `Unattributed`).



