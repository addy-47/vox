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
│   │   ├── supertonic_07_hi_weather.wav    # TTS golden ref (Supertonic, HI)
│   │   ├── compaction_100_turns.json       # 100-turn dataset subset for Compaction eval
│   │   └── memory_pipeline_prefill.db      # Pre-populated SQLite DB for pipeline & retrieval tests
│   ├── stt_test.rs                → REFACTOR: split into seam files below, helpers → common/
│   ├── passive_streaming_test.rs      # Seam 1
│   ├── modular_ptt_test.rs            # Seam 2
│   ├── realtime_ptt_test.rs           # Seam 3 + Ghost Audio Gate (Seam 6)
│   ├── tts_test.rs                    # Seam 4
│   ├── llm_test.rs                    # Seam 5
│   ├── vad_ducking_test.rs            # Seam 7
│   ├── dictation_ptt_test.rs          # Seam 8
│   ├── memory_compaction_test.rs      # Seam 9 (#[ignore] - Nvidia API)
│   ├── memory_ingestion_test.rs       # Seam 10 (4-Stage Pipeline)
│   ├── memory_retrieval_test.rs       # Seam 11 (Scope Routing & BFS Graph)
│   ├── settings_persistence_test.rs   # Seam 12 (SQLite Settings Round-trip)
│   ├── model_eviction_test.rs         # Seam 13 (Zero Idle RAM & Lifecycle)
│   └── model_manager_test.rs          # Seam 14 (Model Integrity & Verified Marker)
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

| Defect                                        | Would test fail?                                    |
| --------------------------------------------- | --------------------------------------------------- |
| **VAD actor never pops from ring buffer**     | **Yes — SpeechStart never fires**                   |
| Speech onset detection broken                 | Yes — SpeechStart assertion fails                   |
| Speech offset detection broken                | Yes — SpeechEnd assertion fails; transcript timeout |
| SttCommand::Final never dispatched on offset  | Yes — transcript timeout                            |
| STT produces wrong text                       | Yes — similarity assertion fails                    |
| VAD runs in wrong mode (PTT suppresses audio) | Yes — audio suppressed, no speech events            |

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

| Defect                                                                   | Would test fail?                                            |
| ------------------------------------------------------------------------ | ----------------------------------------------------------- |
| **ingest_audio called but IS_RECORDING=false (frames silently dropped)** | **Yes — empty buffer → no SttCommand → transcript timeout** |
| handle_ptt_stop_with_sender called before ingest_audio (empty buffer)    | Yes — empty buffer guard fires → transcript timeout         |
| PTT_BUFFER populated but SttCommand never sent                           | Yes — transcript timeout                                    |
| STT produces wrong text                                                  | Yes — similarity assertion fails                            |
| Chunk size mismatch (wrong granularity passed to ingest_audio)           | Yes — VAD_CHUNK_SIZE constraint catches this                |

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

| Defect                                                                   | Would test fail?                                         |
| ------------------------------------------------------------------------ | -------------------------------------------------------- |
| **SPEECH_DETECTED=false check missing → buffer always flushed to cloud** | **Yes — mock_engine.push_audio called, assertion fails** |
| ingest_audio writes to buffer but SPEECH_DETECTED never checked          | Yes — above                                              |
| set_speech_detected(true) required but buffer still not flushed          | Yes — push_audio count == 0                              |
| PTT state machine emits STATUS_PROCESSING instead of STATUS_IDLE         | Yes — ptt_status event assertion fails                   |
| Buffer not cleared after ghost gate (memory leak)                        | Yes — get_buffer_len()==0 assertion                      |

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

| Defect                                                  | Would test fail?                      |
| ------------------------------------------------------- | ------------------------------------- |
| **TTS actor receives command but synthesis never runs** | **Yes — no PlaybackStarted**          |
| Synthesis runs but audio is silent (provider bug)       | Yes — RMS energy assertion fails      |
| Audio produced but not forwarded to output              | Yes — PlaybackStarted absent          |
| Model produces degraded audio (wrong voice/language)    | Yes — F0/MFCC delta outside tolerance |
| Duration wildly wrong (synthesis crashed mid-word)      | Yes — duration tolerance check fails  |

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

