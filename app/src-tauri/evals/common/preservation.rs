use std::collections::HashSet;

use vox_lib::services::memory::personal::parse_content_elements;

use super::pipeline_report::SuggestionObservation;

/// Content lines of a markdown document: headings, blanks, and list bullets'
/// marker are excluded so preservation is compared on claim text.
pub fn content_lines(document: &str) -> Vec<String> {
    document
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            line.strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .unwrap_or(line)
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

/// The anti-erosion invariant, checked against the real documents: every base content
/// element that was not deleted or replaced by an operation must survive in the accepted document.
pub fn untargeted_preservation(
    base_document: &str,
    accepted_document: &str,
    operations: &[SuggestionObservation],
) -> (usize, usize, Vec<String>) {
    let base_elements = parse_content_elements(base_document);
    let accepted_elements = parse_content_elements(accepted_document);
    let accepted_texts: Vec<&str> = accepted_elements
        .iter()
        .map(|e| e.raw_text.trim())
        .collect();

    let targeted_indices: HashSet<u32> = operations
        .iter()
        .filter(|op| op.op == "replace" || op.op == "delete")
        .map(|op| op.target_index)
        .collect();

    let mut total = 0usize;
    let mut preserved = 0usize;
    let mut unexplained = Vec::new();

    for el in &base_elements {
        if targeted_indices.contains(&el.index) {
            // An operation targeted this element; its removal or replacement is accounted for.
            continue;
        }
        total += 1;
        if accepted_texts
            .iter()
            .any(|candidate| *candidate == el.raw_text.trim())
        {
            preserved += 1;
        } else {
            unexplained.push(el.raw_text.clone());
        }
    }
    (total, preserved, unexplained)
}
