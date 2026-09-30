use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

use crate::services::memory::personal::PersonalMemory;

/// Canonical JSON for an unpopulated Personal Memory model (`db-spec.md §2.5`).
const EMPTY_PERSONAL_MEMORY_JSON: &str = r#"{"sections":[]}"#;

/// Strongly-typed row representation of a Personal Memory version, and the wire type for it.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PersonalMemoryRecord {
    pub id: i64,
    pub project_id: Option<String>,
    pub content: String,
    pub markdown: String,
    pub version: i64,
    pub is_active: i64,
    pub last_consolidated_at: i64,
    pub updated_at: i64,
}

/// Retrieves the active personal memory record for global scope (`project_id: None`) or a specific project.
/// If no active record exists, falls back to the highest version or inserts the empty canonical model.
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
                "SELECT id, project_id, content, version, is_active, last_consolidated_at, updated_at
                 FROM personal_memory
                 WHERE project_id = ? AND is_active = 1
                 ORDER BY version DESC LIMIT 1",
                (pid.to_string(),),
            )
            .await?;
        let rec = parse_record(&mut rows).await?;
        if rec.is_some() {
            rec
        } else {
            // Fallback to highest version if none is marked active
            let mut fallback_rows = conn
                .query(
                    "SELECT id, project_id, content, version, is_active, last_consolidated_at, updated_at
                     FROM personal_memory
                     WHERE project_id = ?
                     ORDER BY version DESC LIMIT 1",
                    (pid.to_string(),),
                )
                .await?;
            parse_record(&mut fallback_rows).await?
        }
    } else {
        let mut rows = conn
            .query(
                "SELECT id, project_id, content, version, is_active, last_consolidated_at, updated_at
                 FROM personal_memory
                 WHERE project_id IS NULL AND is_active = 1
                 ORDER BY version DESC LIMIT 1",
                (),
            )
            .await?;
        let rec = parse_record(&mut rows).await?;
        if rec.is_some() {
            rec
        } else {
            let mut fallback_rows = conn
                .query(
                    "SELECT id, project_id, content, version, is_active, last_consolidated_at, updated_at
                     FROM personal_memory
                     WHERE project_id IS NULL
                     ORDER BY version DESC LIMIT 1",
                    (),
                )
                .await?;
            parse_record(&mut fallback_rows).await?
        }
    };

    if let Some(record) = maybe_record {
        return Ok(record);
    }

    let (pid_str, proj_arg) = if let Some(pid) = project_id {
        (Some(pid.to_string()), Some(pid.to_string()))
    } else {
        (None, None)
    };

    conn.execute(
        "INSERT INTO personal_memory (project_id, content, version, is_active, last_consolidated_at, updated_at)
         VALUES (?, ?, 1, 1, ?, ?)",
        (proj_arg, EMPTY_PERSONAL_MEMORY_JSON, now, now),
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
        content: EMPTY_PERSONAL_MEMORY_JSON.to_string(),
        markdown: String::new(),
        version: 1,
        is_active: 1,
        last_consolidated_at: now,
        updated_at: now,
    })
}

/// Reads one `personal_memory` row, rendering its canonical JSON into `markdown`.
async fn parse_record(rows: &mut turso::Rows) -> Result<Option<PersonalMemoryRecord>> {
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    let version: i64 = row.get(3)?;
    let content: String = row.get(2)?;
    Ok(Some(PersonalMemoryRecord {
        id: row.get(0)?,
        project_id: row.get(1).ok(),
        markdown: render_stored_memory(&content, version),
        content,
        version,
        is_active: row.get(4).unwrap_or(1),
        last_consolidated_at: row.get(5)?,
        updated_at: row.get(6)?,
    }))
}

/// Renders stored canonical JSON to Markdown, degrading to an empty string on a malformed payload.
fn render_stored_memory(content: &str, version: i64) -> String {
    match PersonalMemory::from_json(content) {
        Ok(memory) => memory.render_to_markdown(),
        Err(e) => {
            log::warn!(
                "[Persistence::Memory] Stored content for v{} is not valid semantic JSON ({}); rendering empty.",
                version,
                e
            );
            String::new()
        }
    }
}

