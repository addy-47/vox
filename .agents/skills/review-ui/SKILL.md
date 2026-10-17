---
name: review-ui
description: Audit and review Vox frontend changes with the UI trade-off tiers as the main rubric. Hunts first for the frontend doing the backend's job (hacks around a missing backend contract), then ad-hoc implementations that bypass shared primitives and tokens, then performance and memory. Runs the deterministic scripts (`pnpm lint:arch`, drawer stress test, soak test) and labels every finding with a visible-impact tier and a category. Use whenever frontend code is modified or reviewed (components, drawers, panels, modals, animations, stores, hooks, services), even for a "quick look" or "does this look right".
---

# `review-ui` — Vox Frontend Audit

## 0. Purpose

Eliminates manual guessing, silent simplifications, and repetitive human-in-the-loop debug cycles. Two questions drive every review:
1. **What changes for the user?** Every finding is tiered by visible impact.
2. **Does the frontend stay on its side of the backend contract?** The frontend consumes the contract; it must not replicate it or compensate for gaps in it.

---

## 1. Run the Scripts First (Agent Decision Tree)

```
Did you modify/review frontend code?
 │
 ├── Touched TypeScript/React components, stores, hooks, or services?
 │    └── ▶ RUN: `pnpm lint:arch`, ESLint, and `pnpm test`
 │         🎯 lint:arch catches: canvas teardown, GPU layer promotion on overlay hosts, unmemoized context providers, provider-id literals, theme-transition contract, glass duration utilities, breakpoint/dialog/shortcut SSOT, probe-arg casing, catalog copy, ProviderCaps fields nobody reads.
 │
 ├── Touched Drawers, Panels, Modals, Popovers, or CSS/Framer-Motion animations?
 │    └── ▶ RUN: `node .app/scripts/stress_drawers.mjs`
 │         ⏱ Time: ~35s
 │         🎯 Catches: Animation frame drops (<50 FPS), missing GPU layer promotion (`transform-gpu`), layout thrashing, unmounted DOM accumulation across all 7 drawers/panels.
 │
 └── Touched WebGL shaders, Three.js scenes, audio loops, or long-running state management?
      └── ▶ RUN: `node .app/scripts/soak_test.mjs`
           ⏱ Time: ~5 min
           🎯 Catches: Monotonic RAM growth, WebKitGTK backing store accumulation, uncollected event listeners, memory leaks.
```

**Who owns which check** (do not re-derive what a tool already reports):

| Tool | Owns |
| :--- | :--- |
| ESLint | No `any`; import and module-boundary rules, including raw IPC outside `src/services/` |
| `pnpm lint:arch` | Source-text invariants over `src/**` (list above) |
| `pnpm test` | Behavior tests in `src/test/**` |
| `stress_drawers.mjs` | Transition FPS, layout counts, DOM drift |
| `soak_test.mjs` | Heap slope (MB/min), backing-store growth, listener leaks |

- A script failure is a 🚦 INVARIANT finding; report it with the script's own line numbers.
- `lint:arch` is a source-text check, not a behavior test: a pass does not prove the behavior or the contract is right.
- If you did not run a script, say "not run" in the report. Never imply a pass.

---

## 2. Bundled Scripts

| Script | Command | Purpose |
| :--- | :--- | :--- |
| **`lint-arch`** | `pnpm lint:arch` | Fast static source-text invariant audit over `src/**`. Fails with file:line when a rule is violated. |
| **`stress_drawers.mjs`** | `node .app/scripts/stress_drawers.mjs` | Automated headless Chrome DevTools Protocol (CDP) runner exercising all 7 drawers & edge panels. Measures real transition FPS, layout counts, and DOM drift. |
| **`soak_test.mjs`** | `node .app/scripts/soak_test.mjs` | 5-minute continuous soak test cycling all routes with live drawer/panel interactions. Logs high-frequency telemetry and calculates MB/min heap slope. |

---

## 3. Review Rubric (apply after running the scripts)

### Labels

Every finding carries one **tier**, one **category**, and optionally the 🔴 overlay. Format: `[⚖️ T3 · 🔌 BACKEND-GAP]`.

| Tier | Label | Meaning |
|------|-------|---------|
| T1 | 🐛 Pure Fix | Zero visible change. Same pixels, fewer resources: dead code, redundant re-renders, unnecessary re-computation. |
| T2 | 💡 Subtle Refactor | Users would barely notice: reduced frame rate in a background effect, fewer blur layers, a minor timing shift. |
| T3 | ⚖️ Trade-off | Users would notice: removing a visual layer, changing an animation, restructuring a component's appearance or behavior. |

Tier describes what the user sees after the fix, not how big the diff is. If a proposed fix would push a finding into T3, say so and flag it as needing sign-off before applying: a visible trade-off is a design decision, not just a code cleanup.

| Category | Meaning |
|----------|---------|
| 🔌 BACKEND-GAP | The frontend replicates the backend contract or does the backend's job. |
| 🧩 AD-HOC | A one-off implementation that bypasses a shared primitive, token, hook, or service. |
| ⚡ PERF/MEM | Frame rate, re-renders, leaks, resource cost. |
| 🚦 INVARIANT | Reported by a script or by ESLint. |
| 🪦 DEAD/BLOAT | Dead code, duplicates, or far more code than the job needs. |

**🔴 overlay:** the code will break under a specific, nameable condition ("breaks when X happens because Y"). Not vague unease.

Do not inflate a stylistic preference into a 🔴 or a T3. Say clearly which findings are real risk and which are optional.

---

### Step 0 — Calibrate (Mandatory, First)

