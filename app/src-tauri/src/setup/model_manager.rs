use std::{
    fs::{create_dir_all, read_dir, remove_dir_all, remove_file, rename, File},
    io::{copy, Read},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use super::{
    MODEL_CONNECT_TIMEOUT_SECS, MODEL_DOWNLOAD_IDLE_TIMEOUT_SECS, PROGRESS_EMIT_INTERVAL_MS,
};
use crate::{
    core::events::{emit_ipc, IpcEvent},
    setup::manifest::{ModelEntry, VerifiedMarker},
    utils::disk,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SetupStep {
    Idle,
    Downloading,
    Extracting,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSetupStatus {
    pub model_id: String,
    pub step: SetupStep,
    pub progress: f32,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
}

pub type ModelStatusEmitter = Arc<dyn Fn(ModelSetupStatus) + Send + Sync>;

/// Block size used when streaming an existing partial file through the hasher.
const HASH_PREFIX_BUFFER_BYTES: usize = 64 * 1024;

/// Orchestrates model downloads, extraction, and verification.
pub struct ModelManager {
    app_emitter: Option<ModelStatusEmitter>,
    client: Client,
    pub cancel_flag: Arc<AtomicBool>,
}

impl ModelManager {
    pub fn new<R: tauri::Runtime>(app: Option<AppHandle<R>>) -> Self {
        Self::with_emitter(app, None)
    }

    /// Builds a manager with an explicit status sink.
    ///
    /// `new` wires progress to Tauri IPC only, which makes emissions
    /// unobservable without a live `AppHandle`. This constructor is the egress
    /// seam that lets a headless test subscribe to the same
    /// `ModelSetupStatus` stream the wizard sees.
    pub fn with_emitter<R: tauri::Runtime>(
        app: Option<AppHandle<R>>,
        emitter: Option<ModelStatusEmitter>,
    ) -> Self {
        let app_emitter: Option<ModelStatusEmitter> = emitter.or_else(|| {
            app.map(|handle| {
                Arc::new(move |payload: ModelSetupStatus| {
                    if let Err(e) = emit_ipc(&handle, IpcEvent::ModelProgress(payload)) {
                        log::warn!("[ModelManager] Failed to emit model_progress: {}", e);
                    }
                }) as ModelStatusEmitter
            })
        });

        let client = Client::builder()
            .read_timeout(Duration::from_secs(MODEL_DOWNLOAD_IDLE_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(MODEL_CONNECT_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            app_emitter,
            client,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        log::info!("[ModelManager] Cancellation requested.");
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    /// Sets up a model based on the manifest entry.
    pub async fn setup_model(
        &self,
        entry: &ModelEntry,
        base_url: &str,
        models_dir: &Path,
    ) -> anyhow::Result<()> {
        let model_id = &entry.id;
        let url = format!("{}/{}", base_url, entry.path);
        let dest_path = models_dir.join(&entry.path);
        let verified_path = dest_path.with_extension("verified");

        log::info!("[ModelManager] Starting setup for: {} ({})", model_id, url);
        self.cancel_flag.store(false, Ordering::Relaxed);

        if verified_path.exists() {
            if let Ok(marker) = VerifiedMarker::load_async(&verified_path).await {
                if marker.sha256 == entry.sha256
                    && (entry.archive_type.is_some() || dest_path.exists())
                {
                    log::info!(
                        "[ModelManager] Model/Archive {} already verified. Skipping setup.",
                        model_id
                    );
                    self.emit_status(
                        model_id,
                        SetupStep::Completed,
                        100.0,
                        entry.size_bytes,
                        entry.size_bytes,
                        None,
                    );
                    return Ok(());
                }
            }
        }

        self.cleanup_old_versions(model_id, &entry.sha256, models_dir);

        if let Some(parent) = dest_path.parent() {
            create_dir_all(parent)?;
        }

        let temp_path = dest_path.with_extension("tmp");

        // Bytes still on disk after any partial file is counted.
        let already_downloaded = existing_length(&temp_path).await;
        if already_downloaded > 0 {
            log::info!(
                "[ModelManager] Resuming {} from byte {} of {}",
                model_id,
                already_downloaded,
                entry.size_bytes
            );
        }

        ensure_free_space(entry, already_downloaded, models_dir)?;

        self.emit_status(
            model_id,
            SetupStep::Downloading,
            progress_percent(already_downloaded, entry.size_bytes),
            already_downloaded,
            entry.size_bytes,
            None,
        );

        let hash = match self
            .download_and_hash(model_id, &url, &temp_path, entry.size_bytes)
            .await
        {
            Ok(h) => h,
            Err(e) => {
                let step = if self.cancel_flag.load(Ordering::Relaxed) {
                    SetupStep::Cancelled
                } else {
                    SetupStep::Failed
                };
                self.emit_status(
                    model_id,
                    step,
                    progress_percent(already_downloaded, entry.size_bytes),
                    already_downloaded,
                    entry.size_bytes,
                    Some(e.to_string()),
                );
                // The partial `.tmp` is deliberately RETAINED. Deleting it here
                // is what makes every interrupted download restart from byte
                // zero — the exact defect resumable transfer exists to fix. It
                // is discarded only when it is provably unusable (hash mismatch,
                // or a `416` telling us it is already complete and wrong).
                return Err(e);
            }
        };

        self.emit_status(
            model_id,
            SetupStep::Verifying,
            100.0,
            entry.size_bytes,
            entry.size_bytes,
            None,
        );
        if hash != entry.sha256 {
            let mut err = format!(
                "Hash mismatch for {}. Expected {}, got {}",
                model_id, entry.sha256, hash
            );

            if entry.size_bytes > 1024 && entry.sha256.len() == 64 {
                log::warn!("[ModelManager] Hash mismatch detected. Diagnostic: If you are using Git LFS, ensure your models_manifest.json contains hashes of smudged files, not pointer files.");
                err.push_str("\n\nTip: Manifest hash may be for an LFS pointer. Re-verify models_manifest.json.");
            }

            self.emit_status(
                model_id,
                SetupStep::Failed,
                100.0,
                entry.size_bytes,
                entry.size_bytes,
                Some(err.clone()),
            );
            if let Err(cleanup_err) = remove_file(&temp_path) {
                log::debug!("[ModelManager] Temp file cleanup notice: {}", cleanup_err);
            }
            return Err(anyhow::anyhow!(err));
        }

        if let Some(ref archive_type) = entry.archive_type {
            self.emit_status(
                model_id,
                SetupStep::Extracting,
                100.0,
                entry.size_bytes,
                entry.size_bytes,
                None,
            );

            let extract_dest = if let Some(parent) = Path::new(&entry.path).parent() {
                if parent.as_os_str().is_empty() {
                    models_dir.to_path_buf()
                } else {
                    models_dir.join(parent)
                }
            } else {
                models_dir.to_path_buf()
            };

            let temp_path_clone = temp_path.clone();
            let archive_type_clone = archive_type.clone();
            let extract_dest_clone = extract_dest.clone();

            let extract_res = tauri::async_runtime::spawn_blocking(move || {
                Self::do_extract(&temp_path_clone, &archive_type_clone, &extract_dest_clone)
            })
            .await;

            match extract_res {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    self.emit_status(
                        model_id,
                        SetupStep::Failed,
                        100.0,
                        entry.size_bytes,
                        entry.size_bytes,
                        Some(e.to_string()),
                    );
                    if let Err(cleanup_err) = remove_file(&temp_path) {
                        log::debug!("[ModelManager] Temp file cleanup notice: {}", cleanup_err);
                    }
                    return Err(e);
                }
                Err(e) => {
                    let err = format!("Extraction task panicked: {}", e);
                    self.emit_status(
                        model_id,
                        SetupStep::Failed,
                        100.0,
                        entry.size_bytes,
                        entry.size_bytes,
                        Some(err.clone()),
                    );
                    if let Err(cleanup_err) = remove_file(&temp_path) {
                        log::debug!("[ModelManager] Temp file cleanup notice: {}", cleanup_err);
                    }
                    return Err(anyhow::anyhow!(err));
                }
            }
            if let Err(cleanup_err) = remove_file(&temp_path) {
                log::debug!("[ModelManager] Temp file cleanup notice: {}", cleanup_err);
            }
        } else {
            rename(&temp_path, &dest_path)?;
        }

        let marker = VerifiedMarker {
            model_id: Some(model_id.clone()),
            sha256: entry.sha256.clone(),
            verified_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
            expected_size: entry.size_bytes,
        };
        marker.save_async(&verified_path).await?;

        self.emit_status(
            model_id,
            SetupStep::Completed,
            100.0,
            entry.size_bytes,
            entry.size_bytes,
            None,
        );
        log::info!("[ModelManager] Setup completed for: {}", model_id);

        Ok(())
    }

    /// Downloads `url` into `dest`, resuming from any bytes already present, and
    /// returns the SHA-256 of the complete file.
    ///
    /// A partial `dest` from a previous run is continued rather than discarded.
    /// The partial file is preserved on every failure so an interrupted transfer
    /// never restarts from zero; it is deleted by the caller only when it is
    /// provably unusable.
    async fn download_and_hash(
        &self,
        model_id: &str,
        url: &str,
        dest: &Path,
        expected_size: u64,
    ) -> anyhow::Result<String> {
        let resume_offset = existing_length(dest).await;
        let response = match self.request_transfer(url, resume_offset).await? {
            TransferOutcome::Body(response) => response,
            TransferOutcome::StalePartial => {
                log::warn!(
                    "[ModelManager] {} partial file is already at or past full size and the server rejected the range; discarding it.",
                    model_id
                );
                if let Err(e) = tokio::fs::remove_file(dest).await {
                    log::warn!("[ModelManager] Failed to discard stale partial file: {}", e);
                    return Err(anyhow::anyhow!(
                        "Partial download is unusable and could not be discarded: {}",
                        e
                    ));
                }
                match self.request_transfer(url, 0).await? {
                    TransferOutcome::Body(response) => response,
                    TransferOutcome::StalePartial => {
                        return Err(anyhow::anyhow!("Server refused a full download of {}", url))
                    }
                }
            }
        };

        let append_from = resume_offset_for(response.status(), resume_offset);
        if append_from != resume_offset {
            log::warn!(
                "[ModelManager] Server ignored the range request for {}; restarting from byte 0.",
                model_id
            );
        }

        let mut file = open_for_append(dest, append_from).await?;
        let mut hasher = seed_hasher(dest, append_from).await?;
        let mut downloaded = append_from;
        let mut stream = response.bytes_stream();
        let mut last_emit = Instant::now();

        while let Some(chunk_result) = stream.next().await {
            if self.cancel_flag.load(Ordering::Relaxed) {
                return Err(anyhow::anyhow!("Cancelled"));
            }

            let chunk = chunk_result?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
            hasher.update(&chunk);
            downloaded += chunk.len() as u64;

            if last_emit.elapsed() > Duration::from_millis(PROGRESS_EMIT_INTERVAL_MS) {
                let progress = progress_percent(downloaded, expected_size);
                self.emit_status(
                    model_id,
                    SetupStep::Downloading,
                    progress,
                    downloaded,
                    expected_size,
                    None,
                );
                last_emit = Instant::now();
            }
        }

        tokio::io::AsyncWriteExt::flush(&mut file).await?;
        let hash = format!("{:x}", hasher.finalize());
        Ok(hash)
    }

    /// Issues the transfer request, adding a `Range` header when resuming.
    ///
    /// A `416` is not an error here: it means the partial file is already at or
    /// past the full size, which is a stale or corrupt partial. The caller
    /// discards it and retries once from zero.
    async fn request_transfer(
        &self,
        url: &str,
        resume_offset: u64,
    ) -> anyhow::Result<TransferOutcome> {
        let mut request = self.client.get(url);
        if resume_offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={}-", resume_offset));
        }

        let response = request.send().await?;
        match response.status() {
            StatusCode::RANGE_NOT_SATISFIABLE if resume_offset > 0 => {
                Ok(TransferOutcome::StalePartial)
            }
            status if status.is_success() => Ok(TransferOutcome::Body(response)),
            status => Err(anyhow::anyhow!("Server returned status {}", status)),
        }
    }

    pub fn do_extract(
        archive_path: &Path,
        archive_type: &str,
        dest_dir: &Path,
    ) -> anyhow::Result<()> {
        let file = File::open(archive_path)?;

        match archive_type {
            "zip" => {
                let mut archive = zip::ZipArchive::new(file)?;
                for i in 0..archive.len() {
                    let mut entry = archive.by_index(i)?;
                    let enclosed = match entry.enclosed_name() {
                        Some(path) => path.to_owned(),
                        None => {
                            return Err(anyhow::anyhow!(
                                "Zip-Slip vulnerability detected: illegal file path in archive"
                            ));
                        }
                    };
                    let outpath = dest_dir.join(enclosed);

                    if entry.name().ends_with('/') {
                        create_dir_all(&outpath)?;
                    } else {
                        if let Some(parent) = outpath.parent() {
                            if !parent.exists() {
                                create_dir_all(parent)?;
                            }
                        }
                        let mut outfile = File::create(&outpath)?;
                        copy(&mut entry, &mut outfile)?;
                    }
                }
            }
            "tar.gz" | "tgz" => {
                let tar_gz = flate2::read::GzDecoder::new(file);
                let mut archive = tar::Archive::new(tar_gz);
                for entry_res in archive.entries()? {
                    let mut entry = entry_res?;
                    let path = entry.path()?;
                    if path.components().any(|c| c == Component::ParentDir) {
                        return Err(anyhow::anyhow!(
                            "Tar-Slip vulnerability detected: illegal path traversal in archive: {:?}",
                            path
                        ));
                    }
                    entry.unpack_in(dest_dir)?;
                }
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported archive type: {}",
                    archive_type
                ))
            }
        }
        Ok(())
    }

    fn emit_status(
        &self,
        model_id: &str,
        step: SetupStep,
        progress: f32,
        bytes: u64,
        total: u64,
        error: Option<String>,
    ) {
        if let Some(ref emitter) = self.app_emitter {
            emitter(ModelSetupStatus {
                model_id: model_id.to_string(),
                step,
                progress,
                bytes_downloaded: bytes,
                total_bytes: total,
                error,
            });
        }
    }

    fn cleanup_old_versions(&self, model_id: &str, current_sha: &str, models_dir: &Path) {
        log::info!(
            "[ModelManager] Checking for old versions of model: {}",
            model_id
        );

        let walk_dir = |dir: &Path| -> Vec<PathBuf> {
            let mut verified_files = Vec::new();
            let mut stack = vec![dir.to_path_buf()];

            while let Some(current_dir) = stack.pop() {
                if let Ok(entries) = read_dir(current_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            stack.push(path);
                        } else if path.extension().is_some_and(|ext| ext == "verified") {
                            verified_files.push(path);
                        }
                    }
                }
            }
            verified_files
        };

        for verified_path in walk_dir(models_dir) {
            if let Ok(marker) = VerifiedMarker::load(&verified_path) {
                let matches_id = marker.model_id.as_ref().is_some_and(|id| id == model_id)
                    || verified_path
                        .file_stem()
                        .is_some_and(|stem| stem == model_id); // fallback to filename match

                if matches_id && marker.sha256 != current_sha {
                    log::info!(
                        "[ModelManager] Found outdated model version. Deleting files for: {} (Old Hash: {})", 
                        model_id, marker.sha256
                    );

                    let model_file_path = verified_path.with_extension("");
                    if model_file_path.exists() {
                        if model_file_path.is_dir() {
                            if let Err(e) = remove_dir_all(&model_file_path) {
                                log::warn!(
                                    "[ModelManager] Failed to remove outdated dir {:?}: {}",
                                    model_file_path,
                                    e
                                );
                            }
                        } else if let Err(e) = remove_file(&model_file_path) {
                            log::warn!(
                                "[ModelManager] Failed to remove outdated file {:?}: {}",
                                model_file_path,
                                e
                            );
                        }
                    }
                    if let Err(e) = remove_file(&verified_path) {
                        log::warn!(
                            "[ModelManager] Failed to remove outdated marker {:?}: {}",
                            verified_path,
                            e
                        );
                    }
                }
            }
        }
    }
}

/// Result of a single transfer request.
enum TransferOutcome {
    /// A usable response body, either a full body or a `206` partial.
    Body(reqwest::Response),
    /// The server reported the requested range as unsatisfiable.
    StalePartial,
}

/// Fails the download before it starts when the destination volume cannot hold it.
///
/// Reporting this up front beats letting a multi-gigabyte transfer die partway
/// through with `ENOSPC`. An unknown free-space reading is not treated as a
/// failure — some platforms do not report it — so this never blocks a download
/// that would have succeeded.
fn ensure_free_space(
    entry: &ModelEntry,
    already_downloaded: u64,
    models_dir: &Path,
) -> anyhow::Result<()> {
    let remaining = entry.size_bytes.saturating_sub(already_downloaded);
    if remaining == 0 {
        return Ok(());
    }

    let available = match disk::available_space_at(models_dir) {
        Some(available) => available,
        None => {
            log::warn!(
                "[ModelManager] Free space for {:?} is unknown; proceeding without a preflight check.",
                models_dir
            );
            return Ok(());
        }
    };

    if available < remaining {
        return Err(anyhow::anyhow!(
            "Not enough storage to download {}. Needs {} more, but only {} is free.",
            entry.id,
            format_bytes(remaining),
            format_bytes(available)
        ));
    }

    Ok(())
}

/// Renders a byte count for a user-facing error message.
fn format_bytes(bytes: u64) -> String {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.0} MB", bytes_f / MB)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Returns a 0–100 completion fraction, guarding against a zero-size entry.
fn progress_percent(downloaded: u64, expected_size: u64) -> f32 {
    if expected_size == 0 {
        return 100.0;
    }
    (downloaded as f32 / expected_size as f32 * 100.0).min(100.0)
}

/// Returns the byte length of an existing partial file, or 0 if there is none.
async fn existing_length(dest: &Path) -> u64 {
    match tokio::fs::metadata(dest).await {
        Ok(meta) => meta.len(),
        Err(e) => {
            log::debug!("[ModelManager] No partial file at {:?}: {}", dest, e);
            0
        }
    }
}

/// Returns the offset the response body should be written at.
///
/// A `206` continues from the existing partial. Any other successful status —
/// notably a `200` from a server that ignored the `Range` header — carries the
/// **whole** body, so it must be written from zero. Appending to a `200` would
/// duplicate every byte already on disk and silently corrupt the file.
fn resume_offset_for(status: StatusCode, resume_offset: u64) -> u64 {
    if status == StatusCode::PARTIAL_CONTENT {
        resume_offset
    } else {
        0
    }
}

/// Opens the partial file positioned to append at `offset`, creating it if absent.
async fn open_for_append(dest: &Path, offset: u64) -> anyhow::Result<tokio::fs::File> {
    let mut options = tokio::fs::OpenOptions::new();
    options.create(true).write(true);
    let file = if offset > 0 {
        options.append(true).open(dest).await?
    } else {
        options.truncate(true).open(dest).await?
    };
    Ok(file)
}

/// Builds a hasher primed with the bytes already on disk.
///
/// SHA-256 cannot be computed over an appended file from the tail alone: the
/// hash covers the whole byte stream, so the existing prefix must be hashed
/// before newly-received bytes are folded in. Without this, a resumed download
/// hashes only the tail and fails verification against the manifest.
async fn seed_hasher(dest: &Path, offset: u64) -> anyhow::Result<Sha256> {
    let mut hasher = Sha256::new();
    if offset == 0 {
        return Ok(hasher);
    }

    let mut file = File::open(dest)?;
    let mut buf = vec![0u8; HASH_PREFIX_BUFFER_BYTES];
    let mut remaining = offset;

    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let read = file.read(&mut buf[..want])?;
        if read == 0 {
            // The partial file changed underneath us. Appending at `offset` would
            // then disagree with the hash, so fail rather than produce a file
            // that can never verify. The partial is left intact and the next run
            // discards or resumes it from a consistent state.
            return Err(anyhow::anyhow!(
                "Partial download shrank from {} bytes to {} while being resumed",
                offset,
                offset - remaining
            ));
        }
        hasher.update(&buf[..read]);
        remaining -= read as u64;
    }

    Ok(hasher)
}
