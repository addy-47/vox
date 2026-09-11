# Implementation Plan — Plugin-Based LLM Agent Harness Runtime (v2)

Translate the approved [LLM Agent Harness & Plugin Runtime Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md) into code. This refactor cleanly replaces the legacy `services/harness/` subsystem (over 1,600 lines) with a modular 5-plugin agent chassis (`ConversationHistoryPlugin`, `PromptBuilderPlugin`, `ContextBudgetPlugin`, `CompactionPlugin`, `StreamRoutingPlugin`), decouples `services/llm/actor.rs` into a pure stateless token engine, introduces `InteractionState::Working` and `AudioIntent` (defined directly in `core/events.rs`), establishes the 1:1 session lifecycle (`start_session(Option<sessionId>)`), and eliminates ambient background polling loops.

---

## User Review Required

> [!IMPORTANT]
> **Scope & Role Boundary Alignment**:
> 1. **`AudioIntent` Placement**: Defined directly in [core/events.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/events.rs) alongside pipeline event payloads.
> 2. **Test Suite Scope**: Authoring and updating integration test suites is strictly out of scope for this backend implementation plan; it is deferred to the dedicated **test-engineer** role. Backend verification within these batches is strictly scoped to fast syntax/build/type checks (`cargo check --release`, `cargo clippy`, `pnpm build`).
> 3. **Batch Sizing & Subdivision**: The former large harness batch is divided into 3 balanced, bite-sized batches:
>    - **Batch 2**: Decouple `LlmActor` & Author `StreamRoutingPlugin`.
>    - **Batch 3**: Harness Core Messages, Tags, Filler Catalogs, `ConversationHistoryPlugin`, and `PromptBuilderPlugin`.
>    - **Batch 4**: `ContextBudgetPlugin`, `CompactionPlugin`, and Reactive Soft Compaction Watcher.
>    - **Batch 5**: `HarnessSession` Chassis Assembly & Pipeline 1:1 Lifecycle Integration.

---

## Specification Cross-Reference Matrix (Full Coverage Audit)

Every section, invariant, and threshold from [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md) is mapped to its implementing batch:

| Spec Section | Topic & Mandate | Target Subsystem / File | Mapped Batch |
| :--- | :--- | :--- | :---: |
| **§1.1 – 1.2** | Clean-slate deletion of 5 legacy harness files | `services/harness/` | **Batch 3** |
| **§2.1 – 2.2** | Agent = Model + Harness; Spatial & Temporal composability | `services/llm/actor.rs` & `services/harness/session.rs` | **Batch 2 & 5** |
| **§3.1** | 1:1 Lifecycle mapping (`session_start` to `session_end`) | `core/state.rs`, `pipeline/assistant/session.rs` | **Batch 5** |
| **§3.2** | `continue_session` (UI fetch only) vs `start_session(Option<id>)` | `ipc/persistence.rs`, `ipc/pipeline.rs` | **Batch 5** |
| **§4.1** | Harness as cognitive front door; sole emitter of `LlmFinished` | `StreamRoutingPlugin`, `pipeline/assistant/transcript.rs` | **Batch 2 & 5** |
| **§4.2** | Duplex Dialogue Pipe (`LlmCommand::Generate` with `response_tx`) | `services/llm/actor.rs`, `HarnessSession` | **Batch 2 & 5** |
| **§5.1** | `InteractionState::Working = 8` state invariant | `core/state.rs`, `eventsService.ts`, `voiceDisplay.ts` | **Batch 1** |
| **§5.2** | `AudioIntent::InterimFiller` vs `AudioIntent::TurnResponse` | `core/events.rs`, `services/tts/actor.rs` | **Batch 1** |
| **§5.3** | Playback Engine Gating (InterimFiller locks Working, no Speaking/Ready) | `services/audio/playback.rs`, `pipeline/assistant/playback.rs` | **Batch 1** |
| **§6.1** | Token accounting formula `(tokens / (max - reserved_gen))`, 20% system share | `ContextBudgetPlugin`, `PromptBuilderPlugin` | **Batch 3 & 4** |
| **§6.2** | Nominal path (< 85% utilization) token routing | `HarnessSession::prepare_turn`, `StreamRoutingPlugin` | **Batch 2 & 5** |
| **§6.3** | Critical inline compaction (utilization $\ge 85\%$): Devanagari detection, filler dispatch $\to$ `Working`, 2-attempt LLM summarization (45s timeout), fact staging, buffer rebuild | `CompactionPlugin`, `filler.rs`, `pipeline/assistant/transcript.rs` | **Batch 3, 4 & 5** |
| **§6.4** | Degraded FIFO fallback (context $\le 4096$, history $\le 3$, or retries fail) | `ContextBudgetPlugin`, `CompactionPlugin` | **Batch 4** |
| **§6.5** | Reactive opportunistic soft compaction (65%–85%): 20s quiet debounce timer, zero idle polling loops | `CompactionPlugin`, removal from `lib.rs` | **Batch 4 & 5** |
| **§7.1** | 5-Plugin Component Taxonomy | `services/harness/plugins/` | **Batch 2, 3 & 4** |
| **§7.2** | Strongly-typed `PromptTag` (`UserIdentity`, `SessionContext`, `PastTurns`) | `services/harness/tags.rs` | **Batch 3** |
| **§8.0** | Domain configurations (Modular mounts all; Realtime mounts history/prompt only) | `HarnessSession::new_fresh`, `new_realtime` | **Batch 5** |
| **§9.1 – 9.2** | Integration slots for Tagged Demuxer & On-Demand Episodic Tool Calling | `StreamRoutingPlugin` | **Batch 2** |
| **§10.1 – 10.5** | Architectural invariants (ZBC, hot path lock-free, lock discipline across await, decoupled actor, LlmFinished authority) | Entire codebase | **All Batches** |

