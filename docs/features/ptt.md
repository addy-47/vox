# 📄 `ptt.md` — Push-To-Talk (PTT) Feature

---

## 1. Purpose

Define a **Push-To-Talk (PTT) interaction mode** for Vox that provides **explicit user control** over recording, with **live waveform feedback** and **incremental background STT processing**.

---

## 2. Core Principle

> **PTT = Capture-first UX, transcript-second UX**

* **During recording**: Waveform only (no live text)
* **After stop**: Full transcript appears instantly
* **Background**: System pre-processes audio during capture

---

## 3. Mode Separation (CRITICAL)

### Interaction Modes

```rust
pub enum InteractionMode {
    Passive,  // VAD-triggered, always listening
    PTT,      // Button-controlled, explicit recording
}
```

### Mode Configuration

```rust
pub struct InteractionSettings {
    pub main_app_mode: InteractionMode,  // Main window behavior
    pub auto_sleep_timeout: u32,         // Dormancy timeout in seconds
    pub pipeline_mode: PipelineMode,     // Modular or Realtime
}

pub struct DictationSettings {
    pub enabled: bool,
    pub interaction_mode: DictationInteractionMode, // Passive | Ptt
    pub hotkey: String,                            // "Alt+Space"
    pub output_mode: DictationOutputMode,          // Paste | Clipboard | Tray
}
```

### Behavioral Differences

| Aspect | Passive Mode | PTT Mode |
|--------|-------------|----------|
| Trigger | VAD speech detection | User button press |
| Feedback | Live streaming transcript | Live waveform only |
| Control | Automatic start/stop | Explicit user control |
| Continuity | Session merging | Discrete recordings |
| VAD State | Active listening | Preserved but bypassed |

---

## 4. PTT State Machine

### States

```typescript
enum PttState {
  IDLE = "IDLE",          // Ready to record
  RECORDING = "RECORDING", // Actively capturing audio
  PROCESSING = "PROCESSING" // Post-processing audio
}
```

### Transitions

```
// Start recording (owner parameter determines target domain)
IDLE → RECORDING (user presses PTT button/hotkey, owner = MainWindow | Dictation)

// Stop recording (checks speech_detected atomic)
RECORDING → PROCESSING (user releases, speech detected)
RECORDING → IDLE       (user releases, NO speech detected — discard hold)

// Processing complete
PROCESSING → IDLE (transcript ready, state reset)
```

### Error Transitions

```
// Cancel during recording
RECORDING → IDLE (discard buffer, no transcript, cancel_flag set)

// Mode guard rejection
IDLE → IDLE (attempted PTT start in Passive mode — error returned)
```

### Realtime PTT Mode Differentiation

| Concern | Modular PTT | Realtime PTT |
|---------|------------|-------------|
| Audio routing | PTT buffer → STT → LLM → TTS | PTT buffer → WebSocket via `activity_start`/`activity_end` |
| Server-side VAD | N/A | Disabled (`disabled: true` in setup) |
| Client-side VAD | Earshot/Ten classifies speech → discard silent holds | **Required** — gates PCM before sending, prevents hallucination |
| PTT duration cap | 10 min hard limit (`MAX_PTT_SAMPLES`) | 30s long-hold cutoff, auto-sends `ActivityEnd` |
| Speech discard | No speech → clear buffer, skip `SttCommand::Final` | No speech → no `ActivityEnd` sent, server stays silent |

---

## 5. User Experience Flow

### Recording Phase

```typescript
User clicks PTT button
  ↓
Waveform appears + animates
  ↓
Recording starts immediately
  ↓
NO transcript shown (waveform only)
  ↓
User speaks freely
```

### Stop Phase

```typescript
User clicks PTT button again
  ↓
Waveform stops animating
  ↓
Short processing indicator (200-400ms)
  ↓
Full transcript appears instantly
  ↓
Transcript saved to history
```

### Display Phase

```typescript
Transcript visible + selectable
  ↓
Stored in ephemeral history (last 10)
  ↓
Read-only interface
  ↓
Ready for next interaction
```

---

## 6. Technical Implementation

### PTT Command Handlers

#### Start Recording (`ptt_start`)

