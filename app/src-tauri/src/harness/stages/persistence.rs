use std::time::Duration;

use crossbeam_channel::Sender;

use crate::{
    core::error::PersistenceError,
    harness::{current_timestamp_ms, tools::ToolExecutionOutcome},
    persistence::PersistenceEvent,
};

pub type PersistenceSender = Sender<PersistenceEvent>;

const PERSIST_SEND_TIMEOUT: Duration = Duration::from_secs(2);

fn channel_error(turn_id: u32) -> PersistenceError {
    PersistenceError::ChannelUnavailable {
        message: format!(
            "persistence channel unavailable while recording turn {}",
            turn_id
        ),
    }
}

/// Persists parent StepCompleted record followed by child ToolCallExecuted records in strict FIFO order.
#[allow(clippy::too_many_arguments)]
pub fn persist_step_and_tools(
    persist_tx: Option<&Sender<PersistenceEvent>>,
    session_id: i64,
    turn_id: u32,
    step_id: String,
    step_index: u32,
    preamble_text: Option<String>,
    outcomes: &[ToolExecutionOutcome],
    duration_ms: u64,
    prompt_tokens: u32,
    completion_tokens: u32,
    strike_count: u32,
    is_compacted: bool,
) -> Result<(), PersistenceError> {
    let tx = persist_tx.ok_or_else(|| channel_error(turn_id))?;
    let now = current_timestamp_ms();

    // 1. Parent must be accepted before any child is queued (FK guarantee).
    tx.send_timeout(
        PersistenceEvent::StepCompleted {
            id: step_id.clone(),
            session_id,
            turn_id,
            step_index,
            preamble_text,
            tool_count: outcomes.len() as u32,
            duration_ms: Some(duration_ms),
            prompt_tokens: Some(prompt_tokens),
            completion_tokens: Some(completion_tokens),
            guardrail_strike: strike_count,
            is_compacted,
            created_at: now,
        },
        PERSIST_SEND_TIMEOUT,
    )
    .map_err(|_| channel_error(turn_id))?;

    // 2. Dispatch child tool call records with a valid parent row.
    for outcome in outcomes {
        tx.send_timeout(
            PersistenceEvent::ToolCallExecuted {
                id: outcome.call_id.clone(),
                session_id,
                turn_id,
                step_id: Some(step_id.clone()),
                step_index,
                tool_name: outcome.tool_name.clone(),
                arguments: outcome.arguments.clone(),
                result: outcome.result.content.clone(),
                is_error: outcome.is_error,
                duration_ms: outcome.duration_ms,
                truncated: outcome.truncated,
                spillover_path: outcome.spillover_path.clone(),
                created_at: now,
            },
            PERSIST_SEND_TIMEOUT,
        )
        .map_err(|_| channel_error(turn_id))?;
    }
    Ok(())
}

/// Persists interrupted or cancelled turn when partial assistant text is present.
pub fn persist_cancelled_turn(
    persist_tx: Option<&Sender<PersistenceEvent>>,
    session_id: i64,
    turn_id: u32,
    user_text: String,
    partial_text: String,
) -> Result<(), PersistenceError> {
    let tx = persist_tx.ok_or_else(|| channel_error(turn_id))?;
    tx.send_timeout(
        PersistenceEvent::TurnCompleted {
            session_id,
            turn_id,
            user_text,
            assistant_text: partial_text,
        },
        PERSIST_SEND_TIMEOUT,
    )
    .map_err(|_| channel_error(turn_id))
}
