# Vox — Backend Architecture (Native Inference)

---

## 1. Overview

The Vox backend is a **real-time, event-driven native audio processing system**.

It is built using:

* **Rust (Tauri)** → system orchestration, audio I/O, IPC
* **C++ Inference Layer** → model execution (`onnxruntime`, `llama.cpp`)

---

## 2. Core Design Principles

---

### Native-First Execution

All inference MUST run using:

* ONNX Runtime (C++)
* llama.cpp (C++)

Python is **not part of runtime**.

---

### Streaming First

The system operates as a continuous stream:

```text
audio → VAD → STT → LLM → TTS → output
```

No stage waits for completion.

---

### Event-Driven Architecture

```text
audio_chunk → speech_start → text_delta → llm_token → tts_chunk
```

Each stage emits incremental outputs.

---

### Low-Latency Constraint

Target: **Accurate, complete outputs**

Every component must minimize:

* buffering
* blocking
* memory allocation

**Speed is a result of good engineering, not a target that overrides accuracy.**

---

## 3. System Topology

---

### [Tauri (Rust)]
    ├── Audio Capture (cpal)
    ├── Event Bus (mpsc)
    ├── UI IPC (Tauri Events)
    │
    ↓
[Core Layer (Shared State & Constants)]
    ├── events.rs (VoxEvent enum)
    ├── settings.rs (VoxSettings struct)
    ├── state.rs (InteractionState, PipelineAtomics)
    ├── constants.rs (Model paths, timing)
    └── metrics.rs (PipelineMetrics)
    │
    ↓
[Services Layer (Actor-Engine Pattern)]
    ├── VAD (Actor -> Engine: Earshot / TenVAD)
    ├── STT (Actor -> Engine: Nvidia Nemotron-3.5 / Qwen3-ASR)
    ├── LLM (Actor -> Engine: llama.cpp)
    ├── TTS (Actor -> Engine: Supertonic 3)
    ├── Pipeline (Orchestrator: LLM→TTS→Playback coordination)
    ├── Playback (CPAL output, jitter buffer, upsampling)
    ├── PTT (Push-to-talk mode)
    └── Utils (should_flush, transliteration, chunking, stitching)
    │
    ↓
[Infrastructure Layer]
    ├── IPC (Tauri command handlers)
    ├── Persistence (SQLite session storage)
    ├── Monitoring (Telemetry aggregator, system monitor)
    ├── Setup (First-run onboarding)
    └── Wizard (Setup wizard window + model health checks)

---

## 4. Threading Model (CRITICAL)

---

### Core Principle

Avoid CPU thrashing. All inference runs on **dedicated OS threads**, never async tokio tasks.

Threads are NOT Send because llama.cpp and onnxruntime are not thread-safe across cores.

---

### Thread Allocation

```text
Total cores = N
LLM threads = N - 2
Remaining:
    - audio thread (Tier 1: highest priority)
    - VAD thread (Tier 2: high priority)
```

---

### Thread Priority

* Audio capture callback: `ThreadPriority::Max`
* VAD worker: `Crossplatform(ThreadPriorityValue::from(80u8))`
* STT worker: Same as VAD
* LLM/TTS: Default priority

---

## 5. Audio Ingestion (Tier 1 - Realtime)

---

### Implementation

* library: `cpal`
* format: 16kHz mono PCM
* chunk size: 10–20 ms

---

### Zero-Allocation Callback

The CPAL callback reuses pre-allocated buffers:

* `mono_buffer` — for channel averaging
* `resampled_buffer` — for 48kHz→16kHz linear interpolation

---

### Ring Buffer Transport

Audio flows via SPSC lock-free ring buffer:

```text
Producer: AudioStream (CPAL callback)
Consumer: VAD worker (Tier 2)
Capacity: 16000 * 4 = 64000 samples (4s)
```

---

### Overflow Handling

```rust
// Throttled logging: only every 100 drops
if pushed < resampled_buffer.len() {
    DROP_COUNT.fetch_add(1);
    if prev % 100 == 0 { log warning }
}
```

## 5. Directory Structure (src/)

The backend is organized into domain-specific modules. Services use the **Actor-Engine pattern**:

