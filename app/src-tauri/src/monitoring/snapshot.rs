use serde::Serialize;

/// A normalized, read-only snapshot of the Vox engine runtime state.
/// This is the primary source of truth for the frontend monitoring UI.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSnapshot {
    /// Current pipeline state (Idle, Listening, Thinking, AssistantSpeaking, etc.)
    pub pipeline_state: String,
    /// Ephemeral turn ID for the current interaction.
    pub current_turn_id: u32,
    /// Persistent conversation session ID. 0 if inactive (Tray mode).
    pub conversation_id: u64,

    /// System activity flags.
    pub playback_active: bool,
    pub llm_generating: bool,
    pub tts_generating: bool,

    /// System resource utilization.
    pub cpu_usage: f32,
    pub ram_mb: u32,

    /// Real-time VAD characteristics.
    pub vad_energy: f32,
    pub vad_probability: f32,

    /// Latency metrics for the last completed turn.
    pub stt_latency_ms: Option<u32>,
    pub ttft_ms: Option<u32>,
    pub total_voice_latency_ms: Option<u32>,

    /// Persistence health.
    pub persistence_queue_depth: usize,
    pub dropped_persistence_events: u64,

    /// Playback health.
    pub playback_buffer_samples: usize,
    pub playback_underruns: u64,

    /// Current interaction owner (Tray, MainWindow, Ptt).
    pub active_owner: String,

    /// Unix timestamp of the snapshot in milliseconds.
    pub timestamp_ms: u64,
}
