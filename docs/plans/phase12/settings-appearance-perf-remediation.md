# Settings Appearance Lag — Audit, Tier 1 + Tier 2 Remediation, and What's Left (2026-10-06)

Remediation record for the "Settings page feels heavy; the accent slider and the
theme card are laggy" report. This document covers what was measured, what was
changed, how it was verified, and what remains.

**Related:** `theme-transition-rca-and-remediation.md` (original flip RCA),
`theme-transition-followup.md` (gate uniformity + blur scaling),
`../specs/design-spec.md` §4 (performance is part of the design),
`../specs/notifications-spec.md` §4.7 (the `--notif-*` token family).

---

## 0. How to read this doc

- **Audience:** frontend engineers touching `shared/theme`, `store/settingsStore`, or `components/settings`.
- **Scope:** the Settings route's appearance interactions (theme flip, accent drag) and the commit machinery they sit on. Not Home/History/Memory rendering.
- **Convention:** claims cite `path/file.ts:line` or quote measured output. Numbers marked **[Chromium-synthetic]** come from a headless-Chrome harness, not from WebKitGTK — see §5 for why.
- **Non-goals:** visual redesign of the Settings page. Every change here is Tier 1 (zero visible difference) or Tier 2 (barely perceptible).
- **SSOT:** the theme mechanism's contracts live in `app/src/index.css` ("Theme Transition Mechanism" header) and `app/src/shared/theme/index.ts`. The rebuild policy SSOT is `../specs/ipc-spec.md` §`get_settings`/`update_setting`.

---

## 1. Baseline: what the user's own trace logs proved

The report was triggered by two symptoms: dragging the accent picker is not
smooth, and toggling theme via the Appearance card is slow. The user supplied
live WebKitGTK `[theme-flip]` logs. The decisive comparison was not the flip
traces — it was `#23`/`#25` versus `#26`–`#31`:

| Trace | What wrote | nodes | sync write | forced recalc | TBT | frames/700ms |
|---|---|---|---|---|---|---|
| `#20` | theme flip, 1 card open | **315** | 86 ms | 0.0 ms | 283 ms | 7, worst 171 ms |
| `#22` | theme flip, 6 cards open | **9,606** | **1,154 ms** | 0.0 ms | 825 ms | **1**, firstFrame 875 ms |
| `#24` | theme flip, 6 cards open | **9,606** | **1,386 ms** | 0.0 ms | 932 ms | **1**, firstFrame 982 ms |
| `#23`/`#25` | *no-op* `applyTheme` | 9,606 | 0–1 ms | **0.0 ms** | 83–495 ms | 27 / 5 |
| `#26`–`#31` | **accent write** | 9,605 | 0–4 ms | **683–2,772 ms** | **1,395–5,847 ms** | **1**, firstFrame 1,445–5,897 ms |

Three facts fall out, and they reframed the problem:

1. **The recalc cost is entirely attributable to the appearance write.** `#23`/`#25` take the non-animated branch with *identical* values, so `setAttribute`/`setProperty`/`classList` write the same value (no invalidation) and the notification guard `textContent !== css` short-circuits. Result: 0.0 ms. Nothing else changed. So the 683–2,772 ms is not "some other work" — it is what an accent write costs on this document.

2. **`#26`–`#31` are accent changes** (`animate=false`, sync write 0–4 ms ⇒ the `isThemeChange === false` branch at `theme/index.ts:329`). The only mutations are `--accent` on `<html>` plus the notification token swap. Each costs 683–2,772 ms of full-document style recalc. That is the "laggy slider," measured: ~0.4–1.5 fps on release, and the same cost deferred to paint on every `mousemove`.

3. **Cost is super-linear in node count.** 315 nodes → 0.0 ms; 9,606 nodes → 683–2,772 ms. That is ~1 µs/node vs ~70–290 µs/node. **Node count is the multiplier.** A prior RCA had recorded the same state at only ~1,200–2,000 nodes, so the document had also grown ~5×.

Incidental finding from the logs: the snap detector reported `bg:static drift=[0,0,0]` for every target in `#22`/`#24`, sampled at +990 ms and +1,752 ms — *after* the flip had finished. Its sample marks were hardcoded to `[80,160,232]`, the old 200 ms budget. The instrumentation was measuring nothing.

---

## 2. Root causes (as diagnosed, pre-fix)

