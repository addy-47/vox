//! ============================================================================
//! model_manager_test.rs — model downloads are verified, resumed safely, archive-extracted without path traversal, and cleaned up on removal
//! ============================================================================
//! Category  : Integration (Seam 17 — model setup)
//! Component : src/setup/model_manager.rs, src/setup/manager_ops.rs, src/setup/manifest.rs
//! Binary    : model_manager_test
//! Prereqs   : none (temporary model directory and in-process HTTP origin per test)
//! Run       : cargo nextest run --test model_manager_test --release --test-threads=1
//! ============================================================================

mod common;

use std::{
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};

use common::{
    archive::{
        compute_sha256, create_synthetic_model_entry, create_test_tar_gz_archive,
        create_test_zip_archive,
    },
    http_server::{ServeConfig, TestHttpServer},
    paths::TempPathsGuard,
};
use tempfile::tempdir;
use vox_lib::setup::{
    manager_ops::{delete_model_file, is_model_file_present},
    manifest::VerifiedMarker,
    model_manager::{ModelManager, ModelSetupStatus, ModelStatusEmitter, SetupStep},
};

// ============================================================================
// Archive extraction guards
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

// ============================================================================
// Resumable transfer fixtures
// ============================================================================

/// Every network-bearing test is wrapped in this (testing-style-guide.md §5.4).
const NET_TIMEOUT: Duration = Duration::from_secs(30);

const MODEL_ID: &str = "resumable_model";
const MODEL_REL_PATH: &str = "resumable/model.bin";
const PAYLOAD_LEN: usize = 4096;
const PREFIX_LEN: usize = 1024;

/// Non-uniform, non-periodic-in-a-short-cycle payload so that a prefix, a
/// duplicated body, or an off-by-one splice is always visible in the bytes.
fn resumable_payload() -> Vec<u8> {
    (0..PAYLOAD_LEN)
        .map(|i| ((i * 7 + i / 13) % 251) as u8)
        .collect()
}

fn tmp_path_for(models_dir: &std::path::Path) -> std::path::PathBuf {
    models_dir.join(MODEL_REL_PATH).with_extension("tmp")
}

fn verified_path_for(models_dir: &std::path::Path) -> std::path::PathBuf {
    models_dir.join(MODEL_REL_PATH).with_extension("verified")
}

/// Pre-seeds the `.tmp` that a previous interrupted run would have left behind.
fn seed_partial_tmp(models_dir: &std::path::Path, payload: &[u8], len: usize) {
    let tmp = tmp_path_for(models_dir);
    std::fs::create_dir_all(tmp.parent().unwrap()).unwrap();
    std::fs::write(&tmp, &payload[..len]).unwrap();
}

/// PURPOSE:  A model file whose bytes match the manifest hash must be marked
///           present and get a `.verified` marker; without the marker the app
///           re-downloads a model it already has on every boot.
/// SCENARIO: Write the payload, ask `is_model_file_present` and inspect the
///           marker it writes.
/// ASSERTS:  The file is reported present, the marker exists and records the
///           model id, hash and expected size.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the marker write in `is_model_file_present` — the marker
///           existence assertion would fail.
#[test]
fn model_file_with_matching_hash_is_marked_present_and_verified() {
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

/// PURPOSE:  A model file whose size does not match the manifest must not be
///           accepted; a truncated download would be treated as a complete
///           model and fail at inference time.
/// SCENARIO: Write a tampered payload that is shorter than the manifest
///           entry and ask `is_model_file_present`.
/// ASSERTS:  The file is not reported present and no `.verified` marker is
///           written.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Compare only the file name and not the size in
///           `is_model_file_present` — the presence assertion would fail.
#[test]
fn model_file_with_mismatching_size_is_not_marked_present() {
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
}

/// PURPOSE:  An outdated marker that records a different hash must not
///           validate the file; a stale marker would let a corrupted model
///           pass verification.
/// SCENARIO: Write a wrong `.verified` marker next to a size-matching file
///           and ask `is_model_file_present`.
/// ASSERTS:  The file is not reported present.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Trust the marker without comparing its hash to the manifest — the
///           presence assertion would fail.
#[test]
fn stale_marker_with_wrong_hash_does_not_validate_the_model() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let mut payload = vec![0u8; 64];
    payload[0..10].copy_from_slice(b"WHISPER_01");
    let sha256 = compute_sha256(&payload);

    let model_rel_path = "stt/encoder.bin";
    let model_full_path = models_dir.join(model_rel_path);
    std::fs::create_dir_all(model_full_path.parent().unwrap()).unwrap();
    let tampered_payload = vec![1u8; 32];
    std::fs::write(&model_full_path, &tampered_payload).unwrap();

    let entry = create_synthetic_model_entry(
        "whisper_stt",
        model_rel_path,
        payload.len() as u64,
        &sha256,
        None,
    );

    let verified_path = models_dir.join(model_rel_path).with_extension("verified");
    let bad_marker = VerifiedMarker {
        model_id: Some("whisper_stt".to_string()),
        sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        verified_at: 1000,
        expected_size: 32, // matches the 32-byte tampered_payload
    };
    bad_marker.save(&verified_path).unwrap();

    assert!(
        !is_model_file_present(&entry, models_dir),
        "Outdated marker with mismatched sha256 must not pass validation"
    );
}

