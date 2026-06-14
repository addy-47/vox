use crate::core::events::VoxEvent;
use crate::services::audio::PlaybackEngine;
use crate::services::realtime::{
    audio_bridge::AudioBridge, playback_bridge::PlaybackBridge, RealtimeSession,
    RealtimeVoiceProvider,
};
use anyhow::{Context, Result};
use std::sync::mpsc::Sender;
use std::sync::Arc;

pub struct RealtimeEngine {
    provider: Box<dyn RealtimeVoiceProvider>,
    session: Option<Arc<dyn RealtimeSession>>,
    audio_bridge: AudioBridge,
    playback_bridge: PlaybackBridge,
    tokio_handle: tokio::runtime::Handle,
}

impl RealtimeEngine {
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

    pub fn start(
        &mut self,
        interaction_mode: crate::core::settings::InteractionMode,
        playback_engine: Arc<PlaybackEngine>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<()> {
        log::info!("[RealtimeEngine] Starting realtime voice engine...");

        let config = self.provider.audio_config();
        self.playback_bridge
            .start(playback_engine, config, &self.tokio_handle);

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

    pub fn stop(&mut self) {
        log::info!("[RealtimeEngine] Stopping realtime voice engine...");

        self.audio_bridge.stop();
        self.playback_bridge.stop();

        if let Some(session) = self.session.take() {
            let _ = session.disconnect();
        }

        log::info!("[RealtimeEngine] Realtime voice engine stopped.");
    }

    pub fn push_audio(&self, pcm: &[i16]) {
        self.audio_bridge.send_pcm(pcm);
    }

    pub fn get_audio_sender(&self) -> Option<tokio::sync::mpsc::Sender<Vec<i16>>> {
        self.audio_bridge.get_sender()
    }

    pub fn barge_in(&self, playback_engine: &PlaybackEngine) {
        log::info!("[RealtimeEngine] Interruption (barge-in) triggered.");
        playback_engine.cancel();

        if let Some(ref session) = self.session {
            let _ = session.cancel();
        }
    }

    pub fn activity_start(&self) -> Result<()> {
        log::info!("[RealtimeEngine] PTT activity_start called.");
        if let Some(ref session) = self.session {
            session.activity_start()
        } else {
            Ok(())
        }
    }

    pub fn activity_end(&self) -> Result<()> {
        log::info!("[RealtimeEngine] PTT activity_end called.");
        if let Some(ref session) = self.session {
            session.activity_end()
        } else {
            Ok(())
        }
    }

    pub fn is_connected(&self) -> bool {
        if let Some(ref session) = self.session {
            session.is_connected()
        } else {
            false
        }
    }

    pub fn last_activity_time(&self) -> u64 {
        if let Some(ref session) = self.session {
            session.last_activity_time()
        } else {
            0
        }
    }
}
