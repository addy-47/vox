use std::sync::{
    atomic::{AtomicBool, Ordering::Relaxed},
    mpsc, Arc,
};

use parking_lot::Mutex;

use crate::services::{
    harness::{
        stages::{
            streaming::{StreamPassOutcome, StreamRoutingHandles, StreamRoutingStage},
            tools::{ToolFilter, ToolRegistry},
        },
        ChatMessage,
    },
    llm::{
        actor::{LlmCommand, LlmResponse},
        ConversationInput, GenerationPurpose, GenerationRequest, OutputConstraint, ToolFlow,
    },
};

use super::{
    finalize::{commit_completed_turn, handle_turn_cancelled, handle_turn_error},
    tools::{handle_non_terminal_tool, handle_terminal_tool},
    Harness, TurnExecutionRequest, TurnOutcome,
};

const MAX_TOOL_ITERATIONS: usize = 5;

pub struct TurnLoopContext<'a, R: tauri::Runtime> {
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

/// Orchestrates Phase 4 (Assembly), Phase 5 (Dispatch), and Phase 6 (Reentrant Loop).
pub async fn run_cognitive_loop<R: tauri::Runtime + 'static>(
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

    let Some(ref pipeline_tx) = req.pipeline_tx else {
        return handle_turn_error(harness_arc, turn_id, "No pipeline event channel".to_string());
    };

    let cancel_atomic = Arc::new(AtomicBool::new(req.cancel.is_cancelled()));
    let cancel_atomic_clone = Arc::clone(&cancel_atomic);
    let cancel_token_clone = req.cancel.clone();
    let cancel_bridge = tauri::async_runtime::spawn(async move {
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
async fn execute_loop_iterations<R: tauri::Runtime + 'static>(
    ctx: &TurnLoopContext<'_, R>,
    supports_tools: bool,
    cancel_atomic: &Arc<AtomicBool>,
) -> TurnOutcome {
    let turn_id = ctx.req.turn_id;
    let mut scratchpad: Vec<ChatMessage> = Vec::new();
    let mut allow_tools = supports_tools;
    let mut iteration: usize = 0;

    while iteration <= MAX_TOOL_ITERATIONS {
        if ctx.req.cancel.is_cancelled() || cancel_atomic.load(Relaxed) {
            return handle_turn_cancelled(ctx.harness_arc, turn_id);
        }

        let generation_request = match build_generation_request(
            ctx.harness_arc,
            &scratchpad,
            turn_id,
            allow_tools,
        ) {
            Ok(req_obj) => req_obj,
            Err(e) => return handle_turn_error(ctx.harness_arc, turn_id, e),
        };

        let response_rx = match dispatch_llm_request(
            ctx.req.llm_tx.as_ref(),
            generation_request,
            turn_id,
            &ctx.req.cancel,
        ) {
            Ok(rx) => rx,
            Err(err) => return handle_turn_error(ctx.harness_arc, turn_id, err),
        };

        let stream_res = run_stream_pass(ctx.stream_stage, ctx.stream_handles, response_rx).await;
        match stream_res {
            Ok(pass_outcome) => {
                let action = handle_pass_outcome(
                    ctx,
                    pass_outcome,
                    &mut scratchpad,
                    cancel_atomic,
                )
                .await;
                match action {
                    LoopAction::Terminal(outcome) => return outcome,
                    LoopAction::Continue => {
                        iteration += 1;
                        if iteration >= MAX_TOOL_ITERATIONS {
                            log::warn!(
                                "[Harness::Loop] Turn {} reached MAX_TOOL_ITERATIONS; disabling tools.",
                                turn_id
                            );
                            allow_tools = false;
                        }
                    }
                }
            }
            Err(err) => return handle_turn_error(ctx.harness_arc, turn_id, err),
        }
    }

    handle_turn_error(
        ctx.harness_arc,
        turn_id,
        "Exceeded max tool iterations without terminal response".to_string(),
    )
}

/// Assembles the complete GenerationRequest payload including history, scratchpad, and tools.
fn build_generation_request(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    scratchpad: &[ChatMessage],
    turn_id: u32,
    allow_tools: bool,
) -> Result<GenerationRequest, String> {
    let guard = harness_arc.lock();
    let Some(ref harness) = *guard else {
        return Err("Harness unmounted during execution".to_string());
    };

    let mut messages = harness.history.messages().to_vec();
    messages.extend_from_slice(scratchpad);

    let tools = if harness.supports_tools && allow_tools {
        let filter = ToolFilter {
            turn_id,
            title_is_unset: !harness.title_set,
            memory_retrieval_enabled: true,
        };
        let active = harness.tool_registry.active_definitions(&filter);
        if active.is_empty() {
            None
        } else {
            Some(active)
        }
    } else {
        None
    };

    Ok(GenerationRequest {
        input: ConversationInput { messages },
        options: harness.generation_options.clone(),
        output: OutputConstraint::Text,
        purpose: GenerationPurpose::Conversation,
        tools,
    })
}

/// Dispatches an LlmCommand::Generate command over the actor channel.
fn dispatch_llm_request(
    llm_tx: Option<&mpsc::Sender<LlmCommand>>,
    request: GenerationRequest,
    turn_id: u32,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<mpsc::Receiver<LlmResponse>, String> {
    let Some(tx) = llm_tx else {
        return Err("No LLM channel available".to_string());
    };
    let (response_tx, response_rx) = mpsc::channel();
    let cmd = LlmCommand::Generate {
        request: Box::new(request),
        turn_id,
        cancel: cancel.clone(),
        response_tx,
    };
    tx.send(cmd)
        .map_err(|e| format!("Failed to dispatch LlmCommand: {}", e))?;
    Ok(response_rx)
}

/// Executes a single streaming pass on an isolated blocking thread.
async fn run_stream_pass<R: tauri::Runtime + 'static>(
    stream_stage: &StreamRoutingStage,
    stream_handles: &StreamRoutingHandles<R>,
    response_rx: mpsc::Receiver<LlmResponse>,
) -> Result<StreamPassOutcome, String> {
    let handles_clone = stream_handles.clone();
    let stage_clone = stream_stage.clone();
    tokio::task::spawn_blocking(move || {
        stage_clone.route_stream(handles_clone, response_rx)
    })
    .await
    .unwrap_or_else(|join_err| Err(format!("Stream task panic: {:?}", join_err)))
}

/// Processes single-pass stream outcomes and branches to terminal resolution or reentrant looping.
async fn handle_pass_outcome<R: tauri::Runtime + 'static>(
    ctx: &TurnLoopContext<'_, R>,
    pass_outcome: StreamPassOutcome,
    scratchpad: &mut Vec<ChatMessage>,
    cancel_atomic: &Arc<AtomicBool>,
) -> LoopAction {
    let turn_id = ctx.req.turn_id;
    match pass_outcome {
        StreamPassOutcome::Completed { assistant_text } => {
            if cancel_atomic.load(Relaxed) {
                LoopAction::Terminal(handle_turn_cancelled(ctx.harness_arc, turn_id))
            } else {
                LoopAction::Terminal(commit_completed_turn(
                    ctx.harness_arc,
                    turn_id,
                    assistant_text,
                ))
            }
        }
        StreamPassOutcome::Cancelled { .. } => {
            LoopAction::Terminal(handle_turn_cancelled(ctx.harness_arc, turn_id))
        }
        StreamPassOutcome::Error(err) => {
            LoopAction::Terminal(handle_turn_error(ctx.harness_arc, turn_id, err))
        }
        StreamPassOutcome::ToolCallReceived { partial_text, call } => {
            let tool_flow = ctx
                .tool_registry
                .get(&call.name)
                .map(|t| t.flow())
                .unwrap_or(ToolFlow::Terminal);

            if tool_flow == ToolFlow::Terminal {
                let outcome = handle_terminal_tool(ctx, call).await;
                LoopAction::Terminal(outcome)
            } else {
                handle_non_terminal_tool(ctx, call, &partial_text, scratchpad).await;
                LoopAction::Continue
            }
        }
    }
}
