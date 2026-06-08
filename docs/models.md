# Vox — Model Architecture & Selection (Native Edge Stack)

---

## 1. Overview & Default Stack

Vox is a **model-agnostic, role-based system** with a selection strategy that is **hardware-constrained and user-dependent**. 

* **Hardware constraints**: While the baseline target is **8GB RAM systems** running on CPU, users with higher-end hardware (e.g. 16GB+ RAM) can select larger and more powerful reasoning backbones.
* **Accuracy-first output**: Coherent, correct transcription and response generation are non-negotiable.
* **Native C++ inference**: ONNX Runtime + llama.cpp.

### Default Model Set at a Glance

| Pipeline Stage | Model / Engine | ID | Footprint | Key Config / Parameter |
| :--- | :--- | :--- | :---: | :--- |
| **VAD** | Earshot VAD | `earshot` | < 1 MB | Threshold: `0.5`, sample rate `16000` |
| **STT** (Primary) | **Nvidia Nemotron-3.5** | `nvidia_nemotron` | ~720 MB | INT8 Quantized, FastConformer-RNNT (`parakeet-rs`) |
| **STT** (Fallback) | Qwen3-ASR-0.6B | `qwen3_asr` | ~800 MB | INT8 Quantized (`sherpa-onnx`) |
| **LLM** (Default) | Llama 3.2 1B Instruct | `llama_3_2_reasoning` | ~1.0 GB | GGUF Q6_K, context size `2048`, threads `N-2` |
| **LLM** (Alternative) | Gemma 4 E2B-it | `gemma_4_reasoning` | ~2.2 GB | GGUF Q4_K_M |
| **TTS** (Sole) | **Supertonic 3** | `supertonic_tts` | ~144 MB | INT8 Quantized, sherpa-onnx native, 31 languages, 10 voices |

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

**ALL inference runs in C++ or Rust**, never Python:

```rust
// parakeet-rs / ONNX Runtime (C++)
use parakeet_rs::Nemotron;

// llama.cpp (C++)
use llama_cpp_2::{model::LlamaModel, context::LlamaContext};
```

**Python is completely absent** from the inference path.

### Accuracy First — Always

> Accuracy → Memory → Speed. In that exact order.

* **Accuracy is non-negotiable**: A wrong or truncated answer is a system failure, not a tradeoff.
* **Speed is a result**: The system should be as fast as it can be *while* being accurate.
* Every component is tuned to maximize output quality within resource constraints.

### Memory & Hardware-Dependent Scaling

* **Baseline budget: ~5.5GB usable** (of 8GB system RAM)
  ```text
  OS + UI overhead: ~2.5GB
  Available for models: ~5.5GB
  Safety margin: 1.35GB (prevents OS swap)
  ```
* **Higher-End Systems**: If a system has 16GB+ RAM, the model constraints lift, permitting the choice of larger LLMs (like Gemma 4 E4B-it) and higher-fidelity voices.

---

## 4. Voice Activity Detection (VAD)

### Default Model: **Earshot (Rust-native)**
Vox uses **Earshot VAD** as its primary voice activity detector. It features sub-millisecond voice detection, zero ONNX overhead, and executes entirely in native Rust.

### Legacy Option: **TenVAD (ONNX)**

```rust
let config = VadModelConfig {
    ten_vad: TenVadModelConfig {
        model: Some("ten_vad.onnx"),
        threshold: 0.50,  // Configurable
        min_silence_duration: 0.5,
        min_speech_duration: 0.25,
    },
    sample_rate: 16000,
    num_threads: 1,
};
```

---

## 5. Speech-to-Text (STT)

### Default Primary Engine: **Nvidia Nemotron-3.5 (INT8 quantized, parakeet-rs)**

Nemotron-3.5 runs via `parakeet-rs`, integrating an INT8 quantized FastConformer-RNNT model for state-of-the-art Hindi/Hinglish speech decoding on CPU with an RTF of ~0.50.

```rust
// Load the FastConformer-RNNT model structure
let model = Nemotron::from_pretrained(&model_dir, None)
    .map_err(|e| anyhow!("Failed to load Nemotron: {:?}", e))?;
```

