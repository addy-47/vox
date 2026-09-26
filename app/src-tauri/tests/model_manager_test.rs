//! ============================================================================
//! model_manager_test.rs — Model Download, Verification & Archive Extraction Test
//! ============================================================================
//! Category     : Integration Test (Seam 17)
//! Component    : setup/model_manager.rs + setup/manager_ops.rs + setup/manifest.rs
//! Prerequisites: Local tempdir isolation
//! Execution    : cargo nextest run --test model_manager_test --release --nocapture --test-threads=1
//! Metrics      : SHA256 verification, .verified marker lifecycle, Zip-Slip guard, deletion cleanup
//! ============================================================================

mod common;

use common::{
    archive::{
        compute_sha256, create_synthetic_model_entry, create_test_tar_gz_archive,
        create_test_zip_archive,
    },
    paths::TempPathsGuard,
};
use tempfile::tempdir;
use vox_lib::setup::{
    manager_ops::{delete_model_file, is_model_file_present},
    manifest::VerifiedMarker,
    model_manager::ModelManager,
};

// ============================================================================
// Subtest 1: Valid payload verification and marker generation
// ============================================================================
#[test]
fn test_model_manager_valid_payload_verification() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = b"Simulated neural network weight tensor content bytes 0123456789";
    let sha256 = compute_sha256(payload);

    let model_rel_path = "vad/model.onnx";
    let model_full_path = models_dir.join(model_rel_path);
    std::fs::create_dir_all(model_full_path.parent().unwrap()).unwrap();
    std::fs::write(&model_full_path, payload).unwrap();

    let entry = create_synthetic_model_entry(
        "ten_vad",
        model_rel_path,
        payload.len() as u64,
        &sha256,
        None,
    );

    // Initial check: is_model_file_present should recognize size match and create marker
    let present = is_model_file_present(&entry, models_dir);
    assert!(
        present,
        "Model file with matching size must be marked present"
    );

    let verified_path = models_dir.join(model_rel_path).with_extension("verified");
    assert!(
        verified_path.exists(),
        ".verified marker must exist on disk"
    );

    let marker =
        VerifiedMarker::load(&verified_path).expect("Marker should load and parse as JSON");
    assert_eq!(marker.model_id.as_deref(), Some("ten_vad"));
    assert_eq!(marker.sha256, sha256);
    assert_eq!(marker.expected_size, payload.len() as u64);

    // Calling is_model_file_present a second time uses marker fast-path
    assert!(is_model_file_present(&entry, models_dir));
}

// ============================================================================
// Subtest 2: Corrupted payload detection
// ============================================================================
#[test]
fn test_model_manager_corrupted_payload_detection() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let mut payload = vec![0u8; 64];
    payload[0..10].copy_from_slice(b"WHISPER_01");
    let sha256 = compute_sha256(&payload);

    let model_rel_path = "stt/encoder.bin";
    let model_full_path = models_dir.join(model_rel_path);
    std::fs::create_dir_all(model_full_path.parent().unwrap()).unwrap();

    // Tamper with content: different bytes and smaller size
    let tampered_payload = vec![1u8; 32];
    std::fs::write(&model_full_path, &tampered_payload).unwrap();

    // Entry expects original sha256 and original size
    let entry = create_synthetic_model_entry(
        "whisper_stt",
        model_rel_path,
        payload.len() as u64,
        &sha256,
        None,
    );

    // Size doesn't match expected_size, so is_model_file_present returns false
    let present = is_model_file_present(&entry, models_dir);
    assert!(
        !present,
        "Corrupted model file with mismatching size must fail verification"
    );

    let verified_path = models_dir.join(model_rel_path).with_extension("verified");
    assert!(
        !verified_path.exists(),
        ".verified marker must NOT be created for invalid payload"
    );

    // Even if an outdated/corrupted marker exists with the wrong sha256, is_model_file_present
    // must not falsely validate it against entry.sha256 when file size matches but hash differs
    let bad_marker = VerifiedMarker {
        model_id: Some("whisper_stt".to_string()),
        sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        verified_at: 1000,
        expected_size: 32, // matches the 32-byte tampered_payload
    };
    bad_marker.save(&verified_path).unwrap();

    // entry expects original 64-byte payload with sha256; bad_marker has wrong sha256 and expected_size 32
    assert!(
        !is_model_file_present(&entry, models_dir),
        "Outdated marker with mismatched sha256 must not pass validation"
    );
}

// ============================================================================
// Subtest 3: Zip-Slip and Tar-Slip path traversal rejection
//
// These tests drive the REAL guard: `ModelManager::do_extract`.
//
// The previous versions of these tests asserted the third-party `zip`/`tar` crate's
// own `enclosed_name()` / `Component::ParentDir` behaviour and then asserted a file
// was absent that nothing had ever attempted to create. They passed even if
// `do_extract`'s guards were deleted entirely — advertising "Zip-Slip guard" as a
// covered metric while providing no security coverage at all (integration-test-spec.md
// §0.1.3 Self-Execution Ban).
//
// Each test now asserts BOTH directions:
//   1. NEGATIVE — a malicious archive is rejected with the specific guard error, and
//      no file is written outside (or inside) the destination directory.
//   2. POSITIVE CONTROL — a benign archive extracts successfully, proving the guard
//      rejects *traversal specifically* rather than rejecting everything.
// ============================================================================

