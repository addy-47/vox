//! ============================================================================
//! tests/remote_security_test.rs — Remote Security & Trust Seam (Seam 01)
//! ============================================================================
//! Category     : Integration Test (Seam 01 — security, noise & trust)
//! Component    : src/services/remote/security/{noise,pairing,trust,identity,key_store}.rs
//! Binary       : remote_security_test
//! Prerequisites: Turso embedded engine; isolated temporary databases
//! Run          : cargo nextest run --test remote_security_test --release --nocapture --test-threads=1
//! ============================================================================

mod common;

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};

use common::remote_noise::{completed_transport, drive_handshake, peer_pair, try_drive_handshake};
use tempfile::tempdir;
use vox_lib::{
    persistence::{
        paired_devices::{insert_paired_device, NewPairedDevice, PairedDeviceRole},
        schema::run_migrations,
        VoxDb,
    },
    services::remote::{
        security::{
            default_noise_prologue, derive_device_id, noise_prologue, DeviceKey, NoiseHandshake,
            PairingCoordinator, PairingOutcome, PairingRefusal, PairingRequest, SecurityError,
            TrustDecision, TrustRefusal, TrustStore, MAX_PREAUTH_MESSAGE_BYTES,
            NOISE_MAX_HANDSHAKE_MESSAGE_BYTES, NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES,
            PAIRING_APPROVAL_TIMEOUT, PAIRING_ATTEMPTS_PER_WINDOW, PAIRING_MAX_FAILED_ATTEMPTS,
            PAIRING_RATE_LIMIT_WINDOW, PAIRING_TOKEN_MIN_ENTROPY_BITS, PAIRING_TOKEN_TTL,
            X25519_KEY_BYTES,
        },
        MAX_CONTROL_MESSAGE_BYTES,
    },
};

/// The size of one uplink audio frame in bytes.
const AUDIO_FRAME_BYTES: usize = 320;

/// The size of a large payload in bytes.
const LARGE_PAYLOAD_BYTES: usize = 60 * 1024;

/// Number of handshake messages produced by Noise_XX.
const EXPECTED_HANDSHAKE_MESSAGES: usize = 3;

/// Number of handshake messages written by the initiator.
const EXPECTED_INITIATOR_MESSAGES: usize = 2;

/// Size of the Noise Poly1305 authentication tag.
const NOISE_TAG_BYTES: usize = 16;

/// Largest Noise message size.
const NOISE_MAX_MESSAGE_BYTES: usize = 65535;

/// Number of bytes a pairing token carries.
const TOKEN_BYTES: usize = PAIRING_TOKEN_MIN_ENTROPY_BITS / 8;

/// Bound on async operations.
const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Builds an IPv4 test address.
fn source(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, last))
}

/// Bounds an async future with TEST_TIMEOUT.
async fn within<F: Future>(label: &str, future: F) -> F::Output {
    match tokio::time::timeout(TEST_TIMEOUT, future).await {
        Ok(value) => value,
        Err(_) => panic!("[{label}] operation did not complete within {TEST_TIMEOUT:?}"),
    }
}

/// Opens an isolated migrated database.
async fn open_migrated_db(dir: &std::path::Path, file_name: &str) -> (VoxDb, turso::Connection) {
    let db = VoxDb::open(dir.join(file_name))
        .await
        .expect("database open failed");
    let conn = db.connect().expect("connection vend failed");
    run_migrations(&conn).await.expect("migrations failed");
    (db, conn)
}

/// Builds a valid pairing request for a given key.
fn request_for(token: &str, key: &DeviceKey, display_name: &str) -> PairingRequest {
    PairingRequest::from_wire(
        token,
        derive_device_id(key.public_key()).as_str(),
        &hex::encode(key.public_key()),
        display_name,
    )
    .expect("request build failed")
}

/// Helper to check if a byte slice contains a sub-slice.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

// ============================================================================
// Noise Handshake & Transport Subtests
// ============================================================================

