# Domain 4 Implementation Checklist: Two-Phase Mid-Work Interruption, Step Steering & Architectural Clean Boundaries

---

### Phase 1: Minimal Signatures Stub (Backend Engineer)
*Depends on: None*
*Commit Gate: `git commit -m "feat(harness): minimal signatures stub for domain 4"`*
- [ ] `app/src-tauri/src/services/harness/stages/inbox.rs`
  - Create stub module with `QueuedMessageDelivery`, `InboxMessage`, and `StepInbox` (including `speech_hold_active: AtomicBool` and `notify: Notify`).
- [ ] `app/src-tauri/src/services/harness/chassis.rs`
  - Add `pub(crate) inbox: Arc<StepInbox>` to `Harness` struct and instantiate in `Harness::new_modular`.
  - Add `pub(crate) has_committed_steps_this_turn: bool` to `Harness` struct (initialized to `false`).
  - Add `pub(crate) uncommitted_user_turn_id: Option<u32>` to `Harness` struct (initialized to `None`).
  - Add `pub fn rollback_user_turn(&mut self, turn_id: u32) -> Option<ChatMessage>` to `Harness` struct.
- [ ] `app/src-tauri/src/core/events.rs`
  - Add `VoxEvent::StopTurn` variant.
- [ ] `app/src-tauri/src/ipc/pipeline.rs`
  - Add stub for `pub async fn stop_turn<R: Runtime>(...) -> Result<(), VoxIpcError>`.

---

### Phase 2: Black-Box Seam Tests (RED) (Test Engineer)
*Depends on: Phase 1*
*Gate: Run `cargo nextest run --test harness_loop_test --release` -> Confirm ALL 10 SUBTESTS FAIL (RED)*
*Commit Gate: `git commit -m "test(harness): add red seam tests for domain 4 interruption & steering"`*
- [ ] `app/src-tauri/tests/harness_loop_test.rs`
  - [ ] Subtest 1: `stop_turn_aborts_active_turn_and_transitions_to_ready_without_ending_session`
  - [ ] Subtest 2: `ready_state_disengages_session_to_idle_on_second_esc`
  - [ ] Subtest 3: `pre_working_thinking_barge_in_aborts_stream_and_merges_prompt`
  - [ ] Subtest 4: `mid_turn_thinking_barge_in_preserves_committed_tools_and_pushes_standalone_user_message`
  - [ ] Subtest 5: `active_working_speech_buffers_in_step_inbox_and_injects_at_step_boundary`
  - [ ] Subtest 6: `active_working_fast_tool_waits_at_step_boundary_until_user_speech_finalizes`
  - [ ] Subtest 7: `active_working_empty_speech_releases_hold_and_resumes_react_loop_without_aborting`
  - [ ] Subtest 8: `step_inbox_drains_immediate_messages_and_retains_turn_boundary_messages`
  - [ ] Subtest 9: `lint_arch_enforces_rule_harn_01_and_forbids_actor_commands_in_loop_and_steps`
  - [ ] Subtest 10: `sibling_tools_chassis_executes_isolated_from_stages`

---

### Phase 3: Atomic Production Implementation (GREEN) (Backend & Frontend Engineers)
*Forbidden from modifying `tests/**`!*

#### Batch 1: 4-Tier Refactor, Tool Sibling & `lint-arch.sh` Invariant (Backend Engineer)
*Depends on: Phase 2*
*Commit Gate: `git commit -m "refactor(harness): batch 1 4-tier boundary & tool sibling promotion"`*
- [ ] Move `app/src-tauri/src/services/harness/stages/tools/` to `app/src-tauri/src/services/harness/tools/`.
- [ ] `app/src-tauri/src/services/harness/mod.rs`
  - Export `pub mod tools;`.
- [ ] `app/src-tauri/src/services/harness/stages/mod.rs`
  - Remove `pub mod tools;`.
- [ ] `app/src-tauri/src/services/harness/stages/speech.rs` (new Tier 4 stage)
  - Implement `dispatch_tts_filler(tts_tx, turn_id, text, pending_jobs)`.
  - Implement `dispatch_tts_response(tts_tx, turn_id, text, pending_jobs)`.
- [ ] `app/src-tauri/src/services/harness/stages/dispatch.rs` (new Tier 4 stage)
  - Implement `dispatch_llm_generation(llm_tx, request, turn_id, cancel)`.
- [ ] `app/src-tauri/src/services/harness/stages/persistence.rs` (new Tier 4 stage)
  - Implement `persist_step_and_tools(persist_tx, session_id, turn_id, step_id, step_index, preamble, outcomes, duration, prompt_tokens, completion_tokens, strike, is_compacted)`.
  - Implement `persist_cancelled_turn(persist_tx, session_id, turn_id, user_text, partial_text)`.
