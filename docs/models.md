# Vox — Model Architecture & Selection (Native Edge Stack)

---

## 1. Overview

Vox is a **model-agnostic, role-based system**, but constrained by:

* **8GB RAM baseline**
* **CPU-only execution**
* **<500ms real-time latency**

---

## 2. Core Model Roles

The system is composed of:

1. **VAD** — speech boundary detection
2. **STT** — audio → text
3. **LLM** — reasoning + response
4. **TTS** — text → audio

Each role is replaceable, but must obey strict **memory + latency constraints**.

---

## 3. Core Selection Philosophy

---

### ⚡ Local-First

* Fully offline operation required
* No cloud dependency

---

### ⚡ Latency > Accuracy

* Sub-500ms response is priority
* Slight accuracy tradeoffs acceptable

---

### ⚡ Native Execution Only

All inference must run via:

* `onnxruntime` (C++)
* `llama.cpp` (C++)

Python is not allowed in inference path.

---

### ⚡ Memory-Constrained Design

System must never exceed safe RAM threshold:

* OS + UI ≈ ~2.5GB
* Available for inference ≈ **~5.5GB**

---

## 4. Memory Budget (STRICT)

---

### Absolute Allocation

```text
Memory_Total ≈
  VAD (0.05GB)
+ STT (0.80GB)
+ LLM (2.20GB)
+ TTS (0.50GB)
+ KV Cache (0.60GB)
= 4.15GB
```

---

### Safety Margin

* Remaining buffer ≈ **1.35GB**
* Prevents OS swap → avoids latency collapse

---

### Hard Rules

* No model >3B parameters (unless quantized)
* No FP16 / FP32 inference
* Use INT8 / INT4 quantization only

---

## 5. Voice Activity Detection (VAD)

---

### ✅ Default

* **TEN VAD (ONNX Runtime, C++)**

---

### Why

* ~306KB footprint
* Real-Time Factor: ~0.015
* Near-zero end-of-speech delay
* Frame-level precision

---

### Behavior

```text
speech_start → 2 consecutive positive frames
speech_end → ~300ms silence window
```

---

### Why NOT Silero

* introduces 300–500ms delay
* breaks real-time interaction

---

## 6. Speech-to-Text (STT)

---

### ✅ Default

* **Qwen3-ASR-0.6B (INT8 ONNX)**

---

### Why

* ~0.8GB memory footprint
* ~90–100ms time-to-first-token
* native multilingual + Hinglish support
* streaming-friendly

---

### Key Advantage

Avoids **code-switching collapse**:

```text
Hindi + English mixed speech → handled natively
```

---

### Streaming Strategy (CRITICAL)

* 240ms overlapping chunks
* continuous inference
* encoder state caching

---

### Output

```text
text_delta (stream)
text_final
```

---

### Why NOT Whisper

* fixed 30s window → inefficient
* high latency on CPU

---

### Why NOT Moonshine

* limited Hindi / Hinglish support
* less robust multilingual handling

---

## 7. Language Model (LLM)

---

### ✅ Default (Phase 4)

* **Gemma 4 E2B-it (GGUF, Q2_M)**

### Why
* **State-of-the-Art Reasoning**: Released March 2026, optimized for 2B scale efficiency.
* **Instruction Tuning**: High adherence to system prompts and conversational flow.
* **Multilingual Capability**: Significantly improved Hindi and Hinglish reasoning.
* **Thinking Mode**: Supports internal reasoning blocks for complex queries.

---

### Why (Gemma)

* stable
* lightweight
* good baseline performance

---

### Why Evaluate Qwen2.5

* superior multilingual reasoning
* better Hinglish generation
* optimized for conversational tasks

---

### Runtime

* `llama.cpp` (C++ backend)

---

### Constraints

* context limit: **4096 tokens**
* quantization: **INT4 (Q4_K_M)**
* KV cache capped (~600MB)

---

### Key Risk

```text
Large context → KV cache explosion → RAM overflow → swap death
```

---

### Mitigation

* sliding window context
* aggressive summarization

---

## 8. Text-to-Speech (TTS)

---

### ✅ Default

* **Kokoro-82M (ONNX)**

---

### Why

* **State-of-the-Art Quality**: Human-level prosody and naturalness at only 82M parameters.
* **Incredible Efficiency**: Runs significantly faster than real-time even on mid-range CPUs.
* **Architecture**: Uses a StyleTTS2-inspired architecture with a BERT-based text encoder.
* **Compatibility**: Native ONNX support allows for low-latency C++ integration.

* **Implementation**: Uses `sherpa-onnx` for high-performance C++ inference.
* **Assets**: Requires `model.onnx`, `voices.bin`, `tokens.txt`, and `espeak-ng-data/`.
* **Latency**: sub-200ms time-to-first-audio.
* **CPU-friendly**: Optimized for 8GB RAM systems.
* **Hindi Support**: natively supported via multi-language models.

---

### Architecture

* distilled one-step decoder
* avoids autoregressive latency loops

---

### Output

```text
text stream → audio chunks (real-time)
```

---

### Why NOT Fish Speech (4B)

* exceeds memory bandwidth
* unusable on CPU
* > 5s latency

---

### Why NOT Qwen3-TTS

* no Hindi support
* poor CPU performance

---

### Why NOT Piper

* no voice cloning
* lower realism

---

## 9. Model Lifecycle

---

### First Launch

* models downloaded on-demand
* minimal defaults installed

---

### Runtime

* models loaded lazily
* only active components consume memory

---

### Switching

* configurable via settings
* reload without full restart (where possible)

---

## 10. Streaming Behavior (System-Level)

---

### Full Pipeline

```text
audio → VAD → STT → LLM → TTS → output
```

---

### Parallel Execution

* STT streams → feeds LLM early
* LLM streams → feeds TTS early

---

### Result

* overlapping execution
* perceived latency <500ms

---

## 11. Future Extensions

---

### Multi-Model Routing

* dynamic model switching
* lightweight vs high-quality modes

---

### Hybrid Execution

* local + cloud fallback

---

### Specialized Models

* wake word detection
* speaker recognition
* intent classifiers

---

## 12. Design Constraints

---

### Must Always Support

* offline operation
* low memory footprint
* real-time responsiveness

---

### Must Avoid

* large default models
* Python inference pipelines
* blocking execution

---

## 13. Final Principle

> Models are **bounded by hardware physics**, not just capability.

Vox is defined by:

* its **real-time pipeline**
* its **event-driven architecture**
* its **latency guarantees**

—not by any specific model.
