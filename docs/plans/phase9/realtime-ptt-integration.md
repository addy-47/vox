# Implementation Plan: Realtime S2S — Home Page Full Functionality

**Objective:** Wire the full realtime voice session lifecycle into `Home.tsx` — engage/end, pause/resume (soft interrupt), PTT mic, client-side VAD gate, transcript flush, idle timeout, session cache persistence — with each mode (Passive vs PTT) fully differentiated across the entire stack.

---

## Architecture Constraints (Applied)

- Backend is source of truth — frontend derives display-only state from IPC events
- No new Zustand store — session/voice state lives in `useRef` + `useState` within `Home.tsx`
- `is_paused` is a new `AtomicBool` in `PipelineAtomics` — soft stop (WS alive, audio halted)
- Session resume token lives in `~/.vox/cache/realtime_session.json` — written by backend, never frontend
- `engageLockRef` extends to cover entire WS handshake period to prevent double-engage
- Transcript history limit (currently 4 turns) is **removed** for realtime mode; transcripts append without cap, cleared only on session end or explicit flush
- Mic button exists **only in PTT mode** (Modular or Realtime); passive modes have no mic button
- Pause/Resume controls are **universal** (available in both Passive and PTT mode, Modular and Realtime)

---

## Execution & Testing Directives [ADDED]

* **Phase-by-Phase Process:** Proceed strictly phase-by-phase. Stop after each phase to run validators (`cargo check`, `pnpm build`, `cargo run --bin vox-bench`, and `cargo run --bin vox_realtime_bench`) and perform a `/review` of the changes.
* **Brittle Code Testing:** Implement unit tests for complex/brittle functions (e.g., resampler contiguous slicing, VAD pre-roll flush logic) and simple integration tests in `app/src-tauri/tests/` for critical pipeline flows. Avoid over-testing non-brittle wrappers.
* **Hand-in-Hand Interleaved Logic:** Do not implement all backend logic first followed by frontend. Work in vertical slices: backend changes for a feature must be paired with its frontend updates immediately.
* **Logical Execution Phases:**
  1. **Phase 1: Foundation & Code Health:** Reorganize `services/audio/` directory (A.1), optimize resampler heap allocations (F.1), migrate `AudioBridge`/`PlaybackBridge` to bounded channels (F.2), and clean up React timeout leaks in `Home.tsx` (F.3).
  2. **Phase 2: Modular Passive Mode:** Implement backend `is_paused` and `cancel_flag` coupling (A.4, A.5, A.6) + Frontend UI universal buttons (C.2, C.5).
  3. **Phase 3: Realtime Passive Mode:** Implement 10-minute active idle timeout (A.9) + Paused timeout & Lazy reconnection (A.11) + Session Cache (A.8, E) + Frontend UI (C.1, C.3, C.4, C.7, D.2).
  4. **Phase 4: Modular PTT Mode:** Implement client-side VAD classification during PTT (B.2.1, B.2.2) + Discard final STT buffers if no speech detected (B.2.3).
  5. **Phase 5: Realtime PTT Mode:** Implement VAD gating + Onset pre-roll flush (B.2.2) + 30s long-hold cutoff (A.10).

---

## Mode Differentiation Matrix [MODIFIED]

| Concern | Modular / Passive | Modular / PTT | Realtime / Passive | Realtime / PTT |
|---|---|---|---|---|
| Audio routing | VAD → STT → LLM → TTS | PTT buffer → STT → LLM → TTS | VAD bypassed; raw PCM → WS | VAD bypassed; PTT buffer → WS via `activity_start/end` |
| Server-side VAD | N/A | N/A | Gemini cloud VAD ON | Gemini cloud VAD OFF (disabled in setup) |
| Client-side VAD | Silero classifies speech | PTT gate | **Not needed** (cloud handles it) | **Silero VAD REQUIRED** — gates PCM before sending, prevents hallucination |
| Mic button shown | No | Yes | **No** | **Yes** |
| Pause behaviour | Cancel flag + lock gate | Cancel flag + lock gate | `ActivityEnd` + Cancel flag + lock audio bridge | `ActivityEnd` + Cancel flag + lock audio bridge |
| Idle timeout | Auto-sleep (500s) after no speech | N/A | **45s** no server activity → auto-disconnect | **Session ends** after PTT hold > 30s with no speech past VAD |
| Barge-in mechanism | Voice / Manual Pause | PTT Mic button press (pulses cancel) | Voice / Manual Pause | PTT Mic button press (pulses cancel) |

