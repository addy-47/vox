# Vox — Model Architecture & Selection (Native Edge Stack)

---

## 1. Overview & Default Stack

Vox is a **model-agnostic, role-based system** with a selection strategy that is **hardware-constrained and user-dependent**.

- **Hardware constraints**: Baseline target is **8GB RAM systems** running on CPU. Users with higher-end hardware (e.g. 16GB+ RAM) can select larger models.
- **Accuracy-first output**: Coherent, correct transcription and response generation are non-negotiable.
- **Native C++ inference**: ONNX Runtime + llama.cpp + parakeet-rs.

### Default Model Set at a Glance

| Pipeline Stage | Model / Engine | ID | Footprint | Key Config / Parameter |
| :--- | :--- | :--- | :---: | :--- |
| **VAD** | Earshot VAD | `earshot` | < 1 MB | Threshold: `0.5`, sample rate `16000` |
| **STT** (Primary) | **Nvidia Nemotron-3.5** | `nvidia_nemotron` | ~2.5 GB | INT8 Quantized, FastConformer-RNNT (`parakeet-rs`) |
| **STT** (Fallback) | Qwen3-ASR-0.6B | `qwen3_asr` | ~800 MB | INT8 Quantized (`sherpa-onnx`) |
| **LLM** (Default) | Llama 3.2 1B Instruct (Q4) | `llama_3_2_reasoning_q4` | ~750 MB | GGUF Q4_K_M, context size `2048`, threads `4` |
| **LLM** (Higher quality) | Llama 3.2 1B Instruct (Q6) | `llama_3_2_reasoning` | ~1.0 GB | GGUF Q6_K, context size `2048` |
| **LLM** (Alternative) | Gemma 4 E2B-it | `gemma_4_reasoning` | ~1.4 GB | GGUF Q4_K_M, context size `4096` |
| **LLM** (Uncensored) | Gemma 4 Uncensored | `gemma_4_uncensored` | ~2.9 GB | GGUF Q2_K_P, unrestricted output |
| **LLM** (Cloud)* | OpenAI / Gemini / Anthropic | provider-configurable | 0 MB (local) | Uses `OpenAiCompatProvider` with API key |
| **TTS** (Primary) | **Supertonic 3** | `supertonic_tts` | ~144 MB | INT8, sherpa-onnx, 31 languages, 10 voices |
| **TTS** (Local clone) | **Chatterbox Local** | `chatterbox_tts` | ~1.1 GB | 340M Q4 GGML, voice cloning from 5s reference |
| **TTS** (Remote) | **Chatterbox Remote** | `chatterbox_remote` | 0 MB (local) | Offload to remote CUDA GPU server |

> \* Cloud LLM options are available via `OpenAiCompatProvider` — see §6.2.

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

### Cloud vs. Local Tradeoffs

Alongside the native-local stack, Vox's provider architecture supports **cloud LLM inference** as an alternative:

| Aspect | Local (EmbeddedProvider) | Cloud (OpenAiCompatProvider) |
| :--- | :--- | :--- |
| **Data privacy** | No data leaves the device | Requires API key — data sent to provider |
| **Memory impact** | ~750 MB–1.4 GB allocated | Zero local memory for inference |
| **Model quality** | Constrained by 5.5 GB budget | Access to frontier models (GPT-4o, Gemini 2.5 Pro, Claude 4) |
| **Internet** | Not required | Required |
| **Cost** | No API costs | Per-token pricing |
| **Latency** | Sub-500 ms pipeline target | Depends on network + provider |

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

- **Accuracy is non-negotiable**: A wrong or truncated answer is a system failure, not a tradeoff.
- **Speed is a result**: The system should be as fast as it can be *while* being accurate.
- Every component is tuned to maximize output quality within resource constraints.

### Memory & Hardware-Dependent Scaling

- **Baseline budget: ~5.5GB usable** (of 8GB system RAM). These are design targets, not enforced runtime limits.
  ```text
  OS + UI overhead: ~2.5GB
  Available for models: ~5.5GB (design target)
  ```
