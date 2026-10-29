use snow::{HandshakeState, TransportState};

use super::{
    errors::SecurityError,
    identity::{DeviceKey, X25519_KEY_BYTES},
};
use crate::services::remote::{
    NOISE_MAX_MESSAGE_BYTES, NOISE_MAX_PAYLOAD_BYTES, NOISE_TAG_BYTES, PROTOCOL_VERSION,
};

/// The Noise pattern the remote-control feature speaks.
///
/// Remote-control spec §5.2: Noise_XX with Curve25519 DH, ChaCha20-Poly1305
/// and BLAKE2s. The pattern is parsed, not assembled: `NoiseParams` carries
/// the pattern inside its handshake choice.
pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// The domain string of the Noise prologue.
///
/// The prologue is mixed into the handshake hash by both peers. It carries
/// the protocol version byte, so a version mismatch is detected by the
/// handshake itself rather than by a friendly error afterwards.
pub const NOISE_PROLOGUE_DOMAIN: &str = "vox/remote-control/noise/v1";

/// The largest Noise handshake message this driver accepts, in bytes.
///
/// A Noise_XX handshake message carries an ephemeral key, a static key and a
/// Diffie-Hellman output, plus the authentication tag. Anything larger is
/// not a handshake message and is refused before it is allocated — the
/// handshake is the one window in which the peer is unauthenticated.
pub const NOISE_MAX_HANDSHAKE_MESSAGE_BYTES: usize = 256;

/// The largest application payload a handshake message may carry.
///
/// The remote-control handshake carries no application payload — the
/// `PairRequest` travels before the handshake, and everything else travels
/// after it — so this is a guard, not a budget. It exists so the driver
/// cannot emit a message that a conforming peer, which caps handshake
/// messages at [`NOISE_MAX_HANDSHAKE_MESSAGE_BYTES`], would refuse.
pub const NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES: usize = 64;

/// Parses the Noise pattern the remote-control feature speaks.
pub fn noise_params() -> Result<snow::params::NoiseParams, SecurityError> {
    let params: snow::params::NoiseParams = NOISE_PATTERN
        .parse::<snow::params::NoiseParams>()
        .map_err(|error| SecurityError::Noise(error.to_string()))?;
    Ok(params)
}

/// The prologue both peers mix into the handshake hash.
///
/// The prologue carries the protocol version byte, so a peer with a
/// different version fails the handshake with a decryption error rather than
/// negotiating. The prologue is **never transmitted**: it is mixed into the
/// handshake hash by both peers, and a transport that sends it leaks the
/// version byte to a passive observer.
pub fn noise_prologue(version: u16) -> Vec<u8> {
    let mut prologue = Vec::with_capacity(NOISE_PROLOGUE_DOMAIN.len() + 2);
    prologue.extend_from_slice(NOISE_PROLOGUE_DOMAIN.as_bytes());
    prologue.extend_from_slice(&version.to_be_bytes());
    prologue
}

/// The prologue for the version this build speaks.
pub fn default_noise_prologue() -> Vec<u8> {
    noise_prologue(PROTOCOL_VERSION)
}

/// The Noise_XX handshake, driven as a message exchange.
///
/// The driver exposes the state machine; it does not count messages. The
/// number of messages is a property of the Noise_XX pattern, and the pattern
/// may change without this code changing.
pub struct NoiseHandshake {
    state: HandshakeState,
    expected_peer: Option<[u8; X25519_KEY_BYTES]>,
}

impl NoiseHandshake {
    /// Builds the initiator's handshake state.
    ///
    /// `expected_peer` pins the static key the peer is expected to hold. When
    /// the expected key is known, a handshake against the wrong peer fails
    /// instead of succeeding against an impostor.
    ///
    /// The pin is enforced by this driver, not by the library. The Noise_XX
    /// pattern *transmits* the peer's static key, and the library's own pin
    /// is overwritten by the key that arrives in the message. The driver
    /// therefore compares the revealed key against the pin the moment it is
    /// revealed: the initiator learns the responder's static key in the
    /// second message, so it refuses the connection before it sends its own
    /// static key to an impostor.
    pub fn initiator(
        local: &DeviceKey,
        expected_peer: Option<&[u8]>,
        prologue: &[u8],
    ) -> Result<Self, SecurityError> {
        Self::build(local, expected_peer, prologue, true)
    }

