use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

use super::{
    generation::execute_personal_llm_pass,
    model::{MemorySection, NewSectionDraft, PersonalMemory},
    operations::{
        apply_operations, extract_json_payload, resolve_operations, ApplyReport,
        ConsolidationOutput, RejectedOperation, ResolvedOp,
    },
    prompts::{
        delta_consolidation_json_schema, whole_memory_json_schema,
        COMMENT_DIRECTED_EDIT_SYSTEM_PROMPT, PERSONAL_COLD_GENERATION_SYSTEM_PROMPT,
        PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT, PERSONAL_REGENERATION_SYSTEM_PROMPT,
    },
    revisions::stage_revisions,
};
use crate::{
    config::settings::PersonalMemorySettings,
    persistence::{
        facts::{
            fetch_active_observations_by_type, mark_observations_integrated, ObservationRecord,
        },
        has_in_progress_compaction,
        personal_memory::{
            get_personal_memory, reject_all_pending_revisions, save_consolidated_memory,
            PersonalMemoryRecord,
        },
        queue::{count_unfinished_items, has_unfinished_items},
    },
    services::llm::{LlmProvider, LlmSettings},
};

const SUGGESTION_POLICY_AUTO_APPLY: &str = "auto_apply";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationReason {
    CompactionInProgress,
    PendingQueueItems,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConsolidateOutcome {
    Completed {
        record: PersonalMemoryRecord,
    },
    ConfirmationRequired {
        reason: ConfirmationReason,
        pending_count: i64,
    },
}

/// A pass result: the outcome to return, plus whether the pass actually changed the document.
struct PassResult {
    outcome: ConsolidateOutcome,
    landed: bool,
}

impl ConsolidateOutcome {
    fn completed(record: PersonalMemoryRecord) -> Self {
        Self::Completed { record }
    }
}

pub struct ConsolidationRequest<'a> {
    pub conn: &'a Connection,
    pub llm_provider: &'a dyn LlmProvider,
    pub comments: Option<Vec<String>>,
    pub project_id: Option<&'a str>,
    pub memory_settings: &'a PersonalMemorySettings,
    pub llm_settings: Option<&'a LlmSettings>,
    pub forced: bool,
}

/// Everything one consolidation pass needs, bundled for the same reason as `ConsolidationRequest`.
struct ConsolidationPass<'a> {
    conn: &'a Connection,
    llm_provider: &'a dyn LlmProvider,
    current_record: &'a PersonalMemoryRecord,
    project_id: Option<&'a str>,
    memory_settings: &'a PersonalMemorySettings,
    llm_settings: &'a LlmSettings,
}

/// Integrates active personal observations into the semantic Personal Memory model, or stages
/// comment-directed edits.
pub async fn consolidate_personal_memory(
    request: ConsolidationRequest<'_>,
) -> Result<ConsolidateOutcome> {
    let fallback_settings = LlmSettings::default();
    let effective_settings = request.llm_settings.unwrap_or(&fallback_settings);
    let current_record = get_personal_memory(request.conn, request.project_id).await?;
    let pass = ConsolidationPass {
        conn: request.conn,
        llm_provider: request.llm_provider,
        current_record: &current_record,
        project_id: request.project_id,
        memory_settings: request.memory_settings,
        llm_settings: effective_settings,
    };

    if let Some(user_comments) = request.comments {
        return stage_comment_directed_edits(&pass, &user_comments).await;
    }

    if let Some(blocked) = gate_on_compaction_and_queue(request.conn, request.forced).await? {
        return Ok(blocked);
    }

    let candidates = fetch_active_observations_by_type(request.conn, "personal").await?;
    if candidates.is_empty() {
        log::info!(
            "[Memory::Personal] No active personal observations to integrate. Memory stays at v{}.",
            current_record.version
        );
        return Ok(ConsolidateOutcome::completed(current_record));
    }

    let existing_sections = stored_sections(&current_record).len();
    let pass_result = if existing_sections == 0 {
        run_cold_generation(&pass, &candidates).await?
    } else {
        run_incremental_integration(&pass, &candidates).await?
    };

    if !pass_result.landed {
        log::warn!(
            "[Memory::Personal] Pass produced no applicable operation; {} observation(s) left 'active' for the next run.",
            candidates.len()
        );
        return Ok(pass_result.outcome);
    }

    let ids: Vec<String> = candidates
        .iter()
        .map(|observation| observation.id.clone())
        .collect();
    mark_observations_integrated(request.conn, &ids).await?;
    log::info!(
        "[Memory::Personal] Integrated {} observation(s) into memory derived from v{}",
        ids.len(),
        current_record.version
    );
    Ok(pass_result.outcome)
}

