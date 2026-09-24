use std::sync::{
    atomic::{AtomicU32, Ordering::Relaxed},
    mpsc, Arc,
};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    core::{
        events::{emit_ipc_to, AudioIntent, IpcEvent, LlmTokenPayload},
        settings::PipelineMode,
        state::{AppState, AppWindow, InteractionState},
    },
    persistence::PersistenceEvent,
    pipeline::router::{transition, RoutingContext},
    services::{
        harness::{
            chassis::Harness,
            current_timestamp_ms,
            r#loop::TurnLoopContext,
            stages::{
                compaction::{CompactionParams, CompactionStage},
                streaming::{
                    normalizer::TextNormalizer, StreamPassOutcome, StreamRoutingHandles,
                    StreamRoutingStage,
                },
                tools::{ToolExecutionContext, ToolExecutor, ToolFilter},
            },
            ChatMessage, Role, TurnExecutionRequest, TurnOutcome,
        },
        llm::{
            actor::{LlmCommand, LlmResponse},
            CanonicalToolCall, GenerationRequest,
        },
        tts::actor::TtsCommand,
    },
};

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
pub fn step1_intake<R: tauri::Runtime>(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: &TurnExecutionRequest<R>,
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

    let can_compact = harness.intake_recorded_turn(req.query.clone());

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
pub struct NonTerminalContext<'a, R: tauri::Runtime> {
    pub query: &'a str,
    pub turn_id: u32,
    pub routing_ctx: &'a RoutingContext,
    pub app: &'a tauri::AppHandle<R>,
    pub app_state: &'a Arc<AppState>,
    pub tts_tx: Option<&'a mpsc::Sender<TtsCommand>>,
    pub pending_synthesis_jobs: &'a Arc<AtomicU32>,
    pub play_filler: bool,
}

/// Uniformly transitions pipeline to Working and dispatches normalized interim filler audio.
pub fn enter_non_terminal_phase<R: tauri::Runtime>(
    phase: &NonTerminalPhase,
    ctx: &NonTerminalContext<'_, R>,
) {
    transition(
        InteractionState::Working,
        ctx.routing_ctx,
        ctx.app,
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
        ctx.pending_synthesis_jobs.fetch_add(1, Relaxed);
        if let Err(e) = tts_tx.send(TtsCommand::Generate {
            turn_id: ctx.turn_id,
            text: normalized_filler,
            intent: AudioIntent::InterimFiller,
        }) {
            log::warn!("[Phase] Failed to dispatch interim filler TTS: {}", e);
            ctx.pending_synthesis_jobs.fetch_sub(1, Relaxed);
        }
    }
}

/// Executes Phase 3: Generic Non-Terminal Phase (Inline compaction & maintenance).
pub async fn step3_execute_compaction<R: tauri::Runtime + 'static>(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: &TurnExecutionRequest<R>,
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
        app: &req.app,
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
    scratchpad: &[ChatMessage],
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
        let is_first_turn = harness.history.messages().len() <= 2;
        let filter = ToolFilter {
            mode: PipelineMode::Modular,
            is_first_turn,
            title_is_unset: !harness.title_set,
            memory_retrieval_enabled: harness.memory_retrieval_enabled,
        };
        let active = harness.tool_registry.active_definitions(&filter);
        if active.is_empty() {
            log::info!(
                "[Harness::Assemble] Turn {}: tools omitted (none active; first_turn={}, title_set={}, memory_enabled={})",
                turn_id,
                is_first_turn,
                !filter.title_is_unset,
                filter.memory_retrieval_enabled
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

    Ok(harness.prompt.build_generation_request(
        harness.history.messages(),
        scratchpad,
        harness.generation_options.clone(),
        tools,
    ))
}

// =========================================================================
// === STEP 5: MODEL PIPE DISPATCH ===
// =========================================================================

/// Dispatches an LlmCommand::Generate command over the actor channel.
pub fn step5_dispatch_llm(
    llm_tx: Option<&mpsc::Sender<LlmCommand>>,
    request: GenerationRequest,
    turn_id: u32,
    cancel: &CancellationToken,
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
    log::info!(
        "[Harness::Dispatch] Turn {} dispatched to LLM actor",
        turn_id
    );
    Ok(response_rx)
}

// =========================================================================
// === STEP 6: STREAM DEMUXING & TOOL INTERCEPTION ===
// =========================================================================

/// Executes a single streaming pass on an isolated blocking thread.
pub async fn step6_run_stream_pass<R: tauri::Runtime + 'static>(
    stream_stage: &StreamRoutingStage,
    stream_handles: &StreamRoutingHandles<R>,
    response_rx: mpsc::Receiver<LlmResponse>,
) -> Result<StreamPassOutcome, String> {
    let handles_clone = stream_handles.clone();
    let stage_clone = stream_stage.clone();
    tokio::task::spawn_blocking(move || stage_clone.route_stream(handles_clone, response_rx))
        .await
        .unwrap_or_else(|join_err| Err(format!("Stream task panic: {:?}", join_err)))
}

