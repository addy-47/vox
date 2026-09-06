pub mod chatterbox;
pub mod chatterbox_remote;
pub mod edge_tts;
pub mod kokoro;
pub mod supertonic;

use std::sync::{
    atomic::{AtomicBool, AtomicU32},
    mpsc::Sender,
    Arc,
};

pub use edge_tts::EdgeTtsProvider;
pub use kokoro::KokoroEngine;

use crate::{core::events::VoxEvent, services::audio::PlaybackEngine};

/// Provider kind identifier for speech synthesis backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsProviderKind {
    Supertonic,
    Kokoro,
    Chatterbox,
    ChatterboxRemote,
    EdgeTts,
}

/// Abstract contract for text-to-speech synthesis providers.
pub trait TtsProvider: Send {
    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        playback: &Arc<PlaybackEngine>,
        event_tx: Sender<VoxEvent>,
        telemetry_rtf: Option<&Arc<AtomicU32>>,
    ) -> anyhow::Result<()>;

    fn set_quality_steps(&self, _steps: u32) {}
    fn set_speed(&self, _speed: f32) {}
    fn set_voice(&self, _voice: i32) {}
    fn kind(&self) -> TtsProviderKind;
    fn health_check(&self) -> bool;
}
