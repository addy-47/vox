//! ============================================================================
//! tests/remote_security_noise_test.rs — The Noise_XX Handshake & Transport
//! ============================================================================
//! Category     : Integration Test (Seam 01 — encrypted channel)
//! Component    : src/services/remote/security/noise.rs
//! Prerequisites: None (no models, no services, no DB, no network)
//! Execution    : cargo nextest run --test remote_security_noise_test --release --nocapture --test-threads=1
//! Metrics      : (a) a handshake completes between two peers and both sides
//!                hold a transport; (b) the peer's static key is read from
//!                the handshake state and is `None` until the peer reveals
//!                it; (c) a handshake against an impostor fails, and the
//!                initiator refuses before it sends its own static key;
//!                (d) the prologue is authenticated and never transmitted;
//!                (e) the transport round-trips several payload sizes and
//!                refuses a corrupted, truncated or oversized message.
//!
//! WHY THIS EXISTS
//! ---------------
//! The Noise channel is the encrypted channel every other part of the feature
//! sits on. A defect here is not a bug in one function: it is a defect in the
//! mutual authentication, the forward secrecy and the replay protection the
//! whole feature claims.
//!
//! THE HANDSHAKE IS DRIVEN, NOT COUNTED
//! ------------------------------------
//! The number of handshake messages is a property of the Noise_XX pattern.
//! The tests therefore derive the count from the state machine's turn
//! predicate, so a test that hardcoded a different number would fail
//! loudly rather than pass quietly.
//!
//! THE PIN IS CHECKED AGAINST THE REVEALED KEY
//! --------------------------------------------
//! The remote-control spec §5.3 makes the *revealed* handshake static the
//! root of trust. A driver that pre-loads the pin into the Noise state
//! returns the pin itself from `remote_static()`, which would make the
//! identity binding degenerate into `pin == pin` and pass vacuously.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! These tests prove the crypto over an in-memory message channel. They
//! cannot prove that a WebSocket transport carries those messages correctly;
//! that is what the link conformance suite is for.
//! ============================================================================

mod common;

use common::remote_noise::{completed_transport, drive_handshake, peer_pair, try_drive_handshake};
use vox_lib::services::remote::{
    security::{
        default_noise_prologue, noise_prologue, DeviceKey, NoiseHandshake, SecurityError,
        NOISE_MAX_HANDSHAKE_MESSAGE_BYTES, NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES, X25519_KEY_BYTES,
    },
    MAX_CONTROL_MESSAGE_BYTES,
};

/// The size of one uplink audio frame, in bytes.
const AUDIO_FRAME_BYTES: usize = 320;

/// The size of a large payload, in bytes.
const LARGE_PAYLOAD_BYTES: usize = 60 * 1024;

/// The number of handshake messages the Noise_XX pattern produces today.
///
/// The value is asserted against what the state machine produced, so a test
/// that hardcoded a different number fails loudly.
const EXPECTED_HANDSHAKE_MESSAGES: usize = 3;

/// The number of handshake messages the initiator writes.
const EXPECTED_INITIATOR_MESSAGES: usize = 2;

/// The size of the Noise authentication tag, in bytes.
const NOISE_TAG_BYTES: usize = 16;

/// The largest Noise message, in bytes.
const NOISE_MAX_MESSAGE_BYTES: usize = 65535;

// ============================================================================
// Test 1: A handshake completes and both sides hold a transport
// ============================================================================