- **Higher-End Systems**: 16GB+ RAM permits larger LLMs (Gemma 4) and higher-fidelity voices.

---

## 4. Voice Activity Detection (VAD)

### Default Model: **Earshot (Rust-native, energy-based)**

Vox uses **Earshot VAD** as its primary voice activity detector. It features sub-millisecond voice detection, zero ONNX overhead (no model file required — embedded neural weights), and executes entirely in native Rust.

- Threshold: 0.5 (configurable)
- Latency: ~1ms per 256-sample frame
- ~20x faster than TenVAD

### Legacy Option: **TenVAD (ONNX via sherpa-onnx)**

```rust
let config = VadModelConfig {
    ten_vad: TenVadModelConfig {
        model: Some("ten_vad.onnx"),
        threshold: 0.50,
        min_silence_duration: 0.5,
        min_speech_duration: 0.25,
    },
    sample_rate: 16000,
    num_threads: 1,
};
```

- Threshold: 0.45 (configurable)
- Latency: ~15ms per frame
- Requires `ten_vad.onnx` model file

---

## 5. Speech-to-Text (STT)

### Default Primary Engine: **Nvidia Nemotron-3.5 (INT8 quantized, parakeet-rs)**

Nemotron-3.5 runs via `parakeet-rs`, integrating an INT8 quantized FastConformer-RNNT model for state-of-the-art Hindi/Hinglish speech decoding on CPU with an RTF of ~0.50.

```rust
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
| **RAM footprint** | **~2.5 GB** | **~800 MB** | Larger RAM but vastly better quality and 12.5x faster. |

### Streaming Strategy

- **Nemotron-3.5** processes streaming audio chunks using stateful FastConformer strides (560ms / 8960 samples stride).
- **Qwen-ASR** uses a rolling overlap window (15s).

---

## 6. Language Model (LLM)

### Primary Default: **Llama-3.2-1B-Instruct Q4_K_M (`llama_3_2_reasoning_q4`)**

```rust
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(0); // CPU-only

let context_params = LlamaContextParams::default()
    .with_n_ctx(2048)
    .with_n_threads(n_threads);
```

| Metric | Value | Why Important |
|--------|-------|---------------|
| Memory | ~750 MB | Lowest footprint, fits comfortably in 5.5 GB budget |
| Context | 2048 tokens | Sufficient for single-turn interactions |
| Stability | 100% (5/5 benchmark) | No crashes, no timeouts |
| Speed | ~4.5 TPS | Fast enough for real-time voice |

### Higher Quality: **Llama-3.2-1B-Instruct Q6_K (`llama_3_2_reasoning`)**

| Metric | Value | Why Important |
|--------|-------|---------------|
| Memory | ~1.0 GB | Larger than Q4 but still budget-friendly |
| Quality | Higher output fidelity | More elaborate, nuanced responses |
| Speed | ~3.3 TPS | Slightly slower but better quality |

### Alternative: **Gemma 4 E2B-it Q4_K_M (`gemma_4_reasoning`)**

| Metric | Value | Why Important |
|--------|-------|---------------|
| Memory | ~1.4 GB | Largest single allocation in budget |
| Context | 4096 tokens | Sufficient for multi-turn conversations |
| Reasoning | State-of-the-art (2026) | Superior instruction following |
| Agentic | Function calling, tool use | Enables action-oriented responses |

### Prompt Format (Llama 3.2 Instruct)

```text
<|begin_of_text|>
<|start_header_id|>system<|end_header_id|>
{system_prompt}<|eot_id|>
<|start_header_id|>user<|end_header_id|>
{user_text}<|eot_id|>
<|start_header_id|>assistant<|end_header_id|>
```

Emotion tags `<laugh>`, `<breath>`, `<sigh>` are appended to the system prompt and processed by the TTS engine.

### Token Streaming Implementation

```rust
loop {
    if cancel_flag.load(Ordering::Relaxed) {
        break; // Immediate abort
    }

    let token = ctx.sample_greedy();
    if is_eog_token(token) { break; }

    let token_str = model.token_to_piece(token)?;
    if !token_str.is_empty() {
        tx.send(VoxEvent::LlmToken { turn_id, token })?;
    }
}
```

### Context Management

```rust
ctx.clear_kv_cache();  // Explicit KV cache clearing

