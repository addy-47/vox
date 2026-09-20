# Vox Full Keyboard Functionality Contract (Phase 11)

> **Status:** Proposed Contract / Plan — SSOT for keyboard behavior going forward.
> **Scope (v2 — practical-only):** Global chrome, Home, History (list + orbit select, no calendar-grid), Memory (rail/search/tooltip/dossier, graph node-select only), Settings (hub/cards/toggles/inputs, no color-picker drag parity), Monitoring popover, Wizard (dead-div triage + heading focus only), rails/panels/overlays, Help panel + tooltips.
> **Goal — practical click parity:** everything *practical* doable with click must be doable with keyboard. Deliberately out of scope: Appearance color-drag parity (`HexColorPicker` — pointer-only stays), full calendar-grid arrows (`CalendarPicker` — Tab/Enter on day buttons is enough), free camera/orbit drag emulation, graph 3D camera nudging, knob-dial drag emulation beyond steppers/inputs.
> **Non-goals:** changing visual design; changing IPC/backend contracts; implementing the contract (this doc is the contract + plan — implementation follows in batches).
> **Revision v2 (2026-09-19):** applied user review — practical-only scope; Help shows shortcuts inline with feature explanations + header toggle for the full key→action map across all pages; every shortcut appears in its control's hover tooltip; new globals `Ctrl+M` / `Alt+N` / `Ctrl+S` / `?` / `Shift+Up/Down`; page-nav arrows fire only when EdgeNav (or nothing focusable) has focus, otherwise arrows move within the page alongside Tab.

---

## 0. How this document was built

Full audit of `app/src/**/*.{ts,tsx}` for `onKeyDown/onKeyUp/onKeyPress`, `keydown/keyup` listeners, `tabIndex`, `autoFocus`/`focus()/blur()`, `role=`/ARIA, `overlayStack`/`useOverlay`, and help copy that *documents* shortcuts.

### 0.1 What exists today (verified, with citations)

**Global:**
- `shared/lib/overlayStack.ts:80-85,113` — single global `Escape` (capture phase) pops topmost `registerOverlay()` entry. Installed once from `App.tsx:135-143` (deferred via `requestIdleCallback`, fallback `setTimeout 200ms`). Consumers: `Drawer.tsx:88-93`, `EdgePanel.tsx:59-65`, `Monitoring.tsx:157-158`, `MemoryNodeTooltip.tsx:25`.
- `layout/ResponsiveLayout.tsx:86-125` — global `ArrowLeft/ArrowRight` cycles pages. Routes `<1024px`: `/ → /history → /memory → /settings → /monitoring`; otherwise without `/monitoring`. Skipped when focus is in `input/textarea/select/contenteditable`. Closes monitor popover. Bubble phase.
- `pages/Settings.tsx:117-125` — window `Escape` pops topmost active Settings domain card (FILO). Runs *in addition* to overlay-stack capture `Escape`.
- `shared/ui/SessionContextMenu.tsx:69-82` — hand-rolled window `Escape` (capture) + `pointerdown` outside; not via `useOverlay`. No focus move, no `role`.

**Local (element-scoped `onKeyDown` only when focused):**
- `shared/components/home/TextInputBar.tsx:41-49` — `Enter` (no Shift) submit, `Escape` discard/close. Autofocus on mount `:30`.
- `shared/components/home/SessionPanel.tsx:168-174` row `role=button tabIndex=0`, `Enter/Space` select; `:207-218` rename input `Enter` commit / `Escape` cancel / `blur` commit; `:408-419` project header `role=button tabIndex=0` `Enter/Space` expand; `:839-852` new-project input `Enter` create / `Escape` cancel.
- `shared/components/memory/PersonalMemoryCommentPopover.tsx:129-137` — textarea `Enter` (no Shift) save, `Shift+Enter` newline, `Escape` discard. `autoFocus` + `focus()` after 30ms.
- `shared/components/memory/PersonalMemoryStagingCard.tsx:330-343` — comment edit `Cmd/Ctrl+Enter` save, `Escape` cancel. (Other textareas `:411,:433` have no key handling.)
- `shared/components/history/VoiceRippleNode.tsx:48-59` — `role=button tabIndex=0`, `Enter/Space` select (with `preventDefault`).
- `shared/components/history/MonthDayCard.tsx:27-38` — same pattern, `Enter/Space` drill.
- `shared/ui/ToggleTile.tsx:35-51` — **reference implementation**: `role=switch aria-checked tabIndex={disabled?-1:0}`, `Space/Enter` toggle with `preventDefault`.
- `shared/components/settings/persona/PersonaCard.tsx:230-245,340-341` — `Backspace/Delete` intercepted to protect `<user_identity>` etc. tags; `onBeforeInput` blocks typing inside tags.
- `shared/components/settings/models/LlmCatalogView.tsx:173-179` custom-model input `Enter` apply / `Escape` close; `:216-225` search input two-stage `Escape` (clear query → close bar), `Enter` blur; `:366-392` remote model card `role=button tabIndex=0` `Enter/Space` select.
- `shared/components/settings/interaction/DictationConfigDesk.tsx:61-101,273-281` — hotkey recorder `input[readOnly,autoFocus]` captures **any** combo; `Escape` cancel, `Enter` save, modifiers from `ctrlKey/altKey/shiftKey/metaKey`, `Space` normalized. `preventDefault+stopPropagation`.
- `shared/ui/VoiceCarousel.tsx:415-425` — inline rename `Enter` save / `Escape` cancel.

**Focus management:**
- `shared/ui/Drawer.tsx:95-104,197` — only true focus management in app: focus sheet on open (`tabIndex=-1 role=dialog aria-modal`), restore previously-focused element on close. No focus trap loop, no `Tab` containment.
- `EdgePanel` — no focus move; `role=dialog` + stack `Escape`/outside only.
- Autofocus/imperative `focus()` sites: `TextInputBar`, `LlmCatalogView` search/custom, `PersonalMemoryCommentPopover`, `PersonalMemoryStagingCard` (×3), `SessionPanel` rename/new-project, `VoiceCarousel` (×3), `DictationConfigDesk` recorder. Only `Drawer` restores focus.
- `Tooltip.tsx:106-107` shows on `onFocus` as well as hover — good. `SearchBar.tsx:76-77` keeps dropdown open 200ms past blur so mouse can click; results use `onMouseDown`.
- `tabIndex`: only `0` and `-1` used; **zero** positive values; **zero** roving tabindex; **zero** `accessKey`.
- **Zero** matches for: `onKeyUp`, `onKeyPress`, `useHotkeys` lib, `accessKey`, `FocusTrap`, `role=listbox/menu/tab/tablist/combobox/menuitem/option`, `aria-activedescendant`, `aria-selected`, `aria-controls`.

**Shortcut help UI:**
- `shared/components/help/HelpControlCard.tsx` renders `<kbd>` pill when `shortcut` set — display only, no binding.
- `data/helpCopy.ts:42-111` documents shortcuts that **do not exist as handlers**: `Space` hold-to-talk (`HOME_PAGE_HELP`), `M` mute (`:81`), `[ or ]` orbit (`HISTORY_PAGE_HELP :110`), arrow-keys orbit text. `data/homeCopy.ts:35,37` tooltips `Send (Enter)`, `Discard & Close (Esc)` (these two *do* exist in `TextInputBar`). Dictation `Alt+Space` rebinder refers to a backend/Tauri global, not a frontend handler.

### 0.2 Gap summary (click without keyboard parity)

> v2 note: this audit trail is unchanged; §1.1 marks which gaps are practical (in scope) vs skipped (calendar-grid, color-drag, camera/drag emulation, resize arrows, reorder keys).

