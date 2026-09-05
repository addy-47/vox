# LLM → Playback Pipeline — End-to-End Flow

> **Status:** 2026-09-02 | Verified at `HEAD` (`app/src-tauri/src/` 154 files) | SSOT: `docs/specs/event-domain-matrix.md:27-93` + live code

---

## 0. TL;DR — What Does Current Code Optimise For?

**Primary: TTFA (Time-To-First-Audio), not speech continuity.**

Every layer is cut for first audible byte, continuity is second-class and protected only by a single atomic guard:

| Optimised for TTFA | Evidence |
|---|---|
| Clause-level streaming TTS while LLM still generating | `services/llm/actor.rs:129-149` `push_token -> TtsClauseChunker -> tx.send(Generate)` inside gen loop, not after `Finished` |
| Tiny pre-roll thresholds gate `Thinking→Speaking` | `services/audio/mod.rs:11-12` `MODULAR=12_000` (250 ms) / `REALTIME=3_840` (80 ms) @ 48 kHz — first `ingest_chunk` that satisfies it fires `PlaybackStarted` |
| `flush_pre_roll()` on `LlmFinished` forces short utterance | `pipeline/handlers/llm.rs:86,97` `engine.playback_engine.flush_pre_roll()` — utterance < 250 ms would deadlock forever without it (`audit` fix 2026-09-02) |
| Filler speech hides compaction latency | `services/harness/facade.rs:98-132` random `TRANSITION_MESSAGES_EN/HI` → immediate `tts_tx.send(Generate)` + `pending_synthesis_jobs.fetch_add(1)` before real LLM request even dispatched (`pipeline/handlers/transcript.rs:91-93`) |

**Continuity guard is single-atomic and fragile:** `pending_synthesis_jobs: Arc<AtomicU32>` + `turn_armed: AtomicBool` inside `PlaybackEngine`. `services/audio/playback.rs:424-446` defers `PlaybackFinished` while `pending>0` (mid-turn gap filler holds with `last_sample`). No jitter buffer, no playout scheduling, no crossfade — if next clause synthesis stalls > buffer drain time (~30 s ring drains fast), you get underrun (zero-filled with `last_sample` hold) until next `ingest_chunk`. Real fix for long-form speech continuity would need scheduled playout, not just atomic counter.

**Bottom line:** This is a low-latency voice assistant optimised to *start talking fast* on 8 GB CPU. For podcast/long-form where gapless 60 s matters more than 200 ms first byte, you would invert the trade: larger clauses, lookahead buffering, and playout delay.

---

## 1. The 4 Edges in One Picture

```
┌─────────────┐     TranscriptFinal(valid)      ┌──────────────────┐     LlmCommand::Generate     ┌──────────────┐    LlmStreamEvent::Token     ┌─────────────────┐   TtsCommand::Generate   ┌────────────┐  ingest(24k→48k)  ┌────────────────┐  VoxEvent::Playback*  ┌─────────┐
│  STT Worker │ ───────────────────────────────▶ │ transcript.rs:28 │ ───────────────────────────▶ │  LLM Worker  │ ───────────────────────────▶ │TurnAccumulator│ ────────────────────────▶ │ TTS Worker │ ─────────────────▶ │PlaybackEngine│ ───────────────────────▶ │  Router   │
│  (whisper)  │  pipeline/handlers/transcript  │ spawn_modular_   │  services/llm/actor.rs:95    │  (1 thread)  │  actor.rs:129  accumulator.rs │  push_token() │  services/tts/actor.rs:30  │ (1 thread) │  audio/playback  │  playback.rs:168   │router.rs│
└─────────────┘  on_transcript_final  28..119   │   llm_task       │  spawn_llm_worker            │              │  TtsClauseChunker            │            │  spawn_tts_worker  │                │ ringbuf 30s  │  on_playback_*   │         │
                │                    192        └──────────────────┘                              └──────────────┘                              └─────────────────┘                      └────────────┘                    └────────────────┘                └─────────┘
                          │  prepare_turn_context (facade.rs:38) ─┐
                          │  ├ retrieval (memory) if enabled      │
                          │  ├ threshold maint/filler dispatch    │  ┌──────────────────────┐
                          │  └ GenerationRequest build            │  │   CPAL callback      │
                          │                                       └──▶│  process_output_     │
                                                                      │  buffer() 349        │
                                                                      │  drain_and_telemetry │
                                                                      └──────────────────────┘
```

*Realtime path skips middle entirely:* `transcript.rs:195-199` sets `pending=1`, `RealtimeActor` streams audio directly via `ingest_chunk_i16` (no local TTS), `llm.rs:89-93` resets `pending=0` on `LlmFinished`. The rest of this doc is **Modular** (local pipeline) unless marked.

