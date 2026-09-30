use anyhow::{anyhow, Result};
use turso::Connection;

use super::model::PersonalMemory;
use crate::persistence::personal_memory::{
    get_personal_memory, save_personal_memory, PersonalMemoryRecord,
};

/// Saves a user-authored Markdown document as the next active Personal Memory version.
pub async fn save_personal_memory_from_markdown(
    conn: &Connection,
    project_id: Option<&str>,
    markdown: &str,
    expected_version: i64,
) -> Result<PersonalMemoryRecord> {
    let current = get_personal_memory(conn, project_id).await?;
    let memory = PersonalMemory::from_markdown(markdown);

    reject_headingless_wipe(&current, markdown, &memory)?;
    if !memory.sections.is_empty() {
        memory
            .validate()
            .map_err(|e| anyhow!("Markdown document is not a valid personal memory: {}", e))?;
    }

    let json = memory.to_json()?;
    let saved = save_personal_memory(conn, project_id, &json, expected_version).await?;
    reject_superseded_revisions(conn, &current).await?;

    log::info!(
        "[Memory::Manual] Saved v{} from Markdown: {} section(s), {} block(s).",
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

/// Refuses a non-empty Markdown document that parsed to nothing, rather than committing an empty
/// Personal Memory over a populated one.
fn reject_headingless_wipe(
    current: &PersonalMemoryRecord,
    markdown: &str,
    parsed: &PersonalMemory,
) -> Result<()> {
    if !parsed.sections.is_empty() || markdown.trim().is_empty() {
        return Ok(());
    }
    let existing = PersonalMemory::from_json(&current.content)
        .map(|memory| memory.sections.len())
        .unwrap_or(0);
    if existing == 0 {
        return Ok(());
    }
    Err(anyhow!(
        "Markdown document contains no '## Section' headings, so saving it would erase all {} existing \
         section(s). Add section headings, or save an empty document to clear Personal Memory explicitly.",
        existing
    ))
}

/// Bulk-rejects pending revisions for the scope, because a manual save re-mints every persistent ID
/// and leaves each staged revision targeting an entity that no longer exists.
async fn reject_superseded_revisions(
    conn: &Connection,
    current: &PersonalMemoryRecord,
) -> Result<()> {
    let rejected = crate::persistence::personal_memory::reject_all_pending_revisions(
        conn,
        current.project_id.as_deref(),
    )
    .await?;
    if rejected > 0 {
        log::info!(
            "[Memory::Manual] Bulk-rejected {} pending revision(s) superseded by re-minted IDs.",
            rejected
        );
    }
    Ok(())
}
