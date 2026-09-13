# Phase 2 — Integration Test Mutation Ledger

This ledger records empirical proof that tests go RED when critical production paths break, following the `/mutate` protocol and consuming Phase 2b False-Green tables.

---

## Seam 1: `tests/passive_streaming_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 1.1 (Final STT Dispatch Guard):** Suppressed `SttCommand::Final` dispatch on speech offset (`services/vad/actor.rs:324` inverted `if false && state.utterance_buffer.len() >= VAD_MIN_UTTERANCE_SAMPLES...`).
     - *Result:* 🔴 **KILLED** (`Transcript was empty for EN clip` at `passive_streaming_test.rs:98:9`, `FAIL [15.684s]`).
  2. **Mutant 1.2 (Speech Offset Boundary Gate):** Suppressed `handle_speech_end` trigger (`services/vad/actor.rs:393` inverted `if false && state.in_speech && state.inactive_frames >= state.speech_end_frames...`).
     - *Result:* 🔴 **KILLED** (`Did not receive VoxEvent::SpeechEnd for EN clip` at `passive_streaming_test.rs:90:9`, `FAIL [10.716s]`).
  3. **Mutant 1.3 (Suppression Logic Inversion):** Hardcoded `should_suppress_audio` to return `true` unconditionally (`services/vad/actor.rs:233`).
     - *Result:* 🔴 **KILLED** (`Did not receive VoxEvent::SpeechStart for EN clip` at `passive_streaming_test.rs:89:9`, `FAIL [10.634s]`).

---

## Seam 2: `tests/ptt_window_modular_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 2.1 (Stop Window Channel Drop):** Suppressed `VadCommand::StopWindowValidation` dispatch on PTT release (`pipeline/assistant/ptt.rs:125` replaced `vad_tx.send(StopWindowValidation)` with dropped channel).
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: State must transition to Thinking upon speech validation and STT dispatch, left: Ready, right: Thinking` at `ptt_window_modular_test.rs:103:13`, `FAIL [1.123s]`).
  2. **Mutant 2.2 (Ghost Gate Logic Inversion):** Deleted non-speech / empty audio discard check on PTT release (`pipeline/assistant/ptt.rs:140` inverted `if false && (!is_speech || audio.is_empty())`).
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Ghost gate: state must revert to Ready on silence hold, left: Thinking, right: Ready` at `ptt_window_modular_test.rs:152:13`, `FAIL [4.766s]`).
  3. **Mutant 2.3 (Cancellation Token Discipline):** Suppressed `state.pipeline.turn_token().cancel()` call upon PTT cancellation (`pipeline/assistant/ptt.rs:172`).
     - *Result:* 🔴 **KILLED** (`turn_token must be cancelled on ptt_cancel` at `ptt_window_modular_test.rs:198:13`, `FAIL [5.707s]`).

---

## Seam 3: `tests/ptt_window_realtime_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 2
- **Survivors:** 1 (Known audio normalization fixture gap)
- **Mutation Score:** 2/3 (66.7%)
- **Mutations Realized & Verified:**
  1. **Mutant 3.1 (Clamping Logic Removed):** Removed `.clamp(-1.0, 1.0)` in `pipeline/assistant/ptt.rs:91` (`(x * 32767.0) as i16`).
     - *Result:* ⚠️ **SURVIVED** (Audio clip `supertonic_01_en_briefing.wav` is already cleanly normalized in `[-1.0, 1.0]`, making clamp a no-op on normal audio; documented fixture gap).
  2. **Mutant 3.2 (Speech Commit Suppressed):** Suppressed `rt_actor.signal_speech_committed(&i16_samples)` in `pipeline/assistant/ptt.rs:95`.
     - *Result:* 🔴 **KILLED** (`Deepgram Voice Agent must respond with server event after speech commit` at `ptt_window_realtime_test.rs:175:13`, `FAIL [17.52s]`).
  3. **Mutant 3.3 (Pipeline Mode Branch Swap):** Routed `PipelineMode::Realtime` through `Modular` STT path in `pipeline/assistant/ptt.rs:88` (`if true || ctx.pipeline_mode == PipelineMode::Modular`).
     - *Result:* 🔴 **KILLED** (`Deepgram Voice Agent must respond with server event after speech commit` at `ptt_window_realtime_test.rs:175:13`, `FAIL [17.37s]`).