/// PURPOSE:  A Noise_XX handshake between two honest peers must complete and yield working transports.
/// SCENARIO: Drive 3-message Noise_XX handshake with default prologue, then encrypt/decrypt round-trip.
/// ASSERTS:  Handshake completes; static keys match peer identities; payload decrypts cleanly.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Omit session cipher state transition in NoiseHandshake::finish.
#[test]
fn handshake_completes_and_both_peers_obtain_transport() {
    let (initiator_key, responder_key) = peer_pair();
    let prologue = default_noise_prologue();

    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("initiator build");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("responder build");

    assert!(initiator.is_my_turn());
    assert!(!responder.is_my_turn());

    let trace = drive_handshake(&mut initiator, &mut responder);
    assert!(initiator.is_complete());
    assert!(responder.is_complete());
    assert_eq!(trace.messages.len(), EXPECTED_HANDSHAKE_MESSAGES);
    assert_eq!(trace.initiator_messages, EXPECTED_INITIATOR_MESSAGES);

    let seen_by_initiator = initiator.remote_static().expect("remote static");
    let seen_by_responder = responder.remote_static().expect("remote static");
    assert_eq!(seen_by_initiator.as_slice(), responder_key.public_key());
    assert_eq!(seen_by_responder.as_slice(), initiator_key.public_key());
    assert_eq!(seen_by_initiator.len(), X25519_KEY_BYTES);

    let mut initiator_transport = initiator.finish().expect("initiator finish");
    let mut responder_transport = responder.finish().expect("responder finish");

    let payload = b"vox-remote-control".to_vec();
    let encrypted = initiator_transport.encrypt(&payload).expect("encrypt");
    let decrypted = responder_transport.decrypt(&encrypted).expect("decrypt");
    assert_eq!(decrypted, payload);
}

/// PURPOSE:  An initiator pinning an expected responder key must refuse an impostor before sending its static key.
/// SCENARIO: Initiator pins impostor key while talking to real responder; exchange messages.
/// ASSERTS:  Initiator fails on 2nd message, sends only 1 message, and finish() returns error.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Ignore remote static verification against pinned key in NoiseHandshake::read_message.
#[test]
fn handshake_against_impostor_fails_before_revealing_static_key() {
    let (initiator_key, responder_key) = peer_pair();
    let (impostor_key, _) = peer_pair();
    let prologue = default_noise_prologue();

    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(impostor_key.public_key()), &prologue)
            .expect("initiator build");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("responder build");

    assert_eq!(initiator.remote_static(), None);

    let (trace, failed) = try_drive_handshake(&mut initiator, &mut responder);
    assert!(failed);
    assert_eq!(trace.initiator_messages, 1);
    assert!(initiator.finish().is_err());
}

/// PURPOSE:  The prologue must be cryptographically bound into the handshake without appearing on the wire.
/// SCENARIO: Drive handshake with prologue; inspect wire messages; then pair peers with mismatched prologues.
/// ASSERTS:  Wire messages do not contain prologue bytes; mismatched prologue fails handshake.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Pass empty prologue bytes to snow builder.
#[test]
fn prologue_is_authenticated_and_not_transmitted() {
    let (initiator_key, responder_key) = peer_pair();
    let prologue = default_noise_prologue();

    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("initiator build");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("responder build");

    let trace = drive_handshake(&mut initiator, &mut responder);
    assert!(!prologue.is_empty());
    for message in &trace.messages {
        assert!(!contains_bytes(message, &prologue));
    }

    let current = noise_prologue(1);
    let other = noise_prologue(2);
    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &current)
            .expect("initiator build");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &other)
            .expect("responder build");

    let (_trace, failed) = try_drive_handshake(&mut initiator, &mut responder);
    assert!(failed);
}

