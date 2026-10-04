# Theme Transition — RCA & Remediation Plan (2026-10-03)

Symptom set reported by user: theme flip is **laggy, non-smooth, and desynchronized**
on Home, History and Settings; the **decorative header on Home/History** and the
**TopRightCluster** are the worst offenders. **Memory is perfect** (duration + sync).
Settings is reported **slow and "in steps"**.

Verdict up front: **there is exactly one implementation, and it is shared by every
page.** Memory is not "better code" — Memory is the same code with every one of its
failure modes switched off (`ResponsiveLayout.tsx:326`). We are also **not** using
industry practice: we animate ~2000 live elements' colors, whereas the standard
approach snapshots the viewport and cross-fades two bitmaps.

---

## 1. The mechanism (single source, all pages)

`applyAppearance()` — `app/src/store/settingsStore.ts:405-444`:

1. `lastAppliedTheme !== null && !== appearance.theme` → it is a real flip (`:411`).
2. `documentElement.setAttribute("data-theme-transition", "true")` (`:415`).
3. Arm a **250 ms** timer to remove the attribute (`:416-419`).
4. Write `data-theme` (`:422`), `--accent` inline (`:423`), `.dark`/`.light` class
   (`:424-430`), `localStorage` (`:435-441`).

`app/src/index.css:185-191` is the only animation mechanism:

```css
@media (prefers-reduced-motion: no-preference) {
  [data-theme-transition],
  [data-theme-transition] * {
    transition: background-color 150ms ease, border-color 150ms ease, color 150ms ease,
                fill 150ms ease, stroke 150ms ease !important;
  }
}
```

Call sites of `applyAppearance` — **5**, all store/event driven, none from rAF:

| line | enclosing fn | theme value | reachable from one toggle? |
|---|---|---|---|
| `:538` | `loadSettings` | fresh | yes, ~280 ms later via the 200 ms appearance debounce → `SettingsUpdated` → `SettingsContext.tsx:36-43`. Backend `ipc/settings.rs:47-51` already mutated in-memory, so `isThemeChange === false` → **no second flip**. |
| `:612` | `updateDraft` appearance branch (`:611-640`) | fresh | **YES — the one real flip**, synchronous |
| `:835` | **`discardDomainChanges`** (NOT `commitChanges` — `:852-965` never calls it) | **stale** | only via per-card Discard while another domain is dirty → **second fade back to old theme** |
| `:1018` | `discardChanges` | stale | mobile ✕, gated on `hasChanges` |
| `:1027` | `restoreDefaults` | fresh | legitimate |

---

## 2. Root causes — six independent defects

### R1 — the `transition` **shorthand** deletes every element's own transitions ⚠ primary

`transition:` is a shorthand, so it also resets `transition-property`. For 250 ms,
**every element in the document** has its transition set narrowed to those 5
properties. Consequences:

- All **232** `transition-all` sites in `app/src` lose their `transform` / `opacity` /
  `filter` / `box-shadow` legs **mid-interaction**.
- `.glass-card`'s own `transition: background 150ms, border-color 150ms, box-shadow 150ms`
  (`index.css:250`) loses its **newly added** `box-shadow` leg. The rule defeats the fix
  shipped alongside it.
- `.amb-glow { transition: opacity 1.5s }` (`:570`), `.rp-wrapper { 0.8s }` (`:596`),
  `.rp-layer { 1.8s }` (`:612`) lose their **only** property — while
  `AmbientBackground.tsx:109-114`'s rAF keeps writing inline `style.opacity` on both.
- At **t=250 ms** the attribute is removed → a **second full-document style recalc and
  restyle**. This tail hitch is the "step-then-jump".

### R2 — `*` does not match pseudo-elements

`#root::after` (`index.css:143-154`) is the full-viewport `position: fixed`,
`z-index: 9999` micro-grain. In light theme it changes `opacity 0.035 → 0.045` **and
gains `mix-blend-mode: multiply`** (`index.css:155-158`). Neither is in the property
list, and neither is matched by `*` → it **pops at t=0** while everything else fades.
The reduced-motion block at `index.css:194-202` correctly uses
`*, *::before, *::after`; the theme block does not.

Also unmatched (own out-of-sync durations): `::-webkit-scrollbar-thumb` 200 ms
(`index.css:348`), range track 200 ms (`:299`), range thumb 150 ms (`:319`),
`.engage-btn-loading::after` conic-gradient (`:438-451`).

