use turso::Connection;

use crate::{
    core::error::PersistenceError,
    persistence::{sessions::ensure_session_exists, PersistenceEvent},
};

/// Persists a cognitive step record into `session_steps`.
pub async fn persist_step(
    conn: &Connection,
    event: &PersistenceEvent,
) -> Result<(), PersistenceError> {
    let PersistenceEvent::StepCompleted {
        id,
        session_id,
        turn_id,
        step_index,
        preamble_text,
        tool_count,
        duration_ms,
        prompt_tokens: _,
        completion_tokens: _,
        guardrail_strike: _,
        is_compacted: _,
        created_at,
    } = event
    else {
        return Ok(());
    };

    ensure_session_exists(conn, *session_id).await?;

    let dur = duration_ms.map(|d| d as i64);
    let created_at_ms = *created_at as i64;

    conn.execute(
        "INSERT INTO session_steps (id, session_id, turn_id, step_index, preamble_text, tool_count, duration_ms, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?);",
        (
            id.clone(),
            *session_id,
            *turn_id as i64,
            *step_index as i64,
            preamble_text.clone(),
            *tool_count as i64,
            dur,
            created_at_ms,
        ),
    )
    .await?;

    log::info!(
        "[Persistence::Steps] Persisted session_step {} for session {} turn {} step {}",
        id,
        session_id,
        turn_id,
        step_index
    );

    Ok(())
}
