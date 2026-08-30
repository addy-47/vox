pub mod actor;
pub mod earshot_vad;
pub mod telemetry;
pub mod ten_onnx;
pub mod utils;

pub use actor::{
    spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles, VadValidationResult,
};
pub use utils::PreRollBuffer;

// ─── VAD Subsystem Constants ─────────────────────────────────────────────────
pub const MODEL_DIR_VAD: &str = "vad";
pub const MODEL_FILE_VAD: &str = "ten_vad.onnx";
pub const VAD_CHUNK_SIZE: usize = 256;
pub const VAD_PRE_ROLL_CAPACITY: usize = 8000;
pub const VAD_SPEECH_START_FRAMES: usize = 2;
pub const VAD_SPEECH_END_FRAMES: usize = 50;
pub const VAD_MIN_UTTERANCE_SAMPLES: usize = 4800;
pub const VAD_PARTIAL_INTERVAL_SAMPLES: usize = 12800;
pub const VAD_MAX_PARTIAL_WINDOW_SAMPLES: usize = 240000;
/// Earshot pure-Rust energy model noise gate calibration multiplier to compensate for dynamic range scale differences.
pub const EARSHOT_NOISE_GATE_MULTIPLIER: f32 = 1.5;

/// Generic operational modes supported by the decoupled VAD actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadOperationalMode {
    /// Autonomous neural speech onset/offset segmentation with utterance dispatch.
    ContinuousSegmentation,
    /// Caller-gated window evaluation (evaluates voice presence for caller-owned recording windows).
    WindowedValidation,
    /// Low-latency direct audio chunk forwarding to a configured realtime sender.
    StreamPassthrough,
}

/// Voice Activity Detection engine contract.
pub trait VadEngine {
    /// Evaluates if the current audio chunk contains active speech.
    fn predict(&mut self, chunk: &[f32]) -> bool;
}

/// Unified dispatch enum for supported Voice Activity Detection backends.
pub enum VadBackend {
    Ten(ten_onnx::VadEngine),
    Earshot(earshot_vad::EarshotVadEngine),
}

impl VadEngine for VadBackend {
    /// Dispatches chunk speech activity evaluation to the selected backend.
    fn predict(&mut self, chunk: &[f32]) -> bool {
        match self {
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
            VadBackend::Ten(_) => 1.0,
        }
    }

    /// Evaluates if raw energy satisfies the noise gate threshold for this backend.
    pub fn is_above_noise_gate(&self, raw_energy: f32, noise_gate: f32) -> bool {
        raw_energy >= (noise_gate * self.noise_gate_multiplier())
    }

    /// Hot-updates the voice detection activation threshold.
    pub fn update_threshold(&mut self, threshold: f32) -> anyhow::Result<()> {
        match self {
            VadBackend::Ten(e) => e.update_detector(threshold),
            VadBackend::Earshot(e) => {
                e.update_threshold(threshold);
                Ok(())
            }
        }
    }

    /// Flushes internal detector state across utterance boundaries.
    pub fn flush(&mut self) {
        match self {
            VadBackend::Ten(e) => e.flush(),
            VadBackend::Earshot(e) => e.flush(),
        }
    }
}