---

## Balanced Execution Batches

```
  ┌────────────────────────────────────────────────────────┐
  │ Batch 1: Core Event Contracts & AudioIntent Gating     │
  │ • AudioIntent defined in core/events.rs                │
  │ • VoxEvent::SessionStart { session_id }, PlaybackIntent│
  │ • InteractionState::Working = 8 backend & frontend     │
  │ • TTS & PlaybackEngine intent gating                   │
  └──────────────────────────┬─────────────────────────────┘
                             │
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Batch 2: Model Decoupling & Stream Routing Plugin      │
  │ • LlmResponse enum (Token, Finished, Error)            │
  │ • Decouple LlmActor into pure stateless token engine   │
  │ • Implement StreamRoutingPlugin (clauses, TTS, IPC)    │
  └──────────────────────────┬─────────────────────────────┘
                             │
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Batch 3: Dialog History, Tags & Prompt Assembly        │
  │ • Delete legacy accountant/buffer/facade/manager/prompt│
  │ • Implement ChatMessage, Role, PromptTag, filler.rs    │
  │ • Implement ConversationHistoryPlugin & PromptBuilder   │
  └──────────────────────────┬─────────────────────────────┘
                             │
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Batch 4: Context Budgeting, Compaction & Quiet Watcher │
  │ • ContextBudgetPlugin (token formula, FIFO shift)      │
  │ • CompactionPlugin (inline summarization, fact staging)│
  │ • Reactive 20s debounced quiet watcher (zero polling)  │
  └──────────────────────────┬─────────────────────────────┘
                             │
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Batch 5: HarnessSession Chassis & 1:1 Lifecycle Wiring │
  │ • HarnessSession chassis assembling the 5 plugins      │
  │ • AppState.harness: Arc<Mutex<Option<HarnessSession>>> │
  │ • 1:1 Session start/end lifecycle, transcript wiring   │
  │ • continue_session zero-idle check; delete lib.rs loop │
  └────────────────────────────────────────────────────────┘
```

---

