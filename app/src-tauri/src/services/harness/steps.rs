use std::sync::{
    atomic::{AtomicU32, Ordering::Relaxed},
    mpsc, Arc,
};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    core::{
        events::{
            ActivityEnvelope, ActivityKind, InteractionOwner, IpcEmitter, IpcEvent,
            PipelineMode, VoxEvent, ACTIVITY_COMPACTION,
        },
        state::{AppState, AppWindow, InteractionState},
    },
    pipeline::router::{transition, RoutingContext},
    services::{
        harness::{
            chassis::Harness,
            stages::{
                self,
                budget::{ContextBudgetStage, ContextStatus},
                compaction::{CompactionParams, CompactionStage},
                guardrails::GuardrailAction,
                inbox::InboxMessage,
            },
            tools::{ToolExecutionContext, ToolExecutor, ToolFilter, ToolRegistry},
            TurnExecutionRequest, TurnOutcome,
        },
        llm::{actor::LlmResponse, CanonicalToolCall, GenerationRequest},
    },
};

pub use crate::services::harness::stages::streaming::{
    normalizer::TextNormalizer, StreamPassOutcome, StreamRoutingHandles, StreamRoutingStage,
};

/// Bundled contextual handles for a single active cognitive turn loop iteration.
pub struct TurnLoopContext<'a> {
    pub harness_arc: &'a Arc<Mutex<Option<Harness>>>,
    pub req: &'a TurnExecutionRequest,
    pub stream_stage: &'a StreamRoutingStage,
    pub stream_handles: &'a StreamRoutingHandles,
    pub tool_registry: ToolRegistry,
    pub session_id: i64,
}

impl<'a> TurnLoopContext<'a> {
    pub fn new(
        harness_arc: &'a Arc<Mutex<Option<Harness>>>,
        req: &'a TurnExecutionRequest,
        stream_stage: &'a StreamRoutingStage,
        stream_handles: &'a StreamRoutingHandles,
        session_id: i64,
    ) -> Result<(Self, bool), TurnOutcome> {
        let guard = harness_arc.lock();
        let Some(ref harness) = *guard else {
            return Err(TurnOutcome::Error {
                turn_id: req.turn_id,
                message: "Harness unmounted during execution".to_string(),
            });
        };
        let tool_registry = harness.tool_registry.clone();
        let supports_tools = harness.supports_tools;
        drop(guard);

        Ok((
            Self {
                harness_arc,
                req,
                stream_stage,
                stream_handles,
                tool_registry,
                session_id,
            },
            supports_tools,
        ))
    }

    pub fn cancelled_ctx(&self, partial_text: String) -> CancelledTurnContext<'_> {
        CancelledTurnContext {
            harness_arc: self.harness_arc,
            turn_id: self.req.turn_id,
            session_id: self.session_id,
            user_text: self.req.query.clone(),
            partial_text,
            app_state: &self.req.app_state,
            event_tx: Some(&self.stream_handles.event_tx),
            owner: self.req.owner,
        }
    }
}

// =========================================================================
// === STEP 1 & STEP 2: INTAKE, DEDUPLICATION & TOKEN BUDGET EVALUATION ===
// =========================================================================

/// Outcome of staging and token budget evaluation.
pub enum IntakeResult {
    Proceed {
        can_compact: bool,
        stream_stage: StreamRoutingStage,
    },
    Terminal(TurnOutcome),
}

