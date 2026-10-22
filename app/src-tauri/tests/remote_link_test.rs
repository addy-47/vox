//! ============================================================================
//! tests/remote_link_test.rs — Remote Control Link Behaviour
//! ============================================================================
//! Category     : Integration Test (Seam 02 — in-process link)
//! Component    : src/services/remote/link/{remote_link,loopback}.rs
//! Prerequisites: None (in-process link; no models, no services, no DB)
//! Execution    : cargo nextest run --test remote_link_test --release --nocapture --test-threads=1
//! Metrics      : (a) a control message and an audio payload arrive intact;
//!                (b) the two channels are independent; (c) an unknown message
//!                type and a survivable protocol violation are skipped, not
//!                fatal, and the link keeps delivering; (d) a send into a full
//!                channel awaits and is not dropped; (e) the link decodes the
//!                real wire form, so a mutated message is rejected; (f) a
//!                closed link reports `Closed` and never panics.
//!
//! WHY THIS EXISTS
//! ---------------
//! The loopback is the only way the remote-control feature can be exercised
//! without a socket, a phone and a model. It is therefore also the first
//! place a defect in the link contract shows up. A link that drops a message,
//! blocks a control send behind a full audio channel, or panics on a send
//! after a close would be found here rather than on a device.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! These tests prove the behaviour of the in-process link. They cannot prove
//! that a socket-backed link behaves the same — that is what the conformance
//! suite in `tests/common/remote_conformance.rs` is for.
//! ============================================================================

mod common;

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use common::remote_samples::sample_ping;
use futures_util::task::noop_waker;
use vox_lib::services::remote::{
    link::{
        LinkCloseReason, LinkError, LoopbackLink, RawLink, RemoteLink, LOOPBACK_CHANNEL_CAPACITY,
    },
    protocol::{
        encode_control,
        framing::{AudioFrame, AudioPayload},
        messages::{AudioControlMessage, ControlMessage, PttAction, PttMarker, Req},
        ProtocolError, AUDIO_CODEC_PCM16, AUDIO_PAYLOAD_TAG_JSON,
    },
    MAX_CONTROL_MESSAGE_BYTES,
};

/// The bound on every await in this file.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

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
///
/// A send into a channel with room completes on the first poll; a send into
/// a full channel does not. That is how these tests find a full channel
/// without sleeping.
fn poll_once<F: Future + ?Sized>(future: Pin<&mut F>) -> Poll<F::Output> {
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    future.poll(&mut context)
}

/// A deterministic audio frame.
fn sample_audio_frame(seq: u32) -> AudioFrame {
    AudioFrame {
        codec: AUDIO_CODEC_PCM16,
        seq,
        epoch: 1,
        sample_offset: u64::from(seq) * 320,
        samples: vec![0, 1, -1, 256],
    }
}

/// A deterministic PTT marker.
fn sample_ptt_marker() -> AudioControlMessage {
    AudioControlMessage::PttMarker(PttMarker {
        action: PttAction::Start,
        sample_offset: 640,
        epoch: 1,
    })
}

// ============================================================================
// Test 1: A control message and an audio payload arrive intact
// ============================================================================

#[tokio::test]
async fn test_a_control_message_and_an_audio_frame_arrive_intact() {
    let (sender, receiver) = LoopbackLink::pair();
    let message = sample_ping(17);
    let payload = AudioPayload::Frame(sample_audio_frame(3));

    within_ok("send control", sender.send_control(message.clone())).await;
    within_ok("send audio", sender.send_audio(payload.clone())).await;

    let received = within_ok("recv control", receiver.recv_control()).await;
    assert_eq!(
        received, message,
        "the control message did not arrive intact"
    );

    let received = within_ok("recv audio", receiver.recv_audio()).await;
    assert_eq!(received, payload, "the audio payload did not arrive intact");

    // The two audio shapes are told apart on the same channel.
    let marker = sample_ptt_marker();
    within_ok(
        "send marker",
        sender.send_audio(AudioPayload::Control(marker.clone())),
    )
    .await;
    let received = within_ok("recv marker", receiver.recv_audio()).await;
    assert!(
        matches!(&received, AudioPayload::Control(got) if *got == marker),
        "the JSON marker must not be received as a frame: {received:?}"
    );
}

// ============================================================================
// Test 2: The control and audio channels are independent
// ============================================================================

#[tokio::test]
async fn test_the_control_and_audio_channels_are_independent() {
    let (sender, receiver) = LoopbackLink::pair();
    let sender = Arc::new(sender);
    let receiver = Arc::new(receiver);

    // Fill the audio channel.
    let mut accepted = 0usize;
    loop {
        let mut future =
            sender.send_audio(AudioPayload::Frame(sample_audio_frame(accepted as u32)));
        match poll_once(future.as_mut()) {
            Poll::Ready(Ok(())) => {
                accepted += 1;
                assert!(
                    accepted <= LOOPBACK_CHANNEL_CAPACITY,
                    "the audio channel accepted more than its named capacity"
                );
            }
            Poll::Ready(Err(error)) => panic!("the probe send failed: {error}"),
            Poll::Pending => break,
        }
    }
    assert_eq!(
        accepted, LOOPBACK_CHANNEL_CAPACITY,
        "the audio channel must be full at its named capacity"
    );

    // The control channel must still carry a message.
    let message = sample_ping(99);
    within_ok("send control", sender.send_control(message.clone())).await;
    let received = within_ok("recv control", receiver.recv_control()).await;
    assert_eq!(
        received, message,
        "a full audio channel must not block a control message"
    );
}