---

## Seam 4: `tests/dictation_window_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 4.1 (Idle Auto-Recover Logic Inversion):** Neuter `was_idle` check in `dictation/error.rs:45`, unconditionally recovering to `Ready` on error.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: PttStart when Idle must remain Idle, left: Ready, right: Idle` at `dictation_window_test.rs:391:13`, `FAIL [8.242s]`).
  2. **Mutant 4.2 (Ghost Gate Logic Inversion):** Deleted non-speech silence hold check in `dictation/ptt.rs:97` (`if false && (!is_speech || audio.is_empty())`).
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Ghost gate: dictation state must revert to Ready on silence hold, left: Thinking, right: Ready` at `dictation_window_test.rs:213:13`, `FAIL [5.956s]`).
  3. **Mutant 4.3 (Window Validation Dispatch Drop):** Suppressed `vad_tx.send(StartWindowValidation)` dispatch in `dictation/ptt.rs:35`.
     - *Result:* 🔴 **KILLED** (`Transcript must not be empty for validated speech` at `dictation_window_test.rs:137:13`, `FAIL [15.860s]`).

---

## Seam 5: `tests/transcript_to_llm_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 5.1 (Gate Inversion):** Deleted `if trimmed.is_empty()` guard via `if false && trimmed.is_empty()` in `pipeline/assistant/transcript.rs:258`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Empty transcript must transition state to Ready (left: Thinking, right: Ready)` at `tests/transcript_to_llm_test.rs:170:13`, `FAIL [0.245s]`).
  2. **Mutant 5.2 (Silent Drop):** Neutered duplex pipe dispatch by commenting out `tx.send(LlmCommand::Generate { .. })` in `pipeline/assistant/transcript.rs:172`.
     - *Result:* 🔴 **KILLED** (`Real LLM generation must route tokens and emit VoxEvent::LlmFinished within 15s` at `tests/transcript_to_llm_test.rs:148:13`, `FAIL [15.112s]`).
  3. **Mutant 5.3 (Threshold Flip):** Set critical compaction threshold to unreachable 150% (`CRITICAL_COMPACTION_THRESHOLD_PERCENT = 150`) in `services/harness/plugins/budget.rs:9`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Filler must have InterimFiller intent (left: TurnResponse, right: InterimFiller)` at `tests/transcript_to_llm_test.rs:282:21`, `FAIL [0.082s]`).

---