```
src/
├── lib.rs            # Tauri app entry, plugin init, engine lifecycle
├── main.rs           # Binary entry point
├── core/
│   ├── events.rs     # VoxEvent enum (pipeline signals)
│   ├── settings.rs   # VoxSettings struct + reload policies
│   ├── state.rs      # InteractionState, InteractionOwner, PipelineAtomics
│   ├── constants.rs  # Model paths, timing, system prompts
│   └── metrics.rs    # MetricField, PipelineMetrics
├── services/
│   ├── mod.rs        # Service registration
│   ├── traits.rs     # Engine interfaces (VadEngine, SttEngine, LlmEngine, TtsEngine)
│   ├── pipeline.rs   # Pipeline orchestrator (LLM→TTS→Playback)
│   ├── utils.rs      # should_flush, count_words, is_devanagari, transliterate, stitch
│   ├── audio.rs      # Audio capture (cpal ring buffer)
│   ├── playback.rs   # Playback engine (cubic Hermite upsample, jitter buffer, fade)
│   ├── ptt.rs        # Push-to-talk mode
│   ├── translit.rs   # Transliteration (Devanagari→Roman, ONNX model)
│   ├── llm/
│   │   ├── mod.rs    # Module entry + global_llama_backend() singleton
│   │   ├── actor.rs  # Command/Event handler (spawn_llm_worker)
│   │   └── llama_cpp.rs # Llama.cpp engine (tag stripping, streaming)
│   ├── stt/
│   │   ├── mod.rs
│   │   ├── actor.rs
│   │   ├── nemotron_onnx.rs  # Nemotron-3.5 ASR (primary, parakeet-rs)
│   │   └── qwen_onnx.rs      # Qwen3-ASR (legacy, sherpa-onnx)
│   ├── tts/
│   │   ├── mod.rs
│   │   ├── actor.rs
│   │   └── supertonic.rs     # Sole TTS engine (sherpa-onnx, anti-aliasing LPF)
│   └── vad/
│       ├── mod.rs
│       ├── actor.rs
│       ├── earshot_vad.rs    # Earshot (default, pure Rust energy-based)
│       └── ten_onnx.rs       # Ten VAD (legacy, ONNX via sherpa-onnx)
├── ipc/
│   ├── mod.rs
│   ├── pipeline.rs  # launch_engine, stop_engine, engage, check_engine_status
│   ├── settings.rs  # get_settings, update_setting, request_model_catalog
│   ├── tray.rs      # hide_tray_window, position_tray_window
│   ├── history.rs   # get_sessions, get_turns, delete_session
│   ├── audio.rs     # Audio device listing
│   ├── monitoring.rs # Runtime snapshot retrieval
│   └── setup.rs     # Boot state, model catalog
├── persistence/
│   ├── mod.rs
│   ├── db.rs         # SQLite schema + connection
│   └── events.rs     # PersistenceEvent enum
├── monitoring/
│   ├── mod.rs
│   ├── aggregator.rs # TelemetryAggregator
│   ├── collector.rs  # Metric collection from atomics
│   ├── snapshot.rs   # RuntimeSnapshot struct
│   └── system_monitor.rs # /proc/stat/meminfo polling
├── setup/
│   └── mod.rs        # First-run setup/orientation
├── tray.rs            # Tray icon, overlay window management
├── wizard.rs          # Setup wizard window config + model health checks
├── utils/
│   └── paths.rs       # Path resolution (dirs crate, cross-platform)
└── bin/
    ├── tts-bench.rs   # TTS-only benchmark
    ├── vox-bench.rs   # Full pipeline benchmark
    └── test-translit.rs # Transliteration test
```

## 6. Voice Activity Detection (VAD) - Tier 2

---

### Actor-Engine Pattern

Each AI domain (VAD, STT, LLM, TTS) follows a strict separation of concerns:

1. **Actor**: Owns the OS thread, manages the command `Receiver`, handles state (turn IDs, cancellation), and emits `VoxEvent`s.
2. **Engine**: Encapsulates the C++/ONNX inference logic, implements the domain trait, and remains stateless where possible.

---

### Models (Two Backends)

**Default: Earshot VAD (Rust-native, energy-based)**
- No model file required — embedded neural weights
- Ultra-low latency (~1ms per frame)
- Threshold: configurable (default 0.5)
- ~20x faster than TenVAD

**Legacy: Ten VAD (ONNX via sherpa-onnx)**
- Requires `ten_vad.onnx` model file
- Higher latency (~15ms per frame)
- threshold: configurable (default 0.45)
- min_silence_duration: 0.5s
- min_speech_duration: 0.25s

---

### Run Loop Logic

```rust
loop {
    if engine_shutdown.load(Ordering::Relaxed) { break; }
    
    // Hot-updates via VadCommand channel (lock-free)
    while let Ok(cmd) = vad_rx.try_recv() { update local state; }
    
    if consumer.occupied_len() >= 256 {
        consumer.pop_slice(&mut chunk);
        self.detector.accept_waveform(&chunk);
        
        if detected && !in_speech {
            in_speech = true;
            current_turn_id += 1;
            stt_tx.send(ResetStream); // Clear STT state
            pipeline_tx.send(SpeechStart { ... });
        }
        
        if detected {
            utterance_buffer.extend_from_slice(&chunk);
            // Every 800ms: send Partial to STT
        } else if in_speech {
            in_speech = false;
            // Send Final to STT if >= 3200 samples
            pre_roll_buffer.extend_from_slice(&chunk); // 500ms pre-roll
        }
    } else {
        std::thread::sleep(Duration::from_millis(5)); // Throttle
    }
}
```

---

### Pre-roll Buffer

* 500ms sliding window during silence
* Injected on speech_start to capture onset frames

---

### Hot-Reloading

VAD settings (threshold, noise gate, mode, owner) update via `VadCommand` channel without blocking the audio path.

---

## 7. Speech-to-Text (STT) - Tier 2



---


### Models



**Default: Nemotron-3.5** (ONNX INT8, ~657 MB encoder, ~99 MB decoder_joint)

- Runtime: ONNX Runtime via `ort` crate

- Files: `encoder.onnx`, `decoder_joint.onnx`, `config.json`, `tokenizer.model`

- Memory: ~1,265 MB RSS

- RTF: 0.02–0.35× (average 0.18×)

- Chunked transcription: 8960-sample windows, `reset_state()` only at end



**Legacy: Qwen3-ASR-0.6B** (ONNX INT8 via sherpa-onnx)

