//! ============================================================================
//! tests/common/remote_conformance.rs — The RemoteLink Conformance Suite
//! ============================================================================
//!
//! WHY THIS EXISTS
//! ---------------
//! A new transport must not be wired into the remote-control feature
//! without proving it behaves like a link. The suite is generic over the
//! link type and names no implementation, so a second transport cannot be
//! added on the strength of a hope. Remote-control spec VERIFY-RC-15.
//!
//! The suite lives in the test tree, not in `src/`: `AGENTS.md` states that
//! `app/src-tauri/src/` carries no test logic, and the `link/` module must
//! not contain protocol semantics. A conformance suite is neither.
//!
//! EVERY CASE CAN FAIL
//! -------------------
//! A suite that always passes is worse than no suite, because it is evidence
//! of nothing. Each case therefore returns a `Result`, not a panic, and the
//! report names the case that broke. The suite is written to be
//! mutation-tested: seeding a defect in a link implementation must turn it
//! red.
//!
//! DETERMINISM
//! -----------
//! No case sleeps. Every await is bounded by a timeout, and the two cases
//! that need a full channel find one by polling a send future once and
//! observing that it did not complete — a deterministic signal, not a
//! timing guess.
//!
//! CHANNEL CAPACITY
//! ----------------
//! Two cases queue several messages before reading them, so the control and
//! audio channels of a conforming link must hold at least
//! `ORDERING_SAMPLE_COUNT` messages. The in-process link holds 64.
//! ============================================================================

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{future::BoxFuture, task::noop_waker};
use vox_lib::services::remote::{
    link::{LinkCloseReason, LinkError, RawLink},
    protocol::{
        framing::{AudioFrame, AudioPayload},
        messages::{ControlMessage, Req},
        AUDIO_PAYLOAD_TAG_JSON,
    },
    MAX_CONTROL_MESSAGE_BYTES,
};

use super::remote_samples::{control_samples, sample_ping};

/// The bound on every await in the suite.
const CASE_TIMEOUT: Duration = Duration::from_secs(10);

/// The number of messages the ordering cases queue before they read.
const ORDERING_SAMPLE_COUNT: usize = 3;

/// The upper bound on the messages the backpressure probe may accept.
///
/// The probe fails rather than loops forever if a channel never applies
/// backpressure.
const MAX_PROBE_MESSAGES: usize = 100_000;

/// An audio payload tag that the protocol does not assign.
///
/// `0x00` is the binary audio frame and `0x01` is the JSON audio-control
/// message. The next tag is unassigned, so a receiver must treat it as a
/// protocol error.
const UNKNOWN_AUDIO_PAYLOAD_TAG: u8 = 0x02;

/// Runs one case and records the outcome.
macro_rules! run_case {
    ($report:expr, $name:literal, $future:expr) => {{
        $report.ran.push($name.to_string());
        match within($name, $future).await {
            Ok(()) => {}
            Err(reason) => $report.failures.push(CaseFailure {
                case: $name.to_string(),
                reason,
            }),
        }
    }};
}

/// Fails a case with a reason instead of panicking.
macro_rules! require {
    ($condition:expr, $($reason:tt)*) => {
        if !$condition {
            return Err(format!($($reason)*));
        }
    };
}

/// A case that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseFailure {
    /// The name of the case.
    pub case: String,
    /// Why it failed.
    pub reason: String,
}

/// The outcome of a conformance run.
#[derive(Debug, Default, Clone)]
pub struct ConformanceReport {
    ran: Vec<String>,
    failures: Vec<CaseFailure>,
}

impl ConformanceReport {
    /// The names of the cases that ran, in the order they ran.
    pub fn ran(&self) -> &[String] {
        &self.ran
    }

    /// The cases that failed.
    pub fn failures(&self) -> &[CaseFailure] {
        &self.failures
    }

    /// The reason a case failed, if it failed.
    pub fn failure_reason(&self, case: &str) -> Option<&str> {
        self.failures
            .iter()
            .find(|failure| failure.case == case)
            .map(|failure| failure.reason.as_str())
    }