### R3 — only 5 properties listed → everything else SNAPS

| snaps | where |
|---|---|
| `background-image` gradients | `.amb-base` `index.css:515-528` (`hsl(240 5% 9%)` → `hsl(40 20% 97%)`), `.amb-glow` `:564-579`, `.orbit-card-surface` `:533/:539`, `PipelineField.tsx:64` (Home), `BottomDockFeather.tsx:21-22`, `CentralClockNode.tsx:147-149`, `Shimmer.tsx:44-45` (`bg-clip-text` gradient glyphs) |
| `box-shadow` | `.glass-card` `index.css:249 → :255`; every `shadow-[0_0_24px_rgba(var(--accent),…)]` — `Home.tsx:345,363,383,400,413,437`, `TopRightCluster.tsx:53,71,95`, `ResponsiveLayout.tsx:399`, `RadialHub.tsx:36`, `GraphControlDock.tsx:31` |
| `opacity` | `.amb-noise` `0.04→0.03` (`index.css:647/651`), `#root::after` (`:151/156`) |
| `border-width` | `.rp-ring` 1px → 1.5px (`index.css:622` → `:637-640`) |

### R4 — forces ~2000 elements to repaint for 150 ms; backdrop-filter re-blurs per frame

When the backdrop animates, **every** `backdrop-filter` region must re-run its blur
kernel **per frame**. Cost scales with paint *area*, not node count.

| page | nodes matched by `[data-theme-transition] *` | `backdrop-filter` regions |
|---|---|---|
| Home idle | ~120-160 | 2 (`EdgeNav.tsx:25`, `ResponsiveLayout.tsx:437`) |
| Home, 10 dialogue turns | ~300-450 | 2-12 (each `DialogueBubble.tsx:43` is `backdrop-blur-xl`) |
| History orbit | ~350-600 | 3 (+ `CentralClockNode.tsx:139`); 12-24 × `VoiceRippleNode` (`orbitMath.ts:92-100`) + 48 SVG ticks (`CentralClockNode.tsx:87-105`) |
| Memory | ~130-150 | 3 — and the graph is **one `<canvas>`** (`MemoryGraph.tsx:296-308`) |
| Settings, 0 cards | ~130-160 | 8 (6 × `RadialHub.tsx:36` `backdrop-blur-sm`) |
| Settings, Appearance open | ~190-215 | 11 (+ `Card.tsx:39` `blur-2xl`) |
| Settings, all 6 cards | ~1200-2000+ | 60-120 |

No virtualization anywhere (no `react-window` / `react-virtuoso`); all multiplication is
raw `Array.map` — `MemorySessionRail.tsx:161,239` are **uncapped**.

### R5 — ambient layers are pure cost during the flip

Present on Home, History **and** Settings; absent on Memory.

- `.amb-base` full-viewport gradient (`:512-529`) — re-rasterized every frame, and it is
  the backdrop for every glass surface.
- `.amb-glow` **60vmax** radial gradient (`:561-568`) with `will-change: opacity` (`:572`).
- 5 × 280 px `.rp-ring` running `ripple-out` for **39.2 s** (Home) / 61.6 s (History),
  `will-change: transform, opacity` (`:625`), staggered `animation-delay`
  (`AmbientBackground.tsx:186`).
- `.amb-noise` full-viewport SVG `feTurbulence` (`:642-652`).
- Home only: `PipelineField.tsx:59-65` — 800 px `blur-[120px)` + `mixBlendMode: "screen"`
  + inline-swapped radial-gradient.
- `AmbientBackground.tsx:109-114` rAF writes inline opacity on two full-viewport
  elements every frame, self-restarting on a 600 ms interval (`:133-138`).

### R6 — no `color-scheme`, no `@property`

- `color-scheme` is **never declared** → UA scrollbars, form controls, default canvas
  never re-theme, and `::-webkit-scrollbar-thumb` keeps its own 200 ms transition.
- `@property` count is **0** → all palette vars are unregistered and flip instantly.
  Only the *consuming* property can interpolate, which is why every gradient snaps.

---

## 3. Why Memory is perfect (the diff)

