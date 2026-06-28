# Vox — End-to-End Voice Pipeline Flow

> **Purpose:** Trace a single voice interaction from microphone to speaker, documenting every stage's algorithm, data format, and metrics. This is the canonical reference for how audio flows through the Vox stack.

---

## Pipeline Overview

```
MODULAR PATH:
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  Audio   │ →  │   VAD    │ →  │   STT    │ →  │   LLM    │ →  │   TTS    │ →  Playback
│ Capture  │    │ Detection │    │(Nemotron)│    │ (Llama)  │    │(Supertonic)│
└──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘
      │               │               │               │               │
  16kHz PCM       Speech VAD      Transcript       LLM Tokens     24kHz PCM
  mono @10-20ms   events          (text)           (streaming)     → 48kHz

  REALTIME S2S PATH (alternative):
  ┌──────────┐    ┌──────────┐    ┌─────────────────────────┐    ┌──────────┐
  │  Audio   │ →  │  Audio   │ →  │   WebSocket Provider    │ →  │Playback  │
  │ Capture  │    │ Router   │    │ (Gemini/Deepgram Live)  │    │ Engine   │
  └──────────┘    └──────────┘    └─────────────────────────┘    └──────────┘
       │               │                     │                        │
  16kHz PCM       256-sample          Server-side                24kHz→48kHz
                   chunks             STT+LLM+TTS                  upsampled
```

---

## Stage 1: Audio Capture

### Implementation
- **Library:** `cpal` (Rust cross-platform audio)
- **Format:** 16 kHz mono PCM, f32 samples
- **Chunk size:** 10–20 ms (160–320 samples)
- **Callback:** Real-time audio callback, zero-allocation buffers

### Transport
Audio flows from the CPAL callback into a **lock-free SPSC ring buffer**:

| Property | Value |
|----------|-------|
| Buffer type | `HeapProd`/`HeapCons` (crossbeam) |
| Capacity | 64,000 samples (4 seconds) |
| Producer | CPAL audio callback |
| Consumer | VAD worker thread |
| Overflow | Drops oldest, logs every 100th drop |

### Resampling
If the input device is not 16 kHz, linear interpolation resamples to 16 kHz in the callback.

### Audio Router (v0.9.0+)
In Realtime S2S mode, the AudioRouter thread (`services/audio/router.rs`) replaces VAD as the direct consumer. It reads 256-sample chunks and routes based on RouteMode:
- `RouteMode::LocalVad` — forward to VAD actor (modular mode + realtime PTT)
- `RouteMode::DirectRealtime` — convert f32→i16 and send to WebSocket session

---

## Stage 2: Voice Activity Detection (VAD)

### Available Backends

| Backend | Format | Threshold | Latency | Notes |
|---------|--------|-----------|---------|-------|
| **Earshot** (Default) | Native Rust energy-based | 0.5 (default) | ~1ms | Ultra-low latency, no model file, ~20x faster than TenVAD |
| **Ten VAD** (Legacy) | ONNX FP32 via sherpa-onnx | 0.45 (default) | ~15ms/frame | CPU-efficient, requires model file |

### VAD Algorithm

The VAD runs on a dedicated OS thread consuming from the ring buffer:

```
loop {
    // Check for hot-update commands (lock-free)
    while let Ok(cmd) = vad_rx.try_recv() { apply_command(cmd); }
    
    if ring_buffer.occupied_len() >= 256 (16ms @ 16kHz) {
        pop 256 samples → chunk
        run VAD prediction on chunk
        
        if speech_detected && !in_speech {
            // Speech onset
            in_speech = true
            send ResetStream to STT
            send SpeechStart to pipeline
        }
        
        if speech_detected {
            buffer chunk into utterance_buffer
            every 800ms: send Partial utterance to STT
        } else if in_speech {
            // Speech offset
            in_speech = false
            if utterance_buffer >= 3200 samples:
                send Final utterance to STT
        }
    } else {
        sleep 5ms (throttle)
    }
}
```

### Pre-roll Buffer
- 500ms sliding window during silence
- Prepended to the first `Partial` chunk on `SpeechStart`
- Captures the speech onset that the VAD may have partially missed

