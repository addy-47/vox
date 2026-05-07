use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use anyhow::Result;

// ─── Audio Output Mode ────────────────────────────────────────────────────────

/// Controls acoustic echo mitigation strategy (Directive 1 from phase4 plan).
///
/// - `Speaker`: mic frames are dropped while playback is active to prevent
///   the TTS output from re-triggering VAD (feedback loop).
/// - `Headset`: mic stays fully active, enabling true barge-in. Speech start
///   triggers atomic cancellation of LLM + TTS + playback.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum AudioOutputMode {
    #[default]
    Speaker,
    Headset,
}

// ─── Settings ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VoxSettings {
    // ── Phase 0.2/0.3 (existing) ─────────────────────────────────────────────
    pub stt_model_dir:  PathBuf,
    pub vad_model_path: PathBuf,
    pub vad_threshold:  f32,
    pub ptt_noise_gate: f32,
    pub theme:          String,

    // ── Phase 4 additions ─────────────────────────────────────────────────────
    /// Path to the Gemma GGUF model file.
    pub llm_model_path: PathBuf,
    /// Directory containing chatterbox ONNX files and tokenizer.
    pub tts_model_dir:  PathBuf,
    /// Directory containing Hindi TTS assets (Piper).
    pub tts_hindi_model_dir: PathBuf,
    /// LLM context window size in tokens. Keep ≤2048 in Phase 4 (KV cache budget).
    pub llm_ctx_size:   u32,
    /// Number of CPU threads for LLM inference. 0 = auto (total_cores - 2).
    pub llm_threads:    u32,
    /// Acoustic echo mitigation mode.
    pub audio_output_mode: AudioOutputMode,
}

impl Default for VoxSettings {
    fn default() -> Self {
        // Auto-detect safe thread count: leave 2 cores for audio + VAD.
        let total_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let llm_threads = (total_cores.saturating_sub(2)).max(1) as u32;

        Self {
            stt_model_dir:    PathBuf::from("assets/qwen3-asr"),
            vad_model_path:   PathBuf::from("assets/ten_vad.onnx"),
            vad_threshold:    0.6,
            ptt_noise_gate:   0.005,
            theme:            "dark".into(),
            llm_model_path:   PathBuf::from("assets/gemma4/google_gemma-4-E2B-it-IQ2_M.gguf"),
            tts_model_dir:    PathBuf::from("assets/kokoro"),
            tts_hindi_model_dir: PathBuf::from("assets/piper_hi"),
            llm_ctx_size:     2048,
            llm_threads,
            audio_output_mode: AudioOutputMode::Speaker,
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
