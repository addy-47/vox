# Phase 10 — Integration Test Specification

**Status:** Final — backend seams unblocked, ready for test engineer execution  
**Location:** `app/src-tauri/tests/`  
**Execution:** `cargo test --test <test_file> --release -- --nocapture`

---

## 0. Test Infrastructure

Before any seam test file is written, the following shared infrastructure must exist. All seam test files import from it.

### Directory Layout

```
app/src-tauri/
├── tests/
│   ├── common/
│   │   ├── mod.rs          # re-exports all submodules
│   │   ├── audio.rs        # WAV decode, ring buffer feed, silence inject, drain-wait
│   │   ├── scoring.rs      # normalize_text, calculate_similarity, assert_similarity_above,
│   │   │                   # compare_acoustic_features, assert_acoustic_within_tolerance
│   │   ├── paths.rs        # model dir resolution, test clip / asset lookup
│   │   └── harness.rs      # spawn_stt_worker, spawn_vad_actor, event drain helpers
│   ├── assets/
│   │   ├── supertonic_01_en_briefing.wav   # TTS golden ref (Supertonic, EN)
│   │   └── supertonic_07_hi_weather.wav    # TTS golden ref (Supertonic, HI)
│   ├── stt_test.rs                → REFACTOR: split into seam files below, helpers → common/
│   ├── passive_streaming_test.rs  # Seam 1
│   ├── modular_ptt_test.rs        # Seam 2
│   ├── realtime_ptt_test.rs       # Seam 3 + Ghost Audio Gate (Seam 6)
│   ├── tts_test.rs                # Seam 4
│   ├── llm_test.rs                # Seam 5
│   ├── vad_ducking_test.rs        # Seam 7
│   └── dictation_ptt_test.rs      # Seam 8
```

### `common/audio.rs` — Required Functions

```rust
// Decode a 16kHz mono WAV to f32 PCM. Handles 16/24/32-bit int and f32 formats.
pub fn decode_wav_to_mono_16k(path: &Path) -> Result<Vec<f32>, String>

// Stream f32 samples to a ring buffer producer in VAD_CHUNK_SIZE chunks. Respects backpressure.
pub fn stream_audio_to_ring_buffer(audio: &[f32], producer: &mut impl Producer<Item=f32>)

// Inject N frames of zero-valued silence into the ring buffer. Triggers VAD speech-end.
pub fn stream_silence_frames(producer: &mut impl Producer<Item=f32>, n_frames: usize)

// Spin-wait until ring buffer producer reports occupied_len == 0.
pub fn wait_for_buffer_drain(producer: &impl Observer, poll_ms: u64)

// Decode a WAV to i16 PCM (realtime PTT uses i16 internally).
pub fn decode_wav_to_i16(path: &Path) -> Result<Vec<i16>, String>
```

### `common/scoring.rs` — Required Functions

```rust
// Strip punctuation, lowercase, collapse whitespace.
pub fn normalize_text(text: &str) -> String

// Normalized Levenshtein similarity [0.0, 1.0].
pub fn calculate_similarity(hyp: &str, reference: &str) -> f32

// Assert similarity >= threshold, panic with a clear diff on failure.
pub fn assert_similarity_above(hyp: &str, reference: &str, threshold: f32, label: &str)

// Acoustic comparison via aubio Rust bindings: F0, RMS, duration, voiced/silence ratio, MFCC.
pub fn compare_acoustic_features(generated: &Path, golden: &Path) -> AcousticReport

// Assert AcousticReport fields within configurable tolerances.
pub fn assert_acoustic_within_tolerance(report: &AcousticReport, tolerances: &AcousticTolerances, label: &str)
```

### `common/paths.rs` — Required Functions

```rust
pub fn get_test_clip_path(filename: &str) -> PathBuf   // resolves from test-clips/
pub fn get_asset_path(filename: &str) -> PathBuf       // resolves from tests/assets/
pub fn get_nemotron_model_dir() -> PathBuf
pub fn get_supertonic_model_dir() -> PathBuf
pub fn get_qwen_model_path() -> PathBuf
```

### `common/harness.rs` — Required Functions

