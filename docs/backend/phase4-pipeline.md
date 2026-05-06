# `phase4_architecture.md`

---

# Vox — Phase 4 Architecture Plan

## Unified Realtime Inference Runtime (Backend-Only)

---

# 1. Objective

Phase 4 establishes the **native realtime inference runtime** for Vox.

This phase is NOT:

* frontend synchronization
* orb animation integration
* advanced UI logic
* persistent memory systems

This phase ONLY focuses on:

```text
STT → LLM → TTS → Playback
```

using:

* realtime-safe threading
* blocking native inference
* interruption-aware orchestration
* low-latency streaming

---

# 2. Core Architectural Principles

---

## ⚡ Native Blocking Inference

Both:

* `llama.cpp`
* `onnxruntime`

are fundamentally blocking native workloads.

Therefore:

```text
NO async inference execution
NO tokio-heavy orchestration
NO futures-based inference chains
```

Every heavy inference stage MUST run on dedicated OS threads.

---

## ⚡ Streaming Pipeline

The system MUST remain streaming-oriented:

```text
audio
 → vad
   → stt
     → llm
       → tts
         → playback
```

But:

```text
Phase 4 LLM execution only starts on FINAL transcript
```

Partial transcripts are UI-only.

---

## ⚡ Event-Driven Coordination

Internal backend systems communicate through:

* lightweight events
* channels
* atomics

Frontend IPC is ONLY a bridge layer.

---

## ⚡ Atomic Cancellation

Realtime interruption MUST use:

```rust
Arc<AtomicBool>
```

NOT:

* channels
* async cancellation
* event messages

because blocking C++ loops cannot be interrupted safely otherwise. ([GitHub][1])

---

## ⚡ Monolithic Module Architecture

DO NOT split into Cargo workspace crates.

Use:

```text
src/
 ├── audio.rs
 ├── vad.rs
 ├── stt.rs
 ├── llm.rs
 ├── tts.rs
 ├── playback.rs
 ├── pipeline.rs
 ├── metrics.rs
 └── state.rs
```

Reason:

* simpler ownership
* simpler channels
* shared atomics
* faster compilation
* avoids over-engineering

---

# 3. Final Runtime Topology

```text
CPAL Input Thread
    ↓
VAD Thread
    ↓
STT Worker
    ↓
Pipeline Orchestrator
    ↓
LLM Worker
    ↓
TTS Worker
    ↓
Playback Jitter Buffer
    ↓
CPAL Output Thread
```

---

# 4. Shared Runtime State

Add the following shared atomics:

```rust
Arc<AtomicBool> cancel_flag
Arc<AtomicBool> playback_active
Arc<AtomicBool> llm_generating
Arc<AtomicBool> tts_generating
```

Optional:

```rust
Arc<AtomicU32> session_id
```

for stale event rejection.

---

# 5. VoxSettings Additions

Extend `settings.rs`.

---

## Add Audio Output Mode

```rust
pub enum AudioOutputMode {
    Speaker,
    Headset,
}
```

Add:

```rust
pub audio_output_mode: AudioOutputMode
```

Default:

```rust
Speaker
```

---

# 6. Acoustic Echo Mitigation (CRITICAL)

---

# Problem

Without mitigation:

```text
TTS output
 → speaker
   → microphone
     → VAD
       → STT
         → infinite feedback loop
```

Neural AEC is too expensive for:

* CPU-only systems
* 8GB RAM target

---

# Solution

Use explicit user-configured routing mode.

---

## Mode 1 — Speaker Mode

When:

```text
audio_output_mode = Speaker
```

Behavior:

```text
playback_active == true
    ↓
Audio ingestion drops microphone frames
```

Implementation:

```rust
if playback_active.load(Ordering::Relaxed) {
    continue;
}
```

This prevents:

* self-triggering
* feedback loops
* recursive STT

---

## Mode 2 — Headset Mode

When:

```text
audio_output_mode = Headset
```

Behavior:

* microphone remains fully active
* true barge-in enabled

If VAD detects speech:

```text
speech_start
    ↓
cancel_flag.store(true)
    ↓
LLM aborts
    ↓
TTS aborts
    ↓
playback clears immediately
```

This enables instant interruption while using headphones.

---