// ============================================================================
// Test 3: A send into a full channel awaits and is not dropped
// ============================================================================

#[tokio::test]
async fn test_a_send_into_a_full_channel_awaits_and_is_not_dropped() {
    let (sender, receiver) = LoopbackLink::pair();
    let sender = Arc::new(sender);
    let receiver = Arc::new(receiver);

    let mut accepted = 0usize;
    loop {
        let mut future = sender.send_control(sample_ping(accepted as u64));
        match poll_once(future.as_mut()) {
            Poll::Ready(Ok(())) => {
                accepted += 1;
                assert!(
                    accepted <= LOOPBACK_CHANNEL_CAPACITY,
                    "the control channel accepted more than its named capacity"
                );
            }
            Poll::Ready(Err(error)) => panic!("the probe send failed: {error}"),
            Poll::Pending => break,
        }
    }
    assert_eq!(
        accepted, LOOPBACK_CHANNEL_CAPACITY,
        "the control channel must be full at its named capacity"
    );

    // This send cannot complete until the peer drains the channel.
    let witness = sample_ping(accepted as u64);
    let task_witness = witness.clone();
    let sending = Arc::clone(&sender);
    let handle = tokio::spawn(async move { sending.send_control(task_witness).await });

    tokio::task::yield_now().await;
    assert!(
        !handle.is_finished(),
        "a send into a full channel must await, not complete"
    );

    let mut received = Vec::with_capacity(accepted + 1);
    for _ in 0..=accepted {
        received.push(within_ok("recv", receiver.recv_control()).await);
    }

    let send_result = within("join", handle)
        .await
        .expect("the sending task must not panic");
    within_ok("send result", async { send_result }).await;

    for (index, message) in received.iter().enumerate() {
        assert!(
            matches!(message, ControlMessage::Ping(ping) if ping.seq == index as u64),
            "message {index} arrived out of order: {message:?}"
        );
    }
    assert!(
        matches!(received.last(), Some(ControlMessage::Ping(ping)) if ping.seq == accepted as u64),
        "the message that waited for room was dropped: {:?}",
        received.last()
    );
}

// ============================================================================
// Test 4: A survivable violation is skipped, not fatal
// ============================================================================

#[tokio::test]
async fn test_the_link_skips_an_unknown_message_type_and_keeps_delivering() {
    let (sender, receiver) = LoopbackLink::pair();

    // The typed API cannot express an unknown type, so it is injected as
    // raw bytes — exactly as a newer peer would put it on the wire.
    let unknown = br#"{"type":"AMessageFromTheFuture","payload":{"nested":true}}"#;
    within_ok("send raw", sender.send_control_bytes(unknown.to_vec())).await;

    // A message with no type tag at all. The protocol layer classifies that
    // as survivable: the receiver cannot tell what it is, so the link
    // continues rather than dying.
    let tagless = br#"{"seq":1}"#;
    within_ok("send raw", sender.send_control_bytes(tagless.to_vec())).await;

    let expected = sample_ping(23);
    within_ok("send", sender.send_control(expected.clone())).await;

    let received = within_ok("recv", receiver.recv_control()).await;
    assert_eq!(
        received, expected,
        "the skipped messages must not end the receive; the next known one must arrive"
    );
    assert_eq!(
        receiver.close_reason(),
        None,
        "a survivable violation must not close the link"
    );
}

// ============================================================================
// Test 5: The link decodes the real wire form
// ============================================================================

#[tokio::test]
async fn test_a_message_mutated_in_transit_is_rejected() {
    let (sender, receiver) = LoopbackLink::pair();

    // A well-formed JSON object whose payload does not match the schema of
    // the type it claims. The typed API could never produce this, so its
    // rejection proves the bytes really are the wire form.
    let mutated = br#"{"type":"Ping","seq":"not-a-number"}"#;
    within_ok("send raw", sender.send_control_bytes(mutated.to_vec())).await;

    let result = within("recv", receiver.recv_control()).await;
    assert!(
        matches!(
            result,
            Err(LinkError::Closed(LinkCloseReason::Protocol(
                ProtocolError::MalformedJson(_)
            )))
        ),
        "a mutated message must be rejected as a protocol failure, it returned {result:?}"
    );
    assert!(
        matches!(
            receiver.close_reason(),
            Some(LinkCloseReason::Protocol(ProtocolError::MalformedJson(_)))
        ),
        "a mutated message must close the link with the protocol cause, it reported {:?}",
        receiver.close_reason()
    );
}

// ============================================================================
// Test 6: A closed link reports `Closed` and never panics
// ============================================================================