---

## Phase A — Backend: Services/Audio Restructuring, Router, and Dynamic Gating [MODIFIED]

### A.1 — Create `services/audio/` module structure
We will reorganize all audio-related services into a unified module to improve cohesion and support decoupling:
1. Rename and move `services/audio.rs` to `services/audio/device.rs`.
2. Move `services/playback.rs` to `services/audio/playback.rs`.
3. Create `services/audio/router.rs` for the audio consumer/router thread.
4. Create `services/audio/mod.rs` to export the public interfaces:
   ```rust
   pub mod device;
   pub mod playback;
   pub mod router;

   pub use device::AudioStream;
   pub use playback::PlaybackEngine;
   pub use router::{AudioRouter, RouteMode};
   ```
5. Update `services/mod.rs` to export `pub mod audio;` and remove `pub mod playback;`. Update all file imports across the backend (e.g. in `lib.rs`, `pipeline.rs`, `ptt.rs`).

---

### A.2 — Implement `services/audio/router.rs`
The `AudioRouter` thread consumes raw chunks of 256 samples from the CPAL hardware ring buffer and routes them based on the active mode:

```rust
pub enum RouteMode {
    LocalVad,       // Cases 1, 2, 3: write samples to the local VAD actor's ring buffer
    DirectRealtime, // Case 4: convert f32 to i16 and send directly to realtime_tx channel
}
```

* **Attributes:**
  * Runs with `ThreadPriority::Max` to prevent hardware buffer underflow / audio dropouts.
  * Checks `is_paused: Arc<AtomicBool>`. If `true`, it discards the audio chunks instead of forwarding.
  * Receives `realtime_tx: Option<tokio::sync::mpsc::UnboundedSender<Vec<i16>>>` via a command channel or atomic reference.

---

### A.3 — `ipc/pipeline.rs` — Dynamic Engine Gating
Modify `launch_engine` to read settings and conditionally spawn only the threads/engines needed:

* **Case 1 & 2 (Modular Passive / PTT):**
  * `need_stt = true`, `need_vad = true`
  * Spawns `spawn_stt_worker` and `spawn_vad_actor`.
  * Set `RouteMode::LocalVad` on the `AudioRouter`.
* **Case 3 (Realtime PTT):**
  * `need_stt = false`, `need_vad = true`
  * Spawns `spawn_vad_actor` (for client VAD gate). **STT worker is not spawned (models remain offloaded).**
  * Set `RouteMode::LocalVad` on the `AudioRouter`.
* **Case 4 (Realtime Passive):**
  * `need_stt = false`, `need_vad = false`
  * Neither STT nor VAD threads are spawned.
  * Set `RouteMode::DirectRealtime` on the `AudioRouter`.

---

### A.4 — `core/state.rs` — Add `is_paused` and `speech_detected`

```rust
// In PipelineAtomics:
pub is_paused: Arc<AtomicBool>, // Initialised as false in PipelineAtomics::new()
```

```rust
// In PttState:
pub speech_detected: std::sync::atomic::AtomicBool, // Initialised as false, reset on ptt_start
```

---

### A.5 — `ipc/pipeline.rs` — `pause_pipeline` command

```
1. Check is_paused — if already true, return Ok(()) (idempotent)
2. store is_paused = true (SeqCst) — lock the audio router gate immediately
3. store cancel_flag = true (SeqCst) — abort current active generation and playback instantly
4. playback_engine.cancel() — immediately stop audio drain
5. If pipeline_mode == Realtime:
   a. session.activity_end() — tell Gemini to stop processing current turn
6. If pipeline_mode == Modular:
   a. send VoxEvent::Cancelled
   b. stt_tx.send(SttCommand::ResetStream)
7. Emit "pipeline_paused" to owning window
8. Update interaction state -> Idle (not "Paused" — Idle IS the visual pause state)
```

---

### A.6 — `ipc/pipeline.rs` — `resume_pipeline` command

```
1. Check is_paused — if false, return Ok(()) (idempotent)
2. store is_paused = false (SeqCst) — re-opens the audio router gate
3. If pipeline_mode == Modular:
   a. cancel_flag = false
   b. send VoxEvent::WarmUp to pipeline
4. Emit "pipeline_resumed" to owning window
5. Update interaction state -> Listening (or Idle if in PTT waiting for press)
```

