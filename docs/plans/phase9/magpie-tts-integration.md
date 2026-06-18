# Implementation Plan: NVIDIA Magpie-TTS-Multilingual-357M — Premium Local TTS Provider

> **Validation note:** This plan was verified against the actual Vox codebase on 2026-06-17. All file paths, trait signatures, function names, and settings keys were cross-referenced with `services/tts/providers/mod.rs`, `services/pipeline.rs`, `core/settings.rs`, `services/utils.rs`, and `services/tts/actor.rs`. Corrections include: `TtsProvider` trait uses `synthesize_chunk()` (not `synthesize()`), returns `TtsProviderKind` enum (not `&'static str`), `health_check()` returns `bool` (not `Result`). `is_devanagari()` lives in `services/utils.rs` not a `translit` module. Supertonic's `TtsProviderConfig` is a unit variant (no fields). Resampling happens per-chunk inside `synthesize_chunk()`, not in a pipeline-level DSP chain. Phase 3 "DSP refactoring" was removed — it was based on a misunderstanding of the architecture.

**Objective:** Integrate NVIDIA Magpie-TTS-Multilingual-357M as a premium local TTS provider for Vox, supporting English and Hindi (with 7 additional languages), CPU-only inference, sub-500ms TTFA, and full `TtsProvider` trait compatibility. Supertonic 3 remains the default TTS for 8GB profiles; Magpie targets 16GB+ profiles.

---

## Architecture Constraints (Applied)

- **No Python at runtime** — Python NeMo is acceptable for validation only. Final deployment must be Rust/C++ native via GGML/GGUF or ONNX.
- **CPU-only inference** — no GPU dependency allowed in production. GPU may be used for model conversion but not inference.
- **Supertonic 3 remains default** — Magpie is additive. The existing `TtsProviderConfig` enum gains a `Magpie` variant alongside `Supertonic`. The pipeline dispatches based on config; no existing behavior changes.
- **Streaming-first** — Magpie must produce partial audio chunks for overlapping playback. No stage waits for completion. The `TtsProvider::synthesize_chunk()` method pushes `VoxEvent::TtsChunk` events via a channel, and the pipeine starts playback on the first chunk. Magpie must follow this contract exactly.
- **Existing trait pattern** — `TtsProvider` trait with `synthesize_chunk()`, `kind() → TtsProviderKind`, `health_check() → bool`, `set_quality_steps()`, `set_speed()`. All providers output **24 kHz f32 mono** (Magpie outputs 22.05 kHz natively → resampled to 24 kHz within the provider; see Phase 3).
- **Language detection reuses existing infrastructure** — the `is_devanagari()` function in `services/utils.rs` detects Hindi; Magpie's tokenizer dispatches based on language code, analogous to current Supertonic logic.
- **Model memory budget** — ~900 MB total (679 MB model + 126 MB NanoCodec decoder + ~95 MB runtime/KV cache). Fits 16GB profile; tight on 8GB. Magpie is NOT available on 8GB profiles.

---

## Magpie-TTS Model Overview

| Property | Value |
|---|---|
| **Parameters** | 357M (241M trainable) |
| **Architecture** | 6L encoder (768d, 12 heads) + 12L causal decoder (768d, 12 SA + 1 XA) + 1L local transformer (8 codebooks) + HiFi-GAN codec decoder |
| **Output sample rate** | 22050 Hz (must resample to 24000 Hz for Vox playback) |
| **Codec** | NanoCodec 1.89kbps-21.5fps — 8 codebooks × 2016 codes, FSQ quantization, ~1025.6 samples/frame |
| **Languages** | **9**: en, es, de, fr, vi, it, zh, hi, ja (v2602) |
| **Speakers** | 5 baked English identities (Sofia, Aria, Jason, Leo, John Van Stan); each speaker works with all 9 languages |
| **License** | NVIDIA Open Model License (gated on HF, requires form) |
| **NeMo version** | v2602 (Feb/March 2026) — Hindi + Japanese added in this release |

**Tokenization:** Per-language phoneme tokenizers (IPA/grapheme-to-phoneme) + optional byte-level. Text conditioning uses a configurable per-language tokenizer. Each language has a ~30 KB tokenizer config JSON.

**Text normalization (TN):** Available for: `en`, `es`, `de`, `fr`, `it`, `zh`. **Not available for:** `vi`, `hi`, `ja`. For Hindi, raw Devanagari text is passed directly.

---

## Top-Level Approach

This plan proceeds in four sequential phases:

| Phase | What | Outcome |
|---|---|---|
| **Phase 0** | Python NeMo CPU validation | RTF numbers, Hindi quality assessment, go/no-go decision |
| **Phase 1** | Native inference engine selection | Build or source a GGML/ONNX/C++ inference path |
| **Phase 2** | Rust `TtsProvider` integration | Full provider wired into pipeline with trait compliance |
| **Phase 3** | DSP pipeline configuration | 22.05kHz → 24kHz resampling, configurable LPF per provider |

---

## Phase 0 — Python NeMo CPU Validation

**Duration:** 2-3 hours  
**Goal:** Measure real-time factor (RTF) and subjective quality before committing engineering resources. If RTF > 1.0 on the target CPU, Magpie is not viable for local inference.

