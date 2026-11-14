# Domain 2 Implementation Plan: ReAct Step Loop, Concurrency Barriers & Immediate Step Commits

---

## 1. Target End-State

Domain 2 delivers the reentrant **ReAct Cognitive Engine** for Vox Harness v2:
1. **Concurrency Barrier Scheduler:** `ToolExecutor::execute_batch` schedules tool batches emitted by the LLM into alternating waves: consecutive read-only/idempotent tools (`ToolExecutionMode::Parallel`, including `search_memory`, `web_search`, `read_file`, and `set_title`) execute concurrently via `futures_util::future::join_all`, while mutating tools (`ToolExecutionMode::Exclusive`, such as `write_file` and `bash`) form strict sequential barriers that drain preceding parallel waves before executing alone.
2. **Deterministic Declaration Order & 1:1 Bijection:** Tool outcomes are reassembled in the exact sequence declared by the model before committing to history. If batch execution is cancelled or suspended mid-batch, fallback observations (`"Tool execution cancelled"`) are synthesized for all unexecuted calls, enforcing a strict 1:1 `tool_calls` $\leftrightarrow$ `Role::Tool` bijection that prevents provider HTTP 400 errors.
3. **Eradication of Ephemeral Scratchpad:** The volatile `scratchpad: Vec<ChatMessage>` is completely removed from the harness. At each step boundary, assistant preambles and tool observations are committed immediately to `ConversationHistoryStage` via `commit_tool_step`.
4. **Protected Turn History Rollback Invariant:** User turn rollback (`rollback_last_user_turn`) is strictly restricted to Step 0 cancellations where zero assistant speech and zero tools were executed (`last.role == Role::User`). Once Step 1 tools begin or preamble speech is committed, `last.role == Role::Tool`, and rollback is forbidden. Completed steps and observations remain permanently preserved in conversation history.
5. **Relational Step Tracking in Turso DB (Option B):** Database schema version 17 introduces `session_steps` (`id`, `session_id`, `turn_id`, `step_index`, `preamble_text`, `tool_count`, `duration_ms`, `created_at`) and links `session_tool_calls` via `step_id` (`REFERENCES session_steps(id) ON DELETE CASCADE`) and `step_index`. `PersistenceEvent::StepCompleted` is dispatched strictly *before* child `PersistenceEvent::ToolCallExecuted` events to guarantee foreign key referential integrity in FIFO queue processing.
6. **Step Ceiling & Mandatory Summary Pass:** Configurable `MAX_TOOL_STEPS = 20`. If reached without a terminal answer, tools are disabled (`allow_tools = false`) and the coordinator executes a mandatory final summary generation pass, speaking the conclusion as `AudioIntent::TurnResponse`.

---

## 2. Resolved Architectural Decisions (Clarification Ledger)

1. **Foreign Key Insertion Order Paradox (Defect 1):**
   - *Issue:* In v1, `execute_tool` called `dispatch_persistence` directly inside each tool. Because `session_tool_calls.step_id` has a foreign key referencing `session_steps(id)`, inserting tool calls before `session_steps` exists crashes Turso DB with `FOREIGN KEY constraint failed`.
   - *Resolution:* Persistence dispatch is removed from inside `execute_tool`. `step6_execute_tool_batch` dispatches `PersistenceEvent::StepCompleted` first, followed immediately by child `PersistenceEvent::ToolCallExecuted` events. In FIFO processing, the parent row is guaranteed to exist before child rows are inserted.
   - *Migration:* Migration v17 uses the 12-step table-rebuild pattern with `PRAGMA foreign_keys = OFF` to add `step_id` and `step_index` to `session_tool_calls`.
2. **Provider HTTP 400 Orphaned `tool_call_id` Corruption (Defect 2):**
   - *Issue:* If the tool loop breaks early due to mid-batch cancellation or `SuspendForUser` (`ask_user`), `calls` contains 3 entries while `outcomes` contains only 1 or 2. Committing an assistant message with 3 `tool_calls` followed by only 1 or 2 `Role::Tool` messages permanently corrupts the session, causing OpenAI/Anthropic/vLLM APIs to reject all subsequent turns with HTTP 400 Bad Request.
   - *Resolution:* When `execute_batch` halts early, it synthesizes fallback outcomes for every remaining unexecuted call in `calls` with `content: "Tool execution cancelled..."` and `is_error: true`. `ConversationHistoryStage::commit_tool_step` enforces `assert_eq!(calls.len(), tool_results.len())`.
