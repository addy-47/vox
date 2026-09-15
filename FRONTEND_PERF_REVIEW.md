# Frontend Adversarial Perf Review — Help, Edge Panels, Memory Page UI

**Date:** 2026-09-15 · **Method:** create-sprints (4 sprints, 3 parallel subagent passes + lead sweep) · **Mode:** READ-ONLY, no code changed
**Checklist:** `temp/frontend-perf-checklist.md` (37/37 units read in full)

---

## Review Summary

**Calibration: reviewing as production single-user desktop software — critique is calibrated accordingly.**
Vox is a realtime voice AI Tauri v2 desktop app (8GB RAM, CPU-first inference, iGPU compositing, sub-200ms perceived latency).
There is no multi-user scale argument here; every finding is priced in main-thread CPU, compositor layers, VRAM on shared
memory, and bundle-on-open. Cleverness that costs a perpetual JS wakeup for a decorative illustration is a bug at this scale.

**Visual-impact tiering WAS applied** (all code in scope is UI/rendering):
Tier 1 🟢 pure fix (same pixels, fewer resources) · Tier 2 🟡 subtle refactor (barely noticeable) · Tier 3 🔴 visible trade-off (needs sign-off).

**Headline:** no hard crash was found in Help or Edge scope. Two 🔴-class defects were found in Memory scope
(per-node `getComputedStyle` in the buffer loop; line-buffer realloc + GPU orphan per keystroke), plus one genuine
animation-stall bug outside the core scope (`LiquidChamber` never restarts its rAF after backgrounding). Everything else
is 🟠 sustained-cost (infinite JS animation drivers, stacked backdrop blurs, DPR 2 + MSAA over two canvases) and
🟡 mechanical hygiene. The single most repeated anti-pattern across all three surfaces: **framer-motion infinite loops
where CSS keyframes would do, with no visibility gating and no `prefers-reduced-motion`.**

**Sprint coverage:** Sprint 1 Help (15 files) · Sprint 2 Edge panels (7 files + overlay/tooltip/cluster support)
· Sprint 3 Memory page (15 files incl. 1065-line `useMemoryGraphScene.ts`) · Sprint 4 opportunistic sweep
(`rg` over `app/src` for rAF/setInterval/blur/layoutId/infinite + targeted reads of `LiquidChamber`, `VoiceCarousel`, `OrbitalLoader`).

---

## 🔴 Will Break

### M-F1 — Palette regenerated per node inside the buffer update loop (hottest loop on the page)
- **Where:** `app/src/shared/hooks/useMemoryGraphScene.ts:197-205` (`getCollectionColor` → `getActiveDynamicPalette` → `getComputedStyle` + 6× `new THREE.Color`, per node per keystroke)
- **Why it matters:** 500 nodes = 500 forced style recalcs + ~3000 `Color` allocs per filter keystroke. Typing stutters orbit on iGPU; scales O(N) with the one dataset guaranteed to grow.
- **Replacement:** hoist once per update:
```ts
const palette = getActiveDynamicPalette(isLight);
const byCat: Record<string, string> = { personal: palette.personal.main, objective: palette.objective.main, workdone: palette.workdone.main, blocker: palette.blocker.main, next_step: palette.next_step.main, pitfall: palette.pitfall.main };
gNodes.forEach((node, i) => { const colHex = byCat[node.collection] ?? palette.objective.main; /* …no getCollectionColor call… */ });
```
- **Severity:** 🔴 · **Tier:** Tier 2 🟡 (typing/filter stutter)

### M-F2 — Line-highlight rewrite allocates + orphans GPU buffers on every selection/search change
- **Where:** `useMemoryGraphScene.ts:275-279` (`new Float32Array` ×2, `new THREE.Color` ×2), `:369` (`colorHelper.clone()` per link), `:371` (`new THREE.Color(0xf8fafc)` per link), `:388-389` (`setAttribute(new THREE.BufferAttribute…)` without disposing old)
- **Why it matters:** defeats the preallocated 40k-line buffers; each keystroke allocates MBs and orphans GPU attributes → VRAM creep + GC pauses on shared-memory iGPU over long sessions.
- **Replacement:** allocate once, write into existing attributes, hoist scratch colors:
```ts
const geo = lineSegments.geometry as THREE.BufferGeometry;
const posAttr = geo.getAttribute("position") as THREE.BufferAttribute;
const colAttr = geo.getAttribute("color") as THREE.BufferAttribute;
(posAttr.array as Float32Array).set(posArray.subarray(0, writePtr));
(colAttr.array as Float32Array).set(colArray.subarray(0, writePtr));
posAttr.needsUpdate = true; colAttr.needsUpdate = true;
geo.setDrawRange(0, writePtr / 3);
// in link loop: SCRATCH.copy(colorHelper).lerp(LIGHT_BG, 0.12) — no clone/new per link
```
- **Severity:** 🔴 · **Tier:** Tier 1 🟢 (invisible until it stutters/leaks)