### 0.1 — Environment Setup

```bash
# Create isolated Python environment
python3 -m venv venv_magpie
source venv_magpie/bin/activate

# Install NeMo TTS with CPU-only PyTorch
# PyTorch must be installed BEFORE nemo_toolkit to avoid CUDA dependency
pip install torch --index-url https://download.pytorch.org/whl/cpu
pip install nemo_toolkit[tts]
pip install kaldialign
```

See [`docs/plans/tts-options.md`](docs/plans/tts-options.md) §5.4 for prior Magpie assessment context.

### 0.2 — Model Loading

```python
import torch
from nemo.collections.tts.models import MagpieTTSModel

# Load model on CPU — may take 30-60s to initialize
# Requires accepting NVIDIA Open Model License on HF
model = MagpieTTSModel.from_pretrained(
    "nvidia/magpie_tts_multilingual_357m",
    map_location='cpu'       # Force CPU
)
model.eval()
model = model.to('cpu')      # Double-ensure no CUDA
```

**Known issue:** PyTorch 2.6+ defaults to `weights_only=True`. Set `TORCH_FORCE_NO_WEIGHTS_ONLY_LOAD=1` if loading `.nemo` checkpoints fails.

### 0.3 — Benchmark Script

```python
import time
import numpy as np

TEST_CASES = [
    # (text, language, speaker_index, description)
    ("Hello, this is a test of the Magpie text to speech system.", "en", 1, "EN short"),
    ("The quick brown fox jumps over the lazy dog. This sentence contains every letter of the alphabet.", "en", 1, "EN medium"),
    ("Welcome to Vox, your voice assistant. I can help you with various tasks including setting reminders, answering questions, and controlling your smart home devices. All of this runs locally on your computer.", "en", 1, "EN long"),
    
    ("नमस्ते, आप कैसे हैं?", "hi", 1, "HI short"),
    ("आज का मौसम बहुत अच्छा है। मुझे घूमने जाना है।", "hi", 1, "HI medium"),
    ("नमस्कार, मैं आपकी सहायता के लिए यहाँ हूँ। कृपया बताएं कि मैं आपके लिए क्या कर सकता हूँ। आप अपनी आवाज़ से मुझे निर्देश दे सकते हैं और मैं तुरंत उत्तर दूंगा।", "hi", 1, "HI long"),
]

for text, lang, speaker_idx, desc in TEST_CASES:
    # Warmup
    for _ in range(2):
        _ = model.do_tts(text, language=lang, speaker_index=speaker_idx)
    
    # Timed run
    torch.cuda.synchronize() if torch.cuda.is_available() else None
    start = time.perf_counter()
    audio, audio_len = model.do_tts(text, language=lang, speaker_index=speaker_idx)
    elapsed = time.perf_counter() - start
    
    audio_duration = audio.shape[-1] / 22050.0  # Magpie outputs at 22.05kHz
    rtf = elapsed / audio_duration
    
    print(f"{desc}: {audio_duration:.2f}s audio in {elapsed:.2f}s wall = RTF {rtf:.2f}x")
    
    # Save for subjective evaluation
    import scipy.io.wavfile as wav
    wav.write(f"magpie_{desc.replace(' ', '_')}.wav", 22050, audio.numpy())
```

### 0.4 — Pass/Fail Criteria

| Metric | Target (Pass) | Acceptable | Fail |
|---|---|---|---|
| RTF English (short) | < 0.5x | < 0.8x | > 1.0x |
| RTF English (long) | < 0.5x | < 0.8x | > 1.0x |
| RTF Hindi (short) | < 0.5x | < 0.8x | > 1.0x |
| RTF Hindi (long) | < 0.5x | < 0.8x | > 1.0x |
| Peak memory | < 2.5 GB | < 3.5 GB | > 4.0 GB |
| Hindi quality | Better than Supertonic 3 | Equal to Supertonic 3 | Worse or garbled |

**Go/no-go rule:** If any test case exceeds RTF > 1.0x on the target CPU, Magpie is **not viable** for local inference. In that case, the only path forward is cloud inference via NVIDIA NIM API (`TtsProvider` wrapping gRPC).

### 0.5 — Deliverable

A single JSON file `magpie_cpu_benchmark_results.json` with all metrics, plus 8 WAV samples (4 test cases × 2 runs each) for subjective A/B comparison against Supertonic 3.

---

## Phase 1 — Native Inference Engine

**Duration:** 1-2 weeks  
**Goal:** Produce a C++ or Rust library that loads Magpie-TTS weights (GGUF) and synthesizes audio without any Python/NeMo dependency.

### 1.1 — Deployment Path Options

