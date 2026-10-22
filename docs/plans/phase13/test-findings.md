# Test Revamp — Findings

Production defects and testability gaps discovered while revamping the suite.

**Campaign rule:** no production fix. Each finding is recorded, then the seam continues. Fixes are a separate decision.

Status values: `open` · `confirmed` · `testability-gap` · `wontfix`

---

## F-1 — Absence helper cannot prove absence

- **Seam:** Slice 1 (shared test support)
- **Test:** `tests/common/harness.rs::assert_channel_empty_after`
- **Symptom:** Sleeps for a fixed duration, then calls `try_recv` once. A slow producer passes; an early producer flakes.
- **Evidence:** `harness.rs:295-307`; 7 call sites in `dictation_window_test.rs`, 2 in `passive_streaming_test.rs`, more across the suite.
- **Suspected cause:** `assert_channel_empty_after` (sleep + single non-blocking receive).
- **Impact:** Every suppression assertion built on it is unproven. Barge-in, gating, and stale-cancel coverage are all affected.
- **Status:** **resolved in Slice 1.** Helper deleted. Replaced by `assert_channel_empty` (never waits) plus a per-call-site quiescence proof, and `drain_pending` (returns the drained items). Passive streaming now asserts after both producers are shut down and joined.

## F-2 — Transcript test bypasses the STT producer

- **Seam:** Slice 6 (transcript ingestion)
- **Test:** `tests/transcript_to_llm_test.rs::test_transcript_to_llm_matrix`
- **Symptom:** Calls `on_transcript_final` directly. Passes if the STT actor never emits a final transcript.
- **Evidence:** direct handler invocation in the test body.
- **Suspected cause:** test wired to the sink, not the trigger.
- **Impact:** The STT → LLM handoff has no real test.
- **Status:** **partially resolved in Slice 6.** `transcript_ingestion_test` proves every ingestion decision (store, merge, suppress, dispatch) with the real handler, the real harness and a recording LLM stub. The single-binary end-to-end (a real STT final reaching the LLM request) is still not covered: `vad_segmentation_test` stops at the transcript and `transcript_ingestion_test` starts at it.

## F-3 — "Paste" subtest does not paste

- **Seam:** Slice 5 (dictation output)
- **Test:** `tests/dictation_window_test.rs`
- **Symptom:** The subtest never exercises clipboard write, backup/restore, the 350 ms delay, Wayland fallback, the 1200 ms auto-stop, or the recovery-cache IPC.
- **Evidence:** no clipboard or paste calls in the body.
- **Suspected cause:** name overstates coverage; assertions stopped at the routing boundary.
- **Impact:** Paste is the primary dictation output path and is effectively untested.
- **Status:** **partially resolved in Slice 5.** `dictation_output_test` asserts the routing decision per mode (Tray, Clipboard, Paste), the typed headless failure, and that the transcript survives every failure in the last transcript, the in-memory history, and the durable JSONL history. The keystroke injection itself is still unobserved: `create_input_adapter()` is a hard-coded factory, so no test can see whether `simulate_paste()` was called. Needs an injectable adapter (production change) to close.

## F-4 — Continuation merging untested

- **Seam:** Slice 6
- **Symptom:** Zero references to `continuation_prefix` or `awaiting_continuation` in `tests/`.
- **Suspected cause:** `RULE-EVT-08A` has no integration test.
- **Impact:** Multi-utterance turns can duplicate or drop context.
- **Status:** **resolved in Slice 6.** `transcript_ingestion_test` covers the merge, the late-arriving final, and the empty-turn promotion of the stashed prefix. Seven unit tests in `pipeline/assistant/mod.rs` cover the accumulator itself.

## F-5 — Stale-cancel ordering untested

- **Seam:** Slice 6
- **Symptom:** No test for cancel-then-new-turn ordering.
- **Impact:** A stale cancel can kill the next turn. Named in `vad-asr-pipeline-bugs-rca.md`.
- **Status:** **resolved in Slices 3 and 6.** `stt_turn_identity_test::cancelled_final_for_a_later_turn_is_not_rejected` proves the worker does not reject a final for a different turn while the cancel flag is set. `transcript_ingestion_test::stale_cancel_does_not_block_the_next_turn` proves the assistant side: a cancelled token is re-armed by `next_turn`, so the next transcript is still dispatched.

## F-6 — VAD segmenter state machine untested

- **Seam:** Slice 2
- **Test:** `src/services/vad/segmenter.rs`
- **Symptom:** Unit tests cover helpers only; `process_continuous_segmentation` has no test.
- **Impact:** Pre-roll wipe and double-hysteresis bugs (RCA) are unprotected.
- **Status:** **resolved in Slice 2.** 10 deterministic state-machine tests drive the real `process_continuous_segmentation` with a scripted Earshot threshold (no weights, no threads). Covered: onset confirmation, unvoiced-dropout decay, pre-roll ordering, stream/final audio equality, turn-id stability, barge-in firmer onset, early-utterance hangover, silence suppression, onset decay, teardown. `actor.rs` gained 5 split `trim_window` tests.

