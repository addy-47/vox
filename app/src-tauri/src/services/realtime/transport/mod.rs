pub mod connection;
pub mod health;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32},
        Arc,
    },
    time::Duration,
};

use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::{
    core::state::InteractionState,
    services::realtime::{OutboundCommand, RealtimeProviderEvent},
};

/// WebSocket sink half for a TLS-wrapped TCP connection.
pub(crate) type WsWriter = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

/// WebSocket source half for a TLS-wrapped TCP connection.
pub(crate) type WsReader = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

/// Async factory that produces a fresh `(WsWriter, WsReader)` pair on each reconnect attempt.
pub(crate) type ReconnectFn = Box<
    dyn Fn() -> futures_util::future::BoxFuture<'static, anyhow::Result<(WsWriter, WsReader)>>
        + Send
        + Sync,
>;

/// Action returned by a provider frame handler to control reconnect behaviour.
pub(crate) enum FrameAction {
    Continue,
    GoAway,
}

/// Provider-specific wire encoding and inbound frame decoding injected into the shared harness.
pub(crate) trait ProviderDriver: Send + Sync + 'static {
    fn encode(&self, cmd: OutboundCommand) -> Option<Message>;
    fn handle_frame(
        &self,
        msg: Message,
        event_tx: &mpsc::Sender<RealtimeProviderEvent>,
    ) -> FrameAction;
    fn keepalive_interval(&self) -> Option<Duration>;
}

/// Tunable capacity and backoff parameters for the shared reconnect harness.
pub(crate) struct HarnessConfig {
    pub(crate) max_reconnect_attempts: usize,
    pub(crate) reconnect_base_delay_secs: u64,
    pub(crate) reconnect_factor_secs: u64,
}

/// Initialisation bundle for `spawn_harness`, grouping the live connection and runtime context.
pub(crate) struct HarnessInit {
    pub(crate) ws_write: WsWriter,
    pub(crate) ws_read: WsReader,
    pub(crate) reconnect_fn: ReconnectFn,
    pub(crate) provider_event_tx: mpsc::Sender<RealtimeProviderEvent>,
    pub(crate) state_rx: tokio::sync::watch::Receiver<InteractionState>,
    pub(crate) turn_id_ref: Arc<AtomicU32>,
    pub(crate) tokio_handle: tokio::runtime::Handle,
}

/// Handles returned to the session struct after `spawn_harness` for command dispatch and shutdown.
pub(crate) struct HarnessHandles {
    pub(crate) outbound_tx: mpsc::Sender<OutboundCommand>,
    pub(crate) shutdown_tx: parking_lot::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub(crate) terminated: Arc<AtomicBool>,
}
