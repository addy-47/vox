use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{
    capabilities::Capabilities,
    errors::{ProtocolError, RemoteErrorCode},
    framing::StreamFormat,
};

/// The JSON key that carries the message type tag.
///
/// Remote-control spec §7.3: every control message is a JSON object whose
/// `"type"` field names the message type exactly as the spec spells it.
pub const TYPE_FIELD: &str = "type";

/// The outcome of decoding one wire message.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeOutcome<T> {
    /// The message was recognised and decoded.
    Known(T),
    /// The message is well-formed, but its `"type"` tag names a message this
    /// build does not know.
    ///
    /// Remote-control spec §7.1 (RAIL-RC-04): the receiver MUST log a
    /// warning and skip the message, not close the connection. This is what
    /// lets a newer peer talk to an older one.
    UnknownType {
        /// The unrecognised type tag.
        type_name: String,
    },
}

/// The identity a peer announces in its `Hello`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SenderIdentity {
    /// The peer's device id, lowercase hex.
    pub device_id: String,
    /// The peer's human-readable name.
    pub display_name: String,
}

/// The first message each side sends after the Noise handshake.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    /// The protocol version the sender speaks.
    pub version: u16,
    /// Who the sender is.
    pub sender_identity: SenderIdentity,
    /// The capabilities the sender offers.
    pub capabilities: Capabilities,
    /// The audio format the sender will transmit.
    pub uplink_format: StreamFormat,
    /// The audio format the sender expects to receive.
    pub downlink_format: StreamFormat,
}

/// An IPC command request from a controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Req {
    /// The request id the response must echo.
    pub request_id: u64,
    /// The command name, resolved by the routing table.
    pub command: String,
    /// The command arguments.
    #[serde(default)]
    pub args: Value,
}

/// An IPC command response from the host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Res {
    /// The request id this response answers.
    pub request_id: u64,
    /// Whether the command succeeded.
    pub ok: bool,
    /// The command result.
    #[serde(default)]
    pub payload: Value,
}

/// An IPC command failure from the host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrResponse {
    /// The request id this failure answers.
    pub request_id: u64,
    /// The IPC error category.
    pub code: RemoteErrorCode,
    /// The human-readable detail.
    pub message: String,
}

/// A pipeline event forwarded to a controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// The event name from the event registry.
    pub event_name: String,
    /// The event payload.
    #[serde(default)]
    pub payload: Value,
}

/// The state snapshot the host sends immediately after `Hello`.
///
/// Remote-control spec §8.3: the snapshot is also the resynchronisation
/// payload, so it is re-sent after a reconnect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachSnapshot {
    /// The current pipeline state.
    pub state: Value,
    /// The active session id, or null when no session is active.
    ///
    /// The id is a number, not a string: the sessions table stores it as an
    /// `INTEGER PRIMARY KEY` and the IPC contract carries it as `sessionId:
    /// number`. The wire field name is snake_case, as the spec's own wire
    /// examples show.
    pub session_id: Option<i64>,
    /// The host settings, with every secret field redacted.
    pub settings: Value,
    /// The recent turns of the active session.
    #[serde(default)]
    pub turns: Vec<Value>,
}

/// The liveness probe the host sends on the control channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ping {
    /// The probe sequence number.
    pub seq: u64,
}

/// The liveness response the controller sends on the control channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    /// The sequence number of the ping being answered.
    pub seq: u64,
}

/// Why the host closed the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkLossReason {
    /// The host paused the session.
    HostPaused,
    /// The host went idle.
    HostIdle,
    /// The host disconnected the controller on purpose.
    HostDisconnected,
}

/// A host-initiated close, sent before the link is closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkLoss {
    /// Why the host closed the link.
    pub reason: LinkLossReason,
}

/// Why the host refused the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkRefusedReason {
    /// `remote.accepts_controllers` is false.
    NotAccepting,
    /// A local session is active on the host.
    HostBusy,
    /// Another controller is already attached.
    ControllerAttached,
}

/// A host refusal, sent before the link is closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRefused {
    /// Why the host refused the connection.
    pub reason: LinkRefusedReason,
}

/// The peer's protocol version is not one this build speaks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionMismatch {
    /// The version the peer announced.
    pub version: u16,
    /// The version this build speaks.
    pub supported_version: u16,
}

/// The peer is not in `paired_devices`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownPeer {
    /// The device id that was not recognised.
    pub device_id: String,
}

/// The peer's stored role contradicts the direction of the connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleNotPermitted {
    /// The role the peer claimed.
    pub role: String,
}

