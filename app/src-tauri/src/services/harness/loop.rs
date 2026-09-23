use std::sync::{
    atomic::{AtomicBool, Ordering::Relaxed},
    Arc,
};

use parking_lot::Mutex;
use tauri::{async_runtime::spawn as spawn_task, Runtime};

use crate::{
    core::settings::PipelineMode,
    services::{
        harness::{
            chassis::Harness,
            stages::{
                streaming::{StreamPassOutcome, StreamRoutingHandles, StreamRoutingStage},
                tools::{ToolFilter, ToolRegistry},
            },
            steps::{
                step1_intake, step3_execute_compaction, step4_assemble_request, step5_dispatch_llm,
                step6_handle_non_terminal_tool, step6_handle_terminal_tool, step6_run_stream_pass,
                step7_commit_completed, step7_handle_cancelled, step7_handle_error,
                CancelledTurnContext, IntakeResult,
            },
            ChatMessage, TurnExecutionRequest, TurnOutcome,
        },
        llm::ToolFlow,
    },
};

const MAX_TOOL_ITERATIONS: usize = 5;

pub struct TurnLoopContext<'a, R: Runtime> {
    pub harness_arc: &'a Arc<Mutex<Option<Harness>>>,
    pub req: &'a TurnExecutionRequest<R>,
    pub stream_stage: &'a StreamRoutingStage,
    pub stream_handles: &'a StreamRoutingHandles<R>,
    pub tool_registry: &'a ToolRegistry,
    pub session_id: i64,
}

enum LoopAction {
    Terminal(TurnOutcome),
    Continue,
}

/// Primary coordinator entry point: orchestrates the 7-phase conversational turn lifecycle.
pub async fn execute_turn<R: Runtime + 'static>(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: TurnExecutionRequest<R>,
) -> TurnOutcome {
    let turn_id = req.turn_id;
    log::info!(
        "[Harness::Loop] Turn {} started: session={} query_len={}",
        turn_id,
        req.app_state.conversation_id.load(Relaxed),
        req.query.len()
    );

    // === STEP 1 & STEP 2: Intake Deduplication & Token Budget Check ===
    let (can_compact, stream_stage) = match step1_intake(harness_arc, &req) {
        IntakeResult::Proceed {
            can_compact,
            stream_stage,
        } => (can_compact, stream_stage),
        IntakeResult::Terminal(outcome) => return outcome,
    };

    // === STEP 3: Compaction & Non-Terminal Working Phase ===
    if can_compact {
        step3_execute_compaction(harness_arc, &req).await;
    }

    // === STEP 4 -> STEP 6: Reentrant Cognitive Generation & Tool Loop ===
    run_cognitive_loop(harness_arc, req, stream_stage, turn_id).await
}

/// Orchestrates Step 4 (Assembly), Step 5 (Dispatch), and Step 6 (Reentrant Loop).
async fn run_cognitive_loop<R: Runtime + 'static>(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: TurnExecutionRequest<R>,
    stream_stage: StreamRoutingStage,
    turn_id: u32,
) -> TurnOutcome {
    let (tool_registry, supports_tools) = {
        let guard = harness_arc.lock();
        let Some(ref harness) = *guard else {
            return TurnOutcome::Error {
                turn_id,
                message: "Harness unmounted during execution".to_string(),
            };
        };
        (harness.tool_registry.clone(), harness.supports_tools)
    };
    log::info!(
        "[Harness::Loop] Turn {} cognitive loop: supports_tools={} registered_tools={}",
        turn_id,
        supports_tools,
        tool_registry.canonical_definitions(PipelineMode::Modular).len()
    );

    let Some(ref pipeline_tx) = req.pipeline_tx else {
        return step7_handle_error(harness_arc, turn_id, "No pipeline event channel".to_string());
    };

    let cancel_atomic = Arc::new(AtomicBool::new(req.cancel.is_cancelled()));
    let cancel_atomic_clone = Arc::clone(&cancel_atomic);
    let cancel_token_clone = req.cancel.clone();
    let cancel_bridge = spawn_task(async move {
        cancel_token_clone.cancelled().await;
        cancel_atomic_clone.store(true, Relaxed);
    });

    let stream_handles = StreamRoutingHandles {
        turn_id,
        owner: req.owner,
        accumulator: Arc::clone(&req.accumulator),
        tts_tx: req.tts_tx.clone(),
        pending_synthesis_jobs: Arc::clone(&req.pending_synthesis_jobs),
        cancel: Arc::clone(&cancel_atomic),
        event_tx: pipeline_tx.clone(),
        app: req.app.clone(),
        turn_metrics: Arc::clone(&req.app_state.turn_metrics),
    };

    let session_id = req.app_state.conversation_id.load(Relaxed) as i64;
    let loop_ctx = TurnLoopContext {
        harness_arc,
        req: &req,
        stream_stage: &stream_stage,
        stream_handles: &stream_handles,
        tool_registry: &tool_registry,
        session_id,
    };

    let outcome = execute_loop_iterations(&loop_ctx, supports_tools, &cancel_atomic).await;
    cancel_bridge.abort();
    outcome
}