### Key Parameters
| Parameter | Default | Effect |
|-----------|---------|--------|
| `threshold` | 0.45 | Lower = more sensitive, higher = less false positive |
| `min_silence_duration` | 0.5s | How long after speech ends to declare utterance complete |
| `pre_roll_ms` | 500 | Audio before speech onset to include |

---

## Stage 3: Speech-to-Text (Nemotron-3.5)

### Model Details
- **Model:** Nvidia Nemotron-3.5-ASR (INT8 quantized)
- **Runtime:** ONNX Runtime (native, via `ort` crate)
- **Files:** `config.json`, `encoder.onnx` (657MB), `decoder_joint.onnx` (98MB), `tokenizer.model`
- **Memory:** ~2.5 GB RSS
- **RTF:** 0.02–0.35× (average 0.18×) — **22× faster than Qwen3-ASR**

### Backend Selection
Vox supports two STT backends via the SttEngine trait:
- **Embedded** (primary): local ONNX Nemotron-3.5-ASR as documented below
- **Cloud** (future): planned API-based STT via provider trait

### Chunked Transcription Algorithm

A critical fix in v0.8.2: audio is fed in sequential 8960-sample windows through the ONNX session, with `reset_state()` called only at the very end.

```
fn transcribe(audio: &[f32]) -> String {
    let window_size = 8960;  // ~560ms @ 16kHz
    let mut offset = 0;
    
    while offset < audio.len() {
        let end = min(offset + window_size, audio.len());
        let chunk = &audio[offset..end];
        
        // Feed chunk to ONNX session
        session.run(ORTFeed {
            name: "audio_signal",
            tensor: chunk,
        });
        
        offset = end;
    }
    
    // Only NOW reset state — keeps context across all chunks
    session.reset_state();
    
    // Decode final output
    decode_output(session)
}
```

### STT Command Types

```rust
pub enum SttCommand {
    Partial(u32, InteractionOwner, Vec<f32>),  // Streaming partial result
    Final(u32, InteractionOwner, Vec<f32>),    // End of utterance
    ResetStream,                                // Clear decoder state for new turn
    Shutdown,                                   // Exit thread
}
```

### Throttling
- Partial transcripts: every 800ms (to prevent CPU spikes)
- Partial is UI-only feedback — **only `Final` is authoritative**

---

## Stage 4: Language Detection & Prompt Routing

After the STT produces a `TranscriptFinal`, the pipeline decides which language prompt to use:

```rust
fn route_prompt(text: &str) -> String {
    if is_devanagari(&text) {
        // Text contains Devanagari chars (U+0900–U+097F) → modular prompt
        assistant_settings.modular_prompt
    } else {
        // No Devanagari → realtime prompt
        assistant_settings.realtime_prompt
    }
}
```

### Expression Tag Injection
The chosen prompt always gets an expression tag appendix:

```
"{prompt} You may use <laugh>, <breath>, <sigh> tags for expressive speech."
```

These tags are processed by the TTS engine (Supertonic 3) to produce emotional/prosodic variation in the audio output. Verified working: `<laugh>` adds ~18% duration and produces audibly different audio.

---

## Stage 5: LLM Generation (Llama-3.2-1B-Instruct)

### Provider Options
The LLM stage supports multiple backends via the LlmProvider trait:
- **Embedded** (default): local GGUF model via llama.cpp (documented below)
- **Cloud** (optional): OpenAiCompatProvider supports OpenAI, Gemini, Anthropic, and any OpenAI-compatible server (Ollama, vLLM, etc.)

### Embedded Model Details
- **Model:** Llama-3.2-1B-Instruct (Q6_K GGUF)
- **Runtime:** llama.cpp via `llama-cpp-4` crate
- **File:** `Llama-3.2-1B-Instruct-Q6_K.gguf` (~1.02 GB)
- **Memory:** ~970 MB RSS
- **TPS:** 2.5–4.4 (average 3.3 on CPU)
- **Context window:** 2048 tokens (configurable)

### Token Streaming Loop

```rust
loop {
    if cancel_flag.load(Ordering::Relaxed) {
        ctx.clear_kv_cache();
        tx.send(VoxEvent::Cancelled { turn_id });
        return;
    }
    
    let token = ctx.sample();  // Greedy sampling
    if is_eog_token(token) { break; }  // End of generation
    
    let token_str = model.token_to_piece(token);
    if !cleaned.is_empty() {
        tx.send(VoxEvent::LlmToken { turn_id, token: cleaned });
    }
    
    ctx.decode(&mut batch);
}
tx.send(VoxEvent::LlmFinished { turn_id });
```

