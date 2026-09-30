use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// One semantic prose block inside a section: a 1–3 sentence unit expressing a single coherent idea.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryBlock {
    pub id: String,
    pub text: String,
}

/// A titled grouping of prose blocks. The title is a first-class field, not a parsed Markdown heading.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemorySection {
    pub id: String,
    pub title: String,
    pub blocks: Vec<MemoryBlock>,
}

/// The canonical Personal Memory model: the source of truth persisted in `personal_memory.content`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonalMemory {
    pub sections: Vec<MemorySection>,
}

/// Bidirectional map between short per-request LLM handles and persistent semantic IDs.
#[derive(Debug, Clone, Default)]
pub struct HandleMap {
    section_ids: HashMap<String, String>,
    block_ids: HashMap<String, String>,
}

impl PersonalMemory {
    pub fn from_json(raw: &str) -> Result<Self> {
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(raw)
            .map_err(|e| anyhow!("Failed to parse personal memory JSON: {}", e))
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| anyhow!("Failed to serialize personal memory to JSON: {}", e))
    }

    pub fn validate(&self) -> Result<()> {
        if self.sections.is_empty() {
            return Err(anyhow!("Personal memory contains no sections"));
        }

        let mut titles: HashSet<String> = HashSet::new();
        for section in &self.sections {
            let title = section.title.trim();
            if title.is_empty() {
                return Err(anyhow!(
                    "Personal memory contains a section with an empty title"
                ));
            }
            if !titles.insert(title.to_lowercase()) {
                return Err(anyhow!(
                    "Personal memory contains a duplicate section title: '{}'",
                    title
                ));
            }
            for block in &section.blocks {
                if block.text.trim().is_empty() {
                    return Err(anyhow!(
                        "Section '{}' contains a block with empty text",
                        title
                    ));
                }
            }
        }
        Ok(())
    }

    /// Renders the model as Markdown for display, clipboard copy, and system-prompt injection.
    pub fn render_to_markdown(&self) -> String {
        let mut out = String::new();
        for (i, section) in self.sections.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str("## ");
            out.push_str(section.title.trim());
            out.push('\n');
            for block in &section.blocks {
                out.push_str(block.text.trim());
                out.push('\n');
            }
        }
        out
    }

    /// Parses a user-authored Markdown document back into the canonical model.
    pub fn from_markdown(markdown: &str) -> Self {
        let mut sections: Vec<MemorySection> = Vec::new();
        let mut current: String = String::new();
        let mut seen_heading = false;

        for line in markdown.lines() {
            let trimmed = line.trim();
            match parse_heading_title(trimmed) {
                Some(title) => {
                    flush_paragraph(&mut sections, std::mem::take(&mut current), seen_heading);
                    seen_heading = true;
                    push_section(&mut sections, title);
                }
                None if trimmed.is_empty() => {
                    flush_paragraph(&mut sections, std::mem::take(&mut current), seen_heading);
                }
                None => {
                    if !seen_heading {
                        continue;
                    }
                    if !current.is_empty() {
                        current.push(' ');
                    }
                    current.push_str(strip_bullet_marker(trimmed));
                }
            }
        }
        flush_paragraph(&mut sections, std::mem::take(&mut current), seen_heading);

        Self { sections }
    }

    /// Renders the model as handle-labelled text for the LLM, alongside the handle→ID map.
    pub fn to_handle_format(&self) -> (String, HandleMap) {
        let mut out = String::new();
        let mut map = HandleMap::default();
        let mut block_seq = 0usize;

        for (s_i, section) in self.sections.iter().enumerate() {
            if s_i > 0 {
                out.push('\n');
            }
            let section_handle = format!("s{}", s_i + 1);
            map.section_ids
                .insert(section_handle.clone(), section.id.clone());
            out.push_str(&format!("[{}] {}\n", section_handle, section.title.trim()));
            for block in &section.blocks {
                block_seq += 1;
                let block_handle = format!("b{}", block_seq);
                map.block_ids.insert(block_handle.clone(), block.id.clone());
                out.push_str(&format!("  [{}] {}\n", block_handle, block.text.trim()));
            }
        }
        (out, map)
    }

    /// Drops every section left with no blocks.
    pub fn prune_empty_sections(&mut self) {
        self.sections.retain(|section| !section.blocks.is_empty());
    }

    /// Builds a model from LLM-proposed new sections, assigning fresh persistent IDs.
    pub fn from_new_sections(new_sections: Vec<NewSectionDraft>) -> Self {
        let sections = new_sections
            .into_iter()
            .map(|draft| MemorySection {
                id: generate_section_id(),
                title: draft.title,
                blocks: draft
                    .blocks
                    .into_iter()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| MemoryBlock {
                        id: generate_block_id(),
                        text,
                    })
                    .collect(),
            })
            .collect();
        Self { sections }
    }

    /// Locates the block owning `block_id` as a `(section_index, block_index)` pair.
    pub fn find_block(&self, block_id: &str) -> Option<(usize, usize)> {
        self.sections.iter().enumerate().find_map(|(s_i, section)| {
            section
                .blocks
                .iter()
                .position(|block| block.id == block_id)
                .map(|b_i| (s_i, b_i))
        })
    }

    /// Locates the section owning `section_id`, returning its index.
    pub fn find_section(&self, section_id: &str) -> Option<usize> {
        self.sections
            .iter()
            .position(|section| section.id == section_id)
    }
}

