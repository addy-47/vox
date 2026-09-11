# Harness v2 Implementation Checklist

## Batch 1: Core Event Contracts & AudioIntent Gating
*Depends on: None*

- [x] `app/src-tauri/src/core/events.rs`
  - `AudioIntent`: Declare enum `InterimFiller`, `TurnResponse` with `Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq`.
  - `VoxEvent::SessionStart`: Add `session_id: Option<i64>` field.
  - `VoxEvent::PlaybackStarted`: Add `intent: AudioIntent` field.
  - `VoxEvent::PlaybackFinished`: Add `intent: AudioIntent` field.
- [x] `app/src-tauri/src/core/state.rs`
  - `InteractionState`: Add variant `Working = 8`.
  - `From<u32> for InteractionState`: Map `8 => InteractionState::Working`.
  - `From<InteractionState> for u32`: Map `InteractionState::Working => 8`.
- [x] `app/src/services/eventsService.ts`
  - `InteractionState`: Add `"Working"` to TypeScript union.
- [x] `app/src/shared/lib/voiceDisplay.ts`
  - `toMood`: Handle `"Working"` state with an organizing/thinking ambient mood.
- [x] `app/src-tauri/src/services/tts/actor.rs`
  - `TtsCommand::Generate`: Add `intent: AudioIntent` field.
  - `spawn_tts_worker`: Forward `intent` to `synthesize_chunk`.
- [x] `app/src-tauri/src/services/tts/providers/mod.rs` & implementations (`supertonic.rs`, `kokoro.rs`, `chatterbox.rs`, `chatterbox_remote.rs`, `edge_tts.rs`)
  - `synthesize_chunk`: Forward `intent` to `playback.ingest_chunk_with_intent`.
- [x] `app/src-tauri/src/services/audio/playback.rs`
  - `PlaybackEngine`: Add `playback_intent: Arc<AtomicU8>` field and rename `stream` to RAII guard `_stream: Option<cpal::Stream>`.
  - `PlaybackEngine::ingest_chunk_with_intent`: Store `intent` and push chunk with pre-roll check.
  - `PlaybackEngine::flush_pre_roll`: Emit `VoxEvent::PlaybackStarted { turn_id, intent }`.
- [x] `app/src-tauri/src/services/audio/sink.rs`
  - `AudioOutputSink::tick`: Emit `VoxEvent::PlaybackFinished { turn_id, intent }` when buffer drains empty and no jobs remain.
- [x] `app/src-tauri/src/pipeline/router.rs`
  - `spawn_router`: Update match on `SessionStart`, `PlaybackStarted`, and `PlaybackFinished`.
  - `ingestion_gate`: Add `InteractionState::Working` to open condition.
- [x] `app/src-tauri/src/pipeline/assistant/playback.rs`
  - `on_playback_started`: If `AudioIntent::InterimFiller`, remain in `Working`; if `AudioIntent::TurnResponse`, transition to `Speaking`.
  - `on_playback_finished`: If `AudioIntent::InterimFiller`, remain in `Working`; if `AudioIntent::TurnResponse`, transition to `Ready`.

---

## Batch 2: Model Decoupling & Stream Routing Plugin
*Depends on: Batch 1*

- [x] `app/src-tauri/src/services/llm/actor.rs`
  - `LlmResponse`: Declare enum `Token(String)`, `Finished`, `Error(PipelineError)`.
  - `LlmCommand::Generate`: Replace accumulator/tts_tx/pending_jobs with `response_tx: mpsc::Sender<LlmResponse>`.
  - `spawn_llm_worker`: Remove `accumulator`, `tts_tx`, `pending_synthesis_jobs`, and UI IPC emissions.
  - `spawn_llm_worker`: Stream `LlmResponse::Token(token)` and `LlmResponse::Finished` to `response_tx`.
  - `spawn_llm_worker`: Remove emission of `VoxEvent::LlmFinished`.