### X-F1 (Sprint 4) — `LiquidChamber` animation permanently stalls after one backgrounding
- **Where:** `app/src/shared/components/monitoring/LiquidChamber.tsx:121-126` vs `:327-336`
- **Why it matters:** in-render `if (document.hidden) { rafId = 0; return; }` kills the rAF chain with `running` still `true`; the `visibilitychange` restart only fires `else if (!running)` → never restarts. One tab backgrounding = dead monitoring canvas until `open`/`popover` dep changes. Deterministic, nameable: background the window once, chamber freezes.
- **Replacement:**
```ts
// in render(): schedule a wakeup instead of dying —
if (document.hidden) { rafId = requestAnimationFrame(render); return; }
// or set running=false in the hidden branch so onVisibility restarts it
```
- **Severity:** 🔴 · **Tier:** Tier 3 🔴 (frozen canvas is visible; fix itself restores intended motion)

### M-F3 — `Math.min(...spread)` over unbounded session arrays throws at scale
- **Where:** `useMemoryGraphScene.ts:484-485`
- **Why it matters:** V8 throws `RangeError` past ~100–200k spread args; one bulk import blanks the whole graph. Unlikely today, hard crash if hit.
- **Replacement:** `const minCreated = (fs: FactRecord[]) => fs.reduce((m, f) => Math.min(m, f.created_at || 0), Infinity);`
- **Severity:** 🟡 (latent crash) · **Tier:** Tier 3 🔴 when hit (blank graph)

---

## 🟠 Real Cost at This Scale

### Help + Edge shared: infinite JS animation drivers with no visibility gating
- **H-F1** `HelpOrbVisualizer.tsx:96-115` — animating SVG `r` (non-compositable) on 2 phase-shifted infinite loops. → Replace with `scale` on `<motion.g style={{transformOrigin}}>` or pure CSS keyframes. 🟠 · Tier 1 🟢
- **H-F2** `HelpPttDiagram.tsx:40-52` — infinite SVG `d` path-morph (re-tessellates every frame for a 16px decoration). → Static path + opacity pulse. 🟠 · Tier 1 🟢
- **H-F3** `HelpHistoryDiagram.tsx:40-44`, `HelpMemoryDiagram.tsx:28-31`, `HelpPttDiagram.tsx:23-27`, `HelpOrbVisualizer.tsx:55-63` — every `repeat: Infinity` runs while scrolled out of view inside `HelpPanel.tsx:88`'s scroller; zero `useInView`/`whileInView`, zero `prefers-reduced-motion`. → Gate with `useReducedMotion` + `whileInView`; long-term convert to CSS. 🟠 · Tier 1 🟢
- **E-F4** same Orb loops (4–5 concurrent incl. `blur-2xl` glow) run the whole time Help is open on Home (default route mounts both `HelpOrbVisualizer` + `HelpPttDiagram`, `HomeHelpContent.tsx:17,20`). → Same CSS/gating fix. 🟠 · Tier 3 🔴 (freezing motion is visible by definition — sign off)
- **E-F5** `HelpPttDiagram` `d`-morph duplicate-flagged from Edge side. Same fix. 🟠 · Tier 3 🔴

### Help + Edge shared: stacked backdrop blurs (the scroll-jank recipe)
- **H-F4** nested `backdrop-blur-md` (outer card + inner pill) in `HelpHistoryDiagram.tsx:7+64`, `HelpMemoryDiagram.tsx:7+70`, `HelpOrbVisualizer.tsx:40+157`. → Drop inner, keep `bg-[rgba(var(--card),0.95)]`. 🟠 · Tier 1 🟢
- **H-F5** `backdrop-blur-md` on all 7–8 help cards inside one scrolling panel. → `bg-[rgba(var(--card),0.92)]`, no blur. 🟠 · Tier 2 🟡 (frosted→solid, honest trade-off)
- **E-F1** `EdgePanel.tsx:69-91` — `mask-image` on the same node as the framer `x` slide forces software raster per frame of the 220ms open/close. → Move mask to static inner wrapper, animate outer only. 🟠 · Tier 2 🟡
- **E-F2** `ResponsiveLayout.tsx:257-270` — `backdrop-blur-xl` feather over ~300×120px mounted whenever `/settings` renders, while the top haze 7 lines above proves the opaque-gradient alone works. → Drop the blur class, keep gradient. 🟠 · Tier 1 🟢
- **E-F3** blurred help cards inside the already-blurred `EdgePanel` (`backdrop-blur-md` × N+1). → Make cards opaque `bg-[rgb(var(--card))]`. 🟠 · Tier 1 🟢
- **H-F6** `HelpOrbVisualizer.tsx:55-63` — `blur-2xl` glow with per-frame `backgroundColor`+`scale`+`opacity` tween. → Static glow + `transition-colors duration-500` per mood. 🟠 · Tier 2 🟡

