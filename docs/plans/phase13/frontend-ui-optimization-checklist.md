# Frontend UI Optimization & Refactoring Checklist

> **Target:** Incremental execution, batch-by-batch, verified visually on Desktop & Android.
> **Verification Gate per Batch:** `pnpm tsc --noEmit && pnpm test && ./scripts/verify-targets.sh`.

---

## Batch 1: Shell, Navigation & Platform Gates (Desktop & Android)

**Depends on:** None.  
**Build Health:** Must build green throughout.

- [x] **`app/src/layout/ResponsiveLayout.tsx`**
  - Symbol: `ambientDisabled`
  - Action: Updated to `isMemory || isMonitoring || (isMobile && !isHome)` so `AmbientBackground` runs exclusively on Home (`/`) on Android devices.
- [x] **`app/src/layout/EdgeNav.tsx`**
  - Symbol: `EdgeNav`
  - Action: Wrapped the `/monitoring` NavLink in `!isRealMobileDevice()` so the Activity icon is omitted on mobile devices.
- [x] **`app/src/App.tsx`**
  - Symbol: `AppRoutes`
  - Action: Added route redirect for `/monitoring` to `/` when `isRealMobileDevice()` is true.
- [x] **`app/src/layout/TitleBar.tsx`**
  - Line 190 (`data-hover-card` app pill): Changed `backdrop-blur-xl` -> `backdrop-blur-md`, `shadow-2xl` -> `shadow-lg`.
  - Line 226 (`data-hover-card` model pill): Changed `backdrop-blur-xl` -> `backdrop-blur-md`, `shadow-2xl` -> `shadow-lg`.
- [x] **`app/index.html`**
  - Symbol: `window.onerror` & `window.addEventListener('error')`
  - Action: Suppressed benign `ResizeObserver loop completed` and `ResizeObserver loop limit exceeded` notifications.

---

## Batch 2: Settings Page & Mobile Layout Clipping Fix (Desktop & Mobile)

**Depends on:** Batch 1.  
**Build Health:** Must build green throughout.

- [x] **`app/src/pages/Settings.tsx`**
  - Line 222: Changed scroll container padding `pb-[95px]` -> `pb-[120px]` so bottom cards clear `EdgeNav` and `BottomDockFeather`.
  - Line 229: Adjusted mobile card padding to `px-2.5 sm:px-4 pt-2 sm:pt-3` to prevent 480px width squashing.
- [x] **`app/src/shared/ui/ToggleTile.tsx`**
  - Restricts hover slide tab to desktop (`!isSmall`), keeping rounded corners stable on touch.
- [x] **`app/src/index.css`**
  - Line 418: Removed `backdrop-filter: blur(20px)` from `.glass-card`; set solid `rgba(var(--card), 0.95)` surface in light mode.
  - Line 675: Changed `.custom-color-picker .react-colorful__saturation` radius from `1rem` to `0.75rem`.
  - Line 680: Changed `.custom-color-picker .react-colorful__hue` margin to `1rem` with `padding: 0 4px` to eliminate pointer clipping.
- [x] **`app/src/shared/ui/Card.tsx`**
  - Removed default `blur = "2xl"` prop so cards render as clean, GPU-efficient surfaces without multi-pass offscreen textures.
- [x] **`app/src/shared/ui/EdgePanel.tsx`**
  - Removed `backdrop-blur-md`; upgraded to solid `bg-[rgb(var(--card))]/96 border-[rgba(var(--border),0.08)] shadow-2xl`.
- [x] **`app/src/shared/components/settings/persona/PersonaCard.tsx`**
  - Removed double-nested inner bordered container in preview mode; text now renders directly into the primary container.
- [x] **`app/src/shared/components/settings/models/LlmCatalogView.tsx`**
  - Softened tooltips from `backdrop-blur-2xl shadow-2xl` to `backdrop-blur-md shadow-lg`.
- [x] **`app/src/shared/components/settings/SubModelCard.tsx`**
  - Changed hover card shadow from `shadow-2xl` to `shadow-md`.

---