- [ ] `app/src-tauri/src/services/harness/stages/budget.rs`
  - Implement helper method on `ContextBudgetStage` to estimate step prompt and completion tokens.
- [ ] `app/src-tauri/src/services/harness/steps.rs` (slimmed step sequencer ~350 lines)
  - Delegate filler/suspended TTS to `stages::speech`.
  - Delegate LLM generation to `stages::dispatch`.
  - Delegate persistence recording to `stages::persistence`.
  - Implement `step3_check_and_run_loop_budget` (moved from `loop.rs`).
  - Implement `step7_commit_suspended_question` (moved from `loop.rs`).
  - Remove imports of `TtsCommand`, `LlmCommand`, and `PersistenceEvent`.
- [ ] `app/src-tauri/src/services/harness/loop.rs` (slimmed coordinator ~150 lines)
  - Pure state machine calling `steps::*`.
  - Remove all direct imports of `stages::*`, `tools::*`, and `TtsCommand`.
- [ ] `app/src-tauri/scripts/lint-arch.sh`
  - Enforce all 3 invariants of `RULE-HARN-01`:
    1. `services/harness/stages/tools` must NOT exist.
    2. Neither `loop.rs` nor `steps.rs` may reference `TtsCommand`, `LlmCommand`, or `PersistenceEvent`.
    3. `loop.rs` (Tier 2) must NOT import `stages::` or `tools::` directly.

#### Batch 2: Two-Step `Esc` & Disengage UX (Backend & Frontend Engineers)
*Depends on: Batch 1*
*Commit Gate: `git commit -m "feat(harness): batch 2 two-step disengage ux and stop_turn ipc"`*
- [ ] `app/src-tauri/src/ipc/pipeline.rs`
  - Implement `stop_turn` sending `VoxEvent::StopTurn` to event router.
- [ ] `app/src-tauri/src/pipeline/assistant/mod.rs`
  - Route `VoxEvent::StopTurn` to `session::on_stop_turn(app, state, ctx)`.
- [ ] `app/src-tauri/src/pipeline/assistant/session.rs`
  - Implement `on_stop_turn`:
    - If `Idle` or `Ready`: no-op.
    - If `Listening`, `Thinking`, `Working`, or `Speaking`:
      - Cancel playback engine and clear AEC render queue.
      - Cancel turn token (`turn_token().cancel()`), set `cancel_flag = true`, and reset turn guards.
      - Synchronously under `state.harness.lock()` on router thread: clear `pipeline_accumulator` and `harness.inbox`.
      - If Step 0 had zero committed steps and zero spoken response, call `harness.history.rollback_last_user_turn()`.
      - Transition to `InteractionState::Ready`.
- [ ] `app/src/services/pipelineService.ts`
  - Export `export function stopTurn(): Promise<void> { return invoke("stop_turn"); }`.
- [ ] `app/src/shared/components/home/HomeControls.tsx`
  - During active turn (`Thinking`, `Working`, `Speaking`, `Listening`), render Stop button with `Square` icon and tooltip `"Stop current turn (Esc)"`.
  - When Stop button is clicked, call `stopTurn()`.
  - During `Ready`, render Disengage button with `X` / `Power` icon and tooltip `"Disengage session (Esc)"`.
  - When Disengage button is clicked, call `disengage()`.
- [ ] `app/src/pages/Home.tsx`
  - In `onKeyDown`: when `e.key === "Escape"`:
    - If `isTextModeOpen`: close text mode bar.
    - Else if `interactionState` is `Thinking`, `Working`, `Speaking`, or `Listening`: call `stopTurn()`.
    - Else if `interactionState === "Ready"`: call `disengage()`.

#### Batch 3: `StepInbox`, Speech Hold & Three-Scenario Steering Engine (Backend Engineer)
*Depends on: Batch 2*
*Commit Gate: `git commit -m "feat(harness): batch 3 step inbox and three-scenario steering"`*
- [ ] `app/src-tauri/src/services/harness/stages/inbox.rs`
  - Implement `StepInbox` with `speech_hold_active: AtomicBool`, `notify: Notify`.
  - Implement `push`, `engage_speech_hold`, `release_speech_hold`, `is_speech_held`, `wait_if_speech_held`, `drain_immediate`, `drain_all`, `clear`.
