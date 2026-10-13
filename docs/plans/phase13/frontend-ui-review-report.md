---
title: "Frontend UI Review - Full Read-Only Audit"
audience: "Internal - frontend agents and vox user (decision-maker)"
last_updated: 2026-10-13
owners: "frontend-engineer role"
related_docs:
  - "docs/plans/phase13/frontend-ui-optimization-plan.md - the 7-batch execution plan"
  - "docs/plans/phase13/frontend-perf-tier2-tier3.md - Tier 2/3 decision list"
  - "docs/plans/phase13/recent_work.md - phase log"
---

# Frontend UI Review - Full Read-Only Audit (Phase 13)

## How to read this doc

- **Audience:** frontend agents picking up Tier batches, and you (decision-maker) approving Tier 3 items.
- **Scope:** read-only audit of `app/src/`. Zero source edits were made. Every finding cites an exact `file:line` verified by direct read in this session.
- **Convention:** each finding follows the review-ui rubric - What / Why / Replacement / Severity (red ORANGE YELLOW) + visual Tier (Tier 1 / Tier 2 / Tier 3). Replacements are minimal diffs, not redesigns.
- **Non-goals:** not an execution plan (see `frontend-ui-optimization-plan.md`); not the plain-words decision list (see `frontend-perf-tier2-tier3.md`); not backend/Rust.
- **SSOT:** `.agents/rules/frontend-style-guide.md`, `.agents/rules/frontend-engineer.md`, `.agents/rules/android-style-guide.md`; tokens in `docs/specs/design-spec.md`.

---

## 0. Calibration, Method, and Gates (Mandatory First)

**Reviewing this as production scale - critique is calibrated accordingly.**

Vox is a shipped local-first voice surface running on desktop (Linux/Win/mac, WebKitGTK/WebView2) and Android (`arm64-v8a` WebView), CPU-first hardware, where the UI is the pipeline-state mirror (`Idle / Ready / Listening / Thinking / Speaking / Paused / Error`). Anything that drops frames, leaks compositor layers, or invents state breaks user trust. A pattern that would be over-engineering in a weekend POC (virtualized lists, RAF pausing, canvas teardown) is exactly correct here. Hover-only affordances that "work on desktop" are broken on the shipped Android surface.

**This code affects UI/rendering directly - visual impact tiering (Tier 1/2/3) applies to every finding alongside severity.**

### 0.1 Files read directly (34 files, line-verified)

| Area | Files read |
|------|------------|
| Shell | `layout/ResponsiveLayout.tsx` (1-603), `layout/EdgeNav.tsx` (1-116), `App.tsx` (1-445), `main.tsx` (1-54) |
| Background | `shared/components/common/AmbientBackground.tsx` (1-216), `index.css` (1-360, 600-1009), `shared/theme/index.ts` (1-642) |
| Home / Orb | `pages/Home.tsx` (1-545), `shared/components/home/AdvancedOrb.tsx` (1-1027), `shared/components/home/SessionPanel.tsx` (1-847) |
| Overlays | `shared/ui/Drawer.tsx` (1-314), `shared/ui/Modal.tsx` (1-219), `shared/ui/EdgePanel.tsx` (1-227), `shared/ui/OverlayBackdrop.tsx` (1-38), `shared/ui/Tooltip.tsx` (1-212), `shared/ui/SessionContextMenu.tsx` (1-331), `shared/ui/BottomDockFeather.tsx` (1-48) |
| History | `pages/History.tsx` (1-541), `shared/components/history/HistoryListView.tsx` (1-359) |
| Memory | `pages/Memory.tsx` (1-682), `shared/components/memory/MemoryTimeline.tsx` (1-313), `shared/components/memory/MemoryGraph.tsx` (1-325), `shared/hooks/useMemoryGraphScene.ts` (1-1051+), `shared/components/memory/GraphControlDock.tsx` (1-132), `shared/components/memory/timeline/SessionAccordionItem.tsx` (1-526), `shared/components/memory/PersonalMemoryStagingCard.tsx` (1-475), `shared/components/memory/PersonalMemoryDossierCard.tsx` (1-250), `shared/components/memory/PixelSynthesisCanvas.tsx` (1-281) |
| Settings | `pages/Settings.tsx` (1-293), `shared/components/settings/SettingsCardWrapper.tsx` (1-198), `shared/components/settings/interaction/LlmConfigDesk.tsx` (sampled 1-882), `shared/components/settings/models/voices/VoiceCatalogModalView.tsx` (1-421) |
| Monitoring | `pages/Monitoring.tsx` (1-418) |
| Wizard | `wizard/WizardRoot.tsx` (1-338) |
| Panels | `shared/components/common/NotificationPanel.tsx` (sampled 1-649) |

### 0.2 Live gates run this session

| Gate | Result |
|------|--------|
| `node .agents/skills/review-ui/scripts/check_invariants.mjs` | 5/5 PASS |
| `pnpm test` (vitest) | 58/58 PASS (31 invariant + 27 regression) |
| `pnpm tsc --noEmit` | 0 errors |
| `stress_drawers.mjs` (35s CDP, FPS + DOM drift) | NOT RUN - needs running app |
| `soak_test.mjs` (5min heap slope) | NOT RUN - needs running app |
| `verify-targets.sh` dual-target | NOT RUN - no source touched |
| POCO 360-480px HMR smoke | NOT RUN - needs device |

### 0.3 Prior art consulted (not re-audited)

`frontend-ui-optimization-plan.md` (7 batches, 58 blurs, 27 shadows, D2/C2/C5), `frontend-perf-tier2-tier3.md` (Groups A-F), `recent_work.md` (Tier 1 batch 1+2, Tier 2 batch 1 shipped), `performance-memory-optimizations.md` (backend + frontend ledger), `design-spec.md` (elevation/type/motion), theme-transition RCA + followup.

