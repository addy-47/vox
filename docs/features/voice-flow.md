# Vox — End-to-End Voice Pipeline Flow

> **Purpose:** Trace a single voice interaction from microphone to speaker, documenting every stage's algorithm, data format, and metrics. This is the canonical reference for how audio flows through the Vox stack.

---

## Pipeline Overview

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  Audio   │ →  │   VAD    │ →  │   STT    │ →  │   LLM    │ →  │   TTS    │ →  Playback
│ Capture  │    │ Detection │    │(Nemotron)│    │ (Llama)  │    │(Supertonic)│
└──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘
      │               │               │               │               │
  16kHz PCM       Speech VAD      Transcript       LLM Tokens     24kHz PCM
  mono @10-20ms   events          (text)           (streaming)     → 48kHz
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

---

## Stage 2: Voice Activity Detection (VAD)

### Available Backends

| Backend | Format | Threshold | Latency | Notes |
|---------|--------|-----------|---------|-------|
| **Ten VAD** | ONNX FP32 via sherpa-onnx | 0.45 (default) | ~15ms/frame | CPU-efficient, configurable |
| **Earshot** | Native Rust energy-based | configurable | ~1ms | Ultra-low latency, no model file needed |

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
- **Memory:** ~1,265 MB RSS
- **RTF:** 0.02–0.35× (average 0.18×) — **22× faster than Qwen3-ASR**

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
        // Text contains Devanagari chars (U+0900–U+097F) → Hindi prompt
        assistant_settings.hindi_prompt
    } else {
        // No Devanagari → English prompt
        assistant_settings.english_prompt
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

### Model Details
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

### Prompt Format (Llama 3.2 instruct)

```
<|begin_of_text|>
<|start_header_id|>system<|end_header_id|>
{system_prompt}<|eot_id|>
<|start_header_id|>user<|end_header_id|>
{transcript_text}<|eot_id|>
<|start_header_id|>assistant<|end_header_id|>
```

### Cancellation
- `cancel_flag` (AtomicBool) checked every token iteration
- On cancel: KV cache cleared, `Cancelled` event sent, TTS queue flushed
- Barge-in: new `SpeechStart` sets cancel flag, interrupting current generation

---

## Stage 6: Chunked TTS Flush (Pipeline Orchestrator)

### The `should_flush` Algorithm

This is the core algorithm controlling when accumulated LLM tokens are sent to TTS for synthesis. It lives in `utils.rs`.

```rust
pub fn should_flush(buf: &str, word_count: usize, elapsed_ms: u128, tps: f32) -> bool {
    // TPS-adaptive thresholds:
    //   Slow (TPS ≤ 2.0):  {soft=3, time_gate=3, fallback=4}   — prioritize TTFA
    //   Medium (2.0 < TPS ≤ 4.0): {soft=3, time_gate=3, fallback=8}  — balance
    //   Fast (TPS > 4.0):  {soft=5, time_gate=5, fallback=12}  — prioritize prosody
    
    // 1. Hard boundaries: always flush
    if matches!(last, '.' | '!' | '?') => true
    
    // 2. Soft boundaries: flush if enough words for coherent speech
    if matches!(last, ',' | ';') && word_count >= soft_words => true
    if ends_with(" — ") or " - " && word_count >= soft_words => true
    
    // 3. Time-based flush: ≥1500ms + word minimum + word boundary
    if word_count >= time_gate_words && elapsed_ms > 1500 && ends_at_word_boundary(buf) => true
    
    // 4. Word-count fallback: independent of time, with word boundary
    if word_count >= fallback_words && ends_at_word_boundary(buf) => true
    
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
| `, ; —` | ≥3 words | Clause boundary |
| ≥1500ms elapsed | ≥ time_gate words + word boundary | Slow generation catch-up |
| ≥ fallback_words | Word boundary | Large accumulated buffer |

### Dynamic Thresholds by TPS

| TPS Range | soft_words | time_gate_words | fallback_words | Use Case |
|-----------|-----------|-----------------|----------------|----------|
| ≤ 2.0 | 3 | 3 | 4 | Slow CPU, prioritize low TTFA |
| 2.0–4.0 | 3 | 3 | 8 | Normal operation |
| > 4.0 | 5 | 5 | 12 | Fast generation, prioritize prosody |

---

## Stage 7: TTS Synthesis (Supertonic 3)

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
  TtsCommand::Generate { text, voice_sid }
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

### Upsampling
```rust
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    // Linear interpolation for exact 2× ratio (24kHz → 48kHz)
    // O(n), no FFT, no external deps
    let mut output = Vec::with_capacity(input.len() * 2);
    for pair in input.windows(2) {
        output.push(pair[0]);
        output.push((pair[0] + pair[1]) / 2.0);
    }
    output.push(*input.last().unwrap());  // Last sample
    output
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

---

## Metrics & Timing

### Key Metrics

| Metric | Definition | v0.8.2 Average | Target |
|--------|-----------|---------------|--------|
| **STT RTF** | STT processing time / audio duration | **0.18×** | < 1.0× |
| **LLM TPS** | Tokens generated per second | **3.30** | > 1.0 |
| **TTFA** | Time from speech end to first audio | **11.30 s** | < 15 s |
| **TTFT** | Time from speech end to first LLM token | **3.98 s** | — |
| **Peak RSS** | Peak process memory | **2,461 MB** | < 7,500 MB |

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
| VAD (Ten) | ~50 MB | ~15ms per 256-sample frame | Always loaded |
| STT (Nemotron) | ~1,265 MB | 0.03–0.31× RTF | ~945 MB encoder ONNX |
| LLM (Llama-3.2-1B) | ~970 MB | 2.5–4.4 TPS | Q6_K quantization |
| TTS (Supertonic) | ~21 MB | 0.79–1.50× RTF | INT8 quantized |
| **Total Peak** | **~2,461 MB** | — | Well within 8 GB target |

---

## Event Flow Sequence

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
