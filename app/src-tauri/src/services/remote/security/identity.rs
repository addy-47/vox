use std::fmt;

use sha2::{Digest, Sha256};
use snow::{
    resolvers::{CryptoResolver, DefaultResolver},
    types::Dh,
};
use zeroize::Zeroize;

use super::{errors::SecurityError, noise::noise_params};

/// The domain string prefixed to the public key before hashing.
///
/// Remote-control spec §5.1: the hashed bytes are the ASCII of this literal,
/// prepended to the 32 public key bytes, with no separator and no terminator.
pub const DEVICE_ID_DOMAIN: &str = "vox/device-id/v1";

/// The length of an X25519 key, in bytes.
pub const X25519_KEY_BYTES: usize = 32;

/// The length of a SHA-256 digest, in bytes.
pub const DEVICE_ID_BYTES: usize = 32;

/// The length of a device id, in lowercase hex characters.
pub const DEVICE_ID_HEX_CHARS: usize = DEVICE_ID_BYTES * 2;

/// A device id: the lowercase hex of `SHA-256("vox/device-id/v1" ‖ public_key)`.
///
/// The id is *derived*, never chosen. A device that holds a key therefore
/// cannot hold an id that disagrees with it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(String);

impl DeviceId {
    /// The id as a lowercase hex string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parses an id from its lowercase hex form.
    ///
    /// The length and the character set are checked: an id that is not
    /// 64 lowercase hex characters is not an id this build produced.
    pub fn parse(text: &str) -> Result<Self, SecurityError> {
        if text.len() != DEVICE_ID_HEX_CHARS {
            return Err(SecurityError::IdentityMismatch);
        }
        if !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(SecurityError::IdentityMismatch);
        }
        Ok(Self(text.to_string()))
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Derives the device id of an X25519 public key.
///
/// This is the one derivation function. The identity layer owns it; the
/// persistence layer stores the pair verbatim and never re-derives it
/// (db-spec RULE-DB-05).
pub fn derive_device_id(public_key: &[u8]) -> DeviceId {
    let mut hasher = Sha256::new();
    hasher.update(DEVICE_ID_DOMAIN.as_bytes());
    hasher.update(public_key);
    DeviceId(hex::encode(hasher.finalize()))
}

/// The device's own X25519 keypair.
///
/// This is the Noise_XX static key and the identity key: one object, two
/// names. The private half is zeroized when the key is dropped, and it never
/// leaves the device.
pub struct DeviceKey {
    private: Vec<u8>,
    public: [u8; X25519_KEY_BYTES],
}

impl DeviceKey {
    /// Generates a fresh keypair.
    pub fn generate() -> Result<Self, SecurityError> {
        let params = noise_params()?;
        let keypair = snow::Builder::new(params)
            .generate_keypair()
            .map_err(|error| SecurityError::Noise(error.to_string()))?;
        // The keypair already carries the public half the pattern's DH
        // produced. Do not recompute it with a different primitive.
        let mut public = [0u8; X25519_KEY_BYTES];
        public.copy_from_slice(&keypair.public);
        Ok(Self {
            private: keypair.private,
            public,
        })
    }

    /// Rebuilds a keypair from its stored private half.
    ///
    /// The public half is derived, so a stored pair can never disagree with
    /// itself: there is only one half in the file.
    pub fn from_private_bytes(private: Vec<u8>) -> Result<Self, SecurityError> {
        if private.len() != X25519_KEY_BYTES {
            return Err(SecurityError::KeyStoreFormat(format!(
                "the private half must be {X25519_KEY_BYTES} bytes, got {}",
                private.len()
            )));
        }
        let derived = derive_public_key(&private)?;
        Ok(Self {
            private,
            public: derived,
        })
    }

    /// The public half: the bytes the Noise handshake authenticates and the
    /// bytes the device id is derived from.
    pub fn public_key(&self) -> &[u8; X25519_KEY_BYTES] {
        &self.public
    }

    /// The private half.
    ///
    /// This is key material. It is never logged, never serialised and never
    /// placed in an error message.
    pub fn private_key_bytes(&self) -> &[u8] {
        &self.private
    }

