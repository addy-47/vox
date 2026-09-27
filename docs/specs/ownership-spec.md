# Interaction Track Ownership & Focus Lifecycle Specification (v1)

---

- **Status:** Approved Target Spec (SSOT for Interaction Track Ownership & Focus Lifecycle)
- **Policy:** Zero Backward Compatibility (ZBC) — Clean architectural break; no legacy wrappers.
- **Scope:** Defines ownership invariants, state machines, hotkey preemption, and session handovers across the dual-track Vox architecture.

---

## 1. Executive Summary & Core Concept

Vox operates with two concurrent execution tracks:
1. **Assistant Track (Session-Scoped):** Conversational AI harness managing multi-turn dialogue, cognitive tools, and LLM/TTS generation.
2. **Dictation Track (Ambient OS Utility):** Low-latency speech-to-text pipeline that pastes transcribed text directly into active desktop applications.

### Architectural Core Model
Historically, Vox treated `state.owner` as an uncoordinated global variable that both tracks and hotkeys mutated. The target model establishes:
- **`state.owner` is the Assistant Session State Marker:** It answers one simple question: *"Is the user currently engaged in an active assistant session, or is the system in ambient desktop dictation mode?"*
- **Ownership is Strictly IPC/User-Command Driven:** `state.owner` is mutated **exclusively** by explicit user session lifecycle commands (`SessionStart`, `PauseSession`, `ResumeSession`, `EndSession`) and initialized on application boot.
- **Dictation NEVER Touches `owner`:** The global dictation hotkey (`Alt+Space`) and the dictation pipeline strictly **consume** events and check state. They **never** mutate `state.owner`.
- **Gating Replaces Hijacking:** If the Assistant is engaged (`Listening`, `Thinking`, `Speaking`, `Working`), pressing the dictation hotkey does not hijack or overwrite the pipeline; it is cleanly gated and notifies the user.

---

## 2. Mandatory Architectural Invariants

### Invariant 1: State Check Over Settings Check (Zero Settings Sniffing)
Neither the Router, VAD, STT, nor any pipeline actor ever reads raw settings (e.g. `settings.dictation.enabled`) to evaluate whether dictation is active:
- **`dictation_state == InteractionState::Idle`** $\iff$ Dictation is disabled.
- **`dictation_state == InteractionState::Ready`** $\iff$ Dictation is enabled and standing by.

Settings mutations in `ipc/settings/core.rs` exclusively drive `transition_dictation(Ready | Idle)`. All runtime code branches strictly on `dictation_state`.

### Invariant 2: Strict IPC User-Command Ownership Driver
`state.owner` is mutated **exclusively** by explicit user-facing session lifecycle actions:
1. **Application Boot:** `state.owner` is initialized to `InteractionOwner::Dictation` (ambient default).
2. **`SessionStart` IPC:** User initiates an assistant conversation $\to$ sets `state.owner = InteractionOwner::Assistant`.
3. **`PauseSession` IPC (or sustained Idle Monitor):** User pauses assistant session $\to$ sets `state.owner = InteractionOwner::Dictation`.
4. **`ResumeSession` IPC:** User resumes assistant session $\to$ sets `state.owner = InteractionOwner::Assistant`.
5. **`EndSession` IPC:** User terminates assistant session $\to$ sets `state.owner = InteractionOwner::Dictation`.

**Zero Backend / Worker Ownership Mutation:** Background worker threads, STT actors, VAD listeners, and the hotkey daemon have **zero authority** to write to `state.owner`.

### Invariant 3: Zero Hotkey Ownership Mutation
The global shortcut listener (`services/dictation/hotkey.rs`) must **never** call `state.owner.store(...)`. It enqueues self-describing events (`VoxEvent::PttStart { owner: Dictation }`) and leaves `state.owner` completely untouched.

### Invariant 4: Deterministic Hotkey Preemption & Gating
When `VoxEvent::PttStart { owner: Dictation }` arrives at the Router / Dictation Handler:
1. **Disabled Gate:** If `dictation_state == Idle` $\to$ drop audio and emit notification toast: *"Dictation is disabled in Settings"*.
2. **Assistant Engaged Gate:** If `assistant_state` is in `[Listening, Thinking, Speaking, Working]` $\to$ drop audio and emit notification toast: *"Cannot use dictation hotkey while Assistant is engaged"*.
3. **Nominal Execution:** If `dictation_state == Ready` and `assistant_state` is in `[Idle, Ready, Paused, Sleeping, Error]` $\to$ dictation proceeds normally (`transition_dictation(Listening)`), transcribes, pastes text, and returns to `Ready`. `state.owner` remains completely unaffected throughout the turn.

### Invariant 5: Settings Mutation Invariant
Mutating settings (e.g. enabling/disabling dictation, changing output mode, switching passive/PTT) updates `dictation_state` and VAD operational modes, but **never mutates `state.owner`**.

---

## 3. Exhaustive User Action & Ownership Transition Matrix