/// A pairing request, sent before the Noise handshake completes.
///
/// The identity in this message is a **claim**, not a credential: it travels
/// in the clear, before any key is established, so anybody on the network
/// can assert any device id and any public key. The host verifies it against
/// the completed handshake — the peer's static key, the claimed public key
/// and the claimed device id must all be the same object — and refuses the
/// pairing when they are not (remote-control spec §5.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairRequest {
    /// The single-use pairing token.
    pub token: String,
    /// The device id of the pairing controller.
    pub device_id: String,
    /// The 32-byte X25519 static public key of the pairing controller, as
    /// lowercase hex.
    pub public_key: String,
    /// The display name of the pairing controller.
    pub display_name: String,
}

/// A pairing refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairDenied {
    /// Why the pairing was refused.
    pub reason: String,
}

/// A PTT button event, carried on the audio socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PttMarker {
    /// What the button did.
    pub action: PttAction,
    /// The sample position the marker sits at.
    pub sample_offset: u64,
    /// The epoch the marker belongs to.
    pub epoch: u64,
}

/// What a PTT button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PttAction {
    /// The button went down.
    Start,
    /// The button came up.
    Stop,
    /// The press was abandoned.
    Cancel,
}

/// A request to discard buffered audio, carried on the audio socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flush {
    /// The new epoch. Frames with a lower epoch are stale.
    pub epoch: u64,
}

/// The kind of a control message, independent of its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlType {
    /// The handshake message.
    Hello,
    /// An IPC command request.
    Req,
    /// An IPC command response.
    Res,
    /// An IPC command failure.
    Err,
    /// A forwarded pipeline event.
    Event,
    /// The state snapshot.
    AttachSnapshot,
    /// An intentional disconnect.
    Disconnect,
    /// A liveness probe.
    Ping,
    /// A liveness response.
    Pong,
    /// A host-initiated close.
    LinkLoss,
    /// A protocol version mismatch.
    VersionMismatch,
    /// An unpaired peer.
    UnknownPeer,
    /// A role contradiction.
    RoleNotPermitted,
    /// A host refusal.
    LinkRefused,
    /// A pairing request.
    PairRequest,
    /// A pairing approval.
    PairApproved,
    /// A pairing refusal.
    PairDenied,
}

impl ControlType {
    /// Every control message kind.
    pub const ALL: [ControlType; 17] = [
        ControlType::Hello,
        ControlType::Req,
        ControlType::Res,
        ControlType::Err,
        ControlType::Event,
        ControlType::AttachSnapshot,
        ControlType::Disconnect,
        ControlType::Ping,
        ControlType::Pong,
        ControlType::LinkLoss,
        ControlType::VersionMismatch,
        ControlType::UnknownPeer,
        ControlType::RoleNotPermitted,
        ControlType::LinkRefused,
        ControlType::PairRequest,
        ControlType::PairApproved,
        ControlType::PairDenied,
    ];

    /// The exact spelling of the `"type"` tag on the wire.
    pub const fn wire_name(self) -> &'static str {
        match self {
            ControlType::Hello => "Hello",
            ControlType::Req => "Req",
            ControlType::Res => "Res",
            ControlType::Err => "Err",
            ControlType::Event => "Event",
            ControlType::AttachSnapshot => "AttachSnapshot",
            ControlType::Disconnect => "Disconnect",
            ControlType::Ping => "Ping",
            ControlType::Pong => "Pong",
            ControlType::LinkLoss => "LinkLoss",
            ControlType::VersionMismatch => "VersionMismatch",
            ControlType::UnknownPeer => "UnknownPeer",
            ControlType::RoleNotPermitted => "RoleNotPermitted",
            ControlType::LinkRefused => "LinkRefused",
            ControlType::PairRequest => "PairRequest",
            ControlType::PairApproved => "PairApproved",
            ControlType::PairDenied => "PairDenied",
        }
    }

    /// Resolves a `"type"` tag back to its kind.
    pub fn from_wire_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.wire_name() == name)
    }
}

/// A control message. Remote-control spec §7.3.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlMessage {
    /// The handshake message.
    Hello(Hello),
    /// An IPC command request.
    Req(Req),
    /// An IPC command response.
    Res(Res),
    /// An IPC command failure.
    Err(ErrResponse),
    /// A forwarded pipeline event.
    Event(Event),
    /// The state snapshot.
    AttachSnapshot(AttachSnapshot),
    /// An intentional disconnect.
    Disconnect,
    /// A liveness probe.
    Ping(Ping),
    /// A liveness response.
    Pong(Pong),
    /// A host-initiated close.
    LinkLoss(LinkLoss),
    /// A protocol version mismatch.
    VersionMismatch(VersionMismatch),
    /// An unpaired peer.
    UnknownPeer(UnknownPeer),
    /// A role contradiction.
    RoleNotPermitted(RoleNotPermitted),
    /// A host refusal.
    LinkRefused(LinkRefused),
    /// A pairing request.
    PairRequest(PairRequest),
    /// A pairing approval.
    PairApproved,
    /// A pairing refusal.
    PairDenied(PairDenied),
}

