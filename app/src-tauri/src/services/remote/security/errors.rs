use std::io;

/// Why a pairing request was refused.
///
/// The reasons are distinct because the caller answers them differently: an
/// expired token is retried with a fresh QR code, a rate-limited source is
/// refused outright, and a consumed token means somebody else already used
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PairingRefusal {
    /// No such token was ever issued, or it was issued by another process.
    #[error("the pairing token is not known")]
    UnknownToken,

    /// The token outlived its lifetime.
    #[error("the pairing token has expired")]
    Expired,

    /// The token was already used. A pairing token is single-use.
    #[error("the pairing token has already been used")]
    Consumed,

    /// Too many failed validations came from this source address.
    #[error("the pairing token was invalidated by repeated failed attempts")]
    Invalidated,

    /// The source address exceeded the pairing attempt limit.
    #[error("the source address exceeded the pairing attempt limit")]
    RateLimited,
}

/// A failure in the remote-control security layer.
///
/// The variants name the *stage* that failed. The security layer never
/// carries key material in an error: a private half that reached an error
/// message would reach a log file.
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    /// A peer identity does not agree with the key it was presented with.
    #[error("the claimed device id does not match the claimed public key")]
    IdentityMismatch,

    /// The device key file could not be read or written.
    #[error("the device key file could not be {action}: {source}")]
    KeyStoreIo {
        /// What the store was doing when it failed.
        action: &'static str,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },

    /// The device key file is not a key file this build understands.
    #[error("the device key file is malformed: {0}")]
    KeyStoreFormat(String),

    /// The Noise handshake or transport failed.
    ///
    /// The stage that failed is carried by where the error was raised, not by
    /// a second variant.
    #[error("the noise channel failed: {0}")]
    Noise(String),

    /// A pairing request was refused.
    #[error("the pairing request was refused: {0}")]
    PairingRefused(#[from] PairingRefusal),

    /// A pre-authentication message exceeded the cap.
    ///
    /// The pairing request is the only message that travels before the Noise
    /// handshake completes, so this is the size a hostile peer controls.
    #[error("the pre-authentication message of {size} bytes exceeds the {limit} byte limit")]
    PreauthMessageTooLarge {
        /// The size of the offending request.
        size: usize,
        /// The cap that applies before the handshake completes.
        limit: usize,
    },

    /// The system random source failed.
    #[error("the system random source failed: {0}")]
    Random(String),
}
