# Vox — Frontend Architecture (Dual-Surface UI System)

---

## 1. Overview

Vox frontend is a **multi-surface UI system**, not a single application UI. It consists of **two independent UI surfaces** that communicate via Tauri IPC:

1. **Main Application UI** (user-invoked, persistent)
2. **Ephemeral Overlay UI** (Tray HUD, system-triggered)

---

## 2. Tech Stack

### Core Framework
* React + TypeScript
* Vite (build system)
* Tauri (desktop runtime)

### State Management
* **zustand v5** (primary store) — `app/src/store/settingsStore.ts`
  * Selective subscriptions via `useSettingsStore(selector)` for zero unnecessary re-renders
  * Settings, draft settings, model catalog
  * Hot-applies theme/accent/interaction mode changes immediately
* **React Context (adapter)** — `SettingsContext.tsx` wraps zustand store for backward compat
  * Existing `useSettings()` consumers unchanged
  * **New components should use `useSettingsStore(selector)` directly**
* Custom hooks for interaction logic (useInteraction, useVisibility, useStreamingRenderer, useTelemetry)
* **Performance hooks** (Phase 0):
  * `useDynamicFPS` — RAF loop with frame-skipping (60/15/0 FPS tiers)
  * `usePerformanceMonitor` — Debug FPS tracker (dev-only)

### UI System
* TailwindCSS
* shadcn/ui (component primitives)
* Framer Motion (animations)
* Glassmorphism design system

### Audio Visualization
* ElevenLabs waveform component
* Custom Orb interface

---

## 3. Project Structure (app/src/)

```
app/src/
├── main.tsx                     # App entry point
├── App.tsx                      # Router setup
├── store/
│   └── settingsStore.ts         # Zustand store for settings (v5)
├── layout/
│   ├── ResponsiveLayout.tsx     # Main app layout (uses AmbientBackground)
│   ├── EdgeNav.tsx              # Unified bottom navigation strip (uses hover tooltips)
│   └── TitleBar.tsx             # Window controls
├── pages/
│   ├── Home.tsx                 # Orb interface page
│   ├── History.tsx              # Conversation history
│   ├── Settings.tsx             # Configuration page
│   └── Monitoring.tsx           # System monitoring
├── tray/
│   ├── TrayApp.tsx              # Overlay UI component
│   └── components/
│       ├── Header.tsx           # Tray header (status + controls)
│       ├── TranscriptRenderer.tsx # Live transcript display
│       └── Footer.tsx           # History navigation
├── wizard/
│   ├── WizardRoot.tsx           # First-run wizard root component
│   ├── steps/
│   │   ├── WelcomeStep.tsx      # Welcome screen
│   │   ├── SystemCheckStep.tsx   # Hardware/OS compatibility check
│   │   ├── ModelSetupStep.tsx   # Model download and selection
│   │   ├── AudioSetupStep.tsx   # Microphone/speaker test
│   │   ├── LiveTestStep.tsx     # End-to-end voice test
│   │   └── CompletedStep.tsx    # Setup complete confirmation
│   └── components/
│       ├── WizardHeader.tsx     # Wizard navigation header
│       ├── WizardFooter.tsx     # Wizard action buttons
│       ├── ModelCategory.tsx    # Model category selector
│       └── StatusCard.tsx       # Status indicator card
├── shared/
│   ├── components/
│   │   ├── AdvancedOrb.tsx           # Central AI state orb (useDynamicFPS optimized)
│   │   ├── AmbientBackground.tsx     # Animated deep-space ambient background
│   │   ├── GlassCard.tsx             # Glassmorphism container
│   │   ├── LiveWaveform.tsx          # Audio visualization (useDynamicFPS optimized)
│   │   ├── PillButton.tsx            # Custom button component
│   │   ├── RestartModal.tsx          # Settings restart prompt
│   │   ├── Typography.tsx            # Text components
│   │   ├── VoxLogo.tsx               # Brand logo component
│   │   ├── CoreSettings.tsx          # Core settings panel
│   │   ├── ModelSettings.tsx         # LLM/STT/TTS model selection
│   │   └── TraySettings.tsx          # Tray/overlay settings panel
│   ├── hooks/
│   │   ├── useDynamicFPS.ts          # RAF loop with frame-skipping (60/15/0 FPS)
│   │   ├── usePerformanceMonitor.ts  # Debug FPS tracker (dev-only)
│   │   ├── useInteraction.ts         # Interaction session management
│   │   ├── useStreamingRenderer.ts   # Text streaming animation
│   │   ├── useTelemetry.ts           # Telemetry data hooks
│   │   └── useVisibility.ts          # Tray visibility logic
│   ├── context/
│   │   └── SettingsContext.tsx        # Settings provider (zustand adapter)
│   └── lib/
│       └── utils.ts                   # Utility functions
```

