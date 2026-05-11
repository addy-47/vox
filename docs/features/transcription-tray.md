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
- Vox runs lightweight VAD + STT service
- Passively listens for speech
- No manual trigger required (default mode)

### On Speech Detection
```
User speaks
  ↓
Speech detected → Small overlay appears (right edge)
  ↓
Real-time transcription streams in
  ↓
UI remains minimal, non-intrusive
```

### During Active Speech
- Text updates continuously (no waiting for full sentence)
- Tray stays visible, follows speech

### On Silence End
```
Silence detected (300ms)
  ↓
Transcription finalizes
  ↓
Tray fades out and disappears
  ↓
Next speech → New tray instance
```

---

## 5. Technical Implementation

### Architecture Integration

The Tray is a **direct surface** of the backend pipeline:

```
audio → VAD → STT → transcript events → tray UI
```

### Component Structure

```typescript
// TrayApp.tsx - Main overlay component
interface TrayAppProps {
  settings: VoxSettings;
}

const TrayApp: React.FC = () => {
  // State management
  const [interactionState, setInteractionState] = useState<string>("Idle");
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');

  // Ephemeral history (in-memory only, never persisted)
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const [viewingHistory, setViewingHistory] = useState(false);

  // Interaction session management
  const {
    committedText,
    partialText,
    startNewInteraction,
    updatePartial,
    commitFinal,
    reset
  } = useInteraction();

  // Visibility state machine
  const {
    state: visibilityState,
    show,
    startHold,
    hideImmediately
  } = useVisibility({
    holdDuration: settings.ui.tray_hide_delay * 1000,
    fadeDuration: settings.ui.tray_fade_transition === 'Snappy' ? 500 : 1500
  });
};
```

### Visibility State Machine

```typescript
enum VisibilityState {
  HIDDEN = 'HIDDEN',      // Not visible
  APPEARING = 'APPEARING', // Fade in animation
  ACTIVE = 'ACTIVE',      // Fully visible, interactive
  HOLD = 'HOLD',          // Holding after speech end
  FADING = 'FADING'       // Fade out animation
}
```

### IPC Event Handling

```typescript
useEffect(() => {
  const unlisteners = [
    // Speech lifecycle events
    window.listen("speech_start", () => {
      setViewingHistory(false);
      startNewInteraction();
      show(); // Show overlay
    }),

    window.listen("transcript_partial", ({ payload }) => {
      if (pttStatus === 'RECORDING') return; // Skip during PTT
      updatePartial(payload.text);
    }),

    window.listen("transcript_final", ({ payload }) => {
      if (payload.text) {
        commitFinal(payload.text);
        // Add to ephemeral history
        setHistory(prev => [payload.text, ...prev.slice(0, MAX_HISTORY - 1)]);
      }
    }),

    window.listen("speech_end", () => {
      endSpeechSegment();
      startHold(); // Begin hold before hide
    }),

    // UI state events
    window.listen("state_changed", ({ payload }) => {
      setInteractionState(payload);
    }),

    window.listen("ptt_status", ({ payload }) => {
      setPttStatus(payload.state);
    }),
  ];

  return () => unlisteners.forEach(u => u());
}, []);
```

---

## 6. UI Design Principles

### Ephemeral by Design
- Tray is **temporary** - exists only during active speech
- **Never persists** or accumulates history
- **One speech session = one UI instance**

### Zero Friction
User should **never** need to:
- Click anything to start
- Switch applications
- Manually trigger recording

### Non-Intrusive
- Does not steal focus
- Does not block interactions
- Appears softly, disappears cleanly

### Instant Feedback
- Partial transcription appears immediately
- No waiting for sentence completion
- Streaming text updates

---

## 7. Visual Design

### Container
- **Size**: 380px × 250px (fixed, non-resizable)
- **Position**: Right edge, vertically centered
- **Styling**: Glassmorphism with backdrop blur
- **Border**: Subtle translucent border
- **Shadow**: Soft drop shadow for depth

### Header
- **Status indicator**: LIVE/Idle dot
- **Copy button**: One-click clipboard copy
- **Close button**: Manual dismiss (optional)
- **PTT toggle**: Button for push-to-talk mode

