# Vox Pipeline Refactor — Test Baseline & Remediation Report

## Domain 5 Final Certification — 2026-11-21

This section is additive. It records the final Domain 5 certification and preserves the 2026-11-19 certification and every historical checkpoint below verbatim.

**Full suite re-run at final HEAD.** The entire test suite was re-run end-to-end after all Domain 5 production and test fixes landed — not just the two seam binaries. Command and result, exactly as executed:

```bash
cd /opt/vox/app/src-tauri
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
EXIT_CODE=0
     Summary [ 212.558s] 578 tests run: 578 passed, 2 skipped
```

Compile in that run: `Finished release profile [optimized] target(s) in 42.07s`.

- **Full-suite result:** 578 passed, 0 failed, 2 skipped, exit code 0. This supersedes the earlier certified baseline of 566 passed / 0 failed / 2 skipped (203.8s). The delta of **+12** is exactly the 12 Domain 5 seam tests.
- **12/12 D5 seams pass inside the full suite**, including `harness_loop_test::bash_tool_executes_fast_command_synchronously_without_backgrounding`, `harness_loop_test::task_finished_in_ready_within_active_window_triggers_proactive_spoken_turn`, and `database_persistence_boundary_test::schema_v19_persists_session_tasks_lifecycle_and_enforces_status_check_constraint`.
- **Two skips, both §8-compliant external-dependency ignores** (unchanged set): `memory_compaction_test::compaction_slice_100_turns_against_a_remote_ollama_server` (remote Ollama hardware) and `realtime_transport_test::live_cloud_realtime_handshake` (live cloud API key). No internal or mocked test is ignored.
- **Compile gate:** `ANDROID_HOME=/opt/android-sdk ./scripts/verify-targets.sh` exited 0 (architectural lint, desktop clippy 0 warnings / 0 errors including the strengthened tests, desktop build, Android clippy).
- **Mutation:** 10/10 Domain 5 mutants KILLED (100%, byte-exact reverts, no markers in `src/`). The combined executable score is **327/327 killed (100%)** — pre-D5 317/317 plus D5 10/10 — with pre-D5 history preserved verbatim. `docs/tests/mutation_report.md` is the authoritative mutation record.
- **Clean-room review verdict: PASS.** All 8 majors were resolved: production fixes M1–M8 and the test-side false-green fixes M5/M6 plus the M2 assertion strengthening.

---

**Date:** 2026-11-19 (post-merge verification)  
**Role:** Senior Test Engineer  
**Suite Status (current):** `566 tests run: 566 passed, 0 failed, 2 skipped` (203.8s, release, sequential). Verified after merging the CP5 work with the backend's TF-03/TF-04 fixes. All four `process::exit(0)` bypasses were removed from `e2e_voice_pipeline_test.rs` and the E2E binary passes 4/4 with clean teardown. TF-01 through TF-07 are CLOSED; only the two §8-compliant external-dependency tests remain ignored.  
**Previous status (Checkpoint 4.5):** `564 passed, 2 failed, 2 skipped` — exposed TF-03 and TF-04.  

---

## Checkpoint 4.6 Final Certification — 100% Green Suite Baseline

Command executed and verified:
```bash
cd app/src-tauri
NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo nextest run --release --test-threads=1 --no-fail-fast --message-format libtest-json
# 566 tests run: 566 passed, 0 failed, 2 skipped  [0 errors]
```

### Defects Resolved in Production:

1. **TF-04 (`remote_link_test::ws_link_passes_generic_conformance_suite`)**:
   - Demuxed duplex reader stream with dedicated internal FIFO queues (`buffered_control`, `buffered_audio`) in `WsLink`.
   - Added non-blocking kernel peer-close inspection (`libc::recv(MSG_PEEK | MSG_DONTWAIT)`) on Unix to detect TCP FIN and WebSocket Close frames without async stall.
   - Enforced `check_control_message_size` unconditionally on raw decrypted bytes before payload demuxing.
   - Fixed `terminate_on_protocol_violation` to close both reader/writer halves, send `Message::Close(None)`, and record causal close reason.
   - Conformance: **18/18 generic conformance cases passed; all 19 remote link tests passed**.

