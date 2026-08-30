use super::super::{
    transition, RoutingContext, EVENT_LLM_TOKEN, EVENT_PIPELINE_ERROR,
    EVENT_TRANSCRIPT_FINAL, EVENT_TRANSCRIPT_PARTIAL, WINDOW_MAIN,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand};
use crate::services::audio::PlaybackEngine;
use crate::services::llm::actor::LlmCommand;
use crate::services::tts::actor::{TtsClauseChunker, TtsCommand};
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};

static PTT_BUFFER: Mutex<Vec<f32>> = Mutex::new(Vec::new());
static CHUNKER: LazyLock<Mutex<TtsClauseChunker>> =
    LazyLock::new(|| Mutex::new(TtsClauseChunker::new()));
static CURRENT_ASSISTANT_RESPONSE: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));
static CURRENT_USER_TRANSCRIPT: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));

/// Ingests streaming audio frames into the Push-To-Talk buffer when recording is active.
pub fn ingest_audio(chunk: &[f32], state: &AppState) {
    if state.pipeline.state() == InteractionState::Listening {
        PTT_BUFFER.lock().extend_from_slice(chunk);
    }
}

/// Returns the current sample count in the Push-To-Talk buffer.
pub fn get_buffer_len() -> usize {
    PTT_BUFFER.lock().len()
}

/// Starts the modular Push-To-Talk session.
pub async fn start_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    crate::core::start_audio_engine(app, state).await?;
    super::ensure_modular_workers(app, state).await?;

    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let conv_id = now;
    state.conversation_id.store(conv_id, Ordering::Relaxed);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(
                crate::persistence::events::PersistenceEvent::SessionStarted {
                    id: conv_id,
                    timestamp_ms: now,
                },
            ) {
                log::warn!("[ModularPTT] Failed to send SessionStarted to persist: {}", e);
            }
        }
    }

    let prompt = state.settings.read().unwrap_or_else(|p| p.into_inner()).persona.modular_prompt.clone();
    super::super::init_new_session(state, &prompt).await;

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    log::info!("[ModularPTT] Modular PTT session started (ID: {})", conv_id);
    Ok(())
}

/// Ends the active modular Push-To-Talk session.
pub async fn end_session<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    PTT_BUFFER.lock().clear();
    CHUNKER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    if conv_id != 0 {
        let mem_lock = state.memory_tx.lock();
        if let Some(ref tx) = *mem_lock {
            if let Err(e) = tx.try_send(crate::persistence::memory_worker::MemoryWorkerEvent::SessionEnd {
                session_id: conv_id.to_string(),
                summary: String::new(),
            }) {
                log::trace!("[ModularPTT] Failed to send SessionEnd to memory worker: {}", e);
            }
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(
                crate::persistence::events::PersistenceEvent::SessionEnded {
                    id: conv_id,
                    timestamp_ms: now,
                },
            ) {
                log::warn!("[ModularPTT] Failed to send SessionEnded to persist: {}", e);
            }
        }
    }

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
        drop(guard);
        crate::core::stop_audio_engine(state).await?;
    } else {
        crate::core::stop_audio_engine(state).await?;
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Idle, &ctx, app, state);

    log::info!("[ModularPTT] Modular PTT session ended");
    Ok(())
}

/// Begins PTT recording and transitions state to listening.
pub fn ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    let current = state.pipeline.state();
    if current == InteractionState::Idle || current == InteractionState::Paused {
        return Err(format!("Cannot start PTT in {:?} state", current));
    }

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            let _ = engine.vad_tx.send(VadCommand::StartWindowValidation);
        }
    }

    PTT_BUFFER.lock().clear();
    CHUNKER.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    log::info!("[ModularPTT] PTT recording started");
    Ok(())
}

/// Stops PTT recording, processes buffered audio through STT, and initiates LLM/TTS generation.
pub fn ptt_stop<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if state.pipeline.state() != InteractionState::Listening {
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    let raw_samples = PTT_BUFFER.lock().clone();
    PTT_BUFFER.lock().clear();

    if raw_samples.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return Ok(());
    }

    let guard = state.engine.blocking_lock();
    let engine = match guard.as_ref() {
        Some(eng) => eng,
        None => {
            let ctx = RoutingContext::from_app_state(state);
            transition(InteractionState::Ready, &ctx, app, state);
            return Ok(());
        }
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let validation_result = if engine
        .vad_tx
        .send(VadCommand::StopWindowValidation { response_tx: tx })
        .is_ok()
    {
        rx.recv_timeout(std::time::Duration::from_millis(500)).ok()
    } else {
        None
    };

    let is_speech = match validation_result {
        Some(ref val) => val.is_speech_detected,
        None => true,
    };

    if !is_speech {
        log::info!("[ModularPTT] Non-speech PTT hold discarded without STT request");
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return Ok(());
    }

    let trimmed = match validation_result {
        Some(ref val) => {
            let start = val.speech_start_sample.min(raw_samples.len());
            let end = val.speech_end_sample.min(raw_samples.len());
            if start < end && (end - start) >= 256 {
                raw_samples[start..end].to_vec()
            } else {
                raw_samples
            }
        }
        None => raw_samples,
    };

    if trimmed.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return Ok(());
    }

    let turn_id = state.pipeline.next_turn_id();
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = engine.stt_tx.send(crate::services::stt::SttCommand::Final(
        turn_id,
        trimmed,
    )) {
        log::warn!("[ModularPTT] Failed to send Final to STT: {}", e);
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
    }

    log::info!("[ModularPTT] PTT recording stopped (turn {})", turn_id);
    Ok(())
}

