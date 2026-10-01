# Frontend UX Audit — Memory Page & Component Surface

---
title: "Frontend UX Audit — Memory Page & Component Surface"
audience: "Internal — Frontend Engineer / Design Reviewer"
last_updated: 2026-09-30
owners: "frontend-engineer role"
related_docs:
  - "docs/features/performance-memory-optimizations.md — prior perf ledger this audit partially falsifies"
  - "docs/plans/phase12/frontend-ux-audit-appwide.md — companion app-wide audit"
  - ".agents/rules/frontend-style-guide.md — perf + design invariants checked against"
  - ".agents/skills/review-ui/ — deterministic audit scripts"
---

## Correction (2026-09-30, post-implementation)

> **C3 ("no keyboard route to the personal-memory drawer") was wrong.** `Shift+Up`
> already opens the page drawer (`ResponsiveLayout.tsx:265-270` →
> `Memory.tsx:124 useRegisterPageDrawer`), so no new shortcut was added. The
> narrower canvas-surface annotation (`role="img"` + summary label on
> `MemoryGraph.tsx`) shipped instead. Heuristic #7, P2-6, the persona findings
> citing "mouse-only", and tier-table row 32 should be read with this correction.

## How to read this doc

- **Audience:** Frontend engineer or design reviewer picking up the fix list.
- **Scope:** `app/src/pages/Memory.tsx` (1182 lines), all 14 components under
  `app/src/shared/components/memory/`, plus the navigation shell (`App.tsx`,
  `layout/ResponsiveLayout.tsx`, `layout/EdgeNav.tsx`) and `app/src/data/memoryCopy.ts`.
- **Convention:** Every material claim cites `file:line`. Numbers come from the
  deterministic scans or from arithmetic shown inline. Unverifiable claims are marked
  `unverified` rather than asserted.
- **Non-goals:** No remediation code. No spec changes. This doc records findings and
  classifies them; it does not author the fix.
- **SSOT:** `.agents/rules/frontend-style-guide.md` for invariants,
  `docs/features/performance-memory-optimizations.md` for the perf ledger this
  audit corrects.

---

## 1. Method & Calibration

**Method:** dual-agent, isolated.
- **Assessment A** (`ses_f0d13f69effe6CaX2bTJpqTbIP`) — unanchored design review.
  Design-director judgment. No detector access.
- **Assessment B** (`ses_f0d0a9fe6ffeS0EBG1A9rjiTV5`) — deterministic + browser
  evidence. `impeccable detect` CLI, headless Chrome over CDP, and source-verified
  censuses. No design judgment.

Assessment B's output did not enter the parent synthesis context before Assessment A
completed.

**Calibration:** production-scale. Linux-first, CPU-constrained desktop app
(Tier 1A = 8GB, no GPU, llvmpipe software rasterisation per
`.agents/rules/frontend-style-guide.md:18`). Professional tool used all day, not a
prototype.

**Mode:** Operate — task completion, scanability, and native expectations outrank
expression.

**Working-tree caveat at audit time:** 19 files were uncommitted. Three
(`app/src-tauri/src/ipc/memory.rs`, `app/src/services/memoryService.ts`,
`app/src/shared/hooks/useObservationsList.ts`) changed *during* the audit by
something other than these agents. Citations into those three files are post-change.
**Any fix list must be re-verified against current source before implementation.**

### 1.1 Deterministic gates run

| Gate | Result |
|---|---|
| `node .agents/skills/review-ui/scripts/check_invariants.mjs` | **5/5 PASSED** |
| `impeccable detect --json app/src/pages/Memory.tsx` | exit 0 — **0 findings** |
| `impeccable detect --json app/src/shared/components/memory` | exit 0 — **0 findings** |
| `impeccable detect --json app/src/App.tsx app/src/layout` | exit 0 — **0 findings** |
| `impeccable detect --json app/src` (control) | 6 findings, all `side-tab` in `help/*` + `Markdown.tsx` |

The zero-results were verified as a real measurement, not suppression: no
`.impeccable/` dir existed, `impeccable ignores list` returned empty for rules /
files / values, no `impeccable-disable*` comments in scope, and a `--no-config`
re-run of the first invocation also returned `[]`.

> **Finding 0 (meta).** Both automated gates are green and the surface is still
> shipping a silent DB write, a missing error state, and an unreachable loader. The
> detector's rule vocabulary is visual (colour, contrast, padding, gradient, motion);
> it has no rule for state-machine or data-integrity defects. `check_invariants.mjs`
> covers lifecycle/architecture but not UX states. **Do not treat a green checkmark as
> evidence of correctness.**

---

## 2. Design Health Score

Method: `/impeccable critique` rubric, 10 Nielsen heuristics scored 0-4.
All ten apply to an Operate-mode desktop tool; no heuristic marked `n/a`.

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 2/4 | Loader has an enter but no exit, no `aria-live` — `Memory.tsx:861` |
| 2 | Match System / Real World | 2/4 | "In-place editor" is a whole-document textarea — `RawEditorView.tsx:20` vs `ActionHubView.tsx:71` |
| 3 | User Control and Freedom | 1/4 | `aria-modal="true"` at `Drawer.tsx:195`, zero focus trap in 266 lines |
| 4 | Consistency and Standards | 1/4 | `role="combobox"` on wrapper div, `aria-expanded`/`controls` on inner input — `SearchBar.tsx:81` vs `:120-123` |
| 5 | Error Prevention | 1/4 | Filter change mid-fetch silently dropped; old rows render under new label — `useObservationsList.ts:36` |
| 6 | Recognition Rather Than Recall | 2/4 | Node picking off by default, toggle named only in tooltip — `MemoryGraph.tsx:202-204` |
| 7 | Flexibility and Efficiency | 2/4 | Ctrl+K/R/± exist; the primary action has no key — `MemoryGraph.tsx:254-262` |
| 8 | Aesthetic and Minimalist Design | 2/4 | Two `AmbientBackground` at two ripple origins; 13 controls compete with drawer open |
| 9 | Recognize / Diagnose / Recover from Errors | 0/4 | Only affordance is `console.error`; user told "No memories saved yet" — `Memory.tsx:205`, `:872` |
| 10 | Help and Documentation | 3/4 | `MEMORY_COPY` is 174 lines and explains *why*; disabled reasons honest (`SuggestionsReviewView.tsx:342`) |
| **Total** | | **16/40** | **Poor — major UX work required** |

