---
trigger: manual
description: TypeScript, React 19, Liquid Space design tokens, and WebGL lifecycle standards for the Vox frontend (`app/src/`).
---

# Vox — Frontend Code Style Guide & Engineering Standards

Durable engineering standards for the Vox frontend application (`app/src/`). Read before creating or editing TypeScript/React files.

---

## 2. Architecture & Layer Boundaries

- **Package manager:** Always use `pnpm`.
- **Design system SSOT:** Visual design tokens, elevation ladders, typography scales, concentric radii ($R_{inner} = R_{outer} - P$), and overlay grammar are governed by root [`DESIGN.md`](file:///home/addy/projects/apps/vox/DESIGN.md). Never invent one-off colors, blurs, or container pill styles.
- **Zero hardcoded copy:** Place labels, options, and descriptions in `src/data/` (e.g. `appData.ts`). Never inline raw strings inside component templates.
- **Service encapsulation:** Components never call `@tauri-apps/api` invoke directly. Route all backend calls through modules in `src/services/`.
- **No raw fetch:** Direct `fetch()` calls in UI components and stores are strictly prohibited and enforced by ESLint `no-restricted-globals`. All network calls must pass through `src/services/transport.ts`.
- **Zero silent error swallowing:** Empty `catch {}` or `catch (_e) {}` blocks are strictly banned. All non-fatal catch blocks must log diagnostic details via `console.debug` or `console.warn`.
- **Page responsibility:** Files in `src/pages/` manage layout and route composition only. Business logic lives in services, hooks, or stores.
- **Component structure:** Group components into domain subdirectories inside `src/shared/components/` (e.g. `layout/`, `settings/`, `memory/`).
- **Strict typing:** Do not use `any`. Write explicit TypeScript interfaces for props, store slices, and service payloads.

---

## 3. State Management & Reactivity

- **High-frequency stream isolation:** Live streaming buffers (`transcript`, `assistantText`, streaming LLM tokens, raw PCM waveform buffers) MUST NEVER be placed in React Context. They belong in dedicated Zustand store slices (`sessionStore`) or mutable refs. Components consuming streams must subscribe via targeted scalar selectors at leaf nodes (e.g. `ActiveTranscript`).
- **Context action/data decoupling:** Context providers that manage both state and actions MUST decouple referentially stable actions (`useMemo(() => ({ doA, doB }), [])`) from volatile data ticks. Never invalidate action dispatchers on streaming state updates.
- **Category-scoped dirty isolation:** Settings and configuration forms must scope dirty tracking to individual domain/category slices (`isCategoryDirty(cat)`), preventing draft changes in one category from causing global re-renders or invalidating unrelated tabs.
- **High-frequency visual state:** Waveform amplitudes, audio visualizer levels, and 60 FPS animation targets live in local component state or mutable refs (`useRef`). Never store high-frequency data in React context.
- **Application state:** User settings, active hardware tier, and connectivity status live in Zustand stores (`src/store/`) or dedicated contexts.
- **Stateful logic extraction:** Extract recurring component state logic into custom hooks in `src/hooks/`.

---

## 4. React 19 Performance & Lifecycle Invariants

Violating these invariants causes performance regressions, memory leaks, or unmount crashes.

### 4.0 Unconditional Hook Ordering
- **Hooks execute first:** Every React hook (`useMemo`, `useState`, `useEffect`, `useCallback`, Zustand selectors) MUST execute unconditionally at the top of the component body.
- **Never hook after early returns:** Guard checks returning `null` (e.g. mobile detection, monitoring bypass) must live in a parent wrapper or after all hook invocations.
- **Never short-circuit hooks:** Calling `useA() ?? useB()` is an order-breaking defect. Call each hook independently at root level and coalesce results afterwards.

### 4.1 Root Shell & Layout Purity
- **No telemetry subscriptions in layout shells:** Root layout containers (`ResponsiveLayout`, `AppHeader`, `EdgeNav`) must NEVER invoke polling or telemetry hooks (`useVoxFootprint`, `useMonitoringMetrics`). Telemetry indicators must be isolated in self-contained, memoized leaf components (`FootprintHudButton`).
- **Memoized context values:** Wrap all Provider `value` objects in `useMemo`. Never pass unmemoized object literals.
- **Separate volatile state from dispatchers:** Do not place rapid telemetry updates into the same context provider as stable action callbacks.

### 4.2 Zustand Selectors & List Row Lazy Mounting
- **Targeted atomic selectors:** Always select minimal scalar slices (`useSettingsStore((s) => s.theme)`) or use `useShallow`.
- **No full-store destructuring:** Avoid `const { ... } = useSettingsStore()`, which forces re-renders on unrelated store mutations.
- **Lazy row item mounting:** In virtualized or mapped lists (`SessionRowItem`, `ProjectRowItem`), expensive auxiliary subtrees (floating context menus, popovers, portals, and tooltips) MUST be conditionally mounted only when actively triggered (`menuOpen && <...>`). Never mount hundreds of dormant portals, floating positioning calculators, or window listeners inside list row items.

### 4.3 Compositor & GPU Filter Safety
- **No universal backdrop-filters:** Never apply `backdrop-filter` via universal (`*`) or descendant wildcard selectors (e.g. `.drawer-keep-blur *`). Every descendant receives its own GPU compositor layer, crashing WebKitGTK frame rates.
- **Constrained SVG filter overlays:** Full-viewport SVG filter overlays (fractal noise `feTurbulence`) must specify explicit tile dimensions (`background-size: 200px 200px`), hardware isolation (`transform: translateZ(0)`), and containment (`contain: strict`). Duplicate ambient filter layers are strictly forbidden.
- **Zero extreme blur/blend layers:** Large-area extreme blurs (`blur-[120px]` on 800px+ surfaces) with blending modes (`mix-blend-mode: screen`) behind active canvases are prohibited; use static radial-gradient color stops instead.

### 4.4 DOM Geometry & Layout Thrash Protection
- **No layout queries in streaming paths:** Never query layout-inducing DOM properties (`scrollHeight`, `clientHeight`, `offsetWidth`, `getBoundingClientRect()`) inside high-frequency render loops or token streaming effects.
- **Threshold guarding:** Layout queries on streaming content must be guarded by character length thresholds (e.g. `content.length < 90`) or microtask debouncing.

### 4.5 Lifecycle & Timer Hygiene
- **Tracked timer cleanup:** Store all timer and interval IDs in mutable refs (`timerRef.current`) and clear them during effect cleanup. Never launch untracked `setTimeout` calls inside effect cleanup returns (e.g. `return () => setTimeout(pop, 260)`).
- **Pure state updaters:** Never invoke `setTimeout`, `setInterval`, or IPC triggers inside `setState(prev => ...)` functions.
- **Mutable target capture:** Animation frame loops must read transition targets from mutable refs (`targetRef.current`) rather than closures.

### 4.6 WebGL & GPU Resource Teardown
- **Canvas dimension reset (WebKitGTK invariant):** Before disposing a WebGL context, set `canvas.width = 1; canvas.height = 1;`. Failing to shrink the canvas leaves full-resolution backing framebuffers in process memory.
- **Forced context loss:** Call `renderer.forceContextLoss()` before `renderer.dispose()` on unmount.
- **Resource cleanup:** Dispose all geometries, materials, and textures during cleanup. Null out dense reference arrays (`nodesRef.current = []`).
- **2D canvas teardown:** 2D canvases must cancel active RAF loops, disconnect `ResizeObserver`, and reset dimensions on unmount.

### 4.7 Three.js & Animation Frame Loop Equilibrium
- **Dynamic FPS throttling:** Animation frame loops must schedule via `useDynamicFPS` (60 FPS active, 15 FPS idle, 0 FPS hidden/settled).
- **Two-phase physics settlement:** Physics simulations (force-directed graphs) must settle to equilibrium within fixed ticks ($< 100$) and freeze all GPU uploads (0 FPS idle).
- **Zero-allocation render loops:** Hoist math and color helper objects (`THREE.Object3D`, `THREE.Color`, vectors) into `useRef` instances. Zero object allocations inside 60 FPS animation callbacks.

### 4.8 Ref Render Purity & Effect Cascades
- **Zero ref mutations during render:** Never read or mutate `useRef.current` in the component render phase. Move mutations to `useEffect`, `useLayoutEffect`, or event callbacks.
- **Zero synchronous `setState` in effect body:** Never call `setState()` directly/synchronously in an effect body. For state synchronization based on prop/state changes, adjust state during render using previous state comparison, or defer execution asynchronously.

### 4.9 Markdown AST Fast-Path
- **Fast-path plain text:** Turn and description renderers must pre-check for markdown syntax (`!/[*_#`\[\]]/.test(content)`) to bypass `ReactMarkdown` AST compilation on plain turns, eliminating 80–150ms rendering latency.

### 4.10 Component Memoization
- **Leaf control memoization:** Wrap shared controls (`Button`, `SegmentedControl`, `SliderField`, `ToggleTile`) in `React.memo` with an explicit `displayName`.
- **Stable callbacks:** Wrap functions passed to memoized children in `useCallback`.

### 4.11 Pure Utility Isolation
- Place pure formatting and string helpers in `src/shared/lib/`. Pure utilities must never import `@tauri-apps/api` or state stores.