if n_cur >= ctx_size { break; }  // Context limit guard
```

---

## 6.1 Cloud LLM Providers

Vox supports **cloud-hosted LLM inference** as an alternative to local models, powered by the `OpenAiCompatProvider` that also handles OpenAI-compatible local servers (Ollama, LM Studio, vLLM). No new provider structs are needed — the provider uses a `provider_name` setting to dynamically map base URLs.

### Supported Providers

| Provider | `provider_name` | Base URL (internal) |
| :--- | :--- | :--- |
| **OpenAI** | `"openai"` | `https://api.openai.com/v1` |
| **Gemini** | `"gemini"` | `https://generativelanguage.googleapis.com/v1beta/openai` |
| **Anthropic** | `"anthropic"` | `https://api.anthropic.com/v1` |

### Architecture

```rust
let provider = OpenAiCompatProvider::new(
    &base_url,          // Auto-mapped from provider_name if empty
    &model,             // e.g. "gpt-4o", "gemini-2.5-pro", "claude-sonnet-4"
    api_key.as_deref(), // Optional; excluded for Ollama/LM Studio
    provider_name.as_deref(), // "openai", "gemini", "anthropic", or None
);
```

The pipeline (`services/pipeline.rs`) is **provider-agnostic** — it calls `LlmProvider` trait methods regardless of whether the backend is local or cloud.

### When to Use Cloud vs. Local

- **Choose cloud** when: you need frontier-level reasoning, have limited RAM, or want zero local memory overhead.
- **Choose local** when: you need offline operation, no API costs, or data must not leave the device.

---

## 7. Text-to-Speech (TTS)

Vox supports three TTS backends via the `TtsProviderConfig` tagged enum: **Supertonic 3** (default), **Chatterbox Local TTS**, and **Chatterbox Remote TTS**.

### Primary: **Supertonic 3 (`supertonic_tts`, `TtsProviderConfig::Supertonic`)**

Supertonic 3 is the default TTS engine — a unified 99M-parameter flow-matching model supporting **31 languages** with **10 voices** (5 male, 5 female). Uses sherpa-onnx native `OfflineTtsSupertonicModelConfig`, INT8 quantized (~144MB). Internally produces 44.1 kHz, resampled to 24 kHz f32 mono output.

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
| Sampling Rate (internal) | 44.1 kHz → 24 kHz output |
| Inference | sherpa-onnx native |
| Load Time | ~400ms cold start |

### Alternative: **Chatterbox Local TTS (`chatterbox_tts`, `TtsProviderConfig::Chatterbox`)**

Chatterbox is a 340M-parameter zero-shot voice cloning model in GGML Q4 format. It can clone any voice from a 5-second reference audio clip.

| Feature | Detail |
|---------|--------|
| Architecture | Transformer (GGML) |
| Parameters | 340M (Q4) |
| Footprint | ~1.1 GB RAM |
| Voice Cloning | 5s reference audio → UUID stored in voices table |
| Languages | Multilingual |
| Sampling Rate | Native 24 kHz |
| Inference | chatterbox-rs |
| CPU Load | Heavy — GPU recommended |

### Alternative: **Chatterbox Remote TTS (`chatterbox_remote`, `TtsProviderConfig::ChatterboxRemote`)**

Offloads TTS inference to a remote CUDA GPU server. Zero local RAM cost — the worker streams audio via `reqwest` blocking HTTP calls.

| Feature | Detail |
|---------|--------|
| Local RAM | 0 MB |
| Remote Endpoint | Configurable URL + remote path |
| Model | 340M (server-side) |
| Audio Transport | `reqwest` blocking streaming |
| Latency | Real-time with GPU server |

