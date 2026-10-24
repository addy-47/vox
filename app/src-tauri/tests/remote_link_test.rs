//! ============================================================================
//! tests/remote_link_test.rs — Remote Control In-Process Link (Seam 02)
//! ============================================================================
//! Category     : Integration Test (Seam 02 — in-process link)
//! Component    : src/services/remote/link/{remote_link,loopback}.rs
//! Binary       : remote_link_test
//! Prerequisites: none (in-process link; no models, no services, no DB)
//! Run          : cargo nextest run --test remote_link_test --release --nocapture --test-threads=1
//! ============================================================================

mod common;

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use common::{remote_conformance::run_link_conformance, remote_samples::sample_ping};
use futures_util::{task::noop_waker, SinkExt};
use tokio_tungstenite::tungstenite::Message;
use vox_lib::services::remote::{
    link::{
        bind, dial, LinkCloseReason, LinkError, LoopbackLink, RawLink, RemoteLink,
        LOOPBACK_CHANNEL_CAPACITY,
    },
    protocol::{
        encode_control,
        framing::{AudioFrame, AudioPayload},
        messages::{AudioControlMessage, ControlMessage, Pong, PttAction, PttMarker, Req},
        ProtocolError, AUDIO_CODEC_PCM16, AUDIO_PAYLOAD_TAG_JSON,
    },
    MAX_CONTROL_MESSAGE_BYTES,
};

/// The bound on every await in this file.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// An audio payload tag the protocol does not assign.
const UNKNOWN_AUDIO_PAYLOAD_TAG: u8 = 0x02;

/// A message type this build does not know.
const UNKNOWN_TYPE: &str = "AMessageFromTheFuture";

/// Expected cases in the generic link conformance suite.
const EXPECTED_CONFORMANCE_CASES: [&str; 18] = [
    "control_round_trip",
    "control_ordering",
    "audio_round_trip",
    "audio_payload_discrimination",
    "audio_ordering",
    "control_independent_of_audio",
    "backpressure",
    "send_after_close",
    "recv_after_close",
    "peer_close_observed",
    "unknown_type_skipped",
    "oversized_message_rejected",
    "all_protocol_types_round_trip",
    "oversized_received_message_rejected",
    "close_on_unknown_audio_payload_tag",
    "protocol_violation_reports_its_cause",
    "skippable_violation_does_not_end_the_receive",
    "skippable_audio_violation_does_not_end_the_receive",
];

/// Bounds one operation by the test timeout.
async fn within<F: Future>(label: &str, future: F) -> F::Output {
    match tokio::time::timeout(TEST_TIMEOUT, future).await {
        Ok(value) => value,
        Err(_) => panic!("[{label}] the operation did not complete within {TEST_TIMEOUT:?}"),
    }
}

/// Bounds one fallible operation by the test timeout and unwraps its result.
async fn within_ok<F, T, E>(label: &str, future: F) -> T
where
    F: Future<Output = Result<T, E>>,
    T: Sized,
    E: std::fmt::Display,
{
    match within(label, future).await {
        Ok(value) => value,
        Err(error) => panic!("[{label}] the operation failed: {error}"),
    }
}

/// Polls a future exactly once, with a waker that does nothing.
fn poll_once<F: Future + ?Sized>(future: Pin<&mut F>) -> Poll<F::Output> {
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    future.poll(&mut context)
}

/// A deterministic audio frame fixture.
fn sample_audio_frame(seq: u32) -> AudioFrame {
    AudioFrame {
        codec: AUDIO_CODEC_PCM16,
        seq,
        epoch: 1,
        sample_offset: u64::from(seq) * 320,
        samples: vec![0, 1, -1, 256],
    }
}

/// A deterministic PTT marker fixture.
fn sample_ptt_marker() -> AudioControlMessage {
    AudioControlMessage::PttMarker(PttMarker {
        action: PttAction::Start,
        sample_offset: 640,
        epoch: 1,
    })
}

/// Builds a control message whose encoded length is exactly `size`.
fn control_message_of_exact_size(size: usize) -> ControlMessage {
    let probe = ControlMessage::Req(Req {
        request_id: 1,
        command: String::new(),
        args: serde_json::Value::Null,
    });
    let overhead = encode_control(&probe)
        .expect("the probe message must encode")
        .len();
    assert!(
        size > overhead,
        "the requested size must leave room for the message envelope"
    );
    let message = ControlMessage::Req(Req {
        request_id: 1,
        command: "x".repeat(size - overhead),
        args: serde_json::Value::Null,
    });
    let encoded = encode_control(&message).expect("the message must encode");
    assert_eq!(encoded.len(), size);
    message
}

