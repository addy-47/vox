# Domain 4 Implementation Plan: Two-Phase Mid-Work Interruption, Step Steering & Architectural Clean Boundaries

---

## 1. Target End-State

Domain 4 completes the Harness v2 core transformation by delivering:
1. **The 4-Tier Architectural Layering Contract (`RULE-HARN-01`)**:
   - `stages/tools/` promoted to sibling `services/harness/tools/`.
   - `loop.rs` slimmed to pure ~150-line turn coordinator importing ONLY `steps.rs`, `chassis.rs`, and `mod.rs`. Zero imports from `stages::*` or `tools::*`.
   - `steps.rs` slimmed to pure ~350-line step sequencer delegating to `stages/` and `tools/`.
   - Raw actor commands (`TtsCommand`, `LlmCommand`, `PersistenceEvent`) removed from `steps.rs` and `loop.rs`.
   - Dedicated Tier 4 stage dispatchers introduced: `stages/speech.rs`, `stages/dispatch.rs`, `stages/persistence.rs`.
   - Deterministic verification check in `scripts/lint-arch.sh` enforcing all 3 parts of `RULE-HARN-01`.
2. **Two-Step `Esc` & Disengage UX (`RULE-HARN-18`)**:
   - Discrete IPC command `stop_turn`: aborts active turn (`Thinking`, `Working`, `Speaking`, `Listening`), cancels playback, clears render queue, and transitions to `Ready`. Retains engaged session, microphone capture, and warm models in memory.
   - `end_session`: disengages session from `Ready` to `Idle`.
   - Frontend UI in `HomeControls.tsx`: displays Stop icon during active turn; displays Power/Close (`X`) icon during `Ready`.
   - Keyboard binding in `Home.tsx`: first `Esc` stops turn to `Ready`; second `Esc` disengages session to `Idle`.
3. **Three-Scenario Mid-Work Steering Engine**:
   - `Harness` carries `pub(crate) has_committed_steps_this_turn: bool` to distinguish Scenario A from Scenario B with 100% certainty.
   - **Scenario A (Pre-Working Thinking — Step 0)**: Aborts Step 0 LLM pass, rolls back premature Step 0 user turn synchronously, stashes transcript in continuation prefix, and re-prompts model with merged speech.
   - **Scenario B (Mid-Turn Thinking — Step $K \ge 1$)**: Aborts Step $K$ LLM pass, discards uncommitted Step $K$ tool calls, preserves all committed tool steps in history, clears accumulator (zero prompt duplication), commits new speech as standalone `Role::User`, and re-prompts model.
   - **Scenario C (Active Working)**: Mutes sound card immediately. Keeps in-flight tools running. Engages speech hold on `StepInbox`. Step boundary awaits speech resolution via `wait_if_speech_held`. Transcripts buffer into `StepInbox` with `Immediate`. Step boundary commits tool results and user message, advancing to Step $K+1$ in `Thinking`.
   - **Empty Speech Recovery**: Coughs or false VAD triggers during `Working` release the hold via `release_speech_hold()` and resume the loop with zero lost work (no split-brain or abort to `Ready`).
4. **Forward-Compatible `StepInbox` & Speech-Hold Latch**:
   - Implements `speech_hold_active: AtomicBool` + `notify: tokio::sync::Notify` in `stages/inbox.rs`.
   - Awaits speech finalization at step boundaries via `wait_if_speech_held`.
   - Implements `QueuedMessageDelivery::{Immediate, TurnBoundary}`.

---

## 2. Resolved Architectural Decisions (Clarification Ledger)

1. **Step-Boundary Hold Latch (`wait_if_speech_held`) (P0 Defect 1 Fix)**:
   - *Issue:* When tools completed at $t=400\text{ms}$ while the user spoke until $t=2900\text{ms}$, `loop.rs` drained an empty inbox at $t=401\text{ms}$ and generated speech while the user was still talking.
   - *Resolution:* Add `speech_hold_active: AtomicBool` and `notify: tokio::sync::Notify` to `StepInbox`. `on_speech_start` in `Working` calls `inbox.engage_speech_hold()`. `step6_await_and_drain_inbox` in `steps.rs` awaits `inbox.wait_if_speech_held(&ctx.req.cancel).await` before draining `Immediate` messages.
