use std::{collections::VecDeque, net::SocketAddr};

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

use super::remote_link::{LinkCloseReason, LinkError, RawLink, RemoteLink};
use crate::services::remote::{
    protocol::{
        errors::{classify_protocol_error, ProtocolResponse},
        framing::{
            check_control_message_size, decode_audio_payload, encode_audio_payload, AudioPayload,
        },
        messages::{decode_control, encode_control, ControlMessage, DecodeOutcome},
        ProtocolError,
    },
    security::{NoiseHandshake, NoiseTransport, SecurityError},
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
    writer: AsyncMutex<Option<WsSink>>,
    reader: AsyncMutex<Option<WsSource>>,
    close_reason: Mutex<Option<LinkCloseReason>>,
    peer_addr: Option<SocketAddr>,
    #[cfg_attr(target_os = "android", allow(dead_code))]
    raw_fd: Option<i32>,
    cipher: AsyncMutex<Option<NoiseTransport>>,
    buffered_control: AsyncMutex<VecDeque<ControlMessage>>,
    buffered_audio: AsyncMutex<VecDeque<AudioPayload>>,
}

/// Helper that inspects raw wire bytes to determine if they form a JSON ControlMessage.
fn is_control_bytes(bytes: &[u8]) -> bool {
    let trimmed = bytes.iter().find(|&&b| !b.is_ascii_whitespace());
    trimmed == Some(&b'{')
}

/// Inbound TCP listener that accepts and upgrades connections to [`WsLink`].
pub struct WsListener {
    listener: TcpListener,
}