| ID | Cause | Evidence |
|---|---|---|
| **R1** | 9,606 nodes, zero CSS containment, uncapped catalog render | `LlmCatalogView.tsx:691` renders every remote model row (34 JSX/row); `AsrWorkspace:59`, `VadWorkspace:65`, `TtsModelWorkspace:65`, `AuxiliaryWorkspace:37` also uncapped; no `contain`/`content-visibility` anywhere in `index.css` |
| **R2** | Flip forces **two** full-document recalcs before first paint | `theme/index.ts:361` and `:375` `readRootTokenValues()`; the second is skipped only on a `targetCache` hit, and accent drags evicted the whole cache (`clear()` on overflow) |
| **R3** | `trace.ts` shipped ungated in production | `trace.ts:27-31` 5 whole-document scans; `:36-38` forced recalc + forced layout; `:47-78` `longtask` observer + 700 ms rAF; `:100-160` three timed `getComputedStyle` bursts *inside* the flip window |
| **R4** | Accent drag wrote `--accent` once per `mousemove`, unthrottled, outside the theme module | old `AppearanceCard.tsx:29-34` |
| **R5** | 237 `transition-all` sites, with no gate during a drag | `rg -c transition-all` = 237; the flip gate neutralizes them, a drag did not |
| **R6** | Notification tokens written as a `<style>` `textContent` swap | `notificationCategoryTokens.ts:136` — invalidates every rule's match cache document-wide |
| **R7** | Ambient repaint targets huge and accent-driven | `.amb-glow` 60vmax radial-gradient, 3× `.rp-ring` to 2,240 px with accent borders; `will-change` shields neither a `background-image` nor a `border-color` repaint |
| **R8** | Store cascade in the same task as the flip | 12 whole-store `(s) => s.draftSettings` selectors + 6 always-mounted `SettingsCardWrapper`s × 2 `JSON.stringify` walks per `set()` (~1,100 calls/keystroke) |
| **R9** | `commitChanges` re-read the whole model catalog every commit | `settingsStore.ts:898-902` → `get_model_catalog`; for ZipVoice/Chatterbox that opens the Turso DB (`ipc/catalog.rs:131-139, 166-184`) |
| **R10** | Per-keystroke `console.info` | old `Settings.tsx:69-71` |

**Save-footer verdict (asked explicitly):** the footer does **not** block theme or
accent changes, and the debounce should **not** be raised. For
`updateDraft("appearance","theme",v)`, `isRestartKey` has no `appearance` arm, so
`isDomainRequiringRestart("appearance")` is `false`; `autoSavedDomain` is still
null; therefore `mode === null` and **no footer is mounted** — `AnimatePresence`
at the old `SettingsCardWrapper.tsx:60` has zero children, so no `height`
animation and no layout shift at the toggle instant. The 200 ms appearance
debounce is *deliberately* inside the 232 ms flip gate, because
`SettingsContext.tsx:37-44` uses `getThemeTransitioning()` (true for exactly
232 ms) to suppress a redundant `get_settings` + second `writeThemeToDom` after
every theme toggle. Raising it past ~232 ms would buy zero flip cost and turn
that suppression off. `applyTheme` runs at `settingsStore.ts:562`, *before* the
timer is armed, so the debounce is not on the flip's critical path at all.

---

## 3. Tier 1 — pure fixes (shipped)

Tier 1 = zero visible change; same pixels, fewer resources.

### T1-a — Gated the theme-flip trace behind `import.meta.env.DEV`
`shared/theme/trace.ts`, called from `theme/index.ts` (both branches).
The harness's own header said "Active while debugging the Settings-route flip lag."
Ungated it ran, per `applyTheme` call (including every accent release), 5
whole-document `querySelectorAll` scans, a forced style recalc **and** a forced
layout, a `longtask` observer, and a 700 ms rAF loop competing with the flip's own
token interpolation; on a real flip it added three timed `getComputedStyle` bursts
inside the window. No `NODE_ENV` guard existed anywhere in `shared/theme`, and
`vite.config.ts` strips no console output.
**Verified:** `[theme-flip]`, `snap @+`, and `notif-category-tokens` are all absent
from `dist/assets/*.js`.