```rust
pub fn setup_stt_worker<R: tauri::Runtime>(app: &AppHandle<R>)
    -> (Sender<SttCommand>, Receiver<VoxEvent>, Arc<AtomicBool>, JoinHandle<()>)

pub fn setup_vad_actor<R: tauri::Runtime>(
    app: &AppHandle<R>,
    stt_tx: Sender<SttCommand>,
    config: VadActorConfig,
    engine_shutdown: Arc<AtomicBool>,
) -> (Sender<VadCommand>, Receiver<VoxEvent>, HeapRb<f32> producer, JoinHandle<()>)

// Drain until TranscriptFinal for turn_id or timeout.
pub fn drain_for_final_transcript(rx: &Receiver<VoxEvent>, turn_id: u32, timeout: Duration)
    -> Result<String, String>

// Drain until N TranscriptFinal events, stitch and return full text.
pub fn collect_final_transcripts(rx: &Receiver<VoxEvent>, expected_turns: usize, timeout: Duration)
    -> String

// Assert channel is empty after a deterministic wait. Mandatory for negative assertions.
// DO NOT substitute with a short recv_timeout — use this function.
pub fn assert_channel_empty_after<T>(rx: &Receiver<T>, wait: Duration, label: &str)
```

---

## Seam 1 — Passive Streaming: Ring Buffer → VAD → STT

**File:** `tests/passive_streaming_test.rs`  
**Status:** ✅ Unblocked

### Phase 1 — Production Path Trace

```
SUT: VAD actor detects speech onset/offset in Passive mode and dispatches audio to STT.

Production Entry Seam:
  Audio pushed to SPSC ring buffer producer (simulates CPAL audio engine output).

Direction Check: PASS — ring buffer push is the upstream trigger.
  The test does NOT call SttCommand::Final directly.

Production Path:
  ring_buffer.push_slice(chunk)
  → VAD Actor: pop_slice, mode=Passive, effective_mode=Passive
  → EarshotVadEngine.process() → speech onset
  → VoxEvent::SpeechStart emitted on vox_event_tx
  → utterance_buffer accumulates chunks
  → silence frames (VAD_SPEECH_END_FRAMES + margin) → speech offset
  → VoxEvent::SpeechEnd emitted
  → SttCommand::Final(turn_id, utterance_buffer) dispatched to stt_tx
  → STT Actor (Nemotron) → VoxEvent::TranscriptFinal on pipeline_event_tx

Observable Exit:
  1. VoxEvent::SpeechStart
  2. VoxEvent::SpeechEnd
  3. VoxEvent::TranscriptFinal.text — similarity >= 0.90

Production functions called:
  setup:   spawn_stt_worker(), spawn_vad_actor() [via common::harness]
  entry:   stream_audio_to_ring_buffer() + stream_silence_frames()
  observe: vox_event_rx (SpeechStart/End), pipeline_event_rx (TranscriptFinal)

Functions written in test file: None — all helpers live in common/.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
|---|---|
| **VAD actor never pops from ring buffer** | **Yes — SpeechStart never fires** |
| Speech onset detection broken | Yes — SpeechStart assertion fails |
| Speech offset detection broken | Yes — SpeechEnd assertion fails; transcript timeout |
| SttCommand::Final never dispatched on offset | Yes — transcript timeout |
| STT produces wrong text | Yes — similarity assertion fails |
| VAD runs in wrong mode (PTT suppresses audio) | Yes — audio suppressed, no speech events |

### Test Functions

```rust
// Primary: EN clip — assert SpeechStart + SpeechEnd + similarity >= 0.90
#[test]
fn test_passive_streaming_en() { ... }

// Primary: HI clip — same assertions
#[test]
fn test_passive_streaming_hi() { ... }

// Guard (NEGATIVE): silence-only input
// Assert: NO SpeechStart, NO TranscriptFinal — use assert_channel_empty_after()
#[test]
fn test_passive_streaming_silence_only() { ... }
```

---

## Seam 2 — Modular PTT: ingest_audio → PTT_BUFFER → handle_ptt_stop_with_sender → STT

**File:** `tests/modular_ptt_test.rs`  
**Status:** ✅ Unblocked — `ingest_audio()` + `handle_ptt_stop_with_sender()` exposed

### Sanity Check Findings

Backend engineer added:
- `pub fn ingest_audio(chunk: &[f32])` at line 28 — writes to `PTT_BUFFER` when `IS_RECORDING=true`. This is the production path that was missing.
- `pub fn handle_ptt_stop_with_sender<R>(app, state, stt_tx: Option<&Sender<SttCommand>>)` at line 213 — accepts injected `stt_tx`, bypasses `state.engine` for tests.
- `pub fn is_recording() -> bool` and `pub fn get_buffer_len() -> usize` — observable state for test assertions.

**Production bug confirmed fixed:** `ingest_audio` now correctly writes frames to `PTT_BUFFER` during `IS_RECORDING=true`.

### Phase 1 — Production Path Trace

```
SUT: Audio accumulated via ingest_audio during recording is dispatched to STT on PTT stop.