| Feature                      | Tolerance                |
| ---------------------------- | ------------------------ |
| Duration                     | ±20% of golden           |
| Mean RMS                     | ±30% of golden           |
| Voiced/silence ratio         | ±15 percentage points    |
| Mean F0 (voiced frames only) | ±20% of golden           |
| MFCC DTW distance            | < 2.0 (tune empirically) |

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

| Defect                                                 | Would test fail?                      |
| ------------------------------------------------------ | ------------------------------------- |
| **LLM actor receives command but never reads channel** | **Yes — no LlmToken events, timeout** |
| Tokens generated but not forwarded to event channel    | Yes — no LlmToken events              |
| LlmFinished never emitted (actor stalls)               | Yes — collection timeout              |
| Response is empty (zero tokens)                        | Yes — token count assertion           |
| Cancel flag race: generation stops before first token  | Yes — token count = 0                 |

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

| Defect                                                              | Would test fail?                             |
| ------------------------------------------------------------------- | -------------------------------------------- |
| **should_suppress_audio() returns false during playback**           | **Yes — SpeechStart fires, assertion fails** |
| playback_active flag ignored entirely                               | Yes — SpeechStart fires                      |
| VAD suppresses first chunk but processes subsequent ones            | Yes — SpeechStart fires for later chunks     |
| Suppression works but SpeechStart emitted via a different code path | Yes — event channel captures all emissions   |

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

| Defect                                                                    | Would test fail?                             |
| ------------------------------------------------------------------------- | -------------------------------------------- |
| **ingest_audio writes to buffer but IS_RECORDING=false (frames dropped)** | **Yes — empty buffer guard → no transcript** |
| Transcript produced but routed to LLM instead of output_router            | Yes — llm_tx assertion catches it            |
| on_transcript_final never called (event dispatch broken)                  | Yes — dictation_success event absent         |
| output_router::route_transcript panics silently                           | Yes — dictation_success absent               |
| Empty buffer guard incorrectly fires despite non-empty audio              | Yes — transcript timeout                     |

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

## Seam 9 — Memory Compaction: 100-Turn History → LLM Extraction → Validated Fact Schema

**File:** `tests/memory_compaction_test.rs`  
**Status:** ⚠️ Requires Nvidia API Key (`temp/.env`) → Annotated with `#[ignore]` by default.

### Phase 1 — Production Path Trace

```
SUT: run_compaction() sends multi-turn conversation history to LLM, extracts 6 collection
     facts + narrative summary, and validates output against schema/quality constraints.

Production Entry Seam:
  run_compaction(provider, history_messages, settings)

Direction Check: PASS — run_compaction() is the upstream extraction trigger.

Production Path:
  Load 100-turn dataset from tests/assets/compaction_100_turns.json
  → build_compaction_request(history_messages, settings)
  → provider.generate() (Nvidia LLM API call)
  → stream tokens → parse_compaction_json(response)
  → populate CompactionResult { context_summary, personal_memory, diff_to_enqueue }

Observable Exit:
  1. CompactionResult returned without error
  2. All 6 core collections present: Identity, Directives, Narrative, Profile, Entities, Constraints
  3. Narrative summary is a non-empty string (> 20 chars)
  4. Zero single-word or trivial facts (all fact strings length >= 15 chars)
  5. Total facts extracted >= 5 across collections

Production functions called:
  setup:   LlmProvider instance initialized with Nvidia API credentials from temp/.env
  entry:   run_compaction(&provider, &history_messages, Some(&settings))
  observe: CompactionResult fields and fact string invariants
```

### Phase 2b — False-Green Table

| Defect                                                              | Would test fail?                                  |
| ------------------------------------------------------------------- | ------------------------------------------------- |
| **LLM returns malformed JSON or markdown fences that fail parsing** | **Yes — run_compaction retry fails, returns Err** |
| LLM drops required collections (e.g. Identity or Profile missing)   | Yes — collection presence assertion fails         |
| LLM extracts trivial / 1-word hallucinated tokens                   | Yes — minimum fact length (> 15 chars) fails      |
| Narrative is empty or formatted as array instead of string          | Yes — narrative structure assertion fails         |

