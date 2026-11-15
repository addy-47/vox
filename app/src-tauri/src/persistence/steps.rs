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
        prompt_tokens,
        completion_tokens,
        guardrail_strike,
        is_compacted,
        created_at,
    } = event
    else {
        return Ok(());
    };

    ensure_session_exists(conn, *session_id).await?;

    let dur = duration_ms.map(|d| d as i64);
    let p_tok = prompt_tokens.map(|t| t as i64);
    let c_tok = completion_tokens.map(|t| t as i64);
    let strike = *guardrail_strike as i64;
    let compacted = if *is_compacted { 1i64 } else { 0i64 };
    let created_at_ms = *created_at as i64;

    conn.execute(
        "INSERT INTO session_steps (id, session_id, turn_id, step_index, preamble_text, tool_count, duration_ms, prompt_tokens, completion_tokens, guardrail_strike, is_compacted, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        (
            id.clone(),
            *session_id,
            *turn_id as i64,
            *step_index as i64,
            preamble_text.clone(),
            *tool_count as i64,
            dur,
            p_tok,
            c_tok,
            strike,
            compacted,
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
