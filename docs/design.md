---
name: Vox Liquid Space Design System
version: v0.8.3+ (2026)
replaces: v0.1.0 design
---

# Vox Design System — "Liquid Space"

A fusion of deep space ambient backgrounds with refined glassmorphism surfaces that feel like liquid layers. The design philosophy for Vox, a local-first voice AI assistant.

## Brand & Style

The emotional response should be one of **calm intelligence**. The interface feels like looking through liquid glass at a dynamic, breathing space. Surfaces are treated as ephemeral layers of light and glass at varying densities, with the Orb as the central light source that subtly illuminates surrounding surfaces.

- **Glassmorphism 2.0** — Not just cards: the entire app (sidebar, titlebar, page content) is glass at controlled densities
- **Deep Space Ambient** — Animated organic blob shapes float behind the glass, creating a living background
- **The Orb as Light Source** — The central Three.js orb influences the ambient glow and glass reflections
- **Calm Interactions** — Spring-physics animations, 200-400ms transitions, nothing jarring
- **Full Dark/Light Parity** — Light mode gets the same glass treatment, not just inverted colors

## Colors

**Primary palette** (defined as CSS variables in `:root`):

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| `--background` | `5, 5, 5` | `248, 250, 252` | Base background |
| `--foreground` | `229, 226, 225` | `15, 23, 42` | Primary text |
| `--foreground-muted` | `160, 160, 160` | `71, 85, 105` | Secondary text |
| `--accent` | `0, 219, 233` (cyan) | `8, 145, 178` | Interactive states, glow, borders |
| `--card` | `10, 10, 10` | `255, 255, 255` | Card surface |
| `--border` | `255, 255, 255` | `0, 0, 0` | Border/divider |

The `--accent` variable is **user-configurable** via the HexColorPicker in settings. When changed, all glass borders and glows update automatically via `rgb(var(--accent))`.

**Ambient background colors** (not CSS variables, static per theme):
- Dark: `#0a0a1a` → `#050508` → `#050505` (deep navy-black gradient)
- Light: `#eef1f5` → `#e4e8ee` → `#f4f6f9` (warm pearl gradient)

**Blob colors** (the 3 ambient background orbs):
- Blob 1: Cool cyan `rgba(0, 219, 233, 0.4)` — top-left, slow morph
- Blob 2: Warm violet `rgba(216, 186, 255, 0.35)` — bottom-right, slower drift
- Blob 3: Bright cyan `rgba(0, 240, 255, 0.3)` — center-right, pulse

## Typography

| Token | Font | Size | Weight | Usage |
|-------|------|------|--------|-------|
| `display-lg` | Space Grotesk | 48px | 600 (Semi-Bold) | Headlines |
| `headline-md` | Space Grotesk | 24px | 500 (Medium) | Section headers |
| `body-md` | Inter | 16px | 400 (Regular) | Body text, assistant responses |
| `label-sm` | Space Grotesk | 12px | 600 (Semi-Bold) | Labels, uppercase tracking |

- **Space Grotesk**: Display face for headlines, data readouts, labels — "cyber" aesthetic (imported from Google Fonts)
- **Inter**: Body text for assistant responses — maximum legibility (imported from Google Fonts)

## Page Root Transparency

To prevent solid blocks from covering the layouts' breathing background layers, all page root containers (`Home.tsx`, `Settings.tsx`, `History.tsx`, `Monitoring.tsx`) are styled as `bg-transparent`. This exposes the `AmbientBackground` and animated blobs behind the elements.

## Glass Elevation System (4 Levels)

Replaces the previous single `.liquid-glass` class. We utilize standard premium frosted glassmorphism rules with varying backdrop blurs and translucencies. On platforms with limited compositor capabilities, these degrade gracefully into elegant, semi-translucent overlays through which the colorful background blobs remain visible.

### Elevation Levels

| Level | CSS Class | Backdrop Blur | Dark Mode BG | Light Mode BG | Border | Use Case |
|-------|-----------|---------------|--------------|---------------|--------|----------|
| Whisper | `.glass-whisper` | 8px | `rgba(10, 12, 14, 0.2)` | `rgba(255, 255, 255, 0.45)` | 1px line (0.03 opacity) | Ambient page-level wrappers, minor pills |
| Surface | `.glass-surface` | 16px | `rgba(15, 18, 22, 0.45)` | `rgba(255, 255, 255, 0.65)` | 1px line (0.06 opacity) | Sidebar, titlebar, section containers |
| Card | `.glass-card` | 24px | `rgba(20, 24, 30, 0.65)` | `rgba(255, 255, 255, 0.8)` | 1px accent + 2px top accent | Main settings modules, chat bubbles |
| Elevated | `.glass-elevated` | 40px | `rgba(8, 10, 12, 0.85)` | `rgba(255, 255, 255, 0.92)` | 1px stronger accent | Modals, dropdowns, tray overlay HUD |

### Tactile Depth Layering