---

In `vad/actor.rs`, add a local `let mut audio_paused: bool = false;` flag. When `StartRealtime` is active:
- `PauseAudio` → set `audio_paused = true`; the `realtime_tx.send()` call is skipped
- `ResumeAudio` → set `audio_paused = false`; forwarding resumes
- In PTT mode: `handle_ptt_audio_sync` also checks `audio_paused` flag via a new `AtomicBool` on `PttState`

### A.7 — `ipc/pipeline.rs` — `barge_in` command (Realtime/Passive only)

```
1. playback_engine.cancel() — local audio flush, <10ms
2. session.cancel() — sends ControlEvent::Interrupt to WS
   (Interrupt = ActivityStart + optional ActivityEnd in is_ptt branch — already implemented)
3. Emit "realtime_interrupted" to "main" window
```

> [!NOTE]
> `barge_in` is NOT exposed in PTT mode. In PTT, the user presses the mic button again to interrupt, which naturally sends `activity_start`. The concept doesn't exist separately.

### A.8 — Session Cache Persistence (Backend)

The `resume_handle` currently lives only in-memory inside `GeminiLiveSession.state`. On disconnect it is lost. We need it to survive a process crash or user-initiated disconnect.

**Write path:**  
In `handle_gemini_server_message`, when `sessionResumptionUpdate.newHandle` arrives:
```rust
// After saving to in-memory SessionState:
let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
let payload = serde_json::json!({
    "provider": "gemini_live",
    "handle": new_handle,
    "expires_at": unix_epoch_ms() + (2 * 60 * 60 * 1000), // 2h TTL
    "model": config.model,
    "conversation_id": <pass in from session start>
});
// Write atomically: write to .tmp then rename
std::fs::write(cache_path.with_extension("tmp"), payload.to_string())?;
std::fs::rename(cache_path.with_extension("tmp"), &cache_path)?;
```

**Read path:**  
In `start_realtime_session` IPC command, before building `GeminiLiveProvider`:
```rust
let cache_path = paths::cache_dir().join("realtime_session.json");
if let Ok(data) = std::fs::read_to_string(&cache_path) {
    if let Ok(cached) = serde_json::from_str::<serde_json::Value>(&data) {
        let expires = cached["expires_at"].as_u64().unwrap_or(0);
        let now_ms = /* unix epoch ms */;
        let cached_model = cached["model"].as_str().unwrap_or("");
        if expires > now_ms && cached_model == settings.realtime.gemini.model {
            resume_handle = cached["handle"].as_str().map(|s| s.to_string());
        }
    }
}
// Pass resume_handle into GeminiRealtimeConfig before creating provider
```

**Clear path:** On `stop_realtime_session`, delete the cache file. On reconnection failure (max retries exhausted), also delete the cache file (stale token).

**Emit:** On session start with a valid resume token: emit `realtime_session_resumed` instead of `realtime_session_started`.

### A.9 — Active Idle Timeout (Realtime/Passive) [MODIFIED]

A tokio task spawned inside `start_realtime_session_internal` monitors the 10-minute timeout using the **Dynamic Deadline Sleep Pattern** to prevent C-State sleep issues and unnecessary context switches.

```rust
const TIMEOUT_MS: u64 = 10 * 60 * 1000;      // 10 minutes
const WARN_1_MS: u64 = TIMEOUT_MS - 15_000;  // 9m 45s (15s warning)
const WARN_2_MS: u64 = TIMEOUT_MS - 5_000;   // 9m 55s (5s warning)

loop {
    let last_activity = engine.last_activity_time();
    let now = unix_epoch_ms();
    let elapsed = now.saturating_sub(last_activity);

    if elapsed >= TIMEOUT_MS {
        // Stop session, delete cache, emit "realtime_session_ended" with reason "idle_timeout"
        break;
    } else if elapsed >= WARN_2_MS {
        // Emit 5s warning event and sleep until the hard timeout deadline
        emit_warning(600 - (elapsed / 1000));
        tokio::time::sleep(std::time::Duration::from_millis(TIMEOUT_MS - elapsed)).await;
    } else if elapsed >= WARN_1_MS {
        // Emit 15s warning event and sleep until the 5s warning deadline
        emit_warning(600 - (elapsed / 1000));
        tokio::time::sleep(std::time::Duration::from_millis(WARN_2_MS - elapsed)).await;
    } else {
        // Sleep precisely until the first warning (15s warning) is due
        let time_until_warning = WARN_1_MS - elapsed;
        tokio::time::sleep(std::time::Duration::from_millis(time_until_warning)).await;
    }
}
```

