---
title: "Vox Frontend Architecture"
audience: "Internal — agents & contributors needing quick, accurate context"
last_updated: 2026-08-31
owners: "frontend-engineer role"
related_docs:
  - "docs/design.md            — Authoritative design system (tokens, type, elevation)"
  - "docs/backend.md           — Rust backend, IPC events, provider architecture"
  - "docs/features/performance-memory-optimizations.md — Sole owner of perf/memory details"
  - "docs/features/memory-architecture.md            — Cognitive memory backend + graph"
  - "AGENTS.md §2, §5          — Workspace map & system invariants"
---

# Vox Frontend Architecture

## 0. How to read this doc

- **Audience:** internal. Other agents and contributors use this to get accurate, fast context on the React/Tauri UI without reading every file.
- **Scope:** the frontend only — `app/src/`. Backend pipeline logic, IPC event contracts, and design tokens live in the referenced docs (see header). This file *consumes and points*, it does not duplicate them.
- **Convention:** every technical claim uses a `path/to/file.ts` or `dir/` pointer, never invented code. Schemas and types are linked, not pasted.
- **Non-goals:** not a design-token reference (→ `docs/design.md`), not a backend/IPC spec (→ `docs/backend.md` §8), not a performance ledger (→ `docs/features/performance-memory-optimizations.md`).

## 1. Overview — dual-surface model

Vox is not one UI; it is **two independent Tauri webviews plus one ephemeral wizard**, all talking to a single Rust core over IPC.

| Surface | Window label | Lifecycle | Entry file |
|---|---|---|---|
| Main App | `main` | Persistent; hidden (not closed) on `CloseRequested`; lazily re-created by `ensure_main_window` if the renderer is destroyed (crash / DevTools `close`) | `app/src/App.tsx`, `app/src/pages/Home.tsx` |
| Tray HUD | `tray` | Created on demand via `ensure_tray_window`, destroyed when dictation disabled / non-Tray output | `app/src/tray/TrayApp.tsx` |
| Setup Wizard | `wizard` | Created on demand via `ensure_wizard_window`, closed on completion | `app/src/wizard/WizardRoot.tsx` |

State flows **one way: pipeline → UI** (see `frontend-engineer.md` invariants). Visual mood is always derived from backend events, never invented locally.

## 2. Stack at a glance

Pinned versions live in `app/package.json`. Summary:

| Layer | Choice | Notes |
|---|---|---|
| Language | TypeScript + React 19 | Strict mode, `any` prohibited (code-style-guide §2) |
| Build | Vite 7 | `pnpm build` = `tsc && vite build` |
| Desktop runtime | Tauri 2.11 | Two/three webviews over a Rust core |
| State | Zustand 5 | Single settings store, draft/committed pattern |
| Styling | Tailwind 4 + shadcn primitives | Glassmorphism "Liquid Space" system (→ `docs/design.md`) |
| Animation | Framer Motion 12 | Mood-synced, frame-throttled |
| 3D / WebGL | Three 0.184 | Memory graph engine only |
| Routing | react-router-dom 7 | File-based lazy routes in `App.tsx` |
| Charts | recharts 3 | Monitoring sparklines |
| Test | vitest 4 | `pnpm test`; service-layer + hook tests under `__tests__/` |

Package manager is **pnpm** (never npm/yarn).

## 3. App shell & routing

