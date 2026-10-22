//! ============================================================================
//! tests/remote_link_close_reason_test.rs — Close Reasons & Violation Classes
//! ============================================================================
//! Category     : Integration Test (Seam 02 — in-process link)
//! Component    : src/services/remote/link/{remote_link,loopback}.rs,
//!                src/services/remote/protocol/errors.rs
//! Prerequisites: None (in-process link; no models, no services, no DB)
//! Execution    : cargo nextest run --test remote_link_close_reason_test --release --nocapture --test-threads=1
//! Metrics      : (a) a close caused by a protocol violation carries the
//!                cause and is not reported as a deliberate close; (b) a
//!                deliberate close, a transport failure and a peer-observed
//!                close still report their own reasons; (c) every
//!                `ProtocolError` variant is pinned to the class the spec
//!                gives it; (d) the loopback's behaviour agrees with that
//!                classification for every variant it can produce.
//!
//! WHY THIS EXISTS
//! ---------------
//! The session manager has to decide whether to reconnect. "The peer sent an
//! oversized message" and "we closed the link because the session ended"
//! demand different reactions: the first is a protocol error worth logging
//! loudly, the second is routine. A close that flattens both into `Local`
//! takes that decision away from the manager.
//!
//! ONE CLASSIFICATION, ONE PLACE
//! -----------------------------
//! Which protocol violations end a link is a protocol rule (remote-control
//! spec §7.1 and §7.4), so the classification lives in the protocol module
//! and the transports obey it. A transport that disagrees with the
//! classification is a transport bug, and these tests are what prove it is
//! not.
//!
//! THE EXPECTED CLASS IS WRITTEN DOWN
//! ----------------------------------
//! The table below names the class each variant must have, taken from the
//! spec. It does not ask the function under test what it thinks: an
//! expectation derived from the function proves nothing.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! These tests prove the loopback's behaviour. They cannot prove that a
//! socket-backed link behaves the same — that is what the link conformance
//! suite in `tests/common/remote_conformance.rs` is for.
//! ============================================================================

mod common;

use std::time::Duration;

use vox_lib::services::remote::{
    link::{LinkCloseReason, LinkError, LoopbackLink, RawLink, RemoteLink},
    protocol::{
        classify_protocol_error, messages::ControlMessage, ProtocolError, ProtocolResponse,
        StreamFormat, AUDIO_PAYLOAD_TAG_BINARY,
    },
    MAX_CONTROL_MESSAGE_BYTES,
};

/// The bound on every await in this file.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// An audio payload tag the protocol does not assign.
const UNKNOWN_AUDIO_PAYLOAD_TAG: u8 = 0x02;

/// A codec byte the protocol does not assign.
const UNKNOWN_CODEC: u8 = 0x09;

/// The size of an audio frame header. Remote-control spec §7.4.
const AUDIO_FRAME_HEADER_BYTES: usize = 21;

/// Bounds one operation by the test timeout.
async fn within<F: std::future::Future>(label: &str, future: F) -> F::Output {
    match tokio::time::timeout(TEST_TIMEOUT, future).await {
        Ok(value) => value,
        Err(_) => panic!("[{label}] the operation did not complete within {TEST_TIMEOUT:?}"),
    }
}

/// Bounds one fallible operation by the test timeout and unwraps its result.
async fn within_ok<F, T, E>(label: &str, future: F) -> T
where
    F: std::future::Future<Output = Result<T, E>>,
    T: Sized,
    E: std::fmt::Display,
{
    match within(label, future).await {
        Ok(value) => value,
        Err(error) => panic!("[{label}] the operation failed: {error}"),
    }
}

// ============================================================================
// Test 1: A protocol violation closes the link with its cause
// ============================================================================

