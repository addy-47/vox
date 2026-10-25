---
typography:
  display:
    fontFamily: "'Sora', 'DM Sans', system-ui, sans-serif"
  body:
    fontFamily: "'DM Sans', system-ui, sans-serif"
  mono:
    fontFamily: "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace"
  scale:
    '2xs': 11px
    xs: 12px
    sm: 13px
    base: 14px
    md: 15px
    lg: 16px
    xl: 18px
    '2xl': 24px
    '3xl': 28px
    '4xl': 36px
colors:
  # ── Theme tokens (dark — Claude matte graphite) ────────
  background: '#131315'
  foreground: '#eceae6'
  foreground-muted: '#9c9994'
  accent: '#00dbe9'
  accent-dark: '#0891b2'
  accent-muted: '#00dbe9'
  accent-foreground: '#131315'
  card: '#1b1b1e'
  border: '#ffffff'
  field: '#16161a'
  signal: '#00dbe9'
  # ── Theme tokens (light — Claude warm linen/ivory) ──────
  background-light: '#f7f6f2'
  foreground-light: '#1c1a18'
  foreground-muted-light: '#66625c'
  accent-light: '#0e7490'
  accent-dark-light: '#155e74'
  accent-foreground-light: '#ffffff'
  card-light: '#ffffff'
  border-light: '#000000'
  field-light: '#eeeae6'
  signal-light: '#0891b2'
  # ── Semantic status palette ────────────────────────────
  success: '#34d399'
  success-dark: '#047857'
  error: '#ef4444'
  error-dark: '#dc2626'
  danger: '#f43f5e'
  danger-dark: '#be123c'
  warning: '#facc15'
  warning-dark: '#d97706'
  amber-deep: '#b45309'
  warn-soft: '#f59e0b'
  info: '#38bdf8'
  info-dark: '#0369a1'
  violet: '#a78bfa'
  violet-dark: '#7c3aed'
  violet-deep: '#6d28d9'
  pink: '#f472b6'
  pink-dark: '#be185d'
  muted: '#64748b'
  muted-soft: '#94a3b8'
  # ── Notification category tokens (derived from accent at runtime;
  #    dark-theme defaults shown — notifications-spec §4.7) ──────
  notif-session-compaction: '#4d88f9'
  notif-memory-consolidation: '#8f4df9'
  notif-pipeline: '#f94de8'
  notif-dictation: '#f94d76'
  notif-hardware: '#f9954d'
  notif-models: '#cdf94d'
  notif-storage: '#4df973'
  # ── Neutral & glass ────────────────────────────────────
  white: '#ffffff'
  black: '#000000'
  glass-tint: '#0a0c0e'
  glass-surface: '#14181e'
  glass-deep: '#04070e'
  glass-navy: '#080c16'
  ghost: '#1e293b'
  border-dark: '#475569'
  border-light-tint: '#e2e8f0'
rounded:
  sm: 0.25rem
  base: 0.375rem
  md: 0.5rem
  lg: 0.75rem
  xl: 1.125rem
  '2xl': 1.5rem
  '3xl': 1.75rem
  pill: 9999px
  # ── Doc metadata ──
  title: "Vox Design System Spec — Liquid Space"
  audience: "Internal — UI contributors, designers, frontend agents"
  last_updated: 2026-08-20
  owners: "frontend-engineer role"
  related_docs:
    - "docs/frontend.md — Consumes these tokens"
    - "docs/backend.md — Pipeline mood source"
    - "docs/features/performance-memory-optimizations.md — UI perf invariants"

# Vox Design System Spec — "Liquid Space"

This document is the **authoritative design system** for Vox, a realtime voice AI desktop
app. It defines the tokens, type system, and visual rules that every user-facing surface
must follow. Implementation and UX-mechanic details live in
[`frontend.md`](./frontend.md); anything user-facing not covered here defers to the
frontend architecture doc and the impeccable design rules.

---

## 0. How to read this doc

- **Audience:** any UI contributor, designer, or frontend agent.
- **Scope:** the authoritative design system — tokens, type roles, elevation, motion, accessibility.
- **Convention:** tokens are declared as CSS variables in `app/src/index.css` and mirrored in this file's frontmatter; implementation lives in `app/src/`.
- **Non-goals:** not the frontend architecture (→ `docs/frontend.md`); not backend (→ `docs/backend.md`).
- **SSOT:** the frontmatter token maps are what the `impeccable` design detector enforces.

