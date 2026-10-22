use std::sync::Arc;

use futures_util::future::BoxFuture;
use parking_lot::Mutex;
use tokio::sync::{
    mpsc::{self, error::TryRecvError},
    Mutex as AsyncMutex, Notify,
};

use super::{
    remote_link::{LinkCloseReason, LinkError, RawLink, RemoteLink},
    LOOPBACK_CHANNEL_CAPACITY,
};
use crate::services::remote::{
    protocol::{
        errors::{classify_protocol_error, ProtocolError, ProtocolResponse},
        framing::{
            check_control_message_size, decode_audio_payload, encode_audio_payload, AudioPayload,
        },
        messages::{decode_control, encode_control, ControlMessage, DecodeOutcome},
    },
    MAX_CONTROL_MESSAGE_BYTES,
};

/// The close signal one end of a loopback link publishes for its peer.
///
/// The reason is written before the notification is sent, so an observer
/// that wakes up always finds the reason that woke it.
struct CloseSignal {
    reason: Mutex<Option<LinkCloseReason>>,
    notify: Notify,
}

impl CloseSignal {
    /// Creates a signal for a link that has not closed.
    fn new() -> Self {
        Self {
            reason: Mutex::new(None),
            notify: Notify::new(),
        }
    }

    /// Records the close reason and wakes every waiter.
    fn close(&self, reason: LinkCloseReason) {
        {
            let mut guard = self.reason.lock();
            if guard.is_some() {
                return;
            }
            *guard = Some(reason);
        }
        self.notify.notify_waiters();
    }

    /// The close reason, if the link closed.
    fn reason(&self) -> Option<LinkCloseReason> {
        self.reason.lock().clone()
    }
}

/// The in-process remote link.
///
/// Two connected ends are created by [`LoopbackLink::pair`]. Each end has a
/// control channel and an audio channel to its peer, exactly mirroring the
/// two WebSocket connections the real link uses. The two channels are
/// independent, so a slow audio reader cannot block a control message — the
/// separation is the reason the spec mandates two sockets, and the loopback
/// reproduces it.
///
/// The link carries **bytes**, not typed messages: a control message is
/// encoded with the protocol module on the way out and decoded with the
/// protocol module on the way in. There is therefore exactly one
/// implementation of the wire format, and the in-process tests exercise the
/// real encoding rather than a shortcut.
pub struct LoopbackLink {
    control_tx: mpsc::Sender<Vec<u8>>,
    control_rx: AsyncMutex<mpsc::Receiver<Vec<u8>>>,
    audio_tx: mpsc::Sender<Vec<u8>>,
    audio_rx: AsyncMutex<mpsc::Receiver<Vec<u8>>>,
    own_close: Arc<CloseSignal>,
    peer_close: Arc<CloseSignal>,
}

impl LoopbackLink {
    /// Creates two connected ends of an in-process link.
    pub fn pair() -> (LoopbackLink, LoopbackLink) {
        let (control_ab_tx, control_ab_rx) = mpsc::channel::<Vec<u8>>(LOOPBACK_CHANNEL_CAPACITY);
        let (control_ba_tx, control_ba_rx) = mpsc::channel::<Vec<u8>>(LOOPBACK_CHANNEL_CAPACITY);
        let (audio_ab_tx, audio_ab_rx) = mpsc::channel::<Vec<u8>>(LOOPBACK_CHANNEL_CAPACITY);
        let (audio_ba_tx, audio_ba_rx) = mpsc::channel::<Vec<u8>>(LOOPBACK_CHANNEL_CAPACITY);
        let close_a = Arc::new(CloseSignal::new());
        let close_b = Arc::new(CloseSignal::new());

        let first = LoopbackLink {
            control_tx: control_ab_tx,
            control_rx: AsyncMutex::new(control_ba_rx),
            audio_tx: audio_ab_tx,
            audio_rx: AsyncMutex::new(audio_ba_rx),
            own_close: Arc::clone(&close_a),
            peer_close: Arc::clone(&close_b),
        };
        let second = LoopbackLink {
            control_tx: control_ba_tx,
            control_rx: AsyncMutex::new(control_ab_rx),
            audio_tx: audio_ba_tx,
            audio_rx: AsyncMutex::new(audio_ab_rx),
            own_close: Arc::clone(&close_b),
            peer_close: Arc::clone(&close_a),
        };
        (first, second)
    }

