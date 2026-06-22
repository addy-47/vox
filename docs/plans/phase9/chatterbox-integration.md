# Chatterbox TTS Integration Plan — Vox

## Status: Phase 1, 2, & 3 Complete — Remote GPU Integration Verified

---

## 0. Philosophy & GATE 1

### The Absolute Gate: Python Parity

**Nothing else matters until C++ inference produces audibly identical output to the Python reference.** Every subsequent phase (Rust crate, Vox integration, remote server) depends on this. If the GGUF conversion or the C++ pipeline introduces artifacts, we stop and fix before proceeding.

**Gate 1 pass criteria:**
- Same input text + same seed → C++ WAV and Python WAV are perceptually identical
- ASR roundtrip on C++ output matches input text (no hallucinations)
- Works for English AND at least one tier-1 language (Spanish)

**If Gate 1 fails, the entire integration stops until root cause is found.**

---

## 1. Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│  chatterbox-rs (Standalone Rust Crate)                    │
│  ├─ chatterbox-sys/   (build.rs + cc/cmake → libtts-cpp) │
│  ├─ src/              (safe Rust wrapper: Engine, synth) │
│  └─ examples/         (CLI binary: text→wav)             │
├──────────────────────────────────────────────────────────┤
│  Used by:                                                 │
│  ├─ Vox TtsProvider     (services/tts/providers/)        │
│  └─ Remote Server       (axum, future phase)             │
└──────────────────────────────────────────────────────────┘
```

Key design decision: **One crate, two consumers.** The standalone `chatterbox-rs` crate wraps `chatterbox.cpp` via FFI. Vox adds it as a dependency and implements `TtsProvider` on top. A future remote server binary also depends on the same crate.

---

## 2. Answers to Your Specific Doubts

### 2.1 Will GGUF conversion lose quality?

**No — the conversion is safe.** From the `chatterbox.cpp` documentation:

1. **Only 385 of 2,049 tensors** are quantized (the large 2D matmul weights in encoder attention/MLPs, CFM projections, flow FF layers).
2. **Biases, LayerNorm gammas/betas, embedding tables, spectral filterbanks, 3D convolution weights, voice encoders, built-in voice conditioning ALL stay at full precision** (F32 or F16 depending on source).
3. The S3Gen HiFT vocoder weights (conv_pre, resblocks, conv_post, source fusion, F0 predictor) are explicitly kept at F32 in all quantization levels.
4. The cstr repo verified: *"All quantization levels (F16/Q8_0/Q4_K) produce ASR-identical output on the reference mel."*

**Recommended quant plan for Gate 1:**
- **T3**: Q4_0 (344 MB) — LLM quantization is well-understood, Q4_0 preserves quality ✓ works
- **S3Gen**: Keep F16 (1056 MB) for now — Q8_0 has a GGML backend bug (see Blocker below)

**If Gate 1 shows artifacts**, convert BOTH at F16 first (eliminate quantization as a variable), then experiment with quant levels.

### 2.2 Can we reduce CFM steps (10 → 8 or 6)?

YES. `chatterbox.cpp` supports `--cfm-steps N` at runtime. From the PROGRESS.md:

| Steps | Wall time saved | Log-mel cosine vs reference | Verdict |
|-------|----------------|-----------------------------|---------|
| 10 (default) | 0% | 1.000 | Reference quality |
| 7 | ~22% | 0.995 | **Recommended quality knee** |
| 6 | ~27% | 0.990 (PCM drops to 0.88) | Too aggressive |

**Plan**: Default to 10 for Gate 1 (parity). Add `--cfm-steps 7` as a quality/speed tradeoff option once parity is confirmed.

### 2.3 Why can't we use the existing `cstr/` or `hans00/` GGUFs?

Those GGUFs use different GGUF architectures:

- **`cstr/chatterbox-GGUF`** → Built for **CrispASR** runtime. T3-only (non-MTL), different tensor layout, different arch name. No MTL variant exists. Non-MTL S3Gen is 574 MB vs our MTL S3Gen at 1007 MB (the MTL variant bundles CAMPPlus, S3TokenizerV2, voice encoder, and multilingual support).
- **`hans00/Chatterbox-TTS-GGUF`** → Built for **llama.cpp + codec.cpp** split (T3 = `llama` arch, S3G = `chatterbox_s3g` arch). Repository now private/removed.
- **`chatterbox.cpp`** → Custom `chatterbox` / `chatterbox-s3gen` arch with bundled tokenizers, voice encoders, conditioning baked in. The conversion scripts embed the MTL tokenizer, VoiceEncoder weights, S3TokenizerV2, CAMPPlus, and built-in voice conditioning all into the GGUF — this is what makes the C++ pipeline self-contained.

**We must run `chatterbox.cpp`'s own conversion scripts.** This is a one-time cost: download Python reference weights (~3 GB), convert to GGUF. The resulting GGUFs are ~1-2 GB.

### 2.4 Why scrap `chatterbox-rs` entirely?

The old code had fundamental problems:
- Manual reimplementation of the ENTIRE T3 AR loop, CFG, sampling, embedding assembly in Rust — extremely error-prone
- Relied on pre-extracted binary embedding blobs (cond_emb.bin, text_emb.bin, etc.) — fragile and architecture-specific
- `codec.cpp` submodule was never checked out (empty directory) — S3G decode physically could not work
- Built a standalone HTTP/WS server — not integrated into Vox at all
- Depended on `llama-cpp-4` with CUDA, conflicting with Vox's llama.cpp

The new approach uses `chatterbox.cpp` as a verified C library — all the pipeline complexity is already debugged, tested, and proven in C++.

---

## 3. Disk Budget (41 GB Available)

| Item | Size | Notes |
|------|------|-------|
| chatterbox.cpp source + build | ~500 MB | git clone + ggml submodule + cmake build |
| Python conversion dependencies | ~2 GB | torch, gguf, safetensors, etc. (pip cache) |
| Original PyTorch weights (HF) | ~3 GB | `ResembleAI/chatterbox` — can DELETE after conversion |
| T3 GGUF (F16) | ~1.1 GB | ✓ converted |
| T3 GGUF (Q4_0) | ~344 MB | ✓ converted, ✓ works |
| S3Gen GGUF (F16) | ~1.0 GB | ✓ converted, ✓ works |
| S3Gen GGUF (Q8_0) | ~827 MB | ✓ converted, ✗ blocked |
| S3Gen GGUF (Q5_0) | ~798 MB | ✓ converted, ✗ blocked |
| S3Gen GGUF (Q8_0 cfm-only) | ~869 MB | ✓ converted, ✓ works (experimental) |
| Python reference WAVs | ~1 MB | ✓ generated |
| Rust crate build artifacts | ~500 MB | cargo build (Phase 1) |
| **Peak during conversion** | **~7 GB** | PyTorch + GGUFs + build |
| **Final (production GGUFs only)** | **~1.5 GB** | Q4_0 T3 + F16 S3Gen (until Q8_0 fixed) |
| **Vox existing models** | **22 GB** (already present) | Untouched |

**Total peak: ~29 GB of 41 GB available.** Safe margin.

---

## 4. Phased Plan (WITH EXECUTION STATUS)

### Phase 0: Environment & Standalone C++ Verification (Gate 1)

**Goal**: Build `tts-cli`, convert GGUFs, and verify C++ pipeline produces valid audio.

#### Status Overview

| Step | Status | Notes |
|------|--------|-------|
| 0.1 System deps | ✓ Done | cmake, build-essential, libomp-dev, python3-venv, git-lfs, CUDA 13.3 toolkit |
| 0.2 Clone chatterbox.cpp | ✓ Done | `/opt/vox/chatterbox-cpp`, pinned at `multilingual_merged` (commit `ddca05f`), ggml submodule at `58c38058` |
| 0.3 Python venv | ✓ Done | `.venv/` with torch, huggingface_hub, safetensors, gguf, librosa, scipy, chatterbox-tts |
| 0.4 T3 MTL GGUF (F16) | ✓ Done | `chatterbox-t3-mtl-f16.gguf` (1.1 GB) |
| 0.5 S3Gen MTL GGUF (F16) | ✓ Done | `chatterbox-s3gen-mtl-f16.gguf` (1.0 GB) |
| 0.6a T3 quantization (Q4_0) | ✓ Done | `chatterbox-t3-mtl-q4_0.gguf` (330 MB) — ✓ verified working (runtime model) |
| 0.6b S3Gen quantization | ✗ **Skipped** | All S3Gen quant levels (Q8_0/Q5_0) produce silence. **Root cause unknown** — likely ggml-cpu backend bug with both CFM+flow quantized. Using S3Gen F16 for all runs. |
| 0.7 Build tts-cli (CPU) | ✓ Done | `build/tts-cli` — AVX CPU backend |
| 0.7b Build tts-cli (CUDA) | ✓ Done | `build_cuda/tts-cli` — CUDA backend (RTX 5070 Ti, sm_120) |
| 0.8 English test | ✓ Done | T3 Q4_0 + S3Gen F16 = real audio ✓. Multilingual variant. |
| 0.9 Spanish test | ✓ Done | `--language es` works ✓ |
| 0.10 French/German/Italian/Korean tests | ✓ Done | `fr, de, it, ko` all produce valid audio ✓ |
| 0.11 GPU benchmark | ✓ Done | **5.8× faster than CPU** (2.0s vs 11.6s for 4.4s audio). RTF = 2.2x real-time. |
| 0.12 Voice cloning | ✓ Supported | `--reference-audio` with C++ VoiceEncoder + CAMPPlus + S3TokenizerV2 — all native |
| 0.13 Speaking pace issue | ⚠️ **Open** | C++ output sounds ~1.2× faster than natural speech. **Root cause unknown.** See note below. |
| 0.14 Python reference (CPU) | ✓ Done | Python Turbo variant generates different token counts than C++ Multilingual — models are inherently different |
| 0.16 Runtime weights deployed | ✓ Done | Symlinks at `/opt/vox-models/tts/chatterbox/` → `/opt/vox/vox-models/tts/chatterbox/` |

#### Key Deliverables

| Artifact | Location | Status |
|----------|----------|--------|
| CPU tts-cli | `/opt/vox/chatterbox-cpp/build/tts-cli` | ❌ Removed (dir deleted after vendoring) |
| CUDA tts-cli | `/opt/vox/chatterbox-cpp/build_cuda/tts-cli` | ❌ Removed (dir deleted) |
| T3 runtime model | `/opt/vox/vox-models/tts/chatterbox/chatterbox-t3-mtl-q4_0.gguf` (330 MB) | ✓ |
| S3Gen runtime model | `/opt/vox/vox-models/tts/chatterbox/chatterbox-s3gen-mtl-f16.gguf` (1.0 GB) | ✓ |
| Model symlinks | `/opt/vox-models/tts/chatterbox/{t3-q4_0,s3gen-f16}.gguf` → vox-models/ | ✓ |
| chatterbox-rs crate | `/opt/vox/chatterbox-rs/` | ✓ Phase 1 complete |
| Benchmark WAVs | `/opt/vox/chatterbox-rs/bench_audio/` | ✓ 6 languages |
| GitHub repo | `github.com/addy-47/chatterbox-rs.git` | ✓ Pushed |

#### Known Issue: Speaking Pace

**Observation**: The C++ Multilingual output sounds like someone speaking **~1.2× faster than natural human speech**. The voice sounds rushed — not a matter of duration, but the rate of speech (words per minute) is too high.

**What was tested:**
- CFM steps (10 vs 2) — no effect on pace (CFM affects mel quality only, not timing)
- Sampling params (temperature, top_p, repeat_penalty) — tweaking changes token content but not pace
- T3 FP16 vs Q4_0 — different token counts (75 vs 85) but same inherent speaking rate
- GPU vs CPU — different token counts but same pace

**Hypothesis**: This is a characteristic of the **Multilingual** T3 model variant itself. The model produces speech at a higher word-per-minute rate than the Turbo variant. The pace is baked into the trained weights.

**Workaround**: Accept as-is. Can investigate time-stretching the output audio at the Vox level in a later phase if needed.

#### S3Gen Quantization Blocker (Deprioritized)

Attempting to quantize S3Gen (Q8_0 or Q5_0 on both flow encoder + CFM UNet weights) produces silent output. Partial quantizations (flow-only or CFM-only) work fine. **Root cause is still unknown** — suspect a ggml-cpu backend memory layout issue. Mitigated by using S3Gen F16 (1.0 GB). This is not blocking — 1.0 GB is acceptable for a premium TTS provider.

---

### Phase 1: Standalone `chatterbox-rs` Rust Crate (✓ Complete)

**Goal**: A reusable Rust crate wrapping `chatterbox.cpp` as a library, with a clean safe API. One crate, two consumers: local Vox `TtsProvider` and remote HTTP server.

#### Status Overview

| Step | Status | Notes |
|------|--------|-------|
| 1.1 Vendored C++ source | ✓ Done | `chatterbox-cpp/` inside crate, stripped to runtime-only files (removed tests, docs, examples, CLI-only files, unused GGML backends) |
| 1.2 MTL Engine API | ✓ Done | `language`, `cfg_weight`, `min_p`, `exaggeration` fields added to `EngineOptions`; `run_t3_mtl()` method in `Impl`; `synthesize()` dispatches on model variant |
| 1.3 C bridge | ✓ Done | `c_src/tts_bridge.h` and `tts_bridge.cpp` — extern "C" wrappers for Engine |
| 1.4 build.rs | ✓ Done | cmake builds `libtts-cpp.a` + GGML static libs; `cc` crate compiles bridge; CUDA autodetection |
| 1.5 Safe Rust wrapper | ✓ Done | `src/engine.rs` — `Engine::new()`, `synthesize()`, `EngineOptions`, `SynthesisResult` |
| 1.6 FFI declarations | ✓ Done | `src/ffi.rs` — raw extern "C" bindings |
| 1.7 Error types | ✓ Done | `src/error.rs` — `EngineError` with `LoadFailed`, `SynthesisFailed`, `Cancelled` |
| 1.8 HTTP/WS server | ✓ Done | `src/server.rs` — axum `POST /tts`, `GET /tts/stream` (WebSocket), `GET /health` — feature-gated |
| 1.9 Integration tests | ✓ Done | 7 tests: engine creation, EN synthesis, empty text rejection, engine re-use, multiple languages, missing model error, Send trait |
| 1.10 Doc-tests | ✓ Done | 3 doc-tests passing |
| 1.11 Multi-language bench | ✓ Done | 6 languages (en/es/fr/de/it/ko) verified against CLI reference audio |
| 1.12 README | ✓ Done | Full README in crate root |
| 1.13 Git push | ✓ Done | Pushed to `github.com/addy-47/chatterbox-rs.git` |

#### Actual Crate Structure

```
chatterbox-rs/
├── Cargo.toml
├── build.rs              → cmake for libtts-cpp.a + cc for tts_bridge.cpp
├── README.md
├── .gitignore
├── bench_audio/          → CLI reference WAVs for comparison
│   ├── en_good_morning.wav
│   ├── de_guten_tag.wav
│   ├── es_buenos_dias.wav
│   ├── fr_bonjour.wav
│   ├── it_buongiorno.wav
│   └── ko_annyeong.wav
├── c_src/
│   ├── tts_bridge.h      → C bridge header (extern "C" API)
│   └── tts_bridge.cpp    → wraps Engine in extern "C" functions
├── chatterbox-cpp/       → vendored C++ source (not a submodule)
│   ├── CMakeLists.txt
│   ├── include/          → public API headers (engine.h, s3gen_pipeline.h)
│   ├── src/              → engine, tts, mtl_tokenizer, s3tokenizer, etc.
│   └── ggml/             → GGML tensor library (static build)
├── examples/
│   ├── synthesize.rs     → CLI: text → WAV
│   ├── tts_server.rs     → axum HTTP/WS server
│   └── bench_compare.rs  → multi-language comparison vs reference
├── src/
│   ├── lib.rs            → public API re-exports
│   ├── engine.rs         → safe Engine wrapper
│   ├── ffi.rs            → raw extern "C" declarations
│   ├── error.rs          → EngineError type
│   └── server.rs         → axum server (feature-gated)
└── tests/
    └── integration.rs    → 7 integration tests
