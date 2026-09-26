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

/// Strongly-typed row representation of a personal memory suggestion.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PersonalMemorySuggestionRecord {
    pub id: String,
    pub base_memory_version: i64,
    pub project_id: Option<String>,
    pub op: String,
    pub section: String,
    pub target_text: Option<String>,
    pub proposed_text: String,
    pub source_fact_ids: Vec<String>,
    pub status: String,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

pub type MemorySuggestionRecord = PersonalMemorySuggestionRecord;

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

/// Inserts a batch of personal memory suggestions within a single transaction.
pub async fn insert_personal_memory_suggestions(
    conn: &Connection,
    suggestions: &[PersonalMemorySuggestionRecord],
) -> Result<()> {
    if suggestions.is_empty() {
        return Ok(());
    }

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        for s in suggestions {
            let fact_ids_json =
                serde_json::to_string(&s.source_fact_ids).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "INSERT INTO personal_memory_suggestions (
                    id, base_memory_version, project_id, op, section,
                    target_text, proposed_text, source_fact_ids, status, created_at, resolved_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    s.id.clone(),
                    s.base_memory_version,
                    s.project_id.clone(),
                    s.op.clone(),
                    s.section.clone(),
                    s.target_text.clone(),
                    s.proposed_text.clone(),
                    fact_ids_json,
                    s.status.clone(),
                    s.created_at,
                    s.resolved_at,
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
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(e)
        }
    }
}