### Test Functions

```rust
/// Primary: 100-turn history compaction against Nvidia LLM provider
/// Contacts Nvidia API. Load credentials from temp/.env. Run: cargo test -- --ignored
#[ignore]
#[test]
fn test_memory_compaction_100_turns_nvidia() { ... }
```

---

## Seam 10 — Memory Ingestion: staged_pending Queue → 4-Stage Pipeline → Active DB Facts

**File:** `tests/memory_ingestion_test.rs`  
**Status:** ✅ Unblocked (uses local MiniLM, DeBERTa, ModernBERT ONNX models; zero network).

### Phase 1 — Production Path Trace

```
SUT: 4-stage pipeline (Dedup -> Embed -> NLI Eval -> Commit & Prune) processes queued facts
     into persistent SQLite tables with graph relations.

Production Entry Seam:
  Pre-populated SQLite database in tests/assets/memory_pipeline_prefill.db containing known
  staged_pending rows in personal_memory_queue (or dynamically inserted staged facts).
  Invoked via drain_pipeline_queue(conn, &cancel_flag).

Direction Check: PASS — drain_pipeline_queue() processes from the queue entry boundary.

Production Path:
  staged_pending rows in personal_memory_queue
  → Stage 1 (Dedup): Jaccard exact match against active facts + queue items → status='deduped'
  → Stage 2 (Embed): MiniLM-L12 generates 384-dim vector, soft cosine dedup (0.95) → status='embedded'
  → Stage 3 (Eval): DeBERTa v3 NLI (intra) + ModernBERT (inter) produce relations_json → status='evaluated'
  → Stage 4 (Commit): INSERT into memory_facts (status='active'), INSERT vectors into memory_facts_vectors,
                      INSERT relations into memory_relations, DELETE processed rows from queue

Observable Exit:
  1. personal_memory_queue count == 0 (fully drained)
  2. memory_facts contains active rows matching deduplicated input
  3. memory_facts_vectors populated with 384-dim non-zero embeddings
  4. memory_relations contains expected structural edges (e.g. SHAPES, DEPENDS_ON, SUPERSEDES)
  5. Inactive/superseded facts correctly marked status='superseded'

Production functions called:
  setup:   open_test_turso_db(), ensure_embedder_loaded(), ensure_nli_loaded(), ensure_edge_classifier_loaded()
  entry:   drain_pipeline_queue(&conn, &cancel_flag)
  observe: Direct SQL queries on memory_facts, memory_facts_vectors, memory_relations, personal_memory_queue
```

### Phase 2b — False-Green Table

| Defect                                                    | Would test fail?                                                |
| --------------------------------------------------------- | --------------------------------------------------------------- |
| **Stage 1 drops all facts or halts queue**                | **Yes — personal_memory_queue remains > 0, memory_facts empty** |
| Stage 2 fails to generate valid 384-dim vectors           | Yes — memory_facts_vectors missing rows or dim != 384           |
| Stage 3 edge classifier produces corrupted relations JSON | Yes — Stage 4 transaction rollback, queue items stranded        |
| Stage 4 fails to delete processed queue items             | Yes — personal_memory_queue count assertion fails               |

### Test Functions

```rust
// Primary: full 4-stage pipeline execution from staged_pending fixture to committed graph
#[test]
fn test_memory_pipeline_4_stage_drain() { ... }

// Guard: Stage 1 exact Jaccard duplicate suppression
#[test]
fn test_memory_pipeline_stage1_exact_dedup() { ... }

// Guard: Stage 2 soft cosine vector dedup (>= 0.95)
#[test]
fn test_memory_pipeline_stage2_soft_vector_dedup() { ... }

// Guard: Stage 3 NLI contradiction (SUPERSEDES edge + old fact deactivation)
#[test]
fn test_memory_pipeline_stage3_nli_contradiction_supersedes() { ... }
```

---

## Seam 11 — Memory Retrieval: Query → Scope Classifier → Vector Search → BFS Graph → Context Budget

**File:** `tests/memory_retrieval_test.rs`  
**Status:** ✅ Unblocked (uses local ModernBERT & MiniLM models).

