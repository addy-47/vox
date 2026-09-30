use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// One semantic prose block inside a section: a 1–3 sentence unit expressing a single coherent idea.
///
/// Block identity is persistent and semantic, never positional. A block keeps its `id` across a
/// text rewrite, which is what lets a revision target a block without shifting any neighbour.
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
///
/// Markdown is a derived rendering produced by `render_to_markdown`, never the stored form. This is
/// the architectural change that removed positional addressing from the LLM contract
/// (`docs/plans/phase12/semantic-structured-personal-memory-architecture.md` §0).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonalMemory {
    pub sections: Vec<MemorySection>,
}

/// Bidirectional map between short per-request LLM handles and persistent semantic IDs.
///
/// The LLM never sees or assigns a persistent ID. It sees `s1`/`b2`-style handles, and the engine
/// resolves them back to `sec_*`/`blk_*` after validation. A handle absent from the map is a
/// validation error, cleanly rejected per operation.
#[derive(Debug, Clone, Default)]
pub struct HandleMap {
    section_ids: HashMap<String, String>,
    block_ids: HashMap<String, String>,
}

impl PersonalMemory {
    /// Deserializes the canonical JSON stored in `personal_memory.content`.
    ///
    /// An unpopulated memory is the empty canonical form `{"sections":[]}`, so an empty or
    /// whitespace-only payload yields an empty model rather than a parse failure. Returns an error
    /// only when the payload is present but malformed.
    pub fn from_json(raw: &str) -> Result<Self> {
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(raw)
            .map_err(|e| anyhow!("Failed to parse personal memory JSON: {}", e))
    }

    /// Serializes the model to its canonical JSON form for persistence.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| anyhow!("Failed to serialize personal memory to JSON: {}", e))
    }

    /// Enforces the four-part structure contract from `memory-spec.md §5.1`.
    ///
    /// Non-empty section titles, case-insensitively unique section titles, at least one section, and
    /// non-empty block text. The same validator gates cold generation, regeneration, manual save,
    /// and acceptance of every revision batch, so a violating candidate is never committed.
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
    ///
    /// Pure function with zero semantic intelligence: it emits IDs nowhere and performs no
    /// validation. Two sections are separated by a blank line.
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
    ///
    /// The deterministic inverse of `render_to_markdown`: an ATX heading opens a section, each
    /// paragraph of non-heading lines becomes exactly one prose block, and leading `- `/`* ` bullet
    /// markers are stripped. Block granularity is the paragraph, matching the renderer, so a block
    /// authored across several lines survives a save/load round-trip as one block.
    ///
    /// Text appearing before the first heading is discarded, because a section-less block has nowhere
    /// to live. A document with no heading yields the empty model; the caller decides whether an
    /// empty result is an explicit clear or an accidental paste.
    ///
    /// This function does not validate, because the empty model is legal. The caller validates any
    /// non-empty result. No LLM is invoked here.
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
        push_section(&mut sections, "");

        Self { sections }
    }

    /// Renders the model as handle-labelled text for the LLM, alongside the handle→ID map.
    ///
    /// Sections receive `s1`, `s2`, … in array order; blocks receive `b1`, `b2`, … numbered
    /// sequentially across all sections. The returned map is the only bridge between the two.
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
    ///
    /// Called after any mutation that deletes blocks. There is no `delete_section` operation: empty
    /// sections disappear as a consequence of deleting their content.
    pub fn prune_empty_sections(&mut self) {
        self.sections.retain(|section| !section.blocks.is_empty());
    }

    /// Builds a model from LLM-proposed new sections, assigning fresh persistent IDs.
    ///
    /// Used by cold generation and regeneration, where the model proposes only whole sections.
    /// Empty block text is dropped rather than producing a block that would fail `validate`.
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

/// Mints a persistent section ID of the form `sec_{timestamp_hex}_{4-char-uuid}`.
///
/// IDs are assigned exclusively by the application. The suffix is drawn from a full UUID v4, so
/// collision probability is negligible even for two IDs minted in the same millisecond.
pub fn generate_section_id() -> String {
    format!("sec_{}_{}", timestamp_hex(), uuid_suffix())
}

/// Mints a persistent block ID of the form `blk_{timestamp_hex}_{4-char-uuid}`.
pub fn generate_block_id() -> String {
    format!("blk_{}_{}", timestamp_hex(), uuid_suffix())
}

/// Extracts the heading text from an ATX heading line (`#` through `######`), or `None` when the line
/// is not a heading or carries no title text.
///
/// Any level is accepted, not just `##`, because a user editing in the UI may write `# Heading` even
/// though the renderer only ever emits `##`.
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

/// Strips a leading `- ` or `* ` bullet marker, returning the bare text.
fn strip_bullet_marker(trimmed: &str) -> &str {
    trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .unwrap_or(trimmed)
        .trim()
}

/// Opens a new section under `title` and returns an ID for it.
///
/// Sections are created eagerly here and filled by `flush_paragraph`, so a heading with no content
/// still yields a section. The caller prunes any section left with no blocks.
fn push_section(sections: &mut Vec<MemorySection>, title: &str) {
    sections.push(MemorySection {
        id: generate_section_id(),
        title: title.to_string(),
        blocks: Vec::new(),
    });
}

/// Emits `paragraph` as a block of the most recently opened section.
///
/// Does nothing when the paragraph is empty or when no heading has been seen yet, which is how
/// text preceding the first heading is discarded.
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



/// Millisecond epoch timestamp in hex, used as the human-readable prefix of every persistent ID.
fn timestamp_hex() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{:x}", millis)
}

/// First four characters of a random UUID v4, used as the collision-avoidance suffix of every
/// persistent ID.
fn uuid_suffix() -> String {
    uuid::Uuid::new_v4().to_string()[..4].to_string()
}