impl RemoteLink for WsLink {
    fn send_control<'a>(&'a self, message: ControlMessage) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            self.check_peer_close().await?;
            let bytes = encode_control(&message)?;
            check_control_message_size(bytes.len(), MAX_CONTROL_MESSAGE_BYTES)?;

            let payload_to_send = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                cipher.encrypt(&bytes).map_err(|e| {
                    LinkError::Protocol(ProtocolError::MalformedJson(format!(
                        "Noise encryption error: {e}"
                    )))
                })?
            } else {
                bytes
            };

            let mut guard = self.writer.lock().await;
            let writer = guard.as_mut().ok_or_else(|| {
                LinkError::Closed(self.close_reason().unwrap_or(LinkCloseReason::Local))
            })?;
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

            // Check if a control message was previously buffered while reading audio.
            {
                let mut guard = self.buffered_control.lock().await;
                if let Some(msg) = guard.pop_front() {
                    return Ok(msg);
                }
            }

            let mut reader_guard = self.reader.lock().await;
            let reader = reader_guard.as_mut().ok_or_else(|| {
                LinkError::Closed(self.close_reason().unwrap_or(LinkCloseReason::Local))
            })?;

            loop {
                // Check buffer again in case another concurrent read populated it.
                {
                    let mut guard = self.buffered_control.lock().await;
                    if let Some(msg) = guard.pop_front() {
                        return Ok(msg);
                    }
                }

                match reader.next().await {
                    Some(Ok(Message::Binary(bytes))) => {
                        let decrypted = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                            cipher.decrypt(&bytes).map_err(|e| {
                                LinkError::Protocol(ProtocolError::MalformedJson(format!(
                                    "Noise decryption error: {e}"
                                )))
                            })?
                        } else {
                            bytes.to_vec()
                        };

                        if let Err(err) =
                            check_control_message_size(decrypted.len(), MAX_CONTROL_MESSAGE_BYTES)
                        {
                            return Err(self
                                .terminate_on_protocol_violation(err, &mut reader_guard)
                                .await);
                        }

                        if is_control_bytes(&decrypted) {
                            match self.decode_incoming(&decrypted) {
                                Ok(Some(msg)) => return Ok(msg),
                                Ok(None) => {}
                                Err(err) => {
                                    return Err(self
                                        .terminate_on_protocol_violation(err, &mut reader_guard)
                                        .await)
                                }
                            }
                        } else {
                            // Demux: audio payload buffered for recv_audio.
                            match decode_audio_payload(&decrypted) {
                                Ok(DecodeOutcome::Known(payload)) => {
                                    self.buffered_audio.lock().await.push_back(payload);
                                }
                                Ok(DecodeOutcome::UnknownType { type_name }) => {
                                    log::warn!("[WsLink] Unknown audio payload type: {type_name}");
                                }
                                Err(err) => match classify_protocol_error(&err) {
                                    ProtocolResponse::Skippable => {
                                        log::warn!(
                                            "[WsLink] Skipping survivable audio error: {err}"
                                        );
                                    }
                                    ProtocolResponse::Fatal => {
                                        return Err(self
                                            .terminate_on_protocol_violation(err, &mut reader_guard)
                                            .await);
                                    }
                                },
                            }
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        let decrypted = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                            cipher.decrypt(text.as_bytes()).map_err(|e| {
                                LinkError::Protocol(ProtocolError::MalformedJson(format!(
                                    "Noise decryption error: {e}"
                                )))
                            })?
                        } else {
                            text.as_bytes().to_vec()
                        };

                        if let Err(err) =
                            check_control_message_size(decrypted.len(), MAX_CONTROL_MESSAGE_BYTES)
                        {
                            return Err(self
                                .terminate_on_protocol_violation(err, &mut reader_guard)
                                .await);
                        }

                        if is_control_bytes(&decrypted) {
                            match self.decode_incoming(&decrypted) {
                                Ok(Some(msg)) => return Ok(msg),
                                Ok(None) => {}
                                Err(err) => {
                                    return Err(self
                                        .terminate_on_protocol_violation(err, &mut reader_guard)
                                        .await)
                                }
                            }
                        } else {
                            match decode_audio_payload(&decrypted) {
                                Ok(DecodeOutcome::Known(payload)) => {
                                    self.buffered_audio.lock().await.push_back(payload);
                                }
                                Ok(DecodeOutcome::UnknownType { type_name }) => {
                                    log::warn!("[WsLink] Unknown audio payload type: {type_name}");
                                }
                                Err(err) => match classify_protocol_error(&err) {
                                    ProtocolResponse::Skippable => {
                                        log::warn!(
                                            "[WsLink] Skipping survivable audio error: {err}"
                                        );
                                    }
                                    ProtocolResponse::Fatal => {
                                        return Err(self
                                            .terminate_on_protocol_violation(err, &mut reader_guard)
                                            .await);
                                    }
                                },
                            }
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

    fn send_audio<'a>(&'a self, payload: AudioPayload) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            self.check_peer_close().await?;
            let bytes = encode_audio_payload(&payload).map_err(LinkError::Protocol)?;
            let payload_to_send = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                cipher.encrypt(&bytes).map_err(|e| {
                    LinkError::Protocol(ProtocolError::MalformedJson(format!(
                        "Noise encryption error: {e}"
                    )))
                })?
            } else {
                bytes
            };

            let mut guard = self.writer.lock().await;
            let writer = guard.as_mut().ok_or_else(|| {
                LinkError::Closed(self.close_reason().unwrap_or(LinkCloseReason::Local))
            })?;
            writer
                .send(Message::Binary(payload_to_send.into()))
                .await
                .map_err(|e| {
                    let reason = LinkCloseReason::Transport(e.to_string());
                    self.record_close(reason.clone());
                    LinkError::Closed(reason)
                })
        })
    }

    fn recv_audio<'a>(&'a self) -> BoxFuture<'a, Result<AudioPayload, LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;

            // Check if an audio payload was previously buffered while reading control messages.
            {
                let mut guard = self.buffered_audio.lock().await;
                if let Some(payload) = guard.pop_front() {
                    return Ok(payload);
                }
            }

            let mut reader_guard = self.reader.lock().await;
            let reader = reader_guard.as_mut().ok_or_else(|| {
                LinkError::Closed(self.close_reason().unwrap_or(LinkCloseReason::Local))
            })?;

            loop {
                // Check buffer again in case another concurrent read populated it.
                {
                    let mut guard = self.buffered_audio.lock().await;
                    if let Some(payload) = guard.pop_front() {
                        return Ok(payload);
                    }
                }

                match reader.next().await {
                    Some(Ok(Message::Binary(bytes))) => {
                        let decrypted = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                            cipher.decrypt(&bytes).map_err(|e| {
                                LinkError::Protocol(ProtocolError::MalformedJson(format!(
                                    "Noise decryption error: {e}"
                                )))
                            })?
                        } else {
                            bytes.to_vec()
                        };

                        if let Err(err) =
                            check_control_message_size(decrypted.len(), MAX_CONTROL_MESSAGE_BYTES)
                        {
                            return Err(self
                                .terminate_on_protocol_violation(err, &mut reader_guard)
                                .await);
                        }

                        if is_control_bytes(&decrypted) {
                            // Demux: control message buffered for recv_control.
                            match self.decode_incoming(&decrypted) {
                                Ok(Some(msg)) => {
                                    self.buffered_control.lock().await.push_back(msg);
                                }
                                Ok(None) => {}
                                Err(err) => {
                                    return Err(self
                                        .terminate_on_protocol_violation(err, &mut reader_guard)
                                        .await)
                                }
                            }
                        } else {
                            match decode_audio_payload(&decrypted) {
                                Ok(DecodeOutcome::Known(payload)) => return Ok(payload),
                                Ok(DecodeOutcome::UnknownType { type_name }) => {
                                    log::warn!("[WsLink] Unknown audio payload type: {type_name}");
                                    continue;
                                }
                                Err(err) => match classify_protocol_error(&err) {
                                    ProtocolResponse::Skippable => {
                                        log::warn!(
                                            "[WsLink] Skipping survivable audio error: {err}"
                                        );
                                        continue;
                                    }
                                    ProtocolResponse::Fatal => {
                                        return Err(self
                                            .terminate_on_protocol_violation(err, &mut reader_guard)
                                            .await);
                                    }
                                },
                            }
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        let decrypted = if let Some(cipher) = self.cipher.lock().await.as_mut() {
                            cipher.decrypt(text.as_bytes()).map_err(|e| {
                                LinkError::Protocol(ProtocolError::MalformedJson(format!(
                                    "Noise decryption error: {e}"
                                )))
                            })?
                        } else {
                            text.as_bytes().to_vec()
                        };

                        if let Err(err) =
                            check_control_message_size(decrypted.len(), MAX_CONTROL_MESSAGE_BYTES)
                        {
                            return Err(self
                                .terminate_on_protocol_violation(err, &mut reader_guard)
                                .await);
                        }

                        if is_control_bytes(&decrypted) {
                            match self.decode_incoming(&decrypted) {
                                Ok(Some(msg)) => {
                                    self.buffered_control.lock().await.push_back(msg);
                                }
                                Ok(None) => {}
                                Err(err) => {
                                    return Err(self
                                        .terminate_on_protocol_violation(err, &mut reader_guard)
                                        .await)
                                }
                            }
                        } else {
                            match decode_audio_payload(&decrypted) {
                                Ok(DecodeOutcome::Known(payload)) => return Ok(payload),
                                Ok(DecodeOutcome::UnknownType { type_name }) => {
                                    log::warn!("[WsLink] Unknown audio payload type: {type_name}");
                                    continue;
                                }
                                Err(err) => match classify_protocol_error(&err) {
                                    ProtocolResponse::Skippable => {
                                        log::warn!(
                                            "[WsLink] Skipping survivable audio error: {err}"
                                        );
                                        continue;
                                    }
                                    ProtocolResponse::Fatal => {
                                        return Err(self
                                            .terminate_on_protocol_violation(err, &mut reader_guard)
                                            .await);
                                    }
                                },
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        self.record_close(LinkCloseReason::Peer);
                        return Err(LinkError::Closed(LinkCloseReason::Peer));
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => continue,
                    Some(Ok(Message::Frame(_))) => continue,
                    Some(Err(e)) => {
                        let reason = LinkCloseReason::Transport(e.to_string());
                        self.record_close(reason.clone());
                        return Err(LinkError::Closed(reason));
                    }
                }
            }
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

            let writer_opt = self.writer.lock().await.take();
            let reader_opt = self.reader.lock().await.take();
            if let Some(mut writer) = writer_opt {
                let _ = writer.send(Message::Close(None)).await;
                let _ = writer.close().await;
            }
            drop(reader_opt);
            Ok(())
        })
    }

    fn close_reason(&self) -> Option<LinkCloseReason> {
        self.close_reason.lock().clone()
    }
}

