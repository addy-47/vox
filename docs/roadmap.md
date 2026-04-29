# Vox — Roadmap (v1.0.0)

---

## Versioning Approach

Follows semantic progression:

```text
0.x → unstable / building core system  
1.0.0 → stable, usable product
```

Each step adds **one core capability**, keeping system testable at all times.

---

## 0.1.0 — UI Foundation

* Main app UI (20:9 layout + fullscreen)
* Orb + animation system
* Basic navigation (Home, Chat)
* Overlay tray UI (static, no backend)
* Tauri multi-window setup

---

## 0.2.0 — Audio + STT Pipeline

* Microphone ingestion (real-time streaming)
* VAD integration (speech start/end detection)
* STT streaming (Moonshine)
* Real-time transcript events
* Tray UI connected to live transcription

---

## 0.3.0 — Core Voice Loop

* LLM integration (local model)
* TTS integration (Piper)
* End-to-end pipeline:

```text
speech → STT → LLM → TTS → playback
```

* Orb reacts to:

  * listening
  * thinking
  * speaking

---

## 0.4.0 — Interaction Modes

* Push-to-Talk (PTT)

  * in tray
  * in main app
* Continuous listening mode
* Interruption handling:

  * speaking stops playback
* Hotword detection ("Hey Vox")

  * excludes tray activation

---

## 0.5.0 — System Integration

* Background service (always running STT)
* Tray lifecycle:

  * show on speech
  * hide on silence
  * disabled when app open
* App auto-launch via hotword
* Logging system (audio + events + responses)

---

## 1.0.0 — Stable Release

* Packaging (cross-platform installers)
* Auto-update system
* Model download on first run
* Performance stabilization (8GB target)
* End-to-end reliability

---

## Final Definition of v1.0.0

Vox becomes:

> A stable, always-available voice system that:
>
> * listens continuously
> * responds in real-time
> * runs fully locally
> * requires zero manual setup after install

---