### Help bundle-on-open
- **H-F12** `HelpPanel.tsx:10-15` eager barrel (`help/index.ts` `export *`) + `helpCopy.ts` 22 icons: opening any route's guide loads all routes' diagrams + motion. → `lazy()` per route via direct file imports + `<Suspense>`. 🟠 · Tier 1 🟢
- **H-F13** Home mounts two heavy animated diagrams unconditionally. → Lazy-mount second diagram on near-viewport. 🟠 · Tier 1 🟢
- **H-F14** `SettingsHelpContent.tsx:4-6` eagerly bundles 3 diagrams, shows ≤1. → `lazy()` per tab. 🟠 · Tier 1 🟢
- **H-F10** framer-motion as decorative-loop engine in 4 files where CSS suffices. → CSS keyframes + `prefers-reduced-motion` kill-switch. 🟠 · Tier 1 🟢

### Edge interaction costs
- **E-F6** `SessionPanel.tsx:381-413,653-660,893-899` — spring FLIP + per-`dragover` `setState` + `whileDrag scale 1.025` with triple-layer shadow/`z-50`. → Guard setter (`prev === id ? prev : id`), `whileDrag={{scale:1.01}}`, `stiffness 300/damping 35`. 🟠 · Tier 2 🟡
- **E-F10** `NotificationPanel.tsx:352,561` — whole-panel `activeActionIds` subscription re-renders every item per action. → Subscribe inside `NotificationItem`. 🟠 · Tier 1 🟢
- **E-F11** `SessionPanel.tsx:88-103` — per-row `s.notifications.find(...)` = O(rows × notifs) per store write. → Store-side derived `Set` (memoized like existing L294-318 caches). 🟠 · Tier 1 🟢

### Memory GPU/frame costs
- **M-F4** `useMemoryGraphScene.ts:1010-1037` — teardown never calls `instancedMesh.dispose()` ×2 (each ~768KB instanceMatrix + color). `Memory.tsx:298` zero-width gate remounts the whole scene → VRAM sawtooth. → Add both `dispose()` calls. 🟠 · Tier 1 🟢
- **M-F9** `useMemoryGraphScene.ts:774-777` + `Memory.tsx:238` — DPR cap 2 + `antialias:true` + alpha over a second always-mounted `<AmbientBackground/>`: 4× pixels with MSAA at 200% scaling, double-canvas composite. Single largest frame-cost multiplier on the page. → DPR cap 1.5; skip/freeze `AmbientBackground` on this route. 🟠 · Tier 2 🟡
- **M-F5** `memoryGraphTypes.ts:68-86` — `DARK/LIGHT_COLLECTION_COLORS` Proxies re-derive full palette per property access (attractive nuisance; unexercised today). → Delete proxies or cached-palette accessor. 🟠 · Tier 1 🟢
- **M-F6** `MemorySessionRail.tsx:241-242`, `SearchBar.tsx:98`, `MemoryLegendOverlay.tsx:39`, `MemoryLegendPopover.tsx:50` — per-row/per-result palette regen (same root cause as M-F1, smaller N; 200 expanded facts = 200 style recalcs per rail render). → `useMemo(getActiveDynamicPalette)` once per render. 🟠 · Tier 2 🟡

---

## 🟡 Stylistic / Optional