### Chunked Synthesis (Accuracy-Quality Mandate — All TTS Providers)

**Goal**: Natural, complete utterances — not choppy word fragments.

Flush to TTS on sentence/clause boundaries, with word-boundary safety to prevent mid-word splits. The same algorithm applies regardless of which TTS provider is active.

**Current algorithm (`utils.rs` — fully dynamic, TPS-continuous):**
1. **Hard boundaries** — `. ! ? ।` → flush immediately (always correct sentence unit)
2. **Clause boundaries** — `, ; —` → flush if word count ≥ threshold (3–7 words, TPS-dependent; disabled above TPS 5.0)
3. **Time-based flush** — wait time scales 1.0s–3.5s, word min scales 3–8 (both continuous functions of TPS)
4. **Word-count fallback** — scales 5–20 words (continuous TPS interpolation)
5. **Word-boundary safety** — Steps 3+4 both require `ends_at_word_boundary()` which checks the last character is whitespace or punctuation
6. **Never flush on 1–2 words** unless a hard sentence boundary is present

Target: **Time-to-First-Audio that sounds natural** — not the shortest possible TTFA.

---

## 8. Speech-to-Speech (S2S) — Cloud Realtime Engine

Vox introduces a **RealtimeVoiceProvider** trait for cloud speech-to-speech APIs that bypass the modular STT → LLM → TTS chain. Instead, raw audio flows over a bidirectional WebSocket, with the cloud provider handling the full voice pipeline server-side.

### RealtimeVoiceProvider Trait

Defined in `services/realtime/mod.rs`, separate from the `LlmProvider` (which remains text-in/text-out):

```rust
pub trait RealtimeVoiceProvider: Send + Sync {
    fn kind(&self) -> RealtimeProviderKind;
    fn audio_config(&self) -> RealtimeAudioConfig;
    fn connect(
        &self,
        interaction_mode: InteractionMode,
        playback_tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<Box<dyn RealtimeSession>>;
    fn health_check(&self) -> bool;
}

pub trait RealtimeSession: Send + Sync {
    fn send_audio(&self, pcm: &[i16]) -> Result<()>;
    fn cancel(&self) -> Result<()>;
    fn disconnect(&self) -> Result<()>;
    fn activity_start(&self) -> Result<()>;
    fn activity_end(&self) -> Result<()>;
    fn is_connected(&self) -> bool { true }
    fn last_activity_time(&self) -> u64 { 0 }
}
```

### Pipeline Routing

The pipeline gains a mode check at the entry point:

```
VAD detects speech
    │
    ├── Mode: modular ──→ STT → LLM → TTS (existing path)
    │
    └── Mode: realtime ──→ RealtimeVoiceProvider → Playback
```

Mode is set via settings and hot-swappable at runtime.

### Thread Model

The realtime engine uses a **hybrid sync/async architecture** — tokio tasks for WebSocket I/O, dedicated OS threads for audio capture and playback, connected via lock-free ring buffers and mpsc channels.

### Provider Candidates

| Provider | Input Sample Rate | Output Sample Rate | Free Tier | Key Advantage |
|----------|:-:|:-:|:---:|:---|
| **Gemini Live** | 16 kHz | 24 kHz | 10-15 RPM | Native 16 kHz input, cheapest |
| **OpenAI Realtime** | 24 kHz | 24 kHz | None | ~232ms P50 latency |
| **Deepgram Voice Agent** (✅) | 16 kHz (flex) | Configurable | $200 credits | Flat $0.075/min pricing |
| **ElevenLabs ConvAI** | 16 kHz | 44.1 kHz | 15 min/mo | Best voice quality, 74 languages |

### Status

**Gemini Live** and **Deepgram Voice Agent** are fully implemented (657-line Deepgram module with full WebSocket reconnect and keepalive). **OpenAI Realtime** and **ElevenLabs ConvAI** remain unimplemented. Detailed integration plans for each provider live in `docs/plans/phase9/`.

---

## 9. Memory Budget Allocation

### Per-Component Running Memory (Active Footprint — Design Targets)