/// Evaluates Phase 1 (Intake & Deduplication) and Phase 2 (Staging & Budget Evaluation).
pub fn step1_intake(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: &TurnExecutionRequest,
) -> IntakeResult {
    let turn_id = req.turn_id;

    if req.cancel.is_cancelled() {
        log::info!(
            "[Harness::Intake] Turn {} pre-cancelled before staging",
            turn_id
        );
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
            "[Harness::Intake] Dropping duplicate user turn {} ('{}')",
            turn_id,
            req.query
        );
        return IntakeResult::Terminal(TurnOutcome::DuplicateIgnored { turn_id });
    }

    let start_intake = std::time::Instant::now();
    let can_compact = harness.intake_recorded_turn(req.turn_id, req.query.clone());
    log::info!(
        "[Harness::Intake] Turn {} intake & budget evaluated in {}ms (can_compact: {})",
        turn_id,
        start_intake.elapsed().as_millis(),
        can_compact
    );

    req.app_state.pipeline.set_turn_open(true);
    req.app_state.pipeline.clear_drained_while_open();
    transition(
        InteractionState::Thinking,
        None,
        &req.routing_ctx,
        &*req.emitter,
        &req.app_state,
    );

    IntakeResult::Proceed {
        can_compact,
        stream_stage: harness.stream.clone(),
    }
}

// =========================================================================
// === STEP 3: INLINE COMPACTION & NON-TERMINAL WORKING TRANSITION ===
// =========================================================================

/// Trigger that initiated an intermediate, non-terminal operational phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonTerminalTrigger {
    Compaction,
    NonTerminalTool { tool_name: String, call_id: String },
}

impl NonTerminalTrigger {
    pub fn activity(&self) -> ActivityEnvelope {
        match self {
            NonTerminalTrigger::Compaction => ActivityEnvelope {
                kind: ActivityKind::Compaction,
                name: ACTIVITY_COMPACTION.to_string(),
                call_id: None,
            },
            NonTerminalTrigger::NonTerminalTool { tool_name, call_id } => ActivityEnvelope {
                kind: ActivityKind::Tool,
                name: tool_name.clone(),
                call_id: Some(call_id.clone()),
            },
        }
    }
}

/// Description of a non-terminal operational phase (compaction, tool execution, etc.).
#[derive(Debug, Clone)]
pub struct NonTerminalPhase {
    pub trigger: NonTerminalTrigger,
    pub filler_phrase: Option<String>,
}

impl NonTerminalPhase {
    pub fn compaction(filler_phrase: Option<String>) -> Self {
        Self {
            trigger: NonTerminalTrigger::Compaction,
            filler_phrase,
        }
    }

    pub fn tool(
        tool_name: impl Into<String>,
        call_id: impl Into<String>,
        filler_phrase: Option<String>,
    ) -> Self {
        Self {
            trigger: NonTerminalTrigger::NonTerminalTool {
                tool_name: tool_name.into(),
                call_id: call_id.into(),
            },
            filler_phrase,
        }
    }
}

/// Bundled handles and contextual metadata for entering a non-terminal operational phase.
pub struct NonTerminalContext<'a> {
    pub query: &'a str,
    pub turn_id: u32,
    pub routing_ctx: &'a RoutingContext,
    pub emitter: &'a Arc<dyn IpcEmitter>,
    pub app_state: &'a Arc<AppState>,
    pub tts_tx: Option<&'a stages::speech::TtsSender>,
    pub pending_synthesis_jobs: &'a Arc<AtomicU32>,
    pub play_filler: bool,
}

/// Uniformly transitions pipeline to Working and dispatches normalized interim filler audio.
pub fn enter_non_terminal_phase(phase: &NonTerminalPhase, ctx: &NonTerminalContext<'_>) {
    let activity = phase.trigger.activity();
    log::info!(
        "[Harness::NonTerminal] Entering working phase: trigger={:?}",
        phase.trigger
    );

    transition(
        InteractionState::Working,
        Some(&activity),
        ctx.routing_ctx,
        &**ctx.emitter,
        ctx.app_state,
    );

    if !ctx.play_filler {
        return;
    }

    let raw_filler = match &phase.filler_phrase {
        Some(phrase) if !phrase.trim().is_empty() => phrase.clone(),
        _ => crate::services::harness::select_filler_phrase(ctx.query, ctx.turn_id).to_string(),
    };

    let normalized_filler = TextNormalizer::normalize_for_speech(&raw_filler);
    if normalized_filler.is_empty() {
        return;
    }

    if let Some(tts_tx) = ctx.tts_tx {
        if let Err(e) = stages::speech::dispatch_tts_filler(
            tts_tx,
            ctx.turn_id,
            normalized_filler,
            ctx.pending_synthesis_jobs,
        ) {
            log::warn!("[Phase] {}", e);
        }
    }
}

