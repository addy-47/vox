//! TTS Provider trait — abstraction for text-to-speech engines.
//!
//! Mirrors the `LlmProvider` pattern in `services/llm/providers/`.
//! Each provider type implements this trait, and the TTS worker dispatches
//! to the active provider via `Box<dyn TtsProvider>`.

pub mod chatterbox;
pub mod chatterbox_remote;
pub mod edge_tts;
pub mod supertonic;

pub use edge_tts::EdgeTtsProvider;

use crate::core::events::VoxEvent;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// Provider kind identifier — used for serialization and frontend display.
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
    // Future providers will be added here:
    // Pocket,         // Kyutai Pocket TTS — zero-shot voice cloning (English)
    // OpenAiCompat,  // OpenAI-compatible remote TTS API
    // OmniVoice,     // k2-fsa OmniVoice — diffusion LM, 600+ languages, zero-shot cloning
    // ElevenLabs,    // ElevenLabs ConvAI API
}

/// Text-to-Speech provider contract.
///
/// # Thread Safety
/// - `&self` methods: providers use interior mutability (`Mutex`, `Atomic*`) when needed.
/// - `Send` but not `Sync`: the TTS worker owns the provider exclusively on its thread.
///
/// # Output Contract
/// All providers MUST output audio at **24 kHz f32 mono**.
/// The provider is responsible for any internal sample rate conversion.
pub trait TtsProvider: Send {
    /// Synthesize a chunk of text into audio samples.
    ///
    /// Audio samples are pushed back via `VoxEvent::TtsChunk { turn_id, samples }`
    /// through the `event_tx` channel. On completion, send `VoxEvent::TtsFinished { turn_id, rtf }`.
    ///
    /// `cancel` — checked during synthesis; return early (stop generating) when `true`.
    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        event_tx: Sender<VoxEvent>,
    ) -> anyhow::Result<()>;

    /// Hot-update the number of quality / diffusion steps. Default no-op.
    fn set_quality_steps(&self, _steps: u32) {}

    /// Hot-update the speed factor. Default no-op.
    fn set_speed(&self, _speed: f32) {}

    /// Returns the provider kind for identification.
    fn kind(&self) -> TtsProviderKind;

    /// Returns `true` if the provider is healthy / ready.
    ///
    /// For local engines, this typically checks that model files exist
    /// or that the engine was constructed successfully.
    fn health_check(&self) -> bool;
}
