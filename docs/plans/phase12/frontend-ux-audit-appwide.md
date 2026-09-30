# Frontend UX Audit — App-Wide

---
title: "Frontend UX Audit — App-Wide (Home, History, Settings, Monitoring, Wizard, Tray, Primitives, Tokens)"
audience: "Internal — Frontend Engineer / Design Reviewer"
last_updated: 2026-09-30
owners: "frontend-engineer role"
related_docs:
  - "docs/plans/phase12/frontend-ux-audit-memory.md — Memory page audit (run 1 of 2); read first"
  - "docs/features/performance-memory-optimizations.md — perf ledger this audit falsifies in 6 places"
  - "docs/specs/design-spec.md — glass/elevation/type-floor rules violated at scale"
  - "docs/frontend.md — frontend architecture doc"
  - ".agents/rules/frontend-style-guide.md — invariants checked against"
---

## How to read this doc

- **Audience:** Frontend engineer or design reviewer picking up the fix list.
- **Scope:** The whole frontend — 45,101 lines across `app/src`. Run 1 of 2 covered
  `pages/Memory.tsx` + `shared/components/memory/` + the router shell (4,550 lines);
  this run covers the remaining ~40,500.
- **Convention:** Every material claim cites `file:line`. Numbers come from the
  deterministic scans or from arithmetic shown inline. Unverifiable claims are marked
  `unverified` rather than asserted.
- **Non-goals:** No remediation code. No spec edits. This doc records findings and
  tiers them; it does not author the fix.
- **SSOT:** `.agents/rules/frontend-style-guide.md` for invariants,
  `docs/specs/design-spec.md` for the visual system,
  `docs/features/performance-memory-optimizations.md` for the perf ledger this audit
  corrects.

**Companion:** `frontend-ux-audit-memory.md` (run 1). Together they are the full audit.
Where this doc says "already reported in run 1," see that file.

---

## 1. Method & Calibration

**Method:** five isolated design-review agents (A1-A5) in parallel, then one isolated
deterministic-evidence agent (B). None saw another's output.

| Agent | Domain | Session |
|---|---|---|
| A1 | Home + `shared/components/home/` + `VoiceSessionContext` | `ses_f0cef14c3ffe5WAlA6zTw08A1K` |
| A2 | History + `shared/components/history/` + `useHistory` | `ses_f0cef1484ffeY5dJPXiyJYuc3P` |
| A3 | Settings + all settings cards + `settingsStore` | `ses_f0cef143bffeB9fRPlMEM2uMxB` |
| A4 | Monitoring + `shared/ui/` + profiler + `shared/components/common/` + `TitleBar` | `ses_f0ce6f66bffeV8StoBdkzKeKKO` |
| A5 | Wizard + Tray + `index.css` + all contexts/hooks | `ses_f0ce6f5f0ffebRGbJ5G5Z8oILd` |
| B | Deterministic + browser evidence, whole-tree | `ses_f0cdd346effeZGU54VpU51jAKL` |

**Calibration:** production-scale. Linux-first, CPU-constrained desktop app
(Tier 1A = 8GB, no GPU, llvmpipe software rasterisation per
`.agents/rules/frontend-style-guide.md:18`). Professional tool used all day.

**Mode:** Operate for the app; Persuade for the wizard (the only marketing surface).

### 1.1 The headline finding, stated before anything else

> **Both automated gates are green and the frontend is still shipping data loss,
> silent failures, and dead diagnostic instruments.**
>
> - `node .agents/skills/review-ui/scripts/check_invariants.mjs` -> **5/5 PASSED**
> - `impeccable detect --json app/src` -> **6 findings, all false positives** (see SS11)
> - Yet: a 500 ms debounce silently writes to the user's database on *browse*
>   (run 1), a failed `get_onboarding_status` **inescapably routes a configured
>   install into the setup wizard**, the profiler's leak detection **can never fire**,
>   and **9 declared npm dependencies have zero importers**.
>
> **Do not treat a green checkmark as evidence of correctness.** The gates cover
> visual style and lifecycle structure. The defects below are state-machine and
> data-integrity defects, which is exactly the class neither gate can see.

### 1.2 Coverage ledger

| Area | Lines | Reviewed | Depth |
|---|---:|:---:|---|
| `pages/` | 2,629 | 5/5 | full |
| `shared/components/` | ~19,000 | all subdirs | full + sampled on long files |
| `shared/ui/` | ~4,300 | all 22 files | full |
| `shared/hooks/` | ~3,100 | all 25 files | full |
| `shared/context/` | ~820 | 4/4 | full |
| `shared/lib/` | ~900 | 10/10 | full |
| `store/` | ~1,700 | 5/5 | full |
| `services/` | ~2,300 | read for state-flow | state read, not audited |
| `layout/` | ~770 | 3/3 | full |
| `wizard/` | ~2,600 | 12/12 | full |
| `tray/` | ~700 | 4/4 | full |
| `index.css` | 856 | token inventory | full |

---

## 2. Cross-Cutting Findings

These appear in **every** domain. Fix them once, fix the app.

### CC-1 — The design system is documented and unenforced. (P0)

`docs/specs/design-spec.md` SS2 defines a closed 4-level glass/elevation ladder. Reality:

| Spec level | Spec blur | Class exists? | Actual blur |
|---|---|---|---|
| Whisper | 8px | **does not exist** | — |
| Surface | 16px | **does not exist** | — |
| Card | 24px | yes (`index.css:194`) | **20px** (`:199`) |

`.glass` is an undocumented 5th class (`:178`, 8px). `Card.tsx:7` declares a
4-value `elevation` enum — **2 of 4 values are ever passed**, in 5 call sites, all
inside `shared/components/settings/`. **`whisper` and `elevated` are dead.**
`Card.tsx:35-40` adds a second, independent `blur?` axis with **0 call sites** — all
7 branches are dead code that widens the declared system to 10 phantom levels.

**38 `glass-card` literals across 23 files bypass `Card` entirely**, and the shell is
where the bypass is densest: `EdgeNav.tsx:39`, `Monitoring.tsx:361`,
`ErrorBoundary.tsx:53,103`, `Drawer.tsx:237`, `Home.tsx:202`, `History.tsx:214,236,250`,
`Settings.tsx:355`, `ResponsiveLayout.tsx:425`, `TrayApp.tsx:288`, plus the whole
`wizard/` tree.

`frontend-style-guide.md:113` says glass elevation is a **closed** system. A closed
system with 12% adoption and 38 exits is not closed — it is a suggestion.

### CC-2 — 43 backdrop blurs, 8 distinct values, 1 mitigation, used once. (P0)

| Value | Sites |
|---|---|
| 4px (`-sm`) | 7 |
| 12px (`-md`) | 15 |
| 16px (`-lg`) | 1 — **dead**, sole site is `Card.tsx:37` |
| 20px (`-[20px]`) | 1 (`MemoryGraph.tsx:59`) |
| 24px (`-xl`) | 9 |
| 40px (`-2xl`) | 7 |
| 64px (`-3xl`) | 2 (`OrbitalLoader.tsx:124`) |

Plus 2 hardcoded in CSS and 1 inline style (`TrayApp.tsx:292` = `blur(20px) saturate(180%)`,
the only `saturate()` in the codebase). **8 distinct values. 46 sites. Spec says 3.**

`index.css:736-740` defines `.no-blur` as the *"WebKitGTK transition workaround:
disable backdrop-filter during animation to prevent flashing"*. It is **correctly
implemented** (both prefixed and unprefixed, both `!important`) and has **1 real
usage** (`OrbitCarousel.tsx:80` add / `:82` remove). **43 of 43 `backdrop-blur` sites
animate something; 1 opts out.** On llvmpipe a `backdrop-filter` on a moving element
is the single most expensive paint operation available.

Additionally: **43 sites where a `backdrop-blur` element also carries `transition-*`** —
that is the exact WebKitGTK flash case the workaround was written for.

### CC-3 — Zero motion tokens. 219 hardcoded durations, 11 hand-written easings. (P1)

`index.css` declares **zero** `--transition-*` / `--duration-*` / `--ease-*` /
`--motion-*` / `--anim-*` tokens. `rg` across `app/src` and `docs`: **0 matches.**

`duration-NNN` in TSX: **219 occurrences, 9 distinct values** — `300`x104, `200`x43,
`500`x32, `150`x16, `400`x12, `700`x6, `1000`x4, `100`x1, `75`x1.

`index.css` adds 23 more hand-written `transition:`/`animation:` declarations with
**11 distinct `cubic-bezier()` curves** (`:236`, `:399`, `:539`, `:666`, `:683`,
`:700`, `:704`, plus named easings). **The signature curve `(0.16, 1, 0.3, 1)` — used
for the tray fade and the range-slider thumb, i.e. the two things a user feels most —
is written out literally twice and is not a token.** One animation, three magic numbers
describing it: `useVisibility.ts:64` (`// Hardcoded 500ms`), `TrayApp.tsx:270`
(`duration: 0.5`), `TrayApp.tsx:287` (`duration: 0.15`).

**Spec contradiction:** `design-spec.md` SS7 sets micro-interactions at 150-300ms.
**54 sites are >=400ms.**

### CC-4 — Zero z-index tokens, and a live stacking defect. (P1)

`index.css` declares **zero** `--z-*` tokens and contains exactly one `z-index`
declaration (`z-index: 0` at `:553`). The only z token anywhere is `--drawer-z`,
injected inline at `Drawer.tsx:179` — a component-local escape hatch, not a scale.

**11 distinct arbitrary values** across 25 sites: `z-[35]`x3, `z-[38]`x1, `z-[60]`x6,
`z-[100]`x6, `z-[110]`x4, `z-[200]`x1, `z-[9999]`x3 — plus 6 Tailwind named steps.
`z-[9999]` (context menus, `SessionContextMenu.tsx:145`, `ProjectContextMenu.tsx:142`)
sits **below** `index.html:240`'s `error-root` at `z-index: 99999`, and below
`index.html:103`'s boot loader at `10000`. Three different "escape to the top" values,
none coordinated.

`design-spec.md` SS13 says surfaces must not install their own Escape/outside-click
listeners; there is one authority (`shared/lib/overlayStack.ts:113`). **27 sites
install their own** — including `pages/Home.tsx:142`, `pages/History.tsx:144`,
`pages/Memory.tsx:737`, `pages/Settings.tsx:122`, and both context menus.

Consequence found by A4: `overlayStack.ts:113` registers on `window` in the **capture**
phase and calls `stopPropagation()`, so `HelpPanel.tsx:97`'s documented two-stage
Escape **can never run** — its own comment describes behaviour the code cannot produce.

### CC-5 — `prefers-reduced-motion` is CSS-only, and framer-motion is ~90 sites. (P0)

`MotionConfig` = **0 occurrences app-wide.** `useReducedMotion` = **0 occurrences
app-wide.** `index.css:149-157` is a structurally sound universal `!important` clamp on
`animation-duration`/`transition-duration` — and it neutralises every CSS animation.

| Escape vector | Count |
|---|---|
| framer-motion `motion.*` elements | ~90 app-wide; **12 in the wizard alone** |
| `repeat: Infinity` loops | 4+ (`WelcomeStep.tsx:201`, `LiveTestStep.tsx:174`, `Header.tsx:30`, `TranscriptRenderer.tsx:82`) |
| JS rAF loops | `useStreamingRenderer.ts:52`, `LiveWaveform`, `AmbientBackground.tsx:97,148`, `useMemoryGraphScene` |
| `scrollIntoView({behavior:'smooth'})` | `ModelCategory.tsx:54` |
| `index.html` boot loader | 4 infinite animations, own `<style>` |
| 24 CSS keyframes not in the opt-out list | `blob-rotate-a/b/c` are 3 permanently-rotating 120s gradients |

**`OrbitCarousel.tsx:288-290` is the only correct implementation in the app** (450ms ->
50ms on reduced motion). The pattern is known and simply not applied.

### CC-6 — Accessibility infrastructure is absent, not broken. (P0)

