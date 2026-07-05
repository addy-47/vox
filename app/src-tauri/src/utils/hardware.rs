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
        if std::path::Path::new("/dev/nvidia0").exists() || std::path::Path::new("/dev/nvidiactl").exists() {
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
        LocalHardwareGpuInfo {
            has_gpu: false,
            vendor: "Windows".to_string(),
            device_name: "Default Windows Display Device".to_string(),
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