```text
Total Active Budget: ~3.5GB (on 8GB Baseline — design target, not enforced)
├── VAD:        < 0.01GB (Earshot VAD)
├── STT:        ~2.50GB (Nvidia Nemotron-3.5)
├── LLM:        ~0.75GB (Llama 3.2 1B Q4)
├── TTS:        ~0.14GB (Supertonic 3)  or  ~1.1GB (Chatterbox Local)  or  0MB (Chatterbox Remote)
├── Audio Buffers: ~0.10GB (Pre-allocated ring buffers)
└── Safety Margin: ~1.10GB (Headroom to prevent OS swap)
```

> **Note**: These are design targets, not runtime-enforced limits. No memory cap or safety margin enforcement exists in the codebase. Model `ram_usage` values in settings metadata are display-only human-readable strings.

---

## 10. Threading & Performance

### Thread Allocation (Production Defaults)

Thread counts are **configured via settings**, not auto-computed. Defaults:

| Component | Default Threads | Location |
|-----------|:---:|---------|
| **VAD** (Earshot) | N/A (sub-ms, no threading) | `services/vad/earshot_vad.rs` |
| **VAD** (TenVAD) | 1 | `services/vad/ten_onnx.rs:40` |
| **AudioRouter** | 1 (dedicated OS thread) | `services/audio/router.rs` |
| **STT** (Nemotron) | Handled by parakeet-rs | `services/stt/nemotron_onnx.rs` |
| **STT** (Qwen) | 4 | `services/stt/qwen_onnx.rs:54` |
| **LLM** | 4 (configurable, default) | `settings.rs:403` |
| **TTS** (Supertonic/Chatterbox) | 2 | `services/tts/actor.rs` |
| **Realtime** (S2S) | 2 tokio tasks + 2 OS threads | `services/realtime/engine.rs` |

### Thread Priorities

