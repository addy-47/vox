use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalHardwareGpuInfo {
    pub has_gpu: bool,
    pub vendor: String,
    pub device_name: String,
    pub resolved_tier: String,
}

/// Standalone boot-time hardware GPU detector.
/// Evaluates local GPU presence (CUDA, ROCm, Metal, Vulkan) independently of network LLM probes.
pub fn detect_local_gpu() -> LocalHardwareGpuInfo {
    #[cfg(target_os = "macos")]
    {
        LocalHardwareGpuInfo {
            has_gpu: true,
            vendor: "Apple".to_string(),
            device_name: "Apple Silicon (Metal)".to_string(),
            resolved_tier: "Tier 1B (Local GPU Available)".to_string(),
        }
    }

    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/dev/nvidia0").exists()
            || std::path::Path::new("/dev/nvidiactl").exists()
        {
            return LocalHardwareGpuInfo {
                has_gpu: true,
                vendor: "Nvidia".to_string(),
                device_name: "NVIDIA CUDA GPU".to_string(),
                resolved_tier: "Tier 1B (Local GPU Available)".to_string(),
            };
        }

        if std::path::Path::new("/dev/dri/renderD128").exists() {
            return LocalHardwareGpuInfo {
                has_gpu: true,
                vendor: "DRI/Linux".to_string(),
                device_name: "Linux Direct Rendering Device (/dev/dri/renderD128)".to_string(),
                resolved_tier: "Tier 1B (Local GPU Available)".to_string(),
            };
        }

        LocalHardwareGpuInfo {
            has_gpu: false,
            vendor: "None".to_string(),
            device_name: "CPU Only".to_string(),
            resolved_tier: "Tier 1A (CPU Only)".to_string(),
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Probe GPU via `wmic` — zero new dependencies, stdlib subprocess only.
        // wmic path Win32_VideoController get Name /value returns lines like: Name=NVIDIA GeForce RTX 3060
        let probe = std::process::Command::new("wmic")
            .args(["path", "Win32_VideoController", "get", "Name", "/value"])
            .output();

        if let Ok(out) = probe {
            let text = String::from_utf8_lossy(&out.stdout);
            // Find the first non-empty Name= line
            for line in text.lines() {
                let line = line.trim();
                if let Some(name_raw) = line.strip_prefix("Name=") {
                    let name = name_raw.trim().to_string();
                    if name.is_empty() {
                        continue;
                    }
                    // Classify vendor by keyword matching
                    let name_lower = name.to_lowercase();
                    let (has_gpu, vendor, tier) = if name_lower.contains("nvidia") {
                        (true, "NVIDIA", "Tier 1B (Local GPU Available)")
                    } else if name_lower.contains("amd") || name_lower.contains("radeon") {
                        (true, "AMD", "Tier 1B (Local GPU Available)")
                    } else if name_lower.contains("intel")
                        && (name_lower.contains("arc") || name_lower.contains("xe"))
                    {
                        (true, "Intel", "Tier 1B (Local GPU Available)")
                    } else if name_lower.contains("microsoft basic")
                        || name_lower.contains("virtual")
                        || name_lower.contains("llvm")
                    {
                        // Virtual/software renderer — treat as CPU-only
                        (false, "Software", "Tier 1A (CPU Only)")
                    } else {
                        // Unknown adapter present — conservatively flag as available
                        (true, "Unknown", "Tier 1B (Local GPU Available)")
                    };

                    return LocalHardwareGpuInfo {
                        has_gpu,
                        vendor: vendor.to_string(),
                        device_name: name,
                        resolved_tier: tier.to_string(),
                    };
                }
            }
        }

        // wmic failed or returned no adapter — fall back to CPU-only
        LocalHardwareGpuInfo {
            has_gpu: false,
            vendor: "None".to_string(),
            device_name: "CPU Only (wmic probe failed)".to_string(),
            resolved_tier: "Tier 1A (CPU Only)".to_string(),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        LocalHardwareGpuInfo {
            has_gpu: false,
            vendor: "Unknown".to_string(),
            device_name: "CPU Only".to_string(),
            resolved_tier: "Tier 1A (CPU Only)".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_local_gpu() {
        let info = detect_local_gpu();
        assert!(!info.device_name.is_empty());
        assert!(!info.resolved_tier.is_empty());
    }
}