## Batch 3: Home Page & Orb Surface Tuning (Desktop & Mobile)

**Depends on:** Batch 1.  
**Build Health:** Must build green throughout.

- [x] **`app/src/pages/Home.tsx`**
  - Line 246 & 264: Changed status/restore error banners from `shadow-2xl bg-black/40 backdrop-blur-md` -> `shadow-lg bg-[rgb(var(--card))]/95`.
- [x] **`app/src/shared/components/home/DialogueBubble.tsx`**
  - Line 43: Changed dialogue bubble from `backdrop-blur-xl shadow-xl` -> `bg-[rgb(var(--card))]/95 shadow-md`.
- [x] **`app/src/shared/components/home/AdvancedOrb.tsx`**
  - Lines 516–524: Low-power coarse-pointer RAF loop pause when calm & off-screen verified.
- [x] **`app/src/shared/components/settings/ModelStatusOverlay.tsx`**
  - Tap-to-toggle tooltip support for touch devices.
  - String truncation with `...` (`max-w-[140px] sm:max-w-[180px]`) for long model names and subtitles.

---

## Batch 4: History Page Solidification & Virtualization (Desktop & Mobile)

**Depends on:** Batch 1.  
**Build Health:** Must build green throughout.

- [x] **`app/src/shared/components/history/HistoryListView.tsx`**
  - Lines 225–235: Virtualized session rows wrapped in relative container matching `rowVirtualizer.getTotalSize()`.
- [x] **`app/src/shared/components/history/HistoryListView.tsx`**
  - Line 117: Changed sticky filter header `bg-[rgb(var(--background))]/80 backdrop-blur-md` -> solid `bg-[rgb(var(--background))] border-b border-[rgba(var(--border),0.12)]`.
- [x] **`app/src/pages/History.tsx`**
  - Line 275: Changed delete confirm banner `shadow-2xl backdrop-blur-md` -> solid `bg-[rgb(var(--card))]/95 shadow-lg`.
  - Line 437: Changed sticky top navigation bar `bg-[rgb(var(--background))]/80 backdrop-blur-md` -> solid `bg-[rgb(var(--background))]`.
- [x] **`app/src/shared/components/history/DetailPanel.tsx`**
  - Line 58: Changed message turn bubble `bg-[rgb(var(--card))]/75 backdrop-blur-md` -> solid `bg-[rgb(var(--card))] border border-[rgba(var(--foreground),0.08)]`.
- [x] **`app/src/shared/components/history/CalendarPicker.tsx`**
  - Line 125: Changed date popup `backdrop-blur-2xl shadow-xl` -> solid `bg-[rgba(var(--card),0.96)] shadow-lg`.
- [x] **`app/src/shared/components/history/HistorySearchBar.tsx`**
  - Line 261: Softened search dropdown shadow from `shadow-2xl` -> `shadow-lg`.

---

## Batch 5: Memory Page Surface Tuning (Uncapped Graph Fidelity)

**Depends on:** Batch 1.  
**Build Health:** Must build green throughout.

- [x] **`app/src/shared/components/memory/MemoryTimeline.tsx`**
  - Lines 870–880: Virtualized accordion rows wrapped in relative container matching `rowVirtualizer.getTotalSize()`.
- [x] **`app/src/shared/components/memory/MemoryTimeline.tsx`**
  - Lines 289, 348: Change context menu `shadow-2xl` -> `shadow-lg`.
- [x] **`app/src/pages/Memory.tsx`**
  - Line 643: Change full-modal `backdrop-blur-2xl shadow-2xl` -> `backdrop-blur-md shadow-xl`.
- [x] **`app/src/shared/ui/ExpandableList.tsx`**
  - Line 169: Change expansion modal `backdrop-blur-2xl shadow-2xl` -> `backdrop-blur-md shadow-xl`.
- [x] **`app/src/shared/components/memory/GraphControlDock.tsx`**
  - Line 33: Change dock `backdrop-blur-2xl shadow-2xl` -> `backdrop-blur-md shadow-lg`.