```rust
set_current_thread_priority(ThreadPriority::Max);          // VAD worker
set_current_thread_priority(ThreadPriority::Max);          // AudioRouter (Max priority for audio I/O)
set_current_thread_priority(ThreadPriorityValue::try_from(80u8).unwrap()); // STT worker
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

## 11. Model Lifecycle Management

### Loading Strategy

```rust
enum LoadState {
    Cold,  // Not loaded
    Warm,  // Loaded and ready
}

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
match cmd {
    VadCommand::UpdateThreshold(v) => {
        let new_detector = VadEngine::create_detector(&model_path, v);
        self.detector = new_detector;
    }
}
```

---

## 12. Settings Integration

### Model Selection (VoxSettings)

Settings use a **nested domain-object structure**. Relevant model fields:

```typescript
interface VoxSettings {
  vad: {
    threshold: number;              // 0.0-1.0
    ptt_noise_gate: number;         // 0.0-1.0
    vad_backend: "Earshot" | "TenVad";
  };
  asr: {
    model: string;                  // "nvidia_nemotron" | "qwen3_asr"
    transliterate_enabled: boolean;
    provider: SttProviderConfig;    // { kind: "embedded", model_type } | { kind: "cloud", ... }
  };
  llm: {
    model: string;                  // "llama_3_2_reasoning_q4" | "llama_3_2_reasoning" | "gemma_4_reasoning" | ...
    ctx_size: number;               // 1024-4096 (local only)
    threads: number;                // 1-N (local only)
    provider: LlmProviderConfig;    // { kind: "embedded" } | { kind: "open_ai_compat", base_url, model, api_key, provider_name }
  };
  tts: {
    provider: TtsProviderConfig;    // { kind: "supertonic" } | { kind: "chatterbox", ... } | { kind: "chatterbox_remote", ... }
    voice: number;                  // Supertonic voice index (0-9)
    quality_steps: number;          // Diffusion steps (2-12)
    speed: number;                  // Speed factor (0.7-2.0)
  };
  interaction: {
    main_app_mode: "Passive" | "PTT";
    tray_mode: "Passive" | "PTT";
    pipeline_mode: "Modular" | "Realtime";
    auto_sleep_timeout: number;
  };
  realtime: {
    provider: "gemini_live" | "openai_realtime" | "deepgram_voice_agent" | "elevenlabs_convai";
    gemini: { api_key, model, voice_name, language_code, temperature, enable_web_search };
    openai: { api_key, model };
    deepgram: { api_key, model };
    elevenlabs: { api_key, agent_id };
  };
  assistant: {
    modular_prompt: string;         // replaces hindi_prompt
    realtime_prompt: string;        // replaces english_prompt
  };
  setup: {
    completed: boolean;
  };
  // ... ui, audio, telemetry, persistence unchanged
}
```

### Reload Policies

```rust
pub fn reload_policy_for(domain: &str, key: &str) -> SettingReloadPolicy {
    match (domain, key) {
        ("ui", _) => Hot,
        ("vad", "threshold") => WorkerCommand,
        ("vad", "ptt_noise_gate") => WorkerCommand,
        ("vad", "vad_backend") => Restart,
        ("audio", "output_mode") => WorkerCommand,
        ("audio", "input_device") => Restart,
        ("asr", "model") => Restart,
        ("asr", "provider") => Restart,
        ("asr", "transliterate_enabled") => Hot,
        ("llm", "model") => Restart,
        ("llm", "ctx_size") => Restart,
        ("llm", "threads") => Restart,
        ("llm", "provider") => Restart,
        ("tts", "provider") => Restart,
        ("tts", "voice") => Restart,
        ("tts", "quality_steps") => WorkerCommand,
        ("tts", "speed") => WorkerCommand,
        ("interaction", "auto_sleep_timeout") => Hot,
        ("interaction", "pipeline_mode") => Restart,
        ("interaction", _) => Hot,
        ("telemetry", "enabled") => Hot,
        ("telemetry", "log_level") => Restart,
        ("persistence", "private_mode") => Hot,
        ("persistence", "max_sessions") => Hot,
        ("persistence", "retention_days") => Hot,
        ("assistant", "modular_prompt") => Hot,
        ("assistant", "realtime_prompt") => Hot,
        ("realtime", _) => Hot,
        _ => Restart,
    }
}
```

---

## 13. Error Handling & Resilience

### Model Load Failures

```rust
match SttEngine::new(&model_path) {
    Ok(engine) => {
        is_loaded.store(true, Ordering::Relaxed);
        Some(engine)
    },
    Err(e) => {
        log::error!("[STT] Load failed: {}", e);
        None  // Continue without STT (graceful degradation)
    }
}
```

### Inference Timeouts

```rust
if cancel_flag.load(Ordering::Relaxed) {
    return Ok(());  // Abort inference immediately
}
```

### Memory Pressure

```rust
if available_memory < SAFETY_MARGIN {
    cool_down_llm();  // Trigger auto-sleep
}
```

---

## 14. Final Architecture Principles

### 1. Hardware-Constrained & Scalable Design

Models are bounded by **physics, not capability**. Running parameters scale dynamically according to system RAM capacity:

- **Baseline**: Bounded to fit cleanly inside 8GB RAM.
- **High performance**: Larger contexts and higher-fidelity parameters for 16GB+ systems.
- **Accuracy**: Output correctness remains the absolute constraint.

### 2. Native Performance

**Zero Python overhead** in inference path:

- ONNX Runtime for neural networks.
- llama.cpp for GGUF transformer models.
- parakeet-rs for streaming speech recognition.

### 3. Streaming-First Architecture

**No batch processing**. Everything streams:

- Audio chunks → VAD → STT partials (UI feedback) + STT final (authoritative)
- LLM tokens → TTS chunks (sentence-level, not word-level)
- Parallel execution for natural, responsive output

### 4. Future-Proof Modularity

Each model role is **replaceable** without affecting others. The same `LlmProvider` trait serves both local GGUF and cloud OpenAI-compatible endpoints. The `RealtimeVoiceProvider` trait extends the same pattern to cloud S2S APIs.