- **Surface:** main window, tray HUD, or setup wizard. They are separate webviews with different lifecycles.
- **Hardware floor:** judge resource cost against the low end the project supports (see the hardware tiers in `docs/backend.md` §2), not a development machine.
- **Visibility:** non-visual code (services, store, hooks) still gets a tier; with no visible effect it is T1 unless it changes behavior.

Read what applies before critiquing: `AGENTS.md` §5 (invariants), `docs/design.md`, `*style-guides` §8 and §10 (event contract, reload policies). Do not assume them.

State it: "Reviewing [surface], calibrated to [hardware floor]."

---

### Step 1 — Read for Understanding First

Understand what the UI does before critiquing. For each value or piece of state that reaches the screen, trace where it comes from: a backend event, a service call, the store, or a local derivation. Understand the whole shape first, then go back in with scrutiny. This trace feeds Step 2a.

---

### Step 2 — Hunt, in this order

#### 2a. 🔌 BACKEND-GAP (most important)

State flows one way, pipeline → UI. The event and command contract, payloads, and reload policies belong to the backend. Look for the frontend working around a missing or awkward contract:

- Inferring or guessing state the backend owns: timers that "wait for the engine to be ready", mood or state derived from combinations of events instead of consuming the backend's state event.
- Re-implementing backend validation, clamping, or defaults.
- Hardcoded capability, hardware-tier, model, or reload-policy tables. For example, check whether the store's `restartKeys` come from the backend's reload policy or are hardcoded on the frontend.
- Parsing error strings or message text to decide behavior.
- Polling or refetching whole state because no change event exists.
- Shadow copies of backend state that the frontend must reconcile.
- Retries, debounces, or delays that paper over a backend race.
- Branching on `pipeline_mode` for lifecycle. The frontend calls the same verbs and the backend routes.
- Passing the same argument in two casings, or other signs the contract is unclear.
- Estimating or computing a metric the backend already measures.

**Test:** could this be one field, event, or command from the backend? If yes, and the frontend is computing or guessing it, flag 🔌.
**Not a finding:** presentation logic. Mapping an event to a visual mood or formatting a value for display is the frontend's job.

**Output rule.** Never fix a 🔌 finding by adding more frontend code. Report a **Backend Ask** (format in Step 3). A hack that must stay for now gets a greppable comment: `backend-gap: <what the backend should provide>`.

#### 2b. 🧩 AD-HOC

- Hex colors, arbitrary Tailwind values, or inline styles instead of design tokens.
- A new glass elevation level instead of the closed set.
- Native `title` attributes used as tooltips instead of the shared `Tooltip`.
- Custom Escape or outside-click listeners instead of the overlay stack.
- Hand-rolled drawers, popovers, or modals instead of the shared primitives.
- Bespoke animation loops instead of the shared frame-rate hook.
- Copy outside `shared/data`.
- Raw IPC in components instead of the service layer.
- A new hook or helper that duplicates one in `shared/`.
- Magic numbers for breakpoints or durations that have an SSOT.

#### 2c. ⚡ PERF/MEM

Script results first. Then what the scripts can't see:
- Unmemoized heavy components, re-render cascades, missing fine-grained selectors.
- Fast-changing values in the store or context instead of local state or refs.
- Unthrottled or non-visibility-aware animation work; idle states should drive 0 FPS.
- Event listeners not registered through the shared cleanup wrapper.
- Stale async results applied after unmount or a route change.
- Unhandled errors and silent failure modes (for example, an IPC call that rejects and shows nothing).
- Assumptions that don't hold (a list never empty, a call that never times out or never resolves).
- The backend not being ready treated as an edge case instead of the default.

#### 2d. 🪦 DEAD/BLOAT

- Dead code, duplicated components, code that duplicates logic that already exists elsewhere in the codebase.
- Abstractions with one implementation and no near-term second.
- Defensive code against conditions that cannot occur on this surface.
- Contract fields or props nobody reads.
- 100 lines doing what 10 could. Locate it and show the replacement.

---

### Step 3 — For Every Finding

1. **What it is:** specific, with file and location.
2. **Why it matters:** the actual cost or failure mode, tied to the Step 0 calibration, and what the user does or doesn't see.
3. **The replacement:** show the actual code, not "this should be simpler." For 🔌, give the Backend Ask instead, plus what the frontend looks like once the backend provides it.
4. **Label:** tier, category, and 🔴 if it applies.

**Backend Ask format:**
- **Hack:** what the frontend does today, and where.
- **Backend should provide:** the command, event, or payload field.
- **Contract home:** where it belongs (event contract, reload policies, or IPC command surface).
- **Interim:** leave it marked with `backend-gap:` or remove it if it is harmful. Never extend it.

Backend Asks are inputs to the create-tickets skill: each becomes a ticket with `Layer: backend`, blocking the frontend ticket that depends on it.

---

### Step 4 — What NOT to Flag

- Patterns that are correct for the calibrated surface and hardware floor.
- Genuine stylistic differences as bugs.
- Presentation-only derivations (see the 2a exception).
- Anything a script or ESLint already reported: report it once as 🚦, do not re-derive it.
- Do not manufacture findings to appear thorough. If a section is fine, say so and move on.

---

### Step 5 — Final Report

1. **Calibration line** from Step 0.
2. **Scripts:** each one as run (pass/fail) or "not run".
3. **Tier summary:** `🐛 T1: n · 💡 T2: n · ⚖️ T3: n (need sign-off)`, plus the count of 🔴 overlays.
4. **Findings,** T3 first, then T2, then T1. Each with its label line, what, why, and replacement.
5. **Backend Asks:** every 🔌 finding, in the format from Step 3.
6. **Not verified:** what was not run, or could not be assessed.

T3 findings are not applied without sign-off.