- [x] `app/src-tauri/src/services/harness/plugins/stream.rs` [NEW]
  - `StreamRoutingPlugin`: Consumes `LlmResponse` stream over duplex pipe.
  - Accumulates tokens into clauses via `TtsClauseChunker`.
  - Dispatches synthesized clauses as `TtsCommand::Generate { turn_id, text, intent: AudioIntent::TurnResponse }`.
  - Emits `IpcEvent::LlmToken` directly to the webview window.
  - On stream finish, flushes remainder to TTS and emits `VoxEvent::LlmFinished { turn_id }`.

---

## Batch 3: Dialog History, Tags & Prompt Assembly
*Depends on: Batch 1, Batch 2*

- [x] `app/src-tauri/src/services/llm/actor.rs`
  - Fully decoupled `spawn_llm_worker` by removing `event_tx` parameter.
  - Added `LlmResponse::Cancelled`.
  - Decomposed worker loop into sub-50 line functions (`handle_warmup`, `handle_generate`, `classify_llm_error`).
  - Removed `pipeline_tx` dependency from `warm_up_llm`.
- [x] `app/src-tauri/src/services/harness/message.rs` [NEW]
  - `Role`: Declare enum `System`, `User`, `Assistant` with Display formatting.
  - `ChatMessage`: Declare struct `role: Role, content: String, timestamp_ms: u64`.
  - `current_timestamp_ms`: Epoch millisecond helper.
- [x] `app/src-tauri/src/services/harness/tags.rs` [NEW]
  - `PromptTag`: Declare enum `UserIdentity`, `SessionContext`, `PastTurns`, `SystemPrompt`, `TaskGuidance` with `open_tag(&self)` and `close_tag(&self)`.
- [x] `app/src-tauri/src/services/harness/filler.rs` [NEW]
  - `TRANSITION_MESSAGES_EN`: 10 English filler phrases.
  - `TRANSITION_MESSAGES_HI`: 10 Hindi filler phrases.
  - `select_filler_phrase`: Selects localized filler phrase using `services::translit::is_devanagari`.
- [x] `app/src-tauri/src/services/harness/plugins/history.rs` [NEW]
  - `ConversationHistoryPlugin`: In-memory message vector, KV-cache index tracking, user deduplication, interruption rollback, and assistant turn append.
- [x] `app/src-tauri/src/services/harness/plugins/prompt.rs` [NEW]
  - `PromptBuilderPlugin`: Assembles base persona with `<user_identity>` markdown document and enforces 20% budget share ceiling.
- [x] `app/src-tauri/src/services/harness/plugins/mod.rs` [NEW]
  - Plugin module declarations and re-exports.
- [x] `app/src-tauri/src/services/harness/` (Clean-slate deletion of legacy files)
  - Delete `accountant.rs`.
  - Delete `buffer.rs`.
  - Delete `facade.rs`.
  - Delete `manager.rs`.
  - Delete `prompt_builder.rs`.

---

## Batch 4: Context Budgeting, Compaction & Reactive Quiet Watcher
*Depends on: Batch 1, Batch 2, Batch 3*

- [x] `app/src-tauri/src/services/harness/plugins/budget.rs` [NEW]
  - `ContextBudgetPlugin`: Real-time token accountant, usable budget calculation `(max_tokens - reserved_generation)`, 65% soft / 85% critical threshold classification, FIFO sliding window shift.
- [x] `app/src-tauri/src/services/harness/plugins/compaction.rs` [NEW]
  - `CompactionPlugin`: Structured compaction execution via LLM duplex pipe (2 attempts, 45s timeout), fact extraction, Turso ledger staging, and reactive 20s debounced quiet watcher.
- [x] `app/src-tauri/src/lib.rs`
  - Remove `spawn_state_compaction_observer` import and background task invocation.

---

## Batch 5: HarnessSession Chassis Assembly & 1:1 Lifecycle Integration
*Depends on: Batches 1, 2, 3, 4*

- [x] `app/src-tauri/src/services/harness/session.rs` [NEW]
  - `HarnessSession`: Orchestrates plugins, duplex dialogue pipe, `prepare_turn`, and constructors `new_fresh`, `new_continuing`, `new_realtime`.
