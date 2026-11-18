use std::sync::{
    atomic::{AtomicBool, Ordering::Relaxed},
    Arc,
};

use parking_lot::Mutex;
use tokio::spawn as spawn_task;
use tokio_util::sync::CancellationToken;

use crate::harness::{
    chassis::Harness,
    steps::{
        step1_intake, step3_check_and_run_loop_budget, step3_execute_compaction,
        step4_assemble_request, step5_dispatch_llm, step6_await_and_drain_inbox,
        step6_execute_tool_batch, step6_run_stream_pass, step7_commit_completed,
        step7_commit_suspended_question, step7_handle_cancelled, step7_handle_error, IntakeResult,
        StreamPassOutcome, StreamRoutingHandles, StreamRoutingStage, TurnLoopContext,
    },
    TurnExecutionRequest, TurnOutcome,
};

pub const MAX_TOOL_STEPS: usize = 20;

enum LoopAction {
    Terminal(TurnOutcome),
    Continue,
}

/// Primary coordinator entry point: orchestrates the 7-phase conversational turn lifecycle.
pub async fn execute_turn(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: TurnExecutionRequest,
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
async fn run_cognitive_loop(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: TurnExecutionRequest,
    stream_stage: StreamRoutingStage,
    turn_id: u32,
) -> TurnOutcome {
    let Some(ref pipeline_tx) = req.pipeline_tx else {
        return step7_handle_error(
            harness_arc,
            turn_id,
            "No pipeline event channel".to_string(),
            None,
        );
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
        live_turn_id: Arc::clone(&req.app_state.pipeline.turn_id),
        owner: req.owner,
        accumulator: Arc::clone(&req.accumulator),
        tts_tx: req.tts_tx.clone(),
        pending_synthesis_jobs: Arc::clone(&req.pending_synthesis_jobs),
        cancel: Arc::clone(&cancel_atomic),
        app_state: Arc::clone(&req.app_state),
        event_tx: pipeline_tx.clone(),
        emitter: Arc::clone(&req.emitter),
        turn_metrics: Arc::clone(&req.app_state.turn_metrics),
    };

    let session_id = {
        let guard = harness_arc.lock();
        guard
            .as_ref()
            .and_then(|h| h.session_id())
            .unwrap_or_else(|| req.app_state.conversation_id.load(Relaxed) as i64)
    };
    let (loop_ctx, supports_tools) = match TurnLoopContext::new(
        harness_arc,
        &req,
        &stream_stage,
        &stream_handles,
        session_id,
    ) {
        Ok(ctx_and_tools) => ctx_and_tools,
        Err(outcome) => {
            cancel_bridge.abort();
            return outcome;
        }
    };

    log::info!(
        "[Harness::Loop] Turn {} cognitive loop: supports_tools={}",
        turn_id,
        supports_tools,
    );

    let outcome = execute_loop_iterations(&loop_ctx, supports_tools, &cancel_atomic).await;
    cancel_bridge.abort();
    outcome
}

/// Executes iterative generation and tool interception passes until completion or budget breach.
async fn execute_loop_iterations(
    ctx: &TurnLoopContext<'_>,
    supports_tools: bool,
    cancel_atomic: &Arc<AtomicBool>,
) -> TurnOutcome {
    let turn_id = ctx.req.turn_id;
    let mut allow_tools = supports_tools;
    let mut iteration: usize = 0;
    let max_steps = {
        let guard = ctx.harness_arc.lock();
        guard
            .as_ref()
            .and_then(|h| h.max_steps)
            .unwrap_or(MAX_TOOL_STEPS)
    };

    // === REENTRANT COGNITIVE LOOP (Up to max_steps passes) ===
    while iteration <= max_steps {
        if ctx.req.cancel.is_cancelled() || cancel_atomic.load(Relaxed) {
            let partial = ctx
                .stream_handles
                .accumulator
                .lock()
                .assistant_response
                .clone();
            return step7_handle_cancelled(ctx.cancelled_ctx(partial));
        }

        step3_check_and_run_loop_budget(ctx, allow_tools, iteration).await;

        // === STEP 4: Assemble Generation Request ===
        let generation_request = match step4_assemble_request(ctx.harness_arc, turn_id, allow_tools)
        {
            Ok(req_obj) => req_obj,
            Err(e) => {
                return step7_handle_error(
                    ctx.harness_arc,
                    turn_id,
                    e,
                    Some(&ctx.stream_handles.event_tx),
                );
            }
        };

        // === STEP 5: Dispatch Request to Duplex LLM Pipe ===
        // The provider must abort on either a turn cancel or a Stage 1 stream pause.
        let gen_cancel = CancellationToken::new();
        let bridge_cancel = gen_cancel.clone();
        let turn_token = ctx.req.cancel.clone();
        let stream_token = ctx.req.app_state.pipeline.stream_cancel_token();
        let cancel_bridge = spawn_task(async move {
            tokio::select! {
                _ = turn_token.cancelled() => {},
                _ = stream_token.cancelled() => {},
            }
            bridge_cancel.cancel();
        });

        ctx.req.app_state.turn_metrics.record_llm_dispatch();
        let response_rx = match step5_dispatch_llm(
            ctx.req.llm_tx.as_ref(),
            generation_request,
            turn_id,
            &gen_cancel,
        ) {
            Ok(rx) => rx,
            Err(err) => {
                cancel_bridge.abort();
                return step7_handle_error(
                    ctx.harness_arc,
                    turn_id,
                    err,
                    Some(&ctx.stream_handles.event_tx),
                );
            }
        };

        // === STEP 6: Stream Tokens & Intercept Tool Calls ===
        let stream_res =
            step6_run_stream_pass(ctx.stream_stage, ctx.stream_handles, response_rx).await;
        cancel_bridge.abort();
        match stream_res {
            Ok(pass_outcome) => {
                let (action, circuit_broken) =
                    handle_pass_outcome(ctx, pass_outcome, cancel_atomic, allow_tools, iteration)
                        .await;
                if circuit_broken {
                    allow_tools = false;
                }
                match action {
                    // === STEP 7: Finalize & Terminal Commit ===
                    LoopAction::Terminal(outcome) => return outcome,
                    LoopAction::Continue => {
                        iteration += 1;
                        log::info!(
                            "[Harness::Loop] Turn {} iteration {} continuing",
                            turn_id,
                            iteration,
                        );
                        if iteration >= max_steps {
                            log::warn!(
                                "[Harness::Loop] Turn {} reached max_steps ({}); running final tool-free pass.",
                                turn_id,
                                max_steps
                            );
                            allow_tools = false;
                        }
                    }
                }
            }
            Err(err) => {
                return step7_handle_error(
                    ctx.harness_arc,
                    turn_id,
                    err,
                    Some(&ctx.stream_handles.event_tx),
                );
            }
        }
    }

    step7_handle_error(
        ctx.harness_arc,
        turn_id,
        "Exceeded max tool iterations without terminal response".to_string(),
        Some(&ctx.stream_handles.event_tx),
    )
}

/// Processes single-pass stream outcomes and branches to terminal resolution or reentrant looping.
async fn handle_pass_outcome(
    ctx: &TurnLoopContext<'_>,
    pass_outcome: StreamPassOutcome,
    cancel_atomic: &Arc<AtomicBool>,
    allow_tools: bool,
    iteration: usize,
) -> (LoopAction, bool) {
    let turn_id = ctx.req.turn_id;
    match pass_outcome {
        StreamPassOutcome::Completed {
            assistant_text,
            thinking_text: _,
        } => {
            log::info!(
                "[Harness::Loop] Turn {} pass completed with no tool call ({} chars); finalizing",
                turn_id,
                assistant_text.len()
            );
            if cancel_atomic.load(Relaxed) {
                (
                    LoopAction::Terminal(step7_handle_cancelled(ctx.cancelled_ctx(assistant_text))),
                    false,
                )
            } else {
                (
                    LoopAction::Terminal(step7_commit_completed(
                        ctx.harness_arc,
                        turn_id,
                        assistant_text,
                    )),
                    false,
                )
            }
        }
        StreamPassOutcome::PausedForSpeech { partial_text } => {
            if ctx.req.cancel.is_cancelled() || cancel_atomic.load(Relaxed) {
                return (
                    LoopAction::Terminal(step7_handle_cancelled(ctx.cancelled_ctx(partial_text))),
                    false,
                );
            }
            let inbox = ctx.harness_arc.lock().as_ref().map(|h| Arc::clone(h.inbox()));
            if let Some(inbox) = inbox {
                inbox.wait_if_speech_held(&ctx.req.cancel).await;
            }
            if ctx.req.cancel.is_cancelled() || cancel_atomic.load(Relaxed) {
                (
                    LoopAction::Terminal(step7_handle_cancelled(ctx.cancelled_ctx(partial_text))),
                    false,
                )
            } else {
                // False barge-in: Stage 2 rearmed the stream token; resume at Step 4.
                (LoopAction::Continue, false)
            }
        }
        StreamPassOutcome::Cancelled { partial_text } => (
            LoopAction::Terminal(step7_handle_cancelled(ctx.cancelled_ctx(partial_text))),
            false,
        ),
        StreamPassOutcome::Superseded { stale, current } => {
            log::warn!(
                "[Harness::Loop] Turn {} superseded by {}; discarding generation pass",
                stale,
                current
            );
            let partial = ctx
                .stream_handles
                .accumulator
                .lock()
                .assistant_response
                .clone();
            (
                LoopAction::Terminal(step7_handle_cancelled(ctx.cancelled_ctx(partial))),
                false,
            )
        }
        StreamPassOutcome::Error(err) => (
            LoopAction::Terminal(step7_handle_error(ctx.harness_arc, turn_id, err, None)),
            false,
        ),
        StreamPassOutcome::ToolCallsReceived {
            preamble_text,
            thinking_text,
            calls,
        } => {
            if !allow_tools {
                log::warn!(
                    "[Harness::Loop] Discarding {} tool calls during final tool-free pass; committing text.",
                    calls.len()
                );
                return (
                    LoopAction::Terminal(step7_commit_completed(
                        ctx.harness_arc,
                        turn_id,
                        preamble_text,
                    )),
                    false,
                );
            }

            let (cancelled, suspended_question, circuit_broken) =
                step6_execute_tool_batch(ctx, calls, &preamble_text, &thinking_text, iteration)
                    .await;
            if cancelled {
                (
                    LoopAction::Terminal(step7_handle_cancelled(ctx.cancelled_ctx(String::new()))),
                    circuit_broken,
                )
            } else if let Some(question) = suspended_question {
                let outcome = step7_commit_suspended_question(ctx, question);
                (LoopAction::Terminal(outcome), circuit_broken)
            } else {
                match step6_await_and_drain_inbox(ctx).await {
                    Ok(_) => (LoopAction::Continue, circuit_broken),
                    Err(outcome) => (LoopAction::Terminal(outcome), circuit_broken),
                }
            }
        }
    }
}