/// PURPOSE:  A zip archive whose entry escapes the destination directory must
///           be rejected and must write nothing; a Zip-Slip write overwrites
///           arbitrary files on the user's disk.
/// SCENARIO: Extract a zip containing `../../evil.txt` through the production
///           `do_extract` guard and, as a positive control, extract a benign
///           zip.
/// ASSERTS:  The malicious archive errors with the `Zip-Slip` guard and
///           writes nothing; the benign archive extracts its entries verbatim.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Remove the traversal check in `ModelManager::do_extract` — the
///           rejection assertion would fail.
#[test]
fn zip_archive_with_a_traversal_path_is_rejected() {
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

/// PURPOSE:  A tar.gz archive whose entry escapes the destination directory
///           must be rejected and must write nothing; a Tar-Slip write
///           overwrites arbitrary files on the user's disk.
/// SCENARIO: Extract a tar.gz containing `../../evil.txt` through the
///           production `do_extract` guard and, as a positive control, extract
///           a benign tar.gz.
/// ASSERTS:  The malicious archive errors with the `Tar-Slip` guard and
///           writes nothing; the benign archive extracts its entries verbatim.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Remove the traversal check in `ModelManager::do_extract` — the
///           rejection assertion would fail.
#[test]
fn tar_archive_with_a_traversal_path_is_rejected() {
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

/// PURPOSE:  Removing a model must delete its `.verified` marker too; a
///           leftover marker makes the app believe a deleted model is
///           installed.
/// SCENARIO: Install a model, then delete it through `delete_model_file`.
/// ASSERTS:  Both the model file and the marker are gone from disk.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Delete only the model file in `delete_model_file` — the marker
///           assertion would fail.
#[test]
fn model_removal_clears_the_file_and_the_verified_marker() {
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

    delete_model_file(&entry, models_dir);

    assert!(!model_full_path.exists(), "Model file must be deleted");
    assert!(!verified_path.exists(), ".verified marker must be deleted");
}

/// PURPOSE:  A fresh download must transfer the whole body and install a
///           byte-identical, hash-verified file; a partial install would
///           corrupt the model silently.
/// SCENARIO: Download the payload from an origin that honours Range but is
///           asked for the whole file.
/// ASSERTS:  The origin sees one `200` response and serves the full body, the
///           installed file is byte-identical and a `.verified` marker is
///           written.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the hash verification in `download_and_hash` — the byte
///           identity assertion would still pass, but the marker assertion
///           would fail.
#[tokio::test]
async fn fresh_download_writes_the_verified_payload() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = resumable_payload();
    let sha256 = compute_sha256(&payload);
    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        payload.len() as u64,
        &sha256,
        None,
    );

    let server = TestHttpServer::start(payload.clone(), ServeConfig::honour_range()).await;
    let manager = ModelManager::new::<tauri::Wry>(None);

    let result = tokio::time::timeout(
        NET_TIMEOUT,
        manager.setup_model(&entry, &server.base_url(), models_dir),
    )
    .await
    .expect("setup_model must not hang");
    let observed = server.observe().await;
    server.shutdown().await;

    result.expect("a fresh, complete download MUST succeed");
    assert_eq!(
        observed.statuses,
        vec![200],
        "no Range header was sent, so 200 is correct"
    );
    assert_eq!(
        observed.bytes_served, PAYLOAD_LEN as u64,
        "the whole body must cross the wire"
    );
    assert_eq!(
        std::fs::read(models_dir.join(MODEL_REL_PATH)).unwrap(),
        payload,
        "the installed model file must be byte-identical to the payload"
    );
    assert!(
        verified_path_for(models_dir).exists(),
        "a verified download MUST write its .verified marker"
    );
}