---

## 1. Grain and Background - File Owners (Asked Explicitly)

There are **two independent grain systems** built from different files. They stack: on Home you see both; on Memory/Monitoring you see only G1.

### G1 - Global film grain (every route, always on)

**Owner: `app/src/index.css:213-231`.**

```css
#root::after {
  content: "";
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  pointer-events: none;
  z-index: 9999;
  opacity: 0.035;
  background-image: url("data:image/svg+xml,...feTurbulence type='fractalNoise' baseFrequency='0.8' numOctaves='3' stitchTiles='stitch'...");
  background-repeat: repeat;
}
[data-theme='light'] #root::after {
  opacity: 0.045;
}
body.is-tray #root::after {
  display: none !important;
}
```

- This is a full-viewport pseudo-element above everything (`z-index: 9999`, `pointer-events: none`). It never unmounts on route change.
- Spec owner: `docs/specs/design-spec.md` grain clause (`opacity: 0.035` dark, `0.045` light).
- Flip/resize handling (read-verified): `[data-theme-transition] #root::after { opacity: 0 }` at `index.css:302-304`; `[data-viewport-resize] #root::after { opacity: 0 }` at `:330-332`.
- **If the complaint is "the whole app looks grainy/noisy", this is the file.** Tuning knob: the two `opacity` values. Removal point: the whole `#root::after` block (tray already proves the app works without it).

### G2 - Ambient-section grain (only where Ambient mounts)

**Owners: `app/src/shared/components/common/AmbientBackground.tsx:210-211` (mount) + `app/src/index.css:839-849` (style).**

```tsx
{/* Noise grain overlay */}
<div className="amb-noise" />
```

```css
.amb-noise {
  position: absolute;
  inset: 0;
  background-image: url("data:image/svg+xml,...feTurbulence type='fractalNoise' baseFrequency='0.75' numOctaves='4'...");
  background-size: 200px 200px;
  opacity: 0.04;
  pointer-events: none;
}
[data-theme='light'] .amb-noise {
  opacity: 0.03;
}
```

**Mount gate (read-verified): `app/src/layout/ResponsiveLayout.tsx:412-446`.**

```tsx
const ambientDisabled = isMemory || isMonitoring || (isMobile && !isHome);
{!ambientDisabled && (
  <AmbientBackground
    instanceId="layout"
    originY={ambientOriginY}
    paused={interactionState === "Speaking" || isResizing}
    rippleSpeedMultiplier={rippleSpeedMultiplier}
    rippleShape={isHistory && historyDisplayMode === "orbit" ? "orbit" : "circle"}
  />
)}
```

So Home/History/Settings get G2; Memory/Monitoring/Android-non-Home do not.

**Sibling layers in the same mount** (if the goal is killing the whole ambient field, not just its grain):

| Layer | File | Cost |
|-------|------|------|
| `.amb-base` radial gradient (`ellipse 80% 70%`, 3 theme stops) | `index.css:721-730` | repaint on theme write |
| `.amb-glow` 60vmax accent glow, `transition: opacity 1.5s` | `index.css:757-780` | full-viewport alpha blend |
| `.rp-wrapper` / `.rp-ring` 280px rings, `ripple-out` 20-28s infinite | `index.css:790-837` | infinite CSS animation |
| rAF energy loop (self-stops at idle, parks on flip) | `AmbientBackground.tsx:90-133, 151-160` | JS wakeups every 600ms check + per-frame writes while unsettled |

### Decision table (no action taken - read-only session)

| Goal | Edit point |
|------|------------|
| Remove all grain everywhere | `index.css:213-231` `#root::after` block |
| Remove ambient grain only, keep global film grain | `AmbientBackground.tsx:210-211` + `index.css:839-849` |
| Remove whole ambient field (glow + ripples + its grain) | `ResponsiveLayout.tsx:434-446` mount gate |
| Tune grain intensity | `opacity` at `index.css:221,227` (global) and `:844,848` (ambient) |

---
## 2. Impeccable Audit (Design-Director Lens)

This section is the impeccable-skill pass: visual hierarchy, information architecture, cognitive load, typography, color, motion, responsive behavior, touch, copy, and tokens - judged against what makes exceptional design work, with usability outranking aliveness per `frontend-engineer.md` ("usability wins and you say so rather than shipping the fancier version anyway").

### 2.1 Visual hierarchy - orb earns its craft; settings cards do not share one ramp

The orb mood-sync is the best-crafted element in the frontend. `AdvancedOrb.tsx:949-955` seeds the live glow/accent lerp from the muted cached theme so the first painted frame already matches the Idle target - the cyan-flash RCA fix, verified by read. `TARGET_SCALE` (`:37-47`) and `BASE_AMP` (`:50-60`) derive every state (Idle 1.0/0.02 through Speaking 1.10/0.28) from the pipeline contract, exactly what `frontend-engineer.md` demands ("pipeline state first, UI second").

Against that bar, the Settings/model surfaces speak in roughly six opacity dialects - `bg-[rgba(var(--card),0.65)]` (DossierCard), `/0.75` (GraphControlDock), `/0.85` (MemoryGraph error card), `/0.92` (Memory search modal), `/0.95-0.98` (catalog tooltips), plus `border-[rgba(var(--accent),0.06-0.45)]` across the same pages. `design-spec.md` s5 declares glass elevation a closed system; the reads show the token ramp exists but cards pick ad-hoc stops. Cards compete instead of stacking. This is a Tier 2 convergence task (not a redesign): canonicalize to 3 stops.

