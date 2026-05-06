use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;
use crate::audio::AudioStream;
use crate::stt::SttCommand;
use crate::settings::VoxSettings;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionMode {
    Passive,
    Ptt,
}

pub struct VoxEngine {
    pub audio_stream: AudioStream,
    pub stt_tx: tokio::sync::mpsc::Sender<SttCommand>,
}

pub struct PttState {
    pub is_recording: Mutex<bool>,
    pub session_id: Mutex<u32>,
    pub audio_buffer: Mutex<Vec<f32>>,
    pub samples_since_partial: Mutex<usize>,
    pub samples_since_waveform: Mutex<usize>,
}

pub struct AppState {
    pub engine: Mutex<Option<VoxEngine>>,
    pub interaction: Mutex<InteractionMode>,
    pub hud_visible: Mutex<bool>,
    pub settings: Mutex<VoxSettings>,
    pub hud_menu_item: Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
    pub ptt: PttState,
    pub config_dir: std::path::PathBuf,
}


impl AppState {
    pub fn new(app: &AppHandle) -> Self {
        let config_dir = app.path().home_dir().map(|mut p| {
            p.push(".vox");
            p
        }).unwrap_or_else(|_| {
            let mut p = std::env::current_dir().unwrap_or_default();
            p.push(".vox");
            p
        });
        
        let settings = VoxSettings::load(&config_dir);
        
        Self {
            engine: Mutex::new(None),
            interaction: Mutex::new(InteractionMode::Passive),
            // Default to visible so passive mode works on first speech without
            // requiring the user to manually enable "Vox Live" from the tray menu.
            hud_visible: Mutex::new(true),
            settings: Mutex::new(settings),
            hud_menu_item: Mutex::new(None),
            ptt: PttState {
                is_recording: Mutex::new(false),
                session_id: Mutex::new(0),
                audio_buffer: Mutex::new(Vec::new()),
                samples_since_partial: Mutex::new(0),
                samples_since_waveform: Mutex::new(0),
            },
            config_dir,
        }

    }

    pub async fn save_settings(&self) -> anyhow::Result<()> {
        let settings = self.settings.lock().await;
        let json = serde_json::to_string_pretty(&*settings)?;
        let path = self.config_dir.join("settings.json");
        tokio::fs::write(path, json).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_settings_serialization() {
        let settings = VoxSettings {
            stt_model_dir: PathBuf::from("models/stt"),
            vad_model_path: PathBuf::from("models/vad.onnx"),
            vad_threshold: 0.6,
            ptt_noise_gate: 0.005,
            theme: "dark".into(),
        };
        
        let json = serde_json::to_string(&settings).unwrap();
        let decoded: VoxSettings = serde_json::from_str(&json).unwrap();
        
        assert_eq!(decoded.stt_model_dir, settings.stt_model_dir);
        assert_eq!(decoded.ptt_noise_gate, settings.ptt_noise_gate);
        assert_eq!(decoded.theme, settings.theme);
    }
}
