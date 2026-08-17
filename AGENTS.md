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

### 5.4 Monitoring & History 3D Chamber View
- **Monitoring** (`Monitoring.tsx`): always-mounted popover but **zero work at idle** — polling + the LiquidChamber canvas loop are gated by visibility/`document.hidden`; `getContext("2d")` hoisted out of the rAF. Subcomponents in `shared/components/monitoring/`.
- **History = 3D Acoustic Chamber Orbit, no WebGL/Three.js** (WebGL atom scrapped — canvas re-rasterization spikes memory and idle CPU). Pure `orbitMath.ts` (35 tests): wide perspective ellipse projection (tilt compression `0.42`), viewport-dynamic radius (`280–560px`), slot-based capacity (`6–12`), depth blur mapping on distant cards (`filter: blur(...)`), and hardware-accelerated CSS 3D transforms. Cards positioned **imperatively onto refs** (dirty-checked style cache, zero re-renders/frame) in a **self-stopping rAF** (drag/momentum only, ~1.6s, then dies). Zero idle CPU.
- **Concentric Resonance Tracks** (`ChamberOrbitRings.tsx`): Lightweight SVG concentric tracks with bottom-front neon spotlight arc filter (<1MB RAM, 0% CPU).
- **Session Hub Core** ([`CentralClockNode.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/CentralClockNode.tsx)): Replaces passive clock with an informative Session Hub — features integrated top `DAY | MONTH` segmented toggle, perimeter tick marks, dual-tone date hero (`AUG 12`) in Day mode or full uppercase month name (`AUGUST`) in Month mode, weekday indicator, session & memory breakdown counters directly beneath the date hero, and active time-window range (`SPAN 08:15 AM – 10:42 PM`). Stack is bounded to the central safe circular area (`w-[74%] h-[74%]`) with zero edge clipping and inner arc padding (`r=46`).
- **History List View** ([`HistoryListView.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/HistoryListView.tsx)): Streamlined single-row header featuring inline `HISTORY` title, active date badge, and date navigation buttons on the same line; top floating title in [`History.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/History.tsx) is gated to Orbit View to avoid duplicate titles.
- **Window model**: count-based chunking labeled by actual coverage (`07:12 – 11:48`, `1–12`); chevrons step windows then roll over dates. Locale-independent `YYYY-MM-DD` keys.
- **History Architecture**: Decoupled into [`useHistory.ts`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/useHistory.ts) (state, IPC, window chunking, dial math, clamped pagination) and [`History.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/History.tsx) (declarative layout <240 lines). Mobile fallback isolated into [`HistoryListView.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/HistoryListView.tsx).
- **Direct Ambient Stage Invariant**: The 3D chamber mounts directly on the fluid page root (`relative flex-1 flex flex-col h-full w-full bg-transparent`) with zero artificial nested stage boxes or background borders. Stage is vertically aligned with the Home orb (`top: calc(50% - 36px)` / `transform: translate(-50%, -50%)`).
- **DetailPanel Vertical Resizability**: Features an interactive top drag-handle bar supporting live pointer dragging between `35%` and `85%` vh (default `62%`), double-click toggle to full expansion, `will-change: scroll`, and layout containment `[contain: content]` on turn elements.