| | Home | History | Memory |
|---|---|---|---|
| `AmbientBackground` | on | on | **off** (`ResponsiveLayout.tsx:326,346-357`) |
| `PipelineField` | on | – | – |
| full-viewport gradients | 3 | 3 | **0** |
| `mix-blend-mode` surfaces | 2 (`:157`, `PipelineField.tsx:65`) | 1 | **0** |
| infinite full-viewport animations | 5 rings + pulse | 5 rings | **0** |
| heaviest surface | `backdrop-blur-xl` bubbles | 24 orbit nodes | **`<canvas>`** |
| backdrop behind header | animated gradient stack | animated gradient stack | flat `background-color` (`ResponsiveLayout.tsx:339`) — **is** in the property list |

Memory's header/cluster class strings are **byte-identical** to Home's and History's
(`Memory.tsx:328-332` vs `ResponsiveLayout.tsx:397` vs `History.tsx:226-230`). The toggle
is the same component (`shared/ui/ThemeToggleButton.tsx`, uncommitted, mounted at
`ResponsiveLayout.tsx:407`, `History.tsx:235`, `Memory.tsx:337`). **Only the backdrop
differs.**

## 4. Why Settings is slow and steppy

At `HEAD` the `data-theme-transition` work is **uncommitted** (`git status`: `index.css`
+10, `settingsStore.ts` +14, `Card.tsx`). So Settings runs its own declared durations —
an **8-way cascade**:

| ms | where |
|---|---|
| 150 | `.glass` `:232`, `.glass-card` `:250` |
| 200 | 21 × `transition-opacity duration-200`, `animate-fade-in` |
| 300 | `SegmentedControl.tsx:78`, `SubModelCard.tsx:149`, `SettingsTopologyMap.tsx:67`, `ModelsTopologyMap.tsx:84` |
| **400** | `RadialHub.tsx:36,46,126,132,142,152`, `SettingsVisualConnectors.tsx:21,26,38,50,63`, `Card.tsx:34` |
| 500 | `RealtimeVisualElements.tsx:126` |
| 800 / 1500 / 1800 | `.rp-wrapper` `:596` / `.amb-glow` `:570` / `.rp-layer` `:612` |

Plus an explicit per-domain `animation-delay: ${DOMAINS.indexOf(d) * 0.12}s` over a
full-page SVG (`SettingsVisualConnectors.tsx:145`), and infinite animations living
*inside* blur regions (`SettingsCardSkeleton.tsx:24,30-84` — 19 × `animate-pulse` +
`skeleton-shimmer 1.6s`; `HubCenter` `border-rotate 18s` + `reactor-pulse 2.5s`).

Ruled out for Settings: no multiple `applyAppearance` per tick; `DomainContent` is
`memo`'d with primitive props (`Settings.tsx:31,56`) so cards do **not** re-render; the
hub geometry depends only on window size (`useSettingsPage.tsx:74-81`). It is purely a
**paint-cost + duration-mismatch** problem.

Settings also has **no `ThemeToggleButton`** — `ResponsiveLayout.tsx:407` sits inside the
`isHome &&` block opened at `:388`. Its only theme control is `AppearanceCard`'s
`SegmentedControl` (`:78-83`), whose active-state `shadow-[0_0_8px_…]` snaps at t=0 while
its pill background fades → hard glow pop + hard Moon/Sun icon swap over a fading pill.

---

## 5. Industry practice — and why it beats ours

Standard practice (next-themes, shadcn/ui, GitHub, Linear, Vercel) is
`document.startViewTransition()`: snapshot the viewport, run the DOM mutation, snapshot
again, cross-fade two **static images**. Per-frame re-rasterization of live elements
never happens — which is the entire cost model we are currently fighting.

Support matrix vs our three targets:

| target | engine | `startViewTransition` |
|---|---|---|
| Windows | WebView2 / Chromium | ✅ 111+ |
| macOS | WKWebView / Safari | ✅ 18.0+ |
| **Linux (our dev box + Tauri)** | **WebKitGTK** | ✅ **2.46+** same-document; 2.48 added cross-document + improvements |

So VT is available everywhere, but **must** be feature-detected with a CSS fallback.

Secondary benefit: during the cross-fade, `backdrop-filter` is baked into the snapshot
(frozen flat). That removes the WebKitGTK backdrop flashing that `.no-blur`
(`index.css:661-665`, currently toggled only by `OrbitCarousel.tsx:82-94`) exists to
suppress.