| Path | Description | Risk | Effort | Recommended? |
|---|---|---|---|---|
| **A: magpie-tts.cpp** | Community GGML port (m1el/magpie-tts.cpp) | Repo returns 404 (private/deleted) | Low (if available) | ⏸ On hold until repo appears |
| **B: GGML Rust (candle/ggml-rs)** | Write inference using `candle` or `ggml-rs` to load GGUF weights directly | High — requires implementing the full transformer forward pass, KV cache, and NanoCodec decoder in Rust | 3-4 weeks | ⚠️ Backup |
| **C: Community ONNX** | Use Knehm's ONNX export of NanoCodec decoder; write only encoder/decoder inference | Medium — ONNX RT handles decoder, but text encoder + AR loop still custom | 2-3 weeks | ⚠️ Viable |
| **D: NeMo → ONNX partial export** | Extract individual submodules from NeMo and export to ONNX separately | Very High — NeMo export infrastructure is not designed for this | 4+ weeks | ❌ Rejected |
| **E: NVIDIA NIM API** | Use NVIDIA's hosted Magpie-TTS gRPC endpoint as a cloud `TtsProvider` | Low — standard HTTP client, no model loading | 1-2 days | ✅ **Phase 1 fallback** |

### 1.2 — Decision Tree

```
Phase 0 RTF < 0.8x?
  ├─ YES → Evaluate Path A (magpie-tts.cpp)
  │          ├─ Repo exists and compiles? → Use Path A with Rust FFI
  │          └─ Repo missing or broken? → Evaluate Path B vs Path C
  │                  ├─ GGML expertise available? → Path B (candle)
  │                  └─ ONNX preference? → Path C (NanoCodec ONNX + custom encoder)
  └─ NO  → Path E (NVIDIA NIM cloud) is the only viable option
```

### 1.3 — Path A: magpie-tts.cpp FFI (Preferred)

This is the lowest-effort path **if the repo becomes publicly available**. The repo (`m1el/magpie-tts.cpp`) existed in Feb-Mar 2026 with 13 commits but is currently 404. If/when it surfaces:

1. Clone and verify build with `-DGGML_CPU=ON`
2. Test inference with existing GGUF weights (7-language version)
3. Identify any gaps: streaming API, error handling, Rust bindings
4. Build `magpie-tts-sys` crate (FFI bindings, similar to `llama-cpp-2`)
5. Build `magpie-tts` crate (safe Rust wrapper)

**FFI binding structure:**

```rust
// magpie-tts-sys/src/lib.rs (auto-generated via bindgen)
extern "C" {
    pub fn magpie_init(model_path: *const c_char, codec_path: *const c_char) -> *mut MagpieContext;
    pub fn magpie_synthesize(ctx: *mut MagpieContext, text: *const c_char, language: *const c_char, 
                             speaker_id: i32, out_samples: *mut *mut f32, out_len: *mut usize) -> i32;
    pub fn magpie_free_samples(samples: *mut f32, len: usize);
    pub fn magpie_destroy(ctx: *mut MagpieContext);
}
```

### 1.4 — Path B: Custom Rust Inference (candle)

If magpie-tts.cpp is unavailable, implement the transformer forward pass using `candle`:

```rust
// Pseudocode for MagpieInferenceEngine
pub struct MagpieInferenceEngine {
    encoder: CausalTransformer,        // 6 layers, d=768, 12 heads
    decoder: CrossAttnTransformer,     // 12 layers, d=768, 12 SA + 1 XA heads, with KV cache
    local_transformer: CodebookHead,   // 1 layer, d=256, predicts 8 codebooks
    codec_decoder: NanoCodecDecoder,   // HiFi-GAN decoder with FSQ dequant
    tokenizers: HashMap<String, Tokenizer>,  // Per-language tokenizers
    sample_rate: u32,                  // 22050
}

impl MagpieInferenceEngine {
    /// Synthesize audio from text
    fn synthesize(&mut self, text: &str, language: &str, speaker_id: u32) -> Vec<f32> {
        // 1. Tokenize text using per-language tokenizer
        let tokens = self.tokenizers.get(language).unwrap().encode(text);
        
        // 2. Run encoder (non-autoregressive, processes all tokens at once)
        let encoded = self.encoder.forward(&tokens);
        
        // 3. Run decoder (autoregressive with KV cache)
        //    Initialize KV cache with 110-step baked speaker context
        let mut kv_cache = self.decoder.init_kv_cache(speaker_id);
        let mut codebook_frames: Vec<[u32; 8]> = Vec::new();
        
        // Decode frame by frame
        for step in 0..max_frames {
            let logits = self.decoder.step(&encoded, &kv_cache, step);
            let tokens = self.local_transformer.sample(logits);
            kv_cache.update(tokens);
            codebook_frames.push(tokens);
        }
        
        // 4. Decode codebook frames to waveform via NanoCodec decoder
        let audio = self.codec_decoder.decode(&codebook_frames);
        
        audio
    }
}
```

**Key implementation details:**

- **Decoder KV cache:** 12 layers × (key, value) per head × baked speaker context (110 frames). The MLX port achieves ~10× speedup by prefilling the speaker context into KV cache (baked prompt).
- **Codec decoder:** HiFi-GAN with Snake activation, 8→1 channels, FSQ dequant with levels [8,7,6,6]. Community ONNX export exists via Knehm/nemo-nano-codec-22khz-1.89kbps-21.5fps-ONNX — can load in `ort` crate.
- **Tokenizers:** Per-language JSON config files (~30KB each, 9 files). Format: custom IPA/phoneme vocabulary with BOS/EOS tokens.

### 1.5 — GGUF Weight Gap: Hindi Support

