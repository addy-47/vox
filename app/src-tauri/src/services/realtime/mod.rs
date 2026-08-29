pub mod audio_bridge;
pub mod engine;
pub mod playback_bridge;
pub mod providers;
pub mod resampler;

use std::time::Duration;
use crate::core::events::VoxEvent;
pub use crate::core::settings::RealtimeProviderKind;
use anyhow::Result;
use std::sync::mpsc::Sender;

pub const DEFAULT_INPUT_SAMPLE_RATE: u32 = 16000;
pub const DEFAULT_OUTPUT_SAMPLE_RATE: u32 = 24000;
pub const BRIDGE_CHANNEL_CAPACITY: usize = 100;
pub const LOG_INTERVAL_PACKETS: u64 = 100;
pub const SINC_CHUNK_SIZE_INPUT: usize = 320;
pub const SINC_CHUNK_SIZE_OUTPUT: usize = 512;
pub const SINC_WINDOW_LEN: usize = 256;
pub const SINC_OVERSAMPLING_FACTOR: usize = 128;
pub const SINC_CUTOFF_FREQUENCY: f32 = 0.95;
pub const PCM_INT16_MAX_FLOAT: f32 = 32767.0;
pub const PCM_INT16_DIVISOR_FLOAT: f32 = 32768.0;

pub const WS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub const WS_HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);
pub const WS_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(4);
pub const MAX_RECONNECT_ATTEMPTS: usize = 3;
pub const RECONNECT_BASE_DELAY_SECS: u64 = 1;
pub const RECONNECT_FACTOR_SECS: u64 = 2;
pub const PTT_INTERRUPT_GAP: Duration = Duration::from_millis(50);
pub const SESSION_CACHE_TTL_MS: u64 = 2 * 60 * 60 * 1000;

pub const DEEPGRAM_DEFAULT_WS_URL: &str = "wss://agent.deepgram.com/v1/agent/converse";
pub const DEEPGRAM_HEALTH_CHECK_ADDR: &str = "agent.deepgram.com:443";
pub const GEMINI_DEFAULT_WS_URL_BASE: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
pub const GEMINI_HEALTH_CHECK_ADDR: &str = "generativelanguage.googleapis.com:443";
pub const GEMINI_HEALTH_CHECK_FALLBACK_SOCKET_ADDR: std::net::SocketAddr =
    std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(142, 250, 190, 42),
        443,
    ));
pub const SESSION_CACHE_FILENAME: &str = "realtime_session.json";

/// Configuration defining input/output sampling rates and resampling requirements for realtime streaming.
#[derive(Debug, Clone, Copy)]
pub struct RealtimeAudioConfig {
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub requires_input_resampling: bool,
    pub requires_output_resampling: bool,
}

/// Provider factory interface for establishing full-duplex realtime voice WebSocket sessions.
pub trait RealtimeVoiceProvider: Send + Sync {
    /// Returns the provider kind identifier.
    fn kind(&self) -> RealtimeProviderKind;
    /// Returns the audio format and resampling configuration.
    fn audio_config(&self) -> RealtimeAudioConfig;
    /// Establishes the WebSocket connection and returns an active session instance.
    fn connect(
        &self,
        interaction_mode: crate::core::settings::InteractionMode,
        playback_tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<Box<dyn RealtimeSession>>;
    /// Performs a connectivity health check against the provider.
    fn health_check(&self) -> bool;
}

/// Active duplex streaming session for bidirectional voice transmission.
pub trait RealtimeSession: Send + Sync {
    /// Sends an audio PCM chunk to the remote server.
    fn send_audio(&self, pcm: &[i16]) -> Result<()>;
    /// Sends cancellation or interruption message to the remote server.
    fn cancel(&self) -> Result<()>;
    /// Gracefully closes the session.
    fn disconnect(&self) -> Result<()>;
    /// Notifies remote server of speech activity start in PTT mode.
    fn activity_start(&self) -> Result<()>;
    /// Notifies remote server of speech activity end in PTT mode.
    fn activity_end(&self) -> Result<()>;
    /// Returns true if the session WebSocket is actively connected.
    fn is_connected(&self) -> bool {
        true
    }
    /// Returns timestamp of the most recent network activity.
    fn last_activity_time(&self) -> u64 {
        0
    }
}
