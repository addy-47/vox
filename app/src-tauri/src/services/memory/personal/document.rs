use anyhow::{anyhow, Result};

/// Classification of a markdown content element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementKind {
    Heading(usize),
    Bullet,
    Paragraph,
}

/// An indexed line/element in the personal memory document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentElement {
    pub index: u32,
    pub kind: ElementKind,
    pub raw_text: String,
}

/// Parses a markdown document into indexed content elements, ignoring blank lines.
pub fn parse_content_elements(doc: &str) -> Vec<ContentElement> {
    let mut elements = Vec::new();
    let mut current_idx = 1u32;

    for line in doc.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let kind = if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            if level <= 6 && trimmed[level..].starts_with(' ') {
                ElementKind::Heading(level)
            } else {
                ElementKind::Paragraph
            }
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            ElementKind::Bullet
        } else {
            ElementKind::Paragraph
        };

        elements.push(ContentElement {
            index: current_idx,
            kind,
            raw_text: trimmed.to_string(),
        });
        current_idx += 1;
    }

    elements
}

/// Formats parsed content elements into a numbered, kind-labelled view for LLM prompt ingestion.
///
/// The `(kind)` label is load-bearing, not decoration. Without it the model has to infer
/// element kind from whether the text happens to start with `#`, and the observed failure
/// (`docs/plans/phase12/consolidation-structured--logic-plan.md` §3.8/D1) was a `replace`
/// aimed one index too low, overwriting `## Technical Projects` with a bullet and erasing the
/// section while every structural check still passed. Labelling the kind removes the inference.
pub fn format_indexed_document(elements: &[ContentElement]) -> String {
    let mut out = String::new();
    for el in elements {
        let kind = match el.kind {
            ElementKind::Heading(_) => "heading",
            ElementKind::Bullet => "bullet",
            ElementKind::Paragraph => "paragraph",
        };
        out.push_str(&format!("[{}] ({}) {}\n", el.index, kind, el.raw_text));
    }
    out
}

/// Formats parsed content elements back into clean markdown with uniform single blank-line delimiters between sections.
pub fn render_content_elements(elements: &[ContentElement]) -> String {
    let mut out = String::new();
    let mut prev_was_element = false;

    for el in elements {
        if let ElementKind::Heading(_) = el.kind {
            if prev_was_element && !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
        }
        out.push_str(&el.raw_text);
        out.push('\n');
        prev_was_element = true;
    }

    out
}

/// Strips markdown fences (e.g. ```markdown ... ```) and leading/trailing whitespace from LLM document payloads.
pub fn clean_markdown_payload(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = if let Some(rest) = trimmed.strip_prefix("```markdown") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        rest
    } else {
        trimmed
    };
    let stripped = if let Some(rest) = stripped.strip_suffix("```") {
        rest
    } else {
        stripped
    };
    stripped.trim().to_string()
}

/// Validates that a generated or reformatted document has non-empty heading titles, unique headings, and valid structure.
pub fn validate_document_structure(content: &str) -> Result<()> {
    let elements = parse_content_elements(content);
    if elements.is_empty() {
        return Err(anyhow!("Generated document is empty"));
    }

    let mut heading_titles = std::collections::HashSet::new();
    let mut heading_count = 0;

    for el in &elements {
        if let ElementKind::Heading(_) = el.kind {
            heading_count += 1;
            let title = el.raw_text.trim_start_matches('#').trim();
            if title.is_empty() {
                return Err(anyhow!(
                    "Document contains nameless heading: '{}'",
                    el.raw_text
                ));
            }
            if !heading_titles.insert(title.to_lowercase()) {
                return Err(anyhow!("Document contains duplicate heading: '{}'", title));
            }
        }
    }

    if heading_count == 0 {
        return Err(anyhow!("Document contains no section headings"));
    }

    Ok(())
}

/// Counts the section headings in a document.
pub fn heading_count(content: &str) -> usize {
    parse_content_elements(content)
        .iter()
        .filter(|el| matches!(el.kind, ElementKind::Heading(_)))
        .count()
}

/// Rejects a patched document that dropped a section the base document had.
///
/// `validate_document_structure` only forbids *malformed* output — a nameless or duplicated
/// heading. It cannot see a *missing* section, because a document that simply lost one is
/// still perfectly well-formed. That is exactly how a `replace` aimed one index too low
/// erases a heading and re-parents its bullets under the previous section while every
/// structural assertion still passes (§3.8/D1).
///
/// Counting rather than comparing titles is deliberate: renaming a section legitimately
/// changes its title while preserving its count, so a title comparison would reject correct
/// behaviour. A decrease in count is the only unambiguous signal that a section was destroyed.
pub fn validate_no_heading_loss(before: &str, after: &str) -> Result<()> {
    let before_count = heading_count(before);
    let after_count = heading_count(after);
    if after_count < before_count {
        return Err(anyhow!(
            "Patch reduced section count from {before_count} to {after_count}; refusing to commit \
             to avoid silent section loss"
        ));
    }
    Ok(())
}