The existing GGUF weights on HF (`m1el/magpie-tts-multilingual-357m-gguf`) only support 7 languages:

| GGUF languages | Missing (v2602) |
|---|---|
| en, es, de, fr, vi, it, zh | **hi, ja** |

**To get Hindi-supporting GGUF weights:**

1. **Download the NeMo `.nemo` checkpoint** from `nvidia/magpie_tts_multilingual_357m` (requires HF login + license acceptance)
2. **Convert to GGUF** — the original `convert_magpie_to_gguf.py` script used by m1el is part of the (unavailable) magpie.cpp repo. We would need to write our own conversion script or contact the maintainer.

**Alternative:** Use the MLX port's conversion as a reference. The MLX port (by `aufklarer`) supports INT8 and INT4 for Apple Silicon — its architecture splits the model into 4 bundles (text_encoder, decoder_prefill, decoder_step, nanocodec_decoder) which maps cleanly to a GGUF multi-tensor format.

### 1.6 — Path E: NVIDIA NIM API (Cloud Fallback)

If local inference is too slow (RTF > 1.0x), implement a cloud provider:

- **API:** NVIDIA NIM gRPC endpoint serving Magpie-TTS
- **Auth:** NVIDIA API key (standard HTTP bearer token)
- **Pricing:** Via NVIDIA NIM (usage-based, typically $0.00x per minute)
- **Implementation:** Follows the same `TtsProvider` trait pattern, sending text via gRPC and receiving audio stream
- **Latency:** Network-dependent (expect 200-500ms RTT + model inference time)
- **No local model memory cost** — uses ~10MB for the gRPC client

---

## Phase 2 — Rust `TtsProvider` Integration

**Duration:** 3-5 days (after Phase 1 completes)  
**Goal:** Full integration of Magpie into Vox's existing TTS pipeline via the `TtsProvider` trait.

### 2.1 — Configuration Changes

#### `core/settings.rs` — Add `Magpie` variant to `TtsProviderConfig`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TtsProviderConfig {
    Supertonic,
    Magpie {
        model_path: PathBuf,           // Path to GGUF model file
        codec_path: PathBuf,           // Path to NanoCodec GGUF file
        language: String,              // Default language ("en", "hi", etc.)
        speaker_id: u32,               // 0-4 (baked English speakers)
        quality_steps: u32,            // CFG scale (1.0-3.0, default 2.5)
        speed: f32,                    // 0.5-2.0, default 1.0
    },
}
```

**Serialization tag:** `kind` (not `provider`) — matches the existing `TtsProviderConfig` serde tag.

**Setting reload policy:** `tts.provider` is already `SettingReloadPolicy::Restart` in `reload_policy_for()` — changing provider requires TTS worker restart (same as Supertonic). No policy change needed.

#### `core/settings.rs` — Speaker metadata

```rust
pub const MAGPIE_SPEAKERS: &[(&str, &str)] = &[
    ("Sofia",      "Female, clear, neutral"),
    ("Aria",       "Female, warm, expressive"),
    ("Jason",      "Male, deep, authoritative"),
    ("Leo",        "Male, young, casual"),
    ("John Van Stan", "Male, mid-range, natural"),
];
```

### 2.2 — Provider Implementation

#### `services/tts/providers/magpie.rs` (new file)

```rust
use crate::core::events::VoxEvent;
use crate::services::tts::providers::{TtsProvider, TtsProviderKind};
use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub struct MagpieProvider {
    engine: Arc<Mutex<MagpieInferenceEngine>>,  // From Phase 1
    config: MagpieConfig,
    language: String,
    speaker_id: u32,
    quality_steps: AtomicU32,
    speed: AtomicU32,  // stored as integer steps internally
}

impl TtsProvider for MagpieProvider {
    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<()> {
        // Auto-detect language from Devanagari text
        let language = if crate::services::utils::is_devanagari(text) {
            "hi"
        } else {
            &self.language
        };

        let mut engine = self.engine.lock().unwrap();
        let raw_audio = engine.synthesize(text, language, self.speaker_id)?;

        // Resample from 22050 to 24000 Hz
        let samples_24k = resample_22050_to_24000(&raw_audio);

        // Push chunk to pipeline — playback begins immediately on first chunk
        let _ = event_tx.send(VoxEvent::TtsChunk {
            turn_id,
            samples: samples_24k,
        });

        let _ = event_tx.send(VoxEvent::TtsFinished { turn_id, rtf: 0.0 });

        Ok(())
    }

    fn set_quality_steps(&self, steps: u32) {
        self.quality_steps.store(steps, Ordering::Relaxed);
    }

    fn set_speed(&self, speed: f32) {
        self.speed.store((speed * 10.0) as u32, Ordering::Relaxed);
    }

    fn kind(&self) -> TtsProviderKind {
        TtsProviderKind::Magpie
    }

