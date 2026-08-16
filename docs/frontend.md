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
├── App.tsx                      # Router setup, lazy loading, setup-completed gate
├── index.css                    # Global styles
├── layout/
│   ├── ResponsiveLayout.tsx     # Main app layout (uses AmbientBackground)
│   ├── EdgeNav.tsx              # Unified bottom navigation strip (uses hover tooltips)
│   └── TitleBar.tsx             # Window controls
├── pages/
│   ├── Home.tsx                 # Orb interface page (~1028 lines, pipeline/realtime control)
│   ├── History.tsx              # Conversation history
│   ├── Settings.tsx             # Full settings page with card-based layout
│   ├── Monitoring.tsx           # System monitoring dashboard
│   └── Memory.tsx               # 3D Cognitive Memory Graph & Ingestion Queue
├── tray/
│   ├── TrayApp.tsx              # Overlay UI root component (~376 lines)
│   └── components/
│       ├── Header.tsx           # Tray header (status + controls)
│       ├── TranscriptRenderer.tsx # Live transcript display
│       └── Footer.tsx           # History navigation
├── wizard/
│   ├── WizardRoot.tsx           # XState-driven first-run setup wizard
│   ├── state/
│   │   └── setupMachine.ts      # XState machine definition
│   ├── steps/
│   │   ├── WelcomeStep.tsx      # Welcome screen
│   │   ├── SystemCheckStep.tsx  # Hardware/OS compatibility check
│   │   ├── ModelSetupStep.tsx   # Model download and selection
│   │   ├── AudioSetupStep.tsx   # Microphone/speaker test
│   │   ├── LiveTestStep.tsx     # End-to-end voice test
│   │   └── CompletedStep.tsx    # Setup complete confirmation
│   └── components/
│       ├── WizardHeader.tsx     # Wizard navigation header
│       ├── WizardFooter.tsx     # Wizard action buttons
│       ├── ModelCategory.tsx    # Model category selector
│       └── StatusCard.tsx       # Status indicator card
├── store/
│   └── settingsStore.ts         # Zustand v5 store (295 lines, draft/committed pattern)
└── shared/
    ├── components/
    │   ├── AdvancedOrb.tsx           # Central AI state orb (useDynamicFPS optimized)
    │   ├── AmbientBackground.tsx     # Animated deep-space ambient background
    │   ├── ErrorBoundary.tsx         # React error boundary
    │   ├── GlassCard.tsx             # Glassmorphism container
    │   ├── GlassSkeleton.tsx         # Loading skeleton with glass styling
    │   ├── LiveWaveform.tsx          # Audio visualization (useDynamicFPS optimized)
    │   ├── MonitoringPopover.tsx     # Telemetry popover overlay
    │   ├── PipelineField.tsx         # Pipeline mode display field
    │   ├── StatusCapsule.tsx         # Status indicator capsule
    │   └── settings/                # Settings sub-components
    │       ├── cards/                # Individual settings cards
    │       └── overlays/             # Settings overlay modals
    ├── hooks/
    │   ├── useDynamicFPS.ts          # RAF loop with frame-skipping (60/15/0 FPS)
    │   ├── usePerformanceMonitor.ts  # Debug FPS tracker (dev-only)
    │   ├── useInteraction.ts         # Interaction session management
    │   ├── useStreamingRenderer.ts   # Text streaming animation
    │   ├── useTelemetry.ts           # Telemetry data hooks
    │   ├── useVisibility.ts          # Tray visibility logic
    │   └── useVoxFootprint.ts        # Runtime memory/footprint tracking
    ├── context/
    │   └── SettingsContext.tsx        # Settings provider (zustand adapter)
    └── lib/
        └── utils.ts                   # cn() helper, hexToRgb, etc.
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
  "width": 400,
  "height": 800,
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
  "width": 420,
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

#### Realtime Session Lifecycle (v0.9.0)

Home.tsx manages the full realtime S2S session lifecycle with local state
(`useState` + `useRef`, no Zustand for hot paths):

- **Pipeline mode**: `"modular"` or `"realtime"` — set by backend on `engage`.
  The `launch_engine` command conditionally spawns VAD/STT based on active mode.
- **Engage handler**: Calls `start_realtime_session` (realtime) or `engage` (modular)
  based on settings. Uses `engageLockRef` to prevent double-engage during WS handshake.
- **End handler**: Calls `stop_realtime_session` — disconnects WS, clears session cache,
  reverts to modular mode, archives conversation.