impl HandleMap {
    /// Resolves a section handle (`s1`, `s2`, …) to its persistent `sec_*` ID.
    pub fn resolve_section(&self, handle: &str) -> Option<&str> {
        self.section_ids.get(handle.trim()).map(String::as_str)
    }

    /// Resolves a block handle (`b1`, `b2`, …) to its persistent `blk_*` ID.
    pub fn resolve_block(&self, handle: &str) -> Option<&str> {
        self.block_ids.get(handle.trim()).map(String::as_str)
    }
}

/// A section proposed by the LLM, before persistent IDs exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSectionDraft {
    pub title: String,
    pub blocks: Vec<String>,
}

pub fn generate_section_id() -> String {
    format!("sec_{}_{}", timestamp_hex(), uuid_suffix())
}

pub fn generate_block_id() -> String {
    format!("blk_{}_{}", timestamp_hex(), uuid_suffix())
}

fn parse_heading_title(trimmed: &str) -> Option<&str> {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let title = rest.trim();
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

fn strip_bullet_marker(trimmed: &str) -> &str {
    trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .unwrap_or(trimmed)
        .trim()
}

fn push_section(sections: &mut Vec<MemorySection>, title: &str) {
    sections.push(MemorySection {
        id: generate_section_id(),
        title: title.to_string(),
        blocks: Vec::new(),
    });
}

fn flush_paragraph(sections: &mut [MemorySection], paragraph: String, seen_heading: bool) {
    if paragraph.is_empty() || !seen_heading {
        return;
    }
    if let Some(section) = sections.last_mut() {
        section.blocks.push(MemoryBlock {
            id: generate_block_id(),
            text: paragraph,
        });
    }
}

fn timestamp_hex() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{:x}", millis)
}