/// Cancels ongoing PTT interaction and resets pipeline state machine to Ready.
pub fn ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    PTT_BUFFER.lock().clear();
    CHUNKER.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    log::info!("[ModularPTT] PTT cancelled");
    Ok(())
}

/// Handles speech start event for Push-To-Talk mode.
fn on_speech_start<R: tauri::Runtime>(_app: &AppHandle<R>, _state: &AppState) {}

/// Handles speech end event for Push-To-Talk mode.
fn on_speech_end<R: tauri::Runtime>(_app: &AppHandle<R>, _state: &AppState) {}

/// Handles interim partial speech recognition results.
fn on_transcript_partial<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>, state: &AppState) {
    let transliterate_enabled = state.settings.read().unwrap_or_else(|p| p.into_inner()).stt.transliterate_enabled;
    let processed_text = crate::services::translit::transliterate_if_hi(&text, false, transliterate_enabled);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_TRANSCRIPT_PARTIAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": processed_text,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit transcript_partial: {}", e);
    }
}

/// Handles finalized speech transcript and initiates LLM generation workflow.
fn on_transcript_final<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>, state: &AppState) {
    let transliterate_enabled = state.settings.read().unwrap_or_else(|p| p.into_inner()).stt.transliterate_enabled;
    let processed_text = crate::services::translit::transliterate_if_hi(&text, true, transliterate_enabled);

    if processed_text.trim().is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return;
    }

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_TRANSCRIPT_FINAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": processed_text,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit transcript_final: {}", e);
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    CURRENT_ASSISTANT_RESPONSE.lock().clear();
    *CURRENT_USER_TRANSCRIPT.lock() = processed_text.clone();

    let settings = state.settings.read().unwrap_or_else(|p| p.into_inner()).clone();
    let cm_arc = Arc::clone(&state.conversation_manager);
    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let cancel = state.pipeline.turn_token();

    let cached_provider = state.llm_provider.read().clone();
    let memory_tx = Arc::new(parking_lot::Mutex::new(state.memory_tx.lock().clone()));
    let (tts_tx, llm_tx) = {
        let guard = state.engine.blocking_lock();
        guard
            .as_ref()
            .map(|e| (e.tts_tx.clone(), e.llm_tx.clone()))
            .unwrap_or((None, None))
    };

    tauri::async_runtime::spawn(async move {
        let db_path = crate::utils::paths::db_path();
        let conn_opt = if settings.memory.context_retrieval_enabled {
            crate::persistence::db::VoxDb::open_readonly(&db_path).await.ok()
        } else {
            None
        };

        let provider_kind = match settings.llm.active {
            crate::core::settings::LlmActiveProvider::Embedded => crate::services::llm::ProviderKind::Embedded,
            crate::core::settings::LlmActiveProvider::Server
            | crate::core::settings::LlmActiveProvider::Cloud => crate::services::llm::ProviderKind::OpenAiCompat,
        };

        let session_id = conv_id.to_string();
        let res = crate::services::memory::prepare_turn_context(
            crate::services::memory::PrepareTurnParams {
                harness: &cm_arc,
                tts_tx: tts_tx.as_ref(),
                memory_tx: Some(&memory_tx),
                conn: conn_opt.as_ref(),
                query: &processed_text,
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
                log::error!("[ModularPTT] Failed to prepare turn context: {}", e);
                return;
            }
        };

        if let Some(filler) = transition_speech {
            if let Some(ref tx) = tts_tx {
                if let Err(e) = tx.send(TtsCommand::Generate {
                    turn_id,
                    text: filler,
                }) {
                    log::warn!("[ModularPTT] Failed to send filler TTS: {}", e);
                }
            }
        }

        if let Some(ref tx) = llm_tx {
            if let Err(e) = tx.send(LlmCommand::Generate {
                request,
                turn_id,
                cancel,
            }) {
                log::warn!("[ModularPTT] Failed to send Generate to LLM: {}", e);
            }
        }
    });
}