/// Appends a new version carrying `content` under an optimistic concurrency check.
pub async fn save_personal_memory(
    conn: &Connection,
    project_id: Option<&str>,
    content: &str,
    expected_version: i64,
) -> Result<PersonalMemoryRecord> {
    append_memory_version(conn, project_id, content, expected_version, false).await
}

/// Appends a new version carrying `content`, stamping `last_consolidated_at` with the commit time.
pub async fn save_consolidated_memory(
    conn: &Connection,
    project_id: Option<&str>,
    content: &str,
    expected_version: i64,
) -> Result<PersonalMemoryRecord> {
    append_memory_version(conn, project_id, content, expected_version, true).await
}

/// Inserts the new active version and deactivates its predecessors inside one transaction.
async fn append_memory_version(
    conn: &Connection,
    project_id: Option<&str>,
    content: &str,
    expected_version: i64,
    stamp_consolidated_at: bool,
) -> Result<PersonalMemoryRecord> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let current = get_personal_memory(conn, project_id).await?;
    if current.version != expected_version {
        return Err(anyhow!(
            "Optimistic version conflict for personal memory: expected version {}, found {}",
            expected_version,
            current.version
        ));
    }

    let last_consolidated_at = if stamp_consolidated_at {
        now
    } else {
        current.last_consolidated_at
    };

    conn.execute("BEGIN IMMEDIATE;", ()).await?;
    let res: Result<PersonalMemoryRecord> = async {
        deactivate_all_versions(conn, current.project_id.as_deref()).await?;
        insert_memory_version(
            conn,
            current.project_id.as_deref(),
            content,
            expected_version + 1,
            last_consolidated_at,
            now,
        )
        .await?;
        get_personal_memory(conn, project_id).await
    }
    .await;

    match res {
        Ok(rec) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(rec)
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Memory] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Marks every version of a project scope inactive.
async fn deactivate_all_versions(conn: &Connection, project_id: Option<&str>) -> Result<()> {
    match project_id {
        Some(pid) => {
            conn.execute(
                "UPDATE personal_memory SET is_active = 0 WHERE project_id = ?",
                (pid.to_string(),),
            )
            .await?;
        }
        None => {
            conn.execute(
                "UPDATE personal_memory SET is_active = 0 WHERE project_id IS NULL",
                (),
            )
            .await?;
        }
    }
    Ok(())
}

/// Inserts one `personal_memory` row as the active version.
async fn insert_memory_version(
    conn: &Connection,
    project_id: Option<&str>,
    content: &str,
    version: i64,
    last_consolidated_at: i64,
    now: i64,
) -> Result<()> {
    match project_id {
        Some(pid) => {
            conn.execute(
                "INSERT INTO personal_memory (project_id, content, version, is_active, last_consolidated_at, updated_at)
                 VALUES (?, ?, ?, 1, ?, ?)",
                (pid.to_string(), content.to_string(), version, last_consolidated_at, now),
            )
            .await?;
        }
        None => {
            conn.execute(
                "INSERT INTO personal_memory (project_id, content, version, is_active, last_consolidated_at, updated_at)
                 VALUES (NULL, ?, ?, 1, ?, ?)",
                (content.to_string(), version, last_consolidated_at, now),
            )
            .await?;
        }
    }
    Ok(())
}

