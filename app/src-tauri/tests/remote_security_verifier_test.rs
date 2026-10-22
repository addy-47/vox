//! ============================================================================
//! tests/remote_security_verifier_test.rs — Coverage the Suite Left Open
//! ============================================================================
//! Category     : Integration Test (Seam 01 — trust boundary)
//! Component    : src/services/remote/security/{pairing,key_store}.rs,
//!                src/utils/paths.rs
//! Prerequisites: None (no models, no services, no DB, no network)
//! Execution    : cargo nextest run --test remote_security_verifier_test --release --nocapture --test-threads=1
//! Metrics      : (a) the pairing token's lifetime has a pinned boundary;
//!                (b) the key file is replaced atomically, not rewritten in
//!                place; (c) the device key path is outside the database
//!                directory.
//!
//! WHY THIS EXISTS
//! ---------------
//! Every test in this file exists because a deliberate mutation of the
//! production code survived the rest of the suite. A mutant that leaves the
//! suite green is a hole in the suite, and the hole is named here so it is
//! closed rather than discovered later.
//!
//! Each test states which mutation it catches in its doc comment.
//! ============================================================================

use std::time::{Duration, Instant};

use tempfile::tempdir;
use vox_lib::services::remote::security::{DeviceKey, PairingCoordinator, PAIRING_TOKEN_TTL};

/// The source address of one pairing attempt.
fn source(last: u8) -> std::net::IpAddr {
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, last))
}

// ============================================================================
// Test 1: The token lifetime has a pinned boundary
// ============================================================================

/// Catches the mutation "the expiry check is `>=` instead of `>`".
///
/// The rest of the suite exercises a token one second inside its lifetime and
/// one second past it. It never exercises the exact boundary, so the
/// comparison operator at the boundary is untested.
///
/// The rule pinned here is the one the implementation states: a token is
/// expired once **strictly more** than the lifetime has elapsed. A token at
/// exactly the lifetime is still accepted. The remote-control spec
/// (§5.3, VERIFY-RC-03) says "expire after 2 minutes" and does not settle
/// the exact instant, so this is a documented decision, not a derivation.
#[test]
fn test_a_token_at_its_lifetime_boundary_is_still_accepted() {
    let coordinator = PairingCoordinator::new();
    let issued = Instant::now();
    let token = coordinator
        .issue_token(issued)
        .expect("a token must be issued");
    let tokens = coordinator.tokens().clone();

    // Exactly at the lifetime: still valid.
    let at_boundary = tokens.validate_token(&token, source(91), issued + PAIRING_TOKEN_TTL);
    assert!(
        at_boundary.is_ok(),
        "a token exactly at its lifetime must still be accepted, it returned {at_boundary:?}"
    );

    // One nanosecond past the lifetime: expired.
    let expired_token = coordinator
        .issue_token(issued)
        .expect("a token must be issued");
    let past = tokens.validate_token(
        &expired_token,
        source(92),
        issued + PAIRING_TOKEN_TTL + Duration::from_nanos(1),
    );
    assert!(
        past.is_err(),
        "a token one nanosecond past its lifetime must be rejected, it returned {past:?}"
    );
}

// ============================================================================
// Test 2: The key file is replaced atomically, not written in place
// ============================================================================

/// Catches the mutation "the key file is written directly, with no temporary
/// file and no rename".
///
/// The rest of the suite never observes the *staging* of the write, so a
/// non-atomic write that truncates the live key file and rewrites it passes
/// every other test — including a crash of the process mid-write, which is
/// exactly the failure the atomic write exists to prevent.
///
/// The observable difference is the file's identity: a staged write replaces
/// the file with a different one (a different inode), while an in-place
/// write keeps the same inode and truncates it. The test therefore checks
/// that the key file is *replaced*, not rewritten.
#[test]
fn test_the_key_file_is_replaced_atomically() {
    use std::os::unix::fs::MetadataExt;

    let dir = tempdir().expect("a temporary directory must be created");
    let path = dir.path().join("device.key");
    let store = vox_lib::services::remote::security::KeyStore::new(path.clone());

    let first = store.load_or_create().expect("a key must be created");
    let first_inode = std::fs::metadata(&path)
        .expect("the key file must be readable")
        .ino();

    let second = DeviceKey::generate().expect("a second key must be generated");
    store.save(&second).expect("the second key must be saved");

    let second_inode = std::fs::metadata(&path)
        .expect("the key file must be readable")
        .ino();

    assert_ne!(
        first_inode, second_inode,
        "the key file must be replaced by a rename, not truncated and rewritten in place"
    );

    // The replacement is complete: the file now holds the second key.
    let reloaded = store.load().expect("the key must reload");
    assert_eq!(
        reloaded.public_key(),
        second.public_key(),
        "the reloaded key must be the one that was saved"
    );
    assert_ne!(reloaded.public_key(), first.public_key());
}

// ============================================================================
// Test 3: The device key path is outside the database directory
// ============================================================================

/// Catches the mutation "the device key is written inside the database
/// directory".
///
/// The database is exported and backed up. A key file inside it would be
/// exported with the data and backed up with it, so the two real production
/// paths are compared here.
#[test]
fn test_the_device_key_path_is_outside_the_database_directory() {
    let dir = tempdir().expect("a temporary directory must be created");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let key_path = vox_lib::utils::paths::device_key_path();
    let database_path = vox_lib::utils::paths::db_path();
    let database_directory = database_path
        .parent()
        .expect("the database path must have a parent");

    assert!(
        !key_path.starts_with(database_directory),
        "the device key must not live inside the database directory: the key is at {} and the \
         database is at {}",
        key_path.display(),
        database_path.display()
    );
}
