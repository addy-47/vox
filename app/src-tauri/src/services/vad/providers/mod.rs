pub mod earshot_vad;
pub mod silero_onnx;
pub mod ten_onnx;

pub use earshot_vad::EarshotVadEngine;
pub use silero_onnx::SileroVadEngine;
pub use ten_onnx::VadEngine as TenVadEngine;

use crate::services::vad::EARSHOT_NOISE_GATE_MULTIPLIER;

/// Voice Activity Detection engine contract.
pub trait VadEngine {
    /// Evaluates if the current audio chunk contains active speech.
    fn predict(&mut self, chunk: &[f32]) -> bool;
}

/// Unified dispatch enum for supported Voice Activity Detection backends.
pub enum VadBackend {
    Silero(SileroVadEngine),
    Ten(TenVadEngine),
    Earshot(EarshotVadEngine),
}

impl VadEngine for VadBackend {
    /// Dispatches chunk speech activity evaluation to the selected backend.
    fn predict(&mut self, chunk: &[f32]) -> bool {
        match self {
            VadBackend::Silero(e) => e.predict(chunk),
            VadBackend::Ten(e) => e.predict(chunk),
            VadBackend::Earshot(e) => e.predict(chunk),
        }
    }
}

impl VadBackend {
    /// Returns the noise gate multiplier required by this backend for calibrated energy filtering.
    pub fn noise_gate_multiplier(&self) -> f32 {
        match self {
            VadBackend::Earshot(_) => EARSHOT_NOISE_GATE_MULTIPLIER,
            VadBackend::Silero(_) | VadBackend::Ten(_) => 1.0,
        }
    }

    /// Evaluates if raw energy satisfies the noise gate threshold for this backend.
    pub fn is_above_noise_gate(&self, raw_energy: f32, noise_gate: f32) -> bool {
        raw_energy >= (noise_gate * self.noise_gate_multiplier())
    }

    /// Hot-updates the voice detection activation threshold.
    pub fn update_threshold(&mut self, threshold: f32) -> anyhow::Result<()> {
        match self {
            VadBackend::Silero(e) => e.update_detector(threshold),
            VadBackend::Ten(e) => e.update_detector(threshold),
            VadBackend::Earshot(e) => {
                e.update_threshold(threshold);
                Ok(())
            }
        }
    }

    /// Hot-updates minimum silence cutoff duration in milliseconds.
    pub fn update_silence_duration(&mut self, ms: u32) -> anyhow::Result<()> {
        let dur_s = ms as f32 / 1000.0;
        match self {
            VadBackend::Silero(e) => e.update_silence_duration(dur_s),
            VadBackend::Ten(e) => e.update_silence_duration(dur_s),
            VadBackend::Earshot(_) => Ok(()),
        }
    }

    /// Hot-updates minimum speech onset duration in milliseconds.
    pub fn update_speech_onset(&mut self, ms: u32) -> anyhow::Result<()> {
        let dur_s = ms as f32 / 1000.0;
        match self {
            VadBackend::Silero(e) => e.update_speech_onset(dur_s),
            VadBackend::Ten(e) => e.update_speech_onset(dur_s),
            VadBackend::Earshot(_) => Ok(()),
        }
    }

    /// Returns true if this backend is an ONNX neural detector with internal temporal hysteresis.
    pub fn is_onnx(&self) -> bool {
        matches!(self, VadBackend::Silero(_) | VadBackend::Ten(_))
    }

    /// Flushes internal detector state across utterance boundaries.
    pub fn flush(&mut self) {
        match self {
            VadBackend::Silero(e) => e.flush(),
            VadBackend::Ten(e) => e.flush(),
            VadBackend::Earshot(e) => e.flush(),
        }
    }
}
