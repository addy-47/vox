use crate::setup::manifest::{AppManifest, VoxManifest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReport {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_notes: Vec<String>,
    pub update_command: String,
}

/// Helper function to perform robust semver comparisons without external crates.
fn is_newer_version(remote: &str, local: &str) -> bool {
    let remote_clean = remote.split('-').next().unwrap_or(remote);
    let local_clean = local.split('-').next().unwrap_or(local);

    let remote_parts: Vec<&str> = remote_clean.split('.').collect();
    let local_parts: Vec<&str> = local_clean.split('.').collect();

    for i in 0..std::cmp::max(remote_parts.len(), local_parts.len()) {
        let r_val: u32 = remote_parts
            .get(i)
            .and_then(|&s| s.parse().ok())
            .unwrap_or(0);
        let l_val: u32 = local_parts
            .get(i)
            .and_then(|&s| s.parse().ok())
            .unwrap_or(0);

        if r_val > l_val {
            return true;
        } else if r_val < l_val {
            return false;
        }
    }
    if remote.contains('-') && !local.contains('-') {
        return false;
    }
    if !remote.contains('-') && local.contains('-') {
        return true;
    }
    false
}

/// Helper to load the cached AppManifest, falling back to network fetch if not cached.
async fn get_app_manifest() -> anyhow::Result<AppManifest> {
    let cache_path = crate::utils::paths::get().cache.join("app_manifest.json");
    if cache_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_path) {
            if let Ok(m) = serde_json::from_str::<AppManifest>(&content) {
                return Ok(m);
            }
        }
    }
    let m = AppManifest::fetch().await?;
    if let Ok(serialized) = serde_json::to_string_pretty(&m) {
        if let Some(parent) = cache_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!("[UpdateCheck] Failed to create cache directory: {}", e);
            }
        }
        if let Err(e) = std::fs::write(&cache_path, serialized) {
            log::warn!("[UpdateCheck] Failed to write app manifest cache: {}", e);
        }
    }
    Ok(m)
}

/// Helper to load the cached VoxManifest, falling back to network fetch if not cached.
async fn get_models_manifest() -> anyhow::Result<VoxManifest> {
    let cache_path = crate::utils::paths::get()
        .cache
        .join("models_manifest.json");
    if cache_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_path) {
            if let Ok(m) = serde_json::from_str::<VoxManifest>(&content) {
                return Ok(m);
            }
        }
    }
    let m = VoxManifest::fetch().await?;
    if let Ok(serialized) = serde_json::to_string_pretty(&m) {
        if let Some(parent) = cache_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!("[UpdateCheck] Failed to create cache directory: {}", e);
            }
        }
        if let Err(e) = std::fs::write(&cache_path, serialized) {
            log::warn!("[UpdateCheck] Failed to write models manifest cache: {}", e);
        }
    }
    Ok(m)
}

/// Performs a version comparison check by fetching the remote AppManifest from GitHub Pages
pub async fn check_app_updates() -> anyhow::Result<UpdateReport> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let manifest = get_app_manifest().await?;

    let update_available = is_newer_version(&manifest.latest_version, &current_version);

    Ok(UpdateReport {
        current_version,
        latest_version: manifest.latest_version,
        update_available,
        release_notes: manifest.release_notes,
        update_command: manifest.linux.update_command,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUpdateReport {
    pub local_version: String,
    pub remote_version: String,
    pub update_available: bool,
    pub outdated_models: Vec<String>,
}

/// Performs a version and individual model checksum comparison check against HF remote manifest.
pub async fn check_model_updates() -> anyhow::Result<ModelUpdateReport> {
    let local_manifest_path = crate::utils::paths::get()
        .models
        .join("models_manifest.json");

    let local_version = if local_manifest_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&local_manifest_path) {
            if let Ok(m) = serde_json::from_str::<VoxManifest>(&content) {
                m.models_version
            } else {
                "0.0.0".to_string()
            }
        } else {
            "0.0.0".to_string()
        }
    } else {
        "0.0.0".to_string()
    };

    let remote_manifest = get_models_manifest().await?;
    let remote_version = remote_manifest.models_version.clone();

    let mut outdated_models = Vec::new();
    let models_dir = crate::utils::paths::get().models.clone();

    for group in &remote_manifest.model_groups {
        let mut group_ok = true;
        for file in &group.files {
            let is_archive = file.archive_type.is_some();
            let dest_path = if is_archive {
                let p_str = file.path.as_str();
                if let Some(stripped) = p_str.strip_suffix(".tar.gz") {
                    models_dir.join(stripped)
                } else if let Some(stripped) = p_str
                    .strip_suffix(".zip")
                    .or_else(|| p_str.strip_suffix(".tgz"))
                {
                    models_dir.join(stripped)
                } else {
                    models_dir.join(&file.path)
                }
            } else {
                models_dir.join(&file.path)
            };

            let verified_path = models_dir.join(&file.path).with_extension("verified");

            let mut file_ok = false;
            if verified_path.exists() {
                if let Ok(marker) = crate::setup::manifest::VerifiedMarker::load(&verified_path) {
                    if marker.sha256 == file.sha256 && dest_path.exists() {
                        file_ok = true;
                    }
                }
            }
            if !file_ok {
                group_ok = false;
                break;
            }
        }
        if !group_ok {
            outdated_models.push(group.name.clone());
        }
    }

    let update_available =
        is_newer_version(&remote_version, &local_version) || !outdated_models.is_empty();

    Ok(ModelUpdateReport {
        local_version,
        remote_version,
        update_available,
        outdated_models,
    })
}
