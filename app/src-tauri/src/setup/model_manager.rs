use super::{MODEL_CONNECT_TIMEOUT_SECS, MODEL_DOWNLOAD_TIMEOUT_SECS, PROGRESS_EMIT_INTERVAL_MS};
use crate::core::events::emit_ipc;
use crate::core::events::IpcEvent;
use crate::setup::manifest::{ModelEntry, VerifiedMarker};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;

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

/// Orchestrates model downloads, extraction, and verification.
pub struct ModelManager {
    app_emitter: Option<ModelStatusEmitter>,
    client: Client,
    pub cancel_flag: Arc<AtomicBool>,
}

impl ModelManager {
    pub fn new<R: tauri::Runtime>(app: Option<AppHandle<R>>) -> Self {
        let app_emitter: Option<ModelStatusEmitter> = app.map(|handle| {
            Arc::new(move |payload: ModelSetupStatus| {
                if let Err(e) = emit_ipc(&handle, IpcEvent::ModelProgress(payload)) {
                    log::warn!("[ModelManager] Failed to emit model_progress: {}", e);
                }
            }) as ModelStatusEmitter
        });

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(MODEL_DOWNLOAD_TIMEOUT_SECS))
            .connect_timeout(std::time::Duration::from_secs(MODEL_CONNECT_TIMEOUT_SECS))
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
            std::fs::create_dir_all(parent)?;
        }

        self.emit_status(
            model_id,
            SetupStep::Downloading,
            0.0,
            0,
            entry.size_bytes,
            None,
        );

        let temp_path = dest_path.with_extension("tmp");
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
                    0.0,
                    0,
                    entry.size_bytes,
                    Some(e.to_string()),
                );
                if let Err(err) = std::fs::remove_file(&temp_path) {
                    log::debug!("[ModelManager] Temp file cleanup notice: {}", err);
                }
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
            if let Err(cleanup_err) = std::fs::remove_file(&temp_path) {
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
                    if let Err(cleanup_err) = std::fs::remove_file(&temp_path) {
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
                    if let Err(cleanup_err) = std::fs::remove_file(&temp_path) {
                        log::debug!("[ModelManager] Temp file cleanup notice: {}", cleanup_err);
                    }
                    return Err(anyhow::anyhow!(err));
                }
            }
            if let Err(cleanup_err) = std::fs::remove_file(&temp_path) {
                log::debug!("[ModelManager] Temp file cleanup notice: {}", cleanup_err);
            }
        } else {
            std::fs::rename(&temp_path, &dest_path)?;
        }

        let marker = VerifiedMarker {
            model_id: Some(model_id.clone()),
            sha256: entry.sha256.clone(),
            verified_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis() as u64,
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

    async fn download_and_hash(
        &self,
        model_id: &str,
        url: &str,
        dest: &Path,
        expected_size: u64,
    ) -> anyhow::Result<String> {
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Server returned status {}",
                response.status()
            ));
        }

        let mut file = tokio::fs::File::create(dest).await?;
        let mut hasher = Sha256::new();
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let mut last_emit = std::time::Instant::now();

        while let Some(chunk_result) = stream.next().await {
            if self.cancel_flag.load(Ordering::Relaxed) {
                return Err(anyhow::anyhow!("Cancelled"));
            }

            let chunk = chunk_result?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
            hasher.update(&chunk);
            downloaded += chunk.len() as u64;

            if last_emit.elapsed() > std::time::Duration::from_millis(PROGRESS_EMIT_INTERVAL_MS) {
                let progress = (downloaded as f32 / expected_size as f32) * 100.0;
                self.emit_status(
                    model_id,
                    SetupStep::Downloading,
                    progress,
                    downloaded,
                    expected_size,
                    None,
                );
                last_emit = std::time::Instant::now();
            }
        }

        tokio::io::AsyncWriteExt::flush(&mut file).await?;
        let hash = format!("{:x}", hasher.finalize());
        Ok(hash)
    }

    pub fn do_extract(
        archive_path: &Path,
        archive_type: &str,
        dest_dir: &Path,
    ) -> anyhow::Result<()> {
        let file = std::fs::File::open(archive_path)?;

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
                        std::fs::create_dir_all(&outpath)?;
                    } else {
                        if let Some(parent) = outpath.parent() {
                            if !parent.exists() {
                                std::fs::create_dir_all(parent)?;
                            }
                        }
                        let mut outfile = std::fs::File::create(&outpath)?;
                        std::io::copy(&mut entry, &mut outfile)?;
                    }
                }
            }
            "tar.gz" | "tgz" => {
                let tar_gz = flate2::read::GzDecoder::new(file);
                let mut archive = tar::Archive::new(tar_gz);
                for entry_res in archive.entries()? {
                    let mut entry = entry_res?;
                    let path = entry.path()?;
                    if path
                        .components()
                        .any(|c| c == std::path::Component::ParentDir)
                    {
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

        let walk_dir = |dir: &Path| -> Vec<std::path::PathBuf> {
            let mut verified_files = Vec::new();
            let mut stack = vec![dir.to_path_buf()];

            while let Some(current_dir) = stack.pop() {
                if let Ok(entries) = std::fs::read_dir(current_dir) {
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
                            if let Err(e) = std::fs::remove_dir_all(&model_file_path) {
                                log::warn!(
                                    "[ModelManager] Failed to remove outdated dir {:?}: {}",
                                    model_file_path,
                                    e
                                );
                            }
                        } else if let Err(e) = std::fs::remove_file(&model_file_path) {
                            log::warn!(
                                "[ModelManager] Failed to remove outdated file {:?}: {}",
                                model_file_path,
                                e
                            );
                        }
                    }
                    if let Err(e) = std::fs::remove_file(&verified_path) {
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