- 4 files: `conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, `tokenizer`

- Higher RTF: 0.38–4.63×

- Still supported but not the default


---


### Worker Thread


```rust

spawn_stt_worker(

    rx: SttCommand,           // Commands from VAD

    model_path: PathBuf,

    engine_type: String,      // "nemotron" or "qwen"

    vox_event_tx: Option<mpsc::Sender<VoxEvent>>,

    is_engaged: Arc<AtomicBool>,

    is_loaded: Arc<AtomicBool>,

    engine_shutdown: Arc<AtomicBool>,

    pre_load: bool,

)

```



---


### SttCommand Types


```rust

pub enum SttCommand {

    Partial(u32, InteractionOwner, Vec<f32>),  // Streaming feedback

    Final(u32, InteractionOwner, Vec<f32>),    // End of utterance

    ResetStream,                               // Clear decoder state

    Shutdown,                                    // Exit thread

}

```



---


### Key Algorithm: Chunked Transcription (Nemotron)



v0.8.2 fix: Nemotron audio is fed as sequential 8960-sample (~560ms @ 16kHz) windows

through the ONNX session. `reset_state()` is called **only at the end**, keeping

context across all chunks. This produces coherent Devanagari Hindi from multilingual

speech (previously produced fragmented English).



```rust

fn transcribe(audio: &[f32]) -> String {

    let window_size = 8960;

    for chunk in audio.chunks(window_size) {

        session.run(ORTFeed { name: "audio_signal", tensor: chunk });

    }

    session.reset_state();  // Only at the end

    decode_output(session)

}

```



---


### Throttling



Partial transcripts throttled to `STT_THROTTLE_MS = 800ms` to prevent CPU spikes.

Partials are UI feedback only. The `Final` transcript is authoritative.


---


## 8. Language Model (LLM) - Tier 3

---

### Decoupled LLM Provider Architecture

The LLM subsystem is decoupled into a **provider-based architecture**. This allows Vox to switch between local embedded models and remote HTTP/API endpoints without changing the core voice pipeline loop (`pipeline.rs`):

```text
Vox Pipeline
    └─ LLM Worker Thread (spawn_llm_worker)
           └─ LlmProvider Trait
                  ├─ EmbeddedProvider (local GGUF via llama.cpp)
                  └─ OpenAiCompatProvider (handles ALL remote/cloud providers)
                       ├─ OpenAI-compatible servers (Ollama, LM Studio, vLLM)
                       ├─ OpenAI cloud          (provider_name: "openai")
                       ├─ Gemini cloud          (provider_name: "gemini")
                       └─ Anthropic cloud       (provider_name: "anthropic")
```

---

### Cloud Provider Routing

The `OpenAiCompatProvider` constructor accepts a `provider_name: Option<&str>` parameter that dynamically maps the base URL to the correct cloud endpoint:

```rust
pub fn new(base_url: &str, model: &str, api_key: Option<&str>, provider_name: Option<&str>) -> Self
```

Logic:
- If `provider_name` is `"openai"` → `base_url = "https://api.openai.com"`
- If `provider_name` is `"gemini"` → `base_url = "https://generativelanguage.googleapis.com/v1beta/openai"`
- If `provider_name` is `"anthropic"` → `base_url = "https://api.anthropic.com"`

For Anthropic, the `inject_headers` method adds `anthropic-version: 2023-06-01` and `x-api-key` headers alongside Bearer auth. The pipeline (`pipeline.rs`) passes `provider_name` from the user's settings when constructing the provider, enabling transparent cloud routing without changes to pipeline orchestration.

---

### Provider Interface

Every LLM provider must implement the `LlmProvider` trait:

```rust
pub trait LlmProvider: Send + Sync {
    /// Submit a generation request and stream tokens via channels
    fn generate(
        &self,
        text: &str,
        system_prompt: &str,
        turn_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &mpsc::Sender<VoxEvent>,
    ) -> anyhow::Result<()>;

    /// Returns true if the provider is healthy / reachable
    fn health_check(&self) -> bool;

    /// Returns a list of model IDs the provider can serve
    fn list_models(&self) -> anyhow::Result<Vec<RemoteModelInfo>>;

