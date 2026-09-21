use turso::Connection;

use crate::{
    persistence::{schema::Result, PersistenceEvent,sessions::ensure_session_exists},
    services::llm::ToolFlow,
};

/// Persists a tool execution record into `session_tool_calls` with self-healing session parent insertion.
pub async fn persist_tool_call(conn: &Connection, event: &PersistenceEvent) -> Result<()> {
    let PersistenceEvent::ToolCallExecuted {
        id,
        session_id,
        turn_id,
        tool_name,
        tool_flow,
        arguments,
        result,
        is_error,
        duration_ms,
        created_at,
    } = event
    else {
        return Ok(());
    };

    ensure_session_exists(conn, *session_id).await?;

    let tool_kind = match tool_flow {
        ToolFlow::Terminal => "terminal",
        ToolFlow::NonTerminal => "non_terminal",
    };

    let args_str = serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string());
    let is_error_int = if *is_error { 1i64 } else { 0i64 };
    let created_at_ms = *created_at as i64;

    conn.execute(
        "INSERT INTO session_tool_calls (id, session_id, turn_id, tool_name, tool_kind, arguments, result, is_error, duration_ms, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        (
            id.clone(),
            *session_id,
            *turn_id as i64,
            tool_name.clone(),
            tool_kind,
            args_str,
            result.clone(),
            is_error_int,
            *duration_ms as i64,
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