- **Boot gate** — `app/src/App.tsx:50-75`: reads `getOnboardingStatus()`; if setup incomplete, routes to `/wizard`, else mounts `ResponsiveLayout` with the five main routes. Window reveal uses a double-`requestAnimationFrame` + `show()` + 300ms cross-fade behind an `OrbitalLoader` (`shared/components/common/OrbitalLoader.tsx`).
- **Lazy + prewarm** — secondary pages (`History`, `Memory`, `Settings`, `Monitoring`) are `React.lazy`; their chunks are imported in the background on boot (`App.tsx:46-48`) so navigation is instant.
- **Providers** (mount order in `App.tsx`): `ErrorBoundary` (root) → `MemoryProfilerProvider` → `VoiceSessionProvider` (persistent pipeline state & event listeners across route changes) → `Router` → `ProfilerDrawerProvider` → `installOverlayStack()` (`shared/lib/overlayStack.ts`).
- **Layout shell** — `app/src/layout/ResponsiveLayout.tsx` renders `TitleBar`, `AmbientBackground`, `EdgeNav`, the bottom-left engine monitor toggle, and an `<Outlet/>` inside a `contain: layout style` `<main>`. It also owns the viewport-transition engine (compact ↔ desktop) and arrow-key page navigation (`ResponsiveLayout.tsx:32-96`).

## 4. Window system (Tauri)

- **Main window** is defined statically in `tauri.conf.json`. **Tray and wizard windows are NOT** — they are constructed on demand by `ensure_tray_window` / `ensure_wizard_window` in `app/src-tauri/src/tray.rs` and `wizard.rs`, and `.close()`d when inactive. This keeps ~490MB RAM off cold boot (detail: `features/performance-memory-optimizations.md` §1.1).
- **Engine offload on hide** — closing the main window hides it and calls `stop_engine()` when dictation is disabled and not engaged (`app/src-tauri/src/lib.rs`).
- **Crash recovery (lazy recreate)** — the main webview is never destroyed on `CloseRequested` (hidden only). If the renderer dies (crash / DevTools), its `WebviewWindow` handle vanishes; the next "Launch Vox" tray action calls `ensure_main_window` (`src/window_main.rs`), which rebuilds a fresh `main` window from `tauri.conf.json` attributes. A "Restart Vox" tray item appears **only after a crash is detected** (`main_window_destroyed`) and performs a full process restart (`app.restart()`) for deep recovery (`src/lib.rs`, `src/tray.rs`).
- **Platform positioning** — Linux uses a GTK virtual layer with Cairo input-shape regions for click-through; macOS/Windows use `tauri-plugin-positioner`. Full detail and the cross-platform matrix are in `features/performance-memory-optimizations.md` §4 matrix. Frontend code must not hardcode window geometry.

## 5. State management

- **Store** — `app/src/store/settingsStore.ts` is the single source of truth for `VoxSettings` (full schema: `settingsStore.ts:108-207`). It uses a draft/committed pattern: edits mutate `draftSettings`; `commitChanges()` diffs against `settings` and writes only changed keys via `updateSetting`, collecting `restartKeys` for `Restart`-policy domains.
- **SSOT Schema Consistency (`vad_backend`)** — VAD backend selection is strictly standardized on `vad_backend: "earshot" | "ten_vad"` across Rust backend (`core::settings::VadSettings`), IPC serialization, Zustand store, and React view components.
- **Category-Scoped Dirty State & Rollback** — In addition to domain-level checks (`isDomainDirty`), the store exposes `isCategoryDirty(category)` and `discardCategoryChanges(category)` (`stt`, `llm`, `tts`, `vad`, `auxiliary`). This decouples stage tabs in `InteractionCard` and `ModelsTopologyMap` so unsaved drafts in one category (e.g. LLM) do not trigger false Save footer prompts when inspecting another clean category (e.g. STT).
- **Selector discipline** — components read with fine-grained selectors (e.g. `useSettingsStore(s => s.ui.theme)`) to avoid re-render cascades.
- **Adapter** — `app/src/shared/context/SettingsContext.tsx` wraps the store for legacy consumers. To prevent Vite Fast Refresh invalidation cascades (`"useSettings" export is incompatible`), the hook is extracted into `src/shared/hooks/useSettings.ts`, leaving `SettingsContext.tsx` strictly component/context only.
- **Model catalog** — `ModelCatalog` / `ModelGroupInfo` / `ModelCapabilities` types and `requestModelCatalog()` live in the store + `services/settingsService.ts`. Catalog drives the Settings model workspaces.
- **Shared state rule** — low-frequency config in the store/context; fast-changing animation values stay in local state or refs (`code-style-guide.md` §2).

