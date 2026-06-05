# Vox — Model Architecture & Selection (Native Edge Stack)

---

## 1. Overview

Vox is a **model-agnostic, role-based system**, constrained by:

* **8GB RAM baseline** (2026 hardware)
* **CPU-only execution** (no GPU acceleration)
* **Accuracy-first output** — correct transcription and coherent responses are non-negotiable
* **Native C++ inference** (ONNX Runtime + llama.cpp)

---

## 2. Core Model Roles

The system is architected around **four specialized roles**:

1. **VAD (Voice Activity Detection)** — Speech/silence classification
2. **STT (Speech-to-Text)** — Audio transcription
3. **LLM (Large Language Model)** — Reasoning + response generation
4. **TTS (Text-to-Speech)** — Audio synthesis

Each role has **strict memory + latency constraints** and is **replaceable** without affecting others.

---

## 3. Selection Philosophy

### Native-First Execution

**ALL inference runs in C++**, never Python:

```rust
// ONNX Runtime (C++)
use sherpa_onnx::{OfflineRecognizer, VadModelConfig};

// llama.cpp (C++)
use llama_cpp_2::{model::LlamaModel, context::LlamaContext};
```

**Python is completely absent** from the inference path.

### Accuracy First — Always

> Accuracy → Memory → Speed. In that exact order.

* **Accuracy is non-negotiable**: A wrong or truncated answer is a system failure, not a tradeoff.
* **Speed is a result**: The system should be as fast as it can be *while* being accurate.
* Every component is tuned to maximize output quality within resource constraints — not to minimize milliseconds at the cost of correctness.

**What this rules out:**
- `max_new_tokens` values too small to fully decode real speech (causes truncation)
- TTS chunk sizes so small they produce robotic 1–2 word utterances
- Any config where the sherpa-onnx truncation warning is accepted as normal

### Memory-Constrained Design

**Total budget: ~5.5GB usable** (of 8GB system RAM)

```text
OS + UI overhead: ~2.5GB
Available for models: ~5.5GB
Safety margin: 1.35GB (prevents OS swap)
```

**Hard rules:**
- No model >3B parameters (unless heavily quantized)
- FP16/FP32 inference forbidden
- INT8/INT4 quantization only
- CPU threads: N-2 (reserve for audio + VAD)

---

## 4. Voice Activity Detection (VAD)

### Selected Model: **TenVAD (ONNX)**

```rust
let config = VadModelConfig {
    ten_vad: TenVadModelConfig {
        model: Some("ten_vad.onnx"),
        threshold: 0.45,  // Configurable
        min_silence_duration: 0.5,
        min_speech_duration: 0.25,
    },
    sample_rate: 16000,
    num_threads: 1,  // Single-threaded for consistency
};
```

### Why TenVAD?

| Metric | Value | Why Important |
|--------|-------|---------------|
| Memory | ~306KB | Minimal RAM footprint |
| RTF | ~0.015 | Near-zero latency |
| End Delay | ~0ms | Critical for real-time UX |
| Accuracy | High | Robust speech detection |

### Behavior Implementation

```rust
// Speech detection logic
if detected && !in_speech {
    in_speech = true;
    // Emit SpeechStart event immediately
    pipeline_tx.send(VoxEvent::SpeechStart { turn_id, owner });
}

// Silence detection
if !detected && in_speech {
    in_speech = false;
    // Send final audio buffer to STT
    stt_tx.send(SttCommand::Final(turn_id, owner, buffer));
}
```

### Why NOT Silero VAD?

* Introduces 300-500ms delay
* Breaks real-time interaction loop
* Higher memory usage (~2MB)

---

---

## 4.1 Future VAD Roadmap (To Be Integrated)

### **Earshot**
* **Role**: Ultra-low latency acoustic event detection and streaming ASR.
* **Status**: Researching integration for multi-turn context awareness.

## 5. Speech-to-Text (STT)

### Selected Model: **Qwen3-ASR-0.6B (INT8 ONNX)**

```rust
let config = OfflineRecognizerConfig {
    model_config: OfflineQwen3ASRModelConfig {
        conv_frontend: Some("conv_frontend.onnx"),
        encoder: Some("encoder.int8.onnx"),
        decoder: Some("decoder.int8.onnx"),
        tokenizer: Some("tokenizer"),
        max_total_len: 2048,
        max_new_tokens: 512,
    },
    num_threads: 2,  // Balanced for 8GB systems
};
```

### Why Qwen3-ASR?