---

### A.10 — PTT Long-Hold Safety Cutoff

In `ptt.rs::handle_ptt_audio_sync`, the existing `MAX_PTT_SAMPLES` check (10 minutes) is too long for realtime. For realtime PTT, add:

```
const MAX_REALTIME_PTT_DURATION_MS: u64 = 30_000; // 30 seconds
```

If `now - ptt_start_ms > 30_000` and pipeline_mode == Realtime:
- Auto-call `ptt_stop` (sends `ActivityEnd` to server)
- Emit `ptt_status: "IDLE"` to frontend
- Log warning: user held PTT for >30s without speech

`ptt_start_ms` is a new `AtomicU64` on `PttState`, set to `now` on every `ptt_start`.

---

### A.11 — Paused Timeout and Lazy Reconnection (Gemini WS) [ADDED]

To prevent resource/battery drain and crash states on laptop sleep or Google's aggressive 10-minute idle WebSocket disconnect:

1. **Catch Disconnect during Pause:**
   * Inside the `GeminiLiveProvider` websocket receiver task: if the connection is dropped (by proxy/load balancer) and `is_paused` is `true`, do **not** attempt automatic reconnection and do **not** emit a pipeline error.
   * Store `ws_connected = false` (a new atomic boolean on the provider session).

2. **Lazy Reconnect on Resume:**
   * When `resume_pipeline` IPC command is called:
     * Check if `ws_connected` is `false`.
     * If `false`, call `start_realtime_session_internal(&state, &app)` to establish a fresh connection.
     * The connection routine will automatically load the resumption token from `~/.vox/cache/realtime_session.json` to seamlessly resume the S2S session.
     * Once connection is established, re-open the audio router.

---

## Phase B — Client-Side VAD Gating across Modes (Modular + Realtime) [MODIFIED]

Client-side VAD (Silero) is required across both Modular and Realtime pipelines to save compute and prevent silent/hallucinated turns.

### B.1 — The Four Interaction Cases & VAD Behavior

1. **Modular PTT:**
   * **Purpose:** Save CPU/synthesis compute by discarding silent key-presses.
   * **VAD Logic:** While PTT button is held, run the incoming audio chunks through the Silero VAD classifier inside the VAD actor. If speech probability exceeds the threshold, set `state.ptt.speech_detected` (a new atomic boolean) to `true`.
   * **Discard Path:** When `ptt_stop` is called, check `speech_detected`. If `false` (no speech occurred), clear the PTT audio buffer, bypass sending `SttCommand::Final` to the STT engine, emit `ptt_status: "IDLE"`, and transition directly back to the `Idle` interaction state on the UI.

2. **Modular Passive:**
   * **Purpose:** Define turn boundaries automatically using speech start/end detection.
   * **VAD Logic:** Run full VAD classification. Speech-start triggers pipeline pre-warming and audio capture. Speech-end flushes the accumulated buffer to the STT engine for transcription.

3. **Realtime PTT:**
   * **Purpose:** Prevent sending silent audio packets to the WebSocket and discard silent PTT holds.
   * **VAD Logic:** Run Silero VAD classification. Keep the WebSocket audio bridge gate closed (`ptt_vad_gate_open = false`) until the first speech chunk is detected. Upon detection, send the 240ms pre-roll context and begin streaming PCM packets to the WebSocket. Set `state.ptt.speech_detected = true`.
   * **Discard Path:** If the user stops recording and `speech_detected` is `false`, send nothing downstream to the server (no finalization signal), reset state, and return to `Idle`.

4. **Realtime Passive:**
   * **Purpose:** Offload VAD to the cloud provider.
   * **VAD Logic:** Bypassed completely. Forward raw audio chunks directly to the WebSocket connection. The cloud provider's server-side VAD manages turn boundaries and response triggers.

---

### B.2 — Backend Changes for VAD Gating