impl RawLink for WsLink {
    fn send_control_bytes<'a>(&'a self, bytes: Vec<u8>) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move { self.send_handshake_message(&bytes).await })
    }

    fn send_audio_bytes<'a>(&'a self, bytes: Vec<u8>) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move { self.send_handshake_message(&bytes).await })
    }
}

impl WsLink {
    /// Creates a new `WsLink` from an established WebSocket stream.
    pub fn new(stream: WsStream) -> Self {
        Self::with_addr(stream, None)
    }

    /// Creates a new `WsLink` with a known peer socket address.
    pub fn with_addr(stream: WsStream, peer_addr: Option<SocketAddr>) -> Self {
        #[cfg(all(unix, not(target_os = "android")))]
        let raw_fd = match stream.get_ref() {
            MaybeTlsStream::Plain(s) => {
                use std::os::fd::AsRawFd;
                Some(s.as_raw_fd())
            }
            _ => None,
        };
        #[cfg(not(all(unix, not(target_os = "android"))))]
        let raw_fd = None;

        let (writer, reader) = stream.split();
        Self {
            writer: AsyncMutex::new(Some(writer)),
            reader: AsyncMutex::new(Some(reader)),
            close_reason: Mutex::new(None),
            peer_addr,
            raw_fd,
            cipher: AsyncMutex::new(None),
            buffered_control: AsyncMutex::new(VecDeque::new()),
            buffered_audio: AsyncMutex::new(VecDeque::new()),
        }
    }

