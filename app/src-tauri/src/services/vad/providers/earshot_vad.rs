use anyhow::Result;
use earshot::Detector;

use super::VadEngine;

/// Voice Activity Detection engine wrapping pure Rust Earshot neural model without ONNX dependency.
pub struct EarshotVadEngine {
    detector: Box<Detector>,
    threshold: f32,
}

impl EarshotVadEngine {
    /// Creates a heap-allocated Earshot neural detector with the given speech threshold.
    pub fn new(threshold: f32) -> Result<Self> {
        log::info!(
            "[VAD] Initializing Earshot VAD Engine (threshold={:.3}, pure-Rust, no ONNX)...",
            threshold
        );
        let detector = Detector::default_boxed();
        log::info!("[VAD] Earshot VAD Engine ready (~8 KiB heap, ~110 KiB binary footprint).");
        Ok(Self {
            detector,
            threshold,
        })
    }

    /// Hot-updates the speech detection probability threshold.
    pub fn update_threshold(&mut self, threshold: f32) {
        log::info!(
            "[VAD/Earshot] Threshold updated: {:.3} → {:.3}",
            self.threshold,
            threshold
        );
        self.threshold = threshold;
    }

    /// Resets detector internal states.
    pub fn flush(&mut self) {
        self.detector.reset();
    }
}

impl VadEngine for EarshotVadEngine {
    /// Evaluates voice activity for a 256-sample frame at 16kHz.
    fn predict(&mut self, chunk: &[f32]) -> bool {
        let score = if chunk.len() == 256 {
            let mut clamped_chunk = [0.0f32; 256];
            for (i, &val) in chunk.iter().enumerate() {
                clamped_chunk[i] = val.clamp(-1.0, 1.0);
            }
            self.detector.predict_f32(&clamped_chunk)
        } else {
            let mut clamped_chunk = [0.0f32; 256];
            let len = chunk.len().min(256);
            for i in 0..len {
                clamped_chunk[i] = chunk[i].clamp(-1.0, 1.0);
            }
            self.detector.predict_f32(&clamped_chunk)
        };

        let cal_threshold = (self.threshold + 0.15).min(0.99);
        score >= cal_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_earshot_vad_predict_and_threshold_update() {
        let mut engine = EarshotVadEngine::new(-1.0).unwrap();
        let frame = [0.0f32; 256];

        let active = engine.predict(&frame);
        assert!(active);

        engine.update_threshold(1.0);
        let inactive = engine.predict(&frame);
        assert!(!inactive);

        engine.flush();
        engine.update_threshold(0.7);
        assert!((engine.threshold - 0.7).abs() < 1e-5);
    }
}
