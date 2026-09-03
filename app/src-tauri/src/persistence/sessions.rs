use anyhow::Result;
use serde::{Deserialize, Serialize};
use turso::Connection;

/// Representation of a stored conversation session.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SessionRow {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub turn_count: i64,
    pub first_message: Option<String>,
}

/// Representation of a single conversation turn in a session.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TurnRow {
    pub id: i64,
    pub session_id: i64,
    pub turn_id: i32,
    pub user_text: String,
    pub assistant_text: String,
    pub stt_latency_ms: Option<i64>,
    pub ttft_ms: Option<i64>,
    pub created_at: i64,
}

/// Returns sessions ordered by most recent first up to the given limit.
pub async fn fetch_sessions(conn: &Connection, limit: u32) -> Result<Vec<SessionRow>> {
    let mut rows = conn
        .query(
            "SELECT s.id, s.started_at, s.ended_at, s.turn_count,
                    (SELECT t.user_text FROM turns t WHERE t.session_id = s.id ORDER BY t.turn_id ASC LIMIT 1) as first_message
             FROM sessions s
             ORDER BY s.started_at DESC LIMIT ?",
            (limit as i64,),
        )
        .await?;

    let mut sessions = Vec::new();
    while let Some(row) = rows.next().await? {
        sessions.push(SessionRow {
            id: row.get(0)?,
            started_at: row.get(1)?,
            ended_at: row.get(2).ok(),
            turn_count: row.get(3)?,
            first_message: row.get(4).ok(),
        });
    }

    Ok(sessions)
}

/// Returns all turns for a given session, oldest first.
pub async fn fetch_turns(conn: &Connection, session_id: i64) -> Result<Vec<TurnRow>> {
    let mut rows = conn
        .query(
            "SELECT id, session_id, turn_id, user_text, assistant_text, stt_latency_ms, ttft_ms, created_at
             FROM turns WHERE session_id = ? ORDER BY created_at ASC",
            (session_id,),
        )
        .await?;

    let mut turns = Vec::new();
    while let Some(row) = rows.next().await? {
        turns.push(TurnRow {
            id: row.get(0)?,
            session_id: row.get(1)?,
            turn_id: row.get::<i64>(2)? as i32,
            user_text: row.get(3)?,
            assistant_text: row.get(4)?,
            stt_latency_ms: row.get(5).ok(),
            ttft_ms: row.get(6).ok(),
            created_at: row.get(7)?,
        });
    }

    Ok(turns)
}

/// Deletes a session and cascades turn deletion.
pub async fn delete_session(conn: &Connection, session_id: i64) -> Result<()> {
    conn.execute("DELETE FROM sessions WHERE id = ?", (session_id,))
        .await?;
    Ok(())
}

/// Deletes sessions where the user engaged but never completed a turn.
pub async fn cleanup_zero_turn_sessions(conn: &Connection) -> Result<u64> {
    let deleted = conn
        .execute(
            "DELETE FROM sessions WHERE turn_count = 0 AND ended_at IS NOT NULL",
            (),
        )
        .await?;
    Ok(deleted)
}
