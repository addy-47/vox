# Vox — Roadmap (v1.0 Focused)

---

## Versioning Logic

* **0.1.x → 0.4.x** = Core system build
* **0.5.x → 0.7.x** = Persistence + UX + Packaging
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

## Phase 0.2.0 — Audio + VAD

**Goal:** System can detect speech in real-time

* Python sidecar setup
* Microphone streaming (non-blocking)
* VAD (speech_start / speech_end)
* IPC bridge (Rust ↔ Python ↔ UI)
* Emit events to frontend

---

## Phase 0.3.0 — STT (Transcription Layer)

**Goal:** Real-time transcription pipeline

* Moonshine STT integration
* Streaming partial transcripts
* Final transcript emission
* Overlay UI fully driven by events
* Proper buffering (no flicker)

---

## Phase 0.4.0 — LLM + TTS

**Goal:** Response generation loop

* Local LLM (quantized, CPU)
* Streaming token generation
* TTS (Piper default)
* Audio playback system
* Basic turn orchestration

---

## Phase 0.5.0 — Real-Time Loop (CRITICAL)

**Goal:** True voice interaction system

* Full pipeline:
  audio → STT → LLM → TTS → output

* Barge-in (interrupt system)

* Parallel execution (no blocking)

* Event-driven orchestration

* Stable latency (<1s perceived)

---

## Phase 0.6.0 — Persistence Layer

**Goal:** Add storage (minimal, structured)

* `config.json` (settings)
* log system (rotating files)
* SQLite (history only — optional UI)
* `.vox/` directory structure

⚠️ No storage in core loop
⚠️ Only final outputs stored (not streams)

---

## Phase 0.7.0 — Onboarding (In-App, NOT installer)

**Goal:** First-run experience

* React onboarding flow:

  * welcome
  * model download
  * mic test
  * settings init

* Model downloader

* Permission checks

* System readiness validation

---

## Phase 0.8.0 — Packaging

**Goal:** Shipable app

* PyInstaller backend binary
* Tauri bundling (sidecar)
* Windows `.exe`, Linux builds
* Auto-updater integration

---

## Phase 0.9.0 — Hardening

**Goal:** Make system stable

* CPU + RAM profiling (8GB target)
* Fix latency spikes
* Crash recovery (backend)
* Multi-device testing
* Logging + debugging improvements

---

## Phase 1.0.0 — Release

**Goal:** Production-ready system

* Stable IPC contract
* Clean onboarding
* Reliable real-time loop
* Fully local-first operation

---

## Final Definition

Vox v1.0 =

* Real-time voice system
* Fully local-first
* Event-driven (NOT request-response)
* Ephemeral UI with optional persistence
