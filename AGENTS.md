# AGENTS.md — Vox

Compact guidance for AI agents working in this repository. Every line here is something
you are likely to get wrong without help.

---

## Project Identity

- **Vox**: A real-time, local-first voice assistant for desktop (Tauri v2 + Rust + React).
- **Phase**: `v0.9.0` (in progress) — Realtime S2S engine: Gemini Live ✅ and Deepgram
  Voice Agent ✅ are fully implemented (WebSocket, auto-reconnect, full frontend session
  lifecycle). PTT for S2S, web realtime bench, engage/pause/resume, session cache, idle
  timeout, client-side VAD gating all complete. Remaining: OpenAI Realtime, ElevenLabs
  ConvAI provider implementations. OpenAI, Gemini, and Anthropic cloud providers are
  already supported through the unified `OpenAiCompatProvider`.
- **Core mandate**: Local-first, CPU-only (~8GB RAM), sub-500ms pipeline, streaming-first.

---

## Repository Layout

```
.
├── app/                      # Tauri workspace — ALL pnpm/cargo commands run from here
│   ├── src/                  # React/TypeScript frontend (Vite)
│   │   ├── pages/            # Home, History, Settings, Monitoring
│   │   ├── tray/             # Ephemeral overlay UI (Tray HUD)
│   │   ├── wizard/           # First-run setup wizard (XState-driven)
│   │   ├── layout/           # ResponsiveLayout, EdgeNav, TitleBar
│   │   ├── store/            # Zustand v5 (settings only)
│   │   └── shared/           # Components, hooks, context, lib
│   └── src-tauri/            # Rust backend
│       ├── src/
│       │   ├── core/         # events.rs, settings.rs, state.rs, constants.rs, metrics.rs
│       │   ├── services/     # audio/, vad/, stt/, llm/, tts/, pipeline, realtime/, ptt, translit
│       │   │   ├── audio/    # device.rs, playback.rs, router.rs, decode.rs (unified audio module)
│       │   │   ├── vad/      # earshot_vad.rs (default), ten_onnx.rs (legacy), actor.rs
│       │   │   ├── stt/      # nemotron_onnx.rs, qwen_onnx.rs, actor.rs, providers/ (SttProvider trait)
│       │   │   ├── realtime/ # engine.rs, audio_bridge.rs, playback_bridge.rs, resampler.rs
│       │   │   │   └── providers/  # gemini_live.rs, deepgram_live.rs (full WS integration)
│       │   │   ├── tts/providers/ # TtsProvider trait + supertonic.rs, chatterbox.rs, chatterbox_remote.rs
│       │   │   └── llm/providers/ # embedded.rs, openai_compat.rs (unified cloud hub)
│       │   ├── ipc/          # Tauri command handlers (pipeline, settings, tray, history, voices, audio, setup…)
│       │   ├── persistence/  # SQLite (rusqlite), db.rs, schema.rs, events.rs, voices.rs, worker.rs
│       │   ├── monitoring/   # aggregator, collector, snapshot, runtime_state, system_monitor, telemetry_emitter
│       │   ├── setup/        # manifest.rs, model_manager.rs, runtime_check.rs, update_check.rs
│       │   ├── utils/        # paths.rs, logging.rs, audio_filters.rs, bench_reporter.rs
│       │   └── bin/          # vox-bench, tts-bench, vox_realtime_bench, voice_clone, etc.
│       └── tests/            # llm_provider_tests.rs, gemini_live_test.rs, realtime_echo_test.rs
├── chatterbox-rs/            # External TTS engine (submodule)
├── vox-models/               # Model storage directory
├── docs/                     # Extensive architecture docs — READ BEFORE MAKING DECISIONS
│   ├── backend.md            # ~1800 lines — threading, events, services, memory
│   ├── frontend.md           # ~1000 lines — component tree, state, IPC, design system
│   ├── models.md             # Model architecture, selection, footprints
│   ├── features/             # Feature-specific docs (ptt, voice-flow, transcription-tray…)
│   ├── plans/                # Phase implementation plans (phase9/ dir for inference expansion)
│   └── roadmap.md            # Brief overview of what's been shipped (not a forward plan)
├── .agents/rules/            # Agent instruction files (system-architect, code-style, finetune)
├── manifests/                # App manifest + model manifest (SHA256 checksums)
└── scripts/                  # Python benchmarks, release scripts
```

---

## Current Architecture — Critical Context

### LLM Provider Architecture (v0.8.5, released)

The LLM was refactored from a single embedded backend into a **trait-based provider system**:

```
LlmProvider trait:
  └─ EmbeddedProvider          (local GGUF via llama.cpp)
  └─ OpenAiCompatProvider      (handles ALL remote/cloud)
       ├─ OpenAI-compatible servers (Ollama, LM Studio, vLLM)
       ├─ OpenAI cloud          (provider_name: "openai")
       ├─ Gemini cloud          (provider_name: "gemini")
       └─ Anthropic cloud       (provider_name: "anthropic")
```

New cloud providers (OpenAI, Gemini, Anthropic, Groq, OpenRouter, Sarvam) should normally
be handled through the `OpenAiCompatProvider` (which supports any OpenAI-compatible
endpoint). Only add a new `impl LlmProvider` struct if the provider uses a fundamentally
different protocol (non-OpenAI-compatible). The trait requires: `generate()`,
`health_check()`, `list_models()`, `kind()`.

The pipeline (`services/pipeline.rs`) is **provider-agnostic** — it calls `LlmProvider`
methods only. Do not modify the pipeline when adding a new provider.

### TTS Provider Architecture (v0.8.5+, refactored)

The TTS was refactored from a single Supertonic-only engine into a **trait-based
provider system** mirroring the LLM pattern:

