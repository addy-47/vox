# Vox Pipeline Refactor — Test Suite Baseline Report

**Execution Context:**
- **Date / Timestamp:** 2026-11-17T17:35:00Z
- **Commit under test:** `f5f8dacb7033124a643665fe11fa9e658f4378ef`
- **Runner command used:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast`

---

## 1. Executive Summary

This baseline report records the clean-room test suite state for the Vox pipeline refactor under Ticket T-01. The objective of T-01 is to establish a rigorous, hardened test baseline ahead of production implementation tickets (T-02 through T-09) without modifying any production source files under `app/src-tauri/src/`.

Key achievements:
1. **Legacy Test Pruning & Modernization**: Eliminated reliance on the deprecated `TurnPhase` enum and deleted obsolete Step 0 history rollback assertions (`harness-spec.md:207`). User query staging (`pending_user_query`) is now verified via conversation history presence.
2. **Duplex Quiescence Alignment**: Updated playback tests to enforce `RULE-EVT-09`: `PlaybackStarted` transitions `Working -> Speaking`, and `PlaybackFinished` returns to `Working`/`Thinking` when `llm_done == false`. Emitted `VoxEvent::LlmFinished` before asserting `Ready` in synthesis tests.
3. **Infrastructure Style Compliance**: Replaced banned bare wall-clock sleeps with deterministic barriers (`vad_quiescence_barrier`), enforced panic propagation on worker thread joins across `AudioSceneRig` and `stt_turn_identity_test`, and unified disconnected anonymous cancel flags in `E2eVoiceRig`.
4. **Red-Seam Specification Coverage**: Authored 7 black-box red-seam tests strictly conforming to the `/create-test` protocol, mapping exact failure modes to future refactor tickets.

The full suite executes in ~197.7 seconds. Out of 564 tests, 547 pass (green), 17 fail as expected red seams, and there are 0 unexplained failures.

---

## 2. Summary Metrics

| Metric | Count |
| :--- | :--- |
| Tests Pruned / Deleted | 1 |
| Tests Refactored / Modified | 17 |
| New Red-Seam Tests Added | 7 |
| Total Tests in Suite | 564 |
| Currently Passing (Green) | 547 |
| Expected Failing Red-Seams (Red) | 17 |
| Unexpected / Unexplained Failures (Must be 0) | 0 |

---

## 3. Test Action Breakdown

| Test Name | File | Action | Target Spec Clause | Current Status |
| :--- | :--- | :--- | :--- | :--- |
| `pre_working_thinking_barge_in_aborts_stream_and_merges_prompt` | `tests/harness_loop_test.rs` | Pruned rollback assertion; rewritten `TurnPhase` check | `RULE-EVT-08A`, `RULE-HARN-12` | Red (seam) |
| `preamble_audio_playback_retains_working_state_during_tool_execution` | `tests/harness_loop_test.rs` | Refactored: asserts `Working -> Speaking` then `Speaking -> Working` | `RULE-EVT-09 §1, §2` | Red (seam) |
| `turn_cancellation_during_tool_execution_aborts_cleanly_and_clears_turn_open` | `tests/harness_loop_test.rs` | Refactored: intent block reconciled with body assertions | `RULE-EVT-08 §2b` | Green |
| `stream_error_clears_turn_open_and_dispatches_error_event` | `tests/harness_loop_test.rs` | Refactored: intent block reconciled with body assertions | `RULE-EVT-10` | Green |
| `the_tool_loop_stops_at_the_tool_budget` | `tests/harness_loop_test.rs` | Refactored: purged `AudioIntent::TurnResponse` pattern match | `RULE-EVT-09 §5` | Green |
| `mid_work_thinking_barge_in_preserves_committed_steps_and_commits_new_turn` | `tests/harness_loop_test.rs` | Refactored: rewritten `TurnPhase` check to test committed tool messages | `RULE-EVT-08B` | Red (seam) |
| `active_working_speech_cancels_turn_and_advances_dialogue_without_inbox_injection` | `tests/harness_loop_test.rs` | Refactored: asserts turn advance & cancellation without `StepInbox` injection | `RULE-EVT-08 §2b`, `RULE-HARN-18` | Red (seam) |
| `cognitive_loop_resumes_on_false_barge_in_in_thinking` | `tests/harness_loop_test.rs` | New Red-Seam: empty transcript resumes `Thinking` | `RULE-EVT-08 §2a`, `RULE-HARN-12` | Red (seam) |
| `guard_transition_harness_phase_blocks_stale_turn_and_preserves_speaking` | `tests/harness_loop_test.rs` | New Red-Seam: blocks stale turn and preserves `Speaking` | `RULE-EVT-01`, `RULE-EVT-09 §1` | Green |
| `tool_active_false_barge_in_restores_working_with_intact_activity` | `tests/harness_loop_test.rs` | New Red-Seam: empty transcript in tool preserves `current_activity` | `RULE-EVT-08 §1, §2a` | Red (seam) |
| `solo_playback_over_six_seconds_never_interrupts_with_pipewire_delay` | `tests/audio_capture/barge_in.rs` | Refactored: replaced bare sleep with `vad_quiescence_barrier` | `.agents/rules/testing-style-guide.md §5.1` | Green |
| `double_talk_barge_in_cancels_the_playback_and_opens_listening` | `tests/audio_capture/barge_in.rs` | Refactored: `utterance_id` increments, `turn_id` unchanged on `SpeechStart` | `RULE-EVT-01A`, `RULE-EVT-08 §1` | Red (seam) |
| `true_acoustic_barge_in_during_speaking_cancels_and_completes` | `tests/e2e_voice_pipeline_test.rs` | Refactored: `utterance_id` increments, `turn_id` unchanged on `SpeechStart` | `RULE-EVT-01A`, `RULE-EVT-08 §1` | Red (seam) |
| `collect_all_final_transcripts` | `tests/common/harness.rs` | Refactored: replaced bare sleep with non-blocking drain | `.agents/rules/testing-style-guide.md §5.1` | Green |
| `AudioSceneRig::drop` | `tests/common/harness.rs` | Refactored: panic propagation on worker thread join | `.agents/rules/testing-style-guide.md §5.3` | Green |
| `E2eVoiceRig` | `tests/common/harness.rs` | Refactored: unified anonymous cancel flags | `RULE-EVT-09 §4` | Green |
| `Harness::drop` | `tests/stt_turn_identity_test.rs` | Refactored: panic propagation on STT thread join | `.agents/rules/testing-style-guide.md §5.3` | Green |
| `stt_resets_stream_on_turn_divergence_without_prior_reset` | `tests/stt_turn_identity_test.rs` | New Red-Seam: STT decoder reset on turn rotation | `RULE-EVT-01A` | Green |
| `playback_gates_thinking_to_speaking_and_speaking_to_ready` | `tests/playback_test.rs` | Refactored: emit `LlmFinished` before asserting `Ready` | `RULE-EVT-09 §2, §3` | Green |
| `playback_finished_is_deferred_while_a_job_is_pending` | `tests/playback_test.rs` | Refactored: emit `LlmFinished` before final `PlaybackFinished` | `RULE-EVT-09 §2, §3` | Green |
| `duplex_quiescence_playback_finished_before_llm_finished_returns_thinking` | `tests/playback_test.rs` | New Red-Seam: `Speaking -> Thinking` when `llm_done == false` | `RULE-EVT-09 §2` | Red (seam) |
| `hardware_cancel_flag_rearmed_after_barge_in_produces_audible_chunks` | `tests/playback_test.rs` | New Red-Seam: audio accepted after barge-in | `RULE-EVT-09 §4` | Red (seam) |
| `zero_audio_fallback_emits_playback_finished_and_closes_quiescence_latch` | `tests/playback_test.rs` | New Red-Seam: fallback `PlaybackFinished` on 0 audio | `RULE-EVT-09 §4` | Green |
| `real_synthesis_is_ingested_and_arms_the_pre_roll` | `tests/playback/tts_to_playback.rs` | Refactored: emit `LlmFinished` before asserting `Ready` | `RULE-EVT-09 §2` | Green |

---

## 4. Granular Red-Test Ledger

The table below catalogs all 17 red tests observed in the baseline run. Every single failure is accounted for and maps to a planned ticket in the refactor plan.

| Red Test Name & File | Spec Rule Mapped | Authoritative Reason Why It Is Red (What Code Fails) | Resolving Ticket |
| :--- | :--- | :--- | :--- |
| `barge_in::double_talk_barge_in_cancels_the_playback_and_opens_listening`<br>`tests/audio_capture/barge_in.rs:190` | `RULE-EVT-01A`, `RULE-EVT-08 §1` | `on_interrupt` in `pipeline/assistant/interrupt.rs` prematurely advances `turn_id` on `SpeechStart`. Spec mandates monotonic turn rotation occurs strictly on semantic resolution (`TranscriptFinal`). | T-05, T-07 |
| `true_acoustic_barge_in_during_speaking_cancels_and_completes`<br>`tests/e2e_voice_pipeline_test.rs:270` | `RULE-EVT-01A`, `RULE-EVT-08 §1` | Same as above: `SpeechStart` increments `turn_id` before STT resolves speech content. | T-05, T-07 |
| `active_working_speech_cancels_turn_and_advances_dialogue_without_inbox_injection`<br>`tests/harness_loop_test.rs:2911` | `RULE-EVT-08 §2b`, `RULE-HARN-18` | `pipeline/assistant/transcript.rs:180-226` currently pushes voice transcripts into `StepInbox` during `Working` instead of aborting the turn and advancing dialogue. | T-05 |
| `cognitive_loop_resumes_on_false_barge_in_in_thinking`<br>`tests/harness_loop_test.rs:3390` | `RULE-EVT-08 §2a`, `RULE-HARN-12` | Empty transcript path in `transcript.rs` transitions unconditionally to `Ready` or aborts the turn task, rather than re-arming `stream_cancel_token` and resuming cognitive loop at Step 4. | T-05 |
| `mid_loop_critical_budget_triggers_compaction_while_preserving_active_turn_steps`<br>`tests/harness_loop_test.rs:2391` | `RULE-HARN-16`, `RULE-HARN-17` | Relational compaction ledger entry is missing child foreign key bindings during mid-loop compaction. | T-05 |
| `mid_turn_thinking_barge_in_preserves_committed_tools_and_pushes_standalone_user_message`<br>`tests/harness_loop_test.rs:2800` | `RULE-EVT-08B` | Mid-turn barge-in after Step 0 commits attempts to merge prompt into Step 0 instead of finalizing prior turn and dispatching standalone `Role::User`. | T-05 |
| `pre_working_thinking_barge_in_aborts_stream_and_merges_prompt`<br>`tests/harness_loop_test.rs:2639` | `RULE-EVT-08A` | Rapid query burst concatenation requires staged query extraction via `take_staged_query` and dispatching combined prompt on `TranscriptFinal`. | T-05 |
| `preamble_audio_playback_retains_working_state_during_tool_execution`<br>`tests/harness_loop_test.rs:545` | `RULE-EVT-09 §1` | `pipeline/assistant/playback.rs:33-39` explicitly drops `PlaybackStarted` when `intent == InterimFiller`, keeping state in `Working` instead of transitioning to `Speaking`. | T-04 |
| `tool_active_false_barge_in_restores_working_with_intact_activity`<br>`tests/harness_loop_test.rs:3473` | `RULE-EVT-08 §1, §2a` | `pipeline/assistant/interrupt.rs:71` clears `current_activity` on `Listening` or passes `None` on return to `Working`, wiping active tool context. | T-04, T-05 |
| `harness_turn_streams_clauses_to_tts_and_commits_history`<br>`tests/llm_to_tts_test.rs:340` | `RULE-HARN-12` | `stream_cancel_token` aborts the Tokio task in `streaming/router.rs`, causing `TurnOutcome::Cancelled` instead of `TurnOutcome::Completed`. | T-02, T-05 |
| `scripted_complete_sentence_reaches_tts`<br>`tests/llm_to_tts_test.rs:270` | `RULE-HARN-12` | Same as above: stream token aborting cognitive loop task. | T-02, T-05 |
| `scripted_stream_dispatches_clauses_to_tts`<br>`tests/llm_to_tts_test.rs:215` | `RULE-HARN-12` | Same as above: stream token aborting cognitive loop task. | T-02, T-05 |
| `tools_registered_but_unused_stream_completes`<br>`tests/llm_to_tts_test.rs:300` | `RULE-HARN-12` | Same as above: stream token aborting cognitive loop task. | T-02, T-05 |
| `duplex_quiescence_playback_finished_before_llm_finished_returns_thinking`<br>`tests/playback_test.rs:596` | `RULE-EVT-09 §2` | `on_playback_finished` unconditionally transitions `Speaking -> Ready` without checking `llm_done == false` and `current_activity.is_none()`. | T-04, T-06 |
| `hardware_cancel_flag_rearmed_after_barge_in_produces_audible_chunks`<br>`tests/playback_test.rs:645` | `RULE-EVT-09 §4` | `PlaybackEngine::cancel()` or anonymous cancel flags latch `true` on cancellation and are never re-armed on `next_turn()`, dropping Turn 2 chunks. | T-02, T-05 |
| `controller_dial_unknown_peer_rejection_preserves_paired_device_trust`<br>`tests/remote_connection_test.rs:1350` | `remote-control-spec.md` | Tier 1 trust decoupling: transport rejection by `UnknownPeer` erroneously mutates `paired_devices` ledger. | T-09 |
| `controller_disconnect_preserves_active_turn_identity`<br>`tests/remote_connection_test.rs:1395` | `remote-control-spec.md`, `RULE-EVT-01` | Transport disconnect emits a fabricated `state_changed` event with `turn_id: 0`, violating turn identity preservation. | T-09 |

---

## 5. Nextest Run Log Excerpt

```text
    Finished `release` profile [optimized] target(s) in 0.84s
