use std::{sync::Arc, time::Duration};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use super::{plugins::budget::ContextStatus, session::HarnessSession};
use crate::{
    core::state::{AppState, InteractionState},
    services::memory::compaction::runner::run_compaction,
};

pub const QUIET_COMPACTION_DEBOUNCE_SECS: u64 = 20;

/// Session-scoped reactive quiet watcher arming a 20-second debounce timer during soft context warning.
#[derive(Default)]
pub struct QuietCompactionWatcher {
    active_cancel: Option<CancellationToken>,
}

impl QuietCompactionWatcher {
    pub fn new() -> Self {
        Self {
            active_cancel: None,
        }
    }

    pub fn abort(&mut self) {
        if let Some(token) = self.active_cancel.take() {
            token.cancel();
            log::info!("[Harness::Watcher] Quiet debounce timer aborted.");
        }
    }

    pub fn on_turn_completed(
        &mut self,
        state: Arc<AppState>,
        harness_lock: Arc<Mutex<Option<HarnessSession>>>,
    ) {
        self.abort();

        let (should_arm, tracked_turns) = {
            let guard = harness_lock.lock();
            let Some(ref harness) = *guard else {
                return;
            };

            let Some(ref budget) = harness.budget else {
                return;
            };

            let Some(ref compaction) = harness.compaction else {
                return;
            };

            if !compaction.auto_compaction_enabled() {
                return;
            }

            let tracked = budget.calculate_tracked_tokens(harness.history.messages());
            let (_, status) = budget.evaluate_utilization(tracked);

            (
                status == ContextStatus::SoftWarning,
                harness.history.messages().to_vec(),
            )
        };

        if !should_arm || tracked_turns.len() < 4 {
            return;
        }

        log::info!(
            "[Harness::Watcher] Soft warning (65%-85%) detected. Arming {}s quiet debounce timer.",
            QUIET_COMPACTION_DEBOUNCE_SECS
        );

        let cancel = CancellationToken::new();
        self.active_cancel = Some(cancel.clone());

        let state_clone = Arc::clone(&state);
        let harness_clone = Arc::clone(&harness_lock);

        tauri::async_runtime::spawn(async move {
            let debounce = Duration::from_secs(QUIET_COMPACTION_DEBOUNCE_SECS);
            let mut state_rx = state_clone.pipeline.state_rx.clone();

            tokio::select! {
                _ = cancel.cancelled() => {
                    log::info!("[Harness::Watcher] Debounce timer cancelled.");
                }
                _ = tokio::time::sleep(debounce) => {
                    let current_state = state_clone.pipeline.state();
                    if current_state != InteractionState::Ready && current_state != InteractionState::Paused {
                        log::info!("[Harness::Watcher] Pipeline left Ready/Paused. Aborting quiet compaction.");
                        return;
                    }

                    log::info!("[Harness::Watcher] 20s quiet window expired. Triggering background soft compaction.");
                    execute_soft_compaction(&state_clone, &harness_clone, &tracked_turns, &cancel).await;
                }
                res = state_rx.changed() => {
                    if res.is_ok() {
                        let current = *state_rx.borrow();
                        if current != InteractionState::Ready && current != InteractionState::Paused {
                            log::info!("[Harness::Watcher] Pipeline state changed to {:?}. Aborting quiet compaction.", current);
                        }
                    }
                }
            }
        });
    }
}

async fn execute_soft_compaction(
    state: &AppState,
    harness_lock: &Mutex<Option<HarnessSession>>,
    history_slice: &[super::ChatMessage],
    cancel: &CancellationToken,
) {
    let provider_opt = state.llm_provider.read().clone();
    let Some(provider) = provider_opt else {
        log::warn!("[Harness::Watcher] No active LLM provider available for quiet compaction.");
        return;
    };

    let settings = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .llm
        .clone();

    let result = match run_compaction(
        provider.as_ref(),
        history_slice,
        Some(&settings),
        Some(cancel),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            log::warn!(
                "[Harness::Watcher] Background soft compaction failed: {}",
                e
            );
            return;
        }
    };

    if !result.context_summary.trim().is_empty() {
        let mut guard = harness_lock.lock();
        if let Some(ref mut harness) = *guard {
            if let Some(ref mut compaction) = harness.compaction {
                compaction.set_rolling_summary(Some(result.context_summary));
                log::info!("[Harness::Watcher] Background soft compaction completed and rolling summary stored.");
            }
        }
    }
}