// ============================================================================
// Subtests
// ============================================================================

/// PURPOSE:  A control message and audio payload must transit the link without corruption.
/// SCENARIO: Send control Ping, binary AudioFrame, and JSON PttMarker across a loopback pair.
/// ASSERTS:  Every payload is received identical to sent; JSON marker is not decoded as frame.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Change decode_audio_payload to swap AudioPayload enum branches.
#[tokio::test]
async fn control_message_and_audio_frame_arrive_intact() {
    let (sender, receiver) = LoopbackLink::pair();
    let message = sample_ping(17);
    let payload = AudioPayload::Frame(sample_audio_frame(3));

    within_ok("send control", sender.send_control(message.clone())).await;
    within_ok("send audio", sender.send_audio(payload.clone())).await;

    let received = within_ok("recv control", receiver.recv_control()).await;
    assert_eq!(received, message);

    let received = within_ok("recv audio", receiver.recv_audio()).await;
    assert_eq!(received, payload);

    let marker = sample_ptt_marker();
    within_ok(
        "send marker",
        sender.send_audio(AudioPayload::Control(marker.clone())),
    )
    .await;
    let received = within_ok("recv marker", receiver.recv_audio()).await;
    assert!(matches!(&received, AudioPayload::Control(got) if *got == marker));
}

/// PURPOSE:  Control traffic must not be head-of-line blocked when the audio channel is saturated.
/// SCENARIO: Fill the audio channel to full capacity (64 frames), then dispatch a control Ping.
/// ASSERTS:  Control message is immediately received despite the audio channel remaining full.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove independent audio queue, forcing control messages into the audio channel.
#[tokio::test]
async fn control_and_audio_channels_are_independent() {
    let (sender, receiver) = LoopbackLink::pair();
    let sender = Arc::new(sender);
    let receiver = Arc::new(receiver);

    let mut accepted = 0usize;
    loop {
        let mut future =
            sender.send_audio(AudioPayload::Frame(sample_audio_frame(accepted as u32)));
        match poll_once(future.as_mut()) {
            Poll::Ready(Ok(())) => {
                accepted += 1;
                assert!(accepted <= LOOPBACK_CHANNEL_CAPACITY);
            }
            Poll::Ready(Err(error)) => panic!("probe send failed: {error}"),
            Poll::Pending => break,
        }
    }
    assert_eq!(accepted, LOOPBACK_CHANNEL_CAPACITY);

    let message = sample_ping(99);
    within_ok("send control", sender.send_control(message.clone())).await;
    let received = within_ok("recv control", receiver.recv_control()).await;
    assert_eq!(received, message);
}

/// PURPOSE:  A control message submitted to a saturated channel must await room and preserve ordering.
/// SCENARIO: Fill control channel to capacity, spawn an asynchronous send, then drain receiver.
/// ASSERTS:  Awaiting send completes only after draining, and all messages arrive strictly in FIFO order.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Drop overflow messages instead of awaiting backpressure.
#[tokio::test]
async fn send_into_full_channel_awaits_and_preserves_order() {
    let (sender, receiver) = LoopbackLink::pair();
    let sender = Arc::new(sender);
    let receiver = Arc::new(receiver);

    let mut accepted = 0usize;
    loop {
        let mut future = sender.send_control(sample_ping(accepted as u64));
        match poll_once(future.as_mut()) {
            Poll::Ready(Ok(())) => {
                accepted += 1;
                assert!(accepted <= LOOPBACK_CHANNEL_CAPACITY);
            }
            Poll::Ready(Err(error)) => panic!("probe send failed: {error}"),
            Poll::Pending => break,
        }
    }
    assert_eq!(accepted, LOOPBACK_CHANNEL_CAPACITY);

    let witness = sample_ping(accepted as u64);
    let task_witness = witness.clone();
    let sending = Arc::clone(&sender);
    let handle = tokio::spawn(async move { sending.send_control(task_witness).await });

    tokio::task::yield_now().await;
    assert!(!handle.is_finished());

    let mut received = Vec::with_capacity(accepted + 1);
    for _ in 0..=accepted {
        received.push(within_ok("recv", receiver.recv_control()).await);
    }

    let send_result = within("join", handle).await.expect("task must not panic");
    within_ok("send result", async { send_result }).await;

    for (index, message) in received.iter().enumerate() {
        assert!(matches!(message, ControlMessage::Ping(ping) if ping.seq == index as u64));
    }
    assert!(
        matches!(received.last(), Some(ControlMessage::Ping(ping)) if ping.seq == accepted as u64)
    );
}

