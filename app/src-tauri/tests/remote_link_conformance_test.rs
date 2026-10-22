//! ============================================================================
//! tests/remote_link_conformance_test.rs — Conformance Suite Run
//! ============================================================================
//! Category     : Integration Test (Seam 02 — conformance gate)
//! Component    : tests/common/remote_conformance.rs + src/services/remote/link/
//! Prerequisites: None (in-process link; no models, no services, no DB)
//! Execution    : cargo nextest run --test remote_link_conformance_test --release --nocapture --test-threads=1
//! Metrics      : (a) the in-process link passes every named conformance
//!                case, asserted case by case; (b) the report names every
//!                case that ran; (c) the suite itself names no link
//!                implementation, so a new transport cannot be added without
//!                proving it behaves like a link.
//!
//! WHY THIS EXISTS
//! ---------------
//! Remote-control spec VERIFY-RC-15: every `RemoteLink` implementation
//! passes the conformance suite, and a failure blocks the ticket. The suite
//! is generic over the link type and names no implementation, so the second
//! transport — the WebSocket link of wave 2 — cannot be wired in without
//! proving it behaves like a link.
//!
//! The source check in Test 3 is not decoration. A reference to the
//! implementation inside the suite is easy to add by accident, invisible in
//! the output, and it turns a conformance suite into a test of one
//! implementation.
//! ============================================================================

mod common;

use std::path::{Path, PathBuf};

use common::remote_conformance::run_link_conformance;
use vox_lib::services::remote::link::LoopbackLink;

/// Every case the conformance suite must run.
const EXPECTED_CASES: [&str; 18] = [
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

/// Collects every `.rs` file under `dir`.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

// ============================================================================
// Test 1: The in-process link passes every conformance case
// ============================================================================

#[tokio::test]
async fn test_the_loopback_link_passes_every_conformance_case() {
    let report = run_link_conformance(LoopbackLink::pair).await;

    for case in EXPECTED_CASES {
        assert!(
            report.passed(case),
            "the conformance case `{case}` failed: {:?}",
            report.failure_reason(case)
        );
    }
    assert_eq!(
        report.ran().len(),
        EXPECTED_CASES.len(),
        "the suite must run every case exactly once; it ran {:?}",
        report.ran()
    );
    assert!(
        report.failures().is_empty(),
        "the conformance run reported failures: {:?}",
        report.failures()
    );
}

// ============================================================================
// Test 2: The suite names no link implementation
// ============================================================================

#[test]
fn test_the_conformance_suite_names_no_implementation() {
    let common_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("common");

    let mut files: Vec<PathBuf> = Vec::new();
    collect_rs_files(&common_dir, &mut files);
    assert!(
        !files.is_empty(),
        "the shared test-infrastructure tree must contain source files"
    );

    for file in &files {
        let source = std::fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("{:?} must be readable: {error}", file.display()));
        let lowered = source.to_lowercase();

        // The implementation type name must not appear anywhere in the
        // shared test infrastructure.
        assert!(
            !lowered.contains("loopbacklink"),
            "the shared test infrastructure must not name a link implementation; \
             {:?} mentions `LoopbackLink`",
            file.display()
        );
    }

    // The suite must still be written against the trait, and must still
    // expose a generic entry point.
    let suite_path = common_dir.join("remote_conformance.rs");
    let suite = std::fs::read_to_string(&suite_path)
        .unwrap_or_else(|error| panic!("the conformance suite must be readable: {error}"));
    assert!(
        suite.contains("RawLink"),
        "the conformance suite must be written against the link trait"
    );
    assert!(
        suite.contains("pub async fn run_link_conformance"),
        "the conformance suite must expose a generic entry point"
    );
}