- [x] `app/src-tauri/src/services/harness/mod.rs`
  - Subsystem root, exports `HarnessSession`, `ChatMessage`, `Role`, `PromptTag`, filler constants.
- [x] `app/src-tauri/src/core/state.rs`
  - `AppState`: Replace `pub conversation_manager` with `pub harness: Arc<parking_lot::Mutex<Option<HarnessSession>>>`.
- [x] `app/src-tauri/src/ipc/pipeline.rs`
  - `start_session`: Accept `session_id: Option<i64>` and route `VoxEvent::SessionStart { owner: Assistant, session_id }`.
- [x] `app/src-tauri/src/ipc/persistence.rs`
  - `continue_session`: Read metadata and turns for UI display only without modifying `HarnessSession`.
  - `create_session`: Clear UI selection and reset `conversation_id = 0` without modifying `HarnessSession`.
- [x] `app/src-tauri/src/ipc/memory.rs` & `services/memory/scheduler.rs`
  - Update `save_personal_memory`, `consolidate_personal_memory`, `import_personal_memory` to update active `state.harness` on the fly if mounted.
- [x] `app/src-tauri/src/pipeline/assistant/session.rs`
  - `on_session_start`: Boot `HarnessSession` (`session_id: Option<i64>`), set up duplex pipe, assign to `state.harness`, transition to `Ready`.
  - `on_end`: Drop `state.harness.lock().take()`, cancel session-scoped tokens, transition to `Idle`.
- [x] `app/src-tauri/src/pipeline/assistant/transcript.rs`
  - `on_transcript_final`: Route turn query to `HarnessSession::prepare_turn`; handle inline critical compaction and filler dispatch.
- [x] `app/src-tauri/src/pipeline/assistant/interrupt.rs`
  - `on_interrupt`: Extract partial turn from `state.harness`, push to history, dispatch persistence event.
- [x] `app/src-tauri/src/pipeline/assistant/llm.rs`
  - `on_llm_finished`: Handle pre-roll flush and audio playback completion guard.
- [x] `app/src-tauri/src/pipeline/lifecycle.rs`
  - Prune obsolete sync helpers replaced by `HarnessSession`.

---

## Batch 6: Hardening, CompactionPlugin Ledger Staging, Watcher Wiring & Test Alignment
*Depends on: Batches 1, 2, 3, 4, 5*

- [x] `app/src-tauri/src/services/harness/plugins/compaction.rs`
  - Encapsulate inline & soft compaction execution and Turso DB persistence (`record_compaction_start` + `commit_compaction_output`) inside `CompactionPlugin`.
  - Wire `QuietCompactionWatcher` into `CompactionPlugin`, managing the 20s debounce timer.
- [x] `app/src-tauri/src/services/harness/session.rs`
  - Expose compaction lifecycle handles (`execute_inline_compaction`, `on_turn_completed`, `abort_watcher`) on `HarnessSession`.
- [x] `app/src-tauri/src/pipeline/assistant/transcript.rs`
  - Call explicit `transition(InteractionState::Working, ctx, app, state)` when inline compaction triggers.
  - Delegate inline compaction and DB fact staging to `HarnessSession` / `CompactionPlugin`, slimming down `transcript.rs`.
  - Resolve engine channels asynchronously via `state.engine.lock().await` inside spawned task instead of synchronous `try_lock()`.
- [x] `app/src-tauri/src/pipeline/assistant/speech.rs`
  - Add `InteractionState::Working` to barge-in check (`if current_state == Thinking || Speaking || Working => on_interrupt(...)`).
  - Abort quiet compaction watcher on speech onset.
- [x] `app/src-tauri/src/pipeline/assistant/ptt.rs`
  - Add `InteractionState::Working` to barge-in check (`if current_state == Thinking || Speaking || Working => on_interrupt(...)`).
- [x] `app/src-tauri/src/pipeline/assistant/playback.rs`
  - Trigger `watcher.on_turn_completed(...)` when `on_playback_finished` transitions to `Ready`.
- [x] `app/src-tauri/src/pipeline/assistant/interrupt.rs`
  - Abort quiet compaction watcher on interrupt.
  - Guard against consecutive `User` turns when `partial_assistant` is empty.