---

## 4. Window Architecture (Tauri)

### Window Types

```typescript
interface WindowLabels {
  "main": WebviewWindow;  // Main application window
  "tray": WebviewWindow;  // Overlay HUD window
}
```

### Main Window Configuration

```json
{
  "label": "main",
  "title": "Vox",
  "width": 800,
  "height": 600,
  "minWidth": 400,
  "minHeight": 300,
  "decorations": true,
  "resizable": true,
  "center": true
}
```

### Overlay Window Configuration

```json
{
  "label": "tray",
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "resizable": false,
  "visible": false,
  "width": 380,
  "height": 250
}
```

### Platform-Specific Setup

#### Linux (Wayland/X11)
- Fullscreen transparent window
- Input shape regions for click-through
- Virtual layer positioning

#### macOS/Windows
- Standard overlay positioning
- Native always-on-top behavior

---

## 5. Main Application UI

### Layout System

The main app uses a **responsive layout system** that adapts to window size:

#### Desktop Mode (>768px)
- Sidebar navigation
- Full page content
- Persistent window controls

#### Mobile Mode (≤768px)
- Bottom tab navigation
- Stacked page layout
- Touch-optimized interactions

### Navigation Structure

```typescript
const routes = [
  { path: "/", label: "Home", icon: HomeIcon },
  { path: "/history", label: "History", icon: HistoryIcon },
  { path: "/settings", label: "Settings", icon: SettingsIcon },
  { path: "/monitoring", label: "Monitor", icon: MonitorIcon },
];
```

### Home Page (Orb Interface)

#### AdvancedOrb Component

```typescript
interface OrbState {
  isListening: boolean;
  isSpeaking: boolean;
  volumeLevel: number;
  interactionState: InteractionState;
}

const OrbVariants = {
  idle: { scale: 1, opacity: 0.7 },
  listening: { scale: 1.1, opacity: 1 },
  speaking: { scale: 1.2, opacity: 1 },
  thinking: { scale: 1.05, opacity: 0.9 },
};
```

#### State Synchronization

```typescript
useEffect(() => {
  const unlisteners = [
    window.listen("state_changed", ({ payload }) => {
      setInteractionState(payload);
    }),
    window.listen("audio_energy", ({ payload }) => {
      setVolumeLevel(payload.energy);
    }),
  ];
  return () => unlisteners.forEach(u => u());
}, []);
```

### History Page

#### Data Structure

```typescript
interface Session {
  id: number;
  started_at: string;
  ended_at?: string;
  turn_count: number;
  turns: Turn[];
}

interface Turn {
  id: number;
  user_text: string;
  assistant_text: string;
  stt_latency_ms: number;
  ttft_ms: number;
  created_at: string;
}
```

#### IPC Integration

```typescript
const [sessions, setSessions] = useState<Session[]>([]);

useEffect(() => {
  invoke<Session[]>("get_sessions").then(setSessions);
}, []);

const loadTurns = (sessionId: number) => {
  invoke<Turn[]>("get_turns", { sessionId }).then(setTurns);
};
```

### Settings Page

#### Settings Structure

