# LLM-to-TTS-to-Playback Streaming & Pacing Specification

---

## 1. Executive Summary & Purpose

This specification formalizes the two producer-consumer boundaries in the Vox audio synthesis pipeline:
1. **Boundary 1 (LLM → TTS):** Converting an asynchronous streaming token feed into syntactically coherent text clauses for speech synthesis.
2. **Boundary 2 (TTS → Playback):** Converting discrete PCM audio chunks from TTS engines into continuous, glitch-free audio output via the CPAL audio ring buffer.

### Core Objectives:
- **Phase 10 Hardening Contract:** Implement an **extremely simple, robust punctuation-based chunking and pre-roll jitter buffering mechanism** that guarantees stability, speech continuity, and immediate barge-in responsiveness across all providers.
- **Adaptive Roadmap (Deferred):** Formally define the **4-Case Dynamic Buffering Matrix** based on Tokens Per Second (TPS) and Real-Time Factor (RTF) to prioritize acoustic prosody and speech continuity over raw TTFT.

---

## 2. Architecture & Pipeline Dataflow

```
┌─────────────────┐       Token Stream       ┌──────────────────────┐
│   LLM Provider  │ ───────────────────────> │  TtsClauseChunker    │
│ (Cloud / Local) │   (TPS Metric Tracked)   │ (Punctuation Split)  │
└─────────────────┘                          └──────────┬───────────┘
                                                        │ Text Clauses
                                                        ▼
┌─────────────────┐     Discrete Chunks      ┌──────────────────────┐
│ Audio Playback  │ <─────────────────────── │     TTS Engine       │
│  (CPAL 48kHz)   │   (2048-sample blocks)   │ (Chatterbox/EdgeTTS) │
└────────┬────────┘                          └──────────────────────┘
         │
         ▼
┌─────────────────┐
│ User Loudspeaker│ (Continuous Speech, Zero Buffer Underruns)
└─────────────────┘
```

---

## 3. Boundary 1: LLM → TTS (Text Pacing & Clause Splitting)

### 3.1 The Pacing Dilemma: Prosody vs. Latency
- **Small Chunks (Word-by-word / 2–3 words):** Minimal Time-to-First-Audio (TTFT), but destroys acoustic prosody. Neural TTS models require semantic context to calculate pitch inflections, stress, and cadence. Sub-clause synthesis sounds clipped, robotic, and disjointed.
- **Large Chunks (Full paragraphs):** Perfect prosody, but latency increases to multiple seconds.

### 3.2 Phase 10 Hardening Contract (Simplified Punctuation Rule)
During Phase 10 hardening, token streaming is governed by a deterministic punctuation rule:
1. **Accumulator Buffer:** Incoming tokens accumulate in `TtsClauseChunker`.
2. **Clause Split Triggers:**
   - **Primary Sentence Boundaries:** `.`, `?`, `!`, `\n` (with abbreviation / decimal guard).
   - **Secondary Clause Boundaries:** `,`, `;`, `:`, `—`, `–` (flushed only if the accumulated clause exceeds a minimum token threshold, e.g. ≥ 3 words).
3. **Turn Completion Flush:** Upon `EVENT_LLM_FINISHED`, any unpunctuated trailing text in the accumulator is immediately flushed to the TTS actor.
4. **Interruption / Barge-in:** `CHUNKER.lock().clear()` immediately drops unconsumed text on `VoxEvent::Cancelled` or VAD barge-in.

---

## 4. Boundary 2: TTS → Playback (Jitter Buffer & Speech Continuity)

### 4.1 The Continuity Problem (Buffer Underrun)
Playback runs strictly at 1.0x real-time (48,000 samples/sec). If the playback DAC drains the ring buffer faster than the TTS synthesizes subsequent chunks, the consumer experiences a **buffer underrun**, causing audible clicks, pops, or silence gaps mid-sentence.

### 4.2 Phase 10 Hardening Contract (Pre-Roll Jitter Buffer)
1. **Pre-Roll Cushion:** When TTS starts generating for a new turn, `PlaybackEngine` does not immediately open audio output to the DAC.
2. **Trigger Threshold:** `playback.start_playback()` is signaled when either:
   - **Threshold A (Minimum Samples):** The ring buffer accumulates at least $T_{\\text{preroll}} = 250\\text{ms}$ of audio (approx 12,000 samples at 48kHz), OR
   - **Threshold B (First Clause Completed):** The TTS engine emits `VoxEvent::TtsFinished` for the first synthesized clause.
3. **Streaming Ingestion:** Once playback begins, subsequent TTS chunks feed directly into the ring buffer lock-free (`prod.push_slice`), allowing continuous playback across clause boundaries.