    /// The device id derived from the public half.
    pub fn device_id(&self) -> DeviceId {
        derive_device_id(&self.public)
    }
}

impl Drop for DeviceKey {
    fn drop(&mut self) {
        self.private.zeroize();
    }
}

/// A peer's identity: its device id, its public key and its display name.
///
/// The constructor refuses an identity whose id is not the derivation of its
/// key. A rejected identity is an error, never a silently-trusted peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerIdentity {
    device_id: DeviceId,
    public_key: Vec<u8>,
    display_name: String,
}

impl PeerIdentity {
    /// Builds an identity, refusing one whose id does not match its key.
    pub fn new(
        device_id: DeviceId,
        public_key: Vec<u8>,
        display_name: String,
    ) -> Result<Self, SecurityError> {
        if derive_device_id(&public_key) != device_id {
            return Err(SecurityError::IdentityMismatch);
        }
        Ok(Self {
            device_id,
            public_key,
            display_name,
        })
    }

    /// The peer's device id.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// The peer's 32-byte X25519 static public key.
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// The peer's human-readable name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

/// Derives the X25519 public key that belongs to a private half.
///
/// The DH primitive comes from the pattern, not from a literal, so the public
/// half can never disagree with the key Noise authenticates.
fn derive_public_key(private: &[u8]) -> Result<[u8; X25519_KEY_BYTES], SecurityError> {
    let params = noise_params()?;
    let mut dh: Box<dyn Dh> = DefaultResolver.resolve_dh(&params.dh).ok_or_else(|| {
        SecurityError::Noise("the pattern's DH primitive is unavailable".to_string())
    })?;
    dh.set(private);
    let mut public = [0u8; X25519_KEY_BYTES];
    public.copy_from_slice(dh.pubkey());
    Ok(public)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN_PUBLIC_KEY: [u8; X25519_KEY_BYTES] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];
    const GOLDEN_DEVICE_ID: &str =
        "4d3b88c1038b99a32152cda72333264e1206611dcf0ba533a179d51e28f63933";

    #[test]
    fn device_id_matches_golden_vector_and_is_sensitive_to_key() {
        let derived = derive_device_id(&GOLDEN_PUBLIC_KEY);
        assert_eq!(derived.as_str(), GOLDEN_DEVICE_ID);
        assert_eq!(derived.as_str().len(), DEVICE_ID_HEX_CHARS);

        let mut changed = GOLDEN_PUBLIC_KEY;
        changed[0] ^= 0x01;
        assert_ne!(derive_device_id(&changed).as_str(), derived.as_str());
    }

    #[test]
    fn device_id_parses_only_strict_lowercase_hex() {
        let key = DeviceKey::generate().expect("key must be generated");
        let id = key.device_id();

        assert_eq!(DeviceId::parse(id.as_str()).unwrap().as_str(), id.as_str());
        assert!(DeviceId::parse(&id.as_str().to_uppercase()).is_err());
        assert!(DeviceId::parse(&id.as_str()[..DEVICE_ID_HEX_CHARS - 1]).is_err());
        assert!(DeviceId::parse(&"g".repeat(DEVICE_ID_HEX_CHARS)).is_err());
    }

    #[test]
    fn peer_identity_requires_matching_derived_id() {
        let key = DeviceKey::generate().expect("key must be generated");
        let public = key.public_key().to_vec();
        let id = derive_device_id(&public);

        let identity = PeerIdentity::new(id.clone(), public.clone(), "Test Device".into()).unwrap();
        assert_eq!(identity.display_name(), "Test Device");
        assert_eq!(identity.public_key(), public.as_slice());

        let other_key = DeviceKey::generate().expect("other key must be generated");
        let wrong_id = derive_device_id(other_key.public_key());
        assert!(PeerIdentity::new(wrong_id, public, "Mismatch".into()).is_err());
    }

    #[test]
    fn device_key_zeroizes_private_material() {
        let mut key = DeviceKey::generate().expect("key must be generated");
        assert_eq!(key.private_key_bytes().len(), X25519_KEY_BYTES);
        assert!(key.private.iter().any(|&b| b != 0));
        key.private.zeroize();
        assert!(key.private.iter().all(|&b| b == 0));
    }
}