/// Evaluates the two user-controlled gates, returning a `ConfirmationRequired` outcome when one
/// blocks the run and `None` when consolidation may proceed.
async fn gate_on_compaction_and_queue(
    conn: &Connection,
    forced: bool,
) -> Result<Option<ConsolidateOutcome>> {
    if has_in_progress_compaction(conn).await? {
        log::info!("[Memory::Personal] Compaction in progress; requesting user resolution.");
        return Ok(Some(ConsolidateOutcome::ConfirmationRequired {
            reason: ConfirmationReason::CompactionInProgress,
            pending_count: 0,
        }));
    }

    if !forced && has_unfinished_items(conn).await? {
        let pending_count = count_unfinished_items(conn).await?;
        log::info!(
            "[Memory::Personal] {} unfinished ingestion item(s); requesting confirmation.",
            pending_count
        );
        return Ok(Some(ConsolidateOutcome::ConfirmationRequired {
            reason: ConfirmationReason::PendingQueueItems,
            pending_count,
        }));
    }

    Ok(None)
}

/// Cold generation: synthesize a complete semantic model from the candidate observations.
async fn run_cold_generation(
    pass: &ConsolidationPass<'_>,
    candidates: &[ObservationRecord],
) -> Result<PassResult> {
    let user_content = format!(
        "<learned_observations>\n{}\n</learned_observations>\n\n\
         Synthesize a complete structured personal memory from these observations.",
        render_observation_bullets(candidates)
    );

    let drafts = run_whole_memory_pass(
        pass,
        PERSONAL_COLD_GENERATION_SYSTEM_PROMPT,
        &user_content,
        "cold generation",
    )
    .await?;

    if drafts.is_empty() {
        return Err(anyhow!(
            "Cold generation returned no usable sections; memory left unchanged at v{}",
            pass.current_record.version
        ));
    }
    let saved = commit_new_structure(pass, drafts, false, "cold generation").await?;
    Ok(PassResult {
        outcome: ConsolidateOutcome::completed(saved),
        landed: true,
    })
}

/// Incremental integration: fold the candidate observations into the existing semantic model.
async fn run_incremental_integration(
    pass: &ConsolidationPass<'_>,
    candidates: &[ObservationRecord],
) -> Result<PassResult> {
    let memory = PersonalMemory::from_json(&pass.current_record.content)?;
    let (handle_view, handle_map) = memory.to_handle_format();
    let user_content = format!(
        "<current_memory>\n{}\n</current_memory>\n\n\
         <new_observations>\n{}\n</new_observations>\n\n\
         Propose the minimal set of semantic operations that integrates the new observations. \
         Reference existing content only by the handles shown above.",
        handle_view,
        render_observation_bullets(candidates)
    );

    let output = run_structured_pass(
        pass,
        PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT,
        &user_content,
        "incremental integration",
    )
    .await?;

    let (resolved, rejected) = resolve_operations(&output, &handle_map);
    log_operation_rejections("incremental integration", &rejected);

    if pass.memory_settings.suggestion_policy == SUGGESTION_POLICY_AUTO_APPLY {
        auto_apply_operations(pass, resolved).await
    } else {
        let staged = stage_revisions(pass.conn, pass.current_record, resolved).await?;
        log::info!(
            "[Memory::Personal] Staged {} revision(s) for review against v{}.",
            staged,
            pass.current_record.version
        );
        Ok(PassResult {
            outcome: ConsolidateOutcome::completed(pass.current_record.clone()),
            landed: staged > 0,
        })
    }
}

