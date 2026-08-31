# 📄 `performance-memory-optimizations.md` — Vox Performance & Memory Optimization Ledger

> **Scope**: All memory and performance optimizations applied across the Vox codebase.  
> **Platforms**: Linux (primary), Windows, macOS.  
> **Last updated**: 2026-08-31.

---

## 1. Backend (Rust) Optimizations

### 1.1 On-Demand WebView Process Creation (Tray + Wizard)

**Problem**: Tauri v2's static `windows[]` config in `tauri.conf.json` spawned both the Tray HUD and Setup Wizard WebKitGTK/WebView2 processes unconditionally at cold boot, consuming ~490MB combined RAM even when neither feature was in active use.

**Fix**: Removed both static window definitions from `tauri.conf.json`. Both windows are now constructed strictly on demand via lazy factory functions:

- `crate::tray::ensure_tray_window(&app)` — creates the Tray HUD `WebviewWindow` if absent, returns the existing handle if already live.
- `crate::wizard::ensure_wizard_window(&app)` — same pattern for the setup wizard.

Both are destroyed (`.close()`) when their owning feature is inactive:
- Tray HUD destroyed when `dictation.enabled == false` or `output_mode != Tray`.
- Wizard closed after setup completion.

**Impact**: ~490MB RAM saved on cold boot.

**Files**: [`app/src-tauri/src/tray.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/tray.rs), [`app/src-tauri/src/wizard.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/wizard.rs)

---

### 1.2 Cross-Platform Heap Trimming (`trim_heap`)

**Problem**: After evicting ONNX models, freed pages were not returned to the OS on Windows and macOS. Only Linux had `malloc_trim(0)` applied — Windows and macOS silently did nothing.

**Fix**: Unified into `pub(crate) fn trim_heap(caller: &str)` in `services/memory/mod.rs`:

| Platform | Mechanism | Notes |
|---|---|---|
| **Linux** | `libc::malloc_trim(0)` | Returns free glibc arena pages to OS immediately |
| **Windows** | `EmptyWorkingSet(GetCurrentProcess())` via raw `extern "system"` FFI | Trims working set. Zero new crate dependencies. |
| **macOS** | Intentional **no-op** | `libmalloc` is self-managing. `malloc_zone_pressure_relief` is a private symbol — must not be called. OS reclaims pages autonomously under pressure. |

Call sites:
1. `unload_memory_pipeline_onnx_models()` — embedder + NLI + edge classifier eviction
2. `unload_all_onnx_models()` — full ONNX eviction
3. `stop_engine()` — post engine thread join

**Files**: [`app/src-tauri/src/services/memory/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/mod.rs), [`app/src-tauri/src/ipc/pipeline/assistant.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/pipeline/assistant.rs) (`stop_engine`)

---

### 1.3 Accurate Memory Profiler Process Attribution & Task Filtering

**Problem**: `get_profiler_snapshot` used process-name heuristics to classify child WebView processes, producing false Tray attribution even when the Tray window was destroyed. Furthermore, `sysinfo` on Linux returned both OS processes and thread-level task entries, inflating memory counts.

**Fix**:
1. Updated profiler to query actual live window handles:
   ```rust
   let has_tray   = app.get_webview_window("tray").is_some();
   let has_wizard = app.get_webview_window("wizard").is_some();
   ```
2. Added `#[cfg(target_os = "linux")]` filter checking `proc.tasks().is_none()` before walking parent chains to exclude thread entries.

**Files**: [`app/src-tauri/src/ipc/memory_profiler.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/memory_profiler.rs), [`app/src-tauri/src/monitoring/system_monitor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/system_monitor.rs)

---

### 1.4 Windows GPU Detection (Real Probe)

**Problem**: `utils/hardware.rs` always returned `has_gpu: false` on Windows, routing Windows users to CPU-only model variants even with discrete GPUs.

**Fix**: Subprocess probe via `wmic path Win32_VideoController get Name /value` (stdlib only):
- `nvidia`, `amd` / `radeon`, `intel arc` / `intel xe` $\to$ Tier 1B (Local GPU Available)
- `microsoft basic` / `virtual` / `llvm` $\to$ Tier 1A (software renderer fallback)