| Metric | Value | Why Important |
|--------|-------|---------------|
| Memory | ~800MB | Fits within budget |
| Accuracy | High | Native Hinglish support — the primary evaluation criterion |
| Streaming | Yes | Enables live partial transcription in UI |
| Latency | RTF ~2.0 on CPU | Acceptable for complete, accurate output |

### Key Advantages

1. **Multilingual Native**: Handles code-switching (English ↔ Hindi) without separate models
2. **Streaming Architecture**: Encoder state caching enables real-time partial transcripts
3. **CPU Optimized**: Designed for edge deployment

### Critical Configuration (ACCURACY MANDATE)

The STT engine is the most critical accuracy point. Misconfiguration here corrupts every downstream output.

```rust
let config = OfflineRecognizerConfig {
    model_config: OfflineQwen3ASRModelConfig {
        conv_frontend: Some("conv_frontend.onnx"),
        encoder: Some("encoder.int8.onnx"),
        decoder: Some("decoder.int8.onnx"),
        tokenizer: Some("tokenizer"),
        max_total_len: 2048,
        // CRITICAL: Must be large enough to decode without truncation.
        // 64 caused "Result is truncated" warnings for any utterance > ~2s.
        // 512 handles up to ~30s of natural speech at normal speaking pace.
        // NEVER reduce this to gain speed — truncated = garbage output.
        max_new_tokens: 512,
    },
    num_threads: 4,  // Use all available perf cores for faster decode
};
```

**The sherpa-onnx warning `"Result is truncated. max_new_tokens X is too small"` is a hard bug — not acceptable.**

### Streaming Strategy

```rust
// Overlapping audio window for streaming partial transcripts
// Partials are UI feedback only — final transcript is authoritative
if samples_since_partial >= 12800 {
    let window_start = buffer.len().saturating_sub(240000); // 15s
    stt_tx.send(SttCommand::Partial(turn_id, owner, buffer[window_start..]));
}
```

**Partial transcripts are for live UI feedback only.** The final transcript must be the complete, accurate decode of the full utterance.

### Why NOT Whisper?

| Issue | Impact |
|-------|--------|
| Fixed 30s window | High latency, inefficient |
| CPU performance | 5-10x slower on same hardware |
| Memory usage | ~1.5GB (exceeds budget) |

### Why NOT Moonshine?

* Limited Hinglish support
* Less robust multilingual handling
* Higher latency on CPU



---

## 6. Language Model (LLM)

### Selected Model: **Gemma 4 E2B-it (GGUF, Q4_K_M)**

```rust
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(0); // CPU-only

let context_params = LlamaContextParams::default()
    .with_n_ctx(2048)
    .with_n_threads(n_threads);
```

### Why Gemma 4?

| Metric | Value | Why Important |
|--------|-------|---------------|
| Memory | ~2.2GB | Largest single allocation |
| Context | 4096 tokens | Sufficient for conversations |
| Reasoning | State-of-the-art (2026) | Superior instruction following |
| Multilingual | Enhanced Hindi/Hinglish | Critical for target market |

### Prompt Format (Gemma 4)

```rust
let prompt = format!(
    "<|turn>system {}\n<turn|>\n<|turn>user {}\n<turn|>\n<|turn>model\n",
    system_prompt, user_text
);
```

### Token Streaming Implementation

```rust
loop {
    // Atomic cancellation check
    if cancel_flag.load(Ordering::Relaxed) {
        break; // Immediate abort
    }

    let token = ctx.sample_greedy();
    if is_eog_token(token) { break; }

    let token_str = model.token_to_piece(token)?;
    if !token_str.is_empty() {
        // Emit for real-time UI updates
        tx.send(VoxEvent::LlmToken { turn_id, token })?;
    }
}
```

### Why NOT Qwen2.5-3B?

* Superior multilingual reasoning
* Better Hinglish generation
* But: Higher memory usage (~3.5GB), exceeds budget

### Context Management

```rust
// Explicit KV cache clearing (Directive: Memory Safety)
ctx.clear_kv_cache();

// Context limits prevent explosion
if n_cur >= ctx_size { break; }
```

---

## 6.1 Future LLM Roadmap (To Be Integrated)

### **Gemma 4 E4B-it**
* **Role**: Larger reasoning backbone for complex tasks.
* **Constraint**: Likely requires 16GB RAM or high-compression quantization (IQ2_XS).

---

## 7. Text-to-Speech (TTS)

### Selected Models: **Dual-Model Routing**

