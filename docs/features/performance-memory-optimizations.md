# 📄 `performance-memory-optimizations.md` — Vox Performance & Memory Optimization Ledger

> **Scope**: All memory and performance optimizations applied across the Vox codebase.  
> **Platforms**: Linux (primary), Windows, macOS.  
> **Last updated**: Phase 9 cross-platform pass.

---

## 1. Backend (Rust) Optimizations

### 1.1 On-Demand WebView Process Creation (Tray + Wizard)

**Problem**: Tauri v2's static `windows[]` config in `tauri.conf.json` spawned both the Tray HUD
and Setup Wizard WebKitGTK/WebView2 processes unconditionally at cold boot, consuming ~490MB
combined RAM even when neither feature was in active use.

**Fix**: Removed both static window definitions from `tauri.conf.json`. Both windows are now
constructed strictly on demand via lazy factory functions:

- `crate::tray::ensure_tray_window(&app)` — creates the Tray HUD WebviewWindow if absent,
  returns the existing handle if already live.
- `crate::wizard::ensure_wizard_window(&app)` — same pattern for the setup wizard.

Both are destroyed (`.close()`) when their owning feature is inactive:
- Tray HUD destroyed when `dictation.enabled == false` or `output_mode != Tray`.
- Wizard closed after setup completion.

**Impact**: ~490MB RAM saved on cold boot.

**Files**: `app/src-tauri/src/tray.rs`, `app/src-tauri/src/wizard.rs`

---

### 1.2 Cross-Platform Heap Trimming (`trim_heap`)

**Problem**: After evicting ONNX models, freed pages were not returned to the OS on Windows
and macOS. Only Linux had `malloc_trim(0)` applied — Windows and macOS silently did nothing.

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

**Files**: `app/src-tauri/src/services/memory/mod.rs`,
`app/src-tauri/src/ipc/pipeline/lifecycle.rs`

---

### 1.3 Accurate Memory Profiler Process Attribution

**Problem**: `get_profiler_snapshot` used process-name heuristics to classify child WebView
processes, producing false Tray attribution even when the Tray window was destroyed.

**Fix**: Updated to query actual live window handles:

```rust
let has_tray   = app.get_webview_window("tray").is_some();
let has_wizard = app.get_webview_window("wizard").is_some();
```

Attribution is gated on whether the window actually exists at snapshot time.

**Files**: `app/src-tauri/src/ipc/memory_profiler.rs`

---

### 1.4 Process Tree Filtering (Linux Thread vs Process Entries)

**Problem**: `sysinfo` on Linux returns both OS processes and thread-level task entries, inflating
memory counts when naively walked.

**Fix**: `#[cfg(target_os = "linux")]` guard filters entries where `proc.tasks().is_none()` before
walking the parent chain. Windows/macOS return only process-level entries and skip the filter.

**Files**: `app/src-tauri/src/ipc/memory_profiler.rs`,
`app/src-tauri/src/monitoring/system_monitor.rs`

---

### 1.5 Windows GPU Detection (Real Probe)

**Problem**: `utils/hardware.rs` always returned `has_gpu: false` on Windows, routing Windows
users to CPU-only model variants even with discrete GPUs.

**Fix**: Subprocess probe via `wmic path Win32_VideoController get Name /value` (stdlib only):
- `nvidia` keyword → Tier 1B (Local GPU Available)
- `amd` / `radeon` → Tier 1B
- `intel arc` / `intel xe` → Tier 1B
- `microsoft basic` / `virtual` / `llvm` → Tier 1A (software renderer)
- Unknown adapter → conservatively Tier 1B
- `wmic` fails → graceful fallback Tier 1A

**Files**: `app/src-tauri/src/utils/hardware.rs`

---

### 1.6 Engine Offload on Main Window Hide

**Problem**: Closing the main window kept the LLM/STT/TTS engine running idle indefinitely,
consuming RAM when the app was backgrounded via the tray icon.

**Fix**: The `CloseRequested` event for `"main"` hides the window (prevents close) and
automatically calls `stop_engine()` if `!dictation.enabled && !is_engaged`.

**Files**: `app/src-tauri/src/lib.rs`

---

## 2. Frontend (React / TypeScript) Optimizations

### 2.1 Memory Profiler Infrastructure

Vox ships a production-grade in-app memory profiler (`ProfilerDrawer`) accessible via a
bottom drawer. It samples four independent memory dimensions simultaneously:

#### 2.1.1 Backend Process Tree RSS (`getProfilerSnapshot`)
Calls `get_profiler_snapshot` Tauri IPC. Returns:

| Field | Description |
|---|---|
| `main_process_ram_mb` | Rust backend process RSS |
| `main_webview_ram_mb` | Main WebKit/WebView2 renderer |
| `tray_webview_ram_mb` | Tray HUD renderer (`null` when window destroyed) |
| `wizard_webview_ram_mb` | Setup Wizard renderer (`null` when window destroyed) |
| `other_children_ram_mb` | Network process + unclassified children |
| `total_vox_ram_mb` | Aggregate of all above |