```
TtsProvider trait:                          (services/tts/providers/mod.rs)
  └─ TtsEngine (providers/supertonic.rs)    (local Supertonic 3 via sherpa-onnx)
  └─ ChatterboxEngine (providers/chatterbox.rs)  (local GGML, zero-shot voice cloning)
  └─ ChatterboxRemoteProvider (providers/chatterbox_remote.rs) (offload to GPU server)
```

The trait uses `&self` (interior mutability via `Mutex`/atomics) instead of `&mut self`
for thread safety. `voice_sid` was removed from the command protocol — voice is now
provider-config level (restart to change). Methods: `kind()`, `health_check()`,
`set_quality_steps()`, `set_speed()`. All providers must output **24 kHz f32 mono**.

**Settings**: `TtsProviderConfig` is a tagged enum (like `LlmProviderConfig`) in
`core/settings.rs`. Variants: `Supertonic`, `Chatterbox`, `ChatterboxRemote`.
Switch requires TTS worker restart (`SettingReloadPolicy::Restart`).

The pipeline (`services/pipeline.rs`) `warm_up_tts()` matches on
`TtsProviderConfig` to construct the correct provider — do NOT hardcode any
provider in the pipeline. Add new providers as new `impl TtsProvider` structs
in `services/tts/providers/`.

**Worker model**: `spawn_tts_worker()` in `services/tts/actor.rs` now accepts
`Box<dyn TtsProvider>` instead of hard-coding Supertonic. The worker owns the
provider exclusively on its dedicated OS thread.

### Trait Organization (v0.8.5+, cleanup)

All engine traits live in their owning module's `mod.rs`, **not** a centralized `traits.rs`:

```
VadEngine  → services/vad/mod.rs     (also: VadBackend enum dispatch for zero-cost audio path)
SttEngine  → services/stt/mod.rs
LlmEngine  → services/llm/mod.rs     (coexists with LlmProvider trait — lower-level GGUF trait)
TtsProvider → services/tts/providers/mod.rs  (provider trait, not an "engine")
```

The former `services/traits.rs` was dissolved. There is no `pub mod traits` in
`services/mod.rs`. Use local super-path imports (`super::VadEngine`, `super::super::LlmEngine`)
in implementation files to avoid name collisions with identically-named structs.

### Realtime S2S Engine (v0.8.5+, completed)

The `RealtimeVoiceProvider` trait-based engine for cloud speech-to-speech APIs (Gemini Live,
OpenAI Realtime, Deepgram Voice Agent, ElevenLabs ConvAI) is implemented in
`services/realtime/`. It uses a **hybrid sync/async threading model** (tokio tasks for
WebSocket, sync threads for audio capture/playback). **Gemini Live is fully integrated**
with the complete frontend session lifecycle. **Deepgram Voice Agent** is also fully
implemented (WebSocket, auto-reconnect, keepalive, ~657 lines). The remaining two
providers (OpenAI Realtime, ElevenLabs ConvAI) have config structs defined but are
not yet implemented — see `docs/plans/phase9/` for their integration plans.

### Backend Threading Model

- Dedicated OS threads for inference (no async for model calls).
- Lock-free communication: ring buffers (audio), atomics (cancellation), mpsc channels (events).
- `global_llama_backend()` singleton in `services/llm/mod.rs` — shared by LlmWorker and
  the TTS engine. `LlamaBackend::init()` is called exactly once per process.
- **Hybrid sync/async for realtime S2S**: tokio tasks for WebSocket I/O, OS threads for
  audio capture and playback, atomic flags for pause/resume coordination.
- `VoxEvent` enum with 16 variants drives all pipeline coordination.

### Frontend

- **Dual-surface**: Main window (invoked, persistent) + Tray HUD (ephemeral overlay,
  always-on-top, transparent).
- **State**: Zustand v5 for settings (selective subscriptions). No context for hot paths.
- **Performance**: `useDynamicFPS` hook (60/15/0 FPS tiers), `React.memo` on visual components,
  refs over state for audio/transcript data.
- **Three windows** in `tauri.conf.json`: `main` (400×800), `wizard` (900×650), `tray` (420×250).
- **Responsive layout**: `ResponsiveLayout`, `EdgeNav`, `TitleBar` in `layout/` directory.
- **Rust is source of truth** — frontend never derives state from local computations.

---

## Important Constraints

- **CPU-only** — no GPU assumption. All inference via ONNX Runtime + llama.cpp C++ bindings.
- **8GB RAM baseline** — model memory budget ~5.5GB across VAD/STT/LLM/TTS.
- **Accuracy → Memory → Speed** — never optimize speed at the expense of accuracy.
- **Streaming-first** — no stage waits for completion. Barge-in must remain functional.

---

## Dogfooding Rule

Update this file after any major task or phase. `docs/` should also be kept in sync with
architecture changes. The `docs/plans/` directory contains authoritative phase implementation
plans — consult them before making architectural decisions. Phase plans are modular: each
provider or subsystem gets its own file within a phase directory (e.g. `docs/plans/phase9/`).

---

## Things You Will Likely Do Wrong

- Running `pnpm` or `cargo` from the repo root instead of from `app/` or `app/src-tauri/`.
- Forgetting that `app/src-tauri/src/main.rs` just calls `vox_lib::run()` — the real entry
  point is in `lib.rs`.
- Adding new dependencies without approval (requires explicit sign-off).
- Using `npm` instead of `pnpm` for frontend operations.
- Assuming GPU availability for inference.
- Silently substituting a user-specified model name when an API call fails.
  (This applies to model IDs, not endpoint URLs — the `OpenAiCompatProvider` URL mapping
  is intentional and config-driven.)
- Modifying the pipeline when adding a new provider (don't — implement the trait instead).
