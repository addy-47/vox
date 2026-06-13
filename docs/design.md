# Vox Design System Spec: "Liquid Space"

This specification details the design philosophy, visual aesthetics, layout rules, and performance systems of Vox, an agentic voice operating system (Jarvis-like OS).

---

## 1. Vision & Core Aesthetic

Vox is designed not as a static application, but as a **sentient digital organism** that lives on the desktop. The interface feels light, organic, responsive, and alive. 

*   **Sentience over UI**: Minimize standard widgets, borders, and input fields. Interactions should lead with voice, sound, and ambient light.
*   **Holographic Elevation**: Elements float as translucent layers above a morphing visual core.
*   **Aesthetics**: Glassmorphism, neon gas glows, deep space obsidian gradients, and anti-aliased micro-animations.

---

## 2. Glass Elevation System

All cards, headers, and navigation bars use a cohesive glassmorphic styling sysytem layered on top of a fully transparent page root. This allows the animated ambient background to bleed through and unify the workspace.

There are **4 levels of elevation** defined by blur density and tint opacity:

| Elevation Level | CSS Class | Blur Radius | Tint Opacity (Dark) | Tint Opacity (Light) | Best Use Cases |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Whisper** | `.glass-whisper` | 8px | `0.20` | `0.45` | Tooltips, status badges, secondary dropdown options. |
| **Surface** | `.glass-surface` | 16px | `0.45` | `0.65` | General content panels, navigation strips, settings containers. |
| **Card** | `.glass-card` | 24px | `0.65` | `0.80` | Major modules, dialog boxes, settings category headers. |
| **Elevated** | `.glass-elevated` | 40px | `0.85` | `0.92` | Modal overlays, system monitoring windows, popovers. |

*   **Sheen & Noise**: Visual depth is enhanced with a noise grain overlay (`.amb-noise` / `.glass-base::after`) to simulate frosted glass texture.

---

## 3. Ambient Background & Sentient Energy

The background is a reactive canvas representing the state of the voice engine.

### Mood Synchronization
The ambient background morphs color and animation velocity based on the pipeline's active phase:
*   `calm` (Idle/Sleep): Low-energy deep obsidian, slow-flowing organic blobs, minimal ripple rings.
*   `active` (Listening/UserSpeaking): High frequency, expanded glow, fast morphing speed.
*   `thinking` (Processing/LLM generation): Swirling cyan/violet orbits, pulsing central energy.
*   `speaking` (TTS playback): Fluid ripple waves spreading from the central core.

### Sentient Membrane (`PipelineField`)
Behind the central Orb is a dashed radial membrane that expands and contracts with VAD probability and audio volume, creating a visual heart rate for the assistant.

---

## 4. Holographic Dialogue Stream

Rather than presenting standard conversation logs or overlapping single lines of text, Vox features a **Holographic Dialogue Stream**:

*   **Left/Right Separation**: User query bubbles align left; AI voice responses align right.
*   **No Card Framing**: Text is rendered directly on the ambient field, maximizing screen space and visual integration.
*   **Faded Scroll Boundary**: The scroll zone has a vertical CSS mask gradient (`mask-image: linear-gradient(to bottom, transparent, black 15%, black 85%, transparent)`) so older turns dissolve into space as they scroll upwards.
*   **Micro-animations**: Words and lines float up smoothly using Framer Motion as they are streamed from the STT and LLM engines.

---

## 5. Responsive & Dynamic Layouts

The desktop layout transitions dynamically to a unified layout on small screens (mobile viewports):

### Central Navigation Capsule (`EdgeNav`)
On desktop, navigation is a floating bottom capsule. On mobile, the system monitoring metrics (which float bottom-left on desktop) are hidden, and the **Activity Monitor** is integrated directly as a 4th `NavLink` tab inside the navigation capsule itself, routing directly to `/monitoring`.

### Full-Page Diagnostics Monitor
On mobile/small screens, monitoring is not rendered as a popover panel overlay (which could cause home page UI bleed-through and flashing). Instead, it renders as a dedicated page route (`/monitoring`) with a solid background containing status badges, CPU/RAM bars, latency metrics, and Sparkline graphs.

### Viewport Transition Engine
Vox handles window resizing dynamically:
*   **Mobile ➔ Desktop**: If the user is on the `/monitoring` route page and resizes to desktop, the router redirects them back to the Home page (`/`) and automatically launches the popover panel.
*   **Desktop ➔ Mobile**: If the user has the popover panel open on desktop and resizes to mobile, the popover closes and they are routed directly to `/monitoring` so they don't lose context.

### Sentient Core Scale
On mobile viewports, the central Orb is scaled up by **50%** (`min(92vw, 85vh)` instead of `min(70vw, 65vh)`) to act as the primary, dominant touch target and visual anchor.

---

## 6. Performance Constraints & Best Practices

To achieve high rendering performance on baseline systems (8GB RAM, CPU-first):
*   **Dynamic FPS (`useDynamicFPS`)**: Heavy visual loops (Three.js WebGL in the Orb, HTML5 Canvas in the Waveform) throttle their frame rate dynamically:
    *   *Active*: 60fps (rendering active wave/orb)
    *   *Idle*: 15fps (slow idle glow)
    *   *Sleep*: 0fps (fully paused loop when tab is hidden or asleep)
*   **React Memoization**: All visually intensive components (`AmbientBackground`, `PipelineField`, `VoxOrb`, `LiveWaveform`) are wrapped in `React.memo` to eliminate unnecessary rendering overhead during text streaming or database reads.
