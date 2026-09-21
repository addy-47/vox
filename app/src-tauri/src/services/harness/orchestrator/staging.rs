use std::sync::Arc;

use parking_lot::Mutex;

use crate::services::harness::stages::{
    budget::ContextStatus,
    streaming::StreamRoutingStage,
};

use super::{Harness, TurnExecutionRequest, TurnOutcome};

/// Outcome of staging and token budget evaluation.
pub enum IntakeResult {
    Proceed {
        can_compact: bool,
        stream_stage: StreamRoutingStage,
    },
    Terminal(TurnOutcome),
}

/// Evaluates Phase 1 (Intake & Deduplication) and Phase 2 (Staging & Budget Evaluation).
pub fn stage_turn_intake<R: tauri::Runtime>(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: &TurnExecutionRequest<R>,
) -> IntakeResult {
    let turn_id = req.turn_id;

    if req.cancel.is_cancelled() {
        log::info!("[Harness::Staging] Turn {} pre-cancelled before staging", turn_id);
        return IntakeResult::Terminal(TurnOutcome::Cancelled { turn_id });
    }

    let mut guard = harness_arc.lock();
    let Some(ref mut harness) = *guard else {
        return IntakeResult::Terminal(TurnOutcome::Error {
            turn_id,
            message: "No active Harness mounted".to_string(),
        });
    };

    if harness.history.is_duplicate_user_turn(&req.query) {
        log::info!(
            "[Harness::Staging] Dropping duplicate user turn {} ('{}')",
            turn_id,
            req.query
        );
        return IntakeResult::Terminal(TurnOutcome::DuplicateIgnored { turn_id });
    }

    harness.history.push_user_turn(req.query.clone());

    let assembled_system = harness.prompt.assemble();
    harness.history.sync_system_prompt(&assembled_system);

    let can_compact = evaluate_compaction_eligibility(harness, turn_id);

    IntakeResult::Proceed {
        can_compact,
        stream_stage: harness.stream.clone(),
    }
}

/// Evaluates context utilization to determine whether in-turn compaction is triggered.
fn evaluate_compaction_eligibility(harness: &mut Harness, turn_id: u32) -> bool {
    let Some(ref budget) = harness.budget else {
        return false;
    };
    let tracked_tokens = budget.calculate_tracked_tokens(harness.history.messages());
    let (utilization, status) = budget.evaluate_utilization(tracked_tokens);
    log::info!(
        "[Harness::Staging] Turn {} context utilization: {:.1}% ({:?})",
        turn_id,
        utilization * 100.0,
        status
    );

    if status != ContextStatus::Critical {
        return false;
    }

    let eligible = harness
        .compaction
        .as_ref()
        .map(|c| c.can_perform_inline_compaction(harness.history.messages().len()))
        .unwrap_or(false);

    if !eligible {
        log::warn!("[Harness::Staging] Compaction ineligible. Executing degraded FIFO shift.");
        budget.execute_fifo_shift(&mut harness.history);
        false
    } else {
        true
    }
}
