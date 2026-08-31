use crate::core::events::VoxEvent;
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::{
    audio_bridge::AudioBridge, playback_bridge::PlaybackBridge, RealtimeSession,
    RealtimeVoiceProvider,
};
use anyhow::{Context, Result};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// High-level orchestration engine coordinating realtime duplex voice sessions and audio bridges.
pub struct RealtimeEngine {
    provider: Box<dyn RealtimeVoiceProvider>,
    session: Option<Arc<dyn RealtimeSession>>,
    audio_bridge: AudioBridge,
    playback_bridge: PlaybackBridge,
    tokio_handle: tokio::runtime::Handle,
}

impl RealtimeEngine {
    /// Creates a new RealtimeEngine wrapping the specified provider backend and runtime handle.
    pub fn new(
        provider: Box<dyn RealtimeVoiceProvider>,
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            provider,
            session: None,
            audio_bridge: AudioBridge::new(),
            playback_bridge: PlaybackBridge::new(),
            tokio_handle,
        }
    }

    /// Initializes playback and capture bridges and connects to the realtime voice provider.
    pub fn start(
        &mut self,
        interaction_mode: crate::core::settings::InteractionMode,
        playback_engine: Arc<PlaybackEngine>,
        event_tx: Sender<VoxEvent>,
        turn_id: Arc<std::sync::atomic::AtomicU32>,
    ) -> Result<()> {
        log::info!("[RealtimeEngine] Starting realtime voice engine...");

        let config = self.provider.audio_config();
        self.playback_bridge.start(
            playback_engine,
            config,
            &self.tokio_handle,
            event_tx.clone(),
            turn_id,
        );

        let playback_tx = self
            .playback_bridge
            .get_sender()
            .context("[RealtimeEngine] PlaybackBridge not started")?;

        let session = self
            .provider
            .connect(interaction_mode, playback_tx, event_tx)?;
        let session_arc: Arc<dyn RealtimeSession> = session.into();
        self.session = Some(session_arc.clone());

        self.audio_bridge
            .start(session_arc, config, &self.tokio_handle);

        log::info!("[RealtimeEngine] Realtime voice engine started successfully.");
        Ok(())
    }

    /// Terminates active session and shuts down audio bridges.
    pub fn stop(&mut self) {
        log::info!("[RealtimeEngine] Stopping realtime voice engine...");

        self.audio_bridge.stop();
        self.playback_bridge.stop();

        if let Some(session) = self.session.take() {
            if let Err(e) = session.disconnect() {
                log::warn!("[RealtimeEngine] Disconnect error during stop: {:?}", e);
            }
        }

        log::info!("[RealtimeEngine] Realtime voice engine stopped.");
    }

    /// Submits microphone audio PCM samples to the capture bridge.
    pub fn push_audio(&self, pcm: &[i16]) {
        self.audio_bridge.send_pcm(pcm);
    }

    /// Returns the active audio input channel sender.
    pub fn get_audio_sender(&self) -> Option<tokio::sync::mpsc::Sender<Vec<i16>>> {
        self.audio_bridge.get_sender()
    }

    /// Sends cancellation signal to the active realtime provider session.
    pub fn cancel(&self) -> Result<()> {
        if let Some(ref session) = self.session {
            session.cancel()
        } else {
            Ok(())
        }
    }

    /// Signals start of user speech activity in Push-to-Talk mode.
    pub fn activity_start(&self) -> Result<()> {
        log::info!("[RealtimeEngine] PTT activity_start called.");
        if let Some(ref session) = self.session {
            session.activity_start()
        } else {
            Ok(())
        }
    }

    /// Signals end of user speech activity in Push-to-Talk mode.
    pub fn activity_end(&self) -> Result<()> {
        log::info!("[RealtimeEngine] PTT activity_end called.");
        if let Some(ref session) = self.session {
            session.activity_end()
        } else {
            Ok(())
        }
    }
}