1. **Dead `<div onClick>` card selects:** `SubModelCard` body (`SubModelCard.tsx:131`), `HistoryListView` session card (`HistoryListView.tsx:321`), `ActiveSessionHeader` breadcrumb (`ActiveSessionHeader.tsx:117`), wizard `ModelCategory` header/checkbox/rows (`ModelCategory.tsx:68,97,144`). All pointer-only.
2. **Pointer-drag controls with no keyboard path:** `OrbitCarousel` drag-rotate, `MemoryGraph` orbit + node/core picking (core-click → dossier drawer unreachable by keyboard), `RotaryKnob` dial drag (steppers OK), project `Reorder`/session HTML5 DnD, `HexColorPicker` (`react-colorful`, no text fallback), Drawer resize handle (`role=separator`, drag only).
3. **PTT has no keyboard path:** orb stage surface (`Home.tsx:196-203` `onPointerDown/Up/Leave` only) + Mic button (`Home.tsx:291-308` `onPointerDown/Up/Leave` only — a `<button>` that ignores `Enter/Space`).
4. **Hover-only content:** `ModelStatusOverlay` spec tooltips, capability chips (`cursor-help` + `Tooltip`), TitleBar update-card reveal (`group-hover`), EdgeNav tooltips (`group-hover`), `WelcomeStep` tray-demo callouts (`onMouseEnter` divs), dossier margin `Tooltip`s partially.
5. **Wizard sidebar pending steps** focusable but silent noop (no `disabled`/`aria-disabled`).
6. **Global `ArrowLeft/Right` page-nav** fires app-wide outside editables — will collide with every new widget-level arrow binding (carousels, knobs, sliders, tabs, calendar, graph, orbit). Must be scoped (see §1.4).
7. **Combobox/search dropdowns** have no arrow/Enter/Escape grid semantics (`SearchBar`, `MemorySessionRail` filter, `HistoryListView` search, `LlmCatalogView` search is closest with two-stage Escape but no arrow-to-result).
8. **`SessionContextMenu`** has no `role=menu`, no focus-on-open, no arrow nav, no submenu keys.
9. **`CalendarPicker`** day cells are Tab-focusable buttons but no calendar-grid arrows (`Arrow/Home/End/PgUp/PgDn`).
10. **Voice delete** uses blocking native `confirm()` — keyboard-operable natively but traps focus; needs custom confirm for parity polish.
11. **Memory comment trigger** requires mouse text selection — keyboard users cannot surface it.
12. **Help-documented `Space`/`M`/`[`/`]`** are fiction in the frontend — contract must either implement or remove the copy (contract implements, see §2).

---

## 1. Global principles (apply everywhere)

### 1.1 Practical parity rule (v2 — replaces full click parity)
Only *practical* pointer affordances get keyboard equivalents:
- Click/select/activate → `Enter` and/or `Space` (see §1.2 for which).
- Hover-reveal info → focus-reveal for practical surfaces (`Tooltip` already does this; extend to `ModelStatusOverlay`, TitleBar cards, EdgeNav labels). Capability-chip hover specs and WelcomeStep tray callouts stay hover-only (skipped as non-practical).
- Panel/drawer open-close → explicit globals (§1.6) + `Shift+Up/Down` (§1.7).
- Text-selection trigger (memory comment) → explicit focus + shortcut trigger (kept — practical).
- Explicitly skipped (no keyboard path required): color-field drag (`HexColorPicker`), calendar day-cell arrow-grid, orbit-ring drag rotation beyond card Tab/Enter, graph camera orbit/pan, knob-dial drag (steppers + numeric inputs cover it), Drawer resize-handle arrows, project drag-reorder (menu path covers moves).

### 1.2 Key-to-meaning table (normative)

| Key(s) | Meaning everywhere (unless a pattern below overrides) |
|---|---|
| `Tab` / `Shift+Tab` | Move through interactive elements in visual/DOM order. Never trap except inside true modal `Drawer` (future trap — see §8). Never use positive `tabindex`. |
| `Enter` | Activate focused control; submit focused form; select focused card/row/node; open focused session/dossier. In single-line inputs: submit. In multi-line textareas: see per-surface rule (comment popover `Enter`=save; staging edit `Cmd/Ctrl+Enter`=save). |
| `Space` | Toggle switches/checkboxes; activate buttons; hold-to-talk **only** when PTT surface armed (see §2). `Space` on a card = same as `Enter` (matches `VoiceRippleNode`, `ToggleTile`). Always `preventDefault` on `Space` activation to avoid scroll. |
| `Escape` | FILO dismiss: close topmost overlay/panel/drawer/menu/dropdown/recorder/edit-mode first; never destructive; never navigates routes. Owned by `overlayStack` for registered overlays; local `Escape` only for non-overlay edit states (inputs, rename, recorder, calendar popover). `Escape` in a text field clears-then-closes where a two-stage pattern is specified, else cancels. |
| `ArrowLeft/Right/Up/Down` | Move within the *current widget group* (tabs, carousels, grids, calendars, graphs, orbit, menus, preset groups). Never navigates pages when a widget claims arrows (see §1.4 scoping). |
| `Home/End` | First/last item in current group (tab strip, carousel, menu, calendar week, orbit ring, session list). |
| `PageUp/PageDown` | Larger jumps: prev/next day-window or month-window (History), drawer height steps, knob large steps. |
| `Delete/Backspace` | Delete/dismiss focused item **only** where a visible delete affordance exists (session row, voice, comment). Always arms confirm first — never instant-deletes. `Backspace` in text respects `PersonaCard` protected-tag guard. |
| `?` (`Shift+/`) | Open Help panel (§1.8). Outside inputs only. |
| `Ctrl/Cmd+K` or `/` | Focus page search where the page has one (Memory graph search, History list search, LLM catalog search). `/` only when not in an input. |
| `Ctrl/Cmd+Enter` | Save/commit in multi-line editors (staging comment/import/edit). |
| `Ctrl+M` | Toggle Monitoring popover (same as bottom-left Activity button). Outside inputs. `Escape` closes, focus restores to toggle. |
| `Alt+N` | Toggle Notifications right rail (same as bell button). Chosen over `Ctrl+N` — browsers reserve `Ctrl+N` for new-window and frequently refuse `preventDefault`. Outside inputs. |
| `Ctrl+S` | Toggle Sessions left rail (same as session-rail trigger). Calls `preventDefault` (browser Save). Outside inputs. No relation to Persona save (autosave — no `Ctrl+S` binding there). |
| `Shift+Up` | Open page drawer (§1.7): profiler (Home) / DetailPanel if a session is selected (History, else no-op) / dossier drawer (Memory) / all Settings cards (Settings = HubCenter open-all). Outside inputs. |
| `Shift+Down` | Close page drawer (§1.7): reverse of `Shift+Up`. `Escape` still closes topmost overlay first. |

### 1.3 Focus rules (normative)
1. Every overlay that appears (`Drawer`, `EdgePanel`, `SessionContextMenu`, `MemoryNodeTooltip`, monitor popover, calendar popover, search dropdowns) **moves focus into itself on open** and **restores focus to the trigger on close**. `Drawer` already restores — extend the pattern to `EdgePanel` + menus + tooltips + popovers.
2. Step/wizard transitions move focus to the new step heading (`tabIndex=-1` + `focus()`).
3. Destructive arms (delete confirm, restore-defaults arm, hotkey recorder) move focus to the confirm/save control and announce via `aria-live`.
4. Never leave focus on a removed element (session delete, voice delete, notification dismiss, comment delete): move to next sibling, else previous, else group header.
5. Visible focus ring on everything (`focus-visible:outline` already used in several places — standardize).
6. No positive `tabindex`. `tabIndex=0` for interactive cards/rows; `tabIndex=-1` for programmatic-only targets (drawer sheet, step headings, graph canvas mirror).

### 1.4 Arrow model — full-spatial in-page movement, page-nav only from EdgeNav (v2, normative)

**User decision:** keep `Tab` AND let all four arrows move around the page; page navigation fires only when EdgeNav (or effectively nothing) has focus.

**How one key set serves "so many things" (the mental model):** arrows never have a global meaning. Exactly one scope owns them at a time, decided by focus:
1. Focus inside a widget group (tab strip, list, menu, search dropdown, table/grid of buttons) → arrows move *within that group* (`data-arrow-nav` roots, `role=tab/slider/menu/listbox/gridcell/option/switch`).
2. Focus on `body`/non-interactive chrome or inside `EdgeNav` (`nav [data-edge-nav]`, `:focus-within`) → `ArrowLeft/Right` navigates pages (existing wrap-around order), `ArrowUp/Down` do nothing (reserved).
3. Focus on a lone button/input/card outside any group → arrows move focus spatially to the nearest focusable in that direction (see below); they never change routes.

