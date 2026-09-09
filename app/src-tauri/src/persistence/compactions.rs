use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

use super::sessions::TurnRow;

/// Record representation of a rolling compaction pass.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CompactionRecord {
    pub id: i64,
    pub session_id: i64,
    pub trigger_kind: String,
    pub from_turn_id: u32,
    pub to_turn_id: u32,
    pub compaction_output: String,
    pub status: String,
    pub error_msg: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

/// Records the initiation of a compaction run and returns the assigned row ID.
pub async fn record_compaction_start(
    conn: &Connection,
    session_id: i64,
    trigger_kind: &str,
    from_turn_id: u32,
    to_turn_id: u32,
) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut rows = conn
        .query(
            "INSERT INTO session_compactions (session_id, trigger_kind, from_turn_id, to_turn_id, compaction_output, status, created_at)
             VALUES (?, ?, ?, ?, '', 'in_progress', ?) RETURNING id;",
            (
                session_id,
                trigger_kind.to_string(),
                from_turn_id as i64,
                to_turn_id as i64,
                now,
            ),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(row.get(0)?)
    } else {
        Err(anyhow!("Failed to retrieve generated compaction id"))
    }
}

/// Marks a compaction run as finished (either 'completed' or 'failed').
pub async fn record_compaction_finish(
    conn: &Connection,
    compaction_id: i64,
    compaction_output: &str,
    status: &str,
    error_msg: Option<&str>,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "UPDATE session_compactions
         SET compaction_output = ?, status = ?, error_msg = ?, finished_at = ?
         WHERE id = ?",
        (
            compaction_output.to_string(),
            status.to_string(),
            error_msg.map(|s| s.to_string()),
            now,
            compaction_id,
        ),
    )
    .await?;

    Ok(())
}

/// Fetches the latest compaction run record for a given session.
pub async fn fetch_latest_compaction_run(
    conn: &Connection,
    session_id: i64,
) -> Result<Option<CompactionRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, session_id, trigger_kind, from_turn_id, to_turn_id, compaction_output, status, error_msg, created_at, finished_at
             FROM session_compactions
             WHERE session_id = ?
             ORDER BY id DESC
             LIMIT 1",
            (session_id,),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        let from_id: i64 = row.get(3)?;
        let to_id: i64 = row.get(4)?;
        Ok(Some(CompactionRecord {
            id: row.get(0)?,
            session_id: row.get(1)?,
            trigger_kind: row.get(2)?,
            from_turn_id: from_id as u32,
            to_turn_id: to_id as u32,
            compaction_output: row.get(5)?,
            status: row.get(6)?,
            error_msg: row.get(7).ok(),
            created_at: row.get(8)?,
            finished_at: row.get(9).ok(),
        }))
    } else {
        Ok(None)
    }
}

/// Fetches turns in a closed interval [from_turn_id, to_turn_id] for compaction context assembly.
pub async fn fetch_turns_for_compaction(
    conn: &Connection,
    session_id: i64,
    from_turn_id: u32,
    to_turn_id: u32,
) -> Result<Vec<TurnRow>> {
    let mut rows = conn
        .query(
            "SELECT id, session_id, turn_id, user_text, assistant_text, created_at
             FROM turns
             WHERE session_id = ? AND turn_id >= ? AND turn_id <= ?
             ORDER BY turn_id ASC",
            (session_id, from_turn_id as i64, to_turn_id as i64),
        )
        .await?;

    let mut turns = Vec::new();
    while let Some(row) = rows.next().await? {
        let tid: i64 = row.get(2)?;
        turns.push(TurnRow {
            id: row.get(0)?,
            session_id: row.get(1)?,
            turn_id: tid as u32,
            user_text: row.get(3)?,
            assistant_text: row.get(4)?,
            created_at: row.get(5)?,
        });
    }

    Ok(turns)
}