2. **Double-Loop Race Prevention in Scenario C (P0 Defect 2 Fix)**:
   - *Issue:* If `turn_token` was kept open in `Working`, `transcript.rs` naively spawned a second concurrent `execute_turn` task on `TranscriptFinal`, corrupting history.
   - *Resolution:* In `transcript.rs::on_transcript_final`, check if the active turn is open and speech-held in `StepInbox`. If non-empty, emit `IpcEvent::TranscriptFinal`, push `InboxMessage` with `Immediate` (waking the step boundary), transition to `Working`, and `return` without calling `spawn_harness_turn_task`. If empty (cough), call `inbox.release_speech_hold()`, transition back to `Working`, and return.
3. **History Invariant & Prompt De-duplication in Scenario B (P0 Defect 3 Fix)**:
   - *Issue:* Calling `stash_for_continuation()` during Step $K \ge 1$ re-prepended Step 0's query from `pipeline_accumulator.user_transcript`, duplicating `"Query 1"` in the steering message.
   - *Resolution:* Add `has_committed_steps_this_turn: bool` to `Harness`. In Scenario B (`has_committed_steps_this_turn == true`), `interrupt.rs` calls `acc.clear()` instead of `stash_for_continuation()`. `TranscriptFinal` commits `Role::User(new_speech)` as a standalone message without re-prepending Step 0.
4. **Synchronous Router Thread Cleanup & Idempotent Turn Rollback (P0 Defect 4 Fix & Implementation Guard)**:
   - *Issue:* An async `TurnOutcome::Cancelled` task executing off-thread raced `vox-router`, overwriting state transitions or popping subsequent user turns.
   - *Resolution:*
     - Track `uncommitted_user_turn_id: Option<u32>` on `Harness` (set to `Some(req.turn_id)` in `intake_recorded_turn`, cleared to `None` in `commit_tool_step`, `step7_commit_completed`, and `rollback_user_turn(turn_id)`).
     - Both `on_interrupt` (Scenario A) / `on_stop_turn` (on the `vox-router` thread) and `step7_handle_cancelled` (on the async Tokio task) call `harness.rollback_user_turn(turn_id)`. The first caller clears `uncommitted_user_turn_id` to `None`. If Turn $N+1$ starts 2ms later and pushes a new `Role::User` before Turn $N$'s async task finishes cancelling, Turn $N$'s `step7_handle_cancelled` safely no-ops rather than popping Turn $N+1$'s user message.
     - Enforce `RULE-EVT-01` (Single-Writer State Transitions): In `transcript.rs::spawn_harness_turn_task`, when `Harness::execute_turn` returns `TurnOutcome::Cancelled`, do NOT call `transition(InteractionState::Ready, ...)`. `on_interrupt` (`Listening`) and `on_stop_turn` (`Ready`) already own the synchronous transition on `vox-router`.
5. **Pure 4-Tier Boundary & Thin `loop.rs` (P0 Defect 5 Fix)**:
   - *Issue:* `loop.rs` directly imported `stages::*` and `tools::*`, ran inline budget checks, and sent TTS commands directly.
   - *Resolution:* Slim `loop.rs` to ~150 lines. Move budget check to `steps::step3_check_and_run_loop_budget`. Move suspended question audio to `steps::step7_commit_suspended_question`. Move inbox drain to `steps::step6_await_and_drain_inbox`. Move raw actor sends to `stages/speech.rs`, `stages/dispatch.rs`, `stages/persistence.rs`.
6. **Separation of Turn Abort (`stop_turn`) and Session Teardown (`end_session`)**:
   - *Issue:* Pressing `Esc` called `end_session`, dismantling the session and unloading models.
   - *Resolution:* Add discrete `stop_turn` IPC command. Calling `stop_turn` returns the pipeline cleanly to `Ready`. `end_session` is called only from `Ready` to `Idle`.

---

## 3. Subagent Orchestration Workflow & Incremental Commit Protocol

Implementation strictly follows **Pattern A (Red-Seam First / TDD)**.
Every batch and phase is implemented sequentially with its own atomic, incremental git commit.

### Orchestration Roles & Boundaries
- **Backend Engineer**: Implements production types, module reorganization, stage dispatchers, `stop_turn` IPC command, `StepInbox` with speech hold, and coordinator loop steering. Strictly forbidden from modifying `tests/**`.
- **Frontend Engineer**: Implements two-step UI state transitions, Stop icon rendering, and `Esc` keyboard routing in `HomeControls.tsx` and `Home.tsx`.
- **Test Engineer**: Authors the 10 black-box seam tests in Phase 2 (verifies RED). In Phase 4, **exclusively owns Mutation Testing**: offloads the mutation campaign to the remote 32-core server via `scripts/remote-mutate.py` (creds in `temp/server.txt`), deliberately seeding mutants from the False-Green Table into production code, confirming tests turn RED, reverting mutants, and confirming green.
- **Adversarial Reviewer Agent**: Evaluates the full diff with **zero prior context** (launched as an independent subagent with fresh eyes). Audits diffs against `backend-style-guide.md`, `testing-style-guide.md`, `architectural-invariants.md`, and authoritative specs (`harness-spec.md`, `ipc-spec.md`, `events-spec.md`). Does NOT run mutation testing.

