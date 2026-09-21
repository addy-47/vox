# Phase 12.1 — Agentic Vox Execution Checklist

## Batch 0 — Orchestrator Decoupling & Non-Terminal Phase Unification
**Dependencies**: None

- [ ] `app/src-tauri/src/services/harness/orchestrator.rs` [DELETE]
  - [ ] Remove monolithic single file.
- [ ] `app/src-tauri/src/services/harness/orchestrator/mod.rs` [NEW]
  - [ ] `Harness`: Define struct with state fields, constructors (`new_modular`), and domain accessors.
  - [ ] Re-export submodules (`turn`, `phase`, `barge_in`).
- [ ] `app/src-tauri/src/services/harness/orchestrator/phase.rs` [NEW]
  - [ ] `NonTerminalTrigger`: Define enum (`Compaction`, `NonTerminalTool { tool_name: String, call_id: String }`).
  - [ ] `NonTerminalPhase`: Define struct (`trigger`, `filler_phrase: Option<String>`).
  - [ ] `Harness::enter_non_terminal_phase`: Centralized helper executing `transition(Working)` and dispatches `AudioIntent::InterimFiller` using `filler_phrase` (with fallback to `select_filler_phrase`).
- [ ] `app/src-tauri/src/services/harness/orchestrator/turn.rs` [NEW]
  - [ ] `Harness::execute_turn`: Sequential lifecycle coordinator with private phase helpers.
  - [ ] `phase_prompt_budget`: Phase 1 & Phase 2 prompt assembly and budget check.
  - [ ] `phase_compaction`: Phase 3 compaction check, refactored to call `enter_non_terminal_phase`.
  - [ ] `phase_generate_and_stream`: Phase 4 & Phase 5 generation and streaming dispatch.
- [ ] `app/src-tauri/src/services/harness/orchestrator/barge_in.rs` [NEW]
  - [ ] `Harness::handle_turn_cancelled`: Single-writer cancellation handling (Invariant 13).
- [ ] `app/src-tauri/src/services/harness/mod.rs`
  - [ ] Update `pub mod orchestrator;` to reference directory module.

---

## Batch 1 — Canonical Types & Shared Interface Expansion
**Dependencies**: Batch 0

- [ ] `app/src-tauri/src/services/llm/provider.rs`
  - [ ] `ToolFlow`: Define enum with `Terminal` and `NonTerminal` variants.
  - [ ] `CanonicalToolDefinition`: Define struct with `name: String`, `description: String`, `parameters: serde_json::Value`, `flow: ToolFlow`.
  - [ ] `CanonicalToolCall`: Define struct with `id: String`, `name: String`, `arguments: serde_json::Value`.
  - [ ] `LlmStreamEvent`: Add variant `ToolCall(CanonicalToolCall)`.
  - [ ] `GenerationRequest`: Add field `tools: Option<Vec<CanonicalToolDefinition>>`.
- [ ] `app/src-tauri/src/services/llm/actor.rs`
  - [ ] `LlmResponse`: Add variant `ToolCall(CanonicalToolCall)`.
  - [ ] `handle_generate`: Forward `LlmStreamEvent::ToolCall` stream events to `LlmResponse::ToolCall`.
- [ ] `app/src-tauri/src/services/harness/mod.rs`
  - [ ] `Role`: Add variant `Role::Tool` with `Display` output `"tool"`.
  - [ ] `ChatMessage`: Add optional fields `tool_call_id: Option<String>` and `tool_calls: Option<Vec<CanonicalToolCall>>`.
- [ ] `app/src-tauri/src/persistence/mod.rs`
  - [ ] `PersistenceEvent`: Add variant `ToolCallExecuted` with `tool_flow` and metadata fields (mapping `ToolFlow::Terminal` $\to$ `"terminal"` and `ToolFlow::NonTerminal` $\to$ `"non_terminal"` for `tool_kind` DB column).
- [ ] `app/src-tauri/src/persistence/schema.rs`
  - [ ] `SCHEMA_VERSION`: Bump from 4 to 5.
  - [ ] `V2_TABLE_STATEMENTS`: Add DDL for `session_tool_calls` table and indexes.
  - [ ] `test_schema_initialization`: Update schema version and table count assertion.
- [ ] `app/src-tauri/src/persistence/tool_calls.rs` [NEW]
  - [ ] `persist_tool_call`: Insert record into `session_tool_calls` with self-healing session insert.

---

## Batch 2 — Synthesis Guard Latch (`turn_open` / `drained_while_open`)
**Dependencies**: None

- [ ] `app/src-tauri/src/pipeline/atomics.rs`
  - [ ] `PipelineAtomics`: Add fields `turn_open: AtomicBool` and `drained_while_open: AtomicBool`.
  - [ ] `PipelineAtomics::new`: Initialize both atomics to `false`.
  - [ ] `PipelineAtomics::reset`: Reset both atomics to `false`.
  - [ ] `PipelineAtomics` accessors: Add `set_turn_open`, `clear_turn_open`, `is_turn_open`, `set_drained_while_open`, `clear_drained_while_open`, `is_drained_while_open`.
