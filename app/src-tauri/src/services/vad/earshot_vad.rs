use super::VadEngine;
use anyhow::Result;
use earshot::Detector;

/// Voice Activity Detection engine wrapping pure Rust Earshot neural model without ONNX dependency.
pub struct EarshotVadEngine {
    detector: Box<Detector>,
    threshold: f32,
    is_speech: bool,
    active_frames: usize,
    inactive_frames: usize,
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
            is_speech: false,
            active_frames: 0,
            inactive_frames: 0,
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

    /// Resets detector internal states and debouncing frame counters.
    pub fn flush(&mut self) {
        self.detector.reset();
        self.is_speech = false;
        self.active_frames = 0;
        self.inactive_frames = 0;
    }
}

impl VadEngine for EarshotVadEngine {
    /// Evaluates voice activity for a 256-sample frame at 16kHz and updates debouncer state.
    fn predict(&mut self, chunk: &[f32]) -> bool {
        let score = if chunk.len() == 256 {
            let mut clamped_chunk = [0.0f32; 256];
            for (i, &val) in chunk.iter().enumerate() {
                clamped_chunk[i] = val.clamp(-1.0, 1.0);
            }
            self.detector.predict_f32(&clamped_chunk)
        } else {
            let clamped: Vec<f32> = chunk.iter().map(|&val| val.clamp(-1.0, 1.0)).collect();
            self.detector.predict_f32(&clamped)
        };

        let cal_threshold = (self.threshold + 0.15).min(0.99);
        let is_active = score >= cal_threshold;

        log::trace!(
            "[VAD/Earshot] score: {:.4}, cal_threshold: {:.4}, is_active: {}, current_speech_state: {}", 
            score, cal_threshold, is_active, self.is_speech
        );

        if is_active {
            self.active_frames += 1;
            self.inactive_frames = 0;
            if !self.is_speech && self.active_frames >= 15 {
                self.is_speech = true;
                log::debug!(
                    "[VAD/Earshot] SPEECH START CONFIRMED (raw_score={:.4}, frames={})",
                    score,
                    self.active_frames
                );
            }
        } else {
            self.inactive_frames += 1;
            self.active_frames = 0;
            if self.is_speech && self.inactive_frames >= 40 {
                self.is_speech = false;
                log::debug!(
                    "[VAD/Earshot] SILENCE CONFIRMED (raw_score={:.4}, frames={})",
                    score,
                    self.inactive_frames
                );
            }
        }

        is_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that speech start is only confirmed after 15 consecutive active frames.
    #[test]
    fn test_earshot_vad_speech_start_debouncing() {
        let mut engine = EarshotVadEngine::new(-1.0).unwrap();
        let frame = [0.0f32; 256];

        for _ in 0..14 {
            let active = engine.predict(&frame);
            assert!(active);
            assert!(!engine.is_speech);
        }
        assert_eq!(engine.active_frames, 14);

        let active_15 = engine.predict(&frame);
        assert!(active_15);
        assert!(engine.is_speech);
        assert_eq!(engine.active_frames, 15);
    }

    /// Tests that silence hangover preserves speech state until 40 consecutive inactive frames pass.
    #[test]
    fn test_earshot_vad_silence_hangover_debouncing() {
        let mut engine = EarshotVadEngine::new(-1.0).unwrap();
        let frame = [0.0f32; 256];

        for _ in 0..15 {
            engine.predict(&frame);
        }
        assert!(engine.is_speech);

        engine.update_threshold(1.0);

        for _ in 0..39 {
            let active = engine.predict(&frame);
            assert!(!active);
            assert!(engine.is_speech);
        }
        assert_eq!(engine.inactive_frames, 39);

        let active_40 = engine.predict(&frame);
        assert!(!active_40);
        assert!(!engine.is_speech);
        assert_eq!(engine.inactive_frames, 40);
    }

    /// Tests that short transient noise bursts (<15 frames) are discarded without triggering speech state.
    #[test]
    fn test_earshot_vad_noise_burst_rejection() {
        let mut engine = EarshotVadEngine::new(-1.0).unwrap();
        let frame = [0.0f32; 256];

        for _ in 0..3 {
            engine.predict(&frame);
            assert!(!engine.is_speech);
        }
        assert_eq!(engine.active_frames, 3);

        engine.update_threshold(1.0);
        let active = engine.predict(&frame);
        assert!(!active);
        assert_eq!(engine.active_frames, 0);
        assert_eq!(engine.inactive_frames, 1);
        assert!(!engine.is_speech);
    }

    /// Tests that flushing the engine resets all debouncer state and frame counters.
    #[test]
    fn test_earshot_vad_flush_and_threshold_update() {
        let mut engine = EarshotVadEngine::new(-1.0).unwrap();
        let frame = [0.0f32; 256];

        for _ in 0..15 {
            engine.predict(&frame);
        }
        assert!(engine.is_speech);

        engine.flush();
        assert!(!engine.is_speech);
        assert_eq!(engine.active_frames, 0);
        assert_eq!(engine.inactive_frames, 0);

        engine.update_threshold(0.7);
        assert!((engine.threshold - 0.7).abs() < 1e-5);
    }
}
