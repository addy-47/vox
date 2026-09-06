pub mod actor;
pub mod audio_bridge;
pub mod providers;
pub mod session;
pub mod transport;

use std::time::Duration;

pub use actor::RealtimeActor;
use anyhow::Result;
pub use session::{create_realtime_provider, purge_session_cache};

use crate::core::settings::InteractionMode;
pub use crate::core::{
    events::{Actionability, PipelineError, PipelineImpact},
    settings::RealtimeProviderKind,
};

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
pub const GEMINI_HEALTH_CHECK_FALLBACK_SOCKET_ADDR: std::net::SocketAddr = std::net::SocketAddr::V4(
    std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(142, 250, 190, 42), 443),
);
pub const SESSION_CACHE_FILENAME: &str = "realtime_session.json";

/// Typed events emitted internally by realtime provider sessions to the RealtimeActor.
#[derive(Debug)]
pub enum RealtimeProviderEvent {
    AudioChunk(Vec<i16>),
    SpeechStart,
    SpeechEnd,
    TranscriptPartial {
        turn_id: u32,
        text: String,
    },
    TranscriptFinal {
        turn_id: u32,
        text: String,
    },
    LlmToken {
        turn_id: u32,
        token: String,
    },
    LlmFinished {
        turn_id: u32,
    },
    Error {
        turn_id: u32,
        message: String,
        impact: PipelineImpact,
        actionability: Actionability,
    },
    SessionResumptionHandle {
        handle: String,
        model: String,
    },
}

/// Typed commands dispatched outbound from session callers to the WebSocket connection writer.
#[derive(Debug)]
pub enum OutboundCommand {
    Audio(Vec<i16>),
    ActivityStart,
    ActivityEnd,
    Interrupt,
    KeepAlive,
}

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
    fn kind(&self) -> RealtimeProviderKind;
    fn audio_config(&self) -> RealtimeAudioConfig;
    fn connect(
        &self,
        interaction_mode: InteractionMode,
    ) -> Result<(
        Box<dyn RealtimeSession>,
        tokio::sync::mpsc::Receiver<RealtimeProviderEvent>,
    )>;
    fn health_check(&self) -> bool;
}

/// Active duplex streaming session for bidirectional voice transmission.
pub trait RealtimeSession: Send + Sync {
    fn send_audio(&self, pcm: &[i16]) -> Result<()>;
    fn commit_speech_turn(&self, pcm: &[i16]) -> Result<()>;
    fn cancel(&self) -> Result<()>;
    fn disconnect(&self) -> Result<()>;
}