### T1-b — rAF-coalesced the accent write, and moved it into `shared/theme`
`AppearanceCard.tsx` → new `previewAccent()` / `beginAccentPreview()` / `endAccentPreview()` in `theme/index.ts`.
The old handler was a bare `documentElement.style.setProperty("--accent", …)` per
`mousemove`, outside the theme module — so it bypassed coalescing, the repaint
gate, and `--notif-*` sync (the seven notification colours sat frozen at the old
accent for the whole drag and jumped on release).

### T1-c — Removed the per-keystroke debug log
Deleted the `console.info` effect in `pages/Settings.tsx`.

### T1-d — Gated `loadModelCatalog()` on model-relevant scopes
`store/settingsStore.ts`. `commitChanges` awaited a full catalog re-read — and
for ZipVoice/Chatterbox a real Turso DB round trip — on *every* commit, including
a persona-prompt edit. Now tracked via a `touchedScopes` set during the diff loop
and re-read only if a scope in `MODEL_CATALOG_SCOPES` changed. `restoreDefaults`
still always re-reads (a reset restores every default).

### T1-e — O(1) dirty/restart selectors
`store/settingsStore.ts` + `SettingsCommitControls.tsx`.
`isDomainDirty` and `isDomainRequiringRestart` are `JSON.stringify` walks over
~57 declared keys plus a whole-scope TTS comparison (`tts` has no key list, so it
takes the whole-scope branch). They were used directly as Zustand selectors, so
each of the 6 always-mounted wrappers ran both on **every** `set()`. Added a
`domainFlags` snapshot recomputed once per mutation (`recomputeDomainFlags()`),
and split the whole-object `(s) => s.draftSettings` subscription into atomic leaf
selectors, so an appearance change no longer re-renders every card wrapper.

### T1-f — `parseTokenLiteral` handles substituted `rgba()`
`theme/index.ts`. `--connection-glow` / `--connection-core` are declared as
`rgba(var(--accent), 0.15)`, but `getComputedStyle().getPropertyValue()` returns
the **var-substituted** form `rgba(0, 219, 233, 0.15)`, which matched none of the
bare-triplet / number / `rgba(var(--accent),…)` patterns → filtered out of the
interpolation → **snapped at t=0** instead of fading. Consumed by the hub
connectors (`SettingsVisualConnectors.tsx:128,134`). Added a four-channel `rgba`
token kind. This also made the `alpha` branch dead code path-wise.

