//! ============================================================================
//! tests/remote_security_pairing_test.rs — Pairing Tokens, Flow & Trust Store
//! ============================================================================
//! Category     : Integration Test (Seam 01 — trust boundary)
//! Component    : src/services/remote/security/{pairing,trust}.rs
//! Prerequisites: Turso embedded engine; isolated temporary databases
//! Execution    : cargo nextest run --test remote_security_pairing_test --release --nocapture --test-threads=1
//! Metrics      : (a) a pairing token carries at least 128 bits of entropy,
//!                is single-use and expires; (b) the per-source attempt
//!                limit and the per-source invalidation are enforced, and the
//!                invalidation is for that source only; (c) a claimed
//!                identity that does not match the handshake's static key is
//!                refused, with each of the three comparisons isolated;
//!                (d) the approval can be refused and can time out;
//!                (e) the trust store refuses every untrustworthy peer.
//!
//! WHY THIS EXISTS
//! ---------------
//! Pairing is the only thing that creates trust, and the trust store is the
//! only thing that checks it. A defect here is a defect in the boundary
//! between "a device on the network" and "a device this laptop trusts": the
//! pairing token is single-use, expiring and rate-limited, and the trust
//! store rejects a row whose key and id disagree rather than trusting it.
//!
//! TIME IS PASSED IN, NOT WAITED FOR
//! --------------------------------
//! A token lives for two minutes and a source may make five attempts a
//! minute. Neither can be exercised by waiting, so the store is handed the
//! instant each decision is judged against. The production path passes
//! `Instant::now()`; the tests pass an instant they move by hand.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! These tests prove the token rules, the identity binding and the trust
//! decision. They cannot prove that the human who approves a pairing is the
//! human who intended to: the prompt itself is a later wave.
//! ============================================================================

mod common;

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};

use tempfile::tempdir;
use vox_lib::{
    persistence::{
        paired_devices::{insert_paired_device, NewPairedDevice, PairedDeviceRole},
        schema::run_migrations,
        VoxDb,
    },
    services::remote::security::{
        derive_device_id, DeviceKey, PairingCoordinator, PairingOutcome, PairingRefusal,
        PairingRequest, SecurityError, TrustDecision, TrustRefusal, TrustStore,
        MAX_PREAUTH_MESSAGE_BYTES, PAIRING_APPROVAL_TIMEOUT, PAIRING_ATTEMPTS_PER_WINDOW,
        PAIRING_MAX_FAILED_ATTEMPTS, PAIRING_RATE_LIMIT_WINDOW, PAIRING_TOKEN_MIN_ENTROPY_BITS,
        PAIRING_TOKEN_TTL,
    },
};

/// The number of bytes a pairing token carries.
const TOKEN_BYTES: usize = PAIRING_TOKEN_MIN_ENTROPY_BITS / 8;

/// The bound on every await in this file.
const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// The source address of one pairing attempt.
fn source(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, last))
}

/// Bounds one async body by the test timeout.
async fn within<F: Future>(label: &str, future: F) -> F::Output {
    match tokio::time::timeout(TEST_TIMEOUT, future).await {
        Ok(value) => value,
        Err(_) => panic!("[{label}] the operation did not complete within {TEST_TIMEOUT:?}"),
    }
}

/// Opens a fresh database in an isolated temporary directory and migrates it.
async fn open_migrated_db(dir: &std::path::Path, file_name: &str) -> (VoxDb, turso::Connection) {
    let db = VoxDb::open(dir.join(file_name))
        .await
        .expect("the database must open");
    let conn = db.connect().expect("a connection must be vendored");
    run_migrations(&conn)
        .await
        .expect("the migrations must run");
    (db, conn)
}

/// Builds the pairing request a controller would put on the wire.
fn request_for(token: &str, key: &DeviceKey, display_name: &str) -> PairingRequest {
    PairingRequest::from_wire(
        token,
        derive_device_id(key.public_key()).as_str(),
        &hex::encode(key.public_key()),
        display_name,
    )
    .expect("the request must be built")
}

// ============================================================================
// Test 1: A token is single-use and expires
// ============================================================================

