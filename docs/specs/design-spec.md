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
  base: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.25rem
  '2xl': 1.75rem
  xs: 0.3125rem
  sm2: 0.5625rem
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
| `xs` | 5px (0.3125rem) | Scrollbars, tiny controls |
| `sm` | 0.25rem | Checkboxes, small chips |
| `base` | 0.5rem | Default inputs, small cards |
| `sm2` | 9px (0.5625rem) | Scrollbar track end caps |
| `md` | 0.75rem | Cards, popovers |
| `lg` | 1rem | Large panels, standard buttons |
| `xl` | 1.25rem | Dialog boxes |
| `2xl` | 1.75rem | Hero panels, drawers |
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

### 5.2 Navigation Tabs Invariant: Simple Underline Tabs Only (Zero Pill Tabs)

All multi-tab navigation strips across the application (in modals, config desks, or page views) MUST be simple underline tabs (matching the canonical config desk components, e.g., `PersonalMemoryConfigDesk` and `CategorySelector`). Pill-shaped buttons or capsule wrappers are strictly prohibited for tab navigation.

* **Container Grammar**: Flex container with a subtle bottom border (`border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5 select-none overflow-x-auto no-scrollbar`), with no capsule styling or enclosed pill track.
* **Tab Elements**: Clean text (`text-[11px] sm:text-[11.5px] font-mono font-bold uppercase tracking-[0.06em]`) with an active bottom underline indicator (`border-b-2 border-[rgb(var(--accent))] text-[rgb(var(--accent))]`) and inactive transparent border (`text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]`).
* **Subtle Dividers**: Delicate vertical dividers between sibling tabs (`<span className="text-[10px] text-[rgb(var(--foreground-muted))]/25 select-none pb-1.5 px-1 sm:px-2">|</span>`).

### 5.3 Clean Vector SVG Pattern (Frameless Linework)

All vector illustrations, domain diagrams, and custom toggle widgets (e.g. `WebSearchGlobe`, `TransliterationToggle`, `RemoteComputeGraphic`, `ManagedContextGraphic`):

* **Frameless Linework:** Vector illustrations must never be enclosed in heavy outer rounded-rectangle or pill-shaped container boxes (`border border-[rgba(var(--accent),...)] bg-[rgba(var(--accent),...)] w-full h-[68px]`). The graphic must breathe cleanly on the transparent surface.
* **Crisp Engineering Geometry:** Use delicate stroke weights (0.8px – 1.25px), subtle opacity ramps (0.25 – 0.85), and theme accent accents. Avoid drawing generic rounded pills to represent compute or context; use authentic structural representations (isometric lattices, blade chassis with data traces, stacked memory planes, or script vectors).
* **Unboxed Accompanying Typography:** Text accompanying an SVG graphic must render as clean, unboxed typography below or beside the illustration (e.g. `text-[9px] font-mono font-bold tracking-[0.14em] uppercase text-[rgb(var(--accent))]`), never wrapped in a faux-button pill box.

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

### 7.1 Vox Logo Loader Invariant (Minimal Stroke Waveform & Zero Typography)

* **Zero Typography**: `OrbitalLoader` and boot loader are purely visual, ambient loading components. They MUST NOT render any text (no title, subtitle, or status text) across any page or view.
* **Minimalist Stroke Logo**: The loading graphic consists of a clean, minimal, stroke-style SVG of the official Vox audio waveform logo (`rgb(var(--accent))` stroke, ~1.5px weight). It renders the 5 vertical sound equalizer lines overlaid with the signature central waveform featuring the deep "V" dip and terminal node dots. Heavy orb silhouettes, outer rotating rings, and ping auras are eliminated in favor of an ultra-lean, single-element SVG.
* **Motion & Performance Budget**: Animates purely through hardware-accelerated CSS `stroke-dashoffset` flow and gentle breathing glow (`opacity` / `filter`). Zero JavaScript animation loops or `requestAnimationFrame` hooks are permitted. Idle loops pause under `prefers-reduced-motion` and `data-viewport-resize`.


---

## 8. Ambient Background & Sentient Energy

The background is a reactive canvas representing the voice engine's state.

### 8.1 Status Surface

The pipeline status surface is a **frameless typographic line** centered above the orb. It is static text plus an optional gradient shimmer sweep. It is never enclosed in a pill, border, background tint, or backdrop blur — per `.agents/rules/frontend-style-guide.md` §5, faux-pill containers are reserved for interactive controls only, and a status label is not one.

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

