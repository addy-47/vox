# Phase 11 — Integration Test Specification (Handler-Native) v2.2

**Status:** Final — Phase 10 domain spec archived, assistant/dictation SSOT verified at `HEAD` (`app/src-tauri/src/` 174 files, `pipeline/assistant/*` + `pipeline/dictation/{mod,ptt,speech,transcript,error}` + `pipeline/router.rs` + `services/dictation/*`)
**Location:** `app/src-tauri/tests/` (integration), `app/src-tauri/src/**/#[cfg(test)]` (unit)
**Execution (IT):** `cargo nextest run --test <file> --release -- --nocapture --test-threads=1`  // `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc)` + `cargo test --lib --release` for UT
**Prerequisites:** Decision `2026-09-03` — Domain seams removed; all seams re-anchored to `pipeline/assistant/*` + `pipeline/dictation/*` per `AGENTS.md:4.1`. v2.2 refresh (`2026-09-04`): dictation seam rewritten for post-refactor truth (unified `InteractionState` + `Sleeping=7`, event-driven `VoxEvent::PttStart/PttStop/PttCancel`, `services/dictation/hotkey.rs`, Option-C `ingestion_gate` 2-layer defense, `try_lock()` discipline, owner handover + VAD mode sync + CPAL teardown gate). No seams deferred — v2.2 defines every seam (memory/session/manager fully defined; cloud-gated tests use `#[ignore]`, never `#[DEFERRED]`).

---

## 0. Test Infrastructure

Before any seam file is written the shared harness must exist. All seam files import from `tests/common/` — no duplication.

### Directory Layout

```
app/src-tauri/
├── src/
│   ├── pipeline/
│   │   ├── dictation/
│   │   │   ├── mod.rs                         # transition_dictation (Idle/Ready/Listening/Thinking/Error only) + handle_event dispatcher
│   │   │   ├── ptt.rs                         # on_ptt_start/stop(+_with_sender)/cancel — event-driven, try_lock discipline
│   │   │   ├── speech.rs                      # on_speech_start/end — passive dictation (Ready->Listening->Thinking)
│   │   │   ├── transcript.rs                  # on_transcript_final -> output_router (never LLM) + TRAY TranscriptFinal
│   │   │   └── error.rs                       # on_error (Error->Ready auto-recovery + VoiceError+TRAY) / on_cancelled
│   │   ├── assistant/
│   │   │   ├── ptt.rs                          # on_ptt_start/stop/cancel + ptt_start/stop/cancel wrappers (Modular/Realtime branch)
│   │   │   ├── transcript.rs                   # on_transcript_final -> facade -> LLM dispatch
│   │   │   ├── llm.rs                          # on_llm_finished -> TTS remainder + persist
│   │   │   ├── playback.rs                     # on_playback_started/finished
│   │   │   ├── interrupt.rs                    # on_interrupt (barge-in, 6-step)
│   │   │   ├── speech.rs                       # on_speech_start/end (passive)
│   │   │   ├── session.rs                      # on_session_start/pause/resume/end + owner handover + VAD sync + CPAL gate
│   │   │   ├── accumulator.rs                  # TurnAccumulator + chunker bridge
│   │   │   ├── error.rs
│   │   │   └── mod.rs
│   │   ├── router.rs                           # single-FIFO dispatch (VoxEvent => owner-routed: Dictation vs Assistant) + dual-track guard
│   │   └── mod.rs                              # RoutingContext (owner-aware), transition() incl. Sleeping, target_window(), spawn_idle_monitor (420s/300s->Sleeping)
│   ├── services/
│   │   ├── dictation/
│   │   │   ├── hotkey.rs                       # init_dictation_hotkey_listener + register_global_hotkey — sends VoxEvent::PttStart/PttStop via event_tx ONLY (never calls handlers directly)
│   │   │   ├── output_router.rs                # route_transcript (Tray bypass / Clipboard / Paste+fallback) + toasts
│   │   │   ├── clipboard.rs                    # set_text / with_clipboard_safe
│   │   │   ├── input.rs                        # create_input_adapter (simulate_paste)
│   │   │   └── mod.rs
│   │   ├── vad/
│   │   │   ├── actor.rs                        # spawn_vad_actor, should_suppress_audio, WindowedValidation/ContinuousSegmentation/StreamPassthrough + ingestion_gate Layer-2 purge
│   │   │   ├── utils.rs                        # PreRollBuffer, f32_to_i16_pcm
│   │   │   └── mod.rs                          # VAD_CHUNK_SIZE=256, VAD_PRE_ROLL_CAPACITY=8000, VAD_VALIDATION_TIMEOUT_MS=500
│   │   ├── stt/actor.rs                        # SttCommand::Final/Partial, spawn_stt_worker
│   │   ├── llm/actor.rs                        # LlmCommand::Generate { turn_id, cancel: CancellationToken, accumulator, tts_tx, pending }
│   │   ├── tts/actor.rs                        # TtsCommand::{Generate,SetVoice,Shutdown}, TtsClauseChunker::push_str/flush/find_split_point
│   │   ├── harness/facade.rs                   # prepare_turn_context (filler, FIFO/compaction, request)
│   │   ├── harness/{accountant,buffer,prompt_builder,manager}.rs
│   │   ├── memory/{ingestion,compaction,retrieval,ml}.rs
│   │   └── audio/playback.rs                   # ingest_chunk / ingest_chunk_i16 / flush_pre_roll / turn_armed + pending_synthesis_jobs
│   │   └── audio/device.rs                     # build_input_stream — ingestion_gate Layer-1 CPAL boundary drop (gate closed => return before push)
│   ├── core/
│   │   ├── state.rs                            # PipelineAtomics::next_turn()/peek_turn_id()/turn_token()/rearm_turn_token(), InteractionState (Idle/Ready/Listening/Thinking/Speaking/Paused/Error/Sleeping=7) reused on BOTH tracks (current_state_atomic + dictation_state_atomic + ingestion_gate invariant), InteractionOwner
│   │   ├── settings.rs                         # VoxSettings::load (JSON) incl. dictation.{enabled,interaction_mode,output_mode,hotkey}
│   │   └── events.rs                           # VoxEvent incl. PttStart/PttStop/PttCancel, IpcEvent (registry-owned)
│   └── utils/paths.rs                          # get().models, db_path, settings_path
│
├── tests/
│   ├── common/
│   │   ├── mod.rs                              # re-exports
│   │   ├── audio.rs                            # decode_wav_to_mono_16k, stream_audio_to_ring_buffer, stream_silence_frames, wait_for_buffer_drain
│   │   ├── scoring.rs                          # normalize_text, calculate_similarity, assert_similarity_above, extract_acoustic_features
│   │   ├── paths.rs                            # get_asset_path, get_nemotron_model_dir, get_supertonic_model_dir, get_qwen_model_path
│   │   └── harness.rs                          # create_mock_playback_engine, get_test_app_handle, setup_stt_worker, setup_vad_actor, drain helpers, attach_mock_engine_*
│   ├── assets/
│   │   ├── edgetts_01_en_briefing.wav
│   │   ├── edgetts_07_hi_weather.wav
│   │   ├── supertonic_01_en_briefing.wav
│   │   └── supertonic_07_hi_weather.wav
│   ├── passive_streaming_test.rs               # Seam 1
│   ├── ptt_window_modular_test.rs              # Seam 2 (Modular branch only)
│   ├── ptt_window_realtime_test.rs             # Seam 3 (Realtime branch only)
│   ├── dictation_window_test.rs                # Seam 4
│   ├── transcript_to_llm_test.rs               # Seam 5
│   ├── llm_to_tts_test.rs                      # Seam 6 (LLM→TTS)
│   ├── tts_to_playback_test.rs                 # Seam 7 (TTS→Playback)
│   ├── tts_transition_test.rs                  # Seam 8 (filler + SetVoice hot-swap)
│   ├── playback_interrupt_test.rs              # Seam 9 (+ VAD suppression)
│   ├── chunking_determinism_test.rs            # Seam 10 (Seam X — clause chunking IT)
│   ├── session_lifecycle_test.rs               # Seam 11
│   ├── memory_compaction_test.rs               # Seam 12 #[ignore] Nvidia API (mock schema variant always runnable)
│   ├── memory_ingestion_test.rs                # Seam 13 local ONNX
│   ├── memory_retrieval_test.rs                # Seam 14 local ONNX
│   ├── settings_persistence_test.rs            # Seam 15
│   ├── model_eviction_test.rs                  # Seam 16
│   └── model_manager_test.rs                   # Seam 17
│
└── benches/  evals/  examples/  (per testing-style-guide.md:2,8 separate)
```

### `common/audio.rs` — Required Functions (HEAD-accurate)

```rust
pub fn decode_wav_to_mono_16k(path: &Path) -> Result<Vec<f32>, String> // hound + linear resample 16k
pub fn decode_wav_to_i16(path: &Path) -> Result<Vec<i16>, String>     // f32->i16 clamp
pub fn stream_audio_to_ring_buffer(audio: &[f32], producer: &mut impl Producer<Item=f32>) // VAD_CHUNK_SIZE=256 chunks, backpressure sleep 1ms
pub fn stream_silence_frames(producer: &mut impl Producer<Item=f32>, n_frames: usize) // zero-f32 frames
pub fn wait_for_buffer_drain(producer: &impl Observer, timeout_secs: u64) // occupied_len==0 spin-wait
```

### `common/scoring.rs` — Required Functions

```rust
pub fn normalize_text(text: &str) -> String // <unk> strip, ascii punct strip, lower, collapse ws
pub fn calculate_similarity(hyp: &str, ref_str: &str) -> f32 // char Levenshtein normalized [0,1]
pub fn assert_similarity_above(hyp: &str, ref_str: &str, threshold: f32, label: &str)
pub fn extract_acoustic_features(path: &Path) -> Result<AcousticReport, String> // RMS, duration, non_silent_ratio
pub fn assert_acoustic_within_tolerance(gen: &AcousticReport, golden: &AcousticReport, tolerances: &AcousticTolerances, label: &str)
```

### `common/paths.rs` — Required Functions

```rust
pub fn get_asset_path(filename: &str) -> PathBuf  // tests/assets/ candidates
pub fn get_nemotron_model_dir() -> PathBuf        // ~/.vox/models/stt/nemotron-3.5/ via utils::paths::get()
pub fn get_supertonic_model_dir() -> PathBuf
pub fn get_qwen_model_path() -> PathBuf           // ~/.vox/models/llm/qwen/ QWEN_MODEL_FILE
```

### `common/harness.rs` — Required Functions (TO-BUILD against HEAD signatures)

> Status: aspirational — `tests/` currently contains only `notifications_crud_test.rs`; `tests/common/` does not exist yet. Signatures below are the build contract, derived from production call sites (`services/vad/actor.rs:VadActorHandles/VadActorChannels`, `services/stt/actor.rs`, `core/state.rs:PipelineAtomics`). No helper may reimplement producer logic (ring push, VadValidation, chunking) per `create-test:Phase1`.

```rust
pub fn create_mock_playback_engine() -> (Arc<PlaybackEngine>, Arc<Mutex<HeapCons<f32>>>)
// HeapRb<f32>::new(PLAYBACK_BUFFER_SAMPLES=1_440_000) + PlaybackEngine::from_parts() no CPAL

pub fn get_test_app_handle() -> AppHandle<tauri::test::MockRuntime> // tauri::test::mock_app().handle()

pub fn setup_stt_worker<R: tauri::Runtime + 'static>(_app: &AppHandle<R>)
    -> (Sender<SttCommand>, Receiver<VoxEvent>, Arc<AtomicBool>, JoinHandle<()>)
 // EmbeddedSttProvider::new(nemotron_dir) + spawn_stt_worker(channels, provider, handles)

pub fn setup_vad_actor(
    stt_tx: Sender<SttCommand>,
    config: VadActorConfig,              // initial_threshold, initial_noise_gate, initial_mode:InteractionMode, initial_audio_mode:AudioOutputMode
    state_atomic: Arc<AtomicU32>,        // PipelineAtomics::current_state_atomic (assistant track; dictation tests pass dictation_state_atomic where the SUT reads dictation track)
    turn_id_atomic: Arc<AtomicU32>,      // PipelineAtomics::turn_id — VAD stamps current_turn_id from here (actor.rs:VadActorHandles)
    audio_suppressed: Arc<AtomicBool>,   // should_suppress_audio flag
    ingestion_gate: Arc<AtomicBool>,     // PipelineAtomics::ingestion_gate — Layer-2 purge gate (actor.rs:487); tests MUST wire the real atomic, never a detached Arc::new(false)
    engine_shutdown: Arc<AtomicBool>,
) -> (Sender<VadCommand>, Receiver<VoxEvent>, RbProducer, JoinHandle<()>)
 // HeapRb 65536, EarshotVadEngine::new(threshold), spawn_vad_actor(vad_backend, consumer, channels, handles, config)
 // MUST also wire telemetry_tx (crossbeam) + vox_event_tx (SpeechStart/End) through VadActorChannels; a harness that drops vox_event_tx makes Seam 1/9 suppression tests vacuously green.

pub fn drain_for_final_transcript(rx: &Receiver<VoxEvent>, expected_turn_id: u32, timeout: Duration) -> Result<String, String>
pub fn collect_all_final_transcripts(rx: &Receiver<VoxEvent>, expected_turns: usize, timeout: Duration) -> String // join(" ")
pub fn assert_channel_empty_after<T: Debug>(rx: &Receiver<T>, wait: Duration, label: &str) // sleep(wait) then try_recv must be Empty — DO NOT use short recv_timeout as proxy for absence

pub fn get_test_app_and_state() -> (AppHandle<MockRuntime>, Arc<AppState>) // Arc<TelemetryState> + AppState::new(app, None, telemetry) + app.manage(state)
pub fn get_test_app_state() -> AppState
pub fn attach_mock_engine_with_vad_to_state<R: Runtime>(_app: &AppHandle<R>, state: &AppState, stt_tx: Sender<SttCommand>, vad_tx: Sender<VadCommand>)
pub fn attach_mock_engine_to_state<R: Runtime>(app: &AppHandle<R>, state: &AppState, stt_tx: Sender<SttCommand>)
```

> Helpers are **valid only if replaceable by a direct production call** `create-test:Phase1 Functions I will write`. No helper may reimplement producer logic (ring push, VadValidation, chunking).

---

## Seam 1 — Passive Streaming: Ring Buffer → ContinuousSegmentation → STT → TranscriptFinal

**File:** `tests/passive_streaming_test.rs`
**Handlers:** `pipeline/assistant/speech.rs:on_speech_start/end` + `pipeline/router.rs` (owner==Assistant) → `services/vad/actor.rs:process_continuous_segmentation` + `services/stt/actor.rs` (Final → TranscriptFinal ALWAYS, even empty)
**Status:** P1 Unblocked — local Nemotron, Earshot, no network
**Preconditions (else drops are setup bugs):** `owner=Assistant`, `interaction.mode=Passive`, assistant state `Ready` (gate open), `ingestion_gate` wired to the real `PipelineAtomics` atomic (a detached gate stuck closed purges everything — Layer-2 `actor.rs` gate head).

### Phase 1 — Production Path Trace