### Phase 1 — Production Path Trace

```
SUT: retrieve_personal_context() classifies query scope, executes scoped SQL/vector
     retrieval, expands 2-hop BFS graph edges, and formats output within 15% token budget.

Production Entry Seam:
  retrieve_personal_context(conn, query, settings, app_state)

Direction Check: PASS — retrieve_personal_context() is the primary retrieval API.

Production Path:
  classify_scope(query) via ModernBERT → MemoryScope { ChitChat, User, Domain, Temporal }
  → route_scope(scope) maps to target SQL & Vector collections
  → SQL Branch (Temporal): fetches narrative/directives within budget
  → Vector Branch (User/Domain): semantic search on memory_facts_vectors (similarity >= 0.40)
  → BFS Expansion: 2-hop neighbor traversal via memory_relations (parent_quota allocation)
  → Token Budget: strictly caps formatted <user_profile> to max_personal_memory_share (15%)

Observable Exit:
  1. ChitChat query (e.g. "Hello", "How are you?") → returns empty string "" (zero context injection)
  2. Domain query with known entities in DB → returns <user_profile><semantic_graph> block
  3. BFS 2-hop connected facts rendered with "  ↳ --[{relation}]--> [{collection}] {fact}"
  4. estimate_tokens(result) <= context_window * max_personal_memory_share

Production functions called:
  setup:   open_test_turso_db_with_fixtures(), ensure_scope_classifier_loaded(), ensure_embedder_loaded()
  entry:   retrieve_personal_context(&conn, query, &settings, &app_state)
  observe: Formatted context string structure, XML tags, and token count calculation
```

### Phase 2b — False-Green Table

| Defect                                                                    | Would test fail?                                         |
| ------------------------------------------------------------------------- | -------------------------------------------------------- |
| **ChitChat queries erroneously trigger vector search & inject memory**    | **Yes — non-empty string returned, assertion fails**     |
| BFS graph expansion traversal broken (0-hop only)                         | Yes — child relation arrow "↳" absent from output        |
| Token budget arithmetic overflows context window                          | Yes — estimate_tokens > budget threshold assertion fails |
| Identity facts fetched dynamically in SQL branch (violates boot pre-load) | Yes — Identity tag found in dynamic output               |

### Test Functions

```rust
// Primary: ChitChat query returns zero context
#[test]
fn test_retrieval_chitchat_scope_returns_empty() { ... }

// Primary: Domain query retrieves seed facts + 2-hop BFS graph expansion
#[test]
fn test_retrieval_domain_scope_with_bfs_expansion() { ... }

// Primary: Temporal query retrieves narrative and directive blocks
#[test]
fn test_retrieval_temporal_scope_narrative_directives() { ... }

// Guard: Massive candidate result set is hard-capped to 15% token budget
#[test]
fn test_retrieval_token_budget_enforcement() { ... }
```

---

## Seam 12 — Settings Persistence: Mutation → SQLite DB Write → Reload Round-trip

**File:** `tests/settings_persistence_test.rs`  
**Status:** ✅ Unblocked (pure SQLite persistence; no models, no network).

### Phase 1 — Production Path Trace

```
SUT: save_settings() writes modified application configuration to SQLite settings table,
     and load_settings() restores exact configuration across restarts.

Production Entry Seam:
  save_settings(conn, &modified_settings) / load_settings(conn)

Direction Check: PASS — save_settings() is the persistence entry point.

Production Path:
  Mutate Voice, LLM, TTS, VAD, and Memory settings fields
  → save_settings(conn, &settings) executes serialized UPSERT into SQLite
  → Drop in-memory structs
  → load_settings(conn) reads and deserializes fresh AppSettings struct from DB

Observable Exit:
  1. save_settings() returns Ok(())
  2. load_settings() produces AppSettings identical to modified input across all fields

Production functions called:
  setup:   open_memory_sqlite_db()
  entry:   save_settings(&conn, &settings) → load_settings(&conn)
  observe: Direct field equality comparisons on reloaded struct
```

### Phase 2b — False-Green Table