Production Entry Seam:
  handle_ptt_start(app, state) — sets IS_RECORDING=true, clears PTT_BUFFER.
  Followed by repeated calls to ingest_audio(chunk) for each audio frame.

Direction Check: PASS — handle_ptt_start is the upstream trigger.
  The test does NOT call SttCommand::Final directly.

Production Path:
  handle_ptt_start(app, state)
  → IS_RECORDING = true, PTT_BUFFER cleared
  → ingest_audio(chunk) called per frame [test calls this directly]
  → IS_RECORDING=true → PTT_BUFFER.extend_from_slice(chunk)
  → handle_ptt_stop_with_sender(app, state, Some(&test_stt_tx))
  → IS_RECORDING.swap(false) → PTT_BUFFER.lock().split_off(0) → non-empty Vec<f32>
  → test_stt_tx.send(SttCommand::Final(turn_id, buffer))
  → STT Actor → VoxEvent::TranscriptFinal

Observable Exit: VoxEvent::TranscriptFinal.text — similarity >= 0.90

Production functions called:
  setup:   spawn_stt_worker(), mock_app().handle()
  entry:   handle_ptt_start(), ingest_audio() per chunk, handle_ptt_stop_with_sender()
  observe: pipeline_event_rx (TranscriptFinal)

Note on ingest_audio: Call it with VAD_CHUNK_SIZE-sized chunks as production does,
  not with the whole audio buffer at once.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
|---|---|
| **ingest_audio called but IS_RECORDING=false (frames silently dropped)** | **Yes — empty buffer → no SttCommand → transcript timeout** |
| handle_ptt_stop_with_sender called before ingest_audio (empty buffer) | Yes — empty buffer guard fires → transcript timeout |
| PTT_BUFFER populated but SttCommand never sent | Yes — transcript timeout |
| STT produces wrong text | Yes — similarity assertion fails |
| Chunk size mismatch (wrong granularity passed to ingest_audio) | Yes — VAD_CHUNK_SIZE constraint catches this |

### Test Functions

```rust
// Primary: start recording, feed EN clip chunks via ingest_audio(), stop, assert similarity >= 0.90
#[test]
fn test_modular_ptt_audio_accumulation_en() { ... }

// Guard: start, immediately stop (no ingest_audio calls)
// Assert: get_buffer_len()==0, NO TranscriptFinal → state transitions to Ready
#[test]
fn test_modular_ptt_empty_buffer_guard() { ... }

// Guard: start, feed audio, then cancel (handle_ptt_cancel)
// Assert: NO TranscriptFinal, get_buffer_len()==0 after cancel
#[test]
fn test_modular_ptt_cancel_discards_audio() { ... }
```

---

## Seam 3 — Realtime PTT + Ghost Audio Gate: ingest_audio → REALTIME_PTT_BUFFER → handle_ptt_stop_with_engine

**File:** `tests/realtime_ptt_test.rs`  
**Status:** ✅ Unblocked for ghost audio gate tests. Happy-path transcript test requires API key → `#[ignore]`.

### Sanity Check Findings

Backend engineer added:
- `pub fn ingest_audio(chunk: &[f32])` — converts f32 to i16 and writes to `REALTIME_PTT_BUFFER` when `IS_RECORDING=true`.
- `pub fn ingest_audio_i16(chunk: &[i16])` — direct i16 path for test efficiency.
- `pub fn set_speech_detected(detected: bool)` — directly controllable from tests.
- `pub fn is_speech_detected() -> bool` and `pub fn get_buffer_len() -> usize` — observable state.
- `pub fn handle_ptt_stop_with_engine<R>(app, state, engine_override: Option<&RealtimeEngine>)` — accepts mock engine, bypasses `state.realtime_engine` for tests.