/// PURPOSE:  An interrupted download must be resumed with a `Range` header
///           and must transfer only the missing tail; re-sending the prefix
///           doubles the file and corrupts it.
/// SCENARIO: Seed a 1024-byte `.tmp`, then download from an origin that
///           honours Range.
/// ASSERTS:  Exactly one request is made, it carries
///           `Range: bytes=1024-`, the origin answers `206` and serves only
///           the tail, and the installed file is byte-identical to the
///           payload.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Ignore the existing `.tmp` and always request the whole file —
///           the range-header assertion would fail.
#[tokio::test]
async fn resume_sends_a_range_header_and_fetches_only_the_tail() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = resumable_payload();
    let sha256 = compute_sha256(&payload);
    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        payload.len() as u64,
        &sha256,
        None,
    );
    seed_partial_tmp(models_dir, &payload, PREFIX_LEN);

    let server = TestHttpServer::start(payload.clone(), ServeConfig::honour_range()).await;
    let manager = ModelManager::new::<tauri::Wry>(None);

    let result = tokio::time::timeout(
        NET_TIMEOUT,
        manager.setup_model(&entry, &server.base_url(), models_dir),
    )
    .await
    .expect("setup_model must not hang");
    let observed = server.observe().await;
    server.shutdown().await;

    result.expect("a resumed download MUST succeed");

    assert_eq!(
        observed.requests.len(),
        1,
        "a resume needs exactly one request; got {:?}",
        observed.requests
    );
    assert_eq!(
        observed.requests[0].range_header.as_deref(),
        Some(format!("bytes={PREFIX_LEN}-").as_str()),
        "a pre-existing .tmp of {PREFIX_LEN} bytes MUST produce `Range: bytes={PREFIX_LEN}-`; \
         a missing or wrong header means the .tmp is silently discarded and re-downloaded"
    );
    assert_eq!(
        observed.statuses,
        vec![206],
        "the server answered the range request with 206; the client must accept it"
    );
    assert_eq!(
        observed.bytes_served,
        (PAYLOAD_LEN - PREFIX_LEN) as u64,
        "only the missing tail may be transferred — re-sending the prefix is the \
         defect this test exists to catch"
    );
    assert_eq!(
        std::fs::read(models_dir.join(MODEL_REL_PATH)).unwrap(),
        payload,
        "prefix and tail must concatenate back into the exact payload"
    );
}