    /// Whether a case passed. A case that did not run counts as failed.
    pub fn passed(&self, case: &str) -> bool {
        self.ran.iter().any(|name| name == case) && self.failure_reason(case).is_none()
    }

    /// Whether every case that ran passed.
    pub fn is_success(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Runs the conformance suite against a link implementation.
///
/// `make_pair` returns two connected ends of a link. The suite is generic
/// over the link type and mentions no implementation, so a new transport
/// proves itself by satisfying the same contract.
pub async fn run_link_conformance<L, F>(make_pair: F) -> ConformanceReport
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let mut report = ConformanceReport::default();

    run_case!(report, "control_round_trip", control_round_trip(&make_pair));
    run_case!(report, "control_ordering", control_ordering(&make_pair));
    run_case!(report, "audio_round_trip", audio_round_trip(&make_pair));
    run_case!(
        report,
        "audio_payload_discrimination",
        audio_payload_discrimination(&make_pair)
    );
    run_case!(report, "audio_ordering", audio_ordering(&make_pair));
    run_case!(
        report,
        "control_independent_of_audio",
        control_independent_of_audio(&make_pair)
    );
    run_case!(report, "backpressure", backpressure(&make_pair));
    run_case!(report, "send_after_close", send_after_close(&make_pair));
    run_case!(report, "recv_after_close", recv_after_close(&make_pair));
    run_case!(
        report,
        "peer_close_observed",
        peer_close_observed(&make_pair)
    );
    run_case!(
        report,
        "unknown_type_skipped",
        unknown_type_skipped(&make_pair)
    );
    run_case!(
        report,
        "oversized_message_rejected",
        oversized_message_rejected(&make_pair)
    );
    run_case!(
        report,
        "all_protocol_types_round_trip",
        all_protocol_types_round_trip(&make_pair)
    );
    run_case!(
        report,
        "oversized_received_message_rejected",
        oversized_received_message_rejected(&make_pair)
    );
    run_case!(
        report,
        "close_on_unknown_audio_payload_tag",
        close_on_unknown_audio_payload_tag(&make_pair)
    );
    run_case!(
        report,
        "protocol_violation_reports_its_cause",
        protocol_violation_reports_its_cause(&make_pair)
    );
    run_case!(
        report,
        "skippable_violation_does_not_end_the_receive",
        skippable_violation_does_not_end_the_receive(&make_pair)
    );
    run_case!(
        report,
        "skippable_audio_violation_does_not_end_the_receive",
        skippable_audio_violation_does_not_end_the_receive(&make_pair)
    );

    report
}

/// Bounds one case by the suite timeout.
async fn within<F>(case: &str, future: F) -> Result<(), String>
where
    F: Future<Output = Result<(), String>>,
{
    match tokio::time::timeout(CASE_TIMEOUT, future).await {
        Ok(result) => result,
        Err(_) => Err(format!(
            "the case `{case}` did not complete within {CASE_TIMEOUT:?}"
        )),
    }
}

/// Polls a future exactly once, with a waker that does nothing.
///
/// This is how the suite detects a full channel without a timing guess: a
/// send into a channel with room completes on the first poll, and a send
/// into a full channel does not.
fn poll_once<F: Future + ?Sized>(future: Pin<&mut F>) -> Poll<F::Output> {
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    future.poll(&mut context)
}

/// Fills a channel until a send stops completing on the first poll.
///
/// Returns how many messages the channel accepted before it applied
/// backpressure. The message that blocked is not sent: the probe future is
/// dropped while it is still pending, which is cancel-safe.
async fn probe_until_blocked<'a, F>(mut send: F) -> Result<usize, String>
where
    F: FnMut(u64) -> BoxFuture<'a, Result<(), LinkError>>,
{
    let mut accepted = 0usize;
    loop {
        let mut future = send(accepted as u64);
        match poll_once(future.as_mut()) {
            Poll::Ready(Ok(())) => {
                accepted += 1;
                if accepted > MAX_PROBE_MESSAGES {
                    return Err(
                        "the channel accepted an unbounded number of messages; it never applied \
                         backpressure"
                            .to_string(),
                    );
                }
            }
            Poll::Ready(Err(error)) => {
                return Err(format!("the probe send failed: {error}"));
            }
            Poll::Pending => return Ok(accepted),
        }
    }
}

/// A control message sent on one end arrives intact on the other.
async fn control_round_trip<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let message = sample_ping(1);
    sender
        .send_control(message.clone())
        .await
        .map_err(|error| format!("the send failed: {error}"))?;
    let received = receiver
        .recv_control()
        .await
        .map_err(|error| format!("the receive failed: {error}"))?;
    require!(
        received == message,
        "the received message differs from the sent one: {received:?} against {message:?}"
    );
    Ok(())
}

