# Pipeline Refactor Checklist: Phase 14

## T-01: Test Engineer Baseline (Forked Subagent)
*Scope: `tests/`, `evals/`, `benches/` (Read-only on production source)*
- [x] Read `.agents/rules/testing-style-guide.md` and `/create-test` skill.
- [x] `tests/harness_loop_test.rs`: Prune/rewrite `TurnPhase` assertions at lines 2646, 2821, 2834 to check `pending_user_query.is_some()`.
- [x] `tests/harness_loop_test.rs`: Delete legacy Step 0 history rollback assertion at line 2696 (`harness-spec.md:207`).
- [x] `tests/harness_loop_test.rs`: Reconcile false-green intent blocks in lines 793 and 959 with body assertions.
- [x] `tests/harness_loop_test.rs`: Purge `AudioIntent::TurnResponse` match at line 1013.
- [x] `tests/harness_loop_test.rs`: Line 584: Update to assert `Working --> Speaking` on `PlaybackStarted`, and `Speaking --> Working` on `PlaybackFinished` when `llm_done == false`.
- [x] `tests/harness_loop_test.rs`: Line 1388: Update so voice barge-in in `Working` tests turn advance and cancellation; test `StepInbox` via programmatic push per `RULE-HARN-18`.
- [x] Update `tests/audio_capture/barge_in.rs:212`, `tests/e2e_voice_pipeline_test.rs:293`, and `tests/harness_loop_test.rs:1244` to assert `utterance_id` increments on `SpeechStart` and `turn_id` increments on non-empty `TranscriptFinal`.
- [x] `tests/audio_capture/barge_in.rs`: Replace bare `sleep(300ms)` at line 162 with `vad_quiescence_barrier`.
- [x] `tests/common/harness.rs`: Fix `let _ = handle.join()` swallowing panics at lines 1003, 1006, 1011; replace bare sleep at lines 342-343.
- [x] `tests/common/harness.rs`: `AudioSceneRig::drop` (lines 582-593): assert `!res.is_err()` when `!std::thread::panicking()`.
- [x] `tests/common/harness.rs`: `E2eVoiceRig` (lines 693, 703, 728, 732): wire cancel flags to `state.pipeline.hardware_cancel_flag`.
- [x] `tests/stt_turn_identity_test.rs`: Fix `let _ = join.join()` at line 160.
- [x] `tests/playback_test.rs`: Emit `VoxEvent::LlmFinished` before asserting `Ready` in:
  - `playback_gates_thinking_to_speaking_and_speaking_to_ready` (line 47)
  - `short_utterance_requires_a_flush_to_arm_the_pre_roll` (line 147)
  - `playback_finished_is_deferred_while_a_job_is_pending` (line 208)
- [x] `tests/playback/tts_to_playback.rs`: Emit `VoxEvent::LlmFinished` before asserting `Ready` in:
  - `real_synthesis_is_ingested_and_arms_the_pre_roll` (line 42)
- [x] Add Red-Seam Tests:
  - Cognitive loop resumption on false barge-in (`Thinking --> Listening --> Thinking`).
  - `llm_done` lifecycle: `PlaybackFinished` before `LlmFinished` returns `Thinking`/`Working`.
  - Guard enforcement: `transition_harness_phase` blocks stale turns and preserves `Speaking`.
  - Cancel flag re-arm: Second turn after barge-in produces audio chunks.
  - Tool-active false barge-in: Stage 2 empty transcript restores `Working` with intact `current_activity`.
  - Zero-audio fallback: Empty TTS synthesis emits fallback `PlaybackFinished` and closes quiescence latch.
  - Utterance binding: VAD holds chunks pre-bind; STT resets on unknown utterance ID.
- [x] Deliver baseline report artifact: `docs/plans/phase14/test-baseline-report.md`.

---

