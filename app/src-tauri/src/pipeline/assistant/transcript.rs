use std::sync::{atomic::Ordering, Arc};

use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc_to, AudioIntent, IpcEvent, TranscriptPayload},
        settings::PipelineMode,
        state::{AppState, InteractionState},
    },
    pipeline::{target_window, transition, RoutingContext},
    services::{
        harness::{StreamRoutingHandles, StreamRoutingPlugin, TurnPreparation},
        llm::{
            actor::LlmCommand, ConversationInput, GenerationPurpose, GenerationRequest,
            OutputConstraint,
        },
        memory::compaction::runner::run_compaction,
        translit::transliterate_if_hi,
        tts::actor::TtsCommand,
    },
};

/// Spawns the background asynchronous task to prepare conversational context and trigger LLM generation.
fn spawn_modular_llm_task<R: tauri::Runtime + 'static>(
    turn_id: u32,
    query: String,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let settings = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let cancel = state.pipeline.turn_token();
    let cancel_flag = Arc::clone(&state.pipeline.cancel_flag);
    let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
    let accumulator = Arc::clone(&state.pipeline_accumulator);
    let app_clone = app.clone();
    let ctx_owner = ctx.owner;

    let (tts_tx, llm_tx, pipeline_tx) = match state.engine.try_lock() {
        Ok(guard) => guard
            .as_ref()
            .map(|e| {
                (
                    e.tts_tx.clone(),
                    e.llm_tx.clone(),
                    Some(e.pipeline_tx.clone()),
                )
            })
            .unwrap_or((None, None, None)),
        Err(_) => {
            log::warn!("[Pipeline::Transcript] Engine lock contended; could not access channels");
            (None, None, None)
        }
    };

    let harness_arc = Arc::clone(&state.harness);
    let provider_arc = Arc::clone(&state.llm_provider);

    tauri::async_runtime::spawn(async move {
        let prep = {
            let mut guard = harness_arc.lock();
            let Some(ref mut harness) = *guard else {
                log::error!("[Pipeline::Transcript] No active HarnessSession mounted");
                return;
            };
            harness.prepare_turn(&query, turn_id)
        };

        let request = match prep {
            TurnPreparation::DuplicateTurnIgnored => {
                log::info!(
                    "[Pipeline::Transcript] Duplicate turn ignored (turn {})",
                    turn_id
                );
                return;
            }
            TurnPreparation::Ready(req) => req,
            TurnPreparation::NeedsInlineCompaction {
                filler_phrase,
                uncompacted_slice,
            } => {
                log::info!(
                    "[Pipeline::Transcript] Context threshold >= 85%. Transitioning to Working."
                );

                if let Some(ref t_tx) = tts_tx {
                    pending_jobs.fetch_add(1, Ordering::Relaxed);
                    if let Err(e) = t_tx.send(TtsCommand::Generate {
                        turn_id,
                        text: filler_phrase.to_string(),
                        intent: AudioIntent::InterimFiller,
                    }) {
                        log::warn!(
                            "[Pipeline::Transcript] Failed to dispatch filler to TTS: {}",
                            e
                        );
                    }
                }

                let provider_opt = provider_arc.read().clone();
                if let Some(provider) = provider_opt {
                    let compaction_res = run_compaction(
                        provider.as_ref(),
                        &uncompacted_slice,
                        Some(&settings.llm),
                        Some(&cancel),
                    )
                    .await;

                    let mut guard = harness_arc.lock();
                    if let Some(ref mut harness) = *guard {
                        match compaction_res {
                            Ok(result) => {
                                harness.apply_compaction_summary(&result.context_summary, &query);
                            }
                            Err(e) => {
                                log::warn!("[Pipeline::Transcript] Compaction error ({}). Falling back to FIFO.", e);
                                if let Some(ref budget) = harness.budget {
                                    budget.execute_fifo_shift(&mut harness.history);
                                }
                            }
                        }
                    }
                }

                let guard = harness_arc.lock();
                let Some(ref harness) = *guard else {
                    return;
                };
                let input = ConversationInput {
                    messages: harness.history.messages().to_vec(),
                };
                GenerationRequest {
                    input,
                    options: Default::default(),
                    output: OutputConstraint::Text,
                    purpose: GenerationPurpose::Conversation,
                }
            }
        };

        if cancel.is_cancelled() {
            log::info!(
                "[Pipeline::Transcript] Turn {} cancelled before LLM dispatch",
                turn_id
            );
            return;
        }

        let (response_tx, response_rx) = std::sync::mpsc::channel();

        if let Some(ref tx) = llm_tx {
            if let Err(e) = tx.send(LlmCommand::Generate {
                request: Box::new(request),
                turn_id,
                cancel,
                response_tx,
            }) {
                log::warn!(
                    "[Pipeline::Transcript] Failed to send Generate to LLM: {}",
                    e
                );
                return;
            }
        }

        if let Some(ref p_tx) = pipeline_tx {
            let stream_plugin = StreamRoutingPlugin::new();
            let handles = StreamRoutingHandles {
                turn_id,
                owner: ctx_owner,
                accumulator,
                tts_tx: tts_tx.as_ref(),
                pending_synthesis_jobs: &pending_jobs,
                cancel: &cancel_flag,
                event_tx: p_tx,
                app: &app_clone,
            };
            if let Err(e) = stream_plugin.route_stream(handles, response_rx) {
                log::warn!("[Pipeline::Transcript] Stream routing failed: {}", e);
            }
        }
    });
}

/// Handles finalized speech transcript, validating non-empty text and routing to LLM or idle recovery.
pub fn on_transcript_final<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Listening
        && current_state != InteractionState::Thinking
        && current_state != InteractionState::Ready
    {
        log::debug!(
            "[Pipeline::Transcript] Transcript dropped (state: {:?})",
            current_state
        );
        return;
    }

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        log::debug!(
            "[Pipeline::Transcript] Dropping empty transcript for turn {}",
            turn_id
        );
        transition(InteractionState::Ready, ctx, app, state);
        return;
    }

    let transliterate_enabled = state
        .settings
        .read()
        .map(|s| s.stt.transliterate_enabled)
        .unwrap_or(false);
    let query = transliterate_if_hi(&trimmed, true, transliterate_enabled);
    log::info!(
        "[Pipeline::Transcript] Turn {}: User said: '{}'",
        turn_id,
        query
    );

    state
        .pipeline_accumulator
        .lock()
        .set_user_transcript(query.clone());

    let payload = TranscriptPayload {
        turn_id,
        text: query.clone(),
        owner: Some(ctx.owner),
    };
    if let Err(e) = emit_ipc_to(
        app,
        target_window(ctx.owner),
        IpcEvent::TranscriptFinal(payload),
    ) {
        log::warn!(
            "[Pipeline::Transcript] Failed to emit TranscriptFinal IPC: {}",
            e
        );
    }

    transition(InteractionState::Thinking, ctx, app, state);

    match ctx.pipeline_mode {
        PipelineMode::Modular => spawn_modular_llm_task(turn_id, query, app, state, ctx),
        PipelineMode::Realtime => {
            log::info!(
                "[Pipeline::Transcript] Turn {} transcript processed in Realtime mode",
                turn_id
            );
        }
    }
}