/// Fetches pending suggestions for a project scope (ordered oldest created first).
pub async fn fetch_pending_suggestions(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<PersonalMemorySuggestionRecord>> {
    let mut rows = if let Some(pid) = project_id {
        conn.query(
            "SELECT id, base_memory_version, project_id, op, section, target_text, proposed_text, source_fact_ids, status, created_at, resolved_at
             FROM personal_memory_suggestions
             WHERE project_id = ? AND status = 'pending'
             ORDER BY created_at ASC",
            (pid.to_string(),),
        )
        .await?
    } else {
        conn.query(
            "SELECT id, base_memory_version, project_id, op, section, target_text, proposed_text, source_fact_ids, status, created_at, resolved_at
             FROM personal_memory_suggestions
             WHERE project_id IS NULL AND status = 'pending'
             ORDER BY created_at ASC",
            (),
        )
        .await?
    };

    let mut suggestions = Vec::new();
    while let Some(row) = rows.next().await? {
        let fact_ids_str: String = row.get(7)?;
        let fact_ids: Vec<String> = serde_json::from_str(&fact_ids_str).unwrap_or_default();
        suggestions.push(PersonalMemorySuggestionRecord {
            id: row.get(0)?,
            base_memory_version: row.get(1)?,
            project_id: row.get(2).ok(),
            op: row.get(3)?,
            section: row.get(4)?,
            target_text: row.get(5).ok(),
            proposed_text: row.get(6)?,
            source_fact_ids: fact_ids,
            status: row.get(8)?,
            created_at: row.get(9)?,
            resolved_at: row.get(10).ok(),
        });
    }

    Ok(suggestions)
}

/// Resolves personal memory suggestions in a single atomic transaction.
/// Either accepts suggestions (applying new_content, bumping version, marking facts 'consolidated')
/// or rejects suggestions (marking suggestions and associated facts 'rejected').
pub async fn resolve_suggestions_transaction(
    conn: &Connection,
    project_id: Option<&str>,
    target_id: Option<&str>,
    action: &str,
    new_content: Option<&str>,
) -> Result<PersonalMemoryRecord> {
    if action != "accept" && action != "reject" {
        return Err(anyhow!("Invalid suggestion resolution action: {}", action));
    }

    let current = get_personal_memory(conn, project_id).await?;
    let pending = fetch_pending_suggestions(conn, project_id).await?;

    let to_resolve: Vec<PersonalMemorySuggestionRecord> = if let Some(tid) = target_id {
        let found = pending
            .into_iter()
            .filter(|s| s.id == tid)
            .collect::<Vec<_>>();
        if found.is_empty() {
            return Err(anyhow!(
                "Pending suggestion '{}' not found",
                tid
            ));
        }
        found
    } else {
        pending
    };

    if to_resolve.is_empty() {
        return Ok(current);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let suggestion_ids: Vec<String> = to_resolve.iter().map(|s| s.id.clone()).collect();
    let mut all_fact_ids: Vec<String> = Vec::new();
    for s in &to_resolve {
        for fid in &s.source_fact_ids {
            if !fid.is_empty() && !all_fact_ids.contains(fid) {
                all_fact_ids.push(fid.clone());
            }
        }
    }

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<PersonalMemoryRecord> = async {
        let quoted_sug_ids: Vec<String> = suggestion_ids.iter().map(|id| format!("'{}'", id)).collect();
        let sug_in_clause = quoted_sug_ids.join(",");

        if action == "accept" {
            let updated_markdown = new_content
                .ok_or_else(|| anyhow!("new_content required when accepting suggestions"))?;

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
                    (pid.clone(), updated_markdown.to_string(), current.version + 1, now, now),
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
                    (updated_markdown.to_string(), current.version + 1, now, now),
                )
                .await?;
            }

            let update_sug_sql = format!(
                "UPDATE personal_memory_suggestions SET status = 'accepted', resolved_at = ? WHERE id IN ({})",
                sug_in_clause
            );
            conn.execute(&update_sug_sql, (now,)).await?;

            // Re-anchor any remaining un-resolved pending suggestions to the new base memory version
            let reanchor_sql = if proj_arg.is_some() {
                format!(
                    "UPDATE personal_memory_suggestions \
                     SET base_memory_version = ? \
                     WHERE project_id = ? AND status = 'pending' AND id NOT IN ({})",
                    sug_in_clause
                )
            } else {
                format!(
                    "UPDATE personal_memory_suggestions \
                     SET base_memory_version = ? \
                     WHERE project_id IS NULL AND status = 'pending' AND id NOT IN ({})",
                    sug_in_clause
                )
            };
            if let Some(ref pid) = proj_arg {
                conn.execute(&reanchor_sql, (current.version + 1, pid.clone())).await?;
            } else {
                conn.execute(&reanchor_sql, (current.version + 1,)).await?;
            }

            if !all_fact_ids.is_empty() {
                let quoted_fact_ids: Vec<String> = all_fact_ids.iter().map(|id| format!("'{}'", id)).collect();
                let fact_in_clause = quoted_fact_ids.join(",");
                let sql_facts = format!(
                    "UPDATE memory_facts SET status = 'consolidated', updated_at = ? WHERE id IN ({})",
                    fact_in_clause
                );
                conn.execute(&sql_facts, (now,)).await?;
                let sql_vectors = format!(
                    "UPDATE memory_facts_vectors SET status = 'consolidated' WHERE fact_id IN ({})",
                    fact_in_clause
                );
                conn.execute(&sql_vectors, ()).await?;
            }

            get_personal_memory(conn, project_id).await

        } else {
            let update_sug_sql = format!(
                "UPDATE personal_memory_suggestions SET status = 'rejected', resolved_at = ? WHERE id IN ({})",
                sug_in_clause
            );
            conn.execute(&update_sug_sql, (now,)).await?;

            if !all_fact_ids.is_empty() {
                let quoted_fact_ids: Vec<String> = all_fact_ids.iter().map(|id| format!("'{}'", id)).collect();
                let fact_in_clause = quoted_fact_ids.join(",");
                let sql_facts = format!(
                    "UPDATE memory_facts SET status = 'rejected', updated_at = ? WHERE id IN ({})",
                    fact_in_clause
                );
                conn.execute(&sql_facts, (now,)).await?;
                let sql_vectors = format!(
                    "UPDATE memory_facts_vectors SET status = 'rejected' WHERE fact_id IN ({})",
                    fact_in_clause
                );
                conn.execute(&sql_vectors, ()).await?;
            }

            Ok(current)
        }
    }
    .await;

    match tx_res {
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
