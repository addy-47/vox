use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

/// System audio device descriptor with default designation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

static INPUT_DEVICE_CACHE: parking_lot::Mutex<Option<(Instant, Vec<AudioDevice>)>> =
    parking_lot::Mutex::new(None);
static OUTPUT_DEVICE_CACHE: parking_lot::Mutex<Option<(Instant, Vec<AudioDevice>)>> =
    parking_lot::Mutex::new(None);

const DEVICE_CACHE_TTL: Duration = Duration::from_secs(5);

use crate::core::error::VoxIpcError;

/// Enumerate available host audio input or output devices, filtering out virtual and monitor devices.
#[tauri::command]
pub async fn list_audio_devices(kind: Option<String>) -> Result<Vec<AudioDevice>, VoxIpcError> {
    let is_output = kind.as_deref().unwrap_or("input").to_lowercase() == "output";

    if is_output {
        let cache = OUTPUT_DEVICE_CACHE.lock();
        if let Some((instant, ref devices)) = *cache {
            if instant.elapsed() < DEVICE_CACHE_TTL {
                return Ok(devices.clone());
            }
        }
    } else {
        let cache = INPUT_DEVICE_CACHE.lock();
        if let Some((instant, ref devices)) = *cache {
            if instant.elapsed() < DEVICE_CACHE_TTL {
                return Ok(devices.clone());
            }
        }
    }

    let result = tokio::task::spawn_blocking(move || {
        let host = cpal::default_host();
        let mut result = Vec::new();

        if is_output {
            let devices = host.output_devices().map_err(|e| {
                VoxIpcError::Engine(format!("Failed to list output devices: {}", e))
            })?;
            let default_device = host.default_output_device().and_then(|d| d.name().ok());

            for device in devices {
                if let Ok(name) = device.name() {
                    if is_virtual_device(&name) {
                        continue;
                    }
                    if let Ok(mut configs) = device.supported_output_configs() {
                        if configs.next().is_some() {
                            result.push(AudioDevice {
                                is_default: Some(&name) == default_device.as_ref(),
                                name,
                            });
                        }
                    }
                }
            }
        } else {
            let devices = host
                .input_devices()
                .map_err(|e| VoxIpcError::Engine(format!("Failed to list input devices: {}", e)))?;
            let default_device = host.default_input_device().and_then(|d| d.name().ok());

            for device in devices {
                if let Ok(name) = device.name() {
                    if is_virtual_device(&name) || name.to_lowercase().contains("monitor") {
                        continue;
                    }
                    if let Ok(mut configs) = device.supported_input_configs() {
                        if configs.next().is_some() {
                            result.push(AudioDevice {
                                is_default: Some(&name) == default_device.as_ref(),
                                name,
                            });
                        }
                    }
                }
            }
        }

        result.sort_by(|a, b| b.is_default.cmp(&a.is_default));
        Ok::<Vec<AudioDevice>, VoxIpcError>(result)
    })
    .await
    .map_err(|e| VoxIpcError::Internal(format!("Task panicked: {}", e)))??;

    if is_output {
        *OUTPUT_DEVICE_CACHE.lock() = Some((Instant::now(), result.clone()));
    } else {
        *INPUT_DEVICE_CACHE.lock() = Some((Instant::now(), result.clone()));
    }

    Ok(result)
}

/// Checks if an audio device name matches virtual or monitor dummy names.
fn is_virtual_device(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    name_lower.contains("null")
        || name_lower.contains("loopback")
        || name_lower.contains("dummy")
        || name_lower.contains("virtual")
}