# 7. Pipeline Orchestrator

Create:

```text
src/pipeline.rs
```

Responsibilities:

* route events
* coordinate workers
* manage cancellation
* enforce audio routing policy
* own session lifecycle
* own playback policy

---

## Responsibilities Table

| Responsibility        | Description                         |
| --------------------- | ----------------------------------- |
| Session Control       | manage active interaction lifecycle |
| Cancellation          | mutate shared cancel_flag           |
| Routing               | transcript → llm → tts              |
| Audio Policy          | speaker/headset handling            |
| Telemetry             | latency timestamps                  |
| Playback Coordination | playback queue lifecycle            |

---

# 8. Internal Event System

Create internal backend event enum:

```rust
enum VoxEvent {
    SpeechStart,
    SpeechEnd,

    TranscriptDelta(String),
    TranscriptFinal(String),

    LlmToken(String),
    LlmFinished,

    TtsChunk(Vec<f32>),
    TtsFinished,

    PlaybackStarted,
    PlaybackFinished,

    Cancelled,
    Error(String),
}
```

---

# 9. LLM Runtime (`llm.rs`)

---

# Objective

Implement native llama.cpp realtime generation worker.

---

## Responsibilities

* load GGUF
* maintain llama context
* token streaming
* cancellation checking
* prompt formatting
* generation telemetry

---

## Installed Model

Use:

```text
assets/gemma4/google_gemma-4-E2B-it-IQ2_M.gguf
```

---

## Context Limit

Start with:

```text
2048
```

NOT 4096 yet.

Reason:
KV cache growth becomes dangerous on low-RAM systems.

---

## Critical Cancellation Rule

Generation loop MUST check:

```rust
cancel_flag.load(Ordering::Relaxed)
```

on EVERY token callback.

---

## Phase 4 Generation Policy

LLM starts ONLY after:

```text
TranscriptFinal
```

NOT partial transcripts.

---

# 10. TTS Runtime (`tts.rs`)

---

# Objective

Implement streaming ONNX TTS synthesis worker.

---

## Use These ONNX Files

```text
speech_encoder.onnx
embed_tokens.onnx
conditional_decoder.onnx
language_model_q4.onnx
```

Avoid:

* fp16
* q4f16

for CPU-first systems.

---

## Responsibilities

* ONNX session management
* text chunk synthesis
* chunk streaming
* cancellation checks
* telemetry emission

---

## Critical Cancellation Rule

TTS synthesis MUST check:

```rust
cancel_flag.load(Ordering::Relaxed)
```

between chunk generations.

---

# 11. Playback Runtime (`playback.rs`)

---

# Objective

Implement stable low-jitter realtime playback.

---

# Critical Rule

Playback timing MUST be owned ONLY by:

```text
CPAL output callback
```

NOT:

* tokio timers
* sleeps
* async intervals

---

# Playback Architecture

```text
TTS Worker
    ↓
Playback Ring Buffer
    ↓
CPAL Output Stream
```

---

# Jitter Buffer (MANDATORY)

Playback MUST NOT begin immediately.

Instead:

* accumulate ~300ms audio
* THEN begin playback

This absorbs:

* ONNX jitter
* CPU spikes
* scheduling delays

without audio stuttering.

---

# Playback State

Playback worker owns:

```rust
playback_active.store(true)
```

when draining audio.

And resets to false after playback ends.

---

# 12. Metrics System (`metrics.rs`)

Add realtime latency telemetry.

---

## Required Timestamps

Track:

```text
speech_start
first_partial
final_transcript
llm_start
first_token
tts_start
first_audio
playback_start
playback_finish
```

---

# Goal

Realtime optimization WITHOUT telemetry is impossible.

---

# 13. Testing Strategy

Every step MUST be independently testable.

---

# STEP 1 — LLM Runtime Test

---

## Goal

Verify:

* GGUF loads
* tokens stream
* cancellation works

---

## Build

Create:

```text
tests/llm_test.rs
```

---

## Test Flow

```text
prompt
 → token stream
 → cancel mid-generation
```

---

## Success Criteria

* model loads successfully
* tokens stream incrementally
* cancellation aborts instantly
* no deadlock

---

# STEP 2 — TTS Runtime Test

---

## Goal

Verify:

* ONNX graph loads
* audio chunks generate
* cancellation works

---

## Build

Create:

```text
tests/tts_test.rs
```

---

## Test Flow

```text
text
 → chunk synthesis
 → wav output
```

---

## Success Criteria

* valid audio produced
* chunks stream incrementally
* cancellation interrupts synthesis safely

---

# STEP 3 — Playback Runtime Test

---

## Goal

Verify:

* jitter buffering
* stable playback
* no underruns

---

## Build

Create:

```text
tests/playback_test.rs
```

---

## Test Flow

```text
simulate delayed chunk arrival
 → playback buffer
 → CPAL output
```

---

## Success Criteria

* no audio stutter
* playback remains continuous
* jitter buffer absorbs delays

---

# STEP 4 — Cancellation Integration Test

---

## Goal

Verify:

* realtime interruption
* atomic cancellation propagation

---

## Build

Create:

```text
tests/cancel_test.rs
```

---

## Test Flow

```text
LLM generating
 → speech_start
 → cancel_flag set
 → generation stops
 → playback clears
```

---

## Success Criteria

* interruption latency <200ms
* playback stops immediately
* no thread deadlock

---

# STEP 5 — Audio Output Mode Test

---

## Goal

Verify:

* Speaker mode mic ducking
* Headset mode true barge-in

---

## Build

Create:

```text
tests/audio_mode_test.rs
```

---

## Speaker Mode Success Criteria

* playback_active=true
* mic frames dropped
* VAD never self-triggers

---

## Headset Mode Success Criteria

* VAD remains active
* speech_start triggers cancellation
* true interruption works

---

# STEP 6 — Full Pipeline Integration Test

---

## Goal

Verify complete backend pipeline.

---

## Build

Create:

```text
tests/pipeline_test.rs
```

---

## Test Flow

```text
input.wav
 → STT
 → TranscriptFinal
 → LLM
 → TTS
 → output.wav
```

---

## Success Criteria

* complete pipeline executes
* no blocking
* no crashes
* cancellation remains functional

---

# 14. Threading Recommendations

Example for 8-core CPU:

| Worker   | Threads |
| -------- | ------- |
| Audio    | 1       |
| VAD      | 1       |
| STT      | 2       |
| LLM      | 3-4     |
| TTS      | 1       |
| Playback | 1       |

Avoid:

```text
use all cores
```

This destroys responsiveness.

---

# 15. Explicitly Out Of Scope

DO NOT build in Phase 4:

* orb synchronization
* frontend animation logic
* persistent conversations
* memory systems
* vector DB
* RAG
* speculative LLM execution
* meeting mode
* multi-model routing
* tool calling

---

# 16. Final Success Condition

Phase 4 is complete when:

```text
audio
 → stt
 → llm
 → tts
 → playback
```

works with:

* realtime-safe cancellation
* stable playback
* no deadlocks
* no feedback loops
* independent test coverage
* latency telemetry

under:

* CPU-only execution
* 8GB RAM constraints

---

# 17. Final Architectural Principles

> Vox is NOT a chatbot.

It is a:

```text
native realtime voice runtime
```

The system is constrained by:

* audio hardware timing
* CPU scheduling
* memory bandwidth
* blocking inference physics

Every architectural decision MUST prioritize:

* low latency
* interruption responsiveness
* realtime stability

over:

* maximum intelligence
* giant models
* feature complexity

---

# Final Review Notes

## 🐛 BUG

Blocking C++ inference cannot be interrupted via channels alone.

Fix:
Shared atomic cancellation tokens checked inside inference callbacks. ([GitHub][1])

---

## 🐛 BUG

Speaker playback can recursively trigger VAD.

Fix:
State-aware mic ducking using explicit `audio_output_mode`.

---

## ⚖️ TRADEOFF

Speaker mode sacrifices true barge-in for guaranteed stability.

Headset mode enables true interruption but assumes isolated audio hardware.

---

## 💡 IMPROVEMENT

Playback jitter buffering is mandatory for smooth streaming TTS.

Fix:
Require ~300ms pre-buffer before CPAL playback begins.

[1]: https://github.com/abetlen/llama-cpp-python/issues/599?utm_source=chatgpt.com "Dynamically intterupt token generation · Issue #599"