Rejected alternative: `@property <color>` interpolation on the palette. Our vars are
comma-separated RGB triplets (`:21-67`, `:78-116`) consumed as `rgb(var(--x) / a)` —
**1893 occurrences across 132 `.tsx` files**. Migration cost is unjustifiable; VT gets
the same result for free.

---

## 6. Remediation plan

Decisions locked with user: **View Transitions API primary + corrected CSS fallback**;
**one 200 ms constant**; scope **includes** `color-scheme` + scrollbar/range stragglers +
perf instrumentation/regression tests.

### B0 — Baseline measurement (no behaviour change)
CDP harness via `.agents/skills/review-ui/scripts`: for Home / History / Memory /
Settings, on one theme flip record `PerformanceObserver` long-task count and rAF frame
deltas; print `document.querySelectorAll("*").length` and the backdrop-filter count
already computed at `services/monitoringService.ts:199,243`. **Every later batch is
judged against this number.**

### B1 — Design tokens + UA surfaces (`app/src/index.css`) — SHIPPED
- `--theme-transition-ms: 200ms`, `--theme-ease: cubic-bezier(0.4, 0, 0.2, 1)` in
  `:root`, plus `color-scheme: dark`; `[data-theme='light'] { color-scheme: light }`.
- Stragglers retargeted to the token: `::-webkit-scrollbar-thumb`, range track,
  range thumb.
- `.glass` / `.glass-card` given an explicit token-based
  `transition-property: background-color, border-color, box-shadow`. They had
  **no** transition of their own at all — the old `!important` override was the
  only thing that ever faded them, so they are now self-sufficient.

> **Deviation from plan.** The plan said to delete the bespoke ambient opacity
> durations (`.amb-glow` 1.5 s, `.rp-wrapper` 0.8 s, `.rp-layer` 1.8 s). That was
> wrong: they are only "dead" *inside* the transition window, and outside it they
> are what eases the ambient field in when the self-stopping rAF loop wakes from
> idle (`AmbientBackground.tsx:117-124`). Deleting them would have made the glow
> and ripples snap on every voice-activity wake. They are left intact; B5's freeze
> removes the reason they were a problem.

### B2 — One theme module (ZBC extraction) — SHIPPED
New `app/src/shared/theme/index.ts`: `applyTheme()`, `supportsViewTransitions()`,
`THEME_TRANSITION_MS`, plus a tiny external store (`subscribeThemeTransition` /
`getThemeTransitioning`) so non-React consumers can park themselves during a flip.
`applyAppearance`, `lastAppliedTheme` and `themeTransitionTimer` are deleted from
`settingsStore.ts`; all 5 call sites route through `applyTheme`.

### B3 — View Transitions API (primary path) — SHIPPED
- Feature-detected; `prefers-reduced-motion` and first paint write instantly.
- `document.activeViewTransition?.skipTransition()` before starting, so a rapid
  second toggle never queues behind the in-flight capture.
- Only `writeThemeToDom` runs inside the callback. Store updates, `localStorage`
  and persistence stay outside; `localStorage` is written again on
  `finished.finally` so the boot script never records an aborted flip.
- `::view-transition-old/new(root)` get the token duration and
  `mix-blend-mode: normal` (the UA default `plus-lighter` flashes white at the
  midpoint of a light↔dark cross-fade).
- Nothing carries a `view-transition-name`, so `root` captures the whole viewport.

### B4 — Corrected CSS fallback — SHIPPED
- `transition-property` / `-duration` / `-timing-function` / `-delay` **longhands**;
  the shorthand is gone from the gate entirely.
- Selector gained `*::before, *::after`.
- Property list completed with `box-shadow`, `text-shadow`, `outline-color`,
  `opacity`, `border-width`, `fill-opacity`, `stroke-opacity`.
- Gate lifetime is `THEME_TRANSITION_MS + 32ms` rather than a hardcoded 250 ms.

### B5 — Freeze ambient during the flip — SHIPPED
- CSS: `[data-theme-transition] .rp-ring, [data-theme-transition] .amb-glow` get
  `animation-play-state: paused`.
- `AmbientBackground` reads `getThemeTransitioning()` through a **ref**, not an
  effect dependency, so `paused` flipping mid-flight does not tear down and
  recreate the effect. `smoothedEnergy` and `isSettled` survive the freeze, so the
  loop resumes mid-interpolation with no snap. The existing `paused` prop was
  folded into the same guard rather than early-returning, which previously left
  the rAF running.

