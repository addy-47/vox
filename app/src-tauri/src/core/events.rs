/// Internal backend event system for the Vox pipeline.
///
/// These are Rust-internal signals flowing between pipeline stages via mpsc channels.
/// They are NOT Tauri IPC events — IPC is a bridge layer above this.
#[derive(Debug, Clone)]
pub enum VoxEvent {
    SpeechStart  { session_id: u32, owner: crate::core::state::InteractionOwner },
    SpeechEnd    { session_id: u32, owner: crate::core::state::InteractionOwner },

    TranscriptPartial { session_id: u32, owner: crate::core::state::InteractionOwner, text: String },
    TranscriptFinal   { session_id: u32, owner: crate::core::state::InteractionOwner, text: String },

    LlmToken    { session_id: u32, token: String },
    LlmFinished { session_id: u32 },

    TtsChunk    { session_id: u32, samples: Vec<f32> },
    TtsFinished { session_id: u32 },

    PlaybackStarted  { session_id: u32 },
    PlaybackFinished { session_id: u32 },

    Cancelled { session_id: u32 },
    Error     { session_id: u32, message: String },
    
    /// Pre-warm the LLM worker (load model in background on engage).
    WarmUp,

    /// Gracefully shutdown the pipeline orchestrator.
    Shutdown,
}