/// Handles Phase 6 Case B: Terminal tool execution (single pass, voice + action, commit).
pub async fn step6_handle_terminal_tool<R: tauri::Runtime + 'static>(
    ctx: &TurnLoopContext<'_, R>,
    call: CanonicalToolCall,
) -> TurnOutcome {
    let turn_id = ctx.req.turn_id;
    let spoken_response = call
        .arguments
        .get("spoken_response")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    ctx.stream_handles.accumulator.lock().assistant_response = spoken_response.clone();

    if !spoken_response.is_empty() {
        ctx.stream_stage.dispatch_spoken_response(
            &spoken_response,
            AudioIntent::TurnResponse,
            ctx.stream_handles,
        );
    }
    ctx.stream_stage.emit_finished(ctx.stream_handles);

    let tool_ctx = ToolExecutionContext {
        app_state: Arc::clone(&ctx.req.app_state),
        session_id: ctx.session_id,
        turn_id,
        cancel: ctx.req.cancel.clone(),
        on_sessions_changed: make_sessions_changed_callback(&ctx.req.app),
    };

    let outcome =
        ToolExecutor::execute_tool(ctx.tool_registry, PipelineMode::Modular, call, tool_ctx).await;
    if ctx.req.cancel.is_cancelled() {
        log::info!(
            "[Harness::Tools] Terminal tool cancelled (turn {})",
            turn_id
        );
        return step7_handle_cancelled(CancelledTurnContext {
            harness_arc: ctx.harness_arc,
            turn_id,
            session_id: ctx.session_id,
            user_text: ctx.req.query.clone(),
            partial_text: spoken_response,
            app_state: &ctx.req.app_state,
        });
    }

    let final_response = if !spoken_response.is_empty() {
        spoken_response
    } else {
        let fallback = outcome.result.spoken_response.unwrap_or_default();
        if !fallback.is_empty() {
            ctx.stream_handles.accumulator.lock().assistant_response = fallback.clone();
        }
        fallback
    };

    let mut guard = ctx.harness_arc.lock();
    if let Some(ref mut harness) = *guard {
        if outcome.tool_name == "respond_and_set_title" && !outcome.is_error {
            harness.title_set = true;
        }
        harness.history.push_assistant_turn(final_response.clone());
    }

    log::info!(
        "[Harness::Tools] Turn {} finalized by terminal tool '{}' (is_error={})",
        turn_id,
        outcome.tool_name,
        outcome.is_error
    );

    if !final_response.is_empty() {
        if let Err(e) = emit_ipc_to(
            &ctx.req.app,
            AppWindow::Main,
            IpcEvent::LlmToken(LlmTokenPayload {
                turn_id,
                token: final_response.clone(),
            }),
        ) {
            log::warn!(
                "[Harness::Tools] Failed to emit LlmToken for terminal tool response: {}",
                e
            );
        } else {
            log::info!(
                "[Harness::Tools] Emitted LlmToken ({} chars) for terminal tool response (turn {})",
                final_response.len(),
                turn_id
            );
        }
    }

    TurnOutcome::Completed {
        turn_id,
        assistant_response: final_response,
    }
}

