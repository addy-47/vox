pub mod audio_bridge;
pub mod engine;
pub mod playback_bridge;
pub mod providers;
pub mod resampler;

use crate::core::events::VoxEvent;
pub use crate::core::settings::RealtimeProviderKind;
use anyhow::Result;
use std::sync::mpsc::Sender;

/// Configuration defining input/output sampling rates and resampling requirements for realtime streaming.
#[derive(Debug, Clone, Copy)]
pub struct RealtimeAudioConfig {
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub requires_input_resampling: bool,
    pub requires_output_resampling: bool,
}

/// Provider factory interface for establishing full-duplex realtime voice WebSocket sessions.
pub trait RealtimeVoiceProvider: Send + Sync {
    /// Returns the provider kind identifier.
    fn kind(&self) -> RealtimeProviderKind;
    /// Returns the audio format and resampling configuration.
    fn audio_config(&self) -> RealtimeAudioConfig;
    /// Establishes the WebSocket connection and returns an active session instance.
    fn connect(
        &self,
        interaction_mode: crate::core::settings::InteractionMode,
        playback_tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<Box<dyn RealtimeSession>>;
    /// Performs a connectivity health check against the provider.
    fn health_check(&self) -> bool;
}

/// Active duplex streaming session for bidirectional voice transmission.
pub trait RealtimeSession: Send + Sync {
    /// Sends an audio PCM chunk to the remote server.
    fn send_audio(&self, pcm: &[i16]) -> Result<()>;
    /// Sends cancellation or interruption message to the remote server.
    fn cancel(&self) -> Result<()>;
    /// Gracefully closes the session.
    fn disconnect(&self) -> Result<()>;
    /// Notifies remote server of speech activity start in PTT mode.
    fn activity_start(&self) -> Result<()>;
    /// Notifies remote server of speech activity end in PTT mode.
    fn activity_end(&self) -> Result<()>;
    /// Returns true if the session WebSocket is actively connected.
    fn is_connected(&self) -> bool {
        true
    }
    /// Returns timestamp of the most recent network activity.
    fn last_activity_time(&self) -> u64 {
        0
    }
}