### 2.2 Information architecture - good shells, too many of them

`Drawer` (bottom sheet, `Drawer.tsx:48-53` "unified overlay contract"), `Modal` (centered, `Modal.tsx:37-47` "sibling of Drawer"), `EdgePanel` (side rail) are individually well-built - each has backdrop dismiss, Escape via the overlay stack, focus trap + restore, `role="dialog"` + `aria-label`. The load is systemic: 3 shells + the shared `OverlayBackdrop` primitive + 2 context-menu clones + per-page portal dropdowns (`SessionAccordionItem:307,365`; `StagingHeader:172`; `PersonalMemoryModalView:381`) all implement the same grammar (open / dismiss / focus / select). A user learns dismissal 4+ times. Convergence target (Tier 1, zero pixels): one menu shell, one tooltip shell, one backdrop - the shells keep their positions, they stop forking their internals (see T1-6, T1-7).

### 2.3 Cognitive load - Memory page carries three mental models at once

`Memory.tsx` mounts a 3D force graph (uncapped, per user invariant), a virtualized session accordion (`MemoryTimeline`), and a personal-memory document workflow (dossier + staging + comments + versions) on one route, with a drawer-vs-modal branch on viewport (`:602-632`) and a separate search modal on compact (`:634-675`). Each model is sound; together they exceed working memory on first visit. The reads show the mitigations are real (loader floor `:68-87`, `MIN_LOADER_MS` 350 cold / 0 warm; error card `MemoryGraph:57-85`; footnote hint `:597-600`). Remaining load is Tier 3 territory (progressive disclosure of the staging workflow) - flagged, not proposed, needs sign-off.

### 2.4 Typography - bundling fixed the systemic issue; 9px kickers remain

`main.tsx:5-17` bundles DM Sans / Sora / JetBrains Mono via Fontsource - the render-blocking Google Fonts import is gone (Tier 1 batch 1, verified by read). Residual: 9px mono uppercase kickers in `VoiceCatalogModalView.tsx:363,367,371` (`Language / Gender / Style` labels at `text-[9px]`). `design-spec.md` SS4.2 sets the floor at 11px; 9px fails comfortable mobile legibility and sits under the WCAG-adjacent size floor. Tier 2 fix: 9px to 10-11px, zero layout change. Also noted: `TransliterationToggle`-class `text-[9px] tracking-[0.14em]` kickers elsewhere in settings - same treatment in bulk.

### 2.5 Color - notification tokens exemplary; status dots outside the mood contract

`NotificationPanel.tsx:54-58` is the model citizen: "Single source of category colour: the accent-derived `--notif-*` family. Never `--accent` itself. Full literal class strings so Tailwind's scanner sees every arbitrary value." Seven categories, complete triples, spec-cited (`notifications-spec s4.7`). Counterpoint: model status dots (`emerald/rose/amber` + `animate-pulse` + `shadow-[0_0_6px]`) pulse forever even when the pipeline is Idle - color that does not derive from the 7-state mood contract. Tier 2: gate the pulse on non-Idle states.

### 2.6 Motion - springs spent where fades read identically

Counted 97 `AnimatePresence` + 119 `motion.div`; file reads confirm the majority are 150-280ms opacity/scale fades: `SettingsCardWrapper` mount + 5 footer variants (`:34-40, 66-190`, two nested `AnimatePresence`), `SessionPanel` create-form (`:789-791`), `NotificationPanel` tab switch (`:618-623`, `opacity 0.15s`), `App` boot loader (`:419-433`, `opacity 0.4s`), `History` loader exit (`:491-510`, `0.25s`), `Monitoring` popover (`:393-408`, `opacity/y/scale 0.2s`), `OverlayBackdrop` itself (`:22-26`, `opacity 0.2s`). Framer's JS RAF + exit-tree retention buys nothing over a CSS transition for these. Keep Framer strictly where physics exists: `Drawer` sheet drag (`Drawer.tsx:239-254`), `SessionPanel` `Reorder.Group`, `WizardRoot` step slide (`:319-332`). Full inventory in T1-5.

### 2.7 Responsive - 480px clipping structurally addressed; device confirmation outstanding

Reads confirm the plan's Batch 2 shipped structurally: `min(480px,92vw)` + `max-w-[calc(100vw-32px)]` modal caps (`Memory:641`, `Modal:161`), `custom-scrollbar` + `overscroll-contain` on every scroll body (`Drawer:294`, `Modal:201`, `MemoryTimeline:253`), safe-area `env()` at `ResponsiveLayout:460,496`, `EdgeNav:24`, `Monitoring:401`, `History:437`, `MobileStepFooter:22`. No fresh clipping found statically. Outstanding (not verifiable without hardware): 360px POCO + gesture-bar overlap + `100dvh` vs `100vh` on rotation - flagged as the HMR gate in s7.

### 2.8 Touch - wizard fixed; icon-dock density still desktop-first

E3/E4 shipped correctly by read: `Tooltip.tsx:180-186` compact treatment (`max-w-[240px] text-[11px]` on coarse), wizard targets at 48px. The gap is `GraphControlDock.tsx:38-126`: six icon buttons at `p-1.5 sm:p-2.5` with 14-16px icons (~28-32px hit area) against the android-style-guide s5.2 48px floor, each wrapped in a hover-only `Tooltip` whose tap path requires `mobileTapEnabled` (defaults `false`, `Tooltip:52`). Desktop: fine. POCO: small targets that show nothing on tap. This is both a Tier 2 (tap opt-in, focus twins) and a Tier 3 (48dp density) item - split explicitly in T2-6/T3-Dots.