impl ControlMessage {
    /// The kind of this message.
    pub fn message_type(&self) -> ControlType {
        match self {
            ControlMessage::Hello(_) => ControlType::Hello,
            ControlMessage::Req(_) => ControlType::Req,
            ControlMessage::Res(_) => ControlType::Res,
            ControlMessage::Err(_) => ControlType::Err,
            ControlMessage::Event(_) => ControlType::Event,
            ControlMessage::AttachSnapshot(_) => ControlType::AttachSnapshot,
            ControlMessage::Disconnect => ControlType::Disconnect,
            ControlMessage::Ping(_) => ControlType::Ping,
            ControlMessage::Pong(_) => ControlType::Pong,
            ControlMessage::LinkLoss(_) => ControlType::LinkLoss,
            ControlMessage::VersionMismatch(_) => ControlType::VersionMismatch,
            ControlMessage::UnknownPeer(_) => ControlType::UnknownPeer,
            ControlMessage::RoleNotPermitted(_) => ControlType::RoleNotPermitted,
            ControlMessage::LinkRefused(_) => ControlType::LinkRefused,
            ControlMessage::PairRequest(_) => ControlType::PairRequest,
            ControlMessage::PairApproved => ControlType::PairApproved,
            ControlMessage::PairDenied(_) => ControlType::PairDenied,
        }
    }
}

/// The kind of an audio-control message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioControlType {
    /// A PTT button event.
    PttMarker,
    /// A request to discard buffered audio.
    Flush,
}

impl AudioControlType {
    /// Every audio-control message kind.
    pub const ALL: [AudioControlType; 2] = [AudioControlType::PttMarker, AudioControlType::Flush];

    /// The exact spelling of the `"type"` tag on the wire.
    pub const fn wire_name(self) -> &'static str {
        match self {
            AudioControlType::PttMarker => "PttMarker",
            AudioControlType::Flush => "Flush",
        }
    }

    /// Resolves a `"type"` tag back to its kind.
    pub fn from_wire_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.wire_name() == name)
    }
}

/// A JSON message carried on the audio socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioControlMessage {
    /// A PTT button event.
    PttMarker(PttMarker),
    /// A request to discard buffered audio.
    Flush(Flush),
}

impl AudioControlMessage {
    /// The kind of this message.
    pub fn message_type(&self) -> AudioControlType {
        match self {
            AudioControlMessage::PttMarker(_) => AudioControlType::PttMarker,
            AudioControlMessage::Flush(_) => AudioControlType::Flush,
        }
    }
}

/// Encodes a control message into its JSON wire form.
pub fn encode_control(message: &ControlMessage) -> Result<Vec<u8>, ProtocolError> {
    let type_name = message.message_type().wire_name();
    let payload = match message {
        ControlMessage::Hello(payload) => payload_map(payload)?,
        ControlMessage::Req(payload) => payload_map(payload)?,
        ControlMessage::Res(payload) => payload_map(payload)?,
        ControlMessage::Err(payload) => payload_map(payload)?,
        ControlMessage::Event(payload) => payload_map(payload)?,
        ControlMessage::AttachSnapshot(payload) => payload_map(payload)?,
        ControlMessage::Disconnect => Map::new(),
        ControlMessage::Ping(payload) => payload_map(payload)?,
        ControlMessage::Pong(payload) => payload_map(payload)?,
        ControlMessage::LinkLoss(payload) => payload_map(payload)?,
        ControlMessage::VersionMismatch(payload) => payload_map(payload)?,
        ControlMessage::UnknownPeer(payload) => payload_map(payload)?,
        ControlMessage::RoleNotPermitted(payload) => payload_map(payload)?,
        ControlMessage::LinkRefused(payload) => payload_map(payload)?,
        ControlMessage::PairRequest(payload) => payload_map(payload)?,
        ControlMessage::PairApproved => Map::new(),
        ControlMessage::PairDenied(payload) => payload_map(payload)?,
    };
    encode_tagged(type_name, payload)
}

/// Decodes a control message from its JSON wire form.
///
/// An unrecognised `"type"` tag is a skip outcome, never an error: the
/// caller logs a warning and keeps the connection open (RAIL-RC-04).
pub fn decode_control(bytes: &[u8]) -> Result<DecodeOutcome<ControlMessage>, ProtocolError> {
    decode_tagged(bytes, ControlType::from_wire_name, build_control)
}

/// Encodes an audio-control message into its JSON wire form.
pub fn encode_audio_control(message: &AudioControlMessage) -> Result<Vec<u8>, ProtocolError> {
    let type_name = message.message_type().wire_name();
    let payload = match message {
        AudioControlMessage::PttMarker(payload) => payload_map(payload)?,
        AudioControlMessage::Flush(payload) => payload_map(payload)?,
    };
    encode_tagged(type_name, payload)
}

