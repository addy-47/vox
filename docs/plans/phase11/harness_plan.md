# Implementation Plan — Vox LLM Agent Harness Runtime Refactor (Production LLD)

Translate the approved [LLM Agent Harness Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md) and [Target Event-Domain Architectural Specification](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md) into code. This plan establishes `Harness` in `services/harness/orchestrator.rs` as the single cognitive turn sequencer owning `execute_turn`, extracts NLP clause chunking out of physical audio synthesis (`services/tts/actor.rs`), enforces the Generic Non-Terminal Phase contract (`InteractionState::Working = 8`), and resolves live failure hazards (dropped finishes, deadlocks on missing channels, and history wipes).

---

## Target End-State

1. **Physical Audio Worker Decoupled (`services/tts/actor.rs`)**:
   - `services/tts/actor.rs` pruned to $<180$ lines, owning physical audio synthesis only. Zero NLP or string chunking logic.
   - `ClauseChunker` ported byte-identically to `services/harness/streaming/chunker.rs`.
   - `create_tts_provider` extracted to `services/tts/factory.rs`.
   - Saturating counter decrement discipline applied to `pending_synthesis_jobs` to prevent `u32` wrap under external resets.
2. **Single Orchestrator Authority (`services/harness/orchestrator.rs`)**:
   - `Harness` is the sole orchestrator owning session lifecycle, quiet compaction watcher, and `execute_turn`.
   - Upstream caller (`pipeline/assistant/transcript.rs`) thinned from ~390 lines to a pure event adapter: receives `VoxEvent::TranscriptFinal`, invokes `harness.execute_turn(req).await`, and translates the returned `TurnOutcome` into pipeline state transitions.
   - Legacy `services/harness/session.rs` and `services/harness/plugins/` (all 6 files) deleted completely.
3. **Pure Stage Taxonomy (`services/harness/stages/`)**:
   - `stages/history.rs`: `ConversationHistoryStage` (FIFO message buffer, deduplication, turn commits, rollback).
   - `stages/prompt.rs`: `PromptBuilderStage` (persona grounding, `<user_identity>` budget ceiling, `build_generation_request` payload assembly).
   - `stages/budget.rs`: `ContextBudgetStage` (token estimation, 65% soft / 85% critical threshold evaluation, `execute_fifo_shift`).
   - `stages/compaction.rs`: `CompactionStage` (0-retry fail-fast compaction, executor isolation on `tokio::task::spawn_blocking` for local compute, Turso DB fact staging).
4. **Decoupled Egress Streaming (`services/harness/streaming/`)**:
   - `streaming/chunker.rs`: `ClauseChunker` (sentence boundary detection, prosody morphing `.` $\to$ `,` under 5 words).
   - `streaming/router.rs`: `StreamRouter` (duplex stream consumer, clause dispatch to TTS with `AudioIntent::TurnResponse`, subtitle IPC emit, `TurnAccumulator` writes, sole emitter of `VoxEvent::LlmFinished`).
   - `streaming/demuxer.rs`: `StreamingTagDemuxer` (deferred slot stub enforcing 64-token speculative buffer ceiling and leading tag resolution invariant upstream of both chunker and UI subtitles).
5. **Robust Concurrency & Error Contracts**:
   - Dropped-finish hazard resolved: `pipeline/assistant/llm.rs::on_llm_finished` guard check expanded to accept `InteractionState::Working` alongside `Thinking` and `Speaking`.
   - Single-turn cancellation token discipline: `CancellationToken` captured by value per turn, eliminating shared `AtomicBool` split-brain hazards. Cancelled turns never emit `VoxEvent::LlmFinished`.
   - Missing-channel deadlock prevention: Pre-dispatch checks return clean `TurnOutcome::Error` if `llm_tx` or `pipeline_tx` is `None`, and run degraded text-only mode if `tts_tx` is `None`.
   - Staging order guarantee: Active user query staged into history *before* cognitive maintenance, guaranteeing zero query loss during fallback FIFO shifts.

