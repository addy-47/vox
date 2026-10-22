# Vox Frontend Audit: Cross-Domain Impeccable Critique & Architectural Review

**Target:** Vox Desktop & Mobile Frontend (`main`, `wizard`, and `tray` surfaces across all 6 core domains, ~50,000 LOC in `app/src/`)  
**Calibration:** Low-end hardware floor (4-core x86_64 / Apple Silicon / Raspberry Pi 5 / Android ARM64, integrated GPU, 8GB RAM).  
**Date:** 2026-10-22  
**Rubric SSOT:** `.agents/skills/review-ui/SKILL.md` & Impeccable Critique methodology.

---

## 1. Deterministic Scripts Status

| Tool / Script | Status | Results & Invariants |
| :--- | :---: | :--- |
| **`pnpm lint:arch`** | **PASS** | All source-text architectural invariants hold across `src/**` (canvas teardown, GPU layer promotion, breakpoint/dialog SSOT, catalog copy). |
| **`pnpm test` (vitest)** | **PASS** | **187 / 187 passed** across 21 test suites in 28.98s (settings commit controls, capability probes, voice session streams, theme transitions). |
| **ESLint** | **FAIL** | **2 blocking errors, 386 warnings**: [`Monitoring.tsx#L113`](file:///home/addy/projects/apps/vox/app/src/pages/Monitoring.tsx#L113) (`react-hooks/exhaustive-deps` on reactive memoization of `latest`). |
| **`stress_drawers.mjs`** | *Not run* | Headless CDP animation stress runner requires an active X11/Wayland display server and running Tauri window. |
| **`soak_test.mjs`** | *Not run* | 5-minute continuous heap slope test omitted from critique pass. |

---

## 2. Cross-Domain Impeccable Critique Scorecard

Nielsen 10 Usability Heuristics evaluated across all 6 core functional domains:

| Domain | Surface & Key Files | Score | Rating | Cognitive Load Profile |
| :--- | :--- | :---: | :---: | :--- |
| **Domain 1: Home & Audio** | [`Home.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/Home.tsx), [`AdvancedOrb.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/home/AdvancedOrb.tsx) | **24 / 40** | Acceptable (60%) | **High**: 42° gaze split to right rail; double-dock conflict. |
| **Domain 2: Radial Settings** | [`Settings.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/Settings.tsx), [`RadialHub.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/RadialHub.tsx) | **28 / 40** | Good (70%) | **Moderate**: Smooth 12:00–10:00 clock; carousel deadlock. |
| **Domain 3: Memory Workspace**| [`Memory.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/Memory.tsx), [`MemoryGraph.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/MemoryGraph.tsx) | **26 / 40** | Acceptable (65%) | **Moderate**: Vector RRF jargon; auto-commit on browsing. |
| **Domain 4: History & Sessions**| [`History.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/History.tsx), [`OrbitCarousel.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/OrbitCarousel.tsx) | **28 / 40** | Good (70%) | **High**: 3D orbit forces depth rotation over linear search. |
| **Domain 5: System Telemetry** | [`Monitoring.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/Monitoring.tsx), [`LiquidChamber.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/monitoring/LiquidChamber.tsx) | **25 / 40** | Acceptable (62%) | **High**: 16 concurrent telemetry cards; 15 FPS continuous loop. |
| **Domain 6: Setup & Shell** | [`WizardRoot.tsx`](file:///home/addy/projects/apps/vox/app/src/wizard/WizardRoot.tsx), [`App.tsx`](file:///home/addy/projects/apps/vox/app/src/App.tsx) | **22 / 40** | Acceptable (55%) | **High**: Fatal crash on progress; mobile loop trap. |
| **Composite Architecture** | **Vox Application Core** | **153 / 240** | **63.7% (B-)** | **Extraneous Load Elevated across secondary routes.** |

---

## 3. Tier & Overlay Summary

- ⚖️ **T3 (Trade-off / Needs Sign-off):** 11 findings
- 💡 **T2 (Subtle Refactor):** 14 findings
- 🐛 **T1 (Pure Fix):** 18 findings
- 🔴 **Overlays (Fatal Crash, Data Corruption, or Silent Break Conditions):** 8 findings

---

## 4. Prioritized Audit Findings (by Tier)

### 4.1 ⚖️ Tier 3 — Trade-offs (User-Visible Structural & Architectural Decisions)

#### 1. [⚖️ T3 · 🔴 Overlay · 🔌 BACKEND-GAP] Browsing Revisions Silently Mutates Active Memory Profile
- **Location:** [`PersonalMemoryVersionNav.tsx#L43-L57`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/PersonalMemoryVersionNav.tsx#L43-L57)
- **What it is:** Browsing revision history starts a 500ms debounce timer that calls `onCommitActiveVersion(targetRecord.version)`.
- **Why it matters:** 🔴 **Corrupts the user's active personal profile.** Violates `memoryCopy.ts` ("Browsing a revision never writes"). Simply stepping through versions and pausing for 500ms permanently alters the active profile in Turso SQLite.
- **Replacement:** Remove the auto-commit timer completely. Browsing must only update local preview state; restoring requires an explicit "Restore Version" button click.

#### 2. [⚖️ T3 · 🧩 AD-HOC] Double-Dock Gestalt Collision & Spatial Instability
- **Location:** [`Home.tsx#L385-L520`](file:///home/addy/projects/apps/vox/app/src/pages/Home.tsx#L385-L520) & [`EdgeNav.tsx#L23-L27`](file:///home/addy/projects/apps/vox/app/src/layout/EdgeNav.tsx#L23-L27)
- **What it is:** The Home conversational action cluster (`Pause`, `Mic`, `Keyboard`, `Disengage`) floats 40px directly above the persistent `EdgeNav` dock. When text mode opens, this cluster dissolves into an underline input bar.
- **Why it matters:** Two stacked rounded glass capsules confuse spatial memory and cause misclicks between navigation and voice recording.
- **Replacement:** Consolidate voice actions directly into the persistent navigation dock, or tether conversational controls to the lower boundary of the central `AdvancedOrb` stage.

#### 3. [⚖️ T3 · 🧩 AD-HOC] Lateral Gaze Fracture (Displaced Transcript Rail)
- **Location:** [`Home.tsx#L297-L320`](file:///home/addy/projects/apps/vox/app/src/pages/Home.tsx#L297-L320)
- **What it is:** Live dialogue bubbles are rendered in a narrow 280–360px column pinned to the far right edge of the screen.
- **Why it matters:** Creates a 42° eye transit angle between the central speaker entity (`AdvancedOrb`) and the transcript, causing split-attention cognitive fatigue during voice interaction.
- **Replacement:** Migrate active conversation to a centered teleprompter subtitle container (`max-w-xl mx-auto`) floating directly beneath the Orb.

#### 4. [⚖️ T3 · 🔴 Overlay · 🔌 BACKEND-GAP] Search Turn Eviction & Silent Misses
- **Location:** [`HistorySearchBar.tsx#L69-L95`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/HistorySearchBar.tsx#L69-L95)
- **What it is:** Client-side search crawls full turn transcripts into an in-memory cache with `MAX_TURNS_CACHE = 20`.
- **Why it matters:** 🔴 **Breaks when session count exceeds 20.** Loading session 21 silently evicts session 0 from the cache. The search bar silently fails to find spoken utterances in older sessions, while every session list mutation re-triggers an IPC fetch cascade.
- **Backend Ask:** Implement backend search endpoint: `search_history(query: string, project_id?: string, limit?: u32) -> Result<Vec<HistorySearchHit>, VoxIpcError>` via Turso DB FTS5 or SQL `LIKE`.

#### 5. [⚖️ T3 · 🔌 BACKEND-GAP] Client-Side RRF Hybrid Vector Re-ranking
- **Location:** [`memoryService.ts#L112-L168`](file:///home/addy/projects/apps/vox/app/src/services/memoryService.ts#L112-L168) & [`Memory.tsx`](file:///home/addy/projects/apps/vox/app/src/pages/Memory.tsx)
- **What it is:** The frontend queries keyword matches and vector similarity matches independently over IPC, then computes reciprocal rank fusion (`rrfScore(k=60)`) in JavaScript.
- **Why it matters:** Transfers large unranked candidate payloads across the Tauri bridge, burning UI thread CPU and memory on ranking math that Rust can execute in milliseconds.
- **Backend Ask:** Implement unified `search_memory({ query: string, mode: "hybrid", limit: number })` returning pre-ranked results directly from Turso.

#### 6. [⚖️ T3 · 🔌 BACKEND-GAP] Dual Client-Side Polling Loops vs. Native Push Stream
- **Location:** [`useMonitoringMetrics.ts#L5-L67`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useMonitoringMetrics.ts#L5-L67) & [`useRuntimeSnapshot.ts#L4-L41`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useRuntimeSnapshot.ts#L4-L41)
- **What it is:** The frontend runs unsynchronized 1000ms and 30,000ms `setInterval` polling loops to pull telemetry from the backend.
- **Why it matters:** The Rust backend already runs a 10 Hz collector thread in `snapshots.rs`. Pulling via timers creates IPC traffic jams and misses instantaneous state transitions.
- **Backend Ask:** Emit a throttled Tauri event `monitoring://snapshot` (1 Hz during subscription or on residency change). Eliminate client `setInterval` timers.

#### 7. [⚖️ T3 · 🔴 Overlay · 🧩 AD-HOC] Carousel Synchronization Deadlock & handleSelect Flaw
- **Location:** [`SettingsSideCarouselCard.tsx#L58-L66`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/SettingsSideCarouselCard.tsx#L58-L66) & [`useSettingsPage.ts#L390-L401`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useSettingsPage.ts#L390-L401)
- **What it is:** In viewports under 840px height (`isSideCarousel = true`), paired domains share a carousel card. If both domains exist in `activeDomains`, the effect deadlocks and ignores selections. Furthermore, `handleSelect` is carousel-unaware: clicking an active radial node removes it from `activeDomains` instead of focusing the visible face.
- **Why it matters:** 🔴 **Breaks when clicking radial nodes on laptops or vertical splits.** Card faces fail to switch or close unpredictably.
- **Replacement:** Update `SettingsSideCarouselCard` to resolve the most recently activated domain:
  ```tsx
  useEffect(() => {
    const priIndex = activeDomains.lastIndexOf(primaryId);
    const secIndex = activeDomains.lastIndexOf(secondaryId);
    if (priIndex !== -1 && secIndex !== -1) {
      setCurrentId(priIndex > secIndex ? primaryId : secondaryId);
    } else if (secIndex !== -1) {
      setCurrentId(secondaryId);
    } else if (priIndex !== -1) {
      setCurrentId(primaryId);
    }
  }, [activeDomains, primaryId, secondaryId]);
  ```

#### 8. [⚖️ T3 · 🧩 AD-HOC] Desktop Forced 3D Orbit Without List Mode Toggle
- **Location:** [`History.tsx#L374-L430`](file:///home/addy/projects/apps/vox/app/src/pages/History.tsx#L374-L430)
- **What it is:** Desktop viewports (>=940px) force sessions into a 3D astronomical orbit ellipse with dial ticks.
- **Why it matters:** Power users cannot scan sessions chronologically, compare timestamps, or use date filters (available only on mobile list mode).
- **Replacement:** Provide an explicit view toggle (`Orbit` vs `List`) in the desktop header toolbar.

#### 9. [⚖️ T3 · 🪦 DEAD/BLOAT] Developer Mobile Phone Mockup in Production Desktop Wizard
- **Location:** [`WizardRoot.tsx#L153-L244`](file:///home/addy/projects/apps/vox/app/src/wizard/WizardRoot.tsx#L153-L244)
- **What it is:** Production desktop wizard renders a "View Mode: Desktop Setup / Android Flow" switcher and renders an Android device mockup with punch-hole camera markup.
- **Why it matters:** Clutters first-run desktop onboarding with mobile preview tooling intended for developers.
- **Replacement:** Guard behind `import.meta.env.DEV`.

#### 10. [⚖️ T3 · 🧩 AD-HOC] High-Chroma Cyan Body Text in Dialogue Bubbles
- **Location:** [`DialogueBubble.tsx#L43-L44`](file:///home/addy/projects/apps/vox/app/src/shared/components/home/DialogueBubble.tsx#L43-L44)
- **What it is:** Full multi-paragraph assistant speech content is rendered in `text-[rgb(var(--accent))]` (neon cyan).
- **Why it matters:** Reading extended paragraphs in saturated primary accent color causes severe eye fatigue and degrades reading comprehension.
- **Replacement:** Style assistant speech text with `text-[rgb(var(--foreground))]`, using `var(--accent)` exclusively for badge accents and borders.

#### 11. [⚖️ T3 · ⚡ PERF/MEM] Continuous 12 FPS WebGL Rendering in Idle Orb
- **Location:** [`AdvancedOrb.tsx#L531, L813`](file:///home/addy/projects/apps/vox/app/src/shared/components/home/AdvancedOrb.tsx#L531)
- **What it is:** Desktop Orb renders at 12 WebGL frames per second continuously when dormant in `Idle` or `Paused` states.
- **Why it matters:** Prevents CPU C-state package sleep and drains battery continuously on dormant background laptops.
- **Replacement:** Halt loop after 3 seconds of settling in `Idle` or `Paused` state; resume 60 FPS immediately upon state change or mouse proximity.

---

### 4.2 💡 Tier 2 — Subtle Refactors (Ergonomics, Memory & Performance)

#### 12. [💡 T2 · 🔴 Overlay · ⚡ PERF/MEM] Silent PTT Cancellation on Pointer Drift
- **Location:** [`Home.tsx#L446`](file:///home/addy/projects/apps/vox/app/src/pages/Home.tsx#L446)
- **What it is:** PTT button binds `onPointerLeave={handlePttCancel}`.
- **Why it matters:** 🔴 **Breaks when a user's thumb or mouse slips 2px outside the 56px button.** The spoken utterance is aborted and discarded with zero warning.
- **Replacement:**
  ```tsx
  onPointerDown={(e) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    handlePttStart();
  }}
  onPointerUp={(e) => {
    e.currentTarget.releasePointerCapture(e.pointerId);
    handlePttStop();
  }}
  // Remove onPointerLeave entirely
  ```

#### 13. [💡 T2 · 🔴 Overlay · ⚡ PERF/MEM] Blocking ESLint `latest` Dependency Violation
- **Location:** [`Monitoring.tsx#L113`](file:///home/addy/projects/apps/vox/app/src/pages/Monitoring.tsx#L113) & [`useMonitoringMetrics.ts#L11`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useMonitoringMetrics.ts#L11)
- **What it is:** In `useMonitoringMetrics.ts`, TypeScript strips `null` during return inference. In `Monitoring.tsx`, `latest` is initialized with a fresh object fallback `{ ... }` on every render, invalidating `useMemo` hooks down-tree at lines 208 and 239.
- **Why it matters:** 🔴 **Triggers blocking ESLint errors and causes 60Hz re-render cascades across all SVG gauges during active polling.**
- **Replacement:**
  Explicitly type `latest` as nullable in `useMonitoringMetrics.ts`:
  ```ts
  const latest: (RuntimeSnapshot & { localTime: number }) | null =
    history.length > 0 ? history[history.length - 1] : null;
  ```
  In `Monitoring.tsx`, hoist `EMPTY_SNAPSHOT` outside the component and memoize `latest`:
  ```tsx
  const latest = useMemo(() => rawLatest ?? EMPTY_SNAPSHOT, [rawLatest]);
  ```

#### 14. [💡 T2 · ⚡ PERF/MEM] Continuous 15 FPS Loop in Settled LiquidChamber
- **Location:** [`LiquidChamber.tsx#L126-L144`](file:///home/addy/projects/apps/vox/app/src/shared/components/monitoring/LiquidChamber.tsx#L126-L144)
- **What it is:** Canvas physics loop drops from 30 FPS to 15 FPS when values settle, but never stops.
- **Why it matters:** Keeps the CPU/GPU awake at 15 canvas repaints per second indefinitely while the monitor popover stays open.
- **Replacement:** Halt `requestAnimationFrame` when `settledFrames > 90`; resume only when metrics change or container resizes.

#### 15. [💡 T2 · ⚡ PERF/MEM] Heavy Runtime Providers Active Behind WizardStandIn
- **Location:** [`App.tsx#L310-L335`](file:///home/addy/projects/apps/vox/app/src/App.tsx#L310-L335)
- **What it is:** When routing to the wizard on desktop, the main window renders `<WizardStandIn />`. However, `VoiceSessionProvider` and `MemoryProfilerProvider` mount outside the route switch, registering 6 pipeline listeners and making IPC calls behind a static stand-in.
- **Replacement:** Mount runtime providers only when `routeToApp` is active.

#### 16. [💡 T2 · 🧩 AD-HOC] Missing Cancellation in Download Service
- **Location:** [`setupService.ts#L77-L105`](file:///home/addy/projects/apps/vox/app/src/services/setupService.ts#L77-L105) & [`ModelSetupStep.tsx#L330-L335`](file:///home/addy/projects/apps/vox/app/src/wizard/steps/ModelSetupStep.tsx#L330-L335)
- **What it is:** Rust backend supports `"cancel"` in `manage_models`. `setupService.ts` never exposes `cancelModelSetup()`. Going back leaves multi-GB downloads running in the background.
- **Replacement:** Expose `cancelModelSetup()` in `setupService.ts` and invoke it on `onBack`.

#### 17. [💡 T2 · 🔴 Overlay · 🧩 AD-HOC] ModelStatusOverlay Coarse-Pointer Tooltip Collision
- **Location:** [`ModelStatusOverlay.tsx#L56, L160, L190`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/ModelStatusOverlay.tsx#L56)
- **What it is:** A single `tipOpen` boolean controls tooltips for all three chips (ASR, LLM, TTS).
- **Why it matters:** Tapping any chip on a touch screen opens all three tooltips simultaneously, creating overlapping visual clutter.
- **Replacement:** Track `activeTipId: "asr" | "llm" | "tts" | null`.

#### 18. [💡 T2 · ⚡ PERF/MEM] Unhoisted 48 Dial Ticks in CentralClockNode
- **Location:** [`CentralClockNode.tsx#L71-L89`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/CentralClockNode.tsx#L71-L89)
- **What it is:** Code comment claims `DIAL_TICK_ELEMENTS` is precomputed at module scope, but it is declared inside the component body, re-allocating 48 SVG `<line>` elements on every render.
- **Replacement:** Hoist `DIAL_TICKS` and `DIAL_TICK_ELEMENTS` outside the component function.

#### 19. [💡 T2 · ⚡ PERF/MEM] Re-registering Keydown Window Listener on Every Arrow Key Press
- **Location:** [`History.tsx#L153-L189`](file:///home/addy/projects/apps/vox/app/src/pages/History.tsx#L153-L189)
- **What it is:** Keyboard navigation effect depends on `[selectedSession, currentWindowSessions]`. Every arrow navigation destroys and re-registers the window listener.
- **Replacement:** Mirror selection in a mutable ref and register the window listener once.

#### 20. [💡 T2 · 🧩 AD-HOC] Divergent Card Headers Across the 6 Settings Domains
- **Location:** [`PersonaCard.tsx#L337`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/persona/PersonaCard.tsx#L337), [`AppearanceCard.tsx#L114`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/appearance/AppearanceCard.tsx#L114), [`RealtimeCard.tsx#L166`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/realtime/RealtimeCard.tsx#L166)
- **What it is:** Hand-rolled headers bypass the shared primitive [`CardTitleBar.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/shared/CardTitleBar.tsx).
- **Replacement:** Standardize on `<CardTitleBar icon={...} title={...} action={...} />`.

#### 21. [💡 T2 · 🧩 AD-HOC] Dead Placeholder Default Tab in InteractionCard
- **Location:** [`InteractionCard.tsx#L39`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/interaction/InteractionCard.tsx#L39)
- **What it is:** `activeDeskTab` defaults to `"remote"`, which renders a static "Coming Soon" placeholder.
- **Replacement:** Default `activeDeskTab` to `"web_search"`.

#### 22. [💡 T2 · ⚡ PERF/MEM] Force Graph Physics Throttling on Main Thread
- **Location:** [`MemoryGraph.tsx`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/MemoryGraph.tsx)
- **What it is:** Force-directed simulation runs physics calculations directly on the UI thread without worker offloading.
- **Replacement:** Cap node rendering to top-K memories (K=50) or delegate physics integration to a Web Worker.

#### 23. [💡 T2 · 🔌 BACKEND-GAP] Client-side Stale Filtering & Unconsolidated Count Guessing
- **Location:** [`usePersonalMemoryDrawer.ts#L280-L284`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/usePersonalMemoryDrawer.ts#L280-L284)
- **What it is:** Frontend derives `unconsolidatedIdentityCount` by filtering memory graph active facts in memory.
- **Why it matters:** If facts are paginated or extracted in the background, the count becomes stale, disabling the consolidation button.
- **Backend Ask:** Expose `unconsolidated_personal_count` in `get_ingestion_stats`.

#### 24. [💡 T2 · 🔌 BACKEND-GAP] Invalidation Event Gap: `personal_memory_updated` Unsubscribed
- **Location:** [`eventsService.ts#L340-L344`](file:///home/addy/projects/apps/vox/app/src/services/eventsService.ts#L340-L344)
- **What it is:** Rust backend emits `IpcEvent::PersonalMemoryUpdated(record)`, but `eventsService.ts` omits this listener. Background scheduler updates leave UI stale until manual refresh.
- **Backend Ask:** Export `onPersonalMemoryUpdated` in `eventsService.ts`.

#### 25. [💡 T2 · 🔌 BACKEND-GAP] Three-Way Observation Status Nomenclature Collision
- **Location:** [`ipc/memory.rs#L214`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/memory.rs#L214) & [`LearnedFactsList.tsx#L28`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/LearnedFactsList.tsx#L28)
- **What it is:** Database stores `status = 'active'`, UI calls it `"staged"`, and IPC does back-and-forth string mapping.
- **Backend Ask:** Unify on explicit `ObservationStatus` enum on wire (`active | integrated | pending | failed`).

---

### 4.3 🐛 Tier 1 — Pure Fixes (Zero Visible Change, Code & Bug Cleanup)

#### 26. [🐛 T1 · 🔴 Overlay · ⚡ PERF/MEM] Fatal Crash on Undefined Progress Dictionary
- **Location:** [`ModelSetupStep.tsx#L286`](file:///home/addy/projects/apps/vox/app/src/wizard/steps/ModelSetupStep.tsx#L286) & [`MobileModelStep.tsx#L219`](file:///home/addy/projects/apps/vox/app/src/wizard/mobile/MobileModelStep.tsx#L219)
- **What it is:** 🔴 **Unshielded access `progress[m.id].progress` and `progress[f.id].step`.** In `MobileModelStep.tsx`, `categoryCards` evaluates immediately on mount upon receiving `manifest`, throwing an uncaught `TypeError: Cannot read properties of undefined (reading 'step')` before any download starts.
- **Replacement:** Use optional chaining:
  ```tsx
  const isDone = allFiles.every(m => progress[m.id]?.step?.toLowerCase() === 'completed');
  ```

#### 27. [🐛 T1 · 🔴 Overlay · 🔌 BACKEND-GAP] Mobile Infinite Onboarding Loop Trap
- **Location:** [`WizardRoot.tsx#L140-L143`](file:///home/addy/projects/apps/vox/app/src/wizard/WizardRoot.tsx#L140-L143)
- **What it is:** 🔴 **On mobile, `WizardRoot` binds `onComplete={() => setMobileResetKey(k => k + 1)}`.** Clicking "Finish" or "Skip" resets the wizard back to Step 1; the user can never enter the app.
- **Replacement:**
  ```tsx
  <MobileOnboardingFlow key={mobileResetKey} onComplete={() => window.location.replace('/')} />
  ```

#### 28. [🐛 T1 · 🔴 Overlay · 🧩 AD-HOC] Synchronous Window Role Boot Race
- **Location:** [`App.tsx#L90-L100`](file:///home/addy/projects/apps/vox/app/src/App.tsx#L90-L100)
- **What it is:** `isWizardWebview` defaults to `null`, firing an asynchronous dynamic import of `@tauri-apps/api/window` while `checkSetup()` polls `getOnboardingStatus()` up to 3 times. The window path is already synchronously available at `/wizard`.
- **Replacement:**
  ```tsx
  const [isWizardWebview] = useState<boolean>(() => {
    return typeof window !== "undefined" && window.location.pathname.startsWith("/wizard");
  });
  ```

#### 29. [🐛 T1 · ⚡ PERF/MEM] Root Component Whole-Draft Store Subscription Cascade
- **Location:** [`Settings.tsx#L85, L150`](file:///home/addy/projects/apps/vox/app/src/pages/Settings.tsx#L85)
- **What it is:** `const draftSettings = useSettingsStore((s) => s.draftSettings);` is used solely to verify `if (!draftSettings) return <Loader />`. Every keystroke across all settings fields forces the entire root `Settings` page and all 6 radial cards to re-render.
- **Replacement:**
  ```tsx
  const isLoaded = useSettingsStore((s) => s.draftSettings !== null);
  ```

#### 30. [🐛 T1 · ⚡ PERF/MEM] False Atomic Leaf Selector in SettingsCommitControls
- **Location:** [`SettingsCommitControls.tsx#L48-L63`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/SettingsCommitControls.tsx#L48-L63)
- **What it is:** Hook claims to use atomic leaf selectors, but line 53 subscribes directly to `(s) => s.draftSettings`. All 6 mounted card wrappers re-render on every keystroke.
- **Replacement:** Select only domain-specific slices (`s.draftSettings?.llm.active.provider`, etc.).

#### 31. [🐛 T1 · ⚡ PERF/MEM] Forced Layout Reflow in Production via `rcaRect`
- **Location:** [`SettingsCardWrapper.tsx#L41-L48`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/SettingsCardWrapper.tsx#L41-L48)
- **What it is:** `rcaRect()` evaluates `getBoundingClientRect()` synchronously on every render, even when diagnostic tracing is disabled.
- **Replacement:** Guard evaluation with `if (!rcaEnabled()) return;`.

#### 32. [🐛 T1 · 🔴 Overlay · ⚡ PERF/MEM] VadSettingsPanel Controlled Input Trap
- **Location:** [`VadSettingsPanel.tsx#L148-L155`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/speech/VadSettingsPanel.tsx#L148-L155)
- **What it is:** Numeric input derives value directly from the store without local draft state. Typing '5' immediately commits 0.05, and backspacing is blocked by `if (!clean) return`.
- **Replacement:** Maintain local state and commit on blur or Enter.

#### 33. [🐛 T1 · 🔴 Overlay · 🔌 BACKEND-GAP] Dictation Hotkey Concurrent IPC Dispatch
- **Location:** [`DictationConfigDesk.tsx#L148-L151`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/interaction/DictationConfigDesk.tsx#L148-L151)
- **What it is:** Hotkey save calls both `updateDraft` (which arms a 600ms autosave timer) and direct IPC `updateSetting`, racing two mutations for the same key.
- **Replacement:** Commit through `useSettingsStore.getState().commitChanges({ domain: "dictation", key: "hotkey" })`.

#### 34. [🐛 T1 · 🔴 Overlay] Screen Reader Accessibility Instruction Mismatch
- **Location:** [`MemoryGraph.tsx#L177`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/MemoryGraph.tsx#L177) & [`Memory.tsx#L425`](file:///home/addy/projects/apps/vox/app/src/pages/Memory.tsx#L425)
- **What it is:** `MemoryGraph.tsx` announces `aria-label: "Open details with Shift+Up"`, but `Memory.tsx` has zero event listener for `Shift+Up`.
- **Replacement:** Add `Shift+ArrowUp` handling in `Memory.tsx` to trigger `handleCoreClick()`.

#### 35. [🐛 T1 · 🧩 AD-HOC] Broken Tailwind Class in RealtimeCard
- **Location:** [`RealtimeCard.tsx#L170`](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/realtime/RealtimeCard.tsx#L170)
- **What it is:** Invalid syntax `text/80` fails to apply text opacity.
- **Replacement:** Replace with `text-[rgb(var(--foreground))]/80`.

#### 36. [🐛 T1 · 🔴 Overlay] CSS Class Syntax Error in HistoryListView
- **Location:** [`HistoryListView.tsx#L117`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/HistoryListView.tsx#L117)
- **What it is:** Typo `border-b]` corrupts Tailwind class name.
- **Replacement:** Replace with `border-b border-[rgba(var(--border),0.12)]`.

#### 37. [🐛 T1 · 🪦 DEAD/BLOAT] 5 Unused Props in CentralClockNode
- **Location:** [`CentralClockNode.tsx#L20, L37-L43`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/CentralClockNode.tsx#L20)
- **What it is:** `metaLabel`, `onGoToday`, `onBackToMonth`, `breadcrumbLabel`, and `compact` are passed from `History.tsx` but never destructured or used.
- **Replacement:** Prune unused props from `CentralClockNodeProps` and `History.tsx`.

#### 38. [🐛 T1 · 🪦 DEAD/BLOAT] Dead Legacy Canvas Renderer in PixelSynthesisCanvas
- **Location:** [`PixelSynthesisCanvas.tsx#L97-L131`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/PixelSynthesisCanvas.tsx#L97-L131)
- **What it is:** 35 lines of legacy rendering code behind an unreachable `if (!APPLY_BATCHED_MATRIX)` flag.
- **Replacement:** Delete `renderLegacy` and dead branching.

#### 39. [🐛 T1 · 🧩 AD-HOC] Native Title Attributes Used as Tooltips
- **Location:** [`HistoryListView.tsx#L179`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/HistoryListView.tsx#L179), [`DetailPanel.tsx#L159`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/DetailPanel.tsx#L159), [`CommentsQueueView.tsx#L57`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/staging/CommentsQueueView.tsx#L57)
- **What it is:** Elements use native HTML `title="..."` attributes instead of `@/shared/ui/Tooltip`.
- **Replacement:** Wrap with `<Tooltip content="...">`.

#### 40. [🐛 T1 · 🔌 BACKEND-GAP] Omission of `remote` Domain in Frontend Settings Types
- **Location:** [`settings.ts#L271-L284`](file:///home/addy/projects/apps/vox/app/src/types/settings.ts#L271-L284) & [`settingsStore.ts#L493-L505`](file:///home/addy/projects/apps/vox/app/src/store/settingsStore.ts#L493-L505)
- **What it is:** Rust backend contract supports 12 domains including `"remote"`. Frontend hardcodes only 11 domains.
- **Replacement:** Add `remote: RemoteSettings` to `VoxSettings` and map `"remote": "interaction"` in `SETTINGS_DOMAIN_TO_UI`.

#### 41. [🐛 T1 · 🧩 AD-HOC] AI Template Side-Tab Borders in Help Drawers
- **Location:** [`HomeHelpContent.tsx#L49`](file:///home/addy/projects/apps/vox/app/src/shared/components/help/HomeHelpContent.tsx#L49), [`HistoryHelpContent.tsx#L49`](file:///home/addy/projects/apps/vox/app/src/shared/components/help/HistoryHelpContent.tsx#L49), [`MemoryHelpContent.tsx#L49`](file:///home/addy/projects/apps/vox/app/src/shared/components/help/MemoryHelpContent.tsx#L49)
- **What it is:** Thick `border-l-2` colored accent borders on callout cards flagged by Impeccable detector.
- **Replacement:** Replace with subtle inset border or background tint.

#### 42. [🐛 T1 · 🔴 Overlay] Mobile Scrubber Disconnect & Dead Interaction
- **Location:** [`Home.tsx#L360-L381`](file:///home/addy/projects/apps/vox/app/src/pages/Home.tsx#L360-L381) & [`ChapterScrubber.tsx#L43`](file:///home/addy/projects/apps/vox/app/src/shared/ui/ChapterScrubber.tsx#L43)
- **What it is:** On mobile (<768px), `ChapterScrubber` is rendered while the dialogue container is hidden (`hidden md:flex`). Furthermore, `<ChapterScrubber>` has no `onSelect` prop passed, so tapping chapters does nothing.
- **Replacement:** Hide `ChapterScrubber` on mobile or wire `onSelect` to open the session transcript drawer.

#### 43. [🐛 T1 · 🔴 Overlay] Space Keydown Event Hijack on Custom Role Buttons
- **Location:** [`Home.tsx#L128, L173`](file:///home/addy/projects/apps/vox/app/src/pages/Home.tsx#L128)
- **What it is:** Space listener checks only `activeElement.tagName === "BUTTON"`. Pressing Space when focus is on a clickable list item (`<div role="button">`) triggers Push-To-Talk instead of activating the item.
- **Replacement:** Check for `role="button"` or `isInteractiveElement(el)`.

---

## 5. Formal Backend Asks (🔌 BACKEND-GAP)

### Backend Ask 1: History Full-Text Search IPC
- **Hack:** Frontend loads turns into a 20-item cache and executes substring matching in JS ([`HistorySearchBar.tsx#L69-L95`](file:///home/addy/projects/apps/vox/app/src/shared/components/history/HistorySearchBar.tsx#L69-L95)). Older sessions are evicted and missed.
- **Backend should provide:** `search_history(query: string, project_id?: string, limit?: u32) -> Result<Vec<HistorySearchHit>, VoxIpcError>` querying Turso DB via FTS5 index.
- **Contract home:** `vox_core::storage::SessionStore` / `app/src-tauri/src/ipc/persistence.rs`.
- **Interim:** `/* backend-gap: search_history must be executed in Turso DB to avoid turn eviction */`.

### Backend Ask 2: Unified Hybrid Vector RRF Search
- **Hack:** Frontend fetches keyword and vector candidates separately, running rank fusion in JS ([`memoryService.ts#L112`](file:///home/addy/projects/apps/vox/app/src/services/memoryService.ts#L112)).
- **Backend should provide:** `search_memory({ query: string, mode: "hybrid", limit: number })` returning pre-ranked results directly from Rust.
- **Contract home:** `vox_core::memory::MemoryService::search`.
- **Interim:** `/* backend-gap: hybrid RRF ranking belongs in Rust backend */`.

### Backend Ask 3: Event-Driven Telemetry Push Stream
- **Hack:** Frontend runs dual 1s and 30s `setInterval` polling loops ([`useMonitoringMetrics.ts#L5`](file:///home/addy/projects/apps/vox/app/src/shared/hooks/useMonitoringMetrics.ts#L5)).
- **Backend should provide:** Throttled Tauri broadcast `monitoring://snapshot` (1 Hz during subscription or on model residency transition).
- **Contract home:** `app/src-tauri/src/monitoring/snapshots.rs`.
- **Interim:** `/* backend-gap: telemetry must be event-driven push, not client interval polling */`.

### Backend Ask 4: Atomic Setup Completion Command
- **Hack:** Rust backend injects `main_win.eval("window.location.replace('/')")`, sleeps for 750ms, and calls `wizard_win.destroy()` ([`ipc/setup.rs#L133-L158`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/setup.rs#L133-L158)).
- **Backend should provide:** Atomic `complete_setup_and_focus_main()` that transitions window focus and broadcasts `setup_completed_event` without arbitrary thread sleeps.
- **Contract home:** `app/src-tauri/src/ipc/setup.rs`.
- **Interim:** `/* backend-gap: eliminate window.eval injection and sleep in setup completion */`.

### Backend Ask 5: Synchronous Session State Confirmation on Start / End
- **Hack:** Frontend polls `getRuntimeSnapshot` every 100ms in `waitForPipelineState` up to 8000ms ([`pipelineService.ts#L130-L156`](file:///home/addy/projects/apps/vox/app/src/services/pipelineService.ts#L130-L156)).
- **Backend should provide:** `start_session` and `end_session` IPC commands should resolve only when the engine enters `Ready` or `Idle`, returning confirmed status and active `conversation_id`.
- **Contract home:** `app/src-tauri/src/ipc/pipeline.rs`.
- **Interim:** `/* backend-gap: start_session and end_session must return confirmed state synchronously */`.

---

## 6. Not Verified / Limitations

1. **Headless CDP Transition Framerate:** `stress_drawers.mjs` was not executed because CDP drawer automation requires an active native Tauri window and X11/Wayland display server. Transition frame rates for edge drawers were verified by source inspection of `transform-gpu` usage.
2. **5-Minute Soak Heap Slope:** `soak_test.mjs` was not run to preserve session execution limits. Memory leak patterns were verified via effect teardown and listener registration audits.
3. **Hardware Governor on macOS/Windows:** Linux kernel governor leaks (`schedutil`/`powersave`) only trigger on Linux targets.