#[test]
fn test_a_handshake_completes_and_both_sides_hold_a_transport() {
    let (initiator_key, responder_key) = peer_pair();
    let prologue = default_noise_prologue();

    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("the responder handshake must be built");

    assert!(initiator.is_my_turn(), "the initiator writes first");
    assert!(!responder.is_my_turn());

    let trace = drive_handshake(&mut initiator, &mut responder);
    assert!(initiator.is_complete(), "the handshake must be complete");
    assert!(responder.is_complete(), "the handshake must be complete");
    assert_eq!(
        trace.messages.len(),
        EXPECTED_HANDSHAKE_MESSAGES,
        "the Noise_XX handshake must be the number of messages the state machine produced"
    );
    assert_eq!(
        trace.initiator_messages, EXPECTED_INITIATOR_MESSAGES,
        "the initiator must write the first and the last handshake message"
    );

    // The peer's static key is read from the handshake state, and it is the
    // key the peer proved possession of — not the pin.
    let seen_by_initiator = initiator
        .remote_static()
        .expect("the initiator must see the responder's static key");
    let seen_by_responder = responder
        .remote_static()
        .expect("the responder must see the initiator's static key");
    assert_eq!(
        seen_by_initiator.as_slice(),
        responder_key.public_key(),
        "the initiator must read the responder's static key"
    );
    assert_eq!(
        seen_by_responder.as_slice(),
        initiator_key.public_key(),
        "the responder must read the initiator's static key"
    );
    assert_eq!(
        seen_by_initiator.len(),
        X25519_KEY_BYTES,
        "a static key must be {X25519_KEY_BYTES} bytes"
    );

    let mut initiator_transport = initiator
        .finish()
        .expect("the initiator must enter transport mode");
    let mut responder_transport = responder
        .finish()
        .expect("the responder must enter transport mode");

    // A transport that both sides hold is a transport with the same session
    // key on both sides.
    let payload = b"vox-remote-control".to_vec();
    let encrypted = initiator_transport
        .encrypt(&payload)
        .expect("the initiator must encrypt");
    let decrypted = responder_transport
        .decrypt(&encrypted)
        .expect("the responder must decrypt");
    assert_eq!(decrypted, payload, "the payload must survive the trip");
}

// ============================================================================
// Test 2: A handshake against an impostor fails before the static key goes out
// ============================================================================

#[test]
fn test_a_handshake_against_an_impostor_fails_before_the_static_key_is_sent() {
    let (initiator_key, responder_key) = peer_pair();
    let (impostor_key, _) = peer_pair();
    let prologue = default_noise_prologue();

    // The initiator believes it is talking to the impostor.
    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(impostor_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("the responder handshake must be built");

    // Before the peer has revealed anything, the driver must not be able to
    // report a remote static key. A driver that pre-loads the pin would
    // return the pin here, and the identity binding would degenerate into
    // `pin == pin`.
    assert_eq!(
        initiator.remote_static(),
        None,
        "the peer has revealed nothing yet, so there is no remote static key"
    );

    let (trace, failed) = try_drive_handshake(&mut initiator, &mut responder);
    assert!(
        failed,
        "an initiator that pinned the wrong peer key must fail the handshake"
    );
    assert_eq!(
        trace.initiator_messages, 1,
        "the initiator must refuse before it sends its static key"
    );
    assert!(
        initiator.finish().is_err(),
        "a handshake against an impostor must not produce a transport"
    );

    // The responder refuses a wrongly pinned initiator at the third message —
    // the message that carries the initiator's static key — and cannot enter
    // transport mode afterwards.
    let (initiator_key, responder_key) = peer_pair();
    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(impostor_key.public_key()), &prologue)
            .expect("the responder handshake must be built");

    let first = initiator
        .write_message(&[])
        .expect("the initiator must write");
    responder
        .read_message(&first)
        .expect("the responder must accept the ephemeral");
    let second = responder
        .write_message(&[])
        .expect("the responder must write");
    initiator
        .read_message(&second)
        .expect("the initiator must accept the responder's message");
    let third = initiator
        .write_message(&[])
        .expect("the initiator must write");
    let refused = responder.read_message(&third);
    assert!(
        refused.is_err(),
        "the responder must refuse an initiator whose static key is not the one it pinned, it \
         returned {refused:?}"
    );
    assert!(
        responder.finish().is_err(),
        "the responder must not be able to enter transport mode against a wrongly pinned peer"
    );
}

// ============================================================================
// Test 3: The prologue is authenticated and never transmitted
// ============================================================================

