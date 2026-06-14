pub mod audio_bridge;
pub mod engine;
pub mod playback_bridge;
pub mod providers;
pub mod resampler;

use crate::core::events::VoxEvent;
pub use crate::core::settings::RealtimeProviderKind;
use anyhow::Result;
use std::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy)]
pub struct RealtimeAudioConfig {
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub requires_input_resampling: bool,
    pub requires_output_resampling: bool,
}

pub trait RealtimeVoiceProvider: Send + Sync {
    fn kind(&self) -> RealtimeProviderKind;
    fn audio_config(&self) -> RealtimeAudioConfig;
    fn connect(
        &self,
        interaction_mode: crate::core::settings::InteractionMode,
        playback_tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<Box<dyn RealtimeSession>>;
    fn health_check(&self) -> bool;
}

pub trait RealtimeSession: Send + Sync {
    fn send_audio(&self, pcm: &[i16]) -> Result<()>;
    fn cancel(&self) -> Result<()>;
    fn disconnect(&self) -> Result<()>;
    fn activity_start(&self) -> Result<()>;
    fn activity_end(&self) -> Result<()>;
    fn is_connected(&self) -> bool {
        true
    }
    fn last_activity_time(&self) -> u64 {
        0
    }
}