**Band:** 16/40 = Poor (12-19 band: "Major UX overhaul required; core experience broken").

---

## 3. Design Specificity

**Authored, not generic — proven by two expensive-to-fake details.**

1. The memory taxonomy at `memoryGraphTypes.ts:11` —
   `personal | objective | workdone | blocker | next_step | pitfall` — is the schema
   of a dictation agent's episodic→semantic consolidation. A graph-explorer demo
   ships `CategoryA/B/C`; an analytics tool ships `Users/Events/Sessions`. "Blocker"
   and "Pitfall" as first-class node kinds with dedicated hues is a model of how a
   voice assistant thinks about what the user told it.
2. `dynamicGraphPalette.ts:67-112` derives the entire six-colour semantic palette from
   the user's chosen `--accent` via a documented harmonic hue-offset scheme with
   per-theme lightness. Change the accent in Appearance and the WebGL node colours,
   legend dots, rail chips, and tooltip swatches all re-tint from one source of truth.
   The doc comment's stated degrees (`:79-84`) match the code's offsets (`:86-90`) —
   rare. **This is the strongest single design decision in the surface.**

**Where it turns generic.** The "Liquid Space" ambient language is authored at app
level (`ResponsiveLayout.tsx:327`) then duplicated unmodified on the page
(`Memory.tsx:751`) at a *different* ripple origin — `calc(50% - 36px)` vs `50%`.
The one genuinely page-specific ambient expression — the `PixelSynthesisCanvas` dot
matrix — is also the page's heaviest sustained cost (~1,548 unbatched path fills per
frame). **The inversion of that single detail is the most damning
design/engineering seam on the page: the most authored moment is the most expensive
one, and the least authored is the most expensive by volume.**

**Detector verdict.** 0 findings on the target surface. 8 findings when scanning the
rendered DOM (bypassing the CSS-text false positives listed in §8). The detector is
functional and non-suppressed; it is simply blind to this class of defect.

---

## 4. What's Working

**S1 — Accent-derived semantic palette.** `dynamicGraphPalette.ts:67-112`. See §3.

**S2 — `SuggestionsReviewView` is a model diff-review UI.** `:162-215` renders
word-level LCS tokens as semantic `<del>`/`<ins>`, re-colours by accept/reject
decision, pairs each entry with `aria-pressed` toggles (`:57-90`), explains *why*
apply is disabled instead of muting it (`:342`), states the exact payload it will
send (`:343`), tells the user undecided items stay pending (`:59`, `:323`), and
surfaces per-revision failure with targeted retry (`:91-97`, `:348-357`). Eight
"basics" done correctly in one component. **This is the standard the rest of the page
should be held to.**

**S3 — Real progressive disclosure in the observations feed.**
`LearnedFactsList.tsx:119-132` container-measured infinite scroll (correctly noting
the feed lives in a drawer, not the viewport), `:220-237` three distinct
loading / loading-more / all-loaded states, `:191-201` per-filter empty copy.

**S4 — Global focus-visible is real.** `index.css:130-140` applies a 2px accent
outline to `button`, `input`, `textarea`, `[tabindex]`,
`[role=button|switch|radio|checkbox]`, correctly gated behind `:focus-visible` with
`*:focus:not(:focus-visible){outline:none}` at `:143-145`.

---

## 5. Priority Issues

### P0-1 — No error state. Failure is rendered as emptiness. Three times.

**Where.** `Memory.tsx:204-209` catches, `console.error`s, falls through to
`finally { setLoading(false) }` → `:872` renders `MEMORY_COPY.emptyFactsTitle` /
`emptyFactsDesc`. Same shape at `MemorySessionRail.tsx:64-69` → `:326-334`
(`noSessions`) and `useObservationsList.ts:67-72` → `LearnedFactsList.tsx:185-202`
(`observationEmptyAll`).

