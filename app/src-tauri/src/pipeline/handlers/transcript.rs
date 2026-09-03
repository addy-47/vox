use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;

use crate::core::events::{emit_ipc_to, IpcEvent, ToastLevel, TranscriptPayload};
use crate::core::settings::PipelineMode;
use crate::core::state::{AppState, InteractionState};
use crate::pipeline::{target_window, transition, RoutingContext};
use crate::services::llm::actor::LlmCommand;
use crate::toast::show_toast;

/// Resolves the provider classification based on the configured active LLM setting.
fn determine_provider_kind(
    active: &crate::core::settings::LlmActiveProvider,
) -> crate::services::llm::ProviderKind {
    match active {
        crate::core::settings::LlmActiveProvider::Embedded => {
            crate::services::llm::ProviderKind::Embedded
        }
        crate::core::settings::LlmActiveProvider::Server
        | crate::core::settings::LlmActiveProvider::Cloud => {
            crate::services::llm::ProviderKind::OpenAiCompat
        }
    }
}

/// Spawns the background asynchronous task to prepare conversational context and trigger LLM generation.
fn spawn_modular_llm_task(turn_id: u32, query: String, state: &AppState) {
    let settings = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let cm_arc = Arc::clone(&state.conversation_manager);
    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let cancel = state.pipeline.turn_token();
    let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
    let accumulator = Arc::clone(&state.pipeline_accumulator);

    let cached_provider = state.llm_provider.read().clone();
    let memory_tx = parking_lot::Mutex::new(state.memory_tx.lock().clone());
    let (tts_tx, llm_tx, pipeline_tx) = {
        let guard = state.engine.blocking_lock();
        guard
            .as_ref()
            .map(|e| (e.tts_tx.clone(), e.llm_tx.clone(), Some(e.pipeline_tx.clone())))
            .unwrap_or((None, None, None))
    };

    tauri::async_runtime::spawn(async move {
        let db_path = crate::utils::paths::db_path();
        let conn_opt = if settings.memory.context_retrieval_enabled {
            crate::persistence::db::VoxDb::open_readonly(&db_path)
                .await
                .ok()
        } else {
            None
        };

        let provider_kind = determine_provider_kind(&settings.llm.active);
        let session_id = conv_id.to_string();
        let res = crate::services::harness::prepare_turn_context(
            crate::services::harness::PrepareTurnParams {
                harness: &cm_arc,
                tts_tx: tts_tx.as_ref(),
                memory_tx: Some(&memory_tx),
                conn: conn_opt.as_ref(),
                query: &query,
                turn_id,
                session_id: &session_id,
                memory: &settings.memory,
                context_window: settings.llm.context_window as usize,
                provider_kind,
                llm_provider: cached_provider.as_deref(),
                llm_settings: Some(&settings.llm),
            },
        )
        .await;

        let (request, transition_speech) = match res {
            Ok((req, filler)) => (req, filler),
            Err(e) => {
                log::error!(
                    "[Pipeline::Transcript] Failed to prepare turn context: {}",
                    e
                );
                if let Some(ref p_tx) = pipeline_tx {
                    if let Err(send_err) = p_tx.send(crate::core::events::VoxEvent::Error {
                        turn_id,
                        message: format!("Turn context preparation failed: {}", e),
                        source: "CriticalCompaction".to_string(),
                    }) {
                        log::warn!(
                            "[Pipeline::Transcript] Failed to emit CriticalCompaction error: {}",
                            send_err
                        );
                    }
                }
                return;
            }
        };

        if cancel.is_cancelled() {
            log::info!(
                "[Pipeline::Transcript] Turn {} cancelled before LLM dispatch",
                turn_id
            );
            return;
        }

        if transition_speech.is_some() {
            pending_jobs.fetch_add(1, Ordering::Relaxed);
        }

        if let Some(ref tx) = llm_tx {
            if let Err(e) = tx.send(LlmCommand::Generate {
                request: Box::new(request),
                turn_id,
                cancel,
                accumulator,
                tts_tx,
                pending_synthesis_jobs: pending_jobs,
            }) {
                log::warn!(
                    "[Pipeline::Transcript] Failed to send Generate to LLM: {}",
                    e
                );
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
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        log::debug!(
            "[Pipeline::Transcript] TranscriptFinal dropped in {:?} state",
            current_state
        );
        return;
    }

    if current_state != InteractionState::Thinking {
        log::debug!(
            "[Pipeline::Transcript] TranscriptFinal dropped: state is {:?}, expected Thinking",
            current_state
        );
        return;
    }

    let transliterate_enabled = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .stt
        .transliterate_enabled;
    let processed_text =
        crate::services::translit::transliterate_if_hi(&text, true, transliterate_enabled);

    if processed_text.trim().is_empty() {
        state.pipeline_accumulator.lock().clear();
        transition(InteractionState::Ready, ctx, app, state);

        if let Err(e) = show_toast(
            app,
            "Voice Assistant",
            "No speech recognized",
            ToastLevel::Info,
        ) {
            log::warn!("[Pipeline::Transcript] Failed to show info toast: {}", e);
        }
        return;
    }

    state
        .pipeline_accumulator
        .lock()
        .set_user_transcript(processed_text.clone());

    let target = target_window(ctx.owner);
    if let Err(e) = emit_ipc_to(
        app,
        target,
        IpcEvent::TranscriptFinal(TranscriptPayload {
            turn_id,
            text: processed_text.clone(),
            owner: Some(ctx.owner),
        }),
    ) {
        log::warn!(
            "[Pipeline::Transcript] Failed to emit transcript_final to {}: {}",
            target,
            e
        );
    }

    if ctx.pipeline_mode == PipelineMode::Modular {
        spawn_modular_llm_task(turn_id, processed_text, state);
    } else if ctx.pipeline_mode == PipelineMode::Realtime {
        state
            .pipeline
            .pending_synthesis_jobs
            .store(1, Ordering::Relaxed);
    }

    log::info!(
        "[Pipeline::Transcript] TranscriptFinal processed (turn: {})",
        turn_id
    );
}