/// Comment-directed editing: apply the user's own directives to the existing memory.
async fn stage_comment_directed_edits(
    pass: &ConsolidationPass<'_>,
    comments: &[String],
) -> Result<ConsolidateOutcome> {
    if comments.is_empty() {
        log::info!(
            "[Memory::Personal] Empty comment list; memory unchanged at v{}.",
            pass.current_record.version
        );
        return Ok(ConsolidateOutcome::completed(pass.current_record.clone()));
    }

    let memory = PersonalMemory::from_json(&pass.current_record.content)?;
    let (handle_view, handle_map) = memory.to_handle_format();
    let user_content = format!(
        "<current_memory>\n{}\n</current_memory>\n\n\
         <user_directive_comments>\n{}\n</user_directive_comments>\n\n\
         Propose the minimal set of semantic operations that applies the directives.",
        handle_view,
        render_comment_bullets(comments)
    );

    let output = run_structured_pass(
        pass,
        COMMENT_DIRECTED_EDIT_SYSTEM_PROMPT,
        &user_content,
        "comment-directed edit",
    )
    .await?;

    let (resolved, rejected) = resolve_operations(&output, &handle_map);
    log_operation_rejections("comment-directed edit", &rejected);
    let staged = stage_revisions(pass.conn, pass.current_record, resolved).await?;
    log::info!(
        "[Memory::Personal] Staged {} comment-driven revision(s) against v{}.",
        staged,
        pass.current_record.version
    );
    Ok(ConsolidateOutcome::completed(pass.current_record.clone()))
}

/// Regeneration: replace the entire structure with a freshly organized one, all persistent IDs new.
pub async fn regenerate_personal_memory(
    conn: &Connection,
    llm_provider: &dyn LlmProvider,
    project_id: Option<&str>,
    memory_settings: &PersonalMemorySettings,
    llm_settings: Option<&LlmSettings>,
) -> Result<PersonalMemoryRecord> {
    let fallback_settings = LlmSettings::default();
    let effective_settings = llm_settings.unwrap_or(&fallback_settings);
    let current_record = get_personal_memory(conn, project_id).await?;

    if stored_sections(&current_record).is_empty() {
        log::info!("[Memory::Personal] Current memory is empty, nothing to regenerate.");
        return Ok(current_record);
    }

    let pass = ConsolidationPass {
        conn,
        llm_provider,
        current_record: &current_record,
        project_id,
        memory_settings,
        llm_settings: effective_settings,
    };

    let memory = PersonalMemory::from_json(&current_record.content)?;
    let (handle_view, _) = memory.to_handle_format();
    let user_content = format!(
        "<current_memory>\n{}\n</current_memory>\n\n\
         Re-synthesize and reorganize this memory into an elevated, coherent structure. Preserve all factual information.",
        handle_view
    );

    let drafts = run_whole_memory_pass(
        &pass,
        PERSONAL_REGENERATION_SYSTEM_PROMPT,
        &user_content,
        "regeneration",
    )
    .await?;

    if drafts.is_empty() {
        return Err(anyhow!(
            "Regeneration returned no usable sections; memory left unchanged at v{}",
            current_record.version
        ));
    }

    commit_new_structure(&pass, drafts, true, "regeneration").await
}

/// Commits a freshly built structure as the next active memory version.
async fn commit_new_structure(
    pass: &ConsolidationPass<'_>,
    drafts: Vec<NewSectionDraft>,
    bulk_reject_stale: bool,
    pass_name: &str,
) -> Result<PersonalMemoryRecord> {
    if bulk_reject_stale {
        let rejected =
            reject_all_pending_revisions(pass.conn, pass.current_record.project_id.as_deref())
                .await?;
        if rejected > 0 {
            log::info!(
                "[Memory::Personal::{}] Bulk-rejected {} pending revision(s) superseded by new IDs.",
                pass_name,
                rejected
            );
        }
    }

    let mut memory = PersonalMemory::from_new_sections(drafts);
    memory.prune_empty_sections();
    memory
        .validate()
        .map_err(|e| anyhow!("{} produced an invalid memory model: {}", pass_name, e))?;

    let json = memory.to_json()?;
    let saved = save_consolidated_memory(
        pass.conn,
        pass.project_id,
        &json,
        pass.current_record.version,
    )
    .await?;
    log::info!(
        "[Memory::Personal::{}] Saved v{} with {} section(s) and {} block(s).",
        pass_name,
        saved.version,
        memory.sections.len(),
        memory
            .sections
            .iter()
            .map(|s| s.blocks.len())
            .sum::<usize>()
    );
    Ok(saved)
}