```rust
#[tauri::command]
pub async fn ptt_start(
    app: AppHandle,
    owner: Option<InteractionOwner>,
) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    // 1. Resolve and synchronize active owner
    let actual_owner = if let Some(o) = owner {
        state.owner.store(o as u32, Ordering::Relaxed);
        // Update VAD actor with new owner for mode-aware routing
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine.vad_tx.send(VadCommand::UpdateOwner(o));
        }
        o
    } else {
        state.owner.load(Ordering::Relaxed).into()
    };

    // 2. Mode guard: reject if target is in Passive mode
    let interaction_mode = match actual_owner { /* check settings */ };
    if interaction_mode != InteractionMode::PTT {
        return Err("Cannot start PTT in Passive mode".to_string());
    }

    // 3. Atomic compare-exchange to prevent double-start
    if state.ptt.is_recording.compare_exchange(false, true, ...).is_err() {
        return Ok(());
    }

    // 4. Reset state and sync turn ID
    state.ptt.speech_detected.store(false, Ordering::Relaxed);
    state.ptt.audio_buffer.lock().unwrap().clear();
    let turn = state.pipeline.turn_id.load(Ordering::Relaxed);
    state.ptt.turn_id.store(turn, Ordering::Relaxed);

    // 5. In realtime mode: signal activity_start to WebSocket
    if is_realtime {
        rt_engine.activity_start()?;
    } else {
        // Send SpeechStart for barge-in
        engine.pipeline_tx.send(VoxEvent::SpeechStart { turn_id: turn, owner });
    }

    // 6. Notify UI and update interaction state
    let _ = app.emit_to(target, "ptt_status", json!({ "state": "RECORDING" }));
    state.pipeline.update_interaction_state(InteractionState::UserSpeaking, owner, &app);

    Ok(())
}
```

#### Stop Recording (`ptt_stop`)

```rust
#[tauri::command]
pub async fn ptt_stop(
    app: AppHandle,
    owner: Option<InteractionOwner>,
) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    // 1. Quick return if not recording
    if !state.ptt.is_recording.load(Ordering::SeqCst) {
        return Ok(());
    }

    // 2. Resolve owner
    let actual_owner = /* resolve from param or global state */;

    // 3. VAD gate: check speech_detected
    if !state.ptt.speech_detected.load(Ordering::Relaxed) {
        // Silence-only hold — discard, no finalization
        discard_ptt_hold_inner(&state.ptt);
        app.emit_to(target, "ptt_status", json!({ "state": "IDLE" }));
        state.pipeline.update_interaction_state(InteractionState::Idle, owner, &app);
        return Ok(());
    }

    // 4. Extract buffer and determine pipeline mode
    let (turn, buffer_clone, is_realtime) = {
        let buffer = state.ptt.audio_buffer.lock().unwrap();
        let turn = state.ptt.turn_id.load(Ordering::Relaxed);
        state.ptt.is_recording.store(false, Ordering::SeqCst);
        let is_realtime = settings.interaction.pipeline_mode == PipelineMode::Realtime;
        (turn, buffer.clone(), is_realtime)
    };

    // 5. Route based on pipeline mode
    if is_realtime {
        rt_engine.activity_end()?;  // Signal end of voice activity
    } else {
        engine.stt_tx.send(SttCommand::Final(turn, owner, buffer_clone));
    }

    // 6. Update UI
    app.emit_to(target, "ptt_status", json!({ "state": "PROCESSING" }));
    state.pipeline.update_interaction_state(InteractionState::Thinking, owner, &app);

    Ok(())
}
```

#### Cancel Recording (`ptt_cancel`)

```rust
#[tauri::command]
pub async fn ptt_cancel(
    app: AppHandle,
    owner: Option<InteractionOwner>,
) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    let actual_owner = /* resolve from param or global state */;

    // Set cancel flag, clear buffer, mark not recording
    state.ptt.is_recording.store(false, Ordering::SeqCst);
    state.ptt.audio_buffer.lock().unwrap().clear();
    state.ptt.speech_detected.store(false, Ordering::Relaxed);

    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    let target = match actual_owner { /* Dictation → "tray", _ → "main" */ };
    let _ = app.emit_to(target, "ptt_status", json!({ "state": "IDLE" }));

    state.pipeline.update_interaction_state(InteractionState::Idle, actual_owner, &app);

    Ok(())
}
```

---

## 7. Audio Processing Logic

### Buffer Management

#### PTT Audio Buffer

```rust
pub struct PttState {
    pub is_recording: AtomicBool,
    pub turn_id: Arc<AtomicU32>,
    pub audio_buffer: Mutex<Vec<f32>>,              // Raw 16kHz mono samples
    pub speech_detected: AtomicBool,                 // VAD gate — set if speech classified during hold
}
```

#### VAD-Gated Continuous Capture

