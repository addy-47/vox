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
    /// Full JSON payload of the ResolvedOp.
    pub content: String,
    /// Original text of target block prior to update or deletion.
    pub old_text: Option<String>,
}

/// Domain errors for personal-memory revision resolution.
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

    report
        .memory
        .validate()
        .map_err(|e| MemoryRevisionError::StructureGate(e.to_string()))?;

    let json = report
        .memory
        .to_json()
        .map_err(|e| MemoryRevisionError::OperationEngine(e.to_string()))?;

    let has_accept = effective_decisions.iter().any(|d| d.action == "accept");
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
        .map(|revision| {
            let operation = serde_json::from_str::<ResolvedOp>(&revision.content).ok();
            let old_text = match &operation {
                Some(ResolvedOp::UpdateBlock { block_id, .. })
                | Some(ResolvedOp::DeleteBlock { block_id }) => model
                    .find_block(block_id)
                    .map(|(s, b)| model.sections[s].blocks[b].text.clone()),
                _ => None,
            };
            MemoryRevisionView {
                preview: render_revision_preview(&revision, &model),
                id: revision.id,
                op: revision.op,
                target_id: revision.target_id,
                status: revision.status,
                created_at: revision.created_at,
                content: revision.content,
                old_text,
            }
        })
        .collect())
}

/// Renders a one-line human-readable description of a revision's operation.
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn sample_pending_record(id: &str, content: &str) -> PersonalMemoryRevisionRecord {
        PersonalMemoryRevisionRecord {
            id: id.to_string(),
            base_memory_version: 1,
            project_id: None,
            op: "create_block".to_string(),
            target_id: "sec_1".to_string(),
            content: content.to_string(),
            status: "pending".to_string(),
            created_at: 1000,
            resolved_at: None,
        }
    }

    #[test]
    fn test_validate_decisions_valid() {
        let mut pending_map = HashMap::new();
        pending_map.insert("rev_1".to_string(), sample_pending_record("rev_1", "{}"));
        pending_map.insert("rev_2".to_string(), sample_pending_record("rev_2", "{}"));

        let decisions = vec![
            RevisionDecision {
                id: "rev_1".to_string(),
                action: "accept".to_string(),
            },
            RevisionDecision {
                id: "rev_2".to_string(),
                action: "reject".to_string(),
            },
        ];

        assert!(validate_decisions(&decisions, &pending_map).is_ok());
    }

    #[test]
    fn test_validate_decisions_rejects_invalid_action() {
        let mut pending_map = HashMap::new();
        pending_map.insert("rev_1".to_string(), sample_pending_record("rev_1", "{}"));

        let decisions = vec![RevisionDecision {
            id: "rev_1".to_string(),
            action: "delete".to_string(),
        }];

        let err = validate_decisions(&decisions, &pending_map).unwrap_err();
        match err {
            MemoryRevisionError::InvalidAction(action) => assert_eq!(action, "delete"),
            other => panic!("Expected InvalidAction, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_decisions_rejects_non_pending() {
        let pending_map = HashMap::new();
        let decisions = vec![RevisionDecision {
            id: "rev_missing".to_string(),
            action: "accept".to_string(),
        }];

        let err = validate_decisions(&decisions, &pending_map).unwrap_err();
        match err {
            MemoryRevisionError::NotPending(id) => assert_eq!(id, "rev_missing"),
            other => panic!("Expected NotPending, got {:?}", other),
        }
    }

    #[test]
    fn test_deserialize_accepted_ops_handles_malformed() {
        let mut pending_map = HashMap::new();
        let valid_op = ResolvedOp::CreateBlock {
            section_id: "sec_1".to_string(),
            text: "Hello".to_string(),
        };
        let valid_json = serde_json::to_string(&valid_op).unwrap();

        pending_map.insert(
            "rev_valid".to_string(),
            sample_pending_record("rev_valid", &valid_json),
        );
        pending_map.insert(
            "rev_corrupt".to_string(),
            sample_pending_record("rev_corrupt", "not json"),
        );

        let decisions = vec![
            RevisionDecision {
                id: "rev_valid".to_string(),
                action: "accept".to_string(),
            },
            RevisionDecision {
                id: "rev_corrupt".to_string(),
                action: "accept".to_string(),
            },
        ];

        let (ops, ids, malformed) = deserialize_accepted_ops(&decisions, &pending_map);
        assert_eq!(ops.len(), 1);
        assert_eq!(ids, vec!["rev_valid"]);
        assert!(malformed.contains("rev_corrupt"));
    }

    #[test]
    fn test_render_revision_preview() {
        use super::super::model::{MemoryBlock, MemorySection};

        let model = PersonalMemory {
            sections: vec![MemorySection {
                id: "sec_12345_abcd".to_string(),
                title: "Bio".to_string(),
                blocks: vec![MemoryBlock {
                    id: "blk_12345_efgh".to_string(),
                    text: "Existing block text.".to_string(),
                }],
            }],
        };

        // CreateSection preview
        let op1 = ResolvedOp::CreateSection {
            title: "Interests".to_string(),
            blocks: vec!["Cycling".to_string()],
        };
        let rev1 = PersonalMemoryRevisionRecord {
            id: "rev_1".to_string(),
            base_memory_version: 1,
            project_id: None,
            op: "create_section".to_string(),
            target_id: "".to_string(),
            content: serde_json::to_string(&op1).unwrap(),
            status: "pending".to_string(),
            created_at: 1000,
            resolved_at: None,
        };
        let p1 = render_revision_preview(&rev1, &model);
        assert_eq!(p1, "Create section 'Interests' with 1 block(s)");

        // CreateBlock preview
        let op2 = ResolvedOp::CreateBlock {
            section_id: "sec_12345_abcd".to_string(),
            text: "User is fluent in Rust.".to_string(),
        };
        let rev2 = PersonalMemoryRevisionRecord {
            id: "rev_2".to_string(),
            base_memory_version: 1,
            project_id: None,
            op: "create_block".to_string(),
            target_id: "sec_12345_abcd".to_string(),
            content: serde_json::to_string(&op2).unwrap(),
            status: "pending".to_string(),
            created_at: 1000,
            resolved_at: None,
        };
        let p2 = render_revision_preview(&rev2, &model);
        assert_eq!(p2, "Create block in 'Bio': User is fluent in Rust.");

        // DeleteBlock preview
        let op3 = ResolvedOp::DeleteBlock {
            block_id: "blk_12345_efgh".to_string(),
        };
        let rev3 = PersonalMemoryRevisionRecord {
            id: "rev_3".to_string(),
            base_memory_version: 1,
            project_id: None,
            op: "delete_block".to_string(),
            target_id: "blk_12345_efgh".to_string(),
            content: serde_json::to_string(&op3).unwrap(),
            status: "pending".to_string(),
            created_at: 1000,
            resolved_at: None,
        };
        let p3 = render_revision_preview(&rev3, &model);
        assert_eq!(p3, "Delete block: Existing block text.");
    }
}
