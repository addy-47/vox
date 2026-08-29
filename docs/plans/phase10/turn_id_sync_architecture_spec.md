# Turn ID Synchronization Architecture Specification

---

## 1. Executive Summary & Purpose

This specification defines the authoritative, monotonic lifecycle of the conversational `turn_id` across all Vox interaction pipelines (`modular`, `realtime`, and `dictation`).

### Core Objectives:
1. **Single Monotonic Authority:** Eliminate conflicting minting sources and hardcoded `0` defaults. The `turn_id` is an atomic, strictly monotonic integer (`AtomicU32`) minted exclusively at the **Turn Inception Seam**.
2. **Deterministic Cancellation & Staleness Guard (Epoch Invalidation):** Protect against async race conditions and late audio bleed-over. Every asynchronous worker (STT, LLM, TTS, Playback) validates `chunk.turn_id == current_active_turn_id`. Work carrying an older `turn_id` is dropped instantly.
3. **Clean Ghost Audio Termination:** In Push-To-Talk modes, if a recording window contains no human speech, the minted `turn_id` is closed as a silent no-op without downstream dispatch, and the state machine resets cleanly to `Ready`.
4. **End-to-End Traceability & History Attribution:** The immutable `turn_id` is propagated through STT -> LLM -> TTS -> Playback -> Turso SQLite DB, guaranteeing 1:1 attribution between user input, assistant output, and dynamic memory.

---

## 2. The Turn ID Lifecycle Matrix

```
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 TURN ID LIFECYCLE & ROUTING MATRIX                                     │
├──────────────────────┬──────────────────────┬─────────────────────────────┬────────────────────────────┤
│ Pipeline Mode        │ Minting Inception    │ Ghost Audio / Silence Gate  │ Barge-In / Interruption    │
├──────────────────────┼──────────────────────┼─────────────────────────────┼────────────────────────────┤
│ 1. Modular Passive   │ Local VAD detects    │ Speech < 150ms: Utterance   │ User speaks during TTS:    │
│                      │ SpeechStart          │ dropped. State -> Ready.    │ Mints T_new, cancels DAC,  │
│                      │ (fetch_add(1))       │ Zero STT/LLM dispatch.      │ invalidates old TTS queue. │
├──────────────────────┼──────────────────────┼─────────────────────────────┼────────────────────────────┤
│ 2. Modular PTT       │ PTT Key Press        │ Release with no speech:     │ PTT press during playback: │
│                      │ (handle_ptt_start)   │ Audio cleared. State->Ready.│ Mints T_new, cancels DAC,  │
│                      │ (fetch_add(1))       │ Zero STT/LLM dispatch.      │ invalidates old turn.      │
├──────────────────────┼──────────────────────┼─────────────────────────────┼────────────────────────────┤
│ 3. Realtime Passive  │ Server signals User  │ Server VAD handles noise.   │ Server sends 'interrupted':│
│    (Gemini / S2S)    │ Turn Start via WS    │ Client simply forwards raw  │ Mints T_new, cancels DAC,  │
│                      │ (fetch_add(1))       │ audio stream.               │ State -> Listening.        │
├──────────────────────┼──────────────────────┼─────────────────────────────┼────────────────────────────┤
│ 4. Realtime PTT      │ PTT Key Press        │ Release with no speech:     │ PTT press during playback: │
│    (Gemini / S2S)    │ (handle_ptt_start)   │ Buffer cleared. State->Ready│ Mints T_new, cancels DAC,  │
│                      │ (fetch_add(1))       │ Zero WebSocket push.        │ arms new buffer.           │
├──────────────────────┼──────────────────────┼─────────────────────────────┼────────────────────────────┤
│ 5. Dictation         │ Hotkey Press or      │ Silence: Discarded.         │ Hotkey release stops STT;  │
│    (PTT / Passive)   │ SpeechStart          │ No OS keystroke injection.  │ Injects text to active app.│
└──────────────────────┴──────────────────────┴─────────────────────────────┴────────────────────────────┘
```

---

## 3. Detailed Component Invariants

### 3.1 Turn Inception & Monotonic Minting
A turn is minted exactly once per interaction cycle:
```rust
let turn_id = state.pipeline.turn_id.fetch_add(1, Ordering::SeqCst) + 1;
state.pipeline.active_turn_id.store(turn_id, Ordering::SeqCst);
```
- **Monotonicity Invariant:** `turn_id` is never decremented or reset across a runtime session.
- **Zero Default Purge:** No function signature, event emitter, or IPC struct may pass `0` or `_turn_id` placeholder variables.

### 3.2 Ghost Audio / Silence Rejection in PTT
```
User Presses PTT ──> Mints Turn 42 ──> UI enters Listening
                           │
                 User Releases PTT Key
                           │
          Is Human Speech Detected in Window?
            ├── NO  ──> Discard Audio Buffer
            │           Emit EVENT_PTT_STATUS { state: "idle" }
            │           UI returns to Ready
            │           (Turn 42 closes as silent no-op; ZERO STT/LLM/DB calls)
            │
            └── YES ──> Emit EVENT_PTT_STATUS { state: "processing", turn_id: 42 }
                        Dispatch to STT / Cloud WebSocket
                        UI enters Thinking
```

### 3.3 Barge-In Invalidation & Playback Staleness Guard
When a user initiates speech or presses PTT during active assistant playback:
1. **New Generation Minted:** `active_turn_id` is incremented to $T_{new}$.
2. **Instant DAC Truncation:** `PlaybackEngine::cancel()` is called:
   - Sets `cancel_flag = true` and `discard_request = true`.
   - Flushes output ring buffer to 0 samples within 5ms.
3. **Async Worker Invalidation:**
   - **LLM Actor:** Checks `if turn_id != active_turn_id.load()`; drops connection immediately if mismatched.
   - **TTS Actor:** Checks `if chunk.turn_id < active_turn_id.load()`; discards synthesized frames.
   - **Playback Ingest:** `PlaybackEngine::ingest_chunk` verifies `chunk.turn_id == active_turn_id`. Stale chunks are dropped before reaching the buffer.

### 3.4 History & Dynamic Memory Attribution
When playback finishes normally without interruption:
1. `VoxEvent::PlaybackFinished { turn_id }` is emitted.
2. `ConversationManager` persists the turn pair to Turso SQLite indexed by `turn_id`:
   ```sql
   INSERT INTO conversation_turns (session_id, turn_id, user_text, assistant_text, duration_ms)
   VALUES (?, ?, ?, ?, ?);
   ```
3. Dynamic memory compaction worker indexes user facts mapped to `turn_id`.

---

## 4. Verification & Invariants

1. **Zero Turn ID Collisions:** Monotonic generation guarantees distinct IDs for all turns across runtime.
2. **Zero In-Flight Audio Bleed:** When Barge-In occurs at Turn $N$, not a single PCM sample tagged with $N$ is played once Turn $N+1$ begins.
3. **Ghost Audio Clean Reset:** Releasing PTT without speaking transitions state back to `Ready` without emitting STT commands or DB writes.
4. **Compile-Time Elimination of Zero Defaults:** Grep for `_turn_id` or `0` passed as `turn_id` must return 0 hits in pipeline logic.