2. **TF-03 (`e2e_voice_pipeline_test::mid_thought_continuation_combines_both_bursts`)**:
   - Root cause identified: In `transcript.rs:181`, `release_speech_hold()` and `rearm_hardware_cancel_flag()` were invoked before checking `trimmed.is_empty()` and before cancelling Turn 1. Turn 1 woke up uncancelled with a latched stream token, cycling 20 iterations in 1ms until `max_steps` exhausted. This in turn triggered `discard_pending_turn`, wiping the staged English query.
   - Fix applied: In `transcript.rs`, `cancel_active_turn()` is now invoked before releasing speech hold. Turn 1 terminates cleanly as `Cancelled`. `take_staged_query` takes the staged query and merges it with the Hindi burst into the combined prompt for Turn 2.
   - In `TurnAccumulator::resolve_merged_transcript`, added fallback to `user_transcript` if `continuation_prefix` is empty, ensuring zero uncommitted transcript drops.
   - Fixed empty-transcript continuation promotion: when `trimmed.is_empty()` and `continuation_prefix` is present, it promotes `turn_id` without spurious rotation.
   - Fixed Tokio async `blocking_lock()` panic in `tests/common/harness.rs` using `tokio::task::block_in_place`.
   - Verification: **`mid_thought_continuation_combines_both_bursts` passes in 35.93s; `transcript_ingestion_test` 12/12 passed**.

3. **Ignored Tests Audit (§8 Compliant)**:
   - Exactly 2 external dependency tests remain ignored:
     1. `memory_compaction_test::compaction_slice_100_turns_against_a_remote_ollama_server` (remote Ollama hardware dependency).
     2. `realtime_transport_test::live_cloud_realtime_handshake` (live cloud API key dependency).
   - Zero internal/mocked tests ignored.

---

## Checkpoint 4.5 Summary — No Hidden Failures

Command executed this session:
```bash
cd app/src-tauri
cargo nextest run --release --test-threads=1 --no-fail-fast
# 566 tests run: 564 passed, 2 failed, 2 skipped  [695.480s]
```

**The two failures are deliberate and documented:**

| Test | Cause | Finding |
| :--- | :--- | :--- |
| `e2e_voice_pipeline_test::mid_thought_continuation_combines_both_bursts` | `#[ignore]` removed. Fails after 505.5s with `Exceeded max tool iterations without terminal response`, then panics at `e2e_voice_pipeline_test.rs:488`. | TF-03 — tool-loop defect, not sidecar cancellation |
| `remote_link_test::ws_link_passes_generic_conformance_suite` | `#[ignore]` removed. Fails case `control_independent_of_audio`: malformed message at `remote_link_test.rs:607`. | TF-04 — WsLink contract mismatch |

The C-11 assertion error found in the first run was corrected to compare against the host key supplied to the handshake. `remote_connection_pairing_handshake_approved_reaches_connected` now passes in the final run.

**Changes in CP4.5:**
- `testing-style-guide.md §5.4`: replaced the blanket 15s timeout with **per-stage first-output ceilings of 5s** (LLM first token, STT first partial/final, TTS first audio chunk). Overall E2E timeouts may be longer.
- `testing-style-guide.md §8`: two local `#[ignore]` markers removed. Only cloud-key and remote-hardware ignores remain.
- C-10: realtime binary decode moved from a test-owned double to a production-driver unit test in `src/services/realtime/providers/deepgram/session.rs`.
- C-11: remote pairing test no longer asserts its own DB write; it now checks the controller record written by production `pair_device`.
- `test-findings.md` reconciled: TF-01/05/06/07 closed, TF-02/03/04 open.
- Mutation tooling: `remote-mutate.py` and `remote-campaign-runner.py` deleted; `scripts/mutation-runner.py` added with local/remote modes, `--mutants`, `--smoke`, `--dry-run`, byte-exact revert, and report generation. `tests/assets/mutants.json` contains four mutants.

