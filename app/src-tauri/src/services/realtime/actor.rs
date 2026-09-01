use anyhow::Result;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tokio::task::JoinHandle;

use crate::core::events::VoxEvent;
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::{
    audio_bridge::AudioBridge, RealtimeProviderEvent, RealtimeSession, RealtimeVoiceProvider,
    BRIDGE_CHANNEL_CAPACITY, SESSION_CACHE_FILENAME, SESSION_CACHE_TTL_MS,
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
    pub fn start(
        &mut self,
        interaction_mode: crate::core::settings::InteractionMode,
        playback_engine: Arc<PlaybackEngine>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<()> {
        log::info!("[RealtimeActor] Starting realtime voice actor...");

        let config = self.provider.audio_config();
        let (playback_tx, playback_rx) =
            tokio::sync::mpsc::channel::<Vec<i16>>(BRIDGE_CHANNEL_CAPACITY);

        playback_engine.spawn_pcm_stream_worker(playback_rx, config, &self.tokio_handle);

        let (session, mut provider_event_rx) = self.provider.connect(interaction_mode)?;
        let session_arc: Arc<dyn RealtimeSession> = session.into();
        self.session = Some(session_arc.clone());

        self.audio_bridge
            .start(session_arc, config, &self.tokio_handle);

        let loop_playback_tx = playback_tx.clone();
        let loop_event_tx = event_tx.clone();

        let event_loop_task = self.tokio_handle.spawn(async move {
            while let Some(event) = provider_event_rx.recv().await {
                match event {
                    RealtimeProviderEvent::AudioChunk(pcm) => {
                        if let Err(e) = loop_playback_tx.send(pcm).await {
                            log::warn!("[RealtimeActor] Playback worker channel closed: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::TranscriptPartial { turn_id, text } => {
                        if let Err(e) =
                            loop_event_tx.send(VoxEvent::TranscriptPartial { turn_id, text })
                        {
                            log::warn!(
                                "[RealtimeActor] Failed to forward TranscriptPartial: {:?}",
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
                        if let Err(e) = loop_event_tx.send(VoxEvent::LlmToken { turn_id, token }) {
                            log::warn!("[RealtimeActor] Failed to forward LlmToken: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::LlmFinished { turn_id } => {
                        if let Err(e) = loop_event_tx.send(VoxEvent::LlmFinished { turn_id }) {
                            log::warn!("[RealtimeActor] Failed to forward LlmFinished: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::Interrupted { turn_id } => {
                        if let Err(e) = loop_event_tx.send(VoxEvent::Interrupted { turn_id }) {
                            log::warn!("[RealtimeActor] Failed to forward Interrupted: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::Error { turn_id, message } => {
                        if let Err(e) = loop_event_tx.send(VoxEvent::Error { turn_id, message }) {
                            log::warn!("[RealtimeActor] Failed to forward Error: {:?}", e);
                        }
                    }
                    RealtimeProviderEvent::SessionResumptionHandle { handle, model } => {
                        write_session_cache_non_blocking(&handle, &model).await;
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

    /// Submits microphone audio PCM samples to the capture bridge for continuous streaming.
    pub fn push_audio(&self, pcm: &[i16]) {
        self.audio_bridge.send_pcm(pcm);
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
}

/// Asynchronously saves the session resumption handle to disk without blocking the Tokio runtime.
async fn write_session_cache_non_blocking(handle: &str, model: &str) {
    let cache_path = crate::utils::paths::cache_dir().join(SESSION_CACHE_FILENAME);
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
