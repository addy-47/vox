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
- **Mutants Attempted:** 7
- **Killed:** 5
- **Survivors:** 2 (Documented gaps in turn_token cancellation assertion and Invariant 13 partial persistence simulation)
- **Mutation Score:** 5/7 (71.4%)
- **Mutations Realized & Verified:**
  1. **Mutant 9.1 (Pending Deferral Bypass):** In `pipeline/assistant/playback.rs:76-83`, comment out `if pending_jobs > 0 { return; }` in `on_playback_finished`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: on_playback_finished must be deferred by router while pending_synthesis_jobs > 0 (left: Ready, right: Speaking)` at `tests/playback_interrupt_test.rs:250:9`, `FAIL [0.08s]`).
  2. **Mutant 9.2 (Ducking Inversion):** In `services/vad/actor.rs:246-259`, force `should_suppress_audio` to return `false` unconditionally.
     - *Result:* 🔴 **KILLED** (`[VAD ducking suppression during Speaker Speaking] Negative assertion failed: expected empty channel, but found item: SpeechStart` at `tests/common/harness.rs:290:9`, `FAIL [1.14s]`).
  3. **Mutant 9.3 (Barge-in Pending Reset Deletion):** In `pipeline/assistant/interrupt.rs:31`, comment out `state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed)` in `on_interrupt`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: pending_synthesis_jobs must be reset to 0 upon barge-in (left: 2, right: 0)` at `tests/playback_interrupt_test.rs:556:9`, `FAIL [0.07s]`).
  4. **Mutant 9.4 (Accumulator Clear Omission on Interrupt):** Commented out `state.pipeline_accumulator.lock().clear()` in `pipeline/assistant/interrupt.rs:54`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/playback_interrupt_test.rs:564:13: Accumulator assistant response must be cleared on interrupt`, `FAIL [0.06s]`).
  5. **Mutant 9.5 (Turn Token Cancel Omission on Interrupt):** Omitted `state.pipeline.turn_token().cancel()` in `pipeline/assistant/interrupt.rs:26`.
     - *Result:* ⚠️ **SURVIVED** (`test_barge_in_cancels_and_advances_turn` asserts `cancel_flag` and `state`, but does not verify `old_turn_token.is_cancelled()`; documented gap).
  6. **Mutant 9.6 (Turn Monotonic Advance Suppressed on Interrupt):** Replaced `next_turn()` with `(interrupted_turn_id, ())` in `pipeline/assistant/interrupt.rs:56`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/playback_interrupt_test.rs:531:9: Interrupt must generate new turn_id > old_turn_id (got 1 vs 1)`, `FAIL [0.06s]`).
  7. **Mutant 9.7 (Interrupt Ready State Inversion):** Changed interrupt transition from `Listening` to `Ready` in `pipeline/assistant/interrupt.rs:58`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/playback_interrupt_test.rs:577:9: assertion left == right failed: left: Ready, right: Listening`, `FAIL [0.06s]`).

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
- **Mutants Attempted:** 5
- **Killed:** 5
- **Survivors:** 0
- **Mutation Score:** 5/5 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 11.1 (Persistence Dispatch Deletion):** Commented out `tx.try_send(PersistenceEvent::SessionStarted { ... })` in `pipeline/assistant/session.rs:205`.
     - *Result:* 🔴 **KILLED** (`Turso SQLite must contain inserted session row from persistence worker` at `tests/session_lifecycle_test.rs:134:9`, `FAIL [0.26s]`).
  2. **Mutant 11.2 (Harness Unmount Deletion):** Commented out `state.harness.lock().take();` in `pipeline/assistant/session.rs:463` on session end.
     - *Result:* 🔴 **KILLED** (`assertion failed: state.harness.lock().is_none(): HarnessSession must be unmounted (None) on session end` at `tests/session_lifecycle_test.rs:759:9`, `FAIL [0.08s]`).
  3. **Mutant 11.3 (Dictation CPAL Gate Inversion):** Inverted `state.pipeline.dictation_state() == InteractionState::Idle` to `if true` in `pipeline/assistant/session.rs:517`.
     - *Result:* 🔴 **KILLED** (`assertion failed: state.engine.lock().is_some(): CPAL engine must remain active when dictation is Ready` at `tests/session_lifecycle_test.rs:592:13`, `FAIL [0.09s]`).
  4. **Mutant 11.4 (Continuation Branch Deletion):** Replaced `if let (Some(sid), Some(conn)) = (session_id, conn.as_ref())` with `if false` in `pipeline/assistant/session.rs:240`.
     - *Result:* 🔴 **KILLED** (`assertion failed: Seeded database turn must be hydrated into working memory history` at `tests/session_lifecycle_test.rs:289:9`, `FAIL [0.24s]`).
  5. **Mutant 11.5 (Capability Cache Inversion):** Inverted cached `supports_tools` boolean in `pipeline/assistant/session.rs:712` in `resolve_model_tool_support`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/session_lifecycle_test.rs:1000:9: Cached model capabilities must immediately set harness.supports_tools = true`, `FAIL [0.08s]`).

