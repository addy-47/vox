//! ============================================================================
//! tests/common/remote_noise.rs — Shared Noise Handshake Fixtures
//! ============================================================================
//!
//! WHY THIS EXISTS
//! ---------------
//! More than one test file needs to drive a Noise_XX handshake between two
//! peers and to hold the resulting transport. The handshake itself is
//! production code (`services/remote/security/noise.rs`); this fixture only
//! decides *who speaks next* and shuttles the bytes, which is a property of
//! the Noise_XX pattern rather than of any implementation.
//!
//! THE FIXTURE NEVER REIMPLEMENTS THE PROTOCOL
//! --------------------------------------------
//! The fixture calls the production driver's `write_message`, `read_message`
//! and `is_my_turn`. It does not decide when a handshake is finished, it does
//! not count messages to decide what to do next, and it does not implement
//! any part of the Noise state machine. Every helper here could be replaced
//! by a direct production call with no production change.
//!
//! THE TURN COMES FROM THE DRIVER
//! ------------------------------
//! The loop is driven by `is_my_turn`, so a pattern that produces a
//! different number of messages still drives correctly. A fixture that
//! hardcoded "three messages" would break on a pattern change and hide it.
//! ============================================================================

use vox_lib::services::remote::security::{
    default_noise_prologue, DeviceKey, NoiseHandshake, NoiseTransport,
};

/// The wire traffic of one handshake, and the roles that produced it.
pub struct HandshakeTrace {
    /// Every message that went over the wire, in order.
    pub messages: Vec<Vec<u8>>,
    /// The number of messages the initiator wrote.
    pub initiator_messages: usize,
    /// The number of messages the responder wrote.
    pub responder_messages: usize,
}

/// Two peers with a fresh keypair each.
pub fn peer_pair() -> (DeviceKey, DeviceKey) {
    (
        DeviceKey::generate().expect("the initiator key must be generated"),
        DeviceKey::generate().expect("the responder key must be generated"),
    )
}

/// Drives a handshake between two peers over an in-memory channel.
///
/// The second element of the result says whether the exchange failed.
pub fn try_drive_handshake(
    initiator: &mut NoiseHandshake,
    responder: &mut NoiseHandshake,
) -> (HandshakeTrace, bool) {
    let mut trace = HandshakeTrace {
        messages: Vec::new(),
        initiator_messages: 0,
        responder_messages: 0,
    };

    loop {
        if initiator.is_complete() && responder.is_complete() {
            return (trace, false);
        }
        if initiator.is_my_turn() {
            let Ok(message) = initiator.write_message(&[]) else {
                return (trace, true);
            };
            trace.messages.push(message.clone());
            trace.initiator_messages += 1;
            if responder.read_message(&message).is_err() {
                return (trace, true);
            }
        } else if responder.is_my_turn() {
            let Ok(message) = responder.write_message(&[]) else {
                return (trace, true);
            };
            trace.messages.push(message.clone());
            trace.responder_messages += 1;
            if initiator.read_message(&message).is_err() {
                return (trace, true);
            }
        } else {
            return (trace, true);
        }
    }
}

/// Drives a handshake that is expected to succeed.
pub fn drive_handshake(
    initiator: &mut NoiseHandshake,
    responder: &mut NoiseHandshake,
) -> HandshakeTrace {
    let (trace, failed) = try_drive_handshake(initiator, responder);
    assert!(!failed, "the handshake must complete");
    trace
}

/// Runs a handshake to completion and returns both transports.
///
/// Both sides pin the other, which is the configuration a paired device
/// uses. The pin is checked against the key the peer *revealed*, so a
/// successful run also proves the pin was honoured.
pub fn completed_transport(
    initiator_key: &DeviceKey,
    responder_key: &DeviceKey,
) -> (NoiseTransport, NoiseTransport) {
    let prologue = default_noise_prologue();
    let mut initiator =
        NoiseHandshake::initiator(initiator_key, Some(responder_key.public_key()), &prologue)
            .expect("the initiator handshake must be built");
    let mut responder =
        NoiseHandshake::responder(responder_key, Some(initiator_key.public_key()), &prologue)
            .expect("the responder handshake must be built");

    drive_handshake(&mut initiator, &mut responder);

    let initiator_transport = initiator
        .finish()
        .expect("the initiator must enter transport mode");
    let responder_transport = responder
        .finish()
        .expect("the responder must enter transport mode");
    (initiator_transport, responder_transport)
}