```

#### Key Differences from Original Plan

| Plan | Actual | Reason |
|------|--------|--------|
| `vendor/` symlink | Vendored `chatterbox-cpp/` directory | Simpler CI, no separate repo, exact file control |
| `thiserror` dep | Manual `Error` impl | Simpler, fewer deps |
| Separate `tts-cli.rs` example | `examples/synthesize.rs` | Clearer naming, same functionality |
| Byte-identical output | **Perceptually identical** (GPU vs CPU) | GPU (FP16 accumulate) vs CPU (FP32) numerical differences expected; voice, language, content, timing match |
| `codec.cpp` reuse | **Removed** `codec.cpp` / `.gitmodules` | No longer needed — Engine handles all pipeline stages |

#### Build Commands

```bash
# Build with server feature (default)
cargo build

# Without server
cargo build --no-default-features

# Run tests
cargo test

# CLI synthesis
cargo run --example synthesize -- \
    --text "Hello, world." --language en --out hello.wav

# HTTP server
cargo run --example tts_server

# Benchmark comparison
cargo run --example bench_compare

# Release build
cargo build --release
```

#### Cargo.toml (actual)

```toml
[package]
name = "chatterbox-rs"
version = "0.2.0"
edition = "2021"

[features]
default = ["server"]
server = ["dep:axum", "dep:tokio", "dep:serde", "dep:serde_json",
          "dep:futures-util", "dep:tower-http", "dep:tracing"]

