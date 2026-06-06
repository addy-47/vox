# Vox Roadmap

## Core Mandate

> Local-first voice system.
>
> Accuracy → Memory → Speed.

---

## v0.1.0 — Foundation

**Goal:** Establish the desktop application foundation.

* Tauri application setup
* Multi-window architecture
* Main UI shell
* Overlay/tray groundwork
* Design system and interaction concepts

---

## v0.2.0 — Audio Pipeline

**Goal:** Build the real-time audio ingestion layer.

* Native audio capture
* VAD integration
* Event-driven pipeline
* Speech lifecycle detection
* Frontend ↔ backend communication

---

## v0.3.0 — Speech Recognition

**Goal:** Introduce real-time transcription.

* Qwen3-ASR integration
* Streaming transcripts
* Partial and final transcript flow
* Tray transcription experience
* Hinglish-focused speech support

---

## v0.4.0 — Voice Intelligence

**Goal:** Complete the first end-to-end voice interaction loop.

* Local LLM runtime
* Local TTS runtime
* Voice-to-voice pipeline
* Streaming responses
* Runtime model architecture definition

---

## v0.5.0 — Interaction System

**Goal:** Make Vox feel like a usable voice assistant.

* Barge-in support
* Interaction state management
* Session orchestration
* Overlay UX improvements
* Full Push-To-Talk mode

---

## v0.6.0 — Language Intelligence

**Goal:** Optimize Vox for Hindi and Hinglish users.

* Custom Qwen3-ASR fine-tuning
* Benchmarking and evaluation pipeline
* Hindi → Hinglish transliteration engine
* Noise robustness improvements
* False-trigger reduction

---

## v0.7.0 — Persistence & Observability

**Goal:** Add configuration, monitoring and system visibility.

* Settings architecture
* Runtime telemetry
* Monitoring dashboard
* Session/history foundation
* Model and runtime controls

---

## v0.8.0 — Distribution & Lifecycle

**Goal:** Prepare Vox for real-world deployment.

* Onboarding experience
* Model download management
* Runtime model lifecycle management
* Packaging and release workflows
* Cross-platform deployment preparation

---

## v0.9.0 — Model & Provider Ecosystem

**Goal:** Make Vox fully model-agnostic.

### Local Providers

* Multiple STT engines
* Multiple LLM runtimes
* Multiple TTS engines
* Runtime model switching

### Cloud Providers

* Gemini
* OpenAI
* ElevenLabs
* Additional external providers

### Outcome

* Unified provider abstraction
* Local and cloud interoperability
* User-selectable model stack

---

## v1.0.0 — Stabilization & Release

**Goal:** Production-ready release.

* End-to-end testing
* Fresh install validation
* Windows testing
* Linux testing
* macOS testing
* Performance validation
* Bug fixing and hardening
* Release readiness review

---

## Final State

Vox v1.0 is:

* Local-first
* Model-agnostic
* Event-driven
* Real-time voice native
* Hindi/Hinglish optimized
* Cross-platform
* Built for 8GB-class hardware