### Prompt Format
The prompt format is **model-dependent** — `ModelFamily` detection selects the correct template:
- **Llama3** (default example):
  ```
  <|begin_of_text|>
  <|start_header_id|>system<|end_header_id|>
  {system_prompt}<|eot_id|>
  <|start_header_id|>user<|end_header_id|>
  {transcript_text}<|eot_id|>
  <|start_header_id|>assistant<|end_header_id|>
  ```
- **Gemma**: `<bos><start_of_turn>user\n{text}<end_of_turn>\n<start_of_turn>model\n`
- **Qwen**: `<|im_start|>system\n{prompt}<|im_end|>\n<|im_start|>user\n{text}<|im_end|>\n<|im_start|>assistant\n`
- **Nemotron**: `<s><|begin_of_text|>system\n{prompt}<|end_of_text|>\nuser\n{text}<|end_of_text|>\nassistant\n`

### Cancellation
- `cancel_flag` (AtomicBool) checked every token iteration
- On cancel: KV cache cleared, `Cancelled` event sent, TTS queue flushed
- Barge-in: new `SpeechStart` sets cancel flag, interrupting current generation

---

## Stage 6: Chunked TTS Flush (Pipeline Orchestrator)

### The `should_flush` Algorithm (Fully Dynamic)

This is the core algorithm controlling when accumulated LLM tokens are sent to TTS for synthesis. It lives in `utils.rs` and uses **continuous TPS interpolation** — no hardcoded categories.

```rust
pub fn should_flush(buf: &str, word_count: usize, elapsed_ms: u128, tps: f32) -> bool {
    let trimmed = buf.trim_end();
    let last = trimmed.chars().last().unwrap_or(' ');
    
    // 1. Hard boundaries: always flush
    if matches!(last, '.' | '!' | '?' | '।') { return true; }
    
    // ─── Continuous dynamic parameter computation ───
    let tps_clamped = tps.clamp(0.5, 6.0);
    let tps_norm = (tps_clamped - 0.5) / (6.0 - 0.5); // 0.0=slow, 1.0=fast
    
    // 2. Clause boundaries (`,`, `;`, `—`)
    //    Fades out between TPS 3.0–5.0. Word threshold increases 3→7.
    if matches!(last, ',' | ';') || trimmed.ends_with(" — ") || trimmed.ends_with(" - ") {
        if tps_norm < clause_norm_high {
            let clause_threshold = (3.0 + t * 4.0).round() as usize;
            if word_count >= clause_threshold { return true; }
        }
    }
    
    // 3. Time-based flush: scales 1.0s→3.5s, word min 3→8
    let max_wait_ms = lerp(tps_norm, 1000.0, 3500.0) as u128;
    let min_time_words = lerp(tps_norm, 3.0, 8.0).round() as usize;
    if elapsed_ms >= max_wait_ms && word_count >= min_time_words && ends_at_word_boundary(buf) {
        return true;
    }
    
    // 4. Word-count fallback: scales 5→20 words
    let max_words = lerp(tps_norm, 5.0, 20.0).round() as usize;
    if word_count >= max_words && ends_at_word_boundary(buf) {
        return true;
    }
    
    false
}
```

### Word-Boundary Safety (`ends_at_word_boundary`)

Prevents mid-word splits when BPE subword tokens cross word boundaries:

```rust
fn ends_at_word_boundary(buf: &str) -> bool {
    match buf.chars().last() {
        Some(c) => c.is_whitespace() || matches!(c, '.' | '!' | '?' | ',' | ';' | ':' | ')' | ']' | '\u{2014}' | '\u{2013}' | '।'),
        None => true  // empty buffer
    }
}
```

The check ensures the buffer ends at whitespace or punctuation. Together with Devanagari boundary mark `।`, this prevents splits like `हा` + `री`.

### Flush Decision Table

| Condition | When it fires | Example |
|-----------|--------------|---------|
| `. ! ?` | Always | Sentence end |
| `, ; —` | ≥3–7 words (TPS-dependent) | Clause boundary |
| Time gate (1.0s–3.5s) | ≥ time_gate words + word boundary | Slow generation catch-up |
| ≥ fallback_words (5–20) | Word boundary | Large accumulated buffer |