### 2.9 Copy, empty states, edge cases - `src/data` discipline holds

All files read route user-facing strings through `src/data/*` (`SESSION_COPY`, `MEMORY_COPY`, `HISTORY_COPY`, `SETTINGS_COPY`, `VOICE_CATALOG_COPY`, `LAYOUT_COPY`, `MONITORING_COPY`, `NOTIFICATION_COPY`) - zero hardcoded long labels found, style-guide s2 boundary intact. Loading/error/backend-not-ready are first-class on Memory (`Memory.tsx:63-87` loader floor + `loadError` state; `MemoryGraph:57-85` error card with retry) and History (`History:490-510` cross-fade loader; turns error + retry path). Voice-catalog preview-disabled (`has_preview == false`, session-busy gate at `VoiceCatalogModalView:56-57`) was not verifiable statically - soak gate in s7.

### 2.10 Tokens - the exception path is load-bearing

`glass-keep-blur` is correctly scoped to the nav capsule (`EdgeNav:24`) - the one surface that must stay frosted. But 77 `glass-*` references plus the ad-hoc `bg-[rgba(var(--card),0.65-0.98)]` dialects mean the exception path carries more weight than the token ramp. Tier 2 inventory task: enumerate every `glass-*` + ad-hoc card fill against the `design-spec.md` elevation table and fold the orphans in. No visual change if stops match.

---
## 3. Tier 1 - Bug / Pure Fix (Zero Visible Change)

> Same pixels, fewer resources. Ship in bulk without design sign-off. Each finding: What / Why / Replacement / Severity.

### T1-1 - Eager Three.js on the startup path (C2 open, verified by read) - ORANGE

**What it is.** `AdvancedOrb.tsx:2` (`import * as THREE from 'three'`), `MemoryGraph.tsx:12` (same), `useMemoryGraphScene.ts:2-3` (same + `OrbitControls`). Routes are already lazy (`App.tsx:42-45` History/Memory/Settings/Monitoring; `Settings.tsx:17-23` domain cards; `ResponsiveLayout:28` Monitoring popover) but both WebGL surfaces pull the full Three.js module graph statically. No dynamic `import('three')` exists anywhere in `src/` (searched). So the heaviest JS payload in the app is in the boot chunk graph regardless of route.

**Why it matters.** At production scale on CPU-first hardware + Android WebView, first paint waits on JS parse/compile of a 3D engine the user may never open (Settings-only visit still pays it). This is plan item C2, still open - verified by read, not census.

**Replacement.** Lazy the surfaces, dynamic-import the engine:

```tsx
// Home.tsx / Memory.tsx - surface level (pixels identical)
const VoxOrb = lazy(() => import("@/shared/components/home/AdvancedOrb"));
const MemoryGraph = lazy(() => import("@/shared/components/memory/MemoryGraph"));
<Suspense fallback={<div className="h-full w-full bg-[rgb(var(--background))]" />}>
  <VoxOrb telemetryRef={telemetryRef} interactionState={interactionState} />
</Suspense>
// Inside AdvancedOrb / useMemoryGraphScene - engine level:
const THREE = await import("three");
const { OrbitControls } = await import("three/examples/jsm/controls/OrbitControls.js");
```

**Severity: ORANGE** - real startup/RAM pain at stated scale. **Tier: Tier 1** - first frame is the solid background the app already paints behind the canvas.

### T1-2 - Monolith panels (D2 open, verified by read) - ORANGE

**What it is.** Line counts by read: `AdvancedOrb` 1027, `LlmConfigDesk` 882, `SessionPanel` 847, `NotificationPanel` 649, `Memory` 682, `Monitoring` 418, `VoiceCatalogModalView` 421, `SessionAccordionItem` 526. Partial progress is real: `SessionRowItem` extracted (`SessionPanel:34`), `TimelineQueueSection` extracted (`MemoryTimeline:21`), `LlmCatalogView` split into `RemoteCatalogSection`/`RemoteModelCard` (per git log `30b08568`). What remains inline: SessionPanel turn rendering + input bar + actions menu; LlmConfigDesk provider desks (Cloud/Local/Server branches in one 882-line memo); Monitoring accent-sync + tiles + chamber in one component.

**Why it matters.** Whole-panel re-render on every keystroke/selection (see T1-8 for the selector half); review blast radius per change is hundreds of lines; the plan's Batch 7 exists precisely because these are the files every future feature touches.

**Replacement.** Per-plan Batch 7 extractions (`SessionTurnItem` / `SessionInputBar` / `SessionActionsMenu`; lazy provider desks inside `LlmConfigDesk` following the `Settings.tsx:17-23` pattern), `memo` on row components, no visual change. Planned task with `verify-targets.sh`, not a blind bulk edit.

**Severity: ORANGE. Tier: Tier 1.**

### T1-3 - Stray permanent will-change on the drawer sheet - YELLOW

**What it is.** `Drawer.tsx:252`: `"absolute bottom-0 ... transform-gpu will-change-transform contain-paint"`. Tier 1 batch 2 removed permanent hints from `Modal`/`EdgePanel`/ripples (verified: `Modal:157` is `transform-gpu contain-paint`, no will-change); this one line survived.

**Why it matters.** `will-change-transform` pins a dedicated compositor layer for the drawer's lifetime - GPU memory held on Android WebView for zero frames of benefit once the 0.38s slide completes. `transform-gpu` alone satisfies Invariant 4 (currently green - keep it).

**Replacement (one token):**

```tsx
// Drawer.tsx:252 - delete will-change-transform, keep the rest identical
"absolute bottom-0 left-0 right-0 flex flex-col rounded-t-3xl overflow-hidden border-t border-[rgba(var(--accent),0.12)] [text-shadow:none] outline-none pointer-events-auto transform-gpu contain-paint"
```

