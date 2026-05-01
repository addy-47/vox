# Vox — Backend Architecture (Native Inference)

---

## 1. Overview

The Vox backend is a **real-time, event-driven native audio processing system**.

It is built using:

* **Rust (Tauri)** → system orchestration, audio I/O, IPC
* **C++ Inference Layer** → model execution (`onnxruntime`, `llama.cpp`)

---

## 2. Core Design Principles

---

### ⚡ Native-First Execution

All inference MUST run using:

* ONNX Runtime (C++)
* llama.cpp (C++)

Python is **not part of runtime**.

---

### ⚡ Streaming First

The system operates as a continuous stream:

```text
audio → VAD → STT → LLM → TTS → output
```

No stage waits for completion.

---

### ⚡ Event-Driven Architecture

```text
audio_chunk → speech_start → text_delta → llm_token → tts_chunk
```

Each stage emits incremental outputs.

---

### ⚡ Low-Latency Constraint

Target: **<500ms voice-to-voice**

Every component must minimize:

* buffering
* blocking
* memory allocation

---

## 3. System Topology

---

### Architecture

```text
[Tauri (Rust)]
    ├── Audio Capture (cpal)
    ├── Event Bus
    ├── UI IPC
    │
    ↓
[C++ Inference Layer]
    ├── VAD (ONNX)
    ├── STT (ONNX)
    ├── LLM (llama.cpp)
    └── TTS (native)
```

---

## 4. Audio Ingestion (Rust)

---

### Implementation

* library: `cpal`
* format: 16kHz mono PCM
* chunk size: 10–20 ms

---

### Requirements

* zero blocking
* consistent timing
* low jitter

---

### Output

```text
audio_chunk (ring buffer)
```

---

## 5. Data Marshalling (CRITICAL)

---

### ❌ Forbidden

* JSON audio transfer
* WebSocket streaming
* copying buffers

---

### ✅ Required

* **Shared Memory Ring Buffer**
* zero-copy audio transfer

---

### Flow

```text
Rust writes → shared buffer
C++ reads → processes
```

---

### Control Signals

Use lightweight IPC:

* Unix sockets / named pipes
* Rust channels

Events:

```text
speech_start
speech_end
text_delta
llm_token
interrupt
```

---

## 6. Voice Activity Detection (VAD)

---

### Model

* TEN VAD (ONNX)

---

### Behavior

```text
2 frames speech → speech_start
300ms silence → speech_end
```

---

### Key Property

* near-zero endpoint delay

---

## 7. Speech-to-Text (STT)

---

### Model

* Qwen3-ASR-0.6B (INT8 ONNX)

---

### Input Strategy

* 240ms overlapping chunks
* continuous streaming

---

### Output

```text
text_delta (streaming)
text_final
```

---

### Critical Optimization

* cache encoder state
* avoid recomputation

---

## 8. Language Model (LLM)

---

### Runtime

* llama.cpp (via Rust bindings)

---

### Model

* Gemma (current)
* future: Qwen2.5-3B

---

### Constraints

* context limit: 4096 tokens
* quantization: Q4 / INT4
* KV cache capped

---

### Streaming

```text
input → token stream → output
```

---

### Optimization

* speculative prompt feeding from STT stream

---

## 9. Text-to-Speech (TTS)

---

### Model

* Chatterbox-Turbo (~350M)

---

### Behavior

```text
text chunks → audio chunks
```

---

### Requirements

* sub-200ms startup
* streaming synthesis

---

## 10. Audio Output

---

### Implementation

* Rust (cpal)

---

### Behavior

* continuous playback
* interruptible

---

### Barge-In Logic

```text
speech_start →
    cancel LLM
    clear TTS buffer
    switch to listening
```

---

## 11. Concurrency Model

---

### Core Principle

Avoid CPU thrashing.

---

### Execution Strategy

* sequential pipeline with overlap
* controlled thread allocation

---

### Thread Allocation

```text
Total cores = N
LLM threads = N - 2
Remaining:
    - audio thread
    - VAD thread
```

---

## 12. Memory Constraints

---

### Total Budget

```text
~5.5GB max usable
```

---

### Allocation

```text
VAD  ~0.05GB
STT  ~0.80GB
LLM  ~2.20GB
TTS  ~0.50GB
KV   ~0.60GB
```

---

### Rule

Never exceed memory ceiling → prevents OS swap

---

## 13. State Management

### Core Principle

The system is **stateless at the logic level**, but **stateful at the buffer level**.

---

### Stateless (Logic)

* each interaction turn is independent
* no persistent conversation state in core loop

---

### Stateful (Buffers — REQUIRED)

The following must maintain short-term state:

* audio sliding window (for VAD stability)
* partial transcript buffer
* response token buffer

---

## 14. Persistence Boundary

### Principle

The real-time pipeline MUST remain independent of storage.

---

### Rules

* no disk writes in critical path
* no blocking I/O during processing
* only final outputs may be persisted

---

### Storage Types

* config → JSON
* logs → file system
* history → SQLite 

---

### Separation

```text
Real-time pipeline (memory only)
        ↓
Async persistence layer
```

---

### Why This Is Critical

Mixing storage with pipeline will:

* increase latency
* break real-time behavior
* introduce blocking operations


---

## 15. Failure Handling

---

### Must Handle

* model crash
* audio device failure
* inference timeout

---

### Behavior

* fail silently
* restart component
* keep listening active

---

## 16. Final Principle

> Vox backend = **native real-time streaming engine**

NOT:

* Python service
* REST API
* batch system