**Files**: [`app/src-tauri/src/utils/hardware.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/hardware.rs)

---

### 1.5 Engine Offload on Window Hide & Crash Recovery

**Problem**: Closing the main window kept the engine running idle indefinitely. Additionally, if the renderer crashed or was closed via DevTools, the `WebviewWindow` handle disappeared, breaking "Launch Vox".

**Fix**:
- **Engine Offload on Hide**: `CloseRequested` for `"main"` hides the window and automatically triggers `stop_engine()` if `!dictation.enabled && state == Idle`.
- **Lazy Crash Recreate**: `ensure_main_window` (`src/window_main.rs`) reconstructs the window from `tauri.conf.json` when its handle is `None`. `AppState::main_window_destroyed` manages the dynamic "Restart Vox" tray recovery item.

**Files**: [`app/src-tauri/src/window_main.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/window_main.rs), [`app/src-tauri/src/core/state.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/state.rs), [`app/src-tauri/src/lib.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/lib.rs), [`app/src-tauri/src/ipc/tray.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/tray.rs)

---

## 2. Frontend (React / TypeScript) Optimizations

### 2.1 Multi-Dimensional Memory Profiling Infrastructure

Vox embeds a production memory profiler (`ProfilerDrawer`) sampling four independent memory vectors:

1. **Backend Process Tree RSS (`getProfilerSnapshot`)**: Tracks `main_process_ram_mb`, `main_webview_ram_mb`, `tray_webview_ram_mb`, `wizard_webview_ram_mb`, `other_children_ram_mb`, and `total_vox_ram_mb`.
2. **JS Heap Sampling (`sampleJSHeap`)**: Reads `window.performance.memory` (`usedMb`, `totalMb`, `limitMb`).
3. **DOM & Resource Telemetry (`sampleDOMStats`)**: Measures live DOM element count (`querySelectorAll("*").length`), font face count (`document.fonts.size`), and decoded resource footprint.
4. **CSS Compositing Layers (`sampleCSSIndicators`)**: Queries GPU blur filters (`backdropFilterCount`), compositor layers (`willChangeCount`), and `<canvas>` elements without layout-thrashing `getComputedStyle` calls.
5. **Per-Route Memory Lifecycle (`PageMemoryRecord`)**: Tracks `baseline` $\to$ `peak` $\to$ `retained` RSS per route to flag monotonic memory growth.

**Files**: [`app/src/services/memoryProfilerService.ts`](file:///home/addy/projects/apps/vox/app/src/services/memoryProfilerService.ts), [`app/src/shared/hooks/useMemoryProfiler.ts`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useMemoryProfiler.ts), [`app/src/shared/components/profiler/ProfilerDrawer.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/profiler/ProfilerDrawer.tsx)

---

### 2.2 Dynamic Frame-Rate Throttling & 0-FPS Idle States

**Problem**: Competing unthrottled `requestAnimationFrame` loops ran continuously at 60 FPS across waveforms, ambient blobs, and monitoring canvases, consuming CPU/GPU cycles even when components were scrolled out of view or tab was backgrounded.

**Fix**:
1. **`useDynamicFPS` Unified Loop**: Manages frame scheduling with active (60 FPS) and idle (15 FPS) tiers. Cancels RAF loops entirely (`rafRef.current = null`) when hidden (via `IntersectionObserver`), paused, or document-hidden. Applied to `LiveWaveform.tsx` and `AdvancedOrb.tsx`.
2. **`LiquidChamber` 30 FPS Throttle**: Throttles wave canvas drawing with `targetInterval = 1000 / 30`, achieving a ~50% CPU reduction on the monitoring draw path and 0 FPS when closed.
3. **`AmbientBackground` Dynamic Layer Demotion**: Promotes blobs to GPU layers with `will-change: transform` during active motion, but dynamically reverts to `will-change: auto` and halts ripple iterations when ambient energy reaches resting zero ($< 0.001$).
4. **Motion Reduction**: Honors `@media (prefers-reduced-motion)` by disabling non-essential decorative pulsing and wave micro-animations.

**Files**: [`app/src/shared/hooks/useDynamicFPS.ts`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useDynamicFPS.ts), [`app/src/shared/components/monitoring/LiquidChamber.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/monitoring/LiquidChamber.tsx), [`app/src/shared/components/common/AmbientBackground.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/common/AmbientBackground.tsx)

---

### 2.3 Three.js WebGL Engine Lifecycle & Zero-GC Buffer Management

**Problem**:
- Force-directed physics in `MemoryGraph` ran indefinitely, causing unbounded node drift and continuous buffer uploads.
- Theme switching triggered full WebGL context teardowns.
- Instantiating new `THREE.Object3D()` and `THREE.Color()` inside 60 FPS settlement loops generated transient heap churn and GC pauses.
- Visibility scans iterated $O(N \times M)$ edges per node tick.

