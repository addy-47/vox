# Pipeline Refactor Checklist: Phase 14

## Batch 0: Core Duplex State Machine, Hardware Cancellation & Staging Hygiene
*Depends on: None (builds on checkpoint commit)*

### `app/src-tauri/src/pipeline/atomics.rs`
- `PipelineAtomics.hardware_cancel_flag`: Add `Arc<AtomicBool>` for low-level C++ thread cancellation.
- `PipelineAtomics.llm_done`: Add `Arc<AtomicBool>` tracking cognitive completion for the active turn.
- `PipelineAtomics.stream_cancel_token`: Add `Arc<parking_lot::Mutex<tokio_util::sync::CancellationToken>>` for in-flight stream pause.
- `PipelineAtomics::cancel_active_turn`: Set `hardware_cancel_flag.store(true)` in lockstep with token cancel.
- `PipelineAtomics::rearm_turn_cancel_token`: Reset `hardware_cancel_flag.store(false)` on token rotation.
- `PipelineAtomics::cancel_active_stream`: Cancel in-flight model stream pass without cancelling turn.
- `PipelineAtomics::rearm_stream_cancel_token`: Rearm stream cancellation token for next pass.
- `PipelineAtomics::next_turn`: Reset `llm_done.store(false)`, rearm `turn_cancel_token`, and rearm `stream_cancel_token`.
- `PipelineAtomics::is_llm_done`: Add getter for atomic `llm_done` flag.
- `PipelineAtomics::set_llm_done`: Add setter for atomic `llm_done` flag.

### `app/src-tauri/src/core/engine.rs`
- `create_playback_engine`: Pass `Arc::clone(&state.pipeline.hardware_cancel_flag)` instead of dummy atomic.
- `spawn_stt_worker`: Pass `Arc::clone(&state.pipeline.hardware_cancel_flag)` instead of dummy atomic.
- `init_audio_stream`: Pass `Arc::clone(&state.pipeline.hardware_cancel_flag)` instead of dummy atomic.

### `app/src-tauri/src/pipeline/router.rs`
- `transition`: Retain `current_activity` when transitioning to `InteractionState::Speaking`.
- `transition_harness_phase`: Implement guarded transition enforcing Epoch/Cancel and Duplex Audio Priority guards.

### `app/src-tauri/src/pipeline/assistant/playback.rs`
- `on_playback_started`: Transition `Thinking` or `Working` to `Speaking` without wiping `current_activity`.
- `on_playback_finished`: Implement Duplex return to `Ready` (if `llm_done`), `Working` (if tool active), or `Thinking`.

### `app/src-tauri/src/pipeline/assistant/llm.rs`
- `on_llm_finished`: Set `state.pipeline.set_llm_done(true)`.
- `evaluate_synthesis_latch`: Transition to `Ready` if audio idle, else remain/transition to `Speaking`.

### `app/src-tauri/src/harness/chassis.rs`
- `TurnPhase`: Delete redundant enum.
- `Harness.turn_phase`: Remove field.
- `Harness::take_staged_query`: Extract `pending_user_query` for continuation without history rollback.
- `Harness::discard_pending_turn`: Discard `pending_user_query` when turn aborted.
- `Harness.has_played_filler`: Remove session-level boolean field.

### `app/src-tauri/src/harness/steps.rs`
- `step1_intake`: Replace raw `transition` call with `transition_harness_phase`.
- `enter_non_terminal_phase`: Replace raw `transition` call with `transition_harness_phase`.
- `step6_await_and_drain_inbox`: Replace raw `transition` call with `transition_harness_phase`.

### `app/src-tauri/src/pipeline/assistant/interrupt.rs` (Stage 1: SpeechStart)
- `on_interrupt`: Mute audio immediately via `playback_engine.cancel()`.
- `on_interrupt`: Cancel in-flight model stream pass via `cancel_active_stream()`.
- `on_interrupt`: Transition immediately to `Listening`.
- `on_interrupt`: Keep `turn_cancel_token` active; do NOT call `next_turn()`.
- Sweep all `.turn_cancel_token().cancel()` call sites across `error.rs`, `interrupt.rs`, `ptt.rs`, and `session.rs` to use `cancel_active_turn()`.

### `app/src-tauri/src/pipeline/assistant/transcript.rs` (Stage 2: TranscriptFinal)
- `on_transcript_final`: On empty transcript, keep turn intact: transition to `Ready` if `llm_done`, `Working` if tool active, or `Thinking` (rearming `stream_cancel_token`).
- `on_transcript_final`: On non-empty transcript, cancel prior turn via `cancel_active_turn()`, check Step 0 query concatenation, allocate `next_turn()`, and dispatch as brand new dialogue turn.

### `app/src-tauri/src/pipeline/assistant/text.rs`
- `on_text_input`: Synchronously call `harness.stage_user_query` on router thread before spawning task.

### `app/src-tauri/src/services/audio/playback.rs`
- `PlaybackEngine::ingest_chunk`: Store `sink_occupied.store(true)`.
- `PlaybackEngine::cancel`: Store `sink_occupied.store(false)`.

### `app/src-tauri/src/pipeline/assistant/playback.rs`
- `on_playback_started`: Record TTFA metrics even if already in `Speaking` state (handles early `LlmFinished`).
- `on_playback_finished`: Add `sink_occupied` early-return check to prevent premature drop while audio plays in sound card.