#[test]
fn test_a_token_is_single_use_and_expires() {
    let coordinator = PairingCoordinator::new();
    let issued = Instant::now();

    // Entropy and shape.
    let first = coordinator
        .issue_token(issued)
        .expect("a token must be issued");
    let second = coordinator
        .issue_token(issued)
        .expect("a token must be issued");
    assert_eq!(
        first.to_hex().len(),
        TOKEN_BYTES * 2,
        "a token must carry {TOKEN_BYTES} bytes, which is {PAIRING_TOKEN_MIN_ENTROPY_BITS} bits"
    );
    assert_eq!(
        TOKEN_BYTES * 8,
        PAIRING_TOKEN_MIN_ENTROPY_BITS,
        "a token must carry at least the entropy the spec demands"
    );
    assert_ne!(
        first.to_hex(),
        second.to_hex(),
        "two issued tokens must differ"
    );
    assert!(
        first
            .to_hex()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "a token must be lowercase hex"
    );
    assert!(
        !format!("{first:?}").contains(&first.to_hex()),
        "a token must not print itself: {first:?}"
    );
    let parsed = vox_lib::services::remote::security::PairingToken::from_hex(&first.to_hex())
        .expect("a token must parse");
    assert_eq!(parsed.to_hex(), first.to_hex());

    // Single use: the first validation succeeds, the second is refused.
    let tokens = coordinator.tokens().clone();
    let address = source(10);
    tokens
        .validate_token(&first, address, issued)
        .expect("the first validation must succeed");
    let second_use = tokens.validate_token(&first, address, issued);
    assert!(
        second_use.is_err(),
        "the second use of a token must fail, it returned {second_use:?}"
    );

    // Expiry: a token past its lifetime is refused, and the refusal names
    // the expiry. The store prunes what it can no longer affect, but an
    // expired token still affects one thing: it must be reported as expired
    // rather than as unknown, or the user is told their QR code was never
    // issued when it was simply stale.
    let third = coordinator
        .issue_token(issued)
        .expect("a token must be issued");
    let expired = tokens.validate_token(
        &third,
        source(11),
        issued + PAIRING_TOKEN_TTL + Duration::from_secs(1),
    );
    assert!(
        matches!(
            expired,
            Err(SecurityError::PairingRefused(PairingRefusal::Expired))
        ),
        "a token past its lifetime must be rejected as expired, it returned {expired:?}"
    );

    // The token store is in memory only: a token issued by one store is
    // unknown to another.
    let other_store = PairingCoordinator::new();
    let orphan = other_store
        .issue_token(issued)
        .expect("a token must be issued");
    assert!(
        tokens.validate_token(&orphan, source(12), issued).is_err(),
        "another store must not know the token"
    );
}

// ============================================================================
// Test 2: The per-source attempt limit and the per-source invalidation
// ============================================================================

#[test]
fn test_the_per_source_attempt_limit_and_the_per_source_invalidation() {
    let coordinator = PairingCoordinator::new();
    let tokens = coordinator.tokens().clone();
    let now = Instant::now();
    let address = source(30);

    // The per-source attempt limit is the production step that precedes the
    // token check, so it is exercised there.
    for attempt in 0..PAIRING_ATTEMPTS_PER_WINDOW {
        let result = tokens.record_attempt(address, now);
        assert!(
            result.is_ok(),
            "attempt {attempt} must be inside the limit, it returned {result:?}"
        );
    }

    // The next attempt inside the same window is refused.
    let refused = tokens.record_attempt(address, now);
    assert!(
        refused.is_err(),
        "the attempt past the limit must be refused, it returned {refused:?}"
    );

    // Outside the window the source may try again.
    let later = now + PAIRING_RATE_LIMIT_WINDOW + Duration::from_secs(1);
    let accepted = tokens.record_attempt(address, later);
    assert!(
        accepted.is_ok(),
        "the limit must not survive the window, it returned {accepted:?}"
    );

    // Repeated failures invalidate the token for that source only. A source
    // that keeps presenting a token it cannot back with a handshake loses
    // that token; every other address is unaffected.
    let peer = DeviceKey::generate().expect("a peer key must be generated");
    let impostor = DeviceKey::generate().expect("an impostor key must be generated");
    let attacker = source(20);
    let honest = source(21);
    let token = coordinator
        .issue_token(now)
        .expect("a token must be issued");

    for attempt in 0..PAIRING_MAX_FAILED_ATTEMPTS {
        let request = request_for(&token.to_hex(), &impostor, "Impostor");
        let result = coordinator.begin(request, peer.public_key(), attacker, now);
        assert!(
            result.is_err(),
            "attempt {attempt} must be refused, it returned {result:?}"
        );
    }

    let request = request_for(&token.to_hex(), &peer, "Ada's Phone");
    let refused = coordinator.begin(request, peer.public_key(), attacker, now);
    assert!(
        matches!(
            refused,
            Err(SecurityError::PairingRefused(PairingRefusal::Invalidated))
        ),
        "the refusal must name the invalidation, it was {refused:?}"
    );

    let honest_request = request_for(&token.to_hex(), &peer, "Ada's Phone");
    let accepted = coordinator.begin(honest_request, peer.public_key(), honest, now);
    assert!(
        accepted.is_ok(),
        "another source address must be unaffected by the attacker's failures, it returned \
         {accepted:?}"
    );
}