### B6 — Regression tests + instrumentation — SHIPPED
`app/src/test/invariants.test.ts` gained a `Theme transition contract` block
(Invariants 9-13): no shorthand in the gate; pseudo-elements present; CSS and TS
durations agree; no component writes `data-theme*` outside `shared/theme`;
`color-scheme` declared for both themes. `sampleThemeTransition()` in
`monitoringService.ts` reports which mechanism the engine takes, the resolved
duration token, reduced-motion, and accumulated long tasks.

### Incidental fixes required to reach a green suite
Both were failing at `HEAD` before any of the above, from the uncommitted work in
`96d9058b`:

- **Invariant 8** — `(window as any).requestIdleCallback(...)` in
  `ResponsiveLayout.tsx:29` replaced with a typed
  `Window & { requestIdleCallback?: ... }` narrowing.
- **Invariant 6** — `data/settingsCopy.ts` hardcoded six TTS provider ids
  (`zipvoice`, `kokoro`, `edge_tts`, `supertonic`, `chatterbox`,
  `chatterbox_remote`) as the `tts` dirty-key list. Fixed by **deleting** the
  entry rather than allowlisting it: all four consumers already implement a generic
  whole-scope path, and `isDomainRequiringRestart` uses
  `rule.keys || Object.keys(draftScope)` filtered by `isRestartKey`. Omitting the
  list is therefore strictly *more* coverage than the allowlist — a newly added
  backend TTS provider is now picked up with no frontend edit.

### Verification
`npx vitest run` → 13/13 pass. `tsc --noEmit` → clean. `vite build` → succeeds, and
the built CSS confirms `::view-transition-old(root){…mix-blend-mode:normal}`, the
longhand-only gate with `:before`/`:after`, both `color-scheme` values, and the
token on `.glass` / scrollbar / range.

---

## 7. Deferred (raised, out of agreed scope)

| # | Item | Location |
|---|---|---|
| D1 | Discard on a Settings card **reverts the theme** (second full fade to the old value) when another domain is dirty | `settingsStore.ts:835` (`discardDomainChanges`), also `:1018` |
| D2 | Settings header has **no** `ThemeToggleButton`; only the `AppearanceCard` segmented control | `ResponsiveLayout.tsx:407` is `isHome`-only |
| D3 | Light-theme `--accent` is **dead**: `settingsStore.ts:423` writes `--accent` as an inline style from `accent_seed`, which always beats `[data-theme='light'] { --accent: 14,116,144 }` at `index.css:81`. Light mode never uses its intended teal | `settingsStore.ts:423`, `index.css:81` |
| D4 | 146 `duration-*` Tailwind overrides across `app/src` collapse to 150 ms inside the window then restore → two-speed feel even under the corrected fallback | repo-wide |
| D5 | `VoxOrb`'s `MutationObserver` (`attributeFilter: ["data-theme","style"]`) forces a full-document `getComputedStyle` on the flip frame | `AdvancedOrb.tsx:399,416-419` |
| D6 | Hardcoded status hexes never adapt | `SettingsTabPane.tsx:271,272,354` |
| D7 | Uncapped `Array.map` in `MemorySessionRail.tsx:161,239` | node-count ceiling |

---

## 8. Evidence index

| Claim | Source |
|---|---|
| mechanism | `app/src/store/settingsStore.ts:405-444`, `app/src/index.css:185-191` |
| ambient gating that explains Memory | `app/src/layout/ResponsiveLayout.tsx:326,346-357` |
| gradient/opacity snaps | `app/src/index.css:143-158,512-529,556-580,642-652` |
| glass + backdrop cost | `app/src/index.css:223-237,240-256`, `app/src/shared/ui/Card.tsx:15,34,39` |
| WebKitGTK blur flashing workaround | `app/src/index.css:661-665`, `OrbitCarousel.tsx:82-94` |
| settings duration cascade | `RadialHub.tsx:36-152`, `SettingsVisualConnectors.tsx:21-63,145`, `Card.tsx:34`, `RealtimeVisualElements.tsx:126` |
| boot fast path | `app/index.html:9-40` |
| VT support | caniuse `view-transitions` (Chrome 111+, Safari 18.0+, Firefox 144+); webkitgtk.org 2.48 release notes |
| `@property` cost of migration | 1893 `rgb(var(--` across 132 `.tsx` files in `app/src` |