    /// Fails when this link is closed, by either side.
    fn ensure_open(&self) -> Result<(), LinkError> {
        if let Some(reason) = self.own_close.reason() {
            return Err(LinkError::Closed(reason));
        }
        if let Some(reason) = observed_peer_reason(&self.peer_close) {
            return Err(LinkError::Closed(reason));
        }
        Ok(())
    }

    /// Terminates the link because the peer broke the wire contract.
    ///
    /// The cause travels with the close: a protocol violation is not a
    /// deliberate close, so the reason is [`LinkCloseReason::Protocol`] and
    /// not `Local`.
    fn terminate_on_protocol_violation(&self, error: &ProtocolError) {
        log::warn!("[Remote::Link] Terminating the link: {error}");
        self.own_close
            .close(LinkCloseReason::Protocol(error.clone()));
    }

    /// Decides what a decode failure does to the link.
    ///
    /// The protocol module owns the classification. A transport consults it
    /// instead of matching protocol errors by hand, so the next transport
    /// cannot disagree with this one about which violations are fatal.
    ///
    /// Only the fatal case reaches this function: a survivable violation is
    /// skipped by the receive loop, not reported to the caller.
    fn handle_decode_failure(&self, error: ProtocolError) -> LinkError {
        self.terminate_on_protocol_violation(&error);
        LinkError::Closed(LinkCloseReason::Protocol(error))
    }

    /// Receives the next payload from a channel, or reports that the link
    /// closed.
    async fn next_bytes(
        &self,
        receiver: &AsyncMutex<mpsc::Receiver<Vec<u8>>>,
    ) -> Result<Vec<u8>, LinkError> {
        let mut guard = receiver.lock().await;
        match guard.try_recv() {
            Ok(bytes) => return Ok(bytes),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(LinkError::Closed(LinkCloseReason::Peer));
            }
        }

        // The notification is enabled before the close reason is read, so a
        // close that lands between the two cannot be missed.
        let notified = self.peer_close.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.peer_close.reason().is_some() {
            return Err(LinkError::Closed(
                observed_peer_reason(&self.peer_close).unwrap_or(LinkCloseReason::Peer),
            ));
        }

        tokio::select! {
            biased;
            result = guard.recv() => match result {
                Some(bytes) => Ok(bytes),
                None => Err(LinkError::Closed(LinkCloseReason::Peer)),
            },
            _ = notified => Err(LinkError::Closed(
                observed_peer_reason(&self.peer_close).unwrap_or(LinkCloseReason::Peer),
            )),
        }
    }
}