### Phase 1 — Production Path Trace (Ghost Audio Gate — primary locally-testable path)

```
SUT: PTT stop with no detected speech discards buffer and returns to Ready without cloud dispatch.

Production Entry Seam:
  handle_ptt_start(app, state) + ingest_audio(silence_or_noise_chunks)
  + handle_ptt_stop_with_engine(app, state, engine_override)
  SPEECH_DETECTED is NOT set (no call to set_speech_detected(true)).

Direction Check: PASS — handle_ptt_start is the upstream trigger.

Production Path (ghost audio gate):
  handle_ptt_start() → IS_RECORDING=true, REALTIME_PTT_BUFFER cleared, SPEECH_DETECTED=false
  → ingest_audio(silence_chunks) → REALTIME_PTT_BUFFER fills with near-zero i16 samples
  → handle_ptt_stop_with_engine(app, state, Some(mock_engine))
  → IS_RECORDING.swap(false) → SPEECH_DETECTED.load() == false
  → REALTIME_PTT_BUFFER.lock().clear()
  → state transitions to Ready, ptt_status: IDLE emitted
  → mock_engine.push_audio() is NEVER called

Observable Exit (NEGATIVE):
  - mock_engine.push_audio() call count == 0
  - ptt_status event == STATUS_IDLE (not STATUS_PROCESSING)
  - is_speech_detected() == false
  - get_buffer_len() == 0 after stop

Production Path (happy, cloud):
  Same as above but set_speech_detected(true) called after handle_ptt_start
  → handle_ptt_stop_with_engine → SPEECH_DETECTED=true → buffer flushed → push_audio() called
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
|---|---|
| **SPEECH_DETECTED=false check missing → buffer always flushed to cloud** | **Yes — mock_engine.push_audio called, assertion fails** |
| ingest_audio writes to buffer but SPEECH_DETECTED never checked | Yes — above |
| set_speech_detected(true) required but buffer still not flushed | Yes — push_audio count == 0 |
| PTT state machine emits STATUS_PROCESSING instead of STATUS_IDLE | Yes — ptt_status event assertion fails |
| Buffer not cleared after ghost gate (memory leak) | Yes — get_buffer_len()==0 assertion |

### Test Functions

```rust
// Primary (ghost audio gate, NEGATIVE): silence PTT hold → assert NO cloud push
#[test]
fn test_realtime_ptt_ghost_audio_gate_silence() { ... }

// Complement: set_speech_detected(true) + feed audio → assert push_audio called once
#[test]
fn test_realtime_ptt_speech_detected_flushes_buffer() { ... }

// Guard: cancel during recording → assert buffer cleared, no cloud push regardless of SPEECH_DETECTED
#[test]
fn test_realtime_ptt_cancel_clears_state() { ... }

// Happy path (cloud): full realtime PTT → TranscriptFinal from Gemini
// Contacts Gemini Live API. Load from temp/.env. Run: cargo test -- --ignored
#[ignore]
#[test]
fn test_realtime_ptt_transcript_en() { ... }
```

---

## Seam 4 — TTS Actor: TtsCommand → Synthesis → Audio Output

**File:** `tests/tts_test.rs`  
**Status:** ✅ Unblocked (requires Supertonic model + golden WAVs in `tests/assets/`)

**Golden reference clips:**
- `test-clips/clip_01_en_briefing.wav` (Edge TTS, EN) — resolved via `get_test_clip_path()`
- `test-clips/clip_07_hi_weather.wav` (Edge TTS, HI)
- `tests/assets/supertonic_01_en_briefing.wav` (Supertonic, EN) — resolved via `get_asset_path()`
- `tests/assets/supertonic_07_hi_weather.wav` (Supertonic, HI)

**Acoustic comparison:** Decode generated WAV → extract via aubio bindings (F0, RMS, duration, voiced/silence ratio, MFCC). Compare against golden using tolerances + DTW on sequences. NOT waveform equality.

### Phase 1 — Production Path Trace

```
SUT: TTS actor synthesizes speech from a TtsCommand::Speak and signals playback lifecycle.

Production Entry Seam: TtsCommand::Speak(text) sent to tts_tx channel.

Direction Check: PASS — TtsCommand is the upstream trigger.

