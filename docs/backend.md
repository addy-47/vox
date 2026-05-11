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

Target: **<500ms voice-to-voice**

Every component must minimize:

* buffering
* blocking
* memory allocation

---

## 3. System Topology

---

### Architecture

```text
[Tauri (Rust)]
    ├── Audio Capture (cpal)
    ├── Event Bus (crossbeam_channel / mpsc)
    ├── UI IPC (Tauri Events)
    │
    ↓
[C++ Inference Layer]
    ├── VAD (TenVAD via sherpa-onnx)
    ├── STT (Qwen3-ASR via sherpa-onnx)
    ├── LLM (llama.cpp)
    └── TTS (Kokoro + Piper via sherpa-onnx)
```

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

---

## 6. Voice Activity Detection (VAD) - Tier 2

---

### Model

* TEN VAD (ONNX via sherpa-onnx)
* threshold: configurable (default 0.45)
* min_silence_duration: 0.5s

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

### Model

* Qwen3-ASR-0.6B (INT8 ONNX via sherpa-onnx)
* 4 files: `conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, `tokenizer`

---

### Worker Thread

```rust
spawn_stt_worker(
    rx: SttCommand,           // Commands from VAD
    model_path: PathBuf,
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

### Throttling

Partial transcripts throttled to `STT_THROTTLE_MS = 800ms` to prevent CPU spikes.

---

## 8. Language Model (LLM) - Tier 3

---

### Model

* Gemma 4 E2B-it (GGUF, Q4_K_M)
* llama.cpp backend

---

### Worker Thread

```rust
spawn_llm_worker(
    rx: mpsc::Receiver<LlmCommand>,
    model_path: PathBuf,
    event_tx: mpsc::Sender<VoxEvent>,
    is_loaded: Arc<AtomicBool>,
)
```

---

### LlmCommand Types

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

### Token Streaming Loop

```rust
loop {
    if cancel_flag.load(Ordering::Relaxed) {
        ctx.clear_kv_cache();
        tx.send(VoxEvent::Cancelled { turn_id });
        return Ok(());
    }
    
    let token = ctx.sample_greedy();
    if is_eog_token(token) { break; }
    
    let token_str = model.token_to_piece(token);
    if !cleaned.is_empty() {
        tx.send(VoxEvent::LlmToken { turn_id, token: cleaned });
    }
    
    ctx.decode(&mut batch);
}
tx.send(VoxEvent::LlmFinished { turn_id });
```

---

### Prompt Format (Gemma 4)

```text
<|turn>system {system_prompt}<turn|>
<|turn>user {text}<turn|>
<|turn>model
```

---

## 9. Text-to-Speech (TTS) - Tier 3

---

### Models

* English: Kokoro-82M (ONNX via sherpa-onnx)
* Hindi: Piper VITS (ONNX via sherpa-onnx)

---

### Worker Thread

```rust
spawn_tts_worker(
    rx: mpsc::Receiver<TtsCommand>,
    en_tts_dir: PathBuf,
    hi_tts_path: PathBuf,
    event_tx: mpsc::Sender<VoxEvent>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
)
```

---

### Language Detection

```rust
fn is_hindi(text: &str) -> bool {
    text.chars().any(|c| c >= '\u{0900}' && c <= '\u{097F}')
}
```

---

### Chunked Synthesis

Tokens flushed to TTS on:

1. Hard boundaries: `.`, `!`, `?`
2. Soft boundaries: `,`, `;`, ` — `, `-`
3. Word count: ≥6 words

---

## 10. Playback Engine (Tier 3)

---

### Architecture

```text
TtsChunk (24kHz) → upsample_2x() → ring buffer → CPAL callback (48kHz)
```

---

### Upsample Function

```rust
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    // Linear interpolation for 24kHz → 48kHz (exact 2x ratio)
    // O(n), no FFT, no external deps
}
```

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

Directive 2: Flush to TTS on:

* Hard boundaries: `.`, `!`, `?`
* Soft boundaries: `,`, `;`, ` — `, `-`
* Word count: ≥6 words

Target: Time-to-First-Audio ≤ ~500ms.

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

### Budget

```text
~5.5GB usable for inference
VAD:   ~0.05GB
STT:   ~0.80GB
LLM:   ~2.20GB
TTS:   ~0.50GB
KV:    ~0.60GB
Safety: ~1.35GB margin
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

* Spawns every 5 seconds
* Reads `/proc/stat`, `/proc/meminfo` for CPU/RAM

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

## 23. Shutdown Sequence

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

