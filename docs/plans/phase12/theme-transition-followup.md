# Theme Transition — Follow-up: Uniform Speed & Scale (2026-10-03)

Follow-up to `theme-transition-rca-and-remediation.md`. That document fixed the
*mechanism* (one gate, one 200 ms token, no View Transitions API). This one fixes
the two things still wrong:

1. **Speed is not visually uniform** across surfaces — the title bar and the
   top-right cluster still change at a different rate than the rest of the app.
2. **Cost scales with page complexity** — Settings gets slower the more cards are
   open, because blur cost scales with open cards.

---

## 1. Answering "why is Memory clean? is its duration different?"

**The duration is identical on every page, and provably so.** There is exactly one
gate — `[data-theme-transition]` on `<html>` (`index.css:236`) — one
`--theme-transition-ms: 200ms` token, and **zero** per-page durations anywhere in
`app/src`. The gate is on the document root, so every element on every route is
forced to `transition-duration: 200ms` with the same timing function.

Memory is not faster. Memory has **nothing that snaps**.

| | Memory | Home / History / Settings |
|---|---|---|
| `AmbientBackground` | **disabled** (`ResponsiveLayout.tsx:342`) | mounted |
| What sits behind the header | `html/body` `background-color` | `.amb-base` full-viewport `radial-gradient` |
| That property's transition | `background-color` — **in** the gate's list → fades | `background-image` — **not in** the gate's list → **snaps at t=0** |

So on every non-Memory route the page's own backdrop changes instantly while all
content fades over 200 ms. Everything else in this document is a consequence of
that one fact.

### 1.1 The title bar ("decorations")

`TitleBar.tsx:141` is `bg-transparent border-transparent`. The bar has **no
background of its own** — its visible colour is 100% whatever is behind it, which
on Home/History/Settings is `.amb-base`'s gradient (`index.css:513-527`,
`hsl(240 5% 9%)` → `hsl(40 20% 97%)`). Gradient snaps; the bar's text and border
fade. That reads as "the header changes at a different speed."

### 1.2 The top-right cluster

`TopRightCluster.tsx:54,74,98` render `bg-[rgba(var(--card),0.5)]` — **50% alpha**.
The snapping gradient shows straight through them, so they *look* like they snap
even though their fill is genuinely fading. Same for
`ResponsiveLayout.tsx:400` and `History.tsx:228`. Every translucent surface above
the ambient field inherits its snap.

### 1.3 Settings with all cards open

Two independent costs, both scaling with open cards:

- **Blur.** Each `backdrop-filter` region must re-run its blur kernel on every
  frame of the fade, because its backdrop is animating. With all six cards open
  that is 60-120 regions; Memory has 3. This is the dominant cost.
- **Fan-out.** 1200-2000 nodes vs Memory's ~130, all restyled twice (gate opens,
  gate closes).

---

## 2. Root cause, stated once

**The ambient field's `background-image` and `opacity` are not in the gate's
property list, so they snap at t=0 while everything else fades over 200 ms — and
because every translucent surface and the title bar are transparent, that snap is
visible through the entire UI.**

Two secondary causes:

- The gate forces `transition-property` on every element, wiping `.glass` /
  `.glass-card`'s own `box-shadow` leg. Same class of defect as the original
  `transition` shorthand, narrowed to one property.
- Live `backdrop-filter` over an animating backdrop is the Settings scaling term.

### Why the fix must be applied as a package

T2 (drop the blur) and T3 (fade the gradient) are **not independent**. T3 makes a
full-viewport gradient repaint every frame; that is only affordable because T2
removed every `backdrop-filter` region that would otherwise have to re-blur it.
**T3 without T2 would be the single most expensive change in this plan.**

---

## 3. The plan

Decisions locked with user: drop blur for the flip window first, with two
documented fallbacks; fade `.amb-base`; keep the title bar transparent; stop the
gate clobbering glass.

### T1 — Gate stops clobbering declared `transition-property`
Selector becomes `*:not(.glass):not(.glass-card)`. Those two keep their own
`transition-property: background-color, border-color, box-shadow` on the token, so
their shadow fades instead of snapping. Everything else keeps the forced list, so
plain `div`s with no transition utility still fade.

The 232 `transition-all` sites are safe to clobber: during a flip nothing on them
actually changes `opacity`/`transform`/`box-shadow`. `opacity-70` on the cluster
(`TopRightCluster.tsx:73,97`) is a panel state, not a theme value.

