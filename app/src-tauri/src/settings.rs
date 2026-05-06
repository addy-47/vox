use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VoxSettings {
    pub stt_model_dir: PathBuf,
    pub vad_model_path: PathBuf,
    pub vad_threshold: f32,
    pub ptt_noise_gate: f32,
    pub theme: String,
}

impl Default for VoxSettings {
    fn default() -> Self {
        Self {
            stt_model_dir: PathBuf::from("assets/qwen3-asr"),
            vad_model_path: PathBuf::from("assets/ten_vad.onnx"),
            vad_threshold: 0.6,
            ptt_noise_gate: 0.005,
            theme: "dark".into(),
        }
    }
}

impl VoxSettings {
    pub fn load(config_dir: &PathBuf) -> Self {
        let path = config_dir.join("settings.json");
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str(&content) {
                return settings;
            }
        }
        
        // Return default if file doesn't exist or is invalid
        let settings = Self::default();
        let _ = settings.save(config_dir);
        settings
    }

    pub fn save(&self, config_dir: &PathBuf) -> Result<()> {
        if !config_dir.exists() {
            fs::create_dir_all(config_dir)?;
        }
        let path = config_dir.join("settings.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