```
SUT: Autonomous VAD segments microphone stream into utterances and dispatches completed utterance to STT without explicit trigger.

Production Entry Seam:
  SPSC ring buffer producer push (simulates CPAL mic engine). Test calls stream_audio_to_ring_buffer(&f32@16k, &mut producer) in VAD_CHUNK_SIZE=256 chunks with 1ms pacing.

Direction Check: PASS — ring push is upstream TRIGGER, not STT output. Test does NOT call SttCommand::Final directly; VAD actor does on speech offset.

Production Path:
  stream_audio_to_ring_buffer(chunk) -> HeapRb producer.push_slice(chunk)
  → VadActor loop consumer.occupied_len()>=VAD_CHUNK_SIZE then pop_slice else sleep VAD_ACTOR_IDLE_SLEEP_MS (actor.rs `spawn_vad_actor` loop; Layer-2 gate head purges + drains while `ingestion_gate` closed — test MUST keep gate open here)
  → process_and_emit_telemetry(&chunk) -> raw_energy
  → should_suppress_audio(&audio_suppressed, &state_atomic, &state) == false  (state=Ready/Passive, audio_mode=Headset or Speaker+not Speaking)
  → operational_mode == ContinuousSegmentation (config.initial_mode=Passive) -> process_continuous_segmentation(chunk, raw_energy, vad, state, handles, stt_tx, vox_event_tx)
     → vad.predict(chunk) && is_above_noise_gate(raw_energy, noise_gate) -> decides is_speech
     → active_frames >= speech_start_frames (derived from speech_onset_ms) -> handle_speech_start(state, handles, stt_tx, vox_event_tx)
        → in_speech=true, current_turn_id = turn_id_atomic.load(), pre_roll_buffer.copy_into(&mut utterance_buffer), VoxEvent::SpeechStart via vox_event_tx, SttCommand::ResetStream via stt_tx
     → while in_speech: accumulate_speech_frames(chunk, state, stt_tx) -> utterance_buffer.extend, partial dispatch every VAD_PARTIAL_INTERVAL_SAMPLES
     → inactive_frames >= speech_end_frames (derived from silence_duration_ms) -> handle_speech_end(vad, state, stt_tx, vox_event_tx)
        → in_speech=false, vad.flush(), VoxEvent::SpeechEnd, if utterance_buffer.len() >= VAD_MIN_UTTERANCE_SAMPLES && realtime_tx.is_none() -> stt_tx.send(SttCommand::Final(turn_id, utterance_buffer.clone()))
  → STT actor (Nemotron transducer) -> VoxEvent::TranscriptFinal { turn_id, text } via pipeline_event_tx (ALWAYS emitted, even empty — stt/actor.rs:emit_final_events)
  → assistant/speech.rs on_speech_start: Ready=>`next_turn()` bundle + `transition(Listening)` (+`ResetStream` via try_lock; Thinking/Speaking=>`on_interrupt` barge-in) + on_speech_end: Listening=>`transition(Thinking)`; transcript requires Thinking (assistant/transcript.rs drops unless Thinking) and routes via assistant/transcript.rs to LLM dispatch

Observable Exit:
  1. VoxEvent::SpeechStart via vox_event_rx
  2. VoxEvent::SpeechEnd via vox_event_rx
  3. VoxEvent::TranscriptFinal.text via pipeline_event_rx — similarity >= 0.90 vs ground truth (Levenshtein, normalize_text)

Production functions called:
  setup: setup_stt_worker(&app), setup_vad_actor(stt_tx, VadActorConfig{InitialMode Passive}, state_atomic=Ready, audio_suppressed=false), create_mock_playback_engine for attach
  entry: stream_audio_to_ring_buffer() + stream_silence_frames(VAD_SPEECH_END_FRAMES+20) + wait_for_buffer_drain(5s)
  observe: vox_event_rx (SpeechStart/End), pipeline_event_rx (TranscriptFinal)
  teardown: VadCommand::Shutdown, SttCommand::Shutdown, engine_shutdown.store(true), join().expect("panicked")

Functions written in test file: None — all helpers in common/.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (stream_audio_to_ring_buffer never called / deleted)** | **Yes — SpeechStart never fires, 5s deadline fails, transcript timeout — mandatory create-test:2b row** |
| VAD actor never pops from ring buffer (consumer.occupied_len check removed in `spawn_vad_actor` loop) | Yes — SpeechStart never fires, 5s deadline fails, transcript timeout |
| speech onset debouncing broken (active_frames threshold ignored — `speech_start_frames` gate removed) | Yes — noise bursts trigger SpeechStart on silence test, silence guard fails |
| speech offset broken (inactive_frames never reaches `speech_end_frames`) | Yes — SpeechEnd assertion fails, transcript timeout (no Final dispatched) |
| SttCommand::Final minimum-utterance guard `utterance_buffer.len() >= VAD_MIN_UTTERANCE_SAMPLES` deleted or always false | Yes — transcript timeout, collect_all_final_transcripts returns "" |
| STT produces wrong text (model drift / stitcher broken) | Yes — calculate_similarity < 0.90 |
| should_suppress_audio returns true during passive streaming (state=Speaking leaked) | Yes — audio suppressed, no SpeechStart |
| realtime_tx inserted accidentally (passthrough mode) | Yes — ContinuousSegmentation bypassed, no speech events |

### Test Functions

```rust
#[test]
fn test_passive_streaming_en() // EN edgetts_01_en_briefing.wav -> SpeechStart+SpeechEnd+sim>=0.90, sync: Instant::now()+Duration::from_secs(60) deadline + recv_timeout 200ms, join().expect("panicked")

#[test]
fn test_passive_streaming_hi() // HI edgetts_07_hi_weather.wav -> same, transliterate_if_hi checked, same 60s deadline

#[test]
fn test_passive_streaming_silence_only() // 100 silence frames -> assert_channel_empty_after(500ms) on both vox_event_rx && pipeline_event_rx — NEGATIVE, Instant + 5s drain wait

// Consolidated per testing-style-guide.md:7.3 Single Worker Lifecycle — if Nemotron re-warm SIGSEGV observed, run EN then HI then silence sequentially in ONE #[test] with single setup_stt_worker/setup_vad_actor, otherwise isolated tests each with Instant::now()+60s hard timeout per testing-style-guide.md:7.1
```

---

## Seam 2 — PTT Window Validation (Modular): Press → VAD Window → SttCommand::Final

**File:** `tests/ptt_window_modular_test.rs`
**Handlers:** `pipeline/assistant/ptt.rs:on_ptt_start/on_ptt_stop/on_ptt_cancel` (Modular branch) + `services/vad/actor.rs:StartWindowValidation/StopWindowValidation` + `pipeline/mod.rs:RoutingContext` + `core/state.rs:PipelineAtomics`
**Status:** P1 Unblocked — local Nemotron, no cloud

### Phase 1 — Production Path Trace (Modular branch only)

```
SUT: User holds/releases PTT in Modular pipeline_mode; VAD window evaluates speech and dispatches trimmed audio to STT or discards ghost audio.

Production Entry Seam:
  ptt_start(&app, &state) -> ptt_stop(&app, &state) bracketing ring buffer pushes.
  ptt_start is the upstream TRIGGER; test does NOT call SttCommand::Final directly.

Direction Check: PASS — handle is pipeline/assistant/ptt.rs, not stt consumer.

Precondition the test must set:
  state.owner.store(Assistant), state.settings.write().interaction.mode=PTT, pipeline_mode=Modular
  otherwise on_ptt_start:14 drops in Passive mode — would be a false failing setup not a production bug.

Production Path:
  ptt_start(&app, &state)  // assistant/ptt.rs:13
    → RoutingContext::from_app_state(state) reads owner/mode/pipeline_mode
    → if Idle/Paused/Listening => drop/return; if Thinking/Speaking => on_interrupt() barge-in path
    → else next_turn() => (turn_id, CancellationToken), accumulator.clear(), cancel_flag=false, playback_engine.cancel()
    → transition(Listening) via pipeline/mod.rs:60 idempotency guard
    → engine.vad_tx.send(VadCommand::StartWindowValidation)  // actor.rs:147
      → state.window_active=true, window_buffer.clear(), pre_roll_buffer.copy_into(window_buffer), window_sample_offset=len, clear pre_roll
  → stream_audio_to_ring_buffer(&f32@16k, &mut producer) while PTT held
    → VadActor::process_windowed_validation(chunk, raw_energy, vad, state) // actor.rs:386
      → if !window_active => pre_roll_buffer.push(chunk); return
      → window_buffer.extend(chunk); predict+gate => update window_speech_detected, window_first_speech_sample, window_last_speech_sample, window_sample_offset += len
  → ptt_stop(&app, &state)  // assistant/ptt.rs:95 async
    → if state != Listening => drop
    → turn_id=peek_turn_id(), clone vad_tx+stt_tx from engine.blocking_lock()
    → vad_tx.send(StopWindowValidation{response_tx}), recv_timeout(VAD_VALIDATION_TIMEOUT_MS=500)
    → validation_result =Ok(VadValidationResult{is_speech_detected, speech_start/end, audio: trimmed_audio })
      trimmed = if detected && start<end && end-start>=256 { window_buffer[start..end].to_vec() } else if detected { take(window_buffer) } else { Vec::new() } // actor.rs:170
    → if !is_speech || audio.is_empty() => log discard, transition(Ready), return (GHOST GATE)
    → else dispatch_ptt_speech_audio(turn_id, audio, &stt_tx, app, state, ctx)
         → transition(Thinking) // ptt.rs:72 even for Modular/Realtime unified
         → stt_tx.send(SttCommand::Final(turn_id, audio)) || transition(Ready) on send failure // ptt.rs:75

Observable Exit:
  Ghost path: state==Ready, VoxEvent TranscriptFinal absent, assert_channel_empty_after 500ms
  Speech path: state==Thinking, TranscriptFinal arrives via pipeline_event_rx with sim>=0.90

Production functions called:
  setup: get_test_app_and_state(), setup_stt_worker, setup_vad_actor with PTT config + state_atomic=Ready, attach_mock_engine_with_vad_to_state, settings.pipeline_mode=Modular
  entry: ptt_start(), stream_audio_to_ring_buffer() or stream_silence_frames(), wait_for_buffer_drain(1s), ptt_stop().await / ptt_cancel()
  observe: state.pipeline.state(), pipeline_event_rx (drain_for_final_transcript)
  teardown: VadCommand::Shutdown, SttCommand::Shutdown if present, join().expect

Functions written in test file: None beyond common.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (ptt_start never invoked / deleted)** | **Yes — state stays Ready, window never active, ghost path always, modular speech never dispatched** |
| ptt_start never sends VadCommand::StartWindowValidation (send deleted in `assistant/ptt.rs:on_ptt_start`) | Yes — window_active stays false, window_buffer empty => ghost path always, TranscriptFinal timeout |
| window_speech_detected never set true (predict gate broken in `process_windowed_validation`) | Yes — ghost gate fires, state==Ready not Thinking |
| ghost gate deleted (`if !is_speech || audio.is_empty()` removed in `assistant/ptt.rs:on_ptt_stop`) | Yes — silence hold would incorrectly go to Thinking and send Final for silence; assert_channel_empty_after fails |
| dispatch routes to Realtime branch instead of Modular (branch swap at `dispatch_ptt_speech_audio`: Modular↔Realtime) | Yes — modular test sees no TranscriptFinal, timeout |
| RoutingContext mode check missing (`if interaction_mode==Passive` drop removed in `on_ptt_start`/`on_ptt_stop`/`on_ptt_cancel`) | Yes — Passive PTT would incorrectly start window; complementary negative test would fire where it should drop |
| transition(Thinking) skipped in `dispatch_ptt_speech_audio` | Yes — state remains Listening where test expects Thinking (and downstream transcript handler requires Thinking, so LLM dispatch never happens) |
| ptt_cancel does not cancel turn_token (turn_token().cancel() removed in `on_ptt_cancel`) | Yes — cancel test asserts turn_token.is_cancelled() |

### Test Functions

```rust
#[tokio::test]
async fn test_ptt_modular_speech_transmits_to_stt() // tokio::time::timeout(30s, async { ... }).await.expect("hard timeout") — EN clip, Modular mode, assert Thinking + TranscriptFinal sim>=0.90, join().expect

#[tokio::test]
async fn test_ptt_modular_ghost_gate_silence_reverts_to_ready() // timeout 10s — stream_silence_frames(20), assert Ready + assert_channel_empty_after(500ms)

#[tokio::test]
async fn test_ptt_modular_cancel_discards_and_cancels_token() // timeout 10s — stream speech, ptt_cancel -> Ready + token cancelled + empty channel
```

---

## Seam 3 — PTT Window Validation (Realtime): Press → VAD Window → Realtime Commit

**File:** `tests/ptt_window_realtime_test.rs`
**Handlers:** `pipeline/assistant/ptt.rs:on_ptt_start/on_ptt_stop` (Realtime branch) + `services/vad/actor.rs` + `services/realtime/RealtimeActor::signal_speech_committed`
**Status:** P1 Unblocked — uses Mock RealtimeActor to avoid cloud; validates ghost gate locally

### Phase 1 — Production Path Trace (Realtime branch only)

```
SUT: User holds/releases PTT in Realtime pipeline_mode; identical VAD window to Seam 2 but dispatches trimmed i16 to cloud actor.

Production Entry Seam:
  ptt_start(&app, &state) -> ptt_stop(&app, &state) bracketing ring buffer pushes. Upstream TRIGGER.

Direction Check: PASS — not RealtimeSession::commit.

Precondition: state.owner=Assistant, interaction.mode=PTT, pipeline_mode=Realtime; else drops.

Production Path:
  Identical prefix to Seam 2 through ptt_stop validation_result trimming // ptt.rs:13-134, actor.rs:147/386/170
    → if !is_speech || audio.is_empty() => Ready (ghost)
    → else dispatch_ptt_speech_audio(turn_id, audio, &stt_tx, app, state, ctx) // ptt.rs:64
         → transition(Thinking) // ptt.rs:72 unified — Realtime also transitions Thinking (not Listening); Listening is only after cloud TranscriptFinal arrival
         → PipelineMode::Realtime => f32->i16 convert (clamp *32767), realtime_engine.try_lock().signal_speech_committed(&i16_samples) // ptt.rs:84
            → RealtimeActor internal: ActivityStart -> PCM chunks -> ActivityEnd framing (providers/gemini/deepgram) OR direct enqueue (Deepgram)
         // State remains Thinking until cloud returns TranscriptFinal via realtime transport -> on_transcript_final -> pending=1 handling

Observable Exit:
  Ghost path: state==Ready, push_counter==0, commit_counter==0, assert_channel_empty_after
  Speech path: state==Thinking (immediate post-dispatch), push_counter>0, commit_counter==1, zero local TranscriptFinal (cloud transcript arrives later via RealtimeActor event loop)

Production functions called:
  setup: get_test_app_and_state(), MockRealtimeActor (RealtimeActor::new(provider, handle).start(PTT, mock_playback, dummy_tx, app)), setup_vad_actor with PTT config + state_atomic=Ready, attach_mock_engine_with_vad_to_state, state.realtime_engine.lock().await = Some(mock_actor), settings.pipeline_mode=Realtime
  entry: ptt_start(), stream_audio_to_ring_buffer() or stream_silence_frames(), wait_for_buffer_drain(1s), ptt_stop().await / ptt_cancel()
  observe: state.pipeline.state(), MockRealtimeSession push/commit counters (Arc<AtomicUsize>), pending_synthesis_jobs not used for realtime (store 1 at transcript.rs:198 for realtime)
  teardown: VadCommand::Shutdown, SttCommand::Shutdown, join().expect, realtime_engine stop

Functions written in test file: MockRealtimeSession/MockProvider + create_mock_actor only (replaceable by prod provider iface).
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (ptt_start deleted)** | **Yes — window never active, ghost path, push 0** |
| window_speech_detected never true (predict gate broken in `process_windowed_validation`) | Yes — ghost gate fires, Ready not Thinking |
| ghost gate deleted (removed in `assistant/ptt.rs:on_ptt_stop`) | Yes — silence would incorrectly commit to cloud; push_counter==0 fails |
| dispatch routes to Modular STT instead of Realtime (branch swap in `dispatch_ptt_speech_audio`) | Yes — push 0 while expecting push>0, plus spurious TranscriptFinal on mock STT |
| f32->i16 conversion clamping removed (`x.clamp(-1.0, 1.0)` deleted in `dispatch_ptt_speech_audio` Realtime branch; mock feeds a full-scale 0.99 sine fixture and asserts every committed sample equals `(x.clamp(-1,1)*32767) as i16`) | Yes — unclamped overshoot samples wrap/clip and the sample-exact assertion fails |
| signal_speech_committed deleted / not awaited | Yes — commit_counter==0 while expected 1 |
| realtime_engine not locked (try_lock skipped, panic path) | Yes — silent drop would leave commit 0 |

### Test Functions

```rust
#[tokio::test]
async fn test_ptt_realtime_speech_commits_to_actor() // timeout 10s — EN clip, Realtime mode, assert Thinking + push>0 + commit==1, join().expect

#[tokio::test]
async fn test_ptt_realtime_ghost_gate_silence_reverts_to_ready() // timeout 10s — silence 20 frames, assert Ready + push 0 + commit 0

#[tokio::test]
async fn test_ptt_realtime_cancel_discards_and_cancels_token() // timeout 10s — stream speech, ptt_cancel -> Ready + token cancelled + push 0
```

---

## Seam 4 — Dictation PTT + Passive + Gate: VoxEvent::PttStart/PttStop/PttCancel → VAD Window → STT → OutputRouter (Not LLM)

**File:** `tests/dictation_window_test.rs`
**Handlers:** `pipeline/dictation/mod.rs:transition_dictation/handle_event` + `pipeline/dictation/ptt.rs:on_ptt_start/on_ptt_stop(+_with_sender)/on_ptt_cancel` + `pipeline/dictation/speech.rs:on_speech_start/on_speech_end` + `pipeline/dictation/transcript.rs:on_transcript_final` + `pipeline/dictation/error.rs:on_error/on_cancelled` + `pipeline/router.rs` (owner==Dictation guard) + `services/dictation/hotkey.rs:init_dictation_hotkey_listener` + `services/dictation/output_router.rs:route_transcript`
**Status:** P1 Unblocked — Nemotron required, no LLM

> v2.2 rewrite note: v2.1 described a fictional API (`handle_hotkey_press/release`, `Recording`/`Transcribing` states, `handle_hotkey_release_with_sender`, direct `engine.lock().await`). None of those exist at HEAD. Dictation reuses the unified `InteractionState` enum on its own track (`dictation_state_atomic`), accepts only `Idle/Ready/Listening/Thinking/Error` in `transition_dictation` (`dictation/mod.rs:24-33` warns+returns otherwise), is entered exclusively via `VoxEvent::PttStart/PttStop/PttCancel` through `state.event_tx` → `router.rs:31` owner guard, uses non-blocking `try_lock()` (never `blocking_lock()` — router-stall avoidance), allocates turns via `next_turn()` bundle (never bare `next_turn_id()`), and is gated by the Option-C `ingestion_gate` (Layer-1 `services/audio/device.rs:142` CPAL drop + Layer-2 `services/vad/actor.rs:487` buffer purge).

### Phase 1 — Production Path Trace

```
SUT: Dictation hotkey/passive accumulation dispatches to STT and routes transcript to OS injection, never to LLM; ingestion gate, owner handover, and error recovery hold.

