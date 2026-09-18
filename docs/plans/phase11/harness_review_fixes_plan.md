# Implementation Plan — System Architect Review: Harness & Pipeline Decoupling Fixes

## 1. Executive Summary

An adversarial Senior System Architect review of the recent Harness refactor, duplex dialogue pipeline, and TTS decoupling identified 10 high-severity reliability defects and architectural regressions:
1. **Stream Error/Disconnect Commits Partial Turn as Completed:** `router.rs` returns `Ok(partial)` on `LlmResponse::Error` and on stream disconnect, causing `orchestrator.rs` to commit truncated text to history and DB as a successful turn.
2. **StreamingTagDemuxer Unwired:** Demuxer is exported with a 64-token ceiling but bypassed by `router.rs`, allowing raw leading control tags (`<think>`, `<tool>`) to leak into frontend subtitles and TTS chunking.
3. **No Executor Isolation for Inline Compaction:** Compaction runs inline on the adapter Tokio task; model compute risks stalling reactor threads.
4. **Detached Cancellation Task Leak:** A detached `cancelled().await -> store(true)` task is spawned every turn and never joined or aborted.
5. **Compaction Window Off-By-One & Empty Summary Context Wipe:** `from_turn_id` returns `last_compacted_to_turn` (should be `+1`), and empty context strings unconditionally overwrite good context to `Some("")`.
6. **`pending_synthesis_jobs` Underflow/Wrap Hazard:** Remainder path uses bare `fetch_sub(1)` and filler dispatch has no compensation on send failure, risking counter wrap to `u32::MAX` on interrupt.
7. **Inline Compaction Drops `llm_settings`:** Passes `None`, bypassing user model options and JsonSchema enforcement.
8. **Empty-Compaction Proceeds Over-Budget Without FIFO:** Failing to compact leaves history over budget without falling back to FIFO shift.
9. **Single-Orchestrator Encapsulation Pierced:** Dead `prepare_turn` and `TurnPreparation` paths persist, allowing tests to bypass `execute_turn`.
10. **TTS Actor Line Budget Miss:** `services/tts/actor.rs` exceeds the approved <180 LOC ceiling (currently 230 LOC).

This plan fixes all 10 items in disciplined, verifiable batches.

---

## 2. Detailed Technical Fixes

### Batch 1: Stream Error Handling, Disconnect Protection, and Demuxer Wiring (Items 1, 2)
- **`services/harness/stages/streaming/router.rs`:**
  - Own a `StreamingTagDemuxer` instance within `route_stream`.
  - Process every incoming token via `demux.process_token(&token)`. Only emit IPC tokens and dispatch to `handles.accumulator.lock().push_token(&clean_token)` when resolved tokens are yielded.
  - On `LlmResponse::Error(err)`:
    - Send `VoxEvent::Error(err)` to `handles.event_tx`.
    - Return `Err(format!("Stream error received: {:?}", err))` immediately.
  - On loop exit:
    - Track `let mut saw_finished = false;`. Set to `true` on `LlmResponse::Finished`.
    - If `!saw_finished` and `!handles.cancel.load(...)`, return `Err("Stream disconnected prematurely".to_string())`.
    - Flush demuxer tail with `demux.flush()` and flush remainder chunker.
- **`services/harness/orchestrator.rs`:**
  - Verify that `Ok(Err(err))` triggers `harness.history.rollback_last_user_turn()` and returns `TurnOutcome::Error`.

### Batch 2: Concurrency, Cancellation Cleanup & Compaction Isolation (Items 3, 4, 7, 8)
- **`services/harness/orchestrator.rs`:**
  - **Cancel Task Leak (Item 4):**
    - Store the `JoinHandle` of the cancellation bridge: `let cancel_bridge = tauri::async_runtime::spawn(...)`.
    - Ensure `cancel_bridge.abort()` is called on all exit branches of `execute_turn`.
  - **Inline Compaction Settings & Fallback (Items 7, 8):**
    - Read `req.app_state.settings.read()?.llm` snapshot and pass `Some(&llm_settings)` to `CompactionParams`.
    - In `CompactionStage::run_and_persist`: wrap DB connection and execution in `tokio::task::spawn_blocking` (or ensure async reactor isolation).
    - If `result.session_context.trim().is_empty()`, log warning, emit `VoxEvent::Error(PipelineError::Harness(...))` degraded warning, and call `harness.fallback_fifo_shift()`.

### Batch 3: Compaction Window Logic & Counter Wrap Defenses (Items 5, 6)
- **`services/harness/stages/compaction.rs`:**
  - In `from_turn_id(&self) -> u32`: return `self.last_compacted_to_turn.saturating_add(1)`.
  - In `apply_session_context(&mut self, context: &str)`: guard with `if !context.trim().is_empty() { self.session_context = Some(context.to_string()); }`.
  - Clean up unused `is_embedded` field.
- **`services/harness/stages/streaming/router.rs`:**
  - In `flush_remainder`: replace bare `handles.pending_synthesis_jobs.fetch_sub(1, ...)` with saturating decrement `fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(v.saturating_sub(1)))`.
- **`services/harness/orchestrator.rs`:**
  - In filler dispatch: if `tts_tx.send(...)` fails, perform saturating decrement compensation on `req.pending_synthesis_jobs`.

### Batch 4: Clean Encapsulation, Drop Implementation & TTS Line Budget (Items 9, 10)
- **`services/harness/orchestrator.rs`:**
  - Remove deprecated `TurnPreparation` enum and `prepare_turn` method.
  - Remove duplicate alias `clone_stream_plugin`.
  - Collapse `apply_quiet_compaction_summary` and `apply_quiet_session_context`.
  - Change `TurnExecutionRequest.query` from `&'a str` to `String` (removes lifetime from public API).
  - Implement `Drop for Harness` to abort the quiet compaction watcher and cancel `session_cancel`.
- **`tests/llm_to_tts_test.rs`:**
  - Migrate tests calling `prepare_turn` to drive `Harness::execute_turn`.
- **`services/tts/actor.rs`:**
  - Refactor verbose trace logging and duplicate match patterns into compact private helpers to bring the file from 230 LOC down to <180 LOC.

### Batch 5: Workspace-Wide Verification & Gate
- Run `cargo clippy --all-targets` (must exit 0 with 0 errors and 0 warnings).
- Update `AGENTS.md` Section 5.
- Update phase 11 plan artifacts.

---

## 3. Verification & Safety Invariants
- Audio hot path: zero allocations, zero lock acquisitions.
- Actor-engine separation: intact.
- Compilation: `cargo check` after each batch; `cargo clippy --all-targets` on completion.
