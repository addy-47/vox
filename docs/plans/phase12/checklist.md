# Phase 12.1 — Agentic Vox Execution Checklist

## Batch 0 — Orchestrator Decoupling & Non-Terminal Phase Unification
**Dependencies**: None

- [x] `app/src-tauri/src/services/harness/orchestrator.rs` [DELETE]
  - [x] Remove monolithic single file.
- [x] `app/src-tauri/src/services/harness/orchestrator/mod.rs` [NEW]
  - [x] `Harness`: Define struct with state fields, constructors (`new_modular`), and domain accessors.
  - [x] Re-export submodules (`turn`, `phase`, `barge_in`).
- [x] `app/src-tauri/src/services/harness/orchestrator/phase.rs` [NEW]
  - [x] `NonTerminalTrigger`: Define enum (`Compaction`, `NonTerminalTool { tool_name: String, call_id: String }`).
  - [x] `NonTerminalPhase`: Define struct (`trigger`, `filler_phrase: Option<String>`).
  - [x] `Harness::enter_non_terminal_phase`: Centralized helper executing `transition(Working)` and dispatches `AudioIntent::InterimFiller` using `filler_phrase` (with fallback to `select_filler_phrase`), bundled via `NonTerminalContext`.
- [x] `app/src-tauri/src/services/harness/orchestrator/turn.rs` [NEW]
  - [x] `Harness::execute_turn`: Sequential lifecycle coordinator with private phase helpers.
  - [x] `phase_prompt_budget`: Phase 1 & Phase 2 prompt assembly and budget check.
  - [x] `phase_compaction`: Phase 3 compaction check, refactored to call `enter_non_terminal_phase`.
  - [x] `phase_generate_and_stream`: Phase 4 & Phase 5 generation and streaming dispatch.
- [x] `app/src-tauri/src/services/harness/orchestrator/barge_in.rs` [NEW]
  - [x] `Harness::handle_turn_cancelled`: Single-writer cancellation handling (Invariant 13).
- [x] `app/src-tauri/src/services/harness/mod.rs`
  - [x] Update `pub mod orchestrator;` to reference directory module.

---

## Batch 1 — Canonical Types & Shared Interface Expansion
**Dependencies**: Batch 0

- [x] `app/src-tauri/src/services/llm/provider.rs`
  - [x] `ToolFlow`: Define enum with `Terminal` and `NonTerminal` variants.
  - [x] `CanonicalToolDefinition`: Define struct with `name: String`, `description: String`, `parameters: serde_json::Value`, `flow: ToolFlow`.
  - [x] `CanonicalToolCall`: Define struct with `id: String`, `name: String`, `arguments: serde_json::Value`.
  - [x] `LlmStreamEvent`: Add variant `ToolCall(CanonicalToolCall)`.
  - [x] `GenerationRequest`: Add field `tools: Option<Vec<CanonicalToolDefinition>>`.
- [x] `app/src-tauri/src/services/llm/actor.rs`
  - [x] `LlmResponse`: Add variant `ToolCall(CanonicalToolCall)`.
  - [x] `handle_generate`: Forward `LlmStreamEvent::ToolCall` stream events to `LlmResponse::ToolCall`.
- [x] `app/src-tauri/src/services/harness/mod.rs`
  - [x] `Role`: Add variant `Role::Tool` with `Display` output `"tool"`.
  - [x] `ChatMessage`: Add optional fields `tool_call_id: Option<String>` and `tool_calls: Option<Vec<CanonicalToolCall>>`.
- [x] `app/src-tauri/src/persistence/mod.rs`
  - [x] `PersistenceEvent`: Add variant `ToolCallExecuted` with `tool_flow` and metadata fields (mapping `ToolFlow::Terminal` $\to$ `"terminal"` and `ToolFlow::NonTerminal` $\to$ `"non_terminal"` for `tool_kind` DB column).
- [x] `app/src-tauri/src/persistence/schema.rs`
  - [x] `SCHEMA_VERSION`: Bump from 4 to 5.
  - [x] `V2_TABLE_STATEMENTS`: Add DDL for `session_tool_calls` table and indexes.
  - [x] `test_schema_initialization`: Update schema version and table count assertion.