---

## 2. Canonical Flow Step-by-Step (with file:line)

### Step 0 — Entry gate
`pipeline/handlers/transcript.rs:122-204` `on_transcript_final(turn_id, text, app, state, ctx)`
- Drops if `Idle`/`Paused` or not `Thinking` (`:130-144`).
- Transliteration, empty check → `Ready` + toast (`:155-168`). Non-empty → `set_user_transcript` + `emit_ipc_to TranscriptFinal` + branch.

### Step 1 — `prepare_turn_context` (async, off router)
`transcript.rs:28-119` `spawn_modular_llm_task` spawns `tauri::async_runtime::spawn`.
- Captures `conversation_manager`, `llm_provider` cache, `tts_tx`/`llm_tx` via `blocking_lock` (`:42-48`), `cancel` token, `pending_synthesis_jobs` atomic, `TurnAccumulator` arc.
- `services/harness/facade.rs:38-268` `prepare_turn_context`:
  1. Optional memory retrieval (`:42-66`) — embedding + `retrieve_turn_profile` if `MemoryScope != ChitChat`.
  2. `push_user_turn` + `ContextHarness::needs_threshold_maintenance()` (`:82-88`). If threshold exceeded:
     - Picks random filler `TRANSITION_MESSAGES_EN/HI[ts%len]` (`:98-100`).
     - Chooses FIFO vs compaction LLM by `max_context_tokens <=4096` for Embedded else cloud (`:102-105`).
     - If FIFO or `messages.len()<=3`: `perform_fifo_maintenance` else pops history for compaction job (`:107-112`).
  3. If compaction job exists: **immediately** dispatches filler to TTS (`:120-132`) — this is the TTFA filler path. Then runs `memory::compaction::run_compaction` on `history_slice` (`:152-159` fallback to FIFO on failure).
  4. Builds `ConversationContext` with `session_history_xml`, `kv_synced_index` (Embedded only, else 0) (`:207-211`), token estimate.
  5. Builds `GenerationRequest { ConversationInput, GenerationOptions{temp, max_output_tokens}, Text, Conversation }` (`:254-265`) and returns `(request, transition_speech)`.
- Back in `transcript.rs:80-93`: pattern matches result; if `transition_speech.is_some()` then `pending_jobs.fetch_add(1)` — this one increment covers filler synthesis life. Early cancel check (`:95-101`) before dispatch.
- Dispatches `LlmCommand::Generate { request, turn_id, cancel, accumulator, tts_tx, pending }` via `llm_tx.send` (`:103-111`).

### Step 2 — LLM Worker (single OS thread, threaded tokio runtime inside)
`services/llm/actor.rs:95-199` `spawn_llm_worker(app, rx, provider, event_tx)`
- Builds `tokio new_current_thread` runtime (`:103-105`). Loop on `rx.recv()` (`:108`).
- For `Generate`: creates `stream_tx/stream_rx` sync channel (`:118`), clones provider, spawns `provider.generate(*request, turn_id, cancel, &stream_tx)` on tokio task (`:121-125`).
- **Synchronous drain loop** `while let Ok(event)=stream_rx.recv()` (`:127`): for each `LlmStreamEvent`:
  ```rust
  // actor.rs:129-148
  Token(token) => {
    let clauses = accumulator.lock().push_token(&token); // TtsClauseChunker inside accumulator.rs:35-38
    for clause in clauses {
      pending.fetch_add(1);               // one job per clause
      tts_tx.send(Generate{turn_id, text: clause}) // non-blocking is actually blocking std::sync::mpsc::send
    }
    emit_ipc_to LlmToken {turn_id, token} // bypasses router, direct to webview
  }
  Finished => break
  ```
  *This is the overlap that gives TTFA: TTS starts on first clause while LLM still decoding token 50.*
- `runtime.block_on(gen_handle)` (`:170-189`): on `Ok(Ok(()))` → `event_tx.send(VoxEvent::LlmFinished{turn_id})` (`:172`), else `VoxEvent::Error`. Errors swallowed only as `Error` event (never raw panic).
- `Shutdown` breaks loop (`:191-194`).

**Provider dispatch** — two families behind `LlmProvider` trait (`services/llm/mod.rs:185-192`):