#### B.2.1 — `core/state.rs` — Add `speech_detected` to `PttState`
```rust
pub struct PttState {
    pub is_recording: std::sync::atomic::AtomicBool,
    pub turn_id: Arc<AtomicU32>,
    pub audio_buffer: std::sync::Mutex<Vec<f32>>,
    pub samples_since_partial: std::sync::atomic::AtomicUsize,
    pub samples_since_waveform: std::sync::atomic::AtomicUsize,
    pub speech_detected: std::sync::atomic::AtomicBool, // [ADDED]
}
```
*Initialized as `false` in `AppState::new()`.*

#### B.2.2 — `vad/actor.rs` — VAD classification in PTT mode [MODIFIED]
Modify the PTT bypass check to run prediction and implement the pre-roll flush:
```rust
if mode == InteractionMode::PTT {
    // Run VAD prediction even in PTT mode to classify speech
    let mut detected = vad.predict(&chunk);
    if raw_energy < effective_noise_gate {
        detected = false;
    }
    
    if detected {
        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
        let was_speech = state.ptt.speech_detected.swap(true, Ordering::Relaxed);
        if !was_speech {
            // SPEECH ONSET TRANSITION: Flush pre-roll buffer to avoid clipping the first word
            if let Some(ref tx) = realtime_tx {
                let pre_roll_i16: Vec<i16> = pre_roll_buffer
                    .iter()
                    .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
                    .collect();
                let _ = tx.send(pre_roll_i16);
                pre_roll_buffer.clear();
            }
        }
    }
    
    // Waveform telemetry and PTT buffering
    crate::services::ptt::handle_ptt_audio_sync(&app, &chunk);
    
    // In Realtime PTT: Gate audio bridge forwarding
    if let Some(ref tx) = realtime_tx {
        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
        if state.ptt.speech_detected.load(Ordering::Relaxed) {
            let i16_samples: Vec<i16> = chunk.iter().map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
            let _ = tx.send(i16_samples);
        }
    } else {
        // In Modular PTT: Accumulate pre-roll when not in speech
        if !state.ptt.speech_detected.load(Ordering::Relaxed) {
            pre_roll_buffer.extend_from_slice(&chunk);
            if pre_roll_buffer.len() > 8000 {
                let excess = pre_roll_buffer.len() - 8000;
                pre_roll_buffer.drain(0..excess);
            }
        }
    }
    continue;
}
```

#### B.2.3 — `ptt.rs` — `ptt_start` and `ptt_stop`
* In `ptt_start`: Reset `state.ptt.speech_detected.store(false, Ordering::SeqCst)`.
* In `ptt_stop`: Load `speech_detected`. If `false`, discard the session (clear buffer, emit `ptt_status: "IDLE"`, update interaction state directly to `Idle`, return `Ok(())`). Do not call `SttCommand::Final` or send `ActivityEnd` to Gemini.

---

---

## Phase C — `Home.tsx` Rewrite (UI + Signal Logic)

### C.1 — New State Variables

```typescript
// Replaces/extends existing state
const [pipelineMode, setPipelineMode] = useState<"modular" | "realtime">("modular");
const [isPaused, setIsPaused] = useState(false);
const [sessionResumed, setSessionResumed] = useState(false); // was reconnected
const [idleTimeout, setIdleTimeout] = useState<number | null>(null); // seconds until timeout
```

No new Zustand state. All of these are local to Home.tsx.

### C.2 — Button Layout Logic [MODIFIED]

Universal controls exist in all modes (both Modular and Realtime). Mode-specific controls are conditionally rendered depending only on `interactionMode`.

* **Universal Session Controls (Always visible when engaged):**
  * **Engage / Disengage Button:** `Power` icon when NOT engaged; `X` icon when engaged.
  * **Pause / Resume Button:** Play/Pause icon shown when engaged (toggles `isPaused` state via backend commands `pause_pipeline` and `resume_pipeline`).

* **Routing Controls (PTT Mode Only):**
  * **Mic Button:** Rendered strictly when `interactionMode === "PTT"` (regardless of whether the active pipeline is Modular or Realtime). In Realtime PTT mode, pressing the Mic button triggers the natural barge-in (instantly calling `ptt_start` which pulses `cancel_flag` and opens the audio gate). In Passive mode, the Mic button is completely hidden.

**Visual Rendering (Engaged):**
```
NOT Engaged:
  [Power icon] (Enables Engagement)

Engaged + PASSIVE mode (Modular or Realtime):
  [Pause/Resume icon]  [X icon (Disengage)]

Engaged + PTT mode (Modular or Realtime):
  [Pause/Resume icon]  [Mic icon]  [X icon (Disengage)]
```