- [x] `app/src-tauri/src/persistence/tool_calls.rs` [NEW]
  - [x] `persist_tool_call`: Insert record into `session_tool_calls` with self-healing session insert.

---

## Batch 2 — Synthesis Guard Latch (`turn_open` / `drained_while_open`)
**Dependencies**: None

- [x] `app/src-tauri/src/pipeline/atomics.rs`
  - [x] `PipelineAtomics`: Add fields `turn_open: Arc<AtomicBool>` and `drained_while_open: Arc<AtomicBool>`.
  - [x] `PipelineAtomics::new`: Initialize both atomics to `false`.
  - [x] `PipelineAtomics::reset_turn_guards`: Reset both atomics to `false`.
  - [x] `PipelineAtomics` accessors: Add `set_turn_open`, `clear_turn_open`, `is_turn_open`, `set_drained_while_open`, `clear_drained_while_open`, `is_drained_while_open`.
- [x] `app/src-tauri/src/pipeline/assistant/playback.rs`
  - [x] `on_playback_started`: Clear `drained_while_open` when `AudioIntent::TurnResponse` begins playback.
  - [x] `on_playback_finished`: If `turn_open` is true, latch `drained_while_open = true`; only transition to `Ready` if `turn_open` is false and synthesis queue is empty.
- [x] `app/src-tauri/src/pipeline/assistant/llm.rs`
  - [x] `on_llm_finished`: Clear `turn_open = false`, and if `drained_while_open` is true with empty synthesis queue, trigger transition to `Ready`.
- [x] `app/src-tauri/src/pipeline/assistant/transcript.rs`
  - [x] `on_transcript_final`: Set `turn_open = true` before dispatching turn execution to Harness.
- [x] `app/src-tauri/src/pipeline/assistant/interrupt.rs`
  - [x] `on_interrupt`: Clear `turn_open = false` and `drained_while_open = false`.
- [x] `app/src-tauri/src/pipeline/assistant/session.rs`
  - [x] `on_pause_session`: Clear `turn_open = false` and `drained_while_open = false`.
  - [x] `on_end_session`: Clear `turn_open = false` and `drained_while_open = false`.

---

## Batch 3 — Tool Execution Stage & Scratchpad (`stages/tools/`)
**Dependencies**: Batch 1

- [x] `app/src-tauri/src/services/harness/mod.rs`
  - [x] Add `pub const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(10);`.
- [x] `app/src-tauri/src/services/harness/stages/tools/mod.rs` [NEW]
  - [x] `ToolDefinition`: Define async trait for name, description, schema, flow (`ToolFlow`), and execution.
  - [x] `ToolResult`: Define struct with `content: String` and `is_error: bool`.
  - [x] `ToolExecutionContext`: Define execution context bundle (`session_id`, `turn_id`, `cancel_token`, `db`, `persist_tx`).
- [x] `app/src-tauri/src/services/harness/stages/tools/registry.rs` [NEW]
  - [x] `ToolRegistry`: Registry storing registered `ToolDefinition` instances.
  - [x] `ToolRegistry::register`: Add tool to internal map.
  - [x] `ToolRegistry::schemas`: Filter and return `Vec<CanonicalToolDefinition>` accepting `ToolFilter { turn_id: u32, title_is_unset: bool }` (`respond_and_set_title` turn 1 only; `search_memory` always).
- [x] `app/src-tauri/src/services/harness/stages/tools/executor.rs` [NEW]
  - [x] `ToolExecutor::execute`: Execute tool with `tokio::time::timeout(TOOL_EXECUTION_TIMEOUT, ...)` and cancellation guard, returning `(ToolResult, Duration)`.
  - [x] `ToolExecutor::dispatch_persistence`: Send `PersistenceEvent::ToolCallExecuted` over channel.
- [x] `app/src-tauri/src/services/harness/stages/tools/title.rs` [NEW]
  - [x] `RespondAndSetTitleTool`: Implement `ToolDefinition` for single-pass terminal title and response delivery.
  - [x] `RespondAndSetTitleTool::execute`: Perform direct awaited DB title update and broadcast `IpcEvent::SessionsChanged`.
- [x] `app/src-tauri/src/services/harness/stages/tools/memory.rs` [NEW]
  - [x] `MemorySearchTool`: Implement `ToolDefinition` stub for non-terminal episodic search with `spoken_filler`.
