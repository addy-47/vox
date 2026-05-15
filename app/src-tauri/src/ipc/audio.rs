use serde::{Serialize, Deserialize};
use cpal::traits::{HostTrait, DeviceTrait};

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

#[tauri::command]
pub async fn list_input_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    let default_device = host.default_input_device().and_then(|d| d.name().ok());

    let mut result = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            let name_lower = name.to_lowercase();
            if name_lower.contains("monitor") || 
               name_lower.contains("null") || 
               name_lower.contains("loopback") || 
               name_lower.contains("dummy") ||
               name_lower.contains("virtual") {
                continue;
            }

            // Check if device actually supports any input configurations
            if let Ok(mut configs) = device.supported_input_configs() {
                if configs.next().is_some() {
                    result.push(AudioDevice {
                        is_default: Some(name.clone()) == default_device,
                        name,
                    });
                }
            }
        }
    }
    
    // Sort so default is first
    result.sort_by(|a, b| b.is_default.cmp(&a.is_default));
    
    Ok(result)
}