**Fix**:
1. **Two-Phase Physics Settlement**: Runs physics simulation for `ticks < 100` (`alpha=0.08`, `repulsion=1200`, `damping=0.85`), then freezes all physics calculations and GPU uploads at equilibrium (0 FPS CPU/GPU idle). Re-arms only when graph data changes. Pre-allocates instance buffers (`maxNodes=10000`, `maxEdges=20000`).
2. **Zero-Teardown Material Updates**: Theme changes modify existing buffer colors, line opacities, and badge palettes in-place without destroying WebGL contexts.
3. **Three.js Object Hoisting**: Hoisted `dummyObjRef` and `colorObjRef` into `useRef` instances, eliminating all per-frame heap allocations.
4. **Adjacency Map Indexing**: Precomputed `relationAdjacencyMap: Map<string, Set<string>>` for $O(1)$ relationship checks in `isNodeVisible`.
5. **Centroid Badge Throttling**: Pre-indexed `nodeById` Map and throttled badge `setState` calls to $\le 8\text{Hz}$ ($\ge 120\text{ms}$ delta).
6. **Explicit Context Teardown**: Added `renderer.forceContextLoss()` to unmount effects in `AdvancedOrb.tsx` and `MemoryGraph.tsx`.

**Files**: [`app/src/shared/components/memory/MemoryGraph.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/MemoryGraph.tsx), [`app/src/shared/components/home/AdvancedOrb.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/home/AdvancedOrb.tsx)

---

### 2.4 State Management, Reactive Geometry & Category-Scoped Dirty Isolation

**Problem**:
- Modifications to draft configuration caused cascading re-renders across all `useSettings()` consumers.
- Whole-domain dirty checks (`isDomainDirty("models")`) caused changes in LLM drafts to trigger global Save prompts even when inspecting clean STT or VAD tabs.
- High-frequency color picker adjustments caused redundant string comparisons.

**Fix**:
1. **Context Fan-Out Elimination**: Converted `SettingsCardWrapper`, `RealtimeCard`, and settings views to fine-grained `useSettingsStore` selectors.
2. **Category-Scoped Dirty Tracking & Rollback**: Implemented `isCategoryDirty(category)` and `discardCategoryChanges(category)` in `settingsStore.ts`. Stage tabs (`Listening`, `Reasoning`, `Speaking`) in `CategorySelector.tsx` and nodes in `ModelsTopologyMap.tsx` display micro amber dirty indicators (`●`), maintaining isolation between STT, LLM, TTS, VAD, and Support models.
3. **Coalesced Appearance Color Picker**: `AppearanceCard` buffers color adjustments in local component state while writing CSS variables directly to `document.documentElement.style` (`--accent`), committing to Zustand state strictly on `pointerup`.
4. **Scalar Scope Keys**: Configured `SETTINGS_SCOPE_KEYS` with explicit key lists, bypassing whole-scope JSON serialization.
5. **Reactive Geometry Calculation**: Replaced stale closures in `useSettingsPage` with functional state updates (`setLines(prev => ...)`).