/// Decodes an audio-control message from its JSON wire form.
pub fn decode_audio_control(
    bytes: &[u8],
) -> Result<DecodeOutcome<AudioControlMessage>, ProtocolError> {
    decode_tagged(bytes, AudioControlType::from_wire_name, build_audio_control)
}

/// Rebuilds a control message from its decoded payload.
fn build_control(
    kind: ControlType,
    payload: Map<String, Value>,
) -> Result<ControlMessage, ProtocolError> {
    Ok(match kind {
        ControlType::Hello => ControlMessage::Hello(payload_value(payload)?),
        ControlType::Req => ControlMessage::Req(payload_value(payload)?),
        ControlType::Res => ControlMessage::Res(payload_value(payload)?),
        ControlType::Err => ControlMessage::Err(payload_value(payload)?),
        ControlType::Event => ControlMessage::Event(payload_value(payload)?),
        ControlType::AttachSnapshot => ControlMessage::AttachSnapshot(payload_value(payload)?),
        ControlType::Disconnect => ControlMessage::Disconnect,
        ControlType::Ping => ControlMessage::Ping(payload_value(payload)?),
        ControlType::Pong => ControlMessage::Pong(payload_value(payload)?),
        ControlType::LinkLoss => ControlMessage::LinkLoss(payload_value(payload)?),
        ControlType::VersionMismatch => ControlMessage::VersionMismatch(payload_value(payload)?),
        ControlType::UnknownPeer => ControlMessage::UnknownPeer(payload_value(payload)?),
        ControlType::RoleNotPermitted => ControlMessage::RoleNotPermitted(payload_value(payload)?),
        ControlType::LinkRefused => ControlMessage::LinkRefused(payload_value(payload)?),
        ControlType::PairRequest => ControlMessage::PairRequest(payload_value(payload)?),
        ControlType::PairApproved => ControlMessage::PairApproved,
        ControlType::PairDenied => ControlMessage::PairDenied(payload_value(payload)?),
    })
}

/// Rebuilds an audio-control message from its decoded payload.
fn build_audio_control(
    kind: AudioControlType,
    payload: Map<String, Value>,
) -> Result<AudioControlMessage, ProtocolError> {
    Ok(match kind {
        AudioControlType::PttMarker => AudioControlMessage::PttMarker(payload_value(payload)?),
        AudioControlType::Flush => AudioControlMessage::Flush(payload_value(payload)?),
    })
}

/// Encodes a payload object with its `"type"` tag attached.
fn encode_tagged(
    type_name: &str,
    mut payload: Map<String, Value>,
) -> Result<Vec<u8>, ProtocolError> {
    payload.insert(
        String::from(TYPE_FIELD),
        Value::String(String::from(type_name)),
    );
    serde_json::to_vec(&Value::Object(payload))
        .map_err(|error| ProtocolError::MalformedJson(error.to_string()))
}

/// Decodes a tagged JSON object, resolving the `"type"` tag to a kind.
fn decode_tagged<T, K: Copy>(
    bytes: &[u8],
    lookup: fn(&str) -> Option<K>,
    build: fn(K, Map<String, Value>) -> Result<T, ProtocolError>,
) -> Result<DecodeOutcome<T>, ProtocolError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| ProtocolError::MalformedJson(error.to_string()))?;
    let Value::Object(mut object) = value else {
        return Err(ProtocolError::MalformedJson(
            "the message must be a JSON object".to_string(),
        ));
    };
    let Some(type_name) = object.remove(TYPE_FIELD).and_then(|tag| match tag {
        Value::String(name) => Some(name),
        _ => None,
    }) else {
        return Err(ProtocolError::MissingTypeTag);
    };
    let Some(kind) = lookup(&type_name) else {
        return Ok(DecodeOutcome::UnknownType { type_name });
    };
    Ok(DecodeOutcome::Known(build(kind, object)?))
}

/// Serialises a message payload into a JSON object.
fn payload_map<T: Serialize>(payload: &T) -> Result<Map<String, Value>, ProtocolError> {
    match serde_json::to_value(payload) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(ProtocolError::MalformedJson(
            "the message payload must be a JSON object".to_string(),
        )),
        Err(error) => Err(ProtocolError::MalformedJson(error.to_string())),
    }
}

/// Deserialises a message payload from a JSON object.
fn payload_value<T: serde::de::DeserializeOwned>(
    payload: Map<String, Value>,
) -> Result<T, ProtocolError> {
    serde_json::from_value(Value::Object(payload))
        .map_err(|error| ProtocolError::MalformedJson(error.to_string()))
}