**Why it matters.** `emptyFactsDesc` reads *"Talk with Vox to help it remember
details about you and your work."* A user with a locked Turso file, corrupted DB, or
failed IPC is told the **opposite** of the truth — that they have no memories, and
that the fix is to talk more. They will talk more. It will not help. Direct violation
of `.agents/rules/frontend-engineer.md:26` ("Never assume an IPC call succeeds.
Loading state, error state, and backend-not-ready state are not optional edge cases").

**Fix.** Add `loadError: string | null`; set it in all three catch blocks; branch the
render to `loadError ? <ErrorCard detail onRetry> : !loading && facts.length === 0 ?
<EmptyState> : null`. Stop hand-rolling the card — `EmptyState` already exists with
`elevation="surface"` and an `action` slot (`EmptyState.tsx:9,22,41`) that
`Memory.tsx:874` bypasses. Add `loadFailedTitle` / `loadFailedRetry` to
`memoryCopy.ts`.

**Command:** `/impeccable harden`

---

### P0-2 — Browsing a revision silently overwrites the active version, 500 ms later.

**Where.** `PersonalMemoryVersionNav.tsx:44-62`. `navigateToRecord` calls
`onSelectVersion` (pure preview) at `:47`, then sets a 500 ms debounce at `:55-59`
that fires `onCommitActiveVersion(targetRecord.version)` → `Memory.tsx:321-338` →
`setActivePersonalMemoryVersion(version)` → **DB write**, plus four more IPC calls
via `refresh(true)` at `:330`.

Independently re-verified line-by-line by the parent agent against source.

**Why it matters.** The affordance is a chevron pair labelled "Previous revision" /
"Next revision" — a *browse* gesture performing a *destructive write*. A
double-click, a stray keypress, or a user who wants to see what changed last Tuesday
silently promotes that revision to canonical. Asymmetry: every other destructive path
is gated — `PendingConfirmationBanner` interrupts consolidation, `disabled` guards
Consolidate (`Memory.tsx:967`). This is the only ungated write, and the one a user
triggers by *looking*.

**Fix.** Delete the debounce at `:55-59`; make navigation preview-only. The copy for
the correct UI **already exists and is unused**: `MEMORY_COPY.versions.viewingHistorical`
/ `.activeBadge` / `.historicalBadge`. Render an inline bar in the dossier header row
(`Memory.tsx:1044-1064`) when `displayedRecord.version !== activeVersionRecord?.version`,
with an explicit Restore button bound to `handleRestoreActive`.

**Command:** `/impeccable harden`

---

### P1-1 — Route transition is a hard unmount/mount swap. The loader contract is unreachable.

**Where.** Three independent confirmations:
- `App.tsx:21-25` declares `History`/`Memory`/`Settings`/`Monitoring` as `React.lazy`.
  `App.tsx:66-70` — under the comment `// Preload secondary page route chunks in the
  background` — immediately `import()`s **the same four modules**. By click time,
  `ResponsiveLayout`'s `<Suspense fallback={<OrbitalLoader/>}>` (`:349-358`) has nothing
  left to suspend on. The `// Lazy load secondary pages for performance` comment
  describes the opposite of what the code does.
- `<Routes>` (`App.tsx:162`) is **not** wrapped in `AnimatePresence` and carries **no
  `location` prop**. App-wide: 34 `AnimatePresence` usages, none on `<Routes>`.
  **Zero exit animation on any route, anywhere in the app.**
- `EdgeNav.tsx:18` sets `navigatingTo` in the click handler; `:21-23` clears it in a
  `useEffect` on `location.pathname` — an effect that flushes in the same commit. For a
  preloaded route that is **one frame**, so the three decorative spinner elements at
  `:70-76` never get a paint.

**Why it matters.** The headline complaint. The app does not acknowledge the click and
does not acknowledge the departure. Plus a full WebGL context create per mount
(`useMemoryGraphScene.ts:890`) and a four-call IPC refresh (`Memory.tsx:190-195`), the
transition is a 300-600 ms freeze framed as a cut.

**Fix.**
1. **Delete `App.tsx:66-70`.** The preloads pull four route chunks into the *initial*
   load path, so the user pays for Memory/Settings/Monitoring before needing them. Keep
   the lazy boundaries; preload on *intent* via `onPointerEnter` on the `NavLink`
   (`EdgeNav.tsx:46-56`). This alone restores a real cold-click first paint **and** makes
   the existing Suspense fallback meaningful.
2. **Give `<Routes>` an exit.** Track a `displayLocation` updated after the outgoing
   exit, pass `location={displayLocation}`, wrap in `<AnimatePresence mode="wait"
   initial={false}>`, make each page a `motion.div` keyed on pathname. 160 ms out /
   200 ms in — under the 250 ms threshold where a transition reads as lag.
3. **Delete `navigatingTo` and `EdgeNav.tsx:70-76`**, or repurpose as a real
   minimum-dwell (≥400 ms via `performance.now()`).

**Command:** `/impeccable animate`, then `/impeccable polish` on `EdgeNav`

---

### P1-2 — "Heavy" is four compounding sources of sustained cost, all measured.

**(a) `PixelSynthesisCanvas` — ~1,548 unbatched path fills per frame.**
`PixelSynthesisCanvas.tsx:57` `DOT_SPACING = 15`; `:117-120` `beginPath()` / `arc()` /
template-string `fillStyle` / `fill()` **inside** the double loop; `:124` raw
`requestAnimationFrame`, **not** the `useDynamicFPS` hook that exists at
`shared/hooks/useDynamicFPS.ts:33` and is used by exactly two other components.
Arithmetic for a 520×620 CSS-px card:
`cols = ⌈520/15⌉+1 = 36`, `rows = ⌈620/15⌉+1 = 43` → **1,548 dots/frame →
6,192 canvas API calls/frame → 92,880 `fill()`/s** at 60 Hz. Runs the full 900 ms commit
flash, on llvmpipe, at maximum user attention. *Arithmetic from source, not measured —
the canvas was not mounted in the browser probe.*

Fix: bucket dots into ~6 alpha bands, `beginPath()` once per band, precompute the 6
`rgba()` strings when `accentRgb` refreshes (`:67-69`), hoist `contourWobble` out of the
inner loop, drive from `useDynamicFPS`. **~86k → ~6 path ops/frame.**

**(b) The graph never rests, and a resting cursor keeps it awake.**
`useMemoryGraphScene.ts:1101` suspends only after **4,000 ms** idle; `:1098` counts as
"moving" anything within **300 ms**; `:1083-1086` binds `wakeLoop` to
`pointermove`/`pointerdown`/`wheel`/`touchstart`. **Any cursor resting on the canvas
holds the loop at the 30 FPS rest rate (`:1112`, `minInterval = isMoving ? 16 : 32`)
indefinitely.** In the 300 ms→4,000 ms window, core pulse, core-wireframe rotation,
nucleus pulse, and both counter-rotating rings (`:1119-1139`) run **unconditionally —
no settled flag exists**. `:890` is `antialias: true`; `:892` is
`setPixelRatio(min(dpr, 1.5))` — MSAA plus 2.25× pixels on a software rasteriser.
Browser-confirmed backing store: **2160×1313 at client 1440×876**, exactly 1.5×.
`powerPreference` set on neither renderer (0 occurrences app-wide).

**(c) Two ambient fields, two ripple centres, one trace key.**
Confirmed in source and in the live DOM: `ResponsiveLayout.tsx:327` (`originY =
calc(50% - 36px)`, running) + `Memory.tsx:751` (`originY = 50%`, frozen). Live count on
`/memory`: **2 containers, 4 `.amb-blob`, 2 `.amb-glow`, 10 `.rp-ring`, 2 `.amb-noise`.**
`AmbientBackground.tsx:52` calls `useMemoryTrace("AmbientBackground (rAF Dynamic Glow)")`
with a constant string; `MemoryProfilerContext.tsx:47,55` keys `tracesRef` by that
string — both instances share one identity, so the profiler reports 2 active under one
name and decrements to 1 on the first unmount. **The tool you would reach for to
diagnose "heavy" is confused about the most-suspected component on the page.**
Note: `paused` *does* correctly short-circuit the rAF loop (`:77-79`), so CPU is
spared — GPU overdraw is not.

**(d) 10 backdrop blurs, 0 mitigations.** Census across `Memory.tsx` +
`components/memory/`:

| File:line | Token |
|---|---|
| `Memory.tsx:874` | `backdrop-blur-xl` |
| `Memory.tsx:1001` | `backdrop-blur-sm` |
| `Memory.tsx:1011` | `backdrop-blur-md` |
| `GraphControlDock.tsx:31` | `backdrop-blur-2xl` |
| `SearchBar.tsx:146` | `backdrop-blur-2xl` |
| `MemoryNodeTooltip.tsx:51` | `backdrop-blur-2xl` |
| `MemorySessionRail.tsx:207` | `backdrop-blur-md` |
| `staging/StagingHeader.tsx:160` | `backdrop-blur-md` |
| `MemoryGraph.tsx:59` | `backdrop-blur-[20px]` |
| `PersonalMemoryStagingCard.tsx:264` | `backdrop-blur-sm` |

`index.css:736-740` defines `.no-blur` explicitly as the *"WebKitGTK transition
workaround: disable backdrop-filter during animation to prevent flashing"* and has
**2 real usages app-wide** (`OrbitCarousel.tsx:80,82`) against 44 `backdrop-blur`
occurrences. **Target surface: 10 / 0.** `PersonalMemoryStagingCard.tsx:263` puts
`transition-all duration-500` on a `glass-card … backdrop-blur-sm` element — precisely
the flashing case the workaround was written for. Browser: 4 elements with a live
`backdrop-filter` at rest.

**Command:** `/impeccable optimize` for (a)/(b); `/review` for renderer settings.

---

### P1-3 — Commit choreography: three uncoordinated clocks racing.

**Where.** Click Save & Commit and simultaneously: the right panel drops to
`opacity-50 pointer-events-none` over `transition-all duration-500`
(`PersonalMemoryStagingCard.tsx:268`); the left dossier is veiled by the dot matrix
fading in over 350 ms (`Memory.tsx:1010`); the content swaps **instantly** at `:352`.

The comment at `Memory.tsx:350` claims *"Wait until overlay is opaque (250ms)"* — the
overlay transition is `duration: 0.35` (`:1010`) interpolating from `initial={{opacity:0}}`
(`:1007`), so at t=0.25 s opacity is **≈0.71, not 1.0**. **The comment is factually
wrong.**

Then 900 ms of greyed-out dead UI during which `CommitSuccessView`'s CTA is unclickable,
because `isCommitting` gates `pointer-events-none` at the same moment it gates the
success view. `Memory.tsx` has **8 `setTimeout` and 0 `clearTimeout`** (`rg` exit 1).
L351/L393 are *uncancellable awaited* sleeps. L358=300, L363=900, L411=900, L464=900,
L485=2000, L635=700 ms.

**Why it matters.** This is the "abrupt" that was felt. ~100 ms of half-transparent
mosaic over half-transparent new document, both moving.

**Fix.** Drive the swap from framer's `onAnimationComplete` instead of a timer; move
`opacity-50 pointer-events-none` off the outer container (`:261`) onto the review-body
branch only so the success view and confirmation banner stay live; track the remaining
timers in a ref set cleared on unmount.

**Command:** `/impeccable animate`

---

### P2 — Missing basic UX details (exhaustive census)

| Missing | Evidence | Consequence |
|---|---|---|
| Error state | `aria-live` / `role="status"` / `role="alert"` = **0 occurrences** | Never told the loader finished, suggestions arrived, Apply succeeded, or a version was restored |
| Focus trap on a modal | `Drawer.tsx:195` `aria-modal="true"`; zero focus trap in 266 lines; `tabIndex={-1}` on container only | Tab escapes the drawer; on close `previouslyFocusedRef?.focus()` restores to a `<div>` that never had focus → focus lands on `<body>`. Lost every close |
| Reduced motion | `MotionConfig` = **0** app-wide; `useReducedMotion` = **0** app-wide. `prefers-reduced-motion` exists only as CSS (`index.css:149,728,823`) + one `matchMedia` (`OrbitCarousel.tsx:288`) | 380 ms drawer slide, 350 ms veil, 150 ms tooltip, perpetual 3D loop, dot matrix, 800 ms Lenis inertia all run at full speed. CSS layer gives a false impression of coverage |
| Keyboard route to primary action | `MemoryGraph.tsx:254-262` — orb is a `<div>`, no `tabIndex`, `role`, `aria-label`, or key handler; hint at `Memory.tsx:888-893` is `pointer-events-none` | Entire feature is mouse-only. Ctrl+K/R/± exist; nothing opens the document |
| Accessible graph | No `role`, `aria-label`, or `aria-describedby` on the canvas container; counts exist only as hover tooltips (`MemoryLegendOverlay.tsx:61-65`) | Screen-reader user has no path to the data except drilling the session rail |
| Page title | `document.title` writes = **0** app-wide; `setTitle` = **0** | Every window shows "Vox \| Ambient Intelligence" |
| Exit animations | `MemoryNodeTooltip.tsx:27` returns `null` *before* `<AnimatePresence>` at `:40` → `exit` at `:49` unreachable | The most-used overlay pops in at 150 ms and vanishes instantly. Every other overlay has a real exit |
| Legal combobox | `role="combobox"` on wrapper (`SearchBar.tsx:81`); `aria-expanded`/`controls`/`activedescendant` on inner input (`:120-123`) | Announced as a broken widget |
| Counting honesty | `LearnedFactsList.tsx:170` shows `observations.length` = loaded page (25), not total | 400 observations display as "3 observations" |
| Scroll reset | `MemorySessionRail.tsx:318` — none on filter change | Filter from position 40, land mid-list |
| Undead affordance | `GraphControlDock.tsx:12` `onFocusCore` destructured at `:23`, never rendered. `MEMORY_COPY.focusPersonalCore` orphaned at `memoryCopy.ts:120` | Cannot get back to the core after panning |

**Command:** `/impeccable harden` + `/impeccable audit`

---

### P2 — Two more data-integrity issues

**(i) `selectionchange` handler does O(document) work per event.**
`Memory.tsx:489-561`. 3 forced-layout calls per event (`:517`, `:518`, `:526`), plus
`fullContent.slice(0, snippetIdx).split("\n").length` at `:539` — a full-document
substring allocation plus newline array, run twice on the fallback path (`:544`). **Not
rAF-throttled, not debounced.** `selectionchange` fires continuously during drag-select.
Effect deps `[personalMemory?.content, drawerOpen]` (`:561`) re-register on every
content change. *The user most likely to be reading that pane sequentially is also the
user who triggers it hardest.*

**(ii) Filter change during in-flight fetch is silently discarded.**
`useObservationsList.ts:81-87` — no `isMounted`, no `AbortController`, no cleanup.
`statusFilter` is in the dep array, but `fetchBatch` bails at `:36`
(`if (isFetchingRef.current) return;`) — a bare global mutex. The new-filter fetch is
discarded with no queue, retry, or error. Violates `frontend-style-guide.md:59`.

**Command:** `/impeccable optimize`

---

### P3 — Copy drift and a page file that has outgrown its layer

`memoryCopy.ts` is 174 lines and unusually thorough. The surface ignores it in **18
places** (5 `aria-label` literals, 7 `title` literals, 0 `placeholder` — that one is
clean, 1 JSX literal), including keys that already exist:
`StagingHeader.tsx:107` hardcodes `"No pending suggestions"` while
`MEMORY_COPY.noPendingSuggestions` sits unused at `memoryCopy.ts:73`;
`MemoryNodeTooltip.tsx:75` hardcodes `"Identity Core"` while `MEMORY_COPY.identityLayer`
("About You & Preferences") exists at `:6`.

Copy drift is not cosmetic — it is where the lies come from. `ActionHubView.tsx:71` and
`StagingHeader.tsx:102` both call Edit an *"in-place editor"*;
`RawEditorView.tsx:20` is a whole-document `<textarea>`. `RawEditorView.tsx:23-26`
advertises `## Overview, ## Preferences` in the Import placeholder then renders the
identical textarea. Two differently-promised affordances, one implementation.

`Memory.tsx` is **1,182 lines with ~20 `useState` and 15 `useCallback`**, containing
data fetching, business logic, keyboard bindings, selection handling, and Lenis setup.
`frontend-style-guide.md:31` says pages "MUST only define visual structure, routing, and
layout composition." It also writes refs in the render body at `:159-161` and `:145` —
the exact shape `ResponsiveLayout.tsx:60` explicitly warns against. And `:1139` passes
an inline arrow into a `memo`'d `PersonalMemoryStagingCard`, defeating the memo on every
parent render (`frontend-style-guide.md:82`).

**Command:** `/impeccable polish`, extract `usePersonalMemoryDossier` first.

---

## 6. Perf Ledger Correction

`docs/features/performance-memory-optimizations.md:130` (§2.3) claims:

> *"Two-Phase Physics Settlement: Runs physics simulation for `ticks < 100` (`alpha=0.08`,
> `repulsion=1200`, `damping=0.85`), then freezes all physics calculations and GPU uploads
> at equilibrium (0 FPS CPU/GPU idle). Re-arms only when graph data changes.
> Pre-allocates instance buffers (`maxNodes=10000`, `maxEdges=20000`)."*