### C.3 — `handleEngage` (Realtime path)

```typescript
const handleEngage = async () => {
  if (engageLockRef.current) return;
  engageLockRef.current = true;
  setIsLaunching(true);

  try {
    // Single invoke — backend branches on pipeline_mode internally
    const response = await invoke<{ pipeline_mode: string }>("engage");
    setPipelineMode(response.pipeline_mode as "modular" | "realtime");
    setIsEngaged(true);
    setIsPaused(false);
    clearTranscripts();
  } catch (err) {
    console.error("[Home] Engage failed:", err);
    // Show error toast — do NOT silently fail
  } finally {
    setIsLaunching(false);
    // Keep lock for 800ms after launch (prevents double-tap during WS handshake)
    setTimeout(() => { engageLockRef.current = false; }, 800);
  }
};
```

### C.4 — `handleEnd` (new — replaces disengage path on X click)

```typescript
const handleEnd = async () => {
  if (!isEngaged || engageLockRef.current) return;
  engageLockRef.current = true;

  try {
    // Single invoke — backend stops realtime + disengages
    await invoke("engage"); // toggle off
    setIsEngaged(false);
    setIsPaused(false);
    setPipelineMode("modular");
    flushAndClearTranscripts(); // archives current turn, clears history
  } catch (err) {
    console.error("[Home] End session failed:", err);
  } finally {
    setTimeout(() => { engageLockRef.current = false; }, 500);
  }
};
```

### C.5 — `handlePause` / `handleResume`

```typescript
const handlePause = async () => {
  if (!isEngaged || isPaused) return;
  try {
    await invoke("pause_pipeline");
    setIsPaused(true);
    // Transcript is NOT cleared — user can see what was said before pause
    // Only archive the in-progress turn into history
    archiveCurrentTurn();
  } catch (err) { console.error("[Home] Pause failed:", err); }
};

const handleResume = async () => {
  if (!isEngaged || !isPaused) return;
  try {
    await invoke("resume_pipeline");
    setIsPaused(false);
    // Clear transcript display for fresh start
    setTranscript("");
    setAssistantText("");
  } catch (err) { console.error("[Home] Resume failed:", err); }
};
```

**On pause:** Current turn transcript is archived into dialogue history (visually dimmed), display cleared. This is the correct UX — user sees what was said before they paused, but the active input area is blank.

### C.6 — `togglePtt` (Realtime PTT mode)

The existing `ptt_start`/`ptt_stop` IPC calls remain. The only change:

```typescript
// Guard: only callable in PTT mode, only when engaged, only when not paused
const togglePtt = async () => {
  if (!isEngaged || isPaused) return;
  if (interactionMode !== "PTT") return; // safety guard

  if (pttStatus === "IDLE") {
    archiveCurrentTurn(); // archive previous exchange before new utterance
    await invoke("ptt_start", { owner: "MainWindow" });
  } else {
    await invoke("ptt_stop", { owner: "MainWindow" });
  }
};
```

No `barge_in` IPC call from the mic button — that's not its purpose in PTT mode. Barge-in (interrupt) in passive mode is via the **Pause button**.

### C.7 — Tauri Event Listener Changes

**New events to listen for:**

```typescript
// Replace/extend existing setup():
await appWindow.listen("realtime_session_started", () => {
  setPipelineMode("realtime");
  setSessionResumed(false);
});

await appWindow.listen("realtime_session_resumed", () => {
  setPipelineMode("realtime");
  setSessionResumed(true);
  // Show "Resumed previous session" toast
});

await appWindow.listen("realtime_session_ended", (event) => {
  const reason = event.payload as string; // "user", "idle_timeout", "error"
  setIsEngaged(false);
  setIsPaused(false);
  setPipelineMode("modular");
  flushAndClearTranscripts();
  if (reason === "idle_timeout") {
    // Show "Session ended: idle timeout" toast
  }
});

await appWindow.listen("pipeline_paused", () => {
  setIsPaused(true);
  archiveCurrentTurn();
});

await appWindow.listen("pipeline_resumed", () => {
  setIsPaused(false);
});

await appWindow.listen("realtime_interrupted", () => {
  // Flash the UI briefly — barge-in confirmed
  setInteractionState("Interrupted");
  setTimeout(() => setInteractionState("UserSpeaking"), 150);
});
```

