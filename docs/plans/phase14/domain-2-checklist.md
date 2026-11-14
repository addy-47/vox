# Domain 2 Implementation Checklist

### Batch 1: Public Contracts, Schema Migration v17 & Persistence Engine
*Depends on: None*
- [ ] `app/src-tauri/src/persistence/schema.rs`
  - Bump `SCHEMA_VERSION` constant from 16 to 17.
  - Add `session_steps` DDL to `SCHEMA_TABLE_STATEMENTS`.
  - Update `session_tool_calls` DDL in `SCHEMA_TABLE_STATEMENTS` to include `step_id` and `step_index`.
  - Add migration block for `current_version < 17` executing 12-step table rebuild of `session_tool_calls` with `PRAGMA foreign_keys = OFF`.
- [ ] `app/src-tauri/src/persistence/mod.rs`
  - Add `StepCompleted` variant to `enum PersistenceEvent`.
  - Update `ToolCallExecuted` variant of `enum PersistenceEvent` to include `step_id: Option<String>` and `step_index: u32`.
  - Declare and export `steps::persist_step`.
- [ ] `app/src-tauri/src/persistence/steps.rs`
  - Create new module implementing `persist_step(conn: &Connection, event: &PersistenceEvent) -> Result<()>`.
- [ ] `app/src-tauri/src/persistence/tool_calls.rs`
  - Update `persist_tool_call` to insert `step_id` and `step_index` into `session_tool_calls`.
- [ ] `app/src-tauri/src/persistence/worker.rs`
  - Add arm in `process_event` matching `PersistenceEvent::StepCompleted` and calling `persist_step`.
  - Add arm in `log_private_mode_skip` matching `PersistenceEvent::StepCompleted`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/mod.rs`
  - Add `pub enum ToolExecutionMode { Parallel, Exclusive }`.
  - Add `pub enum ToolControlAction { Continue, SuspendForUser { question: String, options: Option<Vec<String>> } }`.
  - Add `step_id: String` and `step_index: u32` fields to `struct ToolExecutionContext`.
  - Add `fn execution_mode(&self, _args: &Value) -> ToolExecutionMode` to `pub trait ToolDefinition`.
  - Add `fn handles_own_timeout(&self) -> bool` to `pub trait ToolDefinition`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/search_memory.rs`
  - Implement `execution_mode` returning `ToolExecutionMode::Parallel` on `MemorySearchTool`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/web_search.rs`
  - Implement `execution_mode` returning `ToolExecutionMode::Parallel` on `WebSearchTool`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/set_title.rs`
  - Implement `execution_mode` returning `ToolExecutionMode::Parallel` on `SetTitleTool`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/executor.rs`
  - Remove internal persistence dispatch from `execute_tool`.
  - Add `execute_batch` implementing wave partitioning, exclusive barrier draining, and fallback outcome synthesis on early exit.
  - Add `run_parallel_wave` evaluating wave futures concurrently via `futures_util::future::join_all`.

---

### Batch 2: History Invariants, Scratchpad Eradication & ReAct Coordinator Loop
*Depends on: Batch 1*
- [ ] `app/src-tauri/src/services/harness/stages/history.rs`
  - Add `commit_tool_step(&mut self, preamble_text: String, calls: Vec<CanonicalToolCall>, tool_results: Vec<(String, String)>)` to `ConversationHistoryStage`.
  - Add `assert_eq!(calls.len(), tool_results.len())` invariant check to `commit_tool_step`.
  - Update `rollback_last_user_turn` to check `last.role == Role::User` and ignore committed tool turns.
- [ ] `app/src-tauri/src/services/harness/stages/budget.rs`
  - Remove `scratchpad: &[ChatMessage]` argument from `calculate_tracked_tokens_with_extras`.
  - Remove `scratchpad: &[ChatMessage]` argument from `calculate_max_observation_tokens`.
- [ ] `app/src-tauri/src/services/harness/stages/prompt.rs`
  - Remove `scratchpad: &[ChatMessage]` argument from `build_generation_request`.
- [ ] `app/src-tauri/src/services/harness/steps.rs`
  - Remove `scratchpad` argument from `step4_assemble_request`.
  - Replace `step6_execute_tools` with `step6_execute_tool_batch`.
  - In `step6_execute_tool_batch`: generate `step_id`, invoke `ToolExecutor::execute_batch`, always invoke `commit_tool_step`, and queue `StepCompleted` before `ToolCallExecuted` on `persist_tx`.
  - Remove `scratchpad` argument from `step7_commit_completed`.
  - Remove `scratchpad` argument from `step7_handle_cancelled`.
- [ ] `app/src-tauri/src/services/harness/loop.rs`
  - Eradicate `let mut scratchpad: Vec<ChatMessage>` from `execute_loop_iterations`.
  - Configure `const MAX_TOOL_STEPS: usize = 20`.
  - Implement forced tool-free summary generation pass (`allow_tools = false`) upon reaching `MAX_TOOL_STEPS`.
  - Remove `scratchpad` arguments from `check_loop_budget` and `handle_pass_outcome`.

---

### Batch 3: Seam Binary Test Suite & Mutation Verification
*Depends on: Batch 1, Batch 2*
- [ ] `app/src-tauri/tests/common/harness.rs`
  - Add `BarrierTraceTool` test helper implementing `ToolDefinition` with `Arc<tokio::sync::Barrier>`.
- [ ] `app/src-tauri/tests/harness_loop_test.rs`
  - Add Subtest 1: `parallel_tool_wave_executes_concurrently_and_commits_in_declaration_order`.
  - Add Subtest 2: `exclusive_tool_acts_as_strict_barrier_between_parallel_waves`.
  - Add Subtest 3: `mid_turn_cancellation_on_step_two_preserves_step_one_history_and_forbids_user_rollback`.
  - Add Subtest 4: `step_zero_cancellation_before_speech_or_tools_rolls_back_unanswered_user_turn`.
  - Add Subtest 5: `step_ceiling_breach_disables_tools_and_forces_spoken_summary_pass`.
- [ ] `app/src-tauri/tests/database_persistence_boundary_test.rs`
  - Add Subtest 6: `multi_step_turn_persists_session_steps_and_foreign_key_linked_tool_calls`.
  - Add Subtest 7: `session_hard_delete_cascades_to_session_steps_and_session_tool_calls`.
