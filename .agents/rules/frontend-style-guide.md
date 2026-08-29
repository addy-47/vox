---
trigger: manual
description: Vox Frontend Code Style Guide and Engineering Standards for TypeScript/React (`app/src/`).
---

# Vox — Frontend Code Style Guide & Engineering Standards

This document contains durable coding standards for the Vox frontend (`app/src/`). **Agents doing write operations on frontend code must read this file before modifying code.**

---

## 1. Hardware Tiers & Feature Mapping

The frontend reflects the active hardware tier and dynamic model degradation/upgrades in the ambient UI.

| Tier | Hardware | Pipeline Mode | Memory Ingestion | Memory Retrieval | Tool Calling |
| :--- | :------- | :-----------: | :--------------: | :--------------: | :----------: |
| **1A** | 8GB, CPU-only, no GPU | Modular (Local) | ❌ None (FIFO only) | ✅ Working Memory context window only | ❌ Unavailable |
| **1B** ⭐ | 8GB+, dedicated GPU | Modular (Local) | ✅ Full async ingestion | ✅ Full retrieval (episodic + semantic) | ⚠️ Depends on local LLM capability |
| **2A** ⭐ | Hybrid (Remote LLM + Local Audio) | Modular (Remote LLM) | ✅ Full async ingestion | ✅ Full retrieval | ⚠️ Depends on remote LLM capability |
| **2B** ⭐ default | Hybrid (Cloud LLM + Local Audio) | Modular (Cloud LLM) | ✅ Full async ingestion | ✅ Full retrieval | ✅ All cloud models support tool calling |
| **3** | Any (Realtime S2S) | Realtime (WebSocket) | ✅ Provider-managed | ✅ Via early tool calls in provider | ✅ Via early tool calls |

---

## 2. Frontend Architecture & Boundaries

- **Package Manager:** Always use `pnpm`, never `npm` or `yarn`.
- **Zero Hardcoded Text / Labels:** Inline hardcoded strings, labels, select options, or mock objects inside components/pages are strictly banned. All static content must live in `src/data/` (e.g., `appData.ts`, `settingsDomains.ts`).
- **Strict Service Layer Boundary:** Raw `@tauri-apps/api` invoke calls or direct fetches inside React components are prohibited. All IPC/API calls MUST pass through dedicated service modules in `src/services/` (e.g. `pipelineService.ts`, `settingsService.ts`).
- **Page Responsibility (Layout Only):** Files in `src/pages/` MUST only define visual structure, routing, and layout composition. Business logic, state sync, and data transformations belong in `src/services/`, `src/hooks/`, or `src/store/`.
- **Modular Component Subdirectories:** `src/shared/components/` must be structured into logical feature/domain subdirectories (e.g., `layout/`, `home/`, `history/`, `settings/`, `monitoring/`, `common/`). Flat, uncategorized component directories are banned.
- **Type Safety:** Strict TypeScript. `any` is strictly prohibited — define explicit interfaces/types for all props and service returns.
- **Verification:** Run `pnpm lint` and `pnpm build` after every modification. Zero warnings/errors permitted.

---

## 3. State Management & Reactivity

- **Transient High-Frequency UI State:** Audio visualizers, waveform amplitudes, and 60fps animation states live in local component state or mutable refs (`useRef`). Never store high-frequency animation values in global React Context.
- **Low-Frequency App State:** App configuration, settings, active tier, and connection status live in React Context or Zustand (`src/store/`).
- **Reusable Stateful Logic:** Extract into custom hooks (`src/hooks/`) whenever logic appears in 2+ components.

---

## 4. Mandatory React Performance & Lifecycle Invariants (Universal Across All Pages)

These invariants apply to every component, hook, context, and page in `app/src/`. Violating any invariant is a blocking defect.

### 4.1 Context Value & Identity Stability
- **Zero Raw Object Context Values:** Passing unmemoized object literals (`<Context.Provider value={{ a, b }}>`) or closures to a Provider is strictly banned. Context values MUST be wrapped in `useMemo`.
- **Split Volatile State from Actions:** When context state updates frequently (e.g., streaming transcripts, timestamps), separate volatile data from stable dispatcher actions to prevent unnecessary subtree re-renders.