    /// Human-readable provider kind
    fn kind(&self) -> ProviderKind;
}
```

---

### Worker Thread

The LLM subsystem runs inside a persistent worker thread spawned via `spawn_llm_worker` that listens for generation requests and coordinates with the active provider:

```rust
pub fn spawn_llm_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<LlmCommand>,
    provider: Box<dyn LlmProvider>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    is_loaded: Arc<AtomicBool>,
) {
    is_loaded.store(true, Ordering::Relaxed);
    while let Ok(cmd) = rx.recv() {
        match cmd {
            LlmCommand::Generate { text, system_prompt, turn_id, cancel_flag } => {
                if let Err(e) = provider.generate(&text, &system_prompt, turn_id, &cancel_flag, &event_tx) {
                    let _ = event_tx.send(VoxEvent::Error { turn_id, message: e.to_string() });
                }
            }
            LlmCommand::Shutdown => break,
        }
    }
    is_loaded.store(false, Ordering::Relaxed);
}
```

---

### LlmCommand Types

The orchestrator communicates with the LLM worker using the following message protocol:

```rust
pub enum LlmCommand {
    Generate {
        text: String,
        system_prompt: String,
        turn_id: u32,
        cancel_flag: Arc<AtomicBool>,
    },
    Shutdown,
}
```

---

### Provider Implementations

#### 1. Embedded Provider (`EmbeddedProvider`)
Uses the local `llama.cpp` C++ engine bindings (via `llama-cpp-4` crate) to load and execute GGUF models directly on the host CPU.
* **Primary Model**: Llama-3.2-1B-Instruct (GGUF Q6_K, ~1.02 GB) which consumes ~970 MB RSS memory and runs at ~3.3 TPS on CPU.
* **Alternative Models**: Gemma 2 2B-it (Q4_K_M, ~3.46 GB), Gemma 2 2B Uncensored (Q2_K_P, ~2.30 GB), and MiniCPM5-1B (Q4_K_M, ~688 MB).
* **Multi-Family Formatting**: Automatically detects and formats prompt structure based on the model family (Gemma, Qwen, Llama3, Nemotron, or Unknown).

#### 2. OpenAI-Compatible Remote Provider (`OpenAiCompatProvider`)
Connects to remote inference servers and cloud APIs over HTTP using a non-blocking connection client via the `reqwest` crate. Supports both local OpenAI-compatible servers (Ollama, LM Studio, vLLM) and direct cloud LLM providers (OpenAI, Gemini, Anthropic) — all through the same provider struct, differentiated by the `provider_name` parameter.
* **Streaming & Cancellation**: Submits chat completion requests with `stream: true` and processes chunks as they arrive. Continuously polls the `cancel_flag` to abort the HTTP request instantly when barge-in is triggered.
* **Model Discovery**: Dynamically queries the standard `/v1/models` endpoint (or `/api/tags` for Ollama) to discover and list available models.
* **Cloud Provider Support**: The constructor accepts `provider_name` (`"openai"`, `"gemini"`, or `"anthropic"`) to automatically resolve the correct base URL. Anthropic adds `anthropic-version` and `x-api-key` headers via `inject_headers`.

---

### Language Detection & Prompt Formatting

Before dispatching to the provider, the pipeline formats the user query and system prompt:
* **Language Detection**: The `is_devanagari(text)` function checks if the transcript contains Devanagari Unicode characters (range U+0900–U+097F). If detected, it routes Hindi prompts; otherwise, English prompts.
* **Emotion Tags**: Emotion tags `<laugh>`, `<breath>`, and `<sigh>` are appended to the system prompt to guide the LLM's vocal emotional markers.
* **Template Routing**: The `ModelFamily` helper generates the correct prompt layout matching the loaded model’s spec:
```text
<|begin_of_text|><|start_header_id|>system<|end_header_id|>

{system_prompt}<|eot_id|><|start_header_id|>user<|end_header_id|>