- **Pause/Resume**: Calls `pause_pipeline`/`resume_pipeline` IPC — sets `is_paused` atomic
  (router drops chunks), calls `activity_end()` on session, stops playback; resume
  reopens audio gate, lazy-reconnects WS if disconnected.
- **PTT toggle**: Calls `ptt_start`/`ptt_stop` — only rendered when `interactionMode === "PTT"`.
  In realtime PTT, triggers `activity_start`/`activity_end` over WebSocket.
- **Session cache**: On mount, calls `get_realtime_session_cache` IPC. If a valid cached
  session exists (Gemini provides 2-hour resumption handles), the engage button shows
  "Resume Session" instead of "Engage". Session is persisted to `~/.vox/cache/realtime_session.json`.

Three-button control group layout:
```
NOT Engaged:        [Power icon] (enables engagement)
Engaged + Passive:  [Pause/Resume icon] [X icon (disengage)]
Engaged + PTT:      [Pause/Resume icon] [Mic icon] [X icon (disengage)]
```

#### State Synchronization

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

The Settings page is **fully responsive** — cards (`AppearanceCard`, `InteractionCard`, `MemoryCard`, `ModelsCard`, `PersonaCard`, `TrayCard`) and the `RestoreDefaultsButton` overlay adapt layout across desktop and mobile viewports.

#### Settings Structure

```typescript
interface VoxSettings {
  ui: {
    theme: string;
    accent_seed: string;
    tray_enabled: boolean;
    tray_blur_density: number;
    tray_glass_tint: boolean;
    tray_history_limit: number;
  };
  audio: {
    output_mode: "Speaker" | "Headset";
    input_device: string | null;
  };
  vad: {
    threshold: number;           // 0.0-1.0 (default 0.5)
    ptt_noise_gate: number;      // 0.0-1.0 (default 0.005)
    vad_backend: "Earshot" | "TenVad";  // TenVad=default, Earshot=preferred
  };
  asr: {
    model: string;               // "nvidia_nemotron" | "qwen3_asr"
    transliterate_enabled: boolean;
    provider: {                  // SttProviderConfig tagged enum
      kind: "embedded" | "cloud";
      model_type?: string;       // embedded only
      provider?: string;         // cloud only: "google" | "deepgram" | "whisperflow"
    };
  };
  llm: {
    model: string;
    ctx_size: number;            // 1024-4096 (default 2048)
    threads: number;             // 1-N (default 4)
    provider: {                  // LlmProviderConfig tagged enum
      kind: "embedded" | "open_ai_compat";
      base_url?: string;
      model?: string;
      api_key?: string;
      provider_name?: string;    // "openai" | "gemini" | "anthropic"
    };
  };
  tts: {
    provider: {                  // TtsProviderConfig tagged enum
      kind: "supertonic" | "chatterbox" | "chatterbox_remote";
      language?: string;
      quality_steps?: number;
      speed?: number;
      voice_id?: string;
      endpoint?: string;         // remote only
      remote_path?: string;      // remote only
    };
    voice: number;               // Supertonic voice index (0-9)
    quality_steps: number;       // Diffusion steps (2-12, default 12)
    speed: number;               // Speed factor (0.7-2.0, default 1.05)
  };
  interaction: {
    main_app_mode: "Passive" | "PTT";
    tray_mode: "Passive" | "PTT";
    pipeline_mode: "Modular" | "Realtime";  // v0.9.0
    auto_sleep_timeout: number;  // seconds (default 400)
  };

  // Realtime S2S settings (v0.9.0)
  realtime: {
    provider: "gemini_live" | "openai_realtime" | "deepgram_voice_agent" | "elevenlabs_convai";
    gemini: {
      api_key: string;
      model: string;             // default "gemini-2.0-flash-live-001"
      voice_name: string;        // default "Aoede"
      language_code: string;     // BCP-47, default "en-US"
      temperature: number;       // default 0.2
      enable_web_search: boolean;
    };
    openai: {
      api_key: string;
      model: string;
    };
    deepgram: {
      api_key: string;
      model: string;
    };
    elevenlabs: {
      api_key: string;
      agent_id: string;
    };
  };
  telemetry: {
    enabled: boolean;            // default true
    log_level: string;           // default "info"
  };
  persistence: {
    private_mode: boolean;       // default false
    max_sessions: number;        // default 500
    retention_days: number;      // default 30
  };
  assistant: {
    modular_prompt: string;      // Hindi prompt (alias: hindi_prompt)
    realtime_prompt: string;     // English prompt (alias: english_prompt)
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

#### Offload / Reload Dual-Button UI

The monitoring page uses **conditional button rendering** for engine lifecycle control:

- **Skull button** (red): displayed when `models_loaded === true`. Clicking it invokes `stop_engine` to offload/unload models from memory.
- **RefreshCw button**: displayed when `models_loaded === false`. Clicking it invokes `launch_engine` to reload models into memory.

Only one button is visible at any time based on the current engine state.

#### Runtime Metrics

```typescript
interface RuntimeSnapshot {
  timestamp: number;
  system_cpu: number;       // CPU usage percentage
  system_ram_pct: number;   // System RAM usage percentage
  system_ram_gb: number;    // System RAM in GB
  vox_cpu: number;          // Vox process CPU usage
  vox_ram_mb: number;       // Vox process RAM in MB
  threads: number;          // Active thread count
  stt_rtf: number;          // STT real-time factor
  ttft_ms: number;          // Time to first token
  ttfa_ms: number;          // Time to first audio (voice pipeline latency)
  tts_rtf: number;          // TTS real-time factor
  playback_start_ms: number; // Time from TTS chunk to playback start
  persistence_rate: number;  // Persistence write rate (events/sec)
  playback_underruns: number; // Playback buffer underrun count
  audio_energy: number;      // Current mic RMS energy
  models_loaded: boolean;    // Whether models are warm
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
  const settings = useSettingsStore(s => s.settings);
  const [interactionState, setInteractionState] = useState<string>("Idle");
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');
  const [processingMessage, setProcessingMessage] = useState(false);