**Remove:** The 4-turn cap on `dialogueHistory` via `next.slice(-4)`. For realtime sessions, transcripts accumulate for the full session. The `dialogueHistory` slice limit only applies to modular mode.

```typescript
// Modified archiveCurrentTurn:
const archiveCurrentTurn = useCallback(() => {
  ...
  setDialogueHistory(prev => {
    const next = [...prev, { user: userText, assistant: aiText, id: turnIdCounter.current }];
    // Only cap in modular mode — realtime sessions show full session history
    return pipelineMode === "modular" ? next.slice(-4) : next;
  });
}, [pipelineMode]);
```

### C.8 — Transcript Flush on Pause

When pause is triggered (either from button click OR from `pipeline_paused` backend event):
1. `archiveCurrentTurn()` — moves active text to history
2. `setTranscript("")` — clears input display
3. `setAssistantText("")` — clears assistant display
4. History items remain visible (scrollable), dimmed slightly via CSS class `opacity-60`

This means: user can read what was said in the session so far, but the "active" area is blank and ready for the next utterance after resume.

---

## Phase D — Timeout & Error State Handling

### D.1 — Idle Timeout Timer UI

A countdown shown in the `StatusCapsule` label or a subtle overlay during realtime sessions when no speech has been detected for >30s:

```
Status: "Idle · 12s"  ← countdown before auto-disconnect
```

Backend emits `realtime_idle_warning` with `{ seconds_remaining: number }` at 15s and 5s marks.
Frontend updates the status label accordingly.

### D.2 — Max Reconnect Failure

When the reconnection loop in `gemini_live.rs` exhausts max attempts, it currently emits `VoxEvent::Error`. The pipeline orchestrator must translate this into a `"pipeline_error"` Tauri event. The frontend already listens to `pipeline_error` — extend it:

```typescript
await appWindow.listen("pipeline_error", (event) => {
  const msg = event.payload as string | undefined;
  // If we are in realtime mode:
  if (pipelineMode === "realtime") {
    setIsEngaged(false);
    setPipelineMode("modular");
    flushAndClearTranscripts();
    // Show error toast: msg or "Connection to Gemini Live lost"
  }
  // Existing test clip handling unchanged
});
```

### D.3 — API Key Missing Fast-Fail

When `start_realtime_session` fails with "No API key configured":
- Backend returns `Err("No API key configured...")`
- Frontend catches in `handleEngage`'s try/catch
- Show a targeted error: "Set your Gemini API key in Settings → Realtime" with a direct link to open Settings page

---

## Phase E — Session Cache IPC Exposure

A new `get_realtime_session_cache` IPC command returns:

```rust
#[derive(serde::Serialize)]
pub struct RealtimeSessionCache {
    pub has_session: bool,
    pub provider: String,
    pub expires_in_seconds: i64, // negative = expired
    pub model: String,
}
```

Called on Home mount — if a cached session exists and pipeline_mode is realtime, the engage button shows a subtle "Resume Session" label instead of plain "Engage".

## Phase F — Audio Performance, Safety, and React Race Condition Fixes [ADDED]

### F.1 — `services/realtime/resampler.rs` — Heap Thrashing Fix
Optimize the audio hot-path in `process_i16` to eliminate dynamic allocations every 10ms:
* Replace the `.drain().collect()` allocation with direct indexing or contiguous slice copying:
  ```rust
  self.resampler_in_buf[0].clear();
  self.resampler_in_buf[0].extend_from_slice(&self.input_buf[..self.nbr_frames_needed]);
  self.input_buf.drain(..self.nbr_frames_needed);
  ```
* Ensure `SequentialSliceOfVecs` wrapper adapters reuse the pre-allocated inner buffer capacities instead of generating new vector handles.

---

### F.2 — Bounded Channels in `AudioBridge` / `PlaybackBridge`
Avoid unbounded channels and manual queue depth tracking to resolve backpressure issues natively:
* Delete `queue_depth` tracking atomics.
* Replace `unbounded_channel` with bounded channel `tokio::sync::mpsc::channel::<Vec<i16>>(100)`.
* In `send_pcm(&self, samples: &[i16])`, use `tx.try_send(samples.to_vec())`. If the channel is full, log the dropped chunk and proceed:
  ```rust
  if let Err(e) = tx.try_send(samples.to_vec()) {
      match e {
          tokio::sync::mpsc::error::TrySendError::Full(_) => {
              log::warn!("[AudioBridge] Buffer full, dropping input audio chunk.");
          }
          tokio::sync::mpsc::error::TrySendError::Closed(_) => {
              log::debug!("[AudioBridge] Channel closed.");
          }
      }
  }
  ```

