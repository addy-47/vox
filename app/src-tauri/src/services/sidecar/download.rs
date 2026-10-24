use std::{
    fs::{create_dir_all, File},
    io::{copy, Read},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};

use super::types::SidecarKind;
use crate::utils::paths::get as get_paths;

const SIDECARS_MANIFEST_URL: &str =
    "https://addy-47.github.io/vox/manifests/sidecars_manifest.json";

#[derive(serde::Deserialize)]
struct SidecarsManifest {
    sidecars: std::collections::HashMap<String, std::collections::HashMap<String, SidecarAsset>>,
}

#[derive(serde::Deserialize)]
struct SidecarAsset {
    url: String,
    #[serde(default)]
    sha256: String,
}

fn platform_key() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "linux-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "linux-aarch64";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "macos-aarch64";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "macos-x86_64";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "windows-x86_64";

    #[allow(unreachable_code)]
    "unsupported"
}

pub fn get_sidecar_binary_path(kind: SidecarKind) -> Result<PathBuf> {
    // 1. Check local environment variable override
    let env_var = format!(
        "VOX_SIDECAR_BIN_OVERRIDE_{}",
        kind.binary_name().replace('-', "_")
    );
    if let Ok(val) = std::env::var(&env_var) {
        let p = PathBuf::from(val);
        if p.exists() {
            log::info!("[Sidecar] Using override binary for {}: {:?}", kind.binary_name(), p);
            return Ok(p);
        }
    }

    // 2. Check ~/.vox/bin/<name>
    let bin_dir = get_paths().root.join("bin");
    create_dir_all(&bin_dir)?;

    let bin_name = if cfg!(windows) {
        format!("{}.exe", kind.binary_name())
    } else {
        kind.binary_name().to_string()
    };
    let bin_path = bin_dir.join(&bin_name);

    if bin_path.exists() {
        return Ok(bin_path);
    }

    // 3. Fallback: check workspace target directory in debug/dev builds
    let candidate = PathBuf::from("target/release").join(&bin_name);
    if candidate.exists() {
        return Ok(candidate);
    }
    let candidate_dbg = PathBuf::from("target/debug").join(&bin_name);
    if candidate_dbg.exists() {
        return Ok(candidate_dbg);
    }

    anyhow::bail!(
        "Sidecar binary {} not found on disk at {:?}. Run cargo build -p {} or download it.",
        kind.binary_name(),
        bin_path,
        kind.binary_name()
    )
}

pub async fn ensure_sidecar_binary(kind: SidecarKind) -> Result<PathBuf> {
    if let Ok(p) = get_sidecar_binary_path(kind) {
        return Ok(p);
    }

    let bin_dir = get_paths().root.join("bin");
    create_dir_all(&bin_dir)?;
    let bin_name = if cfg!(windows) {
        format!("{}.exe", kind.binary_name())
    } else {
        kind.binary_name().to_string()
    };
    let dest_bin = bin_dir.join(&bin_name);

    // Fetch manifest
    log::info!("[Sidecar] Downloading manifest from {}", SIDECARS_MANIFEST_URL);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    
    let manifest: SidecarsManifest = client
        .get(SIDECARS_MANIFEST_URL)
        .send()
        .await
        .context("Failed to fetch sidecars manifest")?
        .json()
        .await
        .context("Failed to parse sidecars manifest")?;

    let pkey = platform_key();
    let asset = manifest
        .sidecars
        .get(kind.binary_name())
        .and_then(|m| m.get(pkey))
        .ok_or_else(|| anyhow!("No sidecar asset for {} on platform {}", kind.binary_name(), pkey))?;

    log::info!("[Sidecar] Downloading {} from {}", kind.binary_name(), asset.url);
    let resp = client
        .get(&asset.url)
        .send()
        .await
        .context("Failed to download sidecar archive")?;

    let bytes = resp.bytes().await?;

    // Verify SHA-256 if provided in manifest
    if !asset.sha256.is_empty() {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());
        if hash != asset.sha256 {
            anyhow::bail!("SHA-256 mismatch for {}: expected {}, got {}", kind.binary_name(), asset.sha256, hash);
        }
    }

    // Extract archive
    if asset.url.ends_with(".tar.gz") {
        let tar_gz = flate2::read::GzDecoder::new(&bytes[..]);
        let mut archive = tar::Archive::new(tar_gz);
        archive.unpack(&bin_dir)?;
    } else if asset.url.ends_with(".zip") {
        let cursor = std::io::Cursor::new(&bytes);
        let mut zip = zip::ZipArchive::new(cursor)?;
        zip.extract(&bin_dir)?;
    } else {
        std::fs::write(&dest_bin, &bytes)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if dest_bin.exists() {
            let mut perms = std::fs::metadata(&dest_bin)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest_bin, perms)?;
        }
    }

    if !dest_bin.exists() {
        anyhow::bail!("Failed to locate extracted binary at {:?}", dest_bin);
    }

    Ok(dest_bin)
}