/// PURPOSE:  Unknown control types and tagless messages must be skipped without closing the link.
/// SCENARIO: Send raw unrecognised JSON type followed by a tagless object, then send a valid Ping.
/// ASSERTS:  Receiver skips unrecognised payloads, link remains open, and subsequent Ping arrives.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Treat DecodeOutcome::UnknownType as fatal and close link.
#[tokio::test]
async fn unknown_message_type_is_skipped_and_connection_maintained() {
    let (sender, receiver) = LoopbackLink::pair();

    let unknown = br#"{"type":"AMessageFromTheFuture","payload":{"nested":true}}"#;
    within_ok("send raw", sender.send_control_bytes(unknown.to_vec())).await;

    let tagless = br#"{"seq":1}"#;
    within_ok("send raw", sender.send_control_bytes(tagless.to_vec())).await;

    let expected = sample_ping(23);
    within_ok("send", sender.send_control(expected.clone())).await;

    let received = within_ok("recv", receiver.recv_control()).await;
    assert_eq!(received, expected);
    assert_eq!(receiver.close_reason(), None);
}

/// PURPOSE:  A skipped control message must emit a structured warning naming the skipped type.
/// SCENARIO: Send raw unknown control message followed by valid Ping while capturing logger output.
/// ASSERTS:  Logs contain `[WARN]`, the unrecognised type name, and the text `survivable`.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Downgrade skip log level from warn! to debug! or remove warning.
#[tokio::test]
async fn unknown_message_type_logs_warn_telemetry() {
    common::log_capture::install();
    let _ = common::log_capture::take();

    let (sender, receiver) = LoopbackLink::pair();
    let unknown = format!("{{\"type\":\"{UNKNOWN_TYPE}\",\"payload\":{{\"nested\":true}}}}");
    sender
        .send_control_bytes(unknown.into_bytes())
        .await
        .expect("raw send");

    // A tagless message is a survivable violation that logs "survivable"
    sender
        .send_control_bytes(br#"{"seq":1}"#.to_vec())
        .await
        .expect("raw send");

    let expected = sample_ping(23);
    sender
        .send_control(expected.clone())
        .await
        .expect("send ping");

    let received = receiver.recv_control().await.expect("receive ping");
    assert_eq!(received, expected);

    let captured = common::log_capture::take_text();
    assert!(captured.contains("[WARN]"));
    assert!(captured.contains(UNKNOWN_TYPE));
    assert!(captured.contains("survivable"));
}

/// PURPOSE:  Malformed payload schemas must close the link and report MalformedJson protocol error.
/// SCENARIO: Send control JSON with invalid field types (`seq: "not-a-number"`).
/// ASSERTS:  Receiver returns LinkError::Closed(Protocol(MalformedJson)) and records close reason.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Ignore deserialization errors and return empty Ok(None).
#[tokio::test]
async fn mutated_transit_message_is_rejected_with_cause() {
    let (sender, receiver) = LoopbackLink::pair();

    let mutated = br#"{"type":"Ping","seq":"not-a-number"}"#;
    within_ok("send raw", sender.send_control_bytes(mutated.to_vec())).await;

    let result = within("recv", receiver.recv_control()).await;
    assert!(matches!(
        result,
        Err(LinkError::Closed(LinkCloseReason::Protocol(
            ProtocolError::MalformedJson(_)
        )))
    ));
    assert!(matches!(
        receiver.close_reason(),
        Some(LinkCloseReason::Protocol(ProtocolError::MalformedJson(_)))
    ));
}

/// PURPOSE:  Operating on a closed link must return LinkError::Closed and never panic.
/// SCENARIO: Close sender with LinkCloseReason::Local, then attempt control/audio sends and receives.
/// ASSERTS:  Sender reports Closed(Local); receiver reports Closed(Peer); operations fail gracefully.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Leave channels dangling without notifying peer of closure.
#[tokio::test]
async fn closed_link_reports_closed_state_consistently() {
    let (sender, receiver) = LoopbackLink::pair();

    within_ok("close", sender.close(LinkCloseReason::Local)).await;

    let on_self = within("send", sender.send_control(sample_ping(1))).await;
    assert!(matches!(
        on_self,
        Err(LinkError::Closed(LinkCloseReason::Local))
    ));

    let on_peer = within("send", receiver.send_control(sample_ping(2))).await;
    assert!(matches!(
        on_peer,
        Err(LinkError::Closed(LinkCloseReason::Peer))
    ));

    let audio = within(
        "send audio",
        sender.send_audio(AudioPayload::Frame(sample_audio_frame(1))),
    )
    .await;
    assert!(matches!(audio, Err(LinkError::Closed(_))));

    let control = within("recv", receiver.recv_control()).await;
    assert!(matches!(
        control,
        Err(LinkError::Closed(LinkCloseReason::Peer))
    ));

    let audio = within("recv audio", receiver.recv_audio()).await;
    assert!(matches!(
        audio,
        Err(LinkError::Closed(LinkCloseReason::Peer))
    ));

    assert_eq!(receiver.close_reason(), Some(LinkCloseReason::Peer));
    assert_eq!(sender.close_reason(), Some(LinkCloseReason::Local));
}

/// PURPOSE:  A tagless JSON message on the audio channel must be skipped without closing the audio socket.
/// SCENARIO: Inject JSON payload tag with tagless object into audio channel, followed by valid AudioFrame.
/// ASSERTS:  The invalid payload is skipped, link remains open, and AudioFrame arrives intact.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Treat tagless audio control message as fatal format mismatch.
#[tokio::test]
async fn survivable_audio_violation_skips_payload() {
    let (sender, receiver) = LoopbackLink::pair();

    let mut tagless = vec![AUDIO_PAYLOAD_TAG_JSON];
    tagless.extend_from_slice(br#"{"seq":1}"#);
    within_ok("send raw", sender.send_audio_bytes(tagless)).await;

    let expected = sample_audio_frame(41);
    within_ok(
        "send frame",
        sender.send_audio(AudioPayload::Frame(expected.clone())),
    )
    .await;

    let received = within_ok("recv", receiver.recv_audio()).await;
    assert_eq!(received, AudioPayload::Frame(expected));
    assert_eq!(receiver.close_reason(), None);
}

/// PURPOSE:  The maximum control message size cap must be enforced as an exact boundary.
/// SCENARIO: Send a message of exactly MAX_CONTROL_MESSAGE_BYTES, then attempt cap + 1.
/// ASSERTS:  Exact cap succeeds; cap + 1 returns LinkError::Protocol(Oversized); link remains open for next send.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Change `size > limit` to `size >= limit` in check_control_message_size.
#[tokio::test]
async fn control_message_at_cap_transits_cleanly() {
    let (sender, receiver) = LoopbackLink::pair();

    let at_cap = control_message_of_exact_size(MAX_CONTROL_MESSAGE_BYTES);
    within_ok("send at cap", sender.send_control(at_cap.clone())).await;
    let received = within_ok("recv at cap", receiver.recv_control()).await;
    assert_eq!(received, at_cap);

    let over_cap = control_message_of_exact_size(MAX_CONTROL_MESSAGE_BYTES + 1);
    let refused = within("send over cap", sender.send_control(over_cap)).await;
    assert!(matches!(
        refused,
        Err(LinkError::Protocol(ProtocolError::Oversized { limit, .. }))
            if limit == MAX_CONTROL_MESSAGE_BYTES
    ));

    let after = sample_ping(5);
    within_ok("send after", sender.send_control(after.clone())).await;
    let received = within_ok("recv after", receiver.recv_control()).await;
    assert_eq!(received, after);
}

/// PURPOSE:  A fatal protocol violation must close the link and persist the specific protocol error.
/// SCENARIO: Send raw oversized control buffer and an unknown audio payload tag.
/// ASSERTS:  Link closes with LinkError::Closed(Protocol(...)) identifying the specific violation.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Mask protocol error into generic LinkCloseReason::Local.
#[tokio::test]
async fn protocol_violation_terminates_with_causal_reason() {
    let (sender, receiver) = LoopbackLink::pair();

    let oversized = vec![0u8; MAX_CONTROL_MESSAGE_BYTES + 1];
    within_ok("send raw", sender.send_control_bytes(oversized)).await;

    let received = within("recv", receiver.recv_control()).await;
    assert!(matches!(
        received,
        Err(LinkError::Closed(LinkCloseReason::Protocol(
            ProtocolError::Oversized { .. }
        )))
    ));
    assert!(matches!(
        receiver.close_reason(),
        Some(LinkCloseReason::Protocol(ProtocolError::Oversized { .. }))
    ));

    let (sender, receiver) = LoopbackLink::pair();
    let mut malformed = vec![UNKNOWN_AUDIO_PAYLOAD_TAG];
    malformed.extend_from_slice(&[0u8; 4]);
    within_ok("send raw", sender.send_audio_bytes(malformed)).await;

    let received = within("recv", receiver.recv_audio()).await;
    assert!(matches!(
        received,
        Err(LinkError::Closed(LinkCloseReason::Protocol(
            ProtocolError::UnknownPayloadTag(tag)
        ))) if tag == UNKNOWN_AUDIO_PAYLOAD_TAG
    ));
}

/// PURPOSE:  Deliberate, transport-level, and peer-observed closures must retain distinct close reasons.
/// SCENARIO: Explicitly trigger closes with Local and Transport reasons, and observe across loopback.
/// ASSERTS:  Local reports Local, Transport retains its diagnostic text, and peer observes Peer.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Collapse all close reasons to LinkCloseReason::Local.
#[tokio::test]
async fn deliberate_and_peer_closes_preserve_distinct_reasons() {
    let (deliberate, _peer) = LoopbackLink::pair();
    within_ok("close", deliberate.close(LinkCloseReason::Local)).await;
    assert_eq!(deliberate.close_reason(), Some(LinkCloseReason::Local));

    let (transport, _peer) = LoopbackLink::pair();
    within_ok(
        "close",
        transport.close(LinkCloseReason::Transport("socket reset".to_string())),
    )
    .await;
    assert_eq!(
        transport.close_reason(),
        Some(LinkCloseReason::Transport("socket reset".to_string()))
    );

    let (first, second) = LoopbackLink::pair();
    within_ok("close", first.close(LinkCloseReason::Local)).await;
    let received = within("recv", second.recv_control()).await;
    assert!(matches!(
        received,
        Err(LinkError::Closed(LinkCloseReason::Peer))
    ));
    assert_eq!(second.close_reason(), Some(LinkCloseReason::Peer));
}

/// PURPOSE:  A protocol violation caused by one end must be observed by the peer with its exact cause.
/// SCENARIO: Link endpoint closes itself with LinkCloseReason::Protocol(MalformedJson).
/// ASSERTS:  Observing peer receives LinkError::Closed(Protocol(MalformedJson)) with preserved cause.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Flatten observed protocol violations to LinkCloseReason::Peer.
#[tokio::test]
async fn peer_protocol_violation_survives_cross_link_observation() {
    let (breaker, observer) = LoopbackLink::pair();
    let cause = ProtocolError::MalformedJson("the peer sent nonsense".to_string());
    within_ok(
        "close",
        breaker.close(LinkCloseReason::Protocol(cause.clone())),
    )
    .await;

    let received = within("recv", observer.recv_control()).await;
    assert!(matches!(
        received,
        Err(LinkError::Closed(LinkCloseReason::Protocol(
            ProtocolError::MalformedJson(_)
        )))
    ));
    assert_eq!(
        observer.close_reason(),
        Some(LinkCloseReason::Protocol(cause))
    );
}

/// PURPOSE:  LoopbackLink must satisfy all 18 conformance contract cases defined in the conformance suite.
/// SCENARIO: Execute run_link_conformance(LoopbackLink::pair).
/// ASSERTS:  Every expected conformance case passes with 0 failures.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Break loopback ordering or channel separation in loopback.rs.
#[tokio::test]
async fn loopback_passes_generic_conformance_suite() {
    let report = run_link_conformance(LoopbackLink::pair).await;

    for case in EXPECTED_CONFORMANCE_CASES {
        assert!(
            report.passed(case),
            "conformance case `{case}` failed: {:?}",
            report.failure_reason(case)
        );
    }
    assert_eq!(
        report.ran().len(),
        EXPECTED_CONFORMANCE_CASES.len(),
        "the suite must run every case exactly once; it ran {:?}",
        report.ran()
    );
    assert!(
        report.failures().is_empty(),
        "conformance run reported failures: {:?}",
        report.failures()
    );
}

/// PURPOSE:  A control message must traverse physical WebSocket loopback TCP without corruption.
/// SCENARIO: Bind WsListener on loopback, dial WsLink, exchange Ping and Pong control messages.
/// ASSERTS:  Delivered message is identical to transmitted message; both directions succeed.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Corrupt control payload serialization in WsLink::send_control.
#[tokio::test]
async fn ws_link_control_roundtrip_delivers_payload() {
    let listener = within_ok("bind", bind("127.0.0.1:0".parse().expect("valid loopback"))).await;
    let addr = listener.local_addr().expect("local addr");
    let server_handle = tokio::spawn(async move { listener.accept().await });

    let client = within_ok("dial", dial(&format!("ws://{addr}"))).await;
    let server = server_handle
        .await
        .expect("server task join")
        .expect("server accept");

    let ping = sample_ping(42);
    within_ok("send ping", client.send_control(ping.clone())).await;
    let received_ping = within_ok("recv ping", server.recv_control()).await;
    assert_eq!(received_ping, ping);

    let pong = ControlMessage::Pong(Pong { seq: 42 });
    within_ok("send pong", server.send_control(pong.clone())).await;
    let received_pong = within_ok("recv pong", client.recv_control()).await;
    assert_eq!(received_pong, pong);
}

/// PURPOSE:  Inbound messages exceeding MAX_CONTROL_MESSAGE_BYTES must terminate the link with a protocol error.
/// SCENARIO: Send raw WebSocket binary frame larger than MAX_CONTROL_MESSAGE_BYTES to WsLink.
/// ASSERTS:  recv_control returns LinkError::Protocol(Oversized) and close_reason records Protocol(Oversized).
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Remove check_control_message_size check in WsLink::decode_incoming.
#[tokio::test]
async fn ws_link_oversized_message_closes_with_protocol_error() {
    let listener = within_ok("bind", bind("127.0.0.1:0".parse().expect("valid loopback"))).await;
    let addr = listener.local_addr().expect("local addr");
    let server_handle = tokio::spawn(async move { listener.accept().await });

    let (mut raw_client, _) = tokio_tungstenite::connect_async(format!("ws://{addr}"))
        .await
        .expect("connect");
    let server = server_handle
        .await
        .expect("server task join")
        .expect("server accept");

    let oversized_bytes = vec![0u8; MAX_CONTROL_MESSAGE_BYTES + 1];
    raw_client
        .send(Message::Binary(oversized_bytes.into()))
        .await
        .expect("raw send");

    let recv_res = within("recv", server.recv_control()).await;
    assert!(matches!(
        recv_res,
        Err(LinkError::Protocol(ProtocolError::Oversized { limit, .. }))
            if limit == MAX_CONTROL_MESSAGE_BYTES
    ));
    assert!(matches!(
        server.close_reason(),
        Some(LinkCloseReason::Protocol(ProtocolError::Oversized { limit, .. }))
            if limit == MAX_CONTROL_MESSAGE_BYTES
    ));

    let send_after = server.send_control(sample_ping(1)).await;
    assert!(matches!(send_after, Err(LinkError::Closed(_))));
}

/// PURPOSE:  Closing one end of a WsLink must notify the peer with LinkCloseReason::Peer.
/// SCENARIO: Close server WsLink with LinkCloseReason::Local; client calls recv_control.
/// ASSERTS:  Client recv_control returns LinkError::Closed(Peer) and close_reason is Some(Peer).
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Do not send Message::Close on WsLink::close.
#[tokio::test]
async fn ws_link_clean_close_notifies_peer() {
    let listener = within_ok("bind", bind("127.0.0.1:0".parse().expect("valid loopback"))).await;
    let addr = listener.local_addr().expect("local addr");
    let server_handle = tokio::spawn(async move { listener.accept().await });

    let client = within_ok("dial", dial(&format!("ws://{addr}"))).await;
    let server = server_handle
        .await
        .expect("server task join")
        .expect("server accept");

    within_ok("close", server.close(LinkCloseReason::Local)).await;
    assert_eq!(server.close_reason(), Some(LinkCloseReason::Local));

    let recv_res = within("recv", client.recv_control()).await;
    assert!(matches!(
        recv_res,
        Err(LinkError::Closed(LinkCloseReason::Peer))
    ));
    assert_eq!(client.close_reason(), Some(LinkCloseReason::Peer));
}
