# System Architect Review Fixes Checklist

## Batch 1: Stream Error Handling, Disconnect Protection, and Demuxer Wiring (Items 1, 2)
- [ ] 1.1 Wire `StreamingTagDemuxer` inside `services/harness/stages/streaming/router.rs`
- [ ] 1.2 Return `Err` on `LlmResponse::Error` in `router.rs` instead of `Ok(partial)`
- [ ] 1.3 Track `saw_finished` in `route_stream` and return `Err` on premature channel disconnect
- [ ] 1.4 Flush demuxer tail at `Finished` and verify user turn rollback in `orchestrator.rs`
- [ ] 1.5 Run `cargo check` to verify Batch 1

## Batch 2: Concurrency, Cancellation Cleanup & Compaction Isolation (Items 3, 4, 7, 8)
- [ ] 2.1 Store `JoinHandle` of cancellation bridge task in `orchestrator.rs` and abort on turn exit
- [ ] 2.2 Snapshot `llm_settings` from `AppState` and pass `Some(&llm_settings)` in `orchestrator.rs`
- [ ] 2.3 Isolate inline compaction DB connection and execution in `tokio::task::spawn_blocking`
- [ ] 2.4 Add degraded fallback FIFO shift and warning event when inline compaction yields empty summary
- [ ] 2.5 Run `cargo check` to verify Batch 2

## Batch 3: Compaction Window Logic & Counter Wrap Defenses (Items 5, 6)
- [ ] 3.1 Update `from_turn_id` in `compaction.rs` to return `self.last_compacted_to_turn.saturating_add(1)`
- [ ] 3.2 Guard `apply_session_context` in `compaction.rs` against empty strings
- [ ] 3.3 Replace bare `fetch_sub(1)` with `fetch_update(saturating_sub)` in `router.rs` remainder path
- [ ] 3.4 Add saturating decrement compensation if filler dispatch fails in `orchestrator.rs`
- [ ] 3.5 Run `cargo check` to verify Batch 3

## Batch 4: Clean Encapsulation, Drop Implementation & TTS Line Budget (Items 9, 10)
- [ ] 4.1 Delete deprecated `TurnPreparation` enum and `prepare_turn` from `orchestrator.rs`
- [ ] 4.2 Delete duplicate `clone_stream_plugin` alias and collapse identical quiet session context methods
- [ ] 4.3 Change `TurnExecutionRequest.query` from `&'a str` to owned `String`
- [ ] 4.4 Implement `Drop for Harness` to abort quiet watcher and cancel `session_cancel`
- [ ] 4.5 Migrate `tests/llm_to_tts_test.rs` to drive `Harness::execute_turn`
- [ ] 4.6 Trim `services/tts/actor.rs` under 180 lines by compacting logging boilerplate
- [ ] 4.7 Run `cargo check` to verify Batch 4

## Batch 5: Final Validation & Ledger Sync
- [ ] 5.1 Run `cargo clippy --all-targets` (must produce 0 warnings and 0 errors)
- [ ] 5.2 Append summary to `AGENTS.md` Section 5
- [ ] 5.3 Copy artifacts to `docs/plans/phase11/harness_review_fixes_plan.md` and `checklist.md`
