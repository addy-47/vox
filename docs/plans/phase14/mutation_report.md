# Phase 4: Mutation Testing Campaign Report (Remote 32-Core Server)

- **Server**: `hypr4@100.67.98.126` (32 cores)
- **Campaign Date**: 2026-11-16 11:01:14 UTC
- **Total Mutants Tested**: 10
- **Mutants Killed (RED)**: 10
- **Mutants Survived**: 0
- **Empirical Kill Rate**: **100.0%**
- **Binary Execution**: `cargo test --test harness_loop_test --release -- --test-threads=1`

## Detailed Mutation Results Table

| # | Subtest / Mutant Name | Target File | Expected Failure Mode | Observed Result | Verdict | Time |
| :- | :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | `stop_turn_aborts_active_turn_and_transitions_to_ready_without_ending_session` | `src/pipeline/assistant/session.rs` | pipeline state is Idle instead of Ready; harness is None | `test stop_turn_aborts_active_turn_and_transitions_to_ready_without_ending_session ... FAILED` | ✅ **KILLED** | 63.70s |
| 2 | `ready_state_disengages_session_to_idle_on_second_esc` | `src/pipeline/assistant/session.rs` | pipeline remains in Ready instead of Idle | `test ready_state_disengages_session_to_idle_on_second_esc ... FAILED` | ✅ **KILLED** | 64.33s |
| 3 | `pre_working_thinking_barge_in_aborts_stream_and_merges_prompt` | `src/pipeline/assistant/interrupt.rs` | history contains duplicate unmerged Step 0 user turn | `test pre_working_thinking_barge_in_aborts_stream_and_merges_prompt ... FAILED` | ✅ **KILLED** | 63.78s |
| 4 | `mid_turn_thinking_barge_in_preserves_committed_tools_and_pushes_standalone_user_message` | `src/pipeline/assistant/interrupt.rs` | Step 1 user steering message has Step 0 query prepended | `test mid_turn_thinking_barge_in_preserves_committed_tools_and_pushes_standalone_user_message ... FAILED` | ✅ **KILLED** | 63.75s |
| 5 | `active_working_speech_buffers_in_step_inbox_and_injects_at_step_boundary` | `src/pipeline/assistant/interrupt.rs` | in-flight tool is cancelled and Step 1 never dispatches | `test active_working_speech_buffers_in_step_inbox_and_injects_at_step_boundary ... FAILED` | ✅ **KILLED** | 63.59s |
| 6 | `active_working_fast_tool_waits_at_step_boundary_until_user_speech_finalizes` | `src/services/harness/stages/inbox.rs` | Step 1 dispatches to LLM before TranscriptFinal arrives, missing steering message | `test active_working_fast_tool_waits_at_step_boundary_until_user_speech_finalizes ... FAILED` | ✅ **KILLED** | 63.84s |
| 7 | `active_working_empty_speech_releases_hold_and_resumes_react_loop_without_aborting` | `src/pipeline/assistant/transcript.rs` | pipeline aborts to Ready on cough and orphans the active tool turn | `test active_working_empty_speech_releases_hold_and_resumes_react_loop_without_aborting ... FAILED` | ✅ **KILLED** | 63.55s |
| 8 | `step_inbox_drains_immediate_messages_and_retains_turn_boundary_messages` | `src/services/harness/stages/inbox.rs` | TurnBoundary messages are drained prematurely during intermediate step | `test step_inbox_drains_immediate_messages_and_retains_turn_boundary_messages ... FAILED` | ✅ **KILLED** | 63.64s |
| 9 | `lint_arch_enforces_rule_harn_01_and_forbids_actor_commands_in_loop_and_steps` | `src/services/harness/steps.rs` | lint-arch reports RULE-HARN-01 violation and exits with code 1 | `[1;31mFAILED: 1 architectural invariant violation(s) detected.[0m` | ✅ **KILLED** | 0.97s |
| 10 | `sibling_tools_chassis_executes_isolated_from_stages` | `src/services/harness/stages/tools` | assert !legacy_stages_tools_dir.exists() fails | `test sibling_tools_chassis_executes_isolated_from_stages ... FAILED` | ✅ **KILLED** | 63.61s |

---
### Conclusion
All 10 deliberate mutants seeded into production code were empirically confirmed to fail RED on the 32-core remote server. The test harness contains zero false greens across Domain 4 interruption, steering, two-step ESC disengage, and architectural boundaries.