impl RemoteLink for LoopbackLink {
    fn send_control<'a>(&'a self, message: ControlMessage) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;
            let bytes = encode_control(&message)?;
            check_control_message_size(bytes.len(), MAX_CONTROL_MESSAGE_BYTES)?;
            self.control_tx
                .send(bytes)
                .await
                .map_err(|_| LinkError::Closed(LinkCloseReason::Peer))?;
            Ok(())
        })
    }

    fn recv_control<'a>(&'a self) -> BoxFuture<'a, Result<ControlMessage, LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;
            loop {
                let bytes = self.next_bytes(&self.control_rx).await?;
                if let Err(error) =
                    check_control_message_size(bytes.len(), MAX_CONTROL_MESSAGE_BYTES)
                {
                    return Err(self.handle_decode_failure(error));
                }
                match decode_control(&bytes) {
                    Ok(DecodeOutcome::Known(message)) => return Ok(message),
                    Ok(DecodeOutcome::UnknownType { type_name }) => {
                        log::warn!(
                            "[Remote::Link] Skipping an unknown control message type={type_name}"
                        );
                    }
                    Err(error) => match classify_protocol_error(&error) {
                        // A survivable violation: the link continues. Report
                        // it, drop the message, and wait for the next one.
                        ProtocolResponse::Skippable => {
                            log::warn!("[Remote::Link] Skipping a survivable violation: {error}");
                        }
                        ProtocolResponse::Fatal => {
                            return Err(self.handle_decode_failure(error));
                        }
                    },
                }
            }
        })
    }

    fn send_audio<'a>(&'a self, payload: AudioPayload) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;
            let bytes = encode_audio_payload(&payload)?;
            self.audio_tx
                .send(bytes)
                .await
                .map_err(|_| LinkError::Closed(LinkCloseReason::Peer))?;
            Ok(())
        })
    }

    fn recv_audio<'a>(&'a self) -> BoxFuture<'a, Result<AudioPayload, LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;
            loop {
                let bytes = self.next_bytes(&self.audio_rx).await?;
                if let Err(error) =
                    check_control_message_size(bytes.len(), MAX_CONTROL_MESSAGE_BYTES)
                {
                    return Err(self.handle_decode_failure(error));
                }
                match decode_audio_payload(&bytes) {
                    Ok(DecodeOutcome::Known(payload)) => return Ok(payload),
                    Ok(DecodeOutcome::UnknownType { type_name }) => {
                        log::warn!(
                            "[Remote::Link] Skipping an unknown audio message type={type_name}"
                        );
                    }
                    Err(error) => match classify_protocol_error(&error) {
                        ProtocolResponse::Skippable => {
                            log::warn!("[Remote::Link] Skipping a survivable violation: {error}");
                        }
                        ProtocolResponse::Fatal => {
                            return Err(self.handle_decode_failure(error));
                        }
                    },
                }
            }
        })
    }

    fn close<'a>(&'a self, reason: LinkCloseReason) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            if let Some(existing) = self.own_close.reason() {
                return Err(LinkError::Closed(existing));
            }
            self.own_close.close(reason);
            Ok(())
        })
    }

    fn close_reason(&self) -> Option<LinkCloseReason> {
        if let Some(reason) = self.own_close.reason() {
            return Some(reason);
        }
        observed_peer_reason(&self.peer_close)
    }
}

impl RawLink for LoopbackLink {
    fn send_control_bytes<'a>(&'a self, bytes: Vec<u8>) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;
            self.control_tx
                .send(bytes)
                .await
                .map_err(|_| LinkError::Closed(LinkCloseReason::Peer))?;
            Ok(())
        })
    }

    fn send_audio_bytes<'a>(&'a self, bytes: Vec<u8>) -> BoxFuture<'a, Result<(), LinkError>> {
        Box::pin(async move {
            self.ensure_open()?;
            self.audio_tx
                .send(bytes)
                .await
                .map_err(|_| LinkError::Closed(LinkCloseReason::Peer))?;
            Ok(())
        })
    }
}

/// The reason this end observed, given the reason the peer closed with.
///
/// A deliberate close on either side is observed as `Peer`. A transport
/// failure keeps its detail, because a transport that died is a transport
/// failure whoever noticed it. A protocol violation keeps its cause: the
/// peer broke the wire contract, and that is worth knowing on this side too.
fn observed_peer_reason(signal: &CloseSignal) -> Option<LinkCloseReason> {
    signal.reason().map(|reason| match reason {
        LinkCloseReason::Transport(detail) => LinkCloseReason::Transport(detail),
        LinkCloseReason::Protocol(error) => LinkCloseReason::Protocol(error),
        LinkCloseReason::Local | LinkCloseReason::Peer => LinkCloseReason::Peer,
    })
}