## 1. Design Principles

Vox is not a standard application interface — it is a sentient ambient surface that reacts
to the voice pipeline state. Every visual decision either serves that or works against it.

1. **Sentience over UI.** Minimize standard widgets, borders, and input fields. Interactions
   lead with voice, sound, and ambient light.
2. **State flows one way: pipeline → UI.** Mood and visual state are always derived from the
   backend event stream, never invented by local component logic.
3. **Aliveness never outranks usability.** If an expressive treatment makes a component
   harder to read, slower to operate, or ambiguous, simplify it. Usability wins — and is
   said so rather than shipping the fancier version.
4. **Performance is part of the design.** This runs on constrained, CPU-first hardware
   (8 GB RAM, sub-200 ms perceived latency). Nothing visually heavy ships un-memoized or
   un-throttled.
5. **Glass elevation is a closed system.** A fixed, small number of elevation levels, each
   with a defined purpose. Do not invent a new level to solve a one-off layout problem.
6. **Direct Ambient Stage Invariant (No Redundant Stage Wrappers).** Every major interactive
   surface (the Orb in `Home`, the 3D Graph in `Memory`, the 3D Chamber in `History`) must mount
   **directly on the fluid ambient page root** (`relative flex-1 flex flex-col h-full w-full bg-transparent`).
   NEVER introduce artificial inner container boxes, nested card wrappers, or duplicate radial
   gradient backdrops for a page's primary interactive canvas. All interactive nodes and controls
   position fluidly on the root ambient field.

---

## 2. Elevation & Glass System

All cards, headers, and navigation bars use a cohesive glassmorphic and matte surface system layered on a
transparent page root so the animated ambient background bleeds through and unifies the
workspace.

* **Claude-Style Tactile Micro-Grain**: Depth and surface tactility are unified across the entire application
  via a global procedural SVG micro-noise grain overlay (`#root::after`), creating a warm paper/matte finish
  that eliminates clinical digital plastic glares (`opacity: 0.035` dark, `0.045` light with `mix-blend-mode: multiply`).
* **Hairline Boundaries**: Borders are drawn with `--border` at crisp hairline opacity (`rgba(var(--border), 0.06–0.08)`);
  outer fuzzy glow drops are eliminated in favor of subtle 1–2px micro-elevations (`0 1px 3px rgba(0, 0, 0, 0.12)`).
* **Dynamic Bottom Dock Feathering**:
  - **Compact / Mobile (`< 1024px`)**: Renders full-width floor feathering behind `EdgeNav` (`fixed bottom-0 inset-x-0 h-[110px]`)
    where Monitoring is integrated inside the navigation dock.
  - **Desktop (`>= 1024px`)**: Full-width floor feather is **suppressed**. Feathering is strictly localized around active
    corner clusters and dynamically mounts only when an overlapping side panel/drawer is open:
    1. *Bottom-Left* (Monitoring button & CPU/RAM HUD): Mounts feather only when `sessionsOpen` is true.
    2. *Bottom-Right* (Model Status / Turn Metrics / Memory Legend): Mounts feather only when right panels (`help`, `notifications`, or memory drawer) are open.
    3. *Center EdgeNav*: Enclosed in a compact localized capsule feather rather than bleeding edge-to-edge.

---

## 3. Color System

Colors are declared as RGB-triplet CSS variables (`rgb(var(--token))`) in `index.css` under
`:root` and `[data-theme='light']`. The canonical tokens are mirrored in this file's
frontmatter `colors` map, which is what the impeccable detector enforces.

### Core tokens (Claude-Style Warm Matte & Linen)

| Token | Dark (Warm Graphite) | Light (Warm Linen / Ivory) | Role |
| :--- | :--- | :--- | :--- |
| `--background` | `19, 19, 21` (`#131315`) | `247, 246, 242` (`#f7f6f2`) | Page / app shell |
| `--foreground` | `236, 234, 230` (`#eceae6`) | `28, 26, 24` (`#1c1a18`) | Primary text |
| `--foreground-muted` | `156, 153, 148` (`#9c9994`) | `102, 98, 92` (`#66625c`) | Secondary text, timestamps, hints |
| `--accent` | `0, 219, 233` | `14, 116, 144` | Active states, links, focus, voice signal |
| `--accent-dark` | `8, 145, 178` | `21, 94, 117` | Hover/depressed accent |
| `--accent-foreground` | `19, 19, 21` | `255, 255, 255` | Text on accent fills |
| `--card` | `27, 27, 30` (`#1b1b1e`) | `255, 255, 255` | Card fill |
| `--border` | `255, 255, 255` | `0, 0, 0` | Hairline borders (used at low alpha) |
| `--field` | `22, 22, 26` | `238, 236, 230` | Ambient field base |
| `--signal` | `0, 219, 233` | `8, 145, 178` | Voice signal highlights |

