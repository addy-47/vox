# Domain 3 Implementation Plan: Loop Guardrails, Observation Truncation & Step Provenance

---

## 1. Target End-State

Domain 3 establishes runtime safety guardrails, observation boundary controls, and granular step-level audit provenance for Vox Harness v2:
1. **Two-Strike Monotonic Guardrail Engine**: In-memory sliding window ($\le 1\text{ KB}$) tracking call signatures `(step_index, tool, args_hash, result_hash, is_error)`. Detects identical calls ($\ge 3$), ping-pong oscillations ($A \to B \to A \to B$), and step-boundary error cascades ($\ge 3$). Strike 1 injects a system steering warning. Strike 2 strips tools (`allow_tools = false`) and forces a spoken summary pass (`AudioIntent::TurnResponse`). `strike_count: u8` is monotonic per turn and never resets on clean calls.
2. **Head/Tail Truncation & Tier 4 Disk Spillover**: Tools declare `TruncationPolicy` via `ToolDefinition`. Slices observations exceeding `max_observation_tokens`. Uses UTF-8 character-boundary safe indexing with newline snapping within 200 bytes. Sanitizes `call_id` and writes unabridged raw outputs to scoped disk cache `paths::session_tool_outputs_dir(session_id).join("<sanitized_call_id>.txt")`.
3. **Cache Lifecycle Management**: Wipes session cache on session deletion in `delete_session`. Startup sweeps remove deleted session directories, purge files older than 7 days, and enforce a 100MB LRU directory ceiling.
4. **Dynamic Mid-Loop Budget & Turn-Preserving Compaction**: Evaluates token utilization before every reentrant pass. Triggers `CompactionStage::run_and_persist` when utilization reaches $\ge 85\%$. In `Harness::apply_session_context`, preserves the active turn's trailing slice (`[Role::User, Role::Assistant(tool_calls), Role::Tool, ...]`) intact while compacting prior history. Degraded FIFO shift drops atomic turn units without separating `Role::Assistant` from its matching `Role::Tool` observations.
5. **Relational Step Provenance & Auditing**: Schema v18 adds `prompt_tokens`, `completion_tokens`, `guardrail_strike`, and `is_compacted` to `session_steps`, plus `truncated` and `spillover_path` to `session_tool_calls`. Emits structured `[StepAudit]`, `[TruncationAudit]`, and `[GuardrailAudit]` logs.

---

## 2. Resolved Architectural Decisions (Clarification Ledger)

1. **Active-Turn Step Preservation During Mid-Loop Compaction (Defect 1 Fix)**:
   - *Issue:* Calling `apply_session_context` mid-turn in v1 wiped `self.history` and re-pushed only the initial user query, deleting all intermediate tool steps of the active turn and causing the model to restart from Step 0.
   - *Resolution:* `apply_session_context` extracts the active turn's trailing slice (from the last `Role::User` message to the end of history), resets history with the compacted `<session_context>` prompt, and re-appends the in-flight slice. Intermediate tool steps are preserved. Degraded FIFO shift drops complete turn units and never splits `Role::Assistant` from `Role::Tool`.
2. **Guardrail Monotonicity & Step-Boundary Error Cascades (Defect 2 Fix)**:
   - *Issue:* Volatile strike counters reset to Strike 0 on clean calls, allowing models to loop indefinitely. 3 parallel tool failures within Step 0 falsely triggered an error cascade before the model saw the error.
   - *Resolution:* `strike_count: u8` is monotonic per turn (Strike 1 stays 1; next violation is Strike 2). Error cascades evaluate across step boundaries (3 consecutive failing steps). Identical call stall detection requires matching `result_hash` as well as `args_hash`. `args_hash` uses canonicalized JSON key ordering.
3. **UTF-8 Character-Boundary & Single-Line Safe Truncation (Defect 3 Fix)**:
   - *Issue:* Slicing by `.lines()` breaks or drops minified single-line payloads (JSON/base64). Raw byte slicing panics on multi-byte UTF-8 codepoints.
   - *Resolution:* `TruncationStage` budgets characters, snaps to nearby newlines within 200 bytes when present, and snaps to UTF-8 character boundaries (`floor_char_boundary` / `ceil_char_boundary`). Slices single-line 10,000-token payloads cleanly without panics.