### Sequential Phase Loop & Incremental Commit Gates

```
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 1: Minimal Signatures Stub                                       │
│ Agent: Backend Engineer                                                │
│ Action: Add public types, enums, trait methods & stubs in production   │
│ Commit Gate: git commit -m "feat(harness): minimal signatures stub D4" │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 2: Black-Box Seam Tests (RED)                                    │
│ Agent: Test Engineer                                                   │
│ Action: Write Subtests 1–10 in harness_loop_test and pipeline tests    │
│ Gate: Run cargo nextest -> EMPIRICALLY CONFIRM ALL 10 TESTS FAIL (RED) │
│ Commit Gate: git commit -m "test(harness): add red seam tests for D4"  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 3: Atomic Production Implementation (GREEN)                      │
│ Agent: Backend Engineer & Frontend Engineer [FORBIDDEN FROM tests/**]  │
│                                                                        │
│ Batch 1: 4-Tier Refactor, Tool Sibling & lint-arch.sh Invariant        │
│ Commit Gate: git commit -m "refactor(harness): batch 1 4-tier bounds"  │
│                                                                        │
│ Batch 2: Two-Step Esc & Disengage UX (stop_turn IPC & UI Controls)     │
│ Commit Gate: git commit -m "feat(harness): batch 2 two-step disengage" │
│                                                                        │
│ Batch 3: StepInbox, Speech Hold & Three-Scenario Steering Engine       │
│ Commit Gate: git commit -m "feat(harness): batch 3 step inbox steering"│
│                                                                        │
│ Gate: Run cargo nextest -> EMPIRICALLY CONFIRM ALL 10 TESTS PASS (GREEN│
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 4: Mutation Proof & Verification                                 │
│ Agent: Test Engineer                                                   │
│ Action: Seed deliberate mutants from False-Green table one by one.     │
│ Gate: Confirm nextest fails RED for each mutant, then revert to GREEN. │
│ Commit Gate: git commit -m "test(harness): verify mutation coverage D4"│
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 5: Adversarial Review & Sign-Off                                 │
│ Agent: Independent Reviewer Agent (ZERO prior context / fresh eyes)    │
│ Action: Audit full git diff against specs, invariants & style guides   │
│ Output: Formal Review Report (PASS or BLOCKED)                         │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Implementation Checkpoints & Blast Radius

### Checkpoint A: Minimal Signatures Stub (Phase 1)
*Scope:* Public types, enums, and method stubs so test stubs compile cleanly.
*Files:*
- `app/src-tauri/src/services/harness/stages/inbox.rs` (`QueuedMessageDelivery`, `InboxMessage`, `StepInbox` with `speech_hold_active` and `notify`)
- `app/src-tauri/src/services/harness/chassis.rs` (`Harness::inbox`, `Harness::has_committed_steps_this_turn`)
- `app/src-tauri/src/ipc/pipeline.rs` (stub for `stop_turn`)
- `app/src-tauri/src/core/events.rs` (`VoxEvent::StopTurn`)
*Build Expectation:* Compiles cleanly (`cargo test --no-run --release`).

---

### Checkpoint B: Batch 1 — 4-Tier Boundary Refactor & `lint-arch.sh` Invariant (Phase 3)
*Scope:* Promote `stages/tools/` to `services/harness/tools/`. Create Tier 4 stage dispatchers (`speech.rs`, `dispatch.rs`, `persistence.rs`). Move budget calculation, suspended question audio, and inbox drainage into `steps.rs`. Slim `loop.rs` to pure coordinator importing ONLY `steps.rs`, `chassis.rs`, `mod.rs`. Add invariant gate to `lint-arch.sh`.
*Files:*
- `app/src-tauri/src/services/harness/tools/` (moved from `stages/tools/`)
- `app/src-tauri/src/services/harness/mod.rs`
- `app/src-tauri/src/services/harness/stages/mod.rs`
- `app/src-tauri/src/services/harness/stages/speech.rs` (new Tier 4 stage)
- `app/src-tauri/src/services/harness/stages/dispatch.rs` (new Tier 4 stage)
- `app/src-tauri/src/services/harness/stages/persistence.rs` (new Tier 4 stage)
- `app/src-tauri/src/services/harness/stages/budget.rs` (token calculation helpers)
- `app/src-tauri/src/services/harness/steps.rs` (slimmed step sequencer)
- `app/src-tauri/src/services/harness/loop.rs` (slimmed coordinator)
- `app/src-tauri/scripts/lint-arch.sh`
*Build Expectation:* Full backend compiles clean and `./scripts/lint-arch.sh` passes.
*Definition of Done:* Zero occurrences of `TtsCommand`, `LlmCommand`, or `PersistenceEvent` in `loop.rs` and `steps.rs`. `loop.rs` contains zero direct imports of `stages::` or `tools::`.

---

### Checkpoint C: Batch 2 — Two-Step `Esc` & Disengage UX (Phase 3)
*Scope:* Implement `stop_turn` IPC command, `VoxEvent::StopTurn` event handler in assistant router, synchronous cleanup on router thread, two-step state transitions in `HomeControls.tsx`, and `Esc` keyboard routing in `Home.tsx`.
*Files:*
- `app/src-tauri/src/core/events.rs`
- `app/src-tauri/src/ipc/pipeline.rs`
- `app/src-tauri/src/pipeline/assistant/mod.rs`
- `app/src-tauri/src/pipeline/assistant/session.rs`
- `app/src/services/pipelineService.ts`
- `app/src/shared/components/home/HomeControls.tsx`
- `app/src/pages/Home.tsx`
*Build Expectation:* Frontend and backend compile clean (`pnpm build` & `./scripts/verify-targets.sh`).
*Definition of Done:* In `Thinking`, `Working`, `Speaking`, or `Listening`, pressing `Esc` or clicking Stop transitions cleanly to `Ready`. In `Ready`, pressing `Esc` or clicking Disengage transitions to `Idle`.

---

### Checkpoint D: Batch 3 — `StepInbox`, Speech Hold & Three-Scenario Steering Engine (Phase 3)
*Scope:* Fully implement `StepInbox` with speech-hold synchronization. Integrate `has_committed_steps_this_turn` on `Harness`. Update `speech.rs`, `interrupt.rs`, and `transcript.rs` for Scenario A, B, and C handling without double-loop race. Await speech hold at step boundary in `steps.rs::step6_await_and_drain_inbox`.
*Files:*
- `app/src-tauri/src/services/harness/stages/inbox.rs`
- `app/src-tauri/src/services/harness/stages/mod.rs`
- `app/src-tauri/src/services/harness/chassis.rs`
- `app/src-tauri/src/pipeline/assistant/speech.rs`
- `app/src-tauri/src/pipeline/assistant/interrupt.rs`
- `app/src-tauri/src/pipeline/assistant/transcript.rs`
- `app/src-tauri/src/services/harness/steps.rs`
- `app/src-tauri/src/services/harness/loop.rs`
*Build Expectation:* Full workspace compiles clean (`./scripts/verify-targets.sh`).
*Definition of Done:* Scenario A merges prompt and rolls back Step 0 user turn. Scenario B preserves tool history and appends standalone `Role::User`. Scenario C keeps tools running, engages speech hold, awaits resolution at step boundary, and injects into Step $K+1$. Empty speech releases hold and resumes with zero lost work.

---

## 5. Upgraded 10-Test Black-Box Seam Matrix & Phase 2b Mutation Table

| # | Subtest Name | Behavior Asserted | Seam Binary | Deliberate Mutant (Injected by Test Engineer) | Expected Mutation Result |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **1** | `stop_turn_aborts_active_turn_and_transitions_to_ready_without_ending_session` | Active turn in `Working` transitions to `Ready` via `VoxEvent::StopTurn`; harness remains mounted and `inbox` is cleared | `harness_loop_test` | In `on_stop_turn`, transition pipeline to `InteractionState::Idle` and `.take()` harness | Fails: pipeline state is `Idle` instead of `Ready`; harness is `None` |
| **2** | `ready_state_disengages_session_to_idle_on_second_esc` | `EndSession` while in `Ready` transitions pipeline to `Idle` and unmounts harness | `harness_loop_test` | In `on_end`, return early if `current_state == InteractionState::Ready` | Fails: pipeline remains in `Ready` instead of `Idle` |
| **3** | `pre_working_thinking_barge_in_aborts_stream_and_merges_prompt` | Scenario A (`Step 0 Thinking`) aborts LLM stream, rolls back Step 0 user turn (even if interim preamble tokens streamed), and merges new speech with prefix | `harness_loop_test` | In Scenario A, omit `rollback_last_user_turn()` | Fails: history contains duplicate unmerged Step 0 user turn |
| **4** | `mid_turn_thinking_barge_in_preserves_committed_tools_and_pushes_standalone_user_message` | Scenario B (`Step K >= 1 Thinking`) aborts Step K stream, preserves Step 0 `[User, Assistant(calls), Tool(results)]`, and appends new speech as `Role::User` **without** duplicating Step 0's query | `harness_loop_test` | In `interrupt.rs`, call `stash_for_continuation()` unconditionally during `Thinking` | Fails: Step 1 user steering message has Step 0's query prepended to it |
| **5** | `active_working_speech_buffers_in_step_inbox_and_injects_at_step_boundary` | Scenario C (`Working`) mutes audio, keeps tool running, buffers speech in `StepInbox`, commits `[Assistant(calls), Tool(results), User(steering)]`, and dispatches Step 1 in `Thinking` | `harness_loop_test` | In `on_interrupt` during `Working`, call `state.pipeline.turn_token().cancel()` | Fails: in-flight tool is cancelled and Step 1 never dispatches |
| **6** | `active_working_fast_tool_waits_at_step_boundary_until_user_speech_finalizes` | When a fast tool finishes in `Working` *while* the user is still speaking (`Listening`), the step boundary holds via `wait_if_speech_held` until `TranscriptFinal` arrives, then injects steering message into Step 1 | `harness_loop_test` | In `StepInbox::wait_if_speech_held`, return immediately without awaiting `notify` | Fails: Step 1 dispatches to LLM before `TranscriptFinal` arrives, missing user's steering message |
| **7** | `active_working_empty_speech_releases_hold_and_resumes_react_loop_without_aborting` | When `SpeechStart` occurs in `Working` and `TranscriptFinal("")` (empty speech / cough) arrives, the hold is released, pipeline returns to `Working`/`Thinking`, and Step 1 dispatches with zero lost work | `harness_loop_test` | In `on_transcript_final`, let empty speech during a held turn transition to `InteractionState::Ready` | Fails: pipeline aborts to `Ready` on cough and hangs/orphans the active tool turn |
| **8** | `step_inbox_drains_immediate_messages_and_retains_turn_boundary_messages` | `drain_immediate` partitions messages, returning `Immediate` and retaining `TurnBoundary` | `harness_loop_test` | In `StepInbox::drain_immediate`, drain all messages unconditionally | Fails: `TurnBoundary` messages are drained prematurely during intermediate step |
| **9** | `lint_arch_enforces_rule_harn_01_and_forbids_actor_commands_in_loop_and_steps` | `lint-arch.sh` detects raw actor commands in `loop.rs`/`steps.rs`, forbids `stages/tools/`, and forbids `loop.rs` from importing `stages::` or `tools::` | `scripts/lint-arch.sh` | Add dummy `TtsCommand` reference in `steps.rs` | Fails: `lint-arch.sh` reports `RULE-HARN-01` violation and exits with code 1 |
| **10** | `sibling_tools_chassis_executes_isolated_from_stages` | `tools/` operates as a sibling module under `services/harness/tools/` | `harness_loop_test` | Point `ToolRegistry` import back to `stages::tools` | Fails: compiler error (`stages::tools` does not exist) |

---

## 6. Definition of Done (DoD)

1. **Compilation Gate**: `cd app/src-tauri && ./scripts/verify-targets.sh` succeeds with zero errors and zero warnings.
2. **Deterministic Layering Gate**: `./scripts/lint-arch.sh` passes with zero violations:
   - `services/harness/stages/tools` does not exist.
   - Zero occurrences of `TtsCommand`, `LlmCommand`, or `PersistenceEvent` in `loop.rs` and `steps.rs`.
   - `loop.rs` contains zero direct imports of `stages::` or `tools::`.
3. **Incremental Git History**: Every phase and batch is committed with a separate atomic git commit.
4. **Deterministic Seam Tests**: Full test suite passes under `cargo-nextest`:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
5. **Empirical Mutation Proof**: Test Engineer validates all 10 deliberate mutants fail RED before being reverted to GREEN.
6. **Two-Step Disengage Validated**: UI and IPC tests confirm Step 1 stops turn to `Ready`, and Step 2 disengages session to `Idle`.
7. **Three Steering Scenarios Validated**: Scenario A, B, and C behave deterministically according to specification. Fast-tool race and empty speech recovery hold reliably.
8. **Independent Review Approval**: Independent Reviewer subagent audits the entire diff with fresh eyes and issues a formal PASS verdict.