    fn health_check(&self) -> bool {
        self.engine.lock().map(|e| e.is_ready()).unwrap_or(false)
    }
}
```

**Key trait compliance points:**

- `synthesize_chunk()` pushes `VoxEvent::TtsChunk` for **streaming partial output** — the pipeline starts playback on the first chunk, overlapping with ongoing LLM generation. This matches Supertonic's callback-based streaming pattern.
- `kind()` returns the new `TtsProviderKind::Magpie` enum variant. Add `Magpie` to `TtsProviderKind` in `providers/mod.rs`.
- `health_check()` returns `bool` — verifies engine is initialized and model files loaded.
- `set_quality_steps()` / `set_speed()` are hot-reloadable (`SettingReloadPolicy::WorkerCommand` for both keys — already configured in `reload_policy_for()`).

> [!IMPORTANT]
> `TtsProviderKind` must be updated in `services/tts/providers/mod.rs` to include `Magpie`. This is the single source of truth for provider identification used in serialization, logging, and frontend display.

### 2.3 — Pipeline Dispatch

#### `services/pipeline.rs` — Add Magpie match arm in `warm_up_tts()`

**Current (`app/src-tauri/src/services/pipeline.rs`, lines 259–267):**
```rust
let provider: Box<dyn TtsProvider> = match &provider_config {
    TtsProviderConfig::Supertonic => {
        log::info!("[Pipeline] Warming up TTS worker (Supertonic)...");
        Box::new(
            SupertonicEngine::new(&self.super_tts_path, voice, quality_steps, speed)
                .map_err(|e| format!("Failed to create Supertonic engine: {}", e))?,
        )
    }
};
```

`TtsProviderConfig::Supertonic` is a **unit variant** (no fields). Language detection happens inside Magpie at synthesis time via `is_devanagari()`, not via a config field.

**After (`$CHANGE`, lines 259–276):**
```rust
let provider: Box<dyn TtsProvider> = match &provider_config {
    TtsProviderConfig::Supertonic => {
        log::info!("[Pipeline] Warming up TTS worker (Supertonic)...");
        Box::new(
            SupertonicEngine::new(&self.super_tts_path, voice, quality_steps, speed)
                .map_err(|e| format!("Failed to create Supertonic engine: {}", e))?,
        )
    }
    TtsProviderConfig::Magpie { model_path, codec_path, language, speaker_id, quality_steps, speed } => {
        log::info!("[Pipeline] Warming up TTS worker (Magpie)...");
        Box::new(
            MagpieProvider::new(model_path, codec_path, language, *speaker_id, *quality_steps, *speed)
                .map_err(|e| format!("Failed to create Magpie engine: {}", e))?,
        )
    }
};
```

### 2.4 — App State Changes

```rust
// core/state.rs — Add magpie_engine field (if needed for pre-initialization)
pub struct AppState {
    // ... existing fields ...
    pub magpie_engine: OnceLock<Arc<Mutex<MagpieInferenceEngine>>>,
}
```

`OnceLock` ensures the engine is initialized exactly once, even if the TTS worker restarts. This is important because loading the GGUF model file is expensive (~1-2 seconds on CPU).

### 2.5 — Model Path Resolution

Magpie weights are stored in Vox's model directory alongside Supertonic:

```
~/.vox/models/
├── magpie/
│   ├── magpie-357m-q8.gguf       # ~711 MB
│   └── nano-codec-f32.gguf       # ~126 MB
├── supertonic/
│   └── supertonic-3.onnx         # ~400 MB
└── ...
```

### 2.6 — Settings UI (Frontend)

In the Settings page → TTS section, add:

- **Provider selector:** Supertonic 3 / Magpie (drop-down)
- **Magpie-specific config** (shown when Magpie selected):
  - Language: `en / hi / es / de / fr / vi / it / zh / ja` (drop-down)
  - Speaker: `Sofia / Aria / Jason / Leo / John Van Stan` (drop-down with preview)
  - Quality: CFG scale slider (1.0-3.0) mapped to 1-5 steps
  - Model status: `Download [X MB]` / `Loaded` / `Error`

Settings are persisted via `crate::core::settings::reload_policy_for()` — `tts.provider` already maps to `SettingReloadPolicy::Restart` (see `core/settings.rs:265`). Quality and speed keys map to `WorkerCommand` for hot-reload without restart.

---

## Phase 3 — DSP Pipeline Configuration

**Duration:** 1-2 days  
**Goal:** Make the TTS DSP pipeline configurable per-provider — Magpie outputs 22.05 kHz but Vox playback expects 24 kHz.

### 3.1 — Current DSP Pipeline (Supertonic)

```
Supertonic 3 → 44.1 kHz f32 → Butterworth LPF (11kHz cutoff) → upsample_2x() → 24 kHz f32 → playback
```

The LPF + upsample produces clean 24 kHz audio. The 11 kHz cutoff is valid for 44.1 kHz (Nyquist = 22.05 kHz).

### 3.2 — Required DSP Pipeline (Magpie)

```
Magpie → 22.05 kHz f32 → Butterworth LPF (10.5kHz cutoff) → resample_22050_to_24000() → 24 kHz f32 → playback
```

**Resampling 22050 Hz → 24000 Hz:**

Ratios:
- `24000 / 22050 = 160/147` (non-integer rational)
- Standard approach: upsample by 160, then downsample by 147
- Use `libsamplerate` (`src` crate) with `SRC_SINC_FASTEST` for quality/efficiency balance

**Implementation:**

```rust
use libsamplerate::{resample, SrcType, Ratio};