#[test]
fn test_the_prologue_is_authenticated_and_never_transmitted() {
    let (initiator_key, responder_key) = peer_pair();
    let prologue = default_noise_prologue();

    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("the responder handshake must be built");

    let trace = drive_handshake(&mut initiator, &mut responder);

    assert!(!prologue.is_empty(), "the prologue must not be empty");
    for message in &trace.messages {
        assert!(
            !contains_bytes(message, &prologue),
            "a handshake message must not carry the prologue"
        );
    }

    // Two peers with different prologues fail the handshake. The prologue
    // carries the protocol version byte, so this is the version-mismatch
    // rejection the spec demands.
    let current = noise_prologue(1);
    let other = noise_prologue(2);
    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &current)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &other)
            .expect("the responder handshake must be built");

    let (trace, failed) = try_drive_handshake(&mut initiator, &mut responder);
    assert!(
        failed,
        "two peers with different prologues must fail the handshake"
    );
    assert!(
        !trace.messages.is_empty(),
        "the failed handshake must have put messages on the wire"
    );
}

// ============================================================================
// Test 4: A corrupted or truncated message does not decrypt
// ============================================================================

#[test]
fn test_a_corrupted_or_truncated_message_does_not_decrypt() {
    let (initiator_key, responder_key) = peer_pair();
    let (mut initiator_transport, mut responder_transport) =
        completed_transport(&initiator_key, &responder_key);

    let payload = b"the quick brown fox".to_vec();
    let encrypted = initiator_transport
        .encrypt(&payload)
        .expect("the payload must encrypt");

    // A flipped bit.
    let mut corrupted = encrypted.clone();
    let middle = corrupted.len() / 2;
    corrupted[middle] ^= 0xFF;
    assert!(
        responder_transport.decrypt(&corrupted).is_err(),
        "a corrupted message must not decrypt"
    );

    // A truncated message.
    let truncated = &encrypted[..encrypted.len() - 1];
    assert!(
        responder_transport.decrypt(truncated).is_err(),
        "a truncated message must not decrypt"
    );

    // A message with no room for a tag.
    assert!(
        responder_transport.decrypt(&[0u8; 4]).is_err(),
        "a message with no room for a tag must be refused"
    );

    // The transport never falls back to plaintext.
    let decrypted = responder_transport
        .decrypt(&encrypted)
        .expect("the original message must decrypt");
    assert_eq!(decrypted, payload);
}

// ============================================================================
// Test 5: The transport carries payloads of several sizes, and refuses one
//         that does not fit
// ============================================================================
#[test]
fn test_a_payload_near_the_maximum_size_is_an_error() {
    let (initiator_key, responder_key) = peer_pair();
    let (mut initiator_transport, mut responder_transport) =
        completed_transport(&initiator_key, &responder_key);

    for size in [0usize, 1, AUDIO_FRAME_BYTES, LARGE_PAYLOAD_BYTES] {
        let payload: Vec<u8> = (0..size).map(|index| (index % 251) as u8).collect();
        let encrypted = initiator_transport
            .encrypt(&payload)
            .expect("the payload must encrypt");
        assert_eq!(
            encrypted.len(),
            payload.len() + NOISE_TAG_BYTES,
            "a transport message must carry the payload and its 16-byte tag"
        );
        let decrypted = responder_transport
            .decrypt(&encrypted)
            .expect("the payload must decrypt");
        assert_eq!(
            decrypted, payload,
            "a payload of {size} bytes must survive the trip"
        );
    }

    // The largest payload that fits is the maximum message minus the tag.
    let largest = vec![0u8; NOISE_MAX_MESSAGE_BYTES - NOISE_TAG_BYTES];
    assert!(
        initiator_transport.encrypt(&largest).is_ok(),
        "the largest payload that fits must encrypt"
    );

    let too_large = vec![0u8; NOISE_MAX_MESSAGE_BYTES];
    let result = initiator_transport.encrypt(&too_large);
    assert!(
        result.is_err(),
        "a payload that cannot fit must be an error, it returned {result:?}"
    );
}

// ============================================================================
// Test 6: The unauthenticated window is bounded
// ============================================================================

