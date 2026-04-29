# Vox — Roadmap (v1.0.0)

---

## Versioning Philosophy

Vox follows a **capability-driven** semantic versioning approach:

- **0.1.x**: UI Foundation & Design System
- **0.2.x**: Audio & Speech Intelligence (STT/VAD)
- **0.3.x**: Reasoning & Response Loop (LLM/TTS)
- **0.4.x**: Interaction Fidelity (PTT, Interruption, Hotword)
- **0.5.0 → 0.9.9**: **The Hardening Loop** (Bugs, Packaging, Performance, Reliability)
- **1.0.0**: Production Stable Release

---

## Phase 1: Foundation (0.1.0)

**Goal**: Build a premium, reactive UI shell and establish the design system.

- [ ] **Core Layout**: 20:9 dashboard with sidebar navigation.
- [ ] **The Orb**: Integrate `glob.tsx` as the primary interaction center.
- [ ] **Tray Architecture**: Tauri multi-window setup for the "Overlay Tray".
- [ ] **Design Tokens**: Glassmorphism, smooth gradients, and Inter/Outfit typography.
- [ ] **Mock Interaction**: UI transitions for "Listening", "Thinking", and "Speaking" states.

---

## Phase 2: Perception (0.2.0)

**Goal**: Enable the system to hear and transcribe in real-time.

- [ ] **Streaming Microphones**: Low-latency audio ingestion (PyAudio/SoundDevice).
- [ ] **VAD Integration**: Silero VAD for speech start/end detection.
- [ ] **Streaming STT**: Moonshine (local) or Faster-Whisper integration.
- [ ] **Live Transcripts**: Streaming partial transcripts from backend to UI.
- [ ] **Noise Profile**: Basic filtering for ambient noise.

---

## Phase 3: Intelligence (0.3.0)

**Goal**: Close the loop with a local LLM and voice output.

- [ ] **Local LLM**: GGUF/Llama.cpp integration (Llama 3 8B or similar).
- [ ] **Streaming TTS**: Piper (local) for sub-500ms time-to-audio.
- [ ] **Cloud Fallback**: Optional ElevenLabs integration for high-quality voice.
- [ ] **Turn Management**: Orchestrate the full STT → LLM → TTS pipeline.
- [ ] **Context Injection**: Basic session history for "short-term memory".

---

## Phase 4: Fidelity (0.4.0)

**Goal**: Make the interaction feel natural and frictionless.

- [ ] **Push-to-Talk (PTT)**: Global hotkeys and UI triggers.
- [ ] **Interruption Handling**: Instant audio cutoff when user starts speaking.
- [ ] **Continuous Mode**: VAD-driven "Always Listening" (optional toggle).
- [ ] **Wake Word**: "Hey Vox" local detection (Porcupine or similar).
- [ ] **Audio Ducking**: Automatically lower system volume during voice interaction.

---

## Phase 5: The Hardening Loop (0.5.0 → 0.9.9)

**Goal**: Transition from "Project" to "Product".

- [ ] **Bug Squashing**: Full E2E test coverage.
- [ ] **Packaging Pipeline**: Cross-platform installers (MSI, AppImage, DMG).
- [ ] **Model Downloader**: Onboarding flow to download required model weights.
- [ ] **Performance Profile**: Target 8GB RAM baseline with < 20% CPU overhead.
- [ ] **Update System**: Tauri auto-updater integration.
- [ ] **Observability**: Centralized logging for audio, events, and performance.

---

## Phase 6: Release (1.0.0)

**Goal**: A stable, reliable, local-first voice assistant.

- [ ] **Stable APIs**: Finalize IPC protocols between UI and Backend.
- [ ] **Documentation**: Comprehensive user guide and developer docs.
- [ ] **Security Audit**: Verify local-only data privacy guarantees.
- [ ] **Public Beta**: Release to first-wave users.

---

## Final Definition of v1.0.0

Vox is a **real-time, always-available voice system** that:
1. Listens continuously or via hotkey.
2. Responds in < 1 second.
3. Runs fully locally with no mandatory cloud dependency.
4. Provides a reactive, ephemeral UI that stays out of the way.
