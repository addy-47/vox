pub mod chatterbox;
pub mod chatterbox_remote;
pub mod edge_tts;
pub mod supertonic;

pub use edge_tts::EdgeTtsProvider;

use crate::core::events::VoxEvent;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// Provider kind identifier for speech synthesis backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsProviderKind {
    /// Local Supertonic 3 engine via sherpa-onnx (ONNX Runtime).
    Supertonic,
    /// Chatterbox Multilingual TTS via chatterbox-rs (GGML).
    Chatterbox,
    /// Chatterbox Remote TTS offloaded to a GPU server.
    ChatterboxRemote,
    /// Microsoft Edge TTS via WebSocket.
    EdgeTts,
}

/// Abstract contract for text-to-speech synthesis providers.
pub trait TtsProvider: Send {
    /// Synthesizes text chunk into 24kHz f32 audio and dispatches chunk events.
    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        event_tx: Sender<VoxEvent>,
    ) -> anyhow::Result<()>;

    /// Updates synthesis quality/diffusion step count.
    fn set_quality_steps(&self, _steps: u32) {}

    /// Updates speech synthesis playback speed multiplier.
    fn set_speed(&self, _speed: f32) {}

    /// Returns the provider kind enum identifier.
    fn kind(&self) -> TtsProviderKind;

    /// Returns true if the synthesis engine is initialized and ready.
    fn health_check(&self) -> bool;
}
