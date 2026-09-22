use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use anyhow::Result;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use turso::Connection;

use crate::{
    core::{
        settings::LlmSettings,
        state::{AppState, InteractionState},
    },
    persistence::compactions::{
        commit_compaction_output, record_compaction_finish, record_compaction_start,
    },
    services::{
        harness::{ChatMessage, Harness, PromptTag, Role},
        llm::LlmProvider,
        memory::compaction::runner::{run_compaction, CompactionResult},
    },
};

pub const MIN_MESSAGES_FOR_COMPACTION: usize = 4;
pub const QUIET_COMPACTION_DEBOUNCE_SECS: u64 = 20;

/// Bundled parameters for executing and persisting compaction passes.
pub struct CompactionParams<'a> {
    pub session_id: i64,
    pub trigger_kind: &'a str,
    pub from_turn_id: u32,
    pub to_turn_id: u32,
    pub history_messages: &'a [ChatMessage],
    pub llm_settings: Option<&'a LlmSettings>,
    pub cancel: Option<&'a CancellationToken>,
}

/// Stage managing conversational memory compaction and session context tracking.
#[derive(Debug, Clone)]
pub struct CompactionStage {
    session_context: Option<String>,
    auto_compaction_enabled: bool,
    context_window: usize,
    is_embedded: bool,
    last_compacted_to_turn: u32,
}

/// Session-scoped reactive quiet watcher arming a 20-second debounce timer during soft context warning.
#[derive(Default)]
pub struct QuietCompactionWatcher {
    active_cancel: Option<CancellationToken>,
}

impl CompactionStage {
    /// Creates a new `CompactionStage` instance.
    pub fn new(context_window: usize, is_embedded: bool, auto_compaction_enabled: bool) -> Self {
        Self {
            session_context: None,
            auto_compaction_enabled,
            context_window,
            is_embedded,
            last_compacted_to_turn: 0,
        }
    }

    pub fn auto_compaction_enabled(&self) -> bool {
        self.auto_compaction_enabled
    }

    pub fn context_window(&self) -> usize {
        self.context_window
    }

    pub fn is_embedded(&self) -> bool {
        self.is_embedded
    }

    pub fn from_turn_id(&self) -> u32 {
        self.last_compacted_to_turn.saturating_add(1)
    }

    pub fn set_last_compacted_to_turn(&mut self, turn_id: u32) {
        self.last_compacted_to_turn = turn_id;
    }

    pub fn session_context(&self) -> Option<&str> {
        self.session_context.as_deref()
    }

    pub fn set_session_context(&mut self, context: Option<String>) {
        self.session_context = context;
    }

    pub fn apply_session_context(&mut self, context: &str) {
        if !context.trim().is_empty() {
            self.session_context = Some(context.to_string());
        }
    }

    pub fn can_perform_inline_compaction(&self, history_len: usize) -> bool {
        self.auto_compaction_enabled && history_len >= MIN_MESSAGES_FOR_COMPACTION
    }

    /// Executes compaction across the LLM provider, records the run in the Turso DB ledger,
    /// commits extracted facts to the ingestion queue, and returns the result.
    pub async fn run_and_persist(
        provider: &dyn LlmProvider,
        conn: &Connection,
        params: CompactionParams<'_>,
    ) -> Result<CompactionResult> {
        log::info!(
            "[Harness::Compaction] Running {} compaction for session {} ({} messages)",
            params.trigger_kind,
            params.session_id,
            params.history_messages.len()
        );

        let run_id = if params.session_id > 0 {
            record_compaction_start(
                conn,
                params.session_id,
                params.trigger_kind,
                params.from_turn_id,
                params.to_turn_id,
            )
            .await
            .ok()
        } else {
            None
        };

        let result = match run_compaction(
            provider,
            params.history_messages,
            params.llm_settings,
            params.cancel,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                if let Some(id) = run_id {
                    let err_str = e.to_string();
                    let _ = record_compaction_finish(conn, id, "", "failed", Some(&err_str)).await;
                }
                return Err(e);
            }
        };

        if let Some(id) = run_id {
            if let Err(e) = commit_compaction_output(
                conn,
                id,
                &result.raw_json,
                &result.facts,
                params.session_id,
            )
            .await
            {
                log::warn!(
                    "[Harness::Compaction] Failed to commit compaction output to DB: {}",
                    e
                );
            } else {
                log::info!(
                    "[Harness::Compaction] Successfully committed compaction run {} with {} facts",
                    id,
                    result.facts.len()
                );
            }
        }

        Ok(result)
    }

    /// Prunes conversation history messages after compaction, updating the root system prompt.
    pub fn prune_history_with_summary(
        &self,
        base_system_prompt: &str,
        user_turn: &str,
    ) -> Vec<ChatMessage> {
        let mut pruned = Vec::new();

        let system_content = if let Some(ref context) = self.session_context {
            let wrapped_context = PromptTag::SessionContext.wrap(context);
            format!("{}\n\n{}", base_system_prompt.trim(), wrapped_context)
        } else {
            base_system_prompt.to_string()
        };

        pruned.push(ChatMessage::new(Role::System, system_content));

        if !user_turn.trim().is_empty() {
            pruned.push(ChatMessage::new(Role::User, user_turn.to_string()));
        }

        pruned
    }
}

impl QuietCompactionWatcher {
    /// Constructs a new `QuietCompactionWatcher` instance.
    pub fn new() -> Self {
        Self {
            active_cancel: None,
        }
    }

    /// Cancels any currently pending quiet debounce timer.
    pub fn abort(&mut self) {
        if let Some(token) = self.active_cancel.take() {
            token.cancel();
            log::info!("[Harness::Watcher] Quiet debounce timer aborted.");
        }
    }

    /// Arms a 20-second debounce timer upon turn completion if soft warning was triggered.
    pub fn on_turn_completed(
        &mut self,
        state: Arc<AppState>,
        harness_lock: Arc<Mutex<Option<Harness>>>,
        tracked_turns: Option<Vec<ChatMessage>>,
    ) {
        self.abort();

        let Some(tracked_turns) = tracked_turns else {
            return;
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
            let deadline =
                tokio::time::Instant::now() + Duration::from_secs(QUIET_COMPACTION_DEBOUNCE_SECS);
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

/// Executes background soft compaction asynchronously without blocking active conversational turns.
async fn execute_soft_compaction(
    state: &AppState,
    harness_lock: &Mutex<Option<Harness>>,
    history_slice: &[ChatMessage],
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

    let conn = match state.db.connect() {
        Ok(c) => c,
        Err(e) => {
            monitor_handle.abort();
            log::warn!(
                "[Harness::Watcher] Failed to vend connection for soft compaction: {}",
                e
            );
            return;
        }
    };

    let result = match CompactionStage::run_and_persist(provider.as_ref(), &conn, params).await {
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
