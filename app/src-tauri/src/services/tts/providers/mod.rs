pub mod chatterbox;
pub mod chatterbox_remote;
pub mod edge_tts;
pub mod kokoro;
pub mod supertonic;
pub mod zipvoice;

use std::sync::{
    atomic::{AtomicBool, AtomicU32},
    mpsc::Sender,
    Arc,
};

pub use edge_tts::EdgeTtsProvider;
pub use kokoro::KokoroEngine;
pub use zipvoice::{ZipvoiceEngine, ZipvoiceReference};

use crate::{
    core::{
        events::{AudioIntent, VoxEvent},
        settings::{ParamRange, ProviderCaps},
    },
    services::audio::PlaybackEngine,
};

/// Declares the bounds a shared speed control must respect.
const fn speed_range(min: f32, max: f32) -> ParamRange {
    ParamRange {
        min,
        max,
        step: 0.05,
    }
}

/// Execution handles and context passed to a `TtsProvider` for synthesizing a text chunk.
pub struct SynthesisContext<'a> {
    pub turn_id: u32,
    pub intent: AudioIntent,
    pub cancel: Arc<AtomicBool>,
    pub playback: &'a Arc<PlaybackEngine>,
    pub event_tx: Sender<VoxEvent>,
    pub telemetry_rtf: Option<&'a Arc<AtomicU32>>,
}

/// Abstract contract for text-to-speech synthesis providers.
pub trait TtsProvider: Send {
    /// Declares this provider's capabilities. Static so the frontend can query
    /// them without instantiating an engine.
    fn caps() -> ProviderCaps
    where
        Self: Sized;

    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> anyhow::Result<()>;

    fn set_speed(&self, _speed: f32) {}
    fn set_voice(&self, _voice: i32) {}
    fn health_check(&self) -> bool;
}
