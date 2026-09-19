use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering::Relaxed},
    mpsc, Arc,
};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use super::{
    stages::{
        budget::{ContextBudgetStage, ContextStatus},
        compaction::{CompactionParams, CompactionStage, QuietCompactionWatcher},
        history::ConversationHistoryStage,
        prompt::PromptBuilderStage,
        streaming::{StreamRoutingHandles, StreamRoutingStage},
    },
    ChatMessage, Role,
};
use crate::{
    core::{
        events::{AudioIntent, VoxEvent},
        settings::{LlmProviderConfig, VoxSettings},
        state::{AppState, InteractionOwner, InteractionState},
    },
    persistence::{db::VoxDb, TurnRow},
    pipeline::{
        assistant::accumulator::TurnAccumulator,
        router::{transition, RoutingContext},
    },
    services::{
        llm::{
            actor::LlmCommand, ConversationInput, GenerationOptions, GenerationPurpose,
            GenerationRequest, LlmProvider, OutputConstraint, ReasoningMode,
        },
        tts::actor::TtsCommand,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineDomain {
    ModularAssistant,
    RealtimeS2S,
    Dictation,
}

/// Strongly typed terminal outcome of executing a conversational turn.
#[derive(Debug)]
pub enum TurnOutcome {
    Completed {
        turn_id: u32,
        assistant_response: String,
    },
    DuplicateIgnored {
        turn_id: u32,
    },
    Cancelled {
        turn_id: u32,
    },
    Error {
        turn_id: u32,
        message: String,
    },
}

/// Bundled parameters and subsystem handles for executing a turn through the harness.
pub struct TurnExecutionRequest<R: tauri::Runtime> {
    pub query: String,
    pub turn_id: u32,
    pub cancel: CancellationToken,
    pub owner: InteractionOwner,
    pub llm_tx: Option<mpsc::Sender<LlmCommand>>,
    pub tts_tx: Option<mpsc::Sender<TtsCommand>>,
    pub provider: Option<Arc<dyn LlmProvider>>,
    pub db: Arc<VoxDb>,
    pub pipeline_tx: Option<mpsc::Sender<VoxEvent>>,
    pub accumulator: Arc<Mutex<TurnAccumulator>>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub app: tauri::AppHandle<R>,
    pub routing_ctx: RoutingContext,
    pub app_state: Arc<AppState>,
}

/// The session-scoped conversational orchestrator and stage chassis.
pub struct Harness {
    session_id: Option<i64>,
    domain: PipelineDomain,
    cancel_token: CancellationToken,
    llm_tx: Option<mpsc::Sender<LlmCommand>>,
    history: ConversationHistoryStage,
    prompt: PromptBuilderStage,
    budget: Option<ContextBudgetStage>,
    compaction: Option<CompactionStage>,
    stream: StreamRoutingStage,
    watcher: Option<QuietCompactionWatcher>,
    generation_options: GenerationOptions,
}

impl Harness {
    pub fn new_modular(
        session_id: Option<i64>,
        base_prompt: String,
        personal_memory: Option<String>,
        settings: &VoxSettings,
        llm_tx: mpsc::Sender<LlmCommand>,
    ) -> Self {
        let is_cloud = matches!(
            settings.llm.to_provider_config(),
            LlmProviderConfig::OpenAiCompat { .. }
        );
        let is_embedded = !is_cloud;
        let ctx_window = settings.llm.context_window as usize;
        let max_share = settings.working_memory.max_context_share;

        let mut prompt_stage = PromptBuilderStage::new(base_prompt, ctx_window, max_share);
        prompt_stage.set_personal_memory(personal_memory);
        let assembled_prompt = prompt_stage.assemble();

        let history_stage = ConversationHistoryStage::with_system_prompt(assembled_prompt);
        let budget_stage =
            ContextBudgetStage::new(ctx_window, settings.llm.max_output_tokens as usize);
        let compaction_stage = CompactionStage::new(
            ctx_window,
            is_embedded,
            settings.working_memory.auto_compaction,
        );
        let generation_options = GenerationOptions {
            temperature: Some(settings.llm.temperature),
            max_output_tokens: Some(settings.llm.max_output_tokens),
            reasoning: ReasoningMode::from_enabled(settings.llm.reasoning_enabled),
            ..Default::default()
        };

        Self {
            session_id,
            domain: PipelineDomain::ModularAssistant,
            cancel_token: CancellationToken::new(),
            llm_tx: Some(llm_tx),
            history: history_stage,
            prompt: prompt_stage,
            budget: Some(budget_stage),
            compaction: Some(compaction_stage),
            stream: StreamRoutingStage::new(),
            watcher: Some(QuietCompactionWatcher::new()),
            generation_options,
        }
    }

    pub fn new_realtime(
        session_id: Option<i64>,
        base_prompt: String,
        personal_memory: Option<String>,
        settings: &VoxSettings,
    ) -> Self {
        let ctx_window = settings.llm.context_window as usize;
        let max_share = settings.working_memory.max_context_share;

        let mut prompt_stage = PromptBuilderStage::new(base_prompt, ctx_window, max_share);
        prompt_stage.set_personal_memory(personal_memory);
        let assembled_prompt = prompt_stage.assemble();

        let history_stage = ConversationHistoryStage::with_system_prompt(assembled_prompt);

        Self {
            session_id,
            domain: PipelineDomain::RealtimeS2S,
            cancel_token: CancellationToken::new(),
            llm_tx: None,
            history: history_stage,
            prompt: prompt_stage,
            budget: None,
            compaction: None,
            stream: StreamRoutingStage::new(),
            watcher: None,
            generation_options: GenerationOptions::default(),
        }
    }

    pub fn session_id(&self) -> Option<i64> {
        self.session_id
    }

    pub fn domain(&self) -> PipelineDomain {
        self.domain
    }

    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }

    pub fn assembled_system_prompt(&self) -> String {
        self.prompt.assemble()
    }

    pub fn generation_options(&self) -> &GenerationOptions {
        &self.generation_options
    }

    pub fn from_turn_id(&self) -> u32 {
        self.compaction
            .as_ref()
            .map(|c: &CompactionStage| c.from_turn_id())
            .unwrap_or(0)
    }

    pub fn set_last_compacted_to_turn(&mut self, turn_id: u32) {
        if let Some(ref mut compaction) = self.compaction {
            compaction.set_last_compacted_to_turn(turn_id);
        }
    }

    pub fn update_personal_memory(&mut self, personal_memory: Option<String>) {
        self.prompt.set_personal_memory(personal_memory);
    }

    pub fn commit_turn(&mut self, assistant_text: String) {
        self.history.push_assistant_turn(assistant_text);
    }

    pub fn fallback_fifo_shift(&mut self) {
        if let Some(ref budget) = self.budget {
            budget.execute_fifo_shift(&mut self.history);
        }
    }

    pub fn create_generation_request(&self) -> GenerationRequest {
        let input = ConversationInput {
            messages: self.history.messages().to_vec(),
        };
        GenerationRequest {
            input,
            options: self.generation_options.clone(),
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
        }
    }

    pub fn check_quiet_compaction_eligibility(&self) -> Option<Vec<ChatMessage>> {
        let budget = self.budget.as_ref()?;
        let compaction = self.compaction.as_ref()?;
        if !compaction.auto_compaction_enabled() {
            return None;
        }
        let tracked = budget.calculate_tracked_tokens(self.history.messages());
        let (_, status) = budget.evaluate_utilization(tracked);
        if status == ContextStatus::SoftWarning && self.history.messages().len() >= 4 {
            Some(self.history.messages().to_vec())
        } else {
            None
        }
    }

    pub fn apply_quiet_compaction_summary(&mut self, session_context: &str) {
        self.apply_session_context(session_context, "");
    }

    pub fn apply_quiet_session_context(&mut self, session_context: &str) {
        self.apply_session_context(session_context, "");
    }

    pub fn on_turn_completed(
        &mut self,
        state: Arc<AppState>,
        harness_lock: Arc<Mutex<Option<Harness>>>,
    ) {
        let tracked_turns = self.check_quiet_compaction_eligibility();
        if let Some(ref mut watcher) = self.watcher {
            watcher.on_turn_completed(state, harness_lock, tracked_turns);
        }
    }

    pub fn abort_watcher(&mut self) {
        if let Some(ref mut watcher) = self.watcher {
            watcher.abort();
        }
    }

    pub fn llm_tx(&self) -> Option<&mpsc::Sender<LlmCommand>> {
        self.llm_tx.as_ref()
    }

    pub fn push_user_turn(&mut self, text: String) {
        self.history.push_user_turn(text);
    }

    pub fn push_assistant_turn(&mut self, text: String) {
        self.history.push_assistant_turn(text);
    }

    pub fn messages(&self) -> &[ChatMessage] {
        self.history.messages()
    }

    pub fn check_critical_compaction_eligibility(&self) -> Option<Vec<ChatMessage>> {
        let budget = self.budget.as_ref()?;
        let compaction = self.compaction.as_ref()?;
        let tracked = budget.calculate_tracked_tokens(self.history.messages());
        let (_, status) = budget.evaluate_utilization(tracked);
        if status == ContextStatus::Critical
            && compaction.can_perform_inline_compaction(self.history.messages().len())
        {
            Some(self.history.messages().to_vec())
        } else {
            None
        }
    }

    pub fn seed_continuation(&mut self, session_context: Option<String>, turns: Vec<TurnRow>) {
        if let Some(ref mut compaction) = self.compaction {
            compaction.set_session_context(session_context);
        }
        for turn in turns {
            self.history.push_user_turn(turn.user_text);
            self.history.push_assistant_turn(turn.assistant_text);
        }
    }

    pub fn apply_session_context(&mut self, session_context: &str, active_query: &str) {
        if let Some(ref mut compaction) = self.compaction {
            compaction.apply_session_context(session_context);
            let pruned_messages =
                compaction.prune_history_with_summary(&self.prompt.assemble(), active_query);
            self.history.clear();
            for msg in pruned_messages {
                if msg.role == Role::System {
                    self.history.reset(msg.content);
                } else if msg.role == Role::User {
                    self.history.push_user_turn(msg.content);
                } else {
                    self.history.push_assistant_turn(msg.content);
                }
            }
            log::info!(
                "[Harness] History rebuilt with session context. Total turns: {}",
                self.history.messages().len()
            );
        }
    }

    /// Orchestrates the full 7-phase conversational turn lifecycle.
    /// Locks `harness_arc` briefly during synchronous phase transitions and releases before `.await` points.
    pub async fn execute_turn<R: tauri::Runtime + 'static>(
        harness_arc: &Arc<Mutex<Option<Harness>>>,
        req: TurnExecutionRequest<R>,
    ) -> TurnOutcome {
        let turn_id = req.turn_id;

        // Phase 1 & 2: Pre-cancellation, duplicate check, and query staging
        if req.cancel.is_cancelled() {
            log::info!("[Harness] Turn {} pre-cancelled before staging", turn_id);
            return TurnOutcome::Cancelled { turn_id };
        }

        let (can_compact, stream_stage) = {
            let mut guard = harness_arc.lock();
            let Some(ref mut harness) = *guard else {
                return TurnOutcome::Error {
                    turn_id,
                    message: "No active Harness mounted".to_string(),
                };
            };

            if harness.history.is_duplicate_user_turn(&req.query) {
                log::info!(
                    "[Harness] Dropping duplicate user turn {} ('{}')",
                    turn_id,
                    req.query
                );
                return TurnOutcome::DuplicateIgnored { turn_id };
            }

            // Guaranteed User Query Staging
            harness.history.push_user_turn(req.query.clone());

            // In-place system prompt synchronization
            let assembled_system = harness.prompt.assemble();
            harness.history.sync_system_prompt(&assembled_system);

            // Phase 3 check
            let can_compact = if let Some(ref budget) = harness.budget {
                let tracked_tokens = budget.calculate_tracked_tokens(harness.history.messages());
                let (utilization, status) = budget.evaluate_utilization(tracked_tokens);
                log::info!(
                    "[Harness] Turn {} context utilization: {:.1}% ({:?})",
                    turn_id,
                    utilization * 100.0,
                    status
                );

                if status == ContextStatus::Critical {
                    let eligible = harness
                        .compaction
                        .as_ref()
                        .map(|c| c.can_perform_inline_compaction(harness.history.messages().len()))
                        .unwrap_or(false);

                    if !eligible {
                        log::warn!(
                            "[Harness] Compaction ineligible. Executing degraded FIFO shift."
                        );
                        budget.execute_fifo_shift(&mut harness.history);
                        false
                    } else {
                        true
                    }
                } else {
                    false
                }
            } else {
                false
            };

            (can_compact, harness.stream.clone())
        };

        // Phase 3: Inline Compaction (if critical utilization triggered)
        if can_compact {
            transition(
                InteractionState::Working,
                &req.routing_ctx,
                &req.app,
                &req.app_state,
            );

            let filler = super::select_filler_phrase(&req.query, turn_id);
            if let Some(ref tts_tx) = req.tts_tx {
                req.pending_synthesis_jobs.fetch_add(1, Relaxed);
                let res = tts_tx.send(TtsCommand::Generate {
                    turn_id,
                    text: filler.to_string(),
                    intent: AudioIntent::InterimFiller,
                });
                if res.is_err() {
                    let _ = req
                        .pending_synthesis_jobs
                        .fetch_update(Relaxed, Relaxed, |val| Some(val.saturating_sub(1)));
                }
            }

            if let Some(provider) = req.provider.as_ref() {
                let compactor_cancel = req.cancel.clone();
                let session_id = req.app_state.conversation_id.load(Relaxed) as i64;
                let (from_turn, history_slice) = {
                    let guard = harness_arc.lock();
                    guard
                        .as_ref()
                        .map(|h| (h.from_turn_id(), h.history.messages().to_vec()))
                        .unwrap_or((0, Vec::new()))
                };
                let to_turn = turn_id;
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
                    Err(anyhow::anyhow!(
                        "Compaction blocking task panicked: {:?}",
                        join_err
                    ))
                });

                let mut guard = harness_arc.lock();
                if let Some(ref mut harness) = *guard {
                    match compaction_res {
                        Ok(result) => {
                            if !result.session_context.trim().is_empty() {
                                harness.apply_session_context(&result.session_context, &req.query);
                                harness.set_last_compacted_to_turn(to_turn);
                                log::info!(
                                    "[Harness] Inline compaction succeeded; history refreshed."
                                );
                            } else {
                                log::warn!(
                                    "[Harness] Inline compaction returned empty context; executing degraded FIFO shift."
                                );
                                harness.fallback_fifo_shift();
                            }
                        }
                        Err(err) => {
                            log::warn!(
                                "[Harness] Inline compaction failed (0-retry): {}. Executing degraded FIFO shift.",
                                err
                            );
                            harness.fallback_fifo_shift();
                        }
                    }
                }
            } else {
                log::warn!(
                    "[Harness] Critical context threshold reached on turn {} but no LLM provider available — executing degraded FIFO shift.",
                    turn_id
                );
                let mut guard = harness_arc.lock();
                if let Some(ref mut harness) = *guard {
                    harness.fallback_fifo_shift();
                }
            }
        }

        // Phase 4: Prompt Assembly & GenerationRequest Building
        let generation_request = {
            let guard = harness_arc.lock();
            let Some(ref harness) = *guard else {
                return TurnOutcome::Error {
                    turn_id,
                    message: "Harness unmounted during execution".to_string(),
                };
            };
            GenerationRequest {
                input: ConversationInput {
                    messages: harness.history.messages().to_vec(),
                },
                options: harness.generation_options.clone(),
                output: OutputConstraint::Text,
                purpose: GenerationPurpose::Conversation,
            }
        };

        // Phase 5: Transport Ingestion & Duplex Pipe Dispatch
        let Some(ref llm_tx) = req.llm_tx else {
            log::error!("[Harness] No LLM channel available for turn {}", turn_id);
            let mut guard = harness_arc.lock();
            if let Some(ref mut harness) = *guard {
                harness.history.rollback_last_user_turn();
            }
            return TurnOutcome::Error {
                turn_id,
                message: "No LLM channel available".to_string(),
            };
        };

        let (response_tx, response_rx) = mpsc::channel();
        let cancel_atomic = Arc::new(AtomicBool::new(req.cancel.is_cancelled()));

        let cancel_atomic_clone = Arc::clone(&cancel_atomic);
        let cancel_token_clone = req.cancel.clone();
        let cancel_bridge = tauri::async_runtime::spawn(async move {
            cancel_token_clone.cancelled().await;
            cancel_atomic_clone.store(true, Relaxed);
        });

        let cmd = LlmCommand::Generate {
            request: Box::new(generation_request),
            turn_id,
            cancel: req.cancel.clone(),
            response_tx,
        };

        if let Err(e) = llm_tx.send(cmd) {
            cancel_bridge.abort();
            log::error!("[Harness] Failed to dispatch LlmCommand: {}", e);
            let mut guard = harness_arc.lock();
            if let Some(ref mut harness) = *guard {
                harness.history.rollback_last_user_turn();
            }
            return TurnOutcome::Error {
                turn_id,
                message: format!("Failed to dispatch LlmCommand: {}", e),
            };
        }

        // Phase 6: Streaming Egress & Demuxing (spawn_blocking)
        let Some(ref pipeline_tx) = req.pipeline_tx else {
            cancel_bridge.abort();
            log::error!("[Harness] No pipeline event channel for turn {}", turn_id);
            let mut guard = harness_arc.lock();
            if let Some(ref mut harness) = *guard {
                harness.history.rollback_last_user_turn();
            }
            return TurnOutcome::Error {
                turn_id,
                message: "No pipeline event channel".to_string(),
            };
        };

        let stream_handles = StreamRoutingHandles {
            turn_id,
            owner: req.owner,
            accumulator: Arc::clone(&req.accumulator),
            tts_tx: req.tts_tx.clone(),
            pending_synthesis_jobs: Arc::clone(&req.pending_synthesis_jobs),
            cancel: Arc::clone(&cancel_atomic),
            event_tx: pipeline_tx.clone(),
            app: req.app.clone(),
        };

        let stream_result = tokio::task::spawn_blocking(move || {
            stream_stage.route_stream(stream_handles, response_rx)
        })
        .await;

        // Phase 7: Finalization & Commit
        let outcome = match stream_result {
            Ok(Ok(assistant_text)) => {
                if cancel_atomic.load(Relaxed) {
                    log::info!(
                        "[Harness] Turn {} cancelled during streaming. Rolling back user turn.",
                        turn_id
                    );
                    let mut guard = harness_arc.lock();
                    if let Some(ref mut harness) = *guard {
                        harness.history.rollback_last_user_turn();
                    }
                    TurnOutcome::Cancelled { turn_id }
                } else {
                    log::info!(
                        "[Harness] Turn {} successfully streamed ({} chars). Committing to history.",
                        turn_id,
                        assistant_text.len()
                    );
                    let mut guard = harness_arc.lock();
                    if let Some(ref mut harness) = *guard {
                        harness.history.push_assistant_turn(assistant_text.clone());
                    }
                    TurnOutcome::Completed {
                        turn_id,
                        assistant_response: assistant_text,
                    }
                }
            }
            Ok(Err(err)) => {
                log::error!(
                    "[Harness] Streaming routing error on turn {}: {}",
                    turn_id,
                    err
                );
                let mut guard = harness_arc.lock();
                if let Some(ref mut harness) = *guard {
                    harness.history.rollback_last_user_turn();
                }
                TurnOutcome::Error {
                    turn_id,
                    message: err,
                }
            }
            Err(join_err) => {
                log::error!(
                    "[Harness] Streaming task panicked on turn {}: {:?}",
                    turn_id,
                    join_err
                );
                let mut guard = harness_arc.lock();
                if let Some(ref mut harness) = *guard {
                    harness.history.rollback_last_user_turn();
                }
                TurnOutcome::Error {
                    turn_id,
                    message: format!("Stream task panic: {:?}", join_err),
                }
            }
        };

        cancel_bridge.abort();
        outcome
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.abort_watcher();
        self.cancel_token.cancel();
    }
}
