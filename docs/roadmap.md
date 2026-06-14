# Vox Roadmap — What's Been Built

> A brief overview of delivered milestones. Detailed plans live in `docs/plans/`.
> This is a log of what shipped, not a forward-looking spec.

## Core Mandate

> Local-first voice system.
>
> Accuracy → Memory → Speed.

---

## v0.1.0 — Foundation

**Goal:** Establish the desktop application foundation.

- Tauri application setup
- Multi-window architecture (main + tray + wizard)
- Main UI shell and overlay/tray groundwork
- Design system and interaction concepts

---

## v0.2.0 — Audio Pipeline

**Goal:** Build the real-time audio ingestion layer.

- Native audio capture (CPAL)
- VAD integration (Earshot)
- Event-driven pipeline
- Speech lifecycle detection
- Frontend ↔ backend IPC

---

## v0.3.0 — Speech Recognition

**Goal:** Introduce real-time transcription.

- Qwen3-ASR integration
- Streaming transcripts (partial + final)
- Tray transcription experience
- Hinglish-focused speech support

---

## v0.4.0 — Voice Intelligence

**Goal:** Complete the first end-to-end voice interaction loop.

- Local LLM runtime (llama.cpp)
- Local TTS runtime
- Voice-to-voice pipeline
- Streaming responses
- Runtime model architecture

---

## v0.5.0 — Interaction System

**Goal:** Make Vox feel like a usable voice assistant.

- Barge-in support
- Interaction state management
- Session orchestration
- Overlay UX improvements
- Full Push-To-Talk mode

---

## v0.6.0 — Language Intelligence

**Goal:** Optimize Vox for Hindi and Hinglish users.

- Custom Qwen3-ASR fine-tuning
- Benchmarking and evaluation pipeline
- Hindi → Hinglish transliteration engine
- Noise robustness improvements
- False-trigger reduction

---

## v0.7.0 — Persistence & Observability

**Goal:** Add configuration, monitoring and system visibility.

- Settings architecture
- Runtime telemetry
- Monitoring dashboard
- Session/history foundation
- Model and runtime controls

---

## v0.8.0 — Distribution & Lifecycle

**Goal:** Prepare Vox for real-world deployment.

- Onboarding experience
- Model download management
- Runtime model lifecycle management
- Packaging and release workflows
- Cross-platform deployment preparation

---

## v0.8.3 — UI Revamp

**Goal:** Turn Vox UI from an AI SaaS app into a voice OS aesthetic.

- Complete UI redesign per `docs/design.md`
- Responsive layout, EdgeNav, GlassCard components
- AmbientBackground, StatusCapsule, PipelineField
- Zustand v5 settings store
- Performance hooks (useDynamicFPS, usePerformanceMonitor)

---

## v0.8.4 — LLM Provider Architecture (Released)

**Goal:** Refactor the LLM from a single embedded backend into a trait-based provider architecture.

- `LlmProvider` trait (generate, stream, cancel, health_check, list_models)
- `EmbeddedProvider` (local GGUF via llama.cpp)
- `OpenAiCompatProvider` with `provider_name` URL remapping
- Cloud providers: OpenAI, Gemini, Anthropic via unified provider
- Pipeline remains unchanged — provider-agnostic

---

## v0.8.5 — UI Polish & System Monitoring (Released)

**Goal:** Complete UI responsiveness, monitoring UX, and system-level fixes.

- Settings page made responsive across window sizes
- Monitoring page offload/reload UX (model lifecycle management)
- Linux system monitor fixed (sub-task filtering)
- General UI polish and interaction card refactors
- CI/CD hardened (Linux, macOS, Windows pipelines green)

---

## v0.8.5 - Present

Active development. Direction is inference expansion: Gemini Live full integration,
S2S frontend integration, PTT for S2S, testing and hardening across all cloud
providers and local modes.

---

## Final State Vision

Vox v1.0 is:

- Local-first -agentic voice OS 
- Model-agnostic
- Event-driven
- Real-time voice native
- Hindi/Hinglish optimized
- Cross-platform