**Contract:**
1. Replace the current app-wide page-nav guard (`ResponsiveLayout.tsx:84-125`, today only skips editables) with: page-nav fires on `ArrowLeft/Right` **iff** `document.activeElement` is `body`, OR inside `[data-edge-nav]`, OR is not inside any `button/input/textarea/select/[contenteditable]/[data-arrow-nav]/[role=tab/slider/menu/menuitem/listbox/option/gridcell/switch/dialog]`. Every other arrow press is in-page movement or widget movement — never a route change.
2. Widget groups keep their §9.1 arrows (tabs, lists, menus, dropdowns, preset pill rows, topology tabs, filter pills, notification tabs). Group roots set `data-arrow-nav` and `stopPropagation` after handling.
3. **Full-spatial fallback (new):** when focus is on a focusable element with no arrow-group owner, `ArrowUp/Down/Left/Right` moves DOM focus to the nearest visible focusable in that direction (bounding-rect distance, same-origin page only; skip `disabled`/`aria-hidden`/`pointer-events-none`). `Tab` order is unchanged and remains the SR ground truth; arrows are an accelerator, not a replacement. Skip entirely when focus is in `input/textarea/select/contenteditable` (caret keys win) — except `Escape` and combos with `Ctrl/Shift`.
4. Spatial move must never trap: if no candidate exists in that direction, keep focus (no wrap, no page change). Announce nothing (focus ring + SR name suffice).
5. `Drawer`/`EdgePanel`/`popover` open → arrows are scoped to the overlay first (same rule; overlay content wins over page content).
6. Document the precedence in a code comment at `ResponsiveLayout.tsx:84-125` when implemented: `EdgeNav-or-body → page-nav; arrow-group → widget; otherwise → spatial; editable → caret`.

**Conflicts answered:** yes, three real ones, all resolved by the precedence above — (a) page-nav vs widget arrows (EdgeNav/body gate fixes it), (b) caret arrows in inputs (editables always win), (c) `Space` PTT vs button activation (§2.1 keeps PTT only on Mic/orb focus). `Shift+Up/Down` never collides with arrows because the `Shift` modifier takes a separate branch and is ignored inside text inputs (would extend selection).

### 1.5 Screen-reader / ARIA minimum (normative)
- Tabs: `role=tablist/tab/tabpanel` + `aria-selected` (replaces bare `aria-pressed` on tab strips: `CategorySelector`, `ViewSelector`, `ModelsTopologyMap`, `SettingsTopologyMap`, dictation output tabs, persona tabs, profiler tabs, notification tabs, History pill).
- Switches: `role=switch` + `aria-checked` (already `ToggleTile`; add to `RealtimeToggleRow`).
- Menus: `role=menu/menuitem` + `aria-haspopup/aria-expanded` on trigger (`SessionContextMenu`).
- Comboboxes: `role=combobox` input + `role=listbox/option` dropdown + `aria-activedescendant` + `aria-expanded` (`SearchBar`, rail filter, history search, LLM search results if arrow-to-result added).
- Grids: skipped (CalendarPicker out of scope — no `role=grid` retrofit).
- Sliders: `role=slider` + `aria-valuemin/max/now/text` (`RotaryKnob` dial, Drawer handle as `separator` with keyboard = keep `separator` + add keys).
- Canvas/graph/orbit surfaces: `role=application` + `aria-label` + offscreen listbox mirror for SR (Memory graph, orbit ring).
- Status/live: keep `role=status aria-live=polite` (`StatusCapsule`); add `aria-live` to recorder value, save/commit toasts, delete arms, search result counts.

### 1.6 New global affordances (normative)
| Keys | Scope (all outside inputs unless noted) | Action |
|---|---|---|
| `?` (`Shift+/`) | Global | Open Help right rail on the current route's guide (§1.8). `Escape` closes, focus restores to trigger. |
| `Ctrl+M` | Global | Toggle Monitoring popover (same as Activity button). `Escape` closes. |
| `Alt+N` | Global | Toggle Notifications right rail (same as bell). `Alt` avoids the `Ctrl+N` new-window trap. |
| `Ctrl+S` | Global | Toggle Sessions left rail (same as rail trigger). `preventDefault` browser Save. |
| `Shift+Up` / `Shift+Down` | Global, per-page map (§1.7) | Open / close the page's bottom drawer (or all Settings cards). No-op where no valid target (e.g. History with no selection). Ignored inside text inputs. |
| `Ctrl/Cmd+K` or `/` | Per-page with search | Focus page search (Memory, History list, LLM catalog). |
| `Escape` | Global | FILO dismiss via `overlayStack` (extend registration to shortcut map overlay, monitor popover, search dropdowns when open, custom confirms). |

### 1.7 `Shift+Up` / `Shift+Down` page-drawer map (normative)
Same keys everywhere, per-page target (mirrors existing click handlers; no new state):
- **Home:** `Shift+Up` → `openProfiler()` (same as CPU/RAM HUD button `ResponsiveLayout.tsx:261-265`); `Shift+Down` → close profiler drawer (same as Drawer X/backdrop/`Escape`).
- **History:** `Shift+Up` → open `DetailPanel` drawer **iff** a session is already selected (same as clicking its card; user decision: no selection → no-op, never auto-pick). `Shift+Down` → close it (`setSelectedSession(null)`, same as X/backdrop/`Escape`).
- **Memory:** `Shift+Up` → open dossier drawer (same as core-click `handleCoreClick` `Memory.tsx:255-261`); `Shift+Down` → close it (same `onClose`, keeps pending-comments reopen flag).
- **Settings:** `Shift+Up` → open **all** domain cards (same as HubCenter `handleCenterClick` when none active — `DOMAINS.map(id)`); `Shift+Down` → clear all (same as HubCenter when any active). Compact `<1024px` renders all cards already — keys are no-ops there.
- **Monitoring:** no bottom drawer — keys are no-ops (popover uses `Ctrl+M`/`Escape`).
- Guards: ignored when focus is in `input/textarea/select/contenteditable` (would clash with text selection); `Shift+Down` never closes more than the page drawer (overlays still unwind via `Escape` first).