All glass levels share a `.glass-base` class that provides a modern, tactile depth simulation on top of the content:
- `::after` — Faint noise grain texture (SVG `feTurbulence` pattern at 3% opacity) combined with a diagonal linear light sweep (specular sheen) to give the glass a physical, premium feel.
- Inset box-shadows (e.g. `inset 0 1px 0 0 rgba(255, 255, 255, 0.05)`) are used to simulate light catching on the upper edges of the glass panel.

## Ambient Background

A `fixed`, `pointer-events: none`, `z-index: 0` component positioned behind all content:

- **Base gradients**:
  - Dark Mode: Rich midnight-indigo space gradient (`radial-gradient(ellipse at 50% 50%, #0c0d21 0%, #06060c 60%, #020204 100%)`)
  - Light Mode: Sophisticated pastel ceramic pearl-blue-violet gradient (`radial-gradient(ellipse at 50% 50%, #f0f3fd 0%, #e4eaf8 60%, #f4f6fc 100%)`)
- **3 animated organic blobs**: Pure CSS `border-radius` keyframe animation (no canvas/WebGL) with increased opacities to ensure dynamic colors are visible and prevent a bland/flat look.
  - Blob 1: Cool cyan `rgba(0, 219, 233, 0.4)` — top-left, 60s slow float
  - Blob 2: Warm violet `rgba(216, 186, 255, 0.35)` — bottom-right, 90s slow drift
  - Blob 3: Bright cyan `rgba(0, 240, 255, 0.3)` — center-right, 45s pulse
  - Opacity scales: Dark Mode (`0.12` to `0.18`), Light Mode (`0.14` override)
- **Noise grain overlay**: SVG data URI with `mixBlendMode: overlay`
- **`prefers-reduced-motion: reduce`**: Disables all blob animations

## Elevation & Depth

1. **Level 0 (Base)**: Deep midnight-indigo / ceramic pearl gradient (AmbientBackground)
2. **Level 1 (Ambient)**: Animated organic blobs with `blur(80-120px)` behind the glass
3. **Level 2 (Glass Surfaces)**: Page content, sidebar, titlebar — `glass-whisper` / `glass-surface`
4. **Level 3 (Cards)**: Interactive panels — `glass-card` with accent top-border
5. **Level 4 (Active Glow)**: Modals, tooltips — `glass-elevated` with strong backdrop blur

## Components

### Glass Cards
- `.glass-card` for standard cards
- Subtle top accent border (2px) as active indicator
- Noise grain + specular highlight via `::after` pseudo-element

### The Orb
- Three.js custom shader with 7 silk disc layers + Fresnel outer shell
- Dynamic FPS: 60fps active, 15fps idle, 0fps sleeping
- Pauses when page not visible (`document.hidden`, `IntersectionObserver`)

### Action Buttons/Controls
- **Engage/Stop**: Circular button with `glass-card` styling + border-rotate animation on active
- **PTT Mic**: Circular glass button with pulse animation on recording
- **Pill buttons**: `rounded-full`, accent-colored for primary, ghost for secondary
- **Toggles**: Glass track with pill thumb, smooth transition
- **Range sliders**: Glass track with accent-colored thumb and glow

### Status Indicator
- Glowing dot with pulse animation
- Status text: `shimmer-text` class for animated gradient
- States: System Dormant / System Ready / Listening / Recording / Thinking / Responding

## Performance Architecture

### Dynamic FPS (useDynamicFPS hook)
- Frame-skipping algorithm: `frameInterval = 1000 / targetFps`
- Three tiers: Active (60fps) / Idle (15fps) / Sleeping (0fps)
- Reacts to `document.visibilityState` and `IntersectionObserver`
- Used by: `AdvancedOrb` (Three.js), `LiveWaveform` (Canvas 2D)

### State Performance (zustand)
- Selective subscriptions prevent full-tree re-renders
- `useSettingsStore(selector)` — only re-renders on selector change
- `SettingsContext` is now a thin adapter wrapper for backward compatibility

## Shapes & Radii

- Cards: `1rem` (16px) corner radius (glass-surface), `1.25rem` (20px) for glass-card
- Buttons/chips: Pill-shape (9999px)
- The Orb: The only organic/fluid shape (3D iridescent sphere)
- Range slider thumbs: `rounded-full`

## Spacing System

| Token | Value |
|-------|-------|
| `base` | 8px |
| `container-padding` | 24px |
| `element-gap` | 16px |
| `section-margin` | 40px |
| `safe-zone` | 40px |

## Changes from v0.1.0 Design

| v0.1.0 | v0.8.3+ | Rationale |
|--------|---------|-----------|
| Single `.liquid-glass` class | 4-level glass token system | Clear elevation hierarchy, better perf |
| 3 static cyan gradient blobs | Animated organic CSS blob shapes | Living background that doesn't jank |
| Solid `#050505` sidebar + titlebar | Glass surface sidebar + titlebar | Unified glass immersion |
| React Context (all consumers re-render) | zustand (selective subscriptions) | Eliminates re-render bottleneck |
| Orb at constant 60fps | Orb at 60/15/0 dynamic FPS | Saves GPU when idle/sleeping |
| LiveWaveform always rendering | LiveWaveform pauses when not active | Stops unnecessary canvas draws |
| Light mode inverted colors | Light mode full glass treatment | Equal design attention |
