# Phase 12.1 — Agentic Vox Implementation Plan

> Target: Normalized LLM event stream, tool execution chassis, `respond_and_set_title` foundational tool, turn-local ephemeral scratchpad, `turn_open` synthesis guard latch, `session_tool_calls` DB ledger, and session boot capability gating.

---

## Resolved Questions

All questions below were resolved during the spec synchronization and Claude review phases. No open assumptions remain.

| # | Question | Resolution |
|---|---|---|
| 1 | Where does `session_id` come from for tool call persistence? | `state.conversation_id.load(Relaxed) as i64` — same as `TurnCompleted`. |
| 2 | Does `Working` exist in `InteractionState`? | Yes: variant `8` in [`state.rs:97`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/state.rs#L97). |
| 3 | Does `AudioIntent::InterimFiller` exist? | Yes: variant `1` in [`events.rs:17`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/events.rs#L17). Playback handlers already gate on it correctly. |
| 4 | Does capability probe infrastructure exist? | Yes: `probe_capabilities` in [`probe.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/catalog/probe.rs) + `supports_tools` field on `ModelProbeResult` and `ModelSpec`. |
| 5 | Does the `session_tool_calls` table exist in code? | **No**. Only in `db-spec.md`. Must be created. |
| 6 | Does `ToolCallExecuted` exist in `PersistenceEvent`? | **No**. Only in `db-spec.md`. Must be added. |
| 7 | Does the `turn_open` / `drained_while_open` latch exist? | **No**. Must be added to `PipelineAtomics`. |
| 8 | Does `LlmResponse::ToolCall` exist? | **No**. Currently only `Token`, `Finished`, `Cancelled`, `Error`. Must be added. |
| 9 | Does `LlmStreamEvent::ToolCall` exist? | **No**. Currently only `Token`, `Finished`. Must be added. |
| 10 | Does `GenerationRequest` have a `tools` field? | **No**. Must be added. |
| 11 | Does `Role::Tool` exist in `ChatMessage`? | **No**. Must be added. |
| 12 | Does `stages/tools/` exist? | **No**. Entirely new directory. |
| 13 | Is the `Cancelled` branch in Harness already the single writer? | **Partially**. The current orchestrator does rollback on cancel, but does not inspect partial text or commit non-empty partials. Spec requires the enhanced branch (Invariant 13). |
| 14 | Who handles `turn_open` clearing on interrupt? | `on_interrupt` in [`interrupt.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/interrupt.rs) — must be updated to clear the new atomics. |
| 15 | Gap 1: Pre-tool partial text disposition? | If `ToolCallReceived` has non-empty `partial_text`: for NonTerminal tools, flush `partial_text` through `ClauseChunker` $\to$ `TtsActor` as `AudioIntent::TurnResponse` before entering `NonTerminalPhase` & playing filler. For Terminal tools, discard `partial_text` (superseded by `spoken_response`). |
| 16 | Gap 2: Terminal `spoken_response` routing? | Route through `TextNormalizer` $\to$ `ClauseChunker` $\to$ `TtsActor` as `AudioIntent::TurnResponse` for optimal TTFA and prosody morphing on longer responses (do not bypass chunker). |
| 17 | Gap 3: Tool execution timeout? | Pinned at `TOOL_EXECUTION_TIMEOUT = Duration::from_secs(10)` as a harness constant in `services/harness/mod.rs`. Wrapped via `tokio::time::timeout`. |
| 18 | Gap 4: `tool_kind` DB serialization? | `PersistenceEvent::ToolCallExecuted` maps `ToolFlow::Terminal` $\to$ `"terminal"` and `ToolFlow::NonTerminal` $\to$ `"non_terminal"` to match `db-spec.md` column `tool_kind`. |
| 19 | Gap 5: `has_played_filler` scope across loops? | `has_played_filler` strictly gates only harness-owned compaction filler (at most once per turn). Model-provided `spoken_filler` in NonTerminal tool calls is dispatched per tool invocation and is NOT gated or suppressed by `has_played_filler`. |
| 20 | Gap 6: Spec §9.1 directory tree? | Harmonized: `harness-spec.md` §9.1 updated to reflect `orchestrator/` module directory layout (`mod.rs`, `turn.rs`, `phase.rs`, `barge_in.rs`). |
| 21 | Gap 7: Schema injection filtering logic? | `ToolRegistry::schemas(filter)` accepts `ToolFilter { turn_id: u32, title_is_unset: bool }`. `respond_and_set_title` is injected only on Turn 1 when `title_is_unset == true`. `search_memory` is injected on all turns when `supports_tools == true`. |
| 22 | Gap 8: Context exclusion for Terminal tools? | On Terminal tool resolution, zero `ChatMessage` records (neither tool call nor tool role response) are appended to history or scratchpad. Only `spoken_response` text is committed as assistant response in `ConversationHistoryStage`. |

---

## Proposed Changes — Batched by Shared Blast Radius

> [!IMPORTANT]
> **Build expectation key:**  
> 🟢 = Green build throughout batch  
> 🟡 = Red mid-batch, green on completion  
> ⛔ = Structurally blocked until dependency completes

---

### Batch 0 — Orchestrator Decoupling & Non-Terminal Phase Unification 🟢

**Blast radius**: `services/harness/orchestrator.rs` refactored to `services/harness/orchestrator/`. Zero downstream API changes (`Harness::execute_turn` and `Harness::new_modular` retain exact signatures).

**Dependency**: None. Must land before Batch 1 to establish clean file boundaries.

**Rationale**:
- `orchestrator.rs` is 682 lines and contains a 320-line `execute_turn` method, violating `backend-style-guide.md` §2 (<600 lines) and §4 (<50 lines per function).
- Adding tool execution, reentrant loop handling, and scratchpad management directly to `orchestrator.rs` would balloon it past 1,000 lines.
- Compaction currently couples state transitions and interim filler audio inside an ad-hoc `if can_compact` block. This violates `harness-spec.md §5.1` (Generic Non-Terminal Phase Contract).
- Batch 0 decouples `orchestrator.rs` into a clean directory and introduces the canonical `NonTerminalPhase` contract, refactoring inline compaction to use it before tools are introduced.

#### Changes

##### [DELETE] [`orchestrator.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator.rs)
- Remove single-file orchestrator.

##### [NEW] [`orchestrator/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/mod.rs)
- `Harness` struct definition, fields, and constructors (`new_modular`).
- Domain constants, accessors, and module re-exports. Zero business logic.

##### [NEW] [`orchestrator/phase.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/phase.rs)
- `NonTerminalTrigger` enum (`Compaction`, `NonTerminalTool { tool_name: String, call_id: String }`).
- `NonTerminalPhase` struct (`trigger`, `filler_phrase: Option<String>`).
- `Harness::enter_non_terminal_phase(&self, phase, req)`: Uniformly executes `transition(Working)` and dispatches `AudioIntent::InterimFiller` to `tts_tx` using `filler_phrase` (with fallback to `select_filler_phrase`).

##### [NEW] [`orchestrator/turn.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/turn.rs)
- `Harness::execute_turn` sequential lifecycle:
  - Private helper `phase_prompt_budget`: Phase 1 & 2.
  - Private helper `phase_compaction`: Phase 3 (refactored to call `enter_non_terminal_phase`).
  - Private helper `phase_generate_and_stream`: Phase 4 & 5.
- Deconstructs the 320-line monolith into focused, testable phase functions.

##### [NEW] [`orchestrator/barge_in.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/barge_in.rs)
- `Harness::handle_turn_cancelled`: Single-writer cancellation branch (Invariant 13).

##### [MODIFY] [`mod.rs` (harness)](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/mod.rs)
- Update `pub mod orchestrator;` to reference the new directory module.

**Done when**: `cargo check` passes. All existing tests pass with zero behavior change.

---

### Batch 1 — Canonical Types & Shared Interface Expansion 🟡

**Blast radius**: Every downstream consumer of `LlmStreamEvent`, `LlmResponse`, `ChatMessage`, `Role`, `GenerationRequest`, and `PersistenceEvent`. This is the widest interface change and must land first.

**Dependency**: Batch 0.

**Rationale**: Adding `ToolCall` variants to the stream events, `Role::Tool` to message types, `tools` field to `GenerationRequest`, and `ToolCallExecuted` to `PersistenceEvent` touches types consumed across `services/llm/`, `services/harness/`, `pipeline/`, and `persistence/`. All downstream code must compile against the new shapes.

#### Changes

##### [MODIFY] [`provider.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/provider.rs)
- Add canonical tool types:
  ```rust
  pub enum ToolFlow { Terminal, NonTerminal }
  pub struct CanonicalToolDefinition { name: String, description: String, parameters: serde_json::Value, flow: ToolFlow }
  pub struct CanonicalToolCall { id: String, name: String, arguments: serde_json::Value }
  ```
- Add `ToolCall(CanonicalToolCall)` variant to `LlmStreamEvent`.
- Add `tools: Option<Vec<CanonicalToolDefinition>>` to `GenerationRequest`.

##### [MODIFY] [`actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/actor.rs)
- Add `ToolCall(CanonicalToolCall)` variant to `LlmResponse`.
- Update `handle_generate` stream loop to forward `LlmStreamEvent::ToolCall` → `LlmResponse::ToolCall`.

##### [MODIFY] [`mod.rs` (harness)](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/mod.rs)
- Add `Role::Tool` variant to `Role` enum, with `Display` impl returning `"tool"`.
- Add `tool_call_id: Option<String>` field to `ChatMessage` (for `Role::Tool` messages carrying the invocation ID back to the model).
- Add `tool_calls: Option<Vec<CanonicalToolCall>>` to `ChatMessage` (for `Role::Assistant` messages containing tool call requests).

##### [MODIFY] [`mod.rs` (persistence)](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/mod.rs)
- Add `ToolCallExecuted { id, session_id, turn_id, tool_name, tool_flow, arguments, result, is_error, duration_ms, created_at }` variant to `PersistenceEvent`.
- **Serialization Mapping (Gap 4)**: The persistence worker maps `ToolFlow::Terminal` $\to$ `"terminal"` and `ToolFlow::NonTerminal` $\to$ `"non_terminal"` for the `tool_kind` column in SQLite to match `db-spec.md`.

##### [MODIFY] [`schema.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/schema.rs)
- Add `session_tool_calls` CREATE TABLE to `V2_TABLE_STATEMENTS`.
- Bump `SCHEMA_VERSION` to `5`.
- Add the table to the test assertions.

##### [NEW] [`persistence/tool_calls.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/tool_calls.rs)
- `persist_tool_call(conn, event) -> Result<()>` — with self-healing `INSERT OR IGNORE INTO sessions` before the tool call insert.

**Done when**: `cargo check` passes. All existing tests pass. The new types exist but no consumer uses `ToolCall` variants yet.

---

### Batch 2 — Synthesis Guard Latch (`turn_open` / `drained_while_open`) 🟢

**Blast radius**: `PipelineAtomics`, `on_playback_started`, `on_playback_finished`, `on_llm_finished`, `on_interrupt`, `on_pause`, `on_end`.

**Dependency**: None (independent of Batch 1).

**Rationale**: The `turn_open` latch prevents premature `Speaking → Ready` transitions when playback finishes before `LlmFinished` arrives. It is required by the spec for correct turn lifecycle and is independent of tool calling.

#### Changes

##### [MODIFY] [`atomics.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/atomics.rs)
- Add `turn_open: AtomicBool` and `drained_while_open: AtomicBool` to `PipelineAtomics`.
- Add helper methods: `set_turn_open()`, `clear_turn_open()`, `set_drained_while_open()`, `clear_drained_while_open()`, `is_turn_open()`, `is_drained_while_open()`.

##### [MODIFY] [`playback.rs` (pipeline/assistant)](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/playback.rs)
- `on_playback_started`: Clear `drained_while_open` on `TurnResponse` playback onset.
- `on_playback_finished`: Instead of always transitioning `Speaking → Ready`, check:
  - If `turn_open == true`: set `drained_while_open = true`, stay in `Speaking`.
  - If `turn_open == false` and `pending_synthesis_jobs == 0`: transition to `Ready`.

##### [MODIFY] [`llm.rs` (pipeline/assistant)](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/llm.rs)
- `on_llm_finished`: After persisting, clear `turn_open`. Then check: if `drained_while_open == true && pending_synthesis_jobs == 0`, transition to `Ready`.

##### [MODIFY] [`transcript.rs` (pipeline/assistant)](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/transcript.rs)
- Set `turn_open = true` when dispatching a valid transcript to `Harness::execute_turn`.

##### [MODIFY] [`interrupt.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/interrupt.rs)
- Clear `turn_open = false`, `drained_while_open = false` on interrupt.

##### [MODIFY] [`session.rs` (pipeline/assistant)](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/session.rs)
- Clear `turn_open = false`, `drained_while_open = false` on `PauseSession` and `EndSession`.

**Done when**: `cargo check` passes. Existing test suite passes. Latch clears correctly on all terminal paths.

---

### Batch 3 — Tool Execution Stage & Scratchpad (`stages/tools/`) 🟡

**Blast radius**: New directory. Consumers: `orchestrator` (Batch 4). No existing code depends on this.

**Dependency**: Batch 1 (needs `CanonicalToolDefinition`, `CanonicalToolCall`, `ToolFlow`).

**Rationale**: The tool registry, executor, and the two foundational tools are self-contained modules that can be built and unit-tested before wiring into the orchestrator.

#### Changes

##### [MODIFY] [`mod.rs` (harness)](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/mod.rs)
- Define `pub const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(10);` (Gap 3).

##### [NEW] [`stages/tools/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/tools/mod.rs)
- `ToolDefinition` trait: `name()`, `description()`, `parameters() -> serde_json::Value`, `flow() -> ToolFlow`, `execute(args, ctx) -> ToolResult`.
- `ToolResult { content: String, is_error: bool }`.
- `ToolExecutionContext { session_id, turn_id, cancel, db, app, persist_tx, ... }`.

##### [NEW] [`stages/tools/registry.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/tools/registry.rs)
- `ToolRegistry`: holds `HashMap<String, Box<dyn ToolDefinition>>`.
- `register(tool)`, `lookup(name) -> Option<&dyn ToolDefinition>`, `schemas(filter) -> Vec<CanonicalToolDefinition>`.
- **Filtering Logic (Gap 7)**: `ToolRegistry::schemas(filter: ToolFilter)` where `ToolFilter { turn_id: u32, title_is_unset: bool }`.
  - `respond_and_set_title`: Injected only when `turn_id == 1 && title_is_unset == true`.
  - `search_memory`: Injected when `supports_tools == true`.

##### [NEW] [`stages/tools/executor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/tools/executor.rs)
- `ToolExecutor::execute(call, registry, ctx) -> (ToolResult, Duration)`.
- **Timeout Wrap (Gap 3)**: Tool execution future is wrapped with `tokio::time::timeout(TOOL_EXECUTION_TIMEOUT, tool.execute(...))`. On timeout, returns `ToolResult { is_error: true, content: "Tool execution timed out after 10s".to_string() }`.
- On unknown tool / malformed args: returns `ToolResult { is_error: true, content: "Unknown tool: ..." }` — never panics or aborts.
- Dispatches `PersistenceEvent::ToolCallExecuted` via `persist_tx` after execution.

##### [NEW] [`stages/tools/title.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/tools/title.rs)
- `RespondAndSetTitleTool` implementing `ToolDefinition`.
- Flow: `ToolFlow::Terminal`.
- `execute`: writes title to DB via awaited persistence, emits `IpcEvent::SessionsChanged`.
- Parameters: `{ "spoken_response": { "type": "string", "description": "..." }, "title": { "type": "string", "description": "..." } }`. Required: `["spoken_response", "title"]`.

##### [NEW] [`stages/tools/memory.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/tools/memory.rs) — *Stub only*
- `MemorySearchTool` implementing `ToolDefinition`.
- Flow: `ToolFlow::NonTerminal`.
- Parameters: `{ "query": { "type": "string", "description": "..." }, "spoken_filler": { "type": "string", "description": "..." } }`. Required: `["query", "spoken_filler"]`.
- `execute`: Stub returns `ToolResult { content: "Memory search not yet implemented", is_error: true }`.
- Full RRF hybrid retrieval deferred to Phase 12.2.

##### [MODIFY] [`stages/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/mod.rs)
- Add `pub mod tools;` declaration.

**Done when**: `cargo check` passes. Unit tests for `ToolRegistry` registration and lookup pass. `RespondAndSetTitleTool` schema generation is correct.

---

### Batch 4 — Orchestrator Reentrant Loop & Stream Demuxing 🟡

**Blast radius**: `orchestrator`, `router.rs` (streaming), `transcript.rs`. This is the core behavioral change.

**Dependency**: Batches 0, 1, 2, 3.

**Rationale**: This batch wires the tool execution loop into `execute_turn`, changes `StreamRoutingStage::route_stream` to handle `LlmResponse::ToolCall`, and implements the single-pass terminal execution and reentrant cognitive loop.

#### Changes

##### [MODIFY] [`orchestrator/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/mod.rs)
- Add `tool_registry: ToolRegistry` to `Harness` struct, initialized with `RespondAndSetTitleTool` (and stubbed `MemorySearchTool`).
- Add `supports_tools: bool` field to `Harness`, read from capability cache at session boot.

##### [MODIFY] [`orchestrator/turn.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/turn.rs)
- In Phase 4 helper: Inject filtered tool schemas via `ToolRegistry::schemas(ToolFilter { turn_id, title_is_unset })` when `supports_tools == true` (Gap 7).
- Implement loop handling in Phase 6/7:
  1. Maintain turn-local `scratchpad: Vec<ChatMessage>`.
  2. Evaluate `StreamPassOutcome`:
     - `Completed { assistant_text }` — commit spoken turn, drop scratchpad.
     - `ToolCallReceived { partial_text, call }`:
       - **Pre-Tool Partial Text Handling (Gap 1)**:
         - If `ToolFlow::NonTerminal`: Flush `partial_text` through `TextNormalizer` $\to$ `ClauseChunker` $\to$ `TtsActor` as `AudioIntent::TurnResponse` *before* transitioning to `Working` and dispatching `spoken_filler`.
         - If `ToolFlow::Terminal`: Discard `partial_text` without chunker flush or TTS dispatch (`spoken_response` supersedes it entirely).
       - **If `ToolFlow::Terminal` (e.g. `respond_and_set_title`)**:
         - Extracts required `spoken_response`.
         - **TTS Routing (Gap 2)**: Route `spoken_response` through `TextNormalizer` $\to$ `ClauseChunker` $\to$ `TtsActor` as `AudioIntent::TurnResponse` for optimal TTFA and prosody morphing on longer responses.
         - Executes tool action asynchronously.
         - **Context Exclusion (Gap 8)**: Zero `ChatMessage` records appended to scratchpad or conversation history. Only `spoken_response` text is committed as assistant response in `ConversationHistoryStage`.
         - Turn completes in a **single pass** (zero loopback, zero second LLM call).
       - **If `ToolFlow::NonTerminal` (e.g. `search_memory`)**:
         - Extracts `spoken_filler` from call arguments.
         - Calls `self.enter_non_terminal_phase(&NonTerminalPhase::from_tool_call(&call), req)`.
         - **Filler Scope (Gap 5)**: Model-provided `spoken_filler` is played immediately as `AudioIntent::InterimFiller` and transitions to `Working`. It is NOT suppressed by `has_played_filler` (which gates compaction filler only).
         - Executes tool under `TOOL_EXECUTION_TIMEOUT = 10s`.
         - Appends assistant tool call and tool result to `scratchpad`.
         - Re-enters Phase 2 Step 4 (budget check with scratchpad).
  3. Bounded by `MAX_TOOL_ITERATIONS = 5`. On breach, strip tools and execute final generation pass.
  4. On cancellation: Call `barge_in::handle_turn_cancelled` (Invariant 13).

##### [MODIFY] [`router.rs` (streaming)](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs)
- Replace `route_stream` return type from `Result<String, String>` to `Result<StreamPassOutcome, String>`.
- Add `LlmResponse::ToolCall` match arm: return `StreamPassOutcome::ToolCallReceived { partial_text, call }` with accumulated partial text.
- On `Cancelled`: return `StreamPassOutcome::Cancelled { partial_text }` with the accumulated text so far.
- `StreamPassOutcome::Completed` carries the full assistant text.

##### [MODIFY] [`mod.rs` (harness)](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/mod.rs)
- Re-export `ToolRegistry`, `ToolFlow`, `ToolDefinition`, `TOOL_EXECUTION_TIMEOUT`.

**Done when**: `cargo check` passes. Integration test with a mock provider emitting `ToolCall` events verifies the single-pass terminal execution and reentrant non-terminal loop. Title generation single-pass test passes.

---

### Batch 5 — Session Boot Capability Gating 🟢

**Blast radius**: `session.rs` (pipeline/assistant), `Harness::new_modular`.

**Dependency**: Batches 1, 3 (needs `ToolRegistry` and `supports_tools` field).

**Rationale**: At session boot, the Harness must read the capability cache to determine if the active model supports tool calling. If unknown, a blocking probe runs with a 4s timeout. This gating determines whether tool schemas are injected.

#### Changes

##### [MODIFY] [`session.rs` (pipeline/assistant)](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/session.rs)
- In `on_session_start` (modular path): after model warm-up, read `ModelProbeResult` from capability cache.
- If `supports_tools` is `None`/unknown: run `probe_capabilities` with 4s timeout **off the router thread** (set `session_starting = true` flag on `AppState`).
- Pass resolved `supports_tools: bool` to `Harness::new_modular`.
- On probe timeout/error: default `supports_tools = false`, emit system notification via `NotificationParams { group_key: "model_tool_unsupported" }`.
- Only set `owner = Assistant` and transition to `Ready` after probe completes.

##### [MODIFY] [`orchestrator/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/mod.rs)
- `Harness::new_modular` accepts `supports_tools: bool` parameter and stores it.

**Done when**: Session boots with tool support flag correctly resolved. Models with `supports_tools = false` produce `GenerationRequest` with `tools = None`.

---

### Batch 6 — Persistence Worker & DB Integration 🟢

**Blast radius**: Persistence worker loop, `tool_calls.rs`.

**Dependency**: Batch 1 (needs `PersistenceEvent::ToolCallExecuted` variant and `tool_calls.rs`).

**Rationale**: The persistence background worker must handle the new `ToolCallExecuted` event variant and write to the `session_tool_calls` table.

#### Changes

##### [MODIFY] Persistence worker loop (file where `PersistenceEvent` is consumed)
- Add match arm for `ToolCallExecuted`: call `persist_tool_call(conn, event)`.

**Done when**: Tool call records appear in `session_tool_calls` table after a turn with tool invocation.

---

### Batch 7 — Provider Adapter Normalization (Deferred Detail) 🟢

> [!NOTE]
> Resolution tapers: This batch's exact file-and-symbol detail depends on Batch 4's final `CanonicalToolCall` shape. Coarser specification here is intentional.

**Blast radius**: `services/llm/transport/` (OpenAI-compat adapter).

**Dependency**: Batch 1 (canonical types).

**Rationale**: Provider adapters must translate their native tool call streaming frames into `LlmStreamEvent::ToolCall`. The OpenAI-compat adapter is the primary target since cloud providers (OpenAI, Gemini via OpenAI-compat) are the first tool-calling models.

#### Changes

- Accumulate streamed `tool_calls[]` delta fragments in the OpenAI-compat adapter.
- Emit `LlmStreamEvent::ToolCall(CanonicalToolCall)` once argument JSON is closed and validated.
- For embedded GGUF models: textual `<tool_call>` tag interception lives in the provider adapter ingress filter (deferred to Phase 12.2 unless embedded models gain tool support sooner).

**Done when**: A cloud model emitting tool calls produces correctly normalized `LlmResponse::ToolCall` events at the harness boundary.

---

## Verification Plan

### Automated Tests
```bash
# Full suite — should remain at 105+ tests baseline
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast

# Isolated new tests
cargo nextest run -E 'test(tool)' --release --nocapture --test-threads=1
cargo nextest run -E 'test(latch)' --release --nocapture --test-threads=1
cargo nextest run -E 'test(schema)' --release --nocapture --test-threads=1
```

### Manual Verification
- Start a session with a cloud model that has `supports_tools = true`.
- Verify `respond_and_set_title` fires on Turn 1, voice response plays in single pass, and title appears in sidebar.
- Verify Turn 2+ does NOT include `respond_and_set_title` schema.
- Verify the `turn_open` latch prevents premature `Ready` transitions during slow TTS.
- Inspect `session_tool_calls` table via `tursodb` to confirm tool call records are persisted.

---

## Risk Register

| Risk | Mitigation |
|---|---|
| `spawn_blocking` for stream routing may conflict with the reentrant loop needing async tool execution | The loop itself runs in async context; only individual stream passes use `spawn_blocking`. Tool execution happens between passes in async. |
| Tool call delta accumulation varies across providers | Batch 7 is isolated per-provider. Only OpenAI-compat is in scope for Phase 12.1. |
| Schema version bump (4→5) breaks existing DBs | `CREATE TABLE IF NOT EXISTS` + `SCHEMA_VERSION` check ensures additive migration. No destructive changes. |
| `turn_open` latch race between `LlmFinished` and `PlaybackFinished` | Both read/write atomics with `Relaxed` ordering, which is safe because both events are serialized through the single-writer Router FIFO. |

---

## Spec Coverage Confirmation

Every requirement from the active specifications has been mapped to a batch:

- ✅ Normalized `LlmResponse` stream (`TextDelta`, `ToolCall`, `Finished`, `Error`) — Batch 1
- ✅ Turn-local ephemeral scratchpad with drop-at-commit — Batch 4
- ✅ Reentrant cognitive loop (Phase 2 Step 4 re-entry for NonTerminal tools) — Batch 4
- ✅ `Terminal` vs `NonTerminal` execution dispatch — Batch 3 + 4
- ✅ `respond_and_set_title` foundational tool (single-pass with `spoken_response`) — Batch 3 + 4
- ✅ `search_memory` stub (with `spoken_filler`) — Batch 3
- ✅ `session_tool_calls` DB table + self-healing FK — Batch 1 + 6
- ✅ `turn_open` / `drained_while_open` synthesis guard — Batch 2
- ✅ Single-writer barge-in persistence (Invariant 13) — Batch 4
- ✅ Session boot capability gating with 4s probe — Batch 5
- ✅ Provider ingress normalization — Batch 7
- ✅ `MAX_TOOL_ITERATIONS = 5` recursion guard — Batch 4
- ✅ Tool failure contract (never abort, return error to model) — Batch 3

**Deliberately deferred** (per spec §10 / tools-spec §6):
- Interactive capability probe with retry/backoff
- Adaptive recursion guard (token-budget-aware)
- Multi-turn session rollback engine
- Compensating action rollback registry
- MCP multi-server daemon lifecycle
- `search_memory` full RRF hybrid retrieval implementation