| Measure | Count | Verdict |
|---|---|---|
| `aria-live` / `role="status"` / `role="alert"` | **2 files app-wide** (`Home.tsx:226`, `StatusCapsule.tsx:12`) | The one thing `aria-live` exists for — a stream of new text — is the one thing this app lacks |
| `focus-visible` rings | **35** vs **258 `<button>`** + **25 `<input>`** | 53 `outline-none` sites strip focus from inputs (`SessionPanel.tsx:221,504,967`, `ApiKeyField.tsx:72`, `MemorySessionRail.tsx:297`, `LlmCatalogView.tsx:165,210`, `PersonalMemoryConfigDesk.tsx:367,419`, `UnderlineInput.tsx:58`) |
| `aria-modal` | 1 (`Drawer.tsx:195`) — **0 focus traps** | Focus lands on `<body>` on every drawer close |
| Text inputs with a `<label>` | Labels are `<span>` in `ApiKeyField.tsx:38`, `UnderlineInput.tsx:35`, `RealtimeVisualElements.tsx:270` | **Every text input on the settings surface has no accessible name** |
| `role="slider"` | 0 — `RotaryKnob.tsx:127` is a bare `<div>` | TTS speed + guidance are mouse-only |
| `role="grid"` | 0 — `CalendarPicker.tsx:178` is 42 unstyled `<button>` tab stops | Arrow keys do nothing |
| `role="log"` | 0 | The transcript is silent to a screen reader |
| `AbortController` | **0 in all of `app/src`** | — |

### CC-7 — Token bypass at scale. (P1)

| Measure | Count |
|---|---|
| Six-digit hex literals in `.ts`/`.tsx` | **48** — worst file `useMemoryGraphScene.ts` (18) |
| Raw Tailwind palette classes (`red-500`, `emerald-400`, ...) | **302** |
| Literal colours hardcoded **inside `index.css`** | **22** — 16 `rgba()`, 6 `hsl()` |
| CSS custom properties declared | **40** |
| Custom properties **never `var()`-referenced** | 17, including all 5 `--glass-*` colour tokens |

`index.css` **is not tokenised against itself**: `.glass` hardcodes
`rgba(10, 12, 14, 0.18)` at `:182` and `.glass-card` hardcodes `rgba(20, 24, 30, 0.60)`
at `:198` — **the tokens that exist to be used by those classes are not used by those
classes.** The light theme redefines 34 of 40 tokens but **none of the 5 glass
tokens**, so the entire glass colour system is dark-only by omission. And `:561-563`
hardcodes `hsl(230 30% 6%)` for the page background, fighting `:58`'s
`rgb(var(--background))`.

`AGENTS.md` SS5 records that hardcoded amber/yellow/white-on-light bugs were purged from
`Settings.tsx` in favour of `var(--accent)`. **The wizard, tray, and shared primitives
were never swept** — `StatusCard.tsx:35,43`, `ModelSetupStep.tsx:248,249,253`,
`LiveTestStep.tsx:100,147,157,209`, `WizardRoot.tsx:177`, `VoiceCarousel.tsx` (10 sites).

`RotaryKnob.tsx:169` uses `var(--surface-bg)` — **that token is not defined anywhere in
the codebase.** The declaration is invalid, the background is dropped, and the knob's
value text renders on an unstyled transparent hub.

### CC-8 — Copy centralisation is bypassed, and 400+ copy keys are dead. (P1)

`frontend-style-guide.md:29` bans inline user-visible strings. A4 counted **~90 literals
across 12 of 34 files** in the shared primitive layer alone, including
`HelpPanel.tsx` with **11 hardcoded strings and zero `src/data/` imports** while
`helpCopy.ts` (17.1K) sits beside it. A3 counted 17 attribute literals + ~25 JSX
text literals in settings. A2 counted 18 in history. A5 counted 6 in wizard+tray.

Meanwhile the copy modules are full of dead keys — the inverse failure:

| Module | Keys | Referenced | **Dead** |
|---|---:|---:|---:|
| `MEMORY_COPY` | 163 | 92 | **71** |
| `PROFILER_COPY` | 79 | 10 | **69** |
| `INTERACTION_CONFIG_DESK_COPY` | 54 | 1 | **53** |
| `PERSONAL_MEMORY_CONFIG_DESK_COPY` | 49 | 3 | **46** |
| `MONITORING_COPY` | 57 | 16 | **41** |
| `SESSION_COPY` | 58 | 24 | **34** |
| `LAYOUT_COPY` | 39 | 7 | **32** |
| `HOME_CONTROLS_COPY` | 37 | 6 | **31** |
| `NOTIFICATION_COPY` | 47 | 22 | **25** |
| `WORKING_MEMORY_SETTINGS_COPY` | 23 | 0 | **23** |

`HISTORY_SETTINGS_COPY` and `MEMORY_CONFIG_DESK_COPY` are **fully dead** — 1 textual
occurrence each (the definition). `MetricCarousel.tsx:34-84` **invents 16 replacement
strings** for `MONITORING_COPY` keys that already existed.

**Caveat on the nested-object modules:** `INTERACTION_CONFIG_DESK_COPY`,
`PERSONAL_MEMORY_CONFIG_DESK_COPY`, `WORKING_MEMORY_SETTINGS_COPY`, `WELCOME_TOOLTIPS`,
and `*_SETTINGS_COPY` over-count because nested keys are reached via `OBJ.sub.key`.
`MEMORY_COPY`, `PROFILER_COPY`, `MONITORING_COPY`, `SESSION_COPY`, `LAYOUT_COPY`,
`NOTIFICATION_COPY`, `HISTORY_COPY` are flat and reliable.

### CC-9 — Route changes have no side effects at all. (P1)

| Effect | Count app-wide |
|---|---|
| `document.title` writes | **0** |
| Route-change title update | **0** |
| `scrollRestoration` / `window.scrollTo` | **0** |
| Focus move to `<main>` on route change | **0** — `querySelector('main')` = 0 |

Every window shows "Vox | Ambient Intelligence". Browser-confirmed identical across all
5 routes. Combined with run 1's finding of **zero route exit animation**, navigating the
app is a hard cut with no scroll reset, no focus move, and no title change.

### CC-10 — 9 declared dependencies have zero importers. (P1)

`package.json` declares, `app/src` imports **0 files**:

`d3-force`, `graphology`, `graphology-layout-forceatlas2`, `react-colorful`,
`@elevenlabs/react`, `@sdkrouter/client`, `react-dropzone`,
`@fontsource-variable/geist`, `class-variance-authority`

`knip` is already wired as `pnpm lint:dead`. It was never run.

### CC-11 — Fetch-hook guards are 4/11 absent, and no request can be aborted. (P0)

| Hook | `isMounted` | `AbortController` | Sequence guard |
|---|---|---|---|
| **`useObservationsList`** | no | no | no (`isFetchingRef` mutex only, `:34,:36,:73`) |
| **`useRuntimeSnapshot`** | no | no | no |
| **`useRemoteLlmProbing`** | no | no | cache-key only |
| **`useHomePage`** | no | no | no |
| `useHistory` | yes | no | yes (5/5) |
| `useSessionPanel` | yes | no | partial |
| `useMemoryProfiler` | yes | no | yes |
| `useMonitoringMetrics` | yes | no | no |
| `useSessionHydration` | yes | no | yes |
| `useModelDownloads` | yes | no | partial |

**`AbortController` appears 0 times in `app/src`.** Violates
`frontend-style-guide.md:59` in 4 hooks.

### CC-12 — Bundle structure is correct; one comment is a lie. (pass)

**Good news, stated plainly:** `pages/` has a **transitive fan-in of 2** — every route
page is reachable only via `React.lazy(() => import(...))` in `App.tsx`. Entry closure
from `main.tsx` is **190 of 234 files**; the 44 unreachable are exactly the Settings
sub-tree (31 files) + 3 help diagrams + `useConversationList`. `rollup-plugin-visualizer`
is configured (`vite.config.ts:17-22` -> `dist/stats.html`) with 4 `manualChunks`
(`vendor`, `three`, `tauri-api`, `ui-core`). **The chunking architecture is sound.**

**But:** `Settings.tsx:11` says `// Loader functions for eager prewarming` and defines
7 named loaders — **nothing calls them.** `rg` finds only the definitions and the 7
`lazy()` calls. The comment is false and the first click on any radial node is a cold
chunk fetch.

Top-15 files by transitive fan-in (what the initial bundle pays for):
`shared/lib/utils.ts` **157**, `services/memoryService.ts` 112,
`services/eventsService.ts` 91, `data/settingsCopy.ts` 85, `store/settingsStore.ts` 81,
`data/shortcuts.ts` 66, `shared/lib/spatialNavigation.ts` 65, `shared/ui/Tooltip.tsx` 64,
`services/monitoringService.ts` 64, `data/sessionCopy.ts` 62.

---

## 3. Domain A1 — Home

**Score: 22/40.** Highest design quality in the app. Lowest error recovery.

### Strengths — do not "fix" these

- `AdvancedOrb.tsx:853-872` — textbook WebGL teardown: `canvas.width=1;height=1`
  (`:867-868`) -> `forceContextLoss()` (`:870`) -> `dispose()` (`:871`), with
  `sharedDiscGeo` (`:859`) shared across all 7 meshes and disposed exactly once.
  Every clause of `frontend-style-guide.md:84-88`, in the right order.
- `useSessionPanel.ts:87-97`, `useTelemetry.ts:14-36`, `eventsService.ts:164-196` —
  the async/listener contract done right, including a `cancelled` latch that calls
  `u()` immediately if a Tauri listener resolves after teardown (`:184-186`) plus a
  `beforeunload`+`pagehide` registry sweep (`:144-158`).
- `useStreamingRenderer.ts:12-13,34` — correct mutable-target rAF capture
  (`targetTextRef.current` read inside `tick`, never from a render closure).
  Exactly `frontend-style-guide.md:78`.
- `PipelineField.tsx:59-79` — frameless, border-dashed membrane with a `blur-[120px]`
  heat blob. No pill, no box, no badge. What `frontend-style-guide.md:117` asks for.
- `AdvancedOrb.tsx:34-57` — all nine backend states encoded into per-state scale targets
  and amplitude floors; `Tick`'s 2nd-order critically-damped spring (`:576-594`) makes
  `Listening -> Speaking` a physical settle rather than a CSS snap.

### Priority issues

**P0-1 — `sessionError` is written by 0 readers.** `VoiceSessionContext.tsx:224,240`
sets it. `rg sessionError` returns 4 hits, all in `store/sessionStore.ts` (field,
initial, setter, reset). **No component reads it.** `Home.tsx:329` branches on
`interactionState === "Error"`, which arrives via `state_changed` — but
`pipelineService.ts:143-152` swallows the engage timeout, calls `resyncFromSnapshot()`,
and returns `{success}`. If the resync succeeds the pipeline reports `Ready` and no
error reaches the UI; if it fails, `interactionState` is unchanged and the user is back
at `Idle` with nothing.
*Impact:* Alex clicks Engage, the power icon spins up to 8s (`:432,437-440`), then stops
and **nothing happens**. No toast, no banner, no `role="alert"`. Direct violation of
`frontend-engineer.md:26`. **Tier 1** — a banner that only appears in a currently-invisible
state is zero visual change for every healthy user.

**P0-2 — Two uncapped rAF loops driving React reconciliation, no visibility gate.**
`ActiveTranscript.tsx:11-12` instantiates `useStreamingRenderer` **twice**. Each runs a
60fps rAF calling `setDisplayText(...)` every frame, re-rendering a `memo`'d
`DialogueBubble` -> `Markdown` -> ReactMarkdown parse. Neither consults an
`IntersectionObserver`, and the containing rail is `hidden md:flex` (`Home.tsx:246`) —
so below 768px both loops run at full rate rendering into a `display:none` subtree.
On `/` during streaming there are **three** persistent rAF loops: one WebGL submit plus
these two, both driving React rather than pixels. **Tier 2** (30Hz throttle), **Tier 3**
if implemented as integer-step (the caret visibly steps).

**P0-3 — The orb is configured for a GPU and running on llvmpipe.**
`AdvancedOrb.tsx:687` `antialias: true` with no `powerPreference`; `:689`
`setPixelRatio(Math.min(dpr, 2))`; `:28` `NUM_SHEETS = 7` `DoubleSide` alpha-blended
discs at `:749`. On Tier 1A that is a software MSAA resolve of a 4x-multisampled buffer,
every dimension doubled, 8 layers of alpha overdraw with no depth rejection — worst
exactly when it matters most (60fps precisely because `isActive` is true, `:459`).
`powerPreference` is set on **neither** renderer in the whole app (0 occurrences).
**Tier 1** for `antialias:false`/`dpr:1` (invisible on a soft blurred alpha form),
**Tier 2** for `NUM_SHEETS` 7->4.