Production Entry Seam:
  VoxEvent::PttStart -> VoxEvent::PttStop (or PttCancel) sent via state.event_tx (the same Sender the hotkey listener uses).
  Upstream trigger is the global hotkey press/release; test does NOT call SttCommand::Final or output_router directly.
  Direct on_ptt_start/stop calls are handler-level and bypass the router.rs:31 owner guard — canonical IT sends VoxEvent through event_tx with owner pre-set to Dictation. The on_ptt_stop_with_sender(stt_tx override) hook exists ONLY for tests (ptt.rs:52); production on_ptt_stop() calls it with None.

Direction Check: PASS — VoxEvent via event_tx is the upstream TRIGGER (what services/dictation/hotkey.rs:init_dictation_hotkey_listener sends on Press/Release); calling output_router::route_transcript or SttCommand::Final directly would test the sink.

Preconditions the test MUST set (else drops are setup bugs, not production bugs):
  state.owner.store(Dictation)
  settings.dictation.enabled=true, settings.dictation.interaction_mode=Ptt (PTT tests) or Passive (passive test)
  state.pipeline.transition_dictation(Ready) via transition_dictation() (emits StateChanged{owner:Dictation} to WINDOW_TRAY — do NOT write dictation_state_atomic directly)
  state.pipeline.update_ingestion_gate() open (Ready on dictation track opens it per state.rs:142-156)
  VoxEngine attached with vad_tx + stt_tx visible via try_lock (attach_mock_engine_with_vad_to_state)

Production Path A — PTT window (hotkey hold):
  hotkey Press -> event_tx.send(VoxEvent::PttStart) -> router (owner==Dictation) -> dictation::handle_event -> ptt::on_ptt_start(app, state) // dictation/ptt.rs:8
    → dictation_state Idle => error::on_error(0, "Dictation is disabled in Settings.") + return (NOT silent drop)
    → Listening => return idempotent; Thinking => pipelined overlap (fall through, allocate N+1 without aborting N)
    → Ready => next_turn() bundle (turn_id + renewed CancellationToken), cancel_flag=false
    → engine.try_lock() (non-blocking) => vad_tx.send(StartWindowValidation) // window_active=true, window_buffer seeded from pre-roll
    → transition_dictation(Listening) + StateChanged{Dictation,Listening} to TRAY
  → stream_audio_to_ring_buffer(&f32@16k) while Listening (VAD window accumulates: process_windowed_validation updates window_speech_detected/first/last + offset)
  → hotkey Release -> event_tx.send(VoxEvent::PttStop) -> router -> ptt::on_ptt_stop -> on_ptt_stop_with_sender(app, state, None) // dictation/ptt.rs:47
    → if dictation_state != Listening => drop
    → peek_turn_id(), try_lock snapshot (vad_tx_opt, engine_stt_tx_opt); contended => (None,None) => ghost path (no panic)
    → vad_tx.send(StopWindowValidation{response_tx}) + recv_timeout(VAD_VALIDATION_TIMEOUT_MS) => VadValidationResult{is_speech_detected, audio: trimmed}
    → if !is_speech || audio.is_empty() => transition_dictation(Ready) + return (GHOST GATE, ptt.rs:97)
    → transition_dictation(Thinking)
    → stt_tx override Some(tx) ? tx.send(Final(turn_id, audio)) : engine_stt_tx.send(Final(turn_id, audio))
  → STT actor handle_final_command -> transcribe_chunk(final=true) -> emit_final_events ALWAYS sends VoxEvent::TranscriptFinal{turn_id, text} even when text=="" (stt/actor.rs:160-208; empty-text validation lives downstream, not in STT)
  → router (owner still Dictation) -> dictation/transcript.rs:on_transcript_final // dictation/transcript.rs:9
    → if dictation_state==Idle => drop (disabled)
    → if text.trim().is_empty() => transition Ready (unless pipelined Listening preserved) + toast "No speech recognized", return
    → transliterate_if_hi(text), read settings.dictation.output_mode, dictation_last_transcript=Some(processed)
    → spawn output_router::route_transcript(&app, &text, mode).await // Tray=bypass, Clipboard=set_text+toast, Paste=with_clipboard_safe+simulate_paste with clipboard-fallback
    → if dictation_state != Listening => transition_dictation(Ready) (pipelined N+1 Listening preserved)
    → emit_ipc_to(TRAY, TranscriptFinal{turn_id, text: processed, owner: Dictation})
  // LLM must NOT be touched — no LlmCommand::Generate, no tts_tx.send, no pending_synthesis_jobs mutation anywhere on this track.

Production Path B — Passive dictation (background speech, no hotkey):
  settings.dictation.interaction_mode=Passive => RoutingContext maps to InteractionMode::Passive; VAD ContinuousSegmentation; owner==Dictation routes SpeechStart/End + TranscriptFinal to dictation/speech.rs + transcript.rs
  speech::on_speech_start // dictation/speech.rs:7: Idle=>drop, Listening=>drop, Thinking=>overlap allocate N+1, Ready=>next_turn()+transition Listening
  speech::on_speech_end // dictation/speech.rs:25: Listening=>transition Thinking (STT Final was already dispatched by VAD handle_speech_end on the continuous path)
  transcript path identical to Path A tail (output_router, never LLM).

Production Path C — Ingestion gate (Option C, 2-layer; tested here because dictation Ready/Idle flips it):
  state.rs:143 update_ingestion_gate: OPEN <=> assistant in {Ready,Listening,Thinking,Speaking} OR dictation in {Ready,Listening,Thinking}.
  Layer-1 services/audio/device.rs:142 CPAL callback returns before push when gate closed (mic bytes never enter ring).
  Layer-2 services/vad/actor.rs:487 loop head: gate closed => clear pre_roll/utterance/window buffers + in_speech=false + window_active=false, drain ring by pop_slice, sleep, continue (stale audio can never dispatch STT after reopen).
  Dictation toggle enabled=false => transition_dictation(Idle) => gate may close (if assistant also Idle) => in-flight window purged.

Production Path D — Lifecycle integration (asserted lightly here, canonically in Seam 11):
  session.rs:on_pause + on_end unconditionally yield owner->Dictation and sync VAD SetOperationalMode to dictation.interaction_mode (Passive=>ContinuousSegmentation, Ptt=>WindowedValidation); on_end stops CPAL via stop_audio_engine_sync ONLY if dictation_state==Idle, else keeps engine for dictation.
  ipc/settings/mutation.rs:handle_dictation_side_effects: enabled toggle => transition_dictation(Ready|Idle) + VAD hot-reload when owner==Dictation + engine start/stop; interaction_mode change => live SetOperationalMode; hotkey key => init_dictation_hotkey_listener re-register.
  ipc/tray.rs:cancel_active_dictation_turn sends VoxEvent::PttCancel via event_tx when owner==Dictation (never calls on_ptt_cancel directly).
  dictation/error.rs:on_error logs, transition Error, VoiceError{source:"Dictation",owner:Dictation} to TRAY, gated toast via should_show_error_toast, auto-recover Ready if !=Idle; on_cancelled => Ready.

Observable Exit:
  1. Speech PTT hold: dictation_state Ready->Listening (after PttStart) ->Thinking (after valid PttStop, asserted BEFORE transcript arrives) ->Ready (after TranscriptFinal, unless pipelined overlap holds Listening); VoxEvent::TranscriptFinal non-empty via pipeline_event_rx, sim>=0.90 vs EN ground truth.
  2. Ghost hold (silence / immediate release): dictation_state==Ready, pipeline_event_rx empty via assert_channel_empty_after(500ms), no STT Final dispatched.
  3. LLM zero invariant: llm_rx (LlmCommand channel injected into VoxEngine) empty via assert_channel_empty_after(500ms) after TranscriptFinal; dictation_last_transcript==Some(processed); TRAY TranscriptFinal{owner:Dictation} emitted (captured via mock IPC or state assertion — at minimum assert owner field, not just text).
  4. Cancel: PttCancel via event_tx while Listening => Ready + no TranscriptFinal + no STT dispatch (tray path).
  5. Passive: SpeechStart=>Listening, SpeechEnd=>Thinking, TranscriptFinal=>Ready with same LLM-zero invariant.
  6. Gate: gate closed => buffers purged; audio streamed while closed then gate reopened => still no TranscriptFinal (stale-audio kill).
  7. Error: PttStart while Idle => VoiceError{source:"Dictation"} to TRAY + auto-recover (stays Idle because disabled — assert NO transition to Ready and NO window activation); empty transcript => "No speech recognized" toast + Ready.

Production functions called:
  setup: get_test_app_and_state() with owner=Dictation + dictation.enabled=true, setup_stt_worker, setup_vad_actor(PTT config for window tests / Passive config for passive test; MUST pass real ingestion_gate + turn_id_atomic + telemetry_tx + vox_event_tx — never a detached gate), attach_mock_engine_with_vad_to_state (+ inject llm_tx capture channel into VoxEngine for zero-invariant), transition_dictation(Ready)
  entry: event_tx.send(VoxEvent::PttStart) / PttStop / PttCancel (canonical); stream_audio_to_ring_buffer / stream_silence_frames between Start and Stop; on_ptt_stop_with_sender(app, state, Some(&test_stt_tx)) ONLY where the test needs to observe Final without a full engine STT
  observe: state.pipeline.dictation_state(), state.pipeline.ingestion_gate.load(), pipeline_event_rx (drain_for_final_transcript), dictation_last_transcript lock, llm_rx assert_channel_empty_after, TRAY StateChanged/TranscriptFinal/VoiceError emissions
  teardown: VadCommand::Shutdown, SttCommand::Shutdown, engine_shutdown.store(true), join().expect("panicked")

Functions written in test file: None beyond common. (The stt_tx override is a production-provided test hook at ptt.rs:52 — using it is not reimplementation.)
```

### Phase 2b — False-Green Table

| Defect (one-line production mutant, `mutate` taxonomy) | Would test fail? |
| --- | --- |
| **upstream producer completely silent (no VoxEvent::PttStart sent via event_tx / on_ptt_start never invoked — hotkey listener deleted)** [silent drop] | **Yes — dictation_state stays Ready, window never active, ghost path always, TranscriptFinal timeout — mandatory `create-test:2b` row** |
| `vad_tx.send(StartWindowValidation)` deleted in `dictation/ptt.rs:on_ptt_start` (try_lock block kept, send removed) [silent drop] | Yes — window_active stays false, window_buffer empty => ghost gate fires every time, speech hold yields Ready not Thinking, TranscriptFinal timeout |
| ghost gate deleted (`if !is_speech \|\| audio.is_empty()` at `dictation/ptt.rs:97` removed; always proceeds to Thinking+Final) [gate inversion] | Yes — silence hold dispatches STT and goes Thinking where ghost test asserts Ready + `assert_channel_empty_after` on pipeline_event_rx fails |
| `transition_dictation(Thinking)` deleted in `on_ptt_stop_with_sender` valid path (Final still sent) [silent drop] | Yes — state stays Listening after valid stop where test asserts Thinking-before-transcript; pipelined-overlap assertion also breaks |
| transcript routes to LLM (`transcript.rs` spawn `route_transcript` replaced with `llm_tx.send(Generate{...})`, shape unchanged) [routing swap] | Yes — `llm_rx` not empty where test asserts `assert_channel_empty_after(500ms)`; `dictation_last_transcript` stays None where expected Some — the mutation killer; survivor if llm_rx assertion omitted |
| Idle-start error path neutered (`error::on_error(0, …)` call at `dictation/ptt.rs:12-18` replaced with silent `return`) [default/early-return fallback] | Yes — PttStart while Idle emits no `VoiceError{source:"Dictation"}` to TRAY where error test asserts it; window would also incorrectly stay shut without the toast contract |
| Layer-2 gate purge deleted (`services/vad/actor.rs:487-506` clear+`in_speech=false`+`window_active=false` block removed; gate check kept) [gate inversion] | Yes — audio streamed while gate closed survives in window_buffer; after reopen + PttStop the stale audio dispatches TranscriptFinal where gate test asserts absence |
| tray cancel send deleted (`tx.send(VoxEvent::PttCancel)` at `ipc/tray.rs:77` removed, function still returns Ok) [silent drop] | Yes — PttCancel via event_tx while Listening leaves state Listening where cancel test asserts Ready; follow-up TranscriptFinal arrives where expected absent |

> Dropped v2.1 rows and why (not real production mutants): `stt_tx override ignored` (the override is a test-only hook — ignoring it breaks only tests that use it, not production `on_ptt_stop()` which passes None); `state stays Recording / set Transcribing missing` (Recording/Transcribing do not exist — dictation reuses InteractionState; replaced by the Thinking-transition row above); `on_transcript_final never spawned` (duplicate of the LLM-routing row — merged).

### Test Functions

```rust
#[tokio::test]
async fn test_dictation_ptt_window_routes_to_output_not_llm() // timeout 30s — owner Dictation, Ptt mode, PttStart via event_tx, stream EN clip, PttStop via event_tx, assert Listening->Thinking->TranscriptFinal sim>=0.90->Ready + dictation_last_transcript Some + TRAY TranscriptFinal owner Dictation + llm_rx empty 500ms, join().expect

#[tokio::test]
async fn test_dictation_ghost_hold_discards_to_ready() // timeout 10s — PttStart, stream_silence_frames(20), PttStop => Ready + assert_channel_empty_after(500ms) on pipeline_event_rx + no Thinking observed

#[tokio::test]
async fn test_dictation_cancel_via_event_tx_discards() // timeout 10s — PttStart, stream speech, event_tx.send(PttCancel) (tray path) => Ready + assert_channel_empty_after on pipeline_event_rx + turn_token NOT required cancelled (dictation cancel is window-discard, unlike assistant barge-in)

#[tokio::test]
async fn test_dictation_passive_speech_routes_without_hotkey() // timeout 30s — interaction_mode Passive, ContinuousSegmentation, inject SpeechStart/SpeechEnd via event_tx (owner Dictation) + stream EN clip => Listening->Thinking->TranscriptFinal sim>=0.90 + llm_rx empty

#[test]
fn test_dictation_ingestion_gate_purge_drops_stale_audio() // Instant+15s — gate closed (assistant Idle + transition_dictation(Idle)), stream EN clip frames, assert buffers purged + ring drained, reopen gate (transition_dictation(Ready)), PttStart/Stop => ghost Ready + no stale TranscriptFinal

#[test]
fn test_dictation_idle_start_emits_voice_error_not_silence() // Instant+10s — transition_dictation(Idle) (disabled), event_tx.send(PttStart) => VoiceError{source:"Dictation"} to TRAY + stays Idle (no Listening, no window) — asserts error::on_error path, not silent drop

#[test]
fn test_dictation_transcript_never_touches_llm() // sync with Instant::now()+Duration::from_secs(10) deadline + recv_timeout 200ms loop per guide 7.1 — dictation::handle_event with TranscriptFinal injected (non-empty + empty variants), assert llm_rx empty + Ready (+ empty-toast contract) — the mutation killer
```

---

## Seam 5 — Transcript Handler → Context Harness → LLM Dispatch

**File:** `tests/transcript_to_llm_test.rs`
**Handlers:** `pipeline/assistant/transcript.rs:on_transcript_final` + `services/harness/facade.rs:prepare_turn_context` + `services/llm/actor.rs:spawn_llm_worker`
**Status:** P2 — Requires Qwen GGUF at `~/.vox/models/llm/qwen/` or mock provider; no network if embedded

### Phase 1 — Production Path Trace

```
SUT: Final user transcript triggers context preparation (buffer push + threshold maintenance + retrieval + prompt assembly) and dispatch of a GenerationRequest to the LLM worker; the LLM remains Listening->Thinking until token stream starts.

Production Entry Seam:
  VoxEvent::TranscriptFinal { turn_id, text } delivered to pipeline router (or direct call to on_transcript_final for test isolation).
  Test provides a valid non-empty transcript and mocked Retrieval profile. Test does NOT call llm_tx.send directly — the handler does.

