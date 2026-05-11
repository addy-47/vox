/// Internal backend event system for the Vox pipeline.
///
/// These are Rust-internal signals flowing between pipeline stages via mpsc channels.
/// They are NOT Tauri IPC events — IPC is a bridge layer above this.
///
/// IMPORTANT — ID semantics (Phase 6.3):
///   - `turn_id`  — monotonic u32, increments every TranscriptFinal. Used for stale-event
///                  rejection and cancellation routing. NEVER persisted.
///   - Conversation session identity lives in `AppState::conversation_id` (AtomicU64).
///                  Persistence worker reads it at TurnCompleted time.
#[derive(Debug, Clone)]
pub enum VoxEvent {
    SpeechStart  { turn_id: u32, owner: crate::core::state::InteractionOwner },
    SpeechEnd    { turn_id: u32, owner: crate::core::state::InteractionOwner },

    TranscriptPartial { turn_id: u32, owner: crate::core::state::InteractionOwner, text: String },
    TranscriptFinal   { turn_id: u32, owner: crate::core::state::InteractionOwner, text: String },

    LlmToken    { turn_id: u32, token: String },
    LlmFinished { turn_id: u32 },

    TtsChunk    { turn_id: u32, samples: Vec<f32> },
    TtsFinished { turn_id: u32, rtf: f32 },

    PlaybackStarted  { turn_id: u32 },
    PlaybackFinished { turn_id: u32 },

    Cancelled { turn_id: u32 },
    Error     { turn_id: u32, message: String },

    /// Pre-warm the LLM worker (load model in background on engage).
    WarmUp,

    /// Gracefully shutdown the pipeline orchestrator.
    Shutdown,
}