Production Path:
  tts_tx.send(TtsCommand::Speak(text))
  → TTS Actor (Supertonic or Edge TTS provider)
  → synthesis runs → audio bytes produced
  → audio forwarded to playback path
  → VoxEvent::PlaybackStarted emitted on pipeline_event_tx
  → VoxEvent::PlaybackFinished emitted

Observable Exit:
  1. VoxEvent::PlaybackStarted
  2. VoxEvent::PlaybackFinished
  3. Synthesized audio passes acoustic comparison vs golden reference

Production functions called:
  setup:   warm_up_tts() [tts/actor.rs]
  entry:   tts_tx.send(TtsCommand::Speak(text))
  observe: pipeline_event_rx + synthesized audio output path

Functions written in test file: None — acoustic comparison via common::scoring.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
|---|---|
| **TTS actor receives command but synthesis never runs** | **Yes — no PlaybackStarted** |
| Synthesis runs but audio is silent (provider bug) | Yes — RMS energy assertion fails |
| Audio produced but not forwarded to output | Yes — PlaybackStarted absent |
| Model produces degraded audio (wrong voice/language) | Yes — F0/MFCC delta outside tolerance |
| Duration wildly wrong (synthesis crashed mid-word) | Yes — duration tolerance check fails |

### Test Functions

```rust
// Primary: Edge TTS × EN → PlaybackStarted + acoustic match vs clip_01_en_briefing.wav
#[test]
fn test_tts_edge_en() { ... }

// Primary: Edge TTS × HI → PlaybackStarted + acoustic match vs clip_07_hi_weather.wav
#[test]
fn test_tts_edge_hi() { ... }

// Primary: Supertonic × EN → PlaybackStarted + acoustic match vs supertonic_01_en_briefing.wav
#[test]
fn test_tts_supertonic_en() { ... }

// Primary: Supertonic × HI → PlaybackStarted + acoustic match vs supertonic_07_hi_weather.wav
#[test]
fn test_tts_supertonic_hi() { ... }

// Guard: empty text → NO PlaybackStarted, graceful return
#[test]
fn test_tts_empty_text_guard() { ... }
```

**Starting acoustic tolerances (tune after first golden run):**

| Feature | Tolerance |
|---|---|
| Duration | ±20% of golden |
| Mean RMS | ±30% of golden |
| Voiced/silence ratio | ±15 percentage points |
| Mean F0 (voiced frames only) | ±20% of golden |
| MFCC DTW distance | < 2.0 (tune empirically) |

---

## Seam 5 — LLM Actor: GenerationRequest → Token Stream → LlmFinished

**File:** `tests/llm_test.rs`  
**Status:** ⚠️ Requires Qwen model at `~/.vox/models/llm/`. Long-running (~30–120s per test).

### Phase 1 — Production Path Trace

```
SUT: LLM actor generates a coherent response token stream from a conversation input.

Production Entry Seam: LlmCommand::Generate(GenerationRequest) sent to llm_tx.

Direction Check: PASS — LlmCommand is the upstream trigger.

Production Path:
  llm_tx.send(LlmCommand::Generate(request))
  → LLM Actor (llama.cpp / Qwen) receives command
  → inference loop generates tokens
  → each token → VoxEvent::LlmToken on pipeline_event_tx
  → generation completes → VoxEvent::LlmFinished emitted

Observable Exit:
  1. At least one VoxEvent::LlmToken arrives
  2. VoxEvent::LlmFinished arrives within timeout

Production functions called:
  setup:   warm_up_llm() [llm/actor.rs]
  entry:   llm_tx.send(LlmCommand::Generate(request))
  observe: pipeline_event_rx (LlmToken*, LlmFinished)
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
|---|---|
| **LLM actor receives command but never reads channel** | **Yes — no LlmToken events, timeout** |
| Tokens generated but not forwarded to event channel | Yes — no LlmToken events |
| LlmFinished never emitted (actor stalls) | Yes — collection timeout |
| Response is empty (zero tokens) | Yes — token count assertion |
| Cancel flag race: generation stops before first token | Yes — token count = 0 |

### Test Functions

```rust
// Primary: factual EN prompt → assert ≥1 LlmToken + LlmFinished within timeout
#[test]
fn test_llm_generates_response_en() { ... }