## Seam 6: `tests/llm_to_tts_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 2
- **Survivors:** 1 (Input property: terminal punctuation flushes complete sentences immediately)
- **Mutation Score:** 2/3 (66.7%)
- **Mutations Realized & Verified:**
  1. **Mutant 6.1 (Silent Drop):** Neuter clause dispatch in `services/harness/plugins/stream.rs:146` (`if let Err(e) = tx.send(cmd)`).
     - *Result:* 🔴 **KILLED** (`Real LLM token streaming must chunk and dispatch at least 1 clause BEFORE LlmFinished: []` at `tests/llm_to_tts_test.rs:222:9`, `FAIL [2.57s]`).
  2. **Mutant 6.2 (Boundary Flip):** Comment out `self.flush_remainder(&handles);` in `services/harness/plugins/stream.rs:93`.
     - *Result:* ⚠️ **SURVIVED** (Known boundary behavior: Qwen's output for this prompt ends on a terminal period (`.`), flushed by `push_token` without leaving unflushed remainder in the chunker).
  3. **Mutant 6.3 (Intent Swap):** Swap `AudioIntent::TurnResponse` → `AudioIntent::InterimFiller` in `services/harness/plugins/stream.rs:144`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: LLM streaming clauses must have TurnResponse intent (left: InterimFiller, right: TurnResponse)` at `tests/llm_to_tts_test.rs:213:17`, `FAIL [2.61s]`).

---

## Seam 7: `tests/tts_to_playback_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 7.1 (Pre-roll Gate Inversion):** In `services/audio/playback.rs:137`, change `if occupied >= preroll_threshold` to `if true`.
     - *Result:* 🔴 **KILLED** (`Short chunk (< 12,000 samples) must NOT arm playback before worker flush_pre_roll` at `tests/tts_to_playback_test.rs:264:17`, `FAIL [0.082s]`).
  2. **Mutant 7.2 (Flush Deletion):** In `services/audio/playback.rs:198`, comment out `self.turn_armed.store(true, Ordering::Relaxed)`.
     - *Result:* 🔴 **KILLED** (`flush_pre_roll must immediately arm playback when unplayed samples exist` at `tests/tts_to_playback_test.rs:333:9`, `FAIL [0.091s]`).
  3. **Mutant 7.3 (Accounting Drop):** In `services/tts/actor.rs:89`, neuter `jobs.fetch_sub(1, Ordering::Relaxed)` to `jobs.load(Ordering::Relaxed)`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: pending_synthesis_jobs must decrement to 0 upon chunk synthesis completion (left: 1, right: 0)` at `tests/tts_to_playback_test.rs:179:9`, `FAIL [1.215s]`).

---

## Seam 8: `tests/tts_transition_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 8.1 (Voice Switch Deletion):** In `services/tts/actor.rs:130` in `spawn_tts_worker`, comment out `provider.set_voice(voice)`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: TTS provider active voice must be updated to 2 via IPC mutation (left: 0, right: 2)` at `tests/tts_transition_test.rs:206:9`, `FAIL [1.21s]`).
  2. **Mutant 8.2 (Intent Corruption):** In `pipeline/assistant/transcript.rs:100`, change compaction filler intent from `AudioIntent::InterimFiller` to `AudioIntent::TurnResponse`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Filler command must have InterimFiller intent (left: TurnResponse, right: InterimFiller)` at `tests/tts_transition_test.rs:347:17`, `FAIL [0.12s]`).
  3. **Mutant 8.3 (Threshold Inversion):** In `services/harness/plugins/budget.rs:61`, invert `percent >= CRITICAL_COMPACTION_THRESHOLD_PERCENT` to `<`.
     - *Result:* 🔴 **KILLED** (`tts_rx must receive filler TtsCommand::Generate from real router dispatch` timeout at `tests/tts_transition_test.rs:339:35`, `FAIL [5.12s]`).

---

## Seam 9: `tests/playback_interrupt_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 9.1 (Pending Deferral Bypass):** In `pipeline/assistant/playback.rs:76-83`, comment out `if pending_jobs > 0 { return; }` in `on_playback_finished`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: on_playback_finished must be deferred by router while pending_synthesis_jobs > 0 (left: Ready, right: Speaking)` at `tests/playback_interrupt_test.rs:250:9`, `FAIL [0.08s]`).
  2. **Mutant 9.2 (Ducking Inversion):** In `services/vad/actor.rs:246-259`, force `should_suppress_audio` to return `false` unconditionally.
     - *Result:* 🔴 **KILLED** (`[VAD ducking suppression during Speaker Speaking] Negative assertion failed: expected empty channel, but found item: SpeechStart` at `tests/common/harness.rs:290:9`, `FAIL [1.14s]`).
  3. **Mutant 9.3 (Barge-in Pending Reset Deletion):** In `pipeline/assistant/interrupt.rs:31`, comment out `state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed)` in `on_interrupt`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: pending_synthesis_jobs must be reset to 0 upon barge-in (left: 2, right: 0)` at `tests/playback_interrupt_test.rs:556:9`, `FAIL [0.07s]`).

---

## Seam 10: `tests/chunking_determinism_test.rs`
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 10.1 (Upstream Producer Silence / Token Insertion Suppressed):** Suppressed token insertion into chunker in `pipeline/assistant/accumulator.rs:35` (mandatory row: accumulator never fed).
     - *Result:* 🔴 **KILLED** (`Upstream producer must produce clauses (clauses_a was empty)` at `tests/chunking_determinism_test.rs:74:5`, `FAIL [0.018s]`).
  2. **Mutant 10.2 (Emergency Cap Boundary Inversion):** Altered `target_word_count` from 20 to 15 on the 25-word emergency cap in `services/tts/actor.rs:296`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: First chunk must have exactly 20 words from emergency cap, left: 15, right: 20` at `tests/chunking_determinism_test.rs:163:5`, `FAIL [0.009s]`).