```typescript
interface VoxSettings {
  ui: {
    theme: string;
    accent_seed: string;
    tray_enabled: boolean;
    tray_blur_density: number;
    tray_glass_tint: boolean;
    tray_hide_delay: number;
    tray_fade_transition: string;
    tray_history_limit: number;
  };
  audio: {
    output_mode: "Speaker" | "Headset";
    input_device: string | null;
  };
  vad: {
    threshold: number;
    ptt_noise_gate: number;
    vad_backend: "Earshot" | "TenVad";
  };
  asr: {
    model: string;  // "nvidia_nemotron" | "qwen3_asr"
    transliterate_enabled: boolean;
  };
  llm: {
    model: string;
    ctx_size: number;
    threads: number;
  };
  tts: {
    voice: number;         // Supertonic voice index (0-9)
    quality_steps: number; // Supertonic diffusion steps (2-12)
    speed: number;         // Speed factor (0.7-2.0)
  };
  interaction: {
    main_app_mode: "Passive" | "PTT";
    tray_mode: "Passive" | "PTT";
    auto_sleep_timeout: number;
  };
  telemetry: {
    enabled: boolean;
    log_level: string;
  };
  persistence: {
    enabled: boolean;
    private_mode: boolean;
    max_sessions: number;
    retention_days: number;
  };
  assistant: {
    system_prompt: string;
    hindi_prompt: string;
    english_prompt: string;
  };
  setup: {
    completed: boolean;
  };
}
```

#### Hot-Reload Logic

```typescript
const updateSetting = async (domain: string, key: string, value: any) => {
  const reloadPolicy = await invoke<string>("reload_policy_for", { domain, key });

  if (reloadPolicy === "restart") {
    setShowRestartModal(true);
  }

  await invoke("update_setting", { domain, key, value });
  updateLocalSettings(domain, key, value);
};
```

### Monitoring Page

#### Runtime Metrics

```typescript
interface RuntimeSnapshot {
  timestamp: number;
  system_cpu: number;
  system_ram_pct: number;
  vox_cpu: number;
  vox_ram_mb: number;
  threads: number;
  stt_ms: number;
  ttft_ms: number;
  voice_latency_ms: number;
  tts_rtf: number;
  playback_start_ms: number;
  persistence_rate: number;
  playback_underruns: number;
}
```

#### Live Charts

Uses Chart.js or similar for:
- CPU usage over time
- Memory usage trends
- Latency distributions
- Throughput metrics

---

## 6. Overlay UI (Tray HUD)

### Core Concept

The Tray HUD is an **ephemeral transcription capsule** that:

- Appears automatically on speech detection
- Displays real-time transcription
- Disappears after silence
- Never persists or accumulates history

### Visibility State Machine

```typescript
enum VisibilityState {
  HIDDEN = 'HIDDEN',
  APPEARING = 'APPEARING',
  ACTIVE = 'ACTIVE',
  HOLD = 'HOLD',
  FADING = 'FADING'
}

interface VisibilityConfig {
  holdDuration: number;    // ms to hold after speech end
  fadeDuration: number;    // ms for fade animation
}
```

### Positioning Logic

#### Linux (Virtual Layer)

```typescript
const setup_linux_virtual_layer = (app: AppHandle, label: string) => {
  const window = app.get_webview_window(label);
  const monitor = window.primary_monitor();

  // Position at top-right with padding
  const x = screen_w - hud_w - padding_x;
  const y = (screen_h * 0.15); // 15vh from top

  // Create input shape region for click-through
  const region = cairo::Region::create_rectangle(rect);
  gtk_window.input_shape_combine_region(Some(&region));
};
```

#### macOS/Windows

```typescript
// Use tauri-plugin-positioner
window.move_window(Position::TopRight);
```

### Component Architecture

#### TrayApp.tsx (Root Component)

```typescript
const TrayApp: React.FC = () => {
  const { settings } = useSettings();
  const [interactionState, setInteractionState] = useState<string>("Idle");
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');

  // History system (ephemeral, in-memory only)
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const [viewingHistory, setViewingHistory] = useState(false);

  // Interaction management
  const {
    interactionId,
    committedText,
    partialText,
    startNewInteraction,
    endSpeechSegment,
    updatePartial,
    commitFinal,
    reset
  } = useInteraction();

  // Visibility management
  const {
    state: visibilityState,
    setIsHovered,
    show,
    startHold,
    hideImmediately
  } = useVisibility({
    holdDuration: (settings?.ui.tray_hide_delay || 3) * 1000,
    fadeDuration: settings?.ui.tray_fade_transition === 'Snappy' ? 500 : 1500
  });
};
```

#### Header Component

```typescript
interface HeaderProps {
  isListening: boolean;
  hasContent: boolean;
  copied: boolean;
  isPttActive: boolean;
  interactionMode: string;
  onCopy: () => void;
  onClose: () => void;
  onTogglePtt: () => void;
}
```

#### TranscriptRenderer Component

