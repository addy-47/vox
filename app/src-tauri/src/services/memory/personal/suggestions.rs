use thiserror::Error;
use turso::Connection;

use super::{
    document::{validate_document_structure, validate_no_heading_loss},
    patch::{apply_patch_operations, MemoryPatchOperation},
};
use crate::persistence::personal_memory::{
    fetch_pending_suggestions, get_personal_memory, resolve_batch_suggestions_transaction,
    PersonalMemoryRecord, PersonalMemorySuggestionRecord, SuggestionDecision,
};

/// Domain errors for personal-memory suggestion resolution.
///
/// Typed so the IPC layer can preserve its error taxonomy (InvalidArgument /
/// NotFound / Engine / Database) without re-deriving the failure by re-running
/// the resolution preconditions itself.
#[derive(Debug, Error)]
pub enum MemorySuggestionError {
    #[error("Invalid suggestion resolution action: {0}. Expected 'accept' or 'reject'")]
    InvalidAction(String),

    #[error("Pending suggestion '{0}' not found among pending suggestions")]
    NotPending(String),

    #[error("Failed to apply patch operations: {0}")]
    PatchEngine(String),

    #[error("Patched document failed the structure contract, nothing was committed: {0}")]
    StructureGate(String),

    #[error(transparent)]
    Database(#[from] anyhow::Error),
}

/// Resolves pending personal-memory suggestions in batch and returns the resulting record.
///
/// Accepts a list of suggestion decisions (`accept` or `reject`). Accepted suggestions have their
/// patch operations sorted and applied to the active document. All decisions are committed in a
/// single atomic database transaction via `resolve_batch_suggestions_transaction`.
pub async fn batch_resolve_memory_suggestions(
    conn: &Connection,
    project_id: Option<&str>,
    decisions: &[SuggestionDecision],
) -> std::result::Result<PersonalMemoryRecord, MemorySuggestionError> {
    if decisions.is_empty() {
        let active_memory = get_personal_memory(conn, project_id).await?;
        return Ok(active_memory);
    }

    for d in decisions {
        if d.action != "accept" && d.action != "reject" {
            return Err(MemorySuggestionError::InvalidAction(d.action.clone()));
        }
    }

    let active_memory = get_personal_memory(conn, project_id).await?;
    let pending = fetch_pending_suggestions(conn, project_id).await?;
    let pending_map: std::collections::HashMap<String, PersonalMemorySuggestionRecord> =
        pending.into_iter().map(|s| (s.id.clone(), s)).collect();

    for d in decisions {
        if !pending_map.contains_key(&d.id) {
            return Err(MemorySuggestionError::NotPending(d.id.clone()));
        }
    }

    let mut accepted_ops: Vec<MemoryPatchOperation> = Vec::new();
    // Parallel to `accepted_ops`: the suggestion row each operation came from, so a rejection
    // can be attributed to a row rather than only to a log line.
    let mut accepted_ids: Vec<String> = Vec::new();
    for d in decisions {
        if d.action == "accept" {
            if let Some(s) = pending_map.get(&d.id) {
                accepted_ops.push(MemoryPatchOperation {
                    op: s.op.clone(),
                    index: s.target_index,
                    text: s.content.clone(),
                });
                accepted_ids.push(s.id.clone());
            }
        }
    }

    let updated_record = if !accepted_ops.is_empty() {
        let report = apply_patch_operations(&active_memory.content, &accepted_ops)
            .map_err(|e| MemorySuggestionError::PatchEngine(e.to_string()))?;

        // Operations the engine refused are settled as `reject` rather than left pending. Leaving
        // them pending was the Sprint 2 stall: the row survived, the next cycle stacked more rows
        // on top, the batch grew, and the cycle became likelier to contain another unsafe
        // operation, so the pile grew without bound (1 -> 17 blocked operations) and the document
        // stopped changing entirely (`consolidation-structured--logic-plan.md` §4.5).
        let rejected_positions: std::collections::HashSet<usize> = report
            .rejected
            .iter()
            .map(|rejection| rejection.position)
            .collect();
        for rejection in &report.rejected {
            log::warn!(
                "[Memory::Personal::Patch] refused suggestion {} ({}): {}",
                accepted_ids
                    .get(rejection.position)
                    .map(String::as_str)
                    .unwrap_or("<unknown>"),
                rejection.operation.op,
                rejection.reason
            );
        }
        let effective_decisions: Vec<SuggestionDecision> = decisions
            .iter()
            .map(|d| {
                let position = accepted_ids.iter().position(|id| *id == d.id);
                let refused = position.is_some_and(|p| rejected_positions.contains(&p));
                SuggestionDecision {
                    id: d.id.clone(),
                    action: if refused { "reject" } else { d.action.as_str() }.to_string(),
                }
            })
            .collect();

        // INVARIANT (memory-spec.md §5.1 structure contract, §5.3 step 3): a patch set can
        // mint a nameless or duplicate heading, and the resulting document cannot self-repair
        // because every later pass copies the malformed headings it finds. Gate before commit so
        // a rejected candidate leaves the active version and all suggestion rows untouched.
        validate_document_structure(&report.document)
            .map_err(|e| MemorySuggestionError::StructureGate(e.to_string()))?;

        // Defence in depth. `apply_patch_operations` now refuses a heading-destroying `replace`
        // per operation, so this should not fire; it remains because a batch interaction could
        // still drop a section in a way no single operation does.
        validate_no_heading_loss(&active_memory.content, &report.document)
            .map_err(|e| MemorySuggestionError::StructureGate(e.to_string()))?;

        resolve_batch_suggestions_transaction(
            conn,
            project_id,
            &effective_decisions,
            Some(&report.document),
        )
        .await?
    } else {
        resolve_batch_suggestions_transaction(conn, project_id, decisions, None).await?
    };

    Ok(updated_record)
}

/// Resolves pending personal-memory suggestions and returns the resulting record.
///
/// This delegates to `batch_resolve_memory_suggestions` for either a single target ID
/// or all pending suggestions when `target_id` is None.
pub async fn resolve_memory_suggestions(
    conn: &Connection,
    project_id: Option<&str>,
    target_id: Option<&str>,
    action: &str,
) -> std::result::Result<PersonalMemoryRecord, MemorySuggestionError> {
    if action != "accept" && action != "reject" {
        return Err(MemorySuggestionError::InvalidAction(action.to_string()));
    }

    let pending = fetch_pending_suggestions(conn, project_id).await?;

    let decisions: Vec<SuggestionDecision> = if let Some(id) = target_id {
        if !pending.iter().any(|s| s.id == *id) {
            return Err(MemorySuggestionError::NotPending(id.to_string()));
        }
        vec![SuggestionDecision {
            id: id.to_string(),
            action: action.to_string(),
        }]
    } else {
        pending
            .into_iter()
            .map(|s| SuggestionDecision {
                id: s.id,
                action: action.to_string(),
            })
            .collect()
    };

    batch_resolve_memory_suggestions(conn, project_id, &decisions).await
}