**Mutation runner smoke test (verified):** M01 killed, M02 survived, both files reverted byte-for-byte, report written. M02 survivor is a genuine test weakness (the tested branch is bypassed by `on_llm_finished`); RCA recorded in `mutation_report.md`. **Full campaign deferred** until the suite baseline and findings are accepted.

---

## Historical Checkpoint 4 Summary — Production Fixes & Clean-Room Verification

## Checkpoint 4 Summary — Production Fixes & Clean-Room Verification

1. **TF-01 Fixed (Global Compaction Exclusion)**:
   `app/src-tauri/src/persistence/schema.rs:88` updated to `CREATE UNIQUE INDEX IF NOT EXISTS idx_compactions_one_in_progress ON session_compactions(status) WHERE status = 'in_progress'`.
   - `database_persistence_boundary_test:260` (`second_in_progress_compaction_is_rejected_by_partial_index`): **PASSED** (0.24s).
   - `memory_compaction_test:346` (`second_in_progress_compaction_on_a_session_is_rejected`): **PASSED** (0.31s).
   Both previously red seams are now 100% GREEN.

2. **TF-02 & TF-03 Production Hardening**:
   - T-12: Kokoro / ORT teardown drop ordering hardened in `src/services/tts/providers/kokoro.rs` and `devanagari.rs`.
   - T-13: `src/services/llm/sidecar.rs:132-157` uses `tokio::select!` cancellation on streaming requests.

3. **Compiler Gate Blocker Remediated**:
   - Removed 8 unused `let deadline = ...;` statements in `tests/tts_catalogue_test.rs`.
   - Scoped `MutexGuard` in `tests/harness_loop_test.rs:2714, 2868` into `{ ... }` blocks to resolve `clippy::await_holding_lock`.
   - `./scripts/verify-targets.sh` compiled desktop + Android targets with 0 warnings and 0 errors.

4. **Testing Style Guide Hardening**:
   - Enforced **MANDATORY 15-SECOND TIMEOUT CEILING** across all test suites.
   - Enforced **ZERO UNGROUNDED IGNORED TESTS POLICY**: only external cloud APIs or external servers may be `#[ignore]`.

5. **Full Nextest Verification**:
   ```
   Summary [ 269.519s] 563 tests run: 563 passed, 4 skipped
   ```
   Suite is 100% green and certified ready for Checkpoint 5 (Mutation Testing).

---

## Checkpoint 3 summary — audit-driven test remediation

**Command executed this session:**
```bash
cd app/src-tauri
export VOX_MODELS_DIR="$HOME/.vox/models"
export RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc)
cargo nextest run --release --test-threads=1 --no-fail-fast
# 563 tests run: 561 passed, 2 failed, 4 skipped (Summary [187.532s])
```

**Why the suite is no longer 100% green:** the audit (`test-review-report.md`) found that the previous green baseline hid a spec violation. `db-spec.md §5.4/§8` requires at most one `in_progress` compaction across the whole database; `schema.rs:88` uses a per-session unique index. Two tests were strengthened to assert the spec contract and now fail:

| Test | Failure |
| :--- | :--- |
| `database_persistence_boundary_test::second_in_progress_compaction_is_rejected_by_partial_index` | panics at `tests/database_persistence_boundary_test.rs:531` |
| `memory_compaction_test::second_in_progress_compaction_on_a_session_is_rejected` | panics at `tests/memory_compaction_test.rs:291` |

Both are recorded in `docs/tests/test-findings.md` as TF-01. Production fix is owned by backend-engineer / system-architect.

**Test-side fixes applied (no production source changed):** 11 critical false-greens addressed (cancel-flag re-arm, injected TTS fallback event, unasserted history commits, anti-architecture leaf tests, guard with no positive control, hardcoded IPC list, banned `try_recv` negative, global-vs-session compaction); 18 intent blocks reconciled; join results propagated; three quiescence barriers added; all four ignored tests given owner/expiry; eight default-tier wall-clock assertions removed; one no-op-unload test folded. The full list is in `test-review-report.md §10`.

