# Vox — Phase 5 Final Architecture Plan

## Phase Goal

Phase 5 transforms Vox from:

* a functional backend runtime

into:

* a coherent real-time interaction system.

This phase focuses on:

* frontend/runtime synchronization
* session coordination
* interaction ownership
* realtime UI behavior
* interruption UX
* stable orchestration

NOT:

* persistence
* memory systems
* advanced optimization
* hotword infrastructure
* ultra-low latency engineering

Those are intentionally deferred.

---

# Core Principles

## 1. Rust Is The Source Of Truth

Frontend is a dumb terminal.

Frontend NEVER:

* derives runtime state
* validates sessions
* infers ownership
* decides playback state
* interprets pipeline behavior

Rust backend owns:

* interaction state
* interaction ownership
* session lifecycle
* stale event rejection
* routing
* cancellation

Frontend only renders backend commands.

---

## 2. Single Active Interaction Owner

At all times there is ONLY ONE active interaction owner.

Allowed owners:

```rust
pub enum InteractionOwner {
    Tray,
    MainWindow,
    Ptt,
}
```

Ownership determines:

* where transcript events route
* which UI surface is active
* which controls are visible
* which session is authoritative

The actual audio pipeline remains shared.

We DO NOT:

* spawn multiple STT workers
* duplicate pipelines
* reload models
* create parallel runtimes

---

## 3. Shared Heavy Models, Resettable Streams

Critical distinction:

### Persistent

Heavy model weights remain permanently loaded:

* Sherpa STT model
* llama.cpp model
* TTS models

### Resettable

Lightweight runtime streams/contexts are disposable:

* STT stream
* transcript accumulators
* session buffers

---

# Final Runtime Architecture

```text
Audio Input
    ↓
AudioStream (CPAL)
    ↓
VAD Loop
    ↓
STT Worker
    ↓
PipelineOrchestrator
    ↓
LLM Worker
    ↓
TTS Worker
    ↓
Playback Engine
    ↓
Frontend IPC
```

---

# Phase 5 Deliverables

---

# STEP 1 — Unified Interaction State Machine

## Goal

Create centralized runtime interaction state.

Current state is fragmented across:

* playback atomics
* VAD
* frontend mock states
* pipeline events

This must unify.

---

## Backend State Enum

```rust
pub enum InteractionState {
    Idle,
    Listening,
    UserSpeaking,
    Thinking,
    AssistantSpeaking,
    Interrupted,
}
```

---

## Ownership

The state machine lives ONLY inside:

```text
PipelineOrchestrator
```

Reason:

* already coordinates runtime lifecycle
* already manages session flow
* already owns cancellation semantics

---

## Transition Rules

### Examples

```text
speech_start
    → UserSpeaking

transcript_final
    → Thinking

first_tts_chunk
    → AssistantSpeaking

cancelled
    → Interrupted

playback_finished
    → Idle
```

---

## IPC Contract

Rust emits ONLY:

```rust
StateChanged(InteractionState)
```

Frontend:

* blindly renders state
* does zero validation
* performs zero inference

---

# STEP 2 — Interaction Ownership System

## Goal

Prevent tray/main/PTT conflicts.

---

## Backend Owner Enum

```rust
pub enum InteractionOwner {
    Tray,
    MainWindow,
    Ptt,
}
```

Stored centrally in:

* `AppState`
  OR
* `PipelineOrchestrator`

---

## Ownership Rules

### Priority

```text
MainWindow
    > Ptt
    > Tray
```

---

## Main Window Acquisition

When user starts main assistant session:

```text
Acquire(MainWindow)
    ↓
cancel current transient session
    ↓
increment session_id
    ↓
reset STT stream
    ↓
clear transcript accumulators
    ↓
switch routing owner
    ↓
emit StateChanged(Listening)
```

---

## Critical Rule

Ownership switching DOES NOT:

* reload models
* restart workers
* recreate inference engines

ONLY:

* resets lightweight stream state
* resets session buffers
* changes routing ownership

---

# STEP 3 — STT Stream Reset Protocol (CRITICAL)

## Problem

Clearing Rust transcript strings is NOT enough.

Sherpa stream internally stores:

* acoustic features
* decoder context
* speech continuation state

Without reset:

* cross-session hallucinations occur

---

## Correct Architecture

Distinguish:

### Persistent

```text
OfflineRecognizer
```

### Disposable

```text
OfflineStream
```

---

## Required STT Worker Command

Add:

```rust
pub enum SttCommand {
    Partial(...),
    Final(...),
    ResetStream,
}
```

---

## Reset Behavior

On owner/session switch:

```text
ResetStream
    ↓
drop current Sherpa stream
    ↓
create fresh stream
```

Heavy model remains loaded permanently.

Expected reset cost:

* <5ms

This prevents:

* acoustic bleed
* transcript continuation hallucinations
* cross-surface contamination

---

# STEP 4 — Backend-Owned Event Validation

## Goal

Prevent stale event corruption.

Frontend must NEVER validate sessions.

Rust backend fully owns stale rejection.

---

## Rule

ALL pipeline events carry session_id internally.

PipelineOrchestrator drops:

* stale TTS chunks
* stale playback events
* stale LLM tokens
* stale transcript events

before IPC emission.

---

## Frontend Rule

Frontend trusts backend absolutely.

No:

* session comparisons
* stale filtering
* event reconciliation

---

# STEP 5 — IPC Telemetry Aggregation Layer

