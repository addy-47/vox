use std::{
    fs::{metadata, remove_file, write},
    path::Path,
};

// Only the desktop `check_mic_access` arm calls cpal's host; the Android arm
// asks the platform over JNI instead. Gating the trait keeps it off the mobile
// target graph rather than tripping `-D warnings`.
#[cfg(desktop)]
use cpal::traits::HostTrait;
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};

use crate::{
    config::VoxSettings,
    setup::manifest::{VerifiedMarker, VoxManifest},
    utils::paths,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeReport {
    pub write_access: bool,
    pub available_space_gb: f32,
    pub total_space_gb: f32,
    pub required_space_gb: f32,
    pub disk_space_ok: bool,
    pub disk_space_unknown: bool,
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

/// Disk space reading. `known` is false when no mount point matched the path,
/// in which case the numbers are zero and `ok` must not be read as a verdict.
struct DiskSpace {
    available_gb: f32,
    total_gb: f32,
    ok: bool,
    known: bool,
}

/// Performs system validation.
/// If a manifest is provided, it calculates required disk space dynamically.
/// Otherwise, it uses a conservative 6GB fallback as per Part 1/2 directives.
pub fn verify_runtime(manifest: Option<&VoxManifest>) -> RuntimeReport {
    let p = paths::get();

    let write_access = check_write_access(&p.root);
    let required_bytes = manifest
        .map(|m| m.calculate_required_space())
        .unwrap_or(6 * 1024 * 1024 * 1024);
    let required_gb = required_bytes as f32 / 1024.0 / 1024.0 / 1024.0;
    let disk = check_disk_space(&p.root, required_bytes);
    let mic_access = check_mic_access();
    let (ram_gb, cpu_cores) = get_system_info();
    let settings_exists = p.settings.exists();
    let models_dir_exists = p.models.exists();
    let (missing, models_verified) = check_model_integrity(&p.models, manifest);
    let setup_completed = p.settings.exists() && {
        let settings = VoxSettings::load();
        settings.system.setup_completed
    };

    RuntimeReport {
        write_access,
        available_space_gb: disk.available_gb,
        total_space_gb: disk.total_gb,
        required_space_gb: required_gb,
        disk_space_ok: disk.ok,
        disk_space_unknown: !disk.known,
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
    match write(&test_file, "vox") {
        Ok(_) => {
            if let Err(e) = remove_file(test_file) {
                log::debug!("[RuntimeCheck] Write test file cleanup notice: {}", e);
            }
            true
        }
        Err(_) => false,
    }
}

fn check_disk_space(path: &Path, required_bytes: u64) -> DiskSpace {
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
        return DiskSpace {
            available_gb,
            total_gb,
            ok: available >= required_bytes,
            known: true,
        };
    }
    for disk in &disks {
        if disk.mount_point() == Path::new("/") {
            let available = disk.available_space();
            let available_gb = available as f32 / 1024.0 / 1024.0 / 1024.0;
            let total_gb = disk.total_space() as f32 / 1024.0 / 1024.0 / 1024.0;
            return DiskSpace {
                available_gb,
                total_gb,
                ok: available >= required_bytes,
                known: true,
            };
        }
    }
    log::warn!(
        "[verify_runtime] Could not determine disk space for path {:?}. Failing verification.",
        path
    );
    DiskSpace {
        available_gb: 0.0,
        total_gb: 0.0,
        ok: false,
        known: false,
    }
}

fn get_system_info() -> (f32, u32) {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_ram_gb = sys.total_memory() as f32 / 1024.0 / 1024.0 / 1024.0;
    let cpu_cores = sys.cpus().len() as u32;

    (total_ram_gb, cpu_cores)
}

/// Whether the process holds a usable microphone.
///
/// Desktop asks cpal whether an input device exists. Android cannot ask that:
/// cpal's Oboe host implements `default_input_device()` as an unconditional
/// `Some(Device(None))`, so a device probe answers "yes" even with zero
/// permissions granted — which would render the permission prompt permanently
/// satisfied. Android therefore asks the platform for `RECORD_AUDIO` directly.
///
/// Both arms report `false` rather than guessing when the check itself cannot
/// be answered, because this field gates onboarding.
#[cfg(desktop)]
fn check_mic_access() -> bool {
    let host = crate::services::audio::resolve_audio_host();
    host.default_input_device().is_some()
}

#[cfg(not(desktop))]
fn check_mic_access() -> bool {
    android_record_audio_granted()
}

/// `PackageManager.PERMISSION_GRANTED`.
#[cfg(not(desktop))]
const ANDROID_PERMISSION_GRANTED: i32 = 0;

#[cfg(not(desktop))]
fn android_record_audio_granted() -> bool {
    std::panic::catch_unwind(|| {
        let ctx = ndk_context::android_context();
        // SAFETY: `android_context()` is initialized by MainActivity.onCreate via
        // `initAndroidContext` during application startup, before any command runs.
        // Both handles are read-only here and every fallible JNI call below is
        // handled rather than unwrapped.
        let vm = match unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) } {
            Ok(vm) => vm,
            Err(e) => {
                log::warn!(
                    "[RuntimeCheck] JavaVM unavailable; reporting mic as denied: {}",
                    e
                );
                return false;
            }
        };
        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(e) => {
                log::warn!(
                    "[RuntimeCheck] JNI attach failed; reporting mic as denied: {}",
                    e
                );
                return false;
            }
        };

        // `Context.checkSelfPermission(String)` is an instance method on the
        // application context, added in API 23. minSdk is 24, so it always exists.
        // jni 0.21 has no `&str -> JValue` conversion, so the `JString` is bound to a
        // local: dropping it earlier would free the local ref out from under the call.
        let permission = env.new_string("android.permission.RECORD_AUDIO");
        let granted = match permission {
            Ok(permission) => {
                // SAFETY: `ctx.context()` is the live application `Context` that
                // MainActivity retained at startup; it outlives this call.
                let context: jni::objects::JObject =
                    unsafe { jni::objects::JObject::from_raw(ctx.context() as jni::sys::jobject) };
                env.call_method(
                    &context,
                    "checkSelfPermission",
                    "(Ljava/lang/String;)I",
                    &[(&permission).into()],
                )
                .and_then(|v| v.i())
                .map(|code| code == ANDROID_PERMISSION_GRANTED)
            }
            Err(e) => {
                log::warn!("[RuntimeCheck] Could not build permission string: {}", e);
                return false;
            }
        };

        match granted {
            Ok(yes) => yes,
            Err(e) => {
                log::warn!(
                    "[RuntimeCheck] checkSelfPermission failed; reporting mic as denied: {}",
                    e
                );
                false
            }
        }
    })
    .unwrap_or_else(|e| {
        log::warn!(
            "[RuntimeCheck] Caught panic in android_record_audio_granted; reporting mic as denied: {:?}",
            e
        );
        false
    })
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

            if !model_path.exists() {
                missing.push(entry.id.clone());
                all_verified = false;
                continue;
            }

            if !is_archive {
                if let Ok(metadata) = metadata(&model_path) {
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