**None of the stated mechanism exists** in `useMemoryGraphScene.ts`:

| Claim | Actual |
|---|---|
| `ticks < 100` counter | No tick counter exists |
| `alpha=0.08` | Only `0.08` in the file is `controls.dampingFactor` (`:902`), an OrbitControls property |
| `repulsion=1200` | Absent |
| `damping=0.85` | Absent |
| `maxNodes=10000` | **12000** (`:1011`) |
| `maxEdges=20000` | **No `maxEdges` constant exists** |
| "Re-arms only when graph data changes" | Re-arms on `pointermove`/`pointerdown`/`wheel`/`touchstart` (`:1083-1086` → `wakeLoop` `:1066-1073`) |
| "0 FPS CPU/GPU idle" | Conditionally true — `:1102-1106` does reach rAF 0 — but only after 4,000 ms with no pointer, wheel, touch, or controls change. A resting cursor holds 30 FPS indefinitely |

The mechanism is a **4,000 ms wall-clock inactivity timer**, not a tick-count
equilibrium. **§2.3 must be corrected before anyone optimises against it.**

---

## 7. Persona Red Flags

### Alex (Impatient Power User)
1. No keyboard route to the feature — 5 shortcuts exist, all navigation (`:693-743`);
   the two high-value actions (open document, inspect node) have none.
