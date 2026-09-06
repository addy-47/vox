use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use sherpa_onnx::{TenVadModelConfig, VadModelConfig, VoiceActivityDetector};

use super::VadEngine as VadEngineTrait;

/// Voice Activity Detection engine wrapping TenVAD ONNX model via Sherpa-ONNX.
pub struct VadEngine {
    detector: VoiceActivityDetector,
    model_path: PathBuf,
    threshold: f32,
}

impl VadEngine {
    /// Loads TenVAD ONNX model and initializes the voice activity detector.
    pub fn new(model_path: &Path, threshold: f32) -> Result<Self> {
        let model_path_buf = model_path.to_path_buf();
        let detector = Self::create_detector(&model_path_buf, threshold)?;

        log::info!("[VAD] TenVAD Engine loaded successfully.");
        Ok(Self {
            detector,
            model_path: model_path_buf,
            threshold,
        })
    }

    /// Helper creating a Sherpa VoiceActivityDetector instance with specified model path and threshold.
    fn create_detector(model_path: &Path, threshold: f32) -> Result<VoiceActivityDetector> {
        log::info!(
            "[VAD] >>> Initializing Sherpa-ONNX TenVAD Engine (threshold={})...",
            threshold
        );

        let config = VadModelConfig {
            silero_vad: Default::default(),
            ten_vad: TenVadModelConfig {
                model: Some(model_path.to_string_lossy().into()),
                threshold,
                min_silence_duration: 0.5,
                min_speech_duration: 0.25,
                window_size: 256,
                max_speech_duration: 10.0,
            },
            sample_rate: 16000,
            num_threads: 1,
            debug: false,
            provider: Some("cpu".into()),
        };

        VoiceActivityDetector::create(&config, 60.0).ok_or_else(|| {
            anyhow!(
                "Failed to create Sherpa VoiceActivityDetector. Check model path: {:?}",
                model_path
            )
        })
    }

    /// Hot-updates the detector instance with a new speech threshold if changed.
    pub fn update_detector(&mut self, threshold: f32) -> Result<()> {
        if (self.threshold - threshold).abs() < 1e-4 {
            return Ok(());
        }
        self.detector = Self::create_detector(&self.model_path, threshold)?;
        self.threshold = threshold;
        Ok(())
    }

    /// Resets internal detector state at utterance boundaries.
    pub fn flush(&mut self) {
        self.detector.flush();
    }
}

impl VadEngineTrait for VadEngine {
    /// Evaluates if the current audio buffer chunk contains speech.
    fn predict(&mut self, chunk: &[f32]) -> bool {
        self.detector.accept_waveform(chunk);
        self.detector.detected()
    }
}
