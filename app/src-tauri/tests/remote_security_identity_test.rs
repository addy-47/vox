//! ============================================================================
//! tests/remote_security_identity_test.rs — Device Identity & Key Store
//! ============================================================================
//! Category     : Integration Test (Seam 01 — device identity)
//! Component    : src/services/remote/security/{identity,key_store}.rs
//! Prerequisites: None (no models, no services, no DB, no network)
//! Execution    : cargo nextest run --test remote_security_identity_test --release --nocapture --test-threads=1
//! Metrics      : (a) the device id matches an independently computed golden
//!                vector and is sensitive to both of its inputs; (b) the key
//!                file holds only the private half, is written mode 0600 and
//!                reloads to the same public key; (c) a malformed key file is
//!                an error; (d) a device id and a peer identity parse only
//!                from their exact wire forms.
//!
//! WHY THIS EXISTS
//! ---------------
//! The device identity is the root of trust for the whole feature. Everything
//! the phone believes about the laptop, and everything the laptop believes
//! about the phone, is derived from one X25519 keypair and a SHA-256 of that
//! keypair's public half. A defect here is not a bug in one function: it is
//! a defect in the thing every later wave trusts.
//!
//! THE GOLDEN VECTOR
//! -----------------
//! The expected device id in this file was computed with an independent
//! implementation (Python's `hashlib`), not with this code. A vector that
//! came out of the implementation it checks proves nothing.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! These tests prove the derivation, the file format and the permissions. They
//! cannot prove that a device's key file is protected from a user who can
//! read the user's home directory; that is the operating system's job.
//! ============================================================================

mod common;

use std::fs;

use sha2::{Digest, Sha256};
use tempfile::tempdir;
use vox_lib::services::remote::security::{
    derive_device_id, DeviceId, DeviceKey, KeyStore, PeerIdentity, DEVICE_ID_DOMAIN,
    X25519_KEY_BYTES,
};

/// The public key of the golden vector: the bytes 01..20.
const GOLDEN_PUBLIC_KEY: [u8; X25519_KEY_BYTES] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
];

/// The device id of the golden vector, computed with Python's `hashlib`.
const GOLDEN_DEVICE_ID: &str = "4d3b88c1038b99a32152cda72333264e1206611dcf0ba533a179d51e28f63933";

/// The length of a device id in lowercase hex characters.
const DEVICE_ID_HEX_CHARS: usize = 64;

/// The mode a device key file must carry.
const KEY_FILE_MODE: u32 = 0o600;

/// The mode of a key file that is wider than it should be.
const WIDE_KEY_FILE_MODE: u32 = 0o644;

/// The number of bytes a key file holds: the version byte and the private key.
const KEY_FILE_BYTES: usize = 1 + X25519_KEY_BYTES;

/// The version byte of the key file format.
const KEY_FILE_VERSION: u8 = 1;

// ============================================================================
// Test 1: The derivation reproduces an independently computed golden vector
// ============================================================================

#[test]
fn test_the_device_id_matches_the_golden_vector() {
    let derived = derive_device_id(&GOLDEN_PUBLIC_KEY);
    assert_eq!(
        derived.as_str(),
        GOLDEN_DEVICE_ID,
        "the device id derivation must reproduce the golden vector"
    );
    assert_eq!(
        derived.as_str().len(),
        DEVICE_ID_HEX_CHARS,
        "a device id must be {DEVICE_ID_HEX_CHARS} hex characters"
    );
    assert!(
        derived
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "a device id must be lowercase hex, it is `{}`",
        derived.as_str()
    );

    // The derivation is sensitive to the key.
    let mut changed = GOLDEN_PUBLIC_KEY;
    changed[0] ^= 0x01;
    assert_ne!(
        derive_device_id(&changed).as_str(),
        derived.as_str(),
        "a different public key must produce a different device id"
    );

    // And to the domain prefix. The alternative is computed here,
    // independently of the implementation, so the comparison is between two
    // implementations and not between two calls.
    let mut hasher = Sha256::new();
    hasher.update(b"vox/device-id/v2");
    hasher.update(GOLDEN_PUBLIC_KEY);
    assert_ne!(
        hex::encode(hasher.finalize()),
        derived.as_str(),
        "a different domain prefix must produce a different device id"
    );

    // The domain is part of the hash, not a separator around it.
    let mut separator_hasher = Sha256::new();
    separator_hasher.update(DEVICE_ID_DOMAIN.as_bytes());
    separator_hasher.update(b":");
    separator_hasher.update(GOLDEN_PUBLIC_KEY);
    assert_ne!(
        hex::encode(separator_hasher.finalize()),
        derived.as_str(),
        "the domain must be prepended without a separator"
    );
}

// ============================================================================
// Test 2: The key file is private and round-trips
// ============================================================================