```typescript
interface TranscriptRendererProps {
  displayText: string;
  interactionState: string;
  pttStatus: string;
  telemetryRef: React.RefObject<any>;
}
```

### Text Streaming Animation

#### useStreamingRenderer Hook

```typescript
const useStreamingRenderer = (targetText: string) => {
  const [displayText, setDisplayText] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);

  useEffect(() => {
    if (targetText === displayText) return;

    setIsStreaming(true);
    const streamText = async () => {
      // Character-by-character streaming animation
      for (let i = 0; i <= targetText.length; i++) {
        setDisplayText(targetText.slice(0, i));
        await new Promise(resolve => setTimeout(resolve, 20)); // 50 CPS
      }
      setIsStreaming(false);
    };

    streamText();
  }, [targetText]);

  return displayText;
};
```

### History Navigation

#### Ephemeral History System

```typescript
const MAX_HISTORY = settings?.ui.tray_history_limit || 10;

const addToHistory = (text: string) => {
  setHistory(prev => [text, ...prev.slice(0, MAX_HISTORY - 1)]);
};

const navigateHistory = (direction: 'prev' | 'next') => {
  setViewingHistory(true);
  setHistoryIndex(prev => {
    if (direction === 'prev') {
      return prev === -1 ? history.length - 1 : Math.max(0, prev - 1);
    } else {
      return prev <= 0 ? -1 : prev - 1;
    }
  });
};
```

---

## 7. State Management Patterns

### Interaction Management (useInteraction)

```typescript
const CONTINUITY_WINDOW = 1200; // ms

export const useInteraction = () => {
  const [interactionId, setInteractionId] = useState(0);
  const [committedText, setCommittedText] = useState("");
  const [partialText, setPartialText] = useState("");

  const lastSpeechEndTime = useRef<number>(0);
  const currentIdRef = useRef<number>(0);

  const startNewInteraction = useCallback(() => {
    const now = Date.now();
    const diff = now - lastSpeechEndTime.current;

    // Continuity logic: merge if within window
    if (diff > CONTINUITY_WINDOW || currentIdRef.current === 0) {
      currentIdRef.current += 1;
      setInteractionId(currentIdRef.current);
      setCommittedText("");
      setPartialText("");
    }
  }, []);

  const commitFinal = useCallback((text: string) => {
    if (!text) return;
    setCommittedText(prev => prev ? `${prev} ${text}` : text);
    setPartialText("");
  }, []);
};
```

### Visibility Management (useVisibility)

```typescript
const useVisibility = (config: VisibilityConfig) => {
  const [state, setState] = useState<VisibilityState>('HIDDEN');
  const [isHovered, setIsHovered] = useState(false);

  const holdTimeoutRef = useRef<NodeJS.Timeout>();
  const fadeTimeoutRef = useRef<NodeJS.Timeout>();

  const show = useCallback(() => {
    setState('APPEARING');
    setTimeout(() => setState('ACTIVE'), 50);
  }, []);

  const startHold = useCallback(() => {
    setState('HOLD');
    holdTimeoutRef.current = setTimeout(() => {
      if (!isHovered) {
        setState('FADING');
        fadeTimeoutRef.current = setTimeout(() => {
          setState('HIDDEN');
        }, config.fadeDuration);
      }
    }, config.holdDuration);
  }, [config, isHovered]);
};
```

### Settings Context

```typescript
const SettingsContext = createContext<SettingsContextType>({
  settings: null,
  isLoading: true,
  updateSetting: async () => {},
});

export const SettingsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [settings, setSettings] = useState<VoxSettings | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    invoke<VoxSettings>('get_settings')
      .then(setSettings)
      .finally(() => setIsLoading(false));
  }, []);

  const updateSetting = async (domain: string, key: string, value: any) => {
    await invoke('update_setting', { domain, key, value });
    setSettings(prev => prev ? deepMerge(prev, { [domain]: { [key]: value } }) : null);
  };
};
```

---

## 8. IPC Communication

### Event Listeners

#### Main Window Events

```typescript
// State synchronization
window.listen("state_changed", ({ payload }) => {
  setInteractionState(payload);
});

// Audio visualization
window.listen("audio_energy", ({ payload }) => {
  setVolumeLevel(payload.energy);
});

// PTT status updates
window.listen("ptt_status", ({ payload }) => {
  setPttStatus(payload.state);
});
```