- **H-F7** duplicate `animate-pulse` (`HelpInteractionDiagram.tsx:69` + Orb dot) — delete the Radio pulse. Tier 1 🟢
- **H-F8 / E-F14 / global** `transition-all` on buttons/tabs/nav/icons (`HelpControlCard:18`, `HelpInteractionDiagram:25/38/51`, `HelpMemoryKnobs:40`, `HelpOrb:177`, `SettingsHelp:42`, `EdgeNav:34/66`, `NotificationPanel:205/239/295`, ~40 `transition-all` sites repo-wide) — mechanical `transition-all` → `transition-colors` (keep `transition-transform` where scale is intended). Tier 1 🟢
- **H-F9** per-render allocations in tab hot paths (`KNOBS` array + `.find`, `MOODS` literal + inline `style={{}}`, inline `onClick` closures) — hoist to module scope, `Record` lookup. Tier 1 🟢
- **H-F11** `HelpInteractionDiagram.tsx:64-161` 3× near-identical flow markup — data-drive `FLOWS` map. Tier 1 🟢
- **H-F15** `helpCopy.ts` single all-routes module + `icon: any` — type as `LucideIcon`, split per route after lazy lands. Tier 1 🟢
- **H-F16** dead props (`HelpPanel deepLink`/`onClose`, `SettingsHelpContent initialCardId` never passed) — wire or delete. Tier 1 🟢
- **H-F17** index-as-key on static lists (History/Home/Memory/Settings contents) — key by heading/tip/id. Tier 1 🟢
- **H-F18** `<selected.icon>` member-expression render — hoist to `const SelectedIcon`. Tier 1 🟢
- **H-F19** `SettingsHelpContent` full-tree re-render per tab — deferred; split memoized children only if list grows. Tier 1 🟢
- **H-F20** `HelpPipelineDiagram.tsx:18` nested `overflow-x-auto` inside vertical scroller for content that fits — drop to `justify-around`. Tier 1 🟢
- **E-F7** `handleSessionDragLeave` dep on `dragOverProjectId` breaks `ProjectRowItem` memo mid-drag — functional setter, `[]` deps. Tier 1 🟢
- **E-F8** `NotificationPanel key={activeTab}` full remount + N enter animations per tab switch — drop `key`, animate dismiss-only. Tier 2 🟡
- **E-F9** `layoutId="notif-tab-underline"` global measurement for a 2px rule — CSS `after:` opacity/scaleX. Tier 1 🟢
- **E-F12** `Tooltip.tsx:28-91` unthrottled capture-phase scroll/resize with sync read+write — rAF-coalesce. Tier 1 🟢
- **E-F13** per-row `JSON.parse(action_payload)` + `new Date()` allocs in `NotificationPanel` — `useMemo` per row. Tier 1 🟢
- **E-F14 batch** — `TimeGroupHeader` not memo; `EdgePanel` always renders `border-r border-l` (one offscreen — pick by side); `NotificationPanel:424+573` ~128px dead scroll (`pb-24` + `h-8`); `ResponsiveLayout:44-65` raw resize listener re-subscribed per route — rAF-guard + stable deps. All Tier 1 🟢
- **M-F7** duplicate theme `MutationObserver`s (`Memory.tsx:72-80` + scene `:104-165` watching `style,class` too) — narrow to `attributeFilter: ["data-theme"]` or single hook. Tier 1 🟢
- **M-F8** `MemorySessionRail onClose` declared, destructured away, caller passes it (`Memory.tsx:293`) — wire or remove. Tier 1 🟢
- **M-F10** transparent near-opaque spheres + `frustumCulled=false` ×3 + eternal 30fps idle for a breathing core — make nodes opaque or suspend loop after 4s idle. Tier 1 🟢 (power/thermals)
- **M-F11** idle/move pacing reads private OrbitControls `state` — damping glide misclassified as idle → 30fps mid-fling judder. Drive from camera-delta or start/end events. Tier 2 🟡
- **M-F12** `sessionAnchors.find` per link (O(L×S)) — `Map` lookup. Tier 1 🟢
- **M-F13** unthrottled `ResizeObserver → setState → renderer.setSize` — rAF-coalesce in `Memory.tsx:91-102`. Tier 2 🟡 (resize only)
- **M-F14** inline arrow props defeat `memo` on dock + search (`Memory.tsx:243-268`) — `useCallback` handlers. Tier 1 🟢
- **M-F15** legend triplication (Overlay mounted; 190-line Popover + 140-line Card dead but exported; Card hardcodes hexes at `MemoryLegendCard.tsx:15-22` → accent-drift fork if remounted). Tier 1 🟢 latent, Tier 3 🔴 if Card remounted
- **M-F16** `MemoryGraphClusterBadges.tsx` 147 lines fully unmounted + impure `window` reads in render. Tier 1 🟢
- **M-F17** `dims.w > 0` gate tears down full WebGL context on transient zero-width → blank flash + re-init. Always mount, overlay placeholder. Tier 2 🟡
- **M-F18** barrel double `export *` + per-row `getCollectionIcon` switch — hygiene only, not worth touching alone. Tier 1 🟢
- **X-F2 (Sprint 4)** `VoiceCarousel.tsx:137-153` — `else { clearInterval(interval); }` clears an unassigned local (no-op; cleanup handles it) — harmless, delete the branch. Tier 1 🟢
- **X-F3 (Sprint 4)** `OrbitalLoader.tsx:124` fullscreen `backdrop-blur-3xl` overlay — heaviest blur class in repo, but transient (loading only). Leave unless it shows during scroll; never stack content above it. Tier 1 🟢
- **X-F4 (Sprint 4)** `Tooltip.tsx:114` `backdrop-blur-2xl` on every tooltip + `SessionContextMenu.tsx:119` `backdrop-blur-xl` — small surfaces, fine as-is; do not add more. Tier 1 🟢