### Content Area
- **Transcript display**: Streaming text with typewriter animation
- **Typography**: Clean, readable font
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
- **Memory only**: Never written to disk
- **Limited retention**: Max 10 items (configurable)
- **No persistence**: Cleared on app restart
- **Read-only**: Historical transcripts cannot be edited

### Navigation
```typescript
const handlePrev = () => {
  if (history.length === 0) return;
  setViewingHistory(true);
  setHistoryIndex(prev => prev === -1 ? history.length - 1 : Math.max(0, prev - 1));
};

const handleNext = () => {
  setHistoryIndex(prev => {
    if (prev === -1 || prev >= history.length - 1) {
      setViewingHistory(false);
      return -1;
    }
    return prev + 1;
  });
};
```

### Integration with Live Mode
- **Live transcripts**: `historyIndex = -1`
- **History viewing**: `historyIndex >= 0`
- **Seamless switching**: Between live and historical views

---

## 10. Settings Integration

### Tray-Specific Settings
```typescript
interface TraySettings {
  tray_enabled: boolean;              // Master toggle
  tray_blur_density: number;           // Backdrop blur (px)
  tray_glass_tint: boolean;           // Accent color tint
  tray_hide_delay: number;            // Hold duration (seconds)
  tray_fade_transition: string;       // 'Snappy' | 'Smooth' | 'Gentle'
  tray_history_limit: number;         // Max history items
}
```

### Reload Behavior
- **Hot reload**: All tray settings apply immediately
- **No restart required**: Changes take effect instantly
- **Validation**: Settings clamped to safe ranges

---

## 11. Performance Constraints

### Memory Usage
- **Minimal footprint**: <50MB additional RAM
- **Efficient rendering**: No heavy animations or effects
- **Text streaming**: Throttled character updates

### CPU Usage
- **Idle**: ~0% CPU when hidden
- **Active**: <5% CPU during transcription
- **Animation**: Hardware-accelerated transforms

### Battery Impact
- **Passive listening**: Minimal drain
- **Active transcription**: Moderate drain
- **Animation**: GPU-accelerated, minimal impact

---

## 12. Error Handling & Resilience

### IPC Failures
```typescript
// Graceful degradation
try {
  await invoke("sync_hud_visibility", { visible: true });
} catch (error) {
  console.warn("[Tray] IPC failed:", error);
  // Continue with local state
}
```

### Component Crashes
```typescript
// Error boundary
class TrayErrorBoundary extends React.Component {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  render() {
    if (this.state.hasError) {
      return <div>Tray temporarily unavailable</div>;
    }
    return this.props.children;
  }
}
```

### Backend Disconnection
- **Auto-hide**: Tray disappears if backend unavailable
- **Reconnection**: Automatically shows when backend recovers
- **State reset**: Clean slate on reconnection

---

## 13. Accessibility

### Keyboard Navigation
- **Tab order**: Logical focus flow
- **Escape**: Dismiss tray
- **Arrow keys**: History navigation
- **Enter/Space**: Copy to clipboard

### Screen Reader Support
- **ARIA labels**: Descriptive labels for all controls
- **Live regions**: Transcript updates announced
- **Status messages**: State changes communicated

### High Contrast
- **Color schemes**: Respects system preferences
- **Focus indicators**: Clear focus outlines
- **Text contrast**: WCAG AA compliant



---

## 16. Relationship to Core System

### Backend Dependencies
- **VAD engine**: Speech detection triggers
- **STT pipeline**: Real-time transcription
- **Event bus**: State synchronization

### Integration Points
- **Settings system**: Configuration persistence
- **Monitoring**: Performance telemetry
- **Persistence**: History management (future)

### Architectural Fit
The Tray is the **primary user interface** for passive voice interaction, complementing the main application UI for explicit control.

---

## 17. Success Metrics

### User Experience
- **Time-to-text**: <500ms perceived latency
- **Workflow disruption**: Zero context switches
- **Learnability**: Intuitive, no training required

### Technical Performance
- **Reliability**: 99.9% uptime
- **Responsiveness**: 60fps animations
- **Efficiency**: <5% CPU usage during active transcription

### Adoption
- **Feature usage**: >80% of voice interactions through tray
- **User satisfaction**: Positive feedback on friction reduction
- **Retention**: Increased daily active usage