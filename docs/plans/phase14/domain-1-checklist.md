# Domain 1 Implementation Checklist

### Batch 1: Core Types, Enums & Database Schema (ZBC)
*Depends on: None*
- [x] `app/src-tauri/src/services/llm/provider.rs`
  - Add `Thinking(String)` variant to `enum LlmStreamEvent`.
  - Remove `tool_flow` field from `struct CanonicalToolDefinition`.
  - Delete `enum ToolFlow` and its `Terminal` and `NonTerminal` variants.
- [x] `app/src-tauri/src/services/llm/actor.rs`
  - Remove all matching arms and references to `ToolFlow`.
- [x] `app/src-tauri/src/persistence/schema.rs`
  - Bump `SCHEMA_VERSION` constant from 15 to 16.
  - Remove `tool_kind TEXT NOT NULL` from `session_tool_calls` table DDL.
  - Add migration block for `current_version < 16` executing `ALTER TABLE session_tool_calls DROP COLUMN tool_kind;`.
- [x] `app/src-tauri/src/persistence/tool_calls.rs`
  - Remove `tool_kind` parameter from `persist_tool_call` function.
  - Remove `tool_kind` from the SQL `INSERT INTO session_tool_calls` statement.
- [x] `app/src-tauri/tests/database_persistence_boundary_test.rs`
  - Update test queries and assertions to drop `tool_kind`.

---

### Batch 2: Network Ingress Provider Stream Adapters
*Depends on: Batch 1*
- [x] `app/src-tauri/src/services/llm/providers/chat_completions.rs`
  - Parse `reasoning_content` delta field in streaming chunks to yield `LlmStreamEvent::Thinking`.
  - Yield standard text deltas as `LlmStreamEvent::Token`.
- [x] `app/src-tauri/src/services/llm/providers/ollama.rs`
  - Parse `thought` / reasoning fields in Ollama streaming chunks to yield `LlmStreamEvent::Thinking`.
- [x] `app/src-tauri/src/services/sidecar/mod.rs` (and `think_tags.rs`)
  - Add state-machine stripper for `<think>...</think>` tags to route tokens to `Thinking` vs `Token`.

---

### Batch 3: Universal Tool Cleanup & SetTitle Migration
*Depends on: Batch 1*
- [x] `app/src-tauri/src/services/harness/stages/tools/search_memory.rs`
  - Remove `spoken_filler` from parameters JSON schema.
  - Remove `spoken_filler` extraction logic and `ToolFlow` trait implementation.
- [x] `app/src-tauri/src/services/harness/stages/tools/web_search.rs`
  - Remove `spoken_filler` from parameters JSON schema.
  - Remove `spoken_filler` extraction logic and `ToolFlow` trait implementation.
- [x] `app/src-tauri/src/services/harness/stages/tools/set_title.rs`
  - Create new `SetTitleTool` struct implementing `ToolDefinition`.
  - Accept parameter schema `{"title": "..."}` and execute title update.
- [x] `app/src-tauri/src/services/harness/stages/tools/respond_and_set_title.rs`
  - Delete file (ZBC removal).
- [x] `app/src-tauri/src/services/harness/stages/tools/mod.rs`
  - Remove `flow(&self)` method from `pub trait ToolDefinition`.
  - Register `SetTitleTool` and unregister `RespondAndSetTitleTool`.

---

### Batch 4: Router Demuxing & Preamble Preservation
*Depends on: Batch 1, Batch 2, Batch 3*
- [x] `app/src-tauri/src/services/harness/stages/streaming/router.rs`
  - Add `thinking_buffer: String` to stream accumulator.
  - Handle `LlmStreamEvent::Thinking`: append to `thinking_buffer` without emitting token IPC or pushing to chunker.
  - Handle `LlmStreamEvent::Token`: push to `ClauseChunker` with `AudioIntent::InterimFiller` (if `allow_tools == true`) or `AudioIntent::TurnResponse` (if `allow_tools == false`).
  - Handle `LlmStreamEvent::ToolCall`: append to `tool_calls` batch accumulator; continue reading stream until `Finished`.
  - Delete `acc.chunker.clear()` preamble wipe logic.
  - Update `StreamPassOutcome` enum: include `thinking_text` and `preamble_text`.
- [x] `app/src-tauri/src/services/harness/stages/orchestration/steps.rs`
  - Handle `StreamPassOutcome::ToolCallsReceived`: commit preamble text to dialogue history as assistant turn.
  - Transition interaction state to `InteractionState::Working` with active envelope.
  - Raise loop ceiling from 5 to `MAX_TOOL_STEPS = 20`.

---

### Batch 5: Two-Key Quiescence Latch & State Authority
*Depends on: Batch 4*
- [x] `app/src-tauri/src/services/harness/loop.rs`
  - Mark `turn_open = false` upon cognitive turn completion and emit `VoxEvent::LlmFinished`.
- [x] `app/src-tauri/src/pipeline/assistant/llm.rs`
  - In `on_llm_finished`: clear `turn_open`; transition to `Speaking` if synthesis or playback is active; transition to `Ready` if audio already drained.
- [x] `app/src-tauri/src/pipeline/assistant/playback.rs`
  - In `on_playback_started`: keep `Thinking`/`Working` state if `turn_open == true` and intent is `InterimFiller`; transition to `Speaking` if `turn_open == false` or intent is `TurnResponse`.
  - In `on_playback_finished`: return early if `turn_open == true`; transition to `Ready` if `turn_open == false && pending_synthesis_jobs == 0` regardless of chunk intent tag.

---

### Batch 6: Verification & Compile Gate
*Depends on: Batch 5*
- [x] `app/src-tauri/scripts/verify-targets.sh`
  - Verify clean compilation with zero warnings on all supported targets.
- [x] `app/src-tauri/tests/harness_pipeline_test.rs`
  - Verify 0-tool conversational turns transition through `Thinking` -> `Speaking` -> `Ready`.
  - Verify 1-tool turns preserve preamble speech and transition `Thinking` -> `Working` -> `Speaking` -> `Ready`.
  - Verify multi-tool batches accumulate and execute without truncation.