### Semantic status palette

Used for live status/telemetry only (memory health, model state, ingestion results):

- **Success** `#34d399` · **Success deep** `#047857`
- **Error** `#ef4444` · **Error deep** `#dc2626`
- **Danger** `#f43f5e` · **Danger deep** `#be123c`
- **Warning** `#facc15` / `#f59e0b` · **Warning deep** `#d97706`
- **Info** `#38bdf8` · **Info deep** `#0369a1`
- **Violet** `#a78bfa` / `#7c3aed` · **Pink** `#f472b6` / `#be185d`
- **Muted** `#64748b` / `#94a3b8`

### Notification category tokens

`--notif-<category>` (one per closed notification category) is **derived at runtime** from the
live `--accent` HSL base via harmonic hue rotation — the same mechanism as the memory legend
palette — and recomputed on every theme/accent write so it flips with the rest of the token set.
`index.css` declares static dark/light defaults (`#4d88f9`, `#8f4df9`, `#f94de8`, `#f94d76`,
`#f9954d`, `#cdf94d`, `#4df973` for the default cyan accent). No category token may ever equal
`--accent`; the card kicker and its primary action button are the only surfaces they colour
(notifications-spec §4.7).

### Rules

- Text and fills **must** come from tokens (`rgb(var(--token))`); hardcoded hex is reserved
  for data visualization palettes (memory graph collections) and the semantic status set
  above.
- Opacity is expressed as the token's alpha (`/10`, `/25`, `/50`), never as a different color.
- Light mode muted text must hold WCAG AA ≥ 4.5:1 against light glass (see §Accessibility).

---

## 4. Typography System

### 4.1 Role stack

| Role | Family | Utility / Class | Purpose |
| :--- | :--- | :--- | :--- |
| **Display** | Sora | `font-display` / `.field-text` | Brand titles, page & section headings, stage titles, active voice signals |
| **Body / UI** | DM Sans | `font-sans` / `.ambient-label` | UI labels, settings options, body copy, dialogue |
| **Mono / Data** | JetBrains Mono | `font-mono` / `.signal-text` | Numeric metrics, latencies, timestamps, code-like readouts |

* **Display (Sora) is for headings only.** Page/section titles, wizard `h1`s, and stage
  headers use `font-display`. Do not use it for body copy or buttons.
* **Mono is for data only.** `font-mono` belongs on numbers, timestamps, latency readouts,
  and telemetry — never on descriptive prose or button labels.
* The app default (`html`, `body`) is `font-sans` (DM Sans) at **14px**.

### 4.2 Type scale

Sizes come from the scale ramp only (frontmatter `typography.scale`). No arbitrary sizes.

| Step | Size | Typical use |
| :--- | :--- | :--- |
| `2xs` | 11px | Tooltips, badges, micro-labels. **The floor — nothing renders below 11px.** |
| `xs` | 12px | Small labels, button text, table cells |
| `sm` | 13px | Secondary body, input hints, meta |
| `base` | 14px | Default body / UI text |
| `md` | 15px | Emphasis body, sub-headings |
| `lg` | 16px | Card titles, body emphasis |
| `xl` | 18px | Section headers |
| `2xl` | 24px | Sub-page headings |
| `3xl` | 28px | Page headings (display) |
| `4xl` | 36px | Hero / wizard titles (display) |

### 4.3 Uppercase policy

Uppercase (`uppercase`) is a **label voice, not a design voice**. It is reserved for:

1. Display headings that are intentionally shout-y (`font-display` page titles).
2. Short labels and kickers (1–3 words): tab labels, section kickers, active controls.
3. Short button text (≤ 4 words).

Uppercase is **forbidden** on:

* Subtext, descriptions, and explanatory copy (sentence case).
* Muted subtitles under headings.
* Timestamps, durations, and status messages (sentence case).
* Any text longer than ~4 words that must be read, not scanned.

### 4.4 Reading rules

* Body copy stays in the **45–75 character measure**.
* Line height for prose: **1.5–1.7**; for UI labels: ≥ 1.3.
* Light-on-dark text gets slightly more line height, a touch more tracking, and one more
  weight step when the face needs it.
* Tracking (`tracking-*`) is tuned to the role: labels/kickers may track up, body copy never.
* Preserve browser zoom and user font settings. Load only the used weights.

---

## 5. Shape & Rounding

Radii come from the `rounded` scale only (frontmatter `rounded`):

| Token | Value | Use |
| :--- | :--- | :--- |
| `sm` | 4px (0.25rem) | Dense session/project rows, context menus, checkboxes |
| `base` | 6px (0.375rem) | Default base radius |
| `md` | 8px (0.5rem) | Badges, static metadata chips, compact utility buttons |
| `lg` | 12px (0.75rem) | Primary action buttons, boxed inputs, nested card panels |
| `xl` | 18px (1.125rem) | Primary cards, preview banners, dropdown popovers |
| `2xl` | 24px (1.5rem) | Modals, dialog boxes, hero workspaces |
| `3xl` | 28px (1.75rem) | Bottom sheet drawer top corners |
| `pill` | 9999px | **Interactive buttons and controls only** (never static copy) |

Roundness should read as **consistent and calm** — do not mix `rounded-lg` and `rounded-xl`
on sibling cards within the same group.

### 5.1 Pill Container Invariant: Buttons Only (Zero Faux-Pill Copy)

Pill-shaped containers (`rounded-full` / `rounded-pill` with background, border, and inset padding) are **strictly reserved for clickable, interactive controls**:

* **Permitted on:**
  1. Interactive action buttons, preset choices, and pills in `SegmentedControl`.
  2. Toggle triggers and switches (e.g. `WebSearchGlobe`, `TransliterationToggle`).
  3. The main navigation floating capsule dock (`EdgeNav`).
* **Strictly Forbidden on Static Copy:**
  1. **Never wrap descriptive text, labels, or static metrics in pill containers.** Text with background fill and border looks like an interactive button and confuses the visual hierarchy.
  2. **No Redundant Value / Status Badges Next to Titles:** Never add a pill badge next to a section or card title that parrots adjacent control settings (e.g. repeating `"300 ms"`, `"1.05x"`, `"Active"`, `"Manual"`). When an active control, button grid, or toggle switch is visible on the card, any duplicate status pill is redundant visual clutter and strictly banned.
  3. Static readouts, kickers, and titles must render as clean, unboxed typography (using font weight, tracking, opacity, or accent text color directly on the card surface).

### 5.2 Navigation Tabs Invariant: Simple Underline Tabs Only

All multi-tab navigation strips across the application (in modals, config desks, or page views) MUST be simple underline tabs. Pill-shaped buttons or capsule wrappers are strictly prohibited for tab navigation.

* **Container Grammar**: Flex container with a subtle bottom border (`border-b`), zero capsule styling, and no enclosed pill track.
* **Tab Elements**: Monospace or sans uppercase typography with active bottom underline indicator and inactive transparent border.
* **Subtle Dividers**: Delicate vertical dividers between sibling tabs.
* **Exception**: Settings category selector strips may use bordered icon + label cells of fixed width, provided no pill or capsule wrappers are used.

### 5.3 Clean Vector SVG Pattern (Frameless Linework)

All vector illustrations, domain diagrams, and custom toggle widgets:

* **Frameless Linework**: Never enclose vector illustrations in heavy outer rounded-rectangle or pill-shaped container boxes. The graphic must breathe cleanly on the transparent surface.
* **Crisp Engineering Geometry**: Use delicate stroke weights (0.8px – 1.25px), subtle opacity ramps (0.25 – 0.85), and theme accent highlights. Use authentic structural representations (isometric lattices, chassis data traces, stacked memory planes).
* **Unboxed Accompanying Typography**: Text accompanying an SVG graphic must render as clean, unboxed typography, never wrapped in a faux-button pill box.

---

## 6. Spacing & Density

* Spacing is a **4px rhythm** (0.25rem steps: `p-1 = 4px`, `p-2 = 8px`, `p-3 = 12px`,
  `p-4 = 16px`, …).