| Provider | File | Streaming innards | TTFT / throughput lever |
|---|---|---|---|
| **Embedded** (`Qwen`/`Gemma` GGUF via `llama.cpp`) | `embedded/mod.rs:19-49`, `worker.rs:32-85`, `generate.rs:345-471` | `LlmWorker::generate` (`:347`) → `prefill_or_reuse_kv_cache` → decode loop `sample_token` → `StreamingEmitter::process_token_bytes` per token bytes (`:71-108`) with `strip_tags_raw` + `partial_tag_len` holdback (`:130-134`) then `tx.send(Token(delta))` | KV-cache hit reuses `current_seq_tokens_len` prefix (`generate.rs:197-256`), else full prefill chunked `DEFAULT_BATCH_CHUNK_SIZE=512` (`:279-311`). `GenerationLimits` soft caps `ctx_size` + `safety=512` (`:18-44`) — **ignores `settings.max_output_tokens`** (audit R1). Qwen sampler chain `penalties, top_k 20, temp 1.0` (`:390-398`). TTFT dominated by prefill (CPU). |
| **Remote** (`OpenAICompat`) | `transport/mod.rs:45-101`, `chat_completions.rs`, `responses.rs`, `ollama.rs`, `sse.rs` | `RemoteTransport::generate:110-191` picks `stream_ollama` if OllamaNative+NumPredict, else `stream_responses` if `Responses`, else `stream_chat_completions`. SSE over `reqwest::Client` (`:77-82` `connect_timeout 5s`, **no total 180s timeout** — audit R7) with `inject_auth_headers`. `sse.rs:17` unbounded `buffer:String` (audit R2). Single retry negotiation flipping `TokenLimitField` on `400 unsupported_parameter` (`:139-187`). | TTFT = network RTT + server prefill (GPU fast). `capability_probe` negotiates `max_tokens` vs `max_completion_tokens`. Cancellation via `cancel.is_cancelled()` poll every `DEFAULT_CANCEL_POLL_INTERVAL_MS=50`. |

### Step 3 — Clause chunking (the algorithm you asked)
`services/tts/actor.rs:242-381` `TtsClauseChunker`

**State:** Single `buffer: String` accumulates raw token fragments.

**API:**
- `push_str(text) -> Vec<String>` (`:257-260`) appends `text` then `extract_chunks()` (`:362-380`).
- `flush() -> Option<String>` (`:263-271`) trims remainder on `LlmFinished` (see `pipeline/handlers/llm.rs:42-68`).
- `clear()` on cancel/interrupt.

**Algorithm — `find_split_point() -> Option<(usize, usize)>` (`:289-359`):**

Pseudo / priority order. Scans `char_indices` left-to-right, first match wins (earliest split):

```
1. EMERGENCY WORD CAP (buffer bloat guard)
   if words.len() >= 25 {               // :293
     return boundary after 20th whitespace // :295-305
   }

2. STRONG sentence terminators (always split)
   if c == '\n' or c == '?' or c == '!'  // :311
     return (pos, len)

3. WEAK sub-clause (comma family) — gated by prosody
   if c in {',',';',';',':','—','–'}     // :316
     word_count = words in buffer[..pos]
     if word_count >= 5 { return split }  // :318-322, else continue
     // intuition: prevents "Hello, world" from stuttering

4. PERIOD '.' — three guards before split
   if c == '.'                           // :327
     if digit '.' digit → continue        // :328-341  e.g. 3.14
     last_word = last token before '.' trimmed of punct // :343-348
     if is_abbreviation(last_word) → continue // :350-352
     else return split

5. No match → None (buffer waits for more tokens)
```

`is_abbreviation` (`:384-409`): lowercases word and checks `ABBREVS` constant `(dr,mr,mrs,ms,prof,sr,jr,st,vs,e.g,i.e,etc,approx,dept,fig,ver,vol,inc,ltd,co,no,p,v1,pp)` + `v{digits}` (`:400`) + single uppercase letter (`:404`) e.g. `"J."`.

`extract_chunks` loop (`:362-380`): while buffer not empty, finds split, takes `buffer[..end] trimmed`, drains `buffer = buffer[end..]`, pushes non-empty. Remainder stays in buffer until next `push_str` or `flush`. So output clauses are **variable length**, 1-~20 words, never mid-abbreviation/decimal.

**Why this shape:** `< 25 words` + `>=5 words for commas` yields 8-14 word clauses — Sweet spot: TTS first-audio latency vs prosody (longer clauses = more natural but higher TTFA). Documented directly in code as *Natural flow & prosody pacing* (`:317`).

