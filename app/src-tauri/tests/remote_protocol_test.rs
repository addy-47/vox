//! ============================================================================
//! tests/remote_protocol_test.rs — Remote Control Wire Protocol Contract
//! ============================================================================
//! Category     : Integration Test (Seam 01 — pure protocol module)
//! Component    : src/services/remote/protocol/{messages,framing,capabilities,errors}.rs
//! Prerequisites: None (pure in-memory encoding; no models, no services, no DB)
//! Execution    : cargo nextest run --test remote_protocol_test --release --nocapture --test-threads=1
//! Metrics      : (a) every control and audio-control message round-trips with
//!                the exact "type" tag and the documented field names, and the
//!                sample list covers every declared message type;
//!                (b) the snapshot's session id is a JSON number;
//!                (c) an unknown type, an unknown field and a missing type
//!                tag are all skip outcomes, never errors;
//!                (d) the binary audio layout matches the spec byte for byte,
//!                header big-endian and samples little-endian;
//!                (e) the audio payload tag discriminates the two audio shapes;
//!                (f) the size caps, the stream formats, the capability mask
//!                and the wire error vocabulary behave as specified.
//!
//! WHY THIS EXISTS
//! ---------------
//! The wire contract is the one thing a remote-control implementation cannot
//! change later without breaking every paired device. A field renamed in Rust
//! but not in the spec is a silent wire break; a swapped byte order in the
//! audio header is a silent corruption that does not crash, it just plays
//! noise. Neither shows up in a compile error.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! These tests exercise the protocol module in memory. They prove the bytes
//! this build produces and the bytes it accepts. They cannot prove that a
//! peer running a different build agrees — that is what the conformance
//! suite and the second transport are for.
//! ============================================================================

mod common;

use std::collections::BTreeSet;

use common::remote_samples::{audio_control_samples, control_samples};
use vox_lib::services::remote::protocol::{
    errors::{classify_protocol_error, ProtocolError, ProtocolResponse},
    framing::{
        check_control_message_size, check_stream_format, decode_audio_frame, decode_audio_payload,
        encode_audio_frame, encode_audio_payload, AudioFrame, AudioPayload, StreamFormat,
        AUDIO_CODEC_PCM16, AUDIO_FRAME_HEADER_BYTES, AUDIO_PAYLOAD_TAG_BINARY,
        AUDIO_PAYLOAD_TAG_JSON, PCM16_SAMPLE_BYTES,
    },
    messages::{
        decode_audio_control, decode_control, encode_audio_control, encode_control, AttachSnapshot,
        ControlMessage, ControlType, DecodeOutcome,
    },
    RemoteErrorCode, CAP_CONTROL_MESSAGES, KNOWN_MASK, MAX_CONTROL_MESSAGE_BYTES,
    MAX_PREAUTH_MESSAGE_BYTES, NOISE_MAX_PAYLOAD_BYTES, PROTOCOL_VERSION,
};

// ============================================================================
// Test 1: Every message type round-trips with the documented wire shape
// ============================================================================

#[test]
fn test_every_control_message_round_trips_with_the_documented_type_tag() {
    let samples = control_samples();
    assert_eq!(
        samples.len(),
        ControlType::ALL.len(),
        "the sample list must cover every control message type; a message added \
         to the protocol without a sample would pass silently"
    );

    let declared: BTreeSet<&str> = ControlType::ALL
        .iter()
        .map(|kind| kind.wire_name())
        .collect();
    let sampled: BTreeSet<&str> = samples.iter().map(|sample| sample.type_tag).collect();
    assert_eq!(
        declared, sampled,
        "the samples and the declared control types have drifted apart"
    );

    for sample in &samples {
        let kind = ControlType::from_wire_name(sample.type_tag).unwrap_or_else(|| {
            panic!(
                "the type tag `{}` is not a declared control type",
                sample.type_tag
            )
        });
        assert_eq!(
            kind,
            sample.message.message_type(),
            "the sample tagged `{}` does not report that type",
            sample.type_tag
        );

        let bytes = encode_control(&sample.message).unwrap_or_else(|error| {
            panic!("the {} message failed to encode: {error}", sample.type_tag)
        });
        let text = String::from_utf8(bytes.clone()).unwrap_or_else(|error| {
            panic!("the {} message is not UTF-8: {error}", sample.type_tag)
        });

        assert!(
            text.contains(&format!("\"type\":\"{}\"", sample.type_tag)),
            "the {} message must carry the exact type tag the spec spells; the \
             encoded form was {text}",
            sample.type_tag
        );
        for field in sample.fields {
            assert!(
                text.contains(&format!("\"{field}\"")),
                "the {} message must carry the documented field `{field}`; the \
                 encoded form was {text}",
                sample.type_tag
            );
        }

        let outcome = decode_control(&bytes).unwrap_or_else(|error| {
            panic!("the {} message failed to decode: {error}", sample.type_tag)
        });
        assert_eq!(
            outcome,
            DecodeOutcome::Known(sample.message.clone()),
            "the {} message did not survive the round trip",
            sample.type_tag
        );
    }
}