/// The handshake is the one window in which the peer is unauthenticated, so
/// the driver refuses an oversized handshake message before it allocates a
/// buffer for it. Remote-control spec §7.1 caps the pre-authentication
/// traffic at 4 KB; the handshake cap is stricter still.
#[test]
fn test_an_oversized_handshake_message_is_refused() {
    let key = DeviceKey::generate().expect("a device key must be generated");
    let prologue = default_noise_prologue();
    let mut handshake =
        NoiseHandshake::initiator(&key, None, &prologue).expect("the handshake must be built");

    let oversized = vec![0u8; NOISE_MAX_HANDSHAKE_MESSAGE_BYTES + 1];
    let refused = handshake.read_message(&oversized);
    assert!(
        refused.is_err(),
        "an oversized handshake message must be refused, it returned {refused:?}"
    );

    // The boundary itself is still a handshake message, so it is read (and
    // then rejected by the pattern, not by the size check).
    let at_the_limit = vec![0u8; NOISE_MAX_HANDSHAKE_MESSAGE_BYTES];
    let at_limit = handshake.read_message(&at_the_limit);
    assert!(
        !matches!(
            at_limit,
            Err(SecurityError::Noise(ref message)) if message.contains("byte limit")
        ),
        "a message at the cap must not be refused for its size, it returned {at_limit:?}"
    );

    // The write side is capped too, so the driver cannot emit a message that
    // a conforming peer would refuse.
    let mut handshake =
        NoiseHandshake::initiator(&key, None, &prologue).expect("the handshake must be built");
    let refused = handshake.write_message(&[0u8; NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES + 1]);
    assert!(
        refused.is_err(),
        "an over-cap handshake payload must be refused, it returned {refused:?}"
    );
}

// ============================================================================
// Test 7: A valid handshake message over the cap is still refused
// ============================================================================

/// The cap must be the thing that refuses the message.
///
/// A handshake message that is too large can be refused for two different
/// reasons: the cap, or the cipher failing on nonsense bytes. Only the
/// first is the cap doing its job. This test therefore hands the driver a
/// message that is **valid Noise** — one the pattern produced — and only
/// too large, so the cipher would accept it and the cap must not.
///
/// The driver will not *emit* such a message: it caps its own output, so
/// the oversized message here is a real handshake message padded past the
/// cap. The size check runs before the cipher, so the refusal names the
/// size and not a decryption failure.
#[test]
fn test_a_valid_handshake_message_over_the_cap_is_still_refused() {
    let (initiator_key, responder_key) = peer_pair();
    let prologue = default_noise_prologue();
    let mut oversized_writer =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");

    // A real handshake message, padded past the cap. The cipher would have
    // accepted the unpadded message.
    let mut oversized = oversized_writer
        .write_message(&[])
        .expect("the initiator must be able to write the message");
    oversized.resize(NOISE_MAX_HANDSHAKE_MESSAGE_BYTES + 1, 0);
    assert!(
        oversized.len() > NOISE_MAX_HANDSHAKE_MESSAGE_BYTES,
        "the fixture must produce a message of {} bytes, which is over the \
         {NOISE_MAX_HANDSHAKE_MESSAGE_BYTES} byte cap",
        oversized.len()
    );

    // The responder refuses it, and the refusal is the cap's.
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("the responder handshake must be built");
    let refused = responder.read_message(&oversized);
    assert!(
        refused.is_err(),
        "a valid handshake message over the cap must be refused, it returned {refused:?}"
    );
    assert!(
        matches!(
            refused,
            Err(SecurityError::Noise(ref message)) if message.contains("byte limit")
        ),
        "the refusal must be the cap's and not the cipher's, it was {refused:?}"
    );

    // The refusal was the cap's and not the cipher's: the responder's
    // handshake state is untouched, so a normal handshake still completes
    // against it.
    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");
    let first = initiator
        .write_message(&[])
        .expect("the initiator must write");
    responder
        .read_message(&first)
        .expect("a refused message must not have consumed the handshake");
    let second = responder
        .write_message(&[])
        .expect("the responder must write");
    initiator
        .read_message(&second)
        .expect("the initiator must accept the responder's message");
    let third = initiator
        .write_message(&[])
        .expect("the initiator must write");
    responder
        .read_message(&third)
        .expect("the handshake must complete after a refusal");
    assert!(
        responder.finish().is_ok(),
        "the responder must be able to enter transport mode after a refused message"
    );
}

