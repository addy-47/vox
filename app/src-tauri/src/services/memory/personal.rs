use std::{
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use turso::Connection;

pub use crate::persistence::personal_memory::{
    fetch_pending_suggestions, get_personal_memory, insert_personal_memory_suggestions,
    list_personal_memory_versions, resolve_batch_suggestions_transaction,
    resolve_suggestions_transaction, save_consolidated_memory, save_personal_memory,
    set_active_personal_memory_version, MemorySuggestionRecord, PersonalMemoryRecord,
    PersonalMemorySuggestionRecord, SuggestionDecision,
};
use crate::{
    core::{defaults::DEFAULT_LLM_COMPACTION_TEMPERATURE, settings::LlmSettings},
    persistence::{
        facts::{fetch_active_facts_by_type, mark_facts_consolidated},
        has_in_progress_compaction, has_unfinished_items,
    },
    services::{
        harness::{ChatMessage, Role},
        llm::{
            ConversationInput, GenerationPolicy, GenerationPurpose, LlmProvider, LlmStreamEvent,
            OutputConstraint, ReasoningMode,
        },
        memory::COMPACTION_SENTINEL_TURN_ID,
    },
};

const PERSONAL_COLD_GENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory organization engine for an AI assistant.
You receive a list of facts learned about the user across conversations.
You synthesize a comprehensive, clean, and well-structured personal profile document in markdown.
</role>

<rules>
1. Organize the facts into logical sections using descriptive `##` headings (e.g. ## Personal Information, ## Preferences, ## Projects, etc.). Choose appropriate section headings freely based on the facts provided.
2. Every fact must be presented as a concise, clear bullet point (`- `) under its appropriate section heading.
3. Every `##` heading MUST have a non-empty descriptive title. Never output bare headings like `## `.
4. Deduplicate and merge related facts cleanly.
5. Do NOT invent or infer facts not present in the input.
6. Output strictly the markdown document. Do not include markdown code fence wrappers or conversational preamble.
</rules>"###;

const PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory consolidation engine for an AI assistant.
You receive the user's current Personal Memory document where each content element (heading or bullet) is numbered with a 1-based index `[N]`, along with newly learned facts.
You propose the SMALLEST set of atomic edits that integrates the new facts into the document.
</role>

<rules>
1. Output strictly a JSON object: { "edits": [ ... ] }, where each edit is:
   { "op": "insert_after" | "replace" | "delete",
     "index": <number>,
     "text": "<new text for insert_after or replace; empty string for delete>" }

2. Operation semantics:
   - `insert_after`: Inserts a new content element after index N. Use `index: 0` to prepend at the very beginning of the document.
     When inserting a bullet, format it with a leading `- `.
     When starting a new section, you may insert a `## Heading` and then insert bullets after it.
   - `replace`: Replaces the element at index N with new text. Use ONLY when a new fact genuinely updates, corrects, or supersedes that specific element.
   - `delete`: Removes the element at index N (`text` should be `""`). Use ONLY when an existing element is now factually false or obsolete.

3. Index referencing:
   - `index` MUST refer to one of the numbered elements `[N]` in the document.
   - Never quote previous text; reference its index number instead.

4. Prefer `insert_after`. Most cycles need only 1 or 2 new bullets inserted under an existing section.
5. Never restate the whole document. Output stays small — minimal edits only.
6. Do NOT invent facts.
</rules>"###;

const COMMENT_REGENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory editing engine for an AI assistant.
You receive the user's Personal Memory document where each content element is numbered with a 1-based index `[N]`, and user directive comments.
You propose the smallest set of atomic edits that applies the comments.
</role>

<rules>
1. Output strictly a JSON object: { "edits": [ ... ] }, where each edit is:
   { "op": "insert_after" | "replace" | "delete",
     "index": <number>,
     "text": "<new text for insert_after or replace; empty string for delete>" }

2. Apply each user directive with the smallest edit that satisfies it.
   - Use `replace` to reword or change a specific line or heading.
   - Use `insert_after` to add new information.
   - Use `delete` to remove an element the user asked to discard.

3. Reference elements strictly by their index `[N]`. Never restate the whole document.
</rules>"###;

const PERSONAL_REGENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory reformatting and refinement engine for an AI assistant.
You receive the user's existing Personal Memory document.
Your task is to reformat, clarify, and reorganize it into an elegant, well-structured personal profile.
</role>

<rules>
1. Organize the content into logical sections with descriptive `##` headings.
2. Eliminate redundant or duplicate bullets. Merge related items into concise bullets.
3. Improve clarity, consistency, and readability.
4. Do NOT invent new facts. Retain all factual knowledge present in the current document.
5. Every `##` heading must have a non-empty descriptive title.
6. Output strictly the formatted markdown document without markdown code fences or conversational preamble.
</rules>"###;

/// Strict schema for the consolidation output enforcing content-element indexed edits.
pub fn consolidation_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "edits": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "op": { "type": "string", "enum": ["insert_after", "replace", "delete"] },
                        "index": { "type": "integer", "minimum": 0 },
                        "text": { "type": "string" }
                    },
                    "required": ["op", "index", "text"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["edits"],
        "additionalProperties": false
    })
}