---

## Resolved Architectural Decisions

| Decision / Gap | Production Resolution |
| :--- | :--- |
| **Struct Name & File Placement** | Struct is `Harness`; file is `services/harness/orchestrator.rs`. |
| **`turn_id` Wire Representation** | Wire type is strictly `u32` across `TurnExecutionRequest`, `TurnOutcome`, and `Harness` to match `PipelineAtomics`. |
| **Compaction Policy & Retries** | Zero-retry fail-fast. If the single inline compaction attempt fails or returns malformed JSON, immediately fallback to deterministic FIFO shift to preserve conversational latency. |
| **Compaction Executor Isolation** | Embedded/local provider inference executed inside `tokio::task::spawn_blocking` to prevent starving the Tokio async reactor. Cloud providers await natively. |
| **Prompt In-Place Synchronization** | On prompt changes, `PromptBuilderStage` updates `messages[0]` (System) in place. It never wipes the message vec, eliminating the historical turn wipe bug. |
| **Dropped Query Preservation** | User query is staged in Phase 2 *before* budget evaluation or compaction. On FIFO shift, oldest turn pairs are dropped from index 1 while preserving the active query at the tail. |
| **Leading Tag Demuxer Placement** | Demuxer sits strictly upstream of both `ClauseChunker` (audio) and `IpcEvent::LlmToken` (subtitles), with bounded 64-token speculative buffer. |
| **Dropped-Finish Hazard** | `pipeline/assistant/llm.rs::on_llm_finished` guard accepts `Thinking \| Working \| Speaking`. |
| **Missing Channels (`None`)** | `llm_tx == None` or `pipeline_tx == None` fails fast with `Error` without blocking on orphaned `response_rx`. `tts_tx == None` executes degraded text-only stream. |

---

## Detailed Type Contracts & Signatures

### 1. Domain Types (`services/harness/mod.rs`)

```rust
pub mod orchestrator;
pub mod stages;
pub mod streaming;
pub mod watcher;

pub use orchestrator::{Harness, PipelineDomain, TurnExecutionRequest, TurnOutcome};
pub use watcher::QuietCompactionWatcher;

pub enum Role {
    System,
    User,
    Assistant,
}

pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp_ms: u64,
}

pub enum PromptTag {
    UserIdentity,
    SessionContext,
    PastTurns,
}

pub enum ContextStatus {
    Nominal,
    SoftWarning,
    Critical,
}

pub enum TurnOutcome {
    Completed { assistant_response: String, turn_id: u32 },
    DuplicateIgnored { turn_id: u32 },
    Cancelled { turn_id: u32 },
    Error { turn_id: u32, message: String },
}

pub struct TurnExecutionRequest<'a, R: tauri::Runtime> {
    pub query: &'a str,
    pub turn_id: u32,
    pub cancel: tokio_util::sync::CancellationToken,
    pub owner: crate::core::state::InteractionOwner,
    pub llm_tx: Option<std::sync::mpsc::Sender<crate::services::llm::actor::LlmCommand>>,
    pub tts_tx: Option<std::sync::mpsc::Sender<crate::services::tts::actor::TtsCommand>>,
    pub provider: Option<std::sync::Arc<dyn crate::services::llm::LlmProvider>>,
    pub db: std::sync::Arc<crate::persistence::db::VoxDb>,
    pub pipeline_tx: Option<std::sync::mpsc::Sender<crate::core::events::VoxEvent>>,
    pub accumulator: std::sync::Arc<parking_lot::Mutex<crate::pipeline::assistant::accumulator::TurnAccumulator>>,
    pub pending_synthesis_jobs: std::sync::Arc<std::sync::atomic::AtomicU32>,
    pub app: tauri::AppHandle<R>,
    pub routing_ctx: crate::pipeline::router::RoutingContext,
    pub app_state: std::sync::Arc<crate::core::state::AppState>,
}
```

