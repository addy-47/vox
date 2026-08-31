use super::super::{transition, RoutingContext, WINDOW_MAIN};
use crate::core::events::VoiceErrorPayload;
use crate::core::events::{emit_ipc_to, IpcEvent, LlmTokenPayload, TranscriptPayload, VoxEvent};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::audio::PlaybackEngine;
use crate::services::llm::actor::LlmCommand;
use crate::services::tts::actor::{TtsClauseChunker, TtsCommand};
use crate::services::vad::VadCommand;
use crate::services::vad::VadOperationalMode;
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use tauri::AppHandle;

struct TurnAccumulator {
    chunker: TtsClauseChunker,
    assistant_response: String,
    user_transcript: String,
}

impl TurnAccumulator {
    fn new() -> Self {
        Self {
            chunker: TtsClauseChunker::new(),
            assistant_response: String::new(),
            user_transcript: String::new(),
        }
    }

    fn clear(&mut self) {
        self.chunker.clear();
        self.assistant_response.clear();
        self.user_transcript.clear();
    }

    fn push_token(&mut self, token: &str) -> Vec<String> {
        self.assistant_response.push_str(token);
        self.chunker.push_str(token)
    }

    fn flush_chunker(&mut self) -> Option<String> {
        self.chunker.flush()
    }

    fn set_user_transcript(&mut self, text: String) {
        self.user_transcript = text;
    }

    fn take_assistant_response(&mut self) -> String {
        std::mem::take(&mut self.assistant_response)
    }

    fn user_transcript(&self) -> String {
        self.user_transcript.clone()
    }
}

static ACCUMULATOR: LazyLock<Mutex<TurnAccumulator>> =
    LazyLock::new(|| Mutex::new(TurnAccumulator::new()));

/// Starts the modular Push-To-Talk session.
pub async fn start_session<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    super::ensure_modular_workers(app, state).await?;

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(
                VadOperationalMode::WindowedValidation,
            )) {
                log::warn!("[ModularPTT] Failed to set VAD operational mode: {}", e);
            }
        }
    }

    ACCUMULATOR.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    log::info!("[ModularPTT] Modular PTT session started (ID: {})", conv_id);
    Ok(())
}

/// Ends the active modular Push-To-Talk session.
pub async fn end_session<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    ACCUMULATOR.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
    }

    log::info!("[ModularPTT] Modular PTT session ended");
    Ok(())
}

/// Handles dedicated barge-in interruption for modular PTT pipeline.
pub fn on_interrupt<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
    }

    state.pipeline.renew_turn_token();

    let partial_assistant = ACCUMULATOR.lock().take_assistant_response();
    let user_text = ACCUMULATOR.lock().user_transcript();
    let interrupted_turn_id = state.pipeline.peek_turn_id();
    let conv_id = state.conversation_id.load(Ordering::Relaxed);

    if !partial_assistant.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(partial_assistant.clone());
    }

    let persist_lock = state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        if let Err(e) = tx.try_send(
            crate::persistence::events::PersistenceEvent::TurnCompleted {
                conversation_id: conv_id,
                turn_id: interrupted_turn_id,
                user_text,
                assistant_text: partial_assistant,
                stt_latency_ms: 0,
                ttft_ms: 0,
            },
        ) {
            log::warn!(
                "[ModularPTT] Failed to send TurnCompleted on interrupt: {}",
                e
            );
        }
    }

    ACCUMULATOR.lock().clear();

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);
    log::info!(
        "[ModularPTT] Interruption handled (turn: {})",
        interrupted_turn_id
    );
}

/// Handles Push-To-Talk button press event and initiates audio accumulation.
pub fn ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        return Err("Modular PTT session not active".to_string());
    }

    if current_state == InteractionState::Thinking || current_state == InteractionState::Speaking {
        on_interrupt(app, state);
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    let (turn_id, _) = state.pipeline.next_turn();
    ACCUMULATOR.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if let Err(e) = engine.vad_tx.send(VadCommand::StartWindowValidation) {
                log::warn!("[ModularPTT] Failed to start window validation: {}", e);
            }
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    log::info!("[ModularPTT] PTT recording started (turn {})", turn_id);
    Ok(())
}