// Guard: cancel flag set immediately after dispatch — assert generation halts
#[test]
fn test_llm_cancel_mid_generation() { ... }
```

---

## Seam 6 — Ghost Audio Gate

Merged into Seam 3. See `tests/realtime_ptt_test.rs`.  
Primary test: `test_realtime_ptt_ghost_audio_gate_silence`.

---

## Seam 7 — VAD Ducking / Playback Suppression: `should_suppress_audio()` Gate

**File:** `tests/vad_ducking_test.rs`  
**Status:** ✅ Unblocked — `playback_active: Arc<AtomicBool>` is injectable via `VadActorHandles`

### Phase 1 — Production Path Trace

```
SUT: VAD actor skips process_speech_frame when should_suppress_audio() returns true.

Production Entry Seam:
  Audio pushed to ring buffer while VadActorHandles.playback_active=true.

Direction Check: PASS — ring buffer push is the upstream trigger.

Production Path:
  VadActorHandles.playback_active.store(true)
  → ring_buffer.push_slice(audio_chunk)
  → VAD Actor: pop_slice
  → should_suppress_audio(owner, &playback_active, &is_dictation_enabled, &state)
     returns true (playback_active=true, owner=Assistant)
  → continue; [process_speech_frame skipped]
  → NO SpeechStart emitted, NO SttCommand dispatched

Observable Exit (NEGATIVE):
  After streaming full audio clip with playback_active=true:
  - vox_event_rx is EMPTY (no SpeechStart)
  - pipeline_event_rx is EMPTY (no TranscriptFinal)
  Use common::harness::assert_channel_empty_after() — not a short timeout.

Production functions called:
  setup:   setup_vad_actor() with playback_active=true in VadActorHandles
  entry:   stream_audio_to_ring_buffer() + stream_silence_frames()
  observe: vox_event_rx — assert ABSENCE of SpeechStart
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
|---|---|
| **should_suppress_audio() returns false during playback** | **Yes — SpeechStart fires, assertion fails** |
| playback_active flag ignored entirely | Yes — SpeechStart fires |
| VAD suppresses first chunk but processes subsequent ones | Yes — SpeechStart fires for later chunks |
| Suppression works but SpeechStart emitted via a different code path | Yes — event channel captures all emissions |

### Test Functions

```rust
// Primary (NEGATIVE): stream speech audio while playback_active=true
// Assert: NO SpeechStart, NO TranscriptFinal
#[test]
fn test_vad_ducking_suppresses_audio_during_playback() { ... }

// Complement: suppress during playback, then set playback_active=false, stream more audio
// Assert: first phase = no events; second phase = SpeechStart fires
#[test]
fn test_vad_ducking_resumes_after_playback() { ... }

// Ownership gate: owner=User, playback_active=true — suppression must NOT apply
// Assert: SpeechStart still fires (user barge-in must work through playback)
#[test]
fn test_vad_ducking_does_not_suppress_user_owner() { ... }
```

---

## Seam 8 — Dictation PTT: ingest_audio → DICTATION_BUFFER → handle_hotkey_release_with_sender → STT → output_router (not LLM)

**File:** `tests/dictation_ptt_test.rs`  
**Status:** ✅ Unblocked — `ingest_audio()` + `handle_hotkey_release_with_sender()` exposed

### Sanity Check Findings

Backend engineer added (in `services/pipeline/dictation.rs`):
- `pub fn ingest_audio(chunk: &[f32])` — writes to `DICTATION_BUFFER` when `IS_RECORDING=true`.
- `pub fn handle_hotkey_release_with_sender<R>(app, state, stt_tx: Option<&Sender<SttCommand>>)` — injectable sender, bypasses `state.engine`.
- `pub fn is_recording() -> bool` and `pub fn get_buffer_len() -> usize`.

Dictation transcript routing (`dictation.rs:on_transcript_final`) calls `output_router::route_transcript()` — NOT `LlmCommand::Generate`. The routing is owned by production, not the test.

**Note on observable exit for LLM non-dispatch:** The test must wire an observable `llm_tx` and assert it remains empty after `TranscriptFinal` fires. This requires `warm_up_llm()` with a capturable `llm_tx` — or asserting solely on the dictation event path (`dictation_success` / `transcript_final` on WINDOW_TRAY) and confirming the `llm_tx` channel is not writable from this code path.

### Phase 1 — Production Path Trace

