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

**Vox is a Tauri 2 desktop app with a Rust core library (`vox_lib`) and a standalone `neutts` crate.**

- `app/` — Frontend workspace (React 19, Vite 7, TailwindCSS 4, TypeScript)
- `app/src-tauri/` — Tauri backend. The lib target is `vox_lib` (not `app`). The `vox_lib` crate contains ALL core logic.
- `app/src-tauri/src/services/` — Engine subsystems: `vad/`, `stt/`, `llm/`, `tts/`, `translit.rs`, `audio.rs`, `pipeline.rs`
- `app/src-tauri/src/services/traits.rs` — Engine trait contracts (`VadEngine`, `SttEngine`, `LlmEngine`, `TtsEngine`). Pure sync interfaces; no thread/Tauri awareness.
- `app/src-tauri/src/bin/` — Standalone binaries: `tts-bench.rs`, `vox-bench.rs`, `test-translit.rs`, `test-neutts.py`
- `neutts/` — Standalone Rust crate: NeuTTS Nano port (GGUF backbone + NeuCodec decoder). Path dependency of `vox_lib`.
- `neutts/src/codec.rs` — NeuCodec decoder (FSQ, Vocos, ISTFT). Active site for audio quality parity work.
- `neutts/src/codec_burn.rs` — Burn wgpu/NdArray backend for NeuCodec (compiled only with `wgpu` feature).
- `manifests/` — Model validation manifests (`app_manifest.json`, `models_manifest.json`).
- `scripts/` — Benchmark scripts (Python) and release/packaging scripts.
- `vox-models/` — Local model storage directory.

**Three separate Cargo workspaces** with independent `Cargo.lock` files:
1. `app/src-tauri/` (main app)
2. `neutts/` (standalone crate)
3. `app/src-tauri/plugins/tauri-plugin-positioner/`

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

### neutts crate checks
```bash
cd neutts
cargo test --lib --tests --no-default-features --features fast
```

### TTS benchmark (Rust)
```bash
cd app/src-tauri
cargo run --bin tts-bench
```
Output WAVs go to `docs/benchmarks/audio_outputs/`.

### TTS reference comparison (Python)
```bash
cd app/src-tauri/src/bin
source ~/projects/apps/vox/venv/bin/activate
python test-neutts.py
```
Requires: `torch`, `numpy`, `soundfile`, and the `neutts` Python package in the venv.

---

## Critical Build Quirks

- **Linux requires clang** for the Tauri build (llama.cpp compilation). Env vars `CC=clang CXX=clang++` are set in CI and may be needed locally.
- **Linux system deps** (must be installed): `libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev patchelf libasound2-dev`
- **`app/src-tauri/.cargo/config.toml`** forces `-lc++ -lc++abi` linker flags on Linux and `FORCE:MULTIPLE` on Windows. Required for llama.cpp symbol resolution.
- **`neutts/.cargo/config.toml`** sets `MACOSX_DEPLOYMENT_TARGET=13.4` and cmake equivalent. After changing these, delete stale cmake cache: `rm -rf target/release/build/llama-cpp-sys-2-*/`.
- **neutts features**: `backbone` (default, GGUF via llama-cpp-4), `espeak` (phonemizer, needs `libloading`), `wgpu` (GPU codec via Burn), `fast` (default, approximate RoPE), `precise` (exact RoPE). The `vox_lib` dependency uses `features = ["espeak"]`.
- **neutts `auto*` = false** in Cargo.toml — tests/examples/benches are explicitly declared, not auto-discovered.
- **The `neutts` crate is NOT a workspace member** of the app. It's a path dependency (`../../neutts`). Changes in `neutts/` require rebuilding `app/src-tauri` to pick up.

---

## Testing Conventions

- Integration tests in `app/src-tauri/tests/` often need **real model files** at `~/.vox/models/`. Many are `#[ignore]`-d and run with `--ignored`.
- Tests requiring actual audio hardware or models should use `--test-threads=1` to avoid resource contention.
- `vox-bench` is the full production-parity benchmark (STT + LLM + TTS + VAD pipeline). `tts-bench` is TTS-only.
- Python `test-neutts.py` is the reference implementation for NeuTTS audio quality comparison.

---

## NeuTTS Audio Quality Parity (Active Work)

The Rust NeuTTS decoder (`neutts/src/codec.rs`) has been corrected for FSQ coordinates, IFFT phase leakage, and ISTFT envelope alignment. A residual clarity gap remains where Rust output sounds slightly muffled vs Python.

**Key files:**
- `neutts/src/codec.rs` — Decoder (FSQ, Vocos, ISTFT, window functions)
- `neutts/src/codec_burn.rs` — Burn backend decoder
- `app/src-tauri/src/bin/tts-bench.rs` — Rust benchmark (saves WAVs to `docs/benchmarks/audio_outputs/`)
- `app/src-tauri/src/bin/test-neutts.py` — Python reference
- `neutts/src/model.rs` — NeuTTS model orchestration

**Known fixes applied:**
1. FSQ coordinate decoding: `levels / 2` (integer) not `levels / 2.0` (float)
2. Complex-to-complex IFFT: force DC + Nyquist imaginary bins to 0.0
3. ISTFT envelope: same-pad trim alignment with PyTorch `(n_fft - hop) / 2`

**Remaining investigation areas:**
- Phoneme/token alignment: compare `tts._infer_ggml` output between Python and Rust
- Spectrogram normalization: compare log-magnitudes before ISTFT
- Hann window: verify periodic vs symmetric generation matches PyTorch's `torch.hann_window`

---


## Common Pitfalls

- **Forgetting `--test-threads=1`** on tests that load models or use audio hardware → race conditions or OOM
- **Editing `neutts/src/` without rebuilding the app** — path dependency won't hot-reload
- **Running `cargo test` from repo root** — there's no root workspace. Always `cd` into `app/src-tauri` or `neutts/`.
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
   - NeuTTS codec work → update the "Known fixes" and "Remaining investigation" lists above