    /// Returns the remote peer socket address if known.
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.peer_addr
    }

    /// Sets the active NoiseTransport cipher state for frame encryption/decryption.
    pub async fn set_cipher(&self, cipher: NoiseTransport) {
        *self.cipher.lock().await = Some(cipher);
    }

    /// Sends a raw handshake frame before or during Noise handshake negotiation.
    pub async fn send_handshake_message(&self, bytes: &[u8]) -> Result<(), LinkError> {
        self.check_peer_close().await?;
        let mut guard = self.writer.lock().await;
        let writer = guard.as_mut().ok_or_else(|| {
            LinkError::Closed(self.close_reason().unwrap_or(LinkCloseReason::Local))
        })?;
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
        let mut guard = self.reader.lock().await;
        let reader = guard.as_mut().ok_or_else(|| {
            LinkError::Closed(self.close_reason().unwrap_or(LinkCloseReason::Local))
        })?;
        loop {
            match reader.next().await {
                Some(Ok(Message::Binary(bytes))) => return Ok(bytes.to_vec()),
                Some(Ok(Message::Close(_))) => {
                    self.record_close(LinkCloseReason::Peer);
                    return Err(LinkError::Closed(LinkCloseReason::Peer));
                }
                Some(Ok(Message::Ping(_)))
                | Some(Ok(Message::Pong(_)))
                | Some(Ok(Message::Frame(_))) => continue,
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
        mut handshake: NoiseHandshake,
    ) -> Result<NoiseTransport, SecurityError> {
        loop {
            if handshake.is_complete() {
                break;
            }
            if handshake.is_my_turn() {
                let msg = handshake.write_message(&[])?;
                self.send_handshake_message(&msg)
                    .await
                    .map_err(|e| SecurityError::Noise(e.to_string()))?;
            } else {
                let msg = self
                    .recv_handshake_message()
                    .await
                    .map_err(|e| SecurityError::Noise(e.to_string()))?;
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

    /// Checks for any peer close frame or transport failure buffered on the reader.
    async fn check_peer_close(&self) -> Result<(), LinkError> {
        self.ensure_open()?;

        #[cfg(all(unix, not(target_os = "android")))]
        if let Some(fd) = self.raw_fd {
            let mut buf = [0u8; 1];
            let n = unsafe {
                libc::recv(
                    fd,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    1,
                    libc::MSG_PEEK | libc::MSG_DONTWAIT,
                )
            };
            if n == 0 || (n > 0 && buf[0] == 0x88) {
                self.record_close(LinkCloseReason::Peer);
                return Err(LinkError::Closed(LinkCloseReason::Peer));
            } else if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ECONNRESET)
                    || err.raw_os_error() == Some(libc::EPIPE)
                {
                    self.record_close(LinkCloseReason::Peer);
                    return Err(LinkError::Closed(LinkCloseReason::Peer));
                }
            }
        }

        if let Ok(mut guard) = self.reader.try_lock() {
            if let Some(reader) = guard.as_mut() {
                let mut pinned = std::pin::pin!(reader.next());
                if let std::task::Poll::Ready(msg) = futures_util::poll!(&mut pinned) {
                    match msg {
                        Some(Ok(Message::Close(_))) | None => {
                            self.record_close(LinkCloseReason::Peer);
                            return Err(LinkError::Closed(LinkCloseReason::Peer));
                        }
                        Some(Ok(Message::Binary(bytes))) => {
                            let decrypted = if let Some(cipher) = self.cipher.lock().await.as_mut()
                            {
                                cipher.decrypt(&bytes).unwrap_or_else(|_| bytes.to_vec())
                            } else {
                                bytes.to_vec()
                            };
                            if let Err(err) = check_control_message_size(
                                decrypted.len(),
                                MAX_CONTROL_MESSAGE_BYTES,
                            ) {
                                return Err(self
                                    .terminate_on_protocol_violation(err, &mut guard)
                                    .await);
                            }
                            if is_control_bytes(&decrypted) {
                                if let Ok(Some(m)) = self.decode_incoming(&decrypted) {
                                    self.buffered_control.lock().await.push_back(m);
                                }
                            } else if let Ok(DecodeOutcome::Known(p)) =
                                decode_audio_payload(&decrypted)
                            {
                                self.buffered_audio.lock().await.push_back(p);
                            }
                        }
                        Some(Ok(Message::Text(text))) => {
                            let bytes = text.as_bytes();
                            if let Err(err) =
                                check_control_message_size(bytes.len(), MAX_CONTROL_MESSAGE_BYTES)
                            {
                                return Err(self
                                    .terminate_on_protocol_violation(err, &mut guard)
                                    .await);
                            }
                            if is_control_bytes(bytes) {
                                if let Ok(Some(m)) = self.decode_incoming(bytes) {
                                    self.buffered_control.lock().await.push_back(m);
                                }
                            }
                        }
                        Some(Err(e)) => {
                            let reason = LinkCloseReason::Transport(e.to_string());
                            self.record_close(reason.clone());
                            return Err(LinkError::Closed(reason));
                        }
                        _ => {}
                    }
                }
            }
        }
        self.ensure_open()
    }

    /// Atomically records the close reason if not already closed.
    fn record_close(&self, reason: LinkCloseReason) {
        let mut guard = self.close_reason.lock();
        if guard.is_none() {
            *guard = Some(reason);
        }
    }

    /// Terminates the link on a fatal protocol violation, dropping streams and recording the cause.
    async fn terminate_on_protocol_violation(
        &self,
        error: ProtocolError,
        reader_guard: &mut Option<WsSource>,
    ) -> LinkError {
        let reason = LinkCloseReason::Protocol(error);
        self.record_close(reason.clone());
        let _ = reader_guard.take();
        if let Some(mut writer) = self.writer.lock().await.take() {
            let _ = writer.send(Message::Close(None)).await;
            let _ = writer.close().await;
        }
        LinkError::Closed(reason)
    }

    /// Validates frame bounds and decodes incoming wire control bytes.
    fn decode_incoming(&self, bytes: &[u8]) -> Result<Option<ControlMessage>, ProtocolError> {
        check_control_message_size(bytes.len(), MAX_CONTROL_MESSAGE_BYTES)?;

        match decode_control(bytes) {
            Ok(DecodeOutcome::Known(message)) => Ok(Some(message)),
            Ok(DecodeOutcome::UnknownType { type_name }) => {
                log::warn!("[Remote::WsLink] Skipping unknown wire type: {type_name}");
                Ok(None)
            }
            Err(err) => match classify_protocol_error(&err) {
                ProtocolResponse::Skippable => {
                    log::warn!("[Remote::WsLink] Skipping survivable control error: {err}");
                    Ok(None)
                }
                ProtocolResponse::Fatal => Err(err),
            },
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
pub async fn dial_with_timeout(
    url: &str,
    timeout: std::time::Duration,
) -> Result<WsLink, LinkError> {
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
