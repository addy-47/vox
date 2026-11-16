use crossbeam_channel::Sender;

use crate::{
    persistence::PersistenceEvent,
    services::harness::{current_timestamp_ms, tools::ToolExecutionOutcome},
};

pub type PersistenceSender = Sender<PersistenceEvent>;

/// Persists parent StepCompleted record followed by child ToolCallExecuted records in strict FIFO order.
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
) {
    let Some(tx) = persist_tx else {
        return;
    };
    let now = current_timestamp_ms();

    // 1. Dispatch parent StepCompleted record
    let _ = tx.try_send(PersistenceEvent::StepCompleted {
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
    });

    // 2. Dispatch child tool call records with valid foreign key
    for outcome in outcomes {
        let _ = tx.try_send(PersistenceEvent::ToolCallExecuted {
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
        });
    }
}

/// Persists interrupted or cancelled turn when partial assistant text is present.
pub fn persist_cancelled_turn(
    persist_tx: Option<&Sender<PersistenceEvent>>,
    session_id: i64,
    turn_id: u32,
    user_text: String,
    partial_text: String,
) {
    let Some(tx) = persist_tx else {
        return;
    };
    if let Err(e) = tx.try_send(PersistenceEvent::TurnCompleted {
        session_id,
        turn_id,
        user_text,
        assistant_text: partial_text,
    }) {
        log::warn!(
            "[Harness::Finalize] Failed to persist interrupted turn {}: {}",
            turn_id,
            e
        );
    }
}