## T-02: PipelineAtomics & Owned Hardware Cancel Flag
- [ ] `pipeline/atomics.rs`: Add `hardware_cancel_flag: Arc<AtomicBool>` and `llm_done: Arc<AtomicBool>`.
- [ ] `pipeline/atomics.rs`: Add `stream_cancel_token: parking_lot::Mutex<tokio_util::sync::CancellationToken>`, with `cancel_active_stream()` and `rearm_stream_cancel_token()`.
- [ ] `pipeline/atomics.rs`: Update `cancel_active_turn` to store `hardware_cancel_flag = true`.
- [ ] `pipeline/atomics.rs`: Update `rearm_turn_cancel_token` to store `hardware_cancel_flag = false`.
- [ ] `pipeline/atomics.rs`: Update `next_turn` to reset `llm_done = false`, `hardware_cancel_flag = false`, and re-arm tokens.
- [ ] `pipeline/atomics.rs`: Add `mark_llm_finished(turn_id)` with active `turn_id` validation.
- [ ] `services/audio/playback.rs`: Split `PlaybackEngine::cancel()`: remove `self.cancel_flag.store(true)`. Strictly set `discard_request = true`, `turn_armed = false`, `sink_occupied = false`, flush remote sink.
- [ ] `core/engine.rs`: Replace all 4 anonymous cancel flags with `Arc::clone(&state.pipeline.hardware_cancel_flag)`:
  - `create_playback_engine` (line 184)
  - `init_audio_stream` (line 417)
  - `spawn_stt_worker` (line 586)
  - `TtsWarmUpHandles` in `warm_up_tts`
- [ ] Sweep direct `turn_cancel_token().cancel()` callers in `error.rs`, `interrupt.rs`, `ptt.rs`, `session.rs` to call `cancel_active_turn()`.

---

## T-03: AudioIntent & TurnPhase Clean Sweep
- [ ] `core/events.rs`: Delete `AudioIntent` enum; remove `intent` field from `PlaybackStarted` and `PlaybackFinished`.
- [ ] `services/audio/mod.rs`: Delete `playback_intent: Arc<AtomicU8>`.
- [ ] `services/tts/providers/`: Remove `intent` parameter from `edge_tts.rs`, `chatterbox_remote.rs`, and provider traits.
- [ ] `pipeline/assistant/playback.rs`: Remove `intent` matches and imports.
- [ ] `harness/chassis.rs`: Delete `TurnPhase` enum and `turn_phase` field.
- [ ] `harness/mod.rs`: Remove `TurnPhase` re-export.
- [ ] `pipeline/assistant/interrupt.rs`: Rewrite `has_committed` check to use `harness.pending_user_query.is_some()`.
- [ ] Sweep evals (`evals/agentic-tool/tts_capture.rs`) and benches (`benches/common/tts_harness.rs`).

---

## T-04: Duplex Router & Activity Preservation
- [ ] `pipeline/router.rs`: Update `transition()` to preserve `current_activity` on `Listening` and `Speaking`; clear only on other 7 states.
- [ ] `pipeline/router.rs`: When `new_state == InteractionState::Working && activity.is_none()`, retain existing `state.pipeline.current_activity()`.
- [ ] `pipeline/router.rs`: Implement `transition_harness_phase` with Epoch/Cancel and Duplex Audio Priority guards.
- [ ] `harness/steps.rs`: Replace raw `transition()` calls with `transition_harness_phase`.
- [ ] `pipeline/assistant/llm.rs::evaluate_synthesis_latch`: Invoke `mark_llm_finished(turn_id)`. Delete `if current_state != InteractionState::Speaking { transition(...) }`. If `!audio_active`, transition to `Ready`; if `audio_active`, do not call `transition(...)`.

---

## T-05: Stage 2 Barge-In Hardening & Cognitive Loop Resumption
- [ ] `harness/mod.rs`: Pass both `turn_cancel` and `stream_cancel` into `TurnExecutionRequest`.
- [ ] `harness/stages/streaming/router.rs` & `harness/loop.rs`: Stream cancel halts token emission without exiting task; parks at `StepInbox::wait_if_speech_held()`. On real barge-in, exit via `step7_handle_cancelled`; on false barge-in, loop back to Step 4.
- [ ] `harness/steps.rs`: In `step7_handle_cancelled`, if `!ctx.partial_text.trim().is_empty()`, commit turn via `harness.commit_completed_turn(ctx.partial_text.clone())`.
- [ ] `pipeline/assistant/session.rs`: In `on_stop_turn`, only discard `pending_user_query` if `assistant_response` in `pipeline_accumulator` is empty.
- [ ] `pipeline/assistant/interrupt.rs`: Stage 1 cancels stream pass and mutes audio; preserves turn token, hardware flag, and activity.
- [ ] `pipeline/assistant/interrupt.rs`: Allocate `utterance_id` on speech onset and send `BindUtterance`.
- [ ] `pipeline/assistant/transcript.rs`: Stage 2 empty transcript re-arms `hardware_cancel_flag = false`, releases speech hold, and restores state.
- [ ] `pipeline/assistant/transcript.rs`: Stage 2 non-empty transcript releases speech hold, finalizes prior turn (`RULE-EVT-08B`), cancels prior turn, stashes Step 0 query via `stash_for_continuation()`, and calls `next_turn()`.
- [ ] `pipeline/assistant/transcript.rs`: Key late-final check on `utterance_id < state.pipeline.peek_utterance_id()`.
- [ ] `pipeline/assistant/text.rs`: If interrupting `Thinking`/`Speaking`/`Working`, call `cancel_active_turn()` and `next_turn()`. Stage query synchronously before spawning task.