* Respect parent-container padding: if a parent panel already applies default padding
  (e.g. `p-3`), do not duplicate horizontal padding or margins on child components.
* Align text labels, active tab items, inputs, and cards along the exact same vertical axis.
* Minimum tappable/target size: **32×32px**; prefer 40px+ for primary controls.

---

## 7. Motion & Feedback

* **Dynamic FPS** (`useDynamicFPS`): heavy visual loops (Three.js WebGL orb, canvas waveform)
  throttle to 60/15/0 fps by activity tier (active / idle / sleep).
* Mood sync is universal — any element meant to feel "alive" responds to the pipeline state
  cycle and, while `Working`, to the resolved activity (§8.2).
* Micro-interactions are short and eased (150–300 ms, ease-out). Avoid continuous looping
  animations on functional UI (no `animate-bounce` on buttons).
* Respect `prefers-reduced-motion`: reduce or pause decorative loops.
* **Viewport-resize gate** (`data-viewport-resize` on `<html>`, `app/src/layout/viewportResize.ts`):
  while the window is being resized, app-wide CSS transitions are suspended,
  keyframe animations are paused, and `backdrop-filter` is dropped — the same
  suspension contract as the theme-flip gate, but unconditional (it is a
  performance gate, not a motion preference). Drastic vw/vh changes (minimize,
  maximize, crossing the 1024px compact boundary) reflow as an instantaneous
  static layout instead of a 60 fps compositing storm, without flashing modal
  loader overlays during standard window resizing.

### 7.1 Logo Loader Invariant

* **Zero Typography**: Loading components (`OrbitalLoader`, boot loader) are purely visual, ambient elements. They MUST NOT render text (no title, subtitle, or status text).
* **Minimalist Stroke Logo**: Loading graphic consists of a clean, minimal stroke-style SVG of the official Vox audio waveform logo. Silhouettes and outer rotating rings are eliminated.
* **Compositor Performance**: Animates purely through hardware-accelerated CSS `stroke-dashoffset` flow and gentle breathing glow. Zero JavaScript rAF animation loops.

### 7.2 Route Commit Motion & Bloom

* **Unmount First, Then Reveal**: Navigation immediately commits and unmounts the previous page, displaying an ambient textless loader until destination chunks paint. Never keep stale routes mounted behind overlays.
* **Orb Entrance Bloom**: The WebGL orb stays invisible until its first frame is painted, blooming in over 700 ms via compositor transforms and opacity. The bloom is skipped under `prefers-reduced-motion`.

---

## 8. Ambient Background & Sentient Energy

The background is a reactive canvas representing the voice engine's state.

### 8.1 Status Surface

The pipeline status surface is a **frameless typographic line** centered above the orb with an optional gradient shimmer sweep. It is never enclosed in a pill, border, background tint, or backdrop blur — per §5.1, faux-pill containers are reserved for interactive controls only, and a status label is not one.

| `interactionState` | Label | Tone | Shimmer |
| :--- | :--- | :--- | :--- |
| `Idle` (disengaged) | `Dormant` | `dormant` | no |
| `Ready` | `Ready` | `live` | yes |
| `Listening` | `Listening` | `live` | yes |
| `Thinking` | `Thinking` | `live` | yes |
| `Speaking` | `Speaking` | `live` | yes |
| `Paused` | `Paused` | `dormant` | no |
| `Sleeping` | `Sleeping` | `dormant` | no |
| `Error` | `Error` | `alert` | no |
| `Working` (no activity) | `Working` | `live` | yes |

Tone constrains typography only — never a colored container. The surface carries `role="status"` and `aria-live="polite"`. While `Working`, the label is overridden by the resolved **activity** (§8.2).

### 8.2 Activity Resolution & Orb Motion

`Working` operations resolve via a **three-tier cascade** (first hit wins):
1. **`activity.name`** (e.g. `web_search`, `search_memory`, `compaction`).
2. **`activity.kind`** (`tool` or `compaction`). Catches newly added operations without frontend registry edits.
3. **`interactionState`** default fallback.

Orb motion presets map to activity state (`silk` = default noise displacement, `orbits` = orbital planes, `globe` = meridian scan, `web` = edge pulses, `morph` = particle re-boundary). Transitions use a smooth sheet-to-particle cross-dissolve (~300 ms).

### 8.3 Sentient Membrane (`PipelineField`)

