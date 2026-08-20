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
- **Zero Idle RAM & Lazy Warming**: In PTT mode, 0 ONNX models loaded on boot; `DictationController` lazily initializes audio/STT pipeline on-demand when the hotkey is triggered. The Tray HUD webview window is dynamically destroyed when Dictation or Tray output mode is inactive, and lazily re-created on-demand via `ensure_tray_window` when Tray output mode is selected or Vox Live is invoked.
- **Transliteration & Recovery Invariants**: Spoken Hindi/Devanagari text is transliterated before output dispatch across all modes. Last transcript is cached in `AppState.dictation_last_transcript` for recovery (`get_last_dictation_transcript`, `copy_last_dictation_transcript`).
- **UI System**: `InteractionCard.tsx` provides clean layman switching between `Assistant` and `Dictation` views, mounting `DictationConfigDesk.tsx` with a matching minimal ribbon header, full-width extending SVG connector arrow, underline tabs (`Paste | Clipboard | Tray`), and hotkey trigger rebinding.
- **Dictation Serialization & Error Safeguards**: Dictation interaction mode normalized to `"passive" | "ptt"` matching Rust `serde(rename_all = "snake_case")`. ErrorBoundary covers all critical interactive canvases (`HistoryStage`, `MemoryGraphCanvas`, `LiquidChamber`, `TrayAppContent`, `MemoryProfilerTabs`). `DictationConfigDesk` features interactive keyboard shortcut recording with Liquid Space aesthetics.
- **Cascading Subsystem Power & Clean Tray Sync**: When Voice Typing (`dictation.enabled`) is turned off or output mode is non-Tray, child controls are dimmed and the system tray `Vox Live` menu item is strictly disabled (`set_enabled(dictation.enabled && output_mode == Tray)`). The menu item is only clickable when dictation is enabled and output mode is set to Tray, preventing accidental auto-enabling.

### 5.3 Zero-Noise Testing & Dictation Benchmark Standards
- **Zero-Noise Testing Policy (`code-style-guide.md`)**: Banned trivial tests that only verify struct field assignments, derive serde roundtrips, or isolated local mutexes. Integration tests in `tests/` must validate real contracts, state machines, platform adapter resolutions, and error fallback recovery.
- **End-to-End Dictation Benchmark (`dictation_bench.rs`)**: Replaced shallow microbenchmarks with a CLI-driven tool (`--mode`, `--clip`, `--engine`, `--transliterate`) that ingests real audio WAV clips, runs acoustic STT inference, measures per-stage and $T_{\text{e2e}}$ latency, and verifies physical OS output dispatch (clipboard readback, keystroke fallback).

### 5.4 Design-System Compliance Sweep (impeccable critique)
- **Type floor enforced app-wide**: all sub-11px text raised to the 11px floor (History hero clamp reduced to `clamp(28px,3.2vw,36px)`); `ds-scan.mjs` reports **0 findings** across 147 files.
- **Semantic + glass tokens implemented in `index.css`**: `:root` and `[data-theme='light']` now define the full `docs/design.md` palette — `--success/-error/-danger/-warning/-warn-soft/-info/-violet/-pink/-muted/-muted-soft` and glass `--glass-tint/-surface/-deep/-navy`, `--ghost`. `.orbit-card-surface`/`-selected` tokenized. Components use `rgb(var(--danger))` etc. instead of `red-500/400` classes.
- **Motion scoped to ambient/live layers**: continuous `animate-pulse`/`animate-ping`/`animate-spin` removed from static functional chrome (missing-model chips, restore confirm, update pill, wizard error, error toasts, incognito lock); retained only on live-activity indicators (recording, pipeline stage dots, loading skeletons) and decorative ambient loops in `index.css` (blobs, wave-bars, ripple rings).
- **Tooltips**: native `title` attributes replaced with the custom `Tooltip` component (`SegmentedControl.tsx`, `DetailPanel.tsx`); `resizeHint` copy added to `data/historyCopy.ts`.
- **Icon language**: `⚠️` emoji replaced with lucide `AlertCircle` in `ModelStatusOverlay.tsx`.
- **Terminology**: EdgeNav nav label unified to "Settings" (was "System") to match the Settings page header. Wizard step titles are already uppercase via the `WizardHeader` `uppercase` class — no copy change needed.
- **font-mono saturation**: removed from non-data chrome labels (Home kicker labels, CPU-mode pill, pipeline tab, TTS voice-status); retained on genuine data readouts (memory bytes, timestamps, metric values, inputs). Graph-collection hex (`docs/design.md` sanctioned data-viz palette) retained; node-card surface hex tokenized to `rgba(var(--glass-navy),0.98)`.
- **Detector false-clean fix** (`.agents/skills/impeccable/.../design-system.mjs`): `findDesignRoot` now treats a project-boundary marker as non-terminal when the parent carries a `DESIGN.md`, so target-rooted walks resolve `docs/design.md` instead of stopping at `app/package.json`. Preserves the sibling-project no-inherit guard (parent with no design still stops).

