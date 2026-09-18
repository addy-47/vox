# Checklist — Vox LLM Agent Harness Runtime Refactor (Production LLD)

## Batch 0: Scaffolding & Scaffolding Types
- **Prior Dependencies**: None.
- **Build Health**: Expected to stay GREEN throughout.

### Files & Symbols Touched
- `app/src-tauri/src/services/harness/mod.rs` [MODIFY]
  - Add module declarations: `pub mod stages;`, `pub mod streaming;`.
  - Add types: `TurnOutcome` (`Completed`, `DuplicateIgnored`, `Cancelled`, `Error`).
  - Add types: `TurnExecutionRequest<'a, R: tauri::Runtime>`.
  - Add types: `ContextStatus` (`Nominal`, `SoftWarning`, `Critical`).
- `app/src-tauri/src/services/harness/stages/mod.rs` [NEW]
  - Module scaffold file with exports.
- `app/src-tauri/src/services/harness/streaming/mod.rs` [NEW]
  - Module scaffold file with exports.

---

## Batch 1: Audio Layer Boundary Decoupling (TTS Actor & Chunker)
- **Prior Dependencies**: Batch 0.
- **Build Health**: Expected to stay GREEN throughout.

### Files & Symbols Touched
- `app/src-tauri/src/services/harness/streaming/chunker.rs` [NEW]
  - `ClauseChunker`: Port `TtsClauseChunker` byte-identically from `services/tts/actor.rs:333-490`.
  - `is_abbreviation`: Port honorific and conversational abbreviation lookup.
  - Tests: Port all 8 inline unit tests verifying clause thresholds `(5,8,12)/(10,15,20)/(16,24,32)`, punctuation splitting, and prosody morphing (`.` $\to$ `,`).
- `app/src-tauri/src/services/harness/streaming/mod.rs` [MODIFY]
  - Export: `pub mod chunker;` and `pub use chunker::ClauseChunker;`.
- `app/src-tauri/src/services/tts/factory.rs` [NEW]
  - `create_tts_provider`: Extract engine initialization from `services/tts/actor.rs:200-267`.
  - `resolve_reference_audio`: Extract helper from `services/tts/actor.rs`.
- `app/src-tauri/src/services/tts/actor.rs` [MODIFY]
  - Delete `TtsClauseChunker` struct definition, implementation, and inline unit tests (~320 lines).
  - Delete `create_tts_provider` and `resolve_reference_audio` (delegated to `factory.rs`).
  - Update `pending_synthesis_jobs` decrement: replace bare `fetch_sub` with saturating `fetch_update` or guard `previous > 0` to prevent `u32` wrap.
  - Retain `TtsCommand`, `TtsWorkerHandles`, `spawn_tts_worker`, `TtsWarmUpHandles`, `warm_up_tts`, `cool_down_tts` (reduces to $<180$ lines).
- `app/src-tauri/src/services/tts/mod.rs` [MODIFY]
  - Module declaration: `pub mod factory;` and re-export `factory::create_tts_provider;`.
  - Export: Remove `TtsClauseChunker`.
- `app/src-tauri/src/pipeline/assistant/accumulator.rs` [MODIFY]
  - Import: Replace `use crate::services::tts::actor::TtsClauseChunker;` with `use crate::services::harness::streaming::ClauseChunker;`.
  - Field: Update `TurnAccumulator.chunker: ClauseChunker`.
- `app/src-tauri/tests/chunking_determinism_test.rs` [MODIFY]
  - Import: Update from `services::tts::actor::TtsClauseChunker` to `vox_lib::services::harness::streaming::ClauseChunker`.

---

## Batch 2: Pure Stages Implementation
- **Prior Dependencies**: Batch 0, Batch 1.
- **Build Health**: Expected to stay GREEN throughout.

### Files & Symbols Touched
- `app/src-tauri/src/services/harness/stages/history.rs` [NEW]
  - `ConversationHistoryStage`: Port from `plugins/history.rs` (`messages: Vec<ChatMessage>`, `kv_synced_index: usize`).
  - Methods: `new`, `with_system_prompt`, `messages`, `kv_synced_index`, `is_duplicate_user_turn`, `push_user_turn`, `push_assistant_turn`, `rollback_last_user_turn`, `reset`, `clear`, `truncate_oldest_turns`.
- `app/src-tauri/src/services/harness/stages/prompt.rs` [NEW]
  - `PromptBuilderStage`: Port from `plugins/prompt.rs`.
  - Methods: `new`, `set_personal_memory`, `assemble`, `bound_personal_memory`.
  - Method: `build_generation_request(&self, history: &[ChatMessage], query: &str, options: GenerationOptions) -> GenerationRequest` (constructs payload without mutating history).
- `app/src-tauri/src/services/harness/stages/budget.rs` [NEW]
  - `ContextBudgetStage`: Port from `plugins/budget.rs`.
  - Methods: `new`, `usable_budget`, `calculate_tracked_tokens`, `evaluate_utilization`.
  - Method: `execute_fifo_shift(&self, history: &mut ConversationHistoryStage) -> usize` (drops turn pairs from index 1 until context $<65\%$).
- `app/src-tauri/src/services/harness/stages/compaction.rs` [NEW]
  - `CompactionStage`: Port from `plugins/compaction.rs` (delete `is_embedded` field).
  - Types: `CompactionParams`.
  - Methods: `new`, `session_context`, `set_session_context`, `auto_compaction_enabled`, `from_turn_id`, `set_last_compacted_to_turn`, `can_perform_inline_compaction`, `apply_session_context`, `prune_history_with_summary`, `run_and_persist`.
  - Invariant: Zero-retry fail-fast enforcement (`Err` returned immediately on parse or provider failure).