#[test]
fn test_every_audio_control_message_round_trips_with_the_documented_type_tag() {
    for sample in audio_control_samples() {
        let bytes = encode_audio_control(&sample.message).unwrap_or_else(|error| {
            panic!("the {} message failed to encode: {error}", sample.type_tag)
        });
        let text = String::from_utf8(bytes.clone()).unwrap_or_else(|error| {
            panic!("the {} message is not UTF-8: {error}", sample.type_tag)
        });

        assert!(
            text.contains(&format!("\"type\":\"{}\"", sample.type_tag)),
            "the {} message must carry the exact type tag the spec spells; the \
             encoded form was {text}",
            sample.type_tag
        );
        for field in sample.fields {
            assert!(
                text.contains(&format!("\"{field}\"")),
                "the {} message must carry the documented field `{field}`; the \
                 encoded form was {text}",
                sample.type_tag
            );
        }

        let outcome = decode_audio_control(&bytes).unwrap_or_else(|error| {
            panic!("the {} message failed to decode: {error}", sample.type_tag)
        });
        assert_eq!(
            outcome,
            DecodeOutcome::Known(sample.message.clone()),
            "the {} message did not survive the round trip",
            sample.type_tag
        );
    }
}

// ============================================================================
// Test 2: The snapshot's session id is a number on the wire
// ============================================================================

#[test]
fn test_the_snapshot_session_id_is_a_number_on_the_wire() {
    // The spec pins the session id as a number: the sessions table stores it
    // as an `INTEGER PRIMARY KEY`, the IPC contract carries `sessionId:
    // number`, and the frontend types it `number | null`. A string would
    // break every one of them, and the round trip alone would not catch it.
    let snapshot = AttachSnapshot {
        state: serde_json::json!({"state": "Ready"}),
        session_id: Some(42),
        settings: serde_json::json!({"theme": "dark"}),
        turns: Vec::new(),
    };

    let bytes = encode_control(&ControlMessage::AttachSnapshot(snapshot.clone()))
        .expect("the snapshot must encode");
    let text = String::from_utf8(bytes).expect("the snapshot must be UTF-8");

    assert!(
        text.contains("\"session_id\":42"),
        "the session id must travel as a JSON number, the encoded form was {text}"
    );
    assert!(
        !text.contains("\"session_id\":\"42\""),
        "the session id must not travel as a string, the encoded form was {text}"
    );

    let outcome = decode_control(text.as_bytes()).expect("the snapshot must decode");
    assert_eq!(
        outcome,
        DecodeOutcome::Known(ControlMessage::AttachSnapshot(snapshot)),
        "the snapshot must survive the round trip"
    );

    // No active session: the field is null, not an empty string and not zero.
    let without_session = AttachSnapshot {
        state: serde_json::json!({"state": "Ready"}),
        session_id: None,
        settings: serde_json::json!({}),
        turns: Vec::new(),
    };
    let bytes = encode_control(&ControlMessage::AttachSnapshot(without_session))
        .expect("the snapshot must encode");
    let text = String::from_utf8(bytes).expect("the snapshot must be UTF-8");
    assert!(
        text.contains("\"session_id\":null"),
        "an inactive session must travel as null, the encoded form was {text}"
    );

    // A string session id is a contract violation and must not decode.
    let malformed =
        br#"{"type":"AttachSnapshot","state":{},"session_id":"42","settings":{},"turns":[]}"#;
    assert!(
        decode_control(malformed).is_err(),
        "a string session id must be rejected: the contract says a number"
    );
}