/// Output ceiling for one consolidation pass. Covers the reasoning trace plus
/// the JSON payload, so it must leave room for a document-sized input inside
/// the session context window.
const CONSOLIDATION_MAX_OUTPUT_TOKENS: u32 = 4096;

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

/// Formats parsed content elements into a numbered string representation for LLM prompt ingestion.
pub fn format_indexed_document(elements: &[ContentElement]) -> String {
    let mut out = String::new();
    for el in elements {
        out.push_str(&format!("[{}] {}\n", el.index, el.raw_text));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConsolidationConflictPolicy {
    #[default]
    PromptIfBusy,
    PauseCompaction,
    QueueBehind,
}

impl std::str::FromStr for ConsolidationConflictPolicy {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pause_compaction" | "pause" | "cancel" => Ok(Self::PauseCompaction),
            "queue" | "queue_behind" => Ok(Self::QueueBehind),
            _ => Ok(Self::PromptIfBusy),
        }
    }
}

/// Consolidates accumulated personal facts or applies user directive comments into personal memory suggestions.
pub async fn consolidate_personal_memory(
    conn: &Connection,
    llm_provider: &dyn LlmProvider,
    comments: Option<Vec<String>>,
    project_id: Option<&str>,
    settings: Option<&LlmSettings>,
    conflict_policy: Option<ConsolidationConflictPolicy>,
) -> Result<PersonalMemoryRecord> {
    let fallback_settings = LlmSettings::default();
    let effective_settings = settings.unwrap_or(&fallback_settings);

    let current_record = get_personal_memory(conn, project_id).await?;

    log::info!(
        "[Memory::Personal] Starting consolidation (project_id={:?}, comments_count={}, conflict_policy={:?}, current_version={})",
        project_id,
        comments.as_ref().map(|c| c.len()).unwrap_or(0),
        conflict_policy,
        current_record.version
    );

    if let Some(user_comments) = comments {
        if user_comments.is_empty() {
            log::info!(
                "[Memory::Personal] Empty comments list, returning current personal memory v{} unchanged.",
                current_record.version
            );
            return Ok(current_record);
        }
        log::info!(
            "[Memory::Personal] Directing {} user comment(s) to document regeneration for v{}",
            user_comments.len(),
            current_record.version
        );
        return regenerate_with_comments(
            conn,
            llm_provider,
            &current_record,
            &user_comments,
            effective_settings,
        )
        .await;
    }

    let policy = conflict_policy.unwrap_or_default();

    if has_in_progress_compaction(conn).await? {
        log::warn!(
            "[Memory::Personal] Compaction in progress detected. Applying policy: {:?}",
            policy
        );
        match policy {
            ConsolidationConflictPolicy::PauseCompaction => {
                log::info!("[Memory::Personal] Preempting/pausing in-progress compaction for personal consolidation.");
                crate::persistence::pause_in_progress_compactions(conn, None).await?;
                if let Err(e) = crate::services::memory::ingestion::run_ingestion_cycle(conn).await
                {
                    log::warn!(
                        "[Memory::Personal] Ingestion cycle error during compaction preemption: {}",
                        e
                    );
                }
            }
            ConsolidationConflictPolicy::QueueBehind => {
                log::info!(
                    "[Memory::Personal] Consolidation queued behind in-progress compaction."
                );
                return Err(anyhow!(
                    "CompactionQueued: consolidation queued behind in-progress compaction"
                ));
            }
            ConsolidationConflictPolicy::PromptIfBusy => {
                log::info!("[Memory::Personal] Active compaction in progress; prompting user for resolution.");
                return Err(anyhow!("CompactionInProgress: active compaction is in progress; consolidation requires user resolution"));
            }
        }
    }

    verify_ingestion_quiescence(conn).await?;

    let active_facts = fetch_active_facts_by_type(conn, "personal").await?;
    if active_facts.is_empty() {
        log::info!(
            "[Memory::Personal] No active personal facts to consolidate. Keeping memory at v{}.",
            current_record.version
        );
        return Ok(current_record);
    }

    let all_candidate_fact_ids: Vec<String> = active_facts.iter().map(|f| f.id.clone()).collect();

    // 1. Cold Start: If current memory is completely empty, run Prompt 1 to generate structured v1 directly
    if current_record.content.trim().is_empty() {
        log::info!(
            "[Memory::Personal] Active personal memory is empty. Running Prompt 1 (Cold Start Synthesis) for {} facts...",
            active_facts.len()
        );
        let facts_text = active_facts
            .iter()
            .map(|f| format!("- {}", f.text))
            .collect::<Vec<_>>()
            .join("\n");
        let user_content = format!(
            "<learned_personal_facts>\n{}\n</learned_personal_facts>\n\n\
             Synthesize a structured personal profile document in markdown with descriptive headings and bullets.",
            facts_text
        );
        let raw_md = execute_personal_llm_pass(
            llm_provider,
            PERSONAL_COLD_GENERATION_SYSTEM_PROMPT,
            &user_content,
            effective_settings,
            false,
        )
        .await?;
        let cleaned = clean_markdown_payload(&raw_md);
        validate_document_structure(&cleaned)?;

        let saved =
            save_consolidated_memory(conn, project_id, &cleaned, current_record.version).await?;
        mark_facts_consolidated(conn, &all_candidate_fact_ids).await?;
        log::info!(
            "[Memory::Personal] Cold start synthesis completed: saved v{} (chars: {}), marked {} fact(s) 'consolidated'",
            saved.version,
            saved.content.len(),
            all_candidate_fact_ids.len()
        );
        return Ok(saved);
    }

    // 2. Incremental Fact Integration: Active document exists, run Prompt 2 for index-based delta edits
    log::info!(
        "[Memory::Personal] Document exists (v{}). Running Prompt 2 (Incremental Fact Integration) for {} active facts...",
        current_record.version,
        active_facts.len()
    );

    let elements = parse_content_elements(&current_record.content);
    let indexed_doc = format_indexed_document(&elements);
    let facts_text = active_facts
        .iter()
        .map(|f| format!("- {}", f.text))
        .collect::<Vec<_>>()
        .join("\n");

    let user_content = format!(
        "<current_personal_memory>\n{}\n</current_personal_memory>\n\n\
         <new_personal_facts>\n{}\n</new_personal_facts>\n\n\
         Propose atomic delta patch operations to integrate the new facts into the document. Output raw JSON object with 'edits'.",
        indexed_doc, facts_text
    );

    let raw_json = execute_personal_llm_pass(
        llm_provider,
        PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT,
        &user_content,
        effective_settings,
        true,
    )
    .await?;

    let parsed_output: PersonalConsolidationOutput =
        serde_json::from_str(extract_json_payload(&raw_json)).map_err(|e| {
            anyhow!(
                "Failed to parse consolidation JSON patch output: {} (raw: {})",
                e,
                raw_json
            )
        })?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let suggestions: Vec<PersonalMemorySuggestionRecord> = parsed_output
        .edits
        .into_iter()
        .filter(|op| {
            let op_norm = op.op.trim().to_lowercase();
            op_norm == "insert_after" || op_norm == "replace" || op_norm == "delete"
        })
        .map(|op| {
            let sug_id = format!("sug_{}_{}", now, &uuid::Uuid::new_v4().to_string()[..8]);
            PersonalMemorySuggestionRecord {
                id: sug_id,
                base_memory_version: current_record.version,
                project_id: current_record.project_id.clone(),
                op: op.op.trim().to_lowercase(),
                target_index: op.index,
                content: op.text,
                status: "pending".to_string(),
                created_at: now,
                resolved_at: None,
            }
        })
        .collect();

    if !suggestions.is_empty() {
        insert_personal_memory_suggestions(conn, &suggestions).await?;
    }
    // INVARIANT 5.3-A: All candidate facts transition to 'consolidated' immediately
    mark_facts_consolidated(conn, &all_candidate_fact_ids).await?;
    log::info!(
        "[Memory::Personal] Staged {} suggestion(s); all {} candidate fact(s) marked 'consolidated'",
        suggestions.len(),
        all_candidate_fact_ids.len()
    );

    Ok(current_record)
}