{user_transcript}<|eot_id|><|start_header_id|>assistant<|end_header_id|>
```

---

### Tag Stripping (Accumulated-Buffer + Delta Emission)

v0.8.2+: The LLM token stream strips emotion tags (`<laugh>`, `<breath>`, `<sigh>`) before passing text to the TTS engine to prevent raw tags from being read aloud:
1. **Accumulated-buffer stripping**: Tags are removed from the full accumulated text instead of individual token slices, avoiding partial-tag leakage.
2. **Delta emission**: Emits only the differences between current and historical cleaned strings, maintaining the per-token display cadence.
3. **Partial-tag holdback**: The `partial_tag_len()` helper detects incomplete tags at the buffer end (e.g. `"<lau"`) using `char_indices()` to safely handle multi-byte UTF-8, holding them back until they resolve or complete.
4. **Think-block suppression**: Suppresses internal chain-of-thought blocks enclosed in `<think>...</think>` or `[think]...[/think]` tags.
```rust
// tag stripping in the token generation / cleaning loop
let mut cleaned = full_accumulated.clone();
for tag in &["<laugh>", "<breath>", "<sigh>"] {
    cleaned = cleaned.replace(tag, "");
}
if let Some(pos) = cleaned.rfind(partial_tag) {
    cleaned.truncate(pos); // Hold back partial tag
}
let delta = &cleaned[old_cleaned_len..];
if !delta.is_empty() {
    tx.send(VoxEvent::LlmToken { token: delta.to_string() });
}
```

## 9. Text-to-Speech

---

### Model

* Supertonic 3 — 99M param flow-matching, INT8 quantized (~144MB), 31 languages, 10 voices (sherpa-onnx native)

---

### Anti-Aliasing Low-Pass Filter (v0.8.2+)

Supertonic 3's vocoder produces audio at 44.1kHz. The engine downsamples to 24kHz
for TTS delivery. To prevent aliasing artifacts from high-frequency content near
Nyquist (22.05kHz), a 2nd-order Butterworth LPF is applied before downsampling:

- **Type**: Biquad low-pass filter (2nd-order Butterworth)
- **Cutoff**: 11000 Hz (below 24kHz Nyquist of 12000 Hz, with 1kHz margin)
- **Sample rate**: 44100 Hz
- **Coefficients**: Pre-computed via `BiquadFilter::new(Lpf, 11000.0, 44100.0)`
- **Execution**: Applied sample-by-sample in the resampling loop, not as a separate pass

```rust
// supertonic.rs: anti-aliasing LPF
let mut lpf = BiquadFilter::new(BiquadType::Lpf, 11000.0, 44100.0);
for i in 0..output_samples {
    let filtered = lpf.process(supertonic_output[i]);
    interpolated_24k[i] = filtered;
}
```

The filter coefficients (`BiquadCoefficients`) use the standard RBJ biquad formulae.
No external DSP library required.

---

### Worker Thread

```rust
spawn_tts_worker(
    rx: mpsc::Receiver<TtsCommand>,
    tts_dir: PathBuf,
    event_tx: mpsc::Sender<VoxEvent>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
)
```

---



### Chunked Synthesis (Quality Mandate)

Tokens flushed to TTS using the fully dynamic `should_flush` algorithm in `utils.rs`.
See [Section 11: Sub-Sentence Chunking](#sub-sentence-chunking) for the complete algorithm
description. The TTS engine receives text chunks that are prosodically coherent (end at
sentence or clause boundaries where possible) without mid-word splits.


---

## 10. Playback Engine (Tier 3)

---

### Architecture

```text
TtsChunk (24kHz) → upsample_2x() → ring buffer → CPAL callback (48kHz)
```

---

### Upsample Function (Cubic Hermite Interpolation)

```rust
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    // Cubic Hermite (Catmull-Rom) interpolation for 24kHz → 48kHz (exact 2x ratio).
    // Uses 4-point basis with weights [-1/16, 9/16, 9/16, -1/16].
    // Produces smoother waveform than linear interpolation.
    // O(n), no FFT, no external deps.
    for i in 0..len {
        out.push(input[i]);
        let p0 = if i > 0 { input[i - 1] } else { input[i] };
        let p2 = if i + 1 < len { input[i + 1] } else { input[i] };
        let p3 = if i + 2 < len { input[i + 2] } else { p2 };
        let midpoint = (-p0 + 9.0 * input[i] + 9.0 * p2 - p3) / 16.0;
        out.push(midpoint);
    }
}
```

**Improvement over linear interpolation**: Cubic Hermite produces continuous first
derivatives at sample boundaries, reducing high-frequency artifacts compared to
linear interpolation's piecewise-linear output. This is particularly noticeable
in higher-frequency audio content where linear interpolation creates "staircase"
distortion.

---

### Playback Underrun Fade (v0.8.2+)

When the TTS ring buffer is empty (generation hasn't started or is delayed), a
short fade prevents audible click/pop artifacts:

```rust
// playback.rs: underrun protection
const FADE_STEP: f32 = 0.002;  // ~10ms fade at 48kHz
if let Some(sample) = ring_buffer.try_pop() {
    let faded = sample * (1.0 - fade_progress.min(1.0));
    fade_progress += FADE_STEP;
}
```

- **Duration**: ~10ms (96 samples at 48kHz, step=0.002)
- **Direction**: Fades out (linear ramp to silence) on underrun, instant resume on next chunk
- **State reset**: Fade progress reset to 0 on `TtsFinished` and `Cancelled` events
- **Tradeoff**: 10ms of silence at utterance start is imperceptible; prevents the 1–3 sample
  DC pop that would otherwise result from abrupt ring buffer underrun

---

### Jitter Buffer

* Pre-buffer: 300ms (14,400 samples)
* Total capacity: 2s (192,000 samples)
* Drop policy: log warning, never block

---

### Barge-In (Speaker Mode)

```rust
// VAD thread checks:
if playback_active.load(Ordering::Relaxed) && mode == Speaker {
    continue; // Drop mic frame
}
```

---

## 11. Pipeline Orchestrator

---

### State Machine

```rust
pub enum InteractionState {
    Idle,
    Listening,
    UserSpeaking,
    Thinking,
    AssistantSpeaking,
    Interrupted,
}
```

---

### Sub-Sentence Chunking

The flush-to-TTS algorithm is defined in `utils.rs` (`should_flush`) and uses a
**fully dynamic, model/hardware-agnostic** algorithm with continuous TPS interpolation.

No hardcoded TPS categories. Every threshold is a continuous function of the observed
generation speed (tokens per second):

| Condition | TPS=1 (slow) | TPS=3.5 (medium) | TPS=6 (fast) |
|-----------|:---:|:---:|:---:|
| Sentence boundary (`.!?।`) | Always flush | Always flush | Always flush |
| Clause boundary (`,;—`) | Flush at 3 words | Flush at 4 words | Disabled |
| Time gate | 1.0s / 3 words | 2.2s / 5 words | 3.5s / 8 words |
| Word-count fallback | 5 words | 12 words | 20 words |

**Algorithm:**

```rust
let tps_clamped = tps.clamp(0.5, 6.0);
let tps_norm = (tps_clamped - 0.5) / (6.0 - 0.5); // 0.0=slowest, 1.0=fastest

// Clause boundary flushing (fades out between TPS 3.0 and 5.0):
//   At low TPS: flush on `,` or `;` with ≥3 words (prioritize TTFA)
//   At high TPS: skip clause flushes — sentences complete fast
if tps_norm < clause_norm_high {
    let clause_threshold = (3.0 + t * 4.0).round() as usize; // 3→7 words
    if word_count >= clause_threshold { return true; }
}