### Dynamic Thresholds (Sample Points)

| TPS | Clause Threshold | Time Gate | Min Words | Fallback |
|-----|:-:|:-:|:-:|:-:|
| ~1.0 (slow) | 3 words | 1.0s / 3 words | 3 | 5 words |
| ~3.5 (medium) | 4 words | 2.2s / 5 words | 5 | 12 words |
| ~6.0 (fast) | Disabled | 3.5s / 8 words | 8 | 20 words |

---

## Stage 7: TTS Synthesis (Supertonic 3)

Vox supports multiple TTS providers via the TtsProvider trait:
- **Supertonic 3** (default): 99M param flow-matching, INT8 quantized, 31 languages, 10 voices
- **Chatterbox Local**: 340M param, voice cloning from 5s reference audio, ~1.1GB RAM
- **Chatterbox Remote**: Offload to GPU server, 0MB local RAM
This section covers Supertonic 3. See `docs/backend.md` for Chatterbox details.

### Model Details
- **Model:** Supertonic 3 — 99M param flow-matching TTS
- **Runtime:** sherpa-onnx native (C++)
- **Quantization:** INT8 (~144 MB total across 7 model files)
- **Languages:** 31
- **Voices:** 10 (James, David, Alex, Ryan, Ethan, Sophia, Olivia, Emma, Ava, Mia)
- **Memory:** ~21 MB RSS
- **Output sample rate:** 44.1 kHz (internally resampled to 24 kHz by progress callback)

### Architecture

```text
TTS Worker (OS Thread):
  TtsCommand::Generate { text }
    → supertonic::synthesize(text, config)
    → progress_callback (each chunk of audio: resample 44.1k→24kHz)
    → send TtsChunk { samples } to pipeline
    → send TtsFinished { rtf } on completion
```

### Model Files (7 files, flat directory)

| File | Size | Purpose |
|------|------|---------|
| `duration_predictor.int8.onnx` | 3.7 MB | Predicts phoneme durations |
| `text_encoder.int8.onnx` | 36.4 MB | Encodes text to latent space |
| `vector_estimator.int8.onnx` | 78.4 MB | Flow-matching vector field |
| `vocoder.int8.onnx` | 26.0 MB | Converts mel to waveform |
| `tts.json` | 8.3 KB | Model configuration |
| `unicode_indexer.bin` | 262 KB | Unicode character index |
| `voice.bin` | 517 KB | Voice style embeddings |

### Supertonic Config

```rust
OfflineTtsSupertonicModelConfig {
    duration_predictor: path,
    text_encoder: path,
    vector_estimator: path,
    vocoder: path,
    tts_json: path,
    unicode_indexer: path,
    voice_style: path,
}

GenerationConfig {
    sid: voice_id,        // 0-9
    num_steps: 8_i32,     // quality (2-12)
    speed: 1.0,           // 0.7-2.0
    extra: { "lang": "hi"|"en" }  // Language hint
}
```

### Anti-Aliasing Low-Pass Filter (v0.8.2+)

Supertonic 3's vocoder produces audio at 44.1kHz. The progress callback downsamples to 24kHz for TTS delivery. To prevent aliasing artifacts from high-frequency content near Nyquist (22.05kHz), a 2nd-order Butterworth LPF is applied before downsampling:

- **Type**: Biquad low-pass filter (2nd-order Butterworth)
- **Cutoff**: 11000 Hz (below 24kHz Nyquist of 12000 Hz, with 1kHz margin)
- **Execution**: Applied sample-by-sample in the resampling loop

```rust
let mut lpf = BiquadFilter::new(BiquadType::Lpf, 11000.0, 44100.0);
for i in 0..output_samples {
    let filtered = lpf.process(supertonic_output[i]);
    interpolated_24k[i] = filtered;
}
```

### Expression Tags
`<laugh>`, `<breath>`, `<sigh>` tags in the input text are processed by the sherpa-onnx Supertonic engine to produce expressive/prosodic variation. These are injected into the LLM system prompt (Stage 4).

---

## Stage 8: Playback Engine

### Architecture

```
TtsChunk (24kHz PCM, f32)
  → upsample_2x() (linear interpolation 24→48 kHz)
  → CPAL ring buffer
  → Audio output device callback (48 kHz)
```

