use std::time::{SystemTime, UNIX_EPOCH};

use turso::Connection;

use crate::core::{
    error::PersistenceError,
    events::{TaskDto, TaskStatus},
};

/// Inserts or replaces a task record when it is promoted to the background.
pub async fn record_task_promoted(
    conn: &Connection,
    task_id: &str,
    session_id: i64,
    tool_call_id: Option<&str>,
    command: &str,
    log_path: &str,
    started_at: i64,
) -> Result<(), PersistenceError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO session_tasks (task_id, session_id, tool_call_id, command, status, log_path, started_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'running', ?, ?, ?, ?);",
        (
            task_id,
            session_id,
            tool_call_id,
            command,
            log_path,
            started_at,
            now,
            now,
        ),
    )
    .await?;

    Ok(())
}

/// Updates or upserts a task record to a terminal state (`completed`, `failed`, or `killed`).
pub async fn record_task_finished(
    conn: &Connection,
    task_id: &str,
    session_id: i64,
    status: &TaskStatus,
) -> Result<(), PersistenceError> {
    let now_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    match status {
        TaskStatus::Completed {
            exit_code,
            duration_ms,
            completed_at,
        } => {
            conn.execute(
                "INSERT INTO session_tasks (task_id, session_id, command, status, exit_code, duration_ms, completed_at, log_path, started_at, created_at, updated_at)
                 VALUES (?1, ?2, 'background task', 'completed', ?3, ?4, ?5, '', ?5, ?5, ?6)
                 ON CONFLICT(task_id) DO UPDATE SET status = 'completed', exit_code = excluded.exit_code, duration_ms = excluded.duration_ms, completed_at = excluded.completed_at, updated_at = excluded.updated_at;",
                (task_id, session_id, *exit_code as i64, *duration_ms as i64, *completed_at as i64, now_ts),
            ).await?;
        }
        TaskStatus::Failed {
            error,
            duration_ms,
            failed_at,
        } => {
            conn.execute(
                "INSERT INTO session_tasks (task_id, session_id, command, status, error, duration_ms, completed_at, log_path, started_at, created_at, updated_at)
                 VALUES (?1, ?2, 'background task', 'failed', ?3, ?4, ?5, '', ?5, ?5, ?6)
                 ON CONFLICT(task_id) DO UPDATE SET status = 'failed', error = excluded.error, duration_ms = excluded.duration_ms, completed_at = excluded.completed_at, updated_at = excluded.updated_at;",
                (task_id, session_id, error.as_str(), *duration_ms as i64, *failed_at as i64, now_ts),
            ).await?;
        }
        TaskStatus::Killed { killed_at } => {
            conn.execute(
                "INSERT INTO session_tasks (task_id, session_id, command, status, completed_at, log_path, started_at, created_at, updated_at)
                 VALUES (?1, ?2, 'background task', 'killed', ?3, '', ?3, ?3, ?4)
                 ON CONFLICT(task_id) DO UPDATE SET status = 'killed', completed_at = excluded.completed_at, updated_at = excluded.updated_at;",
                (task_id, session_id, *killed_at as i64, now_ts),
            ).await?;
        }
        _ => {}
    }

    Ok(())
}

/// Fetches a single persisted task, scoped to a session, for terminal status read-through.
pub async fn fetch_task_by_id(
    conn: &Connection,
    task_id: &str,
    session_id: i64,
) -> Result<Option<TaskDto>, PersistenceError> {
    let mut rows = conn
        .query(
            "SELECT task_id, session_id, command, status, exit_code, error,
                    log_path, started_at, completed_at, duration_ms
             FROM session_tasks
             WHERE task_id = ? AND session_id = ?",
            (task_id.to_string(), session_id),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        let status: String = row.get(3)?;
        Ok(Some(TaskDto {
            task_id: row.get(0)?,
            session_id: row.get(1)?,
            command: row.get(2)?,
            status,
            exit_code: row.get(4).ok(),
            error: row.get(5).ok(),
            log_path: row.get(6)?,
            started_at: row.get(7).unwrap_or(0),
            completed_at: row.get(8).ok(),
            duration_ms: row.get(9).ok(),
        }))
    } else {
        Ok(None)
    }
}

/// Marks a specific task as `killed`.
pub async fn mark_task_killed(conn: &Connection, task_id: &str) -> Result<(), PersistenceError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "UPDATE session_tasks SET status = 'killed', completed_at = ?, updated_at = ? WHERE task_id = ?;",
        (now, now, task_id),
    )
    .await?;

    Ok(())
}

/// Marks all active tasks for a session as `killed`.
pub async fn mark_session_tasks_killed(
    conn: &Connection,
    session_id: i64,
) -> Result<(), PersistenceError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "UPDATE session_tasks SET status = 'killed', completed_at = ?, updated_at = ? WHERE session_id = ? AND status = 'running';",
        (now, now, session_id),
    )
    .await?;

    Ok(())
}