#[tokio::test]
async fn test_a_protocol_violation_closes_the_link_with_its_cause() {
    let (sender, receiver) = LoopbackLink::pair();

    // An oversized control message.
    let oversized = vec![0u8; MAX_CONTROL_MESSAGE_BYTES + 1];
    within_ok("send raw", sender.send_control_bytes(oversized)).await;

    let received = within("recv", receiver.recv_control()).await;
    assert!(
        matches!(
            received,
            Err(LinkError::Closed(LinkCloseReason::Protocol(
                ProtocolError::Oversized { .. }
            )))
        ),
        "an oversized message must close the link with the protocol cause, it returned \
         {received:?}"
    );
    assert!(
        matches!(
            receiver.close_reason(),
            Some(LinkCloseReason::Protocol(ProtocolError::Oversized { .. }))
        ),
        "the close must carry the protocol cause, it reported {:?}",
        receiver.close_reason()
    );

    // An unknown audio payload tag.
    let (sender, receiver) = LoopbackLink::pair();
    let mut malformed = vec![UNKNOWN_AUDIO_PAYLOAD_TAG];
    malformed.extend_from_slice(&[0u8; 4]);
    within_ok("send raw", sender.send_audio_bytes(malformed)).await;

    let received = within("recv", receiver.recv_audio()).await;
    assert!(
        matches!(
            received,
            Err(LinkError::Closed(LinkCloseReason::Protocol(
                ProtocolError::UnknownPayloadTag(tag)
            ))) if tag == UNKNOWN_AUDIO_PAYLOAD_TAG
        ),
        "an unknown audio payload tag must close the link with the protocol cause, it returned \
         {received:?}"
    );
}

// ============================================================================
// Test 2: The other close reasons are unchanged
// ============================================================================

#[tokio::test]
async fn test_the_other_close_reasons_are_unchanged() {
    let (deliberate, _peer) = LoopbackLink::pair();
    within_ok("close", deliberate.close(LinkCloseReason::Local)).await;
    assert_eq!(
        deliberate.close_reason(),
        Some(LinkCloseReason::Local),
        "a deliberate close must still report `Local`"
    );

    let (transport, _peer) = LoopbackLink::pair();
    within_ok(
        "close",
        transport.close(LinkCloseReason::Transport("socket reset".to_string())),
    )
    .await;
    assert_eq!(
        transport.close_reason(),
        Some(LinkCloseReason::Transport("socket reset".to_string())),
        "a transport failure must still report its detail"
    );

    let (first, second) = LoopbackLink::pair();
    within_ok("close", first.close(LinkCloseReason::Local)).await;
    let received = within("recv", second.recv_control()).await;
    assert!(
        matches!(received, Err(LinkError::Closed(LinkCloseReason::Peer))),
        "a peer-observed close must still report `Peer`, it returned {received:?}"
    );
    assert_eq!(
        second.close_reason(),
        Some(LinkCloseReason::Peer),
        "the link must report the peer's close"
    );
}

// ============================================================================
// Test 3: Every protocol error variant is pinned to its spec class
// ============================================================================