// ============================================================================
// Test 8: A version mismatch produces no transport
// ============================================================================

/// A peer that speaks a different protocol version must not end up with a
/// transport.
///
/// Remote-control spec §7.1: the prologue carries the protocol version
/// byte, so a peer on another version fails the handshake rather than
/// negotiating. The `Hello`-level `VersionMismatch` message does not exist
/// yet — that is a session-layer concern — but the half that does exist is
/// the one that matters for the channel: a failed handshake must not hand
/// out a transport, on either side.
///
/// A handshake that failed and still produced a transport would let a peer on
/// the wrong version carry control messages.
#[test]
fn test_a_version_mismatch_produces_no_transport() {
    let (initiator_key, responder_key) = peer_pair();
    let current = noise_prologue(1);
    let other = noise_prologue(2);

    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &current)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &other)
            .expect("the responder handshake must be built");

    let (trace, failed) = try_drive_handshake(&mut initiator, &mut responder);
    assert!(
        failed,
        "two peers with different prologues must fail the handshake"
    );
    assert!(
        !trace.messages.is_empty(),
        "the failed handshake must have put messages on the wire"
    );

    assert!(
        initiator.finish().is_err(),
        "a handshake against a peer on another version must not produce a transport"
    );
    assert!(
        responder.finish().is_err(),
        "a handshake against a peer on another version must not produce a transport"
    );
}

// ============================================================================
// Test 9: The pin is never reported as the peer's static key
// ============================================================================

