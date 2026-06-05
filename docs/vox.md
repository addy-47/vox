# Vox — Project Definition

---

## 1. What is Vox?

**Vox** is a **local-first, accuracy-driven voice intelligence system** designed to function as a persistent, reliable personal assistant.

It is not a chatbot, nor a traditional voice assistant.

> Vox is a **continuous listening system** that reacts to speech, understands it accurately, processes intent, and responds naturally — while remaining lightweight enough to run on everyday devices.

---

## 2. Core Philosophy

---

### 🎯 Accuracy First — Always

Vox prioritizes:

* correct transcription of what was actually said
* meaningful, complete responses
* natural, coherent speech output

Over:

* raw speed of response
* minimizing latency metrics
* throughput optimization

**A fast wrong answer is worse than a correct answer that takes a moment longer.**

Speed is a byproduct of a well-engineered system — not a design target to be optimized at the cost of quality.

---

### 🧠 Memory Second

Vox prioritizes:

* contextual awareness across a session
* accurate recall of prior turns
* coherent multi-turn conversations

Over:

* stateless single-turn processing
* minimizing memory footprint at the cost of context

---

### ⚡ Speed Third

Speed matters — but only after accuracy and memory are satisfied. The system should be as fast as it can be **while being accurate**. Do not tune parameters to hit a millisecond target at the cost of output quality.

Removed forever:
- `< 500ms voice-to-voice` as a hard constraint
- Any config that truncates transcription or responses to hit a latency target

---

### 🏠 Local-First Intelligence

* Runs fully offline by default
* No dependency on cloud services
* User data remains on-device

Cloud integrations (LLMs, STT, TTS) are **optional extensions**, not core dependencies.

---

### 🌊 Ephemeral Interaction Model

Vox is designed around **transient interactions**:

* User speaks → system reacts
* Output is contextual
* UI appears only when needed and disappears after

There is no persistent conversational UI in the core loop.

---

### 🔇 Minimal Cognitive Load

The system must:

* require zero setup after onboarding
* avoid manual triggering whenever possible
* feel ambient rather than intrusive

---

### 🔧 Modular Intelligence

Vox is not tied to:

* a specific model
* a specific provider
* a fixed pipeline

Every component is replaceable.

---

## 3. Core System Loop

At its core, Vox operates on a continuous voice loop:

```text
audio input → speech detection → transcription → reasoning → response → audio output
```

This follows the standard voice agent pipeline:

* Speech-to-Text (STT)
* Language Model (LLM)
* Text-to-Speech (TTS)

Each stage operates in a **streaming manner**, with accuracy as the primary constraint and latency as a secondary optimization.

---

## 4. Interaction Model

---

### Passive Listening

* System listens continuously (or via hotword)
* Detects speech using VAD (Voice Activity Detection)

---

### Turn-Based Processing

* A "turn" starts when speech begins
* A "turn" ends after silence threshold

Each turn is:

* independent
* short-lived
* context-aware (but not permanently stored in UI)

---

### Feedback Mechanism

* Real-time transcription during speech
* System response after turn ends (complete, accurate response is prioritized)

---

### UI Philosophy

The UI is **reactive, not primary**:

* Appears only during interaction
* Provides minimal feedback (transcription, status)
* Disappears when interaction ends

Design system reference: 

---

## 5. System Characteristics

---

### Accuracy-Driven System

* Transcription must be complete and correct — truncated results are bugs, not tradeoffs
* LLM responses must be coherent and complete — partial responses are not acceptable
* TTS output must be natural and fluid — choppy 1–2 word utterances are failures

---

### Resource-Constrained Design

Target environment:

* 8 GB RAM (baseline)
* CPU-first execution
* No GPU dependency

Every decision must consider:

* memory footprint
* CPU usage
* background overhead

Resource constraints shape **how** the system is built. They do not override the accuracy mandate.

---

### Always-Available

* Runs in background
* Minimal system impact
* Can be invoked instantly

---

### Multi-Language Support

* Must support:

  * English
  * Hindi
  * Hinglish (code-switching)

---

## 6. Extensibility Vision

Vox is designed as a **foundation system**, not a single feature product.

Future capabilities include (non-exhaustive):

---

### Advanced Interaction Modes

* real-time voice-to-voice systems
* conversational agents
* interruption-aware dialogue

---

### Memory & Context

* vector-based memory
* semantic recall
* contextual awareness across sessions

---

### Agentic Capabilities

* tool calling
* task automation
* multi-step reasoning flows

---

### Ambient Intelligence

* meeting assistance (transcription + replies)
* system-level integrations
* passive context awareness

---

### Multi-Provider Support

* local models (default)
* optional cloud integrations:

  * LLM APIs
  * STT services
  * TTS providers

---

## 7. Design Principles

---

### 1. Accuracy Above All

* Every feature must produce correct, reliable output
* Truncated transcripts, partial responses, and choppy TTS are all bugs

---

### 2. Simplicity First

* Every feature must reduce friction
* Avoid complex UI and flows

---

### 3. Replaceability

* Any model or component should be swappable
* No hard dependencies on specific tools

---

### 4. Incremental Capability

* System evolves by adding modules
* Core loop remains stable

---

### 5. Privacy by Default

* No data leaves device unless explicitly configured

---

### 6. Coherence Over Responsiveness

* Complete, meaningful answers > fast, broken answers

---

## 8. Project Structure (Documentation)

This file is the **root context**.

Other components are defined in dedicated documents:

* `frontend.md` → UI architecture and interaction surfaces : [UI Architecture and Interaction Surfaces](/docs/frontend.md)
* `backend.md` → pipeline, services, and system orchestration : [Pipeline, Services, and System Orchestration](/docs/backend.md)
* `models.md` → model selection, configurations, providers : [Model Selection, Configurations, and Providers](/docs/models.md)
* `ft-*.md` → feature-specific designs (e.g., meeting mode)

---

## 9. What Vox is NOT

To avoid drift:

* Not a chatbot interface
* Not a dashboard-driven system
* Not cloud-dependent
* Not a speed-first system — never sacrifice accuracy for latency
* Not UI-first

---

## 10. Final Definition

> Vox is a **local-first, accuracy-driven voice system** that operates as an ambient layer over the user's device —
> continuously listening, understanding accurately, and assisting meaningfully.

---