Direction Check: PASS — TranscriptFinal is upstream from LLM; invoking LlmCommand directly would test the sink. If LlmCommand were the entry, a broken transcript handler would still pass.

Production Path:
  transcript.rs:on_transcript_final(turn_id, text, app, state, ctx) // 122
    → drops if state==Idle/Paused or not Thinking (guard 130-144)
    → transliterate_if_hi(text), empty check => Ready + toast if empty
    → set_user_transcript accumulator, emit_ipc_to TranscriptFinal{owner:Assistant}
    → branch: if Modular => spawn_modular_llm_task(app, state, turn_id, text, ctx) // transcript.rs:28 spawns tauri::async_runtime::spawn
         → inside async: clone conversation_manager, llm_provider cache via blocking_lock, cancel token via turn_token(), pending_synthesis_jobs atomic, TurnAccumulator arc
         → services/harness/facade.rs:prepare_turn_context(params): // facade.rs:38
              retrieval (if MemoryScope!=ChitChat embedding+retrieve_turn_profile:42) -> push_user_turn:80 -> ContextHarness::needs_threshold_maintenance()? // 88
                if critical: pick TRANSITION_MESSAGES_EN/HI filler, choose FIFO vs compaction LLM (102), if filler dispatched: tts_tx.send(Generate filler:122) + pending.fetch_add(1) (transition speech immediate), run run_compaction on history_slice (152) with fallback FIFO on error (174)
              build ConversationContext { messages, token_count, kv_synced_index } and GenerationRequest { input: ConversationalInput{messages}, options{temp, max_output_tokens}, output:Text, purpose:Conversation } // 254
         → if transition_speech.is_some() => pending.fetch_add(1) (filler), early cancel check 91
         → llm_tx.send(LlmCommand::Generate{request, turn_id, cancel, accumulator, tts_tx, pending}) // 103-111
       if Realtime => pending.store(1) at transcript.rs:198 (no LLM dispatch)
  → services/llm/actor.rs:spawn_llm_worker loop: recv Generate => create stream_tx/rx sync_channel, provider.generate(request, turn_id, cancel, &stream_tx) spawned on current_thread runtime
    → drain Token events via accumulator.lock().push_token(&token) => TtsClauseChunker -> pending.fetch_add(1), tts_tx.send(Generate clause), emit LlmToken direct
    → Finished => event_tx.send(VoxEvent::LlmFinished{turn_id})
  → pipeline/assistant/llm.rs:on_llm_finished checks Thinking/Speaking else drop; for Modular flush chunker remainder, flush_pre_roll, take_assistant_response+user_transcript, persist via ConversationManager::push_assistant_turn + persist_tx.try_send(TurnCompleted)

Observable Exit:
  1. LlmCommand received on injected llm_rx (or VoxEvent::LlmFinished via event_tx when using real provider)
  2. When using real embedded LLM: VoxEvent::LlmFinished + accumulator.assistant_response non-empty + LlmToken stream
  3. Context harness filler path: pending_synthesis_jobs increment for filler + filler clause tts_tx entry when threshold exceeded

Production functions called:
  setup: get_test_app_and_state, create_mock_llm_provider or warm_up_llm with Qwen, ConversationManager seeded with base_system_prompt + identity facts, AppState.llm_provider cache Some, PlaybackEngine mock, tts_tx channel for filler observation, pipeline state set to Listening then Thinking via transition()
  entry: router send VoxEvent::TranscriptFinal or direct on_transcript_final(turn_id, "hello world".to_string(), app, state, &ctx) with RoutingContext{Modular, PTT or Passive, Assistant}
  observe: llm_rx (MockProvider capture of GenerationRequest fields), event_rx for LlmFinished, accumulator.assistant_response, pending counter, tts_rx for filler
  teardown: cancel tokens, cool_down_llm, join handles

Functions written in test file: MockLlmProvider that captures request and returns synthetic token stream (or delegates to real provider when not mock).
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (no VoxEvent::TranscriptFinal delivered / assistant/transcript.rs:on_transcript_final never invoked)** | **Yes — llm_rx empty after 3s, mandatory create-test:2b row** |
| on_transcript_final drops valid transcript (Idle/Paused guard inverted at `assistant/transcript.rs:on_transcript_final` state checks, or `!=Thinking` drop misfires) | Yes — no LlmCommand dispatched, llm_rx empty after 3s, assert fails |
| prepare_turn_context never pushes user turn (push_user_turn deleted in harness) | Yes — GenerationRequest input.messages missing last user turn, mock asserts messages.len() fails |
| needs_threshold_maintenance inverted (always false) where buffer >0.85 | Yes — filler tts_tx empty when test seeded >0.85, filler assertion fails |
| GenerationRequest built with wrong purpose/options (max_output_tokens not propagated) | Yes — mock capture asserts options.max_output_tokens == expected |
| llm_tx.send skipped (early return before Generate send in `spawn_modular_llm_task`) | Yes — llm_rx stays empty, channel empty assertion fails |
| Modular dispatch deleted (`spawn_modular_llm_task` call removed at `assistant/transcript.rs` Modular branch) | Yes — valid transcript in Modular yields no LlmCommand where expected one (Realtime path unaffected — branch-specific kill) |
| Realtime pending arming deleted (`pending_synthesis_jobs.store(1)` removed at `assistant/transcript.rs` Realtime branch) | Yes — realtime transcript leaves pending 0 where test expects 1, downstream playback gate never arms |
| prepare_turn_context fallback on compaction error not executed (FIFO fallback deleted) | Yes — seeded error case would panic instead of FIFO, no GenerationRequest dispatched |
| retrieval ChitChat not pruned (MemoryScope::ChitChat still triggers vector search at facade.rs:47) | Yes — ChitChat query would incorrectly inject <user_profile> where test expects empty |

### Test Functions

```rust
#[tokio::test]
async fn test_transcript_dispatches_generation_request() // tokio::time::timeout(20s) — MockLlmProvider, send valid transcript, assert llm_rx receives request with last message == transcript, purpose==Conversation

#[tokio::test]
async fn test_transcript_empty_guards_to_ready() // timeout 15s Mock — transcript="   " -> state==Ready, llm_rx empty 500ms — NEGATIVE

#[tokio::test]
async fn test_transcript_critical_triggers_filler_and_pending() // timeout 20s — seed MessageBuffer 100 turns force accountant >0.85, assert tts_rx receives filler (TRANSITION_MESSAGES) + pending==1

#[tokio::test]
async fn test_transcript_compaction_fallback_to_fifo() // timeout 15s — mock run_compaction error path, assert fallback performs FIFO and still dispatches GenerationRequest with reduced messages

#[tokio::test]
async fn test_transcript_chitchat_yields_no_retrieval() // timeout 10s — query "hello" classified ChitChat, assert retrieved_profile empty, no vector search, still dispatches LLM

// Real LLM variant #[ignore] (Qwen): Transcript -> LlmFinished + accumulator non-empty, asserts LlmToken stream at least 1 token, no cargo nextest default
```

---

## Seam 6 — LLM Token Streaming → Clause Chunking → TTS Dispatch (LLM→TTS)

**File:** `tests/llm_to_tts_test.rs`
**Handlers:** `services/llm/actor.rs:Token loop` + `pipeline/assistant/accumulator.rs:TurnAccumulator::push_token` + `services/tts/actor.rs:TtsClauseChunker`
**Doc:** `docs/plans/phase11/llm_to_playback_flow.md` steps 2-3 authoritative
**Status:** P2 — Mock TTS provider (no audio) or real Supertonic/Kokoro; no Playback needed

### Phase 1 — Production Path Trace

```
SUT: Streaming LLM tokens are deterministically chunked into speakable clauses and each clause is dispatched as a TTS synthesis job with pending accounting.

Production Entry Seam:
  LlmCommand::Generate { request, turn_id, cancel: CancellationToken, accumulator, tts_tx, pending_synthesis_jobs } sent to llm_tx.
  Upstream is transcript handler; test does NOT directly call TtsCommand::Generate nor examine Playback — those are downstream.

Direction Check: PASS — if test called TtsCommand directly it would test sink; thin wire is LLM token -> chunker -> TTS dispatch.

Production Path:
  llm_tx.send(Generate) -> spawn_llm_worker loop (llm/actor.rs:108)
    → gen via Embedded (Qwen GGUF via llama.cpp) or Remote (OpenAICompat SSE) produce LlmStreamEvent::Token(token_bytes) per sampled token (StreamingEmitter with partial_tag_len holdback)
    → for each Token: let clauses = accumulator.lock().push_token(&token) // accumulator.rs:35 -> TtsClauseChunker::push_str -> extract_chunks loop find_split_point:289
         for clause in clauses { pending.fetch_add(1); tts_tx.send(Generate{turn_id, text: clause}) } // pending per clause, llm/actor.rs:130-134
         emit_ipc_to LlmToken{turn_id, token} (bypass router)
    → on Finished break; runtime.block_on(provider.generate) Ok -> event_tx.send(VoxEvent::LlmFinished{turn_id}) // llm/actor.rs:172
  → TTS side (observed via tts_rx channel, no synthesis needed for this seam): tts_rx.recv() yields Generate{turn_id, text: clause}
  → After LlmFinished, pipeline/assistant/llm.rs:42 flush_modular_tts_remainder handles unpunctuated tail: accumulator.flush_chunker() -> Option<remainder> -> pending.fetch_add(1) -> tts_tx.send Generate remainder // llm.rs:47-66

Observable Exit:
  1. tts_rx receives N Generate messages with correct clause texts (deterministic splits per TtsClauseChunker spec) — count equals number of punctuation boundaries + emergency caps in token stream
  2. pending_synthesis_jobs final count == number of dispatched clauses (including remainder if flushed)
  3. LlmToken stream emitted (via event fast-path) mirrors input tokens
  4. On LlmFinished, remainder tail (e.g. "hello world" without punctuation) appears as final TTS job via flush

Production functions called:
  setup: create_mock_tts_channel (mpsc), MockLlmProvider emitting predetermined token array ["Hello", " world.", " How are", " you?", " This is", " a longer, sentence with comma", " and tail without punct"], TurnAccumulator::new(), pending AtomicU32(0), event_tx for LlmFinished, tts_tx Some
  entry: llm_tx.send(Generate{request: Box::new(ConversationInput single turn "hello"), turn_id=1, cancel fresh, accumulator fresh, tts_tx Some, pending fresh})
  observe: tts_rx drained with timeout 5s, pending counter, accumulator.buffer remaining, LlmFinished via event_rx, LlmToken count
  teardown: cool_down_llm, join, Instant::now()+15s hard deadline

Functions written in test file: MockLlmProvider (emits synthetic tokens with controlled timing) and mock TTS capture (channel count).
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (LlmCommand::Generate never sent to llm_tx / Token stream never emitted)** | **Yes — tts_rx empty after 5s, mandatory row** |
| Token->clause chunking bypassed: push_token returns vec![] always at accumulator.rs:35 | Yes — no TtsCommand dispatched, tts_rx empty, pending 0 |
| find_split_point comma gate word_count>=5 removed (always splits on comma at tts/actor.rs:320) | Yes — "Hello, world" would split into two clauses where test with 2-word prefix expects 1, clause count assertion fails |
| find_split_point period abbreviation guard deleted (is_abbreviation not checked at tts/actor.rs:350) | Yes — "Hello Dr. Smith is here. Next" would split at "Dr." incorrectly; chunk assertion fails |
| emergency 25-word cap deleted at tts/actor.rs:294 | Yes — 30-word unpunctuated input would produce 0 clauses, expected 1 emergency chunk fails |
| flush() deleted on LlmFinished (assistant/llm.rs:47 remainder not sent) | Yes — unpunctuated tail "hello world" never dispatched, tts_rx count off by one, pending mismatch |
| pending fetch_add for clauses deleted at llm/actor.rs:131 | Yes — pending remains 0 while tts_rx has 2 entries, pending==count assertion fails |
| pending fetch_add for remainder deleted at assistant/llm.rs:53 | Yes — tail pending off by one |

### Test Functions

```rust
#[test]
fn test_llm_to_tts_clause_dispatches_per_punctuation() // Instant+15s — 4 synthetic tokens "Hello world. How are you?" -> expect 2 clauses split at "." -> 2 TTS jobs, pending==2, no playback needed

#[test]
fn test_llm_to_tts_flushes_tail_remainder() // Instant+10s — single token "hello world without punct" -> 0 chunks during stream, 1 tail via flush on LlmFinished, pending==1

#[test]
fn test_llm_to_tts_comma_gate_and_abbreviation() // Instant+10s — "Hello, world" (2w pre) -> 0 splits; "This is a longer sentence, and continues" (5w pre) -> 1 split; "Dr. Smith" not split

#[test]
fn test_llm_to_tts_emergency_cap() // Instant+10s — 30-word unpunctuated join(" ") -> 1 emergency chunk of 20 words

// Real LLM variant #[ignore] 20s: real Qwen small prompt -> at least 1 clause dispatched, pending accurate
```

---

## Seam 7 — TTS Synthesis → Playback Ingest & Pre-roll Gates (TTS→Playback) — Spot-check only; canonical gate defer/cancel tests live in Seam 9

**File:** `tests/tts_to_playback_test.rs`
**Handlers:** `services/tts/actor.rs:spawn_tts_worker` + `services/tts/providers::{supertonic,kokoro,chatterbox,edge}*::synthesize_chunk` + `services/audio/playback.rs:ingest_chunk/flush_pre_roll` + `pipeline/assistant/playback.rs`
**Doc:** `docs/plans/phase11/llm_to_playback_flow.md` steps 4-5
**Status:** P2 — Supertonic/Kokoro local (mock) + mock PlaybackEngine; no LLM

### Phase 1 — Production Path Trace

```
SUT: Dispatched TTS synthesis jobs produce PCM audio that prefills the lock-free playback ring and arms the Thinking→Speaking gate via pre-roll thresholds.

Production Entry Seam:
  TtsCommand::Generate { turn_id, text } sent to tts_tx (as produced by Seam 6).
  Upstream is clause chunker; test does NOT call PlaybackEngine::ingest_chunk directly — synthesis does.

Direction Check: PASS — calling ingest_chunk would test playback only; thin wire is TTS dispatch -> synthesis -> playback ingest.

Production Path:
  tts_tx.send(Generate{turn_id, text: clause text}) -> spawn_tts_worker loop (tts/actor.rs:30)
    → provider.synthesize_chunk(text, turn_id, cancel_flag, &playback, event_tx, telemetry_rtf) // tts/providers/trait:41
       Progressive (Supertonic generate_with_config callback:242, Kokoro:142): callback invoked per diffusion step → playback.ingest_chunk(&f32@24k) progressively
       Batch (Chatterbox:172, EdgeTTS:316 chunks 2048): buffer full then for chunk in output.chunks(2048) ingest_chunk
       All providers respect cancel_flag load at playback.rs:106 early return
    → after synthesis returns: if pending Some{jobs.fetch_sub(1); if remaining<=1 { playback.flush_pre_roll() } } // tts/actor.rs:50
  → PlaybackEngine::ingest_chunk_with_threshold(chunk_24k, MODULAR=12000 or REALTIME=3840) // playback.rs:105
     → cancel_flag check, upsample_2x_into(chunk_24k, scratch)-> push_slice(scratch), if !turn_armed && occupied>=threshold { turn_armed=true; event_tx.send(PlaybackStarted{tid}) } // Gate1 playback.rs:123
     → flush_pre_roll(): if !cancel && !turn_armed && occupied>0 { turn_armed=true; send PlaybackStarted } // playback.rs:176
     → CPAL callback process_output_buffer: if pending>0 { underruns++ } else if armed && consumer.is_empty() { turn_armed=false; send PlaybackFinished } // aggregated to assistant/playback.rs:48 guard
  → pipeline/assistant/playback.rs:on_playback_started (playback.rs:8) transition Thinking->Speaking if state==Thinking else drop
  → pipeline/assistant/playback.rs:on_playback_finished (playback.rs:32) if state!=Speaking drop, if pending>0 defer, else transition Ready and persist

Observable Exit:
  1. After synthesis of first clause meeting threshold, PlaybackStarted dispatched (event_rx) and state transitions Thinking->Speaking
  2. After synthesis of all clauses + flush + ring drained + pending==0, PlaybackFinished dispatched and state Speaking->Ready
  3. Short utterance < threshold (e.g. "Hi." 180ms -> 8640 samples <12000) without flush would deadlock — flush_pre_roll ensures PlaybackStarted even for short
  4. Overflow: push_slice returns pushed < len -> warn dropped (playback.rs:115) detectable via log capture