### Upsampling (Cubic Hermite Catmull-Rom)
```rust
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    // Cubic Hermite (Catmull-Rom) interpolation for 24kHz → 48kHz (exact 2x ratio)
    // Uses 4-point basis with weights [-1/16, 9/16, 9/16, -1/16]
    // Produces smoother waveform than linear, continuous 1st derivatives
    let mut out = Vec::with_capacity(len * 2);
    for i in 0..len {
        out.push(input[i]);
        let p0 = if i > 0 { input[i - 1] } else { input[i] };
        let p2 = if i + 1 < len { input[i + 1] } else { input[i] };
        let p3 = if i + 2 < len { input[i + 2] } else { p2 };
        let midpoint = (-p0 + 9.0 * input[i] + 9.0 * p2 - p3) / 16.0;
        out.push(midpoint);
    }
    out
}
```

### Jitter Buffer
| Property | Value |
|----------|-------|
| Pre-buffer | 300 ms (14,400 samples) |
| Total capacity | 2 s (192,000 samples) |
| Drop policy | Log warning, never block |

### Barge-In (Interruption)
```rust
// VAD thread checks during playback:
if playback_active.load(Ordering::Relaxed) && mode == Speaker {
    // Drop microphone frame — user is speaking over assistant
}
```

When user speech is detected during playback:
1. `cancel_flag` is set
2. LLM generation stops (checked every token)
3. TTS queue is flushed
4. Playback stops
5. New turn begins

### Playback Underrun Protection (v0.8.2+)