#[test]
fn test_the_key_file_is_private_and_round_trips() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("a temporary directory must be created");
    let path = dir.path().join("device.key");
    let store = KeyStore::new(path.clone());

    let key = store.load_or_create().expect("a key must be created");
    assert!(
        path.exists(),
        "the key file must exist after a load-or-create"
    );

    let mode = fs::metadata(&path)
        .expect("the key file must be readable")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, KEY_FILE_MODE,
        "the key file must be written mode 0600, it is mode {mode:o}"
    );

    let reloaded = store.load().expect("the key must reload");
    assert_eq!(
        reloaded.public_key(),
        key.public_key(),
        "the reloaded key must have the same public half"
    );
    assert_eq!(
        reloaded.device_id().as_str(),
        key.device_id().as_str(),
        "the reloaded key must have the same device id"
    );

    // The file holds the version byte and the raw private half, and nothing
    // else. The public half is derived, so it is never stored.
    let bytes = fs::read(&path).expect("the key file must be readable");
    assert_eq!(
        bytes.len(),
        KEY_FILE_BYTES,
        "the key file must hold the version byte and the private half, and nothing else"
    );
    assert_eq!(
        bytes[0], KEY_FILE_VERSION,
        "the first byte of the key file must be the format version"
    );
    assert_eq!(
        &bytes[1..],
        key.private_key_bytes(),
        "the key file must hold the raw private half and nothing else"
    );

    // A wide mode is tightened on load, and the load still succeeds: a file
    // mode is a warning, not a reason to refuse to start.
    fs::set_permissions(&path, fs::Permissions::from_mode(WIDE_KEY_FILE_MODE))
        .expect("the mode must be widened");
    let reloaded = store.load().expect("a wide mode must not stop the load");
    assert_eq!(
        reloaded.public_key(),
        key.public_key(),
        "the reloaded key must have the same public half"
    );
    let mode = fs::metadata(&path)
        .expect("the key file must be readable")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, KEY_FILE_MODE,
        "the mode must be tightened to 0600 on load, it is mode {mode:o}"
    );
}

// ============================================================================
// Test 3: A malformed key file is an error
// ============================================================================

#[test]
fn test_a_malformed_key_file_is_an_error() {
    let dir = tempdir().expect("a temporary directory must be created");
    let path = dir.path().join("device.key");
    let store = KeyStore::new(path.clone());

    // The wrong length.
    fs::write(&path, vec![0u8; KEY_FILE_BYTES - 1]).expect("the file must be written");
    assert!(
        store.load().is_err(),
        "a key file of the wrong length must be an error"
    );

    // An unknown version byte.
    let mut unknown_version = vec![KEY_FILE_VERSION + 1];
    unknown_version.extend_from_slice(&[0u8; X25519_KEY_BYTES]);
    fs::write(&path, unknown_version).expect("the file must be written");
    assert!(
        store.load().is_err(),
        "a key file with an unknown version byte must be an error"
    );

    // The right length and version, but a key the curve cannot use is still
    // accepted: every 32-byte string is a valid X25519 private half.
    let mut valid = vec![KEY_FILE_VERSION];
    valid.extend_from_slice(&[7u8; X25519_KEY_BYTES]);
    fs::write(&path, valid).expect("the file must be written");
    assert!(store.load().is_ok(), "a well-formed key file must load");

    // Two generated keys differ, and a stored private half reproduces the
    // public half it was generated with.
    let first = DeviceKey::generate().expect("a device key must be generated");
    let second = DeviceKey::generate().expect("a device key must be generated");
    assert_ne!(
        first.public_key(),
        second.public_key(),
        "two generated keys must differ"
    );
    let rebuilt =
        DeviceKey::from_private_bytes(first.private_key_bytes().to_vec()).expect("it must rebuild");
    assert_eq!(
        rebuilt.public_key(),
        first.public_key(),
        "the public key derived from the stored half must equal the original"
    );
    assert!(
        DeviceKey::from_private_bytes(vec![0u8; X25519_KEY_BYTES - 1]).is_err(),
        "a private half of the wrong length must be refused"
    );
}

// ============================================================================
// Test 4: A device id and a peer identity parse only from their exact forms
// ============================================================================

#[test]
fn test_a_device_id_parses_only_from_its_exact_wire_form() {
    let key = DeviceKey::generate().expect("a device key must be generated");
    let id = key.device_id();

    let parsed = DeviceId::parse(id.as_str()).expect("a well-formed id must parse");
    assert_eq!(parsed.as_str(), id.as_str());

    assert!(
        DeviceId::parse(&id.as_str().to_uppercase()).is_err(),
        "an uppercase id must be refused"
    );
    assert!(
        DeviceId::parse(&id.as_str()[..DEVICE_ID_HEX_CHARS - 1]).is_err(),
        "a short id must be refused"
    );
    assert!(
        DeviceId::parse(&"z".repeat(DEVICE_ID_HEX_CHARS)).is_err(),
        "a non-hex id must be refused"
    );

    // A peer identity whose id does not match its key is refused: a peer is
    // either the key it claims or it is not a peer at all.
    let public = key.public_key().to_vec();
    let matching = PeerIdentity::new(
        derive_device_id(&public),
        public.clone(),
        "Ada's Phone".to_string(),
    )
    .expect("an identity whose id matches its key must be accepted");
    assert_eq!(matching.public_key(), public.as_slice());
    assert_eq!(matching.display_name(), "Ada's Phone");

    let other_key = DeviceKey::generate().expect("a device key must be generated");
    assert!(
        PeerIdentity::new(
            derive_device_id(other_key.public_key()),
            public,
            "Impostor".to_string(),
        )
        .is_err(),
        "an identity whose id does not match its key must be refused"
    );
}