/// Control messages arrive in the order they were sent.
async fn control_ordering<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    for seq in 0..ORDERING_SAMPLE_COUNT as u64 {
        sender
            .send_control(sample_ping(seq))
            .await
            .map_err(|error| format!("the send of {seq} failed: {error}"))?;
    }
    for expected in 0..ORDERING_SAMPLE_COUNT as u64 {
        let received = receiver
            .recv_control()
            .await
            .map_err(|error| format!("the receive of {expected} failed: {error}"))?;
        require!(
            matches!(&received, ControlMessage::Ping(ping) if ping.seq == expected),
            "message {expected} arrived out of order: {received:?}"
        );
    }
    Ok(())
}

/// An audio frame arrives byte for byte.
async fn audio_round_trip<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let frame = sample_audio_frame(7);
    let payload = AudioPayload::Frame(frame.clone());
    sender
        .send_audio(payload.clone())
        .await
        .map_err(|error| format!("the send failed: {error}"))?;
    let received = receiver
        .recv_audio()
        .await
        .map_err(|error| format!("the receive failed: {error}"))?;
    require!(
        received == payload,
        "the received payload differs from the sent one: {received:?} against {payload:?}"
    );
    Ok(())
}

/// A JSON marker and a binary frame are told apart on the same channel.
async fn audio_payload_discrimination<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let frame = sample_audio_frame(9);
    let marker = sample_ptt_marker();
    sender
        .send_audio(AudioPayload::Frame(frame.clone()))
        .await
        .map_err(|error| format!("the frame send failed: {error}"))?;
    sender
        .send_audio(AudioPayload::Control(marker.clone()))
        .await
        .map_err(|error| format!("the marker send failed: {error}"))?;

    let first = receiver
        .recv_audio()
        .await
        .map_err(|error| format!("the first receive failed: {error}"))?;
    let second = receiver
        .recv_audio()
        .await
        .map_err(|error| format!("the second receive failed: {error}"))?;

    require!(
        matches!(&first, AudioPayload::Frame(received) if *received == frame),
        "the binary frame was not received as a frame: {first:?}"
    );
    require!(
        matches!(&second, AudioPayload::Control(received) if *received == marker),
        "the JSON marker was not received as a marker: {second:?}"
    );
    Ok(())
}

/// Audio payloads arrive in order, independently of the control channel.
async fn audio_ordering<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    for seq in 0..ORDERING_SAMPLE_COUNT as u32 {
        sender
            .send_audio(AudioPayload::Frame(sample_audio_frame(seq)))
            .await
            .map_err(|error| format!("the send of {seq} failed: {error}"))?;
    }
    for expected in 0..ORDERING_SAMPLE_COUNT as u32 {
        let received = receiver
            .recv_audio()
            .await
            .map_err(|error| format!("the receive of {expected} failed: {error}"))?;
        require!(
            matches!(&received, AudioPayload::Frame(frame) if frame.seq == expected),
            "audio frame {expected} arrived out of order: {received:?}"
        );
    }
    Ok(())
}

/// A full audio channel does not block a control send.
async fn control_independent_of_audio<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let accepted = probe_until_blocked(|seq| {
        sender.send_audio(AudioPayload::Frame(sample_audio_frame(seq as u32)))
    })
    .await?;
    require!(
        accepted >= 1,
        "the audio channel accepted no message, so nothing was actually blocked"
    );

    let message = sample_ping(4242);
    sender
        .send_control(message.clone())
        .await
        .map_err(|error| format!("a control send was blocked by a full audio channel: {error}"))?;
    let received = receiver
        .recv_control()
        .await
        .map_err(|error| format!("the receive failed: {error}"))?;
    require!(
        received == message,
        "the received control message differs from the sent one: {received:?}"
    );
    Ok(())
}