**Files**: [`app/src/store/settingsStore.ts`](file:///home/addy/projects/apps/vox/app/src/store/settingsStore.ts), [`app/src/pages/Settings.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/Settings.tsx), [`app/src/shared/components/settings/interaction/CategorySelector.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/interaction/CategorySelector.tsx), [`app/src/shared/components/settings/models/ModelsTopologyMap.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/models/ModelsTopologyMap.tsx), [`app/src/shared/components/settings/appearance/AppearanceCard.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/appearance/AppearanceCard.tsx)

---

### 2.5 Component Lifecycle Stabilization, Context Memoization & Listener Hygiene

**Problem**:
- Unmemoized context value objects in `VoiceSessionContext` and `MemoryProfilerContext` triggered re-render cascades across consumer trees on every streaming audio/token chunk (~33 Hz).
- Global keyboard event listeners were re-bound on every state tick.
- Streaming character renderers lost token catch-up state during fast LLM output bursts.

**Fix**:
1. **Context Value Memoization**: Wrapped `VoiceSessionContext` and `MemoryProfilerContext` values in `useMemo`, decoupling actions from volatile streaming counters.
2. **Stabilized Keyboard Listeners**: Bound window Space/Escape listeners strictly once on mount using mutable `kbStateRef` to read live session states.
3. **Streaming Token Catch-up Guard**: Introduced mutable `targetTextRef` in `useStreamingRenderer.ts` so RAF loops catch up to incoming token bursts without premature termination.
4. **Timer & Interval Cleanup**: Corrected nested interval lifetimes in `useMemoryProfiler.ts` and extracted timeout side-effects from `useVisibility.ts` functional updaters.
5. **Leaf Primitive Memoization**: Wrapped controls in `React.memo` and stabilized callbacks (`ToggleTile`, `SegmentedControl`, `SliderField`, `SearchInput`, `ApiKeyField`, `LiquidChamber`, `MetricCarousel`, `OrbitCarousel`).

**Files**: [`app/src/shared/context/VoiceSessionContext.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/context/VoiceSessionContext.tsx), [`app/src/shared/context/MemoryProfilerContext.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/context/MemoryProfilerContext.tsx), [`app/src/shared/hooks/useStreamingRenderer.ts`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useStreamingRenderer.ts), [`app/src/shared/hooks/useVisibility.ts`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useVisibility.ts), [`app/src/shared/ui/`](file:///home/addy/projects/apps/vox/app/src/shared/ui/)

---

### 2.6 Layout Containment, Stacking Invariants & Viewport Resilience

**Problem**:
- `<EdgeNav />` rendered before `<main contain: layout style>` in `ResponsiveLayout.tsx`, causing large settings cards in lower grid rows to visually trap pointer events and block navigation clicks.
- `Drawer.tsx` mounted inside layout-contained containers, clipping z-indexes.
- Rotary controls intercepted mouse wheel events, interfering with page scrolling.
- `MemoryNodeTooltip` overflowed viewport boundaries near screen edges.

**Fix**:
1. **DOM Stacking Order Invariant**: Fixed overlay elements (`<EdgeNav />`, `<EngineMonitor />`, and `{isSettings && <ModelStatusOverlay />}`) are explicitly rendered **after** `<main>` in the JSX tree, guaranteeing full clickability across all card activations.
2. **Global Drawer Portal Mounting**: `Drawer.tsx` with `position="global"` renders via `React.createPortal(..., document.body)` to escape layout containment.
3. **Calibrated Travel & Wheel Preservation**: Softened `RotaryKnob` travel denominator to `280px` for smooth precision and removed wheel capture to preserve native scroll.
4. **Tooltip Viewport Clamping**: Clamped `MemoryNodeTooltip` coordinates with `Math.max(16, Math.min(x, window.innerWidth - tooltipWidth - 16))` to prevent edge clipping.
5. **Markdown Fast-Path**: Pre-checks plain text with `!/[*_#`\[\]]/.test(content)` in `DetailPanel.tsx` to bypass `ReactMarkdown` AST parsing on plain turns, eliminating 80–150ms click latency.

**Files**: [`app/src/layout/ResponsiveLayout.tsx`](file:///home/addy/projects/apps/vox/app/src/layout/ResponsiveLayout.tsx), [`app/src/shared/ui/Drawer.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/ui/Drawer.tsx), [`app/src/shared/ui/RotaryKnob.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/ui/RotaryKnob.tsx), [`app/src/shared/components/memory/MemoryNodeTooltip.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/MemoryNodeTooltip.tsx), [`app/src/shared/components/history/DetailPanel.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/DetailPanel.tsx)

---

## 3. Observed Memory Behavior & Expected Baseline Shifts

### 3.1 Why RSS Doesn't Return to Cold-Boot Baseline

After the first model load $\to$ unload cycle, a ~125MB permanent baseline shift is **expected and normal**:

| Cause | ~Contribution |
|---|---|
| WebKit JIT bytecode cache | 40–60MB |
| ONNX Runtime global environment singleton | 30–40MB |
| glibc Tokio thread-pool maintenance arenas | 10–20MB |

---

### 3.2 Memory Target Matrix & Leak Triage

1. Cold boot $\to$ open profiler $\to$ manual snapshot = **baseline**
2. Load model, run a session, unload
3. Manual snapshot $\to$ check `retainedDeltaMb`
4. Repeat: if `retainedDeltaMb` grows monotonically $\to$ real leak candidate. If it stabilizes at ~125MB after the first cycle $\to$ expected runtime behavior.

---

## 4. Cross-Platform Optimization Matrix

| Optimization | Linux | macOS | Windows |
|---|---|---|---|
| **On-demand WebView (Tray + Wizard)** | ✅ | ✅ | ✅ |
| **Heap trim after model eviction** | `malloc_trim(0)` | No-op (intentional) | `EmptyWorkingSet` |
| **Process tree thread-task filter** | ✅ (Linux tasks API) | N/A | N/A |
| **GPU detection** | `/dev/nvidia0`, `/dev/dri` | Metal (always Tier 1B) | `wmic` Win32 probe |
| **ALSA noise suppression** | `ALSA_LOG_LEVEL=0` | N/A | N/A |
| **Tray HUD positioning** | GTK virtual layer + Cairo | `tauri-plugin-positioner` | `tauri-plugin-positioner` |
| **Engine offload on window hide** | ✅ | ✅ | ✅ |
| **JS heap sampling** | ✅ (WebKit may omit API) | ✅ (WebKit may omit API) | ✅ |
| **CSS compositing scan** | ✅ | ✅ | ✅ |
| **DOM node count** | ✅ | ✅ | ✅ |
| **LiquidChamber 30 FPS cap** | ✅ | ✅ | ✅ |
| **MemoryGraph physics settlement & zero drift** | ✅ | ✅ | ✅ |
| **useDynamicFPS unified RAF loop** | ✅ | ✅ | ✅ |
| **Three.js GC object hoisting** | ✅ | ✅ | ✅ |
| **Category-scoped dirty tracking & reset** | ✅ | ✅ | ✅ |
| **DOM stacking order navigation guard** | ✅ | ✅ | ✅ |