Production functions called:
  setup: create_mock_playback_engine (HeapRb 1_440_000, event_tx channel), TtsWarmUpHandles with pending atomic, warm_up_tts with Supertonic mock or real Supertonic dir, create MockTtsProvider that does ingest_chunk with synthetic 0.1 sine at 24k (or delegates to real provider), pending AtomicU32 pre-seeded, state PipelineAtomics set to Thinking, PlaybackEngineHandles with turn_armed/cancel_flag
  entry: tts_tx.send(Generate{turn_id=1, text="Hello world."}) + tts_tx.send(Generate{turn_id=1, text="How are you?"}) or single filler
  observe: event_rx (PlaybackStarted/Finished), playback.buffer_len(), pending counter, consumer HeapCons try_pop, state.pipeline.state(), underruns via playback telemetry
  teardown: cool_down_tts, playback.cancel, join handles, Instant::now()+30s hard deadline
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (TtsCommand::Generate never sent to tts_tx)** | **Yes — no synthesis, no ingest, PlaybackStarted timeout — mandatory** |
| TTS synthesis deleted (provider.synthesize_chunk never calls ingest_chunk) | Yes — ring stays 0, no PlaybackStarted |
| ingest_chunk cancel_flag check removed (deleted at playback.rs:106) | Negative: when cancel flag set during barge-in, audio would still push and arm, PlaybackStarted would fire when it should be suppressed — assert cancelled run has absent fails |
| turn_armed gate deleted (always PlaybackStarted regardless of threshold at playback.rs:123) | Yes — short utterance <threshold would still arm, negative buffer_len 100 < threshold asserts PlaybackStarted absent before flush would now be present and fail negative |
| flush_pre_roll deleted (deadlock guard at playback.rs:176 removed) | Yes — short utterance 8000 < 12000 never arms, PlaybackStarted absent, timeout |
| Progressive provider callback not progressive (buffers full utterance then ingest once) | Yes — latency probe measuring time to first PlaybackStarted would exceed 500ms threshold where progressive should be <300ms; not strict fail but RTF metric regression detectable |
| pending fetch_sub + flush pairing deleted at tts/actor.rs:50 | Yes — last clause never triggers flush_pre_roll, short tail deadlock, pending remains 1 while PlaybackFinished deferred |
| pending guard at assistant/playback.rs:48 deleted (always Ready even if pending>0) | Yes — set pending=1 then drain ring => PlaybackFinished would incorrectly fire while synthesis still in-flight; test asserts deferred fails |

### Test Functions

```rust
// NOTE: `pending>0 deferred` and `cancel suppress` gates are canonically asserted in Seam 9 `playback_interrupt_test.rs`; the two tests below are light spot-checks for ingest wiring to avoid duplicate wire per testing-style-guide.md:3 — not the canonical gate proof
#[test]
fn test_tts_to_playback_arms_and_completes() // Instant+30s — 2 clauses "Hello world." + "How are you?" -> PlaybackStarted after first threshold (12000) -> Speaking -> drain pending==0 -> Ready + Finished

#[test]
fn test_tts_to_playback_short_utterance_flush_ensures_start() // Instant+15s NEGATIVE — single chunk 8000 (<12000) assert no Started after 200ms, flush_pre_roll => Started

#[test]
fn test_tts_to_playback_finished_deferred_while_pending() // Instant+10s — pending=1, ingest+drain -> assert no Finished, pending.store(0)+drain -> Finished

#[test]
fn test_tts_to_playback_cancellation_suppresses_ingest() // Instant+10s NEGATIVE — cancel_flag=true before ingest, send Generate "hello" -> ring stays empty, no Started

// Real provider variant #[ignore] 30s: Supertonic real synthesis "One moment please." -> PlaybackStarted within 1s, acoustic RMS >0.01
```

---

## Seam 8 — TTS Transition & Voice Hot-Swap (Critical Missing Feature)

**File:** `tests/tts_transition_test.rs`
**Handlers:** `services/tts/actor.rs:TtsCommand::SetVoice` + `services/tts/providers/{supertonic,kokoro,chatterbox}::set_voice` + `services/harness/facade.rs:transition_speech filler dispatch` + `services/audio/playback.rs:cancel/discard` + `core/settings.rs:voice_index reload`
**Status:** P1 Unblocked — mock provider, no model weights required for SetVoice logic; filler uses mock TTS

### Phase 1 — Production Path Trace

```
SUT: TTS voice and transition filler can hot-swap mid-session without dropping pending synthesis accounting or deadlocking playback gates.

This seam covers two tightly coupled critical paths that were previously untested:

Path A — Voice hot-swap (SetVoice without engine restart):
  Production Entry Seam: TtsCommand::SetVoice(voice_idx)
  Upstream is settings mutation (ipc/settings/mutation.rs dispatches SetVoice on voice_index change) or direct voice switch.
  Test does NOT call provider.set_voice directly — it sends TtsCommand via worker channel.

  Production Path:
    settings.tts.voice_index mutated (e.g. 0 -> 2) -> dispatch TtsCommand::SetVoice(2) to tts_tx // tts/actor.rs:65
      → spawn_tts_worker loop: TtsCommand::SetVoice(voice) => provider.set_voice(voice) // tts/actor.rs:67, providers/supertonic.rs:174 Kokoro:90 clamping 0..9
        → provider internally stores AtomicU32 voice_idx or re-resolves voice embedding (Chatterbox voice_id -> wav)
        → next Generate clause uses new voice without worker restart or playback ring clear
    // Critical: SetVoice must not clear pending jobs, turn_armed, or playback queue; it must be processed serially in the worker loop after any in-flight Generate completes

  Observable Exit:
    1. After Generate clause A with voice0, send SetVoice(2), send Generate clause B -> both syntheses complete, tts_rx shows clause B audio with new voice characteristic (mock asserts provider.voice.load()==2) and no dropped clause
    2. Pending accounting remains symmetric: 2 fetch_add (2 clauses) -> 2 fetch_sub -> 0, flush still works, no deadlock

Path B — Transition filler speech (compaction latency hiding):
  Production Entry Seam: prepare_turn_context detecting critical threshold at harness/facade.rs:88 triggers filler before compaction.

  Production Path:
    facade.rs:prepare_turn_context // 38
      → ContextHarness::needs_threshold_maintenance() true at >=0.85 // accountant.rs:89
      → pick TRANSITION_MESSAGES_EN/HI[ts%len] // facade.rs:99
      → if tts_tx Some: tts_tx.send(Generate{turn_id, text:filler}) // facade.rs:122 synchronously BEFORE run_compaction 152
      → caller transcript.rs:99 pending.fetch_add(1) for filler (transcript.rs:99) — filler contributes one pending job
      → run_compaction on history_slice (152) -> apply_compaction_result rebuilds buffer with 2 items (sys+last_user) -> diff_to_enqueue
      → build GenerationRequest -> return (request, Some(filler))
    // If compaction fails: fallback FIFO (facade.rs:174) still retains filler already dispatched — no duplicate
    // Playback: filler audio fills ring first, arms Speaking, then real LLM clauses follow without gap (pending 1 filler + N clauses)

  | Defect path: if filler not sent but pending still fetch_add => pending 1 never decremented -> PlaybackFinished never fires (deadlock)
  | Defect path: if filler sent twice (both facade and transcript pending) => pending 2 but only 1 fetch_sub -> leak

Observable Exit (filler):
  1. tts_rx receives filler clause BEFORE second Generate (LLM clause) — ordering assert filler first
  2. pending counter after facade dispatch ==1 (filler) then after LLM clause dispatches ==2
  3. PlaybackStarted may be triggered by filler alone (if filler audio >= threshold) — assert Started fires even before LLM tokens arrive
  4. On compaction error fallback, GenerationRequest still dispatched with FIFO-reduced messages and filler still exactly once

Production functions called:
  setup A: MockTtsProvider with AtomicU32 voice_idx 0, spawn_tts_worker with mock playback, tts_tx channel, pending AtomicU32, cancel_flag false
  entry A: tts_tx.send(Generate{turn_id=1, text="hello."}) -> sleep 50ms -> tts_tx.send(SetVoice(2)) -> tts_tx.send(Generate{turn_id=1, text="world."}) -> drain tts completions
  observe A: provider.voice==2 after SetVoice, tts_rx count 2, pending 0, playback.buffer_len >0 for both

  setup B: ConversationManager seeded with 100-turn mock history to exceed 0.85 (sync accountant 0.9), MockLlmProvider for compaction, mock tts channel, db None or in-memory, context_window 4096, provider_kind OpenAiCompat (non-fifo path)
  entry B: facade.prepare_turn_context(PrepareTurnParams{harness, tts_tx:Some, memory_tx, conn:None, query:"hello", turn_id:1, session_id:"s", memory: enabled, context_window, provider_kind, llm_provider:Some(mock), llm_settings}) -> await
  observe B: returned filler Some string from TRANSITION_MESSAGES set, tts_rx filler present, pending after transcript caller fetch_add ==1, second call with non-critical context yields None filler and pending not incremented
  teardown: cool_down_tts, cancel, join

Functions written in test file: MockTtsProvider capturing voice_idx and synthesize_chunk calling playback.ingest_chunk with synthetic samples, MockCompactionProvider returning ok or error for fallback path.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (SetVoice never sent / filler never dispatched — tts_tx channel empty)** | **Yes — voice stays 0, filler tts_rx empty, mandatory row** |
| SetVoice deleted (handler ignores SetVoice at tts/actor.rs:65) | Yes — second clause still uses voice 0, provider.voice==2 assertion fails |
| SetVoice clears pending or playback queue (spurious clear at provider) | Yes — pending would incorrectly drop to 0, filler or first clause lost, clause count fails |
| SetVoice not serialised (races with in-flight Generate, voice change applied mid-synthesis) | Yes — mock asserts voice sampled at synthesis start equals expected per clause ordering; interleaved would mismatch |
| filler not sent when critical threshold true (facade.rs:122 deleted) but pending still fetch_add at transcript.rs:99 | Yes — pending 1 leak: tts_rx count 0 but pending 1, final Finished never fires (test asserts tts_rx filler present and pending==1) |
| filler sent but pending fetch_add missing at transcript.rs:99 | Yes — filler dispatched but pending 0, later PlaybackFinished would fire too early (before filler drained) where test asserts pending==1 |
| filler sent twice (both facade and caller duplicate) | Yes — tts_rx would have duplicate filler text, clause count off by one |
| compaction error fallback not executed (FIFO branch at facade.rs:174 deleted) | Yes — error-proc would return Err instead of Ok(request, filler) with FIFO-reduced messages; test asserts Ok with 3 messages in fallback |

### Test Functions

```rust
#[test]
fn test_tts_hot_swap_sets_voice_without_dropping_pending() // Instant+15s — voice 0 -> Generate A -> SetVoice(2) -> Generate B -> assert voice 2, 2 completions, pending 0

#[test]
fn test_tts_transition_filler_dispatched_before_compaction_and_pending_once() // tokio 15s — critical threshold seeded 0.9 -> prepare_turn_context -> assert filler Some + tts_rx filler first + pending after transcript fetch_add ==1

#[test]
fn test_tts_compaction_error_fallback_still_fills_and_requests() // tokio 10s — mock compaction Err -> assert Ok(request) with FIFO messages.len<=3 + filler exactly once, no duplicate

#[test]
fn test_tts_filler_not_sent_when_below_threshold() // Instant+10s — accountant 0.5 -> prepare_turn_context -> filler None + tts_rx empty + pending 0 — NEGATIVE
```

---

## Seam 9 — Playback Lifecycle + VAD Suppression + Barge-in Interrupt

**File:** `tests/playback_interrupt_test.rs`
**Handlers:** `pipeline/assistant/playback.rs:on_playback_started/finished` + `pipeline/assistant/interrupt.rs:on_interrupt` + `pipeline/mod.rs:transition` + `services/vad/actor.rs:should_suppress_audio` + `services/audio/playback.rs:turn_armed/pending/cancel`
**Status:** P1 Unblocked — no models, no network

### Phase 1 — Production Path Trace

```
SUT: Playback engine doors (Thinking->Speaking->Ready) are guarded by pre-roll and pending jobs; speaking arms VAD suppression for speaker output; barge-in cancels canvas and returns to Listening with fresh turn.

Production Entry Seam: Two related entries exercised separately:
  A) Playback gates: playback_engine.ingest_chunk(&f32@24k,threshold) / flush_pre_roll() while in Thinking
  B) Barge-in: on_ptt_start (or direct on_interrupt routing) delivered while state==Thinking or Speaking

Direction Check: PASS — A tests playback producer->router, not direct transition(). B tests interrupt path triggered by user speech/PTT during synthesis, not synthesize itself.

Production Path A — Playback gates:
  transition(Thinking, ctx, app, state) // pre-seeded via state.pipeline.set_state(Thinking)
  → ingest_chunk(&audio) with occupied>=MODULAR_PREROLL_THRESHOLD_SAMPLES (12k) // playback.rs:123
    → if !turn_armed && occupied>=threshold => turn_armed=true, event_tx.send(PlaybackStarted{turn_id})
    → router::on_playback_started => if state==Thinking => transition(Speaking) else drop // assistant/playback.rs:8
  → more ingest_chunk while Speaking
  → CPAL drain: when consumer.is_empty() { if pending>0 { underruns++ } else if armed { turn_armed=false; send(PlaybackFinished) } }
    → router::on_playback_finished => if state!=Speaking => drop; if pending>0 => defer; else transition(Ready), persist_assistant_turn via pipeline/assistant/llm.rs helper // assistant/playback.rs:32

  Short utterance edge (must be tested negative): occupied>0 but < threshold, flush_pre_roll() arms // playback.rs:176

  VAD Suppression sub-gate (sacred hot path):
    VAD loop: should_suppress_audio(&audio_suppressed,&state_atomic,&state=VADState) == true when (audio_suppressed=true) OR (realtime_tx.is_none() && state==Speaking && audio_mode==Speaker) // actor.rs:216
      → continue (skip process_continuous_segmentation/windowed)
    Else process speech as normal. Headset mode must NOT suppress; realtime passthrough bypasses suppression.

Production Path B — Barge-in:
  While state==Thinking or Speaking, new transcript start or ptt_start with higher priority
  → pipeline/assistant/interrupt.rs:on_interrupt(app, state, ctx)
    → PipelineAtomics::next_turn() => (new_turn_id, new CancelToken) rotates turn_token, cancel old token // core/state.rs:235
    → accumulator.lock().clear(), cancel_flag=true, playback_engine.cancel() (sets cancel_flag+discard_request:252)
    → cancels opportunistic compaction if any, returns new_turn_id to caller which caller then uses for new utterance
  Observable: old turn pending synthesis cancelled, playback drained, state moves Listening for new turn, new_turn_id > old_turn_id, old CancellationToken.is_cancelled()==true

Production functions called:
  setup: get_test_app_and_state(), create_mock_playback_engine, setup_vad_actor with audio_suppressed AtomicBool(false) and state_atomic=Speaking/Speaker variant, attach engines, seed state to Thinking
  entry A: playback_engine.ingest_chunk(&vec![0.1; 12000]), playback_engine.flush_pre_roll(), wait drain via consumer.is_empty poll 50ms
  entry B: ptt_start while Speaking or interrupt signal via router channel
  observe: event_rx PlaybackStarted/Finished, state.pipeline.state() transitions, turn_armed flag via buffer_len, pending atomic, audio_suppressed gate via vox_event_rx empty / SpeechStart presence, turn_token cancellation, TurnAccumulator empty after interrupt
  teardown: cancel, shutdown actors, join().expect
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (no ingest_chunk nor flush_pre_roll called / playback engine never fed)** | **Yes — PlaybackStarted never fires, gated test timeout — mandatory row** |
| turn_armed gate deleted (always PlaybackStarted regardless of threshold at playback.rs:123) | Yes — short utterance <threshold would still arm, negative buffer_len 100 < threshold asserts PlaybackStarted absent before flush would now be present and fail negative |
| flush_pre_roll deleted (deadlock guard at playback.rs:176 removed) | Yes — short utterance occupied < threshold never arms, PlaybackStarted absent, test timeout |
| pending_synthesis_jobs guard deleted at assistant/playback.rs:48 (always PlaybackFinished when empty even if pending>0) | Yes — set pending=1 then drain ring => PlaybackFinished would incorrectly fire while synthesis still in-flight; test asserts Finished deferred until pending==0 fails |
| should_suppress_audio returns false unconditionally (gate inversion at actor.rs:216) | Yes — vad_ducking_suppresses_audio_during_playback seeds Speaker+Speaking, streams speech; SpeechStart would fire when it must be suppressed, assert_channel_empty_after fails |
| should_suppress_audio returns true for Headset (audio_mode check removed) | Yes — headset_no_suppression test streams speech in Headset+Speaking; SpeechStart must fire but would be suppressed, assert fails |
| on_interrupt does not call next_turn() (old turn_id reused at interrupt.rs) | Yes — second turn turn_id==first, assert new_turn_id>old fails, old token not cancelled |
| on_interrupt does not clear accumulator (curr assistant_response + chunker buffer leaked) | Yes — after barge-in new token stream contains previous utterance prefix; accumulator empty assert fails |

### Test Functions

