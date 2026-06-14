/// Persistence events emitted by the pipeline at turn/session lifecycle boundaries.
///
/// This enum is deliberately separate from VoxEvent to avoid coupling the
/// realtime event bus to storage concerns. Only finalized, stable state is
/// ever pushed here — never raw runtime payloads like Vec<f32> or streaming tokens.
///
/// The persistence worker ignores all events where conversation_id == 0.
/// This is the architectural enforcement of the tray-is-ephemeral rule.
#[derive(Debug, Clone)]
pub enum PersistenceEvent {
    /// A new conversation session has started (user pressed Engage on Main UI).
    SessionStarted {
        id: u64, // epoch milliseconds — used as primary key
        timestamp_ms: u64,
    },

    /// The active conversation session has ended (user pressed Disengage).
    SessionEnded { id: u64, timestamp_ms: u64 },

    /// A single interaction turn has completed successfully.
    ///
    /// Emitted ONLY after PlaybackFinished (or polled-drain detection confirms
    /// all audio has been delivered). Never emitted on raw LLM tokens.
    TurnCompleted {
        conversation_id: u64,
        turn_id: u32,
        user_text: String,
        assistant_text: String,
        stt_latency_ms: u32,
        ttft_ms: u32,
    },

    /// A turn was interrupted before completion (barge-in or explicit cancel).
    /// The persistence layer records the partial state — it does NOT discard the turn.
    TurnCancelled { conversation_id: u64, turn_id: u32 },

    /// Signals the persistence worker to flush and exit cleanly.
    Shutdown,
}