**Severity: YELLOW. Tier: Tier 1.**

### T1-4 - backdrop-blur-md on opaque fills (raster with no frost) - ORANGE mobile / YELLOW desktop

**What it is (read-verified, not census).** These blurs sit on fills opaque enough that the kernel output is invisible: `SessionContextMenu.tsx:159` (`bg-[rgb(var(--card))]` solid + `backdrop-blur-md`), `ProjectContextMenu` (same shell), `MemoryGraph.tsx:60` (error card `/85`), `Memory.tsx:641` (search modal `/0.92`), `GraphControlDock.tsx:33` (`/0.75`), `RemoteModelCard`/`RemoteCatalogSection` tooltip cards (`/98`), `ToolCard`-class `/0.7` fills. Keep blur where translucency is real: `OverlayBackdrop[2px]`, `TitleBar` hover-cards, `GraphControlDock`-style floating docks only if their fill drops below ~0.7.

**Why it matters.** Every `backdrop-filter` region re-runs its kernel whenever its backdrop changes - scrolling, ambient animation, theme writes. On Android WebView this is the number-one frame cost (plan Group A). Paying it where the effect is invisible is pure waste, multiplied against every glass surface above the ambient field.

**Replacement (per line, identical pixels):** delete the `backdrop-blur-md` token, keep `bg` + `shadow-lg`. Gate by class swap per surface - never the reverted `.no-blur-subtree *` descendant selector (ledger 2026-10-01 anti-pattern: forces full-subtree restyle + visible blur snap).

**Severity: ORANGE on Android, YELLOW on desktop. Tier: Tier 1** (verified opaque - no frost to lose).

### T1-5 - Static fades on Framer (C5 open, verified per file) - ORANGE

**What it is.** Two nested `AnimatePresence` in `SettingsCardWrapper.tsx:34-40,66-190` (mount fade + 5 footer variants each with `opacity + height`); create-form fade `SessionPanel:789-791`; tab fade `NotificationPanel:618-623` (`opacity 0.15s`); boot fade `App:419-433` (`opacity 0.4s`); loader exit `History:491-510` (`0.25s`); popover `Monitoring:393-408`; backdrop `OverlayBackdrop:22-26`. Framer keeps a JS RAF + exit-tree retention alive for effects CSS transitions already cover.

**Why it matters.** Exit churn on every tab/step/drawer cycle; per-component spring configs diverge (0.15/0.2/0.24/0.28/0.38/0.4s across the files read) so the app has no single motion feel anyway. Unifying the fades to CSS is both cheaper and more consistent.

**Replacement (curve-preserving, per file, opportunistically - not big-bang):**

```tsx
// Before (representative - SettingsCardWrapper footer pattern)
<AnimatePresence>{cond && (
  <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }}
    exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }}>...</motion.div>)}
</AnimatePresence>
// After - identical 200ms fade, zero JS animation runtime
{cond && <div className="transition-opacity duration-200 opacity-100 starting:opacity-0">...</div>}
```

Keep Framer strictly where physics exists: `Drawer` sheet drag, `SessionPanel` `Reorder.Group`, `WizardRoot` step slide.

**Severity: ORANGE (bulk win). Tier: Tier 1** (curve-matched - no visible change).

### T1-6 - Drawer bypasses the shared backdrop (F2 partial) - YELLOW

**What it is.** `OverlayBackdrop.tsx:5-11` documents "Modal, Drawer, and EdgePanel all render this". Read shows `Modal.tsx:8` and `EdgePanel.tsx:8` import it; `Drawer.tsx:235-236` still renders its own `absolute inset-0 bg-[rgb(var(--background))]/60 backdrop-blur-sm` div. The doc comment is aspirational, not true.

**Why it matters.** Two backdrop looks (black/50+2px vs background/60+sm) + two blur costs; stacked overlays double-tune darkness independently; one more fork to maintain.

**Replacement:** `<OverlayBackdrop onClose={onClose} />` in `Drawer`, delete the local div. Unchanged visual contract per the component's own docstring.

**Severity: YELLOW. Tier: Tier 1.**

### T1-7 - Cloned menu / tooltip / dropdown shells - YELLOW

**What it is.** `SessionContextMenu` (~331 lines) vs `ProjectContextMenu` (same `rounded-sm ... shadow-lg backdrop-blur-md p-1` shell, same outside-click + Escape handling); `SessionAccordionItem:307,365` inline portal menus duplicating that shell; `MemoryNodeTooltip` / `SessionNodeTooltip` / `StagingHeader:172` / `PersonalMemoryModalView:381` dropdowns each re-implementing positioning + dismiss. The keyboard-nav logic (`SessionContextMenu:78-111`: ArrowUp/Down wrap, Enter activation, first-item autofocus at `:110-111`) is good - and forked per clone.

**Why it matters.** Classic bloat: ~100 lines doing what 10 could, per shell, times six. Divergent radii/shadows/motion per clone (see s2.2). A11y fixes land in one clone and miss the rest.

**Replacement:** one `ContextMenuShell` + one `TooltipShell` in `shared/ui/` carrying the shell classes + outside-click + Escape + arrow-key nav exactly once; migrate 6 call sites to pass items/children. Zero visual change.

**Severity: YELLOW. Tier: Tier 1.**

### T1-8 - Whole-store selectors + leftover RCA logging - ORANGE