```rust
pub fn handle_ptt_audio_sync(app: &AppHandle, samples: &[f32]) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    // Check recording state (atomic, no lock)
    if !state.ptt.is_recording.load(Ordering::Relaxed) { return; }

    let mut buffer = state.ptt.audio_buffer.blocking_lock();

    // Append ALL audio (no VAD filtering for capture itself)
    if buffer.len() < MAX_PTT_SAMPLES {
        buffer.extend_from_slice(samples);
    } else {
        // Safety cap: Auto-stop if >10 minutes (modular) or 30s (realtime)
        log::warn!("[PTT] Hard limit reached. Auto-stopping.");
        drop(buffer);
        tauri::async_runtime::spawn(async move {
            let _ = ptt_stop(app.clone(), None).await;
        });
        return;
    }
}
```

#### VAD Gating (`speech_detected`)

A `speech_detected: AtomicBool` on `PttState` tracks whether speech was classified during a PTT hold:

- On **speech onset**: flips `speech_detected = true`, flushes a 240ms pre-roll buffer to the realtime WebSocket (prevents clipped first word)
- On **ptt_stop**: if `speech_detected == false`, the entire hold is discarded — no `SttCommand::Final` sent to STT or `ActivityEnd` to Gemini
- On **ptt_start**: `speech_detected` is reset to `false`

### Background STT Processing

#### Partial Transcripts

```rust
// Every 800ms during recording
if *samples_since >= 12800 {
    let turn = state.ptt.turn_id.load(Ordering::Relaxed);
    let owner: InteractionOwner = state.owner.load(Ordering::Relaxed).into();

    // Send last 15 seconds for partial transcription
    let start_idx = buffer.len().saturating_sub(240000); // 15s * 16kHz
    let _ = engine.stt_tx.send(SttCommand::Partial(
        turn,
        owner,
        buffer[start_idx..].to_vec()
    ));

    *samples_since = 0;
}
```

#### Final Transcript

```rust
// On ptt_stop: Send complete buffer
let _ = engine.stt_tx.send(SttCommand::Final(
    turn,
    owner,
    buffer_clone // Full recording
));
```

### Waveform Feedback

#### RMS Energy Calculation

```rust
// Every 60ms during recording
if *samples_waveform >= 960 {
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum_sq / samples.len() as f32).sqrt();

    let noise_gate = settings.vad.ptt_noise_gate;
    let gated_energy = if rms > noise_gate { (rms * 8.0).min(1.0) } else { 0.0 };

    // Send to telemetry for waveform rendering
    let _ = engine.telemetry_tx.send(TelemetryEvent::AudioEnergy {
        energy: gated_energy,
        vad_prob: 0.0
    });

    *samples_waveform = 0;
}
```

---

## 8. UI Integration

### Tray Interface

#### Header Controls

```typescript
const Header: React.FC = () => {
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');

  const togglePtt = async () => {
    try {
      if (pttStatus === 'IDLE') {
        await invoke("ptt_start", { owner: "Dictation" });
      } else {
        await invoke("ptt_stop", { owner: "Dictation" });
      }
    } catch (error) {
      console.error("[PTT] Toggle failed:", error);
    }
  };

  return (
    <div className="flex items-center justify-between">
      <StatusDot status={pttStatus === 'RECORDING' ? 'recording' : 'idle'} />
      <PillButton
        onClick={togglePtt}
        variant={pttStatus === 'IDLE' ? 'primary' : 'destructive'}
      >
        {pttStatus === 'IDLE' ? '🎙️ Record' : '⏹️ Stop'}
      </PillButton>
    </div>
  );
};
```

#### Waveform Display

```typescript
const LiveWaveform: React.FC<{ energy: number }> = ({ energy }) => {
  return (
    <ElevenLabsWaveform
      energy={energy}
      isRecording={pttStatus === 'RECORDING'}
      className="w-full h-16"
    />
  );
};
```

#### Processing Indicator

```typescript
const ProcessingIndicator: React.FC = () => {
  const [dots, setDots] = useState('');

  useEffect(() => {
    const interval = setInterval(() => {
      setDots(prev => prev.length >= 3 ? '' : prev + '.');
    }, 200);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="flex items-center gap-2 text-sm text-muted-foreground">
      <div className="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
      <span>Processing{dots}</span>
    </div>
  );
};
```

### Main Window Integration

#### PTT Button in Main UI

```typescript
const PttControls: React.FC = () => {
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');

  // Same toggle logic as tray
  const togglePtt = async () => { /* ... */ };

  return (
    <div className="fixed bottom-4 right-4">
      <PillButton
        onClick={togglePtt}
        variant={pttStatus === 'IDLE' ? 'primary' : 'destructive'}
        size="lg"
      >
        {pttStatus === 'IDLE' ? '🎙️ PTT' : '⏹️ Stop'}
      </PillButton>
    </div>
  );
};
```