When the TTS ring buffer is empty (generation hasn't started or is delayed), a short linear volume fade prevents audible click/pop artifacts:

```rust
// Linear fade to avoid clicks (~10ms fade window at 48kHz)
let step = 0.002f32;
if current_volume < target_volume {
    current_volume = (current_volume + step).min(target_volume);
} else if current_volume > target_volume {
    current_volume = (current_volume - step).max(target_volume);
}
```

- **Duration**: ~10ms (96 samples at 48kHz, step=0.002)
- **Direction**: Smooth transition between silence and active playback (bidirectional)
- **State reset**: Volume resets to 1.0 on `PlaybackFinished`/`Cancelled`
- **Tradeoff**: 10ms fade is imperceptible; prevents the DC pop that would result from abrupt ring buffer underrun

---

## Metrics & Timing

### Key Metrics

| Metric | Definition | v0.8.2 Average | Target |
|--------|-----------|---------------|--------|
| **STT RTF** | STT processing time / audio duration | **0.04–0.31×** | < 1.0× |
| **LLM TPS** | Tokens generated per second | **1.2–3.2** | > 1.0 |
| **TTFA** | Time from speech end to first audio | **15.5–25.3s** | < 30 s |
| **TTFT** | Time from speech end to first LLM token | **~4.0s** | — |
| **LLM Mem** | LLM process memory load | **969–970 MB** | < 1,500 MB |
| **Peak RSS** | Peak process memory | **2,446–2,503 MB** | < 7,500 MB |

### Metric Collection
Each interaction records:
- `stt_proc_sec`: Wall clock time of STT processing
- `llm_proc_sec`: Wall clock time of LLM generation
- `tts_proc_sec`: Wall clock time of TTS synthesis
- `ttfa_sec`: Speech end → first TTS chunk output
- `ttft_sec`: Speech end → first LLM token
- `llm_tps`: Generated tokens / LLM processing time
- `stt_rtf`: STT processing time / input audio duration
- `tts_rtf`: TTS processing time / output audio duration
- `peak_process_rss_mb`: Peak memory usage

---

## Interaction State Machine

```
           ┌──────────────────────────────────────────────────┐
           │                                                  │
           v                                                  │
    ┌──────────┐  SpeechStart  ┌──────────────┐  STT Final  ┌──────────┐
    │   Idle   │ ────────────→ │  Listening   │ ───────────→ │ Thinking │
    └──────────┘               └──────────────┘              └──────────┘
         ↑                          │                              │
         │                          │ Barge-in                     │
         │     PlaybackFinish       │ (new SpeechStart)            │ LLM Token
         │     ┌────────────────────┘                              │
         │     ↓                                                   ↓
    ┌──────────┐                                            ┌──────────┐
    │Assistant │←───────────────────────────────────────────│  User    │
    │Speaking  │  First TTSChunk                             │Speaking │ (handoff)
    └──────────┘                                            └──────────┘
```

---

## Performance Budget (Measured v0.8.2)

| Component | Memory (RSS) | Typical Latency | Notes |
|-----------|-------------|-----------------|-------|
| VAD (Earshot) | ~50 MB | ~1ms per frame | Always loaded, no model file |
| VAD (Ten) | ~50 MB | ~15ms per frame | Legacy, requires model file |
| STT (Nemotron) | ~2,500 MB | 0.04–0.31× RTF | INT8 quantized ONNX |
| LLM (Llama-3.2-1B) | ~970 MB | 2.5–4.4 TPS | Q6_K quantization |
| TTS (Supertonic) | ~21 MB | 0.79–1.50× RTF | INT8 quantized |
| TTS (Chatterbox Local) | ~1,100 MB | TBD | Optional, 340M param voice cloning |
| **Total Peak** | **~3,541 MB** | — | ~4,641 MB with Chatterbox |

---

## Event Flow Sequence

The event flow below shows the **modular** (VAD→STT→LLM→TTS) path. In **Realtime S2S mode**, the flow is different:
- Audio capture → AudioRouter → WebSocket session (direct server-side STT+LLM+TTS)
- Server sends audio frames directly to the playback bridge
- No intermediate VAD/STT/LLM/TTS events — only session lifecycle events (SessionStarted, RealtimeAudioReceived, SessionEnded)

```
 VAD                          Pipeline                  STT                      LLM                      TTS              Playback
  │                              │                       │                        │                        │                  │
  │──── SpeechStart ────────────→│                       │                        │                        │                  │
  │                              │──── ResetStream ─────→│                        │                        │                  │
  │                              │                       │                        │                        │                  │
  │──── (audio chunks) ─────────→│ (forwarded)           │                        │                        │                  │
  │                              │──── Partial(audio) ──→│                        │                        │                  │
  │                              │                       │                        │                        │                  │
  │──── SpeechEnd ──────────────→│                       │                        │                        │                  │
  │                              │──── Final(audio) ────→│                        │                        │                  │
  │                              │                       │                        │                        │                  │
  │                              │←── TranscriptFinal ───│                        │                        │                  │
  │                              │     (detect language) │                        │                        │                  │
  │                              │──── Generate(text) ──→│                        │                        │                  │
  │                              │                       │                        │                        │                  │
  │                              │←── LlmToken(token) ───│                        │                        │                  │
  │                              │     (buffer & flush)  │                        │                        │                  │
  │                              │──── Generate(chunk) ──────────────────────────→│                        │                  │
  │                              │                       │                        │                        │                  │
  │                              │←── TtsChunk(samples) ─────────────────────────│                        │                  │
  │                              │                                                     ──→ Play(chunk) ──→│                  │
  │                              │                       │                        │                        │                  │
  │                              │←── LlmFinished ───────│                        │                        │                  │
  │                              │──── (remainder) ───────────────────────────────→│                        │                  │
  │                              │                       │                        │                        │                  │
  │                              │←── TtsFinished ────────────────────────────────│                        │                  │
  │                              │                                                      ──→ (done) ──────→│── PlaybackFin →│
```

---

## Appendix: Flush Algorithm Decision Flow

```
LLM Token arrives
       │
       ▼
Append to token_buf
       │
       ▼
Compute word_count, elapsed_ms, tps
       │
       ▼
┌─────────────────────────────────────────────────────┐
│ should_flush(token_buf, word_count, elapsed_ms, tps) │
└─────────────────────────────────────────────────────┘
       │
       ├─ Hard boundary (. ! ?) ───────────────────→ YES
       │
       ├─ Soft boundary (, ; —) + word_count ≥ soft_words ──→ YES
       │
       ├─ elapsed_ms > 1500 + word_count ≥ time_gate_words ──→
       │      └─ ends_at_word_boundary(buf)? ────→ YES / NO
       │                                                    │
       ├─ word_count ≥ fallback_words ──────────────────→   │
       │      └─ ends_at_word_boundary(buf)? ────→ YES / NO │
       │                                                    │
       └──────────────────────────────────────────────→ NO  │
                                                            ▼
                                           YES → flush to TTS, clear buffer
                                           NO  → continue accumulating
```