### Step 4 — TTS Worker (single OS thread, `ThreadPriority::Max`)
`services/tts/actor.rs:30-77` `spawn_tts_worker(rx, provider, handles)`
- Loop `rx.recv()` (`:37`). For `Generate{turn_id, text}` (`:39`): calls `provider.synthesize_chunk(&text, turn_id, cancel_flag, &playback, event_tx, telemetry_rtf)` (`:41-48`).
- After synthesis returns (success or error), does jitter protection:
  ```rust
  // :50-55
  if let Some(jobs)=pending_synthesis_jobs { 
    let remaining = jobs.fetch_sub(1, Relaxed);
    if remaining <= 1 { playback.flush_pre_roll(); } // last clause: force gate if < threshold
  }
  ```
  So clause accounting is symmetric: `fetch_add(1)` on dispatch side, `fetch_sub(1)` here. Filler uses same pair.

**TTS provider trait** `services/tts/providers/mod.rs:28-37` `fn synthesize_chunk(text, turn_id, cancel, playback, event_tx, telemetry_rtf) -> Result<()>`

| Provider | File | Ingest pattern | Resample / rate | Key knobs | TTFA vs continuity |
|---|---|---|---|---|---|
| **Supertonic** | `providers/supertonic.rs:80-289` `TtsEngine` (Sherpa-ONNX) | **Progressive** via callback `generate_with_config` `Some(|raw: &[f32]| { resample & ingest; true })` (`:242-256`) — audio emitted chunk-by-chunk during diffusion | Native 44.1 kHz → `resample_44100_to_24000` via `BiquadFilter::new_lpf_11k()` Butterworth 11 kHz anti-alias (`:31-78`) then `upsample_2x_into` 24→48 kHz in `PlaybackEngine` | `quality_steps` 2..16 (`MIN..MAX_SUPERTONIC:22`), `speed` 0.7..2.0 native in `GenerationConfig` (`:228-235`), `sid` 0..9 voice, `lang` hi/en by `is_devanagari` (`:205-209`) | Best TTFA among locals — first progress callback ~150-300 ms for short clause, progressive avoids buffering. Quality/steps directly trades latency vs fidelity. |
| **Kokoro** | `providers/kokoro.rs:35-189` `KokoroEngine` | **Progressive** same callback shape (`:142-156`), but **native 24 kHz** — no resample (`:153` `ingest_chunk(raw)`) | Native 24 kHz, `speed` 0.7..2.0 native, `sid` voice, `espeak-ng-data` phonemizer | Slightly faster than Supertonic (no 44.1→24k LPF cost). RTF ~0.2-0.4 fastest locals. Ideal for tight TTFA budget. |
| **Chatterbox** | `providers/chatterbox.rs:16-220` `ChatterboxEngine` (chatterbox-rs GGUF) | **Batch** — `engine.lock().synthesize(text)` blocks until **full** PCM (`:172-177`), then `for chunk in output.chunks(TTS_CHUNK_SIZE=2048) ingest_chunk(chunk)` (`:192-197`) | Native 24 kHz `TTS_SAMPLE_RATE`, speed via `apply_speed` linear-interp post-hoc (`:107-126`, `:186-190`), `cfm_steps` 2..10 quality | **Worst TTFA** (full-utterance before first audio). Holding `parking_lot::Mutex<Engine>` across synthesis blocks `set_quality_steps`/`set_speed`. Continuity trivial (all audio ready). Good for clone voice but not latency. |
| **ChatterboxRemote** | `providers/chatterbox_remote.rs:15-310` | **Progressive HTTP stream** — blocking `reqwest::blocking::Client` (30s timeout `:33`), `POST /tts/stream-pcm` (`:267`), `stream_pcm_response` reads 8 kB, decodes `f32 LE` (`:165-167`), drains when `>=2048` (`:172-188`), `apply_speed_stretch` per chunk (`:114-133`) | 24 kHz mono f32 stream | `quality_steps`, `speed` via JSON payload; `health /health` poll. **Blocks TTS worker thread** on `response.read` — stalls whole TTS lane until stream ends. Network jitter directly visible as playback gaps (no adaptive buffer). |
| **EdgeTTS** | `providers/edge_tts.rs:74-385` `EdgeTtsProvider` (MS cloud) | **Batch after download** — `EDGE_TTS_RUNTIME.block_on` (`:293-354`) on nested `new_multi_thread(2)` runtime: `connect_edge_websocket` (3 retries, SHA256 `Sec-MS-GEC` auth `:34-54`), `send_ssml_request` (`:169-218`), `collect_mp3_payload` until `Path:turn.end` (`:221-252`), then `decode_bytes_to_24khz_mono(mp3)` (`:316`) + `chunks(2048)` ingest (`:326`) | 24 kHz MP3 mono `audio-24khz-96kbitrate` → decoded via `audio/decode.rs` + `upsample_2x_into` | Voice `en-US-AriaNeural` default, speed `%` in SSML `rate` (`:274`). No per-chunk callback — TTFA = WS connect + full MP3 transfer + decode. Timeout bug: `collect_mp3_payload` loops `ws_stream.next().await` with only `cancel` check (`:225`), **no total timeout** — hung server blocks TTS thread forever (audit R6). |