---

## 🔴 Tier 3 Visible Trade-offs (need explicit sign-off regardless of severity)

1. **Freezing Orb/PTT motion** (E-F4, E-F5, H-F2) — killing the ripple/morph/wobble changes what Help looks like. Static-glow + pulse alternative reads the same at a glance, but it is a design call.
2. **Frosted→solid help cards** (H-F5) — removing per-card `backdrop-blur-md` flattens the frosted aesthetic. Recommend: keep one blur on the panel, none on cards.
3. **`LiquidChamber` restart fix** (X-F1) — restoring intended motion is visible only in that it un-freezes; no design risk, but call it out since the canvas will animate again.
4. **`M-F3` crash + `M-F15` Card color fork** — only visible if triggered/remounted; fixing is invisible until then.
5. Everything else in this report is Tier 1/2 — safe to apply without design review.

---

## What's Actually Fine (do not "fix")

- **Help:** `HelpControlCard` (memo'd, static — the template); `HelpPipelineDiagram` (zero motion imports — copy this pattern); `HelpPanel` route memo + exclusive render (waste is bundle, not render); `SettingsHelpContent` `useMemo(activeCard)`; `helpCopy` module-level constants; no layout thrash (zero `getBoundingClientRect` in surface); no SVG `<filter>` abuse (depth is CSS blur + cheap `radialGradient` fills).
- **Edge:** single mask + single `backdrop-blur-md` on the panel itself; top-right haze opaque-gradient pattern (copy it); no `ResizeObserver` loops in scope; no `onScroll` handlers to throttle (native `overflow-y-auto` + `overscroll-contain`); notification store atomic selectors + module ref-caches working; `NotificationItem`/`HelpPanel`/row memoization real; overlay dismissal centralized (`useOverlay` + `overlayStack`); `usePanelState` left/right split correct; `TitleBar` × EdgePanel no interaction; `notificationCopy` static `as const` SSOT.
- **Memory:** rAF **has** visibility + idle pacing (`:952-959` — quality issue F11, not absence); teardown disposes almost everything (only gap is F4); render tick is allocation-free (hoisted temps — allocations are in update paths F2); no `setState`-per-frame across React↔Three bridge; `SearchBar` **is** debounced (150ms + cleanup — remaining cost is filter + palette F6); tooltip renders once per selection, no pointermove stream; `MemorySessionRail` has zero drag/FLIP code (do not port Edge logic in); mounted legend Overlay has no blur; scene mounts once with correct filter/topology layering; conduit `NormalBlending` cleanup already done; `useMemoryTrace` negligible.
- **Sprint 4:** `LiquidChamber` frame-pacing (`elapsed < targetInterval` gate) is sound; `VoiceCarousel` 1s recording timer + cleanup correct; `OrbitCarousel` rAF loops are canvas-scoped.

---

## Bottom Line

**Not yet clean for its stated purpose:** the Memory page will stutter under its own growth curve (M-F1 per-node style recalcs, M-F2 buffer churn) while Help burns main-thread budget on decorative infinite loops the moment it opens. Fix M-F1 + M-F2 first (both Tier 1/2, zero design risk), then lazy-split Help diagrams and convert the four infinite loops to CSS — that sequence captures most of the report's value with almost no visible change.

**Fix order:** (1) M-F1 + M-F2 (scene buffer/palette) → (2) H-F12/H-F13/H-F14 lazy splits → (3) H-F10/H-F1/H-F2/H-F3 CSS keyframes + gating → (4) H-F4/H-F5/H-F6 + E-F1/E-F2/E-F3 un-stack blurs → (5) X-F1 chamber restart → (6) E-F6/E-F10/E-F11 interaction paths → (7) M-F9 DPR/ambient → (8) all 🟡 hygiene.