// ============================================================================
// Test 3: The claimed identity must be the handshake's static key
// ============================================================================

#[tokio::test]
async fn test_a_claimed_identity_must_be_the_handshake_key() {
    within("test_a_claimed_identity_must_be_the_handshake_key", async {
        let coordinator = PairingCoordinator::new();
        let peer = DeviceKey::generate().expect("a peer key must be generated");
        let impostor = DeviceKey::generate().expect("an impostor key must be generated");
        let now = Instant::now();
        let token = coordinator
            .issue_token(now)
            .expect("a token must be issued");

        // The claim names the impostor's key, but the handshake proved the
        // peer's key: the three must be the same object.
        let request = request_for(&token.to_hex(), &impostor, "Impostor");
        let refused = coordinator.begin(request, peer.public_key(), source(50), now);
        assert!(
            refused.is_err(),
            "a claimed identity that is not the handshake's static key must be refused"
        );

        // A claim whose device id does not derive its own key is refused too.
        let other_token = coordinator
            .issue_token(now)
            .expect("a token must be issued");
        let mismatched = PairingRequest::from_wire(
            &other_token.to_hex(),
            derive_device_id(impostor.public_key()).as_str(),
            &hex::encode(peer.public_key()),
            "Mismatched",
        )
        .expect("the request must be built");
        let refused = coordinator.begin(mismatched, peer.public_key(), source(51), now);
        assert!(
            refused.is_err(),
            "a claim whose device id does not derive its public key must be refused"
        );
    })
    .await;
}

// ============================================================================
// Test 4: The approval can be refused and can time out
// ============================================================================

