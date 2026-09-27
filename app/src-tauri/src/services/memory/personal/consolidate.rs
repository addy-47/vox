use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

use super::{
    document::{
        clean_markdown_payload, format_indexed_document, parse_content_elements,
        validate_document_structure,
    },
    generation::execute_personal_llm_pass,
    patch::{extract_json_payload, PersonalConsolidationOutput},
    prompts::{
        COMMENT_REGENERATION_SYSTEM_PROMPT, PERSONAL_COLD_GENERATION_SYSTEM_PROMPT,
        PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT, PERSONAL_REGENERATION_SYSTEM_PROMPT,
    },
};
use crate::{
    core::settings::LlmSettings,
    persistence::{
        facts::{fetch_active_facts_by_type, mark_facts_consolidated},
        has_in_progress_compaction, has_unfinished_items,
        personal_memory::{
            get_personal_memory, insert_personal_memory_suggestions, save_consolidated_memory,
            PersonalMemoryRecord, PersonalMemorySuggestionRecord,
        },
    },
    services::llm::LlmProvider,
};

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
         The document above has {} content element(s), so valid indices are 0 to {}. \
         Propose atomic delta patch operations to integrate the new facts into the document. \
         Output raw JSON object with 'edits'.",
        indexed_doc,
        facts_text,
        elements.len(),
        elements.len()
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
         The document above has {} content element(s), so valid indices are 0 to {}. \
         Propose atomic delta patch operations to apply the user comments. \
         Output raw JSON object with 'edits'.",
        indexed_doc,
        comments_list,
        elements.len(),
        elements.len()
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