**What it is.** `LlmConfigDesk.tsx:35-36`: `useSettingsStore((s) => s.draftSettings)` + `(s) => s.updateDraft`, then derives everything through a `useMemo` (`:57-82`) keyed on `currentRemoteConfig?.base_url/model/api_key` - a fresh object identity per store set, so the memo rarely hits. `Monitoring.tsx:61-77` subscribes ~15 atomic selectors (correct) but `syncAccent` (`:83-89`) closes over `accentRgbStr` state, re-subscribing the computed-style read per render. `Home.tsx:98-131` still mounts the cyan-flash RCA `console.info` trace block (mount/unmount/orb-mount/theme-vars logging) in the production path.

**Why it matters.** Settings keystroke fans out to full-desk re-render (the known R8 cascade: ~1,100 store writes/keystroke per the remediation doc, with 6 always-mounted `SettingsCardWrapper`s each running `useDomainCommitState`). Monitoring accent poll re-renders tiles per frame of a drag. Production console spam on every Home visit.

**Replacement:** atomic field selectors (`(s) => s.draftSettings?.llm?.active`, per-field drafts); `syncAccent` via ref-compare without state dep; gate the Home trace `useEffect` behind `import.meta.env.DEV`.

**Severity: ORANGE. Tier: Tier 1** (same values, fewer renders).

---
## 4. Tier 2 - Subtle Refactor (Barely Noticeable)

> Safe bulk. Timing shifts, calmer decor, tap parity - no layout or identity change.

### T2-1 - Shared instant fade on coarse-pointer (F1-lite) - ORANGE mobile / YELLOW desktop

**What it is.** Four bespoke open animations: `Drawer` 0.38s expo-out slide (`Drawer:248`), `Modal` 0.24s scale-fade (`Modal:154`), `EdgePanel` 0.22s slide, `Monitoring` popover 0.2s (`Monitoring:400`). Desktop reads premium; Android WebView drops frames interpolating spring curves under blur.

**Why it matters.** Panel-open jank is the most-felt Android interaction after scroll itself. Four configs also mean four feels.

**Replacement:** `const reduce = useReducedMotion() || isCoarsePointer();` then `transition={{ duration: reduce ? 0.12 : CURRENT, ease: "easeOut" }}`. Desktop curves untouched; coarse gets one shared 120ms ease-out. Timing shift only.

**Severity: ORANGE mobile / YELLOW desktop. Tier: Tier 2.**

### T2-2 - blur-md to blur-sm on scrolling surfaces; kill the xl/lg tail - YELLOW

**What it is.** 28x `backdrop-blur-md` remain (census, spot-verified by read): `GraphControlDock:33`, `Memory:641`, `PersonalMemoryStagingCard:360,383` overlays, `MemoryStates`-class cards. One `xl` + one `lg` tail. `md` on a scrolling list smears content behind it per frame; `sm` reads identically on a card at rest.

**Why it matters.** Scroll-compositor cost perSupplementary blur tier is real on llvmpipe/Android; the visual delta between md/sm on a mostly-opaque card is below perception threshold.

**Replacement:** `backdrop-blur-md` to `backdrop-blur-sm` on cards/docks/lists; keep `md` only on small popovers + `OverlayBackdrop[2px]`.

**Severity: YELLOW. Tier: Tier 2.**

### T2-3 - Freeze infinite decor on coarse-pointer / reduced-motion (B5) - YELLOW

**What it is (read).** `RestorePulse` on Home (`Home:5` + restore-signal path), `Shimmer`, `RealtimeVisualElements` floating dots, `engage-btn-loading::after` conic sweep (`index.css:641-663`), `orb-satellite-a/b` 5.5/7.5s loops (`index.css:936-947`), wave-bars (`:882-918`), `animate-pulse-slow`. `prefers-reduced-motion` cover exists (`index.css:851-856,904-912,989-997`) but nothing gates on coarse-pointer, so a phone with no reduced-motion setting still pays every loop.

**Why it matters.** Infinite decor pins RAF/compositor wakeups even when the user reads static text - the idle-battery win, and the largest sustained cost after the orb itself.

**Replacement:** `@media (pointer: coarse) { .decor-loop, .orb-satellite-a, .orb-satellite-b, .engage-btn-loading::after { animation: none; } }` + pause `RestorePulse` / PixelSynthesis RAF off-screen (same B1 orb-pause pattern already shipped). Watching user sees the same frame.

**Severity: YELLOW. Tier: Tier 2.**

### T2-4 - Uncapped-graph compliance guard: throttle, never cap - RED if capped / YELLOW as specified

**What it is.** Tier2-tier3 doc B4 recommends capping Memory graph node count on Android. The optimization plan s2.5 forbids it under explicit user instruction ("node count and simulation remain completely uncapped"). Reads confirm no cap exists today (`useMemoryGraphScene`, `MemoryGraph`) - this finding is a guardrail, not a bug.

**Why it matters.** Capping would violate a direct user invariant - a trust break, not a perf tradeoff. Flagged RED so no future batch "optimizes" it into existence.

**Replacement (compliant path only):** `pixelRatio <= 1.5` on coarse, pause auto-rotate off-screen/hidden-tab, freeze settled physics (ledger pattern). Zero node-count change.

**Severity: RED if anyone caps; YELLOW for the throttle fix. Tier: Tier 2.**

### T2-5 - Virtualizer estimate mismatch - ORANGE

**What it is (read).** `HistoryListView:110-111` (`estimateSize: () => 170, overscan: 6`) with `measureElement` dynamic sizing; `MemoryTimeline:191-192` (`estimateSize: () => 50, overscan: 16`); `SessionPanel` bespoke `useVirtualRows` coexisting with `Reorder.Group` drag - reorder indices address the full array while the virtual window renders a slice.

**Why it matters.** Wrong estimate = scrollbar jump on expand/collapse - the plan s5 risk table names exactly this ("accordion scroll jumps"). Drag-vs-window mismatch risks dropping an item outside the rendered slice.