- [x] `app/src-tauri/src/services/harness/stages/mod.rs`
  - [x] Module tree: Add `pub mod tools;`.

---

## Batch 4 — Orchestrator Reentrant Loop & Stream Demuxing
**Dependencies**: Batch 0, Batch 1, Batch 2, Batch 3

- [ ] `app/src-tauri/src/services/harness/stages/streaming/router.rs`
  - [ ] `StreamPassOutcome`: Define enum with `Completed { assistant_text }`, `ToolCallReceived { partial_text, call }`, `Cancelled { partial_text }`, `Error(String)`.
  - [ ] `route_stream`: Update return type to `Result<StreamPassOutcome, String>` and demux `LlmResponse::ToolCall` mid-stream.
- [ ] `app/src-tauri/src/services/harness/orchestrator/mod.rs`
  - [ ] `Harness`: Add `tool_registry: ToolRegistry` and `supports_tools: bool` fields.
- [ ] `app/src-tauri/src/services/harness/orchestrator/turn.rs`
  - [ ] `execute_turn`: Instantiate turn-local `scratchpad: Vec<ChatMessage>`.
  - [ ] `phase_generate_and_stream`: Inject tool schemas into `GenerationRequest` when `supports_tools` is true, using `ToolFilter { turn_id, title_is_unset }`.
  - [ ] `execute_turn` (Tool handling loop): Handle `StreamPassOutcome::ToolCallReceived`:
    - [ ] Pre-tool partial text disposition: If NonTerminal, flush `partial_text` through `ClauseChunker` $\to$ `TtsActor` as `AudioIntent::TurnResponse` before entering `Working`; if Terminal, discard `partial_text`.
    - [ ] If `ToolFlow::Terminal` (e.g. `respond_and_set_title`): Extract `spoken_response`, route through `TextNormalizer` $\to$ `ClauseChunker` $\to$ `TtsActor` as `AudioIntent::TurnResponse`, execute tool action asynchronously, and complete turn in a single pass with zero `ChatMessage` records appended to history or scratchpad (commit only `spoken_response` text).
    - [ ] If `ToolFlow::NonTerminal` (e.g. `search_memory`): Extract `spoken_filler`, call `enter_non_terminal_phase` (plays filler immediately as `AudioIntent::InterimFiller` without `has_played_filler` suppression, transitions to `Working`), execute tool under `TOOL_EXECUTION_TIMEOUT`, append to scratchpad, re-enter Phase 2 Step 4.
    - [ ] Guard loop with `MAX_TOOL_ITERATIONS = 5`, falling back to tool-stripped final generation on breach.
  - [ ] `execute_turn` (Commit): Drop ephemeral scratchpad; commit only spoken user and assistant turns to history.
  - [ ] `execute_turn` (Cancellation): Delegate to `barge_in::handle_turn_cancelled`.

---

## Batch 5 — Session Boot Capability Gating
**Dependencies**: Batch 0, Batch 1, Batch 3

- [x] `app/src-tauri/src/pipeline/assistant/session.rs`
  - [x] `on_session_start`: Check `ModelProbeResult` for active model; run 4s probe if unprobed off the router thread.
  - [x] `on_session_start`: Pass `supports_tools` boolean flag into `Harness::new_modular`.
  - [x] `on_session_start`: Emit `NotificationCreated` warning if tool calling is unsupported or probe timed out.
- [x] `app/src-tauri/src/services/harness/orchestrator/chassis.rs`
  - [x] `Harness::new_modular`: Accept `supports_tools: bool` parameter during initialization.

---

## Batch 6 — Persistence Worker & DB Integration
**Dependencies**: Batch 1

- [x] `app/src-tauri/src/persistence/worker.rs`
  - [x] `handle_event`: Add arm for `PersistenceEvent::ToolCallExecuted` calling `persist_tool_call`.
  - [x] `log_skipped_private_event`: Add arm for `PersistenceEvent::ToolCallExecuted` to log skipped tool calls under private mode.

---

## Batch 7 — Provider Adapter Normalization
**Dependencies**: Batch 1

