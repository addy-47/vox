use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::model::{
    generate_block_id, generate_section_id, HandleMap, MemoryBlock, MemorySection, PersonalMemory,
};

/// The flat grouped JSON payload emitted by every consolidation LLM pass.
///
/// Homogeneous arrays grouped by verb, rather than a discriminated union, because flat arrays are
/// more robust under grammar-constrained decoding (`semantic-structured-personal-memory-architecture.md` §6).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsolidationOutput {
    #[serde(default)]
    pub new_sections: Vec<NewSectionOutput>,
    #[serde(default)]
    pub creates: Vec<CreateBlockOutput>,
    #[serde(default)]
    pub updates: Vec<UpdateBlockOutput>,
    #[serde(default)]
    pub deletes: Vec<DeleteBlockOutput>,
}

/// A whole new section proposed by the LLM, carrying its own initial blocks so that creating a
/// section and its content is one atomic operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSectionOutput {
    pub title: String,
    #[serde(default)]
    pub blocks: Vec<String>,
}

/// A block to append to an existing section, addressed by that section's per-request handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBlockOutput {
    pub section: String,
    pub text: String,
}

/// A block whose text is to be replaced, addressed by that block's per-request handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBlockOutput {
    pub block: String,
    pub text: String,
}

/// A block to remove, addressed by that block's per-request handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteBlockOutput {
    pub block: String,
}

/// A fully resolved semantic operation, addressing persistent IDs rather than LLM handles.
///
/// This is what gets persisted in `personal_memory_revisions.content` and replayed at acceptance
/// time. Because every target is a persistent semantic ID, each operation is independently
/// resolvable: accepting one never shifts the target of another, which eliminates the re-anchoring
/// cascade that stalled the previous positional design.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ResolvedOp {
    CreateSection { title: String, blocks: Vec<String> },
    CreateBlock { section_id: String, text: String },
    UpdateBlock { block_id: String, text: String },
    DeleteBlock { block_id: String },
}

impl ResolvedOp {
    /// The operation verb as stored in `personal_memory_revisions.op`.
    pub fn op_name(&self) -> &'static str {
        match self {
            Self::CreateSection { .. } => "create_section",
            Self::CreateBlock { .. } => "create_block",
            Self::UpdateBlock { .. } => "update_block",
            Self::DeleteBlock { .. } => "delete_block",
        }
    }

    /// The persistent `sec_*` or `blk_*` ID this operation targets, or an empty string when the
    /// operation mints a new entity (`create_section`).
    pub fn target_id(&self) -> &str {
        match self {
            Self::CreateSection { .. } => "",
            Self::CreateBlock { section_id, .. } => section_id,
            Self::UpdateBlock { block_id, .. } => block_id,
            Self::DeleteBlock { block_id } => block_id,
        }
    }

    /// True when the operation only adds or rewrites content and therefore cannot lose user data.
    /// Used by the `auto_apply` revision policy, which commits these directly and holds deletions.
    pub fn is_non_destructive(&self) -> bool {
        !matches!(self, Self::DeleteBlock { .. })
    }
}

/// An operation the engine refused to apply, with the reason.
///
/// Refusing an operation must never be silent and must never escalate to the whole batch: while
/// rejections were log-only in an earlier design, an unsafe operation could not be attributed to a
/// revision row, so the refusal had to be escalated, and escalating stalled consolidation permanently
/// (`consolidation-structured--logic-plan.md` §4.5). Per-operation rejection with attribution is the
/// fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedOperation {
    /// Index into the operations slice the caller passed in, so a rejection maps back to the
    /// revision row it came from.
    pub position: usize,
    pub operation: ResolvedOp,
    pub reason: String,
}

/// Outcome of applying a batch of semantic operations: the resulting model plus everything refused.
#[derive(Debug, Clone)]
pub struct ApplyReport {
    pub memory: PersonalMemory,
    pub rejected: Vec<RejectedOperation>,
}