/// PURPOSE:  The hash of a resumed download must cover the prefix already on
///           disk, not only this run's bytes; otherwise the resumed file can
///           never match the manifest hash.
/// SCENARIO: Seed a 1024-byte `.tmp`, resume the download and verify the
///           installed file.
/// ASSERTS:  The request carries the Range header, the installed file is
///           exactly the payload length and byte-identical, its SHA-256 equals
///           the manifest hash, the marker records that hash, and no `.tmp` is
///           left behind.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Seed the hasher with the tail only instead of the whole file —
///           the hash assertion would fail.
#[tokio::test]
async fn resumed_download_hash_matches_the_manifest() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = resumable_payload();
    let sha256 = compute_sha256(&payload);
    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        payload.len() as u64,
        &sha256,
        None,
    );
    seed_partial_tmp(models_dir, &payload, PREFIX_LEN);

    let server = TestHttpServer::start(payload.clone(), ServeConfig::honour_range()).await;
    let manager = ModelManager::new::<tauri::Wry>(None);

    let result = tokio::time::timeout(
        NET_TIMEOUT,
        manager.setup_model(&entry, &server.base_url(), models_dir),
    )
    .await
    .expect("setup_model must not hang");
    let observed = server.observe().await;
    server.shutdown().await;

    result.expect("a resumed download whose hash is computed over prefix+tail MUST succeed");

    assert_eq!(
        observed.requests[0].range_header.as_deref(),
        Some(format!("bytes={PREFIX_LEN}-").as_str()),
        "without a Range header the server sends 200 and nothing is actually resumed"
    );
    let installed = std::fs::read(models_dir.join(MODEL_REL_PATH)).unwrap();
    assert_eq!(
        installed.len(),
        PAYLOAD_LEN,
        "resuming must not lengthen the file (appending a full body to a partial \
         .tmp yields {} bytes of corruption)",
        PAYLOAD_LEN + PREFIX_LEN
    );
    assert_eq!(
        installed, payload,
        "the resumed file must be byte-identical to the payload"
    );
    assert_eq!(
        compute_sha256(&installed),
        sha256,
        "SHA-256 over the resumed file must equal the manifest hash — this only \
         holds if the existing prefix was streamed through the hasher"
    );

    let marker = VerifiedMarker::load(&verified_path_for(models_dir))
        .expect("a hash-correct download MUST write its .verified marker");
    assert_eq!(marker.sha256, sha256);
    assert_eq!(marker.expected_size, PAYLOAD_LEN as u64);
    assert!(
        !tmp_path_for(models_dir).exists(),
        "on success the .tmp is renamed into place, not left behind"
    );
}

/// PURPOSE:  An origin that ignores `Range` and replies `200` with the whole
///           body must force a restart; appending that body to the existing
///           `.tmp` silently doubles the file.
/// SCENARIO: Seed a 1024-byte `.tmp` and download from an origin that always
///           answers `200`.
/// ASSERTS:  The ranged request is still made, the origin answers `200` with
///           the full body, and the installed file is exactly the payload
///           length, byte-identical and hashes to the manifest hash.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Append the `200` body to the existing `.tmp` — the length
///           assertion would fail.
#[tokio::test]
async fn server_ignoring_range_restarts_without_duplicating_bytes() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = resumable_payload();
    let sha256 = compute_sha256(&payload);
    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        payload.len() as u64,
        &sha256,
        None,
    );
    seed_partial_tmp(models_dir, &payload, PREFIX_LEN);

    let server =
        TestHttpServer::start(payload.clone(), ServeConfig::ignore_range_return200()).await;
    let manager = ModelManager::new::<tauri::Wry>(None);

    let result = tokio::time::timeout(
        NET_TIMEOUT,
        manager.setup_model(&entry, &server.base_url(), models_dir),
    )
    .await
    .expect("setup_model must not hang");
    let observed = server.observe().await;
    server.shutdown().await;

    assert_eq!(
        observed.requests.len(),
        1,
        "the ranged request must actually be made; got {:?}",
        observed.requests
    );
    assert_eq!(
        observed.requests[0].range_header.as_deref(),
        Some(format!("bytes={PREFIX_LEN}-").as_str()),
        "resume must be attempted with a Range header before the 200-instead-of-206 \
         case can even be detected"
    );
    assert_eq!(
        observed.statuses,
        vec![200],
        "this origin deliberately ignores Range and replies 200 with the full body"
    );
    assert_eq!(
        observed.bytes_served, PAYLOAD_LEN as u64,
        "the server sent the complete body; the client must discard its partial .tmp"
    );

    result.expect("restarting on a 200-instead-of-206 MUST succeed");
    let installed = std::fs::read(models_dir.join(MODEL_REL_PATH)).unwrap();
    assert_eq!(
        installed.len(),
        PAYLOAD_LEN,
        "appending a full body to the existing {PREFIX_LEN}-byte .tmp would silently \
         produce {} bytes; the client must truncate and restart instead",
        PAYLOAD_LEN + PREFIX_LEN
    );
    assert_eq!(
        installed, payload,
        "the restarted file must be byte-identical to the payload"
    );
    assert_eq!(
        compute_sha256(&installed),
        sha256,
        "the restarted download must hash to the manifest hash"
    );
}