### Batch 1: Core Event Contracts & AudioIntent Gating
- **Goal**: Establish core event contracts (`AudioIntent` in `core/events.rs`, `InteractionState::Working = 8`, `VoxEvent::SessionStart`, `PlaybackStarted`, `PlaybackFinished`) and intent-gated audio playback.
- **Dependencies**: None.
- **Files Touched**:
  - `app/src-tauri/src/core/events.rs`:
    - Define `AudioIntent` directly:
      ```rust
      #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
      #[serde(rename_all = "snake_case")]
      pub enum AudioIntent {
          InterimFiller,
          TurnResponse,
      }
      ```
    - Update `VoxEvent::SessionStart { owner: InteractionOwner, session_id: Option<i64> }`.
    - Update `VoxEvent::PlaybackStarted { turn_id: u32, intent: AudioIntent }`.
    - Update `VoxEvent::PlaybackFinished { turn_id: u32, intent: AudioIntent }`.
  - `app/src-tauri/src/core/state.rs`:
    - Add `InteractionState::Working = 8` (and `From<u32>` / `From<InteractionState> for u32`).
  - `app/src/services/eventsService.ts`:
    - Add `"Working"` to TypeScript `InteractionState` union.
  - `app/src/shared/lib/voiceDisplay.ts`:
    - Map `"Working"` to mood/status in `toMood`.
  - `app/src-tauri/src/services/tts/actor.rs`:
    - Update `TtsCommand::Generate { turn_id: u32, text: String, intent: AudioIntent }`.
    - Forward `intent` to `synthesize_chunk`.
  - `app/src-tauri/src/services/tts/providers/mod.rs` & providers (`supertonic.rs`, `kokoro.rs`, `chatterbox.rs`, `chatterbox_remote.rs`, `edge_tts.rs`):
    - Accept `intent: AudioIntent` and forward to `playback.ingest_chunk_with_intent(&chunk, intent)`.
  - `app/src-tauri/src/services/audio/playback.rs`:
    - Add `playback_intent: Arc<AtomicU8>` to `PlaybackEngine`. Rename `stream` to `_stream: Option<cpal::Stream>` (fixing dead_code warning).
    - Expose `ingest_chunk_with_intent(&self, chunk_24khz: &[f32], intent: AudioIntent)`.
    - Pre-roll threshold satisfied $\to$ emit `VoxEvent::PlaybackStarted { turn_id, intent }`.
  - `app/src-tauri/src/services/audio/sink.rs`:
    - Output buffer empty $\to$ emit `VoxEvent::PlaybackFinished { turn_id, intent }`.
  - `app/src-tauri/src/pipeline/router.rs`:
    - Update pattern matches on `SessionStart`, `PlaybackStarted`, and `PlaybackFinished`.
    - Update `ingestion_gate`: include `InteractionState::Working`.
  - `app/src-tauri/src/pipeline/assistant/playback.rs`:
    - `on_playback_started`: If `AudioIntent::InterimFiller`, remain in `Working` (no transition to `Speaking`, no IPC event). If `AudioIntent::TurnResponse`, transition to `Speaking` and emit `IpcEvent::StateChanged(Speaking)`.
    - `on_playback_finished`: If `AudioIntent::InterimFiller`, remain in `Working` (no transition to `Ready`, no IPC event). If `AudioIntent::TurnResponse`, transition to `Ready` and emit `IpcEvent::StateChanged(Ready)`.
- **Verification**: `cargo check --release` passes; frontend builds cleanly.

---