/// Executes iterative generation and tool interception passes until completion or budget breach.
async fn execute_loop_iterations<R: Runtime + 'static>(
    ctx: &TurnLoopContext<'_, R>,
    supports_tools: bool,
    cancel_atomic: &Arc<AtomicBool>,
) -> TurnOutcome {
    let turn_id = ctx.req.turn_id;
    let mut scratchpad: Vec<ChatMessage> = Vec::new();
    let mut allow_tools = supports_tools;
    let mut iteration: usize = 0;

    // === REENTRANT COGNITIVE LOOP (Up to MAX_TOOL_ITERATIONS passes) ===
    while iteration <= MAX_TOOL_ITERATIONS {
        if ctx.req.cancel.is_cancelled() || cancel_atomic.load(Relaxed) {
            let partial = ctx
                .stream_handles
                .accumulator
                .lock()
                .assistant_response
                .clone();
            return step7_handle_cancelled(cancelled_ctx(ctx, partial));
        }

        check_loop_budget(ctx, &scratchpad, allow_tools, iteration);

        // === STEP 4: Assemble Generation Request with Scratchpad ===
        let generation_request =
            match step4_assemble_request(ctx.harness_arc, &scratchpad, turn_id, allow_tools) {
                Ok(req_obj) => req_obj,
                Err(e) => return step7_handle_error(ctx.harness_arc, turn_id, e),
            };

        // === STEP 5: Dispatch Request to Duplex LLM Pipe ===
        ctx.req.app_state.turn_metrics.record_llm_dispatch();
        let response_rx = match step5_dispatch_llm(
            ctx.req.llm_tx.as_ref(),
            generation_request,
            turn_id,
            &ctx.req.cancel,
        ) {
            Ok(rx) => rx,
            Err(err) => return step7_handle_error(ctx.harness_arc, turn_id, err),
        };

        // === STEP 6: Stream Tokens & Intercept Tool Calls ===
        let stream_res =
            step6_run_stream_pass(ctx.stream_stage, ctx.stream_handles, response_rx).await;
        match stream_res {
            Ok(pass_outcome) => {
                let action = handle_pass_outcome(
                    ctx,
                    pass_outcome,
                    &mut scratchpad,
                    cancel_atomic,
                    allow_tools,
                )
                .await;
                match action {
                    // === STEP 7: Finalize & Terminal Commit ===
                    LoopAction::Terminal(outcome) => return outcome,
                    LoopAction::Continue => {
                        iteration += 1;
                        log::info!(
                            "[Harness::Loop] Turn {} iteration {} continuing: scratchpad_messages={}",
                            turn_id,
                            iteration,
                            scratchpad.len()
                        );
                        if iteration >= MAX_TOOL_ITERATIONS {
                            log::warn!(
                                "[Harness::Loop] Turn {} reached MAX_TOOL_ITERATIONS; running final tool-free pass.",
                                turn_id
                            );
                            allow_tools = false;
                        }
                    }
                }
            }
            Err(err) => return step7_handle_error(ctx.harness_arc, turn_id, err),
        }
    }

    step7_handle_error(
        ctx.harness_arc,
        turn_id,
        "Exceeded max tool iterations without terminal response".to_string(),
    )
}