/// PURPOSE:  Tampered or truncated encrypted messages must fail authentication tag verification.
/// SCENARIO: Flip bits in ciphertext, truncate ciphertext, and attempt decryption.
/// ASSERTS:  Transport returns decryption error on corrupted, truncated, and undersized buffers.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Bypass authentication tag check and return plaintext slice on error.
#[test]
fn corrupted_or_truncated_message_fails_decryption() {
    let (initiator_key, responder_key) = peer_pair();
    let (mut initiator_transport, mut responder_transport) =
        completed_transport(&initiator_key, &responder_key);

    let payload = b"the quick brown fox".to_vec();
    let encrypted = initiator_transport.encrypt(&payload).expect("encrypt");

    let mut corrupted = encrypted.clone();
    let middle = corrupted.len() / 2;
    corrupted[middle] ^= 0xFF;
    assert!(responder_transport.decrypt(&corrupted).is_err());

    let truncated = &encrypted[..encrypted.len() - 1];
    assert!(responder_transport.decrypt(truncated).is_err());

    assert!(responder_transport.decrypt(&[0u8; 4]).is_err());

    let decrypted = responder_transport
        .decrypt(&encrypted)
        .expect("decrypt valid");
    assert_eq!(decrypted, payload);
}

/// PURPOSE:  Transport encryption must handle diverse payload sizes up to the limit and reject over-cap buffers.
/// SCENARIO: Encrypt/decrypt buffers from 0 to 65519 bytes; attempt encrypt on 65535-byte payload.
/// ASSERTS:  Payloads up to max capacity succeed; payload leaving no room for 16-byte tag fails.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Disable size check allowing payload to overflow 16-bit wire length.
#[test]
fn payload_near_maximum_size_roundtrips_and_over_cap_fails() {
    let (initiator_key, responder_key) = peer_pair();
    let (mut initiator_transport, mut responder_transport) =
        completed_transport(&initiator_key, &responder_key);

    for size in [0usize, 1, AUDIO_FRAME_BYTES, LARGE_PAYLOAD_BYTES] {
        let payload: Vec<u8> = (0..size).map(|index| (index % 251) as u8).collect();
        let encrypted = initiator_transport.encrypt(&payload).expect("encrypt");
        assert_eq!(encrypted.len(), payload.len() + NOISE_TAG_BYTES);
        let decrypted = responder_transport.decrypt(&encrypted).expect("decrypt");
        assert_eq!(decrypted, payload);
    }

    let largest = vec![0u8; NOISE_MAX_MESSAGE_BYTES - NOISE_TAG_BYTES];
    assert!(initiator_transport.encrypt(&largest).is_ok());

    let too_large = vec![0u8; NOISE_MAX_MESSAGE_BYTES];
    assert!(initiator_transport.encrypt(&too_large).is_err());
}

/// PURPOSE:  Handshake frames exceeding NOISE_MAX_HANDSHAKE_MESSAGE_BYTES must be rejected early.
/// SCENARIO: Pass handshake buffer of NOISE_MAX_HANDSHAKE_MESSAGE_BYTES + 1 to read_message.
/// ASSERTS:  Refused with byte limit error without executing cipher decode.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Remove pre-allocation size check in NoiseHandshake::read_message.
#[test]
fn oversized_handshake_message_is_refused() {
    let key = DeviceKey::generate().expect("key generate");
    let prologue = default_noise_prologue();
    let mut handshake = NoiseHandshake::initiator(&key, None, &prologue).expect("handshake build");

    let oversized = vec![0u8; NOISE_MAX_HANDSHAKE_MESSAGE_BYTES + 1];
    assert!(handshake.read_message(&oversized).is_err());

    let mut handshake = NoiseHandshake::initiator(&key, None, &prologue).expect("handshake build");
    let refused = handshake.write_message(&[0u8; NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES + 1]);
    assert!(refused.is_err());
}