/// Lists all historical versions of personal memory for a project (ordered newest version first).
pub async fn list_personal_memory_versions(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<PersonalMemoryRecord>> {
    let mut rows = if let Some(pid) = project_id {
        conn.query(
            "SELECT id, project_id, content, version, is_active, last_consolidated_at, updated_at
             FROM personal_memory
             WHERE project_id = ?
             ORDER BY version DESC",
            (pid.to_string(),),
        )
        .await?
    } else {
        conn.query(
            "SELECT id, project_id, content, version, is_active, last_consolidated_at, updated_at
             FROM personal_memory
             WHERE project_id IS NULL
             ORDER BY version DESC",
            (),
        )
        .await?
    };

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        let version: i64 = row.get(3)?;
        let content: String = row.get(2)?;
        list.push(PersonalMemoryRecord {
            id: row.get(0)?,
            project_id: row.get(1).ok(),
            markdown: render_stored_memory(&content, version),
            content,
            version,
            is_active: row.get(4).unwrap_or(1),
            last_consolidated_at: row.get(5)?,
            updated_at: row.get(6)?,
        });
    }
    Ok(list)
}

/// Sets a specific historical version of personal memory to active, deactivating all others.
pub async fn set_active_personal_memory_version(
    conn: &Connection,
    project_id: Option<&str>,
    target_version: i64,
) -> Result<PersonalMemoryRecord> {
    conn.execute("BEGIN IMMEDIATE;", ()).await?;
    let res: Result<PersonalMemoryRecord> = async {
        if let Some(pid) = project_id {
            conn.execute(
                "UPDATE personal_memory SET is_active = 0 WHERE project_id = ?",
                (pid.to_string(),),
            )
            .await?;
            let affected = conn
                .execute(
                    "UPDATE personal_memory SET is_active = 1 WHERE project_id = ? AND version = ?",
                    (pid.to_string(), target_version),
                )
                .await?;
            if affected == 0 {
                return Err(anyhow!(
                    "Version {} not found for project {}",
                    target_version,
                    pid
                ));
            }
        } else {
            conn.execute(
                "UPDATE personal_memory SET is_active = 0 WHERE project_id IS NULL",
                (),
            )
            .await?;
            let affected = conn
                .execute(
                    "UPDATE personal_memory SET is_active = 1 WHERE project_id IS NULL AND version = ?",
                    (target_version,),
                )
                .await?;
            if affected == 0 {
                return Err(anyhow!(
                    "Version {} not found for global personal memory",
                    target_version
                ));
            }
        }
        get_personal_memory(conn, project_id).await
    }
    .await;

    match res {
        Ok(rec) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(rec)
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Memory] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Strongly-typed row representation of a pending semantic memory revision.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PersonalMemoryRevisionRecord {
    pub id: String,
    pub base_memory_version: i64,
    pub project_id: Option<String>,
    pub op: String,
    pub target_id: String,
    pub content: String,
    pub status: String,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

/// A single accept or reject instruction for one pending revision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevisionDecision {
    pub id: String,
    pub action: String,
}

/// Inserts a batch of pending revisions within a single transaction.
pub async fn insert_personal_memory_revisions(
    conn: &Connection,
    revisions: &[PersonalMemoryRevisionRecord],
) -> Result<()> {
    if revisions.is_empty() {
        return Ok(());
    }

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        for rev in revisions {
            conn.execute(
                "INSERT INTO personal_memory_revisions (
                    id, base_memory_version, project_id, op,
                    target_id, content, status, created_at, resolved_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    rev.id.clone(),
                    rev.base_memory_version,
                    rev.project_id.clone(),
                    rev.op.clone(),
                    rev.target_id.clone(),
                    rev.content.clone(),
                    rev.status.clone(),
                    rev.created_at,
                    rev.resolved_at,
                ),
            )
            .await?;
        }
        Ok(())
    }
    .await;

    match tx_res {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Memory] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Fetches pending revisions for a project scope (ordered oldest created first).