**Common tail:** every provider stores RTF `elapsed/audio_duration` into `telemetry_rtf` (`supertonic.rs:284-286`, `kokoro.rs:184-186`, etc.)

### Step 5 — PlaybackEngine (lock-free ring + dual gate)
`services/audio/playback.rs:43-52,80-236` + `services/audio/mod.rs:1-12`

**Buffer:** `HeapRb<f32>::new(PLAYBACK_BUFFER_SAMPLES)` (`playback.rs:86`) where `PLAYBACK_BUFFER_SAMPLES = 48_000 * 30 = 1_440_000` (~5.7 MB mono→stereo, 30 s). Producer `HeapProd<f32>` + scratch `Vec::with_capacity(4096)`. SPSC, allocation-free on hot path.

**Ingest path:**
```
TTS provider callback → playback.ingest_chunk(&[f32]@24kHz) (:138)
  └ ingest_chunk_with_threshold(chunk_24k, threshold) (:143)
       ├ cancel_flag check (Relaxed) → early return
       ├ producer.lock() → upsample_2x_into(chunk_24k, scratch) (:150)  // 24→48 kHz 2x linear
       ├ prod.push_slice(scratch)  // ringbuf; warn on overflow (:152-158)
       └ Gate 1 check:
            if !turn_armed && occupied >= threshold { turn_armed=true; event_tx.send(PlaybackStarted{tid}) }
```
Threshold comes from caller: `ingest_chunk` uses `MODULAR_PREROLL_THRESHOLD_SAMPLES=12_000` (250 ms) (`:139`), `ingest_chunk_i16` (realtime PCM) uses `REALTIME_PREROLL_THRESHOLD_SAMPLES=3_840` (80 ms) (`:205`). So modular waits longer for cushion, realtime fires faster for S2S.

**Gate 1 — Start (`ingest_chunk_with_threshold:160-179`):** Transitions `Thinking→Speaking` only when ring holds enough samples. Guarantees not starting on 10 ms blip. Filler speech hits this first.

**Flush path (short utterance fix):** `llm.rs:86,97` `flush_pre_roll()` + `tts/actor.rs:53` on last job:
```rust
// playback.rs:209-236 flush_pre_roll()
if !cancel && !turn_armed && occupied>0 { turn_armed=true; send(PlaybackStarted{tid}) }
```
Without it, utterance < 250 ms would never reach threshold → never `PlaybackStarted` → deadlock in `Thinking`. `pending_synthesis_jobs` guard in `playback.rs:52` defers `PlaybackFinished` when `pending>0` — realtime multi-packet jitter fix.

**CPAL output thread** `playback.rs:347-455` `process_output_buffer(output: &mut[f32])`
- Runs at `PLAYBACK_SAMPLE_RATE=48 kHz` stereo `BufferSize::Default`, `f32` (`:502-516`).
- `discard_request` → `skip(occupied)` + reset (`:350-354`). `cancel_flag` → drain + silence (`:364-367`).
- `is_speaking || turn_armed` else `fill(0.0)` (`:357-371`) — drains even while `Thinking` once armed (start gate passed).
- `drain_and_telemetry(:384-455)`: `try_pop` per frame, if `None` hold `last_sample` with volume ramp (`PLAYBACK_VOLUME_RAMP_STEP=0.002` click-free `:401-407`), apply `FilterBank` for `low/mid/high` energy, write stereo `output[2*frame]`.
- **Gate 2 — End (`:423-446`)** when `consumer.is_empty()`:
  ```rust
  if pending_jobs > 0 { underruns.fetch_add(1); }          // gap, but hold
  else if armed { turn_armed=false; send(PlaybackFinished{tid}); }
  ```
  So `PlaybackFinished` only when ring empty **and** no synthesis in-flight. Guards network jitter cutting short (`realtime` sets `pending=1` at turn start, resets 0 on server `LlmFinished`, then ring drains).

### Step 6 — Router finish
`pipeline/handlers/llm.rs:72-111` `on_llm_finished` + `pipeline/handlers/playback.rs:8-66`
- `on_llm_finished` checks `Thinking/Speaking` else drop (`:74`).
  - Modular: `flush_modular_tts_remainder` → `accumulator.flush_chunker()` remaining tail as one `Generate`, `fetch_add(1)` (`:53`), `send`, then `flush_pre_roll()` (`:86-87`).
  - Realtime: `pending.store(0)` (`:93`) + `flush_pre_roll()` (`:97`).
  - Then `take_assistant_response` + `user_transcript` (`:102-105`), `persist_assistant_turn` (`ConversationManager::push_assistant_turn` + `persist_tx.try_send(TurnCompleted)` `:22-37`).
