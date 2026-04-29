# 📄 `frontend.md` — Vox Frontend Architecture

---

# 1. Overview

Vox frontend is a **multi-surface UI system**, not a single application UI.

It consists of **two independent UI surfaces**:


### 1. Main Application UI

* User-invoked (click icon)
* Default **20:9 vertical layout**
* Expandable to fullscreen
* Used for:

  * Conversations
  * History
  * Settings

---

### 2. Ephemeral Overlay UI (Tray System)

* System-level overlay
* Appears automatically on speech detection
* Displays **single-turn real-time transcription**
* Disappears after silence (VAD-driven)

---

## 🧠 Core Principle

> UI is **event-driven**, not user-driven

* Backend (STT + VAD) controls UI lifecycle
* Overlay is **transient**, not persistent

---

# 2. Tech Stack

### Core

* React + TypeScript
* Vite

### Desktop Layer

* Tauri (multi-window system)

### UI System

* TailwindCSS
* shadcn/ui (for primitives)
* Custom Glassmorphism components

### Audio Visualization

* ElevenLabs waveform components
* Custom Orb (already implemented)

---

# 3. Project Structure

```bash
frontend/
├── src/                     # Main app UI
│   ├── pages/
│   │   ├── Home/            # Orb UI
│   │   ├── Chat/
│   │   └── Settings/
│   ├── components/
│   ├── layout/
│   └── state/
│
├── src-tray/                # ⚡ Overlay UI (separate app)
│   ├── TrayApp.tsx
│   ├── components/
│   │   ├── Transcript.tsx
│   │   ├── MiniWaveform.tsx
│   │   └── StatusDot.tsx
│   └── animations/
│
├── shared/                  # Shared logic
│   ├── hooks/
│   ├── store/
│   └── ui/
```

---

# 4. Window Architecture (Tauri)

Vox uses **multiple windows**, not a single app window.

---

## 4.1 Main Window

* Label: `main`
* Default: 20:9 ratio
* Resizable
* Can go fullscreen

---

## 4.2 Overlay Window (Tray UI)

* Label: `overlay`
* Purpose: transcription capsule

---

### Required Properties

```json
{
  "label": "overlay",
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "resizable": false,
  "visible": false
}
```

---

### Why these matter

* `alwaysOnTop` → stays above all apps ([Tauri][1])
* `transparent` → enables glass UI ([Tauri][2])
* `skipTaskbar` → not treated as app
* `decorations: false` → no window chrome

---

## Optional (Advanced)

* click-through when idle
* no focus stealing
* cursor-relative positioning

These patterns are used in overlay systems where UI should not interrupt workflow ([Mintlify][3])

---

# 5. Main App UI

---

## 5.1 Layout Modes

### Default Mode (Primary)

* Aspect Ratio: **20:9**
* Centered window
* Mobile-like interaction

---

### Fullscreen Mode

* Same layout scaled
* More space for chat/history

---

## 5.2 Pages

---

### 1. Home (Orb Interface)

* Central orb (AI state representation)
* Reactive to:

  * mic input
  * speaking
  * idle

---

### 2. Chat Page

* Conversation history
* Minimal bubbles
* Voice + text hybrid

---

### 3. Settings Page

* Model selection
* Voice selection
* Toggles:

  * Always listening
  * Overlay UI enable/disable

---

## 5.3 Navigation

* Bottom tabs (default mode)
* Side panel (fullscreen mode)

---

# 6. Overlay UI (Core Feature)

---

## 6.1 Definition

> **Ephemeral transcription capsule**

NOT:

* sidebar
* tray app
* persistent UI

---

## 6.2 Lifecycle

```text
Speech detected → Show overlay
Streaming text → Update UI
Silence (VAD) → Hide overlay
Destroy/reset state
```

---

## 6.3 Behavior

* Appears automatically
* No user interaction required
* No history retained
* One session = one UI instance

---

## 6.4 Size & Position

* Width: ~280–360px
* Height: ~20–30% of screen
* Position:

  * Right edge
  * Vertically centered or slightly lower

---

## 6.5 UI Structure

```text
[ Status ]
• LIVE / Listening

[ Content ]
Real-time transcript (streaming)

[ Footer ]
Subtle waveform / orb glow
```

---

## 6.6 Visual Design

Based on design system 

* Glassmorphism card
* Backdrop blur (~20px)
* 1px translucent border
* Cyan glow when active
* Rounded corners (lg / xl)

---

## 6.7 Animations

### Entry

* Slide from right (200–300ms)
* Fade-in
* Slight scale

---

### Exit

* Fade-out
* Slide back
* Destroy after animation

---

## 6.8 Interaction Rules

* No focus stealing
* No blocking clicks
* No keyboard capture

---

# 7. State Management

---

## Core State

```ts
{
  isListening: boolean
  isSpeaking: boolean
  transcript: string
  volumeLevel: number
}
```

---

## Sources

* Backend emits events:

  * `speech_start`
  * `transcript_partial`
  * `speech_end`

---

## Flow

```text
Backend → IPC → Overlay UI
```

---

# 8. Dev Workflow

---

## 8.1 UI Development (Fast)

```bash
pnpm dev
```

* Build overlay as fixed div
* Simulate transcript

---

## 8.2 Overlay Simulation

```css
position: fixed;
right: 20px;
top: 40%;
```

---

## 8.3 Real Testing

```bash
pnpm tauri dev
```

Test:

* Always-on-top
* Cross-app overlay
* Transparency
* Animations

---

## 8.4 VAD Simulation

Use:

* keyboard shortcut
* button trigger

---

# 9. Performance Constraints

---

## Critical Requirements

* Must run on **8GB RAM systems**
* Minimal GPU usage
* Minimal re-renders

---

## Rules

* Avoid heavy re-renders
* Use memoization
* Throttle animations
* Use requestAnimationFrame for waveform

---

## Overlay Optimization

* Mount only when needed
* Keep hidden instead of destroying (preferred)
* Avoid large DOM trees

---

# 10. Design System Enforcement

From your `design.md` 

---

## Must Follow

* Dark obsidian base
* Cyan primary glow
* Violet secondary accents
* Glass surfaces with blur
* Orb as central element

---

## Avoid

* heavy dashboards
* clutter
* boxed UI

---

# 11. Key Architectural Decisions

---

## 1. Separate overlay app (`src-tray`)

→ prevents coupling with main UI

---

## 2. Event-driven overlay

→ backend controls visibility

---

## 3. Reuse window (not recreate)

→ better performance

---

## 4. Minimal UI philosophy

→ only show what is needed

---

# 12. Final Definition

> Vox frontend = **Dual-surface UI system**
>
> * Main App → user-controlled
> * Overlay → system-triggered

---

# ✅ You are now ready to build

---

If you want next step, I can:

* Generate **exact React components for tray**
* Or give **Tauri IPC bridge code (Python ↔ UI)**
* Or design **animation system (Framer Motion config)**

[1]: https://tauri.app/v1/api/js/window?utm_source=chatgpt.com "window | Tauri v1"
[2]: https://v2.tauri.app/learn/window-customization/?utm_source=chatgpt.com "Window Customization"
[3]: https://www.mintlify.com/devv-shayan/Trueears/features/dictation?utm_source=chatgpt.com "AI Dictation - Trueears"
