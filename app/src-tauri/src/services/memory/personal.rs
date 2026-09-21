use std::{
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use tokio_util::sync::CancellationToken;
use turso::Connection;

use crate::{
    core::settings::LlmSettings,
    persistence::{
        facts::{fetch_active_facts_by_type, mark_facts_consolidated},
        has_in_progress_compaction, has_unfinished_items,
        personal_memory::{
            get_personal_memory, save_consolidated_memory, save_personal_memory,
            PersonalMemoryRecord,
        },
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

const PERSONAL_CONSOLIDATION_SYSTEM_PROMPT: &str = r#"<role>
You are a personal memory consolidation engine for an AI assistant.
Your task is to integrate newly discovered personal facts about the user into their existing Personal Memory markdown document.
</role>

<rules>
1. Preserve all existing accurate information while cleanly integrating new facts.
2. Remove contradictions and supersede outdated facts with newer information.
3. Organize into clear Markdown headings using ## for major sections and bullet points for lists.
4. Output ONLY the raw markdown text of the document. Do not wrap in markdown code blocks or add introductory text.
</rules>"#;

const COMMENT_REGENERATION_SYSTEM_PROMPT: &str = r#"<role>
You are a personal memory editing engine for an AI assistant.
Your task is to update and reorganize the user's Personal Memory markdown document according to user comments anchored to specific lines.
</role>

<rules>
1. Faithfully apply each comment to its referenced line, quote, or section.
2. Preserve all existing text, facts, and headings that are not targeted by comments.
3. Organize into clean Markdown headings using ## for major sections and bullet points for lists.
4. Output ONLY the raw markdown text. Start directly with the first heading or bullet. Never enclose the response in markdown code blocks or triple backticks.
</rules>"#;

/// Consolidates accumulated personal facts or applies user directive comments into the personal memory document.
pub async fn consolidate_personal_memory(
    conn: &Connection,
    llm_provider: &dyn LlmProvider,
    comments: Option<Vec<String>>,
    project_id: Option<&str>,
    settings: Option<&LlmSettings>,
) -> Result<PersonalMemoryRecord> {
    let fallback_settings = LlmSettings::default();
    let effective_settings = settings.unwrap_or(&fallback_settings);

    let current_record = get_personal_memory(conn, project_id).await?;

    if let Some(user_comments) = comments {
        if user_comments.is_empty() {
            return Ok(current_record);
        }
        verify_ingestion_quiescence(conn).await?;
        return regenerate_with_comments(
            conn,
            llm_provider,
            &current_record,
            &user_comments,
            project_id,
            effective_settings,
        )
        .await;
    }

    verify_ingestion_quiescence(conn).await?;

    let active_facts = fetch_active_facts_by_type(conn, "personal").await?;
    if active_facts.is_empty() {
        log::info!("[Memory::Personal] No active personal facts to consolidate.");
        return Ok(current_record);
    }

    log::info!(
        "[Memory::Personal] Consolidating {} active personal facts into personal memory...",
        active_facts.len()
    );

    let mut facts_text = String::new();
    for fact in &active_facts {
        facts_text.push_str(&format!("- {}\n", fact.text));
    }

    let user_content = format!(
        "<current_personal_memory>\n{}\n</current_personal_memory>\n\n\
         <new_personal_facts>\n{}\n</new_personal_facts>\n\n\
         Please consolidate the new facts into the document and output the updated Markdown.",
        current_record.content, facts_text
    );

    let updated_markdown = execute_personal_llm_pass(
        llm_provider,
        PERSONAL_CONSOLIDATION_SYSTEM_PROMPT,
        &user_content,
        effective_settings,
    )
    .await?;

    let saved =
        save_consolidated_memory(conn, project_id, &updated_markdown, current_record.version)
            .await?;

    let fact_ids: Vec<String> = active_facts.into_iter().map(|f| f.id).collect();
    mark_facts_consolidated(conn, &fact_ids).await?;

    log::info!(
        "[Memory::Personal] Personal memory consolidated (v{}). Marked {} facts as consolidated.",
        saved.version,
        fact_ids.len()
    );

    Ok(saved)
}

/// Regenerates the document based on directive comments from the user.
async fn regenerate_with_comments(
    conn: &Connection,
    llm_provider: &dyn LlmProvider,
    current_record: &PersonalMemoryRecord,
    comments: &[String],
    project_id: Option<&str>,
    settings: &LlmSettings,
) -> Result<PersonalMemoryRecord> {
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
         Please update the document following the comments and output the updated Markdown.",
        current_record.content, comments_list
    );

    let updated_markdown = execute_personal_llm_pass(
        llm_provider,
        COMMENT_REGENERATION_SYSTEM_PROMPT,
        &user_content,
        settings,
    )
    .await?;

    save_personal_memory(conn, project_id, &updated_markdown, current_record.version).await
}

/// Checks that no compaction or pending ingestion queue items are currently executing.
async fn verify_ingestion_quiescence(conn: &Connection) -> Result<()> {
    if has_in_progress_compaction(conn).await? {
        return Err(anyhow!(
            "Precondition failed: active compaction is in progress; personal consolidation deferred"
        ));
    }

    if has_unfinished_items(conn).await? {
        return Err(anyhow!(
            "Precondition failed: pending items in memory ingestion queue; personal consolidation deferred"
        ));
    }

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

    request.output = OutputConstraint::Text;
    request.options.reasoning = ReasoningMode::Enabled;
    request.options.max_output_tokens = Some(4096);
    request.options.temperature = Some(settings.compaction_temperature);
    request.options.context_window = Some(settings.context_window);

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
        }
        Ok(Err(e)) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!("[PersonalMemory] Token pump task join error: {}", join_err);
            }
            return Err(anyhow!("LLM generation error: {}", e));
        }
        Err(_) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!("[PersonalMemory] Token pump task join error: {}", join_err);
            }
            return Err(anyhow!(
                "LLM personal memory consolidation timed out after 45s"
            ));
        }
    }

    let cleaned = output.trim();
    if cleaned.is_empty() {
        return Err(anyhow!("LLM generated empty personal memory document"));
    }

    Ok(cleaned.to_string())
}