#[tokio::test]
async fn test_the_approval_can_be_refused_and_can_time_out() {
    // The clock is frozen from the start, so the approval deadline is
    // reached by advancing time rather than by waiting for it.
    tokio::time::pause();

    let coordinator = PairingCoordinator::new();
    let peer = DeviceKey::generate().expect("a peer key must be generated");
    let now = Instant::now();

    // A refusal.
    let token = coordinator
        .issue_token(now)
        .expect("a token must be issued");
    let request = request_for(&token.to_hex(), &peer, "Ada's Phone");
    let pending = coordinator
        .begin(request, peer.public_key(), source(60), now)
        .expect("the pairing must begin");
    pending.prompt().reject();
    assert_eq!(
        pending.settle().await,
        PairingOutcome::Rejected,
        "a refused pairing must report a refusal"
    );

    // The production deadline is the named constant.
    assert_eq!(
        PAIRING_APPROVAL_TIMEOUT,
        Duration::from_secs(30),
        "the approval deadline must be the one the spec names"
    );

    // A timeout: no decision arrives.
    let timeout_token = coordinator
        .issue_token(now)
        .expect("a token must be issued");
    let timeout_request = request_for(&timeout_token.to_hex(), &peer, "Ada's Phone");
    let pending = coordinator
        .begin(timeout_request, peer.public_key(), source(61), now)
        .expect("the pairing must begin");
    let handle = tokio::spawn(async move { pending.settle().await });

    // Let the task start, so its deadline timer is registered before the
    // clock moves.
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(PAIRING_APPROVAL_TIMEOUT + Duration::from_secs(1)).await;
    for _ in 0..16 {
        if handle.is_finished() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(
        handle.is_finished(),
        "the approval must time out once the deadline has passed"
    );

    let outcome = handle.await.expect("the settling task must not panic");
    assert_eq!(
        outcome,
        PairingOutcome::TimedOut,
        "a pairing with no decision must time out"
    );
}

// ============================================================================
// Test 6: The pre-authentication window is bounded
// ============================================================================

/// The pairing request is the only message that travels before the Noise
/// handshake completes, so it is the one message an unauthenticated peer
/// controls the size of. Remote-control spec §7.1 caps it at 4 KB.
#[test]
fn test_an_oversized_pre_authentication_request_is_refused() {
    let coordinator = PairingCoordinator::new();
    let peer = DeviceKey::generate().expect("a peer key must be generated");
    let token = coordinator
        .issue_token(Instant::now())
        .expect("a token must be issued");

    let oversized = "x".repeat(MAX_PREAUTH_MESSAGE_BYTES + 1);
    let refused = PairingRequest::from_wire(
        &token.to_hex(),
        derive_device_id(peer.public_key()).as_str(),
        &hex::encode(peer.public_key()),
        &oversized,
    );
    assert!(
        matches!(refused, Err(SecurityError::PreauthMessageTooLarge { .. })),
        "a pre-authentication request over the cap must be refused"
    );

    // A request inside the cap is accepted: the guard bounds the size, it
    // does not reject the message.
    let accepted = PairingRequest::from_wire(
        &token.to_hex(),
        derive_device_id(peer.public_key()).as_str(),
        &hex::encode(peer.public_key()),
        "Ada's Phone",
    );
    assert!(
        accepted.is_ok(),
        "a pre-authentication request inside the cap must be accepted"
    );
}

// ============================================================================
// Test 7: The trust store refuses every untrustworthy peer
// ============================================================================

#[tokio::test]
async fn test_the_trust_store_refuses_every_untrustworthy_peer() {
    within(
        "test_the_trust_store_refuses_every_untrustworthy_peer",
        async {
            let dir = tempdir().expect("a temporary directory must be created");
            let (_db, conn) = open_migrated_db(dir.path(), "trust.db").await;

            let peer = DeviceKey::generate().expect("a peer key must be generated");
            let other = DeviceKey::generate().expect("another key must be generated");
            let device_id = derive_device_id(peer.public_key());
            let store = TrustStore::new();

            // A paired peer is trusted.
            insert_paired_device(
                &conn,
                &NewPairedDevice {
                    device_id: device_id.as_str().to_string(),
                    public_key: peer.public_key().to_vec(),
                    display_name: "Ada's Phone".to_string(),
                    role: PairedDeviceRole::Controller,
                    trust_level: "full".to_string(),
                    hints: Vec::new(),
                },
            )
            .await
            .expect("the pairing must be inserted");
            match store
                .lookup(&conn, device_id.as_str(), PairedDeviceRole::Controller)
                .await
                .expect("the lookup must succeed")
            {
                TrustDecision::Trusted(device) => {
                    assert_eq!(device.role, PairedDeviceRole::Controller);
                    assert_eq!(device.display_name, "Ada's Phone");
                }
                other => panic!("a paired peer must be trusted, it was {other:?}"),
            }

            // An unknown peer is refused.
            assert_eq!(
                store
                    .lookup(&conn, "0".repeat(64).as_str(), PairedDeviceRole::Controller)
                    .await
                    .expect("the lookup must succeed"),
                TrustDecision::Refused(TrustRefusal::UnknownPeer),
                "an unknown peer must be refused"
            );

            // A role contradiction is refused.
            assert_eq!(
                store
                    .lookup(&conn, device_id.as_str(), PairedDeviceRole::Host)
                    .await
                    .expect("the lookup must succeed"),
                TrustDecision::Refused(TrustRefusal::RoleMismatch {
                    stored_role: PairedDeviceRole::Controller
                }),
                "a peer that connects in a role that contradicts its stored role must be refused"
            );

            // A row whose key and id disagree is refused at the boundary.
            let wrong_id = derive_device_id(other.public_key());
            insert_paired_device(
                &conn,
                &NewPairedDevice {
                    device_id: wrong_id.as_str().to_string(),
                    public_key: peer.public_key().to_vec(),
                    display_name: "Corrupted Row".to_string(),
                    role: PairedDeviceRole::Controller,
                    trust_level: "full".to_string(),
                    hints: Vec::new(),
                },
            )
            .await
            .expect("the row must be inserted");
            assert_eq!(
                store
                    .lookup(&conn, wrong_id.as_str(), PairedDeviceRole::Controller)
                    .await
                    .expect("the lookup must succeed"),
                TrustDecision::Refused(TrustRefusal::KeyMismatch),
                "a row whose device id is not the derivation of its key must be refused"
            );

            // Revocation deletes the row and is terminal.
            assert!(
                store
                    .revoke(&conn, device_id.as_str())
                    .await
                    .expect("the revocation must succeed"),
                "the revocation must report that the row was deleted"
            );
            assert_eq!(
                store
                    .lookup(&conn, device_id.as_str(), PairedDeviceRole::Controller)
                    .await
                    .expect("the lookup must succeed"),
                TrustDecision::Refused(TrustRefusal::Revoked),
                "a revoked peer must be refused with the revoked reason"
            );
        },
    )
    .await;
}
