use std::{
    path::Path,
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
        harness::buffer::{ChatMessage, Role},
        llm::{
            ConversationInput, GenerationPolicy, GenerationPurpose, LlmProvider, LlmStreamEvent,
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
3. Organize into clear Markdown headings .
4. Output ONLY the raw markdown text of the document. Do not wrap in markdown code blocks or add introductory text.
</rules>"#;

const COMMENT_REGENERATION_SYSTEM_PROMPT: &str = r#"<role>
You are a personal memory editing engine for an AI assistant.
Your task is to update and reorganize the user's Personal Memory markdown document according to user directive comments.
</role>

<rules>
1. Faithfully apply the user's requested modifications, additions, and removals.
2. Ensure the resulting document remains clean, well-structured, and concise Markdown.
3. Output ONLY the raw markdown text of the document. Do not wrap in markdown code blocks or add introductory text.
</rules>"#;

/// Consolidates accumulated personal facts or applies user directive comments into the personal memory document.
pub async fn consolidate_personal_memory(
    conn: &Connection,
    llm_provider: &dyn LlmProvider,
    comments: Option<Vec<String>>,
    project_id: Option<&str>,
) -> Result<PersonalMemoryRecord> {
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

/// Exports the personal memory markdown document to disk.
pub async fn export_personal_memory(
    conn: &Connection,
    target_path: &Path,
    project_id: Option<&str>,
) -> Result<()> {
    let record = get_personal_memory(conn, project_id).await?;
    std::fs::write(target_path, record.content).map_err(|e| {
        anyhow!(
            "Failed to export personal memory to {:?}: {}",
            target_path,
            e
        )
    })?;
    Ok(())
}

/// Imports an external markdown document to replace the active personal memory.
pub async fn import_personal_memory(
    conn: &Connection,
    source_path: &Path,
    project_id: Option<&str>,
) -> Result<PersonalMemoryRecord> {
    let imported_text = std::fs::read_to_string(source_path).map_err(|e| {
        anyhow!(
            "Failed to read personal memory from {:?}: {}",
            source_path,
            e
        )
    })?;
    let current = get_personal_memory(conn, project_id).await?;
    save_personal_memory(conn, project_id, &imported_text, current.version).await
}

/// Regenerates the document based on directive comments from the user.
async fn regenerate_with_comments(
    conn: &Connection,
    llm_provider: &dyn LlmProvider,
    current_record: &PersonalMemoryRecord,
    comments: &[String],
    project_id: Option<&str>,
) -> Result<PersonalMemoryRecord> {
    let user_content = format!(
        "<current_personal_memory>\n{}\n</current_personal_memory>\n\n\
         <user_directive_comments>\n{}\n</user_directive_comments>\n\n\
         Please update the document following the directives and output the updated Markdown.",
        current_record.content,
        comments.join("\n- ")
    );

    let updated_markdown = execute_personal_llm_pass(
        llm_provider,
        COMMENT_REGENERATION_SYSTEM_PROMPT,
        &user_content,
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
) -> Result<String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let policy = GenerationPolicy::from_settings(&LlmSettings::default(), Some(4096));
    let request = policy.build_request(
        GenerationPurpose::MemoryCompaction,
        ConversationInput {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: system_prompt.to_string(),
                    timestamp_ms: now_ms,
                },
                ChatMessage {
                    role: Role::User,
                    content: user_content.to_string(),
                    timestamp_ms: now_ms,
                },
            ],
        },
    );

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