#[tokio::test]
async fn test_a_closed_link_reports_closed_and_never_panics() {
    let (sender, receiver) = LoopbackLink::pair();

    within_ok("close", sender.close(LinkCloseReason::Local)).await;

    let on_self = within("send", sender.send_control(sample_ping(1))).await;
    assert!(
        matches!(on_self, Err(LinkError::Closed(LinkCloseReason::Local))),
        "a send on a link that closed itself must report `Closed(Local)`, it returned {on_self:?}"
    );

    let on_peer = within("send", receiver.send_control(sample_ping(2))).await;
    assert!(
        matches!(on_peer, Err(LinkError::Closed(LinkCloseReason::Peer))),
        "a send on a link whose peer closed must report `Closed(Peer)`, it returned {on_peer:?}"
    );

    let audio = within(
        "send audio",
        sender.send_audio(AudioPayload::Frame(sample_audio_frame(1))),
    )
    .await;
    assert!(
        matches!(audio, Err(LinkError::Closed(_))),
        "an audio send on a closed link must report `Closed`, it returned {audio:?}"
    );

    let control = within("recv", receiver.recv_control()).await;
    assert!(
        matches!(control, Err(LinkError::Closed(LinkCloseReason::Peer))),
        "a control receive on a link whose peer closed must report `Closed(Peer)`, it returned \
         {control:?}"
    );

    let audio = within("recv audio", receiver.recv_audio()).await;
    assert!(
        matches!(audio, Err(LinkError::Closed(LinkCloseReason::Peer))),
        "an audio receive on a link whose peer closed must report `Closed(Peer)`, it returned \
         {audio:?}"
    );

    assert_eq!(
        receiver.close_reason(),
        Some(LinkCloseReason::Peer),
        "the link must report the peer's close"
    );
    assert_eq!(
        sender.close_reason(),
        Some(LinkCloseReason::Local),
        "the link must report its own close"
    );
}

// ============================================================================
// Test 7: A survivable violation on the audio channel does not end the receive
// ============================================================================

/// The audio socket carries two payload shapes, told apart by a one-byte
/// tag. A JSON audio-control message with no `"type"` tag is a message this
/// build cannot classify — the same survivable violation the control channel
/// has. The audio receive must skip it and keep waiting, exactly as the
/// control receive does.
///
/// A transport that treated it as fatal would drop the audio channel over a
/// message a newer peer sent, and the spec's forward-compatibility rule
/// (RAIL-RC-04) says a message this build does not understand is skipped,
/// not fatal.
#[tokio::test]
async fn test_a_survivable_violation_on_the_audio_channel_does_not_end_the_receive() {
    let (sender, receiver) = LoopbackLink::pair();

    // The JSON payload tag, followed by an object with no `"type"` tag.
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
    assert_eq!(
        received,
        AudioPayload::Frame(expected),
        "a survivable violation on the audio channel must not end the receive; the next payload \
         must arrive"
    );
    assert_eq!(
        receiver.close_reason(),
        None,
        "a survivable violation must not close the link"
    );
}

// ============================================================================
// Test 8: A control message at the cap survives the link
// ============================================================================

/// Builds a control message whose encoded length is exactly `size`.
///
/// The wire form is JSON and the encoding is linear in the length of the
/// command string, so the size is reached exactly rather than
/// approximately.
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
    assert_eq!(
        encoded.len(),
        size,
        "the fixture must produce a message of exactly {size} bytes"
    );
    message
}

/// The cap is a boundary, not a suggestion.
///
/// A message of exactly the cap must be carried; a message one byte over it
/// must be refused, with the cap named so the reader knows which rule
/// rejected it. A cap applied with the wrong comparison (`>=` instead of
/// `>`) would refuse the largest legal message; a cap applied only on the
/// send side would let an oversized message onto the wire; a cap applied
/// only on the receive side would let a peer be killed by a message the
/// protocol itself calls illegal.
#[tokio::test]
async fn test_a_control_message_at_the_cap_survives_the_link() {
    let (sender, receiver) = LoopbackLink::pair();

    let at_cap = control_message_of_exact_size(MAX_CONTROL_MESSAGE_BYTES);
    within_ok("send at cap", sender.send_control(at_cap.clone())).await;
    let received = within_ok("recv at cap", receiver.recv_control()).await;
    assert_eq!(
        received, at_cap,
        "a control message of exactly the cap must survive the link"
    );

    // One byte over the cap is refused, and the refusal names the cap.
    let over_cap = control_message_of_exact_size(MAX_CONTROL_MESSAGE_BYTES + 1);
    let refused = within("send over cap", sender.send_control(over_cap)).await;
    assert!(
        matches!(
            refused,
            Err(LinkError::Protocol(ProtocolError::Oversized { limit, .. }))
                if limit == MAX_CONTROL_MESSAGE_BYTES
        ),
        "a message one byte over the cap must be refused with the cap named, it returned \
         {refused:?}"
    );

    // The link is still usable: a refused send is a local error, not a
    // protocol violation by the peer.
    let after = sample_ping(5);
    within_ok("send after", sender.send_control(after.clone())).await;
    let received = within_ok("recv after", receiver.recv_control()).await;
    assert_eq!(
        received, after,
        "a message the link refused to send must not have closed the link"
    );
}
