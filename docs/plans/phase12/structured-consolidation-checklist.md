# Checklist: Backend Structured Delta Consolidation

This checklist tracks backend tasks for the Structured Delta Memory Consolidation and Suggestion Review Protocol.

## Batch 1: Database Migration & Persistence Layer (Turso)
- [ ] `app/src-tauri/src/persistence/schema.rs`
  - Add `personal_memory_suggestions` table DDL and index to `V2_TABLE_STATEMENTS`.
  - Add migration check in `run_migrations` for existing databases.
- [ ] `app/src-tauri/src/persistence/personal_memory.rs`
  - Define `PersonalMemorySuggestionRow` struct (`id`, `base_memory_version`, `project_id`, `op`, `section`, `target_text`, `proposed_text`, `source_fact_ids`, `status`, `created_at`, `resolved_at`).
  - Add `insert_personal_memory_suggestions(conn, &suggestions)` query.
  - Add `fetch_pending_suggestions(conn, base_version, project_id)` query.
  - Add `resolve_suggestions_transaction(conn, base_version, target_id, action, new_content)` transaction helper.
- [ ] `app/src-tauri/src/persistence/facts.rs`
  - Add `mark_facts_staged(conn, fact_ids)` updating status from `'active'` to `'staged'`.
  - Add `mark_facts_rejected(conn, fact_ids)` updating status from `'staged'` to `'rejected'`.
- [ ] `app/src-tauri/tests/personal_memory_test.rs`
  - Add unit test verifying suggestion insertion, retrieval, and status transitions.

## Batch 2: Structured Delta Patch Engine & Prompts
- [ ] `app/src-tauri/src/services/memory/personal.rs`
  - Define `MemoryPatchOperation` and `PersonalConsolidationOutput` serde structs.
  - Implement pure helper `apply_patch_operations(content: &str, ops: &[MemoryPatchOperation]) -> Result<String>`.
  - Add unit tests for `apply_patch_operations` (covering `insert`, `replace`, `delete`, missing section creation, whitespace normalization).
  - Update `PERSONAL_CONSOLIDATION_SYSTEM_PROMPT` to enforce JSON patch operations output.
  - Update `COMMENT_REGENERATION_SYSTEM_PROMPT` to enforce JSON patch operations output.
  - Refactor `consolidate_personal_memory` to parse JSON operations, write to `personal_memory_suggestions`, and mark facts `'staged'`.
  - Refactor `regenerate_with_comments` to invoke LLM with comments and stage operations.
- [ ] `app/src-tauri/src/services/memory/mod.rs`
  - Re-export suggestion types and patch engine.

## Batch 3: IPC Layer & Command Exposure
- [ ] `app/src-tauri/src/ipc/memory.rs`
  - Implement `get_memory_suggestions(project_id: Option<String>)` IPC command.
  - Implement `resolve_memory_suggestion(id: Option<String>, action: String, project_id: Option<String>)` IPC command.
- [ ] `app/src-tauri/src/lib.rs`
  - Register `get_memory_suggestions` and `resolve_memory_suggestion` in `tauri::generate_handler!`.

## Batch 4: Eval & Integration Verification
- [ ] `app/src-tauri/evals/memory_consolidation_eval.rs`
  - Update test harness to expect staged suggestions and execute resolution.
  - Assert zero anchor loss on pre-existing text.
- [ ] Run full nextest suite:
  - `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast`
