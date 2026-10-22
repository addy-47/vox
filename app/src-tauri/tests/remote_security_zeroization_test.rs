//! ============================================================================
//! tests/remote_security_zeroization_test.rs — The Device Key Zeroizes Its Private Half
//! ============================================================================
//! Category     : Integration Test (Seam 01 — device identity)
//! Component    : src/services/remote/security/identity.rs
//! Prerequisites: None (no models, no services, no DB, no network)
//! Execution    : cargo nextest run --test remote_security_zeroization_test --release --nocapture --test-threads=1
//! Metrics      : the key's destructor zeroizes the private half it owns.
//!
//! WHY THIS EXISTS
//! ---------------
//! The rest of the suite proves that the private half is zeroized by reading
//! the buffer *after* the key was dropped. That read is a use-after-free: the
//! `Vec` that owned the buffer was dropped, and the allocation was released,
//! before the test reads it. Reading released memory is undefined behaviour,
//! so the test's verdict is whatever the allocator happened to leave behind,
//! not a fact about the production code. It is not a proof.
//!
//! WHY THIS FILE IS A SOURCE CONTRACT
//! ----------------------------------
//! The sound way to observe a zeroization is to inspect the block at the
//! moment the allocator releases it. That needs a hook on the global
//! allocator, and the global allocator of every Vox binary is already
//! claimed by the `turso` crate (`#[global_allocator] static GLOBAL:
//! mimalloc::MiMalloc`), so a second one cannot be installed in this test
//! tree. The runtime observation is therefore not constructible here.
//!
//! What *is* constructible — and what this file does — is a check on the
//! destructor itself: the key's `Drop` body must zeroize the private half it
//! owns. The `zeroize` crate's documented contract ("Ensures the entire
//! capacity of the `Vec` is zeroed") turns that into a guarantee about the
//! bytes, and the check is deterministic and defined behaviour.
//!
//! This is a proxy, and it is labelled as one. It proves the destructor
//! calls `zeroize` on the private half. It does not prove, at runtime, that
//! the bytes reached the heap as zeros.
//! ============================================================================

use std::{fs, path::PathBuf};

use vox_lib::services::remote::security::DeviceKey;

/// The root of the crate under test.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The source of the identity module.
fn identity_source() -> String {
    fs::read_to_string(
        crate_root()
            .join("src")
            .join("services")
            .join("remote")
            .join("security")
            .join("identity.rs"),
    )
    .expect("the identity module must be readable")
}

/// Extracts the body of `impl Drop for <name>` from a Rust source file.
///
/// The extractor is deliberately simple: it finds the `impl Drop for` header,
/// then matches braces to find the body. It returns `None` when the type has
/// no `Drop` implementation.
fn drop_body(source: &str, name: &str) -> Option<String> {
    let header = format!("impl Drop for {name}");
    let start = source.find(&header)?;
    let open = source[start..].find('{')? + start;
    let mut depth = 0usize;
    let bytes = source.as_bytes();
    for index in open..bytes.len() {
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(source[open + 1..index].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

// ============================================================================
// Test 1: The device key zeroizes its private half on drop
// ============================================================================

#[test]
fn test_the_device_key_zeroizes_its_private_half_on_drop() {
    let source = identity_source();
    let body = drop_body(&source, "DeviceKey")
        .expect("the device key must have a destructor that zeroizes its private half");

    assert!(
        body.contains(".zeroize("),
        "the device key's destructor must call `zeroize` on the private half, it reads `{body}`"
    );
    assert!(
        body.contains("private"),
        "the device key's destructor must zeroize the private half itself, it reads `{body}`"
    );

    // The type under test is the one the contract is about.
    let key = DeviceKey::generate().expect("a device key must be generated");
    assert_eq!(
        key.private_key_bytes().len(),
        vox_lib::services::remote::security::X25519_KEY_BYTES,
        "the private half must be the expected length"
    );
}
