# AGENTS.md — Vox

Compact guidance for AI agents working in this repository. Every line here is something
you are likely to get wrong without help.

---

## Project Identity

- **Vox**: A real-time, local-first voice assistant for desktop (Tauri v2 + Rust + React).
- **Phase**: `v0.9.0` (in progress) — Realtime S2S engine complete: Gemini Live integration,
  PTT for S2S, web realtime bench, full frontend session lifecycle (engage/pause/resume,
  session cache, idle timeout, client-side VAD gating). Next: OpenAI Realtime, Deepgram
  Voice Agent, ElevenLabs ConvAI provider implementations. OpenAI, Gemini, and Anthropic
  cloud providers are already supported through the unified `OpenAiCompatProvider`.
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
│   │   ├── store/            # Zustand v5 (settings only)
│   │   └── shared/           # Components, hooks, context, lib
│   └── src-tauri/            # Rust backend
│       ├── src/
│       │   ├── core/         # events.rs, settings.rs, state.rs, constants.rs, metrics.rs
│       │   ├── services/     # audio/, vad/, stt/, llm/, tts/, pipeline, realtime/, ptt, translit
│       │   │   ├── audio/    # device.rs, playback.rs, router.rs (unified audio module)
│       │   │   ├── realtime/ # engine.rs, audio_bridge.rs, playback_bridge.rs, resampler.rs
│       │   │   │   └── providers/  # gemini_live.rs (full Gemini Live WebSocket integration)
│       │   │   └── llm/providers/  # embedded.rs, openai_compat.rs (unified cloud hub)
│       │   ├── ipc/          # Tauri command handlers (pipeline, settings, tray, history…)
│       │   ├── persistence/  # SQLite (rusqlite), event store
│       │   ├── monitoring/   # Telemetry aggregator, system monitor
│       │   └── setup/        # First-run model download, wizard state machine
│       └── tests/            # llm_provider_tests.rs, gemini_live_test.rs (mock WS servers)
├── docs/                     # Extensive architecture docs — READ BEFORE MAKING DECISIONS
│   ├── backend.md            # ~1500 lines — threading, events, services, memory
│   ├── frontend.md           # ~900 lines — component tree, state, IPC, design system
│   ├── plans/                # Phase implementation plans (phase9/ dir for inference expansion)
│   └── roadmap.md            # Brief overview of what's been shipped (not a forward plan)
├── .agents/rules/            # Agent instruction files (system-architect, code-style, finetune)
├── manifests/                # App manifest + model manifest (SHA256 checksums)
└── scripts/                  # Python benchmarks, release scripts
```

---

## Exact Commands

All paths are relative to repo root unless noted.

| What | Command |
|------|---------|
| Dev mode | `cd app && pnpm tauri dev` |
| Sandboxed dev | `cd app && pnpm dev:sandbox` (sets `VOX_HOME=./.vox_sandbox`) |
| Frontend only | `cd app && pnpm build` (tsc + vite) |
| Rust tests | `cd app/src-tauri && cargo test` |
| Specific test | `cd app/src-tauri && cargo test --test llm_provider_tests` |
| Realtime test | `cd app/src-tauri && cargo test --test gemini_live_test` |
| Realtime bench | `cd app/src-tauri && cargo run --bin vox_realtime_bench` |
| Lint Rust | `cd app/src-tauri && cargo clippy` |
| Format Rust | `cd app/src-tauri && cargo fmt` |
| Install deps | `cd app && pnpm install` |
| Prod build | `cd app && pnpm tauri build` |

Key: `pnpm` commands run from `app/`, `cargo` commands from `app/src-tauri/`. Never from root.

---

## Testing Quirks

- Backend tests use `vox_lib::` imports (the lib crate is named `vox_lib`, not `Vox`).
- `llm_provider_tests.rs` spawns **real mock HTTP servers** (`TcpListener`) to test the
  `OpenAiCompatProvider`. Tests are fast and self-contained.
- `gemini_live_test.rs` spawns a **mock WebSocket server** to test the Gemini Live provider
  handshake, audio streaming, server message handling, and turn lifecycle. Fast and self-contained.
- The `EmbeddedProvider` test expects a **valid GGUF file path** (or checks for init failure
  on missing files). Not suitable for CI without models downloaded.
- No frontend tests exist yet.

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

New cloud providers (OpenAI, Gemini, Anthropic, Groq, OpenRouter, Sarvam) should be added
as new `impl LlmProvider` structs in `services/llm/providers/`. The trait requires:
`generate()`, `stream_tokens()`, `cancel()`, `health_check()`, `list_models()`.

The pipeline (`services/pipeline.rs`) is **provider-agnostic** — it calls `LlmProvider`
methods only. Do not modify the pipeline when adding a new provider.

### Realtime S2S Engine (v0.9.0, completed)

The `RealtimeVoiceProvider` trait-based engine for cloud speech-to-speech APIs (Gemini Live,
OpenAI Realtime, Deepgram Voice Agent, ElevenLabs ConvAI) is implemented in
`services/realtime/`. It uses a **hybrid sync/async threading model** (tokio tasks for
WebSocket, sync threads for audio capture/playback). **Gemini Live is fully integrated**
with the complete frontend session lifecycle. The remaining three providers have config
structs defined but return "not yet implemented" — see `docs/plans/phase9/` for their
integration plans.

### Backend Threading Model

- Dedicated OS threads for inference (no async for model calls).
- Lock-free communication: ring buffers (audio), atomics (cancellation), mpsc channels (events).
- `global_llama_backend()` singleton in `services/llm/mod.rs` — shared by LlmWorker and
  the TTS engine. `LlamaBackend::init()` is called exactly once per process.
- **Hybrid sync/async for realtime S2S**: tokio tasks for WebSocket I/O, OS threads for
  audio capture and playback, atomic flags for pause/resume coordination.
- `VoxEvent` enum with 15+ variants drives all pipeline coordination.

### Frontend

- **Dual-surface**: Main window (invoked, persistent) + Tray HUD (ephemeral overlay,
  always-on-top, transparent).
- **State**: Zustand v5 for settings (selective subscriptions). No context for hot paths.
- **Performance**: `useDynamicFPS` hook (60/15/0 FPS tiers), `React.memo` on visual components,
  refs over state for audio/transcript data.
- **Three windows** in `tauri.conf.json`: `main` (400×800), `wizard` (900×650), `tray` (420×250).
- **Rust is source of truth** — frontend never derives state from local computations.

---

## Important Constraints

- **CPU-only** — no GPU assumption. All inference via ONNX Runtime + llama.cpp C++ bindings.
- **8GB RAM baseline** — model memory budget ~5.5GB across VAD/STT/LLM/TTS.
- **Accuracy → Memory → Speed** — never optimize speed at the expense of accuracy.
- **Streaming-first** — no stage waits for completion. Barge-in must remain functional.
- **No Python in runtime** — Python is for benchmarks/scripts only.
- **Never silently substitute a model name** — if a user specifies a model, use it.
  If a call fails, check the library/endpoint first, not the model name.
  (This applies to model IDs, not endpoint URLs — the `OpenAiCompatProvider` URL mapping
  is intentional and config-driven.)

---

## Release & Git Workflow

- **Active branch**: `dev` (not `master`).
- **Tag conventions**: `v0.x.x` (production), `v0.x.x-test[n]` (test releases).
- **CI**: GitHub Actions builds `.deb`/`.dmg`/`.exe` on tags matching `v*`. Linux workflow
  also signs and publishes an APT repo to `gh-pages`.
- **Secrets required**: `APT_GPG_PRIVATE_KEY`, `APT_GPG_PASSPHRASE`, `GITHUB_TOKEN`.

---

## Existing Agent Instructions

These files in `.agents/rules/` contain important expanded guidance that this file
summarizes. Read them for depth:

- **`system-architect.md`** — Architecture decision workflow, Vox-specific constraints,
  resource limits, 5-role AI assistant pipeline.
- **`code-style-guide.md`** — Implementation conventions: no `any`, modularity, no hardcoded
  values, pnpm only, security rules, gitignore requirements.
- **`idea-validator.md`** — Pre-build filter for new features (DROP / VALIDATE / REFRAME / EXPLORE).
- **`finetune.md`** — ASR fine-tuning specifics (Hindi/Hinglish, RTX 5070 Ti constraints).

### Critical Bug Reports (Resolved)

| File | Status | Issue |
|------|--------|-------|
| `docs/plans/phase9/realtime-ptt-mode-bug-report.md` | **Fixed** | Order-of-operations bug in `start_realtime_session_internal` causes PTT/Passive mode misassignment |
| `docs/plans/phase9/state-fragmentation-report.md` | **Fixed** | 11 fragmentation points: `state.owner` desync, IPC param mismatches, dual settings write paths, missing VAD sync |

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
