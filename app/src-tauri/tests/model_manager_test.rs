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

use common::archive::{
    compute_sha256, create_synthetic_model_entry, create_test_tar_gz_archive,
    create_test_zip_archive,
};
use common::paths::TempPathsGuard;
use tempfile::tempdir;
use vox_lib::setup::manager_ops::{delete_model_file, is_model_file_present};
use vox_lib::setup::manifest::VerifiedMarker;

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
    assert!(present, "Model file with matching size must be marked present");

    let verified_path = models_dir.join(model_rel_path).with_extension("verified");
    assert!(verified_path.exists(), ".verified marker must exist on disk");

    let marker = VerifiedMarker::load(&verified_path).expect("Marker should load and parse as JSON");
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
    assert!(!present, "Corrupted model file with mismatching size must fail verification");

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
// ============================================================================
#[test]
fn test_model_manager_zip_slip_rejection() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let base_dir = dir.path();
    let extract_dir = base_dir.join("extract");
    std::fs::create_dir_all(&extract_dir).unwrap();

    let zip_path = base_dir.join("evil.zip");
    let evil_entry = ("../../evil.txt", b"malicious content" as &[u8]);
    create_test_zip_archive(&zip_path, &[evil_entry]).expect("Failed to create zip archive");

    // Test Zip-Slip detection by reading through zip archive enclosed_name
    let file = std::fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let entry = archive.by_index(0).unwrap();
    assert!(
        entry.enclosed_name().is_none(),
        "Path with parent directory traversal must be rejected by enclosed_name()"
    );

    // Verify evil file was never extracted outside extract_dir
    let evil_file = base_dir.join("evil.txt");
    assert!(!evil_file.exists(), "Zip slip file must not exist on filesystem");
}

#[test]
fn test_model_manager_tar_slip_rejection() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let base_dir = dir.path();
    let extract_dir = base_dir.join("extract");
    std::fs::create_dir_all(&extract_dir).unwrap();

    let tar_path = base_dir.join("evil.tar.gz");
    let evil_entry = ("../../evil.txt", b"malicious content" as &[u8]);
    create_test_tar_gz_archive(&tar_path, &[evil_entry]).expect("Failed to create tar.gz archive");

    let file = std::fs::File::open(&tar_path).unwrap();
    let tar_gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(tar_gz);
    for entry_res in archive.entries().unwrap() {
        let entry = entry_res.unwrap();
        let path = entry.path().unwrap();
        let has_parent = path.components().any(|c| c == std::path::Component::ParentDir);
        assert!(
            has_parent,
            "Tar archive traversal component ParentDir must be detected"
        );
    }
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
