use std::{
    fs::{read_to_string, remove_file},
    sync::{mpsc::Sender, Arc},
};

use anyhow::Result;
use tauri::Manager;
use tokio::task::JoinHandle;

use crate::{
    core::{
        error::PipelineError,
        events::{emit_ipc_to, IpcEvent, LlmTokenPayload, TranscriptPayload, VoxEvent},
        settings::{InteractionMode, PipelineMode, RealtimeProviderKind},
        state::{AppState, AppWindow, InteractionOwner},
    },
    pipeline::target_window,
    services::{
        audio::PlaybackEngine,
        harness::stages::tools::{ToolExecutionContext, ToolExecutor, ToolRegistry},
        llm::CanonicalToolCall,
        realtime::{
            audio_bridge::AudioBridge,
            providers::{DeepgramVoiceAgentProvider, GeminiLiveProvider},
            RealtimeProviderEvent, RealtimeSession, RealtimeVoiceProvider, BRIDGE_CHANNEL_CAPACITY,
            SESSION_CACHE_FILENAME, SESSION_CACHE_TTL_MS,
        },
    },
    utils::paths::cache_dir,
};

/// High-level orchestration actor coordinating realtime duplex voice sessions, audio bridges, and event translation.
pub struct RealtimeActor {
    provider: Box<dyn RealtimeVoiceProvider>,
    session: Option<Arc<dyn RealtimeSession>>,
    audio_bridge: AudioBridge,
    tokio_handle: tokio::runtime::Handle,
    event_loop_task: Option<JoinHandle<()>>,
}