Tone constrains typography only — it is never expressed as a coloured container. `dormant` and `alert` render static text; a shimmer sweep on `Dormant` would animate something that is intentionally still. The surface carries `role="status"` and `aria-live="polite"` so label changes are announced.

When the pipeline is in `Working`, the label is overridden by the resolved **activity** (§8.2).

### 8.2 Activity Resolution & Orb Motion

`Working` covers multiple distinct operations. The `activity` envelope (`ipc-spec.md` §4.1) identifies which. Resolution is a **three-tier cascade**, evaluated in order, with the first hit winning:

1. **`activity.name`** — e.g. `web_search`, `search_memory`, `compaction`.
2. **`activity.kind`** — `tool` or `compaction`. Catches any newly added operation with no registry edit.
3. **`interactionState`** — the §8.1 default, which also supplies the orb motion when no activity is present.

A tier-3 fallback guarantees that adding a backend operation never requires a frontend change and never produces an empty label.

| Tier-1 key (`activity.name`) | Label | Orb motion preset |
| :--- | :--- | :--- |
| `web_search` | `Searching web` | `globe` |
| `search_memory` | `Recalling memory` | `web` |
| `compaction` | `Compacting context` | `morph` |

| Tier-2 key (`activity.kind`) | Label | Orb motion preset |
| :--- | :--- | :--- |
| `tool` | `Working` | `orbits` |
| `compaction` | `Compacting context` | `morph` |

Orb motion presets are named for the motion grammar ported from the `thinking-orbs` engine:

| Preset | Motion |
| :--- | :--- |
| `silk` | Default. Silk-sheet noise displacement only — today's behaviour. |
| `orbits` | Particles travelling tilted orbital planes around the silk core. |
| `globe` | A vertical meridian scan band sweeping the particle field; dots brighten as it passes. |
| `web` | A constellation wiring itself — nearest-neighbour edge pulses converging inward. |
| `morph` | The silk sheets dissolve as particles re-boundary (circle → triangle → square). |

Entering `Working` drives a **sheet-to-particle cross-dissolve** (~300 ms): silk opacity falls as particle alpha rises, so the orb visibly decompresses into its activity-specific form and re-compresses on completion. The particle layer is omitted entirely on software rasterisers, where `morph` degrades to `silk`.

### Sentient membrane (`PipelineField`)

Behind the central orb, a dashed radial membrane expands and contracts with VAD probability
and audio volume — the visual heart rate of the assistant.

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

## 13. Gesture Contract (Unified Overlay Grammar)

Every transient surface — panel, drawer, popover, card — must respond to the
same dismissal gestures, enforced by a single global authority rather than
per-surface listeners.

### Overlay tiers

| Tier | Surface | Anchor | Motion |
| :--- | :--- | :--- | :--- |
| **Tier 0** | settings accordion cards | inline | expand / collapse |
| **Tier 1** | popovers & micro-panels (Memory node tooltip, Home test-clip menu, Monitoring popover) | bottom corner / hover / click | scale-fade, transient |
| **Tier 2** | bottom drawers (History detail, Memory pipeline, Memory profiler) | bottom sheet | translate-Y, spring ease |
| **Tier 2b** | centered modal (Personal Memory on compact viewports `< 1024px`; dense list expansion) | screen center | scale-fade, transient |
| **Tier 3** | edge rails (Help, Notifications, Conversations) | top corner trigger | width-collapse / expand |

### Overlay topology

Vox uses one overlay grammar across the shipped interface:

- **Top-corner triggers open edge rails.** Help and Notifications share one right-edge rail group; only one is open at a time. Conversations uses an independent left-edge rail on Home.
- **Bottom-corner triggers open popovers.** Monitoring remains a bottom-left popover on desktop and a route on compact viewports.
- **Central cards and nodes open bottom drawers.** The existing `Drawer` remains the single bottom-sheet primitive for these surfaces. Below the 1024px compact threshold, where a full-bleed sheet crushes two-column content, the Personal Memory surface renders as a Tier 2b centered modal instead, and the History detail renders as a right-edge panel (`EdgePanel`, same dim/surface/spring as every other rail) over the dimmed session list instead of a drawer.
- **Dense lists expand in place via Tier 2b.** Where a card body holds a selection list too long to scan or operate at its inline height, the card keeps its constrained inline list and exposes a single expand affordance that presents the same list in a centered modal. The inline list stays authoritative for selection and never becomes disabled or stale while the modal is open; the modal is a larger viewport onto the same state, not a second editor. The expand affordance is an icon-only control and therefore requires a `Tooltip`.
- **Dismissal is centralized.** Escape closes the topmost surface first (FILO), and outside pointer-down closes the topmost dismissible surface. Surfaces do not install their own Escape or outside-click listeners.
- **The memory profiler is a debug surface.** It is not part of the shipped overlay contract.

