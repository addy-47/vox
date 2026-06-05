# Vox — Roadmap (v1.0 Native Architecture)

---

## Core Mandate

> **Accuracy First. Memory Second. Speed Third.**
>
> The system is useless if it cannot transcribe or respond accurately.
> Speed is a byproduct of good engineering — not a target to optimize at the cost of correctness.

---

## Versioning Logic

* **0.1.x → 0.4.x** = Core pipeline (native, accuracy-focused)
* **0.5.x → 0.7.x** = System intelligence + UX + persistence
* **1.0.0** = Stable release

---

## Phase 0.1.0 — Frontend (DONE)

**Goal:** UI shell + interaction surfaces

* Main UI (20:9 layout)
* Overlay UI (ephemeral tray)
* Orb + animations
* Multi-window Tauri setup
* Mock states (listening / thinking / speaking)

---

## Phase 0.2.0 — Native Audio + VAD

**Goal:** Real-time speech detection using native stack

---

### Core Work

* Rust audio capture using `cpal`
* 16kHz mono PCM stream (10–20ms chunks)
* Integrate TEN VAD via ONNX (`ort` / C++ binding)
* Real-time speech detection:

  * `speech_start`
  * `speech_end`

---

### IPC & Events

* Rust event bus (no JSON streaming)
* Emit to frontend:

  * `speech_start`
  * `speech_end`
  * `audio_level`

---

### Output

```text
audio → VAD → event stream → UI reacts
```

---

## Phase 0.3.0 — STT (Accurate Transcription)

**Goal:** Accurate multilingual transcription — Hinglish, English, Hindi

---

### Core Work

* Integrate Qwen3-ASR-0.6B (INT8 ONNX)
* Implement **ring buffer audio pipeline**
* Feed **overlapping audio chunks**
* Ensure `max_new_tokens` is large enough to never truncate output

---

### Accuracy Constraints

* `max_new_tokens` must be ≥ 256 (512 preferred) — truncated transcripts are bugs
* Partial transcripts are UI feedback only — final transcript must be complete
* The warning `"Result is truncated"` from sherpa-onnx is a hard failure, not acceptable

---

### Streaming Behavior

* emit:

  * `text_delta` (partial transcript — for live UI feedback)
  * `text_final` (authoritative — must be complete)

---

### UI Integration

* overlay driven entirely by streaming text
* final transcript always wins over partial

---

### Critical Constraint

* no full-audio batching
* no reprocessing of old frames
* never truncate to hit a latency target

---

## Phase 0.4.0 — Runtime infernce LLM + TTS 

**Goal:** Real-time reasoning with minimal latency

---

### Core Work

* integrate Gemma via `llama.cpp` (Rust binding)
* quantized GGUF (INT4)
* enforce:

  * `ctx-size = 4096`
  * KV cache limits

---

### Streaming Behavior

* consume `text_delta` from STT
* speculative prompt feeding (pre-fill context early)
* emit:

  * `llm_token`
  * `response_final`

---

### Key Optimization

* LLM begins before STT completes

---

## Phase 0.5.0 — Full Voice Loop frontend integration (CRITICAL)

**Goal:** Complete, coherent voice-to-voice interaction loop

---

### Core Work

* integrate Chatterbox-Turbo (~350M)
* streaming TTS (text → audio chunks)
* audio playback via Rust (`cpal`)

---

### TTS Quality Mandate

Chunking must produce **natural, complete utterances**:

* Flush on sentence boundaries (`.`, `!`, `?`)
* Flush on clause boundaries (`,`, `;`, ` — `)
* Time-based flush: ≥ 1500ms AND ≥ 3 words
* Word count fallback: ≥ 8 words
* **Never flush on 1–2 words** — produces robotic, choppy speech

---

### Barge-In System (MANDATORY)

```text
speech_start →
    cancel LLM
    clear TTS buffer
    switch to listening
```

---

### Final Pipeline

```text
audio → VAD → STT → LLM → TTS → output
```

---

### Quality Target

* Transcription: complete and accurate — no truncation
* LLM response: coherent and complete
* TTS: natural sentence-level utterances — not choppy word fragments
* Pipeline: fully non-blocking

---

## Phase 0.6.0 — Persistence Layer

**Goal:** Add structured storage (outside core loop)

---

### Storage Design

* `config.json` → settings
* logs → rotating files
* SQLite → optional history

---

### Constraints

* no storage in real-time path
* only final outputs persisted

---

### Directory

```text
~/.vox/
  ├── config.json
  ├── logs/
  └── sessions/ (optional)
```

---

## Phase 0.7.0 — Onboarding (In-App)

**Goal:** First-run system setup

---

### Flow

* welcome screen
* model download manager
* microphone test
* system readiness check

---

### Notes

* built in React (NOT installer)
* handles model downloads dynamically

---

## Phase 0.8.0 — Packaging (Native)

**Goal:** Shipable application

---

### Build System

* Tauri bundling (Rust + UI)
* C++ inference binaries included
* ONNX models external (downloaded)

---

### Outputs

* Windows → `.exe`
* Linux → `.AppImage`, `.deb`
* macOS → `.dmg` (future)

---

### Constraints

* no Python runtime
* no external dependencies

---

## Phase 0.9.0 — Hardening

**Goal:** Stability + accuracy validation + performance tuning

---

### Core Work

* CPU profiling (thread allocation)
* RAM monitoring (≤5.5GB inference)
* Accuracy validation: WER testing on Hinglish corpus
* TTS quality review: naturalness and completeness of utterances
* crash recovery (inference layer)

---

### Accuracy KPIs (not latency KPIs)

* Zero `"Result is truncated"` warnings in normal usage
* STT: Hinglish WER ≤ 20% on representative samples
* TTS: No utterances < 3 words unless a sentence boundary is present
* LLM: Responses are grammatically complete (no mid-sentence cutoff)

---

### Testing

* low-end devices (8GB baseline)
* multi-OS validation
* real-world Hinglish speech samples

---

## Phase 1.0.0 — Release

**Goal:** Production-ready system

---

### Requirements

* stable IPC contract
* accurate STT with zero truncation in normal usage
* coherent LLM responses
* natural TTS output (no choppy utterances)
* reliable barge-in behavior
* clean onboarding flow

---

### Final State

Vox v1.0 =

* native, accuracy-first voice system
* fully local-first
* event-driven architecture
* ephemeral UI (no persistent chat)
* optimized for constrained hardware

---

## Final Principle

> Vox is not a chatbot.

It is a **local-first, accuracy-driven voice system** constrained by:

* memory physics
* CPU bandwidth
* the mandate to be correct before being fast