Manual snapshots persist to `temp/<timestamp>-<page>.jsonl`. The drawer header shows a
`CheckCircle2` confirmation badge after each manual snapshot (`lastManualSnapshot` state).

#### 2.1.2 JS Heap Sampling (`sampleJSHeap`)
Reads `window.performance.memory` (Chromium/WebKit extension):
- `usedMb`, `totalMb`, `limitMb`
- Accuracy: `"Measured"` when API available, `"Unattributed"` otherwise
  (WebKit production builds may not expose this — tracked as a known gap)

#### 2.1.3 DOM Node & Resource Count (`sampleDOMStats`)
- `document.querySelectorAll("*").length` — live DOM node count
- `document.fonts.size` — font face count
- `performance.getEntriesByType("resource")` — loaded resource count + estimated bytes via `decodedBodySize`

#### 2.1.4 CSS Compositing Indicators (`sampleCSSIndicators`)
Non-layout-thrashing scan using attribute/class selectors:

| Indicator | Selector | What it measures |
|---|---|---|
| `backdropFilterCount` | `[style*="backdrop-filter"]`, `.glass-card`, `.glass-panel` | Active GPU blur layers |
| `willChangeCount` | `[style*="will-change"]` | Compositor-promoted layers |
| `canvasCount` | `canvas` | WebGL contexts + 2D drawing surfaces |

> **Why not `getComputedStyle`?** Scanning `getComputedStyle` across 300+ DOM elements forces
> a synchronous style recalculation (layout thrash) on every sample cycle. Attribute/class
> selectors query the DOM tree without triggering reflow.

#### 2.1.5 Per-Route Memory Lifecycle (`PageMemoryRecord`)
Tracks per-route: `baseline` (on mount), `peak` (during session), `retained` (on unmount).
`retainedDeltaMb` = retained − baseline RSS (leak indicator per page).

**Files**: `app/src/services/memoryProfilerService.ts`,
`app/src/shared/hooks/useMemoryProfiler.ts`,
`app/src/shared/components/profiler/ProfilerDrawer.tsx`

---

### 2.2 `useDynamicFPS` — Unified Frame-Rate-Targeted Render Loop

**Problem**: Multiple animation loops used raw `requestAnimationFrame` at uncapped 60 FPS,
including two competing loops in `LiveWaveform.tsx` that could not both be cancelled cleanly.
Animations continued running when components were scrolled off-screen or the tab was hidden.

**Fix**: `useDynamicFPS` hook provides a single controlled loop:

```typescript
useDynamicFPS({
  onFrame: (deltaTime) => { /* render */ },
  fpsActive: 60,        // full rate when interactive
  fpsIdle: 15,          // reduced rate when idle
  isActive,             // drives active/idle rate switch
  isVisible,            // pauses when scrolled off-screen (IntersectionObserver)
  isPageVisible,        // cancels RAF entirely when tab is backgrounded
  isPaused,             // explicit pause gate
})
```

Frame-skipping: `if (elapsed < 1000/targetFps) → skip tick`. The RAF loop is cancelled
entirely (`rafRef.current = null`) when paused, hidden, or page-backgrounded — not just
skipping draw but stopping the scheduler.

**Applied to**: `LiveWaveform.tsx` (unified competing loops), `AdvancedOrb.tsx`

**Files**: `app/src/shared/hooks/useDynamicFPS.ts`

---

### 2.3 `LiquidChamber` 30 FPS Throttle

**Problem**: The monitoring panel wave canvas ran at 60 FPS continuously.

**Fix**: Inline frame interval tracking:

```typescript
const targetInterval = 1000 / 30; // 30 FPS
if (now - lastFrameTime < targetInterval) { rafId = requestAnimationFrame(loop); return; }
lastFrameTime = now;
// ... draw
```

Loop cancelled entirely when component unmounts or panel is hidden.

**Impact**: ~50% CPU reduction on the canvas draw path. 0 FPS when panel closed.

**Files**: `app/src/shared/components/monitoring/LiquidChamber.tsx`

---

### 2.4 `AmbientBackground` Compositor Optimization

**Fix**:
- Reduced from multiple blobs to 2 (`42vmax`, `36vmax`)
- `will-change: transform` promotes both blobs to GPU compositor layers — avoids main-thread
  paint per animation frame
- Relaxed idle polling to 600ms

---

### 2.5 `MemoryGraph` WebGL Physics Settlement & Zero Expansion Drift

**Problem**: Force-directed physics ran indefinitely, uploading Float32 GPU buffers every tick,
causing continuous CPU work and unbounded node position drift.

**Fix**: Two-phase simulation lifecycle:

