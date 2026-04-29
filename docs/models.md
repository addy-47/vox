# Vox — Model Architecture & Selection

---

## 1. Overview

Vox is a **model-agnostic system**.

It does not depend on any single:

* model
* framework
* provider

Instead, it defines **roles** that can be fulfilled by interchangeable models.

---

## 2. Core Model Roles

The system is built around three primary model types:

---

### 1. Speech-to-Text (STT)

Converts live audio → text

---

### 2. Language Model (LLM)

Processes text → generates response / actions

---

### 3. Text-to-Speech (TTS)

Converts response → audio output

---

## 3. Model Selection Philosophy

---

### ⚡ Local-First

* All core functionality must work **offline**
* Default models must run on:

  * CPU
  * 8GB RAM systems

---

### ⚡ Latency > Accuracy

* Real-time responsiveness is prioritized
* Slightly lower accuracy is acceptable if:

  * latency is significantly improved

---

### ⚡ Tiered Model Strategy

Each model role supports:

1. **Default (lightweight, always-on)**
2. **Upgrade (higher quality, optional)**
3. **External (future integrations)**

---

## 4. Speech-to-Text (STT)

---

### ✅ Default

* **Moonshine (tiny / smallest variant)**

Why:

* Designed specifically for **real-time streaming speech recognition**
* Optimized for **edge devices and low compute environments**
* Provides **continuous partial transcription while user is speaking** ([LinkedIn][1])
* Up to **5× faster than Whisper on short audio segments** ([arXiv][2])
* Extremely small models (~27M parameters) suitable for constrained systems ([Hugging Face][3])

---

### 🔼 Upgrade Options

* Moonshine (base / medium)
* faster-whisper (base / small)

Use when:

* better accuracy is required
* system has more CPU headroom

---

### 🌐 External (Future)

* Cloud STT providers
* Streaming APIs

Examples:

* Google Speech-to-Text
* Whisper API
* Other real-time transcription services

---

## 5. Language Model (LLM)

---

### ✅ Default

* **Small quantized local LLM (e.g., Gemma / similar class)**

Characteristics:

* ~2–4 GB footprint
* CPU inference via GGUF
* Fast response for short prompts

---

### 🔼 Upgrade Options

* Larger quantized models (Q3 / Q4)
* 7B class local models

Use when:

* better reasoning is needed
* device has more RAM (16GB+)

---

### 🌐 External (Future)

* API-based LLMs
* Live streaming LLMs

Examples:

* Gemini API
* OpenAI API
* other hosted providers

---

## 6. Text-to-Speech (TTS)

---

### ✅ Default

* **Piper TTS**

Why:

* extremely lightweight
* fast inference
* runs fully offline

---

### 🔼 Upgrade Options

* Higher-quality local voices
* Larger neural TTS models

---

### 🎤 Voice Cloning Mode

* **XTTS-v2 (session-based)**

Usage:

* used only when user explicitly enables “personal voice mode”
* requires reference audio

---

### ⚠️ Important Constraint

* XTTS embeddings cannot be transferred to Piper
* Hybrid approach:

  * XTTS used for cloning sessions
  * Piper used for default fast responses

---

### 🌐 External (Future)

* High-quality cloud TTS providers

Examples:

* ElevenLabs
* other neural voice APIs

---

## 7. Model Lifecycle

---

### First Launch

* Default lightweight models are downloaded automatically
* System becomes functional immediately

---

### Runtime Behavior

* Models are loaded dynamically based on:

  * user settings
  * system capability

---

### Switching Models

* User can upgrade/downgrade models in settings
* No system restart required (where possible)

---

## 8. Resource Strategy

---

### Memory Targets

| Component | Target  |
| --------- | ------- |
| STT       | < 200MB |
| LLM       | 2–4 GB  |
| TTS       | < 200MB |

---

### CPU Usage

* Must remain low enough for:

  * background execution
  * multitasking

---

### GPU

* Not required
* Optional acceleration only

---

## 9. Future Extensions

---

### Multi-Model Routing

* Dynamically select models based on:

  * task complexity
  * latency requirements

---

### Hybrid Execution

* Local + cloud fallback
* Example:

  * local STT → cloud LLM → local TTS

---

### Streaming Models

* token streaming LLMs
* real-time voice-to-voice pipelines

---

### Specialized Models

* intent classifiers
* wake word detection
* speaker recognition

---

## 10. Design Constraints

---

### Must Always Support

* fully offline operation
* real-time interaction
* low memory usage

---

### Must Avoid

* large default models
* blocking inference
* cloud dependency

---

## 11. Final Principle

> Models are **replaceable components**, not the system itself.

Vox is defined by:

* its architecture
* its interaction model
* its real-time behavior

—not by any specific model choice.

---

[1]: https://www.linkedin.com/posts/petewarden_github-moonshine-aimoonshine-fast-and-activity-7428109120882393089-YneW?utm_source=chatgpt.com "Introducing Moonshine Voice: Open Source Speech-to-Text Models"
[2]: https://arxiv.org/abs/2410.15608?utm_source=chatgpt.com "Moonshine: Speech Recognition for Live Transcription and Voice Commands"
[3]: https://huggingface.co/UsefulSensors/moonshine?utm_source=chatgpt.com "UsefulSensors/moonshine - Hugging Face"