### Dismissal rules

- **Escape closes the topmost surface first (FILO).** `profiler drawer open →
  monitoring popover opens on top → first Escape closes the popover → second
  Escape closes the profiler`.
- **Clicking the root layout closes any open surface.** Backdrops handle outside
  clicks for Tier 2; Tier 1 popovers close on any pointerdown outside their element.
- **Re-tapping a trigger toggles** the surface closed (History detail, Memory pipeline drawer).
- The stack is the **single Escape authority**; surfaces must not add their own
  Escape listeners. Exceptions that legitimately stay local (non-dismissal): the
  dictation hotkey recorder, search-input clear, inline editing.

### Implementation & Stacking Contracts

- **Backdrop Dimming Contract**: Centered dialogs (`Modal.tsx`) and full edge rails (`EdgePanel.tsx`) requiring focus isolation use the unified dark dim token `bg-black/50 backdrop-blur-[2px]`. Bottom sheets (`Drawer.tsx`) remain unaffected and preserve page ambience using `bg-[rgb(var(--background))]/60 backdrop-blur-sm`.
- **Layering & Z-Index Hierarchy**:
  - `z-[70]`: Global overlays, modals, and open edge rails (including History detail `EdgePanel`).
  - `z-[60]`: Top chrome clusters (`TopRightCluster`, top-left session triggers). Open right-edge rails must layer above or suppress `TopRightCluster` to prevent visual overlap.
  - `z-[50]`: Floating bottom navigation (`EdgeNav`).
  - `z-[40]`: Page headers and stationary HUD bars.
  - `z-[30]`: Page stage content and interactive canvas.
- `shared/lib/overlayStack.ts` — global FILO registry (`registerOverlay`, `closeTopmost`, `getStackSize`); installed once in `App.tsx` via `installOverlayStack()`. Capture-phase `keydown` (Escape) + `pointerdown` (outside-click on the topmost overlay).
- `shared/hooks/useOverlay.ts` — registers on `active`, unregisters on close.
- `shared/ui/Drawer.tsx` — the shared bottom-sheet for all Tier 2 surfaces (`bg-[rgb(var(--background))]/60 backdrop-blur-sm` backdrop).
- `shared/ui/Modal.tsx` — the shared centered dialog for Tier 2b surfaces (`bg-black/50 backdrop-blur-[2px]` dim, scale-fade motion, focus trap + restore, `footer`, `position="page" | "global"`).
- `shared/ui/EdgePanel.tsx` — side-docked sliding rail with unified `bg-black/50 backdrop-blur-[2px]` backdrop.

### Tier 2 & Tier 2b surfaces

- **History detail** — `DetailPanel` inside a bottom `Drawer` on wide orbit viewports (`> 1024px`). On list/compact viewports (`< 1024px`), clicking a session performs an **in-place page drill-down**: the session list transitions in-place into the full-width session transcript view with a top breadcrumb (`← All Sessions`), timestamp, turn count, and actions. Pressing `Escape` or clicking `← All Sessions` returns immediately to the preserved list view without clumsy overlay drawers.
- **Personal Memory Compact Modal (Tier 2b)** — On viewports `< 1024px`, Personal Memory renders as a unified single-entity modal (no nested card-inside-a-card borders or double headers). Features an integrated top masthead with actions (Copy, Regenerate, Consolidate) and underline tabs (`Memory`, `Observations`, `Staging`). The workspace below forms one cohesive surface with distinct inner section headers (`About You`, `Observations`, `Staging`), synchronized 32px icon badges, and status filter parity. Actions affecting persistent memory auto-redirect to `Memory` for instant feedback.
- **Expanded List Modal (Tier 2b)** — When dense, compact inline card lists (`ExpandableList` in settings, catalogs, providers) are expanded into full modals (`w-[min(920px,94vw)] h-[min(720px,88vh)]`), they must not render squished compact rows in a large void. Instead, they transition into a spacious, tailored UI: an integrated toolbar with real-time search and filter chips, multi-column responsive cards (`grid-cols-1 md:grid-cols-2 lg:grid-cols-3`), and directly visible telemetry (speed/TPS, context window, VRAM footprint, capability badges) with zero reliance on hover-only tooltips. The modal header hosts domain commit controls only for states needing action or status (missing credential, apply-restart, saved/restarting/failed), using the same logic as the card footer, while any open modal suppresses the background card footer. Routine hot-save changes show no intermediate Save/Cancel and resolve directly to the saved state. List order follows committed (saved) state only: selecting re-highlights immediately but re-sorts on save, gliding the list to top with a smooth scroll.
- **Memory pipeline** — horizontal, left-to-right stage flow inside a global drawer.
- **Memory profiler** — converted from a route to a global bottom drawer (`ProfilerDrawer`).

