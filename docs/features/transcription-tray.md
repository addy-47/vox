# 📄 `transcription-tray.md` — Transcription Tray Feature

---

## 1. Problem Statement

Modern workflows involve working across multiple applications simultaneously. Voice input should be **system-level, not app-level**.

Users currently face friction when trying to use voice input:
- Open separate transcription app
- Switch context to record
- Copy/paste results
- Return to workflow

This breaks flow and reduces the advantage of voice input.

---

## 2. Core Insight

Voice input should be **system-level**: speak anywhere, get text instantly, use it anywhere — without breaking workflow.

---

## 3. Solution: Transcription Tray

A **system-level, real-time transcription overlay** that works across all applications.

---

## 4. User Experience Flow

### Background Behavior
- Vox runs lightweight VAD + STT service.
- Passively listens for speech.
- No manual trigger required (default passive mode).

### On Speech Detection
```
User speaks
  ↓
Speech detected → Audio engine wakes up (waveform indicator reacts if visible)
  ↓
First non-empty character transcribed → Overlay appears instantly
  ↓
Real-time transcription streams in
```

### During Active Speech
- Text updates continuously and statefully on screen.
- Turns are appended and separated cleanly by newlines (`\n`).
- Overlay remains open indefinitely to support pauses, thinking, and conversational continuity.

### On Session Conclusion / Clear
```
User clicks 'Close' in Header OR Auto-Sleep triggers (3 mins silence)
  ↓
Entire visual session text block is committed to History
  ↓
Active screen resets and clears
  ↓
Tray performs a 500ms exit fade-out and hides
```

---

## 5. Technical Implementation

### Architecture Integration

The Tray is a **direct surface** of the backend pipeline:

```
audio → VAD (300ms gate) → STT (coalesced partials) → dynamic throttle → tray UI
```

### Component Structure

```typescript
// TrayApp.tsx - Main overlay component
export const TrayApp: React.FC = () => {
  // State management
  const [interactionState, setInteractionState] = useState<string>("Idle");
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');

  // Ephemeral history (backend-backed completed sessions list)
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const [viewingHistory, setViewingHistory] = useState(false);

  // Interaction session management (accumulates turns with \n)
  const {
    committedText,
    partialText,
    startNewInteraction,
    updatePartial,
    commitFinal,
    reset
  } = useInteraction();

  // Simplified visibility state machine
  const {
    state: visibilityState,
    show,
    startFade,
    cancelFade,
    hideImmediately
  } = useVisibility();
};
```

### Visibility State Machine

```typescript
enum VisibilityState {
  HIDDEN = 'HIDDEN',      // HUD window is hidden from desktop
  APPEARING = 'APPEARING', // Fade-in and zoom transition
  ACTIVE = 'ACTIVE',      // Fully visible, persistent and interactive
  FADING = 'FADING'       // 500ms exit transition triggered on sleep/close
}
```

### IPC Event Handling

```typescript
useEffect(() => {
  const unlisteners = [
    window.listen("speech_start", () => {
      setViewingHistory(false);
      startNewInteraction(); // Intialize session ID
    }),

    window.listen("transcript_partial", ({ payload }) => {
      if (pttStatus === 'RECORDING') return;
      if (payload.text) {
        if (visibilityState === 'HIDDEN') show(); // Show HUD on first character
        updatePartial(payload.text);
      }
    }),

    window.listen("transcript_final", ({ payload }) => {
      if (payload.text) {
        commitFinal(payload.text); // Appends turn state with \n
      }
    }),

    window.listen("speech_end", () => {
      endSpeechSegment(); // Just update silence timestamp
    }),

    window.listen("auto_sleep_state", ({ payload: isSleeping }) => {
      if (isSleeping) {
        // Auto-sleep: commit session text and fadeout
        const textToCommit = liveTargetText;
        if (textToCommit.trim()) {
          invoke("commit_session_to_history", { text: textToCommit }).then(h => {
            setHistory(h);
          });
        }
        reset();
        startFade();
      } else {
        cancelFade();
      }
    }),
  ];

  return () => unlisteners.forEach(u => u());
}, [liveTargetText]);
```

---

## 6. UI Design Principles

### Session-Persistent by Design
- The HUD is **persistent** during transcription – it stays alive as you talk and pause.
- It groups multiple utterances and formats them into a readable transcription session context.
- **One workflow segment = one unified session context.**

### Zero Flicker
- Decoupling SpeechStart ensures clicking, mic thumps, and breathing noises **never** cause the HUD window to trigger or flicker.
- Safe thresholds on the backend (300ms sweet spot VAD gate) reject short pops, protecting system resource health.

### Non-Intrusive & Snappy
- Does not steal focus.
- Slide and fade transitions are optimized to **150ms entry / 500ms exit** for maximum snappiness.
- Dynamic Backoff Throttle ensures zero typing latency on high-performance rigs while safely avoiding execution spirals on slower CPUs.

---

## 7. Visual Design

### Container
- **Size**: 380px × 250px (fixed, non-resizable)
- **Position**: Right edge, vertically centered
- **Styling**: Glassmorphism with backdrop blur
- **Border**: Subtle translucent border
- **Shadow**: Soft drop shadow for depth

### Header
- **Status indicator**: Translucent/Glowing Active dot
- **Copy button**: One-click clipboard copy
- **Close button**: Dismiss HUD, commit context to history immediately, and hide.
- **PTT toggle**: Toggle push-to-talk manual override

### Content Area
- **Transcript display**: Streaming text formatted with newlines (`\n`) for clean turn boundaries
- **Overflow**: Auto-scroll for long transcripts

### Footer
- **History navigation**: Prev/Next buttons for recent transcripts
- **System stats**: CPU/RAM usage (optional)
- **History indicator**: Current position in history stack

### Animations
- **Entry**: Slide from right + fade in (200-300ms)
- **Exit**: Fade out + slide back (500-1500ms, configurable)
- **Text streaming**: Character-by-character (50 CPS)

---

## 8. Positioning & Platform Handling

### Desktop (macOS/Windows)
```typescript
// Uses tauri-plugin-positioner
await move_window(Position.TopRight);
```

### Linux (Wayland/X11)
```rust
// Creates fullscreen transparent window with input regions
let rect = cairo::RectangleInt::new(x, y, hud_w, hud_h);
let region = cairo::Region::create_rectangle(&rect);
gtk_window.input_shape_combine_region(Some(&region));
```

### Positioning Logic
- **Right edge** with configurable padding
- **Vertical centering** or slight upper bias
- **Screen-aware**: Adjusts for multiple monitors
- **Safe zones**: Avoids taskbars, docks, system UI

---

## 9. History System (Ephemeral)

### Design Constraints
- **In-memory only**: Never written to disk to protect user privacy.
- **Capacity**: Clamped to `5` sessions by default (user configurable up to 15) to maintain flat memory usage.
- **Clear on restart**: Wiped when the main runtime boots down.

---

## 10. Settings Integration

### Tray Config Parameters
```typescript
interface UiSettings {
  tray_enabled: boolean;              // Master HUD toggle
  tray_blur_density: number;          // Glassmorphism blur (px)
  tray_glass_tint: boolean;           // Ambient color tint
  tray_history_limit: number;         // Session memory depth (1 - 15)
}
```
*Note: Configs relating to automatic speech-end hold timers and custom transition animations are completely deprecated to guarantee standard, deterministic behavior.*