Behind the central orb, a radial membrane expands and contracts with VAD probability and audio volume. It renders frameless with gradient falloff; heavy CSS blur filters (`blur-[120px]`) and screen blend modes are strictly prohibited.

---

## 9. Holographic Dialogue Stream

Rather than standard conversation logs, Vox renders a holographic dialogue stream:

* User queries bubble **left**; AI voice responses align **right**.
* No card framing — text renders directly on the ambient field.
* The scroll zone has a vertical CSS mask gradient so older turns dissolve upward.
* Words and lines float up smoothly as they stream from the STT/LLM engines.

---

## 10. Iconography & Tooltips

* **Icon style**: thin-stroke (lucide), consistent 1.5px stroke weight; semantic colors
  reserved for status.
* **Custom tooltips only.** `app/src/shared/ui/Tooltip.tsx` is the only sanctioned tooltip —
  glass, 11px uppercase, 4 sides. Native `title` attributes are banned as tooltips; keep
  `title` only for component props, truncated-text ellipsis, and shared primitives.
* Icon-only buttons **must** have a tooltip.

---

## 11. Accessibility

* **Contrast**: text on its surface must meet WCAG AA — 4.5:1 body, 3:1 large text. Light
  mode `--foreground-muted` (`#334155`) measures 9.8:1 against light glass.
* **Focus**: all interactive elements enforce a visible `focus-visible` ring
  (`outline: 2px solid rgb(var(--accent))`).
* **Keyboard**: all flows operable by keyboard; drawers trap focus and bind `Escape` to close.
* **Font floor**: nothing below **11px** functional text.

---

## 12. Unified Overlay Grammar

Every transient surface (panel, drawer, popover, modal) responds to unified dismissal gestures enforced by a single global authority (`shared/lib/overlayStack.ts`).

### 12.1 Overlay Tiers

| Tier | Surface | Anchor | Motion |
| :--- | :--- | :--- | :--- |
| **Tier 0** | Settings accordion cards | Inline | Expand / collapse |
| **Tier 1** | Popovers & micro-panels (tooltips, test menus, status popovers) | Element anchor / click / hover | Scale-fade, transient |
| **Tier 2** | Bottom drawers (History detail, Memory pipeline, Profiler) | Bottom sheet (`Drawer.tsx`) | Translate-Y, spring ease |
| **Tier 2b** | Centered modal (Dense list expansion, compact modals) | Screen center (`Modal.tsx`) | Scale-fade, transient |
| **Tier 3** | Edge rails (Help, Notifications, Sessions) | Side dock (`EdgePanel.tsx`) | Width-collapse / expand |

### 12.2 Dismissal & Stacking Contracts

* **Single Dismissal Authority**: Surfaces do not install their own Escape or outside-click listeners. `shared/lib/overlayStack.ts` manages a FILO stack: `Escape` pops the topmost surface, and root pointer-down dismisses the topmost active overlay.
* **Backdrop Dimming Contract**:
  - Centered dialogs (`Modal.tsx`) and edge rails (`EdgePanel.tsx`) use `bg-black/50 backdrop-blur-[2px]`.
  - Bottom sheets (`Drawer.tsx`) preserve ambient visibility with `bg-[rgb(var(--background))]/60 backdrop-blur-sm`.
* **Z-Index Ladder**:
  - `z-[70]`: Global overlays, modals, and open edge rails.
  - `z-[60]`: Top chrome clusters (`TopRightCluster`).
  - `z-[50]`: Floating bottom navigation (`EdgeNav`).
  - `z-[40]`: Page headers and stationary HUD bars.
  - `z-[30]`: Page stage content and interactive canvas.

### 12.3 Smooth Scroll Standard (SSOT)

All scrollable lists and panels use inertial smooth scrolling with exactly one mechanism:
* Managed exclusively via `shared/lib/lenisScroll.ts` and `shared/hooks/useLenisScrollContainer.ts` (or `SmoothScrollArea.tsx`). Never call `new Lenis` ad-hoc in components.
* Container classes: `overflow-y-auto overscroll-contain custom-scrollbar`.
* Modals and drawers must yield (`overflow-hidden`) when child content owns its scroller. Nested scrollable bodies are prohibited.
* CSS `snap-*` is banned on any Lenis-managed container.

### 12.4 Master-Detail & Expansion Invariants

