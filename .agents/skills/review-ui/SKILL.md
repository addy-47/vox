---
name: review-ui
description: Deterministic architectural, performance, and memory audit suite for the Vox frontend. Contains internal scripts for fast static invariant checks via `pnpm test`, headless CDP stress-testing across all 7 drawers and edge panels, and extended 5-minute soak testing. Formats reports using the `/review-v2` rubric.
---

# `review-ui` — Vox Frontend Deterministic Audit & Stress Suite

## 0. Purpose

Eliminates manual guessing, silent simplifications, and repetitive human-in-the-loop debug cycles by enforcing automated, deterministic tests for Vox frontend style guide invariants, memory leaks, and drawer/panel rendering bottlenecks.

---

## 1. When to Use Which Script (Agent Decision Tree)

Whenever an agent touches or evaluates frontend code, consult this matrix to pick the right script:

```
Did you modify/review frontend code?
 │
 ├── Touched TypeScript/React components, stores, hooks, or services?
 │    └── ▶ RUN: `pnpm test` (or `node .agents/skills/vox-ui-audit/scripts/check_invariants.mjs`)
 │         ⏱ Time: <300ms
 │         🎯 Catches: Raw IPC outside `src/services/`, store destructuring, canvas unmount leaks, unmemoized context providers.
 │
 ├── Touched Drawers, Panels, Modals, Popovers, or CSS/Framer-Motion animations?
 │    └── ▶ RUN: `node .agents/skills/vox-ui-audit/scripts/stress_drawers.mjs`
 │         ⏱ Time: ~35s
 │         🎯 Catches: Animation frame drops (<50 FPS), missing GPU layer promotion (`transform-gpu`), layout thrashing, unmounted DOM accumulation across all 7 drawers/panels.
 │
 └── Touched WebGL shaders, Three.js scenes, audio loops, or long-running state management?
      └── ▶ RUN: `node .agents/skills/vox-ui-audit/scripts/soak_test.mjs`
           ⏱ Time: ~5 min
           🎯 Catches: Monotonic RAM growth, WebKitGTK backing store accumulation, uncollected event listeners, memory leaks.
```

---

## 2. Bundled Scripts Inside This Skill

All scripts live directly within `.agents/skills/vox-ui-audit/scripts/`:

| Script | Command | Purpose |
| :--- | :--- | :--- |
| **`check_invariants.mjs`** | `node .agents/skills/vox-ui-audit/scripts/check_invariants.mjs`<br>*(also wired to `pnpm test`)* | Instant static AST invariant audit. Fails fast with line numbers if style guide rules are violated. |
| **`stress_drawers.mjs`** | `node .agents/skills/vox-ui-audit/scripts/stress_drawers.mjs` | Automated headless Chrome DevTools Protocol (CDP) runner exercising all 7 drawers & edge panels. Measures real transition FPS, layout counts, and DOM drift. |
| **`soak_test.mjs`** | `node .agents/skills/vox-ui-audit/scripts/soak_test.mjs` | 5-minute continuous soak test cycling all routes with live drawer/panel interactions. Logs high-frequency telemetry and calculates MB/min heap slope. |

---

## 3. Review the findings and componets using the rubric below after running scripts

### Step 0 — Establish Scale and Intent (Mandatory, First)

Before critiquing anything, determine what this code is actually for. If not already clear from context, ask:

- Is this a proof of concept, an MVP, or production code expected to hold under real load?
- Roughly what scale — a handful of users, thousands, or millions?

**Calibrate everything that follows against this answer.** A pattern that's over-engineering for a weekend POC is exactly correct for a system built for a million users, and vice versa. Do not apply production-scale standards to a POC, and do not wave away real risk in something headed to production because "it works for now."

State your calibration explicitly before the review: "Reviewing this as [POC/MVP/production] scale — critique is calibrated accordingly."

**Also determine: does this code affect UI, rendering, or anything user-visible?** If yes, Step 3.5 (visual impact tiering) applies alongside normal severity. If this is backend, infra, or non-visual logic — skip it entirely, do not force it.

---

### Step 1 — Read for Understanding First

Before critiquing, understand what the code is actually trying to do. Do not review line-by-line in isolation — understand the shape of the whole thing first, then go back in with scrutiny.

---

### Step 2 — Hunt

Actively look for, in this order of severity:

**Will break / already broken**
- Code paths that will fail under a specific, nameable condition — not vague unease, an actual scenario ("this will break when X happens because Y")
- Race conditions, unhandled errors, silent failure modes
- Assumptions that don't hold (e.g. assuming a list is never empty, assuming a call never times out)

**Redundant or unnecessary**
- Code that duplicates logic that already exists elsewhere in the codebase
- Abstractions with only one implementation and no near-term second one
- Defensive code against conditions that cannot actually occur given the calibration from Step 0

**Over-engineered relative to stated scale**
- Patterns, layers, or generality that solve a problem this system doesn't have at its current scale
- If this is a POC/MVP: flag anything built for a scale or flexibility need that doesn't exist yet
- If this is production at real scale: flag anything too thin or too naive for the stated load

**Bloat**
- 100 lines doing what 10 could. This is the classic finding — locate it and show the replacement.

---

### Step 3 — For Every Finding

Do not just describe the problem. For each finding:

1. **What it is** — specific, not vague
2. **Why it matters** — the actual failure mode or cost, tied to the scale established in Step 0
3. **The replacement** — show the actual simpler/correct code, not just "this should be simpler." If it's a 100-line block that should be 10 lines, write the 10 lines.
4. **Severity** — 🔴 will break / 🟠 will cause real pain at stated scale / 🟡 stylistic preference, optional

Do not inflate stylistic preferences to the same severity as real risk. Say clearly which is which.

For every finding on UI/visual code, additionally classify:

| Tier | Label | Meaning |
|------|-------|---------|
| Tier 1 | 🐛 Bug - Pure Fix | Zero visible change. Same pixels, fewer resources — dead code, redundant re-renders, unnecessary re-computation. |
| Tier 2 | 💡 Subtle Refactor | Users would barely notice — reduced frame rate in a background effect, fewer blur layers, minor timing shift in an animation. |
| Tier 3 | ⚖️ Trade-off | Users would notice — removing a visual layer, changing an animation, restructuring a component's appearance or behavior. |

If proposing the fix/replacement from Step 3 would push a finding into Tier 3, say so explicitly and flag it as needing sign-off before applying — a visible trade-off is a design decision, not just a code cleanup, even if the code-level fix is trivial.

---

### Step 4 — What NOT to Flag

- Do not flag patterns that are correct for the stated scale, even if they'd be excessive at a different scale
- Do not flag genuine stylistic differences as bugs
- Do not manufacture findings to appear thorough — if a section is actually fine, say so and move on

---

### Step 5 — Final Report
