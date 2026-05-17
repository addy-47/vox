use anyhow::Result;
use earshot::Detector;
use crate::services::traits;

/// Earshot VAD Engine — pure Rust, no ONNX Runtime dependency.
///
/// The neural network weights (~75 KiB) are embedded in the binary.
/// Each `Detector` instance uses ~8 KiB of internal state on the heap.
///
/// Frame requirements (enforced by earshot):
///   - Exactly 256 samples per call to `predict_f32`
///   - 16 kHz sample rate
///   - Samples in [-1.0, 1.0]
///
/// This matches Vox's existing 256-sample chunk stride exactly.
pub struct EarshotVadEngine {
    /// Heap-allocated to avoid placing ~8 KiB on the VAD OS thread stack.
    detector: Box<Detector>,
    /// Voice score threshold. Values ≥ this are classified as speech.
    /// Earshot recommends 0.5 as a general-purpose default.
    /// Stored here so hot-updates are a free f32 write (no model reload).
    threshold: f32,
}

impl EarshotVadEngine {
    /// Create a new Earshot VAD engine.
    ///
    /// # Parameters
    /// - `threshold`: Voice probability threshold in [0.0, 1.0].
    ///   Earshot recommends `0.5`. Values above this are classified as speech.
    pub fn new(threshold: f32) -> Result<Self> {
        log::info!("[VAD] Initializing Earshot VAD Engine (threshold={:.3}, pure-Rust, no ONNX)...", threshold);
        // `default_boxed()` creates the Detector directly on the heap,
        // avoiding an 8 KiB stack allocation before the Box move.
        let detector = Detector::default_boxed();
        log::info!("[VAD] Earshot VAD Engine ready (~8 KiB heap, ~110 KiB binary footprint).");
        Ok(Self { detector, threshold })
    }

    /// Hot-update the voice threshold without restarting the engine.
    ///
    /// Unlike TenVAD, this is a free f32 write — no ONNX detector re-creation needed.
    pub fn update_threshold(&mut self, threshold: f32) {
        log::info!("[VAD/Earshot] Threshold updated: {:.3} → {:.3}", self.threshold, threshold);
        self.threshold = threshold;
    }

    /// Reset the detector's internal state.
    ///
    /// Must be called when:
    /// - The audio recording device changes.
    /// - Starting a new, unrelated audio sequence (e.g., after flush/end of utterance).
    ///
    /// This is the earshot equivalent of TenVAD's `flush()`.
    pub fn flush(&mut self) {
        self.detector.reset();
    }
}

impl traits::VadEngine for EarshotVadEngine {
    /// Predict voice activity for a single 256-sample frame at 16 kHz.
    ///
    /// # Panics
    /// earshot will panic in debug builds if `chunk.len() != 256`.
    /// In Vox's VAD actor this is always 256 by construction.
    fn predict(&mut self, chunk: &[f32]) -> bool {
        let score = self.detector.predict_f32(chunk);
        score >= self.threshold
    }
}