### 5.5 Gesture Contract — Unified Overlay Grammar (design.md §13)
- **Single Escape authority**: `shared/lib/overlayStack.ts` (global FILO registry: `registerOverlay`/`closeTopmost`/`getStackSize`) installed once in `App.tsx` via `installOverlayStack()`; capture-phase `keydown` (Escape → pops topmost) + `pointerdown` (outside-click → dismisses topmost). All per-surface Escape listeners removed.
- **Shared bottom drawer**: `shared/ui/Drawer.tsx` (backdrop, resize handle, double-click expand, focus restore, `footer`, `position="page"|"global"`) backs all Tier 2 surfaces. `shared/hooks/useOverlay.ts` registers a surface on `active`, unregisters on close.
- **Surfaces converted**: History detail (`DetailPanel` → `Drawer position="global"`, `z-60` above `EdgeNav` `z-50`, transparent body by design); Memory pipeline → **horizontal left-to-right stepper** inside a global drawer (was vertical top-to-bottom; first card-based conduit redesign rejected as unreadable — final: single-accent numbered nodes on a continuous connector line, red reserved for the Review stage, counts below each node); Memory profiler → converted from `/memory-profiler` route to a global `ProfilerDrawer` (lazy sampling starts on open, persists across cycles; opened via `openProfiler()` handle from the bottom-left HUD + Monitoring popover; `pages/MemoryProfiler.tsx` deleted).
- **Toggle triggers**: History detail + Memory pipeline drawer re-click closes (`setDrawerOpen((v) => !v)`, `handleSelectSession` toggles).
- **Tier 1 popovers** (Memory node tooltip, Home test-clip menu, Monitoring popover) registered with the stack for Escape; Monitoring keeps a local outside-click (anchor-excluding), Settings cards collapse via local Escape (mirrors outside-click FILO pop).
- **Modal.tsx deleted** (dead file) — not reintroduced. `/memory-profiler` Route + lazy import removed from `App.tsx`.

### 5.6 Memory Management, WebGL Annealing & Drawer Stacking Invariants
- **Global Drawer Portal Mounting**: `Drawer.tsx` with `position="global"` renders via React `createPortal(..., document.body)`, escaping any parent CSS `contain: "layout style"` isolation (such as `<main>` in `ResponsiveLayout.tsx`). This allows the transparent drawer body design to float cleanly over `<EdgeNav />` (`z-50`) without z-index clipping.
- **Markdown Fast-Path Rendering**: `DetailPanel.tsx` uses memoized `TurnBubble` with plain text regex fast-path (`/[*_#`\[\]]/`), bypassing heavy ReactMarkdown AST initialization on session select and eliminating UI click latency.
- **Memory Graph Physics Settlement & Zero Expansion Drift**: `MemoryGraph.tsx` runs the original natural topology physics (`alpha = 0.08`, `repulsion = 1200`, `springLength = 85`, `damping = 0.85`) during initial layout settlement (`ticks < 100`). Once settled, physics calculation and WebGL Float32 instance buffer uploads halt completely, freezing nodes in their natural organic constellation equilibrium and eliminating continuous WebKit GPU allocations and expansion drift with zero UI changes.
- **Linux Heap Trimming (`malloc_trim`)**: On model eviction (`unload_all_onnx_models`, `unload_memory_pipeline_onnx_models`) and audio engine shutdown (`stop_engine`), `libc::malloc_trim(0)` is invoked on Linux to return free memory pages from glibc arenas back to the OS.
- **On-Demand Memory Profiler**: Periodic passive background polling (`setInterval`) is replaced with single initial capture on open + explicit manual trigger (`captureSnapshot()`) from the "Snapshot Now" button.


