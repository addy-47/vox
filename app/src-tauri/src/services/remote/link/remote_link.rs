use futures_util::future::BoxFuture;

use crate::services::remote::protocol::{
    errors::ProtocolError, framing::AudioPayload, messages::ControlMessage,
};

/// Why a link was closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkCloseReason {
    /// This side closed the link deliberately.
    ///
    /// A deliberate close is a decision. A link that terminates itself
    /// because the peer broke the wire contract is not a decision, and it
    /// reports [`LinkCloseReason::Protocol`] instead.
    Local,
    /// The other side closed the link.
    Peer,
    /// The link died at the transport level.
    Transport(String),
    /// The peer broke the wire contract, and this side closed the link.
    ///
    /// The cause travels with the close, so a reader of the close reason can
    /// tell "the peer sent an oversized message" from "we closed the link
    /// because the session ended". The two demand different reactions: the
    /// first is a protocol error worth logging loudly, the second is routine.
    Protocol(ProtocolError),
}

/// Why a peer rejected a connection during initial handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerRejection {
    UnknownPeer(String),
    LinkRefused(crate::services::remote::protocol::messages::LinkRefusedReason),
    VersionMismatch {
        version: u16,
        supported_version: u16,
    },
    RoleNotPermitted(String),
}

/// A link failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    /// The link is closed, in the given way.
    #[error("the link is closed: {0:?}")]
    Closed(LinkCloseReason),
    /// The peer rejected the connection during handshake.
    #[error("peer rejected connection: {0:?}")]
    PeerRejected(PeerRejection),
    /// The transport failed.
    #[error("transport failure: {0}")]
    Transport(String),
    /// A frame could not be decoded.
    #[error("protocol failure: {0}")]
    Protocol(#[from] ProtocolError),
    /// The operation exceeded its deadline.
    #[error("the operation timed out")]
    Timeout,
}

/// The transport-agnostic remote link.
///
/// This is the only way the rest of the remote-control feature talks to a
/// connection. The host role and the controller role use the same trait, so
/// a method that only makes sense for one role does not belong here.
///
/// The trait is object-safe: the session manager holds an
/// `Arc<dyn RemoteLink>`.
pub trait RemoteLink: Send + Sync {
    /// Sends one control message.
    fn send_control<'a>(&'a self, message: ControlMessage) -> BoxFuture<'a, Result<(), LinkError>>;

    /// Receives one control message.
    ///
    /// A message whose type this build does not know is skipped, not fatal:
    /// the receive keeps waiting for the next known message (RAIL-RC-04).
    fn recv_control<'a>(&'a self) -> BoxFuture<'a, Result<ControlMessage, LinkError>>;

    /// Sends one audio payload.
    fn send_audio<'a>(&'a self, payload: AudioPayload) -> BoxFuture<'a, Result<(), LinkError>>;

    /// Receives one audio payload.
    fn recv_audio<'a>(&'a self) -> BoxFuture<'a, Result<AudioPayload, LinkError>>;

    /// Closes the link.
    ///
    /// Closing an already-closed link reports the reason it was closed
    /// first, rather than closing it again with a different reason.
    fn close<'a>(&'a self, reason: LinkCloseReason) -> BoxFuture<'a, Result<(), LinkError>>;

    /// The reason this link is closed, if it is.
    ///
    /// A link that closed itself reports `LinkCloseReason::Local`. A link
    /// whose peer closed reports `LinkCloseReason::Peer`.
    fn close_reason(&self) -> Option<LinkCloseReason>;
}

/// Sends bytes straight onto a link's channel, bypassing the encoder.
///
/// This exists so a conformance suite can put a message on the wire that the
/// encoder would never produce — an unknown type, an oversized frame, a
/// mis-tagged payload. It is a test seam, not a link operation.
///
/// It is deliberately *not* part of [`RemoteLink`]: a production transport
/// must not be able to put an uncapped, unauthenticated frame on the wire.
pub trait RawLink: RemoteLink {
    /// Sends raw bytes on the control channel, without encoding them.
    ///
    /// The raw path does not apply the send-side size cap. The cap is
    /// enforced where the spec requires it: on receipt, an oversized message
    /// terminates the link.
    fn send_control_bytes<'a>(&'a self, bytes: Vec<u8>) -> BoxFuture<'a, Result<(), LinkError>>;

    /// Sends raw bytes on the audio channel, without encoding them.
    fn send_audio_bytes<'a>(&'a self, bytes: Vec<u8>) -> BoxFuture<'a, Result<(), LinkError>>;
}
