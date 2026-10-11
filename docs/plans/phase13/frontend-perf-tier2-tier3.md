---
title: "Frontend Perf — Tier 2 and Tier 3 Decision List"
audience: "Internal — vox user (decision-maker)"
last_updated: 2026-10-12
owners: "frontend-engineer role"
related_docs:
  - "docs/plans/phase13/recent_work.md — phase log"
---

# How to read this doc

- **Audience:** you, picking what to approve. No code, plain words.
- **Scope:** every Tier 2 (barely noticeable) and Tier 3 (visible trade-off) perf item, plus 3 Tier 1s deferred as too risky to do blind.
- **Convention:** each item states category-tier, what you would see/feel, desktop/mobile, and my recommendation (DO / MAYBE / SKIP).
- **Non-goals:** not a plan — no steps, no sequencing. Say the word per item and it gets planned.
- **SSOT:** Tier 1 shipped work is logged in `docs/plans/phase13/recent_work.md`.

## Tier legend

- **Tier 2 — Barely noticeable:** slightly calmer/simpler if you stare. Safe to approve in bulk.
- **Tier 3 — Visible trade-off:** you WILL see a difference. Approve one by one.

---

## Group A — Frosted glass and shadows (the number-1 Android lag cause)

- **A1 — Cut ~51 frosted-glass blur layers to a handful (keep blur on small popups only). Tier 3. Both, hurts Android massively.** Phone stops stuttering when panels open; glass panels become solid instead of see-through-blurry. **Recommendation: DO on Android only** (solid card backgrounds on coarse pointers, keep glass on desktop) — biggest single Android win.
- **A2 — Drop strongest blur (blur-2xl/3xl, ~10 spots) to normal blur or none. Tier 2. Both, mostly Android.** Popups open faster, less smearing while scrolling behind them. **Recommendation: DO.**
- **A3 — Drop 27 extra-deep shadows (shadow-2xl) to normal shadow. Tier 2. Both, mostly Android.** Cards look a touch flatter; scrolling gets smoother. **Recommendation: DO.**
- **A4 — Remove stacked blur+shadow+transparency combos on the same card. Tier 2. Both, mostly Android.** Lists stop sticking mid-scroll. **Recommendation: DO.**

## Group B — Canvases and orbs that never stop moving

- **B1 — Pause Home 3D orb when tab hidden / on Android low-power. Tier 2. Both, mostly Android.** Orb looks the same while you watch; phone cools down when you do not. **Recommendation: DO.**
- **B2 — Replace animated waveform/liquid/pixel canvases with still images on Android. Tier 3. Mobile.** Wavy animations become static pictures on phone; desktop keeps motion. **Recommendation: MAYBE** — only if Android still lags after A1+B1.
- **B3 — Simplify History 3D carousel to flat list on small screens. Tier 3. Mobile.** Spinning cover-flow becomes a plain swipe list on phone. **Recommendation: MAYBE** — only if carousel proves slow on device.
- **B4 — Cap Memory 3D graph node count / auto-rotate off on Android. Tier 2. Mobile.** Graph slightly less lively on phone but stops freezing. **Recommendation: DO.**
- **B5 — Slow/freeze decorative CSS loops (spinners, floating dots, breathing logos) on Android. Tier 2. Both, mostly Android.** Background decor moves less; content feels snappier. **Recommendation: DO.**

## Group C — Startup weight

- **C2 — Load Three.js only when orb/graph actually opens. Tier 1 (deferred — touches chunk graph). Both, helps Android most.** First screen appears faster; 3D loads a beat later only when needed. **Recommendation: DO, but as a planned task** with build verification, not blind.
- **C5 — Replace simple framer-motion fades (36 files) with plain CSS. Tier 2. Both.** Open/close animations look near-identical but use less memory. **Recommendation: MAYBE** — opportunistically per file, not as a big bang.

## Group D — Big lists and panels re-drawing too much

- **D1 — Virtualize long lists (History, Memory timeline, session panel). Tier 2. Both, mostly Android.** Long lists scroll smoothly instead of jumping; rows look the same. **Recommendation: DO** — highest-value Tier 2 after Group A.
- **D2 — Split giant panels (SessionPanel, MemoryTimeline, Settings store ~37KB each) into smaller pieces. Tier 1 (deferred — refactor risk). Both.** Same screen; typing/scrolling no longer freezes the whole panel. **Recommendation: DO, but as a planned task** per panel.
- **D3 — Shorten glowing chat bubbles / markdown in far-up history. Tier 2. Both, mostly Android.** Old messages look plainer when far up; current chat unchanged. **Recommendation: MAYBE.**
- **D4 — Paginate/collapse Settings desks (LLM catalog ~69KB). Tier 2. Both, mostly Android.** Settings opens faster; sections expand on tap. **Recommendation: MAYBE** — only if Settings feels slow on device.

## Group E — Touch and scroll feel (Android-specific)

- **E2 — Replace swipe/spring panel animations with instant snap on Android. Tier 3. Mobile.** Panels jump in instead of gliding; much faster on weak GPUs. **Recommendation: MAYBE** — try A1 first; snap is a last resort.
- **E3 — Tap-only smaller tooltips on phone. Tier 2. Mobile.** Fewer accidental popups covering the screen. **Recommendation: DO.**
- **E4 — Larger tap targets + fewer nested popups in mobile wizard. Tier 2. Mobile.** Setup feels less cramped; fewer mis-taps. **Recommendation: DO.**

## Group F — Motion and overlay plumbing

- **F1 — One shared instant fade for drawers/modals on Android instead of per-component springs. Tier 3. Mobile (desktop keeps animation).** Drawers pop in instead of sliding; noticeably faster. **Recommendation: MAYBE** — pair with E2 decision.
- **F2 — Consolidate Modal+Drawer+EdgePanel backdrop handling. Tier 1. Both.** Already shipped (shared OverlayBackdrop) — listed here only so the list is complete.
- **F3 — Single short fade for boot loader + route transitions. Tier 1 (deferred — touches App shell). Both.** Startup flash looks the same, finishes sooner. **Recommendation: DO, but with the App-shell plan**, not blind.

---

## My recommended approval order (the short version)

1. **A1 (Android solid cards), A2, A3, A4** — the Android lag fix.
2. **D1, B1, B4, B5, E3, E4** — the safe Tier 2 bulk.
3. **C2, D2, F3** — planned Tier 1 follow-ups.
4. **B2, B3, C5, D3, D4, E2, F1** — only if 1–3 are not enough.

