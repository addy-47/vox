use anyhow::{anyhow, Result};
use std::collections::HashMap;
use turso::Connection;

use crate::services::memory::QueueStatus;

/// Represents a session with turns that have not yet been compacted into memory facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncompactedSession {
    pub session_id: i64,
    pub turn_count: u32,
    pub last_compacted_turn_id: u32,
}

/// Dialogue turn text used to build compaction history prompt messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnDialogue {
    pub turn_id: u32,
    pub user_text: String,
    pub assistant_text: String,
}

/// Record of a compaction execution attempt from the `session_compactions` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionRunRecord {
    pub id: i64,
    pub session_id: i64,
    pub trigger_kind: String,
    pub from_turn_id: u32,
    pub to_turn_id: u32,
    pub status: String,
    pub facts_count: u32,
    pub error_msg: Option<String>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

/// Fetches sessions where the total turn count exceeds the highest successfully completed compaction turn.
pub async fn fetch_uncompacted_sessions(conn: &Connection) -> Result<Vec<UncompactedSession>> {
    let mut rows = conn
        .query(
            "SELECT s.id, s.turn_count,
                    COALESCE((SELECT MAX(sc.to_turn_id) FROM session_compactions sc WHERE sc.session_id = s.id AND sc.status = 'completed'), 0) as last_compacted
             FROM sessions s
             WHERE s.turn_count > 0 AND s.turn_count > (
                 SELECT COALESCE(MAX(sc.to_turn_id), 0) FROM session_compactions sc WHERE sc.session_id = s.id AND sc.status = 'completed'
             )
             ORDER BY s.started_at DESC",
            (),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        let session_id: i64 = row.get(0)?;
        let turn_count: i64 = row.get(1)?;
        let last_compacted: i64 = row.get(2)?;
        list.push(UncompactedSession {
            session_id,
            turn_count: turn_count as u32,
            last_compacted_turn_id: last_compacted as u32,
        });
    }
    Ok(list)
}

/// Fetches dialogue turns for a session strictly after `from_turn_id` ordered sequentially.
pub async fn fetch_turns_for_compaction(
    conn: &Connection,
    session_id: i64,
    from_turn_id: u32,
) -> Result<Vec<TurnDialogue>> {
    let mut rows = conn
        .query(
            "SELECT turn_id, user_text, assistant_text
             FROM turns
             WHERE session_id = ? AND turn_id > ?
             ORDER BY turn_id ASC",
            (session_id, from_turn_id as i64),
        )
        .await?;

    let mut turns = Vec::new();
    while let Some(row) = rows.next().await? {
        let turn_id: i64 = row.get(0)?;
        let user_text: String = row.get(1)?;
        let assistant_text: String = row.get(2)?;
        turns.push(TurnDialogue {
            turn_id: turn_id as u32,
            user_text,
            assistant_text,
        });
    }
    Ok(turns)
}

/// Fetches the latest compaction execution record for a session.
pub async fn fetch_latest_compaction_run(
    conn: &Connection,
    session_id: i64,
) -> Result<Option<CompactionRunRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, session_id, trigger_kind, from_turn_id, to_turn_id, status, facts_count, error_msg, created_at, completed_at
             FROM session_compactions
             WHERE session_id = ?
             ORDER BY created_at DESC
             LIMIT 1",
            (session_id,),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        let id: i64 = row.get(0)?;
        let s_id: i64 = row.get(1)?;
        let trigger_kind: String = row.get(2)?;
        let from_turn_id: i64 = row.get(3)?;
        let to_turn_id: i64 = row.get(4)?;
        let status: String = row.get(5)?;
        let facts_count: i64 = row.get(6).unwrap_or(0);
        let error_msg: Option<String> = row.get(7).ok();
        let created_at: i64 = row.get(8).unwrap_or(0);
        let completed_at: Option<i64> = row.get(9).ok();

        Ok(Some(CompactionRunRecord {
            id,
            session_id: s_id,
            trigger_kind,
            from_turn_id: from_turn_id as u32,
            to_turn_id: to_turn_id as u32,
            status,
            facts_count: facts_count as u32,
            error_msg,
            created_at,
            completed_at,
        }))
    } else {
        Ok(None)
    }
}