// Time gate: scales from 1.0s at slow TPS → 3.5s at fast TPS
let max_wait_ms = lerp(tps_norm, 1000.0, 3500.0) as u128;
let min_time_words = lerp(tps_norm, 3.0, 8.0).round() as usize;
if elapsed_ms >= max_wait_ms && word_count >= min_time_words && ends_at_word_boundary(buf) {
    return true;
}

// Word-count fallback: scales from 5→20 words
let max_words = lerp(tps_norm, 5.0, 20.0).round() as usize;
if word_count >= max_words && ends_at_word_boundary(buf) {
    return true;
}
```

Key behaviors:
- **Clause flushing** (`,`, `;`, `—`) fades out between TPS 3.0 and TPS 5.0.
  Below 3.0 TPS: flush aggressively (small word threshold). Above 5.0 TPS: disabled entirely
  (sentences complete quickly enough that clause flushes would only harm prosody).
- **Time gate** scales continuously: slow generation gets a shorter leash (1s) to keep TTFA
  bounded; fast generation gets more time (3.5s) to complete a sentence naturally.
- **Word-count fallback** scales from 5 words (aggressive at low TPS) to 20 words (lenient
  at high TPS).
- **Word-boundary safety**: `ends_at_word_boundary()` blocks flush if the last character
  is not whitespace or punctuation (prevents mid-word splits from BPE subword tokens).
- **Never flush on 1–2 words** unless a hard sentence boundary is present (implicit: clause
  threshold ≥ 3, time words ≥ 3, fallback ≥ 5).

**The goal is natural, complete utterances — not the shortest possible TTFA.**


---

### Turn Management

```rust
pub struct PipelineAtomics {
    pub cancel_flag: Arc<AtomicBool>,
    pub turn_id: Arc<AtomicU32>,
    pub state: Arc<Mutex<InteractionState>>,
    pub is_engaged: Arc<AtomicBool>,
    pub conversation_id: Arc<AtomicU64>,
}
```

---

### Lock-Free State Updates

```rust
impl PipelineAtomics {
    pub fn update_interaction_state(...) {
        // Update atomic flags for monitoring
        self.is_assistant_speaking.store(...);
        self.current_state_atomic.store(... as u32);
        // Send IPC event to owning window only
    }
}
```

---

## 12. Event Bus

---

### VoxEvent Enum

```rust
pub enum VoxEvent {
    SpeechStart { turn_id, owner },
    SpeechEnd { turn_id, owner },
    TranscriptPartial { turn_id, owner, text },
    TranscriptFinal { turn_id, owner, text },
    LlmToken { turn_id, token },
    LlmFinished { turn_id },
    TtsChunk { turn_id, samples },
    TtsFinished { turn_id, rtf },
    PlaybackStarted { turn_id },
    PlaybackFinished { turn_id },
    Cancelled { turn_id },
    Error { turn_id, message },
    WarmUp,
    Shutdown,
    SettingsUpdated(VoxSettings),
}
```

---

### Event Flow

```text
VAD:        SpeechStart, SpeechEnd
STT:        TranscriptPartial, TranscriptFinal
Pipeline:   WarmUp, Cancelled, Error, Shutdown
LLM:        LlmToken, LlmFinished, Error
TTS:        TtsChunk, TtsFinished
Playback:   PlaybackFinished
```

---

## 13. Memory Management

---

### Budget (Measured v0.8.2)



```text

~5.5GB usable for inference

VAD:   ~0.05GB  (~50 MB)

STT:   ~1.27GB  (Nemotron-3.5 ONNX, ~1265 MB actual)

LLM:   ~0.97GB  (Llama-3.2-1B Q6_K, ~970 MB actual)

TTS:   ~0.02GB  (Supertonic 3 INT8, ~21 MB actual)

KV:    ~0.60GB  (llama.cpp KV cache)

Safety: ~2.59GB margin (for OS, UI, other processes)



Total measured peak: ~2.46 GB (well within 8 GB target)

