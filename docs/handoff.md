# Handoff — Chatterbox TTS Integration (Phase 2)

## What's Done

### chatterbox-rs Crate (Phase 1 — Complete)
- Safe Rust FFI wrapper around `chatterbox.cpp` C++ TTS engine
- Vendored C++ source in `chatterbox-cpp/`, cmake builds `libtts-cpp.a` + GGML static libs
- `Engine::new()`, `synthesize()`, `synthesize_streaming()`, `cancel()`
- Axum HTTP/WS server (feature-gated)
- 7 integration tests passing (run with `--test-threads=1` — see below)
- GitHub: `github.com/addy-47/chatterbox-rs.git` (commit `20ce6e1`)

### Vox Integration (Phase 2 — Code Complete)
- `chatterbox-rs` added as path dependency (`Cargo.toml`)
- `ChatterboxEngine` implementing `TtsProvider` (`services/tts/providers/chatterbox.rs`)
- Registered in `TtsProviderKind` + `TtsProviderConfig` (`settings.rs`)
- `warm_up_tts()` match arm in `pipeline.rs`
- `--tts chatterbox` flag in `vox-bench.rs`
- `cargo check --workspace` passes, `cargo build --bin vox-bench` succeeds

### Fixes Applied (commit `20ce6e1`)
1. **CUDA feature flag**: Added `cuda` Cargo feature (default on), Vox uses `default-features = false` to avoid CUDA linker errors (`-fPIC` incompatibility with LLD)
2. **S3Gen cache lifetime**: Removed `s3gen_unload()` from `~Impl()` — the atexit handler cleans up at process exit. Prevents use-after-free when Engine instances overlap.
3. **Thread safety**: Global `ENGINE_INIT_MUTEX` in Rust wrapper serialises `new()` / `synthesize()` / `drop()` — the C++ global S3Gen cache is not safe for concurrent access.
4. **Build cache tracking**: Added `rerun-if-changed` for chatterbox-cpp `src/` and `include/` dirs.

## What to Test (Gate 2)

Run on your local machine (has STT + LLM models):

```bash
# Default bench (supertonic) — verify no regression
cd /opt/vox/vox/app/src-tauri
cargo run --release --bin vox-bench -- \
  --input test-clips/short_en.wav \
  --output supertonic_ref

# Chatterbox bench
cargo run --release --bin vox-bench -- \
  --tts chatterbox \
  --asr nemotron \
  --input test-clips/short_en.wav \
  --output chatterbox_bench
```

Note: In this container, STT (sherpa-onnx) crashes with `free(): invalid pointer` during `SherpaOnnxCreateOfflineRecognizer` — pre-existing onnxruntime static linking issue. Use `--test-threads=1` for chatterbox-rs crate tests.

## Key Constraints
- **CPU-only**: `n_gpu_layers: 0` (Vox constraint)
- **Language fixed at construction**: Engine recreation would be needed per turn (too expensive for v0)
- **Speed via linear interpolation**: Time-stretches output PCM (no Rubato dep)
- **Voice always built-in**: `reference_audio` / `voice_dir` left empty; `voice` setting ignored
- **Default provider**: Supertonic (Chatterbox is opt-in)

## File Inventory

| File | Purpose |
|------|---------|
| `services/tts/providers/chatterbox.rs` | `ChatterboxEngine` — `TtsProvider` impl |
| `services/tts/providers/mod.rs` | `TtsProviderKind::Chatterbox` variant |
| `services/tts/mod.rs` | Re-exports `ChatterboxEngine` |
| `core/settings.rs` | `TtsProviderConfig::Chatterbox { language, quality_steps, speed }` |
| `services/pipeline.rs` | `warm_up_tts()` match arm |
| `ipc/settings.rs` | Model-exists check for Chatterbox |
| `bin/vox-bench.rs` | `--tts` flag |
| `Cargo.toml` | Path dep `chatterbox-rs` |