```rust
#[test]
fn test_playback_gates_thinking_to_speaking_and_speaking_to_ready() // Instant+15s deadline, ingest >= threshold 12k -> Speaking, drain pending==0 -> Ready, assert PlaybackStarted/Finished via event_rx 200ms poll

#[test]
fn test_short_utterance_requires_flush_to_arm() // Instant+10s NEGATIVE — ingest 8000 (<12000) assert no PlaybackStarted after 200ms, flush_pre_roll => Started

#[test]
fn test_playback_finished_deferred_while_pending() // Instant+10s — pending=1, ingest+drain -> assert no Finished, pending.store(0)+drain -> Finished

#[test]
fn test_vad_ducking_suppresses_during_speaker_playback() // Instant+15s NEGATIVE Sacred — Speaker+Speaking, stream EN clip -> SpeechStart absent 500ms, pipeline_event empty

#[test]
fn test_vad_ducking_resumes_after_playback_and_headset_never_suppresses() // Instant+15s — Speaker Ready -> SpeechStart fires; Headset Speaking -> SpeechStart fires

#[tokio::test]
async fn test_barge_in_cancels_and_advances_turn() // tokio::time::timeout(10s, async { ... }).await.expect — seed Speaking turn 1, ptt_start during Speaking -> state Listening, turn_id==2, old token cancelled, accumulator cleared, buffer_len==0
```

---

## Seam 10 — Clause Chunking Determinism (Seam X) — Fully Defined

**File:** `tests/chunking_determinism_test.rs` (IT) — canonical UT remains `services/tts/actor.rs:242 TtsClauseChunker` (9 UT)
**Handlers:** `services/tts/actor.rs:find_split_point` + `accumulator.rs:push_token` (integration across repeated token arrivals)
**Status:** P1 Unblocked — pure logic, zero models, zero network

### Phase 1 — Production Path Trace

```
SUT: Clause chunking is deterministic across fragmented token arrivals; identical logical text produces identical clause splits regardless of tokenization boundaries (LLM TPS jitter).

This seam was previously unspecced as “TTFA vs prosody pending”. After audit of `llm_to_playback_flow.md:98` and `actor.rs:289`, the algorithm is locked and must be IT-probed, not only UT.

Production Entry Seam:
  Repeated TurnAccumulator::push_token(&token_fragment) calls with fragmented tokens that reconstitute the same logical sentence but split at different byte offsets. Upstream is LLM provider’s streaming decoder; test replays same sentence tokenized as 1-char, 2-char, 5-char windows.

Direction Check: PASS — push_token is upstream from TTS synthesis; calling TtsClauseChunker::find_split_point directly would test leaf, not accumulation across stream.

Production Path:
  accumulator = TurnAccumulator::new()
  tokens_a = ["Hel", "lo world", ". How ", "are you?"]  // 4 fragments
  tokens_b = ["H", "e", "l", "l", "o ", "world. How are ", "you?"] // 7 fragments, same text "Hello world. How are you?"
  for tok in tokens_a { clauses_a.extend(accumulator.push_token(tok)) }
  flush remainder
  reset accumulator.clear()
  for tok in tokens_b { clauses_b.extend(accumulator.push_token(tok)) }
  flush remainder
  // Both must yield identical Vec<String> ["Hello world.", "How are you?"]

  Edge sub-paths exercised: comma gated >=5 words, decimal guard, abbreviation guard, emergency 20-word cap — already UT, but IT verifies stability across fragmentation plus pending accounting not used here (pure chunker).

Observable Exit:
  1. clauses_a == clauses_b byte-for-byte, order preserved
  2. For 30-word unpunctuated input with varied fragment sizes, both fragmentations yield identical single emergency chunk of 20 words + remainder 10 words on flush
  3. Buffer after flush empty (is_empty true)

Production functions called:
  setup: TurnAccumulator::new() (no actors, no audio)
  entry: push_token fragmented sets, flush_chunker
  observe: Vec<String> clauses equality, buffer emptiness, word counts
  teardown: none (pure)

Functions written in test file: None — reuses prod accumulator.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (no push_token calls — accumulator never fed)** | **Yes — clauses empty for both fragmentations where expected 2, mandatory row** |
| find_split_point returns earliest split incorrectly (scans right-to-left instead of left-to-right) | Yes — "First! Second? Third." would yield ["Third."] vs correct ["First!","Second?"], deterministic pair diverges |
| buffer concatenation loses bytes on fragmented utf8 (push_str byte-splits mid-char) | Yes — multi-byte Hindi fragmented at byte level would panic or produce replacement chars, clause texts differ |
| emergency cap counts whitespace incorrectly (counts chars not words) | Yes — 30-word case would emit wrong word count (e.g. 25 not 20) where both fragmentations must agree on 20 |
| clear() not resetting chunker (leaves residual) | Yes — second fragmentation starts with leftover "Hello" prefix, clauses_b has extra prefix, inequality fails |

### Test Functions

```rust
#[test]
fn test_chunking_determinism_across_fragmentations() // Instant+5s — same logical text with two fragmentations -> clause vec equal

#[test]
fn test_chunking_determinism_emergency_cap() // Instant+5s — 30w unpunctuated fragmented two ways -> both yield 20w chunk + 10w remainder

#[test]
fn test_chunking_determinism_comma_gate_stable() // Instant+5s — gated vs ungated comma sentence fragmented -> same splits

// UT complement at services/tts/actor.rs:383 already covers strong/?! newline, abbreviation, decimal per testing-style-guide.md:3 — IT probes determinism not single-case correctness
```

---

## Seam 11 — Session Lifecycle: Idle → Ready → Paused/Sleeping/Error → Idle

**File:** `tests/session_lifecycle_test.rs`
**Handlers:** `pipeline/assistant/session.rs:on_session_start/pause/resume/end` + `pipeline/mod.rs:spawn_idle_monitor/init_new_session_sync` + `core/state.rs:owner/conversation_id` + `services/vad/VadOperationalMode`
**Status:** P2 — Requires mock VoxEngine with vad_tx/pipeline_tx/playback_engine and in-memory DB for identity preload; realtime branch uses MockRealtimeActor

### Phase 1 — Production Path Trace

```
SUT: Session orchestrator lifts common init/term (conv_id, SessionStarted/Ended, ActiveSessionChanged, init_new_session DB preload, state Idle↔Ready↔Paused) and delegates to Modular vs Realtime providers.

Production Entry Seam:
  VoxEvent::SessionStart{owner} / PauseSession / ResumeSession / EndSession dispatched via router (or direct handler call with RoutingContext). Upstream is tray IPC `assistant.start_session` etc. Test does NOT call transition() directly.

Direction Check: PASS — handlers emit events and transition via router; calling transition directly would test sink.

Production Path — Start (Modular branch):
  on_session_start(owner=Assistant, app, state, ctx{Modular,Passive/PTT}) // session.rs:144
    → if state != Idle => return (guard)
    → owner.store, cancel_flag false, conv_id = SystemTime millis -> conversation_id.store, persist_tx.try_send(SessionStarted{id,ts}), memory_tx.try_send(ActiveSessionChanged), init_new_session_sync(state, prompt) -> new_session(base_prompt) + fetch_all_active_identity -> set_identity_facts, spawn_idle_monitor(app, state), start_modular_session -> ensure_modular_workers_sync + vad_tx.send(SetOperationalMode per interaction_mode), accumulator.clear(), transition(Ready) // session.rs:222

Production Path — Start (Realtime branch):
  → start_realtime_session: blocking_lock engine clone vad_tx/pipeline_tx/playback, realtime_engine.take old->stop, Vad StopRealtime, create_realtime_provider(state) -> assembled system prompt with <user_profile>, RealtimeActor::new(provider, tokio_handle).start(PTT/Passive, playback, pipeline_tx, app), get_audio_sender -> vad_tx.send(StartRealtime{tx,is_ptt}) -> *rt_guard=Some(actor), then Ready

Production Path — Pause:
  on_pause(app, state, ctx) // 231 if Idle/Paused drop => cancel_flag true, turn_token cancel, accumulator clear, playback cancel, if Realtime StopRealtime, if dictation enabled store Dictation owner + SetOperationalMode per dictation mode, transition(Paused) // 295

Production Path — Resume:
  on_resume(app, state, ctx) // 300 if not Paused/Error drop => owner Assistant, cancel false, renew_turn_token, Modular SetOperationalMode, Realtime resume_realtime (restart actor or fallback to start_realtime_session), transition(Ready) // 359

Production Path — End:
  on_end(app, state, ctx) // 363 if Idle drop => cancel true, turn_token cancel, accumulator clear, playback cancel, Realtime StopRealtime + rt_guard.take().stop + purge_session_cache, persist SessionEnded, memory SessionEnd, if dictation enabled restore Dictation mode else stop_audio_engine_sync, transition(Idle) // 468

Observable Exit:
  1. After Start: state==Ready, conversation_id non-zero and monotonic second start > first, persist_rx got SessionStarted, memory_rx got ActiveSessionChanged, init_new_session identity facts seeded (conversation_manager lock contains identity), VadOperationalMode set per mode, accumulator empty
  2. After Pause: state==Paused, playback buffer cleared, Vad mode switched to dictation mode if enabled, accumulator cleared, cancel true
  3. After Resume from Paused: state==Ready, turn_token renewed (epoch increment), Vad mode restored to assistant mode
  4. After End: state==Idle, persist SessionEnded, memory SessionEnd, realtime cache purged (file absent), if dictation disabled audio engine stopped (stream None)
  5. Idle monitor not directly asserted here (would require 420s time-travel); instead assert spawn_idle_monitor not panicking and state==Ready remains after short sleep 100ms (no false auto-pause)

Production functions called:
  setup: get_test_app_and_state(), create_mock_playback_engine, in-memory DB with narrative/directives fixtures for identity, Mini mock providers for Modular (ensure_modular_workers mocked) and Realtime MockRealtimeProvider, engine with vad_tx/pipeline_tx/playback, state.settings interaction pipeline_mode + dictation.enabled variations
  entry: on_session_start(Assistant, app, state, &RoutingContext{Modular/Passive, Assistant}) etc. or router send VoxEvent
  observe: state.pipeline.state(), conversation_id, persist_rx (crossbeam try_recv), memory_rx, VadMode via actor state (observe via SetOperationalMode side-effect channel), playback has_active_stream, accumulator empty, file exists check for realtime_session.json after End
  teardown: on_end to Idle, shutdown actors, join

Functions written in test file: MockModular ensure helper and MockRealtimeProvider reuse from Seam 3, in-memory DB setup helper.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (no SessionStart dispatched / on_session_start never called)** | **Yes — state stays Idle, Ready timeout, no SessionStarted — mandatory** |
| on_session_start guard `if state != Idle` inverted/deleted | Yes — second Start in Ready would incorrectly re-initialize, conv_id would change where test expects idempotent drop |
| init_new_session not called (identity facts not loaded) | Yes — conversation_manager identity_facts empty where test seeds DB with 2 identities, assert fails |
| VadOperationalMode not set per mode (SetOperationalMode send deleted in `start_modular_session`) | Yes — PTT config would remain ContinuousSegmentation where expected WindowedValidation |
| on_pause not cancelling token (turn_token().cancel deleted in `on_pause`) | Yes — barge-in after pause would reuse old token where expected cancelled |
| on_pause owner handover deleted (owner.store(Dictation) removed) | Yes — post-pause owner stays Assistant where test expects Dictation; dictation hotkey events route to assistant track and get dropped |
| on_resume Sleeping/Error branches deleted (only Paused accepted) | Yes — resume from Sleeping (idle-monitor offload) or Error stays Sleeping/Error where test expects Ready |
| on_end CPAL gate inverted (always `stop_audio_engine_sync` even when dictation_state==Ready) | Yes — ending assistant session with dictation enabled kills the engine dictation still needs; dictation PTT after end yields no window where expected Listening |
| on_end not purging realtime cache (purge_session_cache deleted) | Yes — file still exists after End where test asserts absent |
| transition(Ready) skipped on start | Yes — state stays Idle where expected Ready |

### Test Functions

```rust
#[tokio::test]
async fn test_session_start_modular_sets_ready_and_identity() // tokio::time::timeout(10s, async { ... }).await.expect("hard timeout") — Idle -> Start Passive Modular -> Ready + SessionStarted + identity loaded + Vad Passive mode

#[tokio::test]
async fn test_session_start_realtime_wires_audio() // tokio::time::timeout(10s, async { ... }).await.expect("hard timeout") — Idle -> Start PTT Realtime -> Ready + RealtimeActor Some + Vad StartRealtime is_ptt true

#[tokio::test]
async fn test_session_pause_resume_transitions() // tokio::time::timeout(10s, async { ... }).await.expect("hard timeout") — Ready -> Pause -> Paused + cancel true + owner Dictation + Vad dictation mode if enabled -> Resume -> Ready + owner Assistant + token renewed

#[tokio::test]
async fn test_session_resume_from_sleeping_and_error() // timeout 10s — Paused -> (idle-monitor equivalent) Sleeping via set_state + emit, then Resume -> Ready; Error -> Resume -> Ready — asserts on_resume accepts all three, not just Paused

#[tokio::test]
async fn test_session_end_dictation_gate_keeps_engine() // timeout 10s — Ready + dictation_state Ready -> End -> Idle + owner Dictation + engine kept (stream Some) + VAD in dictation mode; vs dictation_state Idle -> End -> engine stopped — asserts the CPAL teardown gate both ways

#[tokio::test]
async fn test_session_end_purges_and_idles() // tokio::time::timeout(10s, async { ... }).await.expect("hard timeout") — Paused/Ready -> End -> Idle + SessionEnded + cache purged + if dictation disabled engine stopped
```

---

## Seam 12 — Memory Compaction: 100-Turn History → LLM Extraction → Validated Fact Schema

**File:** `tests/memory_compaction_test.rs`  — `#[ignore]` by default (Nvidia API)
**Handlers:** `services/memory/compaction::{run_compaction, runner, prompt}` + `services/llm/Provider` + `services/harness/prompt_builder`
**Status:** P3 Requires Nvidia API key `temp/.env` — annotated `#[ignore]`; local fallback uses MockLlmProvider for schema validation not quality

### Phase 1 — Production Path Trace

```
SUT: run_compaction() sends multi-turn conversation history to LLM, extracts 6 collection facts + narrative summary, and validates output against schema/quality constraints.

Production Entry Seam:
  run_compaction(provider, history_messages, settings, cancel) // compaction/runner.rs
  Upstream is harness prepare_turn_context compaction job (S5); test calls run_compaction directly which is the extraction trigger for this seam.

Direction Check: PASS — run_compaction is the upstream trigger for compaction extraction; the output consumer (apply_compaction_result) is separate.

Production Path:
  Load 100-turn dataset from tests/assets/compaction_100_turns.json OR synthetic 10-turn fallback for mock
  → build_compaction_request(history_messages, settings) -> ConversationInput with COMPACTION_SYSTEM_PROMPT + history last 40 turns
  → provider.generate(request, turn_id=COMPACTION_SENTINEL_TURN_ID=999_999, cancel, &stream_tx) (Nvidia LLM API or Mock)
  → stream tokens -> parse_compaction_json(response) via prompt.rs JSON repair (strip fences, balance braces)
  → populate CompactionResult { context_summary: String, personal_memory: HashMap<MemoryCollection, Vec<String>>, diff_to_enqueue: HashMap }

Observable Exit:
  1. CompactionResult returned Ok
  2. All 6 core collections present keys checked (Identity, Directives, Narrative, Profile, Entities, Constraints) — but Narrative may be empty if no persona change; assert at least 3 collections non-empty
  3. Narrative summary is non-empty string (>20 chars)
  4. Zero single-word or trivial facts (all fact strings length >=15 chars)
  5. Total facts extracted >=5 across collections
  6. On Mock error path, Err contains parse or provider error, no panic

Production functions called:
  setup: LlmProvider (Nvidia from temp/.env OR Mock returning canned JSON), history_messages Vec<ChatMessage> from fixture, LlmSettings from VoxSettings, CancellationToken fresh
  entry: run_compaction(&provider, &history_messages, Some(&settings), None).await
  observe: CompactionResult fields, fact string invariants, error variant
  teardown: none

Functions written in test file: MockCompactionProvider (returns synthetic compaction JSON or malformed), fixture loader helper.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (run_compaction never invoked / history empty not dispatched)** | **Yes — no result, mock not called, timeout — mandatory** |
| LLM returns malformed JSON or markdown fences that fail parsing at prompt.rs | Yes — run_compaction retry fails, returns Err where test expects Ok (mock malformed case asserts Err) |
| LLM drops required collections (e.g. Identity or Profile missing) | Yes — collection presence assertion fails (expects at least 3 non-empty) |
| LLM extracts trivial / 1-word hallucinated tokens | Yes — minimum fact length >=15 fails |
| Narrative is empty or formatted as array instead of string | Yes — narrative length >20 fails |
| diff_to_enqueue not populated (apply path broken) | Yes — on success diff would be empty where expected >=1 for changed facts |
| CancellationToken not respected (provider ignores cancel) | Negative: pending compaction with cancelled token would still return Ok where expected Err/Cancel |

### Test Functions

```rust
#[ignore] // Nvidia
#[tokio::test]
async fn test_memory_compaction_100_turns_nvidia() // tokio::time::timeout(60s, run_compaction(...).await).expect("hard timeout") — fixture 100 turns, asserts collections, narrative >20, facts >=5, each >=15