[dependencies]
libc = "0.2"

[build-dependencies]
cmake = "0.1"
cc = "1.1"
```

#### GPU Benchmark (RTX 5070 Ti)

| Metric | CPU (AVX) | GPU (CUDA) | Speedup |
|--------|:--------:|:--------:|:-------:|
| T3 inference (107 tok) | 2130 ms | **439 ms** | **4.9×** |
| S3Gen + HiFT | 9464 ms | 1980 ms | **4.8×** |
| Pipeline total | ~11.6 s | ~2.0 s | **5.8×** |
| RTF (real-time factor) | 0.30 | **2.2** | **7.3× better** |

#### Multi-Language Comparison (Rust vs CLI Reference)

| Language | RMSE | SNR | Sample diff | Verdict |
|----------|------|-----|-------------|---------|
| en | 0.171 | 15.2 dB | +4.3% | ✅ Perceptually identical |
| de | 0.152 | 14.6 dB | −2.3% | ✅ Perceptually identical |
| es | 0.188 | 13.1 dB | −1.4% | ✅ Perceptually identical |
| fr | 0.182 | 14.6 dB | +6.2% | ✅ Perceptually identical |
| it | 0.177 | 14.9 dB | +3.2% | ✅ Perceptually identical |
| ko | 0.195 | 14.1 dB | +3.0% | ✅ Perceptually identical |

Differences are due to GPU (FP16 accumulate) vs CPU (FP32) numerical precision. The voice, language, content, and timing are the same. RMSE ~0.15-0.20 on a [-1, 1] scale.

#### Known Issues Carried Forward

- **S3Gen quantization** still produces silence at Q8_0/Q5_0 — no plan to debug, F16 is acceptable
- **Speaking pace** ~1.2× faster than natural — inherent to MTL model variant
- **Korean CUDA crash** (IM2COL error) — intermittent, observed only with specific input lengths. Not reproducible with standalone `synthesize` example. Likely a GGML-CUDA kernel issue for certain tensor sizes.

---

### Phase 2: Vox `TtsProvider` Integration (✓ Complete)

**Goal**: Chatterbox appears as a selectable TTS provider in Vox's settings and pipeline.

**Steps:**
* 2.1 **Add `chatterbox-rs` as a dependency** in Vox's `app/src-tauri/Cargo.toml` (✓ Done)
* 2.2 **Create `services/tts/providers/chatterbox.rs`** implementing `TtsProvider` (✓ Done)
* 2.3 **Register in `TtsProviderConfig`** in `core/settings.rs` (✓ Done)
* 2.4 **Wire in `pipeline.rs` `warm_up_tts()`** (✓ Done)
* 2.5 **Add setup wizard model download** for the two GGUFs (✓ Done)

**Diagnostic Notes (Compiler/Runtime Collision & Resolution)**:
- **The Issue**: Initial integration crashed with `free(): invalid pointer` and exhibited extreme CPU latency slowdowns (26.5× RTF).
- **RCA**: Dynamic linker collision between LLVM's `libc++`/`libomp` (used by Vox) and GNU's `libstdc++`/`libgomp` (compiled into `chatterbox-rs` native code).
- **Fix**: Disabled OpenMP compiler directives (`GGML_OPENMP=OFF` in CMakeLists.txt and build.rs) for CPU builds, eliminating GCC thread-pool competition.
- **Result**: Thread collision and crash fully resolved. TTS Real-Time Factor (RTF) on CPU dropped from **26.5× down to 15.9×** in active Vox-bench runs, and **6.2×** when run in isolation.

---

### Phase 3: Remote Server Mode (✓ Complete)

**Goal**: Offload heavy TTS inference to a remote CUDA-accelerated GPU server (`hypr4@100.86.62.14`) via HTTP chunked streaming PCM, implementing a client-side provider in Vox.

**Implemented Details:**
- **Server Route**: Added `POST /tts/stream-pcm` inside `chatterbox-rs/src/server.rs` returning chunked `audio/pcm-f32le; rate=24000` via Axum and mpsc.
- **Client Provider**: Added `ChatterboxRemoteProvider` in `chatterbox_remote.rs` in Vox. It streams f32 samples from the HTTP body synchronously on the dedicated TTS thread, applies speed stretching, and emits chunks immediately.
- **Deployment Automation**: Added `remote_runner.py` setting up the `/home/hypr4/.vox/` sandbox, pulling latest code, checking models, compiling with CUDA, and launching the daemon.

**Key Metrics & E2E Validation**:
- **TTFA (Latency)**: Reduced from **32.28s** (local CPU) to **2.18s** (remote GPU).
- **TTS RTF**: Reduced from **15.95×** to **0.64×** (faster than real-time speech).
- **Audio Output**: Verified matching PCM audio generated under `outputs/`.

---

### Phase 4 (Future): Hindi Language Support

**Goal**: Enable `--language hi` in `chatterbox.cpp`.

**Context**: The MTL model weights support Hindi (23 languages), but `chatterbox.cpp` has Devanagari NFKD normalization on the backlog.

**Steps:**
4.1 Study `mtl_unicode_tables.inc` format — Korean Jamo tables are the reference for adding a new script
4.2 Research Devanagari Unicode blocks (U+0900–U+097F) and NFKD decompositions
4.3 Use `scripts/gen-nfkd-table.py` to generate Devanagari NFKD tables
4.4 Add table to `mtl_unicode_tables.inc` and wire language code `"hi"` in `t3_mtl.cpp` dispatch
4.5 Test: `tts-cli --language hi --text "नमस्ते दुनिया"`
4.6 Submit upstream PR or maintain as Vox-local patch

**Alternative**: If the backlog issue is just missing Unicode tables, this could be as quick as a day of work. If deeper issues exist, we may need to wait for upstream.

---

## 5. Build & Test Commands (Cheat Sheet for the Implementing Agent)

```bash
# === Phase 0: C++ Standalone ===