- `on_playback_finished(:32-66)` drops if not `Speaking`, defers if `pending>0` (`:52`), else `transition(Ready)`.

---

## 3. Provider Combination Matrix — How Choice Changes Playback

### 3.1 LLM Provider Character (what feeds the chunker)

| LLM | Where `LlmStreamEvent::Token` comes from | Token granularity | KV-cache | First token (TTFT) characteristic | Cancellation |
|---|---|---|---|---|---|
| **Embedded Qwen/Gemma** (`embedded/mod.rs:19-49`) | `LlamaContext::decode` + `sample_token` loop (`generate.rs:403-446`), `StreamingEmitter` holds back partial tag bytes via `partial_tag_len` (`:133`) | Per sampled token bytes → `from_utf8` batching, tag-stripped delta (`:130-149`) | Yes — `CacheState {system_prompt, system_tokens_len, current_seq_tokens_len}` (`worker.rs:15-19`), hit reuses prefix (`generate.rs:197-256`) | CPU-bound: prefill 512-token chunks + decode per token. Warm Qwen 0.8B ~300-600 ms TTFT on 8 GB CPU, TPS 8-15. Cold miss slower. | `cancel.is_cancelled()` per batch chunk (`:282-296`) + per token loop (`:404`) + `stream_rx.recv` interrupt |
| **Remote OpenAI-compat** (`transport/mod.rs:45-305`) | `chat_completions::stream_chat_completions` / `responses::stream_responses` SSE over `reqwest::Client` | SSE `data: {"choices":[{"delta":{"content":"..."}}]}` parsed by `sse.rs` | No (server-side) | Network RTT + server GPU prefill. Often 150-400 ms TTFT if server warm, faster than embedded for long prompts | Same `cancel` poll (`DEFAULT_CANCEL_POLL_INTERVAL_MS 50`) |

*No combo changes chunking* — chunker sits downstream of `push_token` regardless of LLM. Faster LLM only means `push_token` calls arrive closer together → chunker may accumulate larger `buffer` before punctuation, but output clauses identical.

### 3.2 TTS Provider Character (what fills the ring)

| TTS | Ingest model | First audio latency lever | Stall behaviour |
|---|---|---|---|
| **Kokoro** | Progressive callback per synthesis frame, native 24 kHz → `ingest_chunk` direct | Fastest locals — no resample, 2 threads ONNX. Short clause "Hello there." ~180-280 ms on CPU | Callback yields continuously — ring never starves mid-clause; stall only between clauses if LLM slow |
| **Supertonic** | Progressive callback, 44.1 kHz → LPF + `resample_44100_to_24000` (`:62-78`) per callback then 24→48k | + ~20-50 ms vs Kokoro due to Biquad LPF per sample + linear interp | Same progressive; `quality_steps` linear extra cost (`num_steps` in `GenerationConfig:230`) |
| **Chatterbox local** | Batch — full `synthesize` before any ingest | Slowest — whole clause buffered: 5-word clause ~800-1500 ms, 15-word ~1500-3000 ms | No mid-clause gaps (all samples queued at once as `chunks(2048)`), so `pending>0` guard rarely needed |
| **ChatterboxRemote** | Progressive HTTP f32 stream, 2048-drain | Network + remote GPU. `connect_timeout 5s` + 30 s total, `read 8192` loop. Short clause adds 1 RTT + remote inference ~400-900 ms | `reqwest::blocking::Client::read` blocks TTS worker thread — concurrent clause dispatch stalls until stream finishes. Network jitter → audible gaps (underrun counter increments) |
| **EdgeTTS** | Batch after full MP3 | WS handshake `Sec-MS-GEC` SHA256 + `wss://` 3 retries + full MP3 DL + `decode_bytes_to_24khz_mono`. Short clause ~900-1800 ms even on good net | `block_on` nested runtime holds thread; `collect_mp3_payload` no timeout → infinite stall possible (audit R6). No progressive, so cannot hide LLM streaming benefit |

### 3.3 Combined Effects (the asked table)