/// Records the start of a compaction execution attempt in the `session_compactions` table.
pub async fn record_compaction_start(
    conn: &Connection,
    session_id: i64,
    trigger_kind: &str,
    from_turn: u32,
    to_turn: u32,
) -> Result<i64> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO session_compactions (session_id, trigger_kind, from_turn_id, to_turn_id, status, facts_count, created_at)
         VALUES (?, ?, ?, ?, 'in_progress', 0, ?)",
        (session_id, trigger_kind.to_string(), from_turn as i64, to_turn as i64, now),
    )
    .await?;

    let mut rows = conn.query("SELECT last_insert_rowid();", ()).await?;
    if let Some(row) = rows.next().await? {
        let id: i64 = row.get(0)?;
        Ok(id)
    } else {
        Err(anyhow!("Failed to retrieve inserted compaction run ID"))
    }
}

/// Records the completion or failure of a compaction execution attempt.
pub async fn record_compaction_finish(
    conn: &Connection,
    run_id: i64,
    status: &str,
    facts_count: u32,
    error: Option<&str>,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "UPDATE session_compactions
         SET status = ?, facts_count = ?, error_msg = ?, completed_at = ?
         WHERE id = ?",
        (
            status.to_string(),
            facts_count as i64,
            error.map(|s| s.to_string()),
            now,
            run_id,
        ),
    )
    .await?;

    Ok(())
}

/// Atomically commits compaction facts to `personal_memory_queue`, saves operational `Context`
/// memory fact (if present), and marks the compaction run record as completed.
pub async fn commit_compaction_results(
    conn: &Connection,
    run_id: i64,
    session_id: &str,
    context_summary: &str,
    facts: HashMap<String, Vec<String>>,
    pipeline_enabled: bool,
) -> Result<u32> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut total_facts = 0u32;
    for list in facts.values() {
        total_facts += list.len() as u32;
    }

    conn.execute("BEGIN IMMEDIATE;", ()).await?;
    let res: Result<()> = async {
        for (collection, fact_list) in &facts {
            let status = if pipeline_enabled {
                QueueStatus::StagedPending.as_str()
            } else {
                QueueStatus::Paused.as_str()
            };

            for fact in fact_list {
                let trimmed = fact.trim();
                if trimmed.is_empty() {
                    continue;
                }
                conn.execute(
                    "INSERT INTO personal_memory_queue (fact, collection, source, session_id, status, created_at)
                     VALUES (?, ?, 'LLM', ?, ?, ?)",
                    (
                        trimmed.to_string(),
                        collection.clone(),
                        session_id.to_string(),
                        status.to_string(),
                        now,
                    ),
                )
                .await?;
            }
        }

        if !context_summary.trim().is_empty() {
            let context_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());
            conn.execute(
                "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
                 VALUES (?, 'operational', 'Context', ?, 'LLM', 'active', ?, ?)",
                (context_id, context_summary.trim().to_string(), now, session_id.to_string()),
            ).await?;
            log::info!("[Persistence::Compactions] Saved session Context memory for session_id={}", session_id);
        }

        conn.execute(
            "UPDATE session_compactions
             SET status = 'completed', facts_count = ?, completed_at = ?
             WHERE id = ?",
            (total_facts as i64, now, run_id),
        )
        .await?;

        Ok(())
    }
    .await;

    if res.is_ok() {
        conn.execute("COMMIT;", ()).await?;
        Ok(total_facts)
    } else {
        let _ = conn.execute("ROLLBACK;", ()).await;
        res.map(|_| total_facts)
    }
}