fn uuid_suffix() -> String {
    uuid::Uuid::new_v4().to_string()[..4].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_model() -> PersonalMemory {
        PersonalMemory {
            sections: vec![
                MemorySection {
                    id: "sec_1".to_string(),
                    title: "Profile".to_string(),
                    blocks: vec![
                        MemoryBlock {
                            id: "blk_1".to_string(),
                            text: "User is a software engineer.".to_string(),
                        },
                        MemoryBlock {
                            id: "blk_2".to_string(),
                            text: "User lives in Seattle.".to_string(),
                        },
                    ],
                },
                MemorySection {
                    id: "sec_2".to_string(),
                    title: "Preferences".to_string(),
                    blocks: vec![MemoryBlock {
                        id: "blk_3".to_string(),
                        text: "Prefers dark mode.".to_string(),
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_validate_accepts_valid_model() {
        let model = sample_model();
        assert!(model.validate().is_ok());
    }

    #[test]
    fn test_validate_rejects_empty_sections() {
        let model = PersonalMemory { sections: vec![] };
        let err = model.validate().unwrap_err();
        assert!(err.to_string().contains("contains no sections"));
    }

    #[test]
    fn test_validate_rejects_empty_title() {
        let mut model = sample_model();
        model.sections[0].title = "   ".to_string();
        let err = model.validate().unwrap_err();
        assert!(err.to_string().contains("empty title"));
    }

    #[test]
    fn test_validate_rejects_duplicate_titles_case_insensitive() {
        let mut model = sample_model();
        model.sections.push(MemorySection {
            id: "sec_3".to_string(),
            title: "profile".to_string(),
            blocks: vec![MemoryBlock {
                id: "blk_4".to_string(),
                text: "Another profile block.".to_string(),
            }],
        });
        let err = model.validate().unwrap_err();
        assert!(err.to_string().contains("duplicate section title"));
    }

    #[test]
    fn test_validate_rejects_empty_block_text() {
        let mut model = sample_model();
        model.sections[0].blocks[0].text = "\n\t ".to_string();
        let err = model.validate().unwrap_err();
        assert!(err.to_string().contains("empty text"));
    }

    #[test]
    fn test_markdown_roundtrip_and_validation() {
        let model = sample_model();
        let md = model.render_to_markdown();
        assert!(md.contains("## Profile\nUser is a software engineer.\nUser lives in Seattle.\n"));
        assert!(md.contains("## Preferences\nPrefers dark mode.\n"));

        let parsed = PersonalMemory::from_markdown(&md);
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].title, "Profile");
        // Consecutive lines under a heading without blank lines are folded into a single paragraph block
        assert_eq!(parsed.sections[0].blocks.len(), 1);
        assert_eq!(
            parsed.sections[0].blocks[0].text,
            "User is a software engineer. User lives in Seattle."
        );
        assert_eq!(parsed.sections[1].title, "Preferences");
        assert_eq!(parsed.sections[1].blocks.len(), 1);
        assert_eq!(parsed.sections[1].blocks[0].text, "Prefers dark mode.");

        // Crucial invariant: parsed markdown must validate cleanly (no trailing empty section)
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn test_from_markdown_strips_bullets_and_ignores_preamble() {
        let raw = r#"Ignored preamble before any heading
Some chatter that should not be parsed into sections.

## Work Experience
- Software engineer at Vox.

* Previously worked on distributed systems.

Third sentence without a bullet.

## Hobbies
Enjoys hiking in the Cascades.
"#;
        let parsed = PersonalMemory::from_markdown(raw);
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].title, "Work Experience");
        // Blank-line separated paragraphs produce 3 distinct blocks with bullets stripped
        assert_eq!(parsed.sections[0].blocks.len(), 3);
        assert_eq!(
            parsed.sections[0].blocks[0].text,
            "Software engineer at Vox."
        );
        assert_eq!(
            parsed.sections[0].blocks[1].text,
            "Previously worked on distributed systems."
        );
        assert_eq!(
            parsed.sections[0].blocks[2].text,
            "Third sentence without a bullet."
        );
        assert_eq!(parsed.sections[1].title, "Hobbies");
        assert_eq!(parsed.sections[1].blocks.len(), 1);
        assert_eq!(
            parsed.sections[1].blocks[0].text,
            "Enjoys hiking in the Cascades."
        );
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn test_to_handle_format_and_handle_map() {
        let model = sample_model();
        let (view, map) = model.to_handle_format();

        assert!(view.contains("[s1] Profile"));
        assert!(view.contains("  [b1] User is a software engineer."));
        assert!(view.contains("  [b2] User lives in Seattle."));
        assert!(view.contains("[s2] Preferences"));
        assert!(view.contains("  [b3] Prefers dark mode."));

        assert_eq!(map.resolve_section("s1"), Some("sec_1"));
        assert_eq!(map.resolve_section("s2"), Some("sec_2"));
        assert_eq!(map.resolve_section("s3"), None);

        assert_eq!(map.resolve_block("b1"), Some("blk_1"));
        assert_eq!(map.resolve_block("b2"), Some("blk_2"));
        assert_eq!(map.resolve_block("b3"), Some("blk_3"));
        assert_eq!(map.resolve_block("b4"), None);
    }

    #[test]
    fn test_prune_empty_sections() {
        let mut model = sample_model();
        model.sections[0].blocks.clear();
        assert_eq!(model.sections.len(), 2);

        model.prune_empty_sections();
        assert_eq!(model.sections.len(), 1);
        assert_eq!(model.sections[0].id, "sec_2");
    }

    #[test]
    fn test_find_block_and_section() {
        let model = sample_model();
        assert_eq!(model.find_section("sec_1"), Some(0));
        assert_eq!(model.find_section("sec_2"), Some(1));
        assert_eq!(model.find_section("sec_99"), None);

        assert_eq!(model.find_block("blk_1"), Some((0, 0)));
        assert_eq!(model.find_block("blk_2"), Some((0, 1)));
        assert_eq!(model.find_block("blk_3"), Some((1, 0)));
        assert_eq!(model.find_block("blk_99"), None);
    }

    #[test]
    fn test_from_new_sections() {
        let drafts = vec![NewSectionDraft {
            title: "Draft Title".to_string(),
            blocks: vec![
                "Block one".to_string(),
                "  ".to_string(), // should be filtered
                "Block two".to_string(),
            ],
        }];
        let model = PersonalMemory::from_new_sections(drafts);
        assert_eq!(model.sections.len(), 1);
        assert_eq!(model.sections[0].title, "Draft Title");
        assert_eq!(model.sections[0].blocks.len(), 2);
        assert!(model.sections[0].id.starts_with("sec_"));
        assert!(model.sections[0].blocks[0].id.starts_with("blk_"));
        assert!(model.validate().is_ok());
    }
}