/// Extracts `archive_path` via the production guard and asserts a traversal rejection.
fn assert_traversal_rejected(archive_path: &std::path::Path, archive_type: &str, guard: &str) {
    let dir = tempdir().expect("Failed to create tempdir");
    let base_dir = dir.path();
    let extract_dir = base_dir.join("extract");
    std::fs::create_dir_all(&extract_dir).unwrap();

    let res = ModelManager::do_extract(archive_path, archive_type, &extract_dir);
    let err = res.expect_err("do_extract MUST reject an archive containing a traversal path");

    assert!(
        err.to_string().contains(guard),
        "do_extract must report the {} guard, got: {}",
        guard,
        err
    );
    assert!(
        !base_dir.join("evil.txt").exists(),
        "{}: no file may be written OUTSIDE extract_dir",
        guard
    );
    assert!(
        !extract_dir.join("evil.txt").exists(),
        "{}: the traversal entry must not be extracted at all",
        guard
    );
    assert!(
        !base_dir.join("../../evil.txt").exists(),
        "{}: no file may escape the tempdir root",
        guard
    );
}

#[test]
fn test_model_manager_zip_slip_rejection() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");

    // Malicious: entry name escapes via parent-directory traversal.
    let evil_zip = dir.path().join("evil.zip");
    let malicious_payload: &[u8] = b"malicious content";
    let evil_entries: &[(&str, &[u8])] = &[("../../evil.txt", malicious_payload)];
    create_test_zip_archive(&evil_zip, evil_entries)
        .expect("Failed to create malicious zip archive");
    assert_traversal_rejected(&evil_zip, "zip", "Zip-Slip");

    // POSITIVE CONTROL: a benign archive must still extract successfully.
    let benign_zip = dir.path().join("benign.zip");
    create_test_zip_archive(
        &benign_zip,
        &[
            ("model.onnx", b"benign model weights" as &[u8]),
            ("nested/config.json", b"{}" as &[u8]),
        ],
    )
    .expect("Failed to create benign zip archive");
    let extract_dir = dir.path().join("benign_extract");
    std::fs::create_dir_all(&extract_dir).unwrap();
    ModelManager::do_extract(&benign_zip, "zip", &extract_dir)
        .expect("a benign zip archive MUST extract successfully");
    assert_eq!(
        std::fs::read(extract_dir.join("model.onnx")).unwrap(),
        b"benign model weights",
        "benign archive content must be written verbatim"
    );
    assert_eq!(
        std::fs::read(extract_dir.join("nested/config.json")).unwrap(),
        b"{}",
        "benign nested entries must be extracted"
    );
}

#[test]
fn test_model_manager_tar_slip_rejection() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");

    // Malicious: entry path escapes via parent-directory traversal.
    let evil_tar = dir.path().join("evil.tar.gz");
    let malicious_payload: &[u8] = b"malicious content";
    let evil_entries: &[(&str, &[u8])] = &[("../../evil.txt", malicious_payload)];
    create_test_tar_gz_archive(&evil_tar, evil_entries)
        .expect("Failed to create malicious tar.gz archive");
    assert_traversal_rejected(&evil_tar, "tar.gz", "Tar-Slip");

    // POSITIVE CONTROL: a benign archive must still extract successfully.
    let benign_tar = dir.path().join("benign.tar.gz");
    create_test_tar_gz_archive(
        &benign_tar,
        &[
            ("model.onnx", b"benign model weights" as &[u8]),
            ("nested/config.json", b"{}" as &[u8]),
        ],
    )
    .expect("Failed to create benign tar.gz archive");
    let extract_dir = dir.path().join("benign_extract");
    std::fs::create_dir_all(&extract_dir).unwrap();
    ModelManager::do_extract(&benign_tar, "tar.gz", &extract_dir)
        .expect("a benign tar.gz archive MUST extract successfully");
    assert_eq!(
        std::fs::read(extract_dir.join("model.onnx")).unwrap(),
        b"benign model weights",
        "benign archive content must be written verbatim"
    );
    assert_eq!(
        std::fs::read(extract_dir.join("nested/config.json")).unwrap(),
        b"{}",
        "benign nested entries must be extracted"
    );
}

// ============================================================================
// Subtest 4: Model removal cleans files and .verified marker
// ============================================================================
#[test]
fn test_model_manager_removal_cleans_marker() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = b"Temporary model payload to be deleted";
    let sha256 = compute_sha256(payload);

    let model_rel_path = "test_group/test_model.bin";
    let model_full_path = models_dir.join(model_rel_path);
    std::fs::create_dir_all(model_full_path.parent().unwrap()).unwrap();
    std::fs::write(&model_full_path, payload).unwrap();

    let entry = create_synthetic_model_entry(
        "test_cleanup_model",
        model_rel_path,
        payload.len() as u64,
        &sha256,
        None,
    );

    // Mark as present and create marker
    assert!(is_model_file_present(&entry, models_dir));
    let verified_path = models_dir.join(model_rel_path).with_extension("verified");
    assert!(model_full_path.exists());
    assert!(verified_path.exists());

    // Execute SUT: delete_model_file
    delete_model_file(&entry, models_dir);

    // Verify both model file and .verified marker are removed
    assert!(!model_full_path.exists(), "Model file must be deleted");
    assert!(!verified_path.exists(), ".verified marker must be deleted");
}