#### English: Kokoro-82M (ONNX)
#### Hindi: Piper VITS (ONNX)

```rust
// English TTS
let en_config = OfflineTtsConfig {
    model: OfflineTtsModelConfig {
        kokoro: OfflineTtsKokoroModelConfig {
            model: Some("kokoro/model.onnx"),
            voices: Some("kokoro/voices.bin"),
            tokens: Some("kokoro/tokens.txt"),
        },
    },
};

// Hindi TTS
let hi_config = OfflineTtsConfig {
    model: OfflineTtsModelConfig {
        vits: OfflineTtsVitsModelConfig {
            model: Some("piper/hi_IN-priyamvada-medium.onnx"),
        },
    },
};
```

### Why Dual-Model?

| Language | Model | Memory | Quality | Latency |
|----------|-------|--------|---------|---------|
| English | Kokoro | ~150MB | State-of-the-art | <200ms |
| Hindi | Piper | ~100MB | High quality | <300ms |

### Language Detection & Routing

```rust
fn is_hindi(text: &str) -> bool {
    text.chars().any(|c| c >= '\u{0900}' && c <= '\u{097F}')
}

// Route based on Unicode ranges
let tts_instance = if is_hindi(text) { &hi_tts } else { &en_tts };
```

### Chunked Synthesis (Accuracy-Quality Mandate)

**Goal**: Natural, complete utterances — not choppy word fragments.

Flush to TTS on sentence/clause boundaries that produce coherent speech:

```rust
fn should_flush(buf: &str, word_count: usize, elapsed_ms: u128) -> bool {
    // Hard boundaries (immediate flush — always correct sentence unit)
    if matches!(last_char, '.' | '!' | '?') { return true; }

    // Soft boundaries (clause-level — still natural)
    if matches!(last_char, ',' | ';') { return true; }
    if buf.ends_with(" — ") || buf.ends_with(" - ") { return true; }

    // Time-based gateway: ONLY if we have enough words for natural speech
    // 800ms/1-word caused 1-2 word utterances = robotic audio
    if word_count >= 3 && elapsed_ms > 1500 { return true; }

    // Word count fallback: minimum viable sentence for TTS
    word_count >= 8
}
```

**Never flush on 1–2 words** unless a hard sentence boundary is present.
Short TTS chunks produce robotic, staccato audio that destroys the conversational UX.

Target: **Time-to-First-Audio that sounds natural** — not the shortest possible TTFA.

### Why NOT Fish Speech (4B)?

* Exceeds memory budget (4GB+)
* Requires GPU acceleration
* >5s latency on CPU

### Why NOT Qwen3-TTS?

* No Hindi support
* Poor CPU performance
* Higher memory usage

---

## 7.1 Future TTS Roadmap (To Be Integrated)

### **NeuTTS**
* **Role**: Emotional and expressive synthesis for personalized AI persona.
* **Status**: Awaiting ONNX export stability for native C++ inference.

---

## 8. Memory Budget Allocation

### Per-Component Breakdown

```text
Total Available: 5.5GB
├── VAD:        0.05GB (306KB)
├── STT:        0.80GB (Qwen3-ASR)
├── LLM:        2.20GB (Gemma 4)
├── TTS:        0.25GB (Kokoro + Piper)
├── KV Cache:   0.60GB (LLM context)
├── Audio Buffers: 0.10GB (Ring buffers)
└── Safety Margin: 1.50GB
```

### Memory Safety Rules

1. **Never exceed ceiling** → prevents OS swap
2. **Explicit cleanup** → `clear_kv_cache()` after each turn
3. **Buffer limits** → ring buffers overflow-drop with logging
4. **Lazy loading** → models loaded only when needed

---

## 9. Threading & Performance

### Thread Allocation Strategy

```rust
let total_cores = num_cpus::get();
let llm_threads = total_cores.saturating_sub(2); // Reserve for audio + VAD

// Thread priorities (Linux/macOS)
set_current_thread_priority(ThreadPriority::Max);        // Audio callback
set_current_thread_priority(ThreadPriority::High);       // VAD worker
set_current_thread_priority(ThreadPriorityValue::from(80u8).unwrap()); // STT worker
```

### Real-Time Constraints

| Component | Quality Target | Current Implementation |
|-----------|----------------|----------------------|
| VAD | Accurate onset/offset detection | Frame-level detection |
| STT | Complete, untruncated decode | max_new_tokens=512, 4 threads |
| LLM | Coherent, complete responses | Token-by-token streaming |
| TTS | Natural, sentence-level utterances | Clause-boundary chunking |
| **Mandate** | **Accuracy First** | **No latency target overrides accuracy** |

