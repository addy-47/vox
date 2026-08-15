use crate::setup::manifest::{VerifiedMarker, VoxManifest};
use crate::utils::paths;
use cpal::traits::HostTrait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeReport {
    pub write_access: bool,
    pub available_space_gb: f32,
    pub total_space_gb: f32,
    pub required_space_gb: f32,
    pub disk_space_ok: bool,
    pub mic_access: bool,
    pub ram_gb: f32,
    pub cpu_cores: u32,
    pub settings_exists: bool,
    pub models_dir_exists: bool,
    pub models_dir: String,
    pub models_missing: Vec<String>,
    pub models_verified: bool,
    pub setup_completed: bool,
}

/// Performs system validation.
///
/// If a manifest is provided, it calculates required disk space dynamically.
/// Otherwise, it uses a conservative 6GB fallback as per Part 1/2 directives.
pub fn verify_runtime(manifest: Option<&VoxManifest>) -> RuntimeReport {
    let p = paths::get();

    // 1. Write Access
    let write_access = check_write_access(&p.root);

    // 2. Disk Space Calculation
    let required_bytes = manifest
        .map(|m| m.calculate_required_space())
        .unwrap_or(6 * 1024 * 1024 * 1024); // Fallback to 6GB if no manifest

    let required_gb = required_bytes as f32 / 1024.0 / 1024.0 / 1024.0;
    let (available_gb, total_gb, space_ok) = check_disk_space(&p.root, required_bytes);

    // 3. Mic Access
    let mic_access = check_mic_access();

    // 3.5 System Resources
    let (ram_gb, cpu_cores) = get_system_info();

    // 4. File Existence
    let settings_exists = p.settings.exists();
    let models_dir_exists = p.models.exists();

    // 5. Model Verification
    let (missing, models_verified) = check_model_integrity(&p.models, manifest);

    // 6. Setup Status
    let setup_completed = p.settings.exists() && {
        let settings = crate::core::settings::VoxSettings::load();
        settings.setup.completed
    };

    RuntimeReport {
        write_access,
        available_space_gb: available_gb,
        total_space_gb: total_gb,
        required_space_gb: required_gb,
        disk_space_ok: space_ok,
        mic_access,
        ram_gb,
        cpu_cores,
        settings_exists,
        models_dir_exists,
        models_dir: p.models.to_string_lossy().to_string(),
        models_missing: missing,
        models_verified,
        setup_completed,
    }
}

fn check_write_access(path: &Path) -> bool {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            return check_write_access(parent);
        }
        return false;
    }

    let test_file = path.join(".write_test");
    match std::fs::write(&test_file, "vox") {
        Ok(_) => {
            let _ = std::fs::remove_file(test_file);
            true
        }
        Err(_) => false,
    }
}

fn check_disk_space(path: &Path, required_bytes: u64) -> (f32, f32, bool) {
    let disks = Disks::new_with_refreshed_list();

    let mut best_match: Option<(&Path, u64, u64)> = None;

    for disk in &disks {
        let mount = disk.mount_point();
        if path.starts_with(mount) {
            let mount_len = mount.to_string_lossy().len();
            if best_match.is_none() || mount_len > best_match.unwrap().0.to_string_lossy().len() {
                best_match = Some((mount, disk.available_space(), disk.total_space()));
            }
        }
    }

    if let Some((_, available, total)) = best_match {
        let available_gb = available as f32 / 1024.0 / 1024.0 / 1024.0;
        let total_gb = total as f32 / 1024.0 / 1024.0 / 1024.0;
        (available_gb, total_gb, available >= required_bytes)
    } else {
        // Fallback for cases where sysinfo fails to match a mount point (common in some containers/Linux setups)
        // We try to find the root "/" disk as a last resort.
        for disk in &disks {
            if disk.mount_point() == Path::new("/") {
                let available = disk.available_space();
                let available_gb = available as f32 / 1024.0 / 1024.0 / 1024.0;
                let total_gb = disk.total_space() as f32 / 1024.0 / 1024.0 / 1024.0;
                return (available_gb, total_gb, available >= required_bytes);
            }
        }
        // If still nothing, we assume it's OK but log a warning (risky, but better than a hard block on valid systems)
        log::warn!("[verify_runtime] Could not determine disk space for path {:?}. Proceeding with fallback.", path);
        (100.0, 100.0, true)
    }
}

fn get_system_info() -> (f32, u32) {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_ram_gb = sys.total_memory() as f32 / 1024.0 / 1024.0 / 1024.0;
    let cpu_cores = sys.cpus().len() as u32;

    (total_ram_gb, cpu_cores)
}

fn check_mic_access() -> bool {
    let host = cpal::default_host();
    host.default_input_device().is_some()
}

/// Checks if all models in the manifest exist and are verified.
/// If no manifest is provided, it returns ([], false) as we can't verify yet.
fn check_model_integrity(models_dir: &Path, manifest: Option<&VoxManifest>) -> (Vec<String>, bool) {
    let mut missing = Vec::new();
    let mut all_verified = true;

    let Some(m) = manifest else {
        return (Vec::new(), false);
    };

    for group in &m.model_groups {
        for entry in &group.files {
            let is_archive = entry.archive_type.is_some();
            let model_path = if is_archive {
                let p_str = entry.path.as_str();
                if let Some(stripped) = p_str.strip_suffix(".tar.gz") {
                    models_dir.join(stripped)
                } else if let Some(stripped) = p_str
                    .strip_suffix(".zip")
                    .or_else(|| p_str.strip_suffix(".tgz"))
                {
                    models_dir.join(stripped)
                } else {
                    models_dir.join(&entry.path)
                }
            } else {
                models_dir.join(&entry.path)
            };

            let verified_path = models_dir.join(&entry.path).with_extension("verified");

            // 1. Existence check
            if !model_path.exists() {
                missing.push(entry.id.clone());
                all_verified = false;
                continue;
            }

            // 2. Size check (skip for extracted archives since size on disk is different)
            if !is_archive {
                if let Ok(metadata) = std::fs::metadata(&model_path) {
                    if metadata.len() != entry.size_bytes {
                        missing.push(format!("{} (size mismatch)", entry.id));
                        all_verified = false;
                        continue;
                    }
                } else {
                    missing.push(entry.id.clone());
                    all_verified = false;
                    continue;
                }
            }

            // 3. Verified marker check
            if !verified_path.exists() {
                all_verified = false;
                continue;
            }

            if let Ok(marker) = VerifiedMarker::load(&verified_path) {
                if marker.sha256 != entry.sha256 || marker.expected_size != entry.size_bytes {
                    missing.push(format!("{} (corrupt marker)", entry.id));
                    all_verified = false;
                }
            } else {
                all_verified = false;
            }
        }
    }

    (missing, all_verified && !m.model_groups.is_empty())
}