2. Two clicks to read one memory — node picking off by default
   (`MemoryGraph.tsx:202-204`), so the first click is a camera drag. Must arm Inspect
   Mode in the dock every time. A mode toggle guarding a default is backwards.
3. Every state change re-fetches 7-8× — `handleApplySuggestions` (`:450-456`),
   `handleConsolidateNow` (`:397-403`), `handleRegenerateWithComments` (`:619-623`),
   `handleRestoreActive` (`:328-330`) each fire versions + revisions *then* `refresh(true)`
   which re-fires both plus memory + observations. And `refresh()` is a mount effect
   (`:212-214`), so he pays all 4 on every trip back.
4. The version chevrons lie to him — P0-2.
5. No window title.
6. Lenis inertia fights his scroll — `:249-256`, `duration: 0.8` on the reading pane he
   uses daily. Applied to *only* the left pane; `LearnedFactsList.tsx:205` uses native
   scroll. Two panes, two scroll physics, side by side.

### Jordan (Confused First-Timer)
1. First click on the orb does nothing, and nothing says why — hint says "Click the
   central orb" (`memoryCopy.ts:136`); `MemoryGraph.tsx:202-204` silently swallows picks;
   `:168-172` early-returns on any target with a `.pointer-events-auto` ancestor.
   Inspect Mode is discoverable only as a tooltip string (`GraphControlDock.tsx:48`).
2. Told he has no memories when the DB failed — P0-1. He talks for ten minutes.
3. Most prominent control on first open is disabled — `Memory.tsx:964-991` renders
   Consolidate at `disabled:opacity-40` when `unconsolidatedIdentityCount === 0`. The
   panel then offers three peer tiles (Comment / Import / Edit), none the obvious next
   step. The actual next step is on the page *behind* the drawer.