* **Dense List Expansion (Tier 2b)**: Constrained inline card lists (`ExpandableList`) expand into spacious centered modals (`Modal.tsx`) featuring an integrated search/filter toolbar, multi-column responsive cards, and directly visible telemetry badges.
* **Header Hierarchy & Step Navigation**:
  - **Step 1 (Top-level card face)**: 16–17px leading icon, uppercase tracked title, and trailing forward affordance swapping card face.
  - **Step 2 (Nested view / Configuration desk)**: Compact 14px icon, leading back breadcrumb arrow (`ArrowLeft`), and normal-case title.
* **In-Place Card Expansion**: Expandable action cards (e.g. cloning, custom provider setups) expand in place without layout jumps. Actions provide immediate inline feedback without popping external dialogs.
* **Action Feedback & Mutual Exclusion**: Heavy or asynchronous operations (e.g. session compaction, model probing) must transition the trigger into an inline spinner and globally disable sibling triggers across the application during execution.
* **Identity Preservation**: Completion refreshes must preserve component identity; updating counts or timestamps must never cause an active detail surface to unmount or reset scroll position.

---

## 13. Keyboard Interaction & Spatial Navigation

### 13.1 Key Hierarchy & Modifiers

Keys are categorized into strict functional tiers to eliminate collision between global and in-page navigation:

| Modifier Level | Scope | Primary Purpose | Example |
| :--- | :--- | :--- | :--- |
| **`Shift + Arrow`** | Application Navigation | Global page cycling (`←` / `→`) and contextual drawer expanding (`↑` / `↓`) | `Shift + Right` (Next Page) |
| **`Ctrl / Cmd + Key`** | Rails & Window Lifecycle | Toggling persistent side rails, window close, and app quit | `Ctrl + S` (Sessions), `Ctrl + W` (Close Window) |
| **Plain Arrows (`↑ ↓ ← →`)** | In-Page Spatial Navigation | Moving focus between cards/widgets strictly within the active zone | Navigating session nodes in History |
| **Single Alpha (`M`, `T`, `P`)** | Voice Pipeline Direct Controls | Immediate single-stroke voice pipeline controls when not typing | `M` (Mute Mic), `P` (Pause/Resume), `T` (Text Mode) |
| **`Space` / `Enter` / `Esc`** | Primaries | PTT hold-to-talk, selection activation, and hierarchical overlay dismissal | `Space` (Hold to Talk), `Escape` (Dismiss / Barge-in) |

Full keybinding definitions are maintained in `app/src/data/shortcuts.ts` as the single source of truth.

### 13.2 Spatial Navigation & Tooltip Invariants

* **Container Isolation**: 2D directional arrows are scoped to explicit zones (`STAGE_CONTAINER`, `DOCK_CONTAINER`, `CLUSTER_CONTAINER`, `RAIL_CONTAINER`). Navigating directional arrows within a stage cannot accidentally hop into chrome without deliberate boundary rules or `Tab`.
* **Tooltip Navigation Silence**: Directional arrow navigation temporarily silences tooltips (`isSpatialNavigating`) to prevent intrusive popup flickering during rapid keyboard navigation.
* **Lazy Tooltip Activation**: Tooltips must remain inert while closed, never subscribing to spatial nav or floating-ui events until hovered or focused.

---

## 14. Do / Don't

| Do | Don't |
| :--- | :--- |
| Use tokens for every color, radius, and size | Hardcode hex or arbitrary px beyond the scale |
| Put every heading in `font-display` (Sora) | Put body copy or buttons in Sora |
| Reserve `font-mono` for numbers & telemetry | Use mono on prose or button labels |
| Uppercase only headings, short labels, short buttons | Uppercase subtext, descriptions, timestamps |
| Use the custom `Tooltip` for hover explanations | Rely on native `title` tooltips |
| Keep cards within one radius step per group | Mix `rounded-lg` + `rounded-xl` on sibling cards |
| Respect the closed 4-tier elevation levels | Invent a 5th glass level for one-off layouts |
| Speak layman copy (no engine/STT/LLM jargon) | Use acronyms or sci-fi jargon in user-facing text |
| Use simple underline tabs for all tab strips | Use pill or capsule buttons for tabs |
| Render frameless linework for vector graphics | Wrap SVG diagrams in heavy rounded container boxes |

---

**Last Updated:** 2026-10-25