- [x] `app/src-tauri/tests/session_lifecycle_test.rs`
  - Migrate `conversation_manager` references to `state.harness.lock().as_ref().unwrap().prompt.assemble()`.
- [x] `app/src-tauri/tests/tts_transition_test.rs`
  - Migrate deleted `facade::prepare_turn_context` calls to `HarnessSession::prepare_turn` with `TurnPreparation::NeedsInlineCompaction`.
- [x] `app/src-tauri/tests/transcript_to_llm_test.rs`
  - Migrate `conversation_manager` references to `state.harness.lock().as_mut().unwrap().history`.
- [x] Verification: `cargo clippy --all-targets -- -D warnings` and `pnpm build` pass with 0 errors.

---

## Batch 7: Two-Front-Door Encapsulation, Audit Remediation & Dead Code Pruning
*Depends on: Batches 1, 2, 3, 4, 5, 6*

- [ ] `app/src-tauri/src/services/harness/plugins/prompt.rs`
  - Replace byte slice with `memory.floor_char_boundary(char_limit)` to prevent UTF-8 boundary panics.
  - Prune unused setters (`set_base_system_prompt`, `set_max_context_tokens`, `set_max_context_share`).
- [ ] `app/src-tauri/src/services/harness/plugins/compaction.rs`
  - Consolidate `<session_context>` into root `Role::System` prompt to prevent consecutive system messages.
  - Track `last_compacted_to_turn` to populate non-zero `from_turn_id`.
  - Prune unused setter `set_auto_compaction_enabled`.
- [ ] `app/src-tauri/src/services/harness/session.rs`
  - Make all 6 plugin fields private (`history`, `prompt`, `budget`, `compaction`, `stream`, `watcher`).
  - Implement `apply_quiet_compaction_summary(&mut self, summary: &str)` to drain and rebuild `self.history`.
  - Implement `commit_turn`, `rollback_user_turn`, `assembled_system_prompt`, and `generation_options`.
  - Prune unused methods (`set_session_id`, `abort_session`). Preserve `new_realtime`.
- [ ] `app/src-tauri/src/services/harness/watcher.rs`
  - Make watcher autonomous: listen to `state_rx` and `turn_token()`, aborting automatically on state change.
  - In `execute_soft_compaction`, invoke `harness.apply_quiet_compaction_summary`.
- [ ] `app/src-tauri/src/pipeline/assistant/transcript.rs`
  - Offload `stream_plugin.route_stream` via `tokio::task::spawn_blocking`.
  - Wire `harness.commit_turn` / `harness.rollback_user_turn` on stream termination.
  - Trigger `harness.on_turn_completed` directly upon turn completion.
  - Restore `options: harness.generation_options().clone()` in inline compaction branch.
- [ ] `app/src-tauri/src/pipeline/assistant/{speech.rs, ptt.rs, interrupt.rs, playback.rs, llm.rs}`
  - Remove all `state.harness.lock()` calls (aborting watcher, manual turn push/rollback, and redundant remainder flush).
- [ ] `app/src-tauri/src/ipc/{persistence.rs, memory.rs}` & `services/memory/scheduler.rs`
  - Remove all direct `state.harness.lock()` calls, establishing strict Two-Front-Door boundary (`session.rs` and `transcript.rs` only).
- [ ] `app/src-tauri/src/services/memory/compaction/prompt.rs`
  - Hardcode constant `DEFAULT_LLM_COMPACTION_TEMPERATURE = 0.2` in `build_compaction_request`.
- [ ] Dead code pruning & script fix:
  - Prune `with_timestamp`, `set_kv_synced_index`, `rollback_last_assistant_turn`, `set_max_context_tokens`, `subscribe_dictation_state`, `get_history`, `send_pcm`.
  - Fix `find_dead_code.py:104` pointer dereference `*` line skip bug.
- [ ] Verification: `cargo clippy --all-targets -- -D warnings`, `pnpm build`, 0 false positives on `find_dead_code.py`, and `git grep "state.harness"` matches only `session.rs` and `transcript.rs`.