3. **Preamble Speech & Mid-Batch Cancellation Rollback Bug (Defect 3):**
   - *Issue:* If cancellation occurred during Step 1 tool execution, `step7_handle_cancelled` dropped the scratchpad and called `rollback_last_user_turn()`, falsely deleting the user's prompt even though preamble speech had already played and tools had executed.
   - *Resolution:* `step6_execute_tool_batch` **always** calls `commit_tool_step` before returning—even on cancellation. Because `commit_tool_step` pushes `Role::Assistant` and `Role::Tool` messages, `rollback_last_user_turn()` sees `last.role == Role::Tool` and safely no-ops.
4. **`set_title` Concurrency Classification (Defect 4):**
   - *Issue:* Marking `set_title` as `Exclusive` causes it to break parallel waves in batches like `[set_title, web_search, search_memory]`, forcing serial execution.
   - *Resolution:* `set_title` is classified as `ToolExecutionMode::Parallel`. It performs an asynchronous persistence dispatch and flips an in-memory boolean without mutating the filesystem or external environment.
5. **Deterministic Concurrency Verification:**
   - *Issue:* Wall-clock latency assertions (`elapsed < 150ms`) are flaky and banned by `testing-style-guide.md` §5.4.
   - *Resolution:* We implement `BarrierTraceTool` in `tests/common/harness.rs` using `tokio::sync::Barrier::new(N)`. Two or three parallel tools must rendezvous at the barrier; if polled concurrently in `join_all`, the barrier resolves in <1ms. If regressed to a serial loop, Tool 1 deadlocks and times out, providing a 100% deterministic mathematical proof of concurrency.

---

## 3. Subagent Orchestration Workflow (Red-Seam First / TDD)

Following `AGENTS.md` §4.4 **Option B (Red-Seam First / TDD)**, execution proceeds across 4 strictly sequential phases:

```
┌────────────────────────────────────────────────────────┐
│ Phase 1: Minimal Signatures Stub                       │
│ (backend-engineer adds public types/stubs — 5 mins)    │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Phase 2: Black-Box Seam Tests (RED)                    │
│ (test-engineer writes 7 subtests — confirms RED)       │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Phase 3: Atomic Implementation (GREEN)                 │
│ (backend-engineer implements Checkpoints A & B)        │
│ [Forbidden from touching tests/**]                     │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Phase 4: Adversarial Review & Mutation Proof           │
│ (review-subagent runs Circuit Breaker + MUTANT tests)  │
└────────────────────────────────────────────────────────┘
```

---

## 4. Implementation Checkpoints & Blast Radius

### Checkpoint A: Contracts, Persistence & Tool Engine
*Scope:* Public trait contracts, database schema migration v17, and tool wave scheduler.
*Files:*
- `app/src-tauri/src/persistence/schema.rs`
- `app/src-tauri/src/persistence/mod.rs`
- `app/src-tauri/src/persistence/steps.rs` (new)
- `app/src-tauri/src/persistence/tool_calls.rs`
- `app/src-tauri/src/persistence/worker.rs`
- `app/src-tauri/src/services/harness/stages/tools/mod.rs`
- `app/src-tauri/src/services/harness/stages/tools/search_memory.rs`
- `app/src-tauri/src/services/harness/stages/tools/web_search.rs`
- `app/src-tauri/src/services/harness/stages/tools/set_title.rs`
- `app/src-tauri/src/services/harness/stages/tools/executor.rs`
*Build Expectation:* Compiles cleanly once Checkpoint A is complete (`cargo check --tests`).
*Definition of Done:* `session_steps` table exists, `session_tool_calls` has `step_id`, `ToolExecutionMode` is integrated, and `execute_batch` supports wave partitioning and early-exit fallback synthesis.