/// A send into a full channel awaits and is not dropped.
async fn backpressure<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let sender = Arc::new(sender);
    let receiver = Arc::new(receiver);

    let accepted = probe_until_blocked(|seq| sender.send_control(sample_ping(seq))).await?;
    require!(
        accepted >= 1,
        "the channel accepted no message, so nothing was actually blocked"
    );

    // This send cannot complete until the peer drains the channel.
    let witness = sample_ping(accepted as u64);
    let task_witness = witness.clone();
    let sending = Arc::clone(&sender);
    let handle = tokio::spawn(async move { sending.send_control(task_witness).await });

    let mut received = Vec::with_capacity(accepted + 1);
    for _ in 0..=accepted {
        received.push(
            receiver
                .recv_control()
                .await
                .map_err(|error| format!("the receive failed: {error}"))?,
        );
    }

    let send_result = handle
        .await
        .map_err(|error| format!("the sending task panicked: {error}"))?;
    send_result.map_err(|error| format!("the blocked send failed: {error}"))?;

    for (index, message) in received.iter().enumerate() {
        require!(
            matches!(message, ControlMessage::Ping(ping) if ping.seq == index as u64),
            "message {index} arrived out of order: {message:?}"
        );
    }
    let last = received.last().cloned();
    require!(
        matches!(&last, Some(ControlMessage::Ping(ping)) if ping.seq == accepted as u64),
        "the message that waited for room was dropped or reordered: {last:?}"
    );
    Ok(())
}

/// A send on a closed link returns `Closed`, never a panic.
async fn send_after_close<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (first, second) = make_pair();
    first
        .close(LinkCloseReason::Local)
        .await
        .map_err(|error| format!("the close failed: {error}"))?;

    let on_closed = first.send_control(sample_ping(1)).await;
    require!(
        on_closed.is_err(),
        "a send on a link that closed itself must fail, it returned {on_closed:?}"
    );
    let on_peer_closed = second.send_control(sample_ping(2)).await;
    require!(
        on_peer_closed.is_err(),
        "a send on a link whose peer closed must fail, it returned {on_peer_closed:?}"
    );
    Ok(())
}

/// A receive on a closed link returns `Closed`, never a panic.
async fn recv_after_close<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (first, _second) = make_pair();
    first
        .close(LinkCloseReason::Local)
        .await
        .map_err(|error| format!("the close failed: {error}"))?;

    let received = first.recv_control().await;
    require!(
        matches!(received, Err(LinkError::Closed(_))),
        "a receive on a closed link must report `Closed`, it returned {received:?}"
    );
    Ok(())
}

/// The receiver sees the peer's close and reports `LinkCloseReason::Peer`.
async fn peer_close_observed<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (first, second) = make_pair();
    first
        .close(LinkCloseReason::Local)
        .await
        .map_err(|error| format!("the close failed: {error}"))?;

    let received = second.recv_control().await;
    require!(
        matches!(received, Err(LinkError::Closed(LinkCloseReason::Peer))),
        "the peer's close was not observed as `Closed(Peer)`, it returned {received:?}"
    );
    require!(
        second.close_reason() == Some(LinkCloseReason::Peer),
        "the link did not report the peer's close, it reported {:?}",
        second.close_reason()
    );
    Ok(())
}

/// An unknown message type is skipped, and the next known message still
/// arrives.
async fn unknown_type_skipped<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let unknown = br#"{"type":"AMessageFromTheFuture","payload":{"nested":true}}"#;
    sender
        .send_control_bytes(unknown.to_vec())
        .await
        .map_err(|error| format!("the raw send failed: {error}"))?;

    let expected = sample_ping(31);
    sender
        .send_control(expected.clone())
        .await
        .map_err(|error| format!("the send failed: {error}"))?;

    let received = receiver
        .recv_control()
        .await
        .map_err(|error| format!("the receive failed: {error}"))?;
    require!(
        received == expected,
        "the unknown message was not skipped: the receiver got {received:?}"
    );
    Ok(())
}