/// PURPOSE:  A `416 Range Not Satisfiable` means the `.tmp` is stale; the
///           client must delete it and restart the transfer once from zero
///           instead of failing the download outright.
/// SCENARIO: Seed a 1024-byte `.tmp` and download from an origin that always
///           answers `416` to a ranged request.
/// ASSERTS:  At least two requests are made, the first is ranged and answered
///           `416`, the second carries no Range header and is answered `200`,
///           the download succeeds, the file is byte-identical and the marker
///           is written.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Treat the `416` as a terminal error — the second-request
///           assertion would fail.
#[tokio::test]
async fn range_not_satisfiable_discards_the_stale_tmp_and_restarts() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = resumable_payload();
    let sha256 = compute_sha256(&payload);
    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        payload.len() as u64,
        &sha256,
        None,
    );
    seed_partial_tmp(models_dir, &payload, PREFIX_LEN);

    let server = TestHttpServer::start(payload.clone(), ServeConfig::range_not_satisfiable()).await;
    let manager = ModelManager::new::<tauri::Wry>(None);

    let result = tokio::time::timeout(
        NET_TIMEOUT,
        manager.setup_model(&entry, &server.base_url(), models_dir),
    )
    .await
    .expect("setup_model must not hang");
    let observed = server.observe().await;
    server.shutdown().await;

    assert!(
        observed.requests.len() >= 2,
        "a 416 must trigger a retry; the client made {} request(s): {:?}",
        observed.requests.len(),
        observed.requests
    );
    assert_eq!(
        observed.requests[0].range_header.as_deref(),
        Some(format!("bytes={PREFIX_LEN}-").as_str()),
        "the stale .tmp must first be probed with a Range request"
    );
    assert_eq!(
        observed.statuses.first().copied(),
        Some(416),
        "the origin answers every ranged request with 416"
    );
    assert_eq!(
        observed.requests[1].range_header, None,
        "the retry MUST go out without a Range header — the stale .tmp was deleted, \
         so requesting a range from offset 0 would be a protocol error"
    );
    assert_eq!(
        observed.statuses.get(1).copied(),
        Some(200),
        "the unranged retry must be answered with the full body"
    );

    result.expect("recovering from a 416 by restarting MUST succeed");
    assert_eq!(
        std::fs::read(models_dir.join(MODEL_REL_PATH)).unwrap(),
        payload,
        "the restarted download must be byte-identical to the payload"
    );
    assert!(
        verified_path_for(models_dir).exists(),
        "a recovered download MUST write its .verified marker"
    );
}