/// The pin is the key this side *expects*. The peer's static key is the key
/// the peer *proved possession of*. The two are different objects, and a
/// driver that reports the pin instead of the revealed key makes the
/// identity binding degenerate into `pin == pin`: it passes vacuously.
///
/// Remote-control spec §5.3: the host verifies, after the handshake, that
/// the peer's static key **as read from the completed handshake**, the
/// claimed public key and the claimed device id are the same object. The
/// first of the three is only meaningful if it is the revealed key.
#[test]
fn test_the_pin_is_never_reported_as_the_remote_static_key() {
    let (initiator_key, responder_key) = peer_pair();
    let (impostor_key, _) = peer_pair();
    let prologue = default_noise_prologue();

    // The initiator believes it is talking to the impostor. The peer on the
    // wire is the responder.
    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(impostor_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("the responder handshake must be built");

    // The first message carries the initiator's ephemeral key only. The peer
    // has revealed nothing, so there is no remote static key to report.
    let first = initiator
        .write_message(&[])
        .expect("the initiator must write");
    responder
        .read_message(&first)
        .expect("the responder must accept the ephemeral");
    assert_eq!(
        initiator.remote_static(),
        None,
        "the peer has revealed nothing yet, so there is no remote static key"
    );

    // The second message carries the responder's static key. The initiator
    // reads it, refuses it because it is not the pinned key, and the refusal
    // must not leave the pin in place of the revealed key.
    let second = responder
        .write_message(&[])
        .expect("the responder must write");
    let refused = initiator.read_message(&second);
    assert!(
        refused.is_err(),
        "an initiator that pinned the wrong peer key must refuse the handshake, it returned \
         {refused:?}"
    );

    let revealed = initiator
        .remote_static()
        .expect("the peer revealed its static key in the second message");
    assert_eq!(
        revealed.as_slice(),
        responder_key.public_key(),
        "the reported static key must be the key the peer proved possession of"
    );
    assert_ne!(
        revealed.as_slice(),
        impostor_key.public_key(),
        "the pin must never be reported as the peer's static key"
    );
    assert!(
        initiator.finish().is_err(),
        "a handshake against an impostor must not produce a transport"
    );

    // The responder's side: the pin is the impostor's key, and the key the
    // initiator reveals is its own. The pin must not be reported there
    // either.
    let (initiator_key, responder_key) = peer_pair();
    let mut initiator =
        NoiseHandshake::initiator(&initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(&responder_key, Some(impostor_key.public_key()), &prologue)
            .expect("the responder handshake must be built");

    let first = initiator
        .write_message(&[])
        .expect("the initiator must write");
    responder
        .read_message(&first)
        .expect("the responder must accept the ephemeral");
    let second = responder
        .write_message(&[])
        .expect("the responder must write");
    initiator
        .read_message(&second)
        .expect("the initiator must accept the responder's message");
    let third = initiator
        .write_message(&[])
        .expect("the initiator must write");
    assert!(
        responder.read_message(&third).is_err(),
        "the responder must refuse an initiator whose static key is not the one it pinned"
    );
    let revealed = responder
        .remote_static()
        .expect("the initiator revealed its static key in the third message");
    assert_eq!(
        revealed.as_slice(),
        initiator_key.public_key(),
        "the reported static key must be the key the peer proved possession of"
    );
    assert_ne!(
        revealed.as_slice(),
        impostor_key.public_key(),
        "the pin must never be reported as the peer's static key"
    );

    // A handshake with no pin at all is the first-pairing case: it completes,
    // and the peer's key is still read from the handshake.
    let (initiator_key, responder_key) = peer_pair();
    let mut initiator = NoiseHandshake::initiator(&initiator_key, None, &prologue)
        .expect("the handshake must be built");
    let mut responder = NoiseHandshake::responder(&responder_key, None, &prologue)
        .expect("the handshake must be built");
    let trace = drive_handshake(&mut initiator, &mut responder);
    assert!(
        !trace.messages.is_empty(),
        "an unpinned handshake must still put messages on the wire"
    );
    assert_eq!(
        initiator
            .remote_static()
            .expect("an unpinned handshake must still reveal the peer's key")
            .as_slice(),
        responder_key.public_key(),
        "the revealed key must be the peer's real key"
    );
    assert!(
        initiator.finish().is_ok(),
        "an unpinned handshake must produce a transport"
    );
}

// ============================================================================
// Test 10: The largest control message fits the encrypted transport
// ============================================================================

/// The control cap and the Noise payload cap must not drift.
///
/// Remote-control spec §7.1 caps a control message at 64 KB. The Noise
/// transport is the tighter constraint: one transport message carries one
/// payload plus a 16-byte tag, and the transport refuses a payload that does
/// not fit. If the two caps were independent numbers, a control message the
/// protocol accepts could still die inside the transport — and the failure
/// would look like a transport bug on the host's side, on long transcripts
/// only.
///
/// The cap is therefore derived from the transport's cap, and this test
/// proves the derivation by exercising both sides of it: the largest control
/// message the protocol accepts must survive the real link and must be
/// transportable by the real encrypted channel.
#[test]
fn test_the_largest_control_message_fits_the_encrypted_transport() {
    let (initiator_key, responder_key) = peer_pair();
    let (mut initiator_transport, mut responder_transport) =
        completed_transport(&initiator_key, &responder_key);

    // The proof is behavioural, not a comparison of two constants: the
    // largest control message the protocol accepts must actually fit
    // through the encrypted channel. The control cap is derived from the
    // transport's payload cap so the two cannot drift. If they ever drift
    // apart, this assertion fails here.
    let payload = vec![0u8; MAX_CONTROL_MESSAGE_BYTES];
    let encrypted = initiator_transport.encrypt(&payload).expect(
        "the largest control message the protocol accepts must be transportable by the encrypted \
         channel: the control cap and the Noise payload cap have drifted apart",
    );
    assert_eq!(
        encrypted.len(),
        MAX_CONTROL_MESSAGE_BYTES + NOISE_TAG_BYTES,
        "a transport message must carry the payload and its {NOISE_TAG_BYTES}-byte tag"
    );
    let decrypted = responder_transport
        .decrypt(&encrypted)
        .expect("the largest control message must survive the encrypted channel");
    assert_eq!(
        decrypted.len(),
        MAX_CONTROL_MESSAGE_BYTES,
        "the payload must survive the trip at full length"
    );
}

/// True when `haystack` contains `needle`.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
