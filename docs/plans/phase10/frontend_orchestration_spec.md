# Frontend Orchestration Specification (Phase 10)

> **Status:** ACTIVE Architectural Specification  
> **Location:** `docs/plans/phase10/frontend_orchestration_spec.md`  
> **Scope:** `app/src/` (Services, Contexts, Hooks, Pages, and Components)  
> **Backend Counterpart:** [`docs/plans/phase10/pipeline_orchestration_spec.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/pipeline_orchestration_spec.md)  
> **Standard:** Strict 1:1 IPC alignment, discrete UI action verbs, zero toggle commands, zero mode branching for lifecycle.

---

## 1. Objective & Core Principles

This specification defines the frontend integration contract for the Phase 10 pipeline refactor. It eliminates legacy toggle commands, merges fragmented Modular vs. Realtime session dispatchers, aligns TypeScript enums with the 7 canonical Rust states, and establishes mode-adaptive UI controls.

### Architectural Invariants:
1. **Zero Frontend Mode Branching for Lifecycle:** Frontend never checks `pipeline_mode === 'Realtime'` to decide which session stop/start API to call. The frontend calls `start_session` and `end_session` unconditionally; backend `RoutingContext` resolves the execution domain.
2. **Discrete Non-Toggle UI Verbs:** Eliminates the single toggle `engage()`. Replaces with explicit `engage()` (calls `start_session`), `disengage()` (calls `end_session`), `pause()` (calls `pause_session`), and `resume()` (calls `resume_session`).
3. **Session State vs. Turn State Separation:**
   - **Tier 1 (Session Lifecycle):** Tracked via `isEngaged: boolean`. `false` = Dormant/Unengaged (`Idle`); `true` = Active/Warm (`Ready` and onwards).
   - **Tier 2 (Turn State):** Tracked via `interactionState: InteractionState` (`"Idle"` | `"Ready"` | `"Listening"` | `"Thinking"` | `"Speaking"` | `"Paused"` | `"Error"`).

---

## 2. Tauri IPC Command Mapping

### 2.1 Deprecated vs. Canonical IPC Commands

| Legacy IPC Command | Replacement Phase 10 Command | TS Service Function | Action Description |
|---|---|---|---|
| `invoke("engage")` (Toggle) | `invoke("start_session")` | `startSession()` | Starts active session (Modular or Realtime). |
| `invoke("engage")` / `stopRealtimeSession()` | `invoke("end_session")` | `endSession()` | Stops active session and clears turn state. |
| `invoke("pause_pipeline")` | `invoke("pause_session")` | `pauseSession()` | Pauses active session (Passive mode). |
| `invoke("resume_pipeline")` | `invoke("resume_session")` | `resumeSession()` | Resumes paused session (Passive mode). |
| `invoke("ptt_start")` | `invoke("ptt_start")` | `pttAudioStart()` | Opens PTT audio gate and starts buffering. |
| `invoke("ptt_stop")` | `invoke("ptt_stop")` | `pttAudioStop()` | Closes PTT gate and sends buffer to STT/S2S. |
| `invoke("ptt_cancel")` | `invoke("ptt_cancel")` | `pttAudioCancel()` | Drops active PTT buffer without inference. |
| `invoke("test_clip_cancel")` | `invoke("test_clip_cancel")` | `cancelTestClip()` | Cancels active developer QA audio test clip. |

---

## 3. TypeScript Type Definitions (`src/services/eventsService.ts`)

```typescript
/**
 * Canonical Rust `InteractionState` enum (core/state.rs).
 * Drives mood sync, visualizers, and UI state indicators.
 */
export type InteractionState =
  | "Idle"               // Dormant / unengaged (isEngaged = false)
  | "Ready"              // Session active, warm & waiting for speech or PTT hold
  | "Listening"          // User is actively speaking; Vox is capturing voice
  | "Thinking"           // Turn complete; LLM inference or RAG compaction active
  | "Speaking"           // System audio playback is actively streaming through speakers
  | "Paused"             // User explicitly paused session
  | "Error";             // Subsystem or provider error

/** Canonical Rust `InteractionOwner` enum (core/state.rs). */
export type InteractionOwner = "Assistant" | "Dictation";

/** Payload emitted on `state_changed` event. */
export type StateChangedPayload = InteractionState;

/** Payload emitted on `transcript_partial` and `transcript_final`. */
export interface TranscriptPayload {
  turn_id: number;
  text: string;
  owner: InteractionOwner;
}

/** Mirror of `TelemetryData` emitted on `telemetry`. */
export interface TelemetryData {
  energy: number;
  vad_prob: number;
  low: number;
  mid: number;
  high: number;
}
```

---

## 4. Voice Session Context API (`src/shared/context/VoiceSessionContext.tsx`)

The root `<VoiceSessionProvider>` maintains long-lived pipeline state across page navigations:

```typescript
export interface VoiceSessionContextValue {
  // State
  isEngaged: boolean;
  interactionState: InteractionState;
  isPaused: boolean;
  dialogueHistory: DialogueTurn[];
  activeTranscript: string;
  activeAssistantText: string;
  telemetry: TelemetryData | null;
  errorAlert: string | null;

  // Discrete UI Actions
  engage: () => Promise<void>;        // Calls startSession()
  disengage: () => Promise<void>;     // Calls endSession()
  pause: () => Promise<void>;         // Calls pauseSession()
  resume: () => Promise<void>;        // Calls resumeSession()

  // PTT Actions
  handlePttStart: () => Promise<void>;
  handlePttStop: () => Promise<void>;
  handlePttCancel: () => Promise<void>;

  // Clear / Reset
  clearHistory: () => void;
  dismissError: () => void;
}
```

### Action Logic:
1. **`engage()`**:
   - Sets `isEngaged = true`.
   - Calls `pipelineService.startSession()`.
   - In PTT mode, initial state resolves to `"Idle"`. In Passive mode, resolves to `"Listening"`.
2. **`disengage()`**:
   - Calls `pipelineService.endSession()`.
   - Sets `isEngaged = false`, resets `activeTranscript` and `activeAssistantText`.
3. **`pause()` / `resume()`**:
   - Calls `pauseSession()` / `resumeSession()`. Backend emits `state_changed` (`"Paused"` / `"Listening"`).

---

## 5. UI Controls & Mode Adaptation (`src/pages/Home.tsx`)

The Home interface adapts dynamically based on `settings.interaction.mode` (Passive vs. PTT):

```
PASSIVE MODE TOOLBAR:
┌────────────────────────────────────────────────────────────────────────┐
│   [ ⏸️ Pause / ▶️ Resume ]                [ ⏹️ Disengage Session ]       │
└────────────────────────────────────────────────────────────────────────┘

PTT (PUSH-TO-TALK) MODE TOOLBAR:
┌────────────────────────────────────────────────────────────────────────┐
│   (Central Orb acts as Hold-to-Talk)        [ ⏹️ Disengage Session ]   │
│   [ 🎙️ Press & Hold Space or Click Orb ]                              │
└────────────────────────────────────────────────────────────────────────┘
```

### 5.1 Ambient Mood Resolution (`src/shared/hooks/useHomePage.ts`)

```typescript
export type AmbientMood =
  | "Idle"
  | "Ready"
  | "Listening"
  | "Thinking"
  | "Speaking"
  | "Paused"
  | "Error";

export function toMood(state: InteractionState, isEngaged: boolean): AmbientMood {
  if (!isEngaged) return "Idle";
  return state;
}
```

### 5.2 PTT Gesture Bindings (`src/shared/components/home/AdvancedOrb.tsx`)
In PTT mode when `isEngaged === true`:
- `onPointerDown` ──► `handlePttStart()`
- `onPointerUp`   ──► `handlePttStop()`
- `onPointerLeave` / `onKeyDown(Escape)` ──► `handlePttCancel()`

---

## 6. Frontend Execution & Refactor Checklist

### 6.1 Services Layer (`src/services/`)
- [ ] `pipelineService.ts`: Replace `engage()` / `stopRealtimeSession()` / `pausePipeline()` with `startSession()`, `endSession()`, `pauseSession()`, `resumeSession()`.
- [ ] `eventsService.ts`: Update `InteractionState` to 7 canonical variants, update `InteractionOwner` to `"Assistant" | "Dictation"`.

### 6.2 State & Context Layer (`src/shared/context/`)
- [ ] `VoiceSessionContext.tsx`: Implement discrete `engage`, `disengage`, `pause`, `resume`, `handlePttStart`, `handlePttStop`, `handlePttCancel`. Remove toggle logic.

### 6.3 Hooks & UI Pages (`src/pages/`, `src/shared/hooks/`, `src/shared/components/`)
- [ ] `useHomePage.ts`: Update `toMood()` and remove legacy mode branching.
- [ ] `Home.tsx`: Update toolbar rendering to hide Pause/Resume in PTT mode.
- [ ] `AdvancedOrb.tsx`: Connect PTT pointer gestures directly to context handlers.

### 6.4 Vitest Test Suite Updates
- [ ] Update `pipelineService.test.ts` to mock and assert `start_session` and `end_session`.
- [ ] Update `useHomePage.test.ts` to assert discrete `start_session`/`end_session` dispatches.
- [ ] Ensure `pnpm test` (or `vitest run`) passes 100%.