/// PURPOSE:  Cancelling a transfer must keep the bytes already received so
///           the next attempt can resume; deleting the partial file throws
///           away a multi-gigabyte download on every phone.
/// SCENARIO: Start a download from an origin that drips the body, cancel it
///           once the origin has written at least 1536 bytes and inspect the
///           `.tmp`.
/// ASSERTS:  The setup call reports an error, the `.tmp` still exists, holds
///           a non-zero but partial number of bytes and those bytes are the
///           true prefix of the payload.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `.tmp` on the cancel arm of `setup_model` — the
///           existence assertion would fail.
#[tokio::test]
async fn cancel_mid_transfer_retains_the_partial_tmp() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = resumable_payload();
    let sha256 = compute_sha256(&payload);
    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        payload.len() as u64,
        &sha256,
        None,
    );

    let config = ServeConfig::honour_range().with_drip(256, Duration::from_millis(30));
    let server = TestHttpServer::start(payload.clone(), config).await;
    let manager = ModelManager::new::<tauri::Wry>(None);

    let base_url = server.base_url();
    let setup = manager.setup_model(&entry, &base_url, models_dir);
    tokio::pin!(setup);

    // `setup` is a lazy future: it must be polled concurrently with the progress
    // watch, otherwise no request is ever issued and the server never writes.
    // `biased` guarantees the progress branch wins while the drip is still running.
    let reached_mid_flight = tokio::select! {
        biased;
        _ = tokio::time::timeout(NET_TIMEOUT, async {
            loop {
                match server.wait_for_progress().await {
                    Some(written) if written >= 1536 => break,
                    Some(_) => continue,
                    None => panic!("server stopped sending before the transfer reached mid-flight"),
                }
            }
        }) => true,
        result = &mut setup => {
            panic!("download finished before cancellation could be issued: {result:?}")
        }
    };
    assert!(
        reached_mid_flight,
        "server never reached mid-flight before the deadline"
    );

    manager.cancel_flag.store(true, Ordering::SeqCst);

    let result = tokio::time::timeout(NET_TIMEOUT, &mut setup)
        .await
        .expect("a cancelled setup_model must return promptly, not hang");
    let observed = server.observe().await;
    server.shutdown().await;

    assert!(
        result.is_err(),
        "a cancelled transfer MUST report an error, got Ok"
    );

    let tmp = tmp_path_for(models_dir);
    assert!(
        tmp.exists(),
        "cancelling MUST retain the partial .tmp — model_manager.rs:155 deletes it, \
         which discards every byte already transferred. Requests: {:?}",
        observed.requests
    );
    let retained = std::fs::metadata(&tmp).unwrap().len();
    assert!(
        retained > 0,
        "the retained .tmp must hold the bytes already received, not be empty"
    );
    assert!(
        retained < PAYLOAD_LEN as u64,
        "the retained .tmp must be partial ({retained} bytes), not a complete file"
    );
    assert_eq!(
        &std::fs::read(&tmp).unwrap()[..],
        &payload[..retained as usize],
        "the retained prefix must be the true prefix of the payload"
    );
}

/// PURPOSE:  A connection dropped mid-body must leave the partial `.tmp`
///           intact so the next run can resume instead of restarting from
///           zero.
/// SCENARIO: Seed a 1024-byte `.tmp` and download from an origin that hangs
///           up after 512 bytes.
/// ASSERTS:  The origin really truncated the body, the setup call reports an
///           error, the `.tmp` exists and holds exactly the prefix plus the
///           512 bytes that did arrive, and those bytes are the true prefix
///           of the payload.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Delete the `.tmp` when the body transfer fails — the existence
///           assertion would fail.
#[tokio::test]
async fn dropped_connection_retains_the_partial_tmp() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = resumable_payload();
    let sha256 = compute_sha256(&payload);
    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        payload.len() as u64,
        &sha256,
        None,
    );
    seed_partial_tmp(models_dir, &payload, PREFIX_LEN);

    let dropped = 512usize;
    let config = ServeConfig::honour_range().with_truncate_after(dropped);
    let server = TestHttpServer::start(payload.clone(), config).await;
    let manager = ModelManager::new::<tauri::Wry>(None);

    let result = tokio::time::timeout(
        NET_TIMEOUT,
        manager.setup_model(&entry, &server.base_url(), models_dir),
    )
    .await
    .expect("setup_model must not hang");
    let observed = server.observe().await;
    server.shutdown().await;

    assert!(
        observed.body_truncated,
        "the origin must really have hung up mid-body"
    );
    assert!(
        result.is_err(),
        "a connection dropped mid-body MUST report an error, got Ok"
    );

    let tmp = tmp_path_for(models_dir);
    assert!(
        tmp.exists(),
        "a dropped connection MUST retain the partial .tmp so the next run resumes"
    );
    let retained = std::fs::metadata(&tmp).unwrap().len();
    assert_eq!(
        retained,
        (PREFIX_LEN + dropped) as u64,
        "the retained .tmp must hold exactly the {PREFIX_LEN}-byte prefix plus the \
         {dropped} bytes that did arrive"
    );
    assert_eq!(
        &std::fs::read(&tmp).unwrap()[..],
        &payload[..(PREFIX_LEN + dropped)],
        "the retained bytes must be the true prefix of the payload"
    );
}