| Defect                                                                | Would test fail?                                              |
| --------------------------------------------------------------------- | ------------------------------------------------------------- |
| **save_settings is a no-op or silently ignores nested struct fields** | **Yes — reloaded struct matches default, not modified input** |
| Serialization format mismatch corrupts settings on disk               | Yes — load_settings returns Err or default fallback           |

### Test Functions

```rust
// Primary: round-trip persistence of custom Voice, LLM, TTS, VAD, and Memory settings
#[test]
fn test_settings_sqlite_roundtrip_persistence() { ... }
```

---

## Seam 13 — Model Eviction & Zero Idle RAM: Load Singletons → unload_all_onnx_models() → State Reset

**File:** `tests/model_eviction_test.rs`  
**Status:** ✅ Unblocked (local model weights required).

### Phase 1 — Production Path Trace

```
SUT: Model singletons initialize ONNX runtime sessions and unload_all_onnx_models() drops
     sessions, resets RwLocks to None, and triggers heap trimming.

Production Entry Seam:
  ensure_*_loaded() followed by unload_all_onnx_models()

Direction Check: PASS — Lifecycle management functions are the direct SUT.

Production Path:
  ensure_embedder_loaded() + ensure_nli_loaded() + ensure_edge_classifier_loaded() + ensure_scope_classifier_loaded()
  → All is_*_loaded() return true, RwLocks hold Some(Engine)
  → unload_all_onnx_models() called
  → Singletons write lock set to None (dropping Session & Environment)
  → trim_heap("MemorySubsystem::unload_all_onnx_models") executes
  → All is_*_loaded() return false

Observable Exit:
  1. Prior to unload: is_embedder_loaded(), is_nli_loaded(), is_edge_classifier_loaded(), is_scope_classifier_loaded() all == true
  2. Post unload: all 4 is_*_loaded() functions return false
  3. No panic during heap trim across target OS

Production functions called:
  setup:   paths::get() resolution
  entry:   ensure_*_loaded() → unload_all_onnx_models()
  observe: is_*_loaded() query helpers
```

### Phase 2b — False-Green Table

| Defect                                                      | Would test fail?                        |
| ----------------------------------------------------------- | --------------------------------------- |
| **unload_all_onnx_models does not reset RwLock singletons** | **Yes — is\_\*\_loaded() remains true** |
| Drop implementation panics on active runtime sessions       | Yes — test panics and fails             |

### Test Functions

```rust
// Primary: initialize all memory ONNX singletons, verify loaded, unload, verify unloaded
#[test]
fn test_onnx_model_singleton_lifecycle_eviction() { ... }
```

---

## Seam 14 — Model Manager: Manifest Parsing → Hash Verification → .verified Marker Lifecycle

**File:** `tests/model_manager_test.rs`  
**Status:** ✅ Unblocked for local fixture & hash verification. Full remote download tests `#[ignore]`.

### Phase 1 — Production Path Trace

```
SUT: ModelManager verifies file integrity against SHA256 manifest, generates .verified marker
     on success, detects corrupted archives, and cleans up files on model removal.

Production Entry Seam:
  ModelManager::setup_model() / verification helper functions.

Direction Check: PASS — ModelManager methods are the entry seam for asset management.

Production Path:
  Synthetic model directory created with valid test payload + matching SHA256 manifest entry
  → Verification calculates SHA256 and creates .verified JSON marker
  → Corrupted payload (tampered byte) calculates mismatched SHA256 → returns VerificationError
  → Removal deletes model directory and .verified marker

Observable Exit:
  1. Valid file → .verified marker file created with correct sha256 and timestamp
  2. Corrupted file → Verification fails with explicit hash mismatch error; no .verified marker
  3. Model deletion → Directory and marker removed from disk

Production functions called:
  setup:   ModelManager::new(None), tempfile directory setup
  entry:   setup_model() or verify_and_mark()
  observe: Filesystem status, marker JSON content, Result error types
```

### Phase 2b — False-Green Table

| Defect                                                              | Would test fail?                                             |
| ------------------------------------------------------------------- | ------------------------------------------------------------ |
| **Hash verification skipped and .verified created unconditionally** | **Yes — corrupted payload test succeeds instead of failing** |
| .verified marker contains wrong metadata schema                     | Yes — marker JSON parsing assertion fails                    |
| Model deletion leaves orphan .verified marker                       | Yes — marker existence check fails                           |