pub async fn fetch_pending_revisions(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<PersonalMemoryRevisionRecord>> {
    let mut rows = if let Some(pid) = project_id {
        conn.query(
            "SELECT id, base_memory_version, project_id, op, target_id, content, status, created_at, resolved_at
             FROM personal_memory_revisions
             WHERE project_id = ? AND status = 'pending'
             ORDER BY created_at ASC",
            (pid.to_string(),),
        )
        .await?
    } else {
        conn.query(
            "SELECT id, base_memory_version, project_id, op, target_id, content, status, created_at, resolved_at
             FROM personal_memory_revisions
             WHERE project_id IS NULL AND status = 'pending'
             ORDER BY created_at ASC",
            (),
        )
        .await?
    };

    let mut revisions = Vec::new();
    while let Some(row) = rows.next().await? {
        revisions.push(PersonalMemoryRevisionRecord {
            id: row.get(0)?,
            base_memory_version: row.get(1)?,
            project_id: row.get(2).ok(),
            op: row.get(3)?,
            target_id: row.get(4)?,
            content: row.get(5)?,
            status: row.get(6)?,
            created_at: row.get(7)?,
            resolved_at: row.get(8).ok(),
        });
    }

    Ok(revisions)
}

/// Bulk-rejects every pending revision of a project scope, used when regeneration supersedes all IDs.
pub async fn reject_all_pending_revisions(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<usize> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let (sql, arg) = match project_id {
        Some(pid) => (
            "UPDATE personal_memory_revisions SET status = 'rejected', resolved_at = ?
             WHERE project_id = ? AND status = 'pending'",
            Some(pid.to_string()),
        ),
        None => (
            "UPDATE personal_memory_revisions SET status = 'rejected', resolved_at = ?
             WHERE project_id IS NULL AND status = 'pending'",
            None,
        ),
    };

    let affected = match arg {
        Some(pid) => conn.execute(sql, (now, pid)).await?,
        None => conn.execute(sql, (now,)).await?,
    };
    Ok(affected as usize)
}

/// Resolves a batch of personal memory revisions in a single atomic transaction.
pub async fn resolve_batch_revisions_transaction(
    conn: &Connection,
    project_id: Option<&str>,
    decisions: &[RevisionDecision],
    new_content: Option<&str>,
) -> Result<PersonalMemoryRecord> {
    if decisions.is_empty() {
        return get_personal_memory(conn, project_id).await;
    }

    for d in decisions {
        if d.action != "accept" && d.action != "reject" {
            return Err(anyhow!("Invalid revision resolution action: {}", d.action));
        }
    }

    let current = get_personal_memory(conn, project_id).await?;
    let pending = fetch_pending_revisions(conn, project_id).await?;
    let pending_ids: std::collections::HashSet<&str> =
        pending.iter().map(|r| r.id.as_str()).collect();

    for d in decisions {
        if !pending_ids.contains(d.id.as_str()) {
            return Err(anyhow!("Pending revision '{}' not found", d.id));
        }
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let accepted: Vec<&RevisionDecision> =
        decisions.iter().filter(|d| d.action == "accept").collect();
    let rejected: Vec<&RevisionDecision> =
        decisions.iter().filter(|d| d.action == "reject").collect();

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<PersonalMemoryRecord> = async {
        if !accepted.is_empty() {
            let updated_json = new_content
                .ok_or_else(|| anyhow!("new_content required when accepting revisions"))?;
            deactivate_all_versions(conn, current.project_id.as_deref()).await?;
            insert_memory_version(
                conn,
                current.project_id.as_deref(),
                updated_json,
                current.version + 1,
                now,
                now,
            )
            .await?;
            set_revision_status(conn, &accepted, "accepted", now).await?;
        }

        if !rejected.is_empty() {
            set_revision_status(conn, &rejected, "rejected", now).await?;
        }

        get_personal_memory(conn, project_id).await
    }
    .await;

    match tx_res {
        Ok(rec) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(rec)
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Memory] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Flips the given revision rows to `status` with a resolution timestamp.
async fn set_revision_status(
    conn: &Connection,
    decisions: &[&RevisionDecision],
    status: &str,
    now: i64,
) -> Result<()> {
    let quoted: Vec<String> = decisions
        .iter()
        .map(|d| format!("'{}'", d.id.replace('\'', "''")))
        .collect();
    let sql = format!(
        "UPDATE personal_memory_revisions SET status = ?, resolved_at = ? WHERE id IN ({})",
        quoted.join(",")
    );
    conn.execute(&sql, (status.to_string(), now)).await?;
    Ok(())
}