- [ ] `app/src-tauri/src/pipeline/assistant/playback.rs`
  - [ ] `on_playback_started`: Clear `drained_while_open` when `AudioIntent::TurnResponse` begins playback.
  - [ ] `on_playback_finished`: If `turn_open` is true, latch `drained_while_open = true`; only transition to `Ready` if `turn_open` is false and synthesis queue is empty.
- [ ] `app/src-tauri/src/pipeline/assistant/llm.rs`
  - [ ] `on_llm_finished`: Clear `turn_open = false`, and if `drained_while_open` is true with empty synthesis queue, trigger transition to `Ready`.
- [ ] `app/src-tauri/src/pipeline/assistant/transcript.rs`
  - [ ] `on_transcript_final`: Set `turn_open = true` before dispatching turn execution to Harness.
- [ ] `app/src-tauri/src/pipeline/assistant/interrupt.rs`
  - [ ] `on_interrupt`: Clear `turn_open = false` and `drained_while_open = false`.
- [ ] `app/src-tauri/src/pipeline/assistant/session.rs`
  - [ ] `on_pause_session`: Clear `turn_open = false` and `drained_while_open = false`.
  - [ ] `on_end_session`: Clear `turn_open = false` and `drained_while_open = false`.

---

## Batch 3 — Tool Execution Stage & Scratchpad (`stages/tools/`)
**Dependencies**: Batch 1

- [ ] `app/src-tauri/src/services/harness/mod.rs`
  - [ ] Add `pub const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(10);`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/mod.rs` [NEW]
  - [ ] `ToolDefinition`: Define async trait for name, description, schema, flow (`ToolFlow`), and execution.
  - [ ] `ToolResult`: Define struct with `content: String` and `is_error: bool`.
  - [ ] `ToolExecutionContext`: Define execution context bundle (`session_id`, `turn_id`, `cancel_token`, `db`, `persist_tx`).
- [ ] `app/src-tauri/src/services/harness/stages/tools/registry.rs` [NEW]
  - [ ] `ToolRegistry`: Registry storing registered `ToolDefinition` instances.
  - [ ] `ToolRegistry::register`: Add tool to internal map.
  - [ ] `ToolRegistry::schemas`: Filter and return `Vec<CanonicalToolDefinition>` accepting `ToolFilter { turn_id: u32, title_is_unset: bool }` (`respond_and_set_title` turn 1 only; `search_memory` always).
- [ ] `app/src-tauri/src/services/harness/stages/tools/executor.rs` [NEW]
  - [ ] `ToolExecutor::execute`: Execute tool with `tokio::time::timeout(TOOL_EXECUTION_TIMEOUT, ...)` and cancellation guard, returning `(ToolResult, Duration)`.
  - [ ] `ToolExecutor::dispatch_persistence`: Send `PersistenceEvent::ToolCallExecuted` over channel.
- [ ] `app/src-tauri/src/services/harness/stages/tools/title.rs` [NEW]
  - [ ] `RespondAndSetTitleTool`: Implement `ToolDefinition` for single-pass terminal title and response delivery.
  - [ ] `RespondAndSetTitleTool::execute`: Perform direct awaited DB title update and broadcast `IpcEvent::SessionsChanged`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/memory.rs` [NEW]
  - [ ] `MemorySearchTool`: Implement `ToolDefinition` stub for non-terminal episodic search with `spoken_filler`.
- [ ] `app/src-tauri/src/services/harness/stages/mod.rs`
  - [ ] Module tree: Add `pub mod tools;`.

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

- [ ] `app/src-tauri/src/pipeline/assistant/session.rs`
  - [ ] `on_session_start`: Check `ModelProbeResult` for active model; run 4s probe if unprobed off the router thread.
  - [ ] `on_session_start`: Pass `supports_tools` boolean flag into `Harness::new_modular`.
  - [ ] `on_session_start`: Emit `NotificationCreated` warning if tool calling is unsupported or probe timed out.
- [ ] `app/src-tauri/src/services/harness/orchestrator/mod.rs`
  - [ ] `Harness::new_modular`: Accept `supports_tools: bool` parameter during initialization.

---

## Batch 6 — Persistence Worker & DB Integration
**Dependencies**: Batch 1

- [ ] `app/src-tauri/src/persistence/worker.rs`
  - [ ] `handle_event`: Add arm for `PersistenceEvent::ToolCallExecuted` calling `persist_tool_call`.

---

## Batch 7 — Provider Adapter Normalization
**Dependencies**: Batch 1

- [ ] `app/src-tauri/src/services/llm/providers/openai.rs`
  - [ ] Stream reader: Accumulate chunked `delta.tool_calls` JSON buffers.
  - [ ] Stream reader: Emit `LlmStreamEvent::ToolCall(CanonicalToolCall)` once full tool argument payload is received and validated.
