use serde::Serialize;

/// A normalized, read-only snapshot of the Vox engine runtime state.
/// This is the primary source of truth for the frontend monitoring UI.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSnapshot {
    /// Current pipeline state (Idle, Ready, Listening, Thinking, Speaking, Paused, Error)
    pub pipeline_state: String,
    /// Ephemeral turn ID for the current interaction.
    pub current_turn_id: u32,
    /// Persistent conversation session ID. 0 if inactive (Tray mode).
    pub conversation_id: u64,

    /// System activity flags.
    pub playback_active: bool,

    /// System resource utilization.
    pub system_cpu_usage: f32,
    pub system_ram_mb: u32,
    pub vox_cpu_usage: f32,
    pub vox_ram_mb: u32,
    pub total_ram_mb: u32,
    pub cpu_cores: u32,

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

    /// Current interaction owner (Dictation, Assistant).
    pub active_owner: String,

    /// Extended Monitoring Metrics
    pub active_threads: u32,
    pub tts_rtf: Option<f32>,
    pub playback_start_ms: Option<u32>,
    pub persistence_writes_per_sec: f32,
    pub is_db_healthy: bool,

    // Tier Status (Model Residency)
    pub is_llm_loaded: bool,
    pub llm_provider_kind: String,
    pub is_tts_loaded: bool,
    pub is_stt_loaded: bool,
    pub is_vad_loaded: bool,
    pub is_embedder_loaded: bool,
    pub is_query_classifier_loaded: bool,
    pub is_intra_edge_classifier_loaded: bool,
    pub is_inter_edge_classifier_loaded: bool,
    pub is_translit_loaded: bool,

    /// CPU frequency governor (Linux only, e.g. "powersave", "performance"). Empty string if unavailable.
    pub cpu_governor: String,
    /// Whether the CPU governor is optimal ("performance"). False if unknown/non-Linux.
    pub cpu_governor_optimal: bool,

    /// Optional per-WebView RAM breakdown in MB (Measured via sysinfo descendant enumeration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_webview_ram_mb: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tray_webview_ram_mb: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wizard_webview_ram_mb: Option<u32>,

    /// Unix timestamp of the snapshot in milliseconds.
    pub timestamp_ms: u64,
}