#### Tray Window Events

```typescript
// Speech lifecycle
window.listen("speech_start", () => {
  startNewInteraction();
  show();
});

window.listen("transcript_partial", ({ payload }) => {
  updatePartial(payload.text);
});

window.listen("transcript_final", ({ payload }) => {
  commitFinal(payload.text);
  addToHistory(payload.text);
  startHold();
});

window.listen("speech_end", () => {
  endSpeechSegment();
  startHold();
});
```

### Command Invocations

```typescript
// Settings management
await invoke("get_settings");
await invoke("update_setting", { domain, key, value });

// Engine control
await invoke("engage");
await invoke("check_engine_status");

// History management
const history = await invoke<string[]>("get_transcript_history");
const sessions = await invoke<Session[]>("get_sessions");
```

---

## 9. Performance Optimizations

### React Optimizations

```typescript
// Memoized components
const Header = memo(({ ...props }) => { ... });

// Callback stabilization
const handleCopy = useCallback(() => {
  navigator.clipboard.writeText(text);
}, [text]);

// Ref-based state for hot paths
const telemetryRef = useRef({ energy: 0, vadProb: 0 });
```

### Animation Performance

```typescript
// Hardware acceleration
const containerVariants = {
  ACTIVE: {
    opacity: 1,
    x: 0,
    scale: 1,
    willChange: "transform, opacity"
  }
};

// Reduced motion support
const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
```

### Bundle Optimization

```typescript
// Lazy loading
const Home = lazy(() => import("@/pages/Home"));
const History = lazy(() => import("@/pages/History"));

// Code splitting
const Monitoring = lazy(() => import("@/pages/Monitoring").then(m => ({
  default: m.Monitoring
})));
```

---

## 10. Design System

### Color Palette

```css
:root {
  --background: 0 0% 4%;      /* Dark obsidian */
  --foreground: 0 0% 98%;
  --accent: 180 100% 50%;      /* Cyan glow */
  --accent-foreground: 0 0% 4%;
  --secondary: 262 83% 58%;    /* Violet accents */
}
```

### Glassmorphism Implementation

```css
.liquid-glass {
  background: rgba(var(--background), 0.8);
  backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}
```

### Orb Animation System

```typescript
const orbAnimations = {
  idle: {
    scale: [1, 1.02, 1],
    opacity: [0.7, 0.8, 0.7],
    transition: {
      duration: 3,
      repeat: Infinity,
      ease: "easeInOut"
    }
  },
  listening: {
    scale: 1.1,
    boxShadow: "0 0 30px rgba(var(--accent), 0.5)",
    transition: { duration: 0.3 }
  },
  speaking: {
    scale: 1.2,
    boxShadow: "0 0 50px rgba(var(--accent), 0.8)",
    transition: { duration: 0.2 }
  }
};
```

---

## 11. Platform-Specific Code

### Linux (Wayland/X11)

```typescript
// Click-through regions
const setupVirtualLayer = async () => {
  const window = getCurrentWindow();
  const monitor = await window.primaryMonitor();

  // Position calculations in logical pixels
  const scaleFactor = await window.scaleFactor();
  const hud_w = 380 * scaleFactor;
  const hud_h = 250 * scaleFactor;

  // GTK input shape for click-through
  // (Handled in Rust backend)
};
```

### macOS/Windows

```typescript
// Native overlay positioning
import { move_window, Position } from "@tauri-apps/plugin-positioner";

await move_window(Position.TopRight);
```

---

## 12. Error Handling & Resilience

### IPC Error Handling

```typescript
try {
  const result = await invoke("some_command", params);
  // Handle success
} catch (error) {
  console.warn("[UI] IPC failed:", error);
  // Graceful degradation
}
```

### Component Error Boundaries

```typescript
class ErrorBoundary extends React.Component {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error, errorInfo) {
    console.error("Component error:", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return <div>Something went wrong. Please restart the app.</div>;
    }
    return this.props.children;
  }
}
```

### Settings Recovery

```typescript
const loadSettings = async () => {
  try {
    const settings = await invoke<VoxSettings>("get_settings");
    setSettings(settings);
  } catch (error) {
    console.error("Failed to load settings:", error);
    // Use defaults or cached settings
  }
};
```