### 4.2 Zustand Selector Discipline
- **Zero Full-Store Destructuring:** Invoking `const { a, b } = useSettingsStore()` is strictly banned. Full-store destructuring subscribes the component to every state change across the entire store.
- **Mandatory Fine-Grained Selectors:** Always use targeted atomic selectors `useSettingsStore((s) => s.draftSettings?.domain)` or shallow equality via `useShallow`.

### 4.3 Async Effect & Event Listener Lifecycle (React 19 / StrictMode Safe)
- **Mandatory `isMounted` Guard for Async Setup:** Every async setup function inside `useEffect` (e.g., Tauri `listen`) must guard state mutations and listener registrations with an `isMounted` flag.
- **Safe Unmount Listener Teardown:** If an async listener resolves after component unmount, invoke the resulting `unlisten()` callback immediately:
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

### 4.4 Timers, Intervals & rAF Hygiene
- **Zero Side-Effects in State Updaters:** Calling `setTimeout`, `setInterval`, or IPC triggers inside a `setState(prev => ...)` updater is strictly prohibited.
- **Track All Nested Timers:** If a timer triggers a subsequent `setInterval` (e.g. staggered polling), both timer references must be stored and cleared in the `useEffect` cleanup.
- **Stable rAF Mutable Target Capture:** `requestAnimationFrame` interpolation loops MUST read target values from a mutable ref (`targetTextRef.current`) rather than closing over a render-scoped variable, preventing early termination or animation stalls.

### 4.5 Component & Leaf Primitive Memoization
- **Mandatory Memo for Shared Leaf Controls:** All shared inputs and leaf controls (`ToggleTile`, `SegmentedControl`, `SliderField`, `Button`, `SearchInput`, `SubModelCard`) must be wrapped in `React.memo` with an explicit `displayName`.
- **Stable Callback References:** Callbacks passed to memoized children must be stabilized with `useCallback`. Avoid passing inline arrow functions in render loops.

### 4.6 WebGL & GPU Resource Teardown
- **Mandatory Force Context Loss:** Every Three.js WebGLRenderer must execute `renderer.forceContextLoss()` prior to `renderer.dispose()` during unmount cleanup, ensuring the WebGL context is released by the browser/webview engine.
- **Geometry & Material Disposals:** All Three.js geometries, instanced meshes, and materials must be explicitly disposed in `useEffect` cleanup.

### 4.7 DOM Ref Callbacks in Loops
- **Zero Inline Ref Callbacks in `.map()`:** Passing inline arrow functions to `ref` inside loops (`ref={(el) => ...}`) forces React to detach (`null`) and reattach every element on every render. Use a stable ref callback cache (`Map<string, (el) => void>`) or dataset query.

### 4.8 Input Debounce & WebGL Buffer Invariant
- **Debounce Heavy Compute/Filter Inputs:** Text inputs driving graph filtering, full-text searches, or WebGL buffer updates must debounce parent state commits by >= 150ms to preserve 60 FPS typing responsiveness.

---

## 5. Liquid Space Design System & Layout Rules

- **Desktop Layout:** Floating bottom `EdgeNav` capsule, monitoring panel as popover bottom-left.
- **Mobile Layout:** Monitoring moves to `/monitoring` route with solid background. Nav capsule gets a 4th tab.
- **Viewport Transitions:** Mobile → desktop redirects from `/monitoring` to `/` and relaunches popover. Desktop → mobile closes popover and routes to `/monitoring`.
- **Orb Responsive Scaling:** Mobile Orb scales to `min(92vw, 85vh)`. Desktop Orb is `min(70vw, 65vh)`.
- **Glass Elevation is a Closed System:** Use only defined design token elevation levels. Do not invent arbitrary ad-hoc shadows or glass backgrounds.
- **Mood Sync:** Visual elements must reflect backend pipeline states (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`).

---

## 6. Documentation Standards

Root architecture and feature docs in `docs/*.md` follow a uniform frontmatter + "How to read" convention:

### 6.1 Required Frontmatter (YAML)
```yaml
---
title: "Doc Title"
audience: "Internal — <who this is for>"
last_updated: YYYY-MM-DD
owners: "frontend-engineer role"
related_docs:
  - "docs/other.md — one-line relationship"
---
```

### 6.2 Required "How to read this doc" Section
Immediately after the title, include:
- **Audience:** who the doc is for.
- **Scope:** what it covers.
- **Convention:** how claims are cited (`path/file.ts` pointers; no invented code blocks).
- **Non-goals:** what it is explicitly NOT (with cross-links).
- **SSOT:** where the authoritative detail lives.