  // Audio visualization
  const [audioEnergy, setAudioEnergy] = useState(0);

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
    fadeDuration: 500, // Always snappy for tray
  });

  // Telemetry ref for live data
  const telemetryRef = useRef({ energy: 0, vadProb: 0 });

  // Listen for IPC events
  useEffect(() => {
    const unlisteners = [
      listen("state_changed", (e) => setInteractionState(e.payload as string)),
      listen("audio_energy", (e) => { setAudioEnergy(e.payload.energy); }),
      listen("ptt_status", (e) => setPttStatus(e.payload.state)),
      // ... more listeners
    ];
    return () => unlisteners.forEach(u => u());
  }, [settings]);
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
  const prevTargetRef = useRef("");

  useEffect(() => {
    if (targetText === prevTargetRef.current) return;
    prevTargetRef.current = targetText;

    // Character-by-character streaming animation
    let i = 0;
    const interval = setInterval(() => {
      i++;
      setDisplayText(targetText.slice(0, i));
      if (i >= targetText.length) clearInterval(interval);
    }, 20); // 50 CPS

    return () => clearInterval(interval);
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

// Realtime session lifecycle (v0.9.0)
window.listen("realtime_session_started", () => {
  setPipelineMode("realtime");
  setSessionResumed(false);
});

window.listen("realtime_session_resumed", () => {
  setPipelineMode("realtime");
  setSessionResumed(true);
  // Show "Resumed previous session" toast
});

window.listen("realtime_session_ended", (event) => {
  const reason = event.payload; // "user" | "idle_timeout" | "error"
  setIsEngaged(false);
  setPipelineMode("modular");
  flushAndClearTranscripts();
});

window.listen("realtime_idle_warning", (event) => {
  // event.payload.seconds_remaining for countdown display
});

window.listen("realtime_interrupted", () => {
  // Flash UI — barge-in confirmed by server
  setInteractionState("Interrupted");
});

// Pause/Resume events
window.listen("pipeline_paused", () => {
  setIsPaused(true);
  archiveCurrentTurn();
});

window.listen("pipeline_resumed", () => {
  setIsPaused(false);
});
```

#### Tray Window Events

```typescript
// Speech lifecycle
window.listen("state_changed", ({ payload }) => {
  // payload is InteractionState: "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted"
  setInteractionState(payload);
});

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

window.listen("ptt_status", ({ payload }) => {
  // payload.state: "IDLE" | "RECORDING" | "PROCESSING"
  setPttStatus(payload.state);
});

window.listen("realtime_session_started", () => {
  setPipelineMode("realtime");
});

window.listen("realtime_session_ended", (event) => {
  const reason = event.payload; // "user" | "idle_timeout" | "error"
  flushAndClearTranscripts();
});

window.listen("realtime_interrupted", () => {
  setInteractionState("Interrupted");
});

window.listen("realtime_idle_warning", (event) => {
  // event.payload.seconds_remaining for countdown display
});

window.listen("pipeline_paused", () => {
  setIsPaused(true);
});

window.listen("pipeline_resumed", () => {
  setIsPaused(false);
});

window.listen("pipeline_error", ({ payload }) => {
  setError(payload);
});
```

### Command Invocations

```typescript
// Settings management
await invoke("get_settings");
await invoke("update_setting", { domain, key, value });
await invoke("request_boot_state");
await invoke("request_model_catalog");
await invoke("reset_settings");
await invoke("update_theme", { theme: "dark" });
await invoke("check_llm_provider_health", { provider });
await invoke("check_stt_provider_health");
await invoke("check_tts_provider_health");
await invoke("list_remote_llm_models", { endpoint, apiKey, providerName });
await invoke("setup_remote_server", { providerKind, endpoint, apiKey });

// Engine control
await invoke("engage");
await invoke("check_engine_status");
await invoke("stop_engine");       // Offload/unload models (Skull button)
await invoke("launch_engine");     // Reload models (RefreshCw button)
await invoke("test_clip");         // Play test clip
await invoke("test_clip_cancel");  // Stop test clip

// Realtime session control (v0.9.0)
await invoke("start_realtime_session");       // Start WebSocket session
await invoke("stop_realtime_session");         // Stop and clean up
await invoke("pause_pipeline");               // Soft pause (WS alive, audio halted)
await invoke("resume_pipeline");              // Resume audio routing
await invoke("get_realtime_session_cache");    // Check for cached resume token

// PTT control
await invoke("ptt_start", { owner: "MainWindow" | "Tray" });
await invoke("ptt_stop", { owner: "MainWindow" | "Tray" });
await invoke("ptt_cancel");

// History management
await invoke<string[]>("get_transcript_history");
const sessions = await invoke<Session[]>("get_sessions");
const turns = await invoke<Turn[]>("get_turns", { sessionId });
await invoke("commit_session_to_history", { turns, ... });
await invoke("delete_session", { sessionId });

// Voice library
await invoke<ListVoicesResponse>("list_voices");
await invoke("add_voice_from_file", { path, name });
await invoke("add_voice_from_recording", { name });
await invoke("start_backend_recording");
await invoke("stop_backend_recording");
await invoke("delete_voice", { voiceId });
await invoke("rename_voice", { voiceId, name });
await invoke("preview_voice", { voiceId });

// Audio device management
const devices = await invoke<AudioDevice[]>("list_input_devices");

// Setup/Wizard
await invoke("fetch_manifest");
await invoke("check_for_updates");
await invoke("check_for_model_updates");
await invoke("get_onboarding_status");
await invoke("get_runtime_report");
await invoke("start_model_setup", { modelIds });
await invoke("cancel_model_setup");
await invoke("complete_setup_wizard");
await invoke("reveal_wizard");
await invoke("check_model_exists", { modelId });
await invoke("download_optional_model", { modelId });
await invoke("delete_model", { modelId });

// Monitoring
await invoke<RuntimeSnapshot>("get_runtime_snapshot");
await invoke<RuntimeSnapshot[]>("get_runtime_history", { limit });
await invoke("clear_runtime_history");
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

The **authoritative design system spec lives in [`design.md`](./design.md)** — tokens
(colors, type ramp, radii), type roles, uppercase policy, elevation levels, and motion
rules. Frontend code must only use tokens and sizes declared there; the impeccable
design-system detector enforces this against `docs/design.md`'s frontmatter.

Implementation of the tokens in CSS/Tailwind:

```css
:root {
  --background: 5, 5, 5;        /* rgb(var(--background)) */
  --foreground: 229, 226, 225;
  --accent: 0, 219, 233;         /* voice signal cyan */
  --accent-foreground: 5, 5, 5;
  --border: 255, 255, 255;
}
```

Glass surfaces use the `.glass-*` elevation classes in `index.css` (blur + tint per
`design.md` §Elevation). For the full token reference and all UI rules, read
`docs/design.md`.

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

---

## 15. Cognitive Memory Graph & Ingestion Telemetry (`Memory.tsx` + `MemoryGraph.tsx`)

Vox features a full-screen, ultra-scalable 3D Cognitive Memory Graph visualization built on a **Custom Three.js InstancedMesh WebGL Engine** capable of rendering 10,000+ nodes and directed edges at sub-60fps with <15MB RAM usage.

### Key Architecture & Invariants:
* **Custom Three.js WebGL Engine**: All nodes are rendered in **1 single `THREE.InstancedMesh`** draw call; all graph edges are packed into **1 single `THREE.LineSegments` BufferGeometry** draw call.
* **Scene Stability**: WebGL scene teardown on state/prop updates is strictly BANNED. Updates to colors, scales, and positions are imperatively written to GPU buffer attributes via stable `useRef` handles (`updateWebGLBuffersRef`).
* **Interaction**:
  * **Screen-space 24px proximity picking**: Projects 3D node coordinates to 2D screen space to guarantee 100% reliable node selection even when zoomed out.
  * **Smart Zoom-Preserving Fly-To**: Smooth camera interpolation centers the target node while preserving the user's current zoom depth if already zoomed in (`targetZ = Math.min(currentZ, 1200)`). Re-clicking an active node toggles its tooltip off.
* **Search & Visual Filtering**: Searching over facts highlights matching nodes with glowing halo rings while non-matching nodes and connecting edges are ghosted out (`#1e293b`). Search suggestions display collection swatch icons and fact snippets constrained to `max-h-[260px]`.
* **Borderless UI Overlays**:
  * **Orbital Network Loader**: Borderless dual-orbital network core with a pulsing central `Sparkles` emblem and clean typography.
  * **Frameless Alternating Zig-Zag Telemetry Drawer (`MemoryPipelineDrawer.tsx`)**: Borderless telemetry dashboard using 100% full available height with a minimal metric strip (Active Nodes, Throughput, In Queue), alternating Left/Right zig-zag conduit pipeline flow, live events log, and consolidation sweep trigger button.

---

## 16. UX Implementation Guidelines

Mechanic-level UX patterns (moved from the old `design.md`). These describe *how* the UI
behaves; the *rules* it must obey live in [`design.md`](./design.md).

### 16.1 Responsive & Dynamic Layouts

The desktop layout transitions dynamically to a unified layout on small screens
(mobile viewports).

* **Central Navigation Capsule (`EdgeNav`)**: On desktop, navigation is a floating bottom
  capsule. On mobile, the system monitoring metrics (bottom-left on desktop) are hidden and
  the **Activity Monitor** is integrated as a 4th `NavLink` tab inside the capsule, routing
  directly to `/monitoring`.
* **Unified Responsive Diagnostics Monitor (`Monitoring.tsx`)**: On mobile it renders as a
  dedicated page route (`/monitoring`) with a solid glass background; on desktop it renders
  as an anchored floating popover modal (`popover={true}`) bottom-left without duplicating
  component hierarchy.
* **Viewport Transition Engine**:
  * Mobile ➔ Desktop: on `/monitoring`, resizing to desktop redirects to `/` and launches
    the popover panel.
  * Desktop ➔ Mobile: an open popover closes and routes to `/monitoring` so context is kept.
* **Sentient Core Scale**: On mobile the central Orb scales up **50%**
  (`min(92vw, 85vh)` instead of `min(70vw, 65vh)`) to act as the dominant touch target.

### 16.2 Performance Constraints & Best Practices

To hit high rendering performance on baseline systems (8GB RAM, CPU-first):

* **Dynamic FPS (`useDynamicFPS`)**: Heavy visual loops (Three.js WebGL Orb, HTML5 Canvas
  Waveform) throttle frame rate: *Active* 60fps, *Idle* 15fps, *Sleep* 0fps.
* **React Memoization**: Visually intensive components (`AmbientBackground`, `PipelineField`,
  `VoxOrb`, `LiveWaveform`) are wrapped in `React.memo` to avoid re-renders during text
  streaming or DB reads.

### 16.3 Settings & Configuration Hub UX

* **Flat Underline Tab Strips**: For list selections (LLM providers, gateway options) avoid
  heavy card grids. Use a flat left-aligned tab strip with a shared underline track
  (`border-b border-[rgba(var(--border),0.12)]`), active tab indicated by `text` color + a
  thicker `border-b-2 border-[rgb(var(--accent))]`, pipe separators (`|`) in
  `text-[rgb(var(--accent))]/30 font-light`. Inline provider/system icons render on desktop
  and hide on mobile.
* **Consolidated Card Headers on Mobile**: Hide internal settings card titles ("Appearance",
  "Model Hub", "Interaction Console") on small layouts; rely on the outer Category Page
  Headers, which are larger and high-contrast (`text-[15px] font-black uppercase
  tracking-[0.18em] text-[rgb(var(--foreground))]`).
* **Hover-Only Slide-Out Action Sidebars**: Toggle buttons wrap in a group row with a hidden
  sidebar panel (`w-0 opacity-0` → `group-hover:w-[38px] group-hover:opacity-100`) while the
  main button scales to fit (`flex-1`) and flattens shared borders.
* **Alignment & Padding Discipline**: Respect parent padding (a `p-3` desk needs no duplicate
  `px-3`/`mx-3` on children); align labels, tabs, inputs, and cards on one vertical axis.

### 16.4 Settings Hub & Synchronous Loading UX

* **Boot-Time Import Prewarming**: All lazy-loaded domain cards (`PersonaCard`, `ModelsCard`,
  `RealtimeCard`, `HistoryCard`, `MemoryCard`, `AppearanceCard`, `InteractionCard`) are
  eagerly prewarmed in parallel at **App boot** (`App.tsx`), so the radial hub opens with no
  lazy-load latency. Cards mount instantly and only play a brief skeleton cross-fade.
* **Premium Charging Skeleton**: `GlassSkeleton variant="card"` is the Suspense fallback for
  every domain card — a glass card with an accent-tinted border, ambient corner glow, a skewed
  shimmer sweep (`skeleton-shimmer`), a pulsing accent orb (`animate-ping`), and breathing
  glow (`skeleton-glow`), matching the Liquid Space aesthetic.
* **Radial Node Tooltips**: Each `RadialNode` and the `HubCenter` are wrapped in the custom
  `Tooltip` primitive (via `wrapperClassName`/`wrapperStyle` so the absolute node placement is
  preserved) with action copy from `SETTINGS_COPY` (`Open {label} settings` /
  `Close {label} settings` / `Open all settings` / `Clear all settings`).
* **Gated Connection Lines + Decorative Power Pulse**: SVG node-to-card connector lines render
  strictly when `activeDomains.includes(domain.id)`. On activation a thin accent `path` runs a
  one-shot `connector-flow` stroke-dashoffset animation (0.9s, staggered 0.12s per domain) —
  a decorative "node sent power to the card" pulse, not load-synced.

### 16.5 Memory Graph & Telemetry Drawer Invariants

See §15 for the WebGL engine invariants. Additional UI mechanics:

* **Decoupled Renderer Setup**: window/panel resizing triggers only a lightweight
  `renderer.setSize` update — GPU buffers and controls are never disposed.
* **Failsafe Stabilization**: layout stabilization enforces a **700ms max failsafe timeout**
  so the borderless network loader (`isLayoutStable`) always dismisses cleanly.
* **Clean Pill Selection**: collection/relation filters in `MemoryLegendCard.tsx` use rounded
  active ring highlights (`ring-1 ring-[rgb(var(--accent))]/30 bg-[rgb(var(--accent))]/15`)
  instead of vertical `border-l-2` accent borders.
* **100% Height Telemetry Drawer**: `MemoryPipelineDrawer.tsx` uses full available height with
  an alternating Left/Right zig-zag conduit (`01 Deduplicate` → `02 Embed` → `03 Evaluate
  Relations` → `04 Commit & Sync`) down to the `Memory Graph` destination. `Escape` closes it.

### 16.6 Keyboard Navigation & Route Sequence

* **Arrow Key Navigation**: `ArrowRight`/`ArrowLeft` cycles views in exact visual order
  matching the `EdgeNav` pills: `Home` (`/`) ➔ `History` (`/history`) ➔ `Memory` (`/memory`)
  ➔ `System` (`/settings`).

### 16.7 Small Layout Navigation & Scroll Backdrop Mask

For mobile and small viewports (`< 1024px`):

* **Floating EdgeNav Capsule**: navigation floats centered near the bottom with compact pill
  styling.
* **Soft Glass Fade Mask Backdrop**: a `110px` gradient overlay
  (`fixed bottom-0 left-0 right-0 pointer-events-none z-40 bg-gradient-to-b from-transparent
  via-[rgb(var(--background))]/60 to-[rgb(var(--background))]/95 backdrop-blur-[16px]`)
  sits behind the EdgeNav pill.
* **Seamless Content Fade**: content scrolls under the mask and fades/blurs out approaching
  the bottom edge.
* **Scroll Padding Baseline**: scrollable views (History, Settings, Monitoring) enforce
  `pb-[110px]` on small layouts.
* **Mobile Category Headers**: small layouts display explicit category headers
  (e.g. `HISTORY & SESSIONS`) at the top of scrollable lists.

---

**Last Updated:** 2026-08-16