---

## 9. Interaction Model

### Session Management

#### No Continuity Merging

Unlike passive mode, PTT recordings are **discrete**:

```typescript
// Each PTT press creates new interaction
const startNewInteraction = () => {
  currentIdRef.current += 1;
  setInteractionId(currentIdRef.current);
  setCommittedText("");
  setPartialText("");
};
```

#### Turn ID Synchronization

```rust
// PTT uses global turn counter
let current_global = state.pipeline.turn_id.load(Ordering::Relaxed);
state.ptt.turn_id.store(current_global, Ordering::Relaxed);
```

### History Integration

#### Ephemeral Storage

```typescript
// Add to in-memory history
const addToHistory = (transcript: string) => {
  setHistory(prev => [transcript, ...prev.slice(0, MAX_HISTORY - 1)]);
};

// Triggered by transcript_final event
useEffect(() => {
  const handleFinal = ({ payload }: { payload: { text: string } }) => {
    if (payload.text) {
      addToHistory(payload.text);
    }
  };

  const unlistener = window.listen("transcript_final", handleFinal);
  return () => unlistener();
}, []);
```

---

## 10. Performance Optimizations

### Memory Management

#### Buffer Limits

```rust
const MAX_PTT_SAMPLES: usize = 16000 * 60 * 10; // 10 minutes at 16kHz
```

#### Chunked Processing

* **Partial sends**: 15-second windows every 800ms
* **Rolling buffer**: Only last 15s kept in memory for partials
* **Final send**: Complete buffer (up to 10min limit)

### CPU Efficiency

#### Throttled Updates

* **Partial STT**: Every 800ms (not every audio chunk)
* **Waveform**: Every 60ms (960 samples)
* **UI updates**: Debounced state changes

#### Background Processing

* **STT**: Asynchronous, non-blocking
* **Waveform**: GPU-accelerated rendering
* **UI**: Minimal re-renders during recording

---

## 11. Error Handling

### Recording Failures

```rust
// Buffer overflow protection
if buffer.len() >= MAX_PTT_SAMPLES {
    log::warn!("[PTT] Buffer limit reached. Auto-stopping.");
    // Trigger ptt_stop automatically
}
```

### STT Processing Errors

```typescript
// UI fallback for failed transcriptions
const [error, setError] = useState<string | null>(null);

useEffect(() => {
  const handleError = ({ payload }: { payload: string }) => {
    setError("Transcription failed. Please try again.");
    setPttStatus('IDLE');
  };

  const unlistener = window.listen("pipeline_error", handleError);
  return () => unlistener();
}, []);
```

### IPC Timeouts

```typescript
// Command invocation with timeout
const togglePtt = async () => {
  try {
    await invoke(pttStatus === 'IDLE' ? "ptt_start" : "ptt_stop", {
      owner: "Dictation"
    });
  } catch (error) {
    console.error("[PTT] Command failed:", error);
    setPttStatus('IDLE'); // Reset to safe state
  }
};
```



---

## 15. Success Metrics

### User Experience

- **Recording reliability**: >99% successful captures
- **Processing speed**: <500ms perceived latency
- **User satisfaction**: Positive feedback on control

### Technical Performance

- **Memory efficiency**: No leaks during extended recording
- **CPU usage**: <10% during active processing
- **Audio quality**: Transparent capture (no artifacts)

### Feature Adoption

- **Usage rate**: >30% of interactions use PTT mode
- **Mode switching**: Seamless transitions between Passive/PTT
- **Error rate**: <1% failed recordings

---

## 16. Architectural Integration

### Relationship to Passive Mode

PTT mode **coexists** with passive mode:

- **Shared infrastructure**: Same STT engine, same buffers
- **Mode isolation**: Clean separation of VAD vs button control
- **State preservation**: VAD state maintained during PTT sessions

### Backend Dependencies

- **STT worker**: Reused for both partial and final transcription
- **Audio ingestion**: Same CPAL stream, different processing logic (gated by `speech_detected` atomic)
- **Pipeline events**: Integrated with main event bus
- **Realtime mode**: PTT triggers `activity_start`/`activity_end` over WebSocket instead of `SttCommand::Final`

### UI Coordination

- **Mode indicators**: Clear visual distinction between modes
- **Seamless switching**: No restart required when changing modes
- **State synchronization**: Consistent state across main/tray windows
- **PTT button**: Only rendered when `interactionMode === "PTT"`; in realtime mode, toggles WebSocket activity