- [ ] `app/src-tauri/src/services/harness/chassis.rs`
  - Instantiate `inbox: Arc::new(StepInbox::new())` on `Harness`.
  - Maintain `pub(crate) has_committed_steps_this_turn: bool` (reset on `intake_recorded_turn`, set on `commit_tool_step`).
  - Expose `pub fn inbox(&self) -> &Arc<StepInbox>`.
  - Expose `pub fn has_committed_steps_this_turn(&self) -> bool`.
- [ ] `app/src-tauri/src/pipeline/assistant/speech.rs` & `interrupt.rs`
  - In `on_speech_start`:
    - Scenario A: If `prior_state == Thinking` and `!has_committed_steps_this_turn`: cancel turn, synchronously roll back Step 0 user turn, stash transcript via `acc.stash_for_continuation()`, transition to `Listening`.
    - Scenario B: If `prior_state == Thinking` and `has_committed_steps_this_turn`: cancel turn, retain committed tool steps, call `acc.clear()` (do NOT stash prefix), transition to `Listening`.
    - Scenario C: If `prior_state == Working`: mute playback, clear AEC queue, keep `turn_token` open, keep `turn_id` unchanged, call `harness.inbox.engage_speech_hold()`, transition to `Listening`.
- [ ] `app/src-tauri/src/pipeline/assistant/transcript.rs`
  - In `on_transcript_final`:
    - Check if active turn is open and speech-held in `StepInbox`:
      - If `trimmed.is_empty()` (cough / false VAD): call `inbox.release_speech_hold()`, transition back to `Working`, and return (zero lost work, no error toast).
      - If `!trimmed.is_empty()` (valid steering): emit `IpcEvent::TranscriptFinal`, call `inbox.push(Immediate)`, transition to `Working`, and return WITHOUT calling `spawn_harness_turn_task`.
    - In `TurnOutcome::Cancelled`: do NOT unconditionally transition to `Ready` if `pipeline.state()` was already transitioned by `vox-router`.
- [ ] `app/src-tauri/src/services/harness/steps.rs`
  - Implement `step6_await_and_drain_inbox`:
    - Await `inbox.wait_if_speech_held(&ctx.req.cancel).await`.
    - If cancelled: return error/cancel.
    - Drain `Immediate` messages and inject into history via `harness.history.push_user_turn(msg.text)`.

---

### Phase 4: Mutation Proof & Verification (Test Engineer)
*Depends on: Phase 3*
*Gate: Confirm nextest fails RED for each mutant in Phase 2b table, then revert to GREEN*
*Commit Gate: `git commit -m "test(harness): verify mutation coverage for domain 4"`*
- [ ] Mutant 1: `stop_turn` transitions to `Idle` instead of `Ready` -> Confirm test 1 fails RED -> Revert to GREEN.
- [ ] Mutant 2: `end_session` ignores call when in `Ready` -> Confirm test 2 fails RED -> Revert to GREEN.
- [ ] Mutant 3: Scenario A omits `rollback_last_user_turn` -> Confirm test 3 fails RED -> Revert to GREEN.
- [ ] Mutant 4: Scenario B calls `stash_for_continuation` unconditionally -> Confirm test 4 fails RED -> Revert to GREEN.
- [ ] Mutant 5: Scenario C cancels in-flight tools immediately on `SpeechStart` -> Confirm test 5 fails RED -> Revert to GREEN.
- [ ] Mutant 6: `wait_if_speech_held` returns immediately without awaiting `notify` -> Confirm test 6 fails RED -> Revert to GREEN.
- [ ] Mutant 7: `on_transcript_final` lets empty speech in held turn transition to `Ready` -> Confirm test 7 fails RED -> Revert to GREEN.
- [ ] Mutant 8: `drain_immediate` drains all messages unconditionally -> Confirm test 8 fails RED -> Revert to GREEN.
- [ ] Mutant 9: `lint-arch.sh` check disabled -> Confirm test 9 fails RED -> Revert to GREEN.
- [ ] Mutant 10: `tools/` referenced from `stages/tools/` -> Confirm test 10 fails RED -> Revert to GREEN.

---

### Phase 5: Adversarial Review & Sign-Off (Reviewer Agent)
*Depends on: Phase 4*
- [ ] Audit full diff against `backend-style-guide.md` and `testing-style-guide.md`.
- [ ] Verify zero occurrences of `TtsCommand`, `LlmCommand`, or `PersistenceEvent` in `loop.rs` or `steps.rs`.
- [ ] Verify `loop.rs` contains zero direct imports of `stages::` or `tools::`.
- [ ] Verify `./scripts/lint-arch.sh` and `./scripts/verify-targets.sh` pass with zero warnings.
- [ ] Issue formal review report and sign-off.