### 2. Audio Boundary Decoupling

#### `services/harness/streaming/chunker.rs` (`ClauseChunker`)
Byte-identical port of `services/tts/actor.rs:333-490`:
- Adaptive thresholds: clause 0 `(5, 8, 12)`, clause 1 `(10, 15, 20)`, steady state `(16, 24, 32)`.
- Strong terminators (`\n`, `?`, `!`) split unconditionally.
- Gated sub-clause terminators (`,`, `;`, `:`, `—`, `–`) split once word count $\ge w_{min}$.
- Abbreviation and decimal guards (`is_abbreviation`, digit-period-digit protection).
- Prosody morphing: periods rewritten to commas when word count $< 5$ and followed by whitespace.
- Emergency word cap fallback at $w_{max}$.

#### `services/tts/factory.rs`
Extracted from `services/tts/actor.rs:200-267`:
```rust
pub fn create_tts_provider(
    settings: &crate::core::settings::VoxSettings,
    super_tts_path: &std::path::Path,
    reference_audio: Option<&str>,
) -> Result<Box<dyn super::providers::TtsProvider>, String>;

pub async fn resolve_reference_audio(
    conn: &turso::Connection,
    voice_id: Option<&str>,
) -> Option<String>;
```

#### Pruned `services/tts/actor.rs` ($<180$ lines)
- Retains only: `TtsCommand`, `TtsWorkerHandles`, `spawn_tts_worker`, `TtsWarmUpHandles`, `warm_up_tts`, `cool_down_tts`.
- Implements saturating decrement on `pending_synthesis_jobs` via `fetch_update` to prevent integer wrap under external resets.

### 3. Harness Stages (`services/harness/stages/`)

- **`stages/history.rs` (`ConversationHistoryStage`)**:
  - In-memory message vec (`Vec<ChatMessage>`) and KV-cache sync index.
  - `push_user_turn`, `push_assistant_turn`, `rollback_last_user_turn`.
  - `is_duplicate_user_turn` (last-message equality check).
  - `truncate_oldest_turns(count)` (drops turn pairs from index 1, preserving root system prompt).
- **`stages/prompt.rs` (`PromptBuilderStage`)**:
  - Bounded `<user_identity>` injection (20% context window ceiling, floor character boundary truncation).
  - `assemble(&self) -> String`: returns canonical root system prompt string.
  - `build_generation_request(&self, history: &[ChatMessage], query: &str, options: GenerationOptions) -> GenerationRequest`: builds payload without mutating history.
- **`stages/budget.rs` (`ContextBudgetStage`)**:
  - Tracks context utilization formula: `tracked_tokens / (max_tokens - reserved_tokens)`.
  - Reserved tokens derived from `settings.llm.max_output_tokens` clamped to `[256, 2048]`.
  - `evaluate_utilization(tokens) -> (f32, ContextStatus)` (`Nominal`, `SoftWarning`, `Critical`).
  - `execute_fifo_shift(&self, history: &mut ConversationHistoryStage) -> usize`: shifts historical turns until utilization $<65\%$.
- **`stages/compaction.rs` (`CompactionStage`)**:
  - `can_perform_inline_compaction`: eligible if message count $\ge 4$.
  - `run_and_persist`: records start in Turso DB `compactions` ledger, invokes `run_compaction`, records finish, and stages facts into ingestion queue.
  - Zero-retry fail-fast execution: failures return `Err`, triggering immediate FIFO fallback.

### 4. Egress Streaming (`services/harness/streaming/`)