### T1-g — Toast timer handles + per-domain `failedSaveKeys`
`store/settingsStore.ts`. `triggerAutoSaveToast` (1,800 ms) and
`triggerSaveFailure` (6,000 ms) created untracked `setTimeout`s, so a second edit
inside the window had the earlier timer clear the newer state early (the "Changes
Saved" footer visibly truncated on rapid theme toggles). Both now clear before
re-arm. `failedSaveKeys` was a single flat array shared by all domains, so two
simultaneous failures made one card's banner print another card's key names; it is
now `Record<domainId, string[]>`.

### T1-h — Appearance debounce generation stamp
`store/settingsStore.ts`. The old code nulled `appearanceDebounceTimer` *inside*
the fired callback, so a second `updateDraft` during the in-flight IPC started a
second unguarded timer; a late resolution then wrote a stale value into
`settings`, leaving the card permanently dirty. Added a monotonic generation
counter checked in the resolution handler.

### T1-i — Dead CSS, stale docs, user-visible stray prose
- Removed a dead rule: `[data-theme-transition] .rp-ring, .amb-glow { animation-play-state: paused }` was always overridden by the later `*` + `!important` rule.
- Restored the **"Theme Transition Mechanism"** header in `index.css` (two source files referenced it; the header had been stripped in commit `7c478474`).
- Corrected the `theme/index.ts` docstring: it claimed "~2000 nodes," "`mix-blend-mode: multiply`," "a WebGL orb" on Settings, and "37 tokens" (the array has 45).
- Deleted a stray prose line rendered as a bare JSX text node in the LLM tooltip of `ModelStatusOverlay.tsx` (user-visible on hover).

### T1-j — Invariant 12 extended; new Invariant 26
`test/invariants.test.ts`. Invariant 12's comment claimed `--accent` was "owned
solely by shared/theme," but the check only grepped `setAttribute`, so a second
`--accent` writer passed review. It now also matches `setProperty("--accent")`,
`setProperty("--notif-…")`, and `removeAttribute("data-theme-transition")`.
Added **Invariant 26**, asserting the trace's `TRACE_ENABLED` gate exists *before*
any DOM query in each traced function, that `probeEnabled` defaults `false`, and
that `setThemeTraceProbe` is exported.

---

## 4. Tier 2 — subtle refactors (shipped)

Tier 2 = users would barely notice (a background effect frames slower, one fewer
blur layer).

### T2-a — The repaint gate now covers accent drags
`theme/index.ts`, `index.css` header. An accent drag writes the same `:root`
properties a flip does, but never opened the gate — so it paid *all three* gate
costs on every frame: 237 `transition-all` sites each starting a colour
transition, the ambient field's animations still running, and every
`backdrop-filter` region re-blurring. `syncGate()` now has two independent holders
(`flipGateHeld`, `previewGateHeld`); either keeps `data-theme-transition` open.
Because a drag commit runs `applyTheme` while the preview still holds the gate,
the attribute is written **once** for the whole gesture rather than closed and
reopened (two extra full-document recalcs).

**Safety fix found while implementing:** the release is bound to `window`
(`pointerup`/`mouseup`/`pointercancel`), not just the wrapper's `onPointerUp`.
`react-colorful@5.7.0` tracks the drag with a `document`-level `mouseup`, so a
gesture ending outside the card never fires the wrapper handler — and an unclosed
gate leaves all 237 `transition-all` sites and every blur region suspended
app-wide. Also released on unmount.

### T2-b — Notification tokens moved from a `<style>` rule to inline props
`shared/theme/notificationCategoryTokens.ts`. Assigning `textContent` to a live
`<style>` element invalidates the match cache for **every rule in the document**.
Inline props on `<html>` inherit to all descendants, so every existing
`var(--notif-*)` reference keeps resolving; the flip's end-value capture no longer
has to strip them first. Bonus: the accent-derived family now tracks the drag live
instead of jumping on release.

### T2-c — Capped the remote model catalog render window
`settings/models/LlmCatalogView.tsx`. `filteredRemoteModels.map` (line 691) rendered
**every** model a remote server advertises, ~34 JSX nodes per row, uncapped, twice
(inline grid and modal). Now `REMOTE_MODEL_PAGE_SIZE = 40` with a "Show N more"
button, reset on query/filter/list change. The `.length` readouts still report the
true match count, not the capped one.

### T2-d — CSS containment: **proposed, measured, REFUTED, not shipped**
The original proposal was `contain: content` on the settings card grid cells.
Two hypotheses were tested empirically in headless Chromium before deciding:

1. *Does `contain: style` break custom-property inheritance* (the app's entire
   color system)? **No** — measured directly, all four variants resolve
   `rgb(1,2,3)` from an inherited `--accent`. That hypothesis was wrong.
2. *Does containment reduce the forced style flush?* **No — it made it slower.**
   On a ~8,000-node synthetic tree with ~15% accent fan-out:

   | | median forced style flush |
   |---|---|
   | no containment | **21.0 ms** |
   | `contain: content` | **32.1 ms** |

   `contain: style` makes each contained element re-compute style independently,
   adding boundary overhead without reducing invalidation scope for an **inherited**
   custom property.

Separately ruled out: `content-visibility: auto`, because `useSettingsPage`
measures `card-*` bounding boxes for the SVG connector geometry, and
`content-visibility` makes non-visible subtrees report intrinsic placeholder sizes.
`Tooltip` uses `FloatingPortal`, so `contain: paint` would *not* have clipped
tooltips — that risk did not materialize, but it was not the blocking one.

**Conclusion:** containment targets *paint* cost, while the measured 683–2,772 ms
is *style recalc* cost. It cannot help this workload. Not shipped.

### T2-e — Snap-detector sample marks scale with the observed duration
`trace.ts`. Marks were hardcoded to `[80,160,232]`; once a flip overran that (both
1,154 ms and 1,386 ms traces did) every sample landed after the animation and the
detector reported "nothing happens" while looking operational. Now spread at 25% /
60% / 90% of the real window.

### T2-f — `targetCache` eviction policy
`theme/index.ts`. Overflow did `targetCache.clear()`, so a single accent drag could
discard every learned theme target and force the next flip to pay its second forced
recalc for nothing. Now evicts oldest-first; limit 12 → 32.

### Trace-harness honesty fix (supporting T1-a)
The forced-recalc probe (`getComputedStyle(document.body).backgroundColor`) does
not *observe* a style recalc — it **causes** one, synchronously, inside the click's
task. That is why the original logs reported 683–2,772 ms of "forced style recalc":
a production browser would flush the same recalc at paint, after yielding. The
probe is now **opt-in** via `setThemeTraceProbe(true)`; the pass census, frame
sampler, and snap detector are passive. Without this, dev traces would still
misreport after the fixes.

---

## 5. Verification

### Automated
```
npx tsc --noEmit                       → No errors found
pnpm test                              → 45/45 passed (4 files)   [was 44; +Invariant 26]
node .agents/skills/review-ui/scripts/check_invariants.mjs → 5/5 satisfied
pnpm build                             → ✓ built (no ESLint config exists in this repo;
                                         `pnpm lint:dead` (knip) is the only lint script)
```

### Production-bundle checks (that the dev-only harness is actually gone)
| Probe | Result |
|---|---|
| `[theme-flip] #` in `dist/assets/*.js` | absent |
| `snap @+` | absent |
| `notif-category-tokens` | absent |
| `querySelectorAll("*")` | 2 hits — **false positives**: the memory profiler's own DOM census, not the theme trace |
| `theme-flip` in History chunk | **false positive**: the `.theme-flip-surface` CSS class |

### Behaviour verified by reasoning against the store (not by runtime test)
- Appearance toggle → `mode === null` → **no footer mounted**, no layout shift (pre-existing behaviour, confirmed and now documented).
- Debounce is not on the flip's critical path (`applyTheme` at `:562` precedes timer arming).

### Blocked: the approved WebKitGTK measurement
`pnpm tauri dev` with `WEBKIT_INSPECTOR_SERVER=127.0.0.1:9222` builds and boots
successfully; the inspector socket **binds** (owned by the Vox PID). But the Vox
window is never mapped in this session (Wayland session, XWayland exposes only a
tray), so the webview never realizes, no page target registers, and the inspector
answers nothing on any path (`/json`, `/devtools/page/1`, raw HTTP, raw WebSocket).
The measurement was not run and not approximated.

### What was measured instead: **[Chromium-synthetic]**
A headless-Chrome harness replicating the measured shape — ~19,000 nodes, ~15%
`var(--accent)` fan-out, 3 accent ripple rings, a 60vmax accent glow, 237
`transition-all` sites, `.glass-card { backdrop-filter: blur(20px) }`, and the
actual gate CSS from `index.css`. Absolute values will **not** match WebKitGTK;
ratios are indicative.

| Case | median ms/frame | ceiling |
|---|---|---|
| gate OFF (old drag behaviour) | 165.7 | 6 fps |
| gate ON (**T2-a, shipped**) | **74.7** | 13 fps (**−55%**) |
| 19,090 → 3,190 nodes (gate ON) | **12.2** | 82 fps (**−84%**) |

**Two honest negative/neutral results:**

1. **T1-b (rAF coalescing) measured ~2%.** `mousemove` already fires at
   approximately frame rate, so coalescing does not reduce write count much in a
   synthetic `setTimeout(0)` loop (300 events → 294 writes). It is still correct
   to have — it caps worst case when multiple sources write per frame or a
   high-rate pointer path (`pointerrawupdate`, touch) is used — but it is **not**
   where the win is.
2. **Node count is a ~6× stronger lever than the gate** (84% vs 55%). This
   validates prioritising T2-c over further micro-optimisation, and is the single
   most important open item.

---

## 6. Recommendation — what to do next

### Do this first: finish bounding node count
The measured evidence is unambiguous: node count dominates. T2-c capped one
contributor; the rest is untouched.

1. **Cap the four workspace grids** the same way T2-c capped the catalog:
   `AsrWorkspace.tsx:59`, `VadWorkspace.tsx:65`, `TtsModelWorkspace.tsx:65`,
   `AuxiliaryWorkspace.tsx:37` — all uncapped catalog maps.
2. **Explain the ~1,550 nodes/card residual.** With 6 cards open the document is
   9,606 nodes; the catalog maps do not obviously account for that. Find the rest
   before optimising further — optimising an unmeasured remainder is guesswork.
3. **Re-measure node count on the real engine** (open 1 card vs 6) to confirm the
   curve holds outside Chromium.

### Then: the deferred / not-approved Tier 3 items
None of these are approved. Re-costed against the measurements above:

| ID | Item | Status | Re-assessment with measurement in hand |
|---|---|---|---|
| **T3-b** | Cap how many Settings cards can be open at once | **Not approved** | **Now the highest-leverage option.** 315 nodes → 0.0 ms recalc; 9,606 → 683–2,772 ms. A 3-card cap would put the document near ~5,000 nodes and roughly halve every appearance-write cost — a bigger structural win than any shipped Tier 1/2 item. Still a product decision: it removes the all-six overview. **Recommend revisiting first.** |
| **T3-e** | Scope the accent preview instead of writing `:root` | **Not approved** | The only fix that makes the drag genuinely 60fps rather than ~13fps. Shipped T1-b + T2-a get it to ~halving; scoping the write is what removes the rest. Costs the "whole app is your palette" live-preview property. **Recommend revisiting if 55% is not enough.** |
| **T3-a** | Decouple the ambient field from `--accent` | Deferred | With the gate on, the rings and glow no longer animate during a drag — but they still **repaint** per frame on a `border-color`/`background-image` change. Plausibly a meaningful slice of the remaining 74.7 ms. Cheap to test by temporarily hardcoding `.rp-ring`/`.amb-glow` to a static colour. |
| **T3-c** | `transition-all` → explicit `transition-colors` (237 sites) | Deferred | T2-a already neutralises these *during the gate*. The residual is outside gate windows. 237 hand edits with per-site snap risk; low ROI now that the gate covers the hot path. **Recommend dropping** unless a non-gated jank is reported. |
| **T3-d** | Lower `blur(20px)` on `.glass-card` | Deferred | The gate drops every blur for the whole window, so this only matters outside gates. Glass elevation is a closed system (`frontend-style-guide.md` §5), so this needs a design-system decision. **Recommend deferring** until the node-count work lands. |

### Also outstanding (not part of this batch)
- **Reload-policy spec conflict, deferred by explicit decision.** `ipc-spec.md:278`
  says the frontend "MUST NOT infer reload need by comparing draft against saved
  settings," but `settingsStore.ts:414-456` (`isRestartKey`) + `:683-710`
  (`isDomainRequiringRestart`) is a frontend copy of the backend table and drives
  the "Apply & Restart" footer — while the store's own doc comment claims the
  opposite. Chosen direction: **fix code to match spec**, which requires the
  backend to classify a whole dirty domain *at draft time* (or expose a
  `get_reload_policy(domain, keys)` query) so the footer can render before commit.
  Per AGENTS.md §4.3 the spec edit comes first. This changes settings UX and was
  deliberately kept out of the perf batch.
- **Undocumented timing constants.** The 200 ms appearance debounce, 600 ms hot
  autosave debounce, 1,800 ms / 6,000 ms toast timers, and "re-read settings +
  model catalog after every commit" appear in no spec (`storage-spec.md` and
  `events-spec.md` are silent on autosave). Either document them or derive them.