### T2 — Suspend `backdrop-filter` for the flip window
```css
[data-theme-transition] .glass,
[data-theme-transition] .glass-card,
[data-theme-transition] [class*="backdrop-blur"],
[data-theme-transition] [style*="backdrop-filter"] {
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}
```
Unlayered, so it beats `@layer components` and Tailwind's `@layer utilities`
`backdrop-blur-*` — no `!important` required. The `[class*=…]` / `[style*=…]`
substring selectors are the idiom already used at `monitoringService.ts:244`.

**Fallback ladder, per user decision — try in order, revert if all read wrong:**

| Variant | Rule | Cost | Reads as |
|---|---|---|---|
| **1 (ship first)** | drop blur everywhere | 0 | glass flat for 200 ms |
| 2 | drop only on small surfaces; keep it on `.glass-card` | a few large blurs | large panels stay frosted, small chips go flat |
| 3 | revert T2 entirely | full | current behaviour — Settings scales with open cards |

Variant 2 selector:
```css
[data-theme-transition] [class*="backdrop-blur"]:not(.glass-card),
[data-theme-transition] [style*="backdrop-filter"],
[data-theme-transition] .glass { backdrop-filter: none; }
```

`.no-blur` (`index.css:659`) exists precisely because animating `backdrop-filter`
makes WebKitGTK flash, which is evidence that removing it is the lesser evil on
this engine.

### T3 — Make the ambient field fade — *this is the title-bar and cluster fix*
```css
[data-theme-transition] .amb-base,
[data-theme-transition] .amb-glow { transition-property: background-image; }
[data-theme-transition] .amb-noise,
[data-theme-transition] #root::after { transition-property: opacity; }
```
`.amb-base`'s dark and light gradients have identical structure, so they are
interpolable. The title bar stays `bg-transparent`; the gradient it shows through
now interpolates over the same 200 ms, so the header changes at the same speed as
everything else and every translucent surface above it stops looking like it snaps.

No per-surface shadow work is planned for the cluster (an earlier draft had it) —
T3 removes the cause. Revisit only if something still looks off afterwards.

`.amb-glow`'s opacity is written by the ambient rAF loop, which is frozen for the
duration of the flip, so there is no conflict with the background-image fade.

### T4 — Measurement on the real engine
`.agents/skills/review-ui/scripts/*.mjs` drive **headless Chrome against
`localhost:1420`** — a different engine and rasterization path from the WebKitGTK
2.52 runtime this app actually ships on. That is why a 1-second stall shipped
undetected.

New `.agents/skills/review-ui/scripts/measure_theme_flip.mjs` attaches to the
WebKitGTK remote inspector (`WEBKIT_INSPECTOR_SERVER=127.0.0.1:9222 pnpm tauri
dev`), clicks the real theme control, and reports per route:

- `timeToFirstChangedFrameMs` — click → first rAF whose paint reflects the flip
- frame deltas across the 200 ms window
- `longtask` count and total blocking time
- `nodeCount` and `blurRegionCount`

Gate: **total blocking time must not scale with the number of open cards.**
Output to `sandbox/results/theme_flip_<route>.json`.

### T5 — Invariants (`test/invariants.test.ts`)
- **16** the flip window must declare `backdrop-filter: none`
- **17** `.amb-base`, `.amb-glow`, `.amb-noise`, `#root::after` each declare a
  theme-window transition
- **18** the gate must contain `:not(.glass)` — guards T1
- **14** (no layout-triggering property in the gate) and **15** (no
  `startViewTransition`) already added and retained

---

## 4. Known residue

- `#root::after` gains `mix-blend-mode: multiply` in light theme
  (`index.css:157`). `mix-blend-mode` is non-animatable, so a one-frame
  full-viewport blend-layer creation remains. If it flashes, the fix is to drop
  that blend mode, not to animate it.
- `.rp-ring` `border-width` 1px → 1.5px in light (`index.css:635-638`) still
  snaps. Five 280 px rings, already paused during the flip — not worth a
  layout-triggering property in the gate to fix.
- `EdgeNav.tsx`, `EdgePanel.tsx` and `BottomDockFeather.tsx` use gradient
  `mask-image`, which is not transitionable at all. Their gradient *stops* are
  var-driven, so they follow `--card`; only the mask is fixed.