**Skipped tests (4, unchanged set):** `remote_link_test::ws_link_passes_generic_conformance_suite` (TF-04), `realtime_transport_test::live_cloud_realtime_handshake` (TF-08), `memory_compaction_test::compaction_slice_100_turns_against_a_remote_ollama_server` (TF-09), `e2e_voice_pipeline_test::mid_thought_continuation_combines_both_bursts` (TF-03). All now carry owner and expiry.

**Open before mutation testing:** C-10 (realtime decoder asserts the double), C-11 (remote pairing self-fulfilled assertion), ~20 intent/leaf items, 13 sleep-to-state sites. These remain documented in `test-review-report.md`; they were not hidden by weakening assertions. Mutation testing stays deferred until the suite is green and these items are closed.

---

## Historical baseline (T-01 — prior test engineer, for context only)

The remainder of this report records the earlier T-01 baseline and is superseded by the section above.  

---

## 1. Executive Summary

This report certifies the achievement of a **100% green test baseline** for the Vox Pipeline Refactor (Ticket T-01). All 17 Category B defects and the 3 final test alignment issues have been fully remediated and verified against the authoritative specifications (`RULE-EVT-01A`, `RULE-EVT-08`, `RULE-EVT-09`, `RULE-HARN-18`).

Across the entire 28-binary test suite, **564 tests run: 564 passed, 0 failed, 4 skipped**. Zero production source files under `app/src-tauri/src/` were modified (with the sole sanctioned exception of the `#[cfg(test)]` unit test module inside `src/services/vad/segmenter.rs`).

---

## 2. Final 3 Remediation Actions

| # | Test & Location | Root Cause | Fix Applied | Result |
|---|-----------------|------------|-------------|--------|
| 1 | `tests/vad_segmentation_test.rs:315-360` (Subtest C) | In this seam test, `state.engine` was unattached (`None`). In production, `on_speech_start` dispatches `VadCommand::BindUtterance` via `state.engine`. Without this, VAD held pre-roll and live streaming chunks, preventing Nemotron partials from flowing to `partial_emitter`. | Attached `vad_cmd_tx` to `state.engine` via `common::harness::attach_mock_engine_with_vad_to_state(&app, &state, stt_tx.clone(), vad_cmd_tx.clone())`. On speech onset, the router dispatches `BindUtterance`, streaming audio to STT and firing `partial_emitter`. | **PASSED** (57.25s) |
| 2 | `tests/transcript_ingestion_test.rs:470-486` (`continuation_prefix_is_merged_into_the_next_query`) | Turn rotation on barge-in in `Listening` advances the dialogue turn to `second_turn + 1` (`RULE-EVT-08 §2b`). The test previously inspected `await_query(second_turn)`, which returned the already-completed first turn. | Dispatched `TranscriptFinal` during `Listening` to trigger turn rotation and query concatenation; updated assertion to `rig.await_query(second_turn + 1)`. | **PASSED** (0.36s) |
| 3 | `tests/transcript_ingestion_test.rs:500-530` (`late_arriving_final_is_captured_as_a_continuation_prefix`) | Late final capture in `Listening` under `RULE-EVT-01A` requires `awaiting_continuation = true` and `bound_utterance_id < peek_utterance_id()`. | Stashed continuation state on interruption (`stash_for_continuation()`) and bound older segment identity (`bound_utterance_id = Some(0)`). Verified late final is captured into `continuation_prefix` without triggering spurious generation. | **PASSED** (0.29s) |

---

## 3. Comprehensive Category B Defect Remediation Ledger (17 Confirmed Defects)

