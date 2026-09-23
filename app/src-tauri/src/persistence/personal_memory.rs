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
    pub is_active: i64,
    pub last_consolidated_at: i64,
    pub updated_at: i64,
}

/// Retrieves the active personal memory document for global scope (`project_id: None`) or a specific project.
/// If no active record exists, falls back to the highest version or inserts a default blank record.
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

    // Insert default blank record if missing
    let (pid_str, proj_arg) = if let Some(pid) = project_id {
        (Some(pid.to_string()), Some(pid.to_string()))
    } else {
        (None, None)
    };

    conn.execute(
        "INSERT INTO personal_memory (project_id, content, version, is_active, last_consolidated_at, updated_at)
         VALUES (?, '', 1, 1, ?, ?)",
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
        is_active: 1,
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
            is_active: row.get(4).unwrap_or(1),
            last_consolidated_at: row.get(5)?,
            updated_at: row.get(6)?,
        }))
    } else {
        Ok(None)
    }
}

/// Saves updated content with optimistic concurrency control against `expected_version`,
/// appending a new revision record with `is_active = 1` and marking older versions inactive.
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

    let current = get_personal_memory(conn, project_id).await?;
    if current.version != expected_version {
        return Err(anyhow!(
            "Optimistic version conflict for personal memory: expected version {}, found {}",
            expected_version,
            current.version
        ));
    }

    conn.execute("BEGIN IMMEDIATE;", ()).await?;
    let res: Result<PersonalMemoryRecord> = async {
        let (proj_arg, last_consol) = (current.project_id.clone(), current.last_consolidated_at);
        if let Some(ref pid) = proj_arg {
            conn.execute(
                "UPDATE personal_memory SET is_active = 0 WHERE project_id = ?",
                (pid.clone(),),
            )
            .await?;
            conn.execute(
                "INSERT INTO personal_memory (project_id, content, version, is_active, last_consolidated_at, updated_at)
                 VALUES (?, ?, ?, 1, ?, ?)",
                (pid.clone(), content.to_string(), expected_version + 1, last_consol, now),
            )
            .await?;
        } else {
            conn.execute(
                "UPDATE personal_memory SET is_active = 0 WHERE project_id IS NULL",
                (),
            )
            .await?;
            conn.execute(
                "INSERT INTO personal_memory (project_id, content, version, is_active, last_consolidated_at, updated_at)
                 VALUES (NULL, ?, ?, 1, ?, ?)",
                (content.to_string(), expected_version + 1, last_consol, now),
            )
            .await?;
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
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(e)
        }
    }
}

/// Saves consolidated content with optimistic concurrency control, stamping `last_consolidated_at`.
/// Appends a new revision record with `is_active = 1` and marks older versions inactive.
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

    let current = get_personal_memory(conn, project_id).await?;
    if current.version != expected_version {
        return Err(anyhow!(
            "Optimistic version conflict for personal memory: expected version {}, found {}",
            expected_version,
            current.version
        ));
    }

    conn.execute("BEGIN IMMEDIATE;", ()).await?;
    let res: Result<PersonalMemoryRecord> = async {
        let proj_arg = current.project_id.clone();
        if let Some(ref pid) = proj_arg {
            conn.execute(
                "UPDATE personal_memory SET is_active = 0 WHERE project_id = ?",
                (pid.clone(),),
            )
            .await?;
            conn.execute(
                "INSERT INTO personal_memory (project_id, content, version, is_active, last_consolidated_at, updated_at)
                 VALUES (?, ?, ?, 1, ?, ?)",
                (pid.clone(), content.to_string(), expected_version + 1, now, now),
            )
            .await?;
        } else {
            conn.execute(
                "UPDATE personal_memory SET is_active = 0 WHERE project_id IS NULL",
                (),
            )
            .await?;
            conn.execute(
                "INSERT INTO personal_memory (project_id, content, version, is_active, last_consolidated_at, updated_at)
                 VALUES (NULL, ?, ?, 1, ?, ?)",
                (content.to_string(), expected_version + 1, now, now),
            )
            .await?;
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
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(e)
        }
    }
}

/// Updates personal memory content as part of background consolidation, updating both
/// `last_consolidated_at` and `updated_at`.
pub async fn update_consolidated_memory(
    conn: &Connection,
    project_id: Option<&str>,
    new_content: &str,
) -> Result<PersonalMemoryRecord> {
    let current = get_personal_memory(conn, project_id).await?;
    save_consolidated_memory(conn, project_id, new_content, current.version).await
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
        list.push(PersonalMemoryRecord {
            id: row.get(0)?,
            project_id: row.get(1).ok(),
            content: row.get(2)?,
            version: row.get(3)?,
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
                return Err(anyhow!("Version {} not found for project {}", target_version, pid));
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
                return Err(anyhow!("Version {} not found for global personal memory", target_version));
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
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(e)
        }
    }
}