```
SUT: In dictation mode, audio is accumulated, dispatched to STT, and the transcript routes
     to OS text injection — NOT LlmCommand::Generate.

Production Entry Seam:
  handle_hotkey_press(app, state) — sets IS_RECORDING=true, clears DICTATION_BUFFER.
  Followed by ingest_audio(chunk) per frame.

Direction Check: PASS — handle_hotkey_press is the upstream IPC trigger.

Production Path:
  handle_hotkey_press(app, state)
  → IS_RECORDING=true, DICTATION_BUFFER cleared
  → ingest_audio(chunk) per frame → DICTATION_BUFFER fills
  → handle_hotkey_release_with_sender(app, state, Some(&test_stt_tx))
  → DICTATION_BUFFER.lock().split_off(0) → non-empty buffer
  → test_stt_tx.send(SttCommand::Final(turn_id, buffer))
  → STT Actor → VoxEvent::TranscriptFinal
  → on_transcript_final() → output_router::route_transcript() [NOT LlmCommand::Generate]
  → dictation_success event emitted

Observable Exit:
  1. VoxEvent::TranscriptFinal with non-empty text
  2. dictation_success event emitted on app
  3. llm_tx channel EMPTY — no LlmCommand::Generate dispatched

Production functions called:
  setup:   spawn_stt_worker(), mock_app().handle()
  entry:   handle_hotkey_press(), ingest_audio() per chunk,
           handle_hotkey_release_with_sender()
  observe: pipeline_event_rx (TranscriptFinal), app events (dictation_success)
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
|---|---|
| **ingest_audio writes to buffer but IS_RECORDING=false (frames dropped)** | **Yes — empty buffer guard → no transcript** |
| Transcript produced but routed to LLM instead of output_router | Yes — llm_tx assertion catches it |
| on_transcript_final never called (event dispatch broken) | Yes — dictation_success event absent |
| output_router::route_transcript panics silently | Yes — dictation_success absent |
| Empty buffer guard incorrectly fires despite non-empty audio | Yes — transcript timeout |

### Test Functions

```rust
// Primary: feed EN clip via ingest_audio(), release hotkey, assert TranscriptFinal + dictation_success
#[test]
fn test_dictation_ptt_transcript_en() { ... }

// Guard: press hotkey, immediately release (no ingest_audio)
// Assert: NO TranscriptFinal, state → Idle
#[test]
fn test_dictation_ptt_empty_buffer_guard() { ... }

// Routing invariant: assert llm_tx channel is NOT written during dictation transcription
// This is the critical false-green protection — LLM must never be invoked from dictation path
#[test]
fn test_dictation_does_not_invoke_llm() { ... }
```

---

## Seam X — LLM → TTS Clause Chunking

**File:** `tests/llm_tts_chunking_test.rs`  
**Status:** 🔶 PENDING DESIGN — requires comprehensive discussion before spec is written

> **Why this seam is intentionally unspecced:** The clause chunking boundary between LLM token streaming and TTS dispatch directly governs TTFA (time-to-first-audio), prosody naturalness, and perceived quality. Getting boundaries wrong produces unnatural speech or degraded TTFA. This seam requires separate alignment on: acceptable chunk size range, TTFA target threshold, prosody break criteria, and how to measure prosody quality. These decisions must precede the test shape.

---

## Execution Priority

| Priority | Seam | Test File | Status |
|---|---|---|---|
| **P0** | Refactor `stt_test.rs` + build `tests/common/` | Split + extract | Do now |
| **P1** | Seam 2: Modular PTT | `modular_ptt_test.rs` | ✅ Unblocked |
| **P1** | Seam 7: VAD Ducking | `vad_ducking_test.rs` | ✅ Unblocked |
| **P1** | Seam 8: Dictation PTT | `dictation_ptt_test.rs` | ✅ Unblocked |
| **P2** | Seam 3 + 6: Realtime PTT + Ghost Audio | `realtime_ptt_test.rs` | ✅ Unblocked (gate tests); `#[ignore]` for cloud tests |
| **P2** | Seam 4: TTS Actor | `tts_test.rs` | ✅ Unblocked |
| **P2** | Seam 5: LLM Actor | `llm_test.rs` | ✅ Unblocked |
| **P4** | Seam X: Clause Chunking | `llm_tts_chunking_test.rs` | 🔶 Pending design |