/// A control message larger than the cap is rejected, not sent.
async fn oversized_message_rejected<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let oversized = ControlMessage::Req(Req {
        request_id: 1,
        command: "x".repeat(MAX_CONTROL_MESSAGE_BYTES + 1),
        args: serde_json::Value::Null,
    });
    let result = sender.send_control(oversized).await;
    require!(
        result.is_err(),
        "an oversized control message must be rejected, the send returned {result:?}"
    );

    let expected = sample_ping(5);
    sender
        .send_control(expected.clone())
        .await
        .map_err(|error| format!("the send failed: {error}"))?;
    let received = receiver
        .recv_control()
        .await
        .map_err(|error| format!("the receive failed: {error}"))?;
    require!(
        received == expected,
        "the oversized message was queued: the receiver got {received:?}"
    );
    Ok(())
}

/// Every protocol message type survives a trip through the link.
async fn all_protocol_types_round_trip<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    for sample in control_samples() {
        let (sender, receiver) = make_pair();
        sender
            .send_control(sample.message.clone())
            .await
            .map_err(|error| format!("the send of {} failed: {error}", sample.type_tag))?;
        let received = receiver
            .recv_control()
            .await
            .map_err(|error| format!("the receive of {} failed: {error}", sample.type_tag))?;
        require!(
            received == sample.message,
            "the message type {} did not survive the trip: {received:?}",
            sample.type_tag
        );
    }
    Ok(())
}

/// An oversized message received on the control channel terminates the link.
///
/// Remote-control spec §7.1: a message that exceeds the size cap MUST cause
/// immediate connection termination. The receive reports the close, and the
/// link stays closed.
async fn oversized_received_message_rejected<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let oversized = vec![0u8; MAX_CONTROL_MESSAGE_BYTES + 1];
    sender
        .send_control_bytes(oversized)
        .await
        .map_err(|error| format!("the raw send failed: {error}"))?;

    let received = receiver.recv_control().await;
    require!(
        matches!(received, Err(LinkError::Closed(_))),
        "an oversized control message must terminate the link, the receive returned {received:?}"
    );

    let after = sender.send_control(sample_ping(1)).await;
    require!(
        after.is_err(),
        "the link must stay terminated after an oversized message, the send returned {after:?}"
    );
    Ok(())
}

/// An unknown audio payload tag closes the audio socket.
///
/// Remote-control spec §7.4: a receiver MUST treat an unknown tag as a
/// protocol error and close the audio socket, because a mis-tagged frame
/// cannot be safely interpreted as either shape.
async fn close_on_unknown_audio_payload_tag<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let mut malformed = vec![UNKNOWN_AUDIO_PAYLOAD_TAG];
    malformed.extend_from_slice(&[0u8; 4]);
    sender
        .send_audio_bytes(malformed)
        .await
        .map_err(|error| format!("the raw send failed: {error}"))?;

    let received = receiver.recv_audio().await;
    require!(
        matches!(received, Err(LinkError::Closed(_))),
        "an unknown audio payload tag must close the audio socket, the receive returned {received:?}"
    );

    let after = sender
        .send_audio(AudioPayload::Frame(sample_audio_frame(1)))
        .await;
    require!(
        after.is_err(),
        "the link must stay closed after an unknown audio payload tag, the send returned {after:?}"
    );
    Ok(())
}

/// A protocol violation closes the link with its cause, not with a bare
/// "local" close.
///
/// A link that terminates because the peer broke the wire contract must say
/// so. A reader of the close reason has to be able to tell "the peer sent an
/// oversized message" from "we closed the link because the session ended":
/// the first is a protocol error worth logging loudly, the second is routine.
async fn protocol_violation_reports_its_cause<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();
    let oversized = vec![0u8; MAX_CONTROL_MESSAGE_BYTES + 1];
    sender
        .send_control_bytes(oversized)
        .await
        .map_err(|error| format!("the raw send failed: {error}"))?;

    let received = receiver.recv_control().await;
    require!(
        matches!(
            received,
            Err(LinkError::Closed(LinkCloseReason::Protocol(_)))
        ),
        "a protocol violation must close the link with its cause, the receive returned {received:?}"
    );
    require!(
        !matches!(receiver.close_reason(), Some(LinkCloseReason::Local)),
        "a protocol violation must not be reported as a deliberate close, it reported {:?}",
        receiver.close_reason()
    );
    Ok(())
}