### Batch 2: Model Decoupling & Stream Routing Plugin
- **Goal**: Convert `LlmActor` into a pure stateless token engine, and author `StreamRoutingPlugin` to own token accumulation, clause chunking, TTS dispatch, and UI IPC emissions.
- **Dependencies**: Batch 1 (`AudioIntent`, `VoxEvent`).
- **Files Touched**:
  - `app/src-tauri/src/services/llm/actor.rs`:
    - Define `LlmResponse`:
      ```rust
      pub enum LlmResponse {
          Token(String),
          Finished,
          Error(PipelineError),
      }
      ```
    - Refactor `LlmCommand::Generate`:
      ```rust
      LlmCommand::Generate {
          request: Box<GenerationRequest>,
          turn_id: u32,
          cancel: tokio_util::sync::CancellationToken,
          response_tx: mpsc::Sender<LlmResponse>,
      }
      ```
    - Strip `accumulator`, `tts_tx`, `pending_synthesis_jobs`, and UI IPC calls from `spawn_llm_worker`.
    - Worker streams tokens directly to `response_tx.send(LlmResponse::Token(token))` and sends `LlmResponse::Finished` on completion.
    - Remove emission of `VoxEvent::LlmFinished` from `LlmActor`.
  - [NEW] `app/src-tauri/src/services/harness/plugins/stream.rs`:
    - `StreamRoutingPlugin`: Consumes `LlmResponse` stream over the duplex pipe.
    - Accumulates tokens into clauses via `TtsClauseChunker`.
    - Dispatches synthesized clauses as `TtsCommand::Generate { turn_id, text, intent: AudioIntent::TurnResponse }`.
    - Emits `IpcEvent::LlmToken` directly to the webview window.
    - On stream finish, flushes remainder to TTS and emits `VoxEvent::LlmFinished { turn_id }`.
- **Verification**: `cargo check --release` passes; `LlmActor` has zero imports of audio, TTS, or IPC window code.

---

### Batch 3: Dialog History, Tags & Prompt Assembly
- **Goal**: Clean-slate delete the 5 legacy harness files and author core messages, strongly-typed XML tags, localized transition filler phrases, `ConversationHistoryPlugin`, and `PromptBuilderPlugin`.
- **Dependencies**: Batch 1 (`AudioIntent`), Batch 2.
- **Files Touched**:
  - [DELETE] `app/src-tauri/src/services/harness/accountant.rs`
  - [DELETE] `app/src-tauri/src/services/harness/buffer.rs`
  - [DELETE] `app/src-tauri/src/services/harness/facade.rs`
  - [DELETE] `app/src-tauri/src/services/harness/manager.rs`
  - [DELETE] `app/src-tauri/src/services/harness/prompt_builder.rs`
  - [NEW] `app/src-tauri/src/services/harness/message.rs`:
    - `Role`: `System`, `User`, `Assistant` with Display formatting.
    - `ChatMessage`: `role: Role`, `content: String`, `timestamp_ms: u64`.
    - `current_timestamp_ms`: Current millisecond epoch.
  - [NEW] `app/src-tauri/src/services/harness/tags.rs`:
    - `PromptTag`: `UserIdentity`, `SessionContext`, `PastTurns` with `open_tag(&self) -> &'static str` and `close_tag(&self) -> &'static str`.
  - [NEW] `app/src-tauri/src/services/harness/filler.rs`:
    - `TRANSITION_MESSAGES_EN`: 10 English filler phrases.
    - `TRANSITION_MESSAGES_HI`: 10 Hindi filler phrases.
    - `select_filler_phrase(&str) -> &'static str`: Detects Devanagari via `services::translit::is_devanagari` to select Hindi vs English.
  - [NEW] `app/src-tauri/src/services/harness/plugins/history.rs`:
    - `ConversationHistoryPlugin`: In-memory FIFO message sequence, KV-cache index synchronization, trailing turn deduplication, and interruption rollback (`pop_last_user_turn`, `save_partial_assistant_turn`).
  - [NEW] `app/src-tauri/src/services/harness/plugins/prompt.rs`:
    - `PromptBuilderPlugin`: Pure text assembly with persona prompt + `<user_identity>` markdown document; enforces 20% budget share ceiling and deterministic truncation; zero legacy Memory v1 XML methods.
  - [NEW] `app/src-tauri/src/services/harness/plugins/mod.rs`: Module exports.
- **Verification**: `services/harness/` message, tag, history, and prompt modules compile cleanly.

---

