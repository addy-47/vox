# Vox — Roadmap (v1.0 Native Architecture)

---

## Versioning Logic

* **0.1.x → 0.4.x** = Core real-time pipeline (native)
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

## Phase 0.3.0 — STT (Streaming Transcription)

**Goal:** Low-latency multilingual transcription

---

### Core Work

* Integrate Qwen3-ASR-0.6B (INT8 ONNX)
* Implement **ring buffer audio pipeline**
* Feed **overlapping 240ms chunks**
* Implement encoder state caching

---

### Streaming Behavior

* emit:

  * `text_delta` (partial transcript)
  * `text_final`

---

### UI Integration

* overlay driven entirely by streaming text
* no buffering delays

---

### Critical Constraint

* no full-audio batching
* no reprocessing of old frames

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

## Phase 0.5.0 — Full Real-Time Loop frontend integration (CRITICAL)

**Goal:** Complete voice-to-voice interaction loop

---

### Core Work

* integrate Chatterbox-Turbo (~350M)
* streaming TTS (text → audio chunks)
* audio playback via Rust (`cpal`)

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

### Target

* <500ms perceived latency
* fully non-blocking pipeline

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

**Goal:** Stability + performance tuning

---

### Core Work

* CPU profiling (thread allocation)
* RAM monitoring (≤5.5GB inference)
* latency tuning (<500ms target)
* crash recovery (inference layer)

---

### Testing

* low-end devices (8GB baseline)
* multi-OS validation

---

## Phase 1.0.0 — Release

**Goal:** Production-ready system

---

### Requirements

* stable IPC contract
* consistent latency
* reliable barge-in behavior
* clean onboarding flow

---

### Final State

Vox v1.0 =

* native real-time voice system
* fully local-first
* event-driven architecture
* ephemeral UI (no persistent chat)
* optimized for constrained hardware

---

## Final Principle

> Vox is not a chatbot.

It is a **real-time streaming voice system** constrained by:

* memory physics
* CPU bandwidth
* latency guarantees
