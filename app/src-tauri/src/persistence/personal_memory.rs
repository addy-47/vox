use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

/// Strongly-typed row representation of an evolving personal memory document.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PersonalMemoryRecord {
    pub id: i64,
    pub project_id: Option<String>,
    pub content: String,
    pub version: i64,
    pub last_consolidated_at: i64,
    pub updated_at: i64,
}

/// Retrieves the personal memory document for global scope (`project_id: None`) or a specific project.
/// If no record exists for a project, inserts a default blank record.
pub async fn get_personal_memory(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<PersonalMemoryRecord> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let maybe_record = if let Some(pid) = project_id {
        let mut rows = conn
            .query(
                "SELECT id, project_id, content, version, last_consolidated_at, updated_at
                 FROM personal_memory
                 WHERE project_id = ?",
                (pid.to_string(),),
            )
            .await?;
        parse_record(&mut rows).await?
    } else {
        let mut rows = conn
            .query(
                "SELECT id, project_id, content, version, last_consolidated_at, updated_at
                 FROM personal_memory
                 WHERE project_id IS NULL",
                (),
            )
            .await?;
        parse_record(&mut rows).await?
    };

    if let Some(record) = maybe_record {
        return Ok(record);
    }

    // Insert default blank record if missing
    let (pid_str, proj_arg) = if let Some(pid) = project_id {
        (Some(pid.to_string()), Some(pid.to_string()))
    } else {
        (None, None)
    };

    conn.execute(
        "INSERT INTO personal_memory (project_id, content, version, last_consolidated_at, updated_at)
         VALUES (?, '', 1, ?, ?)",
        (proj_arg, now, now),
    )
    .await?;

    let mut id_rows = conn.query("SELECT last_insert_rowid()", ()).await?;
    let inserted_id = if let Some(row) = id_rows.next().await? {
        row.get(0)?
    } else {
        1
    };

    Ok(PersonalMemoryRecord {
        id: inserted_id,
        project_id: pid_str,
        content: String::new(),
        version: 1,
        last_consolidated_at: now,
        updated_at: now,
    })
}

async fn parse_record(rows: &mut turso::Rows) -> Result<Option<PersonalMemoryRecord>> {
    if let Some(row) = rows.next().await? {
        Ok(Some(PersonalMemoryRecord {
            id: row.get(0)?,
            project_id: row.get(1).ok(),
            content: row.get(2)?,
            version: row.get(3)?,
            last_consolidated_at: row.get(4)?,
            updated_at: row.get(5)?,
        }))
    } else {
        Ok(None)
    }
}

/// Saves updated content with optimistic concurrency control against `expected_version`.
pub async fn save_personal_memory(
    conn: &Connection,
    project_id: Option<&str>,
    content: &str,
    expected_version: i64,
) -> Result<PersonalMemoryRecord> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let affected = if let Some(pid) = project_id {
        conn.execute(
            "UPDATE personal_memory
             SET content = ?, version = version + 1, updated_at = ?
             WHERE project_id = ? AND version = ?",
            (content.to_string(), now, pid.to_string(), expected_version),
        )
        .await?
    } else {
        conn.execute(
            "UPDATE personal_memory
             SET content = ?, version = version + 1, updated_at = ?
             WHERE project_id IS NULL AND version = ?",
            (content.to_string(), now, expected_version),
        )
        .await?
    };

    if affected == 0 {
        return Err(anyhow!(
            "Optimistic version conflict for personal memory: expected version {}",
            expected_version
        ));
    }

    get_personal_memory(conn, project_id).await
}

/// Saves consolidated content with optimistic concurrency control, stamping `last_consolidated_at`.
/// Used by the background consolidation path so schedule bookkeeping reflects actual merges.
pub async fn save_consolidated_memory(
    conn: &Connection,
    project_id: Option<&str>,
    content: &str,
    expected_version: i64,
) -> Result<PersonalMemoryRecord> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let affected = if let Some(pid) = project_id {
        conn.execute(
            "UPDATE personal_memory
             SET content = ?, version = version + 1, last_consolidated_at = ?, updated_at = ?
             WHERE project_id = ? AND version = ?",
            (
                content.to_string(),
                now,
                now,
                pid.to_string(),
                expected_version,
            ),
        )
        .await?
    } else {
        conn.execute(
            "UPDATE personal_memory
             SET content = ?, version = version + 1, last_consolidated_at = ?, updated_at = ?
             WHERE project_id IS NULL AND version = ?",
            (content.to_string(), now, now, expected_version),
        )
        .await?
    };

    if affected == 0 {
        return Err(anyhow!(
            "Optimistic version conflict for personal memory: expected version {}",
            expected_version
        ));
    }

    get_personal_memory(conn, project_id).await
}

/// Updates personal memory content as part of background consolidation, updating both
/// `last_consolidated_at` and `updated_at`.
pub async fn update_consolidated_memory(
    conn: &Connection,
    project_id: Option<&str>,
    new_content: &str,
) -> Result<PersonalMemoryRecord> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let affected = if let Some(pid) = project_id {
        conn.execute(
            "UPDATE personal_memory
             SET content = ?, version = version + 1, last_consolidated_at = ?, updated_at = ?
             WHERE project_id = ?",
            (new_content.to_string(), now, now, pid.to_string()),
        )
        .await?
    } else {
        conn.execute(
            "UPDATE personal_memory
             SET content = ?, version = version + 1, last_consolidated_at = ?, updated_at = ?
             WHERE project_id IS NULL",
            (new_content.to_string(), now, now),
        )
        .await?
    };

    if affected == 0 {
        return Err(anyhow!("Failed to update consolidated personal memory"));
    }

    get_personal_memory(conn, project_id).await
}