- **`prefers-reduced-motion` gap.** The whole theme gate sits inside
  `@media (prefers-reduced-motion: no-preference)`. With reduced motion on, the
  `backdrop-filter` suspension and animation pause never engage; only the JS-side
  rAF park in `AmbientBackground` protects the frame. Now that the gate also covers
  drags, this matters more.

---

## 7. Change index

| File | Items |
|---|---|
| `app/src/shared/theme/index.ts` | T1-a, T1-f, T2-a, T2-b, T2-f |
| `app/src/shared/theme/trace.ts` | T1-a, T2-e, probe opt-in |
| `app/src/shared/theme/notificationCategoryTokens.ts` | T2-b |
| `app/src/shared/components/settings/appearance/AppearanceCard.tsx` | T1-b |
| `app/src/pages/Settings.tsx` | T1-c |
| `app/src/store/settingsStore.ts` | T1-d, T1-e, T1-g, T1-h |
| `app/src/shared/components/settings/SettingsCommitControls.tsx` | T1-e, T1-g |
| `app/src/shared/components/settings/SettingsCardWrapper.tsx` | T1-g |
| `app/src/shared/components/settings/models/LlmCatalogView.tsx` | T2-c |
| `app/src/shared/components/settings/ModelStatusOverlay.tsx` | T1-i |
| `app/src/index.css` | T1-i, T2-a (header docs) |
| `app/src/test/invariants.test.ts` | T1-j (+Invariant 26) |