/// Handles Phase 6 Case B: Non-terminal tool execution (interim filler, Working state, scratchpad).
pub async fn step6_handle_non_terminal_tool<R: tauri::Runtime + 'static>(
    ctx: &TurnLoopContext<'_, R>,
    call: CanonicalToolCall,
    partial_text: &str,
    scratchpad: &mut Vec<ChatMessage>,
) -> bool {
    let turn_id = ctx.req.turn_id;
    ctx.stream_handles
        .accumulator
        .lock()
        .assistant_response
        .clear();

    let spoken_filler = call
        .arguments
        .get("spoken_filler")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let non_terminal = NonTerminalPhase::tool(&call.name, &call.id, spoken_filler);
    let non_term_ctx = NonTerminalContext {
        query: &ctx.req.query,
        turn_id,
        routing_ctx: &ctx.req.routing_ctx,
        app: &ctx.req.app,
        app_state: &ctx.req.app_state,
        tts_tx: ctx.req.tts_tx.as_ref(),
        pending_synthesis_jobs: &ctx.req.pending_synthesis_jobs,
        play_filler: true,
    };
    enter_non_terminal_phase(&non_terminal, &non_term_ctx);

    let tool_ctx = ToolExecutionContext {
        app_state: Arc::clone(&ctx.req.app_state),
        session_id: ctx.session_id,
        turn_id,
        cancel: ctx.req.cancel.clone(),
        on_sessions_changed: make_sessions_changed_callback(&ctx.req.app),
    };

    let outcome = ToolExecutor::execute_tool(
        ctx.tool_registry,
        PipelineMode::Modular,
        call.clone(),
        tool_ctx,
    )
    .await;
    if ctx.req.cancel.is_cancelled() {
        log::info!(
            "[Harness::Tools] Non-terminal tool cancelled (turn {})",
            turn_id
        );
        return true;
    }

    scratchpad.push(ChatMessage {
        role: Role::Assistant,
        content: partial_text.to_string(),
        timestamp_ms: current_timestamp_ms(),
        tool_call_id: None,
        tool_calls: Some(vec![call.clone()]),
    });

    scratchpad.push(ChatMessage {
        role: Role::Tool,
        content: outcome.result.content,
        timestamp_ms: current_timestamp_ms(),
        tool_call_id: Some(call.id),
        tool_calls: None,
    });
    false
}

fn make_sessions_changed_callback<R: tauri::Runtime + 'static>(
    app: &tauri::AppHandle<R>,
) -> Option<Arc<dyn Fn() + Send + Sync>> {
    let app_clone = app.clone();
    Some(Arc::new(move || {
        if let Err(e) = emit_ipc_to(&app_clone, AppWindow::Main, IpcEvent::SessionsChanged) {
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
}

/// Handles Phase 7 Branch B: turn cancellation / user barge-in (Invariant 13).
pub fn step7_handle_cancelled(ctx: CancelledTurnContext<'_>) -> TurnOutcome {
    log::info!(
        "[Harness::Finalize] Finalizing cancelled turn {} (partial_chars {})",
        ctx.turn_id,
        ctx.partial_text.len()
    );

    let mut guard = ctx.harness_arc.lock();
    if !ctx.partial_text.trim().is_empty() {
        if let Some(ref mut harness) = *guard {
            harness
                .history
                .push_assistant_turn(ctx.partial_text.clone());
        }
        drop(guard);
        let persist_lock = ctx.app_state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(PersistenceEvent::TurnCompleted {
                session_id: ctx.session_id,
                turn_id: ctx.turn_id,
                user_text: ctx.user_text,
                assistant_text: ctx.partial_text,
            }) {
                log::warn!(
                    "[Harness::Finalize] Failed to persist interrupted turn {}: {}",
                    ctx.turn_id,
                    e
                );
            }
        }
    } else if let Some(ref mut harness) = *guard {
        harness.history.rollback_last_user_turn();
    }

    ctx.app_state.pipeline_accumulator.lock().clear();
    TurnOutcome::Cancelled {
        turn_id: ctx.turn_id,
    }
}

/// Handles Phase 7 Branch A: turn error and state rollback.
pub fn step7_handle_error(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    turn_id: u32,
    message: String,
) -> TurnOutcome {
    log::error!("[Harness::Finalize] Turn {} failed: {}", turn_id, message);

    let mut guard = harness_arc.lock();
    if let Some(ref mut harness) = *guard {
        harness.history.rollback_last_user_turn();
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
        harness
            .history
            .push_assistant_turn(assistant_response.clone());
    }

    TurnOutcome::Completed {
        turn_id,
        assistant_response,
    }
}