/// PURPOSE:  A valid pattern message padded past the cap must be rejected by the cap without poisoning state.
/// SCENARIO: Pad a valid handshake message past the cap; send to responder; then complete handshake.
/// ASSERTS:  Responder rejects over-cap message with byte limit error; subsequent valid handshake succeeds.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Mutate handshake state machine on rejected message before checking length.
#[test]
fn valid_handshake_message_over_cap_refused_without_poisoning_state() {
    let (initiator_key, responder_key) = peer_pair();
    let prologue = default_noise_prologue();
    let mut oversized_writer =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("initiator build");

    let mut oversized = oversized_writer.write_message(&[]).expect("write message");
    oversized.resize(NOISE_MAX_HANDSHAKE_MESSAGE_BYTES + 1, 0);

    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("responder build");
    let refused = responder.read_message(&oversized);
    assert!(matches!(
        refused,
        Err(SecurityError::Noise(ref message)) if message.contains("byte limit")
    ));

    // Responder state remains clean
    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("initiator build");
    let first = initiator.write_message(&[]).expect("write 1");
    responder.read_message(&first).expect("read 1");
    let second = responder.write_message(&[]).expect("write 2");
    initiator.read_message(&second).expect("read 2");
    let third = initiator.write_message(&[]).expect("write 3");
    responder.read_message(&third).expect("read 3");
    assert!(responder.finish().is_ok());
}

/// PURPOSE:  A peer with a mismatched protocol version must not establish a session transport.
/// SCENARIO: Run handshake with version 1 prologue against version 2 prologue.
/// ASSERTS:  Handshake fails and neither initiator nor responder can finish into transport mode.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Allow finish() to return default cipher state when handshake failed.
#[test]
fn version_mismatch_fails_handshake_without_transport() {
    let (initiator_key, responder_key) = peer_pair();
    let current = noise_prologue(1);
    let other = noise_prologue(2);

    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &current)
            .expect("initiator build");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &other)
            .expect("responder build");

    let (_trace, failed) = try_drive_handshake(&mut initiator, &mut responder);
    assert!(failed);
    assert!(initiator.finish().is_err());
    assert!(responder.finish().is_err());
}

/// PURPOSE:  The remote static key reported by the handshake must be the key revealed by the peer, never the pin.
/// SCENARIO: Drive handshake where initiator pins impostor key; observe remote_static after message 2.
/// ASSERTS:  Revealed key matches responder's public key, not the pinned impostor key.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Return pinned key from remote_static() instead of extracting state from dh.
#[test]
fn pin_is_never_reported_as_remote_static_key() {
    let (initiator_key, responder_key) = peer_pair();
    let (impostor_key, _) = peer_pair();
    let prologue = default_noise_prologue();

    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(impostor_key.public_key()), &prologue)
            .expect("initiator build");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("responder build");

    let first = initiator.write_message(&[]).expect("write 1");
    responder.read_message(&first).expect("read 1");
    let second = responder.write_message(&[]).expect("write 2");
    let _ = initiator.read_message(&second);

    let revealed = initiator.remote_static().expect("revealed static key");
    assert_eq!(revealed.as_slice(), responder_key.public_key());
    assert_ne!(revealed.as_slice(), impostor_key.public_key());
    assert!(initiator.finish().is_err());
}

/// PURPOSE:  The maximum post-auth control message must fit through the Noise transport without exceeding caps.
/// SCENARIO: Encrypt a payload of exactly MAX_CONTROL_MESSAGE_BYTES through completed transport.
/// ASSERTS:  Payload encrypts with 16-byte tag, decrypts completely, and does not drift from control cap.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Set MAX_CONTROL_MESSAGE_BYTES to 65536 while Noise payload cap is 65519.
#[test]
fn largest_control_message_fits_encrypted_transport() {
    let (initiator_key, responder_key) = peer_pair();
    let (mut initiator_transport, mut responder_transport) =
        completed_transport(&initiator_key, &responder_key);

    let payload = vec![0u8; MAX_CONTROL_MESSAGE_BYTES];
    let encrypted = initiator_transport
        .encrypt(&payload)
        .expect("encrypt max payload");
    assert_eq!(encrypted.len(), MAX_CONTROL_MESSAGE_BYTES + NOISE_TAG_BYTES);
    let decrypted = responder_transport
        .decrypt(&encrypted)
        .expect("decrypt max payload");
    assert_eq!(decrypted.len(), MAX_CONTROL_MESSAGE_BYTES);
}