/// PURPOSE:  A download whose hash does not match the manifest must not be
///           installed and must delete the partial file; resuming from a
///           corrupt prefix would fail forever.
/// SCENARIO: Download content that has the right length but different bytes
///           and inspect the outcome.
/// ASSERTS:  The call fails with a "Hash mismatch" error, the `.tmp` is gone,
///           no model file is installed and no `.verified` marker is written.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Install the file before verifying the hash — the model-file
///           existence assertion would fail.
#[tokio::test]
async fn hash_mismatch_deletes_the_partial_tmp() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let expected = resumable_payload();
    let expected_sha256 = compute_sha256(&expected);
    // Same length, different content — isolates the hash check from the size check.
    let served: Vec<u8> = expected.iter().map(|b| b.wrapping_add(1)).collect();
    assert_ne!(
        compute_sha256(&served),
        expected_sha256,
        "the served payload must genuinely differ from the manifest payload"
    );

    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        expected.len() as u64,
        &expected_sha256,
        None,
    );
    seed_partial_tmp(models_dir, &expected, PREFIX_LEN);

    let server = TestHttpServer::start(served, ServeConfig::honour_range()).await;
    let manager = ModelManager::new::<tauri::Wry>(None);

    let result = tokio::time::timeout(
        NET_TIMEOUT,
        manager.setup_model(&entry, &server.base_url(), models_dir),
    )
    .await
    .expect("setup_model must not hang");
    server.shutdown().await;

    let err = result.expect_err("a hash mismatch MUST fail the setup");
    assert!(
        err.to_string().contains("Hash mismatch"),
        "the error must name the hash mismatch, got: {err}"
    );
    assert!(
        !tmp_path_for(models_dir).exists(),
        "a .tmp that failed verification MUST be deleted — resuming from a corrupt \
         prefix would fail forever"
    );
    assert!(
        !models_dir.join(MODEL_REL_PATH).exists(),
        "a hash-mismatched download MUST NOT be installed"
    );
    assert!(
        !verified_path_for(models_dir).exists(),
        "a hash-mismatched download MUST NOT write a .verified marker"
    );
}