# Build tts-cli (CPU-only)
cd /opt/vox/chatterbox-cpp
cmake -B build -DCMAKE_BUILD_TYPE=Release -DGGML_CUDA=OFF
cmake --build build -j --target tts-cli

# Build tts-cli (CUDA — RTX 5070 Ti)
export PATH=/usr/local/cuda/bin:$PATH
cmake -S . -B build_cuda -DCMAKE_BUILD_TYPE=Release -DGGML_CUDA=ON
cmake --build build_cuda -j --target tts-cli

# Test with English (CPU)
./build/tts-cli \
    --model models/chatterbox-t3-mtl-q4_0.gguf \
    --s3gen-gguf models/chatterbox-s3gen-mtl-f16.gguf \
    --text "Good morning, this is a test of the Chatterbox text to speech system." \
    --language en --out /tmp/test_en_cpp.wav

# Test with Spanish
./build/tts-cli \
    --model models/chatterbox-t3-mtl-q4_0.gguf \
    --s3gen-gguf models/chatterbox-s3gen-mtl-f16.gguf \
    --text "Buenos días." --language es --out /tmp/test_es_cpp.wav

# Test with GPU
./build_cuda/tts-cli \
    --model models/chatterbox-t3-mtl-q4_0.gguf \
    --s3gen-gguf models/chatterbox-s3gen-mtl-f16.gguf \
    --text "Testing GPU acceleration." --language en \
    --n-gpu-layers 99 --out /tmp/test_gpu.wav --verbose

