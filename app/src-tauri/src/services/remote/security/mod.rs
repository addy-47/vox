//! Device identity, trust and pairing.
//!
//! **Owns:** the device identity and its `DeviceId` derivation, the key
//! store for the private half, the Noise handshake, the pairing flow, and the
//! trust store over the `paired_devices` registry.
//!
//! **Must never contain:** session state and IPC commands. Security decides
//! *whether* a peer may talk; the session decides *what* happens next.
//!
//! Workspace rule: `AGENTS.md` §7.1, `security/`.
//!
//! | File | Owns |
//! | :--- | :--- |
//! | `errors.rs` | the security error vocabulary |
//! | `identity.rs` | the X25519 device key, the `DeviceId` derivation, the peer identity |
//! | `key_store.rs` | the mode-0600 file the private half lives in |
//! | `noise.rs` | the Noise_XX handshake driver and the post-handshake transport |
//! | `pairing.rs` | the pairing token, the pairing flow, the identity binding |
//! | `trust.rs` | the trust decision over `paired_devices` |
//!
//! The module opens no socket and touches no network. The Noise handshake is
//! pure cryptography over an abstract message channel: the transport that
//! carries those messages is a later wave.

pub mod errors;
pub mod identity;
pub mod key_store;
pub mod noise;
pub mod pairing;
pub mod trust;

pub use errors::{PairingRefusal, SecurityError};
pub use identity::{
    derive_device_id, DeviceId, DeviceKey, PeerIdentity, DEVICE_ID_DOMAIN, X25519_KEY_BYTES,
};
pub use key_store::{KeyStore, KEY_FILE_BYTES, KEY_FILE_MODE, KEY_FILE_VERSION};
pub use noise::{
    default_noise_prologue, noise_params, noise_prologue, NoiseHandshake, NoiseTransport,
    NOISE_MAX_HANDSHAKE_MESSAGE_BYTES, NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES, NOISE_PATTERN,
};
pub use pairing::{
    get_local_lan_ip, mint_pairing_qr_svg, verify_claimed_identity, PairingCoordinator,
    PairingDecision, PairingOutcome, PairingPrompt, PairingRequest, PairingToken, PairingTokenStore,
};
pub use trust::{TrustDecision, TrustRefusal, TrustStore};

pub use super::{
    MAX_PREAUTH_MESSAGE_BYTES, PAIRING_APPROVAL_TIMEOUT, PAIRING_ATTEMPTS_PER_WINDOW,
    PAIRING_MAX_FAILED_ATTEMPTS, PAIRING_RATE_LIMIT_WINDOW, PAIRING_TOKEN_MIN_ENTROPY_BITS,
    PAIRING_TOKEN_TTL,
};