/// Evaluates context budget including history, scratchpad, and tool schemas.
fn check_loop_budget<R: Runtime>(
    ctx: &TurnLoopContext<'_, R>,
    scratchpad: &[ChatMessage],
    allow_tools: bool,
    iteration: usize,
) {
    let guard = ctx.harness_arc.lock();
    if let Some(ref harness) = *guard {
        if let Some(ref budget) = harness.budget {
            let tools = if allow_tools && harness.supports_tools {
                let filter = ToolFilter {
                    mode: PipelineMode::Modular,
                    is_first_turn: harness.history.messages().len() <= 2,
                    title_is_unset: !harness.title_set,
                    memory_retrieval_enabled: harness.memory_retrieval_enabled,
                };
                Some(harness.tool_registry.active_definitions(&filter))
            } else {
                None
            };
            let total_tokens = budget.calculate_tracked_tokens_with_extras(
                harness.history.messages(),
                scratchpad,
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
            ctx.req.app_state.turn_metrics.record_context_budget(
                total_tokens,
                budget.max_context_tokens(),
            );
        }
    }
}

/// Processes single-pass stream outcomes and branches to terminal resolution or reentrant looping.
async fn handle_pass_outcome<R: Runtime + 'static>(
    ctx: &TurnLoopContext<'_, R>,
    pass_outcome: StreamPassOutcome,
    scratchpad: &mut Vec<ChatMessage>,
    cancel_atomic: &Arc<AtomicBool>,
    allow_tools: bool,
) -> LoopAction {
    let turn_id = ctx.req.turn_id;
    match pass_outcome {
        StreamPassOutcome::Completed { assistant_text } => {
            log::info!(
                "[Harness::Loop] Turn {} pass completed with no tool call ({} chars); finalizing",
                turn_id,
                assistant_text.len()
            );
            if cancel_atomic.load(Relaxed) {
                LoopAction::Terminal(step7_handle_cancelled(cancelled_ctx(ctx, assistant_text)))
            } else {
                LoopAction::Terminal(step7_commit_completed(
                    ctx.harness_arc,
                    turn_id,
                    assistant_text,
                ))
            }
        }
        StreamPassOutcome::Cancelled { partial_text } => {
            LoopAction::Terminal(step7_handle_cancelled(cancelled_ctx(ctx, partial_text)))
        }
        StreamPassOutcome::Error(err) => {
            LoopAction::Terminal(step7_handle_error(ctx.harness_arc, turn_id, err))
        }
        StreamPassOutcome::ToolCallReceived { partial_text, call } => {
            if !allow_tools {
                log::warn!(
                    "[Harness::Loop] Discarding tool call '{}' during final tool-free pass; committing text.",
                    call.name
                );
                return LoopAction::Terminal(step7_commit_completed(
                    ctx.harness_arc,
                    turn_id,
                    partial_text,
                ));
            }

            let tool_flow = ctx
                .tool_registry
                .get(&call.name)
                .map(|t| t.flow())
                .unwrap_or(ToolFlow::Terminal);

            if tool_flow == ToolFlow::Terminal {
                let outcome = step6_handle_terminal_tool(ctx, call).await;
                LoopAction::Terminal(outcome)
            } else {
                let cancelled =
                    step6_handle_non_terminal_tool(ctx, call, &partial_text, scratchpad).await;
                if cancelled {
                    LoopAction::Terminal(step7_handle_cancelled(cancelled_ctx(ctx, String::new())))
                } else {
                    LoopAction::Continue
                }
            }
        }
    }
}

/// Constructs a CancelledTurnContext from active turn loop state.
fn cancelled_ctx<'a, R: Runtime>(
    ctx: &'a TurnLoopContext<'_, R>,
    partial_text: String,
) -> CancelledTurnContext<'a> {
    CancelledTurnContext {
        harness_arc: ctx.harness_arc,
        turn_id: ctx.req.turn_id,
        session_id: ctx.session_id,
        user_text: ctx.req.query.clone(),
        partial_text,
        app_state: &ctx.req.app_state,
    }
}