### 1.8 Help panel: inline shortcuts + header full-map toggle (normative)
User requirement: shortcuts live *with* their feature explanations (not a detached table), plus one header toggle showing the key→action map for every page.
- **Inline (default Help view, per route):** every `HelpControlItem` that has a keyboard equivalent sets `shortcut` (`HelpControlCard.tsx:5-13` already renders a `<kbd>` pill, `action` field stays unused). Copy rule: `shortcut` = the literal keys (`Space`, `M`, `Ctrl+M`, `Alt+N`, `Shift+Up`…), `outcome` describes the result. Route guides keep their current structure (`Home/History/Memory/SettingsHelpContent` + `helpCopy.ts` sections) — authors just fill in `shortcut` per control. No separate shortcut section.
- **Header toggle (new):** add a kbd-icon toggle button in the `HelpPanel` header row (`HelpPanel.tsx:80-92`, right side next to `routeBadge :89-91`). `aria-pressed` + `aria-label="Show all shortcuts"`. On: body swaps the route guide for a **full key→action map grouped by page** (Global, Home, History, Memory, Settings, Monitoring, Wizard, Rails) sourced from one central `SHORTCUTS` registry (single SSOT — inline pills and map render from the same entries so they can't drift). Off: restores the route guide (previous scroll/selection preserved). `Escape` exits map-first (back to guide), second `Escape` closes panel via stack.
- **Registry rule:** one `SHORTCUTS: {id, keys, label, route}` list owns all bindings; Help inline pills, header map, and tooltips (§1.9) all render from it. Adding a binding without a registry entry is a spec violation.

### 1.9 Tooltips show their shortcut (normative)
Industry-standard: hover (and focus) tooltips name the key.
- Every control with a keyboard equivalent appends it to its tooltip: `<Tooltip label="…">` or native `title=` becomes `"Base label (Key)"`, e.g. `Type Query (T)`, `Notifications (Alt+N)`, `Monitor (Ctrl+M)`, `Conversations (Ctrl+S)`, `Send (Enter)`, `Discard & Close (Esc)` (last two already exist in `homeCopy.ts:35,37` — keep as the template).
- `Tooltip.tsx:101-110` already shows on focus as well as hover — keep; add the same suffix to `EdgeNav` custom hover spans (`EdgeNav.tsx:44-46,76-78`, currently `group-hover` only — also mirror on `focus-visible` per §3.2) and to `GraphControlDock`/TopRightCluster/Settings labels at implementation time.
- Tooltip text and the §1.8 registry must match (same source string where feasible); a tooltip claiming a key that does nothing is a bug.

---

## 2. Global transport & voice controls (Home + anywhere PTT/mute exists)

These resolve the fiction in `helpCopy.ts` (`Space` PTT, `M` mute documented but unbound).

### 2.1 Push-to-talk (keyboard hold-to-talk) — normative
- **Where:** Home engaged + `interactionMode==="PTT"` + not paused/sleeping/error. Same gating as pointer PTT (`Home.tsx:110,290-308`).
- **Keys:** hold `Space` to record, release to send. `Escape` mid-hold cancels (same as pointer-leave cancel `handlePttCancel`).
- **How:** new keyboard handler on the PTT Mic button + orb stage wrapper: `onKeyDown(Space, repeat-ignored first press → handlePttStart)` / `onKeyUp(Space → handlePttStop)`. `preventDefault` to stop scroll/button re-click. Ignore auto-repeat (`e.repeat` → ignore). Disabled state ignores keys (matches `disabled={isPaused||isSleeping}`).
- **Pointer behavior unchanged.** Orb surface keeps pointer handlers; Mic button keeps `onPointerDown/Up/Leave`.
- **Announce:** `aria-live` PTT status (`RECORDING` etc. via existing `pttStatus`) — verify existing live region or add one; do not rely on color/pulse alone.
- **Conflict:** `Space` also activates focused buttons. Resolution: when PTT is armed and focus is on the Mic button or orb wrapper, `Space` = PTT hold (not click). Everywhere else `Space` = normal activation. Document in shortcut reference.

### 2.2 Mute + transport — normative
| Keys | Scope | Action (all mirror existing click handlers) |
|---|---|---|
| `M` | Home engaged (outside inputs) | Toggle mic mute (`toggleMicMute`, same as `TextInputBar` mic button). Matches documented `M` in help copy. |
| `Shift+M` (or `P`) | Home engaged | Toggle playback/speaker mute (`togglePlaybackMute`). `P` preferred if free; confirm no collision at implementation time. |
| `T` | Home engaged | Open text-input mode (`setTextModeOpen(true)`, same as Keyboard-icon button `Home.tsx:311`). When text mode open, focus is already in input. |
| `Enter` (in text input) | `TextInputBar` input focused | Submit (exists `TextInputBar.tsx:41-49` — keep). |
| `Escape` (in text input) | `TextInputBar` input focused | Discard & close (exists — keep). |
| `Space` then `E`? No — use explicit buttons | Engaged cluster | `Pause/Resume` (`pause()/resume()`), `Disengage` (`disengage()`), `Engage` (`engage()`), error `Reconnect` (`resume()`) remain `Tab → Enter/Space` buttons (already keyboardable `Home.tsx:259,275,322,350` — no new keys; add to shortcut reference as "Tab to control, Enter to press"). |
| `Up/Down` | Dialogue rail scroll container | Scroll transcript. Requires making the rail focusable (`tabIndex=0`, `aria-label="Conversation transcript"`) — currently `div onScroll` with no tab stop (`Home.tsx:173-176`). `DialogueBubble` Read more/less stays a button (exists). |

---

## 3. Global chrome (TitleBar, EdgeNav, rails, TopRightCluster, dock)

### 3.1 TitleBar — normative
- Minimize/Maximize/Close (`TitleBar.tsx:223,230,237`): already `<button>`s — keep. No new keys.
- App-update pill (`:151` hover-reveals release-notes card): make card focus-reveal (`onFocus/onBlur` mirror `group-hover`, same as `Tooltip` pattern) + inner Copy-command button already keyboardable (`:172`). `Escape` dismisses card (register as overlay or close on blur — implementer's choice, must be `Escape`-dismissable).
- Model-update pill → Manage Models (`:187,211`): inner `navigate('/settings?tab=models')` button already keyboardable — keep + focus-reveal card.

### 3.2 EdgeNav + page navigation — normative (v2)
- `EdgeNav.tsx:24,58` `NavLink`s: keep native Tab/Enter. Add `data-edge-nav` on the `<nav>` root (new scoping hook for §1.4) + focus-visible tooltip (currently `group-hover` only `:44,76` — mirror on `focus-visible`) with shortcut suffix where applicable (§1.9).
- Page cycle keeps routes/order/wrap/close-monitor-popover; **only the trigger changes**: `ArrowLeft/Right` navigates iff focus is in EdgeNav or on body/nothing-focusable (§1.4). All other focus → in-page/ widget arrows.
- Compact-only Monitoring `NavLink` (`lg:hidden`): no keyboard change.

### 3.3 Session rail trigger, ActiveSessionHeader, TopRightCluster — normative
- Session rail trigger (`ResponsiveLayout.tsx:190` `<button data-edge-trigger>`): keep Tab/Enter. Add `aria-expanded` already present — verify `aria-controls` points at rail. Global `Ctrl+S` toggles the same rail (§1.6); tooltip reads `Conversations (Ctrl+S)` (§1.9).
- `ActiveSessionHeader.tsx:117` breadcrumb `motion.div onClick`: **must become keyboardable** — convert to `<button>` (preferred) or add `role=button tabIndex=0` + `Enter/Space → onOpenPanel` (copy `SessionPanel.tsx:168-174` pattern).
- `TopRightCluster.tsx:43,60,84` Temporary-chat / bell / help: already `<button>`s with `aria-pressed/expanded` — keep. No new keys beyond Tab/Enter/Space. Disabled-while-other-open states must use real `disabled` (verify) so SR announces.

### 3.4 Bottom dock (Monitor toggle, CPU/RAM HUD, ModelStatusOverlay, RestoreDefaults) — normative
- Monitor toggle (`ResponsiveLayout.tsx:242`), CPU/RAM HUD (`:261`): keep buttons. `Escape` closes popover (exists via `useOverlay` + page-nav handler closes monitor on page move — keep). Global `Ctrl+M` toggles the popover (§1.6); tooltip reads `Monitor (Ctrl+M)` (§1.9). `Shift+Up` on Home opens the profiler drawer (same as HUD button, §1.7).
- `ModelStatusOverlay.tsx:98,129,159` chips (`div cursor-help group` hover-only): make focusable (`tabIndex=0`) + focus-reveal tooltip (same content as hover). `Enter/Space` optional (no action — info only; focus-reveal suffices for parity since hover has no action either).
- `RestoreDefaultsButton.tsx:27`: keep two-tap arm/confirm. Add `Escape` cancels armed state; announce arm via `aria-live` (label swap alone is insufficient).

### 3.5 EdgePanel + Drawer (hosts) — normative
- `EdgePanel.tsx:103,126` close X: keep. `Escape`/outside via stack: keep. **Add:** focus-into-panel on open + restore-to-trigger on close (copy `Drawer.tsx:95-104` pattern — currently missing on `EdgePanel`).
- `Drawer.tsx:187,235` close X/backdrop/`Escape`: keep. Resize handle (`:211` `role=separator` drag + double-click expand): add `Up/Down` (±5% height, `Shift` = min/max), `Enter/Space` toggle expand. Focusable (`tabIndex=0`) with `aria-label` + `aria-valuenow` (height %).
- Future: focus trap inside modal `Drawer` (Tab wraps). Listed as Phase-2 hardening — not required for click parity v1, but do not regress the existing focus-restore.

---

## 4. Home (`pages/Home.tsx`)

| # | Element (file:line) | Click today | Keyboard contract |
|---|---|---|---|
| H-1 | Restore-error Dismiss (`Home.tsx:131` button) | Clears toast | Keep Tab/Enter. Add `Escape` dismisses toast when focused. |
| H-2 | StatusCapsule / Temporary badge / governor pill (`:143-165`, `role=status`) | Display only | No keys (correct). Keep `aria-live`. |
| H-3 | Dialogue rail scroll (`:173` div onScroll) | Mouse/touch scroll; auto-follow | Make `tabIndex=0` + `aria-label`; `Up/Down/PageUp/PageDown/Home/End` scroll. ResizeObserver auto-follow unchanged. |
| H-4 | `DialogueBubble` Read more/less (button, `aria-expanded`) | Expand/clamp | Keep (already full). |
| H-5 | Orb PTT surface (`:196` div pointer-only) | Hold-to-talk when `isPttActive` | Add §2.1 `Space` hold-to-talk on wrapper (`tabIndex=0` when `isPttActive`, `-1` otherwise) + `aria-label="Hold Space to talk"`. Pointer handlers unchanged. |
| H-6 | PTT Mic button (`:291` button + pointer handlers) | Hold-to-record | Add §2.1 `Space` keydown/up hold (ignore repeat). Keep pointer handlers. `Enter` on this button must also start/stop for non-hold users? No — `Enter` performs momentary toggle (start on keydown, stop on keyup same as Space) OR document "use Space hold". Implementer picks one and documents in shortcut reference; Space-hold is normative, Enter behavior must be documented either way. |
| H-7 | Pause/Resume (`:275`), Text toggle (`:311`), Disengage (`:322`), Engage (`:350`), Reconnect (`:259`) | Click actions | Keep Tab/Enter/Space buttons. Shortcut reference lists `T` (text), `M` (mic), §2.2. No per-button hotkeys (avoid single-key collisions while typing). |
| H-8 | `SessionPanel` in left `EdgePanel` | See §10 | See §10. |
| H-9 | `TextInputBar` (when open) | See §10 (already full) | Keep `Enter`/`Escape`/autofocus. Add `Up` recalls last submitted text (nice-to-have, Phase 2). |

Home sub-view matrix (all covered above): `idle / engaged / paused / sleeping / error × text-mode`.

---

## 5. History (`pages/History.tsx`)

Navigation: `DAY ⇄ MONTH` pill + `ViewSelector` tabs; orbit (desktop) vs `HistoryListView` (narrow); month→day drill; day-window Prev/Next; session → `DetailPanel` drawer; stage click deselects.

### 5.1 Orbit view — normative (practical subset)
Practical: orbit cards stay Tab-focusable with existing `Enter/Space` select (`VoiceRippleNode` pattern — keep). Container arrows rotate only if cheap at implementation time; otherwise Tab-through-cards is the contract (drag-rotation parity explicitly skipped as non-practical).
| Keys | Scope | Action |
|---|---|---|
| `Enter/Space` | Focused orbit card | Open session (`onSelect`, same as click). |
| `Delete` | Focused card (when delete affordance visible) | Arm 2-step delete; `Enter` confirms, `Escape` cancels. Focus moves per §1.3. |
| `Escape` | Detail drawer open | Close drawer (exists via `Drawer`/stack — keep). |
| `[` / `]` | History page outside inputs | Rotate ring (matches documented help copy) — only if container-arrow support ships; else omit and remove the copy claim in the same PR. |

`CentralClockNode` Prev/Next (`:187,205` buttons): keep Tab/Enter (no `PageUp` addition — non-practical). DAY/MONTH pill (`:233,247`): keep buttons; `Left/Right` moves + selects only if tablist work is already being done for §9.1, else Tab/Enter suffices.
`ViewSelector.tsx:27` Day/Month tabs: Tab/Enter suffices; arrows optional.

### 5.2 List view (`HistoryListView`) — normative (practical subset)
- Session card (`:321` **dead `div onClick`**): give the `VoiceRippleNode` treatment — `role=button tabIndex=0`, `Enter/Space → onSelect`. Highest-value History fix (list-view keyboard users currently cannot open sessions at all). Kept.
- Prev/Next day chevrons (`:191,201`), search input + clear (`:217,225`), filter pills (`:249`), calendar funnel toggle (`:270`): keep native Tab/Enter. `Left/Right` across filter pills only if §9.1 tab work is in flight; not required.
- `CalendarPicker.tsx`: **skipped (non-practical).** Day/month buttons stay Tab/Enter-activatable as today; no `role=grid`, no calendar arrow/`Home`/`End`/`PgUp`/`PgDn` contract. Rationale: no UX demand for keyboard date-grid navigation.
- List per-row Delete → Confirm/Cancel (`:365,372,381` buttons): keep + `Delete` key arms focused row's delete; focus moves per §1.3.
- `DetailPanel.tsx:138,177,200` Compact/Retry/Load-older: keep buttons. `Shift+Up/Down` opens/closes the drawer per §1.7 (open only with a selection, else no-op).

---

## 6. Memory (`pages/Memory.tsx`)

### 6.1 Graph canvas (`MemoryGraph.tsx:254`) — normative (practical subset)
Canvas container is pointer-only today. Practical contract only: Tab reaches the canvas; `Enter` opens the dossier (same as core click); `Escape` deselects/closes tooltip. Skipped as non-practical: arrow-to-arrow node hopping with camera follow, `0/+/-/S/R` view aliases, SR listbox mirror, `selectModeEnabled` keyboard gating (pointer behavior unchanged).
- Container: `tabIndex=0`, `role=application`, `aria-label` naming the `Enter`/`Escape` actions above.
- `GraphControlDock` buttons (§6.2) remain the keyboard path for recenter/zoom/refresh/select-mode (Tab/Enter — already full).

### 6.2 Graph chrome — normative
- `GraphControlDock.tsx:35,51,69,90,101` Recenter/Select/Refresh/Zoom±: keep buttons (already full). Shortcut reference lists `0/S/R/+/-` aliases above.
- `MemoryLegendOverlay.tsx:61,104` 6 category filters + Clear: keep buttons + `aria-pressed`; add `Left/Right/Up/Down` across the 3×2 grid (row-aware), `Escape` clears filter.
- `SearchBar.tsx:71,82,109`: combobox contract — input `role=combobox aria-expanded aria-activedescendant`, dropdown `role=listbox`, rows `role=option`; `Down` opens/focuses list, `Up/Down` move, `Enter` selects (same as `onMouseDown` select + `flyToNode`), `Escape` clears-then-closes (two-stage like `LlmCatalogView` search). Result count via `aria-live`. (Fixes current Tab-only + 200ms-blur-hack path; keep `onMouseDown` for pointer.)
- `MemorySessionRail.tsx:164,210,248,296,304,343`: keep buttons/inputs; accordions add `aria-expanded`; add `Up/Down` between sessions/facts, `Left` = back from drill.
- `MemoryNodeTooltip.tsx:78`: keep X + stack `Escape`/outside. **Add:** focus tooltip on open, return focus to node on close (§1.3).
- Session-rail trigger (`Memory.tsx:512`): keep button.

### 6.3 Dossier drawer + staging (`PersonalMemoryStagingCard`, `PersonalMemoryCommentPopover`) — normative
- Drawer chrome (close X, backdrop, `Escape`, Consolidate-Now `:653`, Copy `:730`): keep buttons.
- Idle hub Comment/Import/Edit (`:215,232,249`): keep buttons. Optional `1/2/3` quick-pick when hub focused (Phase 2).
- Comment mode Regenerate/Cancel (`:153,163`), per-card Edit/Delete (`:301,313`), Clear-all (`:384`), Import/Edit Save&Commit/Cancel (`:177,186`): keep buttons. Textareas native. Keep `Cmd/Ctrl+Enter` save + `Escape` cancel in comment editor (exists).
- Comment popover anchor (`MessageSquarePlus :103`): keep button once visible. **Add keyboard surfacing:** when dossier text is focused, `Ctrl/Cmd+M` (or documented `C`) surfaces the comment trigger at the selection/caret (same outcome as mouse selection + trigger). `Enter`/`Shift+Enter`/`Escape` in popover textarea unchanged (exists). Anchor needs real `aria-label` (today only `title`).
- Dossier margin comment icons (`Memory.tsx:778`): keep buttons (already keyboardable).
- Disabled `Save & Commit` states (`isSaving/isCommitting` + `pointer-events-none`): announce via `aria-disabled` + `aria-live`, keep focus stable.

---

## 7. Settings (`pages/Settings.tsx`)

Navigation: desktop radial hub (6 `RadialNode` buttons ⇄ domain cards in grid slots; `HubCenter` clears) + per-card footer (Apply&Reload/Discard or key-blocker or autosaved toast); mobile vertical list + sticky header (commit/discard/restore).

### 7.1 Hub + card chrome — normative
- 6× `RadialNode` (`RadialHub.tsx:32` buttons): keep Tab/Enter. Add `Left/Right` (or 4-arrow by orbit geometry — implementer picks, document) to move between nodes; `Enter` toggles card. `HubCenter` collapse: keep button.
- `Settings.tsx:117-125` window `Escape` pops card: keep. Ensure it composes with overlay-stack `Escape` (capture first) — no double-pop.
- `SettingsCardWrapper.tsx:138,151,159` Apply&Reload/Save/Discard: keep buttons. Mobile header commit/discard/restore (`Settings.tsx:272,289,305`): keep.
- `RestoreDefaultsButton`: add `Escape` cancels armed confirm + `aria-live` arm announcement (§3.4).

### 7.2 Persona (`PersonaCard`) — normative
- Modular/Realtime + Edit/Preview `SegmentedControl`s: add radiogroup + `Left/Right` (see §9.1 pattern).
- Prompt `textarea :336`: keep native + protected-tag `Backspace/Delete` guard (exists). Add `Ctrl/Cmd+S` = commit/blur-save; `Escape` blurs (does not discard — autosave model; document it).

### 7.3 Interaction (`InteractionCard` family) — normative
- `TriggerModeCard`/`PipelineModeCard` (`ToggleTile`): already full — keep.
- `CategorySelector` STT/LLM/TTS tabs: add `role=tablist/tab` + `aria-selected` + `Left/Right`.
- `ProviderSelectorView` Embedded/Server/Cloud cards: add `Left/Right` across the 3-up; `Enter` selects+drills (same as click); breadcrumb Back stays a button.
- `LlmConfigDesk`/`RealtimeConfigDesk` remote URL/key/path inputs + health pill: keep native inputs; health pill is status-only (add `aria-live`, no keys).
- Cloud/realtime `CarouselSelector` ‹ ›: add container `Left/Right` (see §9.1).
- Dictation output-mode tabs (Paste/Clipboard/Tray): add tablist + `Left/Right`.
- Dictation hotkey recorder (`DictationConfigDesk.tsx:61-101`): already the best keyboard implementation in Settings — keep exactly (`Escape` cancel, `Enter` save, live combo in `<kbd>`, `aria-live`). No change.
- `RotaryKnob` dial: slider contract (see §9.1). Steppers/presets already buttons — keep.

### 7.4 Models (`ModelsCard` family) — normative
- Model/Settings `SegmentedControl`: radiogroup + arrows (verify disabled Settings tab is skipped).
- `ModelsTopologyMap` 5 pipeline tabs + `SettingsTopologyMap` subtabs: add `role=tablist/tab` + `aria-current/selected` + `Left/Right`; skip `disabled` subtabs; dirty dots stay visual (`title` → also `aria-label`).
- **`SubModelCard` body (`SubModelCard.tsx:131` dead `div onClick`)**: highest-value Settings fix. Add `role=option tabIndex=0 aria-selected`, `Enter/Space` = select (same gating as click: only if downloaded && !active), `Delete` = arm delete-confirm, `Escape` = cancel confirm. Inner Download/Delete/Confirm-Cancel/Lock buttons keep `stopPropagation` + native keys. Info `group-hover` specs popover → convert to focus-reveal (`Tooltip`/button pattern) so keyboard users get specs.
- `AuxiliaryWorkspace` cards (`onSelect={() => {}}` noop): make non-selectable cards **non-focusable** (`tabIndex=-1`, no `role`) so keyboard users don't land on dead cards. Buttons inside stay live.
- `LlmCatalogView` remote cards (`role=button tabIndex=0` + `Enter/Space` exists): keep. Add `Up/Down/Left/Right` grid nav across cards; `/` focuses search (see §1.6); keep two-stage `Escape` in search + `Enter` apply/`Escape` close in custom input. Benchmark/Re-probe buttons keep + `stopPropagation`. Capability hover chips → focus-reveal.
- Param desks (VAD/STT/LLM/TTS preset 2×2 grids + custom numeric inputs; WorkingMemory budget; PersonalMemory depth/cutoff): keep buttons/inputs; add `Left/Right` between presets, `Up/Down` on focused custom input = ±1 step clamped to documented ranges (thr 5–95%, silence 100–3000ms, onset 16–1000ms, gate 0.001–0.09, throttle 50–1500ms, tokens 50–128k, temp 0–2, context 8192–ceiling), value announced via `aria-live`.
- `TtsVoiceManager` region cyclers: add container `Left/Right`; `VoiceCarousel` voice region: `Left/Right` = prev/next voice (wrap, same as ‹ › buttons); `Escape` closes search/clone mode; custom-voice `Delete` = arm delete (replaces native `confirm()` in Phase 2 — see §10).
- `RemoteServerSetup` host/port/key + Deploy: keep native; progress/log is `aria-live` status (add if missing).
- `AppearanceCard` theme segmented: keep Tab/Enter buttons (no radiogroup-arrow requirement — practical Tab parity suffices). `HexColorPicker` (`react-colorful`, pointer-drag only): **skipped (non-practical).** No hex-input fallback, no arrow-nudge contract; pointer-only stays.
- `RealtimeCard`: `RealtimeInput` native; temperature `input[type=range]` already arrow-capable (keep, verify label linkage); `RealtimeToggleRow` button → add `role=switch aria-checked` (keys already work); voice prev/next + dots → container `Left/Right` (dots display-only).
- `UnderlineInput`/`ApiKeyField`/`CarouselSelector` shared primitives: keep native; ensure Test-Connection button is Tab-reachable (it is) + status `aria-live` (add).

### 7.5 WorkingMemory / PersonalMemory / Appearance toggles — normative
- All `ToggleTile` (`role=switch`): keep — no work.
- Manual/Daily segmented + time text input + Depth/Cutoff subtabs + presets: same tab/preset contracts as above. Time input commits on blur (keep) + `Enter` commits (add).

---

## 8. Monitoring (`pages/Monitoring.tsx` — full page + bottom-left popover)

Same `containerContent` both modes; popover adds `role=dialog` + `Escape`/outside-close.

| # | Element | Click today | Keyboard contract |
|---|---|---|---|
| M-1 | UNLOAD ALL / LOAD MODELS (`:244` buttons) | `stopEngine()`/`launchEngine()` | Keep Tab/Enter. Announce toggling state via `aria-live` (verify). |
| M-2 | Popover X (`:267`) | `onClose` | Keep + stack `Escape` (exists). Return focus to Monitor toggle (§1.3). |
| M-3 | `MetricCarousel` chevrons (`:104,138`) + dots (`:150`) | Page ±1 (wrap), jump to page | Keep buttons. Add container `Left/Right` = page ±1 (wrap, same as chevrons); dots add `aria-current`. `data-arrow-nav` + `stopPropagation` (page-nav scoping). |
| M-4 | Metric cards / `LiquidChamber` canvas + pills | Display only | No keys (correct). `aria-hidden` decorative where applicable. |

---

## 9. Shared widget patterns (normative — implement once, apply everywhere)

### 9.1 Arrow-nav family — practical subset (tabs, lists, menus, dropdowns, pill rows)
- Root: `data-arrow-nav`, `role=tablist/radiogroup/group` as appropriate (only where tablist work is practical — tab strips, filter pills, notification tabs, topology tabs, carousel prev/next groups).
- `Left/Right` (horizontal) move focus + select (automatic activation). `Up/Down` in vertical lists (session rows, notification items, device rows, menu items). `Home/End` first/last. Skip `disabled`. `stopPropagation` on handled arrows (feeds §1.4 precedence).
- Applies to: `SegmentedControl`, `CategorySelector`, `ViewSelector`, `ModelsTopologyMap`, `SettingsTopologyMap`, `CarouselSelector` family, `MemoryLegendOverlay` (flat list nav is fine — row-aware optional), `MetricCarousel`, dictation output tabs, persona tabs, profiler tabs, notification tabs, History filter pills. Calendar grid explicitly excluded (§5.2 skip).

### 9.2 Slider/knob family — practical subset (native inputs + steppers only)
- Native `input[type=range]` (realtime temperature) already arrow-capable — keep, verify label linkage. Custom numeric inputs + preset pill buttons + `RotaryKnob` −/+ steppers: keep Tab/Enter/typing as today. No `role=slider` retrofit, no `PageUp`/`Home`/`End` dial contract, no Drawer-handle arrow contract (drag stays pointer-only).
- Value announcement via existing visible badges where present; add `aria-live` only where a badge already exists.

### 9.3 Card-select family (`SubModelCard`, `HistoryListView` rows, wizard `ModelCategory` rows, `ActiveSessionHeader`)
- Template: `VoiceRippleNode.tsx:48-59` / `LlmCatalogView.tsx:366-392` — `role=button|option tabIndex=0`, `Enter/Space → onSelect` with `preventDefault` on Space, `focus-visible` ring, `aria-selected/current` where applicable.
- Wizard checkbox rows: real semantics — header `role=button`, category/model toggles `role=checkbox aria-checked` (or native `<input type=checkbox>` preferred), `Space` toggles, required rows `aria-disabled` (not focusable-silent-noop).

### 9.4 Menu family (`SessionContextMenu`)
- Trigger: `aria-haspopup=menu aria-expanded`; `Enter/Space/Menu-key/Shift+F10` opens (add `Menu`-key support — currently only click on ⋮ button `SessionPanel.tsx:276`).
- Menu: `role=menu`, items `role=menuitem`, first item autofocused on open, `Up/Down` move, `Right/Enter` enter submenu (Move-to-Project), `Left/Escape` back/close, `Enter` activate, `Delete` on session item arms delete-confirm. Register with `overlayStack` (replace hand-rolled listener) or keep listener but add focus move + roles. Return focus to trigger on close.

### 9.5 Combobox family (Memory `SearchBar`, rail filter, History search, LLM catalog search)
- See §6.2 combobox contract. All get `role=combobox/listbox/option`, `Down` opens, arrows move, `Enter` selects, `Escape` clears-then-closes, count announced.

### 9.6 Calendar-grid family — SKIPPED (non-practical, see §5.2)

### 9.7 Canvas family — practical subset only (see §6.1 + §5.1)

### 9.8 Color family — SKIPPED (non-practical, see §7.4)

---

## 10. Rails, panels, menus, text input (normative)

### 10.1 SessionPanel (Home left rail)
- Rows (`:165` `role=button tabIndex=0`): keep `Enter/Space` select. Add `Up/Down` between rows (listbox-style; Tab keeps working). `aria-current` already present — keep.
- Pin (`:260`), ⋮ trigger (`:276`), + New (`:677`), view toggle (`:692`), retry (`:721`), new-in-project (`:440`), create-project (`:818`), rename/create inputs (`:207,839`): keep native + existing `Enter/Escape`/blur semantics.
- Context menu: `Menu` key / `Shift+F10` / `Ctrl+Enter` on a focused row opens `SessionContextMenu` (same as ⋮ click). DnD parity: session→project move already exists as menu action (`Move-to-Project ›` submenu) — keyboard users use the menu instead of drag; **document this as the DnD alternative** (no separate drag emulation). Project `Reorder` parity: `Ctrl+Up/Down` (or `Ctrl+Left/Right`) on a focused project header moves it (same order state as Framer `Reorder`); announce new position.
- Project headers (`:408`): keep `Enter/Space` expand. Drag-handle via `onPointerDown` stays pointer-only (keyboard uses `Ctrl+Arrow` above).

### 10.2 NotificationPanel (right rail)
- Tasks/Updates tabs (`:429,461`), Dismiss-all (`:495`), per-item Dismiss (`:238`), primary action (`:283`), View › (`:305`): keep buttons.
- Add: `Left/Right` across tabs (tablist); `Up/Down` between items in a group; `Delete` dismisses focused item (same as its Dismiss X); `Enter` triggers focused item's primary action. Focus moves to next item on dismiss (§1.3).
- Globals: `Alt+N` toggles the rail (same as bell, §1.6); tooltip on bell reads `Notifications (Alt+N)` (§1.9).

### 10.3 HelpPanel (right rail — v2: inline shortcuts + header map toggle)
- Guide content stays per-route with shortcuts inline: each practical control's `HelpControlItem.shortcut` is set and rendered as the existing `<kbd>` pill next to its explanation (`HelpControlCard.tsx:45-49,86-90`). No detached shortcut table in the guide body.
- Header toggle button (§1.8): kbd-icon, `aria-pressed`, in `HelpPanel.tsx:80-92` next to `routeBadge`; swaps guide ↔ full key→action map (all pages, from `SHORTCUTS` registry); `Escape` exits map first, then panel.
- Globals: `?` opens Help on the current route's guide (§1.6); tooltip on Help trigger reads `Help & guide (?)` (§1.9).
- `HelpOrbVisualizer` mood buttons, `SettingsHelpContent` category tabs, `HelpInteractionDiagram` mode tabs: keep buttons; tablist arrows only where §9.1 work is already in flight.

### 10.4 TextInputBar (already full — lock in, don't regress)
- Keep: autofocus, `Enter` submit, `Escape` close, mic/playback mute buttons, Send/Discard buttons, disabled-empty Send. Add nothing normative (optional `Up` history in Phase 2).

### 10.5 Overlays-to-come (normative for new work)
- Shortcut full-map (Help header toggle, §1.8): registered overlay content, focus-in + restore, `Escape` exits map first (then panel).
- Custom confirm dialogs (only if voice/session delete migrates off native `confirm()` — optional): `role=alertdialog`, focus confirm on open, `Enter` confirms, `Escape` cancels, restore on close.

---

## 11. Wizard (`wizard/WizardRoot.tsx` + steps)

Linear XState: welcome → checking → downloading → audio → testing → completed.

| # | Element | Click today | Keyboard contract |
|---|---|---|---|
| W-1 | Sidebar step buttons (`WizardRoot.tsx:135`) | `GO_TO` if reached, silent noop otherwise | Pending steps get real `disabled` + `aria-disabled` (no focusable noop). Reached steps keep Tab/Enter jump. |
| W-2 | Step transitions | View swaps, focus stays behind | Move focus to new step heading on advance/back (`tabIndex=-1` + `focus()`). `Escape` never closes wizard (no-op or moves within step, never exits flow). |
| W-3 | Welcome substep chevrons + dots + Begin CTA (`WelcomeStep.tsx:38,45,295,325`) | Paging + next | Keep buttons. Add `Left/Right` on substep pager (same §9.1). Tray-demo hover zones (`:158-209` mouse-only divs) → `tabIndex=0` + focus-reveal callouts (same content). |
| W-4 | `ModelCategory` header/checkbox/rows (`ModelCategory.tsx:68,97,144` — worst wizard offender, all dead divs) | Expand/collapse, toggle category, toggle model | Card-select + checkbox pattern (§9.3): header `role=button tabIndex=0` (`Enter/Space` expand, `Left/Right` collapse/expand); category + model toggles `role=checkbox aria-checked` (or native inputs), `Space` toggles; required rows `aria-disabled`. Keep auto-`scrollIntoView` on expand (also on keyboard expand). |
| W-5 | Catalog Back / Begin Synchronization / Reload / Continue / Return-to-selection (`ModelSetupStep.tsx:259,292,295,375,410,420`) | Flow actions | Keep buttons. Error region `role=alert`. |
| W-6 | `WizardFooter` Back/Skip/Next (`WizardFooter.tsx:47,56,64`) | Flow actions | Keep buttons + disabled gating. Optional global `Enter` = Next when valid and focus not in input (Phase 2); `Alt+Left` = Back (Phase 2). Error banner `role=alert`. |
| W-7 | Audio device rows (`AudioSetupStep.tsx:146` buttons) | Select + engine restart | Keep buttons. Add `Up/Down` between devices (nice-to-have; Tab suffices for parity). Energy meter display-only. |
| W-8 | LiveTest Try-again/Skip/Confirm (`LiveTestStep.tsx:105,223`), Completed Start (`CompletedStep.tsx:55`) | Flow actions | Keep buttons. Transcript-gated Continue stays disabled until transcript (announce via `aria-live`). |

---

## 12. Conflicts & decisions (normative, v2)

1. **Page-nav vs in-page arrows:** solved by §1.4 precedence — EdgeNav-or-body → page-nav; arrow-group → widget; lone focusable → spatial move; editable → caret. Widget roots set `data-arrow-nav` + `stopPropagation`. Without this gate, in-page arrows are unshippable.
2. **`Space` PTT vs `Space` activation:** PTT hold wins only when (a) Home engaged + PTT mode + not paused/sleeping/error, and (b) focus is on the Mic button or orb wrapper. Everywhere else `Space` activates. Help inline + tooltip document this.
3. **Single-letter keys vs typing:** `M`/`T`/`?`/`/`/`[`/`]` fire only outside inputs/textareas/selects/contenteditables (same editable guard as page-nav). `Ctrl+M`, `Alt+N`, `Ctrl+S`, `Shift+Up/Down` also ignored inside text inputs (except `Ctrl+S` still `preventDefault`s globally to block browser Save — but only toggles the rail when outside inputs).
4. **`Ctrl+S` vs browser Save:** always `preventDefault`; rail toggles only outside inputs. No Persona-save binding exists (autosave), so no in-app collision.
5. **`Alt+N` over `Ctrl+N` (user decision):** browsers reserve `Ctrl+N` for new-window and may ignore `preventDefault`, especially in browser builds. `Alt+N` is conflict-free and still one-handed. Tooltip + Help show `Alt+N`.
6. **`Shift+Up/Down` vs text selection:** ignored inside editable focus. `Shift+Down` never cascades past the page drawer (overlays unwind via `Escape` first). History `Shift+Up` with no selection is a deliberate no-op (user decision) — never auto-picks.
7. **`Enter` in multi-line editors stays inconsistent by design** (popover `Enter`=save vs staging `Cmd/Ctrl+Enter`=save). Keep both; Help documents per-surface behavior. New editors follow staging (`Cmd/Ctrl+Enter` save).
8. **Native `confirm()` for voice delete stays** (keyboard-operable natively; custom dialog deferred as non-practical).
9. **Help copy fiction (`Space`/`M`/`[`/`]`):** contract implements the practical subset (§2, §5.1) — copy stays where implemented; any descoped claim (e.g. orbit `[`/`]` if container arrows don't ship) must be edited in the same PR (no spec-code drift per workspace §4.3).
10. **Skipped = skipped:** calendar-grid, color-drag, camera/drag emulation, resize-handle arrows, reorder-via-keys are explicitly out of scope and must not block acceptance (§14).

---

## 13. Implementation plan (practical batches — each shippable, each testable)

> Batch 0 first (arrow precedence + globals unblock everything else). Skipped work (§1.1) never blocks acceptance.

- **Batch 0 — Arrow precedence + globals + tooltip suffix.** Gate `ResponsiveLayout` page-nav to EdgeNav/body (§1.4, add `data-edge-nav`); add full-spatial fallback mover; add `Ctrl+M` / `Alt+N` / `Ctrl+S` / `?` / `Shift+Up/Down` (§1.6–§1.7) with editable guards + `preventDefault` on `Ctrl+S`; append `(Key)` suffixes to practical tooltips (§1.9). Acceptance: arrows never change routes outside EdgeNav/body; every global toggles + restores focus; tooltips name their keys.
- **Batch 1 — Dead-card triage (highest practical value).** `SubModelCard` body, `HistoryListView` rows, `ActiveSessionHeader`, wizard `ModelCategory` rows (§9.3). Pending wizard steps get real `disabled`. Acceptance: every practical card/row reachable + activatable; no focusable noop.
- **Batch 2 — Home voice parity.** §2.1 PTT hold, `M`/`T`/`Shift+M`, dialogue rail tab stop + scroll keys, `Shift+Up` profiler open/close. Acceptance: engage → talk via Space → text via T → mute via M → profiler via Shift+Up — all keyboard-only.
- **Batch 3 — Practical arrow-nav rollout.** §9.1 subset (tab strips, lists, menus, dropdowns, pill rows, notification tabs) + `Up/Down` in session rows/notification items/device rows. Acceptance: arrows move + select in every covered group; page-nav never double-fires.
- **Batch 4 — Help inline + header map + rails/drawers.** Fill `shortcut` on practical `HelpControlItem`s; build header toggle + `SHORTCUTS` registry + full key→action map (§1.8); `EdgePanel` focus-in/restore; History `Shift+Up/Down` (selection-gated), Memory dossier `Shift+Up/Down`, Settings `Shift+Up/Down` open-all/clear-all. Acceptance: shortcuts explained inline; header map matches implementation; every drawer opens/closes via keys with focus restore.
- **Batch 5 — Search/menu/focus polish.** Combobox `Down`/`Enter`/`Escape` on Memory/History/LLM searches (no full ARIA-grid retrofit required beyond `aria-expanded` + count), `SessionContextMenu` `Menu`-key open + `Up/Down` + `Escape`, notification `Delete`/`Enter`, `RestoreDefaults` live region, wizard heading focus. Acceptance: search-to-select, menu-operate, dismiss-focus-move all keyboard-only.

**Testing per batch:** frontend-only (isolated `cargo nextest` unaffected); `pnpm build` green; manual keyboard walkthrough per route (Tab order matches visual order; every practical click has a key; `Escape` unwinds one level; `Shift+Down` closes page drawer; focus visible, never lost). Skipped items (§1.1) are not tested and do not gate.

---

## 14. Acceptance checklist (practical scope — skipped items don't gate)

- [ ] Every practical `<div onClick>` has a keyboard equivalent (§9.3: `SubModelCard`, list rows, breadcrumb, `ModelCategory`).
- [ ] Every practical control with a key names it in its hover/focus tooltip (§1.9) and in Help inline next to its explanation (§1.8); header full-map matches implementation (single registry).
- [ ] Globals work: `Ctrl+M` monitor, `Alt+N` notifications, `Ctrl+S` sessions, `?` Help, `Shift+Up/Down` page drawers (History no-selection = no-op), all outside inputs only.
- [ ] Arrows move within page/widgets alongside Tab and never change routes unless EdgeNav/body focused (§1.4); caret keys win in inputs.
- [ ] PTT, mute, text-mode, pause/resume, engage/disengage all keyboard-operable (§2).
- [ ] `Escape` unwinds exactly one level everywhere; `Shift+Down` closes the page drawer; neither is destructive nor navigates.
- [ ] Focus moves in on every overlay/menu/drawer/step and restores on close; never left on removed nodes.
- [ ] Skipped items (calendar-grid, color-drag, camera/drag emulation, resize arrows, reorder keys) are absent from Help/tooltips and do not block sign-off.

---

## Appendix A — Current-state file index (audit trail)

Global: `shared/lib/overlayStack.ts`, `shared/hooks/useOverlay.ts`, `App.tsx:130-143`, `layout/ResponsiveLayout.tsx:84-125`, `pages/Settings.tsx:117-125`, `shared/ui/SessionContextMenu.tsx:69-82`, `shared/ui/Drawer.tsx`, `shared/ui/EdgePanel.tsx`.
Home: `pages/Home.tsx`, `shared/components/home/TextInputBar.tsx`, `SessionPanel.tsx`, `ActiveSessionHeader.tsx`, `ActiveTranscript.tsx`, `DialogueBubble.tsx`, `StatusCapsule.tsx`.
History: `pages/History.tsx`, `OrbitCarousel.tsx`, `VoiceRippleNode.tsx`, `MonthDayCard.tsx`, `CentralClockNode.tsx`, `CalendarPicker.tsx`, `HistoryListView.tsx`, `DetailPanel.tsx`, `ViewSelector.tsx`.
Memory: `pages/Memory.tsx`, `MemoryGraph.tsx`, `GraphControlDock.tsx`, `SearchBar.tsx`, `MemorySessionRail.tsx`, `MemoryLegendOverlay.tsx`, `MemoryNodeTooltip.tsx`, `PersonalMemoryStagingCard.tsx`, `PersonalMemoryCommentPopover.tsx`.
Settings: `pages/Settings.tsx`, `RadialHub.tsx`, `SettingsCardWrapper.tsx`, `RestoreDefaultsButton.tsx`, `PersonaCard.tsx`, `InteractionCard.tsx`, `TriggerModeCard.tsx`, `PipelineModeCard.tsx`, `CategorySelector.tsx`, `ProviderSelectorView.tsx`, `LlmConfigDesk.tsx`, `RealtimeConfigDesk.tsx`, `DictationConfigDesk.tsx`, `ModelsTopologyMap.tsx`, `SettingsTopologyMap.tsx`, `SubModelCard.tsx`, `LlmCatalogView.tsx`, `VadWorkspace.tsx`, `AsrWorkspace.tsx`, `LlmSettingsView.tsx`, `TtsVoiceManager.tsx`, `VoiceCarousel.tsx`, `RotaryKnob.tsx`, `CarouselSelector.tsx`, `SegmentedControl.tsx`, `UnderlineInput.tsx`, `ApiKeyField.tsx`, `RemoteServerSetup.tsx`, `AuxiliaryWorkspace.tsx`, `WorkingMemoryCard.tsx`, `PersonalMemoryCard.tsx`, `PersonalMemoryConfigDesk.tsx`, `AppearanceCard.tsx`, `RealtimeCard.tsx`.
Monitoring: `pages/Monitoring.tsx`, `MetricCarousel.tsx`, `LiquidChamber.tsx`.
Wizard: `wizard/WizardRoot.tsx`, `WizardFooter.tsx`, `ModelCategory.tsx`, `ModelSetupStep.tsx`, `AudioSetupStep.tsx`, `LiveTestStep.tsx`, `WelcomeStep.tsx`, `CompletedStep.tsx`.
Chrome/panels: `layout/EdgeNav.tsx`, `TitleBar.tsx`, `TopRightCluster.tsx`, `BottomDockFeather.tsx`, `ModelStatusOverlay.tsx`, `NotificationPanel.tsx`, `HelpPanel.tsx`, `HelpControlCard.tsx`, `Tooltip.tsx`, `ToggleTile.tsx`, `Card.tsx`, `Badge.tsx`, `data/helpCopy.ts`, `data/homeCopy.ts`, `data/settingsCopy.ts`.
