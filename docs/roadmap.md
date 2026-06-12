# Vox Roadmap

> Detailed phase plans are maintained in `docs/plans/`. Each phase has a dedicated plan file with full implementation details, architectural decisions, and success criteria.

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

## v0.8.3 - Ui revamp from ground up 

**Goal** Turn vox ui from a AI SaaS app to a voice OS with new design.md as guideline

---

## v0.8.4 — LLM Provider Architecture (Current — Phase 9)

**Goal:** Refactor the LLM from a single embedded backend into a provider-based architecture.

* Embeded local inference preserved
* OpenAI-compatible remote endpoint support (Ollama, LM Studio, vLLM, etc.)
* Streaming, cancellation, health checks, and model discovery as provider capabilities
* Pipeline remains unchanged — only the implementation behind the provider changes

**Non-goals (future phases):**
* Cloud provider integration (v0.8.5+)
* STT/TTS provider refactor (v0.8.4/v0.8.5)

---

## v0.8.5 — Cloud LLM Integration

**Goal:** Integrate direct cloud LLM providers into the provider architecture.

* Custom API key configuration interfaces and secure persistence
* Out-of-the-box cloud provider presets: OpenAI, Gemini, Anthropic, Groq, OpenRouter, and Sarvam
* Capability to stream responses and handle barge-in cancellations natively via standard HTTP interfaces

---

## v0.8.6 - 0.9.0 — Cloud Realtime Integration

**Goal:** Introduce full duplex cloud voice-to-voice APIs as an alternative to the local pipeline.

* OpenAI Realtime API (WebSocket-based audio streaming)
* Gemini Multimodal Live API integration
* Dynamic switching between local pipeline (Local STT -> LLM -> TTS) and cloud voice-to-voice
* Adaptive audio buffer synchronization and lower latency packet streaming

### Outcome

* Unified local-first design with cloud acceleration
* Choice of full-local privacy vs. high-capability cloud duplex streams
* User-controlled model configuration and API provisioning

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