### Test Functions

```rust
// Primary: synthetic valid payload creates .verified marker
#[test]
fn test_model_manager_valid_payload_verification() { ... }

// Guard: corrupted payload fails hash verification and blocks .verified creation
#[test]
fn test_model_manager_corrupted_payload_detection() { ... }

// Remote happy path: live download from manifest entry
// Contacts CDN/remote server. Run: cargo test -- --ignored
#[ignore]
#[test]
fn test_model_manager_live_download_nemotron() { ... }
```

---

## Seam X — LLM → TTS Clause Chunking

**File:** `tests/llm_tts_chunking_test.rs`  
**Status:** 🔶 PENDING DESIGN — requires comprehensive discussion before spec is written

> **Why this seam is intentionally unspecced:** The clause chunking boundary between LLM token streaming and TTS dispatch directly governs TTFA (time-to-first-audio), prosody naturalness, and perceived quality. Getting boundaries wrong produces unnatural speech or degraded TTFA. This seam requires separate alignment on: acceptable chunk size range, TTFA target threshold, prosody break criteria, and how to measure prosody quality. These decisions must precede the test shape.

---

## Execution Priority Matrix (Seams 1–14)

| Priority | Seam                                  | Test File                      | Status         | Notes                                          |
| -------- | ------------------------------------- | ------------------------------ | -------------- | ---------------------------------------------- |
| **P0**   | Shared Test Infra & Refactor          | `tests/common/`                | ✅ Done        | Audio, scoring, paths, harness                 |
| **P1**   | Seam 1: Passive Streaming             | `passive_streaming_test.rs`    | ✅ Ready       | Ring buffer -> VAD -> STT                      |
| **P1**   | Seam 2: Modular PTT                   | `modular_ptt_test.rs`          | ✅ Ready       | `ingest_audio` + `handle_ptt_stop_with_sender` |
| **P1**   | Seam 7: VAD Ducking                   | `vad_ducking_test.rs`          | ✅ Ready       | `should_suppress_audio` gate                   |
| **P1**   | Seam 8: Dictation PTT                 | `dictation_ptt_test.rs`        | ✅ Ready       | Hotkey -> STT -> OutputRouter                  |
| **P2**   | Seam 3 + 6: Realtime PTT & Ghost Gate | `realtime_ptt_test.rs`         | ✅ Ready       | `ingest_audio` + `handle_ptt_stop_with_engine` |
| **P2**   | Seam 4: TTS Actor                     | `tts_test.rs`                  | ✅ Ready       | Aubio acoustic regression vs golden WAVs       |
| **P2**   | Seam 5: LLM Actor                     | `llm_test.rs`                  | ✅ Ready       | Qwen token streaming & LlmFinished             |
| **P2**   | Seam 10: Memory Ingestion             | `memory_ingestion_test.rs`     | ✅ Ready       | 4-stage pipeline drain                         |
| **P2**   | Seam 11: Memory Retrieval             | `memory_retrieval_test.rs`     | ✅ Ready       | Scope routing + 2-hop BFS graph                |
| **P3**   | Seam 12: Settings Persistence         | `settings_persistence_test.rs` | ✅ Ready       | SQLite settings round-trip                     |
| **P3**   | Seam 13: Model Eviction               | `model_eviction_test.rs`       | ✅ Ready       | Zero Idle RAM & RwLock drop                    |
| **P3**   | Seam 14: Model Manager                | `model_manager_test.rs`        | ✅ Ready       | Hash verification & .verified marker           |
| **P3**   | Seam 9: Memory Compaction             | `memory_compaction_test.rs`    | ⚠️ `#[ignore]` | 100-turn eval (Nvidia API key)                 |
| **P4**   | Seam X: Clause Chunking               | `llm_tts_chunking_test.rs`     | 🔶 Pending     | TTFA vs Prosody design session                 |

---

## Mutation Testing Scope (Seams 1–8)