| LLM + TTS | TTFA (first `PlaybackStarted`) | Continuity / gaps | CPU pressure (8 GB) | Failure mode |
|---|---|---|---|---|
| **Embedded + Kokoro** (**recommended local**) | **Best local TTFA** — LLM TTFT 0.3-0.6s + Kokoro first clause 0.2s + pre-roll 0.25s ≈ **0.8-1.1 s**. Overlap hides ~0.2s. | Good: progressive fills ring while LLM still gen. Gap only if LLM token stall > ring drain. 30 s ring absorbs burst. | High — LLM `n_threads` + Kokoro `num_threads 2` contend. Schedule via OS threads both `Max` priority; under load TPS drops | Either thread panic → `VoxEvent::Error` via `event_tx.send` |
| **Embedded + Supertonic** | + ~50 ms vs Kokoro (LPF). ~**0.85-1.2 s**. | Same as above, but extra resample work adds jitter. | Same contention + LPF cost | Same |
| **Embedded + Chatterbox local** | **Worst local TTFA** — LLM 0.5s + Chatterbox batch 1-2s + 0.25s ≈ **1.8-2.8 s**. No overlap benefit (TTS blocks until clause complete) | **Best local continuity** — once first clause arrives, entire audio chunked at once so no mid-clause underrun | Worst contention: `Engine` mutex held across batch blocks settings hot-update | Single long clause holds worker — next clause queued behind `mpsc` |
| **Embedded + ChatterboxRemote** | LLM 0.5s + network 0.4-0.9s + ingress chunking ≈ **1.1-1.7 s** (network dependent) | Fragile — network read blocks TTS worker, so LLM token burst queues behind. Gaps visible as `underruns` metric | LLM on local CPU, TTS offloaded to remote GPU — CPU relief but TTS thread blocked on `read` | Remote ` /tts/stream-pcm` 30 s timeout, `/health` fail, payload JSON mismatch |
| **Embedded + EdgeTTS** | **Worst TTFA** — LLM 0.5s + WS connect 0.3-1.5s + MP3 DL + decode ≈ **1.7-3.0 s**. Filler helps mask initial gap. | Poor — batch MP3 means 2nd clause can't start until 1st fully downloaded; `pending` guard defers `PlaybackFinished` but audible gap between clauses | LLM local, TTS on MS cloud — CPU relief, but nested `block_on` with 2-thread runtime risks deadlock under load | `generate_sec_ms_gec` clock skew, WS 3 retries fail, `collect_mp3_payload` hang (no timeout), MP3 decode error path sends `Error` |
| **Remote LLM + Kokoro** | **Best overall TTFA if net good** — remote TTFT 0.15-0.4s + Kokoro 0.2s + 0.25s ≈ **0.6-0.85 s**. LLM outruns TTS, so TTS becomes bottleneck | Good — LLM ahead fills clause queue; `pending` builds (2-3 clauses queued), ring stays full. Risk: burst overflow `push_slice` drops samples (`:152-158` warn) | Least CPU pressure — LLM offloaded, only TTS on CPU. Best for 8 GB | Remote `400 unsupported_parameter` token field flip retry, 429 rate-limit, SSE `buffer` OOM (audit R2) |
| **Remote LLM + Supertonic** | Same as above + resample overhead ≈ **0.65-0.9 s** | Same | Same + LPF cost | Same |
| **Remote LLM + Chatterbox local** | Remote LLM fast, but Chatterbox batch kills overlap → TTFA still **1.2-2.0 s**. | Poor overlap — LLM delivers all tokens in <1s, but Chatterbox synthesizes serially per clause, queue builds, `mpsc` unbounded growth (audit E5) | LLM offloaded, Chatterbox CPU heavy alone — feasible | Queue growth unbounded (`tts/actor.rs:205` `mpsc::channel` unbounded) |
| **Remote LLM + EdgeTTS** | Both network → TTFA **1.0-2.2 s**, high variance, filler essential | Worst continuity — two network sequential hops, no progressive; `PlaybackStarted` late, gaps between clauses | Minimal CPU (both remote/cloud) — lightest local load | Double network failure domain; either side 5 s connect timeout stacks |
| **Remote LLM + ChatterboxRemote** | Both remote streaming → TTFA ~ **0.9-1.5 s** but TTS progressive hides some | Moderate — remote token stream + remote PCM stream can pipeline, but single TTS worker thread serialises clauses | Minimal CPU | Two blocking `reqwest::blocking` holds thread twice |

**Rule of thumb at 8 GB:**
- Want **lowest TTFA** → `Remote LLM (OpenAICompat) + Kokoro` (or Supertonic if Kokoro voice not desired). Next: `Embedded + Kokoro`.
- Want **voice clone / best clone fidelity** → `Chatterbox` (local or remote) but accept TTFA penalty; use `quality_steps` lowest viable (2-4) and short clauses via emergency 20-word cap to shorten batch.
- Want **lowest CPU** → any remote LLM + cloud TTS (EdgeTTS) but pay with latency variance.
- Never pair **Embedded + Chatterbox local + long prompts** on 8 GB — both contend for CPU cache, prefill + diffusion starve each other.