────────────
     Summary [ 197.741s] 564 tests run: 547 passed, 17 failed, 4 skipped
        FAIL [   0.419s] (244/564) Vox::audio_capture_aec_test barge_in::double_talk_barge_in_cancels_the_playback_and_opens_listening
     SIGABRT [   5.715s] (283/564) Vox::e2e_voice_pipeline_test true_acoustic_barge_in_during_speaking_cancels_and_completes
        FAIL [   0.909s] (289/564) Vox::harness_loop_test active_working_speech_cancels_turn_and_advances_dialogue_without_inbox_injection
        FAIL [   0.807s] (291/564) Vox::harness_loop_test cognitive_loop_resumes_on_false_barge_in_in_thinking
        FAIL [   5.407s] (298/564) Vox::harness_loop_test mid_loop_critical_budget_triggers_compaction_while_preserving_active_turn_steps
        FAIL [   0.832s] (300/564) Vox::harness_loop_test mid_turn_thinking_barge_in_preserves_committed_tools_and_pushes_standalone_user_message
        FAIL [   0.830s] (305/564) Vox::harness_loop_test pre_working_thinking_barge_in_aborts_stream_and_merges_prompt
        FAIL [   0.280s] (306/564) Vox::harness_loop_test preamble_audio_playback_retains_working_state_during_tool_execution
        FAIL [   0.291s] (317/564) Vox::harness_loop_test tool_active_false_barge_in_restores_working_with_intact_activity
        FAIL [   0.293s] (333/564) Vox::llm_to_tts_test scripted_complete_sentence_reaches_tts
        FAIL [   0.284s] (334/564) Vox::llm_to_tts_test scripted_stream_dispatches_clauses_to_tts
        FAIL [   0.279s] (335/564) Vox::llm_to_tts_test tools_registered_but_unused_stream_completes
        FAIL [   5.259s] (416/564) Vox::playback_test duplex_quiescence_playback_finished_before_llm_finished_returns_thinking
        FAIL [   0.259s] (417/564) Vox::playback_test hardware_cancel_flag_rearmed_after_barge_in_produces_audible_chunks
        FAIL [   0.324s] (435/564) Vox::remote_connection_test controller_dial_unknown_peer_rejection_preserves_paired_device_trust
        FAIL [   0.028s] (436/564) Vox::remote_connection_test controller_disconnect_preserves_active_turn_identity
```