```


---

### Droppable Structures

* Ring buffers: overflow drops with warning counter
* Telemetry channel: `try_send`, count drops
* Persistence channel: `try_send`, count drops

---

## 14. Lifecycle Management

---

### Engine States

```rust
pub enum PipelineState {
    Cold,  // Models unloaded, minimal RAM
    Warm,  // LLM/TTS loaded, ready for interaction
}
```

---

### Auto-Sleep

```rust
if last_interaction.elapsed() > auto_sleep_timeout {
    cool_down_llm();
    cool_down_tts();
}
```

---

### Dormancy (Phase 5)

* Tray mode: `is_engaged = false` → LLM/TTS skipped
* Auto-sleep: models offloaded to save RAM

---

## 15. Persistence Layer (Phase 6)

---

### Worker Thread

```rust
spawn_persistence_worker(
    db_path: PathBuf,
    tx: crossbeam::Sender<PersistenceEvent>,
)
```

---

### Events

```rust
pub enum PersistenceEvent {
    SessionStarted { id, timestamp_ms },
    SessionEnded { id, timestamp_ms },
    TurnCompleted { conversation_id, turn_id, user_text, assistant_text, stt_latency_ms, ttft_ms },
    TurnCancelled { conversation_id, turn_id },
    Shutdown,
}
```

---

### Private Mode

* `is_private_mode` atomic checked before each write
* Events skipped but pipeline continues

---

## 16. Telemetry & Monitoring (Phase 6)

---

### TelemetryAggregator

* Dedicated OS thread
* `crossbeam_channel::bounded(4096)`
* Events: `AudioEnergy`, `SystemHealth`, `InteractionMetric`

---

### SystemMonitor

* Spawns every 30 seconds
* Reads `/proc/stat`, `/proc/meminfo` for CPU/RAM
* Filters out Linux process sub-tasks (threads) when iterating processes to prevent double-counting RSS memory — only main process entries with `tasks()` present are aggregated

---

### Monitoring State

```rust
pub struct MonitoringState {
    snapshots: Mutex<Vec<RuntimeSnapshot>>,
    history: Mutex<VecDeque<RuntimeSnapshot>>,
}
```

---

## 17. Settings & Hot-Reloading

---

### Settings Structure

```rust
pub struct VoxSettings {
    pub ui: UiSettings,
    pub audio: AudioSettings,
    pub vad: VadSettings,
    pub asr: AsrSettings,
    pub llm: LlmSettings,
    pub tts: TtsSettings,
    pub interaction: InteractionSettings,
    pub telemetry: TelemetrySettings,
    pub persistence: PersistenceSettings,
    pub assistant: AssistantSettings,
}
```

---

### Reload Policies

```
Hot:             Apply immediately (UI)
WorkerCommand:   Send via channel (VAD threshold, system_prompt)
Restart:        Full pipeline restart (model changes)
```

### Provider Health & Model Discovery

IPC commands `check_llm_provider_health` and `list_remote_llm_models` (in `ipc/settings.rs`) forward `provider_name` from the user's settings when constructing the `OpenAiCompatProvider`, ensuring health checks and model discovery target the correct cloud endpoint.

---

## 18. PTT Mode (Phase 5)

---

### State Machine

```rust
IDLE → RECORDING → PROCESSING → DISPLAY
```

---

### Buffer Strategy

* Continuous capture (no VAD gating)
* 15s window per partial send
* 10min hard limit

---

### Cancel Flow

```rust
pub fn ptt_cancel() {
    recording = false;
    buffer.clear();
    state.pipeline.cancel_flag.store(true);
    state.pipeline.update_interaction_state(Idle);
}
```

---

## 19. Concurrency Patterns Summary

---

### Lock-Free Communication

* Ring buffers: audio transport
* Atomic flags: pipeline control
* Channels: inter-thread events

---

### Locks (Minimized)

* `RwLock<VoxSettings>` - read-heavy, write-rare
* `Mutex<Option<VoxEngine>>` - state mutations
* `Tokio::sync::Mutex` - async IPC state

---

### Never Lock In Hot Paths

* VAD/STT/LLM/TTS workers never call `settings.read()`
* Hot values snapshotted at startup, updated via channels
* Telemetry uses atomics, not mutex

---

## 20. Lock Contention & Atomic Operations

---

### Critical Path Lock Avoidance

**Rule: Zero locks on audio hot path**

```rust
// ❌ WRONG: Settings read on audio callback
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn audio_callback(data: &[f32]) {
    let settings = SETTINGS.read().unwrap(); // BLOCKS ALL OTHER THREADS
    // Process audio...
}

// ✅ CORRECT: Pre-snapshotted values
struct AudioProcessor {
    local_threshold: f32,  // Copied from settings
    local_noise_gate: f32,
}

impl AudioProcessor {
    fn update_from_settings(&mut self, settings: &VoxSettings) {
        self.local_threshold = settings.vad.threshold;
        self.local_noise_gate = settings.vad.ptt_noise_gate;
    }
}
```

### Atomic State Coordination

**Pipeline uses atomic flags for cross-thread signaling**

```rust
// Arc<AtomicBool> for cancellation
pub struct PipelineAtomics {
    pub cancel_flag: Arc<AtomicBool>,        // LLM/TTS abort
    pub playback_active: Arc<AtomicBool>,    // Mic ducking
    pub tts_generating: Arc<AtomicBool>,     // TTS busy state
    pub is_engaged: Arc<AtomicBool>,         // Main app mode
}

// Checked every inference iteration
loop {
    if cancel_flag.load(Ordering::Relaxed) {
        break; // Immediate abort
    }
    // Continue processing
}
```

### Mutex Usage (Minimized)

**Only used for complex state requiring consistency**

```rust
// RwLock for settings (read-heavy, write-rare)
pub settings: Arc<RwLock<VoxSettings>>,

// Mutex for UI state (async compatibility)
pub hud_visible: Mutex<bool>,

// Mutex for pipeline state (state machine)
pub state: Arc<Mutex<InteractionState>>,
```

### Channel Communication

**Lock-free inter-thread messaging**

```rust
// Bounded channels prevent memory explosion
let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
let (pipeline_tx, pipeline_rx) = mpsc::channel::<VoxEvent>();

// crossbeam for high-throughput telemetry
let (telemetry_tx, telemetry_rx) = bounded::<TelemetryEvent>(4096);
```

---

## 21. Tokio Workers vs OS Threads

---

### OS Threads (Inference Workers)

**Dedicated OS threads for blocking C++ inference**

```rust
// spawn_stt_worker - OS thread
std::thread::Builder::new()
    .name("vox-stt-worker".to_string())
    .spawn(move || {
        // Blocking sherpa-onnx calls
        let result = recognizer.decode_stream(&stream);
    })