4. **Centralized Paths Gateway & Path Traversal Guard (Defect 4 Fix)**:
   - *Issue:* Hardcoding `~/.vox/cache` violates `RULE-STORAGE-01` and breaks test path redirection (`TempPathsGuard`). Model-supplied `call_id` could contain directory traversal characters (`../../`).
   - *Resolution:* Add `paths::tool_outputs_dir()` and `paths::session_tool_outputs_dir(session_id)`. Sanitize `call_id` to `[a-zA-Z0-9_-]` before joining to paths.
5. **Exact Step Token Counting Formulas (Defect 5 Fix)**:
   - `prompt_tokens`: estimated via `estimate_tokens` across the step's input messages + active tool definitions.
   - `completion_tokens`: `estimate_tokens(preamble_text) + estimate_tokens(thinking_text) + calls.iter().map(|c| estimate_tokens(&c.name) + estimate_tokens(&c.arguments.to_string())).sum::<usize>()`.
6. **Persistence Lifecycle & Cache Sweeps (Defect 6 Fix)**:
   - `delete_session` in `persistence/sessions.rs` removes `paths::session_tool_outputs_dir(session_id)`.
   - `run_startup_sweeps` in `persistence/worker.rs` sweeps orphaned session directories, purges files with `mtime > 7 days`, and caps directory at 100MB LRU.

---

## 3. Subagent Orchestration Workflow & Incremental Commit Protocol

Implementation strictly follows **Option B (Red-Seam First / TDD)**.
Every batch and phase is implemented sequentially with its own atomic, incremental git commit so changes are clean and easily reviewable.

### Orchestration Roles & Boundaries
- **Backend Engineer**: Implements production types, algorithms, stages, schema migrations, and chassis integration. Strictly forbidden from modifying `tests/**`.
- **Test Engineer**: Authors the 8 black-box seam tests in Phase 2 (verifies RED). In Phase 4, **exclusively owns Mutation Testing**: deliberately seeds the mutants from the False-Green Table into production code, confirms tests turn RED, reverts mutants, and confirms green.
- **Adversarial Reviewer Agent**: Evaluates the full diff with **zero prior context** (launched as an independent subagent with fresh eyes). Runs the `review` skill, auditing code against `backend-style-guide.md`, `testing-style-guide.md`, `architectural-invariants.md`, and the authoritative specs (`harness-spec.md`, `db-spec.md`, `storage-spec.md`, `domain-3-guardrails.md`). Does NOT run mutation testing.

### Sequential Phase Loop & Incremental Commit Gates