## F-7 — STT actor turn identity untested

- **Seam:** Slice 3
- **Test:** `src/services/stt/actor.rs`
- **Symptom:** No tests for turn-id assignment or stream reset.
- **Impact:** Cross-turn transcript bleed is unverified.
- **Status:** **resolved in Slice 3.** New binary `stt_turn_identity_test` runs the real worker with a recording provider: final turn/owner propagation, +1 adoption without reset, turn-jump reset, same-turn idempotence, `u32` wrap adoption, cancelled-current vs cancelled-stale finals, explicit reset, warmup, partial recycling, shutdown. Five unit tests cover `coalesce_partials` and `drain_reset_stream` directly.

## F-15 — Partial coalescing leaks pooled audio buffers

- **Seam:** Slice 3 (STT turn identity)
- **Test:** `src/services/stt/actor.rs::coalesce_partials_keeps_latest_audio_and_recycles_earlier_buffers` (ignored)
- **Symptom:** When the worker coalesces N queued partials, every superseded buffer is sent to the **first** partial's recycle channel. That channel holds one buffer. The rest fail to send and are dropped, and the surviving command carries the first channel, not its own.
- **Evidence:** The ignored test fails at `the superseded second buffer must be recycled` with `Err(Disconnected)`: the second partial's `recycle_tx` was discarded by the `recycle_tx: _` pattern.
- **Suspected cause:** `coalesce_partials` destructures each later partial with `recycle_tx: _` and keeps the outer `recycle_tx`.
- **Impact:** Capture-side buffer pool starvation and extra allocation under load. Not data corruption: the newest audio is still transcribed.
- **Status:** confirmed — not fixed in this campaign. Test kept `#[ignore]`. Fix is one line: carry `next_recycle_tx` and recycle into the buffer's own channel.

## F-8 — Harness loop untested

- **Seam:** Slice 12
- **Test:** `src/harness/{loop,steps,chassis,session}.rs`
- **Symptom:** Zero tests across the main loop.
- **Impact:** The product's primary user path has no regression protection.
- **Status:** open — seeds Slice 12.

## F-9 — Swallowed worker failures

- **Seam:** Slice 1
- **Symptom:** 8 `let _ = ...join()` sites; 10 silent `try_recv` drains.
- **Impact:** A dead worker looks like a passing test; duplicate messages are hidden.
- **Status:** **resolved in Slice 1.** All 8 joins now `.expect("worker thread panicked")`. All 10 silent drains replaced: 8 became `assert_channel_empty` after a proven boundary, 2 became `drain_pending` with an assertion on the drained items.

## F-10 — Time-based flake

- **Seam:** Slice 1, 11, 12
- **Symptom:** Wall-clock `< 15 ms` assertion in `audio_capture_aec_test.rs`; `sleep(50 ms)` then assert `Idle` in `session_lifecycle_test.rs`; 65 raw sleeps overall.
- **Impact:** Red on shared machines for reasons unrelated to correctness; hides real failures.
- **Status:** open — seeds Slice 1 inventory.

## F-11 — File headers claim unasserted metrics

- **Seam:** Slice 0 (standard), applied per seam
- **Symptom:** `dictation_window_test.rs`, `passive_streaming_test.rs`, `ptt_window_modular_test.rs`, `transcript_to_llm_test.rs` claim latency and Levenshtein thresholds the bodies never compute. `chunking_determinism_test.rs` claims a 20-word cap; code uses 22 / 28 / 38.
- **Impact:** Reviewers trust a header that is false.
- **Status:** **partially resolved in Slices 2, 4 and 5.** `passive_streaming_test.rs`, `ptt_window_modular_test.rs`, `ptt_window_realtime_offline_test.rs` and `dictation_window_test.rs` are gone; their seams now carry honest headers. `transcript_to_llm_test.rs` still claims a Levenshtein threshold it does not compute — seeds Slice 6.

## F-12 — Seams lack a terminal acknowledgement for absence proofs

- **Seam:** 2, 4, 6, 9, 10 (wherever a suppression path is asserted)
- **Symptom:** `dictation_window_test`, `ptt_window_modular_test`, and `ptt_window_realtime_offline_test` prove quiescence by observing a state transition (`wait_for_dictation_state(Ready)` or `ptt_*` returning) and then asserting the channel is empty. That is the strongest proof available today, but it assumes the handler emits before it sets the terminal state.
- **Evidence:** no terminal event, ack, or sequence marker exists on the pipeline channels that a test can wait on to prove "this turn emitted everything it will ever emit".
- **Suspected cause:** event channels carry data, not turn boundaries; `RULE-EVT-08A` continuation makes "turn finished" explicit in the event model but not in the channel protocol.
- **Impact:** An emission scheduled after the terminal state transition would be missed by an immediate assertion. The old 500 ms sleep hid this by accident, not by design.
- **Status:** testability-gap — resolve per seam (add a turn-scoped terminal event or an ack) or accept the state-transition proof explicitly in the intent block.