| # | User Action / Trigger | IPC Command / Input Source | Enqueued Event (`VoxEvent`) | State Preconditions & Gating | Action Semantics & Execution | Resulting Owner (`state.owner`) | Reversion / Fallback Rule |
|---|---|---|---|---|---|---|---|
| **1** | **Application Launch** | App Startup (`lib.rs` / `state.rs`) | *None* | Fresh process start | Initializes audio engine and state atomics. Evaluates `settings.dictation.enabled`: if true `dictation_state = Ready`, else `Idle`. | **`Dictation`** | Default boot state. Never reverts. |
| **2** | **Start Assistant Session** | `start_session(sessionId)` | `SessionStart { session_id }` | `assistant_state == Idle` (else rejected with error) | Mounts Harness, warms STT/LLM/TTS, sets VAD operational mode, transitions `assistant_state` $\to$ `Ready`. | **`Assistant`** | If session startup errors $\to$ transitions `assistant_state` $\to$ `Error`, reverts `owner = Dictation`. |
| **3** | **Pause Assistant Session** | `pause_session` (or 7m idle timeout) | `PauseSession` | `assistant_state \in {Ready, Listening, Thinking, Speaking, Working}` | Cancels active playback/tokens, flushes accumulator, keeps CPAL warm, transitions `assistant_state` $\to$ `Paused`. | **`Dictation`** | Resumes to `Assistant` on `ResumeSession`. |
| **4** | **Resume Assistant Session** | `resume_session` | `ResumeSession` | `assistant_state \in {Paused, Sleeping, Error}` | Re-arms VAD and provider streams, re-warms models if sleeping, transitions `assistant_state` $\to$ `Ready`. | **`Assistant`** | If resume fails $\to$ transitions `assistant_state` $\to$ `Error`, reverts `owner = Dictation`. |
| **5** | **End Assistant Session** | `end_session` | `EndSession` | Any active assistant state (idempotent) | Unmounts Harness, clears tokens/accumulators, records session end metrics, transitions `assistant_state` $\to$ `Idle`. CPAL hardware torn down only if `dictation_state == Idle`. | **`Dictation`** | Terminal action. |
| **6** | **Dictation Hotkey Press (Nominal)** | Global OS Shortcut (`Alt+Space`) | `PttStart { owner: Dictation }` | `dictation_state == Ready`<br>`assistant_state \in {Idle, Ready, Paused, Sleeping, Error}` | Starts VAD window validation for dictation, transitions `dictation_state` $\to$ `Listening`. | **Unchanged** (Preserves active session owner) | On key release $\to$ `PttStop`. |
| **7** | **Dictation Hotkey Press (Dictation Disabled)** | Global OS Shortcut (`Alt+Space`) | `PttStart { owner: Dictation }` | `dictation_state == Idle` | Drops audio. Emits ephemeral toast: *"Dictation is disabled in Settings"*. | **Unchanged** | No state transition. |
| **8** | **Dictation Hotkey Press (Assistant Engaged)** | Global OS Shortcut (`Alt+Space`) | `PttStart { owner: Dictation }` | `assistant_state \in {Listening, Thinking, Speaking, Working}` | Drops audio. Emits ephemeral toast: *"Cannot use dictation hotkey while Assistant is engaged"*. Does not interrupt assistant. | **Unchanged** | No state transition. |
| **9** | **Dictation Hotkey Press (Dictation Thinking)** | Global OS Shortcut (`Alt+Space`) | `PttStart { owner: Dictation }` | `dictation_state == Thinking` | Drops audio. Emits ephemeral toast: *"Previous turn transcribing"*. Prevents simultaneous synthetic paste races. | **Unchanged** | Returns to `Ready` once prior STT completes. |
| **10** | **Dictation Hotkey Release** | Global OS Shortcut (`Alt+Space` released) | `PttStop { owner: Dictation }` | `dictation_state == Listening` | Stops VAD validation. If speech captured $\to$ `dictation_state` $\to$ `Thinking` and dispatches audio to STT. If silence $\to$ `dictation_state` $\to$ `Ready`. | **Unchanged** | If empty audio $\to$ returns directly to `Ready`. |
| **11** | **Dictation STT Finalized** | STT Actor Worker | `TranscriptFinal { turn_id, text, owner: Dictation }` | `dictation_state == Thinking` | Transliterates Devanagari if enabled, routes text to OS input simulator (Paste/Clipboard), emits terminal toast card, transitions `dictation_state` $\to$ `Ready`. | **Unchanged** | If paste fails $\to$ falls back to Clipboard with honest user notification. |
| **12** | **Toggle Dictation in Settings (Enable)** | `update_setting("dictation.enabled", true)` | *None (Direct IPC mutation)* | `dictation_state == Idle` | Sets `dictation_state = Ready`. If CPAL dormant, boots audio engine on-demand. | **Unchanged** | If engine boot fails $\to$ `dictation_state = Error`. |
| **13** | **Toggle Dictation in Settings (Disable)** | `update_setting("dictation.enabled", false)` | *None (Direct IPC mutation)* | `dictation_state != Idle` | Sets `dictation_state = Idle`. Dismisses active dictation HUD cards. If `assistant_state == Idle`, tears down CPAL engine to 0% CPU. | **Unchanged** | Clean teardown. |
