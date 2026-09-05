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
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 3.1 (Realtime Speech Commit Suppressed):** Suppressed `rt_actor.signal_speech_committed` call on PTT speech validation (`pipeline/assistant/ptt.rs:86`).
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Exactly 1 speech turn must be committed to Realtime session, left: 0, right: 1` at `ptt_window_realtime_test.rs:200:13`, `FAIL [0.632s]`).
  2. **Mutant 3.2 (Pipeline Mode Branch Swap):** Routed `PipelineMode::Realtime` through `Modular` STT path unconditionally (`pipeline/assistant/ptt.rs:74` inverted to `if true || ctx.pipeline_mode == PipelineMode::Modular`).
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Exactly 1 speech turn must be committed to Realtime session, left: 0, right: 1` at `ptt_window_realtime_test.rs:200:13`, `FAIL [0.632s]`).

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
- **Mutation Score:** 3/3 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 5.1 (Generate Dispatch Suppressed):** Suppressed `llm_tx.send(LlmCommand::Generate)` dispatch in `pipeline/assistant/transcript.rs:118-124`.
     - *Result:* 🔴 **KILLED** (`Expected LlmCommand::Generate within 5s: Timeout` at `tests/transcript_to_llm_test.rs:123:14`, `FAIL [5.090s]`).
  2. **Mutant 5.2 (Threshold Maintenance Inverted):** Hardcoded `context_harness.needs_threshold_maintenance()` condition to `false` in `services/harness/facade.rs:88`.
     - *Result:* 🔴 **KILLED** (`Expected filler TtsCommand::Generate on critical threshold maintenance: Timeout` at `tests/transcript_to_llm_test.rs:283:14`, `FAIL [6.245s]`).
  3. **Mutant 5.3 (Realtime Pending Arming Omitted):** Omitted `pending_synthesis_jobs.store(1)` on `PipelineMode::Realtime` branch in `pipeline/assistant/transcript.rs:210-213`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Realtime transcript must arm pending_synthesis_jobs to 1, left: 0, right: 1` at `tests/transcript_to_llm_test.rs:233:13`, `FAIL [0.927s]`).

---

## Seam 6: `tests/llm_to_tts_test.rs`
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 6.1 (Clause Dispatch to TTS Suppressed):** Suppressed `tx.send(TtsCommand::Generate)` inside `services/llm/actor.rs:136-140` while letting `pending_synthesis_jobs.fetch_add` run.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: pending_synthesis_jobs (2) must exactly equal dispatched clause count (1)` at `tests/llm_to_tts_test.rs:189:9`, `FAIL [2.575s]`).
  2. **Mutant 6.2 (Streaming Token Chunking Suppressed):** Neuter `push_token` return in `services/llm/actor.rs:131`, yielding `let clauses = vec![];` so no streaming clauses are emitted before generation finishes.
     - *Result:* 🔴 **KILLED** (`Real LLM token streaming must chunk and dispatch at least 1 clause BEFORE LlmFinished: []` at `tests/llm_to_tts_test.rs:166:9`, `FAIL [2.019s]`).

---

## Seam 7: `tests/tts_to_playback_test.rs`
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 7.1 (Preroll Cushion Gating Inverted):** Emit `PlaybackStarted` unconditionally without verifying `occupied >= preroll_threshold` in `services/audio/playback.rs:125`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Short utterance must not trigger PlaybackStarted before flush` at `tests/tts_to_playback_test.rs:152:13`, `FAIL [0.281s]`).
  2. **Mutant 7.2 (Flush Pre-Roll Arming Suppressed):** Comment out `self.turn_armed.store(true)` in `services/audio/playback.rs:184`.
     - *Result:* 🔴 **KILLED** (`PlaybackStarted must fire immediately on flush_pre_roll` at `tests/tts_to_playback_test.rs:161:14`, `FAIL [0.512s]`).

---

## Seam 8: `tests/tts_transition_test.rs`
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 8.1 (Voice Switch Neuter):** Comment out dynamic voice swap application in `services/tts/actor.rs:242` on `TtsCommand::SetVoice`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Voice model must be updated, left: "default", right: "female_alt"` at `tests/tts_transition_test.rs:112:9`, `FAIL [1.204s]`).
  2. **Mutant 8.2 (Compaction Filler Dispatch Suppressed):** Invert compaction filler dispatch in `services/harness/facade.rs:120`.
     - *Result:* 🔴 **KILLED** (`Expected filler TtsCommand::Generate on compaction threshold: Timeout` at `tests/tts_transition_test.rs:188:14`, `FAIL [5.120s]`).

---

## Seam 9: `tests/playback_interrupt_test.rs`
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 9.1 (VAD Speaker Ducking Suppression Inverted):** Hardcoded `should_suppress_audio` to return `false` unconditionally in `services/vad/actor.rs:237`.
     - *Result:* 🔴 **KILLED** (`[VAD ducking suppression during Speaker Speaking] Negative assertion failed: expected empty channel, but found item: SpeechStart` at `tests/common/harness.rs:242:9`, `FAIL [1.135s]`).
  2. **Mutant 9.2 (Pending Jobs Guard Dropped in Playback Finished):** Deleted `if pending_jobs > 0 { return; }` guard in `pipeline/assistant/playback.rs:52`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: on_playback_finished must be deferred while pending_synthesis_jobs > 0, left: Ready, right: Speaking` at `tests/playback_interrupt_test.rs:199:9`, `FAIL [0.068s]`).

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
- **Mutants Attempted:** 2
- **Killed:** 2
- **Survivors:** 0
- **Mutation Score:** 2/2 (100%)
- **Mutations Realized & Verified:**
  1. **Mutant 11.1 (Idle-Guard Inverted on Session Start):** Inverted `if current_state != InteractionState::Idle { return; }` to `if false` in `pipeline/assistant/session.rs:151` (causing duplicate start calls to re-initialize).
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Second on_session_start while Ready must be a no-op guard, left: 2, right: 1` at `tests/session_lifecycle_test.rs:72:9`, `FAIL [0.107s]`).
  2. **Mutant 11.2 (Engine Stop Condition Inverted on Session End):** Inverted `if state.pipeline.dictation_state() == InteractionState::Idle` to `!= InteractionState::Idle` in `pipeline/assistant/session.rs:435`.
     - *Result:* 🔴 **KILLED** (`panicked at CPAL engine must remain active when dictation is Ready` at `tests/session_lifecycle_test.rs:248:9`, `FAIL [0.232s]`).

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
