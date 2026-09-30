use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::model::{
    generate_block_id, generate_section_id, HandleMap, MemoryBlock, MemorySection, PersonalMemory,
};

/// The flat grouped JSON payload emitted by every consolidation LLM pass.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedOperation {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_model() -> PersonalMemory {
        PersonalMemory {
            sections: vec![
                MemorySection {
                    id: "sec_alpha".to_string(),
                    title: "Profile".to_string(),
                    blocks: vec![
                        MemoryBlock {
                            id: "blk_1".to_string(),
                            text: "Lives in Austin.".to_string(),
                        },
                        MemoryBlock {
                            id: "blk_2".to_string(),
                            text: "Works on AI.".to_string(),
                        },
                    ],
                },
                MemorySection {
                    id: "sec_beta".to_string(),
                    title: "Hobbies".to_string(),
                    blocks: vec![MemoryBlock {
                        id: "blk_3".to_string(),
                        text: "Plays guitar.".to_string(),
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_resolve_operations_all_variants() {
        let memory = sample_model();
        let (_view, handle_map) = memory.to_handle_format();

        let output = ConsolidationOutput {
            new_sections: vec![NewSectionOutput {
                title: "Projects".to_string(),
                blocks: vec!["Building Vox assistant.".to_string()],
            }],
            creates: vec![CreateBlockOutput {
                section: "s1".to_string(),
                text: "Also loves hiking.".to_string(),
            }],
            updates: vec![UpdateBlockOutput {
                block: "b1".to_string(),
                text: "Lives in Seattle now.".to_string(),
            }],
            deletes: vec![DeleteBlockOutput {
                block: "b3".to_string(),
            }],
        };

        let (resolved, rejected) = resolve_operations(&output, &handle_map);
        assert!(
            rejected.is_empty(),
            "Expected zero rejections, got: {:?}",
            rejected
        );
        assert_eq!(resolved.len(), 4);

        assert_eq!(
            resolved[0],
            ResolvedOp::CreateSection {
                title: "Projects".to_string(),
                blocks: vec!["Building Vox assistant.".to_string()],
            }
        );
        assert_eq!(
            resolved[1],
            ResolvedOp::CreateBlock {
                section_id: "sec_alpha".to_string(),
                text: "Also loves hiking.".to_string(),
            }
        );
        assert_eq!(
            resolved[2],
            ResolvedOp::UpdateBlock {
                block_id: "blk_1".to_string(),
                text: "Lives in Seattle now.".to_string(),
            }
        );
        assert_eq!(
            resolved[3],
            ResolvedOp::DeleteBlock {
                block_id: "blk_3".to_string(),
            }
        );
    }

    #[test]
    fn test_resolve_operations_unknown_handles_and_empty_text_isolated() {
        let memory = sample_model();
        let (_view, handle_map) = memory.to_handle_format();

        let output = ConsolidationOutput {
            new_sections: vec![NewSectionOutput {
                title: "   ".to_string(), // empty title -> reject
                blocks: vec![],
            }],
            creates: vec![
                CreateBlockOutput {
                    section: "s99".to_string(), // unknown section -> reject
                    text: "Valid text".to_string(),
                },
                CreateBlockOutput {
                    section: "s1".to_string(),
                    text: "   ".to_string(), // empty text -> reject
                },
            ],
            updates: vec![
                UpdateBlockOutput {
                    block: "b99".to_string(), // unknown block -> reject
                    text: "Valid text".to_string(),
                },
                UpdateBlockOutput {
                    block: "b1".to_string(),
                    text: "".to_string(), // empty text -> reject
                },
            ],
            deletes: vec![
                DeleteBlockOutput {
                    block: "b99".to_string(), // unknown block -> reject
                },
                DeleteBlockOutput {
                    block: "b2".to_string(), // valid delete -> resolve!
                },
            ],
        };

        let (resolved, rejected) = resolve_operations(&output, &handle_map);
        assert_eq!(resolved.len(), 1);
        assert_eq!(
            resolved[0],
            ResolvedOp::DeleteBlock {
                block_id: "blk_2".to_string(),
            }
        );
        assert_eq!(rejected.len(), 6);
    }

    #[test]
    fn test_apply_operations_success() {
        let memory = sample_model();
        let ops = vec![
            ResolvedOp::CreateSection {
                title: "Reading".to_string(),
                blocks: vec!["Reads Sci-Fi.".to_string()],
            },
            ResolvedOp::CreateBlock {
                section_id: "sec_alpha".to_string(),
                text: "Likes coffee.".to_string(),
            },
            ResolvedOp::UpdateBlock {
                block_id: "blk_1".to_string(),
                text: "Lives in Denver.".to_string(),
            },
        ];

        let report = apply_operations(&memory, &ops).unwrap();
        assert!(report.rejected.is_empty());
        assert_eq!(report.memory.sections.len(), 3);

        // Verify update
        let (s_i, b_i) = report.memory.find_block("blk_1").unwrap();
        assert_eq!(
            report.memory.sections[s_i].blocks[b_i].text,
            "Lives in Denver."
        );

        // Verify create block in sec_alpha
        assert_eq!(report.memory.sections[0].blocks.len(), 3);
        assert_eq!(report.memory.sections[0].blocks[2].text, "Likes coffee.");

        // Verify create section
        assert_eq!(report.memory.sections[2].title, "Reading");
        assert_eq!(report.memory.sections[2].blocks[0].text, "Reads Sci-Fi.");
    }

    #[test]
    fn test_apply_operations_duplicate_section_rejected() {
        let memory = sample_model();
        let ops = vec![ResolvedOp::CreateSection {
            title: "profile".to_string(), // duplicates "Profile"
            blocks: vec!["Text".to_string()],
        }];

        let report = apply_operations(&memory, &ops).unwrap();
        assert_eq!(report.rejected.len(), 1);
        assert!(report.rejected[0].reason.contains("already exists"));
    }

    #[test]
    fn test_apply_operations_missing_targets_rejected() {
        let memory = sample_model();
        let ops = vec![
            ResolvedOp::CreateBlock {
                section_id: "sec_nonexistent".to_string(),
                text: "Hello".to_string(),
            },
            ResolvedOp::UpdateBlock {
                block_id: "blk_nonexistent".to_string(),
                text: "Hello".to_string(),
            },
            ResolvedOp::DeleteBlock {
                block_id: "blk_nonexistent".to_string(),
            },
        ];

        let report = apply_operations(&memory, &ops).unwrap();
        assert_eq!(report.rejected.len(), 3);
        assert!(report.rejected[0].reason.contains("no longer exists"));
        assert!(report.rejected[1].reason.contains("no longer exists"));
        assert!(report.rejected[2].reason.contains("no longer exists"));
    }

    #[test]
    fn test_apply_operations_auto_prunes_empty_section() {
        let memory = sample_model();
        // sec_beta has only blk_3. Deleting blk_3 should auto-prune sec_beta!
        let ops = vec![ResolvedOp::DeleteBlock {
            block_id: "blk_3".to_string(),
        }];

        let report = apply_operations(&memory, &ops).unwrap();
        assert!(report.rejected.is_empty());
        assert_eq!(report.memory.sections.len(), 1);
        assert_eq!(report.memory.sections[0].id, "sec_alpha");
        assert_eq!(report.memory.find_section("sec_beta"), None);
    }

    #[test]
    fn test_extract_json_payload() {
        assert_eq!(
            extract_json_payload(r#"{"key": "value"}"#),
            r#"{"key": "value"}"#
        );
        assert_eq!(
            extract_json_payload("```json\n{\"key\": \"value\"}\n```"),
            r#"{"key": "value"}"#
        );
        assert_eq!(
            extract_json_payload("```\n{\"key\": \"value\"}\n```"),
            r#"{"key": "value"}"#
        );
    }

    #[test]
    fn test_resolved_op_is_non_destructive() {
        assert!(ResolvedOp::CreateSection {
            title: "T".to_string(),
            blocks: vec![]
        }
        .is_non_destructive());

        assert!(ResolvedOp::CreateBlock {
            section_id: "s".to_string(),
            text: "t".to_string()
        }
        .is_non_destructive());

        assert!(ResolvedOp::UpdateBlock {
            block_id: "b".to_string(),
            text: "t".to_string()
        }
        .is_non_destructive());

        assert!(!ResolvedOp::DeleteBlock {
            block_id: "b".to_string()
        }
        .is_non_destructive());
    }
}