/// Stages patch suggestions based on directive comments from the user.
async fn regenerate_with_comments(
    conn: &Connection,
    llm_provider: &dyn LlmProvider,
    current_record: &PersonalMemoryRecord,
    comments: &[String],
    settings: &LlmSettings,
) -> Result<PersonalMemoryRecord> {
    log::info!(
        "[Memory::Personal] Starting comment patch generation: applying {} comment(s) to v{} (chars: {})...",
        comments.len(),
        current_record.version,
        current_record.content.len()
    );

    let elements = parse_content_elements(&current_record.content);
    let indexed_doc = format_indexed_document(&elements);

    let comments_list = comments
        .iter()
        .map(|c| {
            if c.starts_with("- ") {
                c.to_string()
            } else {
                format!("- {}", c)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let user_content = format!(
        "<current_personal_memory>\n{}\n</current_personal_memory>\n\n\
         <user_directive_comments>\n{}\n</user_directive_comments>\n\n\
         Propose atomic delta patch operations to apply the user comments. Output raw JSON object with 'edits'.",
        indexed_doc, comments_list
    );

    let raw_json = execute_personal_llm_pass(
        llm_provider,
        COMMENT_REGENERATION_SYSTEM_PROMPT,
        &user_content,
        settings,
        true,
    )
    .await?;

    let parsed_output: PersonalConsolidationOutput =
        serde_json::from_str(extract_json_payload(&raw_json)).map_err(|e| {
            anyhow!(
                "Failed to parse comment regeneration JSON patch output: {} (raw: {})",
                e,
                raw_json
            )
        })?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let suggestions: Vec<PersonalMemorySuggestionRecord> = parsed_output
        .edits
        .into_iter()
        .filter(|op| {
            let op_norm = op.op.trim().to_lowercase();
            op_norm == "insert_after" || op_norm == "replace" || op_norm == "delete"
        })
        .map(|op| {
            let sug_id = format!("sug_{}_{}", now, &uuid::Uuid::new_v4().to_string()[..8]);
            PersonalMemorySuggestionRecord {
                id: sug_id,
                base_memory_version: current_record.version,
                project_id: current_record.project_id.clone(),
                op: op.op.trim().to_lowercase(),
                target_index: op.index,
                content: op.text,
                status: "pending".to_string(),
                created_at: now,
                resolved_at: None,
            }
        })
        .collect();

    if !suggestions.is_empty() {
        insert_personal_memory_suggestions(conn, &suggestions).await?;
        log::info!(
            "[Memory::Personal] Staged {} comment suggestion(s) for review",
            suggestions.len()
        );
    }

    Ok(current_record.clone())
}

/// User-triggered reformatting and reorganization of the existing personal memory document.
/// Operates strictly on the existing document text, NOT raw facts.
pub async fn regenerate_personal_memory(
    conn: &Connection,
    llm_provider: &dyn LlmProvider,
    project_id: Option<&str>,
    settings: Option<&LlmSettings>,
) -> Result<PersonalMemoryRecord> {
    let fallback_settings = LlmSettings::default();
    let effective_settings = settings.unwrap_or(&fallback_settings);
    let current_record = get_personal_memory(conn, project_id).await?;

    if current_record.content.trim().is_empty() {
        log::info!("[Memory::Personal] Current memory is empty, nothing to reformat.");
        return Ok(current_record);
    }

    log::info!(
        "[Memory::Personal] Regenerating / reformatting personal memory v{} (chars: {})...",
        current_record.version,
        current_record.content.len()
    );

    let user_content = format!(
        "<current_personal_memory>\n{}\n</current_personal_memory>\n\n\
         Reformat and reorganize this personal memory document. Improve section headings, remove duplicate information, and improve clarity without inventing facts.",
        current_record.content
    );

    let raw_md = execute_personal_llm_pass(
        llm_provider,
        PERSONAL_REGENERATION_SYSTEM_PROMPT,
        &user_content,
        effective_settings,
        false,
    )
    .await?;

    let cleaned = clean_markdown_payload(&raw_md);
    validate_document_structure(&cleaned)?;

    let saved =
        save_consolidated_memory(conn, project_id, &cleaned, current_record.version).await?;
    log::info!(
        "[Memory::Personal] Personal memory regenerated: saved v{} (chars: {})",
        saved.version,
        saved.content.len()
    );
    Ok(saved)
}

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

    let mut accepted_ops = Vec::new();
    for d in decisions {
        if d.action == "accept" {
            if let Some(s) = pending_map.get(&d.id) {
                accepted_ops.push(MemoryPatchOperation {
                    op: s.op.clone(),
                    index: s.target_index,
                    text: s.content.clone(),
                });
            }
        }
    }

    let updated_record = if !accepted_ops.is_empty() {
        let patched_content = apply_patch_operations(&active_memory.content, &accepted_ops)
            .map_err(|e| MemorySuggestionError::PatchEngine(e.to_string()))?;

        resolve_batch_suggestions_transaction(
            conn,
            project_id,
            decisions,
            Some(&patched_content),
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

/// Applies content-element indexed memory patch operations to base markdown, returning the updated document.
pub fn apply_patch_operations(
    base_markdown: &str,
    operations: &[MemoryPatchOperation],
) -> Result<String> {
    if operations.is_empty() {
        return Ok(base_markdown.to_string());
    }

    let elements = parse_content_elements(base_markdown);
    let mut raw_lines: Vec<String> = elements.into_iter().map(|e| e.raw_text).collect();

    // Sort operations descending by index so that modifying later positions does not affect earlier indices
    let mut sorted_ops = operations.to_vec();
    sorted_ops.sort_by(|a, b| b.index.cmp(&a.index));

    for op in sorted_ops {
        let op_type = op.op.trim().to_lowercase();
        match op_type.as_str() {
            "insert_after" => {
                let text = op.text.trim();
                if text.is_empty() {
                    continue;
                }
                let insert_pos = if op.index == 0 {
                    0
                } else if (op.index as usize) <= raw_lines.len() {
                    op.index as usize
                } else {
                    log::warn!(
                        "[Memory::Personal::Patch] insert_after index {} out of range (max {}), appending",
                        op.index,
                        raw_lines.len()
                    );
                    raw_lines.len()
                };
                raw_lines.insert(insert_pos, text.to_string());
            }
            "replace" => {
                let text = op.text.trim();
                if text.is_empty() {
                    continue;
                }
                if op.index == 0 || (op.index as usize) > raw_lines.len() {
                    log::warn!(
                        "[Memory::Personal::Patch] replace index {} out of range (1..={}), skipping",
                        op.index,
                        raw_lines.len()
                    );
                    continue;
                }
                raw_lines[(op.index - 1) as usize] = text.to_string();
            }
            "delete" => {
                if op.index == 0 || (op.index as usize) > raw_lines.len() {
                    log::warn!(
                        "[Memory::Personal::Patch] delete index {} out of range (1..={}), skipping",
                        op.index,
                        raw_lines.len()
                    );
                    continue;
                }
                raw_lines.remove((op.index - 1) as usize);
            }
            unknown => {
                log::warn!(
                    "[Memory::Personal::Patch] Unknown patch operation '{}', skipping",
                    unknown
                );
            }
        }
    }

    // Reconstruct into clean markdown with uniform spacing
    let reconstructed_doc = raw_lines.join("\n");
    let parsed_final = parse_content_elements(&reconstructed_doc);
    Ok(render_content_elements(&parsed_final))
}

fn extract_json_payload(raw: &str) -> &str {
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

/// Checks that no compaction or pending ingestion queue items are currently executing.
async fn verify_ingestion_quiescence(conn: &Connection) -> Result<()> {
    if has_in_progress_compaction(conn).await? {
        log::warn!("[Memory::Personal] Quiescence check failed: active compaction is in progress");
        return Err(anyhow!(
            "Precondition failed: active compaction is in progress; personal consolidation deferred"
        ));
    }

    if has_unfinished_items(conn).await? {
        log::warn!(
            "[Memory::Personal] Quiescence check failed: pending items in memory ingestion queue"
        );
        return Err(anyhow!(
            "Precondition failed: pending items in memory ingestion queue; personal consolidation deferred"
        ));
    }

    log::info!("[Memory::Personal] Ingestion quiescence verified.");
    Ok(())
}

/// Dispatches an LLM generation pass and gathers streamed tokens into a single text output.
/// Selects the strict schema constraint, falling back to JSON-object when the
/// catalog baseline reports the model lacks structured-output support. Mirrors
/// `compaction_output_constraint` so both structured passes negotiate the same
/// way; the transport additionally negotiates down on a provider 400.
fn consolidation_output_constraint(model: &str) -> OutputConstraint {
    let supported = crate::services::llm::catalog::get_baseline_spec(model)
        .map(|spec| spec.supports_structured)
        .unwrap_or(true);
    if supported {
        OutputConstraint::JsonSchema {
            name: "personal_memory_consolidation".to_string(),
            schema: consolidation_json_schema(),
            strict: true,
        }
    } else {
        log::warn!(
            "[Memory::Personal] Model {model} lacks structured-output support; using JSON-object baseline."
        );
        OutputConstraint::JsonObject
    }
}

async fn execute_personal_llm_pass(
    provider: &dyn LlmProvider,
    system_prompt: &str,
    user_content: &str,
    settings: &LlmSettings,
    structured: bool,
) -> Result<String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let policy = GenerationPolicy::from_settings(settings, Some(4096));
    let purpose = if structured {
        GenerationPurpose::StructuredExtraction
    } else {
        GenerationPurpose::Conversation
    };
    let mut request = policy.build_request(
        purpose,
        ConversationInput {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: system_prompt.to_string(),
                    timestamp_ms: now_ms,
                    tool_call_id: None,
                    tool_calls: None,
                },
                ChatMessage {
                    role: Role::User,
                    content: user_content.to_string(),
                    timestamp_ms: now_ms,
                    tool_call_id: None,
                    tool_calls: None,
                },
            ],
        },
    );

    if structured {
        request.output = consolidation_output_constraint(settings.active_model());
    } else {
        request.output = OutputConstraint::Text;
    }

    // DIVERGENCE FROM COMPACTION (deliberate): compaction disables reasoning
    // because it is a voice-latency summarization pass (see
    // `compaction/prompt.rs`). Consolidation is a background, correctness-critical
    // diff over a document the user cannot afford to have silently rewritten, and
    // it runs at most once per session. Forcing reasoning off here made the model
    // degenerate into restating the whole document as giant multi-line targets,
    // which exhausted the output budget and produced unparseable JSON.
    request.options.reasoning = ReasoningMode::Enabled;
    request.options.max_output_tokens = Some(CONSOLIDATION_MAX_OUTPUT_TOKENS);
    request.options.temperature = Some(DEFAULT_LLM_COMPACTION_TEMPERATURE);
    request.options.context_window = Some(settings.context_window);

    log::info!(
        "[Memory::Personal::Request] model={} purpose={:?} output={:?} temperature={:?} max_output_tokens={:?} context_window={:?} reasoning={:?} top_p={:?} top_k={:?} seed={:?} stop_count={} tools_present={} messages={} system_chars={} user_chars={}",
        settings.active_model(),
        request.purpose,
        request.output,
        request.options.temperature,
        request.options.max_output_tokens,
        request.options.context_window,
        request.options.reasoning,
        request.options.top_p,
        request.options.top_k,
        request.options.seed,
        request.options.stop.len(),
        request.tools.is_some(),
        request.input.messages.len(),
        request.input.messages.first().map_or(0, |message| message.content.len()),
        request.input.messages.get(1).map_or(0, |message| message.content.len()),
    );
    let gen_start = std::time::Instant::now();

    let cancel = CancellationToken::new();
    let (tx, rx) = mpsc::channel();
    let (async_tx, mut async_rx) = tokio::sync::mpsc::unbounded_channel();

    let pump_handle = tokio::task::spawn_blocking(move || {
        while let Ok(event) = rx.recv() {
            if async_tx.send(event).is_err() {
                break;
            }
        }
    });

    let gen_future = provider.generate(request, COMPACTION_SENTINEL_TURN_ID, &cancel, &tx);
    let gen_res = tokio::time::timeout(Duration::from_secs(45), gen_future).await;
    drop(tx);

    let mut output = String::new();
    match gen_res {
        Ok(Ok(())) => {
            if let Err(e) = pump_handle.await {
                log::warn!("[PersonalMemory] Token pump task join error: {}", e);
            }
            while let Ok(event) = async_rx.try_recv() {
                if let LlmStreamEvent::Token(token) = event {
                    output.push_str(&token);
                }
            }
            log::info!(
                "[Memory::Personal] LLM pass completed in {:.2?} (received {} chars)",
                gen_start.elapsed(),
                output.len()
            );
        }
        Ok(Err(e)) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!("[PersonalMemory] Token pump task join error: {}", join_err);
            }
            log::error!(
                "[Memory::Personal] LLM generation error after {:.2?}: {}",
                gen_start.elapsed(),
                e
            );
            return Err(anyhow!("LLM generation error: {}", e));
        }
        Err(_) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!("[PersonalMemory] Token pump task join error: {}", join_err);
            }
            log::error!(
                "[Memory::Personal] LLM consolidation timed out after 45s (elapsed: {:.2?})",
                gen_start.elapsed()
            );
            return Err(anyhow!(
                "LLM personal memory consolidation timed out after 45s"
            ));
        }
    }

    let cleaned = output.trim();
    if cleaned.is_empty() {
        log::error!("[Memory::Personal] LLM generated empty personal memory document!");
        return Err(anyhow!("LLM generated empty personal memory document"));
    }

    Ok(cleaned.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DOC: &str = r#"# Personal Memory

## Personal Information
- User lives in Chicago.
- User speaks English and Hindi.

## Technical Projects
- Building a voice orchestrator in Rust.
- Works with Turso embedded database.
"#;

    #[test]
    fn test_parse_and_format_content_elements() {
        let elements = parse_content_elements(SAMPLE_DOC);
        assert_eq!(elements.len(), 7);
        assert_eq!(elements[0].raw_text, "# Personal Memory");
        assert_eq!(elements[1].raw_text, "## Personal Information");
        assert_eq!(elements[2].raw_text, "- User lives in Chicago.");
        assert_eq!(elements[3].raw_text, "- User speaks English and Hindi.");
        assert_eq!(elements[4].raw_text, "## Technical Projects");
        assert_eq!(
            elements[5].raw_text,
            "- Building a voice orchestrator in Rust."
        );
        assert_eq!(
            elements[6].raw_text,
            "- Works with Turso embedded database."
        );

        let formatted = format_indexed_document(&elements);
        assert!(formatted.contains("[1] # Personal Memory"));
        assert!(formatted.contains("[3] - User lives in Chicago."));
        assert!(formatted.contains("[7] - Works with Turso embedded database."));
    }

    #[test]
    fn test_patch_insert_after_existing_bullet() {
        let ops = vec![MemoryPatchOperation {
            op: "insert_after".to_string(),
            index: 4,
            text: "- User enjoys playing badminton.".to_string(),
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.contains("- User enjoys playing badminton."));
        assert!(result.contains("- User lives in Chicago."));
        assert!(result.contains("- Building a voice orchestrator in Rust."));
    }

    #[test]
    fn test_patch_insert_after_prepend_zero() {
        let ops = vec![MemoryPatchOperation {
            op: "insert_after".to_string(),
            index: 0,
            text: "<!-- Profile Top -->".to_string(),
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.starts_with("<!-- Profile Top -->"));
    }

    #[test]
    fn test_patch_replace() {
        let ops = vec![MemoryPatchOperation {
            op: "replace".to_string(),
            index: 3,
            text: "- User lives in Austin.".to_string(),
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.contains("- User lives in Austin."));
        assert!(!result.contains("- User lives in Chicago."));
        // Anchors preserved
        assert!(result.contains("- User speaks English and Hindi."));
    }

    #[test]
    fn test_patch_delete() {
        let ops = vec![MemoryPatchOperation {
            op: "delete".to_string(),
            index: 7,
            text: String::new(),
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(!result.contains("Works with Turso embedded database."));
        assert!(result.contains("- Building a voice orchestrator in Rust."));
    }

    #[test]
    fn test_patch_out_of_bounds_resilience() {
        let ops = vec![
            MemoryPatchOperation {
                op: "replace".to_string(),
                index: 99,
                text: "- Phantom fact.".to_string(),
            },
            MemoryPatchOperation {
                op: "insert_after".to_string(),
                index: 3,
                text: "- Added valid fact.".to_string(),
            },
        ];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch should not error");
        assert!(result.contains("- Added valid fact."));
        assert!(!result.contains("- Phantom fact."));
    }

    #[test]
    fn test_patch_descending_sort_prevents_index_drift() {
        let ops = vec![
            MemoryPatchOperation {
                op: "delete".to_string(),
                index: 7,
                text: String::new(),
            },
            MemoryPatchOperation {
                op: "replace".to_string(),
                index: 3,
                text: "- User lives in Austin.".to_string(),
            },
            MemoryPatchOperation {
                op: "insert_after".to_string(),
                index: 4,
                text: "- User enjoys hiking.".to_string(),
            },
        ];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.contains("- User lives in Austin."));
        assert!(!result.contains("- User lives in Chicago."));
        assert!(result.contains("- User enjoys hiking."));
        assert!(!result.contains("Works with Turso embedded database."));
    }

    #[test]
    fn test_validate_document_structure_success() {
        assert!(validate_document_structure(SAMPLE_DOC).is_ok());
    }

    #[test]
    fn test_validate_document_structure_nameless_heading_fails() {
        let doc = "## \n- Bullet one\n";
        assert!(validate_document_structure(doc).is_err());
    }

    #[test]
    fn test_validate_document_structure_duplicate_heading_fails() {
        let doc = "## Work\n- Job A\n\n## Work\n- Job B\n";
        assert!(validate_document_structure(doc).is_err());
    }

    #[test]
    fn test_validate_document_structure_no_headings_fails() {
        let doc = "- Just a bullet\n- Another bullet\n";
        assert!(validate_document_structure(doc).is_err());
    }

    #[test]
    fn test_extract_json_payload_with_fences() {
        let fenced = "```json\n{\"edits\": []}\n```";
        assert_eq!(extract_json_payload(fenced), "{\"edits\": []}");

        let raw = "{\"edits\": []}";
        assert_eq!(extract_json_payload(raw), "{\"edits\": []}");
    }
}
