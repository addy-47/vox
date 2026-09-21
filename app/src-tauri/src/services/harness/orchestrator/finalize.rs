use std::sync::Arc;

use parking_lot::Mutex;

use super::{Harness, TurnOutcome};

/// Handles Phase 7 Branch B: turn cancellation / user barge-in.
pub fn handle_turn_cancelled(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    turn_id: u32,
) -> TurnOutcome {
    log::info!("[Harness::Finalize] Finalizing cancelled turn {}", turn_id);

    let mut guard = harness_arc.lock();
    if let Some(ref mut harness) = *guard {
        harness.history.rollback_last_user_turn();
    }

    TurnOutcome::Cancelled { turn_id }
}

/// Handles Phase 7 Branch A: turn error and state rollback.
pub fn handle_turn_error(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    turn_id: u32,
    message: String,
) -> TurnOutcome {
    log::error!("[Harness::Finalize] Turn {} failed: {}", turn_id, message);

    let mut guard = harness_arc.lock();
    if let Some(ref mut harness) = *guard {
        harness.history.rollback_last_user_turn();
    }

    TurnOutcome::Error { turn_id, message }
}

/// Handles Phase 7 Branch C: successful turn completion and history commit.
pub fn commit_completed_turn(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    turn_id: u32,
    assistant_response: String,
) -> TurnOutcome {
    log::info!(
        "[Harness::Finalize] Committing turn {} ({} chars)",
        turn_id,
        assistant_response.len()
    );

    let mut guard = harness_arc.lock();
    if let Some(ref mut harness) = *guard {
        harness.history.push_assistant_turn(assistant_response.clone());
    }

    TurnOutcome::Completed {
        turn_id,
        assistant_response,
    }
}