### Checkpoint B: History Invariants, Scratchpad Eradication & ReAct Step Loop
*Scope:* Scratchpad removal across history, budget, prompt, steps, and loop coordinator.
*Depends on:* Checkpoint A
*Files:*
- `app/src-tauri/src/services/harness/stages/history.rs`
- `app/src-tauri/src/services/harness/stages/budget.rs`
- `app/src-tauri/src/services/harness/stages/prompt.rs`
- `app/src-tauri/src/services/harness/steps.rs`
- `app/src-tauri/src/services/harness/loop.rs`
*Build Expectation:* Must be executed as an atomic sweep. Removing `scratchpad` from `steps.rs` breaks callers until `loop.rs`, `budget.rs`, and `prompt.rs` are all updated.
*Definition of Done:* Full workspace builds clean (`./scripts/verify-targets.sh`). Zero instances of `scratchpad: &[ChatMessage]` remain. Reentrant loop executes up to `MAX_TOOL_STEPS = 20` and forces a tool-free summary pass on breach.

### Checkpoint C: Seam Tests, Integration & Mutation Proof
*Scope:* Black-box seam testing across concurrency, order preservation, cancellation, step ceiling, and database cascade.
*Depends on:* Checkpoint A, Checkpoint B
*Files:*
- `app/src-tauri/tests/common/harness.rs`
- `app/src-tauri/tests/harness_loop_test.rs` (Subtests 1–5)
- `app/src-tauri/tests/database_persistence_boundary_test.rs` (Subtests 6–7)
*Build Expectation:* All unit, component, and seam tests pass in release mode:
`cargo nextest run --release --test-threads=1 --no-fail-fast`
*Definition of Done:* 7 new subtests pass green. Every `MUTANT:` one-liner turns its corresponding test red when seeded into production code.

---

## 5. Requirement Accounting & Traceability Matrix

| Requirement from Specs | Architectural Mechanism | Implementation Checkpoint | Verification Subtest |
| :--- | :--- | :--- | :--- |
| **Concurrency Wave Scheduling (`RULE-HARN-11A`)** | `ToolExecutor::execute_batch` + `join_all` | Checkpoint A (`executor.rs`) | Subtest 1 (`harness_loop_test.rs`) |
| **Exclusive Barriers (`RULE-HARN-11A`)** | Drains parallel waves before executing exclusive tools alone | Checkpoint A (`executor.rs`) | Subtest 2 (`harness_loop_test.rs`) |
| **Model Declaration Order (`RULE-HARN-11A`)** | `outcomes.sort_by_key` aligning to `calls` index | Checkpoint A (`executor.rs`) | Subtest 1 & 2 (`harness_loop_test.rs`) |
| **1:1 Bijection Guarantee (`RULE-HARN-11B`)** | Fallback outcome synthesis on cancellation/suspend + `assert_eq!` | Checkpoint A (`executor.rs`) & B (`history.rs`) | Subtest 3 (`harness_loop_test.rs`) |
| **Immediate Step Commits (`RULE-HARN-11B`)** | `ConversationHistoryStage::commit_tool_step` | Checkpoint B (`history.rs`, `steps.rs`) | Subtest 1 & 3 (`harness_loop_test.rs`) |
| **Eradication of Scratchpad (`RULE-HARN-11B`)** | Unified history slices in `prompt.rs` and `budget.rs` | Checkpoint B (`budget.rs`, `prompt.rs`, `loop.rs`) | Compiler gate + Subtest 1 |
| **Protected Barge-in Invariant (`RULE-HARN-12`)** | `rollback_last_user_turn` guard (`last.role == Role::User`) | Checkpoint B (`history.rs`, `steps.rs`) | Subtest 3 & 4 (`harness_loop_test.rs`) |
| **Step Ceiling Summary Pass (`RULE-HARN-11C`)** | Forced pass with `allow_tools = false` at step 20 | Checkpoint B (`loop.rs`) | Subtest 5 (`harness_loop_test.rs`) |
| **Relational `session_steps` (`RULE-HARN-11D`)** | Turso DB schema v17 + `persist_step` | Checkpoint A (`schema.rs`, `steps.rs`) | Subtest 6 (`database_persistence_boundary_test.rs`) |
| **FIFO FK Insertion Order (`RULE-HARN-11D`)** | `StepCompleted` dispatched before `ToolCallExecuted` | Checkpoint B (`steps.rs`) | Subtest 6 (`database_persistence_boundary_test.rs`) |
| **Cascade Hard Deletion (`RULE-DB-10`)** | `ON DELETE CASCADE` on `session_steps` & `session_tool_calls` | Checkpoint A (`schema.rs`) | Subtest 7 (`database_persistence_boundary_test.rs`) |