// ============================================================================
// Pairing Coordinator, Tokens & Trust Store Subtests
// ============================================================================

/// PURPOSE:  Pairing tokens must carry at least 128 bits entropy, be single-use, and expire after TTL.
/// SCENARIO: Issue token; validate once; validate twice; then validate another token after TTL.
/// ASSERTS:  Tokens are 32 hex chars; 1st validation ok; 2nd validation fails; expired token reports Expired.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Omit removal of token from active token set upon validation.
#[test]
fn token_is_single_use_and_expires_after_ttl() {
    let coordinator = PairingCoordinator::new();
    let issued = Instant::now();

    let first = coordinator.issue_token(issued).expect("issue token");
    let second = coordinator.issue_token(issued).expect("issue token");
    assert_eq!(first.to_hex().len(), TOKEN_BYTES * 2);
    assert_ne!(first.to_hex(), second.to_hex());

    let tokens = coordinator.tokens().clone();
    let address = source(10);
    tokens
        .validate_token(&first, address, issued)
        .expect("validate 1");
    assert!(tokens.validate_token(&first, address, issued).is_err());

    let third = coordinator.issue_token(issued).expect("issue token");
    let expired = tokens.validate_token(
        &third,
        source(11),
        issued + PAIRING_TOKEN_TTL + Duration::from_secs(1),
    );
    assert!(matches!(
        expired,
        Err(SecurityError::PairingRefused(PairingRefusal::Expired))
    ));
}

/// PURPOSE:  A pairing token at the exact boundary must be accepted, and expired 1 nanosecond later.
/// SCENARIO: Validate token at issued + PAIRING_TOKEN_TTL and at issued + PAIRING_TOKEN_TTL + 1ns.
/// ASSERTS:  Exact boundary succeeds; 1ns past boundary returns Err.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Change `elapsed > PAIRING_TOKEN_TTL` to `elapsed >= PAIRING_TOKEN_TTL`.
#[test]
fn token_at_exact_lifetime_boundary_is_accepted_and_nanosecond_past_expires() {
    let coordinator = PairingCoordinator::new();
    let issued = Instant::now();
    let token = coordinator.issue_token(issued).expect("issue token");
    let tokens = coordinator.tokens().clone();

    assert!(tokens
        .validate_token(&token, source(91), issued + PAIRING_TOKEN_TTL)
        .is_ok());

    let expired_token = coordinator.issue_token(issued).expect("issue token");
    assert!(tokens
        .validate_token(
            &expired_token,
            source(92),
            issued + PAIRING_TOKEN_TTL + Duration::from_nanos(1)
        )
        .is_err());
}