impl RealtimeActor {
    /// Creates a new RealtimeActor wrapping the specified provider backend and runtime handle.
    pub fn new(
        provider: Box<dyn RealtimeVoiceProvider>,
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            provider,
            session: None,
            audio_bridge: AudioBridge::new(),
            tokio_handle,
            event_loop_task: None,
        }
    }

    /// Initializes playback, connects to the realtime provider, and spawns the event routing loop.
    pub fn start<R: tauri::Runtime + 'static>(
        &mut self,
        interaction_mode: InteractionMode,
        playback_engine: Arc<PlaybackEngine>,
        event_tx: Sender<VoxEvent>,
        app: tauri::AppHandle<R>,
    ) -> Result<()> {
        log::info!("[RealtimeActor] Starting realtime voice actor...");

        let config = self.provider.audio_config();
        let (playback_tx, playback_rx) =
            tokio::sync::mpsc::channel::<Vec<i16>>(BRIDGE_CHANNEL_CAPACITY);

        playback_engine.spawn_pcm_stream_worker(playback_rx, config, &self.tokio_handle);

        let (session, mut provider_event_rx) = self
            .provider
            .connect(interaction_mode, &self.tokio_handle)?;
        let session_arc: Arc<dyn RealtimeSession> = session.into();
        self.session = Some(session_arc.clone());

        self.audio_bridge
            .start(session_arc.clone(), config, &self.tokio_handle);

        let loop_playback_tx = playback_tx.clone();
        let loop_event_tx = event_tx.clone();
        let tool_registry = Arc::new(ToolRegistry::with_default_tools());
        let state_arc: Option<Arc<AppState>> =
            app.try_state::<Arc<AppState>>().map(|s| s.inner().clone());
        let sessions_callback = make_sessions_changed_callback(&app);

        let event_loop_task = self.tokio_handle.spawn(async move {
            while let Some(event) = provider_event_rx.recv().await {
                match event {
                    RealtimeProviderEvent::AudioChunk(pcm) => {
                        if let Err(e) = loop_playback_tx.send(pcm).await {
                            log::warn!("[RealtimeActor] Playback worker channel closed: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::SpeechStart => {
                        if let Err(e) = loop_event_tx.send(VoxEvent::SpeechStart) {
                            log::warn!("[RealtimeActor] Failed to forward SpeechStart: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::SpeechEnd => {
                        if let Err(e) = loop_event_tx.send(VoxEvent::SpeechEnd) {
                            log::warn!("[RealtimeActor] Failed to forward SpeechEnd: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::TranscriptPartial { turn_id, text } => {
                        let target = target_window(InteractionOwner::Assistant);
                        if let Err(e) = emit_ipc_to(
                            &app,
                            target,
                            IpcEvent::TranscriptPartial(TranscriptPayload {
                                turn_id,
                                text,
                                owner: Some(InteractionOwner::Assistant),
                            }),
                        ) {
                            log::trace!(
                                "[RealtimeActor] Failed to emit TranscriptPartial IPC: {}",
                                e
                            );
                        }
                    }
                    RealtimeProviderEvent::TranscriptFinal { turn_id, text } => {
                        if let Err(e) =
                            loop_event_tx.send(VoxEvent::TranscriptFinal { turn_id, text })
                        {
                            log::warn!(
                                "[RealtimeActor] Failed to forward TranscriptFinal: {:?}",
                                e
                            );
                        }
                    }
                    RealtimeProviderEvent::LlmToken { turn_id, token } => {
                        let target = target_window(InteractionOwner::Assistant);
                        if let Err(e) = emit_ipc_to(
                            &app,
                            target,
                            IpcEvent::LlmToken(LlmTokenPayload { turn_id, token }),
                        ) {
                            log::trace!("[RealtimeActor] Failed to emit LlmToken IPC: {}", e);
                        }
                    }
                    RealtimeProviderEvent::LlmFinished { turn_id } => {
                        if let Err(e) = loop_event_tx.send(VoxEvent::LlmFinished { turn_id }) {
                            log::warn!("[RealtimeActor] Failed to forward LlmFinished: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::Error {
                        turn_id,
                        message,
                        impact,
                    } => {
                        if let Err(e) = loop_event_tx.send(VoxEvent::Error(PipelineError {
                            turn_id,
                            message,
                            source: "RealtimeActor".to_string(),
                            impact,
                        })) {
                            log::warn!("[RealtimeActor] Failed to forward Error: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::SessionResumptionHandle { handle, model } => {
                        write_session_cache_non_blocking(&handle, &model).await;
                    }
                    RealtimeProviderEvent::ToolCall { id, name, args } => {
                        let session_clone = session_arc.clone();
                        let registry_clone = tool_registry.clone();
                        let state_clone = state_arc.clone();
                        let cb_clone = sessions_callback.clone();
                        tokio::spawn(async move {
                            log::info!(
                                "[RealtimeActor] Processing inbound ToolCall: name={} id={}",
                                name,
                                id
                            );
                            let Some(app_state) = state_clone else {
                                log::error!(
                                    "[RealtimeActor] AppState unavailable for tool execution"
                                );
                                return;
                            };
                            let session_id = app_state
                                .conversation_id
                                .load(std::sync::atomic::Ordering::Relaxed)
                                as i64;
                            let turn_id = app_state
                                .pipeline
                                .turn_id
                                .load(std::sync::atomic::Ordering::Relaxed);
                            let tool_ctx = ToolExecutionContext {
                                app_state: app_state.clone(),
                                session_id,
                                turn_id,
                                cancel: tokio_util::sync::CancellationToken::new(),
                                on_sessions_changed: cb_clone,
                            };
                            let call = CanonicalToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: args,
                            };
                            let outcome = ToolExecutor::execute_tool(
                                &registry_clone,
                                PipelineMode::Realtime,
                                call,
                                tool_ctx,
                            )
                            .await;
                            let result_val: serde_json::Value =
                                match serde_json::from_str(&outcome.result.content) {
                                    Ok(v) => v,
                                    Err(_) => {
                                        serde_json::json!({ "output": outcome.result.content })
                                    }
                                };
                            if let Err(e) =
                                session_clone.send_tool_response(&id, &name, &result_val)
                            {
                                log::warn!(
                                    "[RealtimeActor] Failed to dispatch tool response: {:?}",
                                    e
                                );
                            } else {
                                log::info!(
                                    "[RealtimeActor] Dispatched tool response for {} (id: {})",
                                    name,
                                    id
                                );
                            }
                        });
                    }
                }
            }
            log::info!("[RealtimeActor] Provider event translation loop terminated.");
        });

        self.event_loop_task = Some(event_loop_task);

        log::info!("[RealtimeActor] Realtime voice actor started successfully.");
        Ok(())
    }

    /// Terminates active session, aborts event routing, and shuts down audio bridge.
    pub fn stop(&mut self) {
        log::info!("[RealtimeActor] Stopping realtime voice actor...");

        if let Some(task) = self.event_loop_task.take() {
            task.abort();
        }

        self.audio_bridge.stop();

        if let Some(session) = self.session.take() {
            if let Err(e) = session.disconnect() {
                log::warn!("[RealtimeActor] Disconnect error during stop: {:?}", e);
            }
        }

        log::info!("[RealtimeActor] Realtime voice actor stopped.");
    }

    /// Returns the active audio input channel sender.
    pub fn get_audio_sender(&self) -> Option<tokio::sync::mpsc::Sender<Vec<i16>>> {
        self.audio_bridge.get_sender()
    }

    /// Commits an atomic speech turn buffer to the active realtime provider session.
    pub fn signal_speech_committed(&self, pcm: &[i16]) -> Result<()> {
        log::info!(
            "[RealtimeActor] Committing speech turn ({} samples) to provider session.",
            pcm.len()
        );
        if let Some(ref session) = self.session {
            session.commit_speech_turn(pcm)
        } else {
            Ok(())
        }
    }

    /// Sends cancellation signal to the active realtime provider session.
    pub fn signal_interrupt(&self) -> Result<()> {
        if let Some(ref session) = self.session {
            session.cancel()
        } else {
            Ok(())
        }
    }

    /// Dispatches a typed user text message to the active realtime provider session.
    pub fn send_text(&self, text: &str) -> Result<()> {
        log::info!(
            "[RealtimeActor] Sending text input to provider session (chars: {})",
            text.len()
        );
        if let Some(ref session) = self.session {
            session.send_text(text)
        } else {
            anyhow::bail!("No active realtime session");
        }
    }
}

fn make_sessions_changed_callback<R: tauri::Runtime + 'static>(
    app: &tauri::AppHandle<R>,
) -> Option<Arc<dyn Fn() + Send + Sync>> {
    let app_clone = app.clone();
    Some(Arc::new(move || {
        if let Err(e) = emit_ipc_to(&app_clone, AppWindow::Main, IpcEvent::SessionsChanged) {
            log::warn!(
                "[RealtimeActor::Tools] Failed to emit SessionsChanged IPC: {}",
                e
            );
        } else {
            log::info!("[RealtimeActor::Tools] Emitted SessionsChanged IPC");
        }
    }))
}

/// Asynchronously saves the session resumption handle to disk without blocking the Tokio runtime.
async fn write_session_cache_non_blocking(handle: &str, model: &str) {
    let cache_path = cache_dir().join(SESSION_CACHE_FILENAME);
    let now_ms = chrono::Utc::now().timestamp_millis() as u64;
    let expires_at = now_ms + SESSION_CACHE_TTL_MS;
    let payload = serde_json::json!({
        "provider": "gemini_live",
        "handle": handle,
        "expires_at": expires_at,
        "model": model,
    });

    let tmp_path = cache_path.with_extension("tmp");
    if let Ok(payload_str) = serde_json::to_string_pretty(&payload) {
        if let Err(e) = tokio::fs::write(&tmp_path, payload_str).await {
            log::error!(
                "[RealtimeActor] Failed to write temporary session cache: {:?}",
                e
            );
        } else if let Err(e) = tokio::fs::rename(&tmp_path, &cache_path).await {
            log::error!(
                "[RealtimeActor] Failed to rename session cache file: {:?}",
                e
            );
        } else {
            log::debug!(
                "[RealtimeActor] Saved resumption cache ({} bytes)",
                handle.len()
            );
        }
    }
}

/// Instantiates the configured cloud real-time voice provider.
pub fn create_realtime_provider(
    state: &AppState,
) -> Result<Box<dyn RealtimeVoiceProvider>, String> {
    let mut settings = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let assembled_prompt = state.resolve_base_prompt();

    // Check cached session resumption token with 2-hour TTL
    let cache_path = cache_dir().join(SESSION_CACHE_FILENAME);
    let mut cached_handle = None;
    if cache_path.exists() {
        if let Ok(data) = read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<serde_json::Value>(&data) {
                let expires_at = cached["expires_at"].as_u64().unwrap_or(0);
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                if now_ms < expires_at {
                    if let Some(handle) = cached["handle"].as_str() {
                        log::info!(
                            "[RealtimeSession] Found valid unexpired session resumption token."
                        );
                        cached_handle = Some(handle.to_string());
                    }
                } else {
                    log::info!("[RealtimeSession] Cached session resumption token expired (>2 hours). Purging...");
                    purge_session_cache();
                }
            }
        }
    }

    if let Some(handle) = cached_handle {
        settings.realtime.gemini_live.resume_handle = Some(handle);
    }

    let tools = ToolRegistry::with_default_tools().canonical_definitions(PipelineMode::Realtime);

    match settings.realtime.active {
        RealtimeProviderKind::GeminiLive => Ok(Box::new(GeminiLiveProvider::new(
            settings.realtime.gemini_live.clone(),
            assembled_prompt,
            tools,
            state.pipeline.state_rx.clone(),
            state.pipeline.turn_id.clone(),
        ))),
        RealtimeProviderKind::DeepgramVoiceAgent => Ok(Box::new(DeepgramVoiceAgentProvider::new(
            settings.realtime.deepgram_voice_agent.clone(),
            assembled_prompt,
            state.pipeline.state_rx.clone(),
            state.pipeline.turn_id.clone(),
        ))),
        RealtimeProviderKind::OpenAiRealtime => {
            Err("OpenAI Realtime provider is not implemented".to_string())
        }
        RealtimeProviderKind::ElevenLabsConvai => {
            Err("ElevenLabs Conversational AI provider is not implemented".to_string())
        }
    }
}

/// Explicitly purges the disk cache file containing the session resumption token.
pub fn purge_session_cache() {
    let cache_path = cache_dir().join(SESSION_CACHE_FILENAME);
    if cache_path.exists() {
        if let Err(e) = remove_file(&cache_path) {
            log::warn!(
                "[RealtimeSession] Failed to delete session cache file: {}",
                e
            );
        } else {
            log::info!("[RealtimeSession] Purged session cache file.");
        }
    }
}
