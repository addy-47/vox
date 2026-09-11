use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use super::{
    plugins::compaction::{CompactionParams, CompactionPlugin},
    session::HarnessSession,
};
use crate::core::state::{AppState, InteractionState};

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

        let tracked_turns = {
            let guard = harness_lock.lock();
            let Some(ref harness) = *guard else {
                return;
            };
            let Some(turns) = harness.check_quiet_compaction_eligibility() else {
                return;
            };
            turns
        };

        log::info!(
            "[Harness::Watcher] Soft warning (65%-85%) detected. Arming {}s quiet debounce timer.",
            QUIET_COMPACTION_DEBOUNCE_SECS
        );

        let cancel = CancellationToken::new();
        self.active_cancel = Some(cancel.clone());

        let state_clone = Arc::clone(&state);
        let harness_clone = Arc::clone(&harness_lock);

        tauri::async_runtime::spawn(async move {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(QUIET_COMPACTION_DEBOUNCE_SECS);
            let mut debounce_timer = Box::pin(tokio::time::sleep_until(deadline));
            let mut state_rx = state_clone.pipeline.state_rx.clone();

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        log::info!("[Harness::Watcher] Debounce timer cancelled.");
                        return;
                    }
                    _ = &mut debounce_timer => {
                        let current_state = state_clone.pipeline.state();
                        if current_state != InteractionState::Ready && current_state != InteractionState::Paused {
                            log::info!("[Harness::Watcher] Pipeline left Ready/Paused. Aborting quiet compaction.");
                            return;
                        }

                        log::info!("[Harness::Watcher] 20s quiet window expired. Triggering background soft compaction.");
                        execute_soft_compaction(&state_clone, &harness_clone, &tracked_turns, &cancel).await;
                        return;
                    }
                    res = state_rx.changed() => {
                        if res.is_ok() {
                            let current = *state_rx.borrow();
                            if current != InteractionState::Ready && current != InteractionState::Paused {
                                log::info!("[Harness::Watcher] Pipeline state changed to {:?}. Aborting quiet compaction.", current);
                                return;
                            }
                        } else {
                            return;
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

    let session_id = state.conversation_id.load(Ordering::Relaxed) as i64;
    let from_turn = {
        let guard = harness_lock.lock();
        guard.as_ref().map(|h| h.from_turn_id()).unwrap_or(0)
    };
    let to_turn = state.pipeline.peek_turn_id();

    let params = CompactionParams {
        session_id,
        trigger_kind: "soft",
        from_turn_id: from_turn,
        to_turn_id: to_turn,
        history_messages: history_slice,
        llm_settings: Some(&settings),
        cancel: Some(cancel),
    };

    let compactor_cancel = cancel.clone();
    let mut state_rx = state.pipeline.state_rx.clone();
    let monitor_handle = tauri::async_runtime::spawn(async move {
        while state_rx.changed().await.is_ok() {
            let current = *state_rx.borrow();
            if current != InteractionState::Ready && current != InteractionState::Paused {
                log::info!(
                    "[Harness::Watcher] Pipeline transitioned to {:?} during soft compaction. Cancelling in-flight inference.",
                    current
                );
                compactor_cancel.cancel();
                break;
            }
        }
    });

    let result = match CompactionPlugin::run_and_persist(
        provider.as_ref(),
        &state.db,
        params,
    )
    .await
    {
        Ok(r) => {
            monitor_handle.abort();
            r
        }
        Err(e) => {
            monitor_handle.abort();
            log::warn!(
                "[Harness::Watcher] Background soft compaction failed: {}",
                e
            );
            return;
        }
    };

    if !result.session_context.trim().is_empty() {
        let mut guard = harness_lock.lock();
        if let Some(ref mut harness) = *guard {
            harness.apply_quiet_compaction_summary(&result.session_context);
            harness.set_last_compacted_to_turn(to_turn);
            log::info!("[Harness::Watcher] Background soft compaction completed, rolling summary stored, and history pruned.");
        }
    }
}
