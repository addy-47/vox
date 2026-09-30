use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use turso::Connection;

use super::{
    model::PersonalMemory,
    operations::{apply_operations, ResolvedOp},
};
use crate::persistence::personal_memory::{
    fetch_pending_revisions, get_personal_memory, insert_personal_memory_revisions,
    resolve_batch_revisions_transaction, PersonalMemoryRecord, PersonalMemoryRevisionRecord,
    RevisionDecision,
};

/// A pending memory revision projected for display, with the operation payload rendered as a
/// human-readable preview line (`ipc-spec.md §2.3`, `get_memory_revisions`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryRevisionView {
    pub id: String,
    /// `create_section` | `create_block` | `update_block` | `delete_block`.
    pub op: String,
    /// The persistent `sec_*` or `blk_*` ID this operation targets, empty for `create_section`.
    pub target_id: String,
    pub status: String,
    pub created_at: i64,
    /// Human-readable one-line description, resolved against the active semantic model.
    pub preview: String,
}

/// Domain errors for personal-memory revision resolution.
///
/// Typed so the IPC layer can preserve its error taxonomy (InvalidArgument / NotFound / Engine /
/// Database) without re-deriving the failure by re-running the resolution preconditions itself.
#[derive(Debug, Error)]
pub enum MemoryRevisionError {
    #[error("Invalid revision resolution action: {0}. Expected 'accept' or 'reject'")]
    InvalidAction(String),

    #[error("Pending revision '{0}' not found among pending revisions")]
    NotPending(String),

    #[error("Failed to apply semantic operations: {0}")]
    OperationEngine(String),

    #[error("Resolved memory failed the structure contract, nothing was committed: {0}")]
    StructureGate(String),