## 6. Service layer — the only IPC boundary

Raw `@tauri-apps/api` `invoke` calls are **banned inside components** (code-style-guide §2). All IPC/API access flows through `app/src/services/`:

| Service file | Responsibility |
|---|---|
| `services/settingsService.ts` | Boot state, settings get/update, model catalog, provider health, input devices |
| `services/pipelineService.ts` | Engine lifecycle (`stopEngine`, `launchEngine`), discrete session verbs (`startSession`, `endSession`, `pauseSession`, `resumeSession`), PTT (`pttStart`, `pttStop`, `pttCancel`), test clips (`testClip`, `testClipCancel`), runtime snapshots, voice library, realtime cache, remote deploy |
| `services/eventsService.ts` | Typed Tauri `listen` wrappers for canonical 7-state events, telemetry, transcripts — `on<T>` sync-cleanup wrapper with `beforeunload`/`pagehide` registry |
| `services/historyService.ts` | Session/turn CRUD, transcript history, delete |
| `services/memoryService.ts` | Memory graph topology, stats, fact mutations, ingestion control |
| `services/memoryProfilerService.ts` | Multi-dimensional RAM/heap/DOM profiling snapshots |
| `services/modelService.ts` | Onboarding status, model download/setup, manifest |
| `services/windowService.ts` | Tray/wizard window visibility + HUD control |

**Rule:** pages compose layout only (`code-style-guide.md` §2). Business logic, data transforms, and IPC belong in `services/`, `hooks/`, or `store/`.

## 7. Page architecture

| Page | Entry | Key components (under `shared/components/`) | Notes |
|---|---|---|---|
| Home (Orb) | `pages/Home.tsx` | `home/AdvancedOrb`, `home/PipelineField`, `home/StatusCapsule`, `home/ActiveTranscript`, `home/TestClipsPopover` | Orchestrates engage/pause/PTT via `VoiceSessionContext` + `hooks/useHomePage.ts`. Mode-adaptive toolbar (Passive: Pause/Resume + Disengage; PTT: central hold-to-talk Orb); canonical 7-state ambient mood sync + Space/Escape global PTT bindings; decoupled test clips simulation popover. |
| History | `pages/History.tsx` | `history/HistoryListView`, `history/OrbitCarousel`, `history/CentralClockNode`, `history/DetailPanel`, `history/ChamberOrbitRings` | Dual-view via `ViewSelector` — list + holographic 2.5D single-ring CSS ellipse carousel (`history/orbitMath.ts`), windowed chunking, global `Drawer` detail. (Purged obsolete `VoiceDial.tsx`). |
| Memory | `pages/Memory.tsx` | `memory/MemoryGraph`, `memory/MemoryNodeTooltip`, `memory/MemoryPipelineDrawer`, `memory/SearchBar`, `memory/MemoryLegendCard` | Custom Three.js InstancedMesh WebGL engine with zero-drift physics settlement, hoisted buffer objects, precomputed $O(1)$ adjacency indexing, and non-destructive viewport resize handling (0 extra CPU/RAM). Upgraded with dynamic horizontal expandable mobile action tray, full-width top search overlay with auto-focus/dismiss, desktop-only bottom-right EdgeNav-matched pill button (`h-[56px] px-4 rounded-full`) opening a floating dropup tray (`w-[310px]`) with zero button jump, persistent centroid badge pills, and fixed bottom drawer docking for node detail tooltips and collection detail cards. |
| Settings | `pages/Settings.tsx` | `settings/RadialHub` + domain cards (`appearance/`, `interaction/`, `models/`, `memory/`, `persona/`, `history/`, `realtime/`) | Radial hub of 6 cards with unified typography (`font-display text-[13px] font-black uppercase tracking-[0.2em]`) and `size={17}` icons. Re-architected `InteractionCard` (2-level drilldown: Level 1 has full-width `CategorySelector` text carousel $\to$ `ProviderSelectorView` centered cards with persistent saved active highlight; Level 2 replaces the entire inner panel with `LlmConfigDesk` taking full height, left-aligned `← Providers` breadcrumbs + title, right-aligned status badge, high-density hardware/runtime spec cards dynamically bound to `modelCatalog` with zero hardcoded model names, and auto-discard on back or category switch); `MemoryCard` redesign (top two `ToggleTile` cards for Conversational Recall and Background Auto-Save $\to$ dedicated 5-subtab `MemoryConfigDesk` with `Depth`, `Cutoff`, `Graph`, `Budget`, `Window` following HistoryCard side-by-side ergonomics); `TtsVoiceManager` redesign (2 distributed tabs `Select Voice` \| `Speech Speed`, paragraph-embedded inline accent region carousel `‹ ALL ›`, and in-place search); `ModelsCard` workspace sizing (`h-auto max-h-[235px]` compact, `h-full` desktop); `AppearanceCard` calibrated unclipped `130px` HexColorPicker; `RotaryKnob` travel calibration (280px denominator) without wheel scroll capture. |
| Monitoring | `pages/Monitoring.tsx` | `monitoring/MetricCarousel`, `monitoring/LiquidChamber` + `profiler/*` | Runtime metrics dashboard; offload/reload dual-button engine control; 30 FPS throttled canvas; integrated memory profiler drawer. |