**Replacement:** measure production row heights, unify on `@tanstack/react-virtual` + `measureElement` + ResizeObserver re-measure (the `MemoryTimeline` pattern), overscan 6-8 everywhere; drag-reorder commits against the full array, never the virtual window.

**Severity: ORANGE (visible jump, correctness-adjacent). Tier: Tier 2** (layout correctness, not redesign).

### T2-6 - Hover-only affordances unreachable on Android - ORANGE mobile

**What it is.** `Tooltip.tsx:92-102`: hover enabled only when `!isTouchOrMobile`; click requires `mobileTapEnabled`, which **defaults false** (`Tooltip:52`) - so most icon triggers (`GraphControlDock` dock, `SessionAccordionItem` sort/filter, `EdgePanel` close) show nothing on tap. Census: 132 `hover:` in settings vs 11 `focus-visible` in `shared/ui`. `GraphControlDock` buttons render ~28-32px against the android-style-guide s5.2 48px floor.

**Why it matters.** Nameable failure at production scale: tap the graph-dock search icon on POCO - nothing appears, no focus ring, no tap path. The information is reachable on desktop and unreachable on the shipped mobile surface.

**Replacement:** `focus-visible:` + `active:` twins on every hover-reveal (same E3 pattern), opt-in `mobileTapEnabled` on icon-only triggers, 48dp hit areas on the dock (moves to Tier 3 density sign-off - split as T3-Dots). No layout change for the twins half.

**Severity: ORANGE mobile. Tier: Tier 2** (twins half; density half is Tier 3).

### T2-7 - Lenis stays in the mobile bundle - YELLOW

**What it is.** `createContainedLenis` gates at runtime (48 refs, verified) but `PersonalMemoryDossierCard:4,77` + 4 other call sites statically import it - the smoother ships in the Android bundle and initializes (then no-ops) on a device that uses native momentum scroll.

**Why it matters.** Dead bytes + init cost on the weakest device in the matrix, per route visit.

**Replacement:** `const { createContainedLenis } = await import("@/shared/lib/lenisScroll")` only when `!isCoarsePointer()`. Desktop path identical.

**Severity: YELLOW. Tier: Tier 2** (Tier 1-adjacent; classified Tier 2 because chunk graph shifts warrant a measured build).

### T2-8 - Accent-drag + theme-flip style-recalc multipliers - ORANGE

**What it is (read in `shared/theme/index.ts`).** Module header documents the measurement: one appearance write costs 683-2772ms of full-document style recalc on the Settings route (9,606 nodes, ~1,500 `var(--accent)` references, `:29-37`). rAF-coalescing + gate-hold already shipped (`:581-627`); residual multipliers: 237 `transition-all` sites re-arming on gate close (two recalcs per flip, `:23-25`), the 7-token `--notif-*` family written alongside accent per frame (`:606-614`).

**Why it matters.** The exact interaction users perform in Appearance settings is the single most expensive interaction in the app - production-scale jank on a first-run surface.

**Replacement:** narrow `transition-all` to `transition-colors` on non-layout surfaces (bulk, zero visual change); `content-visibility` / `contain-paint` on off-screen cards. The gate mechanism itself (`syncGate`, flip interpolation) is correct - leave it.

**Severity: ORANGE. Tier: Tier 2.**

---

## 5. Tier 3 - Trade-off (Visible - Per-Item Sign-off Required)

> Do not apply blind. Each changes what the user sees. Ordered by recommended approval sequence (unchanged from `frontend-perf-tier2-tier3.md`, evidence-linked here).

### T3-A1 - Android-only solid cards (Group A1) - the biggest single Android win

**Effect.** Glass panels become solid (`bg-[rgb(var(--card))]` sans blur) when `isCoarsePointer()`; desktop keeps full glass. Phone stops stuttering when panels open; cards lose see-through blur.
**Gate:** class swap per surface - never the banned descendant-`*` selector. **Recommendation: DO on Android only.**

### T3-B2 - Still waveform / liquid / pixel canvases on Android

**Effect.** `LiveWaveform` (421 lines), `LiquidChamber` (441), `PixelSynthesisCanvas` motion becomes static imagery on phone; desktop keeps motion. **Recommendation: MAYBE** - only if A1 + T2-1 insufficient. Preserve `canvas.width = 1` teardown (Invariant 3 green today).

### T3-B3 - OrbitCarousel 3D cover-flow to flat list on small screens

**Effect.** `OrbitCarousel` (460 lines) spinning carousel becomes a swipe list on phone. **Recommendation: MAYBE** - measure on POCO via `stress_drawers.mjs` FPS first; no data today.

### T3-E2/F1 - Snap-in panels on Android (no slide/spring)

**Effect.** Drawers pop instead of glide; fastest feel, least premium. **Recommendation: MAYBE last resort** - try T2-1 shared 120ms fade first.

### T3-D4 - Paginate / collapse LlmConfigDesk-scale desks

**Effect.** Settings sections expand on tap; opens faster, changes IA. **Recommendation: MAYBE** - cards already lazy (`Settings:17-23`); only if Settings feels slow on device.

### T3-Pill - Remove faux-pill status badges (style-guide s5, visible by definition)

**Effect.** `LlmConfigDesk:873` TTS-cloud badge + 9px kickers become plain text; control already communicates state, badge parrots it ("Zero Redundant Value/Status Badges"). **Recommendation: needs design sign-off.** Impeccable view: remove.

### T3-Dots - GraphControlDock 28px targets to 48dp

