# AGENTS.md — Vox Repository Guide

> Compact instructions for AI agents working in this codebase. Every line answers: "Would an agent likely miss this without help?"

## Agent Rules Files

Role-specific instruction files in `.agents/rules/`. Load the relevant one based on your task:

| File | When to use |
|------|-------------|
| `.agents/rules/system-architect.md` | Architectural decisions, implementation plans, pipeline changes |
| `.agents/rules/code-style-guide.md` | All coding — modularity, API design, security, Docker, CLI |
| `.agents/rules/finetune.md` | ASR/LLM fine-tuning, corpus engineering, dataset curation |

---

## Architecture (Non-Obvious Facts)

**Vox is a Tauri 2 desktop app with a Rust core library (`vox_lib`).**

- `app/` — Frontend workspace (React 19, Vite 7, TailwindCSS 4, TypeScript)
- `app/src-tauri/` — Tauri backend. The lib target is `vox_lib` (not `app`). The `vox_lib` crate contains ALL core logic.
- `app/src-tauri/src/services/` — Engine subsystems: `vad/`, `stt/`, `llm/`, `tts/`, `translit.rs`, `audio.rs`, `pipeline.rs`
- `app/src-tauri/src/services/traits.rs` — Engine trait contracts (`VadEngine`, `SttEngine`, `LlmEngine`, `TtsEngine`). Pure sync interfaces; no thread/Tauri awareness.
- `app/src-tauri/src/bin/` — Standalone binaries: `tts-bench.rs`, `vox-bench.rs`, `test-translit.rs`
- `manifests/` — Model validation manifests (`app_manifest.json`, `models_manifest.json`).
- `scripts/` — Benchmark scripts (Python) and release/packaging scripts.
- `vox-models/` — Local model storage directory.

**Two separate Cargo workspaces** with independent `Cargo.lock` files:
1. `app/src-tauri/` (main app)
2. `app/src-tauri/plugins/tauri-plugin-positioner/`

---

## Documentation (`docs/`)

Architecture and design documents that provide deep context. Read the relevant one before making major changes:

| File | Covers |
|------|--------|
| `docs/vox.md` | Core project definition, system goals |
| `docs/backend.md` | Rust/Tauri audio pipeline, engine lifecycle, event flow |
| `docs/frontend.md` | Dual-surface UI (main app + ephemeral overlay/tray), IPC contracts |
| `docs/models.md` | Model stack (VAD, STT, LLM, TTS), hardware constraints, defaults |
| `docs/design.md` | Visual design system, color tokens |
| `docs/packaging.md` | Native desktop distribution strategy |
| `docs/roadmap.md` | Phased versioned roadmap |
| `docs/decision-framework.md` | Rationale behind major architectural decisions |
| `docs/benchmarks/` | Performance results and TTS comparison reports |

---

## Development Commands

### Rust checks (from app/src-tauri)
```bash
cd app/src-tauri
cargo check
```
### Run a single integration test
```bash
cd app/src-tauri
cargo test --test llm_family_test -- --nocapture
cargo test --test tts_test -- --ignored --nocapture --test-threads=1
```

### TTS benchmark (Rust)
```bash
cd app/src-tauri
cargo run --bin tts-bench
```
Output WAVs go to `docs/benchmarks/audio_outputs/`.

---

## Critical Build Quirks

- **Linux requires clang** for the Tauri build (llama.cpp compilation). Env vars `CC=clang CXX=clang++` are set in CI and may be needed locally.
- **Linux system deps** (must be installed): `libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev patchelf libasound2-dev`
- **`app/src-tauri/.cargo/config.toml`** forces `-lc++ -lc++abi` linker flags on Linux and `FORCE:MULTIPLE` on Windows. Required for llama.cpp symbol resolution.

---

## Testing Conventions

- Integration tests in `app/src-tauri/tests/` often need **real model files** at `~/.vox/models/`. Many are `#[ignore]`-d and run with `--ignored`.
- Tests requiring actual audio hardware or models should use `--test-threads=1` to avoid resource contention.
- `vox-bench` is the full production-parity benchmark (STT + LLM + TTS + VAD pipeline). `tts-bench` is TTS-only.





## Common Pitfalls

- **Forgetting `--test-threads=1`** on tests that load models or use audio hardware → race conditions or OOM
- **Running `cargo test` from repo root** — there's no root workspace. Always `cd` into `app/src-tauri`.
- **Hardcoded absolute paths** in code — use `dirs` crate or `vox_lib::utils::paths` instead
- **Modifying streaming/latency/VAD behavior** without reading `.agents/rules/system-architect.md` first — these are critical-path invariants

---

## Post-Task Protocol

After completing any task, an agent should:

1. **Update `AGENTS.md`** if the task revealed new build quirks, conventions, or pitfalls not already documented here.
2. **Update `docs/`** if the task changed architecture, pipeline behavior, model stack, or frontend contracts. Keep the relevant doc in sync:
   - Backend/pipeline changes → `docs/backend.md`
   - Frontend/UI changes → `docs/frontend.md`
   - Model changes → `docs/models.md`
   - New architectural decisions → `docs/decision-framework.md`