- [x] **`app/src/shared/components/memory/MemoryNodeTooltip.tsx`**
  - Line 82: Change node tooltip `shadow-2xl` -> `shadow-lg`.
- [x] **`app/src/shared/components/memory/SessionNodeTooltip.tsx`**
  - Line 116: Change session tooltip `shadow-2xl` -> `shadow-lg`.
- [x] **`app/src/shared/components/memory/SearchBar.tsx`**
  - Line 305: Change search dropdown `shadow-2xl` -> `shadow-lg`.
- [x] **`app/src/shared/components/memory/PersonalMemoryDossierCard.tsx`**
  - Line 93: Change card `shadow-2xl` -> `shadow-lg`.
- [x] **`app/src/shared/components/memory/PersonalMemoryStagingCard.tsx`**
  - Line 348: Change staging card `shadow-2xl` -> `shadow-lg`.
- [x] **`app/src/shared/components/memory/MemoryStates.tsx`**
  - Line 18: Change state card `backdrop-blur-xl shadow-2xl` -> `backdrop-blur-md shadow-lg`.

---

## Batch 6: Context Menus, Tooltips & UI Primitives (Desktop & Mobile)

**Depends on:** Batches 2, 4, 5.  
**Build Health:** Must build green throughout.

- [x] **`app/src/shared/ui/SessionContextMenu.tsx`**
  - Line 150: Change context menu `shadow-2xl` -> `shadow-lg`.
- [x] **`app/src/shared/ui/ProjectContextMenu.tsx`**
  - Line 148: Change context menu `shadow-2xl` -> `shadow-lg`.
- [x] **`app/src/shared/ui/ChapterScrubber.tsx`**
  - Line 402: Change scrubber hover card `backdrop-blur-xl` -> `backdrop-blur-md`.
- [x] **`app/src/shared/ui/Drawer.tsx`**
  - Line 235: Retained inline backdrop per design preference.
- [x] **`app/src/shared/ui/Tooltip.tsx`**
  - Line 184: Retain compact sizing on touch devices (`isTouchOrMobile && "max-w-[240px] px-1.5 text-[11px]"`).

---

## Batch 7: Giant Panel Splits & Startup Weight (D2, C2, C5)

**Depends on:** Batches 2, 3, 4, 5.  
**Build Health:** Expected red mid-refactor, green upon completion.

- [ ] **`app/src/shared/components/home/SessionPanel.tsx` (1058 lines -> Split)**
  - Extract `app/src/shared/components/home/session/SessionTurnBubble.tsx` (individual message bubble + markdown renderer).
  - Extract `app/src/shared/components/home/session/SessionControlsBar.tsx` (input controls, audio visualizer, dictation trigger).
  - Extract `app/src/shared/components/home/session/SessionHeaderActions.tsx` (export, clear, and context popovers).
  - Keep `SessionPanel.tsx` as lightweight orchestrator (<300 lines).
- [ ] **`app/src/shared/components/memory/MemoryTimeline.tsx` (913 lines -> Split)**
  - Extract `app/src/shared/components/memory/timeline/TimelineQueueCard.tsx` (pipeline status & extraction queue).
  - Extract `app/src/shared/components/memory/timeline/TimelineAccordionItem.tsx` (single session row).
  - Keep `MemoryTimeline.tsx` as clean layout wrapper (<300 lines).
- [ ] **`app/src/shared/components/settings/models/LlmCatalogView.tsx` (1380 lines -> Split)**
  - Extract provider sections into lazy-mounted tab bodies (`CloudCatalogSection.tsx`, `LocalCatalogSection.tsx`, `RemoteCatalogSection.tsx`).
- [ ] **`app/src/shared/components/home/AdvancedOrb.tsx` & `Home.tsx` (C2)**
  - Defer Three.js module initialization until after initial React DOM mount.
- [ ] **Wizard & Panel Static Transitions (C5)**
  - Replace static `motion.div` opacity fades in `MobileStepFooter.tsx` and `SystemCheckStep.tsx` with standard Tailwind CSS `transition-opacity`.