4. Three names for one subsystem: "Integrate Observations" (`:18`), "Suggested Profile
   Updates" (`:42`), "Staging Mirror" (`:155`). No mental model possible.
5. Hover-dependent discovery everywhere — category descriptions exist only in `Tooltip`
   (`MemoryLegendOverlay.tsx:61-65`).
6. `GraphControlDock.tsx:12` `onFocusCore` never rendered — see P2 table.

### Sam (Accessibility-Dependent)
1. The primary action is 100% unreachable — `MemoryGraph.tsx:254-262`.
2. The WebGL canvas is an entirely inaccessible data surface.
3. `aria-modal="true"` is a lie — verified at `Drawer.tsx:195`, no trap in the file.
4. Zero `aria-live` in the target surface (0 occurrences, confirmed). `handleApplySuggestions`'
   catch rethrows to a `TriangleAlert` **icon** with an `aria-label`
   (`SuggestionsReviewView.tsx:92-96`) — `aria-label` on an `<svg>` with no `role` is
   not reliably announced. Failure is effectively silent.
5. Async handler does O(document) work per selection event — P2(i).
6. Invalid combobox semantics (`SearchBar.tsx:81` vs `:120-123`); no `tabIndex` on any
   non-interactive element in the surface.
7. Colour-only and tiny text — `foreground-muted` at `/40`–`/50` opacity on 10.5-12 px
   (`LearnedFactsList.tsx:71`, `MemorySessionRail.tsx:71`, `EmptyState.tsx:37`)
   composites to ≈0.2-0.35 alpha, well under 4.5:1.
   `emerald-400`/`rose-400` at 12 px carry the entire accept/reject distinction
   (`SuggestionsReviewView.tsx:190,203,230,253`).
8. **`next_step` and `pitfall` are the same colour** — `dynamicGraphPalette.ts:89-90`
   puts them 65° apart in HSL. The code comment admits it: *"Luminous amber/gold/orange"*
   next to *"Warm amber-coral/canary"*. They encode **different semantic urgency** — one
   is a to-do, one is a scar — and colour is the *only* channel carrying it. Both render
   as 8 px legend dots, below the ~3:1 non-text threshold. One-line fix to the offset
   table.
9. **Focus order fights visual layout** — JSX order is search → rail → dock → legend
   (portalled to `document.body`, `:801-815`) → drawer. Tab order is DOM order, so the
   legend — a *primary filter*, positioned bottom-right — is reached last, after a jump
   to a body portal and back.
10. **Comments can anchor to the wrong line, and the wrong number is baked into the LLM
    prompt** — the handler searches `personalMemory?.content` (`:535`) but the rendered
    document is `displayedRecord.markdown || displayedRecord.content` (`:1116`), and
    `Markdown.tsx:22-49` *transforms* the text before render. On a miss the `firstWord`
    fallback (`:541-545`) can resolve elsewhere. That number goes into
    `Line ${c.line} ("${c.quotedText}")` at `:608` — so line-anchored feedback silently
    trains the model on the wrong location.

---

## 8. Detector False Positives (do not act on these)

| Finding | Why it is wrong |
|---|---|
| `cramped-padding` on `Memory.tsx:874` | The element declares `p-8` (32 px inset on all sides). The rule mis-parsed `bg-[rgba(var(--card),0.85)]` or dropped `p-8` |
| 18 × `ai-color-palette` + `dark-glow` + 3 × `radial-spotlight-glow` | Artifacts of the unconfigured default `--accent: 0, 219, 233` (`index.css:9`) in a fresh browser profile. The cyan is the pre-React default replaced by the configured accent in a real session |
| 8 × `low-contrast` | All from the **wizard** (`welcomeCopy.ts:105-108`): plain Chrome lacks Tauri IPC, `getOnboardingStatus()` rejects, `App.tsx:88,167` redirects to `/wizard`. Not the Memory page |
| 2 × `marquee` | `skeleton-shimmer` / `ribbon-flow` appear only inside Vite-injected `<style>` blocks, never on a Memory DOM element |
| 1 × `gradient-text` | `-webkit-background-clip: text` inside an app-wide `<style>` rule. `bg-clip-text` count = 0 on the surface |
| 2 × `buried-raster` | Rule is correct; the element is `.amb-noise` at `opacity 0.04` (`index.css:716`), a pre-existing app-wide ambient layer. Count is meaningful: 2, one per duplicated `AmbientBackground` |

**Genuine:** `tiny-text` (11 px pervasive: `Memory.tsx:889,918,930,953,969,1030,1058`;
`LearnedFactsList.tsx:139,170`; `MemoryGraph.tsx:71`).

**Browser probe caveat:** a stubbed Tauri snapshot without `vox_cpu_usage` made
`ResponsiveLayout.tsx:447` `voxCpu.toFixed(1)` throw and blank the app to `RENDER ERROR`.
`useVoxFootprint` null-guards the snapshot but dereferences the field unguarded.
**unverified** against the real backend (which always populates it) — a one-line
optional-chain makes it a non-event.

---

## 9. Minor Observations

- `MemoryNodeTooltip.tsx:31` clamps with `tooltipWidth = 360` while rendered width is
  `w-[340px]` (`:51`) — 20 px drift between two numbers.
- `MemoryNodeTooltip.tsx:37` assumes a 280 px tooltip height; a 200-char fact at
  13px/340px runs ~8 lines and clips off the bottom.
- The tooltip has no `role="dialog"` or focus management, yet registers an interactive
  close button in the global overlay stack (`:25`).
- `SearchBar.tsx:43-45` — the `results` `useMemo` is *undebounced* and calls
  `f.text.toLowerCase()` per fact per keystroke. The 150 ms debounce at `:65-67`
  correctly guards only the parent commit (per `frontend-style-guide.md:94`), not the
  local filter. Add a lowercased-text cache per `facts` change.
- `SearchBar.tsx:92` — `onBlur` sets a 200 ms `setTimeout` that is untracked and can
  `setState` after unmount.
- `SearchBar.tsx:86-126` — the input has no `aria-label`; uses `placeholder` as its
  accessible name.
- `MemorySessionRail.tsx:98-106` filters **undebounced** — inconsistent with
  `SearchBar.tsx:65`, which does it correctly. Same style-guide section applies.