// ============================================================================
// Test 3: An unreadable message is a skip outcome, never an error
// ============================================================================

#[test]
fn test_an_unknown_or_unreadable_message_is_a_skip_outcome_not_an_error() {
    // An unknown type: a newer peer. The link must stay open.
    let outcome = decode_control(br#"{"type":"AMessageFromTheFuture","payload":{"nested":true}}"#)
        .expect("an unknown message type must not be an error; the connection must stay open");
    assert_eq!(
        outcome,
        DecodeOutcome::UnknownType {
            type_name: "AMessageFromTheFuture".to_string()
        },
        "an unknown message type must produce the skip outcome"
    );

    // An unknown audio type: the same rule on the audio socket.
    let outcome = decode_audio_control(br#"{"type":"AnAudioMessageFromTheFuture","epoch":3}"#)
        .expect("an unknown audio message type must not be an error");
    assert_eq!(
        outcome,
        DecodeOutcome::UnknownType {
            type_name: "AnAudioMessageFromTheFuture".to_string()
        },
        "an unknown audio message type must produce the skip outcome"
    );

    // An unknown field inside a known type: a newer peer added a field.
    let outcome =
        decode_control(br#"{"type":"Ping","seq":7,"a_field_from_the_future":{"nested":true}}"#)
            .expect("an unknown field inside a known type must not fail the decode");
    assert_eq!(
        outcome,
        DecodeOutcome::Known(ControlMessage::Ping(
            vox_lib::services::remote::protocol::messages::Ping { seq: 7 }
        )),
        "a newer peer may add fields; an older peer must ignore them"
    );

    // A message with no type tag at all: the receiver cannot tell what it is.
    // The protocol layer classifies that as survivable — the link must skip
    // the message and keep waiting, not tear the link down.
    let tagless = decode_control(b"{}").expect_err("a tagless message has no type to decode");
    assert!(
        matches!(tagless, ProtocolError::MissingTypeTag),
        "a tagless message must report the missing tag, it reported {tagless:?}"
    );
    assert_eq!(
        classify_protocol_error(&tagless),
        ProtocolResponse::Skippable,
        "a tagless message must be survivable: the link continues"
    );
}

// ============================================================================
// Test 4: The binary audio layout matches the spec byte for byte
// ============================================================================

#[test]
fn test_the_audio_frame_layout_matches_the_spec_byte_for_byte() {
    let frame = AudioFrame {
        codec: AUDIO_CODEC_PCM16,
        seq: 0x0102_0304,
        epoch: 0x0102_0304_0506_0708,
        sample_offset: 0x1112_1314_1516_1718,
        samples: vec![0x0102, -2, 32767],
    };

    let bytes = encode_audio_frame(&frame);

    assert_eq!(
        bytes.len(),
        AUDIO_FRAME_HEADER_BYTES + frame.samples.len() * PCM16_SAMPLE_BYTES,
        "the encoded frame must be the header plus the sample body"
    );
    assert_eq!(bytes[0], 0x00, "the codec byte must come first");
    assert_eq!(
        &bytes[1..5],
        &0x0102_0304u32.to_be_bytes(),
        "the sequence number must be big-endian"
    );
    assert_eq!(
        &bytes[5..13],
        &0x0102_0304_0506_0708u64.to_be_bytes(),
        "the epoch must be big-endian"
    );
    assert_eq!(
        &bytes[13..21],
        &0x1112_1314_1516_1718u64.to_be_bytes(),
        "the sample offset must be big-endian"
    );
    assert_eq!(
        &bytes[21..23],
        &0x0102i16.to_le_bytes(),
        "the samples must be little-endian"
    );
    assert_eq!(
        &bytes[23..25],
        &(-2i16).to_le_bytes(),
        "a negative sample must be little-endian"
    );

    assert_eq!(
        decode_audio_frame(&bytes).expect("the frame must decode"),
        frame,
        "the frame must survive the round trip"
    );

    // A body that is not a whole number of PCM16 samples is rejected.
    assert!(
        decode_audio_frame(&bytes[..bytes.len() - 1]).is_err(),
        "a body that is not a whole number of PCM16 samples must be rejected"
    );
    assert!(
        decode_audio_frame(&bytes[..AUDIO_FRAME_HEADER_BYTES - 1]).is_err(),
        "a frame shorter than the header must be rejected"
    );
    assert!(
        decode_audio_frame(&[]).is_err(),
        "an empty frame must be rejected"
    );

    // A codec this build cannot interpret is rejected by name.
    let mut unknown_codec = bytes.clone();
    unknown_codec[0] = 0x7f;
    assert!(
        matches!(
            decode_audio_frame(&unknown_codec),
            Err(ProtocolError::UnknownCodec(0x7f))
        ),
        "a codec this build cannot interpret must be rejected"
    );
}

// ============================================================================
// Test 5: The audio payload tag discriminates the two audio shapes
// ============================================================================

#[test]
fn test_the_audio_payload_tag_discriminates_the_two_audio_shapes() {
    let frame = AudioFrame {
        codec: AUDIO_CODEC_PCM16,
        seq: 11,
        epoch: 1,
        sample_offset: 320,
        samples: vec![10, -10],
    };
    let marker = audio_control_samples()
        .into_iter()
        .find(|sample| sample.type_tag == "PttMarker")
        .expect("the sample list must carry a PttMarker");

    let frame_bytes = encode_audio_payload(&AudioPayload::Frame(frame.clone()))
        .expect("the frame payload must encode");
    let marker_bytes = encode_audio_payload(&AudioPayload::Control(marker.message.clone()))
        .expect("the marker payload must encode");

    assert_eq!(
        frame_bytes[0], AUDIO_PAYLOAD_TAG_BINARY,
        "a binary frame must carry the binary payload tag"
    );
    assert_eq!(
        marker_bytes[0], AUDIO_PAYLOAD_TAG_JSON,
        "a JSON audio-control message must carry the JSON payload tag"
    );
    assert_ne!(
        AUDIO_PAYLOAD_TAG_BINARY, AUDIO_PAYLOAD_TAG_JSON,
        "the two payload tags must differ"
    );

    assert_eq!(
        decode_audio_payload(&frame_bytes).expect("the frame payload must decode"),
        DecodeOutcome::Known(AudioPayload::Frame(frame)),
        "the binary frame must decode as a frame"
    );
    assert_eq!(
        decode_audio_payload(&marker_bytes).expect("the marker payload must decode"),
        DecodeOutcome::Known(AudioPayload::Control(marker.message)),
        "the JSON marker must decode as a marker, not as a frame"
    );

    assert!(
        matches!(
            decode_audio_payload(&[0x02, 0x00]),
            Err(ProtocolError::UnknownPayloadTag(0x02))
        ),
        "an unknown payload tag must be a protocol error"
    );
    assert!(
        decode_audio_payload(&[]).is_err(),
        "a payload with no tag must be a protocol error"
    );
}

// ============================================================================
// Test 6: The wire vocabulary is stable
// ============================================================================

#[test]
fn test_the_wire_vocabulary_is_stable() {
    assert_eq!(PROTOCOL_VERSION, 1, "the v1 wire protocol version is 1");

    // The size caps. The post-authentication cap is the Noise payload cap:
    // a message the protocol accepts must fit in the transport, or the link
    // dies on a message the protocol said was legal.
    assert_eq!(MAX_PREAUTH_MESSAGE_BYTES, 4 * 1024);
    assert_eq!(MAX_CONTROL_MESSAGE_BYTES, NOISE_MAX_PAYLOAD_BYTES);
    assert_eq!(
        MAX_CONTROL_MESSAGE_BYTES, NOISE_MAX_PAYLOAD_BYTES,
        "the control cap must be the transport's cap, so a message the protocol accepts always \
         fits in the transport"
    );

    // The pre-authentication window — the one window in which the peer is
    // unauthenticated — is bounded in production by the Noise handshake
    // message cap, which the security suite exercises against the real
    // driver. The application-message cap itself is enforced by the transport
    // when it lands (wave W2b); this assertion is about the constant the
    // spec names, not about an enforcement that does not exist yet.
    assert_eq!(MAX_PREAUTH_MESSAGE_BYTES, 4 * 1024);

    assert!(
        check_control_message_size(MAX_CONTROL_MESSAGE_BYTES, MAX_CONTROL_MESSAGE_BYTES).is_ok(),
        "a post-authentication message of exactly the cap must be accepted"
    );
    assert!(
        matches!(
            check_control_message_size(MAX_CONTROL_MESSAGE_BYTES + 1, MAX_CONTROL_MESSAGE_BYTES),
            Err(ProtocolError::Oversized { limit, .. }) if limit == MAX_CONTROL_MESSAGE_BYTES
        ),
        "a post-authentication message over the cap must be rejected"
    );

    // The stream formats the spec declares.
    assert_eq!(
        StreamFormat::UPLINK,
        StreamFormat {
            sample_rate: 16_000,
            channels: 1,
            frame_size_samples: 320
        },
        "the uplink format must be 16 kHz mono, 320 samples"
    );
    assert_eq!(
        StreamFormat::DOWNLINK,
        StreamFormat {
            sample_rate: 24_000,
            channels: 1,
            frame_size_samples: 480
        },
        "the downlink format must be 24 kHz mono, 480 samples"
    );
    assert!(
        check_stream_format(StreamFormat::UPLINK, StreamFormat::UPLINK).is_ok(),
        "a matching format must be accepted"
    );
    let mismatch = check_stream_format(StreamFormat::UPLINK, StreamFormat::DOWNLINK);
    assert!(
        matches!(mismatch, Err(ProtocolError::FormatMismatch { .. })),
        "a mismatched format must be rejected, it returned {mismatch:?}"
    );

    // The capability mask: a bit this build does not know is ignored.
    assert_eq!(KNOWN_MASK, CAP_CONTROL_MESSAGES);
    let unknown_bit = 1u64 << 63;
    assert_eq!(
        KNOWN_MASK & unknown_bit,
        0,
        "the bit under test must be outside the known mask"
    );
    assert_eq!(
        KNOWN_MASK.count_ones(),
        1,
        "v1 declares exactly one capability"
    );

    // The error vocabulary: every code has one distinct, stable spelling.
    let mut seen = BTreeSet::new();
    for code in RemoteErrorCode::ALL {
        let name = code.wire_name();
        assert!(
            !name.is_empty(),
            "the error code {code:?} must have a wire spelling"
        );
        assert!(
            name.chars()
                .all(|character| character.is_ascii_lowercase() || character == '_'),
            "the wire spelling `{name}` must be lowercase with underscores only"
        );
        assert!(
            seen.insert(name),
            "the wire spelling `{name}` is used by more than one error code"
        );
        assert_eq!(
            RemoteErrorCode::from_wire_name(name),
            Some(code),
            "the wire spelling `{name}` must resolve back to its code"
        );
    }
    assert_eq!(seen.len(), 12, "there are twelve wire error codes");
    assert_eq!(
        RemoteErrorCode::from_wire_name("not_a_code"),
        None,
        "an unknown wire spelling must not resolve to a code"
    );

    let code = RemoteErrorCode::InvalidArgument;
    let encoded = serde_json::to_string(&code).expect("the error code must serialise");
    assert_eq!(
        encoded, "\"invalid_argument\"",
        "the wire form of an error code must be its stable lowercase string"
    );
    let decoded: RemoteErrorCode =
        serde_json::from_str(&encoded).expect("the error code must deserialise");
    assert_eq!(decoded, code);
}