### `app/src-tauri/src/persistence/steps.rs` & `app/src-tauri/src/harness/steps.rs`
- `persistence/steps.rs`: Persist step `content` and execution metrics into `session_steps`. User utterances are stored strictly in `turns` table.
- `harness/steps.rs` (`step3_execute_compaction`): Pass `turn_id` and `step_id` to `record_compaction_start` so `session_compactions` links directly to the triggering step.

### `app/src-tauri/tests/playback_test.rs`
- `test_playback_finished_transitions_speaking_to_ready`: Emit `VoxEvent::LlmFinished` before asserting `Ready`.
- `test_underrun_recovers_when_next_chunk_arrives`: Emit `VoxEvent::LlmFinished` before asserting `Ready`.
- `test_empty_playback_finished_on_zero_length_audio`: Emit `VoxEvent::LlmFinished` before asserting `Ready`.

### `app/src-tauri/tests/playback/tts_to_playback.rs`
- `tts_synthesis_streams_into_playback_engine_and_drains`: Emit `VoxEvent::LlmFinished` before asserting `Ready`.

---

## Batch 1: Audio Hot-Path Hygiene & Utterance Binding
*Depends on: Batch 0*

### `app/src-tauri/src/core/events.rs`
- `VadCommand::BindUtterance`: Add command carrying `turn_id` and `utterance_id`.

### `app/src-tauri/src/services/vad/actor.rs`
- `VadActorState.binding`: Add `Option<(u32, u32)>` storage for bound IDs.
- `VadActor::handle_command`: On `BindUtterance`, record binding and flush held pre-roll audio.

### `app/src-tauri/src/services/vad/segmenter.rs`
- `VadSegmenter::process_frame`: Delete speculative `wrapping_add(1)` prediction logic.
- `VadSegmenter::resync`: Delete atomic re-sync logic at lines 251–254 and 388–392.

### `app/src-tauri/src/services/stt/actor.rs`
- `SttCommand::StreamChunk`: Add `utterance_id: u32` parameter.
- `SttCommand::Partial`: Add `utterance_id: u32` parameter.
- `SttCommand::Final`: Add `utterance_id: u32` parameter.
- `SttActor::handle_stream`: Remove turn guessing; drop chunks with unknown binding.

### `app/src-tauri/src/pipeline/assistant/speech.rs`
- `on_speech_start`: Send `VadCommand::BindUtterance` with router-allocated IDs.

### `app/src-tauri/src/pipeline/dictation/speech.rs`
- `on_dictation_speech_start`: Send `VadCommand::BindUtterance` with router-allocated IDs.

### `app/src-tauri/src/services/tts/actor.rs`
- Delete lines 132–140 where TTS worker emits duplicate `VoxEvent::PlaybackFinished` (`T-AUDIO-02`).
- Respect `engine_shutdown: Arc<AtomicBool>` in `spawn_tts_worker` loop (`T-AUDIO-03`).
- In `cool_down_tts`, join the persistent worker thread handle stored in `handles.tts_handle` (`T-AUDIO-03`).

### `app/src-tauri/src/services/audio/device.rs`
- `SharedRingProducer`: Remove `Mutex` lock from real-time audio producer callback.
- `SharedRingProducer::push`: Replace `log::warn!` with atomic dropped samples counter.

### `app/src-tauri/src/services/audio/sink.rs`
- `process_output_buffer`: Remove string allocations and `log::*` macros from CPAL output callback.

---

## Batch 2: Remote Control Protocol, Leasing & Trust Decoupling
*Depends on: Batch 0*

### `app/src-tauri/src/services/remote/session/manager.rs`
- `ControllerLease`: Add lease struct `(device_id, lease_id)`.
- `RemoteSessionManager::try_reserve_controller`: Add atomic lease reservation.
- `RemoteSessionManager::release_controller`: Add atomic lease release.

### `app/src-tauri/src/services/remote/session/listener.rs`
- `handle_hello`: Reserve controller lease before generating snapshot; return `LinkRefused` if busy.
- `build_attach_snapshot`: Read canonical `StateChangedPayload` from router cache.

### `app/src-tauri/src/services/remote/session/dialer.rs`
- `handle_unknown_peer`: Remove `delete_paired_device` call on `UnknownPeer` error.
- `RemoteFailureKind::UnknownPeer`: Add typed failure variant.

### `app/src-tauri/src/services/remote/session/supervisor.rs`
- `RemoteSupervisorState.current_pipeline_view`: Replace loose `RwLock<String>` with typed `PipelineView`.
- `RemoteSupervisor::emit_disconnect_state`: Emit `state_changed` with true active `turn_id`.

### `app/src-tauri/src/services/remote/audio/controller_runtime.rs`
- `ControllerAudioWorker::run`: Replace detached `tauri::async_runtime::spawn` tasks with `blocking_send`.

### `app/src/services/remoteService.ts`
- `RemoteFailureKind`: Add `'UnknownPeer'` to TypeScript union type.

---

## Batch 3: Verification & Cross-Target Regression Gate
*Depends on: Batches 0, 1, 2*

### Workspace Verification
- `scripts/lint-arch.sh`: Verify backend architectural layering and forbid raw actor commands in loop/steps.
- `cargo clippy --bin Vox --lib --release --no-default-features -D warnings`: Compile desktop library and binary.
- `cargo clippy --target aarch64-linux-android --lib --release -D warnings -D clippy::disallowed_methods`: Android cross-compile gate.
- `cargo nextest run --release --test-threads=1 --no-fail-fast`: Run full 65+ integration test suite to completion.
