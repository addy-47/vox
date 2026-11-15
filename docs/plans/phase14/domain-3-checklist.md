# Domain 3 Implementation Checklist

### Phase 1: Minimal Signatures Stub (Backend Engineer)
*Depends on: None*
*Commit Gate: `git commit -m "feat(harness): minimal signatures stub for domain 3"`*
- [ ] `app/src-tauri/src/utils/paths.rs`
  - Add `tool_outputs_dir() -> PathBuf` resolving to `cache_dir().join("tool_outputs")`.
  - Add `session_tool_outputs_dir(session_id: i64) -> PathBuf` resolving to `tool_outputs_dir().join(session_id.to_string())`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/mod.rs`
  - Add `pub enum TruncationPolicy { None, HeadTail { head_ratio: f32 }, TailHeavy { tail_ratio: f32 } }`.
  - Add `fn truncation_policy(&self) -> TruncationPolicy` to `pub trait ToolDefinition` with default `HeadTail { head_ratio: 0.8 }`.
  - Add `truncated: bool` and `spillover_path: Option<String>` to `ToolResult` and `ToolExecutionOutcome`.
- [ ] `app/src-tauri/src/services/harness/stages/guardrails.rs`
  - Create stub module with `ToolCallFingerprint`, `TurnGuardrailsState`, `GuardrailAction`, and `GuardrailsStage`.
- [ ] `app/src-tauri/src/services/harness/stages/truncation.rs`
  - Create stub module with `TruncationStage`.
- [ ] `app/src-tauri/src/persistence/mod.rs`
  - Update `StepCompleted` variant of `enum PersistenceEvent` with `prompt_tokens: Option<u32>`, `completion_tokens: Option<u32>`, `guardrail_strike: u32`, and `is_compacted: bool`.
  - Update `ToolCallExecuted` variant with `truncated: bool` and `spillover_path: Option<String>`.

---

### Phase 2: Black-Box Seam Tests (RED) (Test Engineer)
*Depends on: Phase 1*
*Gate: Run `cargo nextest run --test harness_loop_test --test database_persistence_boundary_test --release` -> Confirm ALL 8 SUBTESTS FAIL (RED)*
*Commit Gate: `git commit -m "test(harness): add red seam tests for domain 3 guardrails & truncation"`*
- [x] `app/src-tauri/tests/harness_loop_test.rs`
  - Add Subtest 1: `identical_tool_call_loop_triggers_strike_one_warning_then_strike_two_circuit_breaker`.
  - Add Subtest 2: `ping_pong_alternating_tool_loop_triggers_two_strike_circuit_breaker`.
  - Add Subtest 3: `consecutive_error_steps_trigger_two_strike_guardrail_and_force_summary`.
  - Add Subtest 4: `oversized_tool_observation_spills_to_tier4_cache_and_commits_head_tail_preview`.
  - Add Subtest 5: `truncation_policy_none_preserves_full_output_without_disk_spillover`.
  - Add Subtest 6: `mid_loop_critical_budget_triggers_compaction_while_preserving_active_turn_steps`.
- [x] `app/src-tauri/tests/database_persistence_boundary_test.rs`
  - Add Subtest 7: `schema_v18_persists_step_tokens_strikes_compaction_and_tool_spillover_metadata`.
  - Add Subtest 8: `session_deletion_and_startup_sweep_purge_tier4_tool_output_cache_directories`.

---

### Phase 3: Atomic Production Implementation (GREEN) (Backend Engineer)
*Forbidden from modifying tests/**!*

#### Batch 1: Paths Gateway, Schema v18, Persistence Engine & Observation Sizing
*Depends on: Phase 2*
*Commit Gate: `git commit -m "feat(harness): batch 1 schema v18, persistence & observation truncation"`*
- [ ] `app/src-tauri/src/persistence/schema.rs`
  - Bump `SCHEMA_VERSION` constant from 17 to 18.
  - Add `prompt_tokens`, `completion_tokens`, `guardrail_strike`, and `is_compacted` to `session_steps` in `SCHEMA_TABLE_STATEMENTS`.
  - Add `truncated` and `spillover_path` to `session_tool_calls` in `SCHEMA_TABLE_STATEMENTS`.
  - Implement idempotent v18 migration block adding columns via `ALTER TABLE`.
- [ ] `app/src-tauri/src/persistence/steps.rs`
  - Update `persist_step` to insert `prompt_tokens`, `completion_tokens`, `guardrail_strike`, and `is_compacted` into `session_steps`.
- [ ] `app/src-tauri/src/persistence/tool_calls.rs`
  - Update `persist_tool_call` to insert `truncated` and `spillover_path` into `session_tool_calls`.
- [ ] `app/src-tauri/src/persistence/sessions.rs`
  - In `delete_session`, add purge of `paths::session_tool_outputs_dir(session_id)`.
- [ ] `app/src-tauri/src/persistence/worker.rs`
  - Forward v18 fields in `process_event`.
  - In `run_startup_sweeps`, sweep `tool_outputs/`: delete folders for nonexistent/deleted sessions, purge files with `mtime > 7 days`, cap at 100MB LRU.
- [ ] `app/src-tauri/src/services/harness/stages/tools/set_title.rs`
  - Implement `truncation_policy` returning `TruncationPolicy::None`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/search_memory.rs`
  - Implement `truncation_policy` returning `TruncationPolicy::HeadTail { head_ratio: 0.8 }`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/web_search.rs`
  - Implement `truncation_policy` returning `TruncationPolicy::HeadTail { head_ratio: 0.8 }`.
- [ ] `app/src-tauri/src/services/harness/stages/truncation.rs`
  - Implement `TruncationStage`:
    - `apply(session_id: i64, call_id: &str, tool_name: &str, raw_output: &str, max_tokens: usize, policy: TruncationPolicy) -> (String, bool, Option<String>)`.
    - Character-boundary-safe slicing with newline snapping within 200 bytes and UTF-8 boundary snapping.
    - Sanitize `call_id` to `[a-zA-Z0-9_-]` before path construction.
    - Write full output to `paths::session_tool_outputs_dir(session_id).join(format!("{}.txt", sanitized_call_id))`.
    - Structured logging: `[TruncationAudit] call_id={call_id} tool={tool_name} policy={policy} original_tokens={orig} preview_tokens={preview} spillover_path={path}`.
- [ ] `app/src-tauri/src/services/harness/stages/tools/executor.rs`
  - In `execute_tool` / `execute_batch`, run `TruncationStage::apply` on tool outputs. Populate `truncated` and `spillover_path`.

#### Batch 2: Guardrail Engine, Monotonic State & Two-Strike Circuit Breaker
*Depends on: Batch 1*
*Commit Gate: `git commit -m "feat(harness): batch 2 guardrails engine & two-strike circuit breaker"`*
- [ ] `app/src-tauri/src/services/harness/stages/guardrails.rs`
  - Implement `ToolCallFingerprint` with `step_index`, canonical JSON `args_hash`, and `result_hash`.
  - Implement monotonic `TurnGuardrailsState` where `strike_count: u8` never resets back to 0 within a turn.
  - Implement `GuardrailsStage::evaluate_batch`:
    - Identical call loop detection: matching `tool_name`, `args_hash`, and `result_hash` across steps.
    - Ping-pong loop detection: $A \to B \to A \to B$ alternation across steps.
    - Step-boundary error cascade detection: 3 consecutive failing steps (ignoring intra-batch parallel errors on Step 0).
  - Return `GuardrailAction::Proceed`, `GuardrailAction::Strike1Warning { warning_msg }`, or `GuardrailAction::Strike2CircuitBreak`.
  - Structured logging: `[GuardrailAudit] turn_id={turn_id} step_index={step_index} pattern={pattern} strike={strike} tool={tool_name}`.
- [ ] `app/src-tauri/src/services/harness/stages/mod.rs`
  - Export `guardrails` and `truncation`.
- [ ] `app/src-tauri/src/services/harness/chassis.rs`
  - Add `GuardrailsStage` to `Harness` chassis and reset turn-scoped guardrail state on `intake_recorded_turn`.
- [ ] `app/src-tauri/src/services/harness/steps.rs`
  - In `step6_execute_tool_batch`:
    - Append Strike 1 warning to tool result content when triggered.
    - Forward `guardrail_strike` count in `StepCompleted`.
    - Forward `truncated` and `spillover_path` in `ToolCallExecuted`.
- [ ] `app/src-tauri/src/services/harness/loop.rs`
  - Maintain `TurnGuardrailsState` across loop iterations.
  - Handle Strike 2: strip tools (`allow_tools = false`), log warning, and force final tool-free summary generation pass.

#### Batch 3: Turn-Preserving Mid-Loop Compaction & Step Provenance
*Depends on: Batch 2*
*Commit Gate: `git commit -m "feat(harness): batch 3 mid-loop compaction & step provenance"`*
- [ ] `app/src-tauri/src/services/harness/chassis.rs`
  - Update `apply_session_context` to preserve active turn trailing slice (`[Role::User, Role::Assistant(tool_calls), Role::Tool, ...]`) intact.
- [ ] `app/src-tauri/src/services/harness/stages/budget.rs`
  - Update degraded FIFO shift to drop atomic turn units without separating `Role::Assistant` from `Role::Tool`. Never drop active turn.
- [ ] `app/src-tauri/src/services/harness/loop.rs`
  - In `execute_loop_iterations`, evaluate token budget before each pass.
  - If utilization $\ge 85\%$, trigger `step3_execute_compaction` inline. Record `is_compacted = true`.
- [ ] `app/src-tauri/src/services/harness/steps.rs`
  - In `step6_execute_tool_batch`:
    - Compute `prompt_tokens` and `completion_tokens` using `estimate_tokens`.
    - Pass tokens and `is_compacted` in `PersistenceEvent::StepCompleted`.
    - Log structured `[StepAudit]` metrics on step completion.
- [ ] **Verification**: Run `cargo nextest run --test harness_loop_test --test database_persistence_boundary_test --release` -> Confirm ALL 8 SUBTESTS PASS (GREEN).

---

### Phase 4: Mutation Proof & Verification (Test Engineer)
*Agent: Test Engineer exclusively*
*Commit Gate: `git commit -m "test(harness): verify mutation test coverage for domain 3"`*
- [x] Test Engineer seeds each deliberate mutant from False-Green Table into production code:
  - Mutant 1: Bypass identical call check in `guardrails.rs` -> Confirm Subtest 1 fails RED. Revert.
  - Mutant 2: Disable ping-pong check in `guardrails.rs` -> Confirm Subtest 2 fails RED. Revert.
  - Mutant 3: Ignore `is_error` in `guardrails.rs` -> Confirm Subtest 3 fails RED. Revert.
  - Mutant 4: Bypass `TruncationStage` in `executor.rs` -> Confirm Subtest 4 fails RED. Revert.
  - Mutant 5: Apply `HeadTail` regardless of `TruncationPolicy::None` -> Confirm Subtest 5 fails RED. Revert.
  - Mutant 6: Wipe active turn in `apply_session_context` -> Confirm Subtest 6 fails RED. Revert.
  - Mutant 7: Hardcode `0` for v18 columns in `steps.rs` / `tool_calls.rs` -> Confirm Subtest 7 fails RED. Revert.
  - Mutant 8: Omit `remove_dir_all` in `delete_session` -> Confirm Subtest 8 fails RED. Revert.
- [x] Confirm full test suite returns to 100% green.

---

### Phase 5: Adversarial Code & Spec Review (Reviewer Agent)
*Agent: Independent Reviewer Agent (launched with ZERO prior context / fresh eyes)*
*Single Job: Audit entire diff against specs, invariants & style guides*
- [ ] Audit production diff against `backend-style-guide.md` (CSG-01, CSG-03, zero magic numbers).
- [ ] Audit production diff against `architectural-invariants.md` (Turso MVCC, sacred hot-path, ZBC).
- [ ] Audit tests against `testing-style-guide.md` and `/create-test` (black-box seam assertions, zero white-box mocking).
- [ ] Audit persistence against `db-spec.md` (Schema v18 columns, FIFO ordering parent-then-child).
- [ ] Audit cache paths against `storage-spec.md` (`RULE-STORAGE-01`, `RULE-STORAGE-13`, `session_tool_outputs_dir`).
- [ ] Audit harness loop against `harness-spec.md` (`RULE-HARN-13` through `RULE-HARN-17`).
- [ ] Issue formal Review Report with PASS or BLOCKED verdict.