/// Maps LLM output handles onto persistent IDs, rejecting any unresolvable handle per operation.
///
/// `new_sections` need no resolution because they mint new entities. Rejection is per operation and
/// the rest of the batch proceeds, so one hallucinated handle cannot discard an otherwise valid pass.
pub fn resolve_operations(
    output: &ConsolidationOutput,
    handle_map: &HandleMap,
) -> (Vec<ResolvedOp>, Vec<RejectedOperation>) {
    let mut resolved = Vec::new();
    let mut rejected = Vec::new();

    for section in &output.new_sections {
        if section.title.trim().is_empty() {
            rejected.push(RejectedOperation {
                position: resolved.len() + rejected.len(),
                operation: ResolvedOp::CreateSection {
                    title: section.title.clone(),
                    blocks: section.blocks.clone(),
                },
                reason: "new section carried an empty title".to_string(),
            });
            continue;
        }
        resolved.push(ResolvedOp::CreateSection {
            title: section.title.clone(),
            blocks: section.blocks.clone(),
        });
    }

    for create in &output.creates {
        match handle_map.resolve_section(&create.section) {
            Some(section_id) if !create.text.trim().is_empty() => {
                resolved.push(ResolvedOp::CreateBlock {
                    section_id: section_id.to_string(),
                    text: create.text.clone(),
                });
            }
            Some(_) => rejected.push(RejectedOperation {
                position: resolved.len() + rejected.len(),
                operation: ResolvedOp::CreateBlock {
                    section_id: String::new(),
                    text: create.text.clone(),
                },
                reason: "create_block carried no text".to_string(),
            }),
            None => rejected.push(RejectedOperation {
                position: resolved.len() + rejected.len(),
                operation: ResolvedOp::CreateBlock {
                    section_id: String::new(),
                    text: create.text.clone(),
                },
                reason: format!("unknown section handle '{}'", create.section),
            }),
        }
    }

    for update in &output.updates {
        match handle_map.resolve_block(&update.block) {
            Some(block_id) if !update.text.trim().is_empty() => {
                resolved.push(ResolvedOp::UpdateBlock {
                    block_id: block_id.to_string(),
                    text: update.text.clone(),
                });
            }
            Some(_) => rejected.push(RejectedOperation {
                position: resolved.len() + rejected.len(),
                operation: ResolvedOp::UpdateBlock {
                    block_id: String::new(),
                    text: update.text.clone(),
                },
                reason: "update_block carried no text".to_string(),
            }),
            None => rejected.push(RejectedOperation {
                position: resolved.len() + rejected.len(),
                operation: ResolvedOp::UpdateBlock {
                    block_id: String::new(),
                    text: update.text.clone(),
                },
                reason: format!("unknown block handle '{}'", update.block),
            }),
        }
    }

    for delete in &output.deletes {
        match handle_map.resolve_block(&delete.block) {
            Some(block_id) => resolved.push(ResolvedOp::DeleteBlock {
                block_id: block_id.to_string(),
            }),
            None => rejected.push(RejectedOperation {
                position: resolved.len() + rejected.len(),
                operation: ResolvedOp::DeleteBlock {
                    block_id: String::new(),
                },
                reason: format!("unknown block handle '{}'", delete.block),
            }),
        }
    }

    (resolved, rejected)
}

/// Applies resolved semantic operations to a memory model, in order.
///
/// Each operation is applied against the live model, so an operation targeting an ID that a previous
/// operation in the same batch removed is rejected rather than silently applied to the wrong entity.
/// This is the structural guarantee that makes bulk resolution safe without any re-anchoring.
/// Empty sections are pruned once, after the whole batch.
pub fn apply_operations(memory: &PersonalMemory, operations: &[ResolvedOp]) -> Result<ApplyReport> {
    let mut current = memory.clone();
    let mut rejected = Vec::new();

    for (position, op) in operations.iter().enumerate() {
        if let Err(reason) = apply_one(&mut current, op) {
            rejected.push(RejectedOperation {
                position,
                operation: op.clone(),
                reason,
            });
        }
    }

    current.prune_empty_sections();
    Ok(ApplyReport {
        memory: current,
        rejected,
    })
}

/// Applies one operation to `memory`, returning `Err(reason)` when the operation is not applicable.
fn apply_one(memory: &mut PersonalMemory, op: &ResolvedOp) -> std::result::Result<(), String> {
    match op {
        ResolvedOp::CreateSection { title, blocks } => {
            let title = title.trim();
            if title.is_empty() {
                return Err("create_section carried an empty title".to_string());
            }
            if section_title_exists(memory, title) {
                return Err(format!("a section titled '{}' already exists", title));
            }
            memory.sections.push(MemorySection {
                id: generate_section_id(),
                title: title.to_string(),
                blocks: blocks
                    .iter()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| MemoryBlock {
                        id: generate_block_id(),
                        text: text.trim().to_string(),
                    })
                    .collect(),
            });
            Ok(())
        }
        ResolvedOp::CreateBlock { section_id, text } => {
            if text.trim().is_empty() {
                return Err("create_block carried no text".to_string());
            }
            let index = memory
                .find_section(section_id)
                .ok_or_else(|| format!("section '{section_id}' no longer exists"))?;
            memory.sections[index].blocks.push(MemoryBlock {
                id: generate_block_id(),
                text: text.trim().to_string(),
            });
            Ok(())
        }
        ResolvedOp::UpdateBlock { block_id, text } => {
            if text.trim().is_empty() {
                return Err("update_block carried no text".to_string());
            }
            let (s_i, b_i) = memory
                .find_block(block_id)
                .ok_or_else(|| format!("block '{block_id}' no longer exists"))?;
            memory.sections[s_i].blocks[b_i].text = text.trim().to_string();
            Ok(())
        }
        ResolvedOp::DeleteBlock { block_id } => {
            let (s_i, b_i) = memory
                .find_block(block_id)
                .ok_or_else(|| format!("block '{block_id}' no longer exists"))?;
            memory.sections[s_i].blocks.remove(b_i);
            Ok(())
        }
    }
}

/// Returns true when any section already carries `title`, compared case-insensitively.
fn section_title_exists(memory: &PersonalMemory, title: &str) -> bool {
    let needle = title.to_lowercase();
    memory
        .sections
        .iter()
        .any(|section| section.title.trim().to_lowercase() == needle)
}

/// Strips Markdown code fences and surrounding whitespace from an LLM JSON payload.
pub(super) fn extract_json_payload(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        if let Some(end) = stripped.rfind("```") {
            return stripped[..end].trim();
        }
    } else if let Some(stripped) = trimmed.strip_prefix("```") {
        if let Some(end) = stripped.rfind("```") {
            return stripped[..end].trim();
        }
    }
    trimmed
}