## 8. Shared layer

`shared/` is organized by domain (code-style-guide §2 — flat dirs banned):

- **`shared/components/`** — `common/` (ErrorBoundary, AmbientBackground, LiveWaveform, OrbitalLoader, GlassSkeleton, AudioLevelMeter), `home/`, `history/`, `memory/`, `settings/` (`interaction/ProviderSelectorView`, `interaction/LlmConfigDesk`, `interaction/CategorySelector`, `memory/MemoryConfigDesk`, `models/ModelsTopologyMap`, `models/LlmSettingsView`, `models/VadWorkspace`, ...), `monitoring/`, `profiler/`.
- **`shared/hooks/`** — reusable stateful logic (used in 2+ components):
  - `useDynamicFPS` — unified frame-rate-targeted RAF loop (60/15/0 tiers). Owner: `features/performance-memory-optimizations.md` §2.2.
  - `useInteraction` — logical interaction-session continuity (committed/partial text, id stability, 4000-char cap).
  - `useVisibility` — Tray HUD ephemeral state machine (`HIDDEN→APPEARING→ACTIVE→FADING`).
  - `useStreamingRenderer` — character-stream animation for transcripts with mutable catch-up refs.
  - `useOverlay` — registers a surface with the global `overlayStack`.
  - `useHomePage` — `toMood()` + mode-adaptive toolbar derivation (`shared/hooks/useHomePage.ts`).
  - `useTelemetry`, `useMonitoringMetrics`, `useMemoryProfiler`, `useMemoryTrace`, `useVoxFootprint`, `useSettings`, `useSettingsPage`.
- **`shared/ui/`** — primitives: `Drawer` (the single bottom-sheet, `position="page"|"global"` with clean pointer capture release), `Tooltip` (the **only** sanctioned tooltip — native `title` banned for tooltips), `Card`, `SegmentedControl`, `SliderField`, `RotaryKnob` (calibrated drag travel), `Badge`, `SearchInput`, `ProgressBar`, `ToggleTile`, `icons/VendorLogos`.
- **`shared/lib/`** — `overlayStack.ts` (global FILO dismissal authority), `fuzzy.ts` (catalog search), `utils.ts` (`cn`, `hexToRgb`).
- **`shared/context/`** — `VoiceSessionContext` (root pipeline state with memoized context value, discrete session verbs `engage`/`disengage`/`pause`/`resume` + PTT `handlePttStart/Stop/Cancel`, mutable `kbStateRef` global Space/Escape bindings, throttled listeners), `SettingsContext` (adapter), `MemoryProfilerContext` (memoized value, clean diagnostic interval disposal).
- **`shared/data/`** — all static copy (homeCopy, settingsCopy, memoryCopy, ...)

