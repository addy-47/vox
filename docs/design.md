---
name: Vox Intelligence System
colors:
  surface: '#131313'
  surface-dim: '#131313'
  surface-bright: '#3a3939'
  surface-container-lowest: '#0e0e0e'
  surface-container-low: '#1c1b1b'
  surface-container: '#201f1f'
  surface-container-high: '#2a2a2a'
  surface-container-highest: '#353534'
  on-surface: '#e5e2e1'
  on-surface-variant: '#b9cacb'
  inverse-surface: '#e5e2e1'
  inverse-on-surface: '#313030'
  outline: '#849495'
  outline-variant: '#3b494b'
  surface-tint: '#00dbe9'
  primary: '#dbfcff'
  on-primary: '#00363a'
  primary-container: '#00f0ff'
  on-primary-container: '#006970'
  inverse-primary: '#006970'
  secondary: '#d8baff'
  on-secondary: '#440087'
  secondary-container: '#6b01cc'
  on-secondary-container: '#d2b0ff'
  tertiary: '#f6f4ff'
  on-tertiary: '#00149e'
  tertiary-container: '#d4d6ff'
  on-tertiary-container: '#2f44f4'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#7df4ff'
  primary-fixed-dim: '#00dbe9'
  on-primary-fixed: '#002022'
  on-primary-fixed-variant: '#004f54'
  secondary-fixed: '#eddcff'
  secondary-fixed-dim: '#d8baff'
  on-secondary-fixed: '#290055'
  on-secondary-fixed-variant: '#6200bc'
  tertiary-fixed: '#dfe0ff'
  tertiary-fixed-dim: '#bdc2ff'
  on-tertiary-fixed: '#000965'
  on-tertiary-fixed-variant: '#0020dc'
  background: '#131313'
  on-background: '#e5e2e1'
  surface-variant: '#353534'
typography:
  display-lg:
    fontFamily: Space Grotesk
    fontSize: 48px
    fontWeight: '600'
    lineHeight: '1.1'
    letterSpacing: -0.02em
  headline-md:
    fontFamily: Space Grotesk
    fontSize: 24px
    fontWeight: '500'
    lineHeight: '1.3'
  body-md:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: '1.6'
  label-sm:
    fontFamily: Space Grotesk
    fontSize: 12px
    fontWeight: '600'
    lineHeight: '1.0'
    letterSpacing: 0.1em
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  base: 8px
  container-padding: 24px
  element-gap: 16px
  section-margin: 40px
---

## Brand & Style

This design system embodies the intersection of high-fidelity futurism and functional minimalism. It is designed to feel like a sentient companion—lightweight, responsive, and premium. By blending **Glassmorphism** with **Cyber-Minimalism**, the interface avoids the visual clutter of traditional dashboards in favor of a "heads-up display" (HUD) aesthetic. 

The emotional response should be one of calm empowerment. Surfaces are treated as ephemeral layers of light and glass, ensuring that the primary focus remains on the "Orb"—the visual manifestation of the assistant's consciousness.

## Colors

The palette is anchored by a deep **Obsidian (#050505)** to create infinite depth, allowing holographic elements to pop. 

*   **Primary (Cyan):** Used for active states, voice ripples, and critical data points.
*   **Secondary (Violet):** Represents the "intelligence" aspect, used for processing states and soft accents.
*   **Iridescence:** A gradient mesh of Cyan, Violet, and Soft Blue should be applied to the central Orb and used sparingly as a background "light leak" to prevent the dark theme from feeling flat.
*   **Functional Grays:** High-transparency whites (low opacity) are used for glass surfaces to maintain a lightweight feel.

## Typography

This design system utilizes a dual-font approach to balance technical precision with readability. 

**Space Grotesk** is the primary display face, used for headlines, data readouts, and labels to reinforce the "cyber" aesthetic. **Inter** is utilized for body text and assistant responses to ensure maximum legibility during longer interactions. Weights should vary significantly—using Light (300) for secondary info and Medium/Semi-Bold (500-600) for core actions—to create hierarchy without relying on color.

## Layout & Spacing

The layout follows a **Fixed Grid** model with high margins to create a "floating" effect. Components should never feel cramped; negative space is a functional tool used to direct the user's eye to the central Orb.

*   **Safe Zones:** Large 40px+ margins on the edges of the viewport.
*   **Verticality:** Content flows from the bottom up (mobile-first interaction pattern), mimicking a conversation history.
*   **Alignment:** Center-aligned for voice-only modes; left-aligned for data-heavy informational cards.

## Elevation & Depth

Depth is achieved through **Backdrop Blurs** and **Tonal Layering** rather than traditional shadows.

1.  **Level 0 (Base):** Deep Obsidian (#050505).
2.  **Level 1 (Ambient):** Soft background blurs (30px - 60px radius) in iridescent colors to create a sense of atmospheric light.
3.  **Level 2 (Glass Surfaces):** Semi-transparent cards with a `backdrop-filter: blur(20px)` and a 1px solid border at 10% opacity.
4.  **Level 3 (Active Glow):** Elements that are "processing" or "active" emit a 15px outer glow (box-shadow) using the primary cyan color at 30% opacity.

## Shapes

The shape language is sophisticated and modern. Standard cards use a **1rem (16px) corner radius**. For smaller interactive elements like buttons and chips, a **Pill-shape (Full round)** is preferred to contrast against the structured grid of the cards. The Orb remains the only perfectly fluid, organic shape in the UI, distinguishing the AI from the UI container.

## Components

*   **Glass Cards:** The primary container. Must have a subtle top-down linear gradient (white at 5% to white at 2%) and a `1px` stroke.
*   **Action Buttons:** Pill-shaped. Primary buttons use a solid cyan-to-blue gradient; secondary buttons are "Ghost" style with a neon-tinged border.
*   **Voice Ripple:** A series of concentric, semi-transparent rings that pulse from the Orb.
*   **Status Chips:** Small, uppercase labels with a "dot" indicator to show connectivity or mode (e.g., • LIVE).
*   **Input Fields:** Minimalist underlines or glass-morphed fields with a shimmering focus state.
*   **The Orb:** A 3D-effect fluid sphere. It should utilize CSS/SVG filters to create an iridescent "gas" effect that moves in response to voice frequency.

## PTT Interaction & Waveform

The **Push-To-Talk (PTT)** mode provides a high-intent capture experience.

### Visual Language
*   **Capture Overlay:** A deep translucent obsidian layer (`rgba(10, 12, 14, 0.4)`) with high-radius backdrop blur (`20px+`) that isolates the recording action.
*   **Waveform Aesthetic:** Real-time frequency visualization using a high-fidelity bar renderer.
    *   **Color:** Electric Cyan (`#22d3ee`) with varying opacity based on amplitude.
    *   **Geometry:** Rounded bars (2px-3px width) with 2px gaps, creating a technical yet organic pulse.
*   **State Feedback:**
    *   **RECORDING:** Pulsing "Release to Send" label and active waveform.
    *   **PROCESSING:** A minimalist infinite spinner with a tracked "Processing" status label in uppercase tracking.