/// A survivable protocol violation does not end the receive.
///
/// Remote-control spec §7.1: a message this build does not understand is
/// skipped, not fatal. The link must drop the offending message and keep
/// waiting for the next one — it must not report an error to the caller and
/// it must not close.
async fn skippable_violation_does_not_end_the_receive<L, F>(make_pair: &F) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();

    // A JSON object with no `"type"` tag: the receiver cannot tell what it
    // is, so the violation is survivable.
    let tagless = br#"{"seq":1}"#;
    sender
        .send_control_bytes(tagless.to_vec())
        .await
        .map_err(|error| format!("the raw send failed: {error}"))?;

    let expected = sample_ping(77);
    sender
        .send_control(expected.clone())
        .await
        .map_err(|error| format!("the send failed: {error}"))?;

    let received = receiver
        .recv_control()
        .await
        .map_err(|error| format!("the receive failed: {error}"))?;
    require!(
        received == expected,
        "a survivable violation must not end the receive; the receiver got {received:?}"
    );
    require!(
        !matches!(receiver.close_reason(), Some(LinkCloseReason::Protocol(_))),
        "a survivable violation must not close the link, it reported {:?}",
        receiver.close_reason()
    );
    Ok(())
}

/// A survivable violation on the audio channel does not end the receive.
///
/// The audio socket carries two payload shapes, told apart by a one-byte
/// tag. A JSON audio-control message with no `"type"` tag is a message this
/// build cannot classify — the same survivable violation the control channel
/// has — and the audio receive must skip it and keep waiting.
async fn skippable_audio_violation_does_not_end_the_receive<L, F>(
    make_pair: &F,
) -> Result<(), String>
where
    L: RawLink + 'static,
    F: Fn() -> (L, L),
{
    let (sender, receiver) = make_pair();

    // The JSON payload tag, followed by an object with no `"type"` tag.
    let mut tagless = vec![AUDIO_PAYLOAD_TAG_JSON];
    tagless.extend_from_slice(br#"{"seq":1}"#);
    sender
        .send_audio_bytes(tagless)
        .await
        .map_err(|error| format!("the raw send failed: {error}"))?;

    let expected = sample_audio_frame(13);
    sender
        .send_audio(AudioPayload::Frame(expected.clone()))
        .await
        .map_err(|error| format!("the send failed: {error}"))?;

    let received = receiver
        .recv_audio()
        .await
        .map_err(|error| format!("the receive failed: {error}"))?;
    require!(
        received == AudioPayload::Frame(expected),
        "a survivable violation on the audio channel must not end the receive; the receiver got \
         {received:?}"
    );
    require!(
        !matches!(receiver.close_reason(), Some(LinkCloseReason::Protocol(_))),
        "a survivable violation must not close the link, it reported {:?}",
        receiver.close_reason()
    );
    Ok(())
}

/// A deterministic audio frame.
fn sample_audio_frame(seq: u32) -> AudioFrame {
    AudioFrame {
        codec: vox_lib::services::remote::protocol::AUDIO_CODEC_PCM16,
        seq,
        epoch: 2,
        sample_offset: u64::from(seq) * 480,
        samples: vec![0, 1, -1, 32767, -32768],
    }
}

/// A deterministic PTT marker.
fn sample_ptt_marker() -> vox_lib::services::remote::protocol::AudioControlMessage {
    vox_lib::services::remote::protocol::AudioControlMessage::PttMarker(
        vox_lib::services::remote::protocol::PttMarker {
            action: vox_lib::services::remote::protocol::PttAction::Start,
            sample_offset: 4800,
            epoch: 2,
        },
    )
}
