//! ============================================================================
//! tests/common/remote_samples.rs — Canonical Samples of Every Protocol Message
//! ============================================================================
//!
//! WHY THIS EXISTS
//! ---------------
//! Two test files need the same thing: one sample of every control message
//! and every audio-control message, each paired with the exact `"type"` tag
//! the remote-control spec spells and the field names the spec documents.
//!
//! The protocol test asserts the wire shape against those tags. The
//! conformance suite asserts that every message type survives a trip through
//! a link. If the two files carried their own copies of the samples, a
//! message added to one and not the other would pass silently — so the
//! samples live here, once.
//!
//! The list is the drift check as well: the protocol test asserts that the
//! sample count equals `ControlType::ALL.len()`, so a message type added to
//! the protocol without a sample fails the suite.
//! ============================================================================

use serde_json::json;
use vox_lib::services::remote::protocol::{
    messages::{
        AttachSnapshot, AudioControlMessage, ControlMessage, Event, Flush, Hello, LinkLoss,
        LinkLossReason, LinkRefused, LinkRefusedReason, PairDenied, PairRequest, Ping, Pong,
        PttAction, PttMarker, Req, Res, RoleNotPermitted, SenderIdentity, UnknownPeer,
        VersionMismatch,
    },
    ErrResponse, RemoteErrorCode, StreamFormat, CAP_CONTROL_MESSAGES,
};

/// One control message sample.
pub struct ControlSample {
    /// The exact `"type"` tag the remote-control spec spells.
    pub type_tag: &'static str,
    /// The field names the spec documents for this message.
    pub fields: &'static [&'static str],
    /// The message itself.
    pub message: ControlMessage,
}

/// One audio-control message sample.
pub struct AudioControlSample {
    /// The exact `"type"` tag the remote-control spec spells.
    pub type_tag: &'static str,
    /// The field names the spec documents for this message.
    pub fields: &'static [&'static str],
    /// The message itself.
    pub message: AudioControlMessage,
}

/// One sample of every control message type, in spec order.
pub fn control_samples() -> Vec<ControlSample> {
    vec![
        ControlSample {
            type_tag: "Hello",
            fields: &[
                "version",
                "sender_identity",
                "capabilities",
                "uplink_format",
                "downlink_format",
            ],
            message: ControlMessage::Hello(Hello {
                version: vox_lib::services::remote::PROTOCOL_VERSION,
                sender_identity: SenderIdentity {
                    device_id: "0a1b2c3d4e5f60718293a4b5c6d7e8f9".to_string(),
                    display_name: "Studio Laptop".to_string(),
                },
                capabilities: CAP_CONTROL_MESSAGES,
                uplink_format: StreamFormat::UPLINK,
                downlink_format: StreamFormat::DOWNLINK,
            }),
        },
        ControlSample {
            type_tag: "Req",
            fields: &["request_id", "command", "args"],
            message: ControlMessage::Req(Req {
                request_id: 7,
                command: "start_session".to_string(),
                args: json!({"source": "remote"}),
            }),
        },
        ControlSample {
            type_tag: "Res",
            fields: &["request_id", "ok", "payload"],
            message: ControlMessage::Res(Res {
                request_id: 7,
                ok: true,
                payload: json!({"session_id": "abc-123"}),
            }),
        },
        ControlSample {
            type_tag: "Err",
            fields: &["request_id", "code", "message"],
            message: ControlMessage::Err(ErrResponse {
                request_id: 8,
                code: RemoteErrorCode::InvalidArgument,
                message: "the command was malformed".to_string(),
            }),
        },
        ControlSample {
            type_tag: "Event",
            fields: &["event_name", "payload"],
            message: ControlMessage::Event(Event {
                event_name: "state_changed".to_string(),
                payload: json!({"state": "Listening"}),
            }),
        },
        ControlSample {
            type_tag: "AttachSnapshot",
            fields: &["state", "session_id", "settings", "turns"],
            message: ControlMessage::AttachSnapshot(AttachSnapshot {
                state: json!({"state": "Ready"}),
                session_id: Some(42),
                settings: json!({"theme": "dark"}),
                turns: vec![json!({"role": "user", "text": "hello"})],
            }),
        },
        ControlSample {
            type_tag: "Disconnect",
            fields: &[],
            message: ControlMessage::Disconnect,
        },
        ControlSample {
            type_tag: "Ping",
            fields: &["seq"],
            message: ControlMessage::Ping(Ping { seq: 11 }),
        },
        ControlSample {
            type_tag: "Pong",
            fields: &["seq"],
            message: ControlMessage::Pong(Pong { seq: 11 }),
        },
        ControlSample {
            type_tag: "LinkLoss",
            fields: &["reason"],
            message: ControlMessage::LinkLoss(LinkLoss {
                reason: LinkLossReason::HostPaused,
            }),
        },
        ControlSample {
            type_tag: "VersionMismatch",
            fields: &["version", "supported_version"],
            message: ControlMessage::VersionMismatch(VersionMismatch {
                version: 99,
                supported_version: vox_lib::services::remote::PROTOCOL_VERSION,
            }),
        },
        ControlSample {
            type_tag: "UnknownPeer",
            fields: &["device_id"],
            message: ControlMessage::UnknownPeer(UnknownPeer {
                device_id: "ffffffffffffffffffffffffffffffff".to_string(),
            }),
        },
        ControlSample {
            type_tag: "RoleNotPermitted",
            fields: &["role"],
            message: ControlMessage::RoleNotPermitted(RoleNotPermitted {
                role: "host".to_string(),
            }),
        },
        ControlSample {
            type_tag: "LinkRefused",
            fields: &["reason"],
            message: ControlMessage::LinkRefused(LinkRefused {
                reason: LinkRefusedReason::HostBusy,
            }),
        },
        ControlSample {
            type_tag: "PairRequest",
            fields: &["token", "device_id", "public_key", "display_name"],
            message: ControlMessage::PairRequest(PairRequest {
                token: "test_sample_token".to_string(),
                device_id: "11111111111111111111111111111111".to_string(),
                public_key: "2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
                display_name: "Phone".to_string(),
            }),
        },
        ControlSample {
            type_tag: "PairApproved",
            fields: &[],
            message: ControlMessage::PairApproved,
        },
        ControlSample {
            type_tag: "PairDenied",
            fields: &["reason"],
            message: ControlMessage::PairDenied(PairDenied {
                reason: "the user rejected the pairing".to_string(),
            }),
        },
    ]
}

/// One sample of every audio-control message type, in spec order.
pub fn audio_control_samples() -> Vec<AudioControlSample> {
    vec![
        AudioControlSample {
            type_tag: "PttMarker",
            fields: &["action", "sample_offset", "epoch"],
            message: AudioControlMessage::PttMarker(PttMarker {
                action: PttAction::Start,
                sample_offset: 9600,
                epoch: 3,
            }),
        },
        AudioControlSample {
            type_tag: "Flush",
            fields: &["epoch"],
            message: AudioControlMessage::Flush(Flush { epoch: 4 }),
        },
    ]
}

/// A liveness probe carrying the given sequence number.
pub fn sample_ping(seq: u64) -> ControlMessage {
    ControlMessage::Ping(Ping { seq })
}
