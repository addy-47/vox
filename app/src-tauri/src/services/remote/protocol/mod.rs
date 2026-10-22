//! The remote-control wire contract.
//!
//! **Owns:** the wire message types, the binary framing, the capability
//! bitmask, and the wire error codes.
//!
//! **Must never contain:** I/O, mutable state, or Tauri types. This module is
//! pure — no async runtime, no framework, no filesystem — so the host role,
//! the controller role and the test suite all depend on it without a socket,
//! a device, or a model. The rule is enforced by
//! `tests/remote_module_tree_test.rs`.
//!
//! Workspace rule: `AGENTS.md` §7.2 rule 1.
//!
//! The four files inside this module split by responsibility:
//!
//! | File | Owns |
//! | :--- | :--- |
//! | `messages.rs` | the control and audio-control message types, and their JSON encoding |
//! | `framing.rs` | the binary `AudioFrame` layout, the audio payload tag, the stream formats, the size caps |
//! | `capabilities.rs` | the capability bitmask |
//! | `errors.rs` | the wire error vocabulary, and the classification of a violation as fatal or survivable |
//!
//! `messages.rs` and `framing.rs` refer to each other — the `Hello` message
//! carries a `StreamFormat`, and the audio payload codec carries a
//! JSON audio-control message. The cycle is intentional and confined to
//! these two files.

pub mod capabilities;
pub mod errors;
pub mod framing;
pub mod messages;

pub use capabilities::{Capabilities, CAP_CONTROL_MESSAGES, KNOWN_MASK};
pub use errors::{classify_protocol_error, ProtocolError, ProtocolResponse, RemoteErrorCode};
pub use framing::{
    check_control_message_size, check_stream_format, AudioFrame, AudioPayload, StreamFormat,
    AUDIO_CODEC_PCM16, AUDIO_FRAME_HEADER_BYTES, AUDIO_PAYLOAD_TAG_BINARY, AUDIO_PAYLOAD_TAG_JSON,
    PCM16_SAMPLE_BYTES,
};
pub use messages::{
    decode_audio_control, decode_control, encode_audio_control, encode_control, AttachSnapshot,
    AudioControlMessage, AudioControlType, ControlMessage, ControlType, DecodeOutcome, ErrResponse,
    Event, Flush, LinkLoss, LinkLossReason, LinkRefused, LinkRefusedReason, PairDenied,
    PairRequest, Ping, Pong, PttAction, PttMarker, Req, Res, RoleNotPermitted, SenderIdentity,
    UnknownPeer, VersionMismatch,
};

pub use super::{
    MAX_CONTROL_MESSAGE_BYTES, MAX_PREAUTH_MESSAGE_BYTES, NOISE_MAX_MESSAGE_BYTES,
    NOISE_MAX_PAYLOAD_BYTES, NOISE_TAG_BYTES, PROTOCOL_VERSION,
};