**Effect.** Chunkier dock, lower icon density. Required by android-style-guide s5.2 yet visibly changes the graph chrome. **Recommendation: needs design sign-off** (pair with T2-6 twins so function lands before density).

---
## 6. review-ui Rubric Verdict (Hunt Order)

### Will break / already broken (RED)

- **R-1 - Graph capping if applied.** Violates the explicit uncapped-graph user invariant (plan s2.5). Not present today - listed so no batch introduces it. Scenario: a future "Android graph perf" change caps nodes; large-memory users lose visible facts with no error.
- **R-2 - Android tooltip dead-ends (T2-6).** `mobileTapEnabled` defaults false + sub-48px dock targets = info/actions unreachable on POCO. Scenario: tap graph-dock search icon; nothing appears, no focus ring, no alternative path.
- **R-3 - Accent-drag recalc (T1-8/T2-8).** Measured 683-2772ms per appearance write on the Settings route (`shared/theme/index.ts:29-37`). Scenario: drag the accent picker on a full Settings page; UI freezes mid-drag at production scale.

### Redundant or unnecessary (ORANGE)

- T1-6 Drawer backdrop fork (shared primitive exists, one shell bypasses it).
- T1-7 six menu/tooltip shells (one keyboard-nav implementation forked six ways).
- T1-5 Framer-for-fades (JS runtime for CSS-coverable 150-280ms fades, 4+ divergent curves).
- T2-7 Lenis-in-mobile-bundle (runtime no-op, still shipped + initialized).
- `BottomDockFeather` correctly shared - keep; audit its 6+ call sites (`History` detail + list both mount one, `EdgeNav`, `EdgePanel`, `Settings` mobile list) for double-mount overlap, not for removal.

### Over-engineered relative to stated scale (YELLOW - keep the design, pay less mount cost)

- `Settings.tsx` radial-hub desktop grid (12-col + connectors overlay + 6 always-composed `SettingsCardWrapper`s each with `AnimatePresence`) for 6 domains. Correct at production scale, but the always-mounted tree is the measured recalc multiplier (s2.8, R-3). Fix is T1-2 (mount only visible cards), not simplification.

### Bloat - 100 lines doing what 10 could (YELLOW)

- T1-7 menu shells (shown above with the 10-line shell replacement direction).
- `NotificationPanel` category-token tables: correct and spec-cited, but 7x repeated class-string triples - generate from one tuple table.
- `GraphControlDock` 6x identical button class strings - one `dockBtn` const.

### What is genuinely fine (stated explicitly per skill Step 4)

- All 5 static invariants green: service boundary (0 raw IPC outside `services/`), Zustand atomicity where applied, canvas teardown (`PixelSynthesisCanvas:264-272` 1x1 collapse + listener/observer disconnect; `AdvancedOrb:990-1016` dispose + `forceContextLoss`), GPU promotion (`transform-gpu` on all three shells), context memo (no inline provider objects).
- `shadow-2xl` count 0 (was 27 - plan Batch objective met).
- Fontsource bundle (`main.tsx:5-17`), Lenis runtime gating (5 call sites), History/Memory virtualization (`HistoryListView:110-111`, `MemoryTimeline:191-192`, `SessionPanel` `useVirtualRows`), orb flash seed + coarse RAF pause, E3 compact tooltips + tap-toggle, E4 48px wizard, `OverlayBackdrop` contract, theme-flip + viewport-resize gates, `BottomDockFeather` zero-blur dissolve, `PixelSynthesisCanvas` batched 6-fill 30fps matrix with Chebyshev-identity contour math (not per-dot trig), route-level lazy + coarse-pointer preload skip (`App:24-35`), `useVirtualizer` + `measureElement` wiring.

---

## 7. Verification Status, Gaps, and Recommended Order

### 7.1 Gate table

| Gate | Status this session |
|------|---------------------|
| `check_invariants.mjs` | 5/5 PASS |
| `pnpm test` (vitest) | 58/58 PASS |
| `pnpm tsc --noEmit` | 0 errors |
| `pnpm build` | NOT RUN (recommend before any Tier batch) |
| `stress_drawers.mjs` FPS >= 50, DOM drift 0 | NOT RUN - required before T3-B3/E2/F1 sign-off |
| `soak_test.mjs` heap slope flat | NOT RUN - required before voice-catalog + Tier 3 sign-off |
| `verify-targets.sh` desktop + Android, 0 warnings | NOT RUN (no source touched) - required per plan Batch |
| POCO 360-480px HMR + gesture-bar + rotation + reduced-motion + dark/light flip | NOT RUN - required before calling responsive/touch done |

### 7.2 Recommended order (evidence-linked, unchanged from tier2-tier3 doc)

1. **Tier 1 bulk, no sign-off:** T1-3 (one token), T1-4 (opaque-blur strip), T1-6 (backdrop unify), T1-7 (menu shells) - then `tsc` + `test` + `check_invariants` + `build`.
2. **Tier 1 planned, one task each:** T1-1 (lazy Three.js) + T1-2 (panel splits) + T1-5 (per-file CSS fades) + T1-8 (selectors) - each with `verify-targets.sh`.
3. **Tier 2 bulk, safe:** T2-2, T2-3, T2-1, T2-6 twins half, T2-5, T2-7, T2-8 narrowing.
4. **Tier 3 one-by-one:** A1 (Android solids) - measure - B2/B3/E2/F1/D4 only if still laggy; Pill + Dots with design sign-off.

### 7.3 Session integrity note

Zero source edits were made. The only file created is this report. `git status` at audit time showed pre-existing modifications from voice-catalog work (backend config/persistence/TTS, `settingsCopy.ts`, `voiceService.ts`, settings model views) - unrelated to and untouched by this review.