#[tokio::test]
async fn test_memory_compaction_mock_schema_validation() // tokio::time::timeout(15s, async { run_compaction(...).await }).await.expect("hard timeout") — mock provider canned JSON -> same asserts without network

#[tokio::test]
async fn test_memory_compaction_malformed_json_returns_err() // tokio::time::timeout(10s, async { run_compaction(...).await }).await.expect("hard timeout") — mock returns "```json {bad" -> assert Err, not panic
```

---

## Seam 13 — Memory Ingestion: staged_pending Queue → 4-Stage Pipeline → Active DB Facts

**File:** `tests/memory_ingestion_test.rs`
**Handlers:** `services/memory/ingestion::{drain_pipeline_queue, runner, stage1_dedup, stage2_embed, stage3_eval, stage4_commit}` + `services/memory/ml::{embedder, nli, edge_classifier, trim_heap}`
**Status:** P2 Unblocked — uses local MiniLM (384-dim), DeBERTa v3, ModernBERT ONNX; zero network

### Phase 1 — Production Path Trace

```
SUT: 4-stage pipeline (Dedup -> Embed -> NLI Eval -> Commit & Prune) processes queued facts into persistent SQLite tables with graph relations.

Production Entry Seam:
  drain_pipeline_queue(conn, &cancel_flag) // ingestion/runner.rs
  Upstream is MemoryWorkerEvent::PersonalFactsReady enqueue (S5 facade) or direct staged_pending rows; test pre-populates staged_pending rows in personal_memory_queue (or inserts via enqueue_personal_facts).

Direction Check: PASS — drain_pipeline_queue processes from queue entry boundary; calling stage4 alone would test sink.

Production Path:
  staged_pending rows in personal_memory_queue (id, fact, collection, session_id)
  → Stage 1 Dedup (stage1_dedup.rs: run_stage1_dedup): Jaccard exact match (threshold 1.0:31) against active facts + queue in-flight -> status deduped or superseded audit (DedupAuditLog)
  → Stage 2 Embed (stage2_embed.rs): MiniLM-L12 generates 384-dim vector (embedder.rs), soft cosine dedup 0.95:36 -> status embedded or deduped stage2_soft_vector
  → Stage 3 Eval (stage3_eval.rs): DeBERTa v3 NLI intra-collection + ModernBERT inter-collection produce relations_json (thresholds 0.85 NLI, 0.80 Edge:47) -> BatchEvaluationResult relations: Vec<RelationEdge>, superseded_by
  → Stage 4 Commit (stage4_commit.rs): INSERT into memory_facts (status active), INSERT vectors into memory_facts_vectors, INSERT relations into memory_relations, DELETE processed rows from queue, update superseded status

Observable Exit:
  1. personal_memory_queue count ==0 (fully drained) via SELECT COUNT(*) WHERE status staged_pending
  2. memory_facts contains active rows matching deduplicated input (SELECT collection,fact,status=active)
  3. memory_facts_vectors populated with 384-dim non-zero embeddings (query vector blob)
  4. memory_relations contains expected structural edges (e.g. SHAPES, DEPENDS_ON, SUPERSEDES) via SELECT relation
  5. Inactive/superseded facts correctly marked status superseded (queries::fetch facts with status)
  6. PipelineStageMetrics recorded per stage (run_id, duration_ms)

Production functions called:
  setup: in-memory Turso conn (VoxDb::open_memory), ensure_embedder_loaded(), ensure_nli_loaded(), ensure_edge_classifier_loaded(), insert staged_pending fixtures via persistence::mutations::enqueue_personal_facts or direct INSERT
  entry: drain_pipeline_queue(&conn, &cancel_flag) or run_pipeline_cycle(&conn, cancel) or individual stage fns for guard tests
  observe: Direct SQL queries on memory_facts, memory_facts_vectors, memory_relations, personal_memory_queue, DedupAuditLog/CandidateAuditLog counts
  teardown: trim_heap, close conn

Functions written in test file: Fixture builder inserting 5 facts with known duplicates/contradictions, helper to assert vector dim.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (no staged_pending rows inserted / drain never called)** | **Yes — queue count 0 before and after where expected >0 then 0, memory_facts empty — mandatory** |
| Stage 1 drops all facts or halts queue (run_stage1_dedup returns 0 claimed) | Yes — queue remains >0, memory_facts empty |
| Stage 2 fails to generate valid 384-dim vectors (embedder error or dim !=384) | Yes — memory_facts_vectors missing rows or dim !=384, cosine dedup not applied |
| Stage 3 edge classifier produces corrupted relations JSON (empty or malformed) | Yes — Stage4 transaction rollback, queue items stranded status ProcessingEval, not 0 |
| Stage 4 fails to delete processed queue items (DELETE missing) | Yes — queue count assertion fails (still >0) |
| Superseded not marked (is_superseded false where NLI contradiction >=0.85) | Yes — old fact remains active where expected superseded |
| Candidate search thresholds inverted (0.60/0.40 swapped at mod.rs:37) | Yes — inter-collection edge missing where expected, relation count fails |

### Test Functions

```rust
#[tokio::test]
async fn test_memory_pipeline_4_stage_drain() // tokio::time::timeout(60s, drain_pipeline_queue(...)).await.expect("hard timeout") — 5 staged_pending -> drain -> queue 0 + 3 active facts + vectors 384 + relations >=1

#[tokio::test]
async fn test_memory_pipeline_stage1_exact_dedup() // tokio::time::timeout(20s, run_stage1_dedup(...)).await.expect("hard timeout") — insert duplicate Jaccard 1.0 -> stage1 deduped, queue deduped status, no embed

#[tokio::test]
async fn test_memory_pipeline_stage2_soft_vector_dedup() // tokio::time::timeout(30s, run_stage2_embed(...)).await.expect("hard timeout") — insert near-duplicate cosine >=0.95 -> stage2 soft dedup, requires embed

#[tokio::test]
async fn test_memory_pipeline_stage3_nli_contradiction_supersedes() // tokio::time::timeout(30s, run_stage3_eval(...)).await.expect("hard timeout") — insert contradicting fact NLI >=0.85 -> stage3 is_superseded true + old marked superseded + SUPERSEDES edge
```

---

## Seam 14 — Memory Retrieval: Query → Scope Classifier → Vector Search → BFS Graph → Context Budget

**File:** `tests/memory_retrieval_test.rs`
**Handlers:** `services/memory/ml/scope_classifier::classify_scope` + `services/memory/retrieval::{route_scope, retrieve_turn_profile, search}` + `services/harness/prompt_builder::format_retrieved_profile`
**Status:** P2 Unblocked — uses local ModernBERT scope + MiniLM embedder; in-memory DB with prefill

### Phase 1 — Production Path Trace

```
SUT: retrieve_turn_profile() classifies query scope, executes scoped SQL/vector retrieval, expands 2-hop BFS graph edges, and formats output within token budget (max_context_share).

Production Entry Seam:
  retrieve_turn_profile(conn, &embedding, scope, settings, context_window)
  Upstream is prepare_turn_context retrieval branch (S5); test calls retrieval directly which is the waterfall trigger.

Direction Check: PASS — retrieve_turn_profile is primary API; calling format_retrieved_profile alone would test sink.

Production Path:
  classify_scope(query) via ModernBERT scope_classifier (query_sieve::MemoryScope { ChitChat, User, Domain, Temporal }) // ml/scope_classifier.rs
  → route_scope(scope) // retrieval/scope.rs:11 maps to ScopeRouting { sql_collections, vector_collections }
    ChitChat -> empty, User->[Profile,Constraints] vector, Domain->[Entities,Directives,Constraints] vector, Temporal->[Directives,Narrative] SQL + [Constraints] vector
  → SQL Branch (search.rs:60 collect_sql_sections): fetch_narrative_history 3, fetch_latest_directives 5 within remaining_budget (max_context_share * context_window)
  → Vector Branch (search.rs:103 collect_vector_graph_sections): fetch_inter_collection_candidates with semantic_similarity_cutoff, similarity >=0.40, parent_quota = remaining_budget / seed.len, fetch_facts_by_ids, BFS 2-hop max_hops (settings.max_hops min 2) via fetch_graph_neighbors, relation edges SUPERSEDES/SHAPES/DEPENDS_ON
  → RetrievedProfile { sql_sections, vector_seeds, graph_children }
  → format_retrieved_profile(profile) -> "[Directives & Narrative]\n- ..." + "[User Context & Knowledge]\n- ... + ↳ --[relation]-->"

Observable Exit:
  1. ChitChat query (e.g. "Hello", "How are you?") → returns empty RetrievedProfile (is_empty true) and formatted "" (zero injection)
  2. Domain query with known entities in DB → returns <user_profile><semantic_graph> block with vector_seeds non-empty and graph_children maybe non-empty
  3. BFS 2-hop connected facts rendered with "  ↳ --[{relation}]--> [{collection}] {fact}" prefix in formatted output
  4. estimate_tokens(result) <= context_window * max_context_share (budget cap)
  5. Identity facts fetched dynamically in SQL branch must NOT appear (violates boot pre-load) — assert absent

Production functions called:
  setup: in-memory DB with facts (Entities: "User works at X", Profile: "User is Alice"), vectors pre-inserted via embedder, ensure_scope_classifier_loaded(), ensure_embedder_loaded(), MemorySettings { max_context_share 0.15, semantic_similarity_cutoff 0.40, max_hops 2 }
  entry: retrieve_turn_profile(&conn, &embedding, scope, &settings, 4096).await
  observe: RetrievedProfile fields lengths, formatted string contains tags, token count via estimate_tokens
  teardown: close DB

Functions written in test file: Helper to pre-populate DB fixtures and to compute embedding for query via generate_embedding, assertion helper for budget.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (query empty or embedding generation skipped, classify_scope not invoked)** | **Yes — empty query returns is_empty true where domain query expects non-empty, mandatory** |
| ChitChat queries erroneously trigger vector search & inject memory (scope classifier fallen to User) | Yes — non-empty string returned where expected "" , assert fails |
| BFS graph expansion traversal broken (0-hop only at search.rs:167) | Yes — child relation arrow "↳" absent from formatted output where expected BFS 2-hop, graph_children empty |
| Token budget arithmetic overflows context window (budget not capped) | Yes — estimate_tokens > context_window * max_context_share assertion fails |
| Identity facts fetched dynamically in SQL branch (violates boot pre-load at scope.rs: route) | Yes — Identity tag found in sql_sections where forbidden |
| Vector similarity cutoff ignored (0.40 not applied) | Yes — low-sim seed (<0.40) would be included where expected filtered, seed count off |
| max_hops not capped at 2 (search.rs:167 loop) | Yes — 3-hop neighbor would appear where test seeds depth 3 chain and asserts max 2 hops |

### Test Functions

```rust
#[tokio::test]
async fn test_retrieval_chitchat_scope_returns_empty() // tokio::time::timeout(15s, retrieve_turn_profile(...).await).expect("hard timeout") — ChitChat "hello" -> is_empty true + formatted ""

#[tokio::test]
async fn test_retrieval_domain_scope_with_bfs_expansion() // tokio::time::timeout(15s, retrieve_turn_profile(...).await).expect("hard timeout") — Domain query with seeded Entities + relations -> vector_seeds non-empty + graph_children with ↳ present

#[tokio::test]
async fn test_retrieval_temporal_scope_narrative_directives() // tokio::time::timeout(15s, retrieve_turn_profile(...).await).expect("hard timeout") — Temporal -> sql_sections Directives/Narrative non-empty

#[tokio::test]
async fn test_retrieval_token_budget_enforcement() // tokio::time::timeout(15s, retrieve_turn_profile(...).await).expect("hard timeout") — Massive candidate set -> formatted tokens <= 4096*0.15 =614
```

---

## Seam 15 — Settings Persistence: Mutation → JSON File Write → Reload Round-trip

**File:** `tests/settings_persistence_test.rs`
**Handlers:** `core/settings.rs:VoxSettings::load/save` + `utils/paths.rs:settings_path` + `core/settings::VoxSettings` (serde JSON)
**Status:** P1 Unblocked — pure file JSON + in-memory serialization; no DB, no models, no network

### Phase 1 — Production Path Trace

```
SUT: VoxSettings load/save round-trips modified application configuration via settings.json with schema validation.

Production Entry Seam:
  VoxSettings::load() / VoxSettings::save(&path) or settings.write().save() via persistence.
  Upstream is IPC settings mutation (history, llm, tts, vad, memory); test mutates struct directly then persists.

Direction Check: PASS — save is persistence entry; loading via JSON parse is output.

Production Path:
  Mutate Voice (persona prompts), LLM (context_window, active provider, temperature), TTS (active provider, voice_index, quality_steps, speed), VAD (threshold, noise_gate, mode), Memory (context_retrieval_enabled) fields in VoxSettings struct
  → serde_json::to_string_pretty(&settings) -> write to settings.json (utils::paths::settings_path) with atomic write (temp file + rename) and parent dir create
  → drop in-memory struct
  → VoxSettings::load() reads file, serde_json::from_str, applies defaults for missing fields (backward compat), returns AppSettings
  → assert reloaded == modified across all mutated fields (eq or field-wise)

Observable Exit:
  1. save returns Ok(())
  2. file exists at settings_path and is valid JSON with expected keys
  3. load() produces VoxSettings identical to modified input across all fields (roundtrip eq)
  4. Corrupt file handling: load on malformed JSON returns Default (fallback) not panic

Production functions called:
  setup: temp_dir() for isolation (or direct settings_path with backup), VoxSettings::default(), utils::paths::init() to ensure models dir but not needed for settings
  entry: settings.save(&path) or fs::write + VoxSettings::load()
  observe: file existence, serde_json::Value key check, struct field equality via == or individual field asserts, malformed load fallback
  teardown: remove temp file, restore original settings_path if overwritten

Functions written in test file: Helper to create temp settings path and to mutate each domain, cleanup guard.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (save never called / write skipped)** | **Yes — file absent where expected present, load returns default not modified — mandatory** |
| save is no-op or silently ignores nested struct fields (e.g. tts.voice_index not serialized) | Yes — reloaded struct matches default for tts.voice_index not modified input (assert field eq fails) |
| Serialization format mismatch corrupts settings on disk (write invalid JSON) | Yes — load returns Err or default fallback where expected modified; file JSON parse fails |
| Load does not apply defaults for missing fields (backward compat broken) | Yes — old file missing new field would panic on missing key instead of default |
| Malformed file not handled (load panics on bad JSON instead of fallback) | Yes — test for corrupt file expects Ok(default) but would panic |

### Test Functions

```rust
#[test]
fn test_settings_json_roundtrip_persistence() // Instant::now()+Duration::from_secs(10) hard deadline — mutate Voice, LLM, TTS, VAD, Memory fields -> save -> load -> assert field-wise eq

#[test]
fn test_settings_malformed_fallback_to_default() // Instant+10s deadline — write "{bad json" -> load -> assert Ok(default) not panic, warning logged
```

---

## Seam 16 — Model Eviction & Zero Idle RAM: Load Singletons → cool_down/unload → State Reset

**File:** `tests/model_eviction_test.rs`
**Handlers:** `services/memory/ml::{ensure_embedder_loaded, ensure_nli_loaded, ensure_edge_classifier_loaded, ensure_scope_classifier_loaded, unload_all_onnx_models}` + `services/llm/actor::{warm_up_llm, cool_down_llm}` + `services/tts/actor::{warm_up_tts, cool_down_tts}` + `core/engine::stop_audio_engine_sync` + `pipeline/assistant/session.rs:paused flow`
**Status:** P2 Unblocked — local model weights required (`~/.vox/models/`); no network

### Phase 1 — Production Path Trace