- **`streaming/router.rs` (`StreamRouter`)**:
  - Consumes `LlmResponse` across duplex pipe.
  - Checks `cancel.is_cancelled()` on every iteration. On cancel: emits `VoxEvent::Cancelled`, returns partial text without emitting `LlmFinished`.
  - Disconnect without `Finished` returns `Err("stream disconnected")`, triggering abort without committing partial text.
  - Feeds incoming tokens through `StreamingTagDemuxer` before dispatching to `ClauseChunker` or emitting `IpcEvent::LlmToken`.
  - Dispatches clauses to `tts_tx` with `AudioIntent::TurnResponse` and increments `pending_synthesis_jobs`.
  - Sole authority for `VoxEvent::LlmFinished`, emitted only on successful completion with non-empty text.
- **`streaming/demuxer.rs` (`StreamingTagDemuxer`)**:
  - Deferred slot stub with `DEMUXER_MAX_SPECULATIVE_TOKENS = 64`.
  - Buffers tokens until first non-whitespace character is resolved. If non-`<`, flushes speculative buffer immediately as raw text.
  - If `<` prefix does not match a registered tag within 64 tokens, flushes as raw text.

---

## The 7-Phase Execution Sequence (`Harness::execute_turn`)

```
                                  execute_turn(req)
                                          │
                  ┌───────────────────────┴───────────────────────┐
                  ▼                                               ▼
         Is Duplicate Turn?                             Turn Pre-Cancelled?
         [DuplicateIgnored]                                 [Cancelled]
                  │ (No)
                  ▼
         Phase 2: Push User Query to History (Guaranteed Staging)
                  │
                  ▼
         Evaluate Context Utilization (stages/budget.rs)
                  │
        ┌─────────┴───────────────────────────────────────────────┐
        ▼ (Critical >= 85% & len >= 4)                            ▼ (Nominal < 85%)
   Phase 3: Critical Inline Compaction                            │
   • Transition: Thinking -> Working                              │
   • Dispatch: InterimFiller -> TTS                               │
   • Vend DB & Run Compaction (spawn_blocking for local)          │
   • Success: Apply <session_context> summary                     │
   • Failure: Degraded FIFO Shift (Query Preserved!)              │
        │                                                         │
        └─────────────────────────┬───────────────────────────────┘
                                  ▼
         Phase 4: Prompt Assembly & GenerationRequest Building
                  │
                  ▼
         Phase 5: Duplex Model Pipe Dispatch (llm_tx Send)
                  • Check None Channels (Fail-fast on deadlocks)
                  • Create (response_tx, response_rx)
                  │
                  ▼
         Phase 6: Egress Stream Routing (spawn_blocking)
                  • Demuxer Speculative Check (Max 64 Tokens)
                  • Emit Subtitle IPC (IpcEvent::LlmToken)
                  • Push to ClauseChunker & Dispatch TTS (AudioIntent::TurnResponse)
                  • Append to TurnAccumulator
                  │
                  ▼
         Phase 7: Finalization & Commit
                  • Succeeded: Commit turn to History, emit LlmFinished,
                               arm 20s quiet watcher if soft window (65-85%)
                               -> Return Completed
                  • Cancelled: Rollback staged query -> Return Cancelled
                  • Error/Drop: Rollback staged query, transition to Ready
                               -> Return Error
```

---

## Refactoring `pipeline/assistant/transcript.rs`

`spawn_modular_llm_task` drops lines 93-270 of manual orchestration, reducing to a clean adapter:

```rust
// 1. Snapshot lightweight handles
let cancel = state.pipeline.turn_token();
let req = TurnExecutionRequest {
    query: &query,
    turn_id,
    cancel,
    owner: ctx_owner,
    llm_tx: engine_llm_tx,
    tts_tx: engine_tts_tx,
    provider: provider_arc.read().clone(),
    db: Arc::clone(&app_state.db),
    pipeline_tx: engine_pipeline_tx,
    accumulator,
    pending_synthesis_jobs: pending_jobs,
    app: app_clone,
    routing_ctx: ctx_clone,
    app_state: Arc::clone(&app_state),
};

// 2. Spawn and execute via single orchestrator
tauri::async_runtime::spawn(async move {
    let outcome = {
        let mut guard = harness_arc.lock();
        let Some(ref mut harness) = *guard else {
            log::error!("[Pipeline::Transcript] No active Harness mounted");
            return;
        };
        harness.execute_turn(req).await
    };

    // 3. Map strongly typed TurnOutcome to state transitions
    match outcome {
        TurnOutcome::Completed { turn_id, assistant_response } => {
            log::info!("[Pipeline::Transcript] Turn {} completed (chars {})", turn_id, assistant_response.len());
            // Playback engine drives Speaking -> Ready
        }
        TurnOutcome::DuplicateIgnored { turn_id } => {
            log::info!("[Pipeline::Transcript] Duplicate turn {} ignored", turn_id);
            transition(InteractionState::Ready, &ctx_clone, &app_clone, &app_state);
        }
        TurnOutcome::Cancelled { turn_id } => {
            log::info!("[Pipeline::Transcript] Turn {} cancelled", turn_id);
            transition(InteractionState::Ready, &ctx_clone, &app_clone, &app_state);
        }
        TurnOutcome::Error { turn_id, message } => {
            log::error!("[Pipeline::Transcript] Turn {} failed: {}", turn_id, message);
            transition(InteractionState::Ready, &ctx_clone, &app_clone, &app_state);
        }
    }
});
```

---

## Ordered Execution Batches

- **Batch 0 — Scaffolding & Types**: Create `stages/` and `streaming/` module trees; define domain types (`TurnExecutionRequest`, `TurnOutcome`, `ContextStatus`) in `harness/mod.rs`. Holds green build throughout.
- **Batch 1 — Audio Layer Boundary Decoupling**: Move `TtsClauseChunker` $\to$ `services/harness/streaming/chunker.rs::ClauseChunker`. Extract `create_tts_provider` $\to$ `services/tts/factory.rs`. Prune `services/tts/actor.rs` to $<180$ lines with saturating decrement. Update `accumulator.rs` and `chunking_determinism_test.rs`. Holds green build throughout.
- **Batch 2 — Pure Stages Implementation**: Implement `stages/history.rs`, `stages/prompt.rs`, `stages/budget.rs`, `stages/compaction.rs`, and `stages/mod.rs` with in-place prompt synchronization and 0-retry isolation. Holds green build throughout.
- **Batch 3 — Egress Stream Router & Demuxer**: Implement `streaming/router.rs` and `streaming/demuxer.rs` with owned `CancellationToken` and 64-token speculative buffer ceiling. Holds green build throughout.
- **Batch 4 — Orchestrator Assembly & execute_turn**: Implement `services/harness/orchestrator.rs::Harness` owning the 7-phase turn lifecycle. Update `watcher.rs` and re-export from `harness/mod.rs`. Red mid-batch, green at batch end.
- **Batch 5 — Pipeline Adapter Cutover & Legacy Pruning**: Thin `pipeline/assistant/transcript.rs` to delegate to `execute_turn`. Expand `pipeline/assistant/llm.rs::on_llm_finished` guard to accept `InteractionState::Working`. Delete legacy `services/harness/session.rs` and `services/harness/plugins/`. Wire `core/state.rs`, `pipeline/assistant/session.rs`, and tests. Green at batch completion.

---

## Verification Plan

### Automated Tests
1. **Clause Chunking Determinism**:
   ```bash
   cargo nextest run --test chunking_determinism_test --release --nocapture --test-threads=1
   ```
2. **Memory & Compaction Integration**:
   ```bash
   cargo nextest run --test memory_compaction_test --release --nocapture --test-threads=1
   ```
3. **Session Lifecycle & Transitions**:
   ```bash
   cargo nextest run --test session_lifecycle_test --release --nocapture --test-threads=1
   ```
4. **Full 20-Seam Isolated Regression Suite**:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