### 13.1 Session Compaction Triggers (History)

Compaction extracts structured semantic memory facts from uncompacted session turns. Compaction triggers are rendered across four canonical History surfaces, strictly gated by `session.uncompacted_turns > 0`:

1. **Details Panel Header (`DetailPanel`)**: Action button positioned left of the close (`X`) icon in `Drawer:headerActions`, rendering text `"Compact session"` (`variant="button"`).
2. **Orbital View Session Card (`VoiceRippleNode`)**: Tactile circular glass icon button (`variant="icon"`, Sparkles icon) positioned in the card's top right corner, left of the delete trash button, visible on hover/focus-within (`group-hover:flex`).
3. **List View Session Card (`HistoryListView`)**: Circular glass icon button positioned in the bottom right corner of each card, visible always alongside the delete action.
4. **Step 2 Drill-Down Top Right Cluster (`TopRightCluster` / `History`)**: Icon button mounted directly to the left of the Notification (`Bell`) icon when an in-place session drill-down is open.

*Dynamic State & Mutual Exclusion Invariants:*
- **Single Authority**: `uncompacted_turns` is the only source of truth for trigger visibility, on every surface. Notification state is never a second gate — a card whose `uncompacted_turns` is zero is never shown, and a card with uncompacted turns is always shown, regardless of notification rollup or resolution.
- **Active Compaction**: When compaction is initiated, the target session trigger transitions immediately into a spinning loader (`Loader2` spin).
- **Universal Mutual Exclusion**: When compaction is in progress on any session, all other compaction triggers across the UI are disabled (`opacity-50 cursor-not-allowed`) — not only those on the initiating surface.
- **Completion Transition**: When compaction concludes, the backend commits the session's compaction output, emits `NotificationUpdated` (resolving the task card) then `NotificationCreated` (the receipt), then `SessionsChanged`. There is no dedicated compaction event; `SessionsChanged` is the sole signal that `uncompacted_turns` has changed.
- **No Transient Identity Swap**: A completion refresh must not re-key any session-scoped resource on anything other than the session's identity. Refreshing a session in place (new counts, new title, new timestamp) must never cause its transcript, detail surface, or drill-down to unmount, reload, or drop back to a closed state.

---


## 14. Keyboard Interaction, Spatial Navigation & Tooltip Contract

### 14.1 Key Hierarchy & Modifiers Contract

Keys are categorized into strict functional tiers to eliminate collision between global navigation and in-page navigation:

| Modifier Level | Scope | Primary Purpose | Example |
| :--- | :--- | :--- | :--- |
| **`Shift + Arrow`** | Application Navigation | Global page cycling (`←` / `→`) and contextual drawer expanding (`↑` / `↓`) | `Shift + Right` (Next Page) |
| **`Ctrl / Cmd + Key`** | Rails & Window Lifecycle | Toggling persistent side rails, window close, and app quit | `Ctrl + S` (Sessions), `Ctrl + W` (Close), `Ctrl + Q` (Quit) |
| **Plain Arrows (`↑ ↓ ← →`)** | In-Page Spatial Navigation | Moving focus between cards/widgets strictly within the active zone | Navigating session nodes in History orbit |
| **Single Alpha (`M`, `T`, `P`)** | Voice Pipeline Direct Controls | Immediate single-stroke voice pipeline controls when not typing | `M` (Mute Mic), `P` (Pause/Resume), `T` (Text Mode) |
| **`Space` / `Enter` / `Esc`** | Primaries | PTT hold-to-talk, selection activation, and hierarchical overlay dismissal | `Space` (Hold to Talk), `Escape` (Dismiss) |