    /// Builds the responder's handshake state.
    pub fn responder(
        local: &DeviceKey,
        expected_peer: Option<&[u8]>,
        prologue: &[u8],
    ) -> Result<Self, SecurityError> {
        Self::build(local, expected_peer, prologue, false)
    }

    /// Whether it is this peer's turn to write.
    ///
    /// A Noise_XX handshake alternates: the initiator writes, the responder
    /// writes, the initiator writes. The driver never needs to know which
    /// side it is — it only needs to know whose turn it is.
    pub fn is_my_turn(&self) -> bool {
        self.state.is_my_turn()
    }

    /// Produces the next handshake message.
    ///
    /// A payload that would make the message exceed the cap a conforming
    /// peer enforces is refused here, rather than emitted and rejected by
    /// the peer.
    pub fn write_message(&mut self, payload: &[u8]) -> Result<Vec<u8>, SecurityError> {
        if payload.len() > NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES {
            return Err(SecurityError::Noise(format!(
                "a handshake payload of {} bytes exceeds the {NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES} \
                 byte limit",
                payload.len()
            )));
        }
        let mut buffer = vec![0u8; payload.len() + NOISE_MAX_HANDSHAKE_MESSAGE_BYTES];
        let written = self
            .state
            .write_message(payload, &mut buffer)
            .map_err(|error| SecurityError::Noise(error.to_string()))?;
        buffer.truncate(written);
        Ok(buffer)
    }

    /// Consumes a peer's handshake message and returns its payload.
    ///
    /// The moment the peer reveals its static key, the pin is checked. A peer
    /// that is not the one this side expected fails the handshake there and
    /// then, not after the transport has been handed out.
    pub fn read_message(&mut self, message: &[u8]) -> Result<Vec<u8>, SecurityError> {
        if message.len() > NOISE_MAX_HANDSHAKE_MESSAGE_BYTES {
            return Err(SecurityError::Noise(format!(
                "a handshake message of {} bytes exceeds the {NOISE_MAX_HANDSHAKE_MESSAGE_BYTES} \
                 byte limit",
                message.len()
            )));
        }
        let mut buffer = vec![0u8; message.len() + NOISE_MAX_HANDSHAKE_MESSAGE_BYTES];
        let read = self
            .state
            .read_message(message, &mut buffer)
            .map_err(|error| SecurityError::Noise(error.to_string()))?;
        buffer.truncate(read);
        self.verify_expected_peer()?;
        Ok(buffer)
    }

    /// Whether the handshake has finished.
    pub fn is_complete(&self) -> bool {
        self.state.is_handshake_finished()
    }

    /// The peer's static public key, read from the completed handshake.
    ///
    /// `None` until the peer has actually revealed its static key. The value
    /// is never read from a message and never from a pin: it is the key the
    /// peer proved possession of.
    pub fn remote_static(&self) -> Option<[u8; X25519_KEY_BYTES]> {
        self.state
            .get_remote_static()
            .and_then(|bytes| bytes.try_into().ok())
    }

    /// Transitions to transport mode.
    pub fn finish(self) -> Result<NoiseTransport, SecurityError> {
        self.verify_expected_peer()?;
        let remote_static = self.remote_static().ok_or_else(|| {
            SecurityError::Noise("Handshake completed without peer static key".into())
        })?;
        let state = self
            .state
            .into_transport_mode()
            .map_err(|error| SecurityError::Noise(error.to_string()))?;
        Ok(NoiseTransport {
            state,
            remote_static,
        })
    }

    /// Checks the peer's revealed static key against the pin.
    ///
    /// The Noise_XX pattern transmits the peer's static key, so the pin is
    /// checked against the key the peer actually revealed.
    fn verify_expected_peer(&self) -> Result<(), SecurityError> {
        let Some(expected) = self.expected_peer.as_ref() else {
            return Ok(());
        };
        let Some(revealed) = self.state.get_remote_static() else {
            // The peer has not revealed its key yet.
            return Ok(());
        };
        if revealed != expected.as_slice() {
            return Err(SecurityError::Noise(
                "the peer's static key is not the expected one".to_string(),
            ));
        }
        Ok(())
    }