### Performance Optimizations

1. **Lock-free communication**: Ring buffers, atomics, channels
2. **Zero-allocation callbacks**: Pre-allocated buffers in audio path
3. **Streaming everywhere**: No batch processing
4. **Cancellation support**: Atomic flags checked per token/chunk

---

## 10. Model Lifecycle Management

### Loading Strategy

```rust
enum LoadState {
    Cold,  // Not loaded
    Warm,  // Loaded and ready
}

// Lazy loading with pre-warm option
if pre_load || first_use {
    let engine = SttEngine::new(&model_path)?;
    is_loaded.store(true, Ordering::Relaxed);
}
```

### Auto-Sleep (Phase 5)

```rust
if last_interaction.elapsed() > auto_sleep_timeout {
    cool_down_llm();  // Drop LLM model
    cool_down_tts();  // Drop TTS models
    // Save ~2.5GB RAM
}
```

### Hot-Reloading

```rust
// VAD threshold updates without restart
match cmd {
    VadCommand::UpdateThreshold(v) => {
        // Hot-reload detector with new threshold
        let new_detector = VadEngine::create_detector(&model_path, v);
        self.detector = new_detector;
    }
}
```

---

## 11. Settings Integration

### Model Selection

```typescript
interface ModelSettings {
    // VAD
    vad_threshold: number;      // 0.0-1.0
    ptt_noise_gate: number;     // 0.0-1.0

    // STT
    asr_model: string;          // "qwen3-asr"

    // LLM
    llm_model: string;          // "gemma4"
    llm_ctx_size: number;       // 1024-4096
    llm_threads: number;        // 1-N

    // TTS
    en_model: string;           // "kokoro"
    en_voice: number;           // 0-10
    hi_model: string;           // "piper_hi"
    hi_voice: string;           // "hi_IN-priyamvada-medium.onnx"
}
```

### Reload Policies

```rust
pub fn reload_policy_for(domain: &str, key: &str) -> SettingReloadPolicy {
    match (domain, key) {
        ("vad", "threshold") => WorkerCommand,     // Hot-update VAD
        ("llm", "model") => Restart,               // Full restart required
        ("tts", "en_voice") => Hot,                // Instant voice change
        // ... etc
    }
}
```

---

## 12. Error Handling & Resilience

### Model Load Failures

```rust
match SttEngine::new(&model_path) {
    Ok(engine) => {
        is_loaded.store(true, Ordering::Relaxed);
        Some(engine)
    },
    Err(e) => {
        log::error!("[STT] Load failed: {}", e);
        // Continue without STT (graceful degradation)
        None
    }
}
```

### Inference Timeouts

```rust
// Atomic cancellation flags
if cancel_flag.load(Ordering::Relaxed) {
    // Abort inference immediately
    return Ok(());
}
```

### Memory Pressure

```rust
// Monitor available RAM
if available_memory < SAFETY_MARGIN {
    // Trigger auto-sleep or model offloading
    cool_down_llm();
}
```



---

## 16. Final Architecture Principles

### 1. Hardware-Constrained Design

Models are bounded by **physics, not capability**. Every component is chosen to fit within:

- **Memory**: 8GB system constraint
- **CPU**: Available performance cores
- **Accuracy**: Output must be correct — this is the primary constraint

### 2. Native Performance

**Zero Python overhead** in inference path. All heavy computation happens in optimized C++:

- ONNX Runtime for neural networks
- llama.cpp for transformer models
- Sherpa-ONNX for audio processing

### 3. Streaming-First Architecture

**No batch processing**. Everything streams:

- Audio chunks → VAD → STT partials (UI feedback) + STT final (authoritative)
- LLM tokens → TTS chunks (sentence-level, not word-level)
- Parallel execution for natural, responsive output

### 4. Resilience & Safety

- **Memory-safe**: Explicit limits and monitoring
- **Cancellation-support**: Atomic flags for clean shutdowns
- **Graceful degradation**: Continue operation even if components fail

### 5. Future-Proof Modularity

Each model role is **replaceable** without affecting others. New models can be swapped in by:

1. Updating model files
2. Adjusting configuration
3. Minimal code changes (ideally zero)

This architecture ensures Vox can evolve with ML progress while maintaining its **accuracy-first, local-first** edge deployment requirements.