---

## Seam 12: `tests/memory_compaction_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 12.1 (Ingestion Queue Staging Deletion):** Commented out `insert_ingestion_queue_batch` in `persistence/compactions.rs:163`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Ingestion queue must hold exactly 2 facts across both runs (left: 0, right: 2)` at `tests/memory_compaction_test.rs:205:5`, `FAIL [0.18s]`).
  2. **Mutant 12.2 (Partial Unique Index Inversion):** Commented out `Ok(None)` on `latest.status == "in_progress"` in `services/memory/compaction/coordinator.rs:94-96`.
     - *Result:* 🔴 **KILLED** (`assertion failed: slice_res.unwrap().is_none(): Coordinator must return Ok(None) while compaction is in progress` at `tests/memory_compaction_test.rs:238:5`, `FAIL [0.11s]`).
  3. **Mutant 12.3 (Preemptive FIFO Threshold Inversion):** Changed `message_count >= MIN_MESSAGES_FOR_COMPACTION` to `>= 1` in `services/harness/plugins/compaction.rs:43`.
     - *Result:* 🔴 **KILLED** (`assertion failed: !plugin.can_perform_inline_compaction(3)` at `tests/memory_compaction_test.rs:250:5`, `FAIL [0.04s]`).

---

## Seam 13: `tests/memory_ingestion_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 13.1 (Winner-Takes-All Inversion):** Commented out `deactivate_facts_batch(conn, &duplicate_ids).await?;` and returned `Ok(0)` in `services/memory/ingestion/stage1_dedup.rs:135`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Must deactivate older matching fact (left: 0, right: 1)` at `tests/memory_ingestion_test.rs:78:9`, `FAIL [0.13s]`).
  2. **Mutant 13.2 (Cosine Dedup Threshold Drift):** Changed `SOFT_VECTOR_DEDUP_THRESHOLD` from `0.95` to `1.05` in `services/memory/ingestion/mod.rs:17`.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Must deactivate older semantic duplicate (>0.95 cosine) (left: 0, right: 1)` at `tests/memory_ingestion_test.rs:182:9`, `FAIL [0.86s]`).
  3. **Mutant 13.3 (Poison Pill Threshold Deletion):** Changed `retry_count >= 3` to `1 = 0` (false) in `persistence/queue.rs:221` in `reconcile_crashed_queue_on_boot` SQL query.
     - *Result:* 🔴 **KILLED** (`assertion left == right failed: Must reconcile all 3 crashed items (left: 2, right: 3)` at `tests/memory_ingestion_test.rs:347:9`, `FAIL [0.19s]`).

---

## Seam 14: `tests/personal_memory_test.rs`
- **Mutants Attempted:** 4
- **Killed:** 4
- **Survivors:** 0
- **Mutation Score:** 4/4 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 14.1 (Optimistic Concurrency Inversion):** Deleted `AND version = ?` from `UPDATE personal_memory` SQL queries in `persistence/personal_memory.rs:118, 126`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/personal_memory_test.rs:311:9: Stale expected_version must be rejected`, `FAIL [0.15s]`).
  2. **Mutant 14.2 (Quiescence Gate Deletion):** Commented out `verify_ingestion_quiescence(conn).await?;` in `services/memory/personal.rs:77`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/personal_memory_test.rs:376:9: Consolidation must be blocked when compaction is in progress`, `FAIL [0.12s]`).
  3. **Mutant 14.3 (Fact Consolidation Status Omission):** Commented out `mark_facts_consolidated(conn, &fact_ids).await?;` in `services/memory/personal.rs:114`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/personal_memory_test.rs:254:9: assertion left == right failed: Zero active personal facts must remain after consolidation (left: 100, right: 0)`, `FAIL [21.82s]`).
  4. **Mutant 14.4 (Continuation Turn Watermark Off-By-One):** Changed `last_compacted + 1` to `last_compacted` in `persistence/sessions.rs:372` in `fetch_session_continuation`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/personal_memory_test.rs:592:9: assertion left == right failed: Continuation turns must strictly contain uncompacted turns (turns 6..10), got 6 (left: 6, right: 5)`, `FAIL [0.14s]`).

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

---

## Seam 18: `tests/notifications_crud_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 18.1 (Zero-DB Invariant Violation / Routing Inversion):** Changed `Action::Transient => DeliveryChannel::ToastOnly` to `DeliveryChannel::NotificationOnly` in `services/notifications/router.rs:26`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/notifications_crud_test.rs:298:9: assertion left == right failed: (left: NotificationOnly, right: ToastOnly)`, `FAIL [0.14s]`).
  2. **Mutant 18.2 (Group-Key Rollup Bypass / Duplicate Row Creation):** Commented out `try_update_interactive_in_place` in `services/notifications/service.rs:111-115`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/notifications_crud_test.rs:441:9: assertion left == right failed: Group-key rollup must return the existing notification ID`, `FAIL [0.14s]`).
  3. **Mutant 18.3 (In-Place Resolution Omission):** Commented out `resolve_notification_in_place` in `services/notifications/actions.rs:136`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/notifications_crud_test.rs:520:9: Action execution must mark resolution as 'resolved' in metadata, got: {"resolution": "pending"}`, `FAIL [0.15s]`).

---

## Seam 19: `tests/realtime_transport_test.rs`
- **Mutants Attempted:** 3
- **Killed:** 3
- **Survivors:** 0
- **Mutation Score:** 3/3 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 19.1 (Paused State Reconnect Suppression Deletion):** Removed state check suppressing reconnect when `InteractionState::Paused` in `services/realtime/transport/connection.rs`.
     - *Result:* 🔴 **KILLED** (Failed assertion in `test_paused_state_suppresses_reconnect`).
  2. **Mutant 19.2 (Max Reconnect Limit Inversion):** Bypassed `attempt >= max_attempts` check in `services/realtime/transport/connection.rs`.
     - *Result:* 🔴 **KILLED** (Failed assertion in `test_terminal_reconnect_failure_and_halt`).
  3. **Mutant 19.3 (Cache TTL Expiration Bypass):** Commented out timestamp expiration comparison in `services/realtime/session_cache.rs`.
     - *Result:* 🔴 **KILLED** (Failed assertion in `test_session_cache_ttl_and_purge`).

---

## Seam 20: `tests/database_persistence_boundary_test.rs`
- **Mutants Attempted:** 4
- **Killed:** 4
- **Survivors:** 0
- **Mutation Score:** 4/4 (100.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 20.1 (Foreign Keys Inversion):** Set `PRAGMA foreign_keys = OFF;` in `persistence/schema.rs:149`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/database_persistence_boundary_test.rs: Turns must be deleted on session delete (CASCADE)`).
  2. **Mutant 20.2 (Partial Index WHERE Clause Deletion):** Deleted `WHERE status = 'in_progress'` from `idx_compactions_one_in_progress` in `persistence/schema.rs:57`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/database_persistence_boundary_test.rs: New in_progress compaction must succeed after previous one completed: UNIQUE constraint failed`).
  3. **Mutant 20.3 (Vector Float Little-Endian Swap):** Replaced `to_le_bytes()` with `to_be_bytes()` in `persistence/mod.rs:65`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/database_persistence_boundary_test.rs: Float bit-fidelity mismatch at dimension 1: original 0.012345356, decoded 0.0000000000111577934`).
  4. **Mutant 20.4 (Private Mode Event Drop Bypass):** Bypassed `is_private_mode` check for `PersistenceEvent::ToolCallExecuted` in `persistence/worker.rs:105`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/database_persistence_boundary_test.rs:783:9: assertion left == right failed: Tool call record must NOT be inserted into SQLite when private mode is active`, `FAIL [0.08s]`).

---

## Seam 21: `tests/agentic_tool_runtime_test.rs`
- **Mutants Attempted:** 5
- **Killed:** 3
- **Survivors:** 2 (Documented gaps in explicit tool.flow() assertion and empty title negative check)
- **Mutation Score:** 3/5 (60.0%)
- **Mutations Realized & Verified:**
  1. **Mutant 21.1 (Terminal Flow Contract Inversion):** Changed `RespondAndSetTitleTool::flow()` from `ToolFlow::Terminal` to `ToolFlow::NonTerminal` in `services/harness/stages/tools/title.rs:41`.
     - *Result:* ⚠️ **SURVIVED** (`test_terminal_tool_title_and_accumulator_parity` invoked `step6_handle_terminal_tool` directly without asserting `tool.flow() == Terminal`; documented gap).
  2. **Mutant 21.2 (Turn 2+ Title Filter Inversion):** Bypassed `respond_and_set_title` suppression filter on Turn 2+ in `services/harness/stages/tools/registry.rs:51`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/agentic_tool_runtime_test.rs:199:9: Turn 2 must suppress respond_and_set_title`, `FAIL [0.03s]`).
  3. **Mutant 21.3 (Title Emptiness Validation Bypass):** Bypassed `title.is_empty()` error validation check in `services/harness/stages/tools/title.rs:64`.
     - *Result:* ⚠️ **SURVIVED** (Integration suite only executed valid happy path payloads; documented negative input validation gap).
  4. **Mutant 21.4 (Turn Accumulator Parity Omission):** Omitted `stream_handles.accumulator` assignment in `services/harness/steps.rs:440` inside `step6_handle_terminal_tool`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/agentic_tool_runtime_test.rs:165:9: assertion left == right failed: TurnAccumulator must capture spoken_response for DB parity`, `FAIL [0.02s]`).
  5. **Mutant 21.5 (Scratchpad Observation Push Corruption):** Pushed empty string observation to `scratchpad` in `services/harness/steps.rs:556` inside `step6_handle_non_terminal_tool`.
     - *Result:* 🔴 **KILLED** (`panicked at tests/agentic_tool_runtime_test.rs:359:9: Scratchpad observation must contain memory search output`, `FAIL [0.03s]`).