```

**Why OS threads:**
- llama.cpp blocks for seconds
- onnxruntime C++ calls are synchronous
- No async runtime compatibility
- Precise CPU affinity control

### Tokio Workers (IPC & UI)

**Async tasks for Tauri IPC and UI coordination**

```rust
// Tauri command handlers - tokio tasks
#[tauri::command]
pub async fn engage(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // Async state access
    let mut engine_lock = state.engine.lock().await;
    // ...
}
```

**Why tokio:**
- Tauri requires async commands
- UI event handling
- File I/O operations
- Network requests (future)

### Hybrid Architecture

```text
OS Threads (Inference):
├── VAD worker (Tier 2)
├── STT worker (Tier 2)
├── LLM worker (Tier 3)
└── TTS worker (Tier 3)

Tokio Tasks (Coordination):
├── IPC handlers
├── UI state updates
├── Persistence workers
└── Monitoring collectors
```

---

## 22. Memory Lifecycle Management

---

### Model Residency (Warm/Cold States)

```rust
enum ModelState {
    Cold,  // Unloaded, minimal RAM
    Warm,  // Loaded, ready for inference
}

impl PipelineOrchestrator {
    pub fn warm_up_llm(&self) -> Result<(), String> {
        // Load GGUF into memory
        let model = LlamaModel::load_from_file(&backend, &path, &params)?;

        // Initialize context
        let ctx = model.new_context(&backend, ctx_params)?;

        // Mark as loaded
        self.is_llm_loaded.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn cool_down_llm(&self) {
        // Drop context and model
        drop(self.llm_ctx);
        drop(self.llm_model);

        // Clear residency flag
        self.is_llm_loaded.store(false, Ordering::Relaxed);
    }
}
```

### Buffer Lifecycles

**Ring buffers persist across interactions**

```rust
// Audio ring buffer - never deallocated
pub struct AudioStream {
    producer: HeapProd<f32>,  // 4s capacity
    _stream: cpal::Stream,     // CPAL handle
}

// Per-turn buffers - recycled
pub struct PipelineOrchestrator {
    token_buf: String,           // Cleared after each turn
    turn_user_text: String,      // Cleared after persistence
    turn_assistant_text: String, // Cleared after persistence
}
```

### Memory Safety Guarantees

```rust
// Explicit KV cache clearing (llama.cpp requirement)
ctx.clear_kv_cache();

// Buffer size limits prevent OOM
const MAX_PTT_SAMPLES: usize = 16000 * 60 * 10; // 10min hard limit

// Overflow handling with logging
if pushed < upsampled.len() {
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
    let prev = DROP_COUNT.fetch_add(1, Ordering::Relaxed);
    if prev % 100 == 0 {
        log::warn!("Ring buffer overflow: {} chunks dropped", prev);
    }
}
```

---

## 23. Phase 9 — LLM Provider Architecture (v0.8.3)

---

### Motivation

The current LLM implementation treats inference as a single concrete backend:
`llama_cpp.rs` embedded in the voice pipeline. This creates scaling problems:

* Every new backend requires pipeline modifications
* Cloud provider integration becomes difficult
* Future STT/TTS provider support becomes inconsistent

### Target Architecture (v0.8.3)

Move from a single embedded LLM to a **provider-based architecture**:

```text
Vox
 └─ LLM Provider Layer
        ├─ Embedded (local GGUF via llama.cpp)
        └─ OpenAI-Compatible (Ollama, LM Studio, vLLM, etc.)
```

### Provider Interface

Every provider must support:

| Capability | Description |
|-----------|-------------|
| Generate | Submit prompt and receive completion |
| Streaming | Receive tokens incrementally (first-class for real-time) |
| Cancellation | Barge-in must work identically across all providers |
| Health Check | Determine provider availability before use |
| Model Discovery | Fetch available models dynamically |

### Pipeline Impact

The voice pipeline remains unchanged:

```text
Audio → STT → LLM Provider → TTS
```

Only the implementation behind the provider changes. This prevents ripple
effects across VAD, STT, TTS, UI, telemetry, and state management.

### Future Roadmap

| Phase | Scope |
|-------|-------|
| **v0.8.4** | LLM provider architecture & remote OpenAI compatibility |
| **v0.8.5** | Cloud LLM integration — OpenAI, Gemini, and Anthropic cloud APIs via `OpenAiCompatProvider` with `provider_name` routing (current) |
| **v0.9.0** | Cloud Realtime voice-to-voice (full-duplex streaming via OpenAI/Gemini Realtime) |

### Design Principle

The backend should care about **protocol**, not **location**:

```text
localhost
192.168.1.20
gpu-server.local
api.openai.com
```

All of these are simply endpoints. The protocol remains the same.

See `docs/plans/phase9-inference-expansion.md` for the full plan.

---

## 24. Shutdown Sequence

---

### Graceful Cleanup

```rust
// 1. Signal via channels (primary)
engine.pipeline_tx.send(Shutdown);
engine.stt_tx.send(Shutdown);
engine.vad_tx.send(Shutdown);

// 2. Signal via atomics (fallback)
state.pipeline.engine_shutdown.store(true);

// 3. Join threads
orchestrator_handle.join();
stt_handle.join();
vad_handle.join();

// 4. Persistence flush
persist_tx.send(Shutdown);
```

---

