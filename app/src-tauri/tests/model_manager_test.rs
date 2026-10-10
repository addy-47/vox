//! ============================================================================
//! model_manager_test.rs — Model Download, Verification & Archive Extraction Test
//! ============================================================================
//! Category     : Integration Test (Seam 17)
//! Component    : setup/model_manager.rs + setup/manager_ops.rs + setup/manifest.rs
//! Prerequisites: Local tempdir isolation
//! Execution    : cargo nextest run --test model_manager_test --release --nocapture --test-threads=1
//! Metrics      : SHA256 verification, .verified marker lifecycle, Zip-Slip guard, deletion
//!                cleanup, resumable transfer (Range/206/416/200-fallback, cancel retention)
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

// ============================================================================
// Subtest 5: Resumable transfer over a deliberately-misbehaving HTTP origin
//
// `download_and_hash` (setup/model_manager.rs:284-332) has zero coverage in this
// file: everything above is filesystem behaviour. The resumable-transfer logic
// specified in docs/plans/phase13/android-platform-preparation-plan.md §8.6 lives
// entirely inside `download_and_hash`, and it only ever runs in production against
// a multi-GB transfer over a real network — on a phone.
//
// These tests drive the real public entry point `ModelManager::setup_model`
// (:84), so the `.tmp` naming at :135, the status branch at :170 and the cleanup
// arms at :155/:189 are all exercised. `ModelManager::new::<R>(None)` leaves
// `app_emitter` as `None`, so no Tauri runtime is required.
//
// The origin server is hand-rolled (tests/common/http_server.rs) because the
// interesting cases are wrong servers, not happy ones.
// ============================================================================

/// Every network-bearing test is wrapped in this (testing-style-guide.md §6.1).
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

// ---------------------------------------------------------------------------
// Positive control. Without this, a harness that never delivers a body would make
// every failure below indistinguishable from a passing implementation.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_fresh_download_writes_verified_payload() {
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

// ---------------------------------------------------------------------------
// Behaviour 1 — a pre-existing `.tmp` is resumed via `Range`, and only the
// missing tail is transferred.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_resume_sends_range_header_and_fetches_only_the_tail() {
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

// ---------------------------------------------------------------------------
// Behaviour 2 — SHA-256 must cover the resumed prefix, not just this run's bytes.
// `download_and_hash:300` seeds `Sha256::new()` unconditionally, so a resumed
// hash is computed over the tail alone and can never equal the manifest hash
// once the file is actually appended.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_resumed_download_hash_matches_manifest() {
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

// ---------------------------------------------------------------------------
// Behaviour 3 — the server IGNORES `Range` and replies `200` with the whole body.
// `download_and_hash:292` accepts any success status, so a 200 answering a ranged
// request is indistinguishable from a genuine 200. Appending there duplicates the
// prefix and corrupts the file while still looking like a completed download.
// The correct response is to truncate back to zero and restart.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_server_ignoring_range_restarts_without_duplicating_bytes() {
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

// ---------------------------------------------------------------------------
// Behaviour 4 — `416 Range Not Satisfiable` means the `.tmp` is stale or corrupt.
// It must be deleted and the transfer restarted once from zero. Today `:292`
// rejects 416 as a plain error and `:155` deletes the `.tmp` on the way out, so
// the transfer never restarts and the download is simply lost.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_range_not_satisfiable_discards_stale_tmp_and_restarts() {
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

// ---------------------------------------------------------------------------
// Behaviour 6 — cancelling mid-transfer must RETAIN the partial `.tmp`.
// `setup_model:155` calls `remove_file(&temp_path)` on the error arm, so today
// every cancel throws away the bytes already transferred. This is the regression
// the resumable-transfer work exists to remove.
//
// The server drips the body so cancellation lands at a real mid-transfer instant:
// the test reacts to the server's own write notifications rather than sleeping.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_cancel_mid_transfer_retains_partial_tmp() {
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

// ---------------------------------------------------------------------------
// Behaviour 6, deterministic sibling: a dropped connection mid-body. No timing
// involved — the origin announces the full Content-Length, writes a few hundred
// bytes and hangs up. The partial `.tmp` must survive for the next attempt.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_dropped_connection_retains_partial_tmp() {
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

// ---------------------------------------------------------------------------
// Behaviour 7 — a hash mismatch MUST still delete the partial file (plan §8.6
// step 6). This is the one deletion that survives the resumable-transfer change;
// it must not be "fixed" alongside the cancel arm. Expected to PASS against the
// current code — it is a regression guard, not a new behaviour.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_hash_mismatch_deletes_partial_tmp() {
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

// ---------------------------------------------------------------------------
// Behaviour 5 — progress events during a resume must be ABSOLUTE.
//
// This was blocked in the first pass: `ModelManager::new` only wires progress to
// Tauri IPC, so with `app = None` the emitter is dead and nothing is observable.
// `ModelManager::with_emitter` is the egress seam that makes it testable.
//
// The origin drips 256-byte slices 40 ms apart. `PROGRESS_EMIT_INTERVAL_MS` is
// 150, so the transfer spans ~480 ms and emits several in-transfer progress
// events — without the drip the whole 3 KB tail would arrive inside one interval
// and no in-transfer event would ever fire, making this test vacuous.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_model_manager_resume_reports_absolute_progress() {
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