> **Prerequisite:** `/mutate` skill governs the full protocol (Extract → Mutate → Assert RED → Revert & Assert GREEN → Score). Read it before running any mutation cycle. This section supplies only what the skill cannot derive: the production file target per seam, the Tier 1 vs Tier 2 scoping decision, and the Vox-specific invocation commands.

### Scope Table

| Seam                  | Test File                   | Production File(s) to Mutate               | Tier 2 (`cargo-mutants`)? | Reason                                                                       |
| --------------------- | --------------------------- | ------------------------------------------ | ------------------------- | ---------------------------------------------------------------------------- |
| 1 — Passive Streaming | `passive_streaming_test.rs` | `services/vad/actor.rs`                    | ❌ No                     | VAD actor warm-up per mutant is prohibitive                                  |
| 2 — Modular PTT       | `modular_ptt_test.rs`       | `services/pipeline/modular_ptt.rs`         | ✅ Yes                    | Pure logic: buffer accumulation, IS_RECORDING gate, sender dispatch          |
| 3 — Realtime PTT      | `realtime_ptt_test.rs`      | `services/pipeline/realtime_ptt.rs`        | ✅ Yes                    | Pure logic: SPEECH_DETECTED gate, engine_override dispatch                   |
| 4 — TTS Actor         | `tts_test.rs`               | `services/tts/actor.rs`                    | ❌ No                     | TTS model warm-up per mutant is prohibitive                                  |
| 5 — LLM Actor         | `llm_test.rs`               | `services/llm/actor.rs`                    | ❌ No                     | LLM inference per mutant is prohibitive                                      |
| 6 — Ghost Audio Gate  | `realtime_ptt_test.rs`      | `services/pipeline/realtime_ptt.rs`        | ✅ Yes                    | Merged into Seam 3 — same file, same Tier 2 scope                            |
| 7 — VAD Ducking       | `vad_ducking_test.rs`       | `services/vad/actor.rs` (suppress fn only) | ✅ Yes                    | Suppress function is pure logic; scope strictly to `should_suppress_audio()` |
| 8 — Dictation PTT     | `dictation_ptt_test.rs`     | `services/pipeline/dictation.rs`           | ✅ Yes                    | Pure logic: IS_RECORDING gate, buffer accumulation, routing fork             |

### Tier 1 — Phase 2b Manual Mutant Targets (All Seams)

Each row in a seam's Phase 2b table maps to exactly one mutant. The governing rule from `/mutate`: if the row would only produce a crash/compile error, it is not a valid mutant — rewrite it to produce silent wrong behavior. The table below resolves the highest-priority rows per seam into concrete edits.

| Seam | Phase 2b Row                                                  | Mutant Category          | Concrete Edit                                                                                    |
| ---- | ------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------ |
| 1    | VAD actor never pops ring buffer                              | Silent drop              | Delete the `pop_slice()` call inside the VAD loop body; keep the surrounding logic               |
| 2    | `IS_RECORDING=false` → frames silently dropped                | Gate inversion           | Replace `if IS_RECORDING.load(Ordering::Relaxed)` body with `return;` unconditionally            |
| 2    | `SttCommand::Final` never sent                                | Silent drop              | Delete `tx.send(SttCommand::Final(...))` but keep `Ok(())` return                                |
| 3    | `SPEECH_DETECTED=false` check missing → buffer always flushed | Gate inversion           | Replace `if !SPEECH_DETECTED.load(Ordering::Relaxed)` guard with `if false` (gate never fires)   |
| 3    | Ghost gate fires but `push_audio()` still called              | Routing/destination swap | Move `engine.push_audio(&buffer)` above the `SPEECH_DETECTED` check                              |
| 4    | TTS receives command but synthesis never runs                 | Silent drop              | Delete the provider `synthesize()` call, emit `PlaybackStarted` + `PlaybackFinished` immediately |
| 5    | LLM actor reads channel but tokens not forwarded              | Silent drop              | Delete `pipeline_event_tx.send(VoxEvent::LlmToken {...})` inside the token loop                  |
| 7    | `should_suppress_audio()` returns false during playback       | Boolean inversion        | Replace function body with `return false;` unconditionally                                       |
| 8    | `ingest_audio` writes to buffer but IS_RECORDING=false        | Gate inversion           | Replace `if IS_RECORDING.load(Ordering::Relaxed)` in `dictation::ingest_audio` with `if false`   |
| 8    | Transcript routed to LLM instead of `output_router`           | Routing swap             | In `on_transcript_final`, replace `output_router::route_transcript(...)` call with a no-op       |

