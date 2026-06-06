//! Engine trait contracts for all four pipeline domains.
//!
//! These traits define pure synchronous data-in/data-out interfaces.
//! They know NOTHING about channels, threads, or Tauri.
//! This enables headless unit testing and future model swapping.

/// Voice Activity Detection engine contract.
pub trait VadEngine {
    fn predict(&mut self, chunk: &[f32]) -> bool;
}

/// Speech-to-Text engine contract.
pub trait SttEngine: Send + Sync {
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String>;
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String>;
    fn reset_state(&self) -> anyhow::Result<()>;
}

/// Large Language Model engine contract.
pub trait LlmEngine {
    fn generate(
        &self,
        user_text: &str,
        system_prompt: &str,
        turn_id: u32,
        cancel_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
        tx: &std::sync::mpsc::Sender<crate::core::events::VoxEvent>,
    ) -> anyhow::Result<()>;
}

/// Text-to-Speech engine contract.
pub trait TtsEngine {
    fn synthesize_chunk(
        &mut self,
        text: &str,
        voice_sid: i32,
        turn_id: u32,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        event_tx: std::sync::mpsc::Sender<crate::core::events::VoxEvent>,
    ) -> anyhow::Result<()>;
}