/// Evaluates context budget including history and tool schemas, triggering inline compaction at critical threshold.
pub async fn step3_check_and_run_loop_budget(
    ctx: &TurnLoopContext<'_>,
    allow_tools: bool,
    iteration: usize,
) {
    let (should_compact, total_tokens) = {
        let guard = ctx.harness_arc.lock();
        if let Some(ref harness) = *guard {
            if let Some(ref budget) = harness.budget {
                let tools = if allow_tools && harness.supports_tools {
                    let filter = ToolFilter {
                        mode: PipelineMode::Modular,
                        title_is_unset: !harness.title_set,
                        memory_retrieval_enabled: harness.memory_retrieval_enabled,
                        web_search_enabled: harness.web_search_enabled,
                    };
                    Some(harness.tool_registry.active_definitions(&filter))
                } else {
                    None
                };
                let total_tokens = budget.calculate_tracked_tokens_with_extras(
                    harness.history.messages(),
                    tools.as_deref(),
                );
                let (utilization, status) = budget.evaluate_utilization(total_tokens);
                log::debug!(
                    "[Harness::Loop] Loop budget (turn {}, iter {}): tokens={}, util={:.1}%, status={:?}",
                    ctx.req.turn_id,
                    iteration,
                    total_tokens,
                    utilization * 100.0,
                    status
                );
                ctx.req
                    .app_state
                    .turn_metrics
                    .record_context_budget(total_tokens, budget.max_context_tokens());
                let can_compact = status == ContextStatus::Critical
                    && harness
                        .compaction
                        .as_ref()
                        .map(|c| c.can_perform_inline_compaction(harness.history.messages().len()))
                        .unwrap_or(false);

                (can_compact, total_tokens)
            } else {
                (false, 0)
            }
        } else {
            (false, 0)
        }
    };

    if should_compact {
        log::info!(
            "[Harness::Loop] Mid-loop critical budget triggered inline compaction (turn {}, iter {}, tokens {})",
            ctx.req.turn_id,
            iteration,
            total_tokens
        );
        step3_execute_compaction(ctx.harness_arc, ctx.req).await;
    }
}