    /// Builds the handshake state for one side of the exchange.
    fn build(
        local: &DeviceKey,
        expected_peer: Option<&[u8]>,
        prologue: &[u8],
        initiator: bool,
    ) -> Result<Self, SecurityError> {
        let params = noise_params()?;
        let builder = snow::Builder::new(params)
            .local_private_key(local.private_key_bytes())
            .map_err(|error| SecurityError::Noise(error.to_string()))?;
        // No `remote_public_key` here on purpose. The Noise_XX pattern does
        // not take a pre-known remote static, and setting one pre-fills
        // `snow`'s `rs` toggle so `get_remote_static()` would return OUR OWN
        // PIN instead of the key the peer actually revealed. The pin lives in
        // `expected_peer` and is checked against the revealed key.
        let builder = builder
            .prologue(prologue)
            .map_err(|error| SecurityError::Noise(error.to_string()))?;
        let state = if initiator {
            builder.build_initiator()
        } else {
            builder.build_responder()
        }
        .map_err(|error| SecurityError::Noise(error.to_string()))?;
        Ok(Self {
            state,
            expected_peer: pinned_peer(expected_peer)?,
        })
    }
}

/// Converts a pinned peer key into its fixed-size form.
fn pinned_peer(
    expected_peer: Option<&[u8]>,
) -> Result<Option<[u8; X25519_KEY_BYTES]>, SecurityError> {
    match expected_peer {
        Some(peer) => Ok(Some(peer.try_into().map_err(|_| {
            SecurityError::Noise(format!(
                "the expected peer key must be {X25519_KEY_BYTES} bytes, got {}",
                peer.len()
            ))
        })?)),
        None => Ok(None),
    }
}

/// The post-handshake state: one payload in, one payload out.
///
/// The transport is message-framed, not a byte stream. One Noise message
/// carries one payload plus the authentication tag, and the caller decides
/// how to frame the payload inside it.
pub struct NoiseTransport {
    state: TransportState,
    remote_static: [u8; X25519_KEY_BYTES],
}

impl NoiseTransport {
    /// The authenticated remote static public key proved by the handshake.
    pub fn remote_static(&self) -> &[u8; X25519_KEY_BYTES] {
        &self.remote_static
    }

    /// Encrypts one payload into one wire message.
    pub fn encrypt(&mut self, payload: &[u8]) -> Result<Vec<u8>, SecurityError> {
        if payload.len() > NOISE_MAX_PAYLOAD_BYTES {
            return Err(SecurityError::Noise(format!(
                "a payload of {} bytes exceeds the {NOISE_MAX_PAYLOAD_BYTES} byte limit",
                payload.len()
            )));
        }
        let mut buffer = vec![0u8; payload.len() + NOISE_TAG_BYTES];
        let written = self
            .state
            .write_message(payload, &mut buffer)
            .map_err(|error| SecurityError::Noise(error.to_string()))?;
        buffer.truncate(written);
        Ok(buffer)
    }

    /// Decrypts one wire message back into one payload.
    ///
    /// A message that is too short to carry a tag, or whose authentication
    /// does not verify, is an error. The transport never falls back to
    /// plaintext.
    pub fn decrypt(&mut self, message: &[u8]) -> Result<Vec<u8>, SecurityError> {
        if message.len() > NOISE_MAX_MESSAGE_BYTES {
            return Err(SecurityError::Noise(format!(
                "a message of {} bytes exceeds the {NOISE_MAX_MESSAGE_BYTES} byte limit",
                message.len()
            )));
        }
        if message.len() < NOISE_TAG_BYTES {
            return Err(SecurityError::Noise(format!(
                "a message of {} bytes cannot carry a {NOISE_TAG_BYTES} byte tag",
                message.len()
            )));
        }
        let mut buffer = vec![0u8; message.len()];
        let read = self
            .state
            .read_message(message, &mut buffer)
            .map_err(|error| SecurityError::Noise(error.to_string()))?;
        buffer.truncate(read);
        Ok(buffer)
    }
}
