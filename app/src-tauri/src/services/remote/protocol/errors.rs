use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::framing::StreamFormat;

/// The stable wire spelling of an IPC error category.
///
/// Remote-control spec §7.3: the `Err` message carries an IPC error
/// *category*. The category travels as a lowercase string rather than an
/// integer, so a future reorder of the Rust enum cannot break the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RemoteErrorCode {
    /// An unexpected internal failure.
    Internal,
    /// The request was malformed or a parameter was out of range.
    InvalidArgument,
    /// The host is in a state that forbids the command.
    InvalidState,
    /// The named resource does not exist.
    NotFound,
    /// A persistence failure.
    Database,
    /// A model or audio engine failure.
    Engine,
    /// A transport or remote failure.
    Network,
    /// A version or state conflict.
    Conflict,
    /// A pipeline failure.
    Pipeline,
    /// The command is not permitted from a controller.
    Denied,
    /// The host refused the command because a local session is active.
    HostBusy,
    /// The command name is not in the routing table.
    UnknownCommand,
}

impl RemoteErrorCode {
    /// Every wire error code, in the order the routing table lists them.
    pub const ALL: [RemoteErrorCode; 12] = [
        RemoteErrorCode::Internal,
        RemoteErrorCode::InvalidArgument,
        RemoteErrorCode::InvalidState,
        RemoteErrorCode::NotFound,
        RemoteErrorCode::Database,
        RemoteErrorCode::Engine,
        RemoteErrorCode::Network,
        RemoteErrorCode::Conflict,
        RemoteErrorCode::Pipeline,
        RemoteErrorCode::Denied,
        RemoteErrorCode::HostBusy,
        RemoteErrorCode::UnknownCommand,
    ];

    /// The stable lowercase spelling that travels on the wire.
    pub const fn wire_name(self) -> &'static str {
        match self {
            RemoteErrorCode::Internal => "internal",
            RemoteErrorCode::InvalidArgument => "invalid_argument",
            RemoteErrorCode::InvalidState => "invalid_state",
            RemoteErrorCode::NotFound => "not_found",
            RemoteErrorCode::Database => "database",
            RemoteErrorCode::Engine => "engine",
            RemoteErrorCode::Network => "network",
            RemoteErrorCode::Conflict => "conflict",
            RemoteErrorCode::Pipeline => "pipeline",
            RemoteErrorCode::Denied => "denied",
            RemoteErrorCode::HostBusy => "host_busy",
            RemoteErrorCode::UnknownCommand => "unknown_command",
        }
    }

    /// Resolves a wire spelling back to its code.
    pub fn from_wire_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|code| code.wire_name() == name)
    }
}

impl Serialize for RemoteErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}

impl<'de> Deserialize<'de> for RemoteErrorCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::from_wire_name(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown wire error code `{name}`")))
    }
}

/// A failure to decode a wire message.
///
/// Every variant is a defect in what arrived on the wire, never a statement
/// about the link. The link layer turns one of these into
/// `LinkError::Protocol`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// The payload was not valid JSON, or did not match the schema of the
    /// message type it claimed.
    #[error("malformed message: {0}")]
    MalformedJson(String),

    /// The payload carried no usable `"type"` tag.
    #[error("the message carries no `type` tag")]
    MissingTypeTag,

    /// The payload carried no payload tag.
    #[error("the audio payload carries no payload tag")]
    MissingPayloadTag,

    /// A binary frame was shorter than its header, or its sample body was
    /// not a whole number of PCM16 samples.
    #[error("truncated frame: expected {expected} bytes, got {actual}")]
    TruncatedFrame {
        /// The number of bytes the frame needed.
        expected: usize,
        /// The number of bytes that arrived.
        actual: usize,
    },

    /// The audio frame named a codec this build cannot interpret.
    #[error("unknown audio codec: 0x{0:02x}")]
    UnknownCodec(u8),

    /// The audio payload carried a tag this build does not know.
    #[error("unknown audio payload tag: 0x{0:02x}")]
    UnknownPayloadTag(u8),

    /// A message exceeded the size cap that applies at its point in the
    /// link lifecycle.
    #[error("message of {size} bytes exceeds the {limit} byte cap")]
    Oversized {
        /// The size of the offending message.
        size: usize,
        /// The cap that applies.
        limit: usize,
    },

    /// A stream format did not match the one the receiver supports.
    #[error("stream format mismatch: expected {expected:?}, got {actual:?}")]
    FormatMismatch {
        /// The format the receiver supports.
        expected: StreamFormat,
        /// The format that arrived.
        actual: StreamFormat,
    },
}

/// What a receiver must do about a protocol violation.
///
/// The protocol module owns this decision. A transport consults the
/// classification; it does not enumerate protocol errors to decide which
/// ones it dies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolResponse {
    /// The violation ends the link.
    Fatal,
    /// The violation is survivable: the link continues.
    Skippable,
}

/// Decides whether a protocol violation ends the link.
///
/// The function is exhaustive over [`ProtocolError`]: a new variant cannot
/// be added without a decision here, in the protocol layer, rather than in
/// every transport that has to live with it.
///
/// The rules, and where they come from:
///
/// - Remote-control spec §7.1: a message over the size cap MUST cause
///   immediate connection termination.
/// - Remote-control spec §7.4: an unknown audio payload tag is a protocol
///   error and closes the audio socket, because a mis-tagged frame cannot be
///   safely interpreted as either shape.
/// - Remote-control spec §7.4: a frame this build cannot interpret — an
///   unknown codec, a truncated frame, a payload with no tag — cannot be
///   recovered from.
/// - Remote-control spec §7.1 (RAIL-RC-04): a message this build does not
///   understand is skipped, not fatal. A message whose type tag is missing
///   is such a message: the receiver cannot tell what it is, so the link
///   survives and the violation is reported to the caller.
/// - A malformed message is a peer that cannot be understood at all.
pub fn classify_protocol_error(error: &ProtocolError) -> ProtocolResponse {
    match error {
        ProtocolError::MalformedJson(_) => ProtocolResponse::Fatal,
        ProtocolError::MissingTypeTag => ProtocolResponse::Skippable,
        ProtocolError::MissingPayloadTag => ProtocolResponse::Fatal,
        ProtocolError::TruncatedFrame { .. } => ProtocolResponse::Fatal,
        ProtocolError::UnknownCodec(_) => ProtocolResponse::Fatal,
        ProtocolError::UnknownPayloadTag(_) => ProtocolResponse::Fatal,
        ProtocolError::Oversized { .. } => ProtocolResponse::Fatal,
        ProtocolError::FormatMismatch { .. } => ProtocolResponse::Fatal,
    }
}
