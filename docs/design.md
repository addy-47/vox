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
  # ── Theme tokens (dark) ────────────────────────────────
  background: '#050505'
  foreground: '#e5e2e1'
  foreground-muted: '#a0a0a0'
  accent: '#00dbe9'
  accent-dark: '#0891b2'
  accent-muted: '#00dbe9'
  accent-foreground: '#050505'
  card: '#0a0a0a'
  border: '#ffffff'
  field: '#0c0e18'
  signal: '#00dbe9'
  # ── Theme tokens (light) ───────────────────────────────
  background-light: '#f1f5f9'
  foreground-light: '#0f172a'
  foreground-muted-light: '#334155'
  accent-light: '#0e7490'
  accent-dark-light: '#155e74'
  accent-foreground-light: '#ffffff'
  card-light: '#ffffff'
  border-light: '#000000'
  field-light: '#ebeff8'
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

All cards, headers, and navigation bars use a cohesive glassmorphic system layered on a
transparent page root so the animated ambient background bleeds through and unifies the
workspace. There are **4 levels of elevation**, defined by blur density and tint opacity:

| Level | CSS Class | Blur | Tint (Dark) | Tint (Light) | Use Cases |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Whisper** | `.glass-whisper` | 8px | `0.20` | `0.45` | Tooltips, status badges, secondary dropdowns |
| **Surface** | `.glass-surface` | 16px | `0.45` | `0.65` | Content panels, navigation strips, settings containers |
| **Card** | `.glass-card` | 24px | `0.65` | `0.80` | Major modules, dialog boxes, settings category headers |

* **Sheen & noise**: depth is enhanced with a noise grain overlay (`.amb-noise` /
  `.glass-base::after`) to simulate frosted glass.
* **Boundaries**: borders are drawn with `--border` at low opacity (`rgba(var(--border), 0.08–0.15)`);
  never use hard 1px white borders as the primary separation mechanism.

---

## 3. Color System

Colors are declared as RGB-triplet CSS variables (`rgb(var(--token))`) in `index.css` under
`:root` and `[data-theme='light']`. The canonical tokens are mirrored in this file's
frontmatter `colors` map, which is what the impeccable detector enforces.

### Core tokens

| Token | Dark | Light | Role |
| :--- | :--- | :--- | :--- |
| `--background` | `5, 5, 5` | `241, 245, 249` | Page / app shell |
| `--foreground` | `229, 226, 225` | `15, 23, 42` | Primary text |
| `--foreground-muted` | `160, 160, 160` | `51, 65, 85` | Secondary text, timestamps, hints |
| `--accent` | `0, 219, 233` | `14, 116, 144` | Active states, links, focus, voice signal |
| `--accent-dark` | `8, 145, 178` | `21, 94, 117` | Hover/depressed accent |
| `--accent-foreground` | `5, 5, 5` | `255, 255, 255` | Text on accent fills |
| `--card` | `10, 10, 10` | `255, 255, 255` | Card fill |
| `--border` | `255, 255, 255` | `0, 0, 0` | Hairline borders (used at low alpha) |
| `--field` | `12, 14, 24` | `235, 239, 248` | Ambient field base |
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
2. Short labels and kickers (1–3 words): badge text, pill labels, tab labels, section kickers.
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
| `lg` | 1rem | Large panels, buttons |
| `xl` | 1.25rem | Dialog boxes |
| `2xl` | 1.75rem | Hero panels, drawers |
| `pill` | 9999px | Fully rounded pills, badges, the nav capsule |

Roundness should read as **consistent and calm** — do not mix `rounded-lg` and `rounded-xl`
on sibling cards within the same group.

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
* Mood sync is universal — any element meant to feel "alive" responds to the pipeline mood
  cycle (calm / active / thinking / speaking).
* Micro-interactions are short and eased (150–300 ms, ease-out). Avoid continuous looping
  animations on functional UI (no `animate-bounce` on buttons).
* Respect `prefers-reduced-motion`: reduce or pause decorative loops.

---

## 8. Ambient Background & Sentient Energy

The background is a reactive canvas representing the voice engine's state.

### Mood synchronization

| Mood | Phase | Treatment |
| :--- | :--- | :--- |
| `calm` | Idle / sleep | Low-energy deep obsidian, slow organic blobs, minimal ripple rings |
| `active` | Listening / user speaking | High frequency, expanded glow, fast morphing |
| `thinking` | LLM generation | Swirling cyan/violet orbits, pulsing central energy |
| `speaking` | TTS playback | Fluid ripple waves spreading from the central core |

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

### Settings topology subtab icons (Model Hub — Settings mode)

| Pipeline domain | Subtab | Icon | Notes |
| :--- | :--- | :--- | :--- |
| VAD | Sensitivity | `AudioWaveform` | |
| VAD | Silence Cutoff | `Hourglass` | |
| VAD | Noise Gate | `SlidersHorizontal` | |
| STT | Streaming Rate | `Zap` | |
| STT | Transliterate | `Languages` | |
| STT | **Compute Allocation** | **`Microchip`** | Presets Auto/Eco/Max/Custom; maps to `stt.embedded.threads`; SettingReloadPolicy::Restart |
| LLM | Compute Allocation | `Microchip` | Presets Auto/Eco/Max/Custom; maps to `llm.threads` |
| LLM | Response | `TextCursorInput` | |
| LLM | Context | `Layers2` | |
| LLM | Creativity | `WandSparkles` | |
| TTS | Voice | `AudioLines` | |
| TTS | Speech Rate | `Metronome` | |
| TTS | **Compute Allocation** | **`Microchip`** | Presets Auto/Eco/Max/Custom; maps to `tts.threads`; SettingReloadPolicy::Restart |

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
| **Tier 1** | popovers & micro-panels (Memory node tooltip, Home test-clip menu, Monitoring popover) | on hover / click | scale-fade, transient |
| **Tier 2** | bottom drawers (History detail, Memory pipeline, Memory profiler) | bottom sheet | translate-Y, spring ease |

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

### Implementation

- `shared/lib/overlayStack.ts` — global FILO registry (`registerOverlay`,
  `closeTopmost`, `getStackSize`); installed once in `App.tsx` via
  `installOverlayStack()`. Capture-phase `keydown` (Escape) + `pointerdown`
  (outside-click on the topmost overlay).
- `shared/hooks/useOverlay.ts` — registers on `active`, unregisters on close.
- `shared/ui/Drawer.tsx` — the shared bottom-sheet for all Tier 2 surfaces
  (backdrop, resize handle, double-click expand, focus restore, `footer`,
  `position="page" | "global"`).
- Settings cards collapse via local Escape (mirrors the outside-click FILO pop).

### Tier 2 surfaces

- **History detail** — `DetailPanel` inside a `Drawer` (`position="global"`, `z-60` layering over `EdgeNav` `z-50`). By design, bottom sheet bodies are transparent overlays allowing the ambient page field and dialog bubbles to breathe without opaque solid backgrounds, backed only by the ambient dimmed backdrop.
- **Memory pipeline** — horizontal, left-to-right stage flow inside a global drawer.
- **Memory profiler** — converted from a route to a global bottom drawer
  (`ProfilerDrawer`); sampling is lazy (starts on open, persists across cycles).
  The bottom-left HUD button and the Monitoring popover open it via the
  `openProfiler()` handle.

---

## 12. Do / Don't

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

**Last Updated:** 2026-08-20