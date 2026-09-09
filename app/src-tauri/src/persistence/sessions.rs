use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

/// Representation of a stored conversation session.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SessionRow {
    pub id: i64,
    pub project_id: String,
    pub title: Option<String>,
    pub is_pinned: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub turn_count: i64,
    pub first_message: Option<String>,
}

/// Representation of a single conversation turn in a session.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TurnRow {
    pub id: i64,
    pub session_id: i64,
    pub turn_id: u32,
    pub user_text: String,
    pub assistant_text: String,
    pub created_at: i64,
}

/// Creates a new session with an optional project ID.
pub async fn create_session(conn: &Connection, project_id: Option<&str>) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    create_session_with_id(conn, now, project_id).await
}

/// Creates a session with an explicit session ID and optional project ID.
pub async fn create_session_with_id(
    conn: &Connection,
    session_id: i64,
    project_id: Option<&str>,
) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let proj = project_id.unwrap_or("default");

    conn.execute(
        "INSERT INTO sessions (id, project_id, is_pinned, created_at, updated_at)
         VALUES (?, ?, 0, ?, ?)",
        (session_id, proj.to_string(), now, now),
    )
    .await?;

    Ok(session_id)
}

/// Returns all active (non-deleted) sessions, optionally filtered by project, ordered pinned-first then newest.
pub async fn fetch_sessions(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<SessionRow>> {
    let query = if let Some(proj) = project_id {
        let mut rows = conn
            .query(
                "SELECT s.id, s.project_id, s.title, s.is_pinned, s.deleted_at, s.created_at, s.updated_at,
                        (SELECT COUNT(*) FROM turns t WHERE t.session_id = s.id) as turn_count,
                        (SELECT t.user_text FROM turns t WHERE t.session_id = s.id ORDER BY t.turn_id ASC LIMIT 1) as first_message
                 FROM sessions s
                 WHERE s.deleted_at IS NULL AND s.project_id = ?
                 ORDER BY s.is_pinned DESC, s.updated_at DESC",
                (proj.to_string(),),
            )
            .await?;
        collect_session_rows(&mut rows).await?
    } else {
        let mut rows = conn
            .query(
                "SELECT s.id, s.project_id, s.title, s.is_pinned, s.deleted_at, s.created_at, s.updated_at,
                        (SELECT COUNT(*) FROM turns t WHERE t.session_id = s.id) as turn_count,
                        (SELECT t.user_text FROM turns t WHERE t.session_id = s.id ORDER BY t.turn_id ASC LIMIT 1) as first_message
                 FROM sessions s
                 WHERE s.deleted_at IS NULL
                 ORDER BY s.is_pinned DESC, s.updated_at DESC",
                (),
            )
            .await?;
        collect_session_rows(&mut rows).await?
    };

    Ok(query)
}

/// Helper to parse rows into Vec<SessionRow>.
async fn collect_session_rows(rows: &mut turso::Rows) -> Result<Vec<SessionRow>> {
    let mut sessions = Vec::new();
    while let Some(row) = rows.next().await? {
        let is_pinned_int: i64 = row.get(3).unwrap_or(0);
        sessions.push(SessionRow {
            id: row.get(0)?,
            project_id: row.get(1)?,
            title: row.get(2).ok(),
            is_pinned: is_pinned_int != 0,
            deleted_at: row.get(4).ok(),
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
            turn_count: row.get(7)?,
            first_message: row.get(8).ok(),
        });
    }
    Ok(sessions)
}

/// Returns a single session by its unique ID.
pub async fn fetch_session_by_id(
    conn: &Connection,
    session_id: i64,
) -> Result<Option<SessionRow>> {
    let mut rows = conn
        .query(
            "SELECT s.id, s.project_id, s.title, s.is_pinned, s.deleted_at, s.created_at, s.updated_at,
                    (SELECT COUNT(*) FROM turns t WHERE t.session_id = s.id) as turn_count,
                    (SELECT t.user_text FROM turns t WHERE t.session_id = s.id ORDER BY t.turn_id ASC LIMIT 1) as first_message
             FROM sessions s
             WHERE s.id = ?",
            (session_id,),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        let is_pinned_int: i64 = row.get(3).unwrap_or(0);
        Ok(Some(SessionRow {
            id: row.get(0)?,
            project_id: row.get(1)?,
            title: row.get(2).ok(),
            is_pinned: is_pinned_int != 0,
            deleted_at: row.get(4).ok(),
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
            turn_count: row.get(7)?,
            first_message: row.get(8).ok(),
        }))
    } else {
        Ok(None)
    }
}

/// Returns all turns for a given session, ordered by turn_id ascending.
pub async fn fetch_turns(conn: &Connection, session_id: i64) -> Result<Vec<TurnRow>> {
    let mut rows = conn
        .query(
            "SELECT id, session_id, turn_id, user_text, assistant_text, created_at
             FROM turns WHERE session_id = ? ORDER BY turn_id ASC",
            (session_id,),
        )
        .await?;

    let mut turns = Vec::new();
    while let Some(row) = rows.next().await? {
        let turn_id_i64: i64 = row.get(2)?;
        turns.push(TurnRow {
            id: row.get(0)?,
            session_id: row.get(1)?,
            turn_id: turn_id_i64 as u32,
            user_text: row.get(3)?,
            assistant_text: row.get(4)?,
            created_at: row.get(5)?,
        });
    }

    Ok(turns)
}

/// Updates session metadata fields (title, is_pinned, and/or project_id).
pub async fn update_session_metadata(
    conn: &Connection,
    session_id: i64,
    title: Option<&str>,
    is_pinned: Option<bool>,
    project_id: Option<&str>,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    if let Some(t) = title {
        conn.execute(
            "UPDATE sessions SET title = ?, updated_at = ? WHERE id = ?",
            (t.to_string(), now, session_id),
        )
        .await?;
    }

    if let Some(p) = is_pinned {
        conn.execute(
            "UPDATE sessions SET is_pinned = ?, updated_at = ? WHERE id = ?",
            (if p { 1i64 } else { 0i64 }, now, session_id),
        )
        .await?;
    }

    if let Some(pid) = project_id {
        conn.execute(
            "UPDATE sessions SET project_id = ?, updated_at = ? WHERE id = ?",
            (pid.to_string(), now, session_id),
        )
        .await?;
    }

    Ok(())
}

/// Deletes a session. If hard is true, executes hard deletion (cascades to turns and compactions);
/// otherwise marks deleted_at timestamp (soft delete).
pub async fn delete_session(conn: &Connection, session_id: i64, hard: bool) -> Result<()> {
    if hard {
        conn.execute("DELETE FROM sessions WHERE id = ?", (session_id,))
            .await?;
    } else {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let affected = conn
            .execute(
                "UPDATE sessions SET deleted_at = ? WHERE id = ?",
                (now, session_id),
            )
            .await?;
        if affected == 0 {
            return Err(anyhow!("Session not found: {}", session_id));
        }
    }
    Ok(())
}

/// Deletes inactive sessions older than 60 seconds that contain zero turns.
pub async fn cleanup_zero_turn_sessions(conn: &Connection) -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let cutoff = now - 60_000;

    let deleted = conn
        .execute(
            "DELETE FROM sessions
             WHERE id NOT IN (SELECT DISTINCT session_id FROM turns)
               AND created_at < ?",
            (cutoff,),
        )
        .await?;
    Ok(deleted)
}
