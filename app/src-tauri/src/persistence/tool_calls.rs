use turso::Connection;

use crate::persistence::{schema::Result, sessions::ensure_session_exists, PersistenceEvent};

/// Persists a tool execution record into `session_tool_calls` with self-healing session parent insertion.
pub async fn persist_tool_call(conn: &Connection, event: &PersistenceEvent) -> Result<()> {
    let PersistenceEvent::ToolCallExecuted {
        id,
        session_id,
        turn_id,
        step_id,
        step_index,
        tool_name,
        arguments,
        result,
        is_error,
        duration_ms,
        truncated,
        spillover_path,
        created_at,
    } = event
    else {
        return Ok(());
    };

    ensure_session_exists(conn, *session_id).await?;

    let args_str = serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string());
    let is_error_int = if *is_error { 1i64 } else { 0i64 };
    let truncated_int = if *truncated { 1i64 } else { 0i64 };
    let created_at_ms = *created_at as i64;

    conn.execute(
        "INSERT INTO session_tool_calls (id, session_id, turn_id, step_id, step_index, tool_name, arguments, result, is_error, duration_ms, truncated, spillover_path, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        (
            id.clone(),
            *session_id,
            *turn_id as i64,
            step_id.clone(),
            *step_index as i64,
            tool_name.clone(),
            args_str,
            result.clone(),
            is_error_int,
            *duration_ms as i64,
            truncated_int,
            spillover_path.clone(),
            created_at_ms,
        ),
    )
    .await?;

    log::info!(
        "[Persistence::ToolCalls] Persisted tool call {} ({}) for session {} turn {}",
        id,
        tool_name,
        session_id,
        turn_id
    );

    Ok(())
}