### Tier 2 — `cargo-mutants` Invocation (Seams 2, 3, 7, 8 only)

> [!IMPORTANT]
> Run Tier 1 for all seams first. Only run Tier 2 after all Tier 1 mutants are killed or documented. Never run `cargo-mutants` globally — scope it to a single file with a single test target.

**Mandatory thread allocation prefix for all Tier 2 invocations:**

```bash
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo mutants \
  --file <SOURCE_FILE> \
  --test-tool cargo \
  -- --test <TEST_FILE> --release -- --test-threads=1
```

**Per-seam invocations:**

```bash
# Seam 2 — Modular PTT
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo mutants \
  --file app/src-tauri/src/services/pipeline/modular_ptt.rs \
  --test-tool cargo \
  -- --test modular_ptt_test --release -- --test-threads=1 --nocapture

# Seam 3 + 6 — Realtime PTT & Ghost Audio Gate
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo mutants \
  --file app/src-tauri/src/services/pipeline/realtime_ptt.rs \
  --test-tool cargo \
  -- --test realtime_ptt_test --release -- --test-threads=1 --nocapture

# Seam 7 — VAD Ducking (suppress function only — use function filter if supported)
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo mutants \
  --file app/src-tauri/src/services/vad/actor.rs \
  --test-tool cargo \
  -- --test vad_ducking_test --release -- --test-threads=1 --nocapture

# Seam 8 — Dictation PTT
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo mutants \
  --file app/src-tauri/src/services/pipeline/dictation.rs \
  --test-tool cargo \
  -- --test dictation_ptt_test --release -- --test-threads=1 --nocapture
```

> [!WARNING]
> Verify the `cargo-mutants` flag syntax against the currently installed version before running. Flags such as `--file`, `--test-tool`, and filter options vary across versions. Run `cargo mutants --help` first.

### Clean-Revert Invariant (Mandatory Between Every Mutant)

After step 4 (Revert) of each mutation cycle, the following must both be true before the next mutant is started. **Never stack mutations.**

```bash
# Assert diff is clean — no leftover edits
git diff --stat
# Expected output: empty (nothing printed)

# Assert baseline is still green
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo test --test <target_test_file> --release -- --nocapture --test-threads=1
# Expected: all tests pass, 0 failures
```

If `git diff` is not empty, the mutation was not fully reverted. Do not proceed. Fix the revert first.

### Score Output Format

Report results in this format after all mutants for a seam are run:

```
Seam: [name]
Mutants attempted: N
Killed:    K  (failing assertion logged for each)
Survivors: S  (row source, edit made, resolution: test strengthened / equivalent mutant discarded)
Mutation score: K/N

Tier 2 (cargo-mutants): [ran / skipped]
  If ran: survivors from automated sweep (edit, test, resolution)
```

A survivor is a real finding — treat it the same as a production bug. Either the test assertion is too weak (strengthen it) or the mutant is behaviourally equivalent (document why and exclude from count).


### After Running Mutation Tests (Seams 1-8):
[test-engineer.md](rule;file:///home/addy/projects/apps/vox/.agents/rules/test-engineer.md) 
I want you to help me essentially review  the integration test spec  , as you know we defered the seams 9 -14 .
BUt before we start implementing them  , i want your analysis first of the current tests and seams defined , all 1 -14 , are they still correct after the refacotr or should they be changed ? Read all the relevant docs  also their false greens are very crucial too so focus on them as we need to define our logic for mutation testing . 
There are two seams noted in seam X intentionally skipped , and actually make this into seam X and seam Y  , as i realised there are two here 
one LLM to TTS and other TTS to playback. they are completely deferd for now. 
[/grill-me](slashCommand;grill-me) after you explore and lets finalise all seams and is there any critical one missing that should be added .