/// PURPOSE:  Rate limiting must reject excessive pairing attempts and invalidate tokens for offending source.
/// SCENARIO: Trigger PAIRING_ATTEMPTS_PER_WINDOW attempts; exceed limit; then fail auth to invalidate token.
/// ASSERTS:  Attempts past limit are refused; attacker source is invalidated; honest source is unaffected.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Reset failed attempt counter on mismatch rather than incrementing.
#[test]
fn per_source_attempt_limit_and_invalidation_enforced() {
    let coordinator = PairingCoordinator::new();
    let tokens = coordinator.tokens().clone();
    let now = Instant::now();
    let address = source(30);

    for attempt in 0..PAIRING_ATTEMPTS_PER_WINDOW {
        assert!(
            tokens.record_attempt(address, now).is_ok(),
            "attempt {attempt}"
        );
    }
    assert!(tokens.record_attempt(address, now).is_err());

    let later = now + PAIRING_RATE_LIMIT_WINDOW + Duration::from_secs(1);
    assert!(tokens.record_attempt(address, later).is_ok());

    let peer = DeviceKey::generate().expect("peer key");
    let impostor = DeviceKey::generate().expect("impostor key");
    let attacker = source(20);
    let honest = source(21);
    let token = coordinator.issue_token(now).expect("token");

    for _ in 0..PAIRING_MAX_FAILED_ATTEMPTS {
        let req = request_for(&token.to_hex(), &impostor, "Impostor");
        assert!(coordinator
            .begin(req, peer.public_key(), attacker, now)
            .is_err());
    }

    let req = request_for(&token.to_hex(), &peer, "Ada's Phone");
    let refused = coordinator.begin(req, peer.public_key(), attacker, now);
    assert!(matches!(
        refused,
        Err(SecurityError::PairingRefused(PairingRefusal::Invalidated))
    ));

    let honest_req = request_for(&token.to_hex(), &peer, "Ada's Phone");
    assert!(coordinator
        .begin(honest_req, peer.public_key(), honest, now)
        .is_ok());
}

/// PURPOSE:  A pairing request where the claimed key or id contradicts the handshake static key must fail.
/// SCENARIO: Submit pairing request claiming impostor key or mismatched device id.
/// ASSERTS:  coordinator.begin returns SecurityError::PairingRefused on key or id contradiction.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Accept pairing request without checking claimed key against handshake static key.
#[tokio::test]
async fn claimed_identity_must_match_handshake_static_key() {
    within("claimed_identity_must_match_handshake_static_key", async {
        let coordinator = PairingCoordinator::new();
        let peer = DeviceKey::generate().expect("peer key");
        let impostor = DeviceKey::generate().expect("impostor key");
        let now = Instant::now();
        let token = coordinator.issue_token(now).expect("token");

        let req = request_for(&token.to_hex(), &impostor, "Impostor");
        assert!(coordinator
            .begin(req, peer.public_key(), source(50), now)
            .is_err());

        let other_token = coordinator.issue_token(now).expect("token");
        let mismatched = PairingRequest::from_wire(
            &other_token.to_hex(),
            derive_device_id(impostor.public_key()).as_str(),
            &hex::encode(peer.public_key()),
            "Mismatched",
        )
        .expect("build");
        assert!(coordinator
            .begin(mismatched, peer.public_key(), source(51), now)
            .is_err());
    })
    .await;
}

/// PURPOSE:  The pairing coordinator prompt must transition to Rejected when the user declines pairing.
/// SCENARIO: Begin pairing, invoke prompt.reject(), and await pending.settle().
/// ASSERTS:  settle() resolves to PairingOutcome::Rejected.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Force prompt.reject() to transition state to Approved.
#[tokio::test]
async fn approval_prompt_can_be_rejected() {
    let coordinator = PairingCoordinator::new();
    let peer = DeviceKey::generate().expect("peer key");
    let now = Instant::now();

    let token = coordinator.issue_token(now).expect("token");
    let request = request_for(&token.to_hex(), &peer, "Ada's Phone");
    let pending = coordinator
        .begin(request, peer.public_key(), source(60), now)
        .expect("begin");
    pending.prompt().reject();
    assert_eq!(pending.settle().await, PairingOutcome::Rejected);
}