## F-13 — Scenario-boundary drains were no-ops

- **Seam:** 1 (discovered while converting drains)
- **Symptom:** `passive_streaming_test` and `ptt_window_modular_test` drained their event channels at scenario boundaries on the assumption that prior scenarios left output behind. Asserting that the drained set was non-empty failed: the channels were already empty because the prior scenario's assertions had consumed the events.
- **Evidence:** `Subtest 1 must emit events before Subtest 2 starts` failed on unmodified production code.
- **Impact:** The drains proved nothing and hid nothing. They were ritual. They are now explicit empty assertions at the boundary.
- **Status:** resolved in Slice 1.

## F-14 — Minimum-utterance guard is unreachable

- **Seam:** Slice 2 (VAD segmentation)
- **Test:** `src/services/vad/segmenter.rs::utterance_below_minimum_emits_no_final` (ignored)
- **Symptom:** `VAD_MIN_UTTERANCE_SAMPLES` (4800, 0.3 s) never blocks a Final. A cough is transcribed.
- **Evidence:** Running the ignored test fails with `an utterance below the minimum must not emit Final`. Arithmetic confirms it: `compute_dynamic_end_frames` raises the end threshold to 50 frames while `speech_sample_counter < 20_000`. The 49 silence frames accumulated before the close add 12,544 samples, so `utterance_buffer.len()` is always above 4,800 by the time `handle_speech_end` checks the guard.
- **Suspected cause:** early-utterance protection (`EARLY_UTTERANCE_PROTECTION_SAMPLES` → 50-frame hangover) and the minimum-utterance rule were designed independently; hangover silence is appended to `utterance_buffer`.
- **Impact:** Short noises reach STT and the LLM. Cost is a wasted turn, not corruption.
- **Status:** confirmed — not fixed in this campaign. Test kept `#[ignore]` with owner `backend-engineer`. Decision needed: measure voiced samples before the guard, or exclude hangover silence from `utterance_buffer`.

## F-16 — STT turn-identity contract has no spec

- **Seam:** Slice 3
- **Symptom:** `docs/specs/` has no text for `SttCommand`, turn-id ownership, the `+1` adoption rule, `ResetStream` drain semantics, or cancellation precedence. The behaviour is defined only by `services/stt/actor.rs`.
- **Evidence:** `grep -rn "ResetStream|SttCommand|turn identity" docs/specs/` returns nothing.
- **Impact:** Tests encode intent that no approved document states. A future refactor can change the contract without a spec violation. This brushes against AGENTS.md §4.3.
- **Status:** spec-gap — not a code defect. Needs an owner (`create-spec`) before the behaviour is treated as contractual. No production change made.

## F-17 — Ingestion-gate purge has no deterministic observable

- **Seam:** Slice 4 (PTT lifecycle)
- **Test:** `tests/dictation_window_test.rs` (the only surviving purge test)
- **Symptom:** The purge runs inside the VAD actor loop when `ingestion_gate` is closed. No event, ack, or counter marks that it ran. The old test streams audio, then calls `tokio::time::sleep(150 ms)`, then reopens the gate and infers the purge from a later ghost-gate result.
- **Evidence:** `src/services/vad/actor.rs` gate branch clears buffers and sleeps; there is no observable side effect. A slow scheduler makes the inference flaky in the safe direction only by luck.
- **Impact:** The gate contract — "audio captured while nobody owns the mic must not reach the next turn" — is only weakly protected. Removing the sleep would make the test racy; keeping it violates the no-wall-clock rule.
- **Status:** testability-gap — Slice 4 removes the scenario from the PTT binary and leaves it untouched in `dictation_window_test.rs` until Slice 5 decides: add a purge counter/event to the actor, or drive the actor loop directly in a unit test. Production change required; not made.

## F-18 — A stalled LLM response blocks every later turn

- **Seam:** Slice 6 (transcript ingestion)
- **Symptom:** With no LLM response the next turn never reaches the LLM. The request for the
  second turn was not observed within 10 s while the first turn was still waiting for its
  generation response.
- **Evidence:** During Slice 6 the merge scenario deadlocked until the LLM answered. The
  harness mutex is held for the whole turn, so `step1_intake` of the next turn blocks
  before it can even evaluate the duplicate filter.
- **Suspected cause:** `Harness::execute_turn` keeps the harness lock while the stream pass
  awaits the provider response. A provider that never answers therefore wedges the turn
  loop, not just one turn.
- **Impact:** One hung provider request stops all later turns. The turn token is cancelled
  on interrupt, but the awaiting task does not observe it while it is blocked on the
  response channel.
- **Status:** confirmed — not fixed in this campaign. Production change required: release the
  harness lock before awaiting the provider, or make the await cancellable. Test written to
  exercise the path once fixed.