- `Markdown.tsx:33` — `/^[A-Z][A-Za-z0-9, &/'"()-]+$/` promotes any short Title-Cased
  standalone line to a heading. "Prefer dark mode" becomes a section header with no
  indication the text was rewritten for display.
- `Markdown.tsx` has no `remark-gfm` — tables in an imported personal-memory document
  render as literal pipes.
- `Markdown.tsx:282-291` — document-variant links open `target="_blank"` with no
  external-link affordance.
- `dynamicGraphPalette.ts:76` — `baseH = hsl.s < 0.1 ? 0.55 : hsl.h`. A near-grayscale
  accent silently reverts the whole graph to fixed cyan, with no indication the colour
  choice was overridden.
- `dynamicGraphPalette.ts:81-82` — an `import` sits mid-file after several exports.
  Legal, wrong.
- `useObservationsList.ts:79-85` — see P2(ii).
- `ResponsiveLayout.tsx:204-222` — `Ctrl+W/Q/M/S/N//` bound with no guard against
  webview-native shortcuts or IME composition. `Ctrl+S` toggling the session rail
  instead of invoking save is at minimum surprising.
- `EdgeNav.tsx:34` — `BottomDockFeather` is a full-width 110 px gradient scrim pinned to
  the bottom of *every* page, layered over the Memory page's bottom-centre hint
  (`Memory.tsx:888`) and the legend portal (`:803`).
- `AmbientBackground.tsx:52` constant trace key — see P1-2(c).
- `PersonalMemoryStagingCard.tsx:263` `transition-all duration-500` on a container
  holding `glass-card backdrop-blur-sm` — see P1-2(d).

---

## 10. Tier Classification & Proposed Execution Order

review-ui categorisation. **Tier 1 = zero visible change. Tier 2 = barely noticeable.
Tier 3 = users will notice, requires sign-off.**

### Tier 1 — 🐛 Bug / pure fix · same pixels · no sign-off needed (~18 items)

| # | Fix | File |
|---|---|---|
| 1 | Delete the 4 route preloads; they cancel out your own `lazy()` | `App.tsx:66-70` |
| 2 | Delete the nav spinner — 3 rings that never paint | `EdgeNav.tsx:18,21-23,70-76` |
| 3 | **Delete the 500 ms debounce that writes to the DB when browsing revisions** | `PersonalMemoryVersionNav.tsx:55-59` |
| 4 | Line-number lookup: precompute offset table per doc, binary-search | `Memory.tsx:535-545` |
| 5 | rAF-throttle the `selectionchange` handler | `Memory.tsx:489-561` |
| 6 | Cache lowercased fact text per `facts` change | `SearchBar.tsx:43-45` |
| 7 | Add the same 150 ms debounce the search bar has | `MemorySessionRail.tsx:98-106` |
| 8 | Clear the 8 untracked `setTimeout`s on unmount | `Memory.tsx` ×8 |
| 9 | Per-instance trace key for the ambient background | `AmbientBackground.tsx:52` |
| 10 | `isMounted` + `AbortController`; fix the silently-discarded filter fetch | `useObservationsList.ts:81-87` |
| 11 | `voxCpu?.toFixed(1)` — removes an app-blanking crash | `ResponsiveLayout.tsx:447` |
| 12 | Move the 2 render-body ref writes into effects | `Memory.tsx:145,159-161` |
| 13 | `useCallback` the inline arrow defeating a `memo` | `Memory.tsx:1139` |
| 14 | Focus trap + focus restore on the `aria-modal` drawer | `Drawer.tsx:195` |
| 15 | Move `role="combobox"` onto the input it describes | `SearchBar.tsx:81,120-123` |
| 16 | Apply `.no-blur` during commit (workaround exists, used 1 of 44 places) | `PersonalMemoryStagingCard.tsx:263` |
| 17 | Move 18 hardcoded strings into `memoryCopy.ts`, reusing existing keys | 6 files |
| 18 | `powerPreference: "high-performance"` on both renderers | 2 files |

### Tier 2 — 💡 Subtle refactor · barely noticeable (~6 items)

| # | Fix | Visible? |
|---|---|---|
| 19 | Delete the 2nd `AmbientBackground` (`Memory.tsx:751`) — 2 containers, 4 blobs, 10 rings, 2 noise layers, 2 ripple origins | ripple centre moves to the one every other page uses; makes Memory consistent |
| 20 | Batch the dot matrix: 1,548 `beginPath`+`fill`/frame → ~6 | animates at 30 fps not 60; invisible in practice |
| 21 | Gate `antialias` on hardware renderer | very slightly harder edges on software raster; no GPU on Tier 1A anyway |
| 22 | Fix the tooltip's unreachable `exit` | fades out over 150 ms instead of blinking |
| 23 | Show total observation count, not the loaded page count | number corrects itself (400 no longer shows as "3") |
| 24 | Reset rail scroll on filter change | jumps to top when filtering; expected behaviour |

### Tier 3 — ⚖️ Trade-off · users will notice · requires sign-off (~12 items)

| # | Change | What changes visually | Read |
|---|---|---|---|
| 26 | **Route cross-fade** — `AnimatePresence mode="wait"` on `<Routes>` | 160 ms out / 200 ms in. Currently zero exit animation app-wide | Requested. Do it |
| 27 | **Loader exit + minimum dwell** | currently fades in 300 ms then hard-deletes. Becomes symmetric, no sub-frame flash | Requested. Do it |
| 28 | **Commit choreography** | `onAnimationComplete` instead of magic sleep; success CTA clickable | Fixes "abrupt" at source |
| 29 | **Error state card** + copy | new component, Retry button | Required (P0-1) |
| 30 | **"Viewing historical revision" bar** | new bar in dossier header; copy already exists | Required (P0-2) |
| 31 | **Graph stops breathing when idle** | orb goes still after ~2 s | ⚠️ **Hold.** It is the aliveness |
| 32 | **Keyboard route to primary action** | new shortcut + mirrored button | Feature is mouse-only today |
| 33 | **`aria-live` announcer** | invisible; app finally *says* "12 suggestions ready" / "saved" / "failed" | Invisible. Pure win |
| 34 | **Category palette re-derivation** | 2 legend dots change colour (`next_step` vs `pitfall`, 65° apart, different urgency) | Design call |
| 35 | **Grayscale-accent fallback** | near-gray accent no longer silently becomes cyan | Invisible unless gray is used |
| 36 | **Split `Memory.tsx`** (1,182 → hook + subviews) | nothing, but a large diff | Structural; style guide requires it |
| 37 | **`MotionConfig reducedMotion="user"`** | nothing unless the OS setting is on | Invisible for ~92% of users |