#[test]
fn test_every_protocol_error_variant_is_pinned_to_its_spec_value() {
    // Each variant, with the class the spec gives it, written down here.
    let pinned: Vec<(ProtocolError, ProtocolResponse)> = vec![
        // Spec §7.1: a message over the size cap MUST cause immediate
        // connection termination.
        (
            ProtocolError::Oversized {
                size: MAX_CONTROL_MESSAGE_BYTES + 1,
                limit: MAX_CONTROL_MESSAGE_BYTES,
            },
            ProtocolResponse::Fatal,
        ),
        // Spec §7.4: an unknown audio payload tag is a protocol error and
        // closes the audio socket.
        (
            ProtocolError::UnknownPayloadTag(UNKNOWN_AUDIO_PAYLOAD_TAG),
            ProtocolResponse::Fatal,
        ),
        // A frame this build cannot interpret cannot be recovered from.
        (
            ProtocolError::UnknownCodec(UNKNOWN_CODEC),
            ProtocolResponse::Fatal,
        ),
        (
            ProtocolError::TruncatedFrame {
                expected: AUDIO_FRAME_HEADER_BYTES,
                actual: 4,
            },
            ProtocolResponse::Fatal,
        ),
        (ProtocolError::MissingPayloadTag, ProtocolResponse::Fatal),
        (
            ProtocolError::FormatMismatch {
                expected: StreamFormat::UPLINK,
                actual: StreamFormat::DOWNLINK,
            },
            ProtocolResponse::Fatal,
        ),
        // A malformed message is a peer that cannot be understood at all.
        (
            ProtocolError::MalformedJson("not json".to_string()),
            ProtocolResponse::Fatal,
        ),
        // Spec §7.1 (RAIL-RC-04): a message whose type tag is missing is a
        // message this build does not understand. The receiver cannot tell
        // what it is, so the link survives.
        (ProtocolError::MissingTypeTag, ProtocolResponse::Skippable),
    ];

    for (error, expected) in &pinned {
        assert_eq!(
            classify_protocol_error(error),
            *expected,
            "the class of {error:?} does not match the spec"
        );
    }

    assert_eq!(
        pinned.len(),
        8,
        "every `ProtocolError` variant must be pinned; a new variant needs a decision here"
    );
}

// ============================================================================
// Test 4: The loopback agrees with the classification
// ============================================================================

#[tokio::test]
async fn test_the_loopback_agrees_with_the_classification_for_every_variant() {
    // Every violation the loopback can produce, with the bytes that produce
    // it, the channel it arrives on, and the class the spec gives it.
    let cases: Vec<(ProtocolError, Vec<u8>, bool, ProtocolResponse)> = vec![
        (
            ProtocolError::MalformedJson("not json".to_string()),
            b"this is not json".to_vec(),
            false,
            ProtocolResponse::Fatal,
        ),
        (
            ProtocolError::MissingTypeTag,
            b"{}".to_vec(),
            false,
            ProtocolResponse::Skippable,
        ),
        (
            ProtocolError::MissingPayloadTag,
            Vec::new(),
            true,
            ProtocolResponse::Fatal,
        ),
        (
            ProtocolError::TruncatedFrame {
                expected: AUDIO_FRAME_HEADER_BYTES,
                actual: 4,
            },
            vec![AUDIO_PAYLOAD_TAG_BINARY, 0x00, 0x00],
            true,
            ProtocolResponse::Fatal,
        ),
        (
            ProtocolError::UnknownCodec(UNKNOWN_CODEC),
            // A whole header, so the codec byte is the thing under test and
            // not the length check.
            {
                let mut bytes = vec![AUDIO_PAYLOAD_TAG_BINARY, UNKNOWN_CODEC];
                bytes.resize(
                    AUDIO_PAYLOAD_TAG_BINARY as usize + AUDIO_FRAME_HEADER_BYTES,
                    0,
                );
                bytes
            },
            true,
            ProtocolResponse::Fatal,
        ),
        (
            ProtocolError::UnknownPayloadTag(UNKNOWN_AUDIO_PAYLOAD_TAG),
            vec![UNKNOWN_AUDIO_PAYLOAD_TAG, 0x00],
            true,
            ProtocolResponse::Fatal,
        ),
        (
            ProtocolError::Oversized {
                size: MAX_CONTROL_MESSAGE_BYTES + 1,
                limit: MAX_CONTROL_MESSAGE_BYTES,
            },
            vec![0u8; MAX_CONTROL_MESSAGE_BYTES + 1],
            false,
            ProtocolResponse::Fatal,
        ),
    ];

    for (expected, bytes, is_audio, class) in cases {
        let (sender, receiver) = LoopbackLink::pair();
        if is_audio {
            within_ok("send raw", sender.send_audio_bytes(bytes)).await;
            let received = within("recv", receiver.recv_audio()).await;
            assert_reaction(&expected, class, received, &receiver).await;
        } else {
            within_ok("send raw", sender.send_control_bytes(bytes)).await;
            if class == ProtocolResponse::Skippable {
                // A survivable violation must not end the receive: the link
                // has to keep waiting for the next message. A follow-up
                // message proves it does.
                let next = vox_lib::services::remote::protocol::messages::Ping { seq: 4242 };
                within_ok(
                    "send follow-up",
                    sender.send_control(ControlMessage::Ping(next)),
                )
                .await;
                let received = within("recv", receiver.recv_control()).await;
                assert!(
                    matches!(&received, Ok(ControlMessage::Ping(ping)) if ping.seq == 4242),
                    "a survivable violation must not end the receive; the receiver got {received:?}"
                );
                assert!(
                    !matches!(receiver.close_reason(), Some(LinkCloseReason::Protocol(_))),
                    "a survivable violation must not close the link, it reported {:?}",
                    receiver.close_reason()
                );
            } else {
                let received = within("recv", receiver.recv_control()).await;
                assert_reaction(&expected, class, received, &receiver).await;
            }
        }
    }

    // The one variant the loopback cannot produce: a stream-format
    // mismatch is a property of the `Hello` exchange, not of a frame, and
    // the session layer checks it. The classification still covers it.
    assert_eq!(
        classify_protocol_error(&ProtocolError::FormatMismatch {
            expected: StreamFormat::UPLINK,
            actual: StreamFormat::DOWNLINK,
        }),
        ProtocolResponse::Fatal,
        "a stream format mismatch must be fatal"
    );
}