**P1-4 — Two hardcoded constants silently disable two authored affordances.**
`VoiceSessionContext.tsx:452` `restoringSessionId: null` and `:456`
`hasCachedSession: false` are literals in the `useMemo`, not store reads. So
`restoring={...}` at `SessionPanel.tsx:588,861,894` is **always false** — the
`Loader2 animate-spin` at `:186-191` and the `animate-pulse opacity-60 cursor-wait` row
state at `:181` are unreachable. And `Home.tsx:422-425`'s "Resume Session" badge plus
the `resumeAriaLabel` branch at `:435` never render. `sessionStore.ts` has no
`hasCachedSession` field at all — the state was never modelled.
*Impact:* This is "basic UX details are missing" with a mechanical cause: the details
were designed and written, then disabled by a hardcoded `false`.
**Tier 1** for the spinner, **Tier 3** for the badge (a new visible element in the Idle
composition).

**P1-5 — 4 IPC list queries + a 60ms sleep per `sessions_changed`.**
Two components subscribe independently. `ActiveSessionHeader.tsx:83` does
`getRuntimeSnapshot()` -> `await setTimeout(60ms)` (`:102`) -> `getSessions()` +
`getProjects()`. `useSessionPanel.ts:91` concurrently calls `refresh()` -> the same two.
Four full-list queries and a 60ms artificial delay per event, no debounce, no coalescing.
`eventsService.ts:134` documents that `session_title_updated` was removed in v2 and
"title changes surface via `sessions_changed`" — so this fires potentially **once per
turn**. **Tier 1** — pure IPC coalescing, and it also makes titles update ~60ms sooner.

**P1-6 — Space is hijacked on every focused button.** `Home.tsx:105-109` `isEditable`
matches only `input | textarea | select | contenteditable`. A `<button>` returns `false`,
so the global keydown reaches `:147` and calls `handlePttStart()`; the matching `keyup`
at `:153-160` then calls `handlePttStop()`. **Every focusable control on the surface (7
buttons in `Home.tsx` + 25 in `home/*.tsx` + `EdgePanel`) has this trap.** Secondary:
Escape in the text bar fires both `TextInputBar.tsx:77` and `Home.tsx:142` because the
event bubbles to `window`. **Tier 1** — changes nothing until someone presses Space on a
focused button, at which point it fixes a broken interaction.

**P2-7 — Up to 12 simultaneous `backdrop-filter: blur(24px)` regions.**
Every `DialogueBubble` is hand-rolled `bg-[rgb(var(--card))]/80 backdrop-blur-xl`.
`Home.tsx:183-185` allows 10 history turns + up to 2 live bubbles = 12, each
re-compositing every streaming frame (P0-2), on a software rasteriser. `.no-blur` has
**0 usages on Home**. **Tier 1** for the `.no-blur`-while-streaming toggle;
**Tier 3** for removing the blur from the user bubble entirely.

**P2-8 — 25 buttons with no `focus-visible` ring.** `focus-visible` appears 6x in
`Home.tsx` (covering 7 buttons) and **0x** across `TextInputBar.tsx` (4),
`SessionPanel.tsx` (12), `DialogueBubble.tsx` (1), `EdgePanel.tsx` (2). Session-panel
rename inputs (`:218`, `:500`) and the project-name input (`:965`) use bare
`focus:outline-none` with only a `border-b` colour change. **Tier 2** — invisible to
mouse users, critical for keyboard users.

**P3-9 — The transcript has no screen-reader path.** No `role="log"`, no `aria-live`,
no `aria-relevant` in `ActiveTranscript.tsx` or `DialogueBubble.tsx`. When Vox finishes a
reply, nothing is announced. **Tier 1** (zero visual change) — **but fix P0-2 first or
this makes things worse for Sam**, since an `aria-live` region re-rendering 60x/sec
floods the screen reader.

**P3-10 — 5 hardcoded strings, copy already exists.** `Home.tsx:228`
`aria-label="Vox Status: Temporary"` duplicates `HOME_CONTROLS_COPY.temporary.activeBadge`
rendered 5 lines below. `StatusCapsule.tsx:14` templates a string inline.
`EdgePanel.tsx:107` `aria-label={title || "Panel"}` and `title` is never passed from
`Home.tsx:451`, so **the session-list dialog's accessible name is literally "Panel"**.
`:126,130,148,152` hardcode `"Close"`. `SessionContextMenu.tsx:174` hardcodes `"Delete"`
while `data/sessionCopy.ts:57` already defines it. **Tier 1** (except the "Panel" fix,
which is screen-reader-only).

### Cognitive load: 4 of 8 FAIL -> HIGH

