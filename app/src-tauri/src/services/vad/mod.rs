pub mod actor;
pub mod earshot_vad;
pub mod ten_onnx;

pub use actor::spawn_vad_actor;

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