### Batch 4: Context Budgeting, Compaction & Reactive Quiet Watcher
- **Goal**: Author `ContextBudgetPlugin` and `CompactionPlugin`, and wire reactive 20s debounced soft compaction without ambient polling loops.
- **Dependencies**: Batch 1, Batch 2, Batch 3.
- **Files Touched**:
  - [NEW] `app/src-tauri/src/services/harness/plugins/budget.rs`:
    - `ContextBudgetPlugin`: Real-time token accountant, usable budget calculation `(max_tokens - reserved_generation)`, 65% soft / 85% critical threshold classification, deterministic FIFO sliding window shift.
  - [NEW] `app/src-tauri/src/services/harness/plugins/compaction.rs`:
    - `CompactionPlugin`:
      - Inline critical compaction (utilization $\ge 85\%$): sends compaction task over duplex pipe to `LlmActor` (up to 2 attempts, 45s timeout), parses JSON (summary + facts: personal, objective, workdone, blocker, next_step, pitfall), stages to Turso DB queue, rebuilds history `[System Prompt, Rolling Context Summary, Active User Turn]`. Degraded FIFO fallback if model $\le 4096$ or history $\le 3$ or retries fail.
      - Reactive soft compaction (65%–85%): Post-turn, arms 20s quiet debounce timer. Aborts on user speech, interrupt, or leaving Ready/Paused. Commits on 20s quiet interval.
  - `app/src-tauri/src/lib.rs`:
    - Remove `spawn_state_compaction_observer` import and background task invocation.
- **Verification**: `services/harness/` budgeting and compaction modules compile cleanly with zero background polling loops.

---

### Batch 5: HarnessSession Chassis Assembly & 1:1 Lifecycle Integration
- **Goal**: Assemble `HarnessSession` chassis from the 5 plugins and integrate into `AppState`, session lifecycle commands, pipeline event handlers, and IPC handlers.
- **Dependencies**: Batches 1, 2, 3, 4.
- **Files Touched**:
  - [NEW] `app/src-tauri/src/services/harness/session.rs`:
    - `HarnessSession`: Chassis managing the 5 plugins, session-scoped duplex pipe to `LlmActor`, domain configurations for Modular vs Realtime, `prepare_turn`, and constructors (`new_fresh`, `new_continuing`, `new_realtime`).
  - [MODIFY] `app/src-tauri/src/services/harness/mod.rs`:
    - Subsystem root exporting `HarnessSession`, `ChatMessage`, `Role`, `PromptTag`, and filler constants.
  - `app/src-tauri/src/core/state.rs`:
    - Replace `pub conversation_manager` with `pub harness: Arc<parking_lot::Mutex<Option<HarnessSession>>>`.
  - `app/src-tauri/src/ipc/pipeline.rs`:
    - `start_session(session_id: Option<i64>)`: Routes `VoxEvent::SessionStart { owner: Assistant, session_id }`.
  - `app/src-tauri/src/ipc/persistence.rs`:
    - `continue_session`: Queries Turso for metadata and turns for UI display only. Leaves `HarnessSession` untouched while `Idle`.
    - `create_session`: Clears UI selection and resets `conversation_id = 0`. Leaves `HarnessSession` untouched while `Idle`.
  - `app/src-tauri/src/ipc/memory.rs` & `services/memory/scheduler.rs`:
    - Update personal memory in DB. If `state.harness.lock().as_mut()` is `Some(h)`, update active prompt.
  - `app/src-tauri/src/pipeline/assistant/session.rs`:
    - `on_session_start`: Boots `HarnessSession` (`session_id: Option<i64>`). If `Some(id)`, fetches continuation context from Turso; if `None`, boots fresh. Sets up duplex pipe, assigns to `state.harness`, and transitions `Idle` $\to$ `Ready`.
    - `on_end`: Drops `state.harness.lock().take()`, cancels session-scoped tokens, transitions `Current` $\to$ `Idle`.
  - `app/src-tauri/src/pipeline/assistant/transcript.rs`:
    - `on_transcript_final`: Routes turn query to `HarnessSession::prepare_turn`.
    - If utilization $\ge 85\%$: Harness dispatches filler phrase with `AudioIntent::InterimFiller` to TTS, transitions `Thinking` $\to$ `Working`, runs inline compaction via `LlmActor`, prunes history, stages facts, then initiates answer generation with `AudioIntent::TurnResponse`.
  - `app/src-tauri/src/pipeline/assistant/interrupt.rs`:
    - `on_interrupt`: Extracts partial assistant turn from `state.harness`, pushes to history, dispatches `TurnCompleted` persistence event under interrupted turn ID.
  - `app/src-tauri/src/pipeline/assistant/llm.rs`:
    - `on_llm_finished`: Pre-roll flush and audio playback completion guard.
  - `app/src-tauri/src/pipeline/lifecycle.rs`:
    - Prune obsolete sync helpers replaced by `HarnessSession`.