/// Handles Push-To-Talk button release event and triggers STT transcription on speech detection.
pub async fn ptt_stop<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    if state.pipeline.state() != InteractionState::Listening {
        return Ok(());
    }

    let turn_id = state.pipeline.peek_turn_id();
    let guard = state.engine.lock().await;
    let engine = match guard.as_ref() {
        Some(e) => e,
        None => return Err("Audio engine is not initialized".to_string()),
    };

    let (tx, rx) = std::sync::mpsc::channel();
    if let Err(e) = engine
        .vad_tx
        .send(VadCommand::StopWindowValidation { response_tx: tx })
    {
        log::warn!("[ModularPTT] Failed to stop window validation: {}", e);
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return Err("Failed to query VAD actor".to_string());
    }

    let validation_result = match rx.recv_timeout(std::time::Duration::from_millis(500)) {
        Ok(result) => Some(result),
        Err(e) => {
            log::warn!("[ModularPTT] VAD window validation timeout: {}", e);
            None
        }
    };

    let (is_speech, audio) = match validation_result {
        Some(val) => (val.is_speech_detected, val.audio),
        None => (false, Vec::new()),
    };

    if !is_speech || audio.is_empty() {
        log::info!(
            "[ModularPTT] Non-speech PTT hold discarded without STT request (turn {})",
            turn_id
        );
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = engine
        .stt_tx
        .send(crate::services::stt::SttCommand::Final(turn_id, audio))
    {
        log::warn!("[ModularPTT] Failed to send Final to STT: {}", e);
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
    }

    log::info!("[ModularPTT] PTT recording stopped (turn {})", turn_id);
    Ok(())
}

/// Cancels ongoing PTT interaction and resets pipeline state machine to Ready.
pub fn ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    ACCUMULATOR.lock().clear();
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

/// Handles interim partial speech recognition results.
fn on_transcript_partial<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    let transliterate_enabled = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .stt
        .transliterate_enabled;
    let processed_text =
        crate::services::translit::transliterate_if_hi(&text, false, transliterate_enabled);

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::TranscriptPartial(TranscriptPayload {
            turn_id,
            text: processed_text,
            owner: Some(InteractionOwner::Assistant),
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit transcript_partial: {}", e);
    }
}

/// Handles finalized speech transcript and initiates LLM generation workflow.
fn on_transcript_final<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    let transliterate_enabled = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .stt
        .transliterate_enabled;
    let processed_text =
        crate::services::translit::transliterate_if_hi(&text, true, transliterate_enabled);

    if processed_text.trim().is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Ready, &ctx, app, state);
        return;
    }

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::TranscriptFinal(TranscriptPayload {
            turn_id,
            text: processed_text.clone(),
            owner: Some(InteractionOwner::Assistant),
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit transcript_final: {}", e);
    }

    {
        let mut acc = ACCUMULATOR.lock();
        acc.clear();
        acc.set_user_transcript(processed_text.clone());
    }

    let settings = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
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
            crate::persistence::db::VoxDb::open_readonly(&db_path)
                .await
                .ok()
        } else {
            None
        };

        let provider_kind = match settings.llm.active {
            crate::core::settings::LlmActiveProvider::Embedded => {
                crate::services::llm::ProviderKind::Embedded
            }
            crate::core::settings::LlmActiveProvider::Server
            | crate::core::settings::LlmActiveProvider::Cloud => {
                crate::services::llm::ProviderKind::OpenAiCompat
            }
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

        if cancel.is_cancelled() {
            log::info!(
                "[ModularPTT] Turn {} cancelled before LLM dispatch",
                turn_id
            );
            return;
        }

        if let Some(filler) = transition_speech {
            if tts_tx.is_some() {
                // Already dispatched if tts_tx was passed
            } else if let Some(ref tx) = tts_tx {
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
fn on_llm_token<R: tauri::Runtime>(
    turn_id: u32,
    token: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::LlmToken(LlmTokenPayload {
            turn_id,
            token: token.clone(),
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit llm_token: {}", e);
    }

    let clauses = ACCUMULATOR.lock().push_token(&token);
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
    if let Some(remainder) = ACCUMULATOR.lock().flush_chunker() {
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

    let full_text = ACCUMULATOR.lock().take_assistant_response();
    if !full_text.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(full_text.clone());

        let conv_id = state.conversation_id.load(Ordering::Relaxed);
        let user_text = ACCUMULATOR.lock().user_transcript();
        let stt_ms = state.telemetry.latest_stt_ms.load(Ordering::Relaxed);
        let ttft_ms = state.telemetry.latest_ttft_ms.load(Ordering::Relaxed);
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(
                crate::persistence::events::PersistenceEvent::TurnCompleted {
                    conversation_id: conv_id,
                    turn_id,
                    user_text,
                    assistant_text: full_text,
                    stt_latency_ms: stt_ms,
                    ttft_ms,
                },
            ) {
                log::warn!(
                    "[ModularPTT] Failed to send TurnCompleted to persist: {}",
                    e
                );
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
    state
        .telemetry
        .latest_tts_rtf
        .store(rtf.to_bits(), Ordering::Relaxed);
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
fn on_error<R: tauri::Runtime>(
    turn_id: u32,
    message: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    log::error!("[ModularPTT] Error on turn {}: {}", turn_id, message);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Error, &ctx, app, state);

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_MAIN,
        IpcEvent::VoiceError(VoiceErrorPayload {
            message,
            source: "ModularPTT".to_string(),
            owner: Some(InteractionOwner::Assistant),
        }),
    ) {
        log::warn!("[ModularPTT] Failed to emit voice_error: {}", e);
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
        VoxEvent::Interrupted { .. } => on_interrupt(app, state),
        VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
