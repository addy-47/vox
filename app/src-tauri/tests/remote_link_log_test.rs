//! ============================================================================
//! tests/remote_link_log_test.rs — The Skip-Not-Close Warning Is Emitted
//! ============================================================================
//! Category     : Integration Test (Seam 02 — in-process link, log observation)
//! Component    : src/services/remote/link/loopback.rs
//! Prerequisites: None (in-process link; no models, no services, no DB)
//! Execution    : cargo nextest run --test remote_link_log_test --release --nocapture --test-threads=1
//! Metrics      : the link emits a WARN-level log line naming the unknown
//!                message type when it skips it, and the receive continues.
//!
//! WHY THIS EXISTS
//! ---------------
//! Remote-control spec §7.1 (RAIL-RC-04) and ticket 07 both require two
//! separate things of the same branch: the receiver MUST log a warning, and
//! the receiver MUST skip the message and keep waiting. The "keeps waiting"
//! half is covered by `remote_link_test.rs` and by the conformance suite's
//! `unknown_type_skipped` case. The "logs a warning" half was covered by no
//! test, so a link that skipped the message silently would have passed every
//! gate.
//
//! The `log` facade is process-global, so this assertion needs its own test
//! binary: `tests/common/log_capture.rs` installs one capturing logger per
//! process.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! This proves the production branch emits the warning. It does not prove
//! anything about the log's destination, level filtering in a deployed build,
//! or the rate at which the warning is emitted.
//! ============================================================================

mod common;

use std::time::Duration;

use common::remote_samples::sample_ping;
use vox_lib::services::remote::link::{LoopbackLink, RawLink, RemoteLink};

/// The bound on the whole test.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// A message type this build does not know.
const UNKNOWN_TYPE: &str = "AMessageFromTheFuture";

#[tokio::test]
async fn test_the_link_logs_a_warning_when_it_skips_a_message_it_cannot_read() {
    tokio::time::timeout(TEST_TIMEOUT, async {
        common::log_capture::install();

        // Discard whatever was logged before the link under test ran.
        let _ = common::log_capture::take();

        let (sender, receiver) = LoopbackLink::pair();

        // The typed API cannot express an unknown type, so it is injected as
        // raw bytes — exactly as a newer peer would put it on the wire.
        let unknown = format!("{{\"type\":\"{UNKNOWN_TYPE}\",\"payload\":{{\"nested\":true}}}}");
        sender
            .send_control_bytes(unknown.into_bytes())
            .await
            .expect("the raw send must succeed");

        // A message with no type tag at all. The receiver cannot tell what
        // it is, so it survives — but it must still be reported.
        sender
            .send_control_bytes(br#"{"seq":1}"#.to_vec())
            .await
            .expect("the raw send must succeed");

        let expected = sample_ping(23);
        sender
            .send_control(expected.clone())
            .await
            .expect("the send must succeed");

        // The receive must continue past both skipped messages.
        let received = receiver
            .recv_control()
            .await
            .expect("the receive must yield the next known message");
        assert_eq!(
            received, expected,
            "the skipped messages must be skipped and the next known one delivered"
        );

        let captured = common::log_capture::take_text();
        assert!(
            captured.contains("[WARN]"),
            "the skip must be logged at WARN level. Captured:\n{captured}"
        );
        assert!(
            captured.contains(UNKNOWN_TYPE),
            "the warning must name the skipped type `{UNKNOWN_TYPE}`. Captured:\n{captured}"
        );
        assert!(
            captured.contains("survivable"),
            "a skipped violation must be reported as survivable. Captured:\n{captured}"
        );
    })
    .await
    .expect("the test must complete within its timeout");
}