## 9. IPC & events — consumer view

The Rust event contract is authoritative in `docs/backend.md` §8. The frontend consumes it through typed wrappers in `services/eventsService.ts` — `on<T>` provides synchronous `unlisten` via `beforeunload`/`pagehide` registry (`eventsService.ts:110-161`). Key events and their consumers:

| Event | Payload source | Consumer surface |
|---|---|---|
| `state_changed` | `InteractionState` (`"Idle" | "Ready" | "Listening" | "Thinking" | "Speaking" | "Paused" | "Error"`) | Main + Tray (mood sync via `VoiceSessionContext` `state_changed` handler + `useHomePage.toMood`) |
| `transcript_partial` / `transcript_final` | `TranscriptPayload { turn_id, text, owner }` | ActiveTranscript, Tray (throttled 30ms in context) |
| `llm_token` | `string` | Holographic dialogue stream (throttled 30ms) |
| `voice_error` | `VoiceErrorPayload` | Error toasts (`errorAlert` in context) |
| `model_progress` | `ModelSetupStatus` | Wizard steps, catalog progress |
| `telemetry` | `TelemetryData { energy, vad_prob, ... }` | Orb waveform, Tray HUD, `useTelemetry` (energy for waveform) |
| `system_stats` | `SystemStatsPayload` | Monitoring dashboard |
| `settings-updated` | — | Settings hot-reload |
| `toggle_tray` | — | Tray HUD toggle |

Commands are issued via the service modules in §6 — `pipelineService.ts:63-97` (`startSession`→`start_session`, `endSession`→`end_session`, `pauseSession`→`pause_session`, `pttStart`→`ptt_start`, etc.) (never bare `invoke` in components). Full command list and reload policies: `docs/backend.md` §10. Frontend never branches on `pipeline_mode` for lifecycle — always calls the same verbs; backend `RoutingContext` resolves the domain.

## 10. Design system consumption

Frontend consumes — it does not redefine — the design system in `docs/design.md`:

- **Tokens** are CSS variables (`rgb(var(--token))`) declared in `app/src/index.css` and mirrored in `design.md` frontmatter.
- **Elevation** uses the closed glass system (`.glass-whisper` / `.glass-surface` / `.glass-card`). Do not invent a new level.
- **Type roles** (`font-display` / `font-sans` / `font-mono`) and the uppercase policy are enforced by `design.md` §4.
- **Custom Tooltip** (`shared/ui/Tooltip.tsx`) is mandatory for hover explanations.
- The `impeccable` design-system detector enforces token/size compliance against `design.md`.

## 11. Performance & memory invariants

The **sole owner** of all performance and memory detail is `docs/features/performance-memory-optimizations.md`. This section only lists what the frontend owns and where to read it. Frontend must not duplicate those specifics.

- **`useDynamicFPS`** unified loop — `features/performance-memory-optimizations.md` §2.2; hook at `shared/hooks/useDynamicFPS.ts`.
- **`LiquidChamber`** 30 FPS throttle — §2.3.
- **`AmbientBackground`** compositor promotion & idle settlement demotion — §2.4, §2.10.
- **`MemoryGraph`** WebGL physics settlement, zero-teardown theme switch, O(1) badge updates — §2.5, §2.6, §2.7.
- **Markdown fast-path** in `DetailPanel` — §2.8.
- **Global drawer portal** (`position="global"` → `createPortal` to `document.body`) escaping `contain:layout` — §2.9; implementation `shared/ui/Drawer.tsx:240-242`.
- **Settings reactive architecture** (zero context fan-out, pointerup color picker, dirty check scalar comparison) — §2.10.
- **Backend-side** optimizations (on-demand webviews, heap trim, process-tree filtering, zero-idle ONNX eviction) — §1 and §3 of that doc.