---

### F.3 — `Home.tsx` — Engage Ref Race Condition
Ensure timeouts modifying refs or states are cleared when the component unmounts:
* Declare a ref to hold the timeout handle:
  ```typescript
  const engageTimeoutRef = useRef<NodeJS.Timeout | number | null>(null);
  ```
* In `handleEngage` / `handleEnd` finally blocks, assign the `setTimeout` to `engageTimeoutRef.current`.
* In the `useEffect` cleanup hook, clear the timeout if it exists:
  ```typescript
  return () => {
    if (engageTimeoutRef.current) {
      clearTimeout(engageTimeoutRef.current);
    }
    unlisteners.forEach((u) => u());
  };
  ```

---

## Verification Plan [MODIFIED]

### Automated Tests & Benchmarks
- `cargo check` — compile gate after backend changes
- `cargo run --bin vox-bench` — verify local pipeline flows
- `cargo run --bin vox_realtime_bench` — verify cloud/realtime S2S pipeline integration and latencies

### Manual Scenarios (in order)

| # | Scenario | Expected |
|---|---|---|
| 1 | Engage in realtime/passive → speak → Gemini responds | State: Listening → UserSpeaking → AssistantSpeaking → Listening |
| 2 | Engage → press Pause mid-response | Audio stops, WS alive, transcript archived, UI shows paused |
| 3 | Pause → press Resume | Listening resumes, blank input |
| 4 | Engage in realtime/PTT → press Mic → speak → release | VAD gates audio, ActivityStart/End sent, response arrives |
| 5 | Engage in PTT → hold Mic with silence only | Audio discarded (VAD gate), no hallucination |
| 6 | Engage realtime → idle for 10 minutes | Auto-disconnect, toast shows |
| 7 | Engage → disconnect network → reconnect | Reconnect with resume token, "Resumed session" label |
| 8 | Engage realtime, check `~/.vox/cache/realtime_session.json` | File exists with valid token + TTL |
| 9 | Press Engage when session already active | engageLockRef blocks double-engage |
| 10 | Engage with no API key | Fast-fail error toast with Settings link |

---

## Files Changed Summary [MODIFIED]

### Backend (Rust)
| File / Directory | Change |
|---|---|
| [services/audio/](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio) | **[NEW]** Reorganized audio module. Move `audio.rs` -> `device.rs`, `playback.rs` -> `playback.rs`, add `router.rs` and `mod.rs`. |
| [resampler.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/resampler.rs) | Optimize `process_i16` hot-path to clear and reuse `resampler_in_buf` capacity. |
| [audio_bridge.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/audio_bridge.rs) | Replace `unbounded_channel` and atomics with bounded channel and native `try_send` drops. |
| [playback_bridge.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/playback_bridge.rs) | Replace `unbounded_channel` with bounded channel. |
| [state.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/state.rs) | Add `is_paused: Arc<AtomicBool>`, `ptt_start_ms: Arc<AtomicU64>`, and `speech_detected: AtomicBool` to Pipeline/PTT states. |
| [pipeline.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/pipeline.rs) | Conditionally spawn VAD/STT threads in `launch_engine`. Integrate router thread. |
| [vad/actor.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs) | Flush VAD `pre_roll_buffer` (converted to i16) on onset transition to `realtime_tx`. |
| [ptt.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/ptt.rs) | Implement PTT duration check. Reset `speech_detected` on start, validate it on stop. |
| [gemini_live.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/gemini_live.rs) | Handle 10-minute idle disconnect and save resumption token. |
| [lib.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/lib.rs) | Register new IPC commands. |

### Frontend (TypeScript)
| File | Change |
|---|---|
| [Home.tsx](file:///home/addy/projects/apps/vox/app/src/pages/Home.tsx) | Clean timeout refs on unmount. Add universal Play/Pause & disengage buttons. |

---

> [!IMPORTANT]
> We implement all of this in **one phase, top-to-bottom, backend first then frontend**. Do not split. The IPC contracts between backend and frontend must be consistent by the time we touch Home.tsx.