/// PURPOSE:  A pairing prompt with no user interaction must cleanly time out after 30 seconds.
/// SCENARIO: Begin pairing under paused Tokio clock; advance clock past PAIRING_APPROVAL_TIMEOUT.
/// ASSERTS:  settle() resolves to PairingOutcome::TimedOut deterministically without scheduler spins.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Hardcode approval timeout to Duration::MAX.
#[tokio::test]
async fn approval_prompt_times_out_deterministically() {
    tokio::time::pause();

    let coordinator = PairingCoordinator::new();
    let peer = DeviceKey::generate().expect("peer key");
    let now = Instant::now();

    let token = coordinator.issue_token(now).expect("token");
    let request = request_for(&token.to_hex(), &peer, "Ada's Phone");
    let pending = coordinator
        .begin(request, peer.public_key(), source(61), now)
        .expect("begin");

    let handle = tokio::spawn(async move { pending.settle().await });

    // Deterministically advance virtual time past the timeout
    tokio::time::advance(PAIRING_APPROVAL_TIMEOUT + Duration::from_secs(1)).await;

    let outcome = handle.await.expect("settle task completed");
    assert_eq!(outcome, PairingOutcome::TimedOut);
}

/// PURPOSE:  Unauthenticated pre-authentication pairing requests exceeding 4 KB must be refused.
/// SCENARIO: Construct pairing request with payload length > MAX_PREAUTH_MESSAGE_BYTES.
/// ASSERTS:  from_wire returns Err(SecurityError::PreauthMessageTooLarge).
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Disable size check in PairingRequest::from_wire.
#[test]
fn oversized_pre_authentication_request_is_refused() {
    let coordinator = PairingCoordinator::new();
    let peer = DeviceKey::generate().expect("peer key");
    let token = coordinator.issue_token(Instant::now()).expect("token");

    let oversized = "x".repeat(MAX_PREAUTH_MESSAGE_BYTES + 1);
    let refused = PairingRequest::from_wire(
        &token.to_hex(),
        derive_device_id(peer.public_key()).as_str(),
        &hex::encode(peer.public_key()),
        &oversized,
    );
    assert!(matches!(
        refused,
        Err(SecurityError::PreauthMessageTooLarge { .. })
    ));
}

/// PURPOSE:  Trust store must validate paired peers, reject unknown/mismatched peers, and enforce revocation.
/// SCENARIO: Query TrustStore with paired peer, unknown id, role contradiction, corrupted id, and after revoke.
/// ASSERTS:  Trusted on match; Refused(UnknownPeer); Refused(RoleMismatch); Refused(KeyMismatch); Refused(Revoked).
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Omit key derivation check in TrustStore::lookup.
#[tokio::test]
async fn trust_store_evaluates_peer_validity_and_revocation() {
    within(
        "trust_store_evaluates_peer_validity_and_revocation",
        async {
            let dir = tempdir().expect("tempdir");
            let (_db, conn) = open_migrated_db(dir.path(), "trust.db").await;

            let peer = DeviceKey::generate().expect("peer key");
            let other = DeviceKey::generate().expect("other key");
            let device_id = derive_device_id(peer.public_key());
            let store = TrustStore::new();

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
            .expect("insert paired");

            match store
                .lookup(&conn, device_id.as_str(), PairedDeviceRole::Controller)
                .await
                .expect("lookup")
            {
                TrustDecision::Trusted(device) => {
                    assert_eq!(device.role, PairedDeviceRole::Controller);
                    assert_eq!(device.display_name, "Ada's Phone");
                }
                other => panic!("expected trusted, got {other:?}"),
            }

            assert_eq!(
                store
                    .lookup(&conn, &"0".repeat(64), PairedDeviceRole::Controller)
                    .await
                    .expect("lookup"),
                TrustDecision::Refused(TrustRefusal::UnknownPeer)
            );

            assert_eq!(
                store
                    .lookup(&conn, device_id.as_str(), PairedDeviceRole::Host)
                    .await
                    .expect("lookup"),
                TrustDecision::Refused(TrustRefusal::RoleMismatch {
                    stored_role: PairedDeviceRole::Controller
                })
            );

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
            .expect("insert corrupted");

            assert_eq!(
                store
                    .lookup(&conn, wrong_id.as_str(), PairedDeviceRole::Controller)
                    .await
                    .expect("lookup"),
                TrustDecision::Refused(TrustRefusal::KeyMismatch)
            );

            assert!(store
                .revoke(&conn, device_id.as_str())
                .await
                .expect("revoke"));
            assert_eq!(
                store
                    .lookup(&conn, device_id.as_str(), PairedDeviceRole::Controller)
                    .await
                    .expect("lookup"),
                TrustDecision::Refused(TrustRefusal::Revoked)
            );
        },
    )
    .await;
}