/// Resample from 22050 Hz to 24000 Hz using libsamplerate
fn resample_22050_to_24000(input: &[f32]) -> Vec<f32> {
    let ratio = 24000.0 / 22050.0;  // ≈ 1.088435
    let output_len = (input.len() as f64 * ratio as f64).ceil() as usize;
    let mut output = vec![0.0f32; output_len];
    
    resample(
        SrcType::SincFastest,  // Good quality, reasonable speed
        ratio,
        input,
        &mut output,
    ).expect("Resampling failed");
    
    output
}
```

### 3.3 — Resampling Strategy

**Critical observation from `supertonic.rs` (lines 66–83, 237–255):** Resampling is **not** a separate pipeline stage. It happens **inside** `synthesize_chunk()` via the per-chunk callback. Supertonic generates 44.1kHz audio through sherpa-onnx's `generate_with_config()` callback, which delivers raw `&[f32]` chunks. Each chunk is LPF'd then linearly interpolated to 24kHz **within the callback** before being sent as `VoxEvent::TtsChunk`.

The same pattern applies to Magpie. Magpie's engine outputs 22.05kHz chunks; resampling to 24kHz happens **inside `synthesize_chunk()`**, not in a shared DSP module. This means:

- **No `native_sample_rate()` trait method needed** — the provider always sends 24kHz to the pipeline.
- **No changes to `services/pipeline.rs` DSP chain** — the pipeline receives `TtsChunk { samples }` at 24kHz regardless of provider.
- **No shared DSP utility required** — each provider owns its resampling.

**Why the original plan's Option A was wrong:** The document proposed adding `native_sample_rate()` to the trait and making the pipeline's DSP chain conditional. But the actual architecture passes final-rate samples through `VoxEvent::TtsChunk`. The resampling is a provider-internal concern, not a pipeline concern.

### 3.4 — Resampling Implementation for Magpie

Magpie's engine outputs at 22050 Hz. The resampler must be efficient since it runs per-chunk during synthesis:

```rust
/// Resample from 22050 Hz to 24000 Hz using libsamplerate.
///
/// Called inside the Magpie synthesize callback, similar to how
/// `resample_44100_to_24000()` is called in supertonic.rs line 248.
fn resample_22050_to_24000(input: &[f32]) -> Vec<f32> {
    let ratio = 24000.0 / 22050.0;  // ≈ 1.088435
    let output_len = (input.len() as f64 * ratio as f64).ceil() as usize;

    // Simple linear interpolation is sufficient for Magpie's chunk sizes.
    // Supertonic uses the same approach (see supertonic.rs lines 74–81).
    let mut output = Vec::with_capacity(output_len);
    let mut src_idx: f32 = 0.0;

    while (src_idx as usize) < input.len() {
        let idx = src_idx as usize;
        let next_idx = (idx + 1).min(input.len() - 1);
        let frac = src_idx - idx as f32;
        output.push((1.0 - frac) * input[idx] + frac * input[next_idx]);
        src_idx += ratio;
    }

    output
}
```

**Why linear interpolation (not `libsamplerate`):**

- Supertonic already uses linear interpolation (see `supertonic.rs:74–81`) — this is the proven approach in the codebase.
- Magpie's output is already band-limited by the NanoCodec decoder (max ~11kHz content at 22.05kHz sample rate), so a simple LPF + linear interpolation is sufficient for the 22.05→24kHz up-conversion.
- `libsamplerate` adds a dependency for marginal quality improvement. The existing architecture avoids it.

**Anti-aliasing LPF for Magpie:** Apply a 2nd-order Butterworth LPF at 10.5kHz cutoff (vs Supertonic's 11kHz at 44.1kHz). The lower cutoff accounts for Magpie's lower native sample rate. Use the same `BiquadFilter` structure from `supertonic.rs` with adjusted coefficients, or add a `new_lpf_10_5k()` constructor.

**No shared DSP utility needed.** The `BiquadFilter` and resampling logic are self-contained within each provider's `synthesize_chunk()` callback. Adding a shared module would create unnecessary coupling — Supertonic and Magpie have different filter parameters, sample rates, and chunk sizes.

### 3.5 — LPF Configuration (Per-Provider)

The current Butterworth LPF in `supertonic.rs` uses a struct with hardcoded coefficients:

```rust
// supertonic.rs:38–51
fn new_lpf_11k() -> Self {
    Self {
        b0: 0.291851, b1: 0.583701, b2: 0.291851,
        a1: -0.004173, a2: 0.171576,
        x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
    }
}
```

For Magpie, a 10.5kHz LPF at 22.05kHz sample rate requires different coefficients. The Magpie provider will use its own `BiquadFilter::new_lpf_10_5k()` with computed coefficients. This is a per-provider detail, not a shared configuration.

> [!IMPORTANT]
> **Do NOT refactor the LPF into a shared DSP module.** The original plan's Phase 3.3/3.4 "configurable DSP per provider" approach was based on an incorrect understanding of the architecture. Resampling happens inside each provider's `synthesize_chunk()` callback, not in a pipeline-level DSP chain. Adding a `native_sample_rate()` trait method or shared resampler would be unnecessary abstraction with no callers.

---

## Phase 4 — Model Distribution

**Duration:** 1-2 days  
**Goal:** Ensure users can download and verify Magpie-TTS weights through Vox's existing model download infrastructure.

### 4.1 — Weight Sources

| File | Source | Size | License | Checksum |
|---|---|---|---|---|
| `magpie-357m-q8.gguf` | HF `m1el/magpie-tts-multilingual-357m-gguf` | ~711 MB | MIT (GGUF) | SHA256 in model manifest |
| `nano-codec-f32.gguf` | HF `m1el/magpie-tts-multilingual-357m-gguf` | ~126 MB | MIT (GGUF) | SHA256 in model manifest |

**If Hindi-supporting GGUF weights are unavailable:**

1. Download `.nemo` file from `nvidia/magpie_tts_multilingual_357m` (~1.2 GB, gated)
2. Run conversion script to produce GGUF (requires GPU for the conversion pass, or accept slow CPU conversion)
3. The conversion extracts: text_encoder weights, decoder weights, local transformer weights, codec decoder weights, tokenizer configs

### 4.2 — Model Manifest

Update `manifests/model-manifest.json`:

```json
{
  "models": {
    "magpie": {
      "version": "2602",
      "files": {
        "magpie-357m-q8.gguf": {
          "url": "https://huggingface.co/m1el/magpie-tts-multilingual-357m-gguf/resolve/main/magpie-357m-q8.gguf",
          "size": 711000000,
          "sha256": "<pending verification>"
        },
        "nano-codec-f32.gguf": {
          "url": "https://huggingface.co/m1el/magpie-tts-multilingual-357m-gguf/resolve/main/nano-codec-f32.gguf",
          "size": 126000000,
          "sha256": "<pending verification>"
        }
      }
    }
  }
}
```

### 4.3 — Download Integration

Use Vox's existing model download infrastructure (HTTP client with progress reporting, resume support):

- Add "Download Magpie TTS" button in Settings → TTS
- Show progress during download (~711 MB at typical 10 MB/s = ~70s)
- Verify SHA256 after download
- On verification failure, allow retry
- Track download state in existing model download state machine

---

## Memory Budget Analysis

### 16GB Profile (Magpie + 8B LLM)

| Component | Memory | Notes |
|---|---|---|
| OS & background | ~2800 MB | |
| Tauri app (WebView + Rust) | ~300 MB | |
| Local LLM (8B Q8_0) | ~8000 MB | |
| **Magpie TTS (Q8_0 GGUF)** | **~920 MB** | 711 MB model + 126 MB codec + ~85 MB runtime/KV cache |
| **Headroom** | **~5284 MB** | ✅ Comfortable |

### 8GB Profile (Supertonic only — no change)

| Component | Memory | Notes |
|---|---|---|
| OS & background | ~2500 MB | |
| Tauri app | ~300 MB | |
| Local LLM (3B Q4_K_M) | ~3500 MB | |
| **Supertonic 3 (ONNX)** | **~400 MB** | |
| **Headroom** | **~892 MB** | Tight but functional |

**Magpie is NOT available on 8GB profiles.** Settings UI should gray out the option and show "Requires 16GB+ RAM".

---

## Test Plan

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_resample_22050_to_24000() {
        let input = vec![0.0f32; 22050];  // 1 second of silence at 22.05kHz
        let output = resample_22050_to_24000(&input);
        assert_eq!(output.len(), 24000);  // Should be approximately 1 second at 24kHz
    }

    #[test]
    fn test_magpie_config_serialization() {
        // Verify TtsProviderConfig::Magpie serializes/deserializes correctly
        let config = TtsProviderConfig::Magpie {
            model_path: PathBuf::from("/models/magpie.gguf"),
            codec_path: PathBuf::from("/models/codec.gguf"),
            language: "en".to_string(),
            speaker_id: 1,
            quality_steps: 5,
            speed: 1.0,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: TtsProviderConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, TtsProviderConfig::Magpie { .. }));
    }
}
```

