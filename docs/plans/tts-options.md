Here's a comprehensive technical summary for integrating both engines into your Vox backend.

---

## NeuTTS: Complete Integration Guide

### Architecture & Variants

| Variant | Parameters | RAM (Q4) | Speed on CPU | Hindi Support | Best For |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Nano** | ~120M active | ~400-600 MB | Near real-time (RTF 0.8-1.2) | ✅ Multilingual | 8GB devices, speed-first |
| **Air** | ~360M active | ~1.2-1.8 GB | Slower than Nano | ✅ Multilingual | 16GB devices, quality-first |

Both use a 2-stage pipeline: LM Backbone → NeuCodec Decoder. Both support 3-second voice cloning. The key difference is parameter count—Air is ~3x larger, giving better prosody and expressiveness at the cost of speed and memory.

---

### Required Files

**For Nano:**
```bash
# GGUF Backbone (REQUIRED)
huggingface-cli download neuphonic/neutts-nano-q4-gguf \
    --local-dir ./models/neutts-nano

# ONNX Codec Decoder (OPTIONAL but recommended for speed)
huggingface-cli download neuphonic/neucodec-onnx-decoder \
    --local-dir ./models/neucodec-onnx
```

**For Air:**
```bash
# GGUF Backbone (REQUIRED)
huggingface-cli download neuphonic/neutts-air-q4-gguf \
    --local-dir ./models/neutts-air

# ONNX Codec Decoder (OPTIONAL but recommended)
huggingface-cli download neuphonic/neucodec-onnx-decoder \
    --local-dir ./models/neucodec-onnx
```

The ONNX decoder is shared between Nano and Air—you only need one copy.

---

### Rust Crate

