---
trigger: manual
description: TypeScript, React 19, Liquid Space design tokens, and WebGL lifecycle standards for the Vox frontend (`app/src/`).
---

# Vox — Frontend Code Style Guide & Engineering Standards

Durable engineering standards for the Vox frontend application (`app/src/`). Read before creating or editing TypeScript/React files.

---

## 1. Hardware Tiers & UI Mapping

The ambient UI reflects active hardware capabilities. Visual surfaces adapt dynamically to model degradation or cloud streaming.

| Tier | Hardware | Pipeline Mode | Ingestion | Retrieval | Tool Calling |
| :--- | :------- | :-----------: | :-------: | :-------: | :----------: |
| **1A** | 8GB, CPU-only | Modular (Local) | FIFO only | Context window | None |
| **1B** | 8GB+, dedicated GPU | Modular (Local) | Async background | Full (vector + BM25) | Local LLM dependent |
| **2A** | Hybrid (Remote LLM) | Modular (Remote LLM) | Async background | Full | Remote LLM dependent |
| **2B** (default) | Hybrid (Cloud LLM) | Modular (Cloud LLM) | Async background | Full | Full cloud tool calls |
| **3** | Realtime S2S | Realtime (WebSocket) | Provider-managed | Early tool calls | Full provider tools |

---

## 2. Architecture & Layer Boundaries

- **Package manager:** Always use `pnpm`.
- **Zero hardcoded copy:** Place labels, options, and descriptions in `src/data/` (e.g. `appData.ts`). Never inline raw strings inside component templates.
- **Service encapsulation:** Components never call `@tauri-apps/api` invoke directly. Route all backend calls through modules in `src/services/`.
- **Page responsibility:** Files in `src/pages/` manage layout and route composition only. Business logic lives in services, hooks, or stores.
- **Component structure:** Group components into domain subdirectories inside `src/shared/components/` (e.g. `layout/`, `settings/`, `memory/`).
- **Strict typing:** Do not use `any`. Write explicit TypeScript interfaces for props, store slices, and service payloads.

---

## 3. State Management & Reactivity

- **High-frequency state:** Waveform amplitudes, audio visualizer levels, and 60 FPS animation targets live in local component state or mutable refs (`useRef`). Never store high-frequency data in React context.
- **Application state:** User settings, active hardware tier, and connectivity status live in Zustand stores (`src/store/`) or dedicated contexts.
- **Stateful logic extraction:** Extract recurring component state logic into custom hooks in `src/hooks/`.

---

## 4. React 19 Performance & Lifecycle Invariants

Violating these invariants causes performance regressions, memory leaks, or unmount crashes.

### 4.1 Context Stability
- **Memoized context values:** Wrap all Provider `value` objects in `useMemo`. Never pass unmemoized object literals.
- **Separate volatile state from dispatchers:** Do not place rapid telemetry updates into the same context provider as stable action callbacks.

### 4.2 Zustand Selectors
- **Targeted atomic selectors:** Always select minimal scalar slices (`useSettingsStore((s) => s.theme)`) or use `useShallow`.
- **No full-store destructuring:** Avoid `const { ... } = useSettingsStore()`, which forces re-renders on unrelated store mutations.

### 4.3 Async Effects & Event Listeners
- **`isMounted` listener guards:** Always guard async setup callbacks and state updates with an active mount flag:
  ```ts
  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | null = null;
    const setup = async () => {
      const u = await win.listen(...);
      if (isMounted) unlisten = u;
      else u();
    };
    setup();
    return () => { isMounted = false; if (unlisten) unlisten(); };
  }, []);
  ```

### 4.4 Timers & Animation Frames
- **Pure state updaters:** Never invoke `setTimeout`, `setInterval`, or IPC triggers inside `setState(prev => ...)` functions.
- **Nested timer tracking:** Store and clear all timer and interval IDs during effect cleanup.
- **Mutable target capture:** Animation frame loops must read transition targets from mutable refs (`targetRef.current`) rather than closures.

### 4.5 Component Memoization
- **Leaf control memoization:** Wrap shared controls (`Button`, `SegmentedControl`, `SliderField`, `ToggleTile`) in `React.memo` with an explicit `displayName`.
- **Stable callbacks:** Wrap functions passed to memoized children in `useCallback`.

### 4.6 WebGL & GPU Resource Teardown
- **Canvas dimension reset (WebKitGTK invariant):** Before disposing a WebGL context, set `canvas.width = 1; canvas.height = 1;`. Failing to shrink the canvas leaves full-resolution backing framebuffers in process memory.
- **Forced context loss:** Call `renderer.forceContextLoss()` before `renderer.dispose()` on unmount.
- **Resource cleanup:** Dispose all geometries, materials, and textures during cleanup.
- **2D canvas teardown:** 2D canvases must cancel active RAF loops, disconnect `ResizeObserver`, and reset dimensions on unmount.

### 4.7 Pure Utility Isolation
- Place pure formatting and string helpers in `src/shared/lib/`. Pure utilities must never import `@tauri-apps/api` or state stores.

---

## 5. Liquid Space Design System Rules

- **Elevation hierarchy:** Use defined design token elevation classes. Never invent arbitrary shadow or blur values.
- **Mood synchronization:** Visual surfaces reflect the 7 canonical backend pipeline states (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`).
- **Interactive pills only:** Rounded pill containers (`rounded-full`) with borders are reserved strictly for clickable controls. Never place static text, metrics, or badges inside pill containers.
- **No redundant status badges:** Avoid adding badge containers adjacent to a control that already displays the current value.
- **Frameless vector graphics:** Technical diagrams and SVG illustrations render as frameless wireframes (0.8px–1.25px stroke) without heavy background bounding boxes.