/// PURPOSE:  The progress reported during a resumed download must be
///           absolute; a counter that starts at zero shows the UI 25% when
///           three quarters of the file is already on disk.
/// SCENARIO: Resume a 1024-byte-prefixed download from an origin that drips
///           the body and collect the emitted statuses.
/// ASSERTS:  Every status reports the manifest size as its denominator, the
///           in-transfer progress is absolute and strictly increasing, the
///           transfer ends as `Completed` with the full byte count, and the
///           installed file hashes to the manifest hash.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Seed the progress counter at zero instead of the resume offset —
///           the absolute-progress assertion would fail.
#[tokio::test]
async fn resume_reports_absolute_progress() {
    let _guard = TempPathsGuard::new();
    let dir = tempdir().expect("Failed to create tempdir");
    let models_dir = dir.path();

    let payload = resumable_payload();
    let sha256 = compute_sha256(&payload);
    let entry = create_synthetic_model_entry(
        MODEL_ID,
        MODEL_REL_PATH,
        payload.len() as u64,
        &sha256,
        None,
    );
    seed_partial_tmp(models_dir, &payload, PREFIX_LEN);

    let collected: Arc<Mutex<Vec<ModelSetupStatus>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&collected);
    let emitter: ModelStatusEmitter = Arc::new(move |status: ModelSetupStatus| {
        sink.lock().expect("progress sink poisoned").push(status);
    });
    let manager = ModelManager::with_emitter::<tauri::Wry>(None, Some(emitter));

    // The drip interval is deliberately set EQUAL to PROGRESS_EMIT_INTERVAL_MS
    // (150 ms). That forces the first in-transfer progress event to fire within
    // the first one or two 256-byte slices, i.e. while fewer than PREFIX_LEN
    // bytes of the tail have arrived. With a faster drip the first sample already
    // lands past PREFIX_LEN and an ABSOLUTE counter becomes indistinguishable
    // from a zero-seeded one — verified: a `downloaded = 0` mutation then passed.
    let config = ServeConfig::honour_range().with_drip(256, Duration::from_millis(150));
    let server = TestHttpServer::start(payload.clone(), config).await;

    let result = tokio::time::timeout(
        NET_TIMEOUT,
        manager.setup_model(&entry, &server.base_url(), models_dir),
    )
    .await
    .expect("setup_model must not hang");
    server.shutdown().await;

    result.expect("a resumed download reporting absolute progress MUST succeed");

    let statuses = collected.lock().expect("progress sink poisoned").clone();
    assert!(
        !statuses.is_empty(),
        "the emitter must have received at least one status"
    );

    // The denominator is always the manifest size. On a 206 the response
    // Content-Length is the *remaining* length (3072 here); using it as the
    // total would make the UI read 100% at 75% of the real file.
    for status in &statuses {
        assert_eq!(
            status.total_bytes, PAYLOAD_LEN as u64,
            "every status must report the manifest size as the denominator, got {status:?}"
        );
    }

    let downloading: Vec<&ModelSetupStatus> = statuses
        .iter()
        .filter(|s| s.step == SetupStep::Downloading)
        .collect();
    assert!(
        downloading.len() >= 2,
        "the drip MUST yield at least one in-transfer progress event beyond the \
         pre-transfer baseline; got {downloading:?}"
    );

    // `downloading[0]` is `setup_model`'s pre-transfer baseline (model_manager.rs:146),
    // emitted before `download_and_hash` has read the resume offset — it is
    // legitimately 0 and is excluded deliberately. Every event emitted once the
    // transfer is under way must count the prefix that is already on disk.
    for status in downloading.iter().skip(1) {
        assert!(
            status.bytes_downloaded >= PREFIX_LEN as u64,
            "in-transfer progress must be ABSOLUTE — a resume that already holds \
             {PREFIX_LEN} bytes must never report fewer. Got {} in {status:?}",
            status.bytes_downloaded
        );
    }

    // Strictly increasing: the counter may never regress once the transfer is
    // running. A resumed download that reset to 0 would restart this sequence.
    for pair in downloading.iter().skip(1).zip(downloading.iter().skip(2)) {
        assert!(
            pair.1.bytes_downloaded > pair.0.bytes_downloaded,
            "in-transfer progress must increase monotonically, saw {} then {}",
            pair.0.bytes_downloaded,
            pair.1.bytes_downloaded
        );
    }

    // NOTE: the last *throttled* sample is deliberately NOT asserted to equal the
    // full size. `download_and_hash:371` emits at most once per
    // PROGRESS_EMIT_INTERVAL_MS, so the final sample lags by up to one interval's
    // worth of chunks — with a 40 ms drip and a 150 ms interval it lands short of
    // the end by design. The authoritative terminal value is the `Completed`
    // status asserted below.
    let last = downloading.last().expect("checked non-empty above");
    assert!(
        last.bytes_downloaded > PREFIX_LEN as u64,
        "in-transfer progress must climb past the seeded prefix, saw {}",
        last.bytes_downloaded
    );

    // Timing-independent structural check, independent of the drip above. A
    // counter seeded at zero only ever measures the TAIL, so it can never report
    // more than the tail length. An absolute counter that has counted the prefix
    // must report strictly more. This alone distinguishes the two without
    // depending on where the throttled samples happen to land.
    assert!(
        last.bytes_downloaded > (PAYLOAD_LEN - PREFIX_LEN) as u64,
        "in-transfer progress must be absolute: it reached only {}, but a counter \
         seeded at zero could never exceed the {}-byte tail",
        last.bytes_downloaded,
        PAYLOAD_LEN - PREFIX_LEN
    );

    let terminal = statuses.last().expect("checked non-empty above");
    assert_eq!(
        terminal.step,
        SetupStep::Completed,
        "the transfer must terminate as Completed, got {terminal:?}"
    );
    assert_eq!(
        terminal.bytes_downloaded,
        PAYLOAD_LEN as u64,
        "the terminal status must report the whole file, not just the {}-byte tail",
        PAYLOAD_LEN - PREFIX_LEN
    );
    assert_eq!(
        terminal.total_bytes, PAYLOAD_LEN as u64,
        "the terminal status must report the manifest size"
    );

    assert_eq!(
        compute_sha256(&std::fs::read(models_dir.join(MODEL_REL_PATH)).unwrap()),
        sha256,
        "the file the progress described must be the real, hash-correct file"
    );
}