/// Handles incoming streamed tokens from the active LLM provider.
fn on_llm_token<R: tauri::Runtime>(turn_id: u32, token: String, app: &AppHandle<R>, state: &AppState) {
    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_LLM_TOKEN,
        serde_json::json!({
            "turn_id": turn_id,
            "token": token,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit llm_token: {}", e);
    }

    let clauses = {
        CURRENT_ASSISTANT_RESPONSE.lock().push_str(&token);
        CHUNKER.lock().push_str(&token)
    };
    if !clauses.is_empty() {
        let guard = state.engine.blocking_lock();
        if let Some(ref engine) = *guard {
            if let Some(ref tx) = engine.tts_tx {
                for clause in clauses {
                    if let Err(e) = tx.send(TtsCommand::Generate {
                        turn_id,
                        text: clause,
                    }) {
                        log::warn!("[ModularPTT] Failed to send Generate to TTS: {}", e);
                    }
                }
            }
        }
    }
}

/// Finalizes LLM output generation, flushes remaining TTS audio, and persists turn context.
fn on_llm_finished(turn_id: u32, state: &AppState) {
    if let Some(remainder) = CHUNKER.lock().flush() {
        let guard = state.engine.blocking_lock();
        if let Some(ref engine) = *guard {
            if let Some(ref tx) = engine.tts_tx {
                if let Err(e) = tx.send(TtsCommand::Generate {
                    turn_id,
                    text: remainder,
                }) {
                    log::warn!("[ModularPTT] Failed to send Generate to TTS: {}", e);
                }
            }
        }
    }

    let full_text = CURRENT_ASSISTANT_RESPONSE.lock().split_off(0);
    if !full_text.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(full_text.clone());

        let conv_id = state.conversation_id.load(Ordering::Relaxed);
        let user_text = CURRENT_USER_TRANSCRIPT.lock().clone();
        let stt_ms = state.telemetry.latest_stt_ms.load(Ordering::Relaxed);
        let ttft_ms = state.telemetry.latest_ttft_ms.load(Ordering::Relaxed);
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(crate::persistence::events::PersistenceEvent::TurnCompleted {
                conversation_id: conv_id,
                turn_id,
                user_text,
                assistant_text: full_text,
                stt_latency_ms: stt_ms,
                ttft_ms,
            }) {
                log::warn!("[ModularPTT] Failed to send TurnCompleted to persist: {}", e);
            }
        }
    }
}

/// Forwards synthesized audio samples to the audio playback buffer.
fn on_tts_chunk(samples: Vec<f32>, playback: &Arc<PlaybackEngine>) {
    playback.ingest_chunk(&samples);
}

/// Updates latest TTS real-time factor metrics upon synthesis completion.
fn on_tts_finished(rtf: f32, state: &AppState) {
    state.telemetry.latest_tts_rtf.store(rtf.to_bits(), Ordering::Relaxed);
}

/// Transitions pipeline state to assistant speaking when audio playback begins.
fn on_playback_started<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Speaking, &ctx, app, state);
}

/// Finalizes assistant response playback and transitions pipeline back to ready state.
fn on_playback_finished<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);
}

/// Logs pipeline errors and transitions state machine to error condition.
fn on_error<R: tauri::Runtime>(turn_id: u32, message: String, app: &AppHandle<R>, state: &AppState) {
    log::error!("[ModularPTT] Error on turn {}: {}", turn_id, message);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Error, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_MAIN,
        EVENT_PIPELINE_ERROR,
        serde_json::json!({
            "turn_id": turn_id,
            "message": message,
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit pipeline_error: {}", e);
    }
}

/// Handles cancellation event and resets state machine to Ready.
fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!("[ModularPTT] Interaction cancelled on turn {}", turn_id);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);
}

/// Main event dispatcher for the modular Push-To-Talk pipeline domain.
pub fn handle_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match event {
        VoxEvent::SpeechStart { .. } => on_speech_start(app, state),
        VoxEvent::SpeechEnd { .. } => on_speech_end(app, state),
        VoxEvent::TranscriptPartial { turn_id, text } => {
            on_transcript_partial(turn_id, text, app, state)
        }
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::LlmToken { turn_id, token } => on_llm_token(turn_id, token, app, state),
        VoxEvent::LlmFinished { turn_id } => on_llm_finished(turn_id, state),
        VoxEvent::TtsChunk { samples, .. } => on_tts_chunk(samples, playback),
        VoxEvent::TtsFinished { rtf, .. } => on_tts_finished(rtf, state),
        VoxEvent::PlaybackStarted { .. } => on_playback_started(app, state),
        VoxEvent::PlaybackFinished { .. } => on_playback_finished(app, state),
        VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