### 14.2 Shortcuts Matrix

| Shortcut | Action | Scope | Invariant |
| :--- | :--- | :--- | :--- |
| **`Ctrl + W`** (`Cmd + W`) | Close / Hide Window | Global | Closes window to tray; voice engine continues background operation. |
| **`Ctrl + Q`** (`Cmd + Q`) | Quit Application | Global | Terminates backend process and UI entirely. |
| **`Shift + →` / `←`** | Page Cycle | Global | Forward: `Home` → `History` → `Memory` → `Settings` (→ `Monitoring` if compact). Backward in reverse. |
| **`Shift + ↑` / `↓`** | Context Drawer Expand/Collapse | Contextual | Home: Profiler. History: Session Detail. Memory: Personal Memory. Settings: Domain Cards. |
| **`Ctrl + S`** (`Cmd + S`) | Toggle Sessions Rail | Global | Left docked session list. |
| **`Ctrl + N`** (`Cmd + N`) | Toggle Notifications Rail | Global | Right docked notifications. |
| **`Ctrl + /`** (`Cmd + /`) | Toggle Help Panel | Global | Route guide. |
| **`?`** (`Shift + /`) | Shortcuts Cheat Sheet | Global | Direct cheat sheet overlay. |
| **`Space` (Hold)** | Push-To-Talk | Engaged + PTT | Starts audio capture on keydown; commits on keyup. Cancels if <200ms. |
| **`M`** | Toggle Mic Mute | Non-editable | Toggles microphone capture stream. |
| **`Shift + M`** | Toggle Speaker Mute | Non-editable | Toggles TTS / playback output. |
| **`P`** | Pause / Resume Voice | Non-editable | Toggles voice pipeline paused vs. active. |
| **`T`** | Open Text Input Mode | Non-editable | Focuses bottom text prompt input. |
| **`Ctrl + Space`** | Engage / Disengage | Global | Wakes or sleeps the ambient assistant. |
| **`Escape`** | Hierarchical Dismissal | Global | Dismisses top overlay in reverse FILO order; barges into speaking voice agent. |

### 14.3 Spatial Navigation Invariants

* **Container Isolation**: 2D directional arrows are scoped to explicit zones: `STAGE_CONTAINER` (in-page content), `DOCK_CONTAINER` (`EdgeNav`), `CLUSTER_CONTAINER` (top chrome), `RAIL_CONTAINER` (active side rail).
* **Zero Accidental Escape**: Navigating directional arrows within a stage cannot accidentally hop across containers into the bottom dock or top header without deliberate border rules or `Tab`.
* **Focus-State Sync**: Spatial focus synchronization immediately updates selection state (e.g. focused orbit card becomes selected session).

### 14.4 Tooltip Invariants

* **Floating Architecture**: Powered by `@floating-ui/react` with smart boundary collision, flip, and offset handling.
* **Navigation Silence**: Directional arrow navigation temporarily silences tooltips (`isSpatialNavigating`) to prevent intrusive popup flickering during rapid keyboard navigation.
* **Surface Grammar**: Dark glass card, 11px uppercase label, optional `<kbd>` shortcut badge. Native `title` tooltips remain banned.

---

## 15. Do / Don't

| Do | Don't |
| :--- | :--- |
| Use tokens for every color and size | Hardcode hex or arbitrary px beyond the ramp |
| Put every heading in `font-display` (Sora) | Put body copy or buttons in Sora |
| Reserve `font-mono` for numbers & telemetry | Use mono on prose or button labels |
| Uppercase only headings, short labels, short buttons | Uppercase subtext, descriptions, timestamps |
| Use the custom `Tooltip` for hover explanations | Rely on native `title` tooltips |
| Keep cards within one radius step per group | Mix `rounded-lg` + `rounded-xl` on sibling cards |
| Respect the elevation levels | Invent a 5th glass level for one-off layouts |
| Speak layman copy (no engine/STT/LLM jargon) | Use acronyms or sci-fi jargon in user-facing text |

---


**Last Updated:** 2026-10-04