---

## Seam 11: `tests/session_lifecycle_test.rs`
- **Mutants Attempted:** 4
- **Killed:** 4
- **Survivors:** 0
- **Mutation Score:** 4/4 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 11.1 (Persistence Dispatch Deletion):** Commented out `tx.try_send(PersistenceEvent::SessionStarted { ... })` in `pipeline/assistant/session.rs:205`.
     - *Result:* 🔴 **KILLED** (`Turso SQLite must contain inserted session row from persistence worker` at `tests/session_lifecycle_test.rs:134:9`, `FAIL [0.26s]`).
  2. **Mutant 11.2 (Harness Unmount Deletion):** Commented out `state.harness.lock().take();` in `pipeline/assistant/session.rs:463` on session end.
     - *Result:* 🔴 **KILLED** (`assertion failed: state.harness.lock().is_none(): HarnessSession must be unmounted (None) on session end` at `tests/session_lifecycle_test.rs:759:9`, `FAIL [0.08s]`).
  3. **Mutant 11.3 (Dictation CPAL Gate Inversion):** Inverted `state.pipeline.dictation_state() == InteractionState::Idle` to `if true` in `pipeline/assistant/session.rs:517`.
     - *Result:* 🔴 **KILLED** (`assertion failed: state.engine.lock().is_some(): CPAL engine must remain active when dictation is Ready` at `tests/session_lifecycle_test.rs:592:13`, `FAIL [0.09s]`).
  4. **Mutant 11.4 (Continuation Branch Deletion):** Replaced `if let (Some(sid), Some(conn)) = (session_id, conn.as_ref())` with `if false` in `pipeline/assistant/session.rs:240`.
     - *Result:* 🔴 **KILLED** (`assertion failed: Seeded database turn must be hydrated into working memory history` at `tests/session_lifecycle_test.rs:289:9`, `FAIL [0.24s]`).

---

## Seam 15: `tests/settings_persistence_test.rs`
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 15.1 (Upstream Producer Silence / Save Write Suppressed):** Added unconditional early `return Ok(());` in `core/settings.rs:1044` before filesystem write.
     - *Result:* 🔴 **KILLED** (`panicked at tests/settings_persistence_test.rs:269:5: settings.json must exist at paths::settings_path() after save`, `FAIL [0.014s]`).
  2. **Mutant 15.2 (Nested Struct Field Serialization Suppressed):** Added `skip_serializing` to `tts.voice_index` in `core/settings.rs:633`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/settings_persistence_test.rs:284:5: assertion left == right failed: Raw JSON must contain mutated tts.voice_index == 42, left: Null, right: 42`, `FAIL [0.017s]`).

---

## Seam 16: `tests/model_eviction_test.rs`
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 16.1 (ONNX Session Eviction Suppressed):** Commented out `embedder::unload_embedder();` in `services/memory/ml/mod.rs:27`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/model_eviction_test.rs:78:5: Embedder must be evicted after memory pipeline unload`, `FAIL [3.560s]`).
  2. **Mutant 16.2 (Worker Sender Drop/Reset Suppressed in cool_down_tts):** Altered `tts_tx.take()` to inspect via reference `if let Some(ref tx) = *tts_tx` without clearing `tts_tx` in `services/tts/actor.rs:235`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/model_eviction_test.rs:166:9: cool_down_tts must take and reset tts_tx to None`, `FAIL [0.578s]`).

---

## Seam 17: `tests/model_manager_test.rs`
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 17.1 (Zip-Slip Vulnerability Guard Suppressed):** Bypassed `entry.enclosed_name()` path traversal validation in `setup/model_manager.rs:339` with naive `entry.name()` path join.
     - *Result:* 🔴 **KILLED** (`panicked at tests/model_manager_test.rs:209:5: ModelManager::do_extract must fail on Zip-Slip path traversal`, `FAIL [0.014s]`).
  2. **Mutant 17.2 (Marker Deletion Suppressed in delete_model_file):** Commented out `std::fs::remove_file(&verified_path)` in `setup/manager_ops.rs:119`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/model_manager_test.rs:334:5: .verified marker must be removed by delete_model_file`, `FAIL [0.010s]`).
