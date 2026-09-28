use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::document::{parse_content_elements, render_content_elements, ElementKind};

/// An individual atomic patch operation proposed by the consolidation engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPatchOperation {
    pub op: String,
    pub index: u32,
    #[serde(default)]
    pub text: String,
}

/// JSON payload structure emitted by the consolidation LLM pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalConsolidationOutput {
    pub edits: Vec<MemoryPatchOperation>,
}

/// An operation the engine refused to apply, with the reason.
///
/// Refusing an operation must never be silent. Sprint 2 showed why: while rejections were
/// `log::warn!`-only, an unsafe operation could not be attributed to a suggestion row, so the
/// refusal had to be escalated to the whole batch, and escalating to the batch stalled
/// consolidation permanently (`consolidation-structured--logic-plan.md` §4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedOperation {
    /// Index into the `operations` slice the caller passed in, so the caller can map the
    /// rejection back to the suggestion row it came from.
    pub position: usize,
    pub operation: MemoryPatchOperation,
    pub reason: String,
}

/// Outcome of a patch application: the resulting document plus everything refused along the way.
#[derive(Debug, Clone)]
pub struct ApplyReport {
    pub document: String,
    pub rejected: Vec<RejectedOperation>,
}

/// True when `text` has the shape of a markdown heading line.
fn is_heading_text(text: &str) -> bool {
    text.trim_start().starts_with('#')
}

/// Applies content-element indexed memory patch operations to base markdown.
///
/// Unsafe operations are dropped individually and reported in `ApplyReport::rejected`; the rest
/// of the batch is still applied. Refusing the entire batch instead would guarantee the document
/// never changes, which is a worse failure than the one it prevents (§4.5).
///
/// The element kind is carried through to apply time specifically so a `replace` aimed at a
/// heading can be recognised and refused. A `replace` whose target is a heading but whose
/// replacement is not a heading erases that section and re-parents its bullets under the previous
/// heading, while producing a document that still satisfies every structural check.
pub fn apply_patch_operations(
    base_markdown: &str,
    operations: &[MemoryPatchOperation],
) -> Result<ApplyReport> {
    if operations.is_empty() {
        return Ok(ApplyReport {
            document: base_markdown.to_string(),
            rejected: Vec::new(),
        });
    }

    let elements = parse_content_elements(base_markdown);
    let mut raw_lines: Vec<String> = elements.iter().map(|e| e.raw_text.clone()).collect();
    // Kinds are indexed in lockstep with `raw_lines` and kept in sync through every mutation, so
    // an operation's target kind stays addressable as earlier positions shift.
    let mut kinds: Vec<ElementKind> = elements.iter().map(|e| e.kind.clone()).collect();

    let base_len = raw_lines.len();

    // An operation anchored past the end of the document is an append, not an absolute slot.
    // Sorting descending is only sound for absolute indices: a clamped index keeps landing at
    // "wherever the end happens to be", so two appends emitted in order would be applied in
    // reverse and a new section's heading would end up below its own bullets. Absolute operations
    // are applied high-to-low so later positions do not shift earlier ones; appends then land in
    // the order the engine emitted them. This is the shape the consolidation prompt documents
    // for opening a new section: a heading, then its bullets, each anchored further out.
    let mut absolute_ops: Vec<(usize, u32, MemoryPatchOperation)> = Vec::new();
    let mut append_ops: Vec<(usize, MemoryPatchOperation)> = Vec::new();
    for (position, op) in operations.iter().cloned().enumerate() {
        if (op.index as usize) > base_len {
            append_ops.push((position, op));
        } else {
            absolute_ops.push((position, op.index, op));
        }
    }
    // Ties keep their input order, so `position` stays meaningful.
    absolute_ops.sort_by(|a, b| b.1.cmp(&a.1));
    let sorted_ops: Vec<(usize, bool, MemoryPatchOperation)> = absolute_ops
        .into_iter()
        .map(|(position, _, op)| (position, false, op))
        .chain(
            append_ops
                .into_iter()
                .map(|(position, op)| (position, true, op)),
        )
        .collect();

    let mut rejected: Vec<RejectedOperation> = Vec::new();

    for (position, is_append, op) in sorted_ops {
        let op_type = op.op.trim().to_lowercase();
        let text = op.text.trim();
        let op_index = op.index;
        match op_type.as_str() {
            "insert_after" => {
                if text.is_empty() {
                    rejected.push(RejectedOperation {
                        position,
                        operation: op,
                        reason: "insert_after carried no text".to_string(),
                    });
                    continue;
                }
                let insert_pos = if is_append
                    || (op_index != 0 && (op_index as usize) > raw_lines.len())
                {
                    if !is_append {
                        log::debug!(
                            "[Memory::Personal::Patch] insert_after index {op_index} past the end (max {}), appending",
                            raw_lines.len()
                        );
                    }
                    raw_lines.len()
                } else if op_index == 0 {
                    0
                } else {
                    op_index as usize
                };
                raw_lines.insert(insert_pos, text.to_string());
                kinds.insert(insert_pos, kind_for_text(text));
            }
            "replace" => {
                if text.is_empty() {
                    rejected.push(RejectedOperation {
                        position,
                        operation: op,
                        reason: "replace carried no text".to_string(),
                    });
                    continue;
                }
                if op_index == 0 || (op_index as usize) > raw_lines.len() {
                    rejected.push(RejectedOperation {
                        position,
                        operation: op,
                        reason: format!(
                            "replace index {op_index} is out of range (1..={})",
                            raw_lines.len()
                        ),
                    });
                    continue;
                }
                let slot = (op_index - 1) as usize;
                // A heading replaced by a non-heading erases the section. Refused per-operation
                // so the rest of the batch still lands.
                if matches!(kinds[slot], ElementKind::Heading(_)) && !is_heading_text(text) {
                    rejected.push(RejectedOperation {
                        position,
                        operation: op,
                        reason: format!(
                            "replace index {op_index} targets the section heading {:?} with non-heading \
                             text, which would erase the section",
                            raw_lines[slot]
                        ),
                    });
                    continue;
                }
                raw_lines[slot] = text.to_string();
                kinds[slot] = kind_for_text(text);
            }
            "delete" => {
                if op_index == 0 || (op_index as usize) > raw_lines.len() {
                    rejected.push(RejectedOperation {
                        position,
                        operation: op,
                        reason: format!(
                            "delete index {op_index} is out of range (1..={})",
                            raw_lines.len()
                        ),
                    });
                    continue;
                }
                let slot = (op_index - 1) as usize;
                raw_lines.remove(slot);
                kinds.remove(slot);
            }
            unknown => {
                rejected.push(RejectedOperation {
                    position,
                    operation: op,
                    reason: format!("unknown patch operation '{unknown}'"),
                });
            }
        }
    }

    let reconstructed_doc = raw_lines.join("\n");
    let parsed_final = parse_content_elements(&reconstructed_doc);
    Ok(ApplyReport {
        document: render_content_elements(&parsed_final),
        rejected,
    })
}

/// Classifies the element a piece of inserted or replacement text will become.
fn kind_for_text(text: &str) -> ElementKind {
    let trimmed = text.trim();
    if is_heading_text(trimmed) {
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        return ElementKind::Heading(level.clamp(1, 6));
    }
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return ElementKind::Bullet;
    }
    ElementKind::Paragraph
}

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