Fails: mixed units for the same value class (`formatMs` mixes ms/s,
`formatTokens` mixes `0`/`1.2k` — `TurnMetricsBadge.tsx:9-21`); "learn the app"
requirement (6 undiscoverable shortcuts, no cheat sheet — `Home.tsx:104-151`; `T` only
works `if (isEngaged)` at `:139` with no disabled reason); progressive disclosure
(`TurnMetricsBadge` is `hidden lg:flex` and only mounted when `interactionState !== "Idle"`
— **below 1024px there are no turn metrics on Home at all**, and mic/speaker mute state
renders only inside the text bar, which isn't open while dictating).

### Persona highlights

- **Alex:** no keyboard route to the feature; 2 clicks to read one memory (node picking
  off by default — a mode toggle guarding a default is backwards); every state change
  re-fetches 7-8x (`handleApplySuggestions` `:450-456`, `handleConsolidateNow` `:397-403`,
  `handleRegenerateWithComments` `:619-623`, `handleRestoreActive` `:328-330` each fire
  versions+revisions then `refresh(true)` which re-fires both plus memory+observations,
  and `refresh()` is a mount effect at `:212-214`); Lenis `duration: 0.8` (`:249-256`)
  on the pane he reads daily, while the observations list beside it uses native scroll.
- **Jordan:** the first frame has no loader, no fade, no intro — the orb snaps to
  existence (your "abrupt transition" complaint, literally). The Stop button (`Home.tsx:398-414`)
  has no confirm and no undo and wipes history before the IPC resolves
  (`VoiceSessionContext.tsx:252-255`). Engage fails silently. `SessionPanel` shows the
  italic "No pinned sessions" empty state *during* the initial fetch
  (`useSessionPanel.ts:48,74-84` + `SessionPanel.tsx:855-876`) — the list lies about
  itself for ~200ms. Empty states are 11px italic at 35-40% muted opacity (`:873`, `:1029`).
- **Sam:** the transcript is entirely silent. `PTT` state is colour + `animate-pulse-slow`
  only, no `aria-pressed`, no text (`:365-382`). Space is hijacked. `foreground-muted` at
  10-12px with `/35`-`/60` multipliers (`:600,873,906,1029`) is well under 3:1. The orb
  is unpausable by `prefers-reduced-motion` — the CSS block correctly kills animations but
  the 60fps WebGL loop is entirely outside CSS.

### Notable minor observations

- `toMood` (`voiceDisplay.ts:17-41`) is documented as the orb/ambient-lighting mapping and
  **called from nowhere** (`rg toMood` -> 3 hits, all definition/re-export). Dead.
- `Home.tsx:299` hardcodes `isTesting={false}` while `AdvancedOrb.tsx:365` accepts the
  prop and `StatusCapsule.tsx:14,29` branches on it with unreachable copy. Three pieces of
  dead test-mode scaffolding across 4 files.
- `VoiceSessionContext.tsx:147-149` — `handlePartial`/`handleFinal`/`handleToken` are all
  `useCallback(() => {}, [])`, yet `useSessionEvents` registers three real Tauri listeners
  for them (`:172-174`) while `useTranscriptStream` (`:110`) subscribes to the **same
  three** with real handlers. **Home opens 6 listeners where 3 suffice.**
- `useTranscriptStream.ts:14,52,69,91,122,140` calls `setActiveTurnId` on every new turn,
  re-rendering the whole `VoiceSessionProvider` subtree — and `activeTurnId` is **never
  destructured** at `:110`. Pure dead re-render.
- `pipelineService.ts:188-207` `waitForState` polls `getRuntimeSnapshot()` every 100ms for
  up to 8s = **up to 80 IPC round-trips per engage**, on a machine booting an audio engine.
- `AdvancedOrb.tsx:743-745,758-760,763-767` uses `Math.random()` for `u_phase`,
  `u_waveScale`, `u_baseOpacity`, and initial rotations — **the orb looks different on every
  launch**, so it cannot be A/B'd or bug-reported.
- `TextInputBar.tsx:41-49` returns `merged` unconditionally from an effect keyed on
  `initialHistory`, which `Home.tsx:324` rebuilds via `.map()` on **every Home render** —
  i.e. every streaming token tick.
- `Home.tsx:220-222` — the `Mode: {governor}` chip is a static label in a `rounded-full`
  + tinted background + border, which `frontend-style-guide.md:115` reserves for
  interactive controls. The only faux-pill on the surface.

### Questions

1. **Is the transcript rail earning its place on Home at all?** It renders the same turns
   as `/history`'s `DetailPanel`, costs 12 backdrop-filter regions, duplicates 60fps
   reconciliation, and disappears entirely below 768px. If the orb is the interface and
   the transcript is a *destination*, deleting `Home.tsx:245-269` eliminates P0-2 and most
   of P2-7 in one stroke. **Tier 3** — but which is the product? Right now it's neither.
2. **Should Engage show a disabled-with-reason state instead of a spin?** The engage path
   can take 8s and fail silently. Does the backend expose phase progress
   (`model_progress` at `eventsService.ts:124`) that Home could bind to? That would be a
   better fix than a banner.
3. **`Home.tsx:290`'s pulse ring stays lit while the app is asleep.** When Vox sleeps the
   label says `Sleeping`, the field says `Idle` (`PipelineField.tsx:35-38` sends both
   `Working` and `Sleeping` to `default` -> `targetEnergy = 0.12`, byte-identical to Idle),
   and the ring pulses. For a device that sits open on a desk all day listening, making
   Sleep the one visually-quiet state may be the highest-value visual change available.
   **Tier 3.**

---

## 4. Domain A2 — History

**Score: 24/48 (renormalised over 12 sub-criteria).** Best idle behaviour in the app.

**The central verdict: authored skin, unauthored instrument.** Every session card leads
with the clock time the user spoke (`VoiceRippleNode.tsx:88-90`); the card body is a
seven-word truncation of what they actually *said* (`:33-38`); the hub is a 48-tick dial
with `SPAN`/`SESSIONS`/`MEMORIES` readouts (`CentralClockNode.tsx:140-168,298-327`).
Nobody builds a `07:12 - 11:48` label for a generic carousel.

But `orbitMath.ts:166-174` distributes cards by **count** — equal `2π/n` angular steps,
**no temporal component**. Time is encoded only in card *text*, never in *position*. And
the four helpers that would make it a real instrument — `timeToDialAngle` (`:290`),
`dayToDialAngle` (`:296`), `dialDotRadius` (`:306`), `dialDegrees` (`:301`) — together
with the design comment at `:285-287` (*"one dot per session **at its clock position**"*)
have **exactly one occurrence each in the entire codebase: their own declaration.** The
24h dial ring was designed in math, described in a comment, and never rendered.

**And where time does reach the layout, it lies.** `chunkSessionsIntoWindows`
(`orbitMath.ts:220-240`) slices sessions into ordinal buckets then labels each
`formatTimeRange(newest, oldest)` (`:236`). So "07:12 - 11:48" means "the 24 most recent
sessions, which happen to span those hours" — not "everything you said in that range".
The hub prints that caption under the word `SPAN`.

### Strengths

- `OrbitCarousel.tsx:169-204` — the rAF loop genuinely self-stops. Velocity damping (`:189`),
  a hard `MOMENTUM_MAX_MS = 1000` decay cap (`:32,192`), a 5-second stuck-pointer
  watchdog (`:180-186`), and a `rafRef` null-guard. **Idle History costs 0 rAF** — strictly
  better than the Memory graph's 30 FPS rest rate. Plus three independent anti-perf
  systems, each documented in a comment at the point of use: dirty-checked style writes
  (`:138-141`), depth-quantized blur (`:131`), z-banding (`orbitMath.ts:67-82`).
- `useHistory.ts:262-264` + `Drawer.tsx:51-52,87,95-102` — correct async lifecycle, correctly
  layered. **Zero bespoke overlay logic in the History surface.**
- `History.tsx:234-265` — four genuinely distinct async states (error with real message and
  working retry, empty, content, loading). **This is the opposite of run 1's error-to-empty
  conflation. History gets it right.**

### Priority issues

**P0-1 — `.no-blur` is a no-op; the expensive blurs are outside the toggled subtree.**
`OrbitCarousel.tsx:76-84` toggles `.no-blur` on `cardsContainerRef` (`:433`). **Nothing
inside that container has a `backdrop-filter`** — `.orbit-card-surface`
(`index.css:576-580`) is `radial-gradient` + `border` + `box-shadow`, and the card buttons
use flat `bg-white dark:bg-black`. The class disables exactly zero filters. The two real
per-frame offenders are outside: the hub's `backdrop-blur-md` on a `clamp(250px,26vw,320px)`
sphere (`CentralClockNode.tsx:120`), a **sibling** of the card layer, and the loader's
full-viewport `backdrop-blur-2xl` (`History.tsx:374`).
*Impact:* on llvmpipe every orbit frame re-rasterizes a 330px-radius backdrop blur behind
24 moving cards. **The codebase already contains the correctly-shaped helper pointed at the
wrong element.** **Tier 3** — dragging is measurably smoother, users feel it before they
see it.

**P0-2 — Arrow keys silently replace the transcript you are reading.**
`History.tsx:110-146`. The editable guard (`:114-118`) excludes only
`input/textarea/select/contenteditable`. `Drawer` focuses a `tabIndex={-1}` sheet
(`Drawer.tsx:100,197`), so keydown from the open transcript bubbles to `window` and
**passes the guard**. `ArrowRight` then runs `setSelectedSession(nextIndex)` (`:140`),
re-fetching turns and swapping content — while `OrbitCarousel:250-265` simultaneously spins
the ring, because `orbitToAngle` has no `paused` check (contrast `loop:172`). The chevrons
do the same destructively (`useHistory.ts:336,343`).
*Impact:* one stray `->` destroys reading position with no warning and no way back. And
the ring is **already paused whenever a session is open** (`History.tsx:326`), so the
arrows are meaningless in that state. **Tier 3.**

**P1-3 — History orders sessions differently from every other session list.**
`useHistory.ts:102`: `setSessions(data.sort((a, b) => b.created_at - a.created_at))` —
mutates the IPC array in place, ignores `is_pinned`, ignores `updated_at`. Compare
`useSessionPanel.ts:74`, `useConversationList.ts:35`, `MemorySessionRail.tsx:60`,
`SessionPanel.tsx:669` — **all four** call `sortSessionsNewestFirst`, whose own doc comment
(`historyService.ts:123`) says *"`updated_at` reflects the last persisted turn only."*
*Impact:* a pinned session sinks below 24 unpinned ones in History but floats to the top in
Home and Memory. **Tier 3.**

**P1-4 — The clock reports numbers it did not measure.** Five sites:
`CentralClockNode.tsx:343` renders a literal `"00:00 - 23:59"` under a `SPAN` label —
**fabricated telemetry displayed as measurement.** `History.tsx:339` passes
`weekdayLabel={dateSpanLabel ? "RECENT SESSIONS" : formatWeekdayLabel(...)}`, and since
`dateSpanLabel` is non-null for every multi-session window, **the actual weekday is always
suppressed** — the prop is a lie. `orbitMath.ts:227-237` (chunk-by-count, label-by-time).
`History.tsx:380` `statusText="SYNCHRONIZING ORBIT"` — no orbital sync is in flight; the
data was already local. `VoiceRippleNode.tsx:19-22` — `barHeight(turnCount, index)` is a
pure function of `turn_count`, so the rendered "voice signature" waveform is **identical
for every session with the same turn count**. It looks like audio and encodes nothing.
*Impact:* a professional tool showing invented numbers is worse than one showing nothing.
The waveform is the most corrosive — it invites the user to believe the app is analysing
their speech. **Tier 2** for removing the fake span (which is why it went unnoticed),
**Tier 3** for removing the waveform.

**P1-5 — The calendar is 42 tab stops with no grid semantics and no way out.**
`CalendarPicker.tsx:178-235` renders a plain `div.grid` of `<button>`s: no `role="grid"`,
`aria-selected`, roving `tabIndex`, or `onKeyDown`. Arrow keys, Home/End, PageUp/PageDown
and Escape all do nothing. `:143,146,153,161,241-243` are hardcoded strings. Session density
is a 4px dot (`:221-231`) with no accessible text. `HistoryListView.tsx:270-283` toggles the
panel with **no outside-click and no Escape** — only the same icon again. No "today" marker.
*Impact:* the only date-navigation affordance in the compact list view
(`History.tsx:354-362` never passes `onPrevDate`/`onNextDate`, so the chevron block at
`HistoryListView.tsx:189-210` never renders). **Tier 3.**

**P2-6 — Page-layer logic, unvirtualized list, and a search that under-promises.**
`History.tsx:110-146` (window-level keymap doing index arithmetic inside `pages/`) and
`:148-197` (window/node-id derivation) violate `frontend-style-guide.md:31`.
`HistoryListView.tsx:315` maps the **entire** filtered set; no `react-window` in
`package.json`; at 5000 sessions that is 5000 card subtrees each with a nested `<button>`.
`:29-36` `fuzzyMatch` is a **character-subsequence** test behind the placeholder
*"Search conversation history..."* — searching `ses` returns every session containing
s-e-s anywhere — and `:120,220` commit to the filter on **every keystroke** with
`resolveSessionTitle(s).toLowerCase()` per session (`:167`), violating `§4.8`'s 150ms rule.
`useHistory.ts:98-113,267-289` have no request-sequence guard, so Retry during an in-flight
fetch lets the earlier response win.
**Tier 1** for the refactor; **Tier 2** at 200 sessions, **Tier 3** at 5000 for virtualization.

### Minor observations

- **`ViewSelector.tsx` is fully built and completely unused.** Correct
  `role="tablist"`/`role="tab"`/`aria-selected` with arrow-key roving focus (`:21-36`).
  `CentralClockNode.tsx:211-247` hand-rolls the same toggle as two unlabelled `rounded-full`
  buttons and hardcodes `"DAY"` at `:231` while pulling `"MONTH"` from `HISTORY_COPY` at
  `:245`. **A correct component exists and was bypassed.** (The `ViewSelector` pattern
  recurs: A4 found `shared/ui/Badge.tsx` has **zero importers** too.)
- **`orbitMath.ts` has no React/three/tauri imports** — only `import type` at `:1`, which
  TypeScript elides. `§4.11` satisfied. A genuine pass, stated plainly.
- Six `glass-card` hand-rolls bypass the `Card.tsx` elevation enum: `History.tsx:214,236,250`,
  `CalendarPicker.tsx:125`, `DetailPanel.tsx:54,160`. **Each assistant bubble in
  `DetailPanel.tsx:54` carries a `backdrop-blur-md` — a 200-turn session stacks 200 of them.**
- `CalendarPicker.tsx:14` duplicates `orbitMath.ts:332` (`toDayKey`/`toDateKey` are the same
  function written twice in two files).
- `ChamberOrbitRings.tsx:42,50` hardcode global SVG ids `frontArcGlow`/`orbitNeonGlow`. Only
  one instance mounts today, but Home would collide silently.
- `CentralClockNode.tsx:131-133` — the light and dark branches are **byte-identical**.
- `CentralClockNode.tsx:177-203` — `Array.from({length: windowProgress.count})` runs trig for
  every window then `return null` at `:190` for all but the current one. At 208 windows that's
  208 wasted trig pairs plus 208 null children per render.
- `CentralClockNode.tsx:145-167` — 48 SVG `<line>` ticks rebuilt every render. Should be a
  module constant.
- Latent crash, `History.tsx:293` — `currentMonthGroup.monthKey` unguarded;
  `currentMonthGroup` is `undefined` when `monthGroups` is empty. Not reachable today.
  **unverified** under any future deep-link.
- `History.tsx:202` — the stage `onClick` catches everything; the error banner's dismiss
  button (`:221-227`) has no `stopPropagation`, so dismissing a delete error also closes the
  transcript panel.
- `useHistory` returns ~10 symbols History never consumes: `totalDates`, `dateIndex`,
  `currentDateSessions`, `isCompactHeight`, `handleGoToday`, `handleBackToMonth`,
  `formatMonthShortLabel`, `formatDayHeroLabel`. `handleGoToday` + `CentralClockNode`'s
  `onGoToday`/`breadcrumbLabel` props are **built and never wired** — no "jump to today"
  exists anywhere.
- `useHistory.ts:434-446` — `dayTimeSpan` calls `toLocaleTimeString` twice per window.
  `Intl` formatters are expensive.
- 11 dead copy keys in `historyCopy.ts` (`allSessionsCount`, `today`, `viewDay`, `viewMonth`,
  `windowSingular`, `windowPlural`, `closeSession`, `noTranscript`, `hearingTime`,
  `thinkingTime`, `userLabel`, `sessionPrefix`) while `History.tsx:355` hand-rolls
  `Sessions (...)` / `"All Sessions"`.
- `ChamberOrbitRings.tsx:13` doc comment claims *"Costs < 1MB RAM and 0% idle CPU."* The 0%
  idle claim is true. The RAM number is **unverified** — measure it or delete it.

### Questions

1. **If `orbitMath.ts:166-174` distributes by count and not by time, is this a timeline at
   all?** Would anyone notice if you deleted the ring and shipped a 24-hour radial scatter
   — `timeToDialAngle` already exists and works — where *position* is the time and *radius*
   is the turn count? You'd get simultaneity for free and you'd stop lying about spans.
2. **Why is `.no-blur` applied to the one subtree that contains no `backdrop-filter`?**
   Someone profiled, found the toggle helped, and shipped it against the wrong element —
   which means the *measured* improvement came from something else. Have we verified the fix
   did what the comment claims?
3. **If the frontend "feels heavy," have we ever measured History with the app *idle*, or
   only while dragging?** Idle is already 0 FPS. The remaining cost is backdrop-filters and
   up to 24 simultaneous `filter: blur()` layers — all GPU/compositor, none of it JS.
   **Are we optimising the wrong language?**
4. **Soft delete exists (`historyService.ts:105`, `hard = false`) but there is no undo, and
   deletion is a 3-second icon swap.** For a tool whose entire value is an irreplaceable
   record of what the user said out loud, should the confirmation name the session and offer
   a 10-second undo?

---

## 5. Domain A3 — Settings

**Cognitive load: 4 of 8 FAIL -> HIGH. Visible controls at default state: ~55-70.**
The most likely place in the app for "the frontend feels laggy."

**The specificity verdict is split.** The *visuals* are authored — the `wm-globe-float`
line-art chronometer that toggles silence auto-stop (`DictationConfigDesk.tsx:221-388`)
is a 12-tick dial with an arc sweeping to `clampedSecs/5.0 * 270deg`, a decaying
three-bar speech waveform left of a dashed silence gate, a flatline right, and a commit
pip at the gate. The same idea recurs as a 12-tick clock for memory consolidation
(`PersonalMemoryConfigDesk.tsx:231-287`) and a gate SVG for auto-apply (`:469-552`).
A designer understood that a dictation tool's silence threshold *is* a physical
phenomenon and drew it.

What is *not* authored is the information architecture underneath: **six independent
sub-navigation systems coexist on one screen with no shared grammar** — a hex radial ring,
an underlined `tablist`, a `SegmentedControl`, a `TriangularLoopSelector`, a 5-node
topology strip, and a 4-node secondary strip. Each is individually well-made; together the
user must learn six different "move sideways" gestures to configure one product. **The
metaphors are strong enough that nobody unified them.** That gap — authored surface,
conventional guts — is exactly the shape of "heavy."

The second failure is honesty. The app is CPU-first Tier 1A, and the surfaces that matter
most (VAD sensitivity, silence duration, TTS threads, LLM context window, top-k facts)
all present **preset button grids** (`VadWorkspace.tsx:108-118`,
`PersonalMemoryConfigDesk.tsx:344-358,396-410`) with an escape-hatch custom field. That is
the correct, product-specific decision for this hardware class, and it is the one place
where the design is genuinely tuned to the constraint rather than to a generic web app.

### Strengths

- **`SettingsCardWrapper.tsx:62-67` + `index.css:847-855`** — the
  `has-unsaved-changes` footer seam. The card visually grows a footer without the card
  component knowing a footer exists, and it works identically across 8 differently-authored
  card shells. Most codebases solve this with 8 duplicated prop chains.
- **`settingsStore.ts:869-911`** — the save-failure path is honest. Catches per-key
  rejections, re-reads authoritative state from the backend, **replaces the draft
  wholesale** so a rejected key can't linger and retry forever, maps each failure to its
  owning card via `SETTINGS_DOMAIN_TO_UI` (`:382-396`), and surfaces it as a `role="alert"`
  footer naming the exact keys (`SettingsCardWrapper.tsx:158-177`). Most apps optimistically
  snap the toggle back with no message. **This one tells you which key failed.**
- **`index.css:126-141`** — a proper `focus-visible` ring across
  `button/input/select/textarea/[tabindex]/[role=switch]/[role=radio]/[role=checkbox]` with
  `*:focus:not(:focus-visible)` suppressing it for mouse. `SegmentedControl.tsx:38-61`
  implements working arrow-key roving. `ModelsTopologyMap.tsx:79` does roving `tabIndex`.
  **Above average for a Tauri app.**
- `ModelsCard.tsx:235-257` — `setInterval(checkHealth, 5000)`, correctly cleared, correctly
  gated on `document.hidden` and the active tab. **The one polling loop done right.**
  `ModelsCard.tsx:102-117` uses a proper `isMounted` guard. Both are examples to copy.

**Verified still holding from the perf ledger SS2.4/SS2.7:** `AppearanceCard.tsx:21,32,89`
buffers colour and commits on `onPointerUp`; category-scoped dirty tracking and scalar
scope keys hold (`settingsCopy.ts:38-137`); `useSettingsPage.ts` uses functional updaters.
**`SS2.4 item 1` (context elimination) does NOT hold — see below.**

### Priority issues

**P1-1 — `interaction.pipeline_mode` is classified hot; the app freezes after a green
"CHANGES SAVED" toast.** `settingsStore.ts:434-473`. `isRestartKey` covers
`audio.input_device`, six `stt` keys, nine `llm` keys, nine `tts` keys, and `vad.vad_backend`.
**`interaction` is not in the function at all.** `PipelineModeCard.tsx:27` calls
`updateDraft("interaction", "pipeline_mode", ...)`.
*Impact:* Switching modular<->realtime is the single most destructive operation in the
product — it tears down and rebuilds the entire audio pipeline. The frontend calls it *hot*,
so `updateDraft` auto-commits 600ms later (`settingsStore.ts:645-664`) and
`SettingsCardWrapper.tsx:140-154` shows **"CHANGES SAVED"**. Then the engine rebuilds for
seconds with no loader. **The app asserts success immediately before the operation that
could fail.** Same class hits `dictation.hotkey` (registers a global X11 grab) and every
`realtime.*` key.
**Note:** `settingsStore.ts:338-343` states *"The UI must never reconstruct this from a
local key list"* — and this list is what drives the UI. `SS2.7` of the perf ledger claims
this was completed. **It was not.** **Tier 3.**

**P1-2 — Closing a card silently reverts uncommitted changes.** `useSettingsPage.ts:43-52`
fires `discardDomainChanges(domainId)` for every just-closed card, triggered by Escape
(`Settings.tsx:122`), outside-click (`:98`), hub toggle, and radial-node re-click. The
comment calls it *"discard them safely."* **It is not safe.**
Two distinct losses: (1) a restart-classified change **never auto-commits by design**
(`settingsStore.ts:631-639`) — the card is *supposed* to sit dirty with "APPLY & RESTART"
on the footer — and closing destroys it with no prompt, while the user is looking directly
at an Apply button. (2) `discardDomainChanges` clears `settingsAutoSaveTimer`
(`:771-774`), so **any hot change made in the preceding 600ms is dropped without ever
having been written.** Click a toggle, close the card — normal, fast, reasonable — and the
setting silently reverts. **There is no undo anywhere on this surface.** **Tier 3.**

**P1-3 — `DictationConfigDesk` writes to the backend on every keystroke, then silently
swallows failure.** `DictationConfigDesk.tsx:85-101`:
`setSecondsDraft(val)` then, if valid, `updateDraft(...)` **and**
`updateSetting(...).catch(console.error)` (`:97`). Identical unguarded direct-IPC at `:76`,
`:81`, `:109`, `:113`, `:119`. **Every other control on this surface goes through the store's
600ms debounce; these five bypass it entirely.**
*Impact:* this is the SS4.8 violation and the most likely single cause of "the frontend
feels laggy." Each character typed is a Tauri IPC round trip — type `2.5`, get two writes.
Worse: `.catch(console.error)` means a rejected write leaves `secondsDraft` (`:682`,
optimistic local state) showing `2.5` while the persisted value is still `1200`. **The field
lies**, and the footer will not fire because `updateDraft` marks the card clean against a
`snapshot the backend never accepted. **Tier 2** for the lag, **Tier 3** for the lying field.

**P1-4 — The engine restart is invisible on desktop.** `restartInFlight` is consumed at
exactly one place: `Settings.tsx:262-275`, inside the `isCompact` ternary's **mobile
branch** (`:227`). `SettingsCardWrapper.tsx:75-179` has four footer states — missing-key,
apply-restart, saved, save-failed — and **no restarting state.**
*Impact:* the user clicks "APPLY & RESTART", the backend begins a multi-second rebuild, and
on a 1440px display: no spinner, no dim, no loader, no copy. Interaction is dead. The store
even polls for completion with a bounded 400ms interval for 12s (`settingsStore.ts:942-953`)
and throws the result at a component that doesn't render it. For a configure-once-then-forget
product, an unexplained multi-second freeze on the config screen is where trust is lost —
and it recurs every time they change a model. **Tier 3.**

**P2-5 — Seven lazy cards with a prewarm that doesn't exist.** `Settings.tsx:11` says
`// Loader functions for eager prewarming` and defines 7 named loaders (`:12-18`).
`rg` across all of `app/src` returns **only those definitions and the 7 `lazy()` calls at
`:21-27`.** Nothing calls them eagerly.
*Note:* this is **not** run 1's unreachable-Suspense defect — `Settings.tsx:34`'s fallback
is a real branded skeleton (`SettingsCardSkeleton.tsx`), so the fallback is reachable. The
problem is the cold fetch starts *at* the click (because `SettingsCardWrapper` only renders
children when `isActive`, `:54`). Meanwhile `useSettingsPage.ts:212-214` schedules three
re-measures at 100/320/600ms plus a `ResizeObserver` on all six cards (`:218-227`)
specifically because *"lazy-loaded card chunks mount and expand"* — so the SVG connectors
visibly re-settle after each cold open.
Secondary: `SettingsCardSkeleton.tsx:15-16` returns `bg-transparent p-0` for
`layoutMode === "small"`, but the real mobile shell is `glass-card rounded-2xl p-4 sm:p-5`
(`Settings.tsx:355`) — so on compact the skeleton has no card chrome and the list visibly
reflows when the chunk lands. **Tier 2.**

**P2-6 — `PersonaCard` rebuilds a full syntax-highlight tree per keystroke and silently
eats input.** `PersonaCard.tsx:262-277` computes `getProtectedXmlTagSpans` and diffs the
tag list on every keystroke, invalidating `renderSyntaxHighlightedText` (`:228`) and
`parseXmlToSections` (`:286-288`) — a fresh React element tree for the **entire** prompt in
the backdrop layer (`:332`). No debounce (SS4.8 requires >=150ms).
Two problems: **perf** — a 4,000-character persona prompt means a full regex tokenize +
element-tree construction per character on a CPU-first box; that is "typing feels heavy."
**Correctness** — the `return` at `:270` drops the keystroke with *no* error, no shake, no
toast, no copy explaining that tags are protected. A user who types `<` and starts forming a
tag watches the field simply stop accepting characters. **The most confusing possible failure
with zero diagnostics.** **Tier 2** for cost, **Tier 3** for the swallow.

**P3-7 — `RealtimeVisualElements` value badge and a colour-only toggle.**
`:313-315` renders a `rounded-md` + `bg-[rgb(var(--accent))]/10` + `border` pill containing
`{value.toFixed(2)}`, immediately above the `type="range"` at `:317-329` whose thumb sits at
that value. That is the literal `SS5:116` example ("repeating 1.05x") and the recent purge
missed it. The sibling `RotaryKnob.tsx:171` puts `formatValue(value)` *inside* the knob face
— the correct pattern — which proves the fix is known.
Separately `RealtimeToggleRow`'s switch at `:371-385` is a bare `<div>` track + sliding white
dot: state is colour + position only, no `role="switch"`, no `aria-checked`. But
`ToggleTile.tsx:45`, `DictationConfigDesk.tsx:639`, and `PersonalMemoryConfigDesk.tsx:207`
all carry it. **Tier 2.**

### Cognitive load detail

Fails: (1) 6 cards + 7 radial controls visible at once, and 5 nodes in the Models pipeline
tablist — exceeds the 4-item limit; (2) 6 cards each with their own save footer, 3 with
independent drills — no single primary action; (3) **6 concurrent independent
sub-navigation grammars** — not grouped by task; (4) **20 cloud providers** in a 160px
scrolling grid (`providersCopy.tsx:14-33`, `LlmConfigDesk.tsx:556`).

Decision points with >4 options: Models pipeline topology = **5**
(`ModelsTopologyMap.tsx:10`); Cloud LLM provider = **20** (mitigated by search + sort +
scroll); TTS voice carousel = **unbounded** (`modelCatalog.voices`).

**A single setting is 3 clicks deep** (`Settings.tsx:208` hub -> radial node -> topology tab
-> subtab -> workspace -> preset), and there are 6 such ladders on screen at once.

### Emotional journey

Arrival is good (branded `OrbitalLoader` at `:131-136`, hex grid resolves, radial hub reads
as a system). First card open is the first stall (skeleton holds the space, but connectors
re-settle). Changing a hot setting is smooth then a lie (`ModelStatusOverlay` is mounted
app-wide via `ResponsiveLayout.tsx:480` and re-fires two `checkModelExists` IPCs on *every*
store change — the user feels it as a beat of heaviness on every interaction). **Changing a
setting that rebuilds the engine is the emotional low point** (see P1-1/P1-4). **Recovery is
data loss** (P1-2). And **interrogation is missing** — if the user asks "did that take?", the
only evidence is the state of the control.

### Minor observations

- `LlmConfigDesk.tsx:88` `|| "openai"`. A user with `base_url = "https://api.anthropic.com"`
  (no `/v1`) fails the `startsWith` at `:86` and the UI displays **"OpenAI"** as the active
  cloud provider while Anthropic is configured.
- `PersonalMemoryConfigDesk.tsx:314` `placeholder="09:00"` contradicts the actual default
  `"02:00"` (`:55`).
- **Undeletable numeric inputs** — `PersonalMemoryConfigDesk.tsx:95,107`,
  `TtsVoiceManager.tsx:340`, `VadWorkspace.tsx:125,164,203,243` all do `if (!clean) return;`.
  Because the value is controlled (`:370`, `:422`), **backspacing the last character snaps the
  value back. The user cannot clear the field.**
- `ModelStatusOverlay.tsx:29` `const [, setTtsCaps] = useState(...)` fetches
  `getProviderCaps` at `:80-92` and the result is **never read**. A live IPC per `ttsKind`
  change for nothing.
- `SegmentedControl.tsx:76` `aria-label={opt.title || opt.label || opt.id}`. When a segment
  is disabled *with an explanatory title*, the title **replaces** the name.
  `ModelsCard.tsx:390-395` therefore produces a button whose accessible name is "Complete
  server setup first to configure voices" instead of "Settings". WCAG 2.5.3 failure.
- `SegmentedControl.tsx:74-75` `aria-selected` on `<button>` inside `role="radiogroup"`.
  The correct pair is `role="radio"` + `aria-checked`.
- `ModelsTopologyMap.tsx:70` — `isCategoryDirty(id)` is called *during render* from a stable
  function reference, so it is **not a real subscription.** The dirty dot updates only
  because `ModelsCard.tsx:52` subscribes the whole `draftSettings` and re-renders the parent.
  **Works by accident.**
- `ModelsTopologyMap.tsx:91-96` — dirty state is a 1.5px `bg-amber-400` dot with a `title` on
  a `<span>`. Hardcoded amber (against the recent purge) plus hardcoded `rgb(251,191,36)`,
  and entirely absent to screen readers.
- `Settings.tsx:93-97` — `anyNeedsRestart` is used only at `:285/298/302/304`, all inside the
  mobile branch. On desktop it is a **live store subscription running six
  `isDomainRequiringRestart` sweeps per `set()` for no rendered output.**
- `Settings.tsx:86-87` and `SettingsCardWrapper.tsx:27-30` — `isCloudSttMissingKey = false`
  with a TODO. Selecting cloud STT is a documented dead end. Two parallel copies of the same
  suppression.
- `RadialHub.tsx:36` — `backdrop-blur-sm` on an element with `hover:scale-[1.06]
  transition-all duration-400`. `.no-blur` exists at `index.css:736-740` precisely for this
  WebKitGTK case and is used **only** in `OrbitCarousel.tsx` app-wide. **Six of these animate
  on every card open.** Also `HubConnectors.tsx:126-156` restyles 18 SVG `<line>` elements
  with `transition-all duration-400` — main-thread SVG repaint, not composited.
- Untracked timers: `Settings.tsx:333` (4s), `DictationConfigDesk.tsx:140,159` (2.2s),
  `settingsStore.ts:542,553`.
- `settingsStore.ts:942-953` — `trackRestartCompletion`'s `setInterval` is cleared only by its
  own tick logic; there is no teardown path. Bounded at 30 ticks so it self-heals, but it is a
  `setInterval` with no owner.
- `ApiKeyField.tsx:57` "Test Connection" is never wired on `LlmConfigDesk.tsx:485-490` (no
  `onTestConnection` passed) — **the affordance is dead there.** And the field has no
  paste-from-clipboard and no clear button.
- Copy drift: `DictationConfigDesk.tsx:609` tooltip promises "click time badge to type any
  duration in seconds (>0s)" but `:94` silently *rejects* <=0 and
  `handleSecondsBlur:106-109` snaps to `1.2` with no message.
  `PersonalMemoryConfigDesk.tsx:462-466` hardcodes "Click to switch to Manual Review" while
  `copy.suggestions.manualLabel` exists.
- 17 hardcoded strings in attributes + ~25 in JSX text. Most concentrated in
  `DictationConfigDesk.tsx` (`:32-36` `TABS` is entirely inline while `DICTATION_COPY` exists)
  and `LlmConfigDesk.tsx` (`:196` `"Connection failed"`, `:525`, `:545-546`, `:599`, `:607`).

### Questions

1. **If `isRestartKey` is deleted, what breaks first — and does that tell us the backend's
   real answer was never adopted?** The store comment at `:338-343` asserts backend
   ownership and `SS2.7` claims it was completed, yet a 24-entry frontend table still
   decides when the engine freezes. Either the backend policy was never authoritative here,
   or it was adopted and this list was left behind as dead-but-load-bearing code. **If the
   latter, `interaction.pipeline_mode` has been silently mis-classified since the day it was
   written.**
2. **The hex hub is a navigation *metaphor*, not a navigation *system*.** Is the right move
   unifying the six grammars under one pattern — or acknowledging that a power tool with six
   independent workbenches *should* feel like six tools, and fixing only the affordances
   (why-restart, undo, discard) rather than the grammar?
3. **The Settings surface is the only place where a user can lose data with no feedback, and
   the only place an operation can freeze the UI for seconds. Both live in the same three
   files.** Is the durable fix a single `SaveState` state machine
   (`idle | dirty | saving | saved | failed | restarting`) that the wrapper renders
   exhaustively, replacing the current five ad-hoc booleans? That would make the missing
   states *impossible* to omit rather than merely absent today.
4. **`appearance` is the only domain with a bespoke debounce path** (`settingsStore.ts:578-607`)
   and the only one that writes `providers.jsonc` at 200ms while everything else goes through
   `commitChanges` at 600ms. Was that considered, or an artifact of the colour picker being
   built first? It has a hole — drag the saturation handle and release outside the picker and
   the colour silently reverts on card close.

---

## 6. Domain A4 — Monitoring, Shared Primitives, Profiler, TitleBar

**Aggregate: 30/64 (2.1/4) over 16 sub-criteria.**

**Specificity verdict: the Monitoring page and the profiler are genuinely authored; the
shared primitive layer is not.** `LiquidChamber` is not a generic dashboard widget — it is
a two-fluid simulation where the RAM level is the Vox process's own PSS fraction
(`LiquidChamber.tsx:92,156-157`) and CPU is the Vox process's own thread utilisation
(`:159-160`), with the three pipeline roles surfaced as residency indicators.
`MONITORING_COPY.chamberSubtitle` = *"Your computer's activity, visualized"* is the right
claim. `Monitoring.tsx:156-182` correctly resolves variant labels to "On Device" / "In
Cloud" / "On Server" from the actual provider manifest group rather than guessing from
provider names. **That is product knowledge, not theming.**

**The profiler is a real instrument — and its frontend half reports conclusions it never
computed.** See P0-1.

**Strengths — do not "fix" these:**

- **`LiquidChamber.tsx:339-348` — a textbook 2D-canvas teardown.** `running = false` ->
  `removeEventListener("visibilitychange")` -> `resizeObserver.disconnect()` ->
  `cancelAnimationFrame(rafId)` -> `canvas.width = 1; canvas.height = 1`. All three
  `SS4.6` requirements in the right order, **plus** a `visibilitychange` handler (`:322-334`)
  that correctly re-primes `lastFrameTime` on wake so the first frame after a resume isn't a
  10-second `elapsed` spike. `LiveWaveform.tsx:114-120` matches it. **This is the standard the
  rest of the app should meet, and it does.**
- **`AmbientBackground.tsx:100-157` — idle self-stopping rAF with layer demotion.** When
  telemetry energy settles below 0.001 (`:132`) it stops the loop, restores base opacity, and
  **demotes `will-change` from `transform` to `auto`** (`:138-140`) so the compositor
  releases the blob textures, then a 600ms `checkInterval` (`:152`) re-arms only if energy
  actually returned. Gated on `document.hidden` in three places. **The single most correct
  piece of runtime resource management in the app.**
- **`notificationStore.ts:202-353` — referential stability done correctly.** Six module-level
  memo caches keyed on `state.notifications` identity (and `activeActionIds` for the one
  selector that needs it, `:327-331`), with early-return guards. This is exactly the
  discipline `useSyncExternalStore` demands and is almost always got wrong — most
  implementations return a fresh array from a selector and cause an infinite render loop.
  `NotificationPanel.tsx:345-352` then uses nine fine-grained atomic selectors, no
  full-store destructure. **`SS4.2` fully satisfied, correctly.**
- **`SessionContextMenu.tsx:69-97` — the keyboard model the other primitives should copy.**
  `role="menu"`/`role="menuitem"`, wrap-around ArrowUp/ArrowDown, Enter via `data-action`,
  Escape with `stopPropagation`, and a `data-context-menu` attribute that `EdgePanel.tsx:52-54`
  explicitly honours. Compare `ProfilerPanel.tsx:44` and `NotificationPanel.tsx:433`, which
  **omit** the `stopPropagation()` that suppresses the global spatial navigator — a concrete
  one-line delta.
- **`BottomDockFeather.tsx:20-25` — a primitive that explains itself.** Module-constant style
  object (zero per-render allocation), `aria-hidden`, `pointer-events-none` baked in, and a
  doc comment stating *"Deliberately zero `backdrop-filter`: the dissolve alone reads
  identically at these sizes for a fraction of the compositor cost"* plus the
  `relative`-sibling contract callers must honour. **The only place in 22 primitives where
  the perf reasoning is written down next to the code.**
- `ToggleTile.tsx:44-56` — `role="switch"`, `aria-checked`, `aria-label`, `aria-disabled`,
  `tabIndex={disabled ? -1 : 0}`, `onClick={disabled ? undefined : onToggle}`, *and* a
  re-added `focus-visible:ring-2` at `:53` to compensate for its own `focus:outline-none`.
  **The most complete a11y treatment of any primitive here.**
- `ProjectContextMenu.tsx:184-208` — idle->confirm->error state machine with `AlertCircle`
  and `disabled={isDeleting}`. Better than `SessionContextMenu`'s 2-state version.

### Priority issues

**P0-1 — The profiler's leak detection is structurally dead; the instrument reports
conclusions it never computed.** `useMemoryProfiler.ts:106-107` copies `retained` and
`retainedDeltaMb` forward as `existing?.retained ?? null` / `existing?.retainedDeltaMb ?? null`
and **never assigns a non-null value.** `unmountedAt` (`:19`) is declared and never written.
Every consumer is therefore unreachable:
- `PagesTab.tsx:152-153` reads them -> always `undefined`
- `PagesTab.tsx:158` `if (retainedDelta !== null && ...)` -> never true -> `riskBadge` stays
  at the `"Normal"` branch (`:156`) for every row, **permanently**
- `PagesTab.tsx:199` `rec?.unmountedAt` -> never true -> a route you visited and left renders
  **"Unvisited"** (`:204`)
- `PagesTab.tsx:242-245` -> the *current* route renders `PROFILER_COPY.pages.measuringOnExit`
  **indefinitely**, promising a measurement that never arrives
- `InsightsTab.tsx:71-81` — the `retainedDeltaMb > 15` leak heuristic, **the drawer's reason
  for existing**, can never fire

Compounding: `performance-memory-optimizations.md:238` instructs the reader to *"Manual
snapshot -> check `retainedDeltaMb`"* as step 3 of the leak-triage procedure, and `:101`
claims `PageMemoryRecord` "Tracks `baseline` -> `peak` -> `retained` RSS per route to flag
monotonic memory growth." **The documented triage workflow is unexecutable.**
*Impact:* a diagnostic tool whose primary question is "is this leaking?" has a broken answer
path, and the UI covers the break with a confident `MEASURED` badge
(`OverviewTab.tsx:93,135,342`) and an infinite "Measuring on exit...". **A user who trusts
the tool draws the wrong conclusion about their own machine.** Per `AGENTS.md` SS4.3 this is
a spec/code divergence requiring a stop, not a silent fix. **Tier 3** — users will notice,
in the worst way. A blank metric says "I don't know"; a green MEASURED badge over `--` says
"I know, and it's fine," and the user stops looking.

**P0-2 — IPC call inside a `setState` updater in a shared primitive.**
`VoiceCarousel.tsx:137-153`:
```
interval = setInterval(() => {
  setRecordingDuration((prev) => {
    if (prev >= 30) { handleStopRecording(); return 30; }
    return prev + 1;
  });
}, 1000);
```
`handleStopRecording` (`:170-181`) awaits `stopBackendRecording()`, a Tauri IPC that tears
down the recorder. This is the exact pattern `frontend-style-guide.md:76-77` bans.
React 19 double-invokes updaters in StrictMode to surface impurity; the updater is impure.
**A double `stopBackendRecording` on a live audio recorder is a resource-teardown race in a
Tauri backend, not a UI glitch.** Also `:143` forward-references `handleStopRecording`, a
`const` declared at `:170`. **Tier 1** — zero visible change; fix for correctness.

**P1-3 — The perf ledger documents three optimisations that do not exist in the code.** See
the full table in SS10. Summary: **SS2.6.5** claims a `Markdown` plain-text fast path
eliminating 80-150ms click latency — **the regex appears nowhere in `app/src`**;
`Markdown.tsx:300` calls `<ReactMarkdown>` unconditionally.
**SS2.2.1** claims `useDynamicFPS` cancels RAF when hidden via `IntersectionObserver` —
**the hook never constructs one** despite its own `:8` docstring claiming it; `LiveWaveform.tsx:77-83`
passes `fpsIdle: 0` (not 15) and neither `isVisible` nor `isPageVisible`, so both default
`true` and the RAF keeps scheduling at display rate while the tab is hidden.
**SS2.2.4** claims reduced-motion is honoured — true for CSS-driven surfaces, but
`LiquidChamber` and `LiveWaveform` draw via **JS canvas rAF**, which a CSS media query
cannot throttle. **Tier 1** (documentation). **This doc's claim that `SS2.2.1` HOLDS is
contradicted here on the IntersectionObserver detail** — treat the ledger's
IntersectionObserver and reduced-motion sub-claims as FAILS/PARTIAL and the 60/15 tier
values as HOLDS.

**P1-4 — The "closed elevation system" has ~12% adoption and the bypasses are the shell.**
See CC-1. Add: `ErrorBoundary.tsx:75,87,97` uses bare `glass` where its sibling at `:53` uses
`glass-card` — so the error card mixes two elevations and gives the *secondary* action the
higher one (`:103` `glass-card` for Retry vs `:97` `glass` for Home).
`Badge.tsx:26-32` builds 4 of its 7 variants on raw `emerald-500`/`amber-500`/`rose-500`/
`purple-500` while `NotificationPanel.tsx:58-83` correctly uses `var(--error)`/
`var(--warning)`/`var(--violet)`/`var(--accent)`.
**Fix shape:** (1) make the enum real and the escape hatch expensive — add the missing
`whisper`/`elevated` realisations, delete the redundant `elevated` boolean, and **add a lint
rule banning the literal string `glass-card` outside `Card.tsx`**, converting a convention
into an enforced invariant; (2) migrate **shell-first in one commit** (`EdgeNav`,
`EdgePanel`, `Drawer`, `ErrorBoundary`, `Monitoring`, `Home`, `History`,
`ResponsiveLayout`) so the app's frame is coherent before the settings cards are touched.
**Tier 3** — users will notice, *in the wrong direction*: a correct migration shifts border
alpha, blur strength, and background opacity across the whole app at once. That is the
point, but it needs a real visual review pass, not a spot check. `RotaryKnob.tsx:169` alone
is **Tier 2**.

**P1-5 — Six icon-only buttons with no accessible name, and an unnamed primary input.**

| Location | Control | Problem |
|---|---|---|
| `RotaryKnob.tsx:115-122` | `-` stepper | `Tooltip` only; `Tooltip` sets `aria-describedby`, never `aria-label`. Announced as "button" |
| `RotaryKnob.tsx:179-186` | `+` stepper | same |
| `ApiKeyField.tsx:80-86` | show/hide key | `Tooltip` at `:79` only; `"Hide key"`/`"Show key"` also hardcoded |
| `VoiceCarousel.tsx:275-287` | choose file | `Tooltip` at `:272-274` only — while `:397,409,432,440,458,469,501,515` in the same file *do* have them |
| `VoiceCarousel.tsx:291-307` | record / stop | `Tooltip` at `:290` only |
| `VoiceCarousel.tsx:481-487` | clone voice | `Tooltip` at `:480` only |
| `UnderlineInput.tsx:35-45,58-68` | the input | label is a `<span>`, not a `<label>`; no `id`, no `htmlFor`, no `aria-label`/`aria-labelledby`. **Clicking the label does not focus the field and the input is announced with no name.** `errorMessage` (`:76-80`) is not wired via `aria-describedby` and `error` never becomes `aria-invalid` |

`UnderlineInput` is the field primitive for API keys and model names — the highest-value
inputs in the app. **Tier 1** — no pixel changes at all.
**Separate Tier 2 bug:** `ApiKeyField.tsx:79` `className="absolute right-0"` —
`Tooltip.tsx:127` applies `className` to the **floating bubble, not the wrapper**, so the
eye button is **not** absolutely positioned and overlaps the `w-full pr-7` input.

**P1-6 — A failed notification fetch renders "You're all caught up."**
`notificationStore.ts:22-33` declares `notifications`, `activeActionIds`, `loading`, and six
actions. **There is no `error` field.** `fetchNotifications` at `:40-49` catches, logs, and
sets `{ loading: false }` with the array untouched. `NotificationPanel.tsx:550` branches on
`loading && displayedItems.length === 0` (skeleton), `:569` on `displayedItems.length === 0`
(empty). A rejected fetch lands at `:569` and the user sees
`NOTIFICATION_COPY.tasksEmptyTitle`/`tasksEmptySubtitle` — the "all caught up" panel.
*Impact:* notifications are where the app reports **pipeline failures, model-load errors, and
hardware problems**. During the exact window when the backend is unhealthy — the window when
notifications matter most — the panel confidently says there is nothing to report. **The most
load-bearing empty state in the product, and it lies under failure.** **Tier 3** — users
will notice, because they currently see the wrong thing.

### Cognitive load: 7 of 8 fail or partially fail -> HIGH

Fails: (3) "the Live Timeline" is not live — `ENABLE_DIAGNOSTIC_POLL = false`
(`useMemoryProfiler.ts:221`) — and every row is labelled with the *current* route (`:267`),
not the route at capture time, so the timeline misattributes its own history.
(6) terminology: "Full-Scope RAM" (`:91`) vs "Current: X MB" labelled "Total Process Tree RSS"
(`:266`) on the same card — two names, one number; "Unvisited" for a route you demonstrably
visited; a "Retained" column that never retains.
(7) placement: `Monitoring.tsx:361` is `z-[200]`, `Tooltip.tsx:125` is `z-[9999]`, context
menus `z-[9999]`, `EdgePanel.tsx:114` is `z-[35]`, `EdgeNav.tsx:39` is `z-[60]` — six stacking
tiers, one (`35`) *below* the page drawer.
(8) recovery: `notificationStore` has no error state; `ErrorBoundary.tsx:35-40` navigates via
hand-rolled `history.pushState` + synthetic `popstate` rather than the router;
`CarouselSelector.tsx:64` cycles silently with no `aria-live`.

Decision points with >4 options: `MetricCarousel` presents 6 metrics as 3 pages of 2 (`:88`)
with 4px-tall unlabelled dots (`:150-162`) — 6 options, no keyboard paging, no `aria-live`.
`SessionContextMenu`'s Move submenu (`:178-227`) lists *N projects + 1* with no search.
`LiquidChamber`'s single UNLOAD/LOAD toggle (`:287-306`) unloads **all** models behind a
Tooltip, **not a confirm**. `HelpPanel` renders the raw internal scope id (`page:home`) as a
user-facing section heading (`:139-141`).

### Emotional journey

**Peak: a `MEASURED` badge over a value that was never computed.** The user opens the
profiler looking for "which page is heavy." Five KPI cards, all stamped MEASURED with a green
check (`:93,117,135,153,171`) — including three rendering `--` because there is no snapshot.
Pages: 7 columns, and the two that matter (Retained, Risk) are `--` and "Normal" for every
row forever. **This valley is worse than having no profiler, because it is confident.**
Second valley in Insights: "Excessive Backdrop Filter Layers — 44 elements" is **permanently
lit** (threshold `> 15` at `:84`, reality 44). An alarm that is always on is decoration.
Exit is a small loss of place: `Drawer.tsx:99-103` restores focus to
`previouslyFocusedRef`, captured as `document.activeElement` at open, which was `<body>`.

### Minor observations

- **`shared/ui/Badge.tsx` has zero importers** — 0 direct, 0 via the barrel (checked all 19
  barrel importers). A fully-built 7-variant/3-size primitive whose four status variants are
  the *worst* token-bypass in the directory. **Adopt or delete; do not leave it exported.**
- `shared/ui/index.ts:11` exports `VoiceBars`, an internal sub-component of
  `VoiceCarousel.tsx:17` that nothing imports externally.
- `shared/components/help/index.ts:1-8` exports 8 diagram components that `HelpPanel.tsx:14-25`
  never imports (it lazily loads only the 4 `*HelpContent` modules). Whether the `*Content`
  files consume them is **unverified**.
- `KeyboardHotkeySkeleton.tsx:7` declares a `recordingPlaceholder` prop never destructured
  (`:133-139`) or rendered.
- **`HelpPanel.tsx:164` serves `HomeHelpContent` under a "Monitoring Guide" header** —
  visibly wrong content in a titled panel. **Tier 3.**
- `MONITORING_COPY`: **35 of 59 keys are dead** (0 referencing files each), while
  `MetricCarousel.tsx:34-84` invents 16 replacement strings.
- `Monitoring.tsx:263` `{false && (<ProfilerButton/>)}` — 18 lines of unreachable JSX. The
  profiler launch button on the Monitoring page is **dead**; `LAYOUT_COPY.nav.openProfiler`
  says "Shift+Up" and that is the only path.
- `useMonitoringMetrics.ts:8,33-36` maintains a 60-entry `history` array that
  `Monitoring.tsx:88-93` never destructures — 60 snapshots retained in React state for zero
  UI benefit, re-spread every second.
- `useMemoryProfiler.ts:163` — `await new Promise(res => setTimeout(res, 350))` inside
  `captureSnapshot`, purely to "ensure visual feedback" on a button that already has
  `disabled` + `animate-spin`. **Adds 350ms to every manual snapshot for nothing.**
- `RotaryKnob.tsx:149-150` — `else { clearInterval(interval) }` where `interval` is
  `undefined` in that branch. Dead; the real cleanup is `:152`.
- `SessionContextMenu.tsx:127-132` and `ProjectContextMenu.tsx:124-130`:
  `AnimatePresence` wraps a single unkeyed child, and both components early-`return null` at
  `:107`/`:106` when closed. **`exit` can never fire** — 3 decorative exit variants across
  2 primitives. **This is the `MemoryNodeTooltip.tsx:27` defect from run 1, repeated.**
- `SessionContextMenu.tsx:70` / `ProjectContextMenu.tsx:48`: both register `keydown` on
  `window` in the **capture** phase, same as `overlayStack.ts:113`.
  `stopPropagation()` does not stop same-node listeners (that needs
  `stopImmediatePropagation`), so **both `onClose()` handlers run on one Escape.** Benign
  today only because `onClose` is idempotent.
- `VoiceCarousel.tsx:356` — `cloningStatus === "Cloning..."` is a hardcoded literal
  duplicating `VOICE_CAROUSEL_COPY.cloning`, which `:361` compares against properly.
  **Change the copy string and the button becomes clickable mid-clone.** Also
  `cloningStatus` is overloaded to hold both progress (`:198`) and error (`:213`) text, and
  errors render in the amber *warning* colour (`:335`) not red.
- `VoiceCarousel.tsx:44-58` `VoiceBars` renders 18 elements each with
  `will-change-transform` + an infinite desynchronised `dynamic-eq` animation — **~18
  compositor layers per voice card.**
- `SegmentedControl.tsx:33` `aria-label={className ?? "segmented control"}` — the group's
  name is read aloud as a Tailwind string. `AppearanceCard` and `ModelsCard` both pass
  `className`, so this is live in at least two settings cards.
- `TriangularLoopSelector.tsx:137,204,206` — `role="radiogroup"` with three `role="radio"`
  children at `tabIndex={-1}` and no arrow-key handling. The only reachable control is a
  nested `role="button"` that cycles — **invalid inside a radiogroup.**
- `Tooltip.tsx:143` `bg-white/10` + `border-white/15` on the `<kbd>` — **Tier 3 in light
  mode**, invisible in dark. Same class of bug `AGENTS.md` records purging from Settings; it
  survives in the shared primitive, so it now appears on **every shortcut tooltip in the app.**
- `TitleBar.tsx:154-183` the popover is `group-hover` only — its own copy button (`:175`)
  and "Manage Models ->" (`:214`) are tabbable while `opacity-0`. **WCAG 2.4.7 failure.**
  Also `:154,190` two accent-gradient pills with the *same* `ArrowUpCircle` icon and
  `hidden sm:inline` text — **below 640px they are two bare identical icons. Tier 3.**
- `TitleBar.tsx:49` `setTimeout` never cleared; `TitleBar.tsx:18-43` `fetchUpdates` has no
  `isMounted` guard. `AmbientBackground.tsx:194` inline ref callback in `.map()` (SS4.7).
- `useMonitoringMetrics.ts:45-56,58-64,72-76` — visibility-gated interval with a `cancelled`
  flag and full teardown. **Correct.**
- `HelpPanel.tsx:33` destructures `onClose: _onClose` and discards it; `:29` declares
  `deepLink` which is never read.
- `OverviewTab.tsx:208` `preserveAspectRatio="none"` distorts stroke width; `:38-56` path is
  navigation-indexed, not time-indexed, yet labelled "Memory Over Time."

---

## 7. Domain A5 — Wizard, Tray, Tokens

**Scores:** Wizard **14/32** (renormalised over 8; heuristics 7 and 10 excluded as
inapplicable to a first-run Persuade flow). Tray **23/40**.

**The inversion is the finding: the wizard scores lower than the tray despite 6x the
surface area and 10x the design effort.** Effort went into `WelcomeStep`'s annotated
mockup; nothing went into recoverability.

### Specificity verdict

**The wizard is generic, and generically so in a specific way: it is a well-built install
wizard wearing a very expensive coat.** The visual system is genuinely Vox — Sora display
headings, cyan accent, glass, a callout-lined annotated tray mockup at
`WelcomeStep.tsx:147-243` that is the single best piece of product storytelling in the
codebase. But the *flow* is a stock six-step form wizard: sidebar with numbered steps,
"Step N of 6", Back/Skip/Continue, one decision per screen. **Nothing about it is specific
to a voice assistant.** The model-selection step is a generic checkbox tree
(`ModelCategory.tsx:59-209`) indistinguishable from a Docker image picker, with sub-labels
like "Generates Vox's replies" doing all the voice-specific work.

**There is no moment in the flow where the user hears or speaks to Vox.** The first and
only voice interaction is step 5, presented as a diagnostic (*"Try It Out"*), not a first
impression. **That ordering is backwards: we ask for the biggest commitment (disk,
bandwidth, trust) before we demonstrate the smallest delight.**

**The tray, by contrast, is authored** — the 380x250 card, the self-dimming status dot that
only pulses while listening (`Header.tsx:25-33`), the character-by-character type-on
renderer, the history scrubber, and the honest `Standby` state. **It reads as a transcript
surface first, controls second.** That is the right hierarchy and a real design decision.

**The specific failure: the tray's ambient-hud promise is not implemented.** See P1-3.

### index.css token inventory

40 custom properties declared at `:5-53`; `[data-theme='light']` (`:61-101`) overrides
**34 of them**.

| Group | Count | Tokens (line) |
|---|--:|---|
| Base colour | 10 | `--background`(6) `--foreground`(7) `--foreground-muted`(8) `--accent`(9) `--accent-dark`(10) `--accent-muted`(11) `--accent-foreground`(12) `--card`(13) `--border`(14) `--sidebar`(15) |
| Layout | 1 | `--vh`(16) |
| Field / Signal | 3 | `--field`(19) `--signal`(20) `--field-energy`(21) |
| Glass colour | 5 | `--glass-tint`(24) `--glass-surface`(25) `--glass-deep`(26) `--glass-navy`(27) `--ghost`(28) |
| Semantic status | 16 | `--success`(31) `--error`(33) `--danger`(35) `--warning`(37) `--info`(40) `--violet`(42) `--pink`(44) `--muted`(45) `--warn-soft`(39) `--muted-soft`(46) + dark variants |
| Derived alpha / connector | 5 | `--connection-glow`(48) `--connection-core`(49) `--hub-connector-*`(50-52) |

**Zero tokens exist for: spacing, radii, motion/duration/easing, typography, z-index,
shadows/elevation.** See CC-3 and CC-4.

**Dead glass tokens.** All 5 `--glass-*` are consumed at most once each and never by the
glass classes: `--glass-tint` **0** consumers, `--glass-navy` **0**, `--glass-surface` 1
(`:577`), `--glass-deep` 2, `--ghost` 1. `.glass` hardcodes its colour at `:182` and
`.glass-card` at `:198`.

**`will-change` inventory.** `index.css` declares it **4 times** — `:635`, `:665`, `:682`,
`:696` — all in the ambient background. `AmbientBackground.tsx` manages it imperatively and
*correctly* (`"transform"` on wake `:92`, demote to `"auto"` when settled `:139`, re-derive
in JSX `:208`). **That is the only place in the app that treats `will-change` as a
resource.** `transform-gpu` / `will-change-transform` appears on exactly **4 elements**:
`Drawer.tsx:204`, `EdgePanel.tsx:114`, `VoiceCarousel.tsx:48`, `RotaryKnob.tsx:131` — and
`test/invariants.test.ts:178-199` asserts *only* the Drawer. **That is why
`check_invariants.mjs` is green.**

Missing on animating surfaces: `WizardRoot.tsx:111` (logo glow cross-fades opacity on
hover); `WelcomeStep.tsx:300-315` (two `motion.div` animating **`width`** and
`backgroundColor` — `width` is layout-triggering, no promotion);
`WelcomeStep.tsx:385-410` (three `motion.path`/`motion.circle` animating `pathLength`);
`ModelCategory.tsx:140-146` (`height: 0 -> 'auto'` across 8 cards);
`LiveTestStep.tsx:123-133` (**15 `motion.div` spring-animating `height`** — the single most
expensive thing in the wizard on llvmpipe); `TrayApp.tsx:281-296` (**the persistent
always-on card animates `opacity`/`x`/`scale` continuously with no `will-change` and no
`transform-gpu`**, despite being the one surface alive 100% of the time).

### Priority issues

**P0-1 — The wizard webview is never destroyed, and the wizard double-mounts on first run.**
`complete_setup_wizard` (`app/src-tauri/src/ipc/setup.rs:109-140`) evals
`window.location.replace('/')` on the **`main`** webview (`:127`), shows and focuses it
(`:130-134`) — and **never touches the `wizard` webview.** `rg 'wizard' app/src-tauri/src`
confirms **zero `.close()` calls** on it anywhere.

Compounding: at boot `lib.rs:580-585` shows the wizard window, *and* `main` is
`"visible": true, "maximized": true` (`tauri.conf.json:16-18`), and `App.tsx:164-169` routes
`main`'s own `setupCompleted === false` into `<WizardRoot />` (`:166`). **Two `WizardRoot`
instances mount, in two webviews, both registering `revealWizard()` at 150ms
(`WizardRoot.tsx:31-33`)**, both running the full `App.tsx` boot including the 4 lazy
page-chunk preloads at `App.tsx:67-70`.

*Impact (a):* Jordan finishes setup, clicks "Start Using Vox", the main window pops forward,
and a 900x650 "Setup Complete" window is **still there** — on a first run that reads as the
app failing to launch.

*(b):* the wizard webview is never reclaimed, so the ~490MB the ledger attributes to `SS1.1`
is never returned. `ipc/monitoring.rs:32` counts `has_wizard` for the profiler, so **the
leak is observable in the app's own Memory drawer** and unfixed.

*(c):* `main`'s duplicate `WizardRoot` means the whole app boot —
`MemoryProfilerProvider`, `VoiceSessionProvider` (6 pipeline listeners + `getSettings` +
`getRuntimeSnapshot` + `getTurns`), `initSpatialNavigation()` (4 global listeners incl.
`mousemove`, `App.tsx:146`), `installOverlayStack()` (`:140`) — runs **twice** on the
first-run machine, which per `SS1` is a Tier 1A 8GB box.

**The perf ledger `SS1.1` claim "Wizard closed after setup completion" is FALSE for the
current code. Tier 3.**

**P0-2 — `prefers-reduced-motion` does not reach a single line of the wizard.**
See CC-5. The wizard contains **12 `motion.*` elements**, **3 infinite `repeat: Infinity`
loops**, **5 `AnimatePresence mode="wait"` cross-fades**, and **2 `layoutId` shared-element
transitions** — all driven through WAAPI / `element.animate`, which the CSS rule cannot
reach.

*Impact:* for a user with vestibular sensitivity — a real and growing population, and one
disproportionately likely to be a first-time installer's friend or family member — the first
90 seconds of Vox is an unbounded field of infinitely-looping pulsing, scaling, and blurs at
100px radii (`WelcomeStep.tsx:142`). **The spec's own floor is 11px type and restrained
uppercase; the motion layer has no floor at all.** **Tier 2** (zero visible change for 96%
of users; the fix is a one-line `<MotionConfig>` wrapper).

**P1-1 — Two of the eight "Mandatory" model categories do not exist.**
`ModelSetupStep.tsx:149-214` hardcodes **8 categories**, 6 marked `required: true`. All
three copies of the manifest contain **14 groups across 6 categories** (`embedding, llm,
stt, translit, tts, vad`). **There is no `nli` group and no `classifier` group.** So
`ModelCategory.tsx` renders two rows — "Memory Checking / Checks new memories against old
ones" and "Smart Sorting / Keeps memories organized and relevant", both badged **Mandatory**
(`:87`) — that can never be selected, contribute 0 bytes, and expand to an empty list.

Separately: `qwen_3_5_0_8b` is `required` in the manifest and therefore auto-selected by
`ModelSetupStep.tsx:48-51` (totalling 9.85GB), but the wizard labels the `llm` category
`required: false` (`:203`) and renders an **unchecked** box. **The UI says "Optional" for
something the app downloads unconditionally and counts toward the Total.**

*Impact:* step 3 of 6 is the trust decision. Two of eight rows are decorative lies; a third
is mislabelled. **Tier 2** — reads as a rendering bug to anyone who looks.
**Fix:** derive the category list from the manifest, and take `required` from
`g.files.some(f => f.required)` rather than the `CATEGORY_META` literal, so the badge and
the auto-select can never disagree.

**P1-2 — `setTimeout` inside a `setState` updater, untracked.**
`usePanelState.tsx:60-69` schedules `setTimeout(() => setRightPanel(null), 0)` **inside**
a `setLeftPanel((currentLeft) => ...)` updater. A direct breach of
`frontend-style-guide.md:76`, **and** `SS4.4`'s "track all nested timers" — the handle is
discarded. The comment at `:61` (*"Schedule right panel clear outside updater to keep it
pure"*) shows the author knew the rule and believed `setTimeout(..., 0)` was outside the
updater. **It isn't.** It is also **redundant**: `openPanel` (`:79-88`) and `togglePanel`
(`:99-109`) already implement the same exclusivity synchronously and correctly.
**Tier 1** — invisible to the user; the state change is visually correct.

**P1-3 — The tray HUD can never auto-fade; the entire `FADING` state is unreachable.**
`useVisibility` (`:52-71`) implements `startFade` and `cancelFade` and documents the machine
as `HIDDEN -> APPEARING -> ACTIVE -> FADING -> HIDDEN` (`:6-8`). `TrayApp.tsx:270` defines a
`FADING` container variant. `index.css:236` defines a 0.4s opacity transition and `:241-249`
define `.tray-container.hidden/.visible`. **None of it can execute.** `rg 'startFade|
cancelFade'` across `app/src` returns only the definition (`useVisibility.ts:52,68,101,102`)
and four plumbing lines in `TrayApp.tsx:60,103,123,130`. **Zero invocations.** Same for
`startNewInteraction` and `endSpeechSegment` — defined (`useInteraction.ts:22,34,64,65`)
and threaded through `stateRef`, never called.

The only paths to `HIDDEN` are `hideImmediately()` (the X, `TrayApp.tsx:154`) and the
backend `onToggleTray` event (`:245-249`).

*Impact:* the tray is specified as a **persistent ambient HUD**. As shipped it is a
non-dismissible overlay: it appears on the first transcript partial and stays on screen, on
top of Casey whatever they are doing, until they find a 16px X. The auto-sleep behaviour is
also gone, so the tray never gives back the screen. Every line of hover-pause logic
(`:79-90`), the 500ms timer, the `FADING` variant, and the CSS transition are dead code
maintaining a promise the product doesn't keep. **This is the largest design-to-ship gap in
the audit. Tier 3** — a permanent 380x250 of Casey's screen until manually closed; everyone
notices.

**P2-1 — The `error` state in the setup machine renders a dead end.**
`setupMachine.ts:122-127` defines an `error` state with working `RETRY -> checking` and
`BACK -> welcome` transitions, and `checking.FAILURE` targets it (`:77-80`).
**`WizardRoot.tsx:51-90` has no `case` for it.** `renderStep()` falls through to `default:`
(`:89`) and renders `WIZARD_STATUS_COPY.unknownState` — literally the string
**"Unknown State"** (`welcomeCopy.ts:163`). A white screen with the word "Unknown" and no
button.

Nothing currently sends `FAILURE`, so this is a latent trap rather than a live bug — but
`WizardRoot.tsx:60` threads `error` into `SystemCheckStep`, so a future `send` lands in a
state with no UI. **Second dead end, same screen:** `SystemCheckStep.tsx:28-30` only
`console.error`s when `getRuntimeReport()` throws, leaving `isLoading` false with
`report === null` — which renders "Checking..." forever behind a disabled CTA.
**Tier 2.**

**P2-2 — The wizard's teaching surface is mouse-only, and two diagnostics are hardcoded.**
`WelcomeStep.tsx:160,175,183,193,211` — all five tooltip triggers are
`onMouseEnter`/`onMouseLeave`. No `onFocus`/`onBlur`, no `tabIndex`, no button semantics.
`WELCOME_DEMO_DEFAULT.desc` (`welcomeCopy.ts:136`) tells the user to *"Hover over Vox's
screen"* — an instruction a keyboard or touch user cannot follow. Compare
`ModelCategory.tsx:71-74`, which gets `role="button"` + `tabIndex` + Enter/Space right:
**the pattern exists in the same directory.**

Hardcoded strings: `TranscriptRenderer.tsx:65` (`"Listening"`/`"Processing"` while
`welcomeCopy.ts:180-181` already defines `LIVE_TEST_COPY.listening`);
`WelcomeStep.tsx:222` (a fabricated `42MB` metric with no copy key);
`LiveTestStep.tsx:138` (a three-branch inline ternary where
`LIVE_TEST_COPY.voiceDetected` at `:179` already exists). Census for wizard+tray is
**6 hardcoded literals** against ~60 centralised. **Tier 2.**