- **Verification**: `cargo clippy --release --lib -- -D warnings` and `pnpm build` pass with 0 errors.

---

### Batch 6: Hardening, CompactionPlugin Ledger Staging, Watcher Wiring & Test Alignment [ADDED]
- **Goal**: Relocate inline & soft compaction execution and Turso DB fact staging (`record_compaction_start` + `commit_compaction_output`) into `CompactionPlugin` to keep `transcript.rs` lean and compliant with Spec §7.1. Wire `QuietCompactionWatcher` into `CompactionPlugin` and hook into turn lifecycle events (`on_playback_finished`, speech onset, interrupts). Fix `InteractionState::Working` transition and barge-in gating. Modernize test targets to use `HarnessSession` so `cargo clippy --all-targets -- -D warnings` passes.
- **Dependencies**: Batches 1–5.
- **Files Touched**:
  - `app/src-tauri/src/services/harness/plugins/compaction.rs`:
    - Encapsulate inline & soft compaction execution, Turso DB ledger recording (`record_compaction_start`), and fact staging (`commit_compaction_output`) inside `CompactionPlugin`.
    - Integrate `QuietCompactionWatcher` into `CompactionPlugin`, managing the 20s debounce timer.
  - `app/src-tauri/src/services/harness/session.rs`:
    - Expose lifecycle handles (`execute_inline_compaction`, `on_turn_completed`, `abort_watcher`) on `HarnessSession`.
  - `app/src-tauri/src/pipeline/assistant/transcript.rs`:
    - Call explicit `transition(InteractionState::Working, ctx, app, state)` when inline compaction triggers.
    - Delegate inline compaction and DB fact staging to `HarnessSession` / `CompactionPlugin`, slimming down `transcript.rs`.
    - Resolve engine channels asynchronously via `state.engine.lock().await` inside spawned task instead of synchronous `try_lock()`.
  - `app/src-tauri/src/pipeline/assistant/speech.rs`:
    - Add `InteractionState::Working` to barge-in check (`if current_state == Thinking || Speaking || Working => on_interrupt(...)`).
    - Abort quiet compaction watcher on speech onset.
  - `app/src-tauri/src/pipeline/assistant/ptt.rs`:
    - Add `InteractionState::Working` to barge-in check (`if current_state == Thinking || Speaking || Working => on_interrupt(...)`).
  - `app/src-tauri/src/pipeline/assistant/playback.rs`:
    - Trigger `harness.on_turn_completed(...)` when `on_playback_finished` transitions to `Ready`.
  - `app/src-tauri/src/pipeline/assistant/interrupt.rs`:
    - Abort quiet compaction watcher on interrupt.
    - Guard against consecutive `User` turns when `partial_assistant` is empty.
  - `app/src-tauri/tests/session_lifecycle_test.rs`:
    - Migrate `conversation_manager` references to `state.harness.lock().as_ref().unwrap().prompt.assemble()`.
  - `app/src-tauri/tests/tts_transition_test.rs`:
    - Migrate deleted `facade::prepare_turn_context` calls to `HarnessSession::prepare_turn` with `TurnPreparation::NeedsInlineCompaction`.
  - `app/src-tauri/tests/transcript_to_llm_test.rs`:
    - Migrate `conversation_manager` references to `state.harness.lock().as_mut().unwrap().history`.

---

## Verification Plan

1. **Clippy Code Style & Lint Cleanliness (All Targets)**:
   ```bash
   cargo clippy --release --all-targets -- -D warnings
   ```
2. **Integration Test Sanity Check**:
   ```bash
   cargo test --test session_lifecycle_test --test transcript_to_llm_test --test tts_transition_test -- --nocapture
   ```
3. **Frontend Type & Build Health**:
   ```bash
   pnpm build
   ```