/// Atomically commits compaction output and enqueues extracted facts into `memory_ingestion_queue`.
pub async fn commit_compaction_output(
    conn: &Connection,
    compaction_id: i64,
    output_json: &str,
    facts: &[(String, String)], // (fact_type, text)
    session_id: i64,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        conn.execute(
            "UPDATE session_compactions
             SET compaction_output = ?, status = 'completed', finished_at = ?
             WHERE id = ?",
            (output_json.to_string(), now, compaction_id),
        )
        .await?;

        for (fact_type, fact_text) in facts {
            let trimmed = fact_text.trim();
            if trimmed.is_empty() {
                continue;
            }
            conn.execute(
                "INSERT INTO memory_ingestion_queue (session_id, compaction_id, type, text, status, retry_count, created_at)
                 VALUES (?, ?, ?, ?, 'pending', 0, ?)",
                (
                    session_id,
                    compaction_id,
                    fact_type.to_string(),
                    trimmed.to_string(),
                    now,
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
                log::warn!("[Persistence::Compactions] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Resolves the compactable turn range for a session as `(from_turn_id, to_turn_id)`,
/// where `from` is one past the latest completed run and `to` is the highest persisted turn.
/// `to` may be less than `from` when no uncompacted turns are persisted yet (empty range marker).
pub async fn resolve_uncompacted_range(
    conn: &Connection,
    session_id: i64,
) -> Result<(u32, u32)> {
    let from = match fetch_latest_compaction_run(conn, session_id).await? {
        Some(run) if run.status == "completed" => run.to_turn_id.saturating_add(1),
        _ => 1,
    };
    let mut rows = conn
        .query(
            "SELECT COALESCE(MAX(turn_id), 0) FROM turns WHERE session_id = ?",
            (session_id,),
        )
        .await?;
    let max_turn: i64 = if let Some(row) = rows.next().await? {
        row.get(0)?
    } else {
        0
    };
    let to = (max_turn as u32).max(from.saturating_sub(1));
    Ok((from, to))
}

/// Returns true when any compaction run is currently in progress.
pub async fn has_in_progress_compaction(conn: &Connection) -> Result<bool> {
    let mut rows = conn
        .query(
            "SELECT id FROM session_compactions WHERE status = 'in_progress' LIMIT 1",
            (),
        )
        .await?;
    Ok(rows.next().await?.is_some())
}

/// Item representing a session with pending uncompacted turns.
#[derive(Debug, Clone)]
pub struct UncompactedSessionItem {
    pub session_id: i64,
    pub turn_count: u32,
    pub last_compacted_turn_id: u32,
}

/// Fetches all active sessions where turns exist beyond the latest completed compaction.
pub async fn fetch_uncompacted_sessions(conn: &Connection) -> Result<Vec<UncompactedSessionItem>> {
    let mut rows = conn
        .query(
            "SELECT s.id,
                    (SELECT COUNT(*) FROM turns t WHERE t.session_id = s.id) as turn_count,
                    (SELECT COALESCE(MAX(c.to_turn_id), 0) FROM session_compactions c WHERE c.session_id = s.id AND c.status = 'completed') as last_compacted
             FROM sessions s
             WHERE s.deleted_at IS NULL
               AND (SELECT COUNT(*) FROM turns t WHERE t.session_id = s.id) > 0",
            (),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        let tc: i64 = row.get(1)?;
        let lc: i64 = row.get(2)?;
        if tc as u32 > lc as u32 {
            list.push(UncompactedSessionItem {
                session_id: row.get(0)?,
                turn_count: tc as u32,
                last_compacted_turn_id: lc as u32,
            });
        }
    }
    Ok(list)
}

/// Convenience helper for committing compaction results from a facts HashMap.
pub async fn commit_compaction_results(
    conn: &Connection,
    compaction_id: i64,
    session_id_str: &str,
    context_summary: &str,
    facts: std::collections::HashMap<String, Vec<String>>,
    pipeline_enabled: bool,
) -> Result<u32> {
    log::debug!(
        "[Persistence::Compactions] Committing results: session={}, pipeline_enabled={}",
        session_id_str,
        pipeline_enabled
    );
    let session_id = session_id_str.parse::<i64>().unwrap_or(0);
    let mut flat_facts = Vec::new();
    for (fact_type, list) in &facts {
        for text in list {
            flat_facts.push((fact_type.clone(), text.clone()));
        }
    }
    let total = flat_facts.len() as u32;
    let output_json = serde_json::to_string(&serde_json::json!({
        "context_summary": context_summary,
        "facts": facts,
    }))
    .unwrap_or_default();

    commit_compaction_output(conn, compaction_id, &output_json, &flat_facts, session_id).await?;
    Ok(total)
}
