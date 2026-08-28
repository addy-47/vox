pub mod audio_filters;
pub mod hardware;
pub mod json;
pub mod logging;
pub mod paths;

/// Checks the Linux CPU frequency governor. Returns `true` if it's "performance",
pub fn check_cpu_governor() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let path = std::path::Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor");
        if !path.exists() {
            log::warn!("[CPU Governor] scaling_governor not found at {:?}", path);
            return None;
        }
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let governor = content.trim().to_lowercase();
                Some(governor)
            }
            Err(e) => {
                log::warn!("[CPU Governor] Failed to read scaling_governor: {}", e);
                None
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