    #[error(transparent)]
    Database(#[from] anyhow::Error),
}

/// Stages resolved semantic operations as pending revision rows and returns how many were staged.
///
/// The `content` column holds the `ResolvedOp` JSON envelope, so acceptance replays exactly the
/// operation that was resolved at consolidation time rather than re-deriving it from a handle.
pub async fn stage_revisions(
    conn: &Connection,
    current_record: &PersonalMemoryRecord,
    operations: Vec<ResolvedOp>,
) -> anyhow::Result<usize> {
    if operations.is_empty() {
        return Ok(0);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut revisions = Vec::with_capacity(operations.len());
    for operation in operations {
        let content = serde_json::to_string(&operation)
            .map_err(|e| anyhow::anyhow!("Failed to serialize semantic operation: {}", e))?;
        revisions.push(PersonalMemoryRevisionRecord {
            id: format!("rev_{}_{}", now, &uuid::Uuid::new_v4().to_string()[..4]),
            base_memory_version: current_record.version,
            project_id: current_record.project_id.clone(),
            op: operation.op_name().to_string(),
            target_id: operation.target_id().to_string(),
            content,
            status: "pending".to_string(),
            created_at: now,
            resolved_at: None,
        });
    }

    insert_personal_memory_revisions(conn, &revisions).await?;
    Ok(revisions.len())
}

/// Resolves pending personal-memory revisions in batch and returns the resulting record.
///
/// Accepted revisions are deserialized back into `ResolvedOp` values and applied to the active
/// semantic model. Because operations address persistent semantic IDs rather than positional
/// indices, there is no descending-index sort and no re-anchoring of the remaining pending rows.
/// All decisions commit in a single atomic transaction.
pub async fn batch_resolve_memory_revisions(
    conn: &Connection,
    project_id: Option<&str>,
    decisions: &[RevisionDecision],
) -> std::result::Result<PersonalMemoryRecord, MemoryRevisionError> {
    if decisions.is_empty() {
        let active_memory = get_personal_memory(conn, project_id).await?;
        return Ok(active_memory);
    }

    let active_memory = get_personal_memory(conn, project_id).await?;
    let pending = fetch_pending_revisions(conn, project_id).await?;
    let pending_map: std::collections::HashMap<String, PersonalMemoryRevisionRecord> =
        pending.into_iter().map(|r| (r.id.clone(), r)).collect();
    validate_decisions(decisions, &pending_map)?;

    let (accepted_ops, accepted_ids, malformed) = deserialize_accepted_ops(decisions, &pending_map);

    let memory = PersonalMemory::from_json(&active_memory.content)
        .map_err(|e| MemoryRevisionError::OperationEngine(e.to_string()))?;
    let report = apply_operations(&memory, &accepted_ops)
        .map_err(|e| MemoryRevisionError::OperationEngine(e.to_string()))?;

    let rejected_positions: std::collections::HashSet<usize> = report
        .rejected
        .iter()
        .map(|rejection| rejection.position)
        .collect();
    for rejection in &report.rejected {
        log::warn!(
            "[Memory::Personal::Revisions] refused revision {} ({}): {}",
            accepted_ids
                .get(rejection.position)
                .map(String::as_str)
                .unwrap_or("<unknown>"),
            rejection.operation.op_name(),
            rejection.reason
        );
    }

    let effective_decisions: Vec<RevisionDecision> = decisions
        .iter()
        .map(|d| {
            let refused = accepted_ids
                .iter()
                .position(|id| *id == d.id)
                .is_some_and(|p| rejected_positions.contains(&p))
                || malformed.contains(&d.id);
            RevisionDecision {
                id: d.id.clone(),
                action: if refused { "reject" } else { d.action.as_str() }.to_string(),
            }
        })
        .collect();

    // INVARIANT: gate the structure contract before commit, so a violating batch writes nothing.
    report
        .memory
        .validate()
        .map_err(|e| MemoryRevisionError::StructureGate(e.to_string()))?;

    let json = report
        .memory
        .to_json()
        .map_err(|e| MemoryRevisionError::OperationEngine(e.to_string()))?;

    let has_accept = effective_decisions
        .iter()
        .any(|d| d.action == "accept");
    let updated_record = resolve_batch_revisions_transaction(
        conn,
        project_id,
        &effective_decisions,
        has_accept.then_some(json.as_str()),
    )
    .await?;

    Ok(updated_record)
}

/// Rejects unknown actions and decisions targeting non-pending revisions.
fn validate_decisions(
    decisions: &[RevisionDecision],
    pending_map: &std::collections::HashMap<String, PersonalMemoryRevisionRecord>,
) -> std::result::Result<(), MemoryRevisionError> {
    for d in decisions {
        if d.action != "accept" && d.action != "reject" {
            return Err(MemoryRevisionError::InvalidAction(d.action.clone()));
        }
        if !pending_map.contains_key(&d.id) {
            return Err(MemoryRevisionError::NotPending(d.id.clone()));
        }
    }
    Ok(())
}

/// Deserializes each accepted revision's payload, returning the operations with their row IDs plus
/// the IDs of rows whose payloads no longer parse.
fn deserialize_accepted_ops(
    decisions: &[RevisionDecision],
    pending_map: &std::collections::HashMap<String, PersonalMemoryRevisionRecord>,
) -> (
    Vec<ResolvedOp>,
    Vec<String>,
    std::collections::HashSet<String>,
) {
    let mut accepted_ops = Vec::new();
    let mut accepted_ids = Vec::new();
    let mut malformed = std::collections::HashSet::new();
    for d in decisions {
        if d.action != "accept" {
            continue;
        }
        let Some(revision) = pending_map.get(&d.id) else {
            continue;
        };
        match serde_json::from_str::<ResolvedOp>(&revision.content) {
            Ok(operation) => {
                accepted_ops.push(operation);
                accepted_ids.push(revision.id.clone());
            }
            Err(e) => {
                log::warn!(
                    "[Memory::Personal::Revisions] revision {} carries an unparseable payload: {}",
                    revision.id,
                    e
                );
                malformed.insert(revision.id.clone());
            }
        }
    }
    (accepted_ops, accepted_ids, malformed)
}

/// Lists pending revisions projected for display, resolving each operation's payload into a
/// human-readable preview line against the active semantic model.
pub async fn list_memory_revision_views(
    conn: &Connection,
    project_id: Option<&str>,
) -> anyhow::Result<Vec<MemoryRevisionView>> {
    let active_memory = get_personal_memory(conn, project_id).await?;
    let model = PersonalMemory::from_json(&active_memory.content)?;
    let pending = fetch_pending_revisions(conn, project_id).await?;

    Ok(pending
        .into_iter()
        .map(|revision| MemoryRevisionView {
            preview: render_revision_preview(&revision, &model),
            id: revision.id,
            op: revision.op,
            target_id: revision.target_id,
            status: revision.status,
            created_at: revision.created_at,
        })
        .collect())
}

/// Renders a one-line human-readable description of a revision's operation.
///
/// Falls back to the raw target ID when the operation is unresolvable, so an operation whose target
/// has since been removed still renders as something meaningful rather than an empty card.
fn render_revision_preview(
    revision: &PersonalMemoryRevisionRecord,
    model: &PersonalMemory,
) -> String {
    let operation = serde_json::from_str::<ResolvedOp>(&revision.content).ok();

    let Some(operation) = operation else {
        return format!("{} on {}", revision.op, short_id(&revision.target_id));
    };

    match &operation {
        ResolvedOp::CreateSection { title, blocks } => format!(
            "Create section '{}' with {} block(s)",
            title.trim(),
            blocks.len()
        ),
        ResolvedOp::CreateBlock { section_id, text } => {
            let title = model
                .find_section(section_id)
                .map(|i| model.sections[i].title.trim().to_string())
                .unwrap_or_else(|| short_id(section_id));
            format!("Create block in '{}': {}", title, truncate(text))
        }
        ResolvedOp::UpdateBlock { block_id, text } => {
            format!("Update block {}: {}", short_id(block_id), truncate(text))
        }
        ResolvedOp::DeleteBlock { block_id } => {
            let existing = model
                .find_block(block_id)
                .map(|(s, b)| model.sections[s].blocks[b].text.trim().to_string());
            match existing {
                Some(text) => format!("Delete block: {}", truncate(&text)),
                None => format!("Delete block {}", short_id(block_id)),
            }
        }
    }
}

/// Shortens a persistent ID to its unique suffix for display, since the timestamp prefix is noise to
/// a human reader.
fn short_id(id: &str) -> String {
    if id.is_empty() {
        return "<new>".to_string();
    }
    match id.rsplit_once('_') {
        Some((_, suffix)) => suffix.to_string(),
        None => id.to_string(),
    }
}

/// Truncates preview text to a single readable line.
fn truncate(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= 120 {
        return one_line;
    }
    let clipped: String = one_line.chars().take(117).collect();
    format!("{}...", clipped)
}