- `app/src-tauri/src/services/harness/stages/mod.rs` [MODIFY]
  - Re-exports: `history::ConversationHistoryStage`, `prompt::PromptBuilderStage`, `budget::{ContextBudgetStage, ContextStatus}`, `compaction::{CompactionParams, CompactionStage}`.

---

## Batch 3: Egress Stream Router & Demuxer
- **Prior Dependencies**: Batch 1, Batch 2.
- **Build Health**: Expected to stay GREEN throughout.

### Files & Symbols Touched
- `app/src-tauri/src/services/harness/streaming/demuxer.rs` [NEW]
  - `DEMUXER_MAX_SPECULATIVE_TOKENS = 64`.
  - `DemuxerAction`: Enum (`Buffer`, `FlushRaw(String)`, `EmitSpeakable(String)`).
  - `StreamingTagDemuxer`: Struct (`speculative: String`, `token_count: usize`, `resolved: bool`).
  - Methods: `new`, `push_token`, `flush`, `is_resolved`.
- `app/src-tauri/src/services/harness/streaming/router.rs` [NEW]
  - `StreamRoutingHandles`: Struct (`turn_id`, `owner`, `accumulator`, `tts_tx`, `pending_synthesis_jobs`, `cancel: CancellationToken`, `event_tx`, `app`).
  - `StreamRouter`: Struct managing duplex channel consumption.
  - Methods: `new`, `route_stream`, `handle_token`, `dispatch_clauses`, `flush_remainder`, `emit_finished`.
  - Invariants: Checks `cancel.is_cancelled()` per iteration; disconnect returns `Err`; emits `VoxEvent::LlmFinished` strictly on success with non-empty text; feeds tokens through `StreamingTagDemuxer` before subtitle emit or chunker push.
- `app/src-tauri/src/services/harness/streaming/mod.rs` [MODIFY]
  - Re-exports: `router::{StreamRouter, StreamRoutingHandles}`, `demuxer::StreamingTagDemuxer`, `chunker::ClauseChunker`.

---

## Batch 4: Orchestrator Assembly & execute_turn
- **Prior Dependencies**: Batch 2, Batch 3.
- **Build Health**: RED in-flight, GREEN at batch completion.

### Files & Symbols Touched
- `app/src-tauri/src/services/harness/orchestrator.rs` [NEW]
  - `PipelineDomain`: Enum (`ModularAssistant`, `RealtimeS2S`, `Dictation`).
  - `Harness`: Struct owning `session_id`, `domain`, `session_cancel`, `llm_tx`, `history`, `prompt`, `budget`, `compaction`, `generation_options`, `watcher`.
  - Methods: `new_modular`, `new_realtime`, `session_id`, `domain`, `seed_continuation`, `on_turn_completed`, `abort_watcher`, `update_personal_memory`.
  - Method: `execute_turn<R: tauri::Runtime + 'static>(&mut self, req: TurnExecutionRequest<'_, R>) -> TurnOutcome` (implements the 7-phase turn lifecycle).
  - Trait `Drop for Harness`: Aborts active debounce watcher via `session_cancel`.
- `app/src-tauri/src/services/harness/watcher.rs` [MODIFY]
  - Retarget imports: `HarnessSession` $\to$ `Harness`, `plugins/compaction` $\to$ `stages/compaction`.
- `app/src-tauri/src/services/harness/mod.rs` [MODIFY]
  - Module declarations: `pub mod orchestrator;`, `pub mod watcher;`.
  - Re-exports: `orchestrator::{Harness, PipelineDomain, TurnExecutionRequest, TurnOutcome}`, `watcher::QuietCompactionWatcher`.

---

## Batch 5: Pipeline Adapter Cutover & Legacy Pruning
- **Prior Dependencies**: Batch 4.
- **Build Health**: Expected to be GREEN at batch completion.

### Files & Symbols Touched
- `app/src-tauri/src/services/harness/session.rs` [DELETE]
- `app/src-tauri/src/services/harness/plugins/` [DELETE DIRECTORY & ALL 6 FILES]
  - Delete `budget.rs`, `compaction.rs`, `history.rs`, `mod.rs`, `prompt.rs`, and `stream.rs`.
- `app/src-tauri/src/pipeline/assistant/transcript.rs` [MODIFY]
  - `spawn_modular_llm_task`: Delete lines 93-270 of manual orchestration.
  - Snapshot lightweight handles, construct `TurnExecutionRequest`, and invoke `harness.execute_turn(req).await`.
  - Map `TurnOutcome` (`Completed`, `DuplicateIgnored`, `Cancelled`, `Error`) to pipeline state transitions.
- `app/src-tauri/src/pipeline/assistant/llm.rs` [MODIFY]
  - `on_llm_finished`: Expand guard from `state != Thinking && state != Speaking` to `state != Thinking && state != Speaking && state != Working`.
- `app/src-tauri/src/core/state.rs` [MODIFY]
  - Replace `HarnessSession` with `Harness`.
  - `AppState.harness: Arc<parking_lot::Mutex<Option<Harness>>>`.
- `app/src-tauri/src/pipeline/assistant/session.rs` [MODIFY]
  - Update imports: `services::harness::Harness`.
  - `on_session_start`: Call `Harness::new_modular` and `Harness::new_realtime`.
- `app/src-tauri/src/pipeline/test.rs` [MODIFY]
  - Replace `HarnessSession` with `Harness`.
- `app/src-tauri/tests/session_lifecycle_test.rs` [MODIFY]
  - Replace `vox_lib::services::harness::HarnessSession::new_realtime` with `vox_lib::services::harness::Harness::new_realtime`.