---

## 5. TTS Provider Streaming Refactor: Chatterbox & EdgeTTS

### 5.1 Chatterbox Streaming Refactor
- **Current State:** Monolithic `engine.synthesize(text)` executes for the entire clause, then slices output into 2048-sample chunks.
- **Refactor Requirements:**
  1. Maintain 2048-sample chunking (`TTS_CHUNK_SIZE = 2048`) with `cancel.load(Ordering::Relaxed)` evaluated before every chunk dispatch.
  2. Avoid redundant vector cloning when `speed == 1.0` (Sprint 165).
  3. Ensure `TtsFinished` correctly publishes accurate `rtf` telemetry to `AppState`.

### 5.2 EdgeTTS Streaming Refactor
- **Current State:** Gathers the entire WebSocket MP3 payload into memory (`mp3_buffer`), performs a single monolithic decode, and sends one massive `TtsChunk` containing all audio.
- **Refactor Requirements:**
  1. **Chunked Sample Emission:** Slice decoded PCM audio into standard 2048-sample `VoxEvent::TtsChunk` frames rather than a single monolithic event.
  2. **Interleaved Cancel Checks:** Check `cancel.load(Ordering::Relaxed)` during chunk emission so barge-in cancels audio propagation immediately.
  3. **Shared Tokio Runtime:** Utilize the application-wide Tokio runtime handle instead of allocating `Runtime::new()` per utterance (Sprint 159).

---

## 6. Future Adaptive Dynamic Buffering Matrix (Deferred Roadmap)

Once the core application is hardened, an adaptive buffering controller will dynamically adjust chunk thresholds and pre-roll parameters based on live TPS (LLM Tokens Per Second) and RTF (TTS Real-Time Factor).

```
                      TTS Real-Time Factor (RTF)
                      Fast (RTF < 0.5)           Slow (RTF >= 0.8)
                 ┌──────────────────────────┬──────────────────────────┐
  Fast           │  CASE 4: Cloud / Cloud   │  CASE 3: Cloud / Local   │
  (TPS > 30)     │  • Min pre-roll (100ms)  │  • Full sentence chunks  │
                 │  • Sub-clause splitting  │  • Parallel pre-synthesis│
LLM Speed        │  • Ultra-low TTFT        │  • Moderate pre-roll     │
                 ├──────────────────────────┼──────────────────────────┤
  Slow           │  CASE 2: Local / Cloud   │  CASE 1: Local / Local   │
  (TPS <= 15)    │  • Large pre-roll (400ms)│  • Max pre-roll (600ms+) │
                 │  • Full sentence chunks  │  • Paragraph/Clause sync │
                 │  • Wait for token pacing │  • Speech continuity SSOT│
                 └──────────────────────────┴──────────────────────────┘
```

### Case Analysis:
1. **Case 1: Slow LLM + Slow TTS (Local 8GB CPU-only):**
   - *Risk:* Severe buffer starvation between words and sentences.
   - *Strategy:* Prioritize speech continuity above all else. Accumulate complete sentences and buffer 500–800ms of audio before starting playback. Prevent mid-utterance stalls.
2. **Case 2: Slow LLM + Fast TTS (Local LLM + Cloud/EdgeTTS):**
   - *Risk:* TTS finishes synthesis instantly, but playback drains before LLM outputs the next sentence.
   - *Strategy:* Pace TTS generation with a larger pre-roll buffer to prevent audio output from catching up to the slow token generator.
3. **Case 3: Fast LLM + Slow TTS (Cloud Groq + Local Diffusion TTS):**
   - *Risk:* Tokens arrive instantly, but TTS takes seconds to generate audio.
   - *Strategy:* Feed large sentence blocks immediately into the TTS queue. Start playback as soon as the first sentence finishes synthesis.
4. **Case 4: Fast LLM + Fast TTS (Cloud Groq + Cloud ElevenLabs/EdgeTTS):**
   - *Risk:* None.
   - *Strategy:* Aggressive early clause flushing (comma-level), minimal 100ms pre-roll, achieving sub-200ms perceptual latency.

---

## 7. Verification & Invariants

| Component | Invariant | Failure State |
| :--- | :--- | :--- |
| `TtsClauseChunker` | Cleared on barge-in / cancel | Stale assistant speech on interrupt |
| `PlaybackEngine` | Starts DAC only when pre-roll buffer satisfied | Audible audio glitch / underrun |
| `EdgeTtsProvider` | Emits in 2048-sample slices | Monolithic un-cancellable audio burst |
| `ChatterboxEngine` | Evaluates cancel flag per 2048-sample chunk | Synthesis overrun after user speech |
