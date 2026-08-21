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
  - `output_mode`: `Paste` (Simulated keystroke injection — Linux X11: `Ctrl+V`, Linux Wayland: `Ctrl+V` best-effort, macOS: `Cmd+V` via `MacOsInputAdapter`, Windows: `Ctrl+V` via `WindowsInputAdapter` — all with clipboard backup & 350ms restore), `Clipboard` (Clipboard copy only), and `Tray` (Desktop floating HUD window).
- **Zero Idle RAM & Lazy Warming**: In PTT mode, 0 ONNX models loaded on boot; `DictationController` lazily initializes audio/STT pipeline on-demand when the hotkey is triggered. The Tray HUD webview window is dynamically destroyed when Dictation or Tray output mode is inactive, and lazily re-created on-demand via `ensure_tray_window` when Tray output mode is selected or Vox Live is invoked.
- **Transliteration & Recovery Invariants**: Spoken Hindi/Devanagari text is transliterated before output dispatch across all modes. Last transcript is cached in `AppState.dictation_last_transcript` for recovery (`get_last_dictation_transcript`, `copy_last_dictation_transcript`).
- **UI System**: `InteractionCard.tsx` provides clean layman switching between `Assistant` and `Dictation` views, mounting `DictationConfigDesk.tsx` with a matching minimal ribbon header, full-width extending SVG connector arrow, underline tabs (`Paste | Clipboard | Tray`), and hotkey trigger rebinding.
- **Dictation Serialization & Error Safeguards**: Dictation interaction mode normalized to `"passive" | "ptt"` matching Rust `serde(rename_all = "snake_case")`. ErrorBoundary covers all critical interactive canvases (`HistoryStage`, `MemoryGraphCanvas`, `LiquidChamber`, `TrayAppContent`, `MemoryProfilerTabs`). `DictationConfigDesk` features interactive keyboard shortcut recording with Liquid Space aesthetics.
- **Cascading Subsystem Power & Clean Tray Sync**: When Voice Typing (`dictation.enabled`) is turned off or output mode is non-Tray, child controls are dimmed and the system tray `Vox Live` menu item is strictly disabled (`set_enabled(dictation.enabled && output_mode == Tray)`). The menu item is only clickable when dictation is enabled and output mode is set to Tray, preventing accidental auto-enabling.

### 5.3 Memory Management, WebGL Annealing & Drawer Stacking Invariants
- **Global Drawer Portal Mounting**: `Drawer.tsx` with `position="global"` renders via React `createPortal(..., document.body)`, escaping any parent CSS `contain: "layout style"` isolation (such as `<main>` in `ResponsiveLayout.tsx`). This allows the transparent drawer body design to float cleanly over `<EdgeNav />` (`z-50`) without z-index clipping.
- **Markdown Fast-Path Rendering**: `DetailPanel.tsx` uses memoized `TurnBubble` with plain text regex fast-path (`/[*_#`\[\]]/`), bypassing heavy ReactMarkdown AST initialization on session select and eliminating UI click latency.
- **Memory Graph Physics Settlement & Zero Expansion Drift**: `MemoryGraph.tsx` runs the original natural topology physics (`alpha = 0.08`, `repulsion = 1200`, `springLength = 85`, `damping = 0.85`) during initial layout settlement (`ticks < 100`). Once settled, physics calculation and WebGL Float32 instance buffer uploads halt completely, freezing nodes in their natural organic constellation equilibrium and eliminating continuous WebKit GPU allocations and expansion drift with zero UI changes.
- **Cross-Platform Heap Trimming (`trim_heap`)**: On model eviction (`unload_all_onnx_models`, `unload_memory_pipeline_onnx_models`) and audio engine shutdown (`stop_engine`), `trim_heap(caller)` is called via a unified function in `services/memory/mod.rs` with platform-specific branches: Linux → `libc::malloc_trim(0)` (glibc arena release), Windows → `EmptyWorkingSet(GetCurrentProcess())` via raw FFI (working set trim, zero new deps), macOS → intentional no-op (libmalloc is self-managing; `malloc_zone_pressure_relief` is a private symbol and must not be called).
- **`enigo` Platform Feature Scoping**: The `x11rb` feature for `enigo` is scoped to `[target.'cfg(target_os = "linux")'.dependencies]` only. macOS and Windows use `enigo` with `default-features = false` and no extra features (both are supported natively by enigo 0.2's default build).

### 5.4 Heavy UI Component Optimization, Dynamic On-Demand Webviews & Profiler Attribution
- **100% Dynamic On-Demand Webviews (Tray + Wizard)**: Removed static `"tray"` and `"wizard"` window definitions from `tauri.conf.json`. Both Tray HUD and Setup Wizard WebKitGTK processes are constructed strictly on demand (`crate::tray::ensure_tray_window`, `crate::wizard::ensure_wizard_window`) and destroyed/closed when inactive, saving ~490MB combined RAM on cold boot.
- **Accurate Profiler WebView Process Attribution**: In `src-tauri/src/ipc/memory_profiler.rs`, `get_profiler_snapshot` inspects actual existing window handles (`has_main`, `has_tray`, `has_wizard`) to eliminate false attribution of auxiliary WebProcesses as the Tray HUD.
- **MemoryGraph Re-Armed Organic Simulation & Zero Freeze**: Re-arms physics layout settlement (`isSettledRef.current = false`, `ticksRef.current = 0`, `setIsLayoutStable(false)`) whenever topology `nodes` or `edges` props update. Sized GPU instance buffers to generous static capacities (`maxNodes = 10000`, `maxEdges = 20000`) and pre-warms before relaxing smoothly into organic equilibrium at 0 FPS idle.
- **Memory Profiler Snapshot Feedback & Disk Persistence**: `useMemoryProfiler` tracks `lastManualSnapshot` (filename, timestamp, RAM) and renders a live confirmation badge in `ProfilerDrawer.tsx` header (`temp/<timestamp>-<page>.jsonl`) with button loading animation and browser console logging.
- **O(1) Centroid Badges & State Throttling**: Projected cluster centroid updates use a pre-indexed `nodeById` Map and `Set.has` check for O(1) cross-relation analysis, throttled to max ~8Hz (`lastBadgeUpdateRef` delta >= 120ms) to prevent 60 FPS React `setState` overhead.
- **Zero-Teardown Theme Switching**: Removed `isLightMode` from WebGL scene setup effect dependencies. Theme toggling dynamically updates line material opacity, node instance colors, and badge palettes in-place without destroying WebGL contexts, canvas, or controls.
- **LiquidChamber 30 FPS Throttling**: Wave animation loop throttled to 30 FPS with frame interval tracking, cutting canvas drawing CPU usage by 50% while running at 0 FPS when closed or hidden.
- **Unified Waveform Loop**: Eliminated competing duplicate `requestAnimationFrame` loops in `LiveWaveform.tsx` for synthetic processing waves and idle fade; unified all rendering inside `useDynamicFPS`.
- **Layout-Thrash-Free CSS Profiling**: Replaced synchronous `window.getComputedStyle(el)` scan across 300 DOM elements in `sampleCSSIndicators()` with non-blocking CSS class/attribute selectors.
- **AmbientBackground Middle-Ground Optimization**: Optimized atmospheric background to 2 organic blobs (`42vmax` and `36vmax`) with `will-change: transform` compositor acceleration and relaxed idle polling (600ms).