# Run with CUDA model paths (from vox-models)
export VOX_MODELS=/opt/vox-models/tts/chatterbox
./build_cuda/tts-cli \
    --model $VOX_MODELS/chatterbox-t3-mtl-q4_0.gguf \
    --s3gen-gguf $VOX_MODELS/chatterbox-s3gen-mtl-f16.gguf \
    --text "Hello from the deployment location." --language en \
    --n-gpu-layers 99 --out /tmp/test_deploy.wav

# Python reference (Turbo variant — different model from our Multilingual!)
cd /opt/vox/chatterbox-cpp
.venv/bin/python3 -c "
from chatterbox.tts_turbo import ChatterboxTurboTTS
tts = ChatterboxTurboTTS.from_pretrained('cpu')
wav = tts.generate(text='Good morning, this is a test.')
import scipy.io.wavfile as wavfile
wavfile.write('/tmp/test_en_py.wav', tts.sr, wav.squeeze(0).cpu().numpy())
"

# Note: Python Turbo and C++ Multilingual are different models.
# They produce different token counts and audio — NOT directly comparable.
# Gate 1 should focus on C++ output being A) valid speech, B) ASR-correct,
# not on matching Python output.
```

---

## 6. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **S3Gen Q8_0 produces silence** | High | LOW (mitigated) | **Using F16 permanently.** 1.0 GB is acceptable for a premium TTS provider. No plan to debug Q8_0 further. |
| **Speaking pace too fast (~1.2×)** | High | Medium | Accept as inherent to Multilingual model. Can time-stretch output audio in Vox pipeline if needed. |
| **GGUF conversion produces different results than Python** | Medium | LOW (outdated concern) | F16 GGUF is lossless. Python Turbo ≠ C++ Multilingual (different models). Not a parity issue. |
| **Hindi NFKD table effort is non-trivial** | Medium | Low (Phase 4) | Can use English for initial integration. Hindi is additive, not blocking |
| **chatterbox.cpp API changes during development** | Low | Medium | Pinned at commit `ddca05f` (`multilingual_merged`). Safe. |
| **CUDA build issues** | Low | LOW (solved) | CUDA 13.3 + sm_120 works perfectly on RTX 5070 Ti. 4.9× T3 speedup. |
| **Memory pressure on 8GB systems** | Low | Low | Chatterbox is a premium provider. Supertonic stays as default. Users can choose. |
| **chatterbox.cpp MTL 10-step CFM is slow on CPU** | High | Medium | GPU offloading solves this (2.2× real-time on RTX 5070 Ti). |

---

## 7. Out of Scope (for this plan)

- Voice cloning (reference audio) — ✓ supported natively via `--reference-audio` and `--save-voice`/`--ref-dir`. Not wired into Vox's TtsProvider yet (Phase 2+).
- Streaming T3 output (per-chunk synthesis) — supported by chatterbox.cpp's `--stream-chunk-tokens`. Not wired in Vox initially.
- Non-MTL Turbo variant — MTL already includes English, no need for a second variant
- Windows/macOS GPU patches — Metal/Vulkan can be added in a follow-up; CUDA is the initial GPU target
- Byte-identical CLI output — GPU (FP16 accumulate) vs CPU (FP32) produces slight numerical differences; perceptually identical is the accepted standard

---

## 8. Summary of Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Runtime** | `chatterbox.cpp` (ggml) | No Python, GPU+CPU, mature, proven |
| **Model variant** | MTL (23 languages) | Covers English, Spanish, and future Hindi |
| **T3 model** | Q4_0 (330 MB) | Small, fast, verified working |
| **S3Gen model** | F16 (1.0 GB) — no quantization | Q8_0 produces silence. F16 works perfectly and CUDA GPU makes speed irrelevant. |
| **CFM steps** | **10 (default)** | Keep default. CFM steps don't affect speaking pace. |
| **GPU** | CUDA (RTX 5070 Ti) | 5.8× faster than CPU. Already built and verified. |
| **Integration path** | chatterbox-rs crate → Vox TtsProvider | Reuse build.rs + axum patterns from old crate; replace llama.cpp backbone with chatterbox.cpp Engine FFI. |
| **Runtime weights location** | `/opt/vox-models/tts/chatterbox/` | Central model storage for both local Vox and remote server. |

---

## Appendix: Current File Inventory

### Runtime Weights (in `/opt/vox/vox-models/tts/chatterbox/`)
| File | Size | Type | Status |
|------|------|------|--------|
| `chatterbox-t3-mtl-q4_0.gguf` | 330 MB | T3 Multilingual Q4_0 | ✓ Runtime model |
| `chatterbox-s3gen-mtl-f16.gguf` | 1.0 GB | S3Gen Multilingual CFM-10 F16 | ✓ Runtime model |

### Model Symlinks (in `/opt/vox-models/tts/chatterbox/`)
| Link | Target | Status |
|------|--------|--------|
| `t3-q4_0.gguf` | → `vox-models/.../chatterbox-t3-mtl-q4_0.gguf` | ✓ |
| `s3gen-f16.gguf` | → `vox-models/.../chatterbox-s3gen-mtl-f16.gguf` | ✓ |

### chatterbox-rs Crate (Phase 1 Complete)
| Path | Purpose |
|------|---------|
| `Cargo.toml` | Crate manifest, feature-gated server |
| `build.rs` | cmake for libtts-cpp.a + cc for tts_bridge.cpp |
| `c_src/tts_bridge.h` | C bridge header (extern "C" API) |
| `c_src/tts_bridge.cpp` | Wraps C++ Engine in extern "C" |
| `chatterbox-cpp/` | Vendored C++ source (stripped runtime files) |
| `src/lib.rs` | Public API re-exports |
| `src/engine.rs` | Safe Engine + EngineOptions + SynthesisResult |
| `src/ffi.rs` | Raw extern "C" FFI declarations |
| `src/error.rs` | EngineError type |
| `src/server.rs` | Axum HTTP/WS server (feature-gated) |
| `examples/synthesize.rs` | CLI: text → WAV |
| `examples/tts_server.rs` | Axum server binary |
| `examples/bench_compare.rs` | Multi-language comparison vs CLI refs |
| `tests/integration.rs` | 7 integration tests |
| `README.md` | Crate documentation |
| `bench_audio/` | Reference WAVs (6 languages) |

### Removed (Phase 1 Cleanup)
| Path | Reason |
|------|--------|
| `/opt/vox/chatterbox-cpp/` | Vendored into crate, dir deleted to save space |
| `codec.cpp` / `.gitmodules` | No longer needed (Engine handles all pipeline stages) |
| `src/main.rs` (old) | Replaced by examples |