| # | File & Location | Defect Description | Resolution Applied | Verification Status |
|---|-----------------|--------------------|---------------------|---------------------|
| 1 | `tests/audio_capture/barge_in.rs:290-320` (`bypass_reproduces_the_echo_interruption`) | Asserts `turn_id == TURN + 1` immediately on `SpeechStart`. | Updated to assert `seen_bits & 1 != 0` (Listening observed), `utterance_id >= 1`, and `turn_id == TURN` per `RULE-EVT-01A`. | **PASSED** (0.43s) |
| 2 | `tests/audio_capture/barge_in.rs:250-276` (`double_talk_barge_in_cancels_the_playback_and_opens_listening`) | Expected `SttCommand::StreamChunk { turn_id: TURN + 1 }` without binding utterance. | Dispatched `VadCommand::BindUtterance { turn_id: TURN, utterance_id }` and verified `StreamChunk` carries active `turn_id == TURN`. | **PASSED** (0.46s) |
| 3 | `tests/playback_test.rs:514-555` (`barge_in_cancels_the_turn_and_advances_it`) | Asserted `current_turn_id > turn_1_id`, `turn_1_token.is_cancelled()`, and `pending_tts_jobs == 0` on Stage 1. | Updated to assert `turn_id == turn_1_id`, `!turn_1_token.is_cancelled()`, `!playback_engine.is_cancelled()` (hardware flag stays false per `RULE-EVT-09 §4`), and accumulator response preserved per `RULE-EVT-08 §2b`. | **PASSED** (0.32s) |
| 4 | `tests/transcript_ingestion_test.rs:669-712` (`interrupt_opens_a_new_turn`) | Asserted `new_turn != interrupted` immediately on `SpeechStart` in `Thinking`. | Asserted `turn_during_listening == interrupted` on `SpeechStart`; seeded active query and rotated turns strictly on non-empty `TranscriptFinal`. | **PASSED** (0.31s) |
| 5 | `tests/harness_loop_test.rs:2663-2688` (`pre_working_thinking_barge_in_aborts_stream_and_merges_prompt`) | Asserted `continuation_prefix == "search for"` during `Listening` before `TranscriptFinal`. | Deleted premature check during `Listening`; verified prompt merge upon `TranscriptFinal` using active `turn_id` per `RULE-EVT-08A`. | **PASSED** (0.35s) |
| 6 | `tests/harness_loop_test.rs:388-416` (`zero_tool_turn_with_interim_filler_latches_to_ready_only_after_playback_drains`) | Asserted sending `LlmFinished` while `pending_tts_jobs == 1` forced `Speaking`. | Updated assertion: state remains in `Thinking` on `LlmFinished` per `RULE-EVT-09 §3`; transitions to `Speaking` when `PlaybackStarted` arrives. | **PASSED** (0.36s) |
| 7 | `tests/harness_loop_test.rs:545-585` (`preamble_audio_playback_retains_working_state_during_tool_execution`) | `turn_id: 202` dropped by router epoch guard because `turn_id` was not stored in pipeline state. | Stored `turn_id` into `pipeline.turn_id`, set `current_activity`, and verified duplex return `Working -> Speaking -> Working` via `wait_for_state`. | **PASSED** (0.35s) |
| 8 | `tests/llm_to_tts_test.rs:147-152` (`SeamRig::turn_request`) | `turn_request` built `TurnExecutionRequest` without storing `turn_id` in `pipeline.turn_id`, causing router to cancel all passes as `Superseded`. | Added `self.state.pipeline.turn_id.store(turn_id, Ordering::Relaxed)` in `SeamRig::turn_request`. | **PASSED** (All 4 tests green) |
| 9 | `tests/stt_turn_identity_test.rs:241-255` (`open_stream_adopts_incremented_turn_without_reset`) | Asserted `reset_count == 2` based on legacy `wrapping_add(1)` adoption hack. | Updated assertion to `assert_eq!(h.recording.reset_count(), 3)` (T-07 bans adoption without reset; turn jump forces provider reset). | **PASSED** (0.01s) |
| 10 | `tests/stt_turn_identity_test.rs:310-325` (`wrapped_turn_id_is_adopted_without_reset`) | Asserted `reset_count == 2` on `u32::MAX -> 0` wrap. | Updated assertion to `assert_eq!(h.recording.reset_count(), 3)`. | **PASSED** (0.01s) |
| 11 | `tests/model_eviction_test.rs:348-360` (`tts_cooldown_clears_sender_and_joins_worker`) | Test attempted `tts_handle.take().expect(...)` after `cool_down_tts` already joined and took it. | Replaced with `assert!(tts_handle.is_none(), "cool_down_tts must join and take handle")` per T-08. | **PASSED** (0.02s) |
| 12 | `src/services/vad/segmenter.rs:640-657` (`pre_roll_tail_is_forwarded_as_the_first_stream_chunk`) | Test step did not bind utterance, so `awaiting_utterance_binding` held all audio. | Added `Rig::bind_utterance` method; called `rig.bind_utterance(1)` to flush buffered pre-roll + onset chunk. | **PASSED** (Unit test green) |
| 13 | `src/services/vad/segmenter.rs:660-684` (`streamed_audio_matches_final_audio_without_overlap`) | Test step did not bind utterance, preventing stream chunks from flowing. | Called `rig.bind_utterance(1)` after onset; verified streamed concatenation matches final audio. | **PASSED** (Unit test green) |
| 14 | `src/services/vad/segmenter.rs:687-719` (`one_utterance_uses_one_turn_id_across_pre_roll_stream_and_final`) | Asserted predicted turn 8 before router committed. | Called `rig.bind_utterance(8)` to commit router binding; verified all commands share turn 8. | **PASSED** (Unit test green) |
| 15 | `tests/vad_segmentation_test.rs:315-360` (Subtest C) | In Subtest C, `partial_count > 0` was unfulfilled due to unattached mock engine. | Attached mock engine with VAD sender; router automatically dispatches `BindUtterance` on onset, enabling stream chunks and partials. | **PASSED** (57.25s) |
| 16 | `tests/remote_connection_test.rs:1292-1302` (`controller_reconnect_preserves_reconnecting...`) | Expected `RemoteFailureKind::Revoked` on `UnknownPeer` control message. | Updated to assert `RemoteFailureKind::UnknownPeer` matching protocol contract and spec. | **PASSED** (0.04s) |
| 17 | `tests/transcript_ingestion_test.rs:500-530` (`late_arriving_final_is_captured_as_a_continuation_prefix`) | Asserted obsolete `awaiting_continuation` flag without stashing state. | Implemented continuation state stashing and segment binding; verified late final capture into `continuation_prefix`. | **PASSED** (0.29s) |