### Proposed execution order

- **Batch 1 — invisible.** Items 1-18. Highest value: #3 (data loss), #10 (dropped
  data), #4/#5 (perceived lag), #1/#2 (unreachable loader).
- **Batch 2 — visible, needs sign-off on 26/27/28.** These directly answer
  "loader to page transition is abrupt".
- **Batch 3 — hold for user decision.** #31 (graph breathing), #34/#35 (palette).
- **Batch 4 — structural.** #36. Re-run `pnpm lint`, `pnpm build`,
  `check_invariants.mjs` after each batch.

### Explicitly out of scope for auto-application

Lenis inertia removal (feel choice, not a bug) · the bottom hint line · the graph's
ambient breathing · any colour change · `Home` / `History` / `Settings` /
`Monitoring` (unreviewed — see companion audit).

---

## 11. Cognitive Load

**Checklist: 6 of 8 FAILED → HIGH cognitive load.**

| # | Item | Verdict |
|---|---|---|
| 1 | Single focus | FAIL — 5 concurrent staging modes (`idle`/`facts`/`comment`/`edit`/`import`/`suggestions`); current one inferable only from an icon swap in a header chip (`StagingHeader.tsx:67-81`) |
| 2 | Visual hierarchy | FAIL — the *disabled* Integrate button is the most prominent control; resting state is a 3-tile launcher with no primary action (`ActionHubView.tsx:26-74`); the enabled Integrate lives in the drawer header far away (`Memory.tsx:964-991`) |
| 3 | Grouping | FAIL — primary action (Consolidate) in the drawer *header*; secondaries in the *body*; the count badge explaining Consolidate (`Memory.tsx:986-990`) attached to neither |
| 4 | Consistent controls | PASS — one tooltip'd dock, one segmented filter rail, one underline search idiom reused twice |
| 5 | Clear next action | FAIL — first open with zero suggestions says "No pending suggestions" and offers three peer buttons, none the obvious next step |
| 6 | Feedback on action | FAIL — no `aria-live`/`role="status"` anywhere; version restore produces no status at all |
| 7 | Error recovery | FAIL — zero recoverable error states; 3 surfaces conflate failure with emptiness |
| 8 | Low memory burden | FAIL — `next_step` vs `pitfall` indistinguishable at 8 px; 3 names for one subsystem |

**Decision points with >4 visible options (Cowan):**
- Category legend — 6 options as an undifferentiated `grid-cols-3` text grid
  (`MemoryLegendOverlay.tsx:24-31,52-101`). No "All" chip; the escape hatch ("Clear")
  appears only *after* you filter (`:104-118`).
- Staging modes — 6 mutually-exclusive destinations split across three different UI
  regions; the mode switcher is a *different control* in every mode.
- Drawer open — **13 simultaneously visible interactive targets**, ungrouped into tasks.

**Emotional journey.** Click (promise of a spinner, delivered as a cut) → arrive
(peak: the topology thesis rendered as a body is genuinely arresting) → wait
(valley: loader cut off mid-fade) → inspect (valley: the orb does nothing, and the
hint text cannot say why) → edit (recovery, good) → commit (**nadir**: three racing
clocks, half-transparent mush, then 900 ms of dead UI) → after (flat: no toast, no
announcement, no title change, no undo).

---

## 12. Open Questions

1. **Does "alive" need continuous GPU work, or liveness of *response*?** The perpetual
   breathing core and never-resting 30 FPS loop are why the page feels heavy — but they
   are also the only thing on screen that is provably alive, which is the product thesis.
   A core that dilates on `Thinking`, sharpens on `Speaking`, stills on `Idle`, and
   renders once per state change would delete ~30 FPS of constant load **and** give a
   stronger aliveness signal. Which is the more honest expression of "sentient surface"?
2. **Should Personal Memory be behind an orb at all?** Every layer between graph and
   document — orb click, 65%-height drawer, two-column split, tab cluster — exists
   because graph and document are treated as different *places*. They are the same object
   seen two ways. Opening the drawer on node-hover context would delete the orb
   affordance problem, the inspect-mode gate, and the ActionHubView's empty decision —
   and make the graph *the* interface rather than a poster for one.
3. **Should empty and error be one component?** "We couldn't load your memories" and
   "you have no memories yet" both mean *come back when there's something here*. An app
   that says "I couldn't reach my own memory, try again" is more alive than one that
   confidently lies.
4. **Is the category palette the wrong abstraction?** Categories have **urgency**
   structure, not just identity: `blocker` and `pitfall` are things that went wrong;
   `workdone` is settled; `personal` is identity. Encoding **urgency (saturation/
   lightness) × kind (shape/glyph)**, with hue reserved entirely for the user's accent,
   would be legible in greyscale, in forced-colors mode, at 8 px, and for the ~8% of
   users with colour-vision deficiency — and would make the derivation problem
   disappear instead of needing tuning.

---

## 13. Verification Notes

Independently re-verified by the parent agent against source before publication:

| Claim | Result |
|---|---|
| `PersonalMemoryVersionNav.tsx:55-59` 500 ms debounce → DB write | ✅ confirmed |
| `PixelSynthesisCanvas.tsx:117-124` per-dot `beginPath`/`fill` + raw rAF | ✅ confirmed |
| `ResponsiveLayout.tsx:327` + `Memory.tsx:751` duplicate `AmbientBackground` | ✅ confirmed |
| `MemoryNodeTooltip.tsx:27` returns `null` before `AnimatePresence` at `:40` | ✅ confirmed |
| `Drawer.tsx:195` `aria-modal="true"` present | ✅ confirmed |

**Status:** findings only. **No code was modified. No commits made.**