| Phase | Condition | Behavior |
|---|---|---|
| **Settlement** | `ticks < 100` | Physics runs: `alpha=0.08`, `repulsion=1200`, `springLength=85`, `damping=0.85` |
| **Frozen** | `ticks >= 100` | Physics halts. GPU buffer uploads stop. Nodes frozen in organic equilibrium. |
| **Re-arm** | `nodes` or `edges` props change | Resets `isSettledRef = false`, `ticksRef = 0` to settle new topology |

GPU instance buffers pre-allocated at `maxNodes=10000`, `maxEdges=20000` to avoid
reallocation on topology change.

**Impact**: 0 FPS CPU+GPU idle after settlement. Zero expansion drift.

**Files**: `app/src/shared/components/memory/MemoryGraph.tsx`

---

### 2.6 Zero-Teardown Theme Switching

**Problem**: `isLightMode` in WebGL scene `useEffect` deps caused full GPU context teardown
on every theme toggle.

**Fix**: Removed `isLightMode` from effect deps. Theme changes update materials in-place:
line opacity, node instance colors, badge palettes — all via direct buffer/ref writes.

**Files**: `app/src/shared/components/memory/MemoryGraph.tsx`

---

### 2.7 O(1) Centroid Badge Updates

**Problem**: Badge update path used linear node search + triggered `setState` at 60 FPS.

**Fix**:
- Pre-indexed `nodeById` Map for O(1) lookup
- `Set.has` for O(1) cross-relation checks
- Badge `setState` throttled to ≤8Hz (delta ≥ 120ms via `lastBadgeUpdateRef`)

**Files**: `app/src/shared/components/memory/MemoryGraph.tsx`

---

### 2.8 Markdown Fast-Path Rendering

**Problem**: `DetailPanel.tsx` ran `ReactMarkdown` (full AST parse) on every turn bubble,
causing ~80–150ms click latency when opening history sessions.

**Fix**: Regex pre-check fast path:

```typescript
const isPlainText = !/[*_#`\[\]]/.test(content);
// true → plain <p>, no ReactMarkdown
// false → ReactMarkdown
```

**Impact**: Instant session select for the majority of turns (plain text).

---

### 2.9 Global Drawer Portal Mounting

**Problem**: Profiler drawer inside `<main>` (which has `contain: layout style`) caused
z-index clipping at the `<EdgeNav />` boundary.

**Fix**: `Drawer.tsx` with `position="global"` uses `React.createPortal(..., document.body)`,
escaping the `contain` boundary.

---

## 3. Observed Memory Behavior & Expected Baseline Shifts

### 3.1 Why RSS Doesn't Return to Cold-Boot Baseline

After first model load → unload cycle, a ~125MB permanent baseline shift is **expected and normal**:

| Cause | ~Contribution |
|---|---|
| WebKit JIT bytecode cache | 40–60MB |
| ONNX Runtime global environment singleton | 30–40MB |
| glibc Tokio thread-pool maintenance arenas | 10–20MB |
| SQLite WAL pages (after first write) | 5–10MB |

`trim_heap` returns arena pages but cannot reclaim JIT caches or singleton allocations. These
are one-time first-run costs. Unbounded growth across multiple load/unload cycles would indicate
a real accumulation.

### 3.2 Profiler-Based Verification

1. Cold boot → open profiler → manual snapshot = **baseline**
2. Load model, run a session, unload
3. Manual snapshot → check `retainedDeltaMb`
4. Repeat: if `retainedDeltaMb` grows monotonically → real leak candidate
   If it stabilises at ~125MB after first cycle → expected platform behaviour

---

## 4. Cross-Platform Matrix

| Optimization | Linux | macOS | Windows |
|---|---|---|---|
| On-demand WebView (Tray + Wizard) | ✅ | ✅ | ✅ |
| Heap trim after model eviction | `malloc_trim(0)` | No-op (intentional) | `EmptyWorkingSet` |
| Process tree thread-task filter | ✅ (Linux tasks API) | N/A | N/A |
| GPU detection | `/dev/nvidia0`, `/dev/dri` | Metal (always Tier 1B) | `wmic` Win32 probe |
| ALSA noise suppression | `ALSA_LOG_LEVEL=0` | N/A | N/A |
| Tray HUD positioning | GTK virtual layer + Cairo | `tauri-plugin-positioner` | `tauri-plugin-positioner` |
| Engine offload on window hide | ✅ | ✅ | ✅ |
| JS heap sampling | ✅ (WebKit may omit API) | ✅ (WebKit may omit API) | ✅ |
| CSS compositing scan | ✅ | ✅ | ✅ |
| DOM node count | ✅ | ✅ | ✅ |
| LiquidChamber 30 FPS cap | ✅ | ✅ | ✅ |
| MemoryGraph physics settlement | ✅ | ✅ | ✅ |
| useDynamicFPS unified loop | ✅ | ✅ | ✅ |
