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
    pub target_index: u32,
    pub content: String,
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
            conn.execute(
                "INSERT INTO personal_memory_suggestions (
                    id, base_memory_version, project_id, op,
                    target_index, content, status, created_at, resolved_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    s.id.clone(),
                    s.base_memory_version,
                    s.project_id.clone(),
                    s.op.clone(),
                    s.target_index,
                    s.content.clone(),
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
            "SELECT id, base_memory_version, project_id, op, target_index, content, status, created_at, resolved_at
             FROM personal_memory_suggestions
             WHERE project_id = ? AND status = 'pending'
             ORDER BY created_at ASC",
            (pid.to_string(),),
        )
        .await?
    } else {
        conn.query(
            "SELECT id, base_memory_version, project_id, op, target_index, content, status, created_at, resolved_at
             FROM personal_memory_suggestions
             WHERE project_id IS NULL AND status = 'pending'
             ORDER BY created_at ASC",
            (),
        )
        .await?
    };

    let mut suggestions = Vec::new();
    while let Some(row) = rows.next().await? {
        suggestions.push(PersonalMemorySuggestionRecord {
            id: row.get(0)?,
            base_memory_version: row.get(1)?,
            project_id: row.get(2).ok(),
            op: row.get(3)?,
            target_index: row.get(4)?,
            content: row.get(5)?,
            status: row.get(6)?,
            created_at: row.get(7)?,
            resolved_at: row.get(8).ok(),
        });
    }

    Ok(suggestions)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuggestionDecision {
    pub id: String,
    pub action: String, // "accept" | "reject"
}

/// Resolves a batch of personal memory suggestions in a single atomic transaction.
/// Either accepts suggestions (applying new_content, bumping version, marking suggestions 'accepted')
/// and/or rejects suggestions (marking suggestions 'rejected').
pub async fn resolve_batch_suggestions_transaction(
    conn: &Connection,
    project_id: Option<&str>,
    decisions: &[SuggestionDecision],
    new_content: Option<&str>,
) -> Result<PersonalMemoryRecord> {
    if decisions.is_empty() {
        return get_personal_memory(conn, project_id).await;
    }

    for d in decisions {
        if d.action != "accept" && d.action != "reject" {
            return Err(anyhow!("Invalid suggestion resolution action: {}", d.action));
        }
    }

    let current = get_personal_memory(conn, project_id).await?;
    let pending = fetch_pending_suggestions(conn, project_id).await?;
    let pending_map: std::collections::HashMap<String, PersonalMemorySuggestionRecord> =
        pending.into_iter().map(|s| (s.id.clone(), s)).collect();

    for d in decisions {
        if !pending_map.contains_key(&d.id) {
            return Err(anyhow!("Pending suggestion '{}' not found", d.id));
        }
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let accepted: Vec<&SuggestionDecision> = decisions
        .iter()
        .filter(|d| d.action == "accept")
        .collect();

    let rejected: Vec<&SuggestionDecision> = decisions
        .iter()
        .filter(|d| d.action == "reject")
        .collect();

    let all_resolved_ids: Vec<String> = decisions.iter().map(|d| d.id.clone()).collect();

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<PersonalMemoryRecord> = async {
        let proj_arg = current.project_id.clone();

        if !accepted.is_empty() {
            let updated_markdown = new_content
                .ok_or_else(|| anyhow!("new_content required when accepting suggestions"))?;

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

            let quoted_accepted: Vec<String> = accepted.iter().map(|d| format!("'{}'", d.id)).collect();
            let accept_sql = format!(
                "UPDATE personal_memory_suggestions SET status = 'accepted', resolved_at = ? WHERE id IN ({})",
                quoted_accepted.join(",")
            );
            conn.execute(&accept_sql, (now,)).await?;

            // If a single suggestion was accepted and others remain pending, apply INVARIANT 5.3-B arithmetic re-anchoring
            if accepted.len() == 1 {
                let resolved_sug = &pending_map[&accepted[0].id];
                let k = resolved_sug.target_index;
                let sug_in_clause = quoted_accepted.join(",");

                if resolved_sug.op == "insert_after" {
                    let shift_sql = if proj_arg.is_some() {
                        format!(
                            "UPDATE personal_memory_suggestions \
                             SET target_index = target_index + 1 \
                             WHERE project_id = ? AND status = 'pending' AND id NOT IN ({}) AND target_index > ?",
                            sug_in_clause
                        )
                    } else {
                        format!(
                            "UPDATE personal_memory_suggestions \
                             SET target_index = target_index + 1 \
                             WHERE project_id IS NULL AND status = 'pending' AND id NOT IN ({}) AND target_index > ?",
                            sug_in_clause
                        )
                    };
                    if let Some(ref pid) = proj_arg {
                        conn.execute(&shift_sql, (pid.clone(), k)).await?;
                    } else {
                        conn.execute(&shift_sql, (k,)).await?;
                    }
                } else if resolved_sug.op == "delete" {
                    let shift_sql = if proj_arg.is_some() {
                        format!(
                            "UPDATE personal_memory_suggestions \
                             SET target_index = target_index - 1 \
                             WHERE project_id = ? AND status = 'pending' AND id NOT IN ({}) AND target_index > ?",
                            sug_in_clause
                        )
                    } else {
                        format!(
                            "UPDATE personal_memory_suggestions \
                             SET target_index = target_index - 1 \
                             WHERE project_id IS NULL AND status = 'pending' AND id NOT IN ({}) AND target_index > ?",
                            sug_in_clause
                        )
                    };
                    if let Some(ref pid) = proj_arg {
                        conn.execute(&shift_sql, (pid.clone(), k)).await?;
                    } else {
                        conn.execute(&shift_sql, (k,)).await?;
                    }
                }
            }

            // Update base_memory_version for all remaining pending suggestions
            let quoted_all: Vec<String> = all_resolved_ids.iter().map(|id| format!("'{}'", id)).collect();
            let reanchor_sql = if proj_arg.is_some() {
                format!(
                    "UPDATE personal_memory_suggestions \
                     SET base_memory_version = ? \
                     WHERE project_id = ? AND status = 'pending' AND id NOT IN ({})",
                    quoted_all.join(",")
                )
            } else {
                format!(
                    "UPDATE personal_memory_suggestions \
                     SET base_memory_version = ? \
                     WHERE project_id IS NULL AND status = 'pending' AND id NOT IN ({})",
                    quoted_all.join(",")
                )
            };
            if let Some(ref pid) = proj_arg {
                conn.execute(&reanchor_sql, (current.version + 1, pid.clone())).await?;
            } else {
                conn.execute(&reanchor_sql, (current.version + 1,)).await?;
            }
        }

        if !rejected.is_empty() {
            let quoted_rejected: Vec<String> = rejected.iter().map(|d| format!("'{}'", d.id)).collect();
            let reject_sql = format!(
                "UPDATE personal_memory_suggestions SET status = 'rejected', resolved_at = ? WHERE id IN ({})",
                quoted_rejected.join(",")
            );
            conn.execute(&reject_sql, (now,)).await?;
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
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(e)
        }
    }
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
            return Err(anyhow!("Pending suggestion '{}' not found", tid));
        }
        found
    } else {
        pending
    };

    if to_resolve.is_empty() {
        return Ok(current);
    }

    let decisions: Vec<SuggestionDecision> = to_resolve
        .into_iter()
        .map(|s| SuggestionDecision {
            id: s.id,
            action: action.to_string(),
        })
        .collect();

    resolve_batch_suggestions_transaction(conn, project_id, &decisions, new_content).await
}