### 3.4 How thresholds modulate all combos

- Modular `12_000` vs Realtime `3_840` (`audio/mod.rs:11-12`) — changing these trades *perceived* latency vs *crackle*:
  - Lower → faster `Thinking→Speaking` but ring can empty mid-clause (more `underruns` metric, `last_sample` hold audible as stretch).
  - Higher → more cushion, higher TTFA.
- `flush_pre_roll` in `llm.rs:86,97` and `tts/actor.rs:53` is non-negotiable — without it, any provider where `audio_duration < threshold/48000` (e.g. filler "One moment..." ~180 ms @ 80 ms realtime OK, but @ 250 ms modular not) deadlocks in `Thinking`.
- `TTS_CHUNK_SIZE=2048` (`tts/mod.rs:18`) at 24 kHz = 85 ms per chunk — granularity of `ingest_chunk` calls from batch providers; progressive providers ignore it and call per progress callback.

---

## 4. Failure & Edge Paths

- **Empty remainder after LLM:** `flush_modular_tts_remainder` (`llm.rs:42-68`) handles unpunctuated tail; if TTFA filler was dispatched, `pending` already 1 so `LlmFinished` does not orphan filler play.
- **Cancel/barge-in:** `on_interrupt` (`pipeline/handlers/interrupt.rs`) + `PlaybackEngine::cancel()` (`playback.rs:285-289` sets `cancel_flag`+`discard_request`), TTS providers check `cancel.load(Relaxed)` per chunk / callback and return early without `Error`. Router clears `Accumulator` and bumps turn via `next_turn()`.
- **Overflow:** `prod.push_slice` returns `pushed < len` → warn drop (`playback.rs:152-158`). Occurs if Remote LLM floods clauses faster than TTS drains and ring 30 s fills (rare, indicates TTS stall).
- **Blocking pitfalls (audit R6,R7):** `EdgeTtsProvider::synthesize_chunk` `EDGE_TTS_RUNTIME.block_on` + `collect_mp3_payload` no timeout, and `RemoteTransport` client no `timeout(180s)` — hung remote hangs respective worker thread indefinitely. Fix: wrap `collect` in `timeout(30s)` and add `.timeout(180s)` to `transport/mod.rs:77`.

---

## 5. File Map (navigate)

| Concern | File:Line |
|---|---|
| `TranscriptFinal` → `spawn_modular_llm_task` | `pipeline/handlers/transcript.rs:28,122` |
| `prepare_turn_context` + filler/compaction | `services/harness/facade.rs:38-268` + `turn_id_sync_architecture_spec` |
| `LlmCommand::Generate` dispatch & `pending` filler inc | `pipeline/handlers/transcript.rs:91-117` |
| `spawn_llm_worker` loop, `Token→clause→TTS` overlap | `services/llm/actor.rs:95-199:118-163` |
| `TurnAccumulator` + `TtsClauseChunker` | `pipeline/handlers/accumulator.rs:17-59` + `services/tts/actor.rs:242-381` |
| `find_split_point` + `is_abbreviation` | `services/tts/actor.rs:289-409` |
| `TtsProvider` trait + 5 providers | `services/tts/providers/mod.rs:28` + `supertonic.rs:158`, `kokoro.rs:82`, `chatterbox.rs:129`, `chatterbox_remote.rs:210`, `edge_tts.rs:254` |
| `spawn_tts_worker` + `pending fetch_sub / flush_pre_roll` | `services/tts/actor.rs:30-77:50-55` |
| `LlmFinished` remainder + persistence | `pipeline/handlers/llm.rs:42-111` |
| `PlaybackEngine` ring, ingest, gates | `services/audio/playback.rs:43,86,137-236,347-455` + `services/audio/mod.rs:11-12` constants |
| `PlaybackStarted/Finished` router | `pipeline/handlers/playback.rs:8-66` |
| Realtime bypass (no local TTS) | `pipeline/handlers/transcript.rs:195-199` + `pipeline/handlers/llm.rs:89-99` + spec `§1.6` |
| Event SSOT + IPC bypass (`LlmToken` direct) | `core/events.rs:10-45` + `pipeline/router.rs:18-68` |
| Spec invariants (4 domains, barge-in, pause vs teardown) | `docs/specs/event-domain-matrix.md:1-94` |

---

*Generated read-only at HEAD; `docs/plans/` ignored as stale per your directive. To retune, change `services/audio/mod.rs:11-12` thresholds and `services/tts/actor.rs:293-305` 25/20 caps, then measure TTFA vs `underruns` via `monitoring/profiler`.*
