use std::sync::{atomic::Ordering::Relaxed, Arc};

use parking_lot::Mutex;

use crate::services::harness::stages::compaction::{CompactionParams, CompactionStage};

use super::{
    phase::{enter_non_terminal_phase, NonTerminalContext, NonTerminalPhase},
    Harness, TurnExecutionRequest,
};

/// Orchestrates Phase 3 (Inline Compaction / Maintenance) within the turn lifecycle.
pub async fn execute_inline_compaction<R: tauri::Runtime + 'static>(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: &TurnExecutionRequest<R>,
) {
    let non_terminal = NonTerminalPhase::compaction(None);
    let ctx = NonTerminalContext {
        query: &req.query,
        turn_id: req.turn_id,
        routing_ctx: &req.routing_ctx,
        app: &req.app,
        app_state: &req.app_state,
        tts_tx: req.tts_tx.as_ref(),
        pending_synthesis_jobs: &req.pending_synthesis_jobs,
    };
    enter_non_terminal_phase(&non_terminal, &ctx);

    let Some(provider) = req.provider.as_ref() else {
        log::warn!(
            "[Harness::Compaction] Critical context threshold on turn {} without LLM provider; FIFO fallback.",
            req.turn_id
        );
        let mut guard = harness_arc.lock();
        if let Some(ref mut harness) = *guard {
            harness.fallback_fifo_shift();
        }
        return;
    };

    let compactor_cancel = req.cancel.clone();
    let session_id = req.app_state.conversation_id.load(Relaxed) as i64;
    let (from_turn, history_slice) = {
        let guard = harness_arc.lock();
        guard
            .as_ref()
            .map(|h| (h.from_turn_id(), h.history.messages().to_vec()))
            .unwrap_or((0, Vec::new()))
    };
    let to_turn = req.turn_id;
    let provider = provider.clone();
    let llm_settings = req.app_state.settings.read().ok().map(|s| s.llm.clone());
    let db = req.db.clone();
    let handle = tokio::runtime::Handle::current();

    let compaction_res = tokio::task::spawn_blocking(move || {
        let conn = db.connect().map_err(|e| {
            anyhow::anyhow!("Failed to connect to db for compaction: {}", e)
        })?;
        handle.block_on(async {
            let params = CompactionParams {
                session_id,
                trigger_kind: "inline",
                from_turn_id: from_turn,
                to_turn_id: to_turn,
                history_messages: &history_slice,
                llm_settings: llm_settings.as_ref(),
                cancel: Some(&compactor_cancel),
            };
            CompactionStage::run_and_persist(provider.as_ref(), &conn, params).await
        })
    })
    .await
    .unwrap_or_else(|join_err| {
        Err(anyhow::anyhow!("Compaction task panic: {:?}", join_err))
    });

    apply_compaction_result(harness_arc, compaction_res, to_turn, &req.query);
}

/// Applies compaction outcomes to harness history or executes degraded FIFO shift on failure.
fn apply_compaction_result(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    compaction_res: anyhow::Result<crate::services::memory::compaction::CompactionResult>,
    to_turn: u32,
    query: &str,
) {
    let mut guard = harness_arc.lock();
    let Some(ref mut harness) = *guard else {
        return;
    };
    match compaction_res {
        Ok(result) => {
            if !result.session_context.trim().is_empty() {
                harness.apply_session_context(&result.session_context, query);
                harness.set_last_compacted_to_turn(to_turn);
                log::info!("[Harness::Compaction] Inline compaction succeeded; history refreshed.");
            } else {
                log::warn!("[Harness::Compaction] Inline compaction empty; degraded FIFO shift.");
                harness.fallback_fifo_shift();
            }
        }
        Err(err) => {
            log::warn!(
                "[Harness::Compaction] Inline compaction failed (0-retry): {}. FIFO fallback.",
                err
            );
            harness.fallback_fifo_shift();
        }
    }
}