---

## T-06: Playback Quiescence Law Completion
- [ ] `pipeline/assistant/playback.rs`: `on_playback_started` accepts `Ready` state and records TTFA milestone.
- [ ] `pipeline/assistant/playback.rs`: `on_playback_finished` narrowed to `Speaking`. In `Thinking`/`Working`, accept only when `llm_done && pending_tts_jobs == 0 && !sink_occupied` (zero-audio fallback).
- [ ] `pipeline/assistant/playback.rs`: Implement Duplex return (`Ready` if `llm_done`, `Working` if activity active, else `Thinking`).
- [ ] `services/audio/playback.rs`: `PlaybackEngine::ingest_chunk_with_threshold` sets `sink_occupied.store(true, Ordering::Release)` before `tts/actor.rs` decrements `pending_tts_jobs`.
- [ ] `services/audio/playback.rs` & `sink.rs`: `cancel()` and buffer drain set `sink_occupied.store(false, Ordering::Release)`.

---

## T-07: Audio Hot-Path Hygiene & Utterance Binding
- [ ] `core/events.rs`: Add `VadCommand::BindUtterance { turn_id, utterance_id }`.
- [ ] `services/vad/actor.rs`: Add `awaiting_utterance_binding: bool` to `VadActorState`. Buffer pre-roll and incoming frames; flush when `BindUtterance` arrives. Purge speculative `wrapping_add(1)` prediction.
- [ ] `services/stt/actor.rs`: Add `utterance_id` to commands; purge `wrapping_add(1)` branches; reset provider on unknown binding.
- [ ] `pipeline/assistant/ptt.rs` & `dictation/ptt.rs`: Allocate `utterance_id` in `on_ptt_start`, pass through validation and `SttCommand::Final`.
- [ ] `pipeline/assistant/speech.rs`: Dispatch `BindUtterance` upon speech onset.
- [ ] `core/engine.rs`: In `SharedRingProducer`, use `try_lock()` on input capture frames, dropping on contention without blocking mutex locks.
- [ ] `services/audio/sink.rs`: Delete string formatting and `log::*` macros from CPAL output callback.

---

## T-08: TTS Lifecycle Hygiene
- [ ] `services/tts/actor.rs` & `core/engine.rs`: Add `pub engine_shutdown: Arc<AtomicBool>` to `TtsWorkerHandles` and `TtsWarmUpHandles`, wiring `Arc::clone(&state.pipeline.engine_shutdown)`.
- [ ] `services/tts/actor.rs`: Worker loop checks `engine_shutdown`.
- [ ] `services/tts/actor.rs`: Retain fallback `PlaybackFinished` emission strictly when `remaining_jobs == 0 && !playback.is_armed() && playback.occupied_len() == 0`.
- [ ] `services/tts/mod.rs`: `cool_down_tts` joins `handles.tts_handle`.
- [ ] Update `TtsWorkerHandles` struct literals in test harnesses.

---

## T-09: Remote Control Protocol
- [ ] `services/remote/session/manager.rs`: Add `ControllerLease` with atomic reservation.
- [ ] `services/remote/session/listener.rs`: `drive_tier2_handshake` reserves lease; returns `LinkRefusedReason::ControllerAttached` if busy.
- [ ] `services/remote/session/dialer.rs`: Remove `delete_paired_device` on `UnknownPeer`; emit `RemoteFailureKind::UnknownPeer`.
- [ ] `services/remote/session/supervisor.rs`: Upgrade `current_pipeline_state` to `current_pipeline_view: RwLock<PipelineView { state, turn_id }>`.
- [ ] `services/remote/audio/controller_runtime.rs`: Replace detached async tasks with `blocking_send` on dedicated VAD thread.
- [ ] `app/src/services/remoteService.ts`: Add `'UnknownPeer'` to union type.

---

## T-10: Cross-Target Verification Gate
- [ ] `./scripts/verify-targets.sh` succeeds.
- [ ] Desktop release Clippy passes with zero warnings.
- [ ] Android cross-target Clippy passes with zero warnings.
- [ ] `./scripts/lint-arch.sh` passes.
- [ ] Full `cargo nextest` suite passes 100% green.