General rule from `frontend-engineer.md`: anything visually heavy is memoized and frame-throttled; idle/sleep states drive 0 FPS.

## 12. Responsiveness & overlay stack

- **Breakpoints** — desktop `≥1024px` (floating `EdgeNav` capsule + monitoring popover bottom-left); compact `<1024px` (monitoring becomes a 4th `EdgeNav` tab routed to `/monitoring`, soft glass fade mask, `pb-[110px]` scroll padding). Viewport transitions are handled bidirectionally in `ResponsiveLayout.tsx:32-53` — never break this (code-style-guide layout rules).
- **Orb scaling** — mobile `min(92vw, 85vh)`, desktop `min(70vw, 65vh)`; do not change without design review.
- **Overlay stack** — `shared/lib/overlayStack.ts` is the single FILO authority. `installOverlayStack()` (called once in `App.tsx`) installs capture-phase `keydown` (Escape pops topmost) and `pointerdown` (outside-click dismisses topmost if `dismissOnOutside`). Surfaces register via `useOverlay` (`shared/hooks/useOverlay.ts`) and `Drawer` (`shared/ui/Drawer.tsx`). Tier model: `design.md` §13 (Tier 0 accordion cards, Tier 1 popovers, Tier 2 bottom drawers). Surfaces must not add their own Escape listeners.

## 13. Resilience

- **Error boundaries** — `shared/components/common/ErrorBoundary.tsx` wraps the root (`App.tsx`) and every route element individually, so one page crash does not take down the app.
- **IPC failure** — service calls are wrapped in try/catch by consumers; failures degrade to toasts or cached/empty state. The store's `loadSettings` falls back to defaults on error (`store/settingsStore.ts:275-279`).
- **Settings recovery** — `restoreDefaults()` (`store/settingsStore.ts:579-588`) resets via `resetSettings()`; `discardChanges()` / `discardDomainChanges()` revert drafts without IPC.
- **Backend-not-ready** is treated as the default case, not an edge case (`frontend-engineer.md` invariants).

---

## Appendix A — Project structure (`app/src/`)

```
app/src/
├── main.tsx                     # Entry: mounts <App/>
├── App.tsx                      # Boot gate, router, providers, overlay stack
├── index.css                   # Token CSS vars + glass elevation classes
├── layout/                     # ResponsiveLayout, EdgeNav, TitleBar
├── pages/                      # Home, History, Memory, Settings, Monitoring
├── tray/                       # TrayApp + components/{Header,TranscriptRenderer,Footer}
├── wizard/                     # WizardRoot, state/setupMachine, steps/, components/
├── services/                   # IPC boundary (see §6 table)
├── store/                      # settingsStore.ts (Zustand)
└── shared/
    ├── components/             # common/ home/ history/ memory/ settings/ monitoring/ profiler/
    ├── hooks/                  # useDynamicFPS, useInteraction, useVisibility, useOverlay, ...
    ├── ui/                     # Drawer, Tooltip, Card, SegmentedControl, SliderField, ...
    ├── lib/                    # overlayStack, fuzzy, utils
    ├── context/                # SettingsContext, MemoryProfilerContext
    └── data/                   # All static copy (homeCopy, settingsCopy, memoryCopy, ...)
```

## Appendix B — Cross-links

- `docs/design.md` — tokens, type system, elevation, motion, accessibility (authoritative).
- `docs/backend.md` — Rust architecture, IPC event contract (§8), settings reload policies (§10).
- `docs/features/performance-memory-optimizations.md` — **SSOT for all perf/memory detail** (frontend §11 + backend §1).
- `docs/features/memory-architecture.md` — cognitive memory backend, graph topology, pipeline.
- `AGENTS.md §2` — workspace directory map. `AGENTS.md §5` — system invariants (dictation axes, lazy windows, drawer portal).
- `.agents/rules/frontend-engineer.md` — frontend role invariants. `.agents/rules/code-style-guide.md` — Rust + TS standards.

---

**Last Updated:** 2026-08-31