/// PURPOSE:  The device key path must be isolated outside the database directory.
/// SCENARIO: Initialize test paths root; compare device_key_path against parent of db_path.
/// ASSERTS:  device_key_path does not reside inside the database directory.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Set device_key_path to db_path.parent().join("device.key").
#[test]
fn device_key_path_is_isolated_from_database_directory() {
    let dir = tempdir().expect("tempdir");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let key_path = vox_lib::utils::paths::device_key_path();
    let database_path = vox_lib::utils::paths::db_path();
    let database_directory = database_path.parent().expect("db parent");

    assert!(!key_path.starts_with(database_directory));
    assert!(key_path.is_absolute());
}

/// VERIFY-RC-07: routing table contains an entry for every registered IPC command name.
#[test]
fn routing_table_contains_all_registered_ipc_commands() {
    use vox_lib::services::remote::control::get_routing_table;
    let table = get_routing_table();

    // Core set of registered IPC command names (must all be classified).
    let commands = [
        "launch_engine",
        "stop_engine",
        "start_session",
        "end_session",
        "pause_session",
        "resume_session",
        "submit_text_input",
        "set_playback_muted",
        "set_mic_muted",
        "set_session_private_mode",
        "ptt_start",
        "ptt_stop",
        "ptt_cancel",
        "show_main_window",
        "hide_tray_window",
        "set_window_click_through",
        "list_audio_devices",
        "get_settings",
        "get_restart_status",
        "get_settings_contract",
        "reset_settings",
        "get_model_catalog",
        "get_provider_caps",
        "check_provider_health",
        "probe_provider_target",
        "list_llm_models",
        "probe_model_capabilities",
        "get_model_capabilities_cache",
        "list_favorite_models",
        "toggle_model_favorite",
        "setup_remote_server",
        "manage_models",
        "get_projects",
        "create_project",
        "rename_project",
        "delete_project",
        "create_session",
        "continue_session",
        "get_sessions",
        "get_turns",
        "search_history",
        "update_session",
        "delete_session",
        "compact_session",
        "get_transcript_history",
        "get_personal_memory",
        "save_personal_memory",
        "consolidate_personal_memory",
        "regenerate_personal_memory",
        "get_personal_memory_versions",
        "set_active_personal_memory_version",
        "get_observations",
        "get_ingestion_stats",
        "get_memory_revisions",
        "resolve_memory_revisions",
        "list_voices",
        "get_voice_scope",
        "sync_remote_voices",
        "synthesize_voice_preview",
        "toggle_voice_favorite",
        "record_voice_used",
        "set_active_voice",
        "add_voice_from_file",
        "add_voice_from_recording",
        "start_backend_recording",
        "stop_backend_recording",
        "delete_voice",
        "rename_voice",
        "get_runtime_snapshot",
        "get_profiler_snapshot",
        "record_memory_profile_event",
        "fetch_manifest",
        "check_updates",
        "get_onboarding_status",
        "get_runtime_report",
        "complete_setup_wizard",
        "reveal_wizard",
        "notification_permission_state",
        "request_notification_permission",
        "debug_fire_transient",
        "get_notifications",
        "mark_notifications_read",
        "dismiss_notifications",
        "execute_notification_action",
        "pair_new_device",
        "approve_pairing",
        "pair_device",
        "list_paired_devices",
        "revoke_device",
        "disconnect_device",
        "connect_host",
        "disconnect_host",
        "get_link_state",
        "get_command_routing",
    ];

    for cmd in commands {
        assert!(
            table.contains_key(cmd),
            "routing table missing entry for command '{cmd}'"
        );
    }
}