**Note:** Language detection is tested indirectly via the existing `is_devanagari()` tests in `services/utils.rs`. No separate `resolve_language()` function exists — detection happens inline in `synthesize_chunk()`.

### Integration Tests

- **vox-bench:** Run full pipeline with Magpie TTS in place of Supertonic. Measure end-to-end latency.
- **Hindi pipeline test:** `STT (Hindi) → LLM → TTS (Magpie, Hindi)` — verify Hindi output is intelligible.
- **English pipeline test:** Same for English. Compare TTFA vs Supertonic.

### Manual Test Scenarios

| # | Scenario | Expected |
|---|---|---|
| 1 | Select Magpie in Settings → Speak "Hello" | Audio plays with Magpie voice (Sofia), clear and natural |
| 2 | Switch to Hindi → Speak "नमस्ते" | Hindi output is clear, Devanagari text is processed correctly |
| 3 | Rapid successive TTS requests (5 in 10s) | No memory leak or crash. Each request completes. |
| 4 | Barge-in during TTS playback | Previous TTS stops immediately, new TTS starts cleanly |
| 5 | Switch provider back to Supertonic 3 | Restart required (per SettingReloadPolicy). Works after restart. |
| 6 | Long text (~200 chars, multiple sentences) | Chunked correctly, no truncation or silence gaps |
| 7 | Settings → TTS section with Magpie selected | Magpie-specific config (language, speaker, quality) shown correctly |
| 8 | Download Magpie weights | Progress shown, SHA256 verified, "Loaded" status shown |

