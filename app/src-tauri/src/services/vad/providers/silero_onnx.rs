use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};

use super::VadEngine as VadEngineTrait;

/// Minimum internal Sherpa-ONNX speech onset window (1 native 512-sample Silero frame = 32ms).
/// Kept minimal so `SileroVadEngine` acts as a low-latency frame classifier without stacking
/// a second 120ms onset delay on top of `segmenter.rs`.
const SILERO_INTERNAL_MIN_SPEECH_S: f32 = 0.032;

/// Minimum internal Sherpa-ONNX silence hold window (2 native 512-sample Silero frames = 64ms).
/// Smooths 1-frame micro-dropouts inside Sherpa while delegating dynamic hangover (500–1100ms)
/// to the adaptive segmenter in `segmenter.rs`.
const SILERO_INTERNAL_MIN_SILENCE_S: f32 = 0.064;

/// Voice Activity Detection engine wrapping Silero VAD ONNX model via Sherpa-ONNX.
pub struct SileroVadEngine {
    detector: VoiceActivityDetector,
    model_path: PathBuf,
    threshold: f32,
    min_silence_duration: f32,
    min_speech_duration: f32,
    max_speech_duration: f32,
}

impl SileroVadEngine {
    /// Loads Silero VAD ONNX model and initializes the voice activity detector.
    pub fn new(
        model_path: &Path,
        threshold: f32,
        min_silence_duration: f32,
        min_speech_duration: f32,
        max_speech_duration: f32,
    ) -> Result<Self> {
        let model_path_buf = model_path.to_path_buf();
        let detector = Self::create_detector(
            &model_path_buf,
            threshold,
            min_silence_duration,
            min_speech_duration,
            max_speech_duration,
        )?;

        log::info!(
            "[VAD] Silero VAD Engine loaded successfully (threshold={}, configured_min_silence={}s, configured_min_speech={}s, max_speech={}s).",
            threshold,
            min_silence_duration,
            min_speech_duration,
            max_speech_duration
        );
        Ok(Self {
            detector,
            model_path: model_path_buf,
            threshold,
            min_silence_duration,
            min_speech_duration,
            max_speech_duration,
        })
    }

    /// Helper creating a Sherpa VoiceActivityDetector instance with specified model path and parameters.
    fn create_detector(
        model_path: &Path,
        threshold: f32,
        _min_silence_duration: f32,
        _min_speech_duration: f32,
        max_speech_duration: f32,
    ) -> Result<VoiceActivityDetector> {
        // Clamp Sherpa's internal hysteresis to single-window responsiveness so segmenter.rs
        // owns the unified adaptive onset and hangover state machine without double-buffering lag.
        let effective_min_speech = SILERO_INTERNAL_MIN_SPEECH_S;
        let effective_min_silence = SILERO_INTERNAL_MIN_SILENCE_S;

        log::info!(
            "[VAD] >>> Initializing Sherpa-ONNX Silero VAD Engine (threshold={}, internal_min_silence={}s, internal_min_speech={}s, max_speech={}s)...",
            threshold,
            effective_min_silence,
            effective_min_speech,
            max_speech_duration
        );

        let config = VadModelConfig {
            silero_vad: SileroVadModelConfig {
                model: Some(model_path.to_string_lossy().into()),
                threshold,
                min_silence_duration: effective_min_silence,
                min_speech_duration: effective_min_speech,
                window_size: 512,
                max_speech_duration,
            },
            ten_vad: Default::default(),
            sample_rate: 16000,
            num_threads: 1,
            debug: false,
            provider: Some("cpu".into()),
        };

        VoiceActivityDetector::create(&config, 60.0).ok_or_else(|| {
            anyhow!(
                "Failed to create Sherpa VoiceActivityDetector for Silero VAD. Check model path: {:?}",
                model_path
            )
        })
    }

    /// Hot-updates the detector instance with new speech parameters.
    pub fn update_params(
        &mut self,
        threshold: f32,
        min_silence_duration: f32,
        min_speech_duration: f32,
    ) -> Result<()> {
        let threshold_changed = (self.threshold - threshold).abs() >= 1e-4;
        self.min_silence_duration = min_silence_duration;
        self.min_speech_duration = min_speech_duration;

        if !threshold_changed {
            return Ok(());
        }

        self.detector = Self::create_detector(
            &self.model_path,
            threshold,
            self.min_silence_duration,
            self.min_speech_duration,
            self.max_speech_duration,
        )?;
        self.threshold = threshold;
        Ok(())
    }

    /// Hot-updates the detector instance with a new speech threshold if changed.
    pub fn update_threshold(&mut self, threshold: f32) -> Result<()> {
        self.update_params(
            threshold,
            self.min_silence_duration,
            self.min_speech_duration,
        )
    }

    /// Alias for update_threshold matching TenVAD interface.
    pub fn update_detector(&mut self, threshold: f32) -> Result<()> {
        self.update_threshold(threshold)
    }

    /// Hot-updates the detector instance with a new minimum silence duration.
    pub fn update_silence_duration(&mut self, min_silence_duration: f32) -> Result<()> {
        self.update_params(
            self.threshold,
            min_silence_duration,
            self.min_speech_duration,
        )
    }

    /// Hot-updates the detector instance with a new minimum speech onset duration.
    pub fn update_speech_onset(&mut self, min_speech_duration: f32) -> Result<()> {
        self.update_params(
            self.threshold,
            self.min_silence_duration,
            min_speech_duration,
        )
    }

    /// Resets internal detector state at utterance boundaries.
    pub fn flush(&mut self) {
        self.detector.flush();
    }
}

impl VadEngineTrait for SileroVadEngine {
    /// Evaluates if the current audio buffer chunk contains speech.
    fn predict(&mut self, chunk: &[f32]) -> bool {
        self.detector.accept_waveform(chunk);
        let detected = self.detector.detected();
        // Prevent internal Sherpa speech-segment queue growth across long sessions
        while !self.detector.is_empty() {
            self.detector.pop();
        }
        detected
    }
}