/// Commits all resolved operations under the `auto_apply` policy directly into a new version.
async fn auto_apply_operations(
    pass: &ConsolidationPass<'_>,
    resolved: Vec<ResolvedOp>,
) -> Result<PassResult> {
    let memory = PersonalMemory::from_json(&pass.current_record.content)?;
    let report: ApplyReport = apply_operations(&memory, &resolved)?;
    log_operation_rejections("auto-apply", &report.rejected);

    let mut record = pass.current_record.clone();
    let applied = resolved.len().saturating_sub(report.rejected.len());
    if applied > 0 {
        record = commit_applied_memory(pass, &report.memory).await?;
        log::info!(
            "[Memory::Personal::AutoApply] Committed {} operation(s) into v{}.",
            applied,
            record.version
        );
    }

    Ok(PassResult {
        outcome: ConsolidateOutcome::completed(record),
        landed: applied > 0,
    })
}

/// Validates and persists the result of an auto-apply batch as the next active version.
async fn commit_applied_memory(
    pass: &ConsolidationPass<'_>,
    memory: &PersonalMemory,
) -> Result<PersonalMemoryRecord> {
    memory.validate().map_err(|e| {
        anyhow!(
            "auto_apply produced an invalid memory model, nothing committed: {}",
            e
        )
    })?;
    let json = memory.to_json()?;
    save_consolidated_memory(
        pass.conn,
        pass.project_id,
        &json,
        pass.current_record.version,
    )
    .await
}

#[derive(Debug, Deserialize)]
struct WholeMemoryOutput {
    sections: Vec<WholeMemorySectionDraft>,
}

#[derive(Debug, Deserialize)]
struct WholeMemorySectionDraft {
    title: String,
    blocks: Vec<String>,
}

/// Issues one whole-memory structured LLM pass (cold generation or regeneration).
async fn run_whole_memory_pass(
    pass: &ConsolidationPass<'_>,
    system_prompt: &str,
    user_content: &str,
    pass_name: &str,
) -> Result<Vec<NewSectionDraft>> {
    let raw = execute_personal_llm_pass(
        pass.llm_provider,
        system_prompt,
        user_content,
        pass.llm_settings,
        whole_memory_json_schema(),
    )
    .await?;
    let output: WholeMemoryOutput = serde_json::from_str(extract_json_payload(&raw)).map_err(|e| {
        anyhow!(
            "Failed to parse {} JSON output: {} (raw: {})",
            pass_name,
            e,
            raw
        )
    })?;
    Ok(output
        .sections
        .into_iter()
        .filter(|s| !s.title.trim().is_empty())
        .map(|s| NewSectionDraft {
            title: s.title.trim().to_string(),
            blocks: s.blocks,
        })
        .collect())
}

/// Issues one delta structured LLM pass (incremental integration or comment edit).
async fn run_structured_pass(
    pass: &ConsolidationPass<'_>,
    system_prompt: &str,
    user_content: &str,
    pass_name: &str,
) -> Result<ConsolidationOutput> {
    let raw = execute_personal_llm_pass(
        pass.llm_provider,
        system_prompt,
        user_content,
        pass.llm_settings,
        delta_consolidation_json_schema(),
    )
    .await?;
    serde_json::from_str(extract_json_payload(&raw)).map_err(|e| {
        anyhow!(
            "Failed to parse {} JSON output: {} (raw: {})",
            pass_name,
            e,
            raw
        )
    })
}

/// Renders candidate observations as a bullet list for the LLM prompt.
fn render_observation_bullets(observations: &[ObservationRecord]) -> String {
    observations
        .iter()
        .map(|observation| format!("- {}", observation.text.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders free-text user directives as a bullet list for the LLM prompt.
fn render_comment_bullets(comments: &[String]) -> String {
    comments
        .iter()
        .map(|comment| {
            let trimmed = comment.trim();
            if trimmed.starts_with("- ") {
                trimmed.to_string()
            } else {
                format!("- {}", trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parses the stored canonical JSON and returns its sections, or an empty vector when the payload is
/// malformed.
fn stored_sections(record: &PersonalMemoryRecord) -> Vec<MemorySection> {
    match PersonalMemory::from_json(&record.content) {
        Ok(memory) => memory.sections,
        Err(e) => {
            log::warn!(
                "[Memory::Personal] Stored content is not valid semantic JSON ({}); treating as empty.",
                e
            );
            Vec::new()
        }
    }
}

/// Logs per-operation rejections with their attribution, never escalating the batch.
fn log_operation_rejections(pass_name: &str, rejected: &[RejectedOperation]) {
    for rejection in rejected {
        log::warn!(
            "[Memory::Personal::{}] refused operation at position {} ({}): {}",
            pass_name,
            rejection.position,
            rejection.operation.op_name(),
            rejection.reason
        );
    }
}