- [x] `app/src-tauri/src/services/llm/transport/chat_completions.rs`
  - [x] Request payload: Serialize registered tools into OpenAI function calling format and serialize messages with tool calls and tool responses.
  - [x] Stream reader: Accumulate chunked `delta.tool_calls` JSON buffers across streaming choices.
  - [x] Stream reader: Emit `LlmStreamEvent::ToolCall(CanonicalToolCall)` once full tool argument payload is received, validated, and normalized.

---

## Batch 8 — System Architecture Remediation & Spec Harmonization
**Dependencies**: Batches 0–7

- [x] Sub-batch 8.1 — Pure-Move Orchestrator Refactor (§9.1 & §9.2)
  - [x] Decompose `services/harness/orchestrator/` into named step files + `loop.rs` owner (`mod.rs`, `chassis.rs`, `loop.rs`, `intake.rs`, `compaction.rs`, `assemble.rs`, `dispatch.rs`, `stream.rs`, `tools.rs`, `phase.rs`, `finalize.rs`).
  - [x] Restore `PromptBuilderStage::build_generation_request` in `stages/prompt.rs` as sole assembly authority.
  - [x] Verify clean build with `cargo check`.
- [x] Sub-batch 8.2 — Audio Stream & Intent Integrity (A.1, A.2)
  - [x] `stages/streaming/router.rs`: Buffer un-synthesized clauses to prevent prefix token leakage.
  - [x] `orchestrator/tools.rs`: Drop prefix text unconditionally on `ToolCallReceived`; eliminate pre-tool `TurnResponse` audio flush.
  - [x] `orchestrator/tools.rs` (`handle_terminal_tool`): Update `TurnAccumulator.assistant_response` with `spoken_response` before emitting `LlmFinished` (N1).
- [x] Sub-batch 8.3 — Invariant 13 & Barge-In Persistence (B.1, B.2, G.2, G.4)
  - [x] `orchestrator/finalize.rs` (`handle_turn_cancelled`): Commit non-empty `partial_text` to history and DB; rollback staged user turn if empty.
  - [x] `pipeline/assistant/interrupt.rs`: Remove duplicate `TurnCompleted` dispatch on barge-in (N13).
  - [x] `orchestrator/tools.rs`: Check turn `CancellationToken` after `execute_tool` returns; return `Cancelled` on interruption (G.2).
  - [x] `pipeline/assistant/llm.rs` (`on_llm_finished`): Roll back staged user turn on empty finish (G.4).
- [x] Sub-batch 8.4 — Tool Gating, Budget Lifecycle & Compaction (B.3, C.2, C.3, D.3, G.3, G.6)
  - [x] `stages/tools/registry.rs`: Change `respond_and_set_title` gate to session-local first turn (`history.messages().len() <= 2`) (N2).
  - [x] `stages/budget.rs` / `orchestrator/loop.rs`: Include scratchpad AND tool schemas in `ContextBudgetStage` utilization evaluations (C.2, G.6).
  - [x] `orchestrator/loop.rs`: Connect `settings.working_memory` to `ToolFilter.memory_retrieval_enabled` (C.3).
  - [x] `orchestrator/compaction.rs`: Fix `trigger_kind` from `"inline"` to `"critical"` (N5). Add `has_played_filler` check.
  - [x] `orchestrator/loop.rs`: Bound reentrant loop to 5 tool iterations + 1 final tool-free text pass (G.3).
- [x] Sub-batch 8.5 — Retrieval Implementation & Persistence Hardening (C.4, D.1, D.2, D.4, D.5, B.4, G.1, G.6)
  - [x] `stages/tools/memory.rs`: Implement hybrid RRF episodic retrieval ($k=60$) excluding `personal` tag (N3).
  - [x] `stages/tools/title.rs`: Gate `set_session_title` on `!app_state.telemetry.is_private_mode` (D.2, G.6).
  - [x] `persistence/schema.rs`: Add `idx_tool_calls_created` index to migration v5 (D.4).
  - [x] `services/llm/transport/chat_completions.rs`: Reject invalid JSON arguments; emit sanitized `ToolResult { is_error: true }` (N11).
  - [x] `pipeline/assistant/session.rs`: Make capability probe non-blocking with `session_starting = true` (B.4).
  - [x] `services/llm/catalog/probe.rs` & transports: Check `models_manifest.json` for embedded models; parse tool calls; explicitly gate non-OpenAI transports (D.1, G.1).
