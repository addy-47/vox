use std::{
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use turso::Connection;

pub use crate::persistence::personal_memory::{
    fetch_pending_suggestions, get_personal_memory, insert_personal_memory_suggestions,
    list_personal_memory_versions, resolve_suggestions_transaction, save_consolidated_memory,
    save_personal_memory, set_active_personal_memory_version, MemorySuggestionRecord,
    PersonalMemoryRecord, PersonalMemorySuggestionRecord,
};
use crate::{
    core::settings::LlmSettings,
    persistence::{
        facts::{fetch_active_facts_by_type, mark_facts_consolidated, mark_facts_staged},
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

const PERSONAL_CONSOLIDATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory consolidation engine for an AI assistant.
Your task is to analyze newly discovered personal facts about the user and propose atomic delta patch operations to update their Personal Memory markdown document.
</role>

<rules>
1. Output strictly a JSON object matching this schema:
   {
     "operations": [
       {
         "op": "replace" | "insert" | "delete",
         "section": "## Section Heading",
         "target_text": "Exact text to replace or delete (null for insert)",
         "proposed_text": "New text to insert or replace (empty for delete)",
         "source_fact_ids": ["id1", "id2"]
       }
     ]
   }
2. Operations:
   - "insert": Adds new facts under the appropriate section heading (e.g. "## Personal Information", "## Preferences", "## Technical Projects"). If the section does not exist, name it clearly.
   - "replace": Updates outdated, changed, or refined facts. "target_text" must contain the existing text from the document to be replaced.
   - "delete": Removes deprecated or contradicted facts.
3. Every operation MUST include the valid "source_fact_ids" of the facts that motivated the change.
4. If a new fact is already fully captured in the document, do NOT emit an operation for it.
5. NEVER emit line numbers. NEVER rewrite sections that are not changing.
6. Output ONLY the raw JSON object. Do not enclose in markdown code fences or triple backticks.
</rules>"###;

const COMMENT_REGENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory editing engine for an AI assistant.
Your task is to analyze user directive comments on specific lines/quotes of their Personal Memory markdown document and propose atomic delta patch operations.
</role>

<rules>
1. Output strictly a JSON object matching this schema:
   {
     "operations": [
       {
         "op": "replace" | "insert" | "delete",
         "section": "## Section Heading",
         "target_text": "Exact text to replace or delete (null for insert)",
         "proposed_text": "New text to insert or replace (empty for delete)",
         "source_fact_ids": []
       }
     ]
   }
2. Faithfully apply each directive comment:
   - "replace": For comments asking to modify, update, or reword a specific line.
   - "insert": For comments asking to add new information under a section.
   - "delete": For comments asking to remove a specific line.
3. "source_fact_ids" must be an empty array [].
4. Output ONLY the raw JSON object. Do not enclose in markdown code fences or triple backticks.
</rules>"###;

/// An individual atomic patch operation proposed by the consolidation engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPatchOperation {
    pub op: String,
    pub section: String,
    #[serde(default)]
    pub target_text: Option<String>,
    #[serde(default)]
    pub proposed_text: String,
    #[serde(default)]
    pub source_fact_ids: Vec<String>,
}

/// JSON payload structure emitted by the consolidation LLM pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalConsolidationOutput {
    pub operations: Vec<MemoryPatchOperation>,
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

    log::info!(
        "[Memory::Personal] Analyzing {} active personal facts for delta suggestions (v{})...",
        active_facts.len(),
        current_record.version
    );

    let mut facts_text = String::new();
    for fact in &active_facts {
        facts_text.push_str(&format!("- [id: {}] {}\n", fact.id, fact.text));
    }

    let user_content = format!(
        "<current_personal_memory>\n{}\n</current_personal_memory>\n\n\
         <new_personal_facts>\n{}\n</new_personal_facts>\n\n\
         Propose atomic delta patch operations to integrate the new facts into the document. Output raw JSON object with 'operations'.",
        current_record.content, facts_text
    );

    let raw_json = execute_personal_llm_pass(
        llm_provider,
        PERSONAL_CONSOLIDATION_SYSTEM_PROMPT,
        &user_content,
        effective_settings,
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

    let valid_fact_ids: std::collections::HashSet<String> =
        active_facts.iter().map(|f| f.id.clone()).collect();
    let all_candidate_fact_ids: Vec<String> = active_facts.into_iter().map(|f| f.id).collect();

    let suggestions: Vec<PersonalMemorySuggestionRecord> = parsed_output
        .operations
        .into_iter()
        .filter(|op| {
            let valid_op = op.op == "insert" || op.op == "replace" || op.op == "delete";
            if !valid_op {
                log::warn!("[Memory::Personal] Skipping invalid patch op '{}'", op.op);
            }
            valid_op
        })
        .map(|op| {
            let sug_id = format!("sug_{}_{}", now, &uuid::Uuid::new_v4().to_string()[..8]);
            let filtered_fact_ids: Vec<String> = op
                .source_fact_ids
                .into_iter()
                .filter(|id| valid_fact_ids.contains(id))
                .collect();
            PersonalMemorySuggestionRecord {
                id: sug_id,
                base_memory_version: current_record.version,
                project_id: current_record.project_id.clone(),
                op: op.op,
                section: op.section,
                target_text: op.target_text,
                proposed_text: op.proposed_text,
                source_fact_ids: filtered_fact_ids,
                status: "pending".to_string(),
                created_at: now,
                resolved_at: None,
            }
        })
        .collect();

    let mut staged_fact_ids = std::collections::HashSet::new();
    for sug in &suggestions {
        for fid in &sug.source_fact_ids {
            staged_fact_ids.insert(fid.clone());
        }
    }
    let staged_vec: Vec<String> = staged_fact_ids.into_iter().collect();
    let unselected_vec: Vec<String> = all_candidate_fact_ids
        .into_iter()
        .filter(|id| !staged_vec.contains(id))
        .collect();

    if !suggestions.is_empty() {
        insert_personal_memory_suggestions(conn, &suggestions).await?;
    }
    if !staged_vec.is_empty() {
        mark_facts_staged(conn, &staged_vec).await?;
    }
    if !unselected_vec.is_empty() {
        mark_facts_consolidated(conn, &unselected_vec).await?;
    }
    log::info!(
        "[Memory::Personal] Staged {} suggestion(s) covering {} fact(s); {} unselected fact(s) marked 'consolidated'",
        suggestions.len(),
        staged_vec.len(),
        unselected_vec.len()
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
         Propose atomic delta patch operations to apply the user comments. Output raw JSON object with 'operations'.",
        current_record.content, comments_list
    );

    let raw_json = execute_personal_llm_pass(
        llm_provider,
        COMMENT_REGENERATION_SYSTEM_PROMPT,
        &user_content,
        settings,
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
        .operations
        .into_iter()
        .filter(|op| op.op == "insert" || op.op == "replace" || op.op == "delete")
        .map(|op| {
            let sug_id = format!("sug_{}_{}", now, &uuid::Uuid::new_v4().to_string()[..8]);
            PersonalMemorySuggestionRecord {
                id: sug_id,
                base_memory_version: current_record.version,
                project_id: current_record.project_id.clone(),
                op: op.op,
                section: op.section,
                target_text: op.target_text,
                proposed_text: op.proposed_text,
                source_fact_ids: Vec::new(),
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

/// Applies atomic memory patch operations to base markdown, returning the updated document.
pub fn apply_patch_operations(
    base_markdown: &str,
    operations: &[MemoryPatchOperation],
) -> Result<String> {
    if operations.is_empty() {
        return Ok(base_markdown.to_string());
    }

    let mut doc_lines: Vec<String> = base_markdown.lines().map(|s| s.to_string()).collect();

    for op in operations {
        let op_type = op.op.trim().to_lowercase();
        match op_type.as_str() {
            "insert" => {
                apply_insert_op(&mut doc_lines, op);
            }
            "replace" => {
                apply_replace_op(&mut doc_lines, op);
            }
            "delete" => {
                apply_delete_op(&mut doc_lines, op);
            }
            unrecognized => {
                log::warn!(
                    "[Memory::Personal::Patch] Unknown patch operation '{}', skipping",
                    unrecognized
                );
            }
        }
    }

    let mut result = doc_lines.join("\n");
    if (base_markdown.ends_with('\n') || !result.is_empty()) && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

fn normalize_heading(s: &str) -> &str {
    s.trim_start_matches('#').trim()
}

fn is_heading(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') && trimmed.chars().take_while(|c| *c == '#').count() <= 6
}

fn heading_level(line: &str) -> usize {
    line.trim_start().chars().take_while(|c| *c == '#').count()
}

fn find_section_range(lines: &[String], section_name: &str) -> Option<(usize, usize)> {
    let target_norm = normalize_heading(section_name);
    if target_norm.is_empty() {
        return None;
    }

    let mut start_idx = None;
    let mut sec_level = 2;

    for (idx, line) in lines.iter().enumerate() {
        if is_heading(line) {
            let line_norm = normalize_heading(line);
            if line_norm.eq_ignore_ascii_case(target_norm) {
                start_idx = Some(idx);
                sec_level = heading_level(line);
                break;
            }
        }
    }

    let start = start_idx?;
    let mut end = lines.len();
    for (idx, line) in lines.iter().enumerate().skip(start + 1) {
        if is_heading(line) && heading_level(line) <= sec_level {
            end = idx;
            break;
        }
    }

    Some((start, end))
}

fn apply_insert_op(lines: &mut Vec<String>, op: &MemoryPatchOperation) {
    let proposed = op.proposed_text.trim();
    if proposed.is_empty() {
        return;
    }

    let bullet = if proposed.starts_with("- ") || proposed.starts_with("* ") {
        proposed.to_string()
    } else {
        format!("- {}", proposed)
    };

    if let Some((start, end)) = find_section_range(lines, &op.section) {
        let mut insert_pos = end;
        while insert_pos > start + 1 && lines[insert_pos - 1].trim().is_empty() {
            insert_pos -= 1;
        }
        lines.insert(insert_pos, bullet);
    } else {
        let heading_title = normalize_heading(&op.section);
        let heading_line = if op.section.trim_start().starts_with('#') {
            op.section.trim().to_string()
        } else {
            format!("## {}", heading_title)
        };

        if !lines.is_empty() && !lines.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
            lines.push(String::new());
        }
        lines.push(heading_line);
        lines.push(bullet);
    }
}

fn normalize_for_match(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn strip_bullet(s: &str) -> &str {
    if let Some(stripped) = s.strip_prefix("- ") {
        stripped.trim_start()
    } else if let Some(stripped) = s.strip_prefix("* ") {
        stripped.trim_start()
    } else {
        s
    }
}

fn apply_replace_op(lines: &mut [String], op: &MemoryPatchOperation) {
    let target = match op.target_text.as_deref() {
        Some(t) if !t.trim().is_empty() => t.trim(),
        _ => {
            log::warn!("[Memory::Personal::Patch] Replace op missing target_text, skipping");
            return;
        }
    };
    let proposed = op.proposed_text.trim();
    if proposed.is_empty() {
        log::warn!("[Memory::Personal::Patch] Replace op has empty proposed_text, skipping");
        return;
    }

    let (search_start, search_end) =
        find_section_range(lines, &op.section).unwrap_or((0, lines.len()));

    let target_norm = normalize_for_match(target);

    // 1. Exact match within target section
    for line in lines[search_start..search_end].iter_mut() {
        if line.contains(target) {
            let clean_proposed = if (line.trim_start().starts_with("- ")
                || line.trim_start().starts_with("* "))
                && (!target.trim_start().starts_with("- ")
                    && !target.trim_start().starts_with("* "))
            {
                strip_bullet(proposed)
            } else {
                proposed
            };
            *line = line.replace(target, clean_proposed);
            return;
        }
    }

    // 2. Normalized match within target section
    for line in lines[search_start..search_end].iter_mut() {
        let line_norm = normalize_for_match(line);
        if line_norm.contains(&target_norm) {
            let is_bullet = line.trim_start().starts_with("- ")
                || line.trim_start().starts_with("* ");
            if is_bullet {
                *line = format!("- {}", strip_bullet(proposed));
            } else {
                *line = proposed.to_string();
            }
            return;
        }
    }

    // 3. Fallback: exact match anywhere in document
    for line in lines.iter_mut() {
        if line.contains(target) {
            let clean_proposed = if (line.trim_start().starts_with("- ")
                || line.trim_start().starts_with("* "))
                && (!target.trim_start().starts_with("- ")
                    && !target.trim_start().starts_with("* "))
            {
                strip_bullet(proposed)
            } else {
                proposed
            };
            *line = line.replace(target, clean_proposed);
            return;
        }
    }

    // 4. Fallback: normalized match anywhere in document
    for line in lines.iter_mut() {
        if normalize_for_match(line).contains(&target_norm) {
            let is_bullet =
                line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ");
            if is_bullet {
                *line = format!("- {}", strip_bullet(proposed));
            } else {
                *line = proposed.to_string();
            }
            return;
        }
    }

    log::warn!(
        "[Memory::Personal::Patch] Replace target '{}' not found in section '{}' or document; skipped",
        target,
        op.section
    );
}

fn apply_delete_op(lines: &mut Vec<String>, op: &MemoryPatchOperation) {
    let target = match op.target_text.as_deref() {
        Some(t) if !t.trim().is_empty() => t.trim(),
        _ => {
            log::warn!("[Memory::Personal::Patch] Delete op missing target_text, skipping");
            return;
        }
    };

    let (search_start, search_end) =
        find_section_range(lines, &op.section).unwrap_or((0, lines.len()));

    let target_norm = normalize_for_match(target);

    for idx in search_start..search_end {
        if lines[idx].contains(target) {
            lines.remove(idx);
            return;
        }
    }

    for idx in search_start..search_end {
        if normalize_for_match(&lines[idx]).contains(&target_norm) {
            lines.remove(idx);
            return;
        }
    }

    for idx in 0..lines.len() {
        if lines[idx].contains(target) || normalize_for_match(&lines[idx]).contains(&target_norm) {
            lines.remove(idx);
            return;
        }
    }

    log::warn!(
        "[Memory::Personal::Patch] Delete target '{}' not found in section '{}' or document; skipped",
        target,
        op.section
    );
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
async fn execute_personal_llm_pass(
    provider: &dyn LlmProvider,
    system_prompt: &str,
    user_content: &str,
    settings: &LlmSettings,
) -> Result<String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let policy = GenerationPolicy::from_settings(settings, Some(4096));
    let mut request = policy.build_request(
        GenerationPurpose::MemoryCompaction,
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

    request.output = OutputConstraint::JsonObject;
    request.options.reasoning = ReasoningMode::Disabled;
    request.options.max_output_tokens = Some(4096);
    request.options.temperature = Some(0.2);
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
    fn test_patch_insert_into_existing_section() {
        let ops = vec![MemoryPatchOperation {
            op: "insert".to_string(),
            section: "## Personal Information".to_string(),
            target_text: None,
            proposed_text: "User enjoys playing badminton.".to_string(),
            source_fact_ids: vec!["fact_1".to_string()],
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.contains("- User enjoys playing badminton."));
        assert!(result.contains("- User lives in Chicago."));
        assert!(result.contains("- Building a voice orchestrator in Rust."));
    }

    #[test]
    fn test_patch_insert_into_new_section() {
        let ops = vec![MemoryPatchOperation {
            op: "insert".to_string(),
            section: "## Hobbies".to_string(),
            target_text: None,
            proposed_text: "Enjoys stargazing.".to_string(),
            source_fact_ids: vec!["fact_2".to_string()],
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.contains("## Hobbies"));
        assert!(result.contains("- Enjoys stargazing."));
    }

    #[test]
    fn test_patch_replace_exact() {
        let ops = vec![MemoryPatchOperation {
            op: "replace".to_string(),
            section: "## Personal Information".to_string(),
            target_text: Some("User lives in Chicago.".to_string()),
            proposed_text: "User lives in Austin.".to_string(),
            source_fact_ids: vec!["fact_3".to_string()],
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.contains("User lives in Austin."));
        assert!(!result.contains("User lives in Chicago."));
        // Anchors preserved
        assert!(result.contains("- User speaks English and Hindi."));
    }

    #[test]
    fn test_patch_replace_normalized_fallback() {
        let ops = vec![MemoryPatchOperation {
            op: "replace".to_string(),
            section: "Personal Information".to_string(), // heading without ##
            target_text: Some("user lives in chicago".to_string()), // lower case, no period
            proposed_text: "User relocated to Seattle.".to_string(),
            source_fact_ids: vec!["fact_4".to_string()],
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.contains("User relocated to Seattle."));
        assert!(!result.contains("User lives in Chicago."));
    }

    #[test]
    fn test_patch_delete() {
        let ops = vec![MemoryPatchOperation {
            op: "delete".to_string(),
            section: "## Technical Projects".to_string(),
            target_text: Some("Works with Turso embedded database.".to_string()),
            proposed_text: String::new(),
            source_fact_ids: vec!["fact_5".to_string()],
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(!result.contains("Works with Turso embedded database."));
        assert!(result.contains("- Building a voice orchestrator in Rust."));
    }

    #[test]
    fn test_patch_missing_target_is_skipped_resiliently() {
        let ops = vec![MemoryPatchOperation {
            op: "replace".to_string(),
            section: "## Personal Information".to_string(),
            target_text: Some("Non-existent fact about flying cars.".to_string()),
            proposed_text: "Replacement for phantom.".to_string(),
            source_fact_ids: vec!["fact_6".to_string()],
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch should not error");
        assert_eq!(result.trim(), SAMPLE_DOC.trim());
    }

    #[test]
    fn test_patch_replace_avoids_double_bullet() {
        let ops = vec![MemoryPatchOperation {
            op: "replace".to_string(),
            section: "## Personal Information".to_string(),
            target_text: Some("User lives in Chicago.".to_string()),
            proposed_text: "- User lives in Austin.".to_string(),
            source_fact_ids: vec!["fact_7".to_string()],
        }];

        let result = apply_patch_operations(SAMPLE_DOC, &ops).expect("Patch failed");
        assert!(result.contains("- User lives in Austin."));
        assert!(!result.contains("- - User lives in Austin."));
    }

    #[test]
    fn test_extract_json_payload_with_fences() {
        let fenced = "```json\n{\"operations\": []}\n```";
        assert_eq!(extract_json_payload(fenced), "{\"operations\": []}");

        let raw = "{\"operations\": []}";
        assert_eq!(extract_json_payload(raw), "{\"operations\": []}");
    }
}
