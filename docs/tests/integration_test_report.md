# Phase 1 — Integration Test Execution Ledger

This ledger records the initial execution results of all translated integration test seams following the `/create-test` and `/test` protocols.

---

## Seam 1: `tests/passive_streaming_test.rs`
- **SUT:** Autonomous VAD segmentation + Nemotron streaming STT (`services/vad/actor.rs` + `services/stt/actor.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test passive_streaming_test --release --nocapture --test-threads=1`
- **Execution Time:** ~8.36s
- **Evidence Observed:**
  - **Subtest 1 (EN `supertonic_01_en_briefing.wav`):** Received `VoxEvent::SpeechStart` and `VoxEvent::SpeechEnd`. Collected 1 `TranscriptFinal` utterance. Met Levenshtein similarity threshold `>= 0.90` against ground truth.
  - **Subtest 2 (HI `supertonic_07_hi_weather.wav`):** Received `VoxEvent::SpeechStart` and `VoxEvent::SpeechEnd`. Collected 1 `TranscriptFinal` utterance. Confirmed Devanagari script presence. Met similarity threshold `>= 0.90`. Confirmed `transliterate_if_hi` yields non-empty ASCII-only Roman script.
  - **Subtest 3 (Silence Only Guard):** Streamed 100 silence frames. Both `vox_event_rx` and `pipeline_event_rx` remained empty after 500ms / 200ms deterministic waits (zero false speech triggers).
  - **Teardown:** Worker threads joined cleanly with zero panics.

---

## Seam 2: `tests/ptt_window_modular_test.rs`
- **SUT:** Push-To-Talk Window Validation (Modular) (`pipeline/assistant/ptt.rs` + `services/vad/actor.rs` + `services/stt/actor.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test ptt_window_modular_test --release --nocapture --test-threads=1`
- **Execution Time:** ~4.88s
- **Defect Resolved:**
  - Initial run caught production panic at `src/pipeline/assistant/ptt.rs:108:34` (`blocking_lock()` on `tokio::sync::Mutex` inside async runtime).
  - Resolved by Backend Engineer applying `try_lock()` discipline at `assistant/ptt.rs:106–121` matching dictation precedent.
- **Evidence Observed:**
  - **Subtest 1 (Speech Validation):** `ptt_start` transitioned to `Listening`. Streamed `supertonic_01_en_briefing.wav`. `ptt_stop` transitioned to `Thinking`. Emitted `TranscriptFinal` meeting threshold `>= 0.90` against ground truth.
  - **Subtest 2 (Ghost Gate):** Streamed 30 silence frames during PTT hold. `ptt_stop` evaluated non-speech and cleanly reverted state to `Ready`. `pipeline_event_rx` remained empty.
  - **Subtest 3 (PTT Cancel):** `ptt_cancel` reverted state to `Ready`, cancelled `turn_token`, and suppressed STT dispatch.
  - **Teardown:** Actors shut down cleanly and joined with zero panics.

---

## Seam 3: `tests/ptt_window_realtime_test.rs`
- **SUT:** Push-To-Talk Window Validation (Realtime) (`pipeline/assistant/ptt.rs` + `services/vad/actor.rs` + `services/realtime/actor.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test ptt_window_realtime_test --release --nocapture --test-threads=1`
- **Execution Time:** ~0.67s
- **Evidence Observed:**
  - **Subtest 1 (Speech Validation & Commit):** `ptt_start` transitioned to `Listening`. Streamed `supertonic_01_en_briefing.wav`. `ptt_stop` transitioned to `Thinking`, converted f32 audio to signed i16 via `(x.clamp(-1.0, 1.0) * 32767.0) as i16`, and invoked `RealtimeActor::signal_speech_committed`. Asserted `commit_count == 1` and all committed samples valid i16.
  - **Subtest 2 (Ghost Gate):** Streamed 30 silence frames during PTT hold. `ptt_stop` evaluated non-speech, reverted to `Ready`, and committed 0 frames (`commit_count == 0`).
  - **Subtest 3 (PTT Cancel):** `ptt_cancel` reverted state to `Ready`, cancelled `turn_token`, and committed 0 frames.
  - **Teardown:** VAD actor shut down cleanly and joined with zero panics.

---

## Seam 4: `tests/dictation_window_test.rs`
- **SUT:** Dictation PTT + Passive + Ingestion Gate + LLM Zero Invariant (`pipeline/dictation/{mod,ptt,speech,transcript,error}.rs` + `services/vad/actor.rs` + `services/dictation/output_router.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test dictation_window_test --release --nocapture --test-threads=1`
- **Execution Time:** ~8.47s
- **Defects Resolved:**
  1. *Defect 1 (`toast.rs:132` / `toast.rs:77`):* Wrapped `window.gtk_window()` in `with_gtk_window` helper with `catch_unwind` to gracefully handle headless / mock runtimes without panicking.
  2. *Defect 2 (`dictation/error.rs:16-48`):* Fixed auto-recover state logic. Captures `was_idle` prior to `transition_dictation(Error)` so disabled (Idle) dictation restores `Idle` rather than erroneously auto-recovering to `Ready`.
- **Evidence Observed:**
  - **Subtest 1 (PTT Speech Routing & LLM Zero Invariant):** `PttStart` transitioned dictation state to `Listening`. Streamed `supertonic_01_en_briefing.wav`. `PttStop` transitioned to `Thinking` and dispatched audio to Nemotron STT. Emitted `TranscriptFinal` meeting threshold `>= 0.90` (similarity 0.96+). Injected transcript into router: updated `dictation_last_transcript`, returned dictation state to `Ready`, and verified `llm_rx` channel was 100% empty after 500ms deterministic wait.
  - **Subtest 2 (Ghost Gate):** Streamed silence frames during PTT hold. `PttStop` discarded non-speech audio and cleanly reverted state to `Ready`. `pipeline_event_rx` remained empty after 500ms.
  - **Subtest 3 (PttCancel via Router):** `PttCancel` sent via router while `Listening` reverted state to `Ready`. `pipeline_event_rx` remained empty.
  - **Subtest 4 (Passive Speech Routing):** `SpeechStart` transitioned state to `Listening`; `SpeechEnd` transitioned state to `Thinking`. Transcript routing updated `dictation_last_transcript`, returned to `Ready`, and verified `llm_rx` channel was completely empty.
  - **Subtest 5 (Option-C Ingestion Gate Purge):** Closed gate via `Idle` state. Streamed audio while gate was closed. Verified VAD actor purged in-flight buffers. Reopened gate: clean `PttStart`/`PttStop` reverted to `Ready` via ghost gate with zero stale transcript emissions.
  - **Subtest 6 (Disabled / Idle Start):** `PttStart` while in `Idle` invoked `error::on_error` and preserved `Idle` state (preventing rogue activation).
  - **Teardown:** Router, VAD, and STT threads joined cleanly.

---

## Seam 5: `tests/transcript_to_llm_test.rs`
- **SUT:** Transcript Handler → Context Harness → LLM Dispatch (`pipeline/assistant/transcript.rs` + `services/harness/facade.rs` + `services/llm/actor.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test transcript_to_llm_test --release --nocapture --test-threads=1`
- **Execution Time:** ~1.24s
- **Defects Resolved:**
  - *Defect (`assistant/transcript.rs:43`):* Called `state.engine.blocking_lock()` on Tokio-reachable thread. Replaced with non-blocking `try_lock()` and gracefully handled lock contention with default empty channels, matching assistant PTT and dictation precedents.
- **Evidence Observed:**
  - **Subtest 1 (Valid Dispatch Shape):** Valid user transcript received in `Thinking` state dispatched a `LlmCommand::Generate` to the LLM worker with matching `turn_id`, `purpose == GenerationPurpose::Conversation`, options (`max_output_tokens == 512`), and user query as the trailing message in `request.input.messages`. Verified user transcript stored in `TurnAccumulator`.
  - **Subtest 2 (Empty / Whitespace Transcript Guard):** Whitespace transcript reverted pipeline state to `Ready` and emitted zero LLM commands (channel remained empty after 500ms).
  - **Subtest 3 (Non-Thinking Drop):** Valid transcript received while in `Listening` state was dropped without dispatching to LLM.
  - **Subtest 4 (Realtime Pipeline Mode):** In `Realtime` mode, armed `pending_synthesis_jobs` to 1 without dispatching any `LlmCommand` (preserving Realtime provider isolation).
  - **Subtest 5 (Critical Threshold Maintenance & Filler):** Pre-seeded working memory past critical threshold (>85% context utilization). Handled threshold maintenance by emitting transition speech filler to `tts_tx` from `TRANSITION_MESSAGES_EN` and incrementing `pending_synthesis_jobs`.

---

## Seam 6: `tests/llm_to_tts_test.rs`
- **SUT:** Real Local Qwen LLM Inference → Token Streaming → Clause Chunking → TTS Dispatch (`services/llm/actor.rs` + `services/llm/embedded/mod.rs` + `pipeline/assistant/accumulator.rs` + `services/tts/actor.rs` + `pipeline/assistant/llm.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test llm_to_tts_test --release --nocapture --test-threads=1`
- **Execution Time:** ~2.64s
- **Evidence Observed:**
  - **Real Local Inference:** Loaded `qwen-3.5-0.8b-q4_k_m.gguf` via production `EmbeddedProvider::new(path, ctx_size=2048, n_threads=4)`. Successfully evaluated system/user prompts with llama.cpp on the dedicated OS worker thread.
  - **Real Token Streaming:** Streamed real sampled tokens across thread channels into `TurnAccumulator`, accumulating full synthesized sentences in `state.pipeline_accumulator`.
  - **Clause Chunking & Dispatch:** `TtsClauseChunker` actively parsed streaming tokens and emitted real `TtsCommand::Generate` clauses to `tts_rx` matching sentence boundaries.
  - **Tail Remainder Flush:** Invoked `on_llm_finished` on `VoxEvent::LlmFinished`, which cleanly flushed the unpunctuated remainder to TTS.
  - **Pending Accounting Invariant:** Verified `pending_synthesis_jobs` matched the exact count of dispatched clauses ($N = \text{clauses.len()}$).
  - **Teardown:** Worker cleanly consumed `LlmCommand::Shutdown` and joined with zero memory leaks or panics.

---

## Seam 7: `tests/tts_to_playback_test.rs`
- **SUT:** Real Local Supertonic ONNX TTS Synthesis → Playback Ingestion & Gating (`services/tts/actor.rs` + `services/tts/supertonic/mod.rs` + `services/audio/playback.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test tts_to_playback_test --release --nocapture --test-threads=1`
- **Execution Time:** ~1.68s
- **Evidence Observed:**
  - **Subtest 1 (Real Synthesis & Playback Cushion):** Dispatched `TtsCommand::Generate` to real local Supertonic ONNX engine. Received `PlaybackStarted` with matching `turn_id` once 12,000 samples were ingested into ring buffer. Verified non-silent audio generated (RMS > 0.01).
  - **Subtest 2 (Pre-Roll Flush for Short Utterance):** Ingested short clause below 12k samples. Negative assertion proved `PlaybackStarted` did not fire before flush. Invoked `flush_pre_roll()` on `TtsFinished`, immediately arming and firing `PlaybackStarted`.
  - **Subtest 3 (Pending Synthesis Accounting):** Synthesized multi-clause utterance. Emptied ring buffer while `pending_synthesis_jobs > 0`; verified `PlaybackFinished` deferred and state remained `Speaking`. Emitted `PlaybackFinished` only when `pending_synthesis_jobs == 0`.

---

## Seam 8: `tests/tts_transition_test.rs`
- **SUT:** TTS Transition, Voice Hot-Swap & Context Filler Dispatch (`services/tts/actor.rs` + `pipeline/assistant/accumulator.rs` + `services/harness/facade.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test tts_transition_test --release --nocapture --test-threads=1`
- **Execution Time:** ~1.58s
- **Evidence Observed:**
  - **Subtest 1 (Voice Hot-Swap):** Tested `TtsCommand::SetVoice` on running worker without thread termination. Subsequent `Generate` commands synthesized with updated voice model.
  - **Subtest 2 (Context Compaction Filler Dispatch):** Compaction threshold crossed during context preparation. Immediately sent transition filler audio to `tts_tx` with atomic increment of `pending_synthesis_jobs`. Verified pipeline correctly accounts for filler before generation begins.

---

## Seam 9: `tests/playback_interrupt_test.rs`
- **SUT:** Playback Lifecycle + VAD Speaker Ducking Suppression + Barge-in Interruption (`services/audio/playback.rs` + `pipeline/assistant/playback.rs` + `pipeline/assistant/interrupt.rs` + `services/vad/actor.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test playback_interrupt_test --release --nocapture --test-threads=1`
- **Execution Time:** ~2.85s
- **Evidence Observed:**
  - **Subtest 1 (Playback State Gates):** Ingested 12,000 samples in `Thinking` state -> emitted `PlaybackStarted` -> transitioned to `Speaking`. Drained consumer with `pending_jobs == 0` -> emitted `PlaybackFinished` -> transitioned to `Ready`.
  - **Subtest 2 (Short Utterance Pre-Roll Flush):** Ingested 2,000 samples (< 12,000 threshold). Asserted `PlaybackStarted` was NOT emitted before flush. Called `flush_pre_roll()` -> `PlaybackStarted` immediately emitted.
  - **Subtest 3 (Pending Synthesis Deferral):** With `pending_synthesis_jobs == 1`, buffer drain did NOT transition to `Ready` (deferred). Decremented to 0 -> transitioned to `Ready`.
  - **Subtest 4 (Sacred VAD Ducking Suppression):** In `Speaker` audio output mode and `Speaking` state, streaming real speech audio clip (`supertonic_01_en_briefing.wav`) was completely ducked/suppressed by VAD actor (`assert_channel_empty_after` confirmed zero `SpeechStart` emitted).
  - **Subtest 5 (VAD Ducking Resumption & Headset Invariant):** When state returned to `Ready` under `Speaker` mode, speech streaming triggered `SpeechStart`. Under `Headset` mode, speech streaming during `Speaking` was never suppressed.
  - **Subtest 6 (Barge-In Lifecycle):** Invoked `on_interrupt` during active playback: advanced monotonic `turn_id`, cancelled old turn `CancellationToken`, created clean uncancelled new token, reset `pending_synthesis_jobs` to 0, cleared `pipeline_accumulator`, cancelled playback engine, and transitioned state to `Listening`.

---

## Seam 10: `tests/chunking_determinism_test.rs`
- **SUT:** Clause Chunking Determinism & Token Fragmentation Invariance (`services/tts/actor.rs` + `pipeline/assistant/accumulator.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test chunking_determinism_test --release --nocapture --test-threads=1`
- **Execution Time:** ~0.05s
- **Evidence Observed:**
  - **Subtest 1 (Fragmentation Determinism):** Replayed multi-clause complex sentence across fine-grained sub-word tokens (Fragmentation A) vs erratic coarse chunks spanning punctuation boundaries (Fragmentation B). Byte-for-byte reconstructed identical clause sequences in identical order. Verified buffer empty after flush and `acc.clear()` contract verified.
  - **Subtest 2 (Emergency 20-Word Cap):** Fed 30 unpunctuated words across 1-word-per-token vs 3-words-per-token streams. Both fragmentations deterministically emitted exactly 2 chunks: first chunk exactly 20 words from emergency cap, second chunk remaining 10 words from flush.
  - **Subtest 3 (Comma Prosody Gating Stability):** Verified commas preceded by < 5 words do not split before sentence boundary; commas preceded by >= 5 words split immediately. Fragmenting tokens around comma preserved split points deterministically.

---

## Seam 11: `tests/session_lifecycle_test.rs`
- **SUT:** Session Lifecycle (Idle → Ready → Paused/Sleeping/Error → Idle) (`pipeline/assistant/session.rs` + `pipeline/mod.rs` + `core/engine.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test session_lifecycle_test --release --nocapture --test-threads=1`
- **Execution Time:** ~0.59s
- **Defects Resolved:**
  1. *Defect 1 (`pipeline/assistant/session.rs:402`):* Mutex self-deadlock. In `on_end`, `state.persist_tx.lock()` was held across the entire function scope. When dictation was idle, `on_end` called `stop_audio_engine_sync`, which attempted to acquire `state.persist_tx.lock()` again on the same thread, causing a deadlocking freeze. Resolved by scoping the mutex lock guards for `state.persist_tx` and `state.memory_tx` to narrow drop blocks.
  2. *Defect 2 (`pipeline/mod.rs:114`):* Tokio runtime nesting in synchronous session initializers. `block_in_place` panicked under single-threaded test runtimes. Guarded runtime flavor and isolated to thread when runtime is not multi-threaded.
- **Evidence Observed:**
  - **Subtest 1 (`test_session_start_modular_sets_ready_and_identity`):** Starting session from Idle transitioned `InteractionState` to `Ready`, initialized conversation manager with base system prompt, and proved idempotent on subsequent start (guard no-op).
  - **Subtest 2 (`test_session_pause_resume_transitions`):** Transitioned `Ready` -> `Paused`, unpaused back to `Ready`; tested pause under `Listening`/`Speaking` cancelling active tokens and returning cleanly to `Paused`.
  - **Subtest 3 (`test_session_resume_from_sleeping_and_error`):** Successfully unpaused/resumed from `Sleeping` and `Error` states to `Ready`.
  - **Subtest 4 (`test_session_end_purges_and_idles`):** Cleanly shut down from `Ready`, cleared turn counters/accumulators, stopped engine when dictation was idle, and transitioned state to `Idle`.
  - **Subtest 5 (`test_session_end_dictation_gate_keeps_engine`):** When dictation state was `Ready`, assistant `on_end` transitioned assistant state to `Idle` while keeping CPAL audio engine alive and active.

---

## Seam 15: `tests/settings_persistence_test.rs`
- **SUT:** Settings Persistence & Mutation Round-Trip (`core/settings.rs` + `ipc/settings/mutation.rs` + `utils/paths.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test settings_persistence_test --release --nocapture --test-threads=1`
- **Execution Time:** ~0.05s
- **Evidence Observed:**
  - **Subtest 1 (`test_settings_json_roundtrip_persistence`):** Mutated settings across Appearance, Audio, VAD, STT, LLM, TTS, Interaction, Dictation, and Memory using production `apply_setting_mutation`. Persisted to disk via `VoxSettings::save`. Verified physical `settings.json` existence, non-empty size, and inspected raw JSON key/values. Reloaded via `VoxSettings::load` and asserted 100% exact field-wise round-trip preservation.
  - **Subtest 2 (`test_settings_malformed_fallback_to_default`):** Wrote corrupt unclosed JSON into `settings.json`. `VoxSettings::load` cleanly fell back to system defaults without panic, created timestamped backup file `settings.corrupt.<ts>.json` containing the original content, and preserved filesystem integrity.
  - **Subtest 3 (`test_settings_partial_section_recovery`):** Tested schema drift / partial corruption. Loaded JSON with valid Appearance and TTS sections alongside omitted and junk sections. Successfully recovered valid sections while restoring missing domains to system defaults.

---

## Seam 16: `tests/model_eviction_test.rs`
- **SUT:** Model Singleton Eviction & Zero Idle RAM (`services/memory/ml` + `services/tts/actor.rs` + `services/translit.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test model_eviction_test --release --nocapture --test-threads=1`
- **Execution Time:** ~4.93s
- **Evidence Observed:**
  - **Subtest 1 (`test_onnx_model_singleton_lifecycle_eviction`):** Lazily initialized 4 memory ONNX models (MiniLM sentence embedder, DeBERTa v3 NLI, ModernBERT edge classifier, ModernBERT memory scope classifier) and Seq2Seq Hindi transliteration engine. Verified active inference (`generate_embedding` yielded 384-dim vector). Performed partial pipeline eviction (`unload_memory_pipeline_onnx_models`) and verified safe fallback (`generate_embedding` returned `Ok(None)` without crash/SIGSEGV). Performed full eviction (`unload_all_onnx_models`), verified all `is_*_loaded()` returned false, confirmed double-eviction idempotency, and proved clean re-loading into memory.
  - **Subtest 2 (`test_tts_worker_cool_down_clears_handles_and_joins`):** Initialized real Supertonic ONNX TTS worker via `warm_up_tts`. Invoked `cool_down_tts(&mut tts_tx)`: confirmed `tts_tx` was immediately taken (`is_none() == true`), `TtsCommand::Shutdown` was processed, and the dedicated worker OS thread joined cleanly with zero panics.

---

## Seam 17: `tests/model_manager_test.rs`
- **SUT:** Model Manager Lifecycle & Safety (`setup/model_manager.rs` + `setup/manifest.rs` + `setup/manager_ops.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test model_manager_test --release --nocapture --test-threads=1`
- **Execution Time:** ~0.046s
- **Evidence Observed:**
  - **Subtest 1 (`test_model_manager_valid_payload_verification`):** Verified synthetic model payload integrity. Proved `.verified` marker file was created on disk with exact matching SHA256, expected file size, model ID, and non-zero timestamp. Subsequent check verified instant cache hit via `.verified` marker.
  - **Subtest 2 (`test_model_manager_corrupted_payload_detection`):** Verified size mismatch (truncated payload) rejects presence and suppresses `.verified` marker creation. Verified tampered marker with mismatched SHA256 falls through to size matching and refreshes the marker with canonical manifest hash.
  - **Subtest 3 (`test_model_manager_zip_slip_and_tar_slip_rejection`):** Synthesized malicious Zip and Tar archives with path traversal entries (`../escaped_file.txt`). Verified `ModelManager::do_extract` detects Zip-Slip and Tar-Slip vulnerabilities, rejects extraction with explicit security error, and leaves destination parent directory untouched. Confirmed legitimate archive unpacks safely.
  - **Subtest 4 (`test_model_manager_removal_cleans_marker_and_dir`):** Verified model deletion via `delete_model_file` removes model binary, purges `.verified` marker file, cleans up empty parent directory structure, and updates presence check to false.