## Problem

Raw audio cadence:

* 50–100 updates/sec

Direct React updates at this cadence will:

* spike CPU
* freeze UI
* starve inference threads

Especially with:

* Three.js orb
* waveform rendering
* 2-core constraint

---

# Required Architecture

## Dedicated Telemetry Aggregator

Telemetry is fully decoupled from inference pipeline.

---

## Data Flow

```text
Audio/VAD threads
    ↓
atomic telemetry metrics
    ↓
telemetry aggregator thread
    ↓
throttled IPC events
```

---

## Important Rule

NO IPC inside:

* audio callback
* VAD hot loop
* inference loops

---

## Shared Metrics

Example:

```rust
Arc<AtomicU32> audio_level
Arc<AtomicU32> vad_probability
```

---

## Emission Rate

Target:

* 15–20Hz IPC max

Example:

* every 50ms

---

## IPC Event

Single compact event:

```rust
VisualTelemetry {
    amplitude: f32,
    vad: f32,
    speaking: bool,
    playback_active: bool,
}
```

Avoid granular spam events.

---

# STEP 6 — Frontend Runtime Refactor

## Goal

Remove React from realtime rendering path.

Current frontend still uses:

* React state for animation telemetry

This is unacceptable for realtime rendering.

---

# Required Frontend Architecture

## Replace

```tsx
setAmplitude()
setFrequency()
```

with:

* refs
* mutable runtime store
* RAF interpolation loop

---

## Correct Flow

```text
backend telemetry
    ↓
frontend refs
    ↓
requestAnimationFrame interpolation
    ↓
Three.js / Canvas update
```

NOT:

```text
IPC
    ↓
React setState
    ↓
rerender
```

---

## Orb Rules

Orb becomes:

* backend-reactive
* state-driven
* interpolation-smoothed

Orb visualizes:

* speaking state
* playback state
* interruption state
* audio energy

---

## Waveform Rules

Waveform driven ONLY from:

* real audio telemetry

NOT:

* fake timers
* CSS animation loops
* mock oscillation

---

# STEP 7 — Home Screen Integration

## Goal

Complete Home screen as primary realtime assistant surface.

Current frontend is mostly mocked.

Need:

* IPC wiring
* transcript streaming
* state-driven visuals
* realtime controls
* interruption feedback

---

## Required UI Behaviors

### Listening

* orb ambient active
* waveform subtle

### UserSpeaking

* waveform responsive
* orb reactive to mic energy

### Thinking

* contained orb motion
* subdued waveform

### AssistantSpeaking

* playback-reactive waveform
* synchronized orb pulse

### Interrupted

* immediate visual collapse/reset

---

# STEP 8 — Interruption UX Stabilization

## Goal

Frontend and backend interruption coherence.

Backend interruption already exists.

Need:

* visual synchronization
* ownership synchronization
* playback synchronization

---

## Required Behavior

During assistant playback:

```text
new user speech
    ↓
cancel_flag set
    ↓
playback cancel
    ↓
LLM/TTS stop
    ↓
StateChanged(Interrupted)
    ↓
StateChanged(UserSpeaking)
```

Frontend instantly reflects:

* interruption
* listening resumption
* playback stop

---

# STEP 9 — Tray/Main Routing Isolation

## Goal

Prevent transcript bleed between surfaces.

---

## Required IPC Metadata

Events internally contain:

* session_id
* owner

Example:

```rust
TranscriptPartial {
    session_id,
    owner,
    text,
}
```

---

## Backend Responsibility

PipelineOrchestrator routes events ONLY to active owner surface.

Frontend does NOT filter.

---

## Example

### Tray Active

```text
speech
    ↓
transcript_partial
    ↓
route → tray only
```

---

### Main Window Acquired

```text
Acquire(MainWindow)
    ↓
ResetStream
    ↓
increment session
    ↓
future events → main window only
```

Tray receives nothing.

---

# STEP 10 — Explicitly Deferred Systems

The following are OUT OF SCOPE for Phase 5:

## Deferred

* hotword engine
* persistence
* vector memory
* conversation history
* advanced optimization
* latency tuning
* speculative decoding
* multi-worker STT
* autonomous systems
* cloud routing

---

# Hotword Status

Hotword system is officially deferred.

Reason:

* continuous second audio pipeline
* scheduling conflicts on 2-core CPUs
* increased VAD contention
* unnecessary Phase 5 complexity

Hotword returns in future optimization phase only.

---

# Hardware Constraints

Target system:

* 8GB RAM
* CPU-only
* ~5.5GB usable inference budget
* realistic 2-core execution

Therefore:

* prioritize stability
* minimize IPC
* minimize render churn
* avoid duplicated inference workers
* avoid background overengineering

---

# Final Phase 5 Success Criteria

Phase 5 is complete when:

* Home screen fully backend-driven
* Orb/waveform react to real runtime telemetry
* Rust fully owns runtime state
* Interaction ownership stable
* Tray/main/PTT transitions clean
* No transcript bleed
* No stale event corruption
* Interruptions visually coherent
* Frontend remains smooth under CPU pressure
* No duplicated inference pipelines
* No model reloads during ownership switches

---

# Final Architectural Principles

Vox is NOT:

* a chatbot UI
* a request/response app
* a collection of independent features

Vox is:

* a realtime event-driven voice runtime
* with a reactive visual interaction layer

Phase 5 must preserve that architecture.
