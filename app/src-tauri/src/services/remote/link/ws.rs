use std::net::SocketAddr;

use futures_util::{
    future::BoxFuture,
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use parking_lot::Mutex;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex as AsyncMutex,
};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::remote_link::{LinkCloseReason, LinkError, RemoteLink};
use crate::services::remote::{
    protocol::{
        framing::{check_control_message_size, AudioPayload},
        messages::{decode_control, encode_control, ControlMessage, DecodeOutcome},
    },
    MAX_CONTROL_MESSAGE_BYTES,
};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsSource = SplitStream<WsStream>;

/// WebSocket implementation of [`RemoteLink`] for the physical LAN transport.
///
/// Wraps a TCP-backed WebSocket stream and serializes/deserializes
/// [`ControlMessage`] wire frames conforming to `remote-control-spec.md` §7.1.
pub struct WsLink {
    writer: AsyncMutex<WsSink>,
    reader: AsyncMutex<WsSource>,
    close_reason: Mutex<Option<LinkCloseReason>>,
}

/// Inbound TCP listener that accepts and upgrades connections to [`WsLink`].
pub struct WsListener {
    listener: TcpListener,
}

impl RemoteLink for WsLink {
    fn send_control<'a>(&'a self, message: ControlMessage) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;
            let bytes = encode_control(&message)?;
            check_control_message_size(bytes.len(), MAX_CONTROL_MESSAGE_BYTES)?;

            let mut writer = self.writer.lock().await;
            writer
                .send(Message::Binary(bytes.into()))
                .await
                .map_err(|e| {
                    let reason = LinkCloseReason::Transport(e.to_string());
                    self.record_close(reason.clone());
                    LinkError::Closed(reason)
                })?;

            Ok(())
        })
    }

    fn recv_control<'a>(&'a self) -> BoxFuture<'a, Result<ControlMessage, LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;
            let mut reader = self.reader.lock().await;

            loop {
                match reader.next().await {
                    Some(Ok(Message::Binary(bytes))) => {
                        if let Some(msg) = self.decode_incoming(&bytes)? {
                            return Ok(msg);
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if let Some(msg) = self.decode_incoming(text.as_bytes())? {
                            return Ok(msg);
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        self.record_close(LinkCloseReason::Peer);
                        return Err(LinkError::Closed(LinkCloseReason::Peer));
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                        continue;
                    }
                    Some(Ok(Message::Frame(_))) => continue,
                    Some(Err(e)) => {
                        let reason = LinkCloseReason::Transport(e.to_string());
                        self.record_close(reason.clone());
                        return Err(LinkError::Closed(reason));
                    }
                    None => {
                        self.record_close(LinkCloseReason::Peer);
                        return Err(LinkError::Closed(LinkCloseReason::Peer));
                    }
                }
            }
        })
    }

    fn send_audio<'a>(&'a self, _payload: AudioPayload) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            Err(LinkError::Transport(
                "Audio streaming deferred to audio slice".into(),
            ))
        })
    }

    fn recv_audio<'a>(&'a self) -> BoxFuture<'a, Result<AudioPayload, LinkError>> {
        Box::pin(async move {
            Err(LinkError::Transport(
                "Audio streaming deferred to audio slice".into(),
            ))
        })
    }

    fn close<'a>(&'a self, reason: LinkCloseReason) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            {
                let mut guard = self.close_reason.lock();
                if let Some(existing) = guard.as_ref() {
                    return Err(LinkError::Closed(existing.clone()));
                }
                *guard = Some(reason);
            }

            let mut writer = self.writer.lock().await;
            let _ = writer.send(Message::Close(None)).await;
            let _ = writer.close().await;
            Ok(())
        })
    }

    fn close_reason(&self) -> Option<LinkCloseReason> {
        self.close_reason.lock().clone()
    }
}

impl WsLink {
    /// Creates a new `WsLink` from an established WebSocket stream.
    pub fn new(stream: WsStream) -> Self {
        let (writer, reader) = stream.split();
        Self {
            writer: AsyncMutex::new(writer),
            reader: AsyncMutex::new(reader),
            close_reason: Mutex::new(None),
        }
    }

    /// Verifies the link is currently open.
    fn ensure_open(&self) -> Result<(), LinkError> {
        if let Some(reason) = self.close_reason() {
            return Err(LinkError::Closed(reason));
        }
        Ok(())
    }

    /// Atomically records the close reason if not already closed.
    fn record_close(&self, reason: LinkCloseReason) {
        let mut guard = self.close_reason.lock();
        if guard.is_none() {
            *guard = Some(reason);
        }
    }

    /// Validates frame bounds and decodes incoming wire control bytes.
    fn decode_incoming(&self, bytes: &[u8]) -> Result<Option<ControlMessage>, LinkError> {
        if let Err(err) = check_control_message_size(bytes.len(), MAX_CONTROL_MESSAGE_BYTES) {
            self.record_close(LinkCloseReason::Protocol(err.clone()));
            return Err(LinkError::Protocol(err));
        }

        match decode_control(bytes) {
            Ok(DecodeOutcome::Known(message)) => Ok(Some(message)),
            Ok(DecodeOutcome::UnknownType { type_name }) => {
                log::warn!("[Remote::WsLink] Skipping unknown wire type: {type_name}");
                Ok(None)
            }
            Err(err) => {
                self.record_close(LinkCloseReason::Protocol(err.clone()));
                Err(LinkError::Protocol(err))
            }
        }
    }
}

impl WsListener {
    /// Accepts a new TCP stream and upgrades it to a WebSocket connection.
    pub async fn accept(&self) -> Result<WsLink, LinkError> {
        let (tcp_stream, _) = self
            .listener
            .accept()
            .await
            .map_err(|e| LinkError::Transport(e.to_string()))?;

        let stream = MaybeTlsStream::Plain(tcp_stream);
        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| LinkError::Transport(e.to_string()))?;

        Ok(WsLink::new(ws_stream))
    }

    /// Returns the local socket address this listener is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr, LinkError> {
        self.listener
            .local_addr()
            .map_err(|e| LinkError::Transport(e.to_string()))
    }
}

/// Binds a TCP listener on `addr` ready to accept WebSocket connections.
pub async fn bind(addr: SocketAddr) -> Result<WsListener, LinkError> {
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| LinkError::Transport(e.to_string()))?;
    Ok(WsListener { listener })
}

/// Connects to a WebSocket endpoint at `url` and returns a configured [`WsLink`].
pub async fn dial(url: &str) -> Result<WsLink, LinkError> {
    let connect_future = tokio_tungstenite::connect_async(url);
    let (ws_stream, _) = tokio::time::timeout(std::time::Duration::from_secs(8), connect_future)
        .await
        .map_err(|_| {
            LinkError::Transport(
                "Connection timed out. Ensure both devices are on the same Wi-Fi network.".into(),
            )
        })?
        .map_err(|e| LinkError::Transport(e.to_string()))?;
    Ok(WsLink::new(ws_stream))
}
