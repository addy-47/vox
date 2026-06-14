pub mod actor;
pub mod earshot_vad;
pub mod ten_onnx;

pub use actor::spawn_vad_actor;

use crate::services::traits::VadEngine as VadEngineTrait;

/// Unified dispatch enum for all supported VAD backends.
///
/// Uses enum dispatch (zero-cost, no vtable) instead of `Box<dyn VadEngine>`
/// to avoid dynamic dispatch overhead in the audio hot path.
pub enum VadBackend {
    Ten(ten_onnx::VadEngine),
    Earshot(earshot_vad::EarshotVadEngine),
}

impl VadEngineTrait for VadBackend {
    fn predict(&mut self, chunk: &[f32]) -> bool {
        match self {
            VadBackend::Ten(e) => e.predict(chunk),
            VadBackend::Earshot(e) => e.predict(chunk),
        }
    }
}

impl VadBackend {
    /// Hot-update the detection threshold.
    ///
    /// For TenVAD: recreates the ONNX detector with the new threshold.
    /// For Earshot: free f32 write, no reinitialization.
    pub fn update_threshold(&mut self, threshold: f32) -> anyhow::Result<()> {
        match self {
            VadBackend::Ten(e) => e.update_detector(threshold),
            VadBackend::Earshot(e) => {
                e.update_threshold(threshold);
                Ok(())
            }
        }
    }

    /// Flush/reset the detector state at utterance boundaries.
    pub fn flush(&mut self) {
        match self {
            VadBackend::Ten(e) => e.flush(),
            VadBackend::Earshot(e) => e.flush(),
        }
    }
}
