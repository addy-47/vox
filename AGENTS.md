# AGENTS.md — Vox Repository Guide

> Compact instructions for AI agents working in this codebase. Every line answers: "Would an agent likely miss this without help?"

---

## Architecture (Non-Obvious Facts)

**Vox is a Tauri 2 desktop app with a Rust core library (`vox_lib`).**

- `app/` — Frontend workspace (React 19, Vite 7, TailwindCSS 4, TypeScript)
- `app/src-tauri/` — Tauri backend. The lib target is `vox_lib` (not `app`). The `vox_lib` crate contains ALL core logic.
- `app/src-tauri/src/services/` — Engine subsystems: `vad/`, `stt/`, `llm/`, `tts/`, `translit.rs`, `audio.rs`, `pipeline.rs`
- `app/src-tauri/src/services/tts/` — TTS engine. `supertonic.rs` is the sole TTS engine. Uses sherpa-onnx native `OfflineTtsSupertonicModelConfig` (99M params, 31 languages, INT8 quantized ~144MB). Progress callback must capture owned Arcs/Senders (`'static`). `actor.rs` dispatches TTS commands. `mod.rs` re-exports.
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
cargo run --bin tts-bench bench
```
Output WAVs go to `docs/benchmarks/audio_outputs/`.

### Supertonic (sole TTS engine, INT8 via sherpa-onnx native)
7 INT8 model files at `~/.vox/models/tts/supertonic-3/` (flat, no subdirectories). Uses sherpa-onnx `OfflineTtsSupertonicModelConfig` with `GenerationConfig { sid, num_steps: i32, speed, extra: { "lang" } }`. Progress callback resamples 44.1→24kHz. Model pack: `sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2`. Expression tags (`<laugh>`, `<breath>`, `<sigh>`) injected into LLM system prompt when engine is Supertonic (see `pipeline.rs` dynamic prompt logic).

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
- **Benchmark audio clips** are at `data/benchmark_clips/` as `hiacc_adult_test_AD09XXX.wav`. The 5-file benchmark suite uses clips `AD09001`, `AD09004`, `AD09021`, `AD09039`, `AD09051` (approx 5-16s each, Hindi multi-lingual). Run with: `cargo run --release --bin vox-bench -- --input /home/addy/projects/apps/vox/data/benchmark_clips/hiacc_adult_test_AD09001.wav --llm minicpm/minicpm5-1b-Q4_K_M.gguf --asr nemotron` from `app/src-tauri/`. Always use absolute paths for `--input`.


## Common Pitfalls

- **Forgetting `--test-threads=1`** on tests that load models or use audio hardware → race conditions or OOM
- **Running `cargo test` from repo root** — there's no root workspace. Always `cd` into `app/src-tauri`.
- **Hardcoded absolute paths** in code — use `dirs` crate or `vox_lib::utils::paths` instead
- **Modifying streaming/latency/VAD behavior** without reading `.agents/rules/system-architect.md` first — these are critical-path invariants
- **`ort::session::Session` API quirks (2.0.0-rc.12):** `Session` is at `ort::session::Session`, not `ort::Session`. Builder methods return `ort::Error<SessionBuilder>` which is `!Send` — always use `.map_err(|e| anyhow!("{:?}", e))?` instead of bare `?`. Access input/output info via `session.inputs()` / `session.outputs()` methods (not fields). `GraphOptimizationLevel` is at `ort::session::builder::GraphOptimizationLevel`.
- **Adding new fields to settings structs:** Always add `#[serde(default)]` to the struct to avoid deserialization failures when loading old settings files missing the new field.
- **sherpa-onnx Supertonic native API quirks:** `OfflineTtsSupertonicModelConfig` has 7 fields: `duration_predictor`, `text_encoder`, `vector_estimator`, `vocoder`, `tts_json`, `unicode_indexer`, `voice_style`. `GenerationConfig::num_steps` is `i32` (cast `quality_steps as i32`). Progress callback is `FnMut(&[f32], f32) -> bool + 'static` — must capture owned Arcs/Senders, not references.
- **CPU-aware LLM thread presets:** ModelSettings.tsx computes LLM thread presets from `navigator.hardwareConcurrency`. Max safe = totalCores − 2 (reserving cores for system + other pipeline stages). Presets are generated dynamically: [2, 4] always, plus `maxSafe` and `totalCores` when they differ. Always guard with `typeof navigator !== 'undefined'` for SSR/SSG compatibility. Do NOT hardcode thread options.
- **CPU governor detection (Linux):** `utils::check_cpu_governor()` reads `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` at startup. If not `"performance"`, emits `cpu_governor_warning` Tauri event. Frontend (`Home.tsx`) listens and shows a dismissible warning banner. On non-Linux it's a no-op.

---

## Post-Task Protocol

After completing any task, an agent should:

1. **Update `AGENTS.md`** if the task revealed new build quirks, conventions, or pitfalls not already documented here.
2. **Update `docs/`** if the task changed architecture, pipeline behavior, model stack, or frontend contracts. Keep the relevant doc in sync:
   - Backend/pipeline changes → `docs/backend.md`
   - Frontend/UI changes → `docs/frontend.md`
   - Model changes → `docs/models.md`
   - New architectural decisions → `docs/decision-framework.md`