**Crate:** `neutts`
**Latest version:** `0.3.0` (as of June 2026)
**Crates.io:** [https://crates.io/crates/neutts](https://crates.io/crates/neutts)
**GitHub:** [https://github.com/neuphonic/neutts](https://github.com/neuphonic/neutts)

**Cargo.toml:**
```toml
[dependencies]
neutts = "0.3"
```

---

### Integration Into Vox Actor-Engine Pattern

```rust
// src/services/tts/neutts_engine.rs

use neutts::{NeuTTS, SynthesisOptions, VoiceCloneSource};
use std::path::PathBuf;

pub struct NeuttsEngine {
    model: NeuTTS,
    sample_rate: u32, // 24000
}

impl NeuttsEngine {
    pub fn new(
        gguf_path: PathBuf,
        onnx_decoder_path: Option<PathBuf>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let model = NeuTTS::builder()
            .gguf_path(gguf_path)
            .onnx_decoder_path(onnx_decoder_path) // Optional, falls back to burn-import
            .num_threads(2) // CRITICAL: Limit threads to avoid CPU thrash
            .build()?;
        
        Ok(Self {
            model,
            sample_rate: 24000,
        })
    }
}

impl TtsEngine for NeuttsEngine {
    fn synthesize(&self, text: &str, lang: &str) -> Result<Vec<f32>, String> {
        let audio = self.model
            .synthesize(text, None)
            .map_err(|e| e.to_string())?;
        Ok(audio) // 24kHz f32 samples
    }

    fn synthesize_with_voice(
        &self,
        text: &str,
        lang: &str,
        reference_audio: &[f32],
    ) -> Result<Vec<f32>, String> {
        let source = VoiceCloneSource::from_samples(reference_audio, 24000);
        let options = SynthesisOptions {
            voice_clone_source: Some(source),
            ..Default::default()
        };
        let audio = self.model
            .synthesize_with_options(text, Some(options))
            .map_err(|e| e.to_string())?;
        Ok(audio)
    }
}
```

---

### Technical Integration Notes for NeuTTS

1. **Thread Allocation**: Allocate exactly **2 OS threads** via `.num_threads(2)`. This prevents CPU thrashing with your other inference workers (LLM, STT, VAD).

2. **Memory Budget Impact**:
   - Nano: +400-600 MB (fits easily in your 8GB budget)
   - Air: +1.2-1.8 GB (only viable on 16GB laptops)

3. **Output Format**: Returns 24kHz f32 mono PCM. Your existing `upsample_2x()` function (24kHz → 48kHz) works directly without modification.

4. **No Sherpa-ONNX Dependency**: NeuTTS uses its own GGUF + ONNX runtime, completely independent of Sherpa-ONNX. This means a new engine file (`neutts_engine.rs`) separate from your existing `kokoro_piper.rs`.

5. **Language Handling**: The multilingual backbone detects language automatically from text content. For Hindi, it recognizes Devanagari script (U+0900–U+097F), matching your existing `is_hindi()` detection logic.

6. **Streaming Support**: Use `self.model.stream_pcm(text, options)` to get an iterator of audio chunks, enabling your sub-sentence chunking pipeline.

---

## Qwen3-TTS: Complete Integration Guide

### Architecture & Variants

| Variant | Parameters | RAM (q8_0 GGUF) | Speed on CPU | Hindi Support | Best For |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **0.6B Base** | 600M | ~2.5-3 GB | RTF 1.3-2x (not real-time on laptop) | ✅ (English, Hindi, +9) | 16GB devices, maximum quality |
| **0.6B CustomVoice** | 600M | ~2.5-3 GB | Same as Base | ✅ | Production-ready voices |
| **1.7B** | 1.7B | ~6-8 GB | RTF 3-5x (very slow on CPU) | ✅ | ❌ Not viable for CPU-only |

Qwen3-TTS uses a 3-stage pipeline: Talker → CodePredictor (187.5 autoregressive steps/sec) → Vocoder. The CodePredictor is the bottleneck—it must run 187.5 steps for every second of generated audio, making it fundamentally slower than NeuTTS's 2-stage design.

---

### Required Files

**For 0.6B Base (Rust-ready via `qts` crate):**
```bash
# Pre-converted GGUF + ONNX bundle
huggingface-cli download dsh0416/Qwen3-TTS-12Hz-0.6B-Base-QTS \
    --local-dir ./models/qwen3-tts-0.6b

# Files you'll get:
# ./models/qwen3-tts-0.6b/qwen3-tts-0.6b-f16.gguf   (Transformer backbone)
# ./models/qwen3-tts-0.6b/qwen3-tts-vocoder.onnx    (12Hz vocoder)
```

**For 0.6B CustomVoice (also Rust-ready):**
```bash
huggingface-cli download dsh0416/Qwen3-TTS-12Hz-0.6B-CustomVoice-QTS \
    --local-dir ./models/qwen3-tts-0.6b-custom
```

**Note:** The `qts` crate currently only supports the 0.6B variants. If you want to experiment with 1.7B, you'd need to use the official Python library, which is not an option for Vox.

---

### Rust Crate

**Crate:** `qts`
**Latest version:** Check GitHub for latest commit (active development)
**GitHub:** [https://github.com/yet-another-ai/qts](https://github.com/yet-another-ai/qts)
**Crates.io:** Not yet published (git dependency required)

**Cargo.toml:**
```toml
[dependencies]
qts = { git = "https://github.com/yet-another-ai/qts" }
```

**Build Requirements:**
```bash
git clone https://github.com/yet-another-ai/qts.git
cd qts
git submodule update --init --recursive  # Pulls ggml, onnxruntime, etc.
```

---

### Integration Into Vox Actor-Engine Pattern

```rust
// src/services/tts/qwen_tts_engine.rs

use qts::{Qwen3Tts, SynthesisOptions, VoiceCloneSource};
use std::path::PathBuf;

pub struct Qwen3TtsEngine {
    model: Qwen3Tts,
    sample_rate: u32, // 24000
}

impl Qwen3TtsEngine {
    pub fn new(
        model_dir: PathBuf,
        num_threads: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let gguf_path = model_dir.join("qwen3-tts-0.6b-f16.gguf");
        let vocoder_path = model_dir.join("qwen3-tts-vocoder.onnx");
        
        let model = Qwen3Tts::builder()
            .gguf_path(gguf_path)
            .vocoder_path(vocoder_path)
            .num_threads(num_threads) // 3-4 threads recommended for this model
            .build()?;
        
        Ok(Self {
            model,
            sample_rate: 24000,
        })
    }
}

impl TtsEngine for Qwen3TtsEngine {
    fn synthesize(&self, text: &str, lang: &str) -> Result<Vec<f32>, String> {
        let options = SynthesisOptions {
            language: Some(lang.to_string()), // "English" or "Hindi"
            temperature: 0.8,
            ..Default::default()
        };
        let audio = self.model
            .synthesize_with_options(text, Some(options))
            .map_err(|e| e.to_string())?;
        Ok(audio)
    }

    fn synthesize_with_voice(
        &self,
        text: &str,
        lang: &str,
        reference_audio: &[f32],
    ) -> Result<Vec<f32>, String> {
        let source = VoiceCloneSource::from_samples(reference_audio, 24000);
        let options = SynthesisOptions {
            language: Some(lang.to_string()),
            voice_clone_source: Some(source),
            temperature: 0.8,
            ..Default::default()
        };
        let audio = self.model
            .synthesize_with_options(text, Some(options))
            .map_err(|e| e.to_string())?;
        Ok(audio)
    }
}
```

---

### Technical Integration Notes for Qwen3-TTS

1. **Thread Allocation**: Allocate **3-4 OS threads** for this model. The CodePredictor's 187.5 autoregressive steps per second of audio are the bottleneck. On a Ryzen 9 7950X, this achieves ~RTF 1.3x. On a laptop i5, expect RTF 2-3x.

2. **Memory Budget Impact**: The 0.6B model at q8_0 precision requires ~2.5-3 GB RAM. This pushes your 8GB device dangerously close to the 5.5GB inference budget (VAD 0.05 + STT 0.80 + LLM 2.20 + Qwen3 2.50 = 5.55 GB). Only viable on 16GB laptops.

3. **Language Parameter is Required**: Unlike NeuTTS, Qwen3-TTS requires explicit language specification. Pass `"Hindi"` or `"English"` based on your existing `is_hindi()` detection function.

4. **Output Format**: Returns 24kHz f32 mono PCM. Compatible with your existing `upsample_2x()` function.

5. **No Sherpa-ONNX Dependency**: Like NeuTTS, this uses its own ggml + ONNX runtime. Create a separate `qwen_tts_engine.rs` file.

6. **Streaming Support**: Qwen3-TTS supports chunked streaming output. Use `self.model.stream_synthesize(text, options)` to get incremental audio chunks for your sub-sentence pipeline.

7. **CustomVoice vs Base**: If you want pre-defined high-quality voices (Vivian, etc.), use the CustomVoice variant. The Base variant is better for voice cloning from reference audio.

---

## Comparison Matrix for Your Threading Model

Here's how each engine impacts your existing thread allocation strategy:

| Engine | OS Threads Needed | RAM Cost | RTF on i5 Laptop | Fits 8GB Budget? |
| :--- | :--- | :--- | :--- | :--- |
| Kokoro (current) | 1 | ~200 MB | 0.03-0.10x | ✅ Yes |
| Piper (current) | 1 | ~100 MB | 0.05-0.15x | ✅ Yes |
| **NeuTTS Nano** | 2 | ~400-600 MB | 0.8-1.2x | ✅ Yes (barely) |
| **NeuTTS Air** | 2 | ~1.2-1.8 GB | 1.5-2.5x | ❌ No |
| **Qwen3-TTS 0.6B** | 3-4 | ~2.5-3 GB | 2-3x | ❌ No |
| **Qwen3-TTS 1.7B** | 4-6 | ~6-8 GB | 3-5x | ❌ Absolutely not |

---

## Recommended File Organization

For your `src/services/tts/` directory, here's how to structure the new engines:

```
src/services/tts/
├── mod.rs                   # Module entry, engine selection
├── actor.rs                 # Command/Event handler (unchanged)
├── kokoro_piper.rs          # Existing Sherpa-ONNX engines (unchanged)
├── neutts_nano.rs           # NEW: NeuTTS Nano engine
├── neutts_air.rs            # NEW: NeuTTS Air engine
└── qwen_tts.rs              # NEW: Qwen3-TTS engine
```

Your `actor.rs` command handler can then dispatch to the appropriate engine based on user settings:

```rust
pub enum TtsEngineType {
    Kokoro,       // Speed-first, UI sounds
    Piper,        // Hindi speed-first
    NeuttsNano,   // Quality for 8GB devices
    NeuttsAir,    // Quality for 16GB devices
    Qwen3Tts,     // Maximum quality for 16GB devices
}
```

---

## Final Testing Strategy

Since you want to test both Nano and Air:

1. **Download both models** to separate directories.
2. **Create two engine files**: `neutts_nano.rs` and `neutts_air.rs`, both implementing the same `TtsEngine` trait.
3. **Benchmark both** with your existing benchmark harness:
   - RTF on your Ryzen 9 7950X (training server)
   - RTF on your i5-1145G7 (target 8GB device)
   - RAM usage (RSS) during synthesis
   - Subjective quality for Hindi and Hinglish
4. **Make the call** based on actual numbers from your hardware.

Would you like me to provide the exact benchmark harness code to measure RTF and RSS for each engine, integrating with your existing Vox monitoring infrastructure?