```
SUT: Model singletons initialize ONNX runtime sessions and unload/cool_down drops sessions, resets RwLocks to None, triggers heap trimming, and leaves no leaked handles.

Production Entry Seam:
  ensure_*_loaded() followed by unload/cool_down sequence.
  Upstream is session Idle/Paused monitor (spawn_idle_monitor 300s cool_down) and explicit Eviction IPC; test drives directly.

Direction Check: PASS — lifecycle management functions are direct SUT for eviction; the alternative (waiting 300s idle) is not testable without time-travel.

Production Path:
  ensure_embedder_loaded() + ensure_nli_loaded() + ensure_edge_classifier_loaded() + ensure_scope_classifier_loaded() (memory/ml)
    -> each loads ONNX Environment + Session via ort, stores in LazyLock RwLock<Option<Arc<Session>>>; is_*_loaded() returns true
  → warm_up_tts + warm_up_llm (if testing TTS/LLM) -> spawn workers, tts_handle/llm_handle Some
  → cool_down_tts(&mut tts_tx) -> sends Shutdown, tts_handle join, tx None
  → cool_down_llm(&mut llm_tx, &llm_provider cache) -> Shutdown, cache cleared
  → unload_all_onnx_models() / unload_memory_pipeline_onnx_models() -> writes None to singletons, drops Session & Environment, trim_heap("MemorySubsystem::unload_all_onnx_models") via ml::trim_heap (madvise)
  → assert is_embedder_loaded()==false, is_nli_loaded()==false, etc., and tts_tx is_none

Observable Exit:
  1. Prior to unload: is_embedder_loaded(), is_nli_loaded(), is_edge_classifier_loaded(), is_scope_classifier_loaded() all == true; warm_up tts/llm handles Some
  2. Post cool_down: tts_tx is_none, llm_tx is_none, handles joined without panic
  3. Post unload: all 4 is_*_loaded() return false, subsequent generate_embedding returns Err (model not loaded) rather than SIGSEGV
  4. No panic during heap trim across OS; double-unload idempotent (second unload returns false/no panic)

Production functions called:
  setup: paths::get() resolution, ensure_* helpers
  entry: ensure_* -> cool_down_tts/llm -> unload_all_onnx_models() or unload_memory_pipeline_onnx_models()
  observe: is_*_loaded() query helpers, tts_tx/llm_tx Option status, handle join, embedder generate should fail after unload
  teardown: none (models stay unloaded until next test setup re-warms — must be last test or sequential per testing-style-guide.md:7.3 single worker lifecycle)

Functions written in test file: None.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (ensure_* never called / no models loaded)** | **Yes — pre-unload is_*_loaded already false where expected true, precondition assert fails — mandatory** |
| unload_all_onnx_models does not reset RwLock singletons to None (leaks Session) | Yes — is_*_loaded() remains true after unload |
| cool_down_tts does not take tx (leaves Some) or doesn't send Shutdown | Yes — tts_tx still Some after cool_down where expected None, worker thread not joined leaks |
| Drop impl panics on active runtime session (Session drop races with Environment) | Yes — test panics on unload where expected clean |
| Double unload panics (not idempotent) | Yes — second unload where test asserts no panic would panic |
| trim_heap not called (memory not reclaimed) | Silent — not directly observable via api, but underrun metric via profiler would show RSS still high; test can assert re-load after unload succeeds (proves drop) |

### Test Functions

```rust
#[test]
fn test_onnx_model_singleton_lifecycle_eviction() // Instant+60s hard deadline — ensure 4 models true -> unload -> assert all false + embed should fail -> re-ensure true

#[tokio::test]
async fn test_llm_tts_cool_down_clears_handles() // tokio::time::timeout(15s, async { warm_up... }).await.expect("hard timeout") — warm_up mock (no real weights) -> cool_down -> tts_tx None + handle joined
```

---

## Seam 17 — Model Manager: Manifest Parsing → Hash Verification → .verified Marker Lifecycle

**File:** `tests/model_manager_test.rs`
**Handlers:** `setup/model_manager.rs::ModelManager` + `setup/manifest.rs:VoxManifest` + `utils/paths::models` + hash verification (SHA256) + `.verified` JSON marker
**Status:** P2 Unblocked — local fixture & hash verification; full remote download #[ignore]

### Phase 1 — Production Path Trace

```
SUT: ModelManager verifies file integrity against SHA256 manifest, generates .verified marker on success, detects corrupted archives, and cleans up files on model removal; handles Zip-Slip/Tar-Slip protection.

Production Entry Seam:
  ModelManager::verify_and_mark(model_dir, expected_sha256) / ModelManager::setup_model(manifest_entry, download=false on fixture) / ModelManager::remove_model(model_id)
  Upstream is model download/install IPC; test uses synthetic model dir.

Direction Check: PASS — Manager methods are entry seam for asset management; calling hash directly would test sink.

Production Path:
  Synthetic model directory provisioned via tempfile with valid test payload file (e.g. "model.bin" 1KB) + matching SHA256 computed via sha256 digest
  → Manifest entry VoxManifest entry with url, sha256, extract flag crafted
  → ModelManager::new(Some(mock_app)) -> resolve models_dir
  → verify_and_mark(): calculates SHA256 via sha2::Sha256 digest of file bytes, compares to manifest sha256, if matches creates .verified JSON marker { sha256, timestamp_ms, model_id } via atomic write
  → Corrupted payload (tampered byte) calculates mismatched SHA256 -> returns VerificationError with explicit hash mismatch, no .verified marker created
  → Zip-Slip guard: do_extract checks enclosed_name() and ParentDir component rejection at setup/model_manager.rs:do_extract (audit SP-19)
  → Removal: ModelManager::remove_model deletes model directory recursive and .verified marker

Observable Exit:
  1. Valid file -> .verified marker file exists with correct sha256 and timestamp (JSON parseable)
  2. Corrupted file -> Verification fails with explicit hash mismatch error; no .verified marker on disk
  3. Archive extraction with ParentDir entry fails with Zip-Slip error, not overwritten
  4. Model deletion -> Directory and marker removed from disk (exists==false)

Production functions called:
  setup: ModelManager::new(None), tempfile::tempdir, create synthetic payload + manifest entry, compute sha256 via utils
  entry: verify_and_mark(&dir, &sha256) or setup_model(&entry) or remove_model(&model_id) or do_extract(&archive_path, &dest)
  observe: Filesystem status (Path::exists), marker JSON content serde parse, Result error type string contains expected hash, Zip-Slip error contains "Path traversal"
  teardown: tempdir drop (auto)

Functions written in test file: Helper to compute sha256 hex, to create temp manifest entry, tempfile guard.
```

### Phase 2b — False-Green Table

| Defect | Would test fail? |
| --- | --- |
| **upstream producer completely silent (ModelManager never instantiated / verify never called)** | **Yes — no marker where expected marker, mandatory** |
| Hash verification skipped and .verified created unconditionally (sha256 compare deleted) | Yes — corrupted payload test would succeed instead of failing (marker exists where expected absent) |
| .verified marker contains wrong metadata schema (missing sha256/timestamp) | Yes — marker JSON parsing assertion fails (serde missing field) |
| Model deletion leaves orphan .verified marker (remove not deleting marker file) | Yes — marker existence check after deletion fails (exists true where expect false) |
| Zip-Slip not handled (enclosed_name check deleted at model_manager.rs:do_extract) | Yes — archive with "../evil" would succeed extraction where expected error, file would appear outside dest |
| SHA256 compared case-sensitive mismatched (hex upper vs lower) | Yes — valid payload would fail verification where expected pass |

### Test Functions

```rust
#[test]
fn test_model_manager_valid_payload_verification() // synthetic valid payload -> .verified marker exists with correct sha256

#[test]
fn test_model_manager_corrupted_payload_detection() // tamper 1 byte -> Verification fails with hash mismatch + no marker

#[test]
fn test_model_manager_zip_slip_rejection() // archive with ../ traversal entry -> do_extract returns Zip-Slip error, no file written

#[test]
fn test_model_manager_removal_cleans_marker() // valid -> verify -> remove -> dir absent + marker absent

// Remote happy path #[ignore]: live download from manifest entry (Nvidia CDN) — run cargo test -- --ignored
```

---

## Execution Priority Matrix (All Seams Defined, None Deferred)

| Priority | Seam | File | Status | Notes |
| -------- | --- | --- | --- | --- |
| **P0** | Shared harness + Mock factories | `tests/common/` | Done | Audio 256, scoring 0.90, mock playback 1.44M ring, mock realtime/tts/llm |
| **P1** | 1 Passive streaming | `passive_streaming_test.rs` | Ready | Ring -> ContinuousSegmentation -> STT; silence guard |
| **P1** | 2 PTT Modular | `ptt_window_modular_test.rs` | Ready | Window trim + ghost gate Modular STT |
| **P1** | 3 PTT Realtime | `ptt_window_realtime_test.rs` | Ready | Window trim + ghost gate Realtime commit (isolated from 2) |
| **P1** | 4 Dictation window+passive+gate | `dictation_window_test.rs` | Ready | VoxEvent PTT/passive → STT → OutputRouter, LLM zero invariant, gate purge, Idle VoiceError |
| **P1** | 9 Playback + Interrupt + Suppression | `playback_interrupt_test.rs` | Ready | Gates + pending defer + should_suppress + next_turn |
| **P2** | 5 Transcript -> LLM | `transcript_to_llm_test.rs` | Ready (mock) | Valid/empty/filler/ChitChat/fallback, #[ignore] real Qwen |
| **P2** | 6 LLM -> TTS | `llm_to_tts_test.rs` | Ready (mock) | Token -> clause determinism, flush tail, gated comma/abbr/cap |
| **P2** | 7 TTS -> Playback | `tts_to_playback_test.rs` | Ready (mock) | Synthesis ingest + thresholds 12k/3840 + flush + cancel |
| **P2** | 8 TTS Transition & Hot-Swap | `tts_transition_test.rs` | Ready (mock) | Filler pending once + SetVoice serialised |
| **P2** | 10 Chunking determinism (X) | `chunking_determinism_test.rs` | Ready | Pure fragment determinism, no models |
| **P2** | 13 Ingestion 4-stage | `memory_ingestion_test.rs` | Ready | Local ONNX 384-dim, Jaccard 1.0, cosine 0.95, NLI 0.85 |
| **P2** | 14 Retrieval scope+BFS | `memory_retrieval_test.rs` | Ready | Scope matrix, BFS 2-hop, budget 0.15 |
| **P2** | 16 Eviction | `model_eviction_test.rs` | Ready | ONNX 4 models + cool_down |
| **P2** | 17 Manager verified | `model_manager_test.rs` | Ready | Hash + marker + ZipSlip |
| **P1** | 11 Session lifecycle | `session_lifecycle_test.rs` | Ready | Idle->Ready->Paused/Sleeping/Error->Idle, owner handover, VAD sync, CPAL dictation gate, cache purge — promoted to P1 (gates all Ready-dependent tests) |
| **P3** | 12 Compaction | `memory_compaction_test.rs` | `#[ignore]` | Nvidia API 100-turn, mock schema variant always runnable |
| **P3** | 15 Settings persistence | `settings_persistence_test.rs` | Ready | JSON roundtrip + malformed fallback |

All IT `cargo nextest run --release --test-threads=1` (sequential, `RAYON_NUM_THREADS=$(nproc)`). UT `cargo test --lib --release` independent. `#[ignore]` not in default loop.

---

## Mutation Testing Scope (All Seams)

> Prerequisite `~/.agents/skills/mutate/SKILL.md` Phase2b table is mutant source. One mutant at a time, assert RED on seam file only, revert+GREEN before next.

### Scope Table

| Seam | Test file | Production file(s) to mutate | Tier 2 `cargo-mutants`? | Reason |
| --- | --- | --- | --- | --- |
| 1 Passive | `passive_streaming_test.rs` | `services/vad/actor.rs` continuous path only (`process_continuous_segmentation` + `should_suppress` + gate-head purge) | No | Warm-up heavy |
| 2 PTT Modular | `ptt_window_modular_test.rs` | `pipeline/assistant/ptt.rs` (Modular branch) | Yes | Pure window/trim/ghost Modular |
| 3 PTT Realtime | `ptt_window_realtime_test.rs` | `pipeline/assistant/ptt.rs` (Realtime branch) + `services/vad/actor.rs:WindowValidation` | Yes | Ghost gate + commit |
| 4 Dictation | `dictation_window_test.rs` | `pipeline/dictation/{ptt,speech,transcript,error}.rs` + `pipeline/router.rs` owner guard + `services/vad/actor.rs` gate purge | Yes | Gates + routing fork + gate purge (hotkey.rs excluded: OS shortcut, not unit-mutable) |
| 5 Transcript | `transcript_to_llm_test.rs` | `pipeline/assistant/transcript.rs` only | Yes | Token gate + filler threshold; facade excluded (heavy) |
| 6 LLM→TTS | `llm_to_tts_test.rs` | `services/tts/actor.rs:TtsClauseChunker` + `pipeline/assistant/accumulator.rs` | Yes | find_split_point + push_token pure |
| 7 TTS→Playback | `tts_to_playback_test.rs` | `services/audio/playback.rs` gates | Yes | turn_armed + flush + cancel pure |
| 8 TTS Transition | `tts_transition_test.rs` | `services/harness/facade.rs:transition_speech` + `services/tts/actor.rs:SetVoice` | No (facade heavy) / Yes for SetVoice | Filler + voice switch pure |
| 9 Playback/Interrupt | `playback_interrupt_test.rs` | `pipeline/assistant/playback.rs` + `pipeline/assistant/interrupt.rs` + `services/vad/actor.rs:should_suppress` | Yes | Transition idempotency + suppress pure |
| 10 Chunking (X) | `chunking_determinism_test.rs` | `services/tts/actor.rs:find_split_point` | Yes | Pure deterministic splits |
| 11 Session | `session_lifecycle_test.rs` | `pipeline/assistant/session.rs` | No | DB + engine heavy |
| 12 Compaction | `memory_compaction_test.rs` | `services/memory/compaction/*` prompt parse | No | Mock only |
| 13 Ingestion | `memory_ingestion_test.rs` | `services/memory/ingestion/stage*.rs` | Yes stage1/2 only | Pure Jaccard/cosine |
| 14 Retrieval | `memory_retrieval_test.rs` | `services/memory/retrieval/scope.rs` + `search.rs` budget/BFS | Yes scope only | Pure routing + budget |
| 15 Settings | `settings_persistence_test.rs` | `core/settings.rs` serde | Yes | Pure JSON |
| 16 Eviction | `model_eviction_test.rs` | `services/memory/ml/mod.rs` unload | No | ONNX singleton heavy |
| 17 Manager | `model_manager_test.rs` | `setup/model_manager.rs:verify_and_mark` | Yes | Hash + ZipSlip pure |

### Tier 1 Mandatory Mutants (first loop — 17 seams, 1 per mandatory row)

- Seam 1: delete upstream push `stream_audio_to_ring_buffer` call + delete `stt_tx.send(Final)` in `handle_speech_end` inside `if realtime_tx.is_none()` — expect `SpeechStart timeout` + `transcript timeout`
- Seam 2: delete `vad_tx.send(StartWindowValidation)` in `assistant/ptt.rs:on_ptt_start` — expect `ghost always` + no ghost fail
- Seam 3: delete ghost guard `if !is_speech||audio.is_empty()` in `assistant/ptt.rs:on_ptt_stop` — expect `silence -> Thinking` + push non-zero where expected 0
- Seam 4: replace `transcript.rs` spawn `route_transcript` with `llm_tx.send(Generate{...})` (routing swap) — expect `llm_rx not empty` kill; plus delete Layer-2 gate purge block in `services/vad/actor.rs` gate head — expect stale-audio survivor in gate test
- Seam 5: delete `llm_tx.send(Generate)` in `spawn_modular_llm_task` (`assistant/transcript.rs`) — expect `llm_rx empty` kill
- Seam 6: replace `push_token` body with `vec![]` in `assistant/accumulator.rs` — expect `tts_rx 0` where expected 2
- Seam 7: delete `playback.ingest_chunk` call inside `synthesize_chunk` mock — expect ring 0 + no PlaybackStarted
- Seam 8: delete filler send in `facade.rs prepare_turn_context` but keep `pending.fetch_add` in `spawn_modular_llm_task` caller — expect pending leak 1 where expected 1 with tts_rx 1
- Seam 8: delete `SetVoice` arm in `spawn_tts_worker` loop (`tts/actor.rs`) — expect provider.voice still 0 after switch
- Seam 9: hardcode `should_suppress_audio=>false` in `services/vad/actor.rs` — expect suppression test survivor (speech fires when must not)
- Seam 10: delete `clear()` body in `assistant/accumulator.rs` — expect second fragmentation carries residual prefix
- Seam 11: delete `transition(Ready)` in `assistant/session.rs:on_session_start` — expect state stays Idle where expected Ready
- Seam 12: return malformed JSON from mock (fences) — expect `run_compaction` Err where expected Ok for good, and Err for bad
- Seam 13: delete stage1 dedup Jaccard check at `stage1_dedup.rs` — expect duplicate not dropped where expected deduped
- Seam 14: change `ChitChat` route to non-empty at `scope.rs:13` — expect `is_empty false` where expected true
- Seam 15: delete `to_string_pretty` field `tts.voice_index` from serialization — expect roundtrip eq fail for that field
- Seam 16: delete `RwLock None` write at `ml/mod.rs:unload` — expect `is_*_loaded` remains true after unload
- Seam 17: delete hash compare at `model_manager.rs:verify` — expect corrupted payload creates marker where expected absent