// ============================================================================
// Test 5: A peer's protocol violation is observed with its cause
// ============================================================================

/// The cause of a close must survive the crossing to the other side.
///
/// The side that *caused* the violation closes with the cause. The side
/// that *observes* the close must learn the same thing: a peer that broke
/// the wire contract is not a peer that hung up politely, and the session
/// manager decides whether to log loudly or to shrug on the basis of the
/// reason it is given.
///
/// A transport that flattens an observed protocol violation into a plain
/// "peer closed" turns a peer's attack into a routine shutdown.
#[tokio::test]
async fn test_a_peer_protocol_violation_is_observed_with_its_cause() {
    let (breaker, observer) = LoopbackLink::pair();

    let cause = ProtocolError::MalformedJson("the peer sent nonsense".to_string());
    within_ok(
        "close",
        breaker.close(LinkCloseReason::Protocol(cause.clone())),
    )
    .await;

    let received = within("recv", observer.recv_control()).await;
    assert!(
        matches!(
            received,
            Err(LinkError::Closed(LinkCloseReason::Protocol(
                ProtocolError::MalformedJson(_)
            )))
        ),
        "a peer's protocol violation must be observed with its cause, it returned {received:?}"
    );
    assert_eq!(
        observer.close_reason(),
        Some(LinkCloseReason::Protocol(cause)),
        "the observing side must report the peer's protocol cause"
    );
}

/// Asserts that the link reacted to a violation the way the pinned class
/// says it must.
///
/// A skippable violation must not end the receive: the link has to keep
/// waiting for the next message.
async fn assert_reaction(
    expected: &ProtocolError,
    class: ProtocolResponse,
    received: Result<impl std::fmt::Debug, LinkError>,
    receiver: &LoopbackLink,
) {
    match class {
        ProtocolResponse::Fatal => {
            assert!(
                matches!(
                    received,
                    Err(LinkError::Closed(LinkCloseReason::Protocol(_)))
                ),
                "a fatal violation ({expected:?}) must close the link with its cause, it returned \
                 {received:?}"
            );
            assert!(
                matches!(receiver.close_reason(), Some(LinkCloseReason::Protocol(_))),
                "the close must carry the protocol cause, it reported {:?}",
                receiver.close_reason()
            );
        }
        ProtocolResponse::Skippable => {
            assert!(
                !matches!(receiver.close_reason(), Some(LinkCloseReason::Protocol(_))),
                "a skippable violation must not close the link, it reported {:?}",
                receiver.close_reason()
            );
        }
    }
}