```
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 1: Minimal Signatures Stub                                       │
│ Agent: Backend Engineer                                                │
│ Action: Add public types, enums, trait methods & stubs in production   │
│ Commit Gate: git commit -m "feat(harness): minimal signatures stub"    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 2: Black-Box Seam Tests (RED)                                    │
│ Agent: Test Engineer                                                   │
│ Action: Write Subtests 1–8 across harness_loop_test and persistence    │
│ Gate: Run cargo nextest -> EMPIRICALLY CONFIRM ALL 8 TESTS FAIL (RED)  │
│ Commit Gate: git commit -m "test(harness): add red seam tests for D3"  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 3: Atomic Production Implementation (GREEN)                      │
│ Agent: Backend Engineer [FORBIDDEN FROM TOUCHING tests/**]             │
│                                                                        │
│ Batch 1: Paths Gateway, Schema v18, Persistence & Observation Sizing   │
│ Commit Gate: git commit -m "feat(harness): batch 1 schema v18 & trunc" │
│                                                                        │
│ Batch 2: Guardrail Engine, Monotonic State & Two-Strike Circuit Break  │
│ Commit Gate: git commit -m "feat(harness): batch 2 guardrails & strikes"│
│                                                                        │
│ Batch 3: Turn-Preserving Compaction, Step Provenance & Coordination    │
│ Commit Gate: git commit -m "feat(harness): batch 3 mid-loop compaction"│
│                                                                        │
│ Gate: Run cargo nextest -> EMPIRICALLY CONFIRM ALL 8 TESTS PASS (GREEN)│
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 4: Mutation Proof & Verification                                 │
│ Agent: Test Engineer                                                   │
│ Action: Seed deliberate mutants from False-Green table one by one.     │
│ Gate: Confirm nextest fails RED for each mutant, then revert to GREEN. │
│ Commit Gate: git commit -m "test(harness): verify mutation coverage D3"│
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 5: Adversarial Review & Sign-Off                                 │
│ Agent: Independent Reviewer Agent (ZERO prior context / fresh eyes)    │
│ Action: Audit full git diff against specs, invariants & style guides   │
│ Output: Formal Review Report (PASS or BLOCKED)                         │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Implementation Checkpoints & Blast Radius

### Checkpoint A: Minimal Signatures Stub (Phase 1)
*Scope:* Public types and method signatures so test stubs compile.
*Files:*
- `app/src-tauri/src/utils/paths.rs` (`tool_outputs_dir`, `session_tool_outputs_dir`)
- `app/src-tauri/src/services/harness/stages/tools/mod.rs` (`TruncationPolicy`, `ToolDefinition::truncation_policy`)
- `app/src-tauri/src/services/harness/stages/guardrails.rs` (stubs for `GuardrailsStage`, `ToolCallFingerprint`, `GuardrailAction`)
- `app/src-tauri/src/services/harness/stages/truncation.rs` (stubs for `TruncationStage`)
- `app/src-tauri/src/persistence/mod.rs` (updated variants for `PersistenceEvent`)

---

### Checkpoint B: Batch 1 — Paths, Schema v18, Persistence & Observation Sizing (Phase 3)
*Scope:* Centralized paths, schema v18 migration, persistence worker sweeps, and UTF-8 safe observation truncation.
*Files:*
- `app/src-tauri/src/utils/paths.rs`
- `app/src-tauri/src/persistence/schema.rs`
- `app/src-tauri/src/persistence/mod.rs`
- `app/src-tauri/src/persistence/steps.rs`
- `app/src-tauri/src/persistence/tool_calls.rs`
- `app/src-tauri/src/persistence/sessions.rs`
- `app/src-tauri/src/persistence/worker.rs`
- `app/src-tauri/src/services/harness/stages/tools/mod.rs`
- `app/src-tauri/src/services/harness/stages/tools/set_title.rs`
- `app/src-tauri/src/services/harness/stages/tools/search_memory.rs`
- `app/src-tauri/src/services/harness/stages/tools/web_search.rs`
- `app/src-tauri/src/services/harness/stages/truncation.rs`
- `app/src-tauri/src/services/harness/stages/tools/executor.rs`
*Build Expectation:* Compiles cleanly (`cargo check --tests`).
*Definition of Done:* Schema version 18 applies cleanly. `TruncationPolicy` is implemented on built-in tools. Oversized tool outputs spill to `paths::session_tool_outputs_dir(session_id)`. `session_tool_calls` records `truncated` and `spillover_path`. Session deletion cleans up cache folder.

---

### Checkpoint C: Batch 2 — Guardrail Engine, Monotonic State & Two-Strike Circuit Breaker (Phase 3)
*Scope:* In-memory sliding window, loop pattern matcher with result hash, monotonic per-turn strike counter, system warning injection, and tool-stripping circuit breaker.
*Files:*
- `app/src-tauri/src/services/harness/stages/guardrails.rs`
- `app/src-tauri/src/services/harness/stages/mod.rs`
- `app/src-tauri/src/services/harness/chassis.rs`
- `app/src-tauri/src/services/harness/steps.rs`
- `app/src-tauri/src/services/harness/loop.rs`
*Build Expectation:* Compiles cleanly (`cargo check --tests`).
*Definition of Done:* Identical calls ($\ge 3$), ping-pongs ($\ge 4$), and step-boundary error cascades ($\ge 3$) trigger Strike 1 warning. Repeated offenses trigger Strike 2 tool stripping and forced summary generation. `[GuardrailAudit]` logs are emitted.

---

### Checkpoint D: Batch 3 — Turn-Preserving Mid-Loop Compaction & Step Provenance (Phase 3)
*Scope:* Mid-loop token evaluation before pass, turn-preserving `apply_session_context`, atomic turn unit FIFO fallback, and step token provenance tracking.
*Files:*
- `app/src-tauri/src/services/harness/stages/budget.rs`
- `app/src-tauri/src/services/harness/chassis.rs`
- `app/src-tauri/src/services/harness/loop.rs`
- `app/src-tauri/src/services/harness/steps.rs`
- `app/src-tauri/src/core/metrics.rs`
*Build Expectation:* Full workspace compiles clean (`./scripts/verify-targets.sh`).
*Definition of Done:* Mid-loop context utilization $\ge 85\%$ triggers `CompactionStage::run_and_persist`. Active turn steps are preserved intact. `session_steps` records `prompt_tokens`, `completion_tokens`, `guardrail_strike`, and `is_compacted`. `[StepAudit]` and `[TruncationAudit]` logs are emitted.

---

## 5. Phase 2b False-Green Mutation Table (Owned by Test Engineer)

| Subtest Name | Behavior Asserted | Seam Binary | Deliberate Mutant (Injected by Test Engineer) | Expected Mutation Result |
| :--- | :--- | :--- | :--- | :--- |
| `identical_tool_call_loop_triggers_strike_one_warning_then_strike_two_circuit_breaker` | 3 identical tool calls trigger Strike 1 warning; 4th call strips tools | `harness_loop_test` | In `GuardrailsStage`, return `GuardrailAction::Proceed` unconditionally | Test fails: loop runs to step 20 instead of breaking at step 4 |
| `ping_pong_alternating_tool_loop_triggers_two_strike_circuit_breaker` | $A \to B \to A \to B$ triggers Strike 1 warning then Strike 2 break | `harness_loop_test` | In `GuardrailsStage`, disable ping-pong pattern matcher | Test fails: loop continues past 4th alternating call |
| `consecutive_error_steps_trigger_two_strike_guardrail_and_force_summary` | 3 consecutive failing steps trigger Strike 1 warning; next failing step forces summary | `harness_loop_test` | In `GuardrailsStage`, ignore `is_error` when evaluating error cascade strikes | Test fails: loop continues past 3rd consecutive failing step |
| `oversized_tool_observation_spills_to_tier4_cache_and_commits_head_tail_preview` | 10,000-token output sliced to head/tail preview; raw saved to disk | `harness_loop_test` | In `ToolExecutor::execute_tool`, bypass `TruncationStage` and return raw `ToolResult` | Test fails: raw 10,000 tokens committed to history; cache file missing |
| `truncation_policy_none_preserves_full_output_without_disk_spillover` | Tools with policy `None` bypass truncation | `harness_loop_test` | In `TruncationStage`, apply `HeadTail` truncation regardless of policy | Test fails: output contains truncation marker |
| `mid_loop_critical_budget_triggers_compaction_while_preserving_active_turn_steps` | Context $\ge 85\%$ triggers inline compaction while preserving in-flight tool steps | `harness_loop_test` | In `apply_session_context` (`chassis.rs`), wipe the active turn's committed tool steps | Test fails: Step 1 request misses Step 0 tool observation |
| `schema_v18_persists_step_tokens_strikes_compaction_and_tool_spillover_metadata` | `session_steps` and `session_tool_calls` record tokens, strikes, paths | `database_persistence_boundary_test` | In `persistence/steps.rs` or `tool_calls.rs`, hardcode `0` or `NULL` for v18 columns | Test fails: database query returns `NULL` for tokens or spillover path |
| `session_deletion_and_startup_sweep_purge_tier4_tool_output_cache_directories` | Deleting session removes `paths::session_tool_outputs_dir(session_id)` | `database_persistence_boundary_test` | In `delete_session` (`sessions.rs`), omit the `remove_dir_all` call | Test fails: cache directory remains on disk after delete |

---

## 6. Definition of Done (DoD)

1. **Compilation Gate**: `cd app/src-tauri && ./scripts/verify-targets.sh` succeeds with zero errors and zero warnings.
2. **Incremental Git History**: Every phase and batch is committed with a separate atomic git commit.
3. **Deterministic Seam Tests**: Full test suite passes under `cargo-nextest`:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
4. **Empirical Mutation Proof**: Test Engineer validates all 8 deliberate mutants fail RED before being reverted to GREEN.
5. **Zero Silent Drops & Zero State Erasure**: No conversation turns dropped without visible compaction events. Intermediate tool steps of the active turn are preserved during mid-loop compaction.
6. **Independent Review Approval**: Independent Reviewer subagent audits the entire diff with fresh eyes and issues a formal PASS verdict.