/// Executes Phase 3: Generic Non-Terminal Phase (Inline compaction & maintenance).
pub async fn step3_execute_compaction(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: &TurnExecutionRequest,
) {
    let should_play_filler = {
        let mut guard = harness_arc.lock();
        if let Some(ref mut harness) = *guard {
            let play = !harness.has_played_filler;
            if play {
                harness.has_played_filler = true;
            }
            play
        } else {
            false
        }
    };

    let non_terminal = NonTerminalPhase::compaction(None);
    let ctx = NonTerminalContext {
        query: &req.query,
        turn_id: req.turn_id,
        routing_ctx: &req.routing_ctx,
        emitter: &req.emitter,
        app_state: &req.app_state,
        tts_tx: req.tts_tx.as_ref(),
        pending_synthesis_jobs: &req.pending_synthesis_jobs,
        play_filler: should_play_filler,
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
        let conn = db
            .connect()
            .map_err(|e| anyhow::anyhow!("Failed to connect to db for compaction: {}", e))?;
        handle.block_on(async {
            let params = CompactionParams {
                session_id,
                trigger_kind: "critical",
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
    .unwrap_or_else(|join_err| Err(anyhow::anyhow!("Compaction task panic: {:?}", join_err)));

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
    if let Some(ref mut harness) = *guard {
        harness.apply_compaction_result(&compaction_res, to_turn, query);
    }
}

// =========================================================================
// === STEP 4: GENERATION REQUEST ASSEMBLY ===
// =========================================================================

/// Assembles the complete GenerationRequest payload delegating to PromptBuilderStage.
pub fn step4_assemble_request(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    turn_id: u32,
    allow_tools: bool,
) -> Result<GenerationRequest, String> {
    let guard = harness_arc.lock();
    let Some(ref harness) = *guard else {
        return Err("Harness unmounted during execution".to_string());
    };

    log::info!(
        "[Harness::Assemble] Assembling request for turn {}",
        turn_id
    );
    let tools = if harness.supports_tools && allow_tools {
        let filter = ToolFilter {
            mode: PipelineMode::Modular,
            title_is_unset: !harness.title_set,
            memory_retrieval_enabled: harness.memory_retrieval_enabled,
            web_search_enabled: harness.web_search_enabled,
        };
        let active = harness.tool_registry.active_definitions(&filter);
        if active.is_empty() {
            log::info!(
                "[Harness::Assemble] Turn {}: tools omitted (none active; title_set={}, memory_enabled={}, web_search_enabled={})",
                turn_id,
                !filter.title_is_unset,
                filter.memory_retrieval_enabled,
                filter.web_search_enabled
            );
            None
        } else {
            let names: Vec<&str> = active.iter().map(|t| t.name.as_str()).collect();
            log::info!(
                "[Harness::Assemble] Turn {}: attaching tools {:?}",
                turn_id,
                names
            );
            Some(active)
        }
    } else {
        log::info!(
            "[Harness::Assemble] Turn {}: tools omitted (supports_tools={}, allow_tools={})",
            turn_id,
            harness.supports_tools,
            allow_tools
        );
        None
    };

    let start_assemble = std::time::Instant::now();
    let req = harness.prompt.build_generation_request(
        harness.history.messages(),
        harness.generation_options.clone(),
        tools,
    );
    log::info!(
        "[Harness::Assemble] Turn {} assembled in {}ms (messages: {})",
        turn_id,
        start_assemble.elapsed().as_millis(),
        req.input.messages.len()
    );
    Ok(req)
}

// =========================================================================
// === STEP 5: MODEL PIPE DISPATCH ===
// =========================================================================

/// Dispatches an LLM generation command over the actor channel.
pub fn step5_dispatch_llm(
    llm_tx: Option<&stages::dispatch::LlmSender>,
    request: GenerationRequest,
    turn_id: u32,
    cancel: &CancellationToken,
) -> Result<mpsc::Receiver<LlmResponse>, String> {
    stages::dispatch::dispatch_llm_generation(llm_tx, request, turn_id, cancel)
}

// =========================================================================
// === STEP 6: STREAM DEMUXING & TOOL INTERCEPTION ===
// =========================================================================

/// Executes a single streaming pass on an isolated blocking thread.
pub async fn step6_run_stream_pass(
    stream_stage: &StreamRoutingStage,
    stream_handles: &StreamRoutingHandles,
    response_rx: mpsc::Receiver<LlmResponse>,
    allow_tools: bool,
) -> Result<StreamPassOutcome, String> {
    let handles_clone = stream_handles.clone();
    let stage_clone = stream_stage.clone();
    tokio::task::spawn_blocking(move || {
        stage_clone.route_stream(handles_clone, response_rx, allow_tools)
    })
    .await
    .unwrap_or_else(|join_err| Err(format!("Stream task panic: {:?}", join_err)))
}

/// Executes a batch of intercepted canonical tool calls, persisting execution audit and metrics.
pub async fn step6_execute_tool_batch(
    ctx: &TurnLoopContext<'_>,
    calls: Vec<CanonicalToolCall>,
    preamble_text: &str,
    thinking_text: &str,
    step_index: usize,
) -> (bool, Option<String>, bool) {
    let turn_id = ctx.req.turn_id;
    let step_start = std::time::Instant::now();
    ctx.stream_handles
        .accumulator
        .lock()
        .assistant_response
        .clear();
    log::info!(
        "[Harness::Tools] Turn {} executing tool batch ({} calls) on step_index {}",
        turn_id,
        calls.len(),
        step_index
    );

    let max_observation_tokens = {
        let guard = ctx.harness_arc.lock();
        if let Some(ref harness) = *guard {
            if let Some(ref budget) = harness.budget {
                let filter = ToolFilter {
                    mode: PipelineMode::Modular,
                    title_is_unset: !harness.title_set,
                    memory_retrieval_enabled: harness.memory_retrieval_enabled,
                    web_search_enabled: harness.web_search_enabled,
                };
                let tools = harness.tool_registry.active_definitions(&filter);
                budget.calculate_max_observation_tokens(
                    harness.history.messages(),
                    Some(&tools),
                )
            } else {
                2000
            }
        } else {
            2000
        }
    };

    let step_id = uuid::Uuid::new_v4().to_string();

    let tool_ctx = ToolExecutionContext::new(
        Arc::clone(&ctx.req.app_state),
        ctx.session_id,
        turn_id,
        ctx.req.cancel.clone(),
        make_sessions_changed_callback(&ctx.req.emitter),
    )
    .with_step(step_id.clone(), step_index as u32)
    .with_observation_budget(max_observation_tokens);

    let non_terminal = if let Some(first_call) = calls.first() {
        NonTerminalPhase::tool(&first_call.name, &first_call.id, None)
    } else {
        NonTerminalPhase::compaction(None)
    };
    let nt_ctx = NonTerminalContext {
        query: &ctx.req.query,
        turn_id,
        routing_ctx: &ctx.req.routing_ctx,
        emitter: &ctx.req.emitter,
        app_state: &ctx.req.app_state,
        tts_tx: ctx.req.tts_tx.as_ref(),
        pending_synthesis_jobs: &ctx.req.pending_synthesis_jobs,
        play_filler: false,
    };
    enter_non_terminal_phase(&non_terminal, &nt_ctx);

    let (mut outcomes, suspended_question) = ToolExecutor::execute_batch(
        &ctx.tool_registry,
        PipelineMode::Modular,
        calls.clone(),
        &tool_ctx,
    )
    .await;

    // Evaluate loop guardrails across completed outcomes
    let (guardrail_action, strike_count) = {
        let mut guard = ctx.harness_arc.lock();
        if let Some(ref mut harness) = *guard {
            let action = harness.guardrails.evaluate_batch(
                turn_id,
                step_index as u32,
                &mut harness.guardrails_state,
                &outcomes,
            );
            let strike = harness.guardrails_state.strike_count;
            (action, strike)
        } else {
            (GuardrailAction::Proceed, 0)
        }
    };

    let circuit_broken = matches!(guardrail_action, GuardrailAction::Strike2CircuitBreak);

    if let GuardrailAction::Strike1Warning { ref warning_msg } = guardrail_action {
        if let Some(last_outcome) = outcomes.last_mut() {
            last_outcome.result.content.push_str(warning_msg);
        }
    }

    // Flip title_set flag on successful set_title execution
    for outcome in &outcomes {
        if outcome.tool_name == "set_title" && !outcome.is_error {
            let mut guard = ctx.harness_arc.lock();
            if let Some(ref mut harness) = *guard {
                harness.title_set = true;
            }
        }
    }

    let completion_tokens = ContextBudgetStage::estimate_step_completion_tokens(
        preamble_text,
        thinking_text,
        &calls,
    );

    let prompt_tokens = {
        let guard = ctx.harness_arc.lock();
        guard
            .as_ref()
            .and_then(|h| {
                let filter = ToolFilter {
                    mode: PipelineMode::Modular,
                    title_is_unset: !h.title_set,
                    memory_retrieval_enabled: h.memory_retrieval_enabled,
                    web_search_enabled: h.web_search_enabled,
                };
                let active_tools = h.tool_registry.active_definitions(&filter);
                h.budget.as_ref().map(|b| {
                    b.estimate_step_prompt_tokens(
                        h.history.messages(),
                        Some(&active_tools),
                    )
                })
            })
            .unwrap_or(0)
    };

    // Always commit tool step to history, even if cancelled mid-batch
    let tool_results: Vec<(String, String)> = outcomes
        .iter()
        .map(|o| (o.call_id.clone(), o.result.content.clone()))
        .collect();

    {
        let mut guard = ctx.harness_arc.lock();
        if let Some(ref mut harness) = *guard {
            harness.commit_tool_step(
                preamble_text.to_string(),
                calls,
                tool_results,
            );
        }
    }

    // Dispatch persistence events in strict parent-then-child FIFO order
    let step_duration_ms = step_start.elapsed().as_millis() as u64;

    let is_compacted = {
        let mut guard = ctx.harness_arc.lock();
        if let Some(ref mut h) = *guard {
            let was = h.is_compacted_this_turn;
            h.is_compacted_this_turn = false;
            was
        } else {
            false
        }
    };

    log::info!(
        "[StepAudit] session_id={} turn_id={} step_index={} duration_ms={} prompt_tokens={} completion_tokens={} tool_count={} guardrail_strike={} is_compacted={}",
        ctx.session_id,
        turn_id,
        step_index,
        step_duration_ms,
        prompt_tokens,
        completion_tokens,
        outcomes.len(),
        strike_count,
        is_compacted
    );

    let persist_lock = ctx.req.app_state.persist_tx.lock();
    stages::persistence::persist_step_and_tools(
        persist_lock.as_ref(),
        ctx.session_id,
        turn_id,
        step_id,
        step_index as u32,
        Some(preamble_text.to_string()),
        &outcomes,
        step_duration_ms,
        prompt_tokens,
        completion_tokens,
        strike_count as u32,
        is_compacted,
    );

    (ctx.req.cancel.is_cancelled(), suspended_question, circuit_broken)
}

/// Awaits any active speech-hold on StepInbox and drains immediate steering messages into history at the step boundary.
pub async fn step6_await_and_drain_inbox(
    ctx: &TurnLoopContext<'_>,
) -> Result<Vec<InboxMessage>, TurnOutcome> {
    let inbox = {
        let guard = ctx.harness_arc.lock();
        guard.as_ref().map(|h| Arc::clone(&h.inbox))
    };

    if let Some(inbox) = inbox {
        inbox.wait_if_speech_held(&ctx.req.cancel).await;
        if ctx.req.cancel.is_cancelled() {
            return Err(step7_handle_cancelled(ctx.cancelled_ctx(String::new())));
        }

        let immediate = inbox.drain_immediate();
        if !immediate.is_empty() {
            let mut guard = ctx.harness_arc.lock();
            if let Some(ref mut harness) = *guard {
                for msg in &immediate {
                    harness.history.push_user_turn(msg.text.clone());
                }
            }
        }

        transition(
            InteractionState::Thinking,
            None,
            &ctx.req.routing_ctx,
            &*ctx.req.emitter,
            &ctx.req.app_state,
        );

        Ok(immediate)
    } else {
        Ok(Vec::new())
    }
}

fn make_sessions_changed_callback(
    emitter: &Arc<dyn IpcEmitter>,
) -> Option<Arc<dyn Fn() + Send + Sync>> {
    let emitter_clone = Arc::clone(emitter);
    Some(Arc::new(move || {
        if let Err(e) = emitter_clone.emit_ipc(AppWindow::Main, IpcEvent::SessionsChanged) {
            log::warn!("[Harness::Tools] Failed to emit SessionsChanged IPC: {}", e);
        } else {
            log::info!("[Harness::Tools] Emitted SessionsChanged IPC");
        }
    }) as Arc<dyn Fn() + Send + Sync>)
}

// =========================================================================
// === STEP 7: FINALIZE & CANCELLATION ===
// =========================================================================

/// Context parameters required to finalize a cancelled or barged-in turn.
pub struct CancelledTurnContext<'a> {
    pub harness_arc: &'a Arc<Mutex<Option<Harness>>>,
    pub turn_id: u32,
    pub session_id: i64,
    pub user_text: String,
    pub partial_text: String,
    pub app_state: &'a AppState,
    pub event_tx: Option<&'a std::sync::mpsc::Sender<VoxEvent>>,
    pub owner: InteractionOwner,
}

/// Finalizes a suspended question by dispatching response TTS, emitting LlmFinished, and committing to history.
pub fn step7_commit_suspended_question(
    ctx: &TurnLoopContext<'_>,
    question: String,
) -> TurnOutcome {
    let turn_id = ctx.req.turn_id;
    if let Some(ref tts_tx) = ctx.req.tts_tx {
        let normalized = TextNormalizer::normalize_for_speech(&question);
        if !normalized.is_empty() {
            if let Err(e) = stages::speech::dispatch_tts_response(
                tts_tx,
                turn_id,
                normalized,
                &ctx.req.pending_synthesis_jobs,
            ) {
                log::warn!(
                    "[Harness::Loop] Failed to dispatch suspended question TTS: {}",
                    e
                );
            }
        }
    }
    if let Err(e) = ctx
        .stream_handles
        .event_tx
        .send(VoxEvent::LlmFinished { turn_id })
    {
        log::warn!(
            "[Harness::Loop] Failed to emit LlmFinished on turn suspension: {}",
            e
        );
    }
    step7_commit_completed(ctx.harness_arc, turn_id, question)
}

/// Handles Phase 7 Branch B: turn cancellation / user barge-in (Invariant 13).
pub fn step7_handle_cancelled(ctx: CancelledTurnContext<'_>) -> TurnOutcome {
    log::info!(
        "[Harness::Finalize] Finalizing cancelled turn {} (partial_chars {})",
        ctx.turn_id,
        ctx.partial_text.len()
    );

    if let Some(tx) = ctx.event_tx {
        let _ = tx.send(VoxEvent::Cancelled {
            turn_id: ctx.turn_id,
            owner: ctx.owner,
        });
    }

    let mut guard = ctx.harness_arc.lock();
    if !ctx.partial_text.trim().is_empty() {
        if let Some(ref mut harness) = *guard {
            harness
                .history
                .push_assistant_turn(ctx.partial_text.clone());
        }
        drop(guard);
        let persist_lock = ctx.app_state.persist_tx.lock();
        stages::persistence::persist_cancelled_turn(
            persist_lock.as_ref(),
            ctx.session_id,
            ctx.turn_id,
            ctx.user_text,
            ctx.partial_text,
        );
    } else if let Some(ref mut harness) = *guard {
        harness.rollback_user_turn(ctx.turn_id);
    }

    ctx.app_state
        .pipeline_accumulator
        .lock()
        .reset_assistant_output();
    TurnOutcome::Cancelled {
        turn_id: ctx.turn_id,
    }
}

/// Handles Phase 7 Branch A: turn error and state rollback.
pub fn step7_handle_error(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    turn_id: u32,
    message: String,
    event_tx: Option<&std::sync::mpsc::Sender<VoxEvent>>,
) -> TurnOutcome {
    log::error!("[Harness::Finalize] Turn {} failed: {}", turn_id, message);

    let mut guard = harness_arc.lock();
    if let Some(ref mut harness) = *guard {
        harness.rollback_user_turn(turn_id);
    }

    if let Some(tx) = event_tx {
        let _ = tx.send(VoxEvent::Error(crate::core::error::PipelineError {
            turn_id,
            message: message.clone(),
            source: "Harness".to_string(),
            impact: crate::core::error::PipelineImpact::TurnAborted,
        }));
    }

    TurnOutcome::Error { turn_id, message }
}

/// Handles Phase 7 Branch C: successful turn completion and history commit.
pub fn step7_commit_completed(
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
        harness.uncommitted_user_turn_id = None;
        harness
            .history
            .push_assistant_turn(assistant_response.clone());
    }

    TurnOutcome::Completed {
        turn_id,
        assistant_response,
    }
}