---

## Risk Register

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| **CPU RTF > 1.0x** | Local inference not viable | Medium | Fall back to Path E (NIM API). Cloud provider uses same TtsProvider trait. |
| **magpie-tts.cpp repo stays private** | Higher implementation effort | Medium | Implement Path B (custom Rust via candle/ggml-rs). Reference MLX port architecture. |
| **Hindi-supporting GGUF weights unavailable** | Hindi not supported in local mode | Medium | Write conversion script from NeMo `.nemo` → GGUF. Reference MLX port conversion. |
| **NanoCodec ONNX variant mismatch** | Codec decoder gives wrong output | Low | Knehm's 1.89kbps ONNX matches Magpie's variant. Verify frame rate and codebook count. |
| **Tokenizer format undocumented** | Cannot load language tokenizers | Low | Extract from NeMo source; tokenizer configs are JSON files in HF repo. |
| **Memory spike during AR decoding** | OOM on 16GB system | Low | KV cache is bounded by max utterance length (20s ≈ 430 frames). Pre-allocate. |
| **NVIDIA license restrictions** | Cannot distribute weights | Low | GGUF weights are MIT-licensed (m1el repo). NeMo weights require individual acceptance. |
| **Resampling inside synthesize_chunk** | Must match Supertonic's per-callback pattern | Medium | Verify Magpie produces chunked output via streaming callback, not batch-only. Test with `generate_with_config`-style callback. |

---

## Files Changed Summary

### Backend (Rust) — New Files

| File | Description |
|---|---|
| `services/tts/providers/magpie.rs` | `MagpieProvider` implementing `TtsProvider` trait — includes `synthesize_chunk()` with per-callback LPF + resampling |
| `services/tts/providers/magpie_engine.rs` | Core inference engine (GGML or ONNX-backed). If Path B: encoder, decoder with KV cache, local transformer, codec decoder. |
| `services/tts/providers/magpie_tokenizer.rs` | Per-language tokenizer loading and text encoding |

### Backend (Rust) — Modified Files

| File | Change |
|---|---|
| `core/settings.rs` | Add `TtsProviderConfig::Magpie` variant with all config fields |
| `services/tts/providers/mod.rs` | Add `Magpie` to `TtsProviderKind` enum — **no `native_sample_rate()` trait method** |
| `services/pipeline.rs` | Add Magpie match arm in `warm_up_tts()` (lines 259–267) — no DSP chain changes needed |
| `services/tts/providers/supertonic.rs` | No changes — resampling stays self-contained within `synthesize_chunk()` callback |
| `services/tts/actor.rs` | No changes needed — already provider-agnostic |

### Frontend (TypeScript) — Modified Files

| File | Change |
|---|---|
| `app/src/store/settingsStore.ts` (Zustand) | Add Magpie-specific settings fields |
| `app/src/pages/Settings.tsx` | Add TTS provider selector, Magpie config panel (language, speaker, quality) |
| `app/src-tauri/manifests/model-manifest.json` | Add Magpie weight entries with SHA256 |

### Documentation

| File | Change |
|---|---|
| `docs/plans/tts-options.md` | Update Magpie section with CPU validation results and integration status |
| `docs/models.md` | Add Magpie model entry |

---

## Execution Order & Dependencies

```
Phase 0 (Python Validation)
  │
  ├─ RTF < 0.8x? ──YES──► Phase 1 (Native Engine)
  │                              │
  │                              ├─ magpie.cpp repo public? ──YES──► Path A (FFI, ~1 week)
  │                              │                                      │
  │                              │                                      └──► Phase 2 (Rust Provider, ~3-5 days)
  │                              │                                              │
  │                              │                                              └──► Phase 3 (DSP, ~1-2 days)
  │                              │                                                      │
  │                              │                                                      └──► Phase 4 (Distribution, ~1-2 days)
  │                              │
  │                              └─ No magpie.cpp? ──► Path B (candle RS, ~3-4 weeks)
  │                                                          │
  │                                                          └──► Phase 2 → Phase 3 → Phase 4
  │
  └─ RTF > 1.0x? ──YES──► Path E (NIM API Cloud, ~1-2 days)
                                  │
                                  └──► Phase 2 (Skip Phase 1, Phase 3, Phase 4)
```

> [!IMPORTANT]
> **Phase 0 is the gating decision.** Do not start Phase 1-4 until Phase 0 benchmarks confirm CPU RTF < 0.8x. If local inference is not viable, skip directly to Path E (cloud fallback). All paths converge on Phase 2's `TtsProvider` trait integration.
>
> **Phase 3 is minimal** — resampling happens inside `MagpieProvider::synthesize_chunk()` via a per-callback LPF + linear interpolation, exactly like Supertonic's existing pattern. No DSP chain changes, no shared module, no trait modifications. 2–4 hours of work per provider.
>
> **No changes to `services/tts/actor.rs`** are required regardless of which path is chosen — the TTS worker is already provider-agnostic.