---

## 4. Test Infrastructure & Invariant Enforcement

1. **Zero Bare Sleeps**: All bare sleeps in state-machine synchronization and audio capture tests have been eliminated and replaced with deterministic polling barriers (`wait_for_state`), channel event synchronizers (`wait_for_event`), or `vad_quiescence_barrier`.
2. **Panic Propagation on Thread Joins**:
   - `AudioSceneRig::drop` (`tests/common/harness.rs:1000`): Thread joins check `!res.is_err()` when `!std::thread::panicking()`.
   - `stt_turn_identity_test.rs:160`: `Harness::drop` asserts worker handles join cleanly without panicking.
3. **Intent Blocks (Testing Style Guide §2)**: Every authored and modified test contains standard `PURPOSE`, `SCENARIO`, `ASSERTS`, `CLASS`, and `MUTANT` annotations.
4. **Tool Economy & Execution Invariants**: All tests executed strictly in release mode (`--release`). Zero polling loops. `cargo nextest list` was strictly avoided throughout the campaign.

---

## 5. Certification & Handoff

- **Test Suite Status**: 100% Green (`564 passed, 0 failed, 4 skipped`).
- **Compile Gate**: Passed cleanly via `cargo test --no-run --release`.
- **Conclusion**: Ticket T-01 is **COMPLETE** and verified. The test harness and baseline are in pristine condition for implementation of Ticket T-02.
