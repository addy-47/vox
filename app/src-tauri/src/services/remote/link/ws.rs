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
    peer_addr: Option<SocketAddr>,
    cipher: AsyncMutex<Option<crate::services::remote::security::NoiseTransport>>,
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

            let payload_to_send = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                cipher.encrypt(&bytes).map_err(|e| {
                    LinkError::Protocol(crate::services::remote::protocol::ProtocolError::MalformedJson(
                        format!("Noise encryption error: {e}"),
                    ))
                })?
            } else {
                bytes
            };

            let mut writer = self.writer.lock().await;
            writer
                .send(Message::Binary(payload_to_send.into()))
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
                        let decrypted = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                            cipher.decrypt(&bytes).map_err(|e| {
                                LinkError::Protocol(crate::services::remote::protocol::ProtocolError::MalformedJson(
                                    format!("Noise decryption error: {e}"),
                                ))
                            })?
                        } else {
                            bytes.to_vec()
                        };
                        if let Some(msg) = self.decode_incoming(&decrypted)? {
                            return Ok(msg);
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        let decrypted = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                            cipher.decrypt(text.as_bytes()).map_err(|e| {
                                LinkError::Protocol(crate::services::remote::protocol::ProtocolError::MalformedJson(
                                    format!("Noise decryption error: {e}"),
                                ))
                            })?
                        } else {
                            text.as_bytes().to_vec()
                        };
                        if let Some(msg) = self.decode_incoming(&decrypted)? {
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
        Self::with_addr(stream, None)
    }

    /// Creates a new `WsLink` with a known peer socket address.
    pub fn with_addr(stream: WsStream, peer_addr: Option<SocketAddr>) -> Self {
        let (writer, reader) = stream.split();
        Self {
            writer: AsyncMutex::new(writer),
            reader: AsyncMutex::new(reader),
            close_reason: Mutex::new(None),
            peer_addr,
            cipher: AsyncMutex::new(None),
        }
    }

    /// Returns the remote peer socket address if known.
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.peer_addr
    }

    /// Sets the active NoiseTransport cipher state for frame encryption/decryption.
    pub async fn set_cipher(&self, cipher: crate::services::remote::security::NoiseTransport) {
        *self.cipher.lock().await = Some(cipher);
    }

    /// Sends a raw handshake frame before or during Noise handshake negotiation.
    pub async fn send_handshake_message(&self, bytes: &[u8]) -> Result<(), LinkError> {
        self.ensure_open()?;
        let mut writer = self.writer.lock().await;
        writer
            .send(Message::Binary(bytes.to_vec().into()))
            .await
            .map_err(|e| {
                let reason = LinkCloseReason::Transport(e.to_string());
                self.record_close(reason.clone());
                LinkError::Closed(reason)
            })
    }

    /// Receives a raw handshake frame before or during Noise handshake negotiation.
    pub async fn recv_handshake_message(&self) -> Result<Vec<u8>, LinkError> {
        self.ensure_open()?;
        let mut reader = self.reader.lock().await;
        loop {
            match reader.next().await {
                Some(Ok(Message::Binary(bytes))) => return Ok(bytes.to_vec()),
                Some(Ok(Message::Close(_))) => {
                    self.record_close(LinkCloseReason::Peer);
                    return Err(LinkError::Closed(LinkCloseReason::Peer));
                }
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => continue,
                Some(Err(e)) => {
                    let reason = LinkCloseReason::Transport(e.to_string());
                    self.record_close(reason.clone());
                    return Err(LinkError::Closed(reason));
                }
                None => {
                    self.record_close(LinkCloseReason::Peer);
                    return Err(LinkError::Closed(LinkCloseReason::Peer));
                }
                Some(Ok(Message::Text(t))) => return Ok(t.as_bytes().to_vec()),
            }
        }
    }

    /// Drives the Noise_XX handshake state machine to completion over this WebSocket stream.
    pub async fn drive_handshake(
        &self,
        mut handshake: crate::services::remote::security::NoiseHandshake,
    ) -> Result<crate::services::remote::security::NoiseTransport, crate::services::remote::security::SecurityError> {
        loop {
            if handshake.is_complete() {
                break;
            }
            if handshake.is_my_turn() {
                let msg = handshake.write_message(&[])?;
                self.send_handshake_message(&msg)
                    .await
                    .map_err(|e| crate::services::remote::security::SecurityError::Noise(e.to_string()))?;
            } else {
                let msg = self
                    .recv_handshake_message()
                    .await
                    .map_err(|e| crate::services::remote::security::SecurityError::Noise(e.to_string()))?;
                handshake.read_message(&msg)?;
            }
        }
        handshake.finish()
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
        let (tcp_stream, peer_addr) = self
            .listener
            .accept()
            .await
            .map_err(|e| LinkError::Transport(e.to_string()))?;

        let stream = MaybeTlsStream::Plain(tcp_stream);
        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| LinkError::Transport(e.to_string()))?;

        Ok(WsLink::with_addr(ws_stream, Some(peer_addr)))
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
    dial_with_timeout(url, std::time::Duration::from_secs(8)).await
}

/// Connects with an explicit dial deadline. Used by the controller when it
/// cycles candidate addresses (explicit → hints → mDNS); a dead candidate
/// must not consume the full legacy timeout per attempt.
pub async fn dial_with_timeout(url: &str, timeout: std::time::Duration) -> Result<WsLink, LinkError> {
    let connect_future = tokio_tungstenite::connect_async(url);
    let (ws_stream, _) = tokio::time::timeout(timeout, connect_future)
        .await
        .map_err(|_| {
            LinkError::Transport(format!(
                "Connection timed out ({}s limit). Host did not respond at target endpoint.",
                timeout.as_secs()
            ))
        })?
        .map_err(|e| LinkError::Transport(e.to_string()))?;
    Ok(WsLink::new(ws_stream))
}