### Fallback Option: **Qwen3-ASR-0.6B (INT8 ONNX, sherpa-onnx)**

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
    num_threads: 4,
};
```

### Why Nemotron-3.5?

| Metric | Nvidia Nemotron-3.5 (INT8) | Qwen3-ASR (Fallback) | Why Important |
| :--- | :---: | :---: | :--- |
| **Real-Time Factor (RTF)** | **0.50** 🚀 | **6.25** 🐌 | Nemotron is **12.5x faster** on CPU. |
| **Accuracy** | **State-of-the-art** | High (but loops easily) | Zero hallucination loop issues. |
| **RAM footprint** | **~1.2 GB** | **~800 MB** | Extremely comparable RAM for vastly better quality. |

### Streaming Strategy

* **Nemotron-3.5** processes streaming audio chunks using stateful FastConformer strides (560ms / 8960 samples stride).
* **Qwen-ASR** uses a rolling overlap window (15s).

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

### Selected Model: **Supertonic 3 (Sole Engine)**

Supertonic 3 is the sole TTS engine — a single unified 99M-parameter flow-matching model supporting **31 languages** with **10 voices** (5 male, 5 female). It uses sherpa-onnx native `OfflineTtsSupertonicModelConfig` and is INT8 quantized (~144MB).

| Feature | Detail |
|---------|--------|
| Architecture | Flow-matching transformer |
| Parameters | 99M |
| Quantization | INT8 |
| Footprint | ~144 MB |
| Languages | 31 (English, Hindi, and 29 more) |
| Voices | 10 (James, David, Alex, Ryan, Ethan, Sophia, Olivia, Emma, Ava, Mia) |
| Quality Steps | 2–12 (Speed→Quality→Best) |
| Speed | 0.7x–2.0x |
| Inference | sherpa-onnx native (OfflineTtsSupertonicModelConfig) |

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

## 7.2 Future Cloud & Realtime Voice API Roadmap (Planned)

To complement the local-first execution stack for resource-constrained or highly interactive requirements, future phases will introduce:

### **Hybrid Cloud Model Pipeline**
* **Cloud Fallback**: Route complex queries to large frontier reasoning models via API endpoints when local hardware is under heavy thermal throttling or memory pressure.
* **Low-Latency Streaming APIs**: Support ultra-low latency cloud STT/TTS bridges as optional alternatives.

### **Direct Voice-to-Voice Realtime APIs**
* **Native Audio Streams**: Route audio buffers directly to native voice-to-voice models (e.g., Gemini Realtime API, OpenAI Realtime API) bypassing the discrete STT → LLM → TTS chain. This minimizes intermediate processing latency and preserves emotional nuances in human voice.

---

## 8. Memory Budget Allocation

### Per-Component Running Memory (Active Footprint)

```text
Total Active Budget: ~3.0GB (on 8GB Baseline)
├── VAD:        < 0.01GB (Earshot VAD)
├── STT:        ~1.25GB (Nvidia Nemotron-3.5)
├── LLM:        ~1.00GB (Llama 3.2 1B Instruct)
├── TTS:        ~0.25GB (Kokoro + Piper)
├── Audio Buffers: ~0.10GB (Pre-allocated ring buffers)
└── Safety Margin: ~1.90GB (Safety headroom to prevent OS swap)
```

### Memory Safety Rules

1. **Never exceed ceiling** → prevents OS swap.
2. **Explicit cleanup** → `clear_kv_cache()` after each turn.
3. **Buffer limits** → ring buffers overflow-drop with logging.
4. **Lazy loading & Auto-sleep** → models loaded only when needed; inactive models unloaded automatically.

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
| VAD | Accurate onset/offset detection | Frame-level Earshot detection |
| STT | Complete, untruncated decode | Nemotron-3.5 FastConformer-RNNT |
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

### Auto-Sleep

```rust
if last_interaction.elapsed() > auto_sleep_timeout {
    cool_down_llm();  // Drop LLM model
    cool_down_tts();  // Drop TTS models
    // Save ~1.5GB RAM
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
    asr_model: string;          // "nvidia_nemotron" | "qwen3_asr"

    // LLM
    llm_model: string;          // "llama_3_2_reasoning" | "gemma_4_reasoning"
    llm_ctx_size: number;       // 1024-4096
    llm_threads: number;        // 1-N

    // TTS
    en_model: string;           // "kokoro_english_tts"
    en_voice: number;           // 0-10
    hi_model: string;           // "piper_hindi_tts"
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

## 13. Final Architecture Principles

### 1. Hardware-Constrained & Scalable Design

Models are bounded by **physics, not capability**. Running parameters scale dynamically according to system RAM capacity:

- **Baseline**: Bounded to fit cleanly inside 8GB RAM.
- **High performance**: Automatically allocates larger contexts and higher-fidelity parameters for 16GB+ systems.
- **Accuracy**: Output correctness remains the absolute constraints.

### 2. Native Performance

**Zero Python overhead** in inference path. All heavy computation happens in optimized C++ or native Rust:

- ONNX Runtime for neural networks.
- llama.cpp for GGUF transformer models.
- parakeet-rs for streaming speech recognition.

### 3. Streaming-First Architecture

**No batch processing**. Everything streams:

- Audio chunks → VAD → STT partials (UI feedback) + STT final (authoritative)
- LLM tokens → TTS chunks (sentence-level, not word-level)
- Parallel execution for natural, responsive output

### 4. Future-Proof Modularity

Each model role is **replaceable** without affecting others. Local-first execution is future-proofed with hooks designed for hybrid cloud fallback models and direct voice-to-voice realtime connections.

This architecture ensures Vox can evolve with ML progress while maintaining its **accuracy-first, local-first** edge deployment requirements.