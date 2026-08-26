/// A simple, fast 1st-order Infinite Impulse Response (IIR) Low-Pass Filter.
/// Formula: y[n] = y[n-1] + alpha * (x[n] - y[n-1])
pub struct LowPass {
    alpha: f32,
    prev_y: f32,
}

impl LowPass {
    pub fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        // Formulate low-pass filter coefficient alpha based on cutoff frequency
        let omega = 2.0 * std::f32::consts::PI * cutoff_hz / sample_rate;
        let alpha = omega / (1.0 + omega);
        Self { alpha, prev_y: 0.0 }
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.prev_y + self.alpha * (x - self.prev_y);
        self.prev_y = y;
        y
    }

    pub fn reset(&mut self) {
        self.prev_y = 0.0;
    }
}

/// A 3-band digital filter bank that splits a signal into:
/// - Lows (Bass, chest voice): < 250 Hz
/// - Mids (Vowels, vocal power): 250 Hz - 2000 Hz
/// - Highs (Treble, sibilance, consonants): > 2000 Hz
///
/// Uses a subtractive approach:
/// - Low = LowPass(250Hz)
/// - Mid = LowPass(2000Hz) - Low
/// - High = Input - LowPass(2000Hz)
///
/// This guarantees stable 1st-order operations with zero phase/group delay issues
/// and sums exactly back to the original input buffer.
pub struct FilterBank {
    lp_low: LowPass,
    lp_high: LowPass,
}

impl FilterBank {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            lp_low: LowPass::new(250.0, sample_rate),
            lp_high: LowPass::new(2000.0, sample_rate),
        }
    }

    /// Tick a single sample through the filter bank, returning the filtered (Low, Mid, High) samples.
    #[inline]
    pub fn tick(&mut self, x: f32) -> (f32, f32, f32) {
        let low = self.lp_low.tick(x);
        let lp_h = self.lp_high.tick(x);
        let mid = lp_h - low;
        let high = x - lp_h;
        (low, mid, high)
    }

    /// Processes a buffer of contiguous audio frames and returns the RMS values
    /// of the (Low, Mid, High) components respectively.
    pub fn process_chunk(&mut self, chunk: &[f32]) -> (f32, f32, f32) {
        if chunk.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let mut sum_low_sq = 0.0;
        let mut sum_mid_sq = 0.0;
        let mut sum_high_sq = 0.0;

        for &x in chunk {
            let low = self.lp_low.tick(x);
            let lp_h = self.lp_high.tick(x);
            let mid = lp_h - low;
            let high = x - lp_h;

            sum_low_sq += low * low;
            sum_mid_sq += mid * mid;
            sum_high_sq += high * high;
        }

        let len = chunk.len() as f32;
        let rms_low = (sum_low_sq / len).sqrt();
        let rms_mid = (sum_mid_sq / len).sqrt();
        let rms_high = (sum_high_sq / len).sqrt();

        (rms_low, rms_mid, rms_high)
    }

    pub fn reset(&mut self) {
        self.lp_low.reset();
        self.lp_high.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests LowPass filter DC steady-state unity gain and high-frequency attenuation.
    #[test]
    fn test_low_pass_dc_and_attenuation() {
        let mut lp = LowPass::new(250.0, 16000.0);

        let mut y = 0.0;
        for _ in 0..1000 {
            y = lp.tick(1.0);
        }
        assert!((y - 1.0).abs() < 1e-3);

        lp.reset();
        let mut nyquist_y = 0.0;
        for i in 0..200 {
            let x = if i % 2 == 0 { 1.0 } else { -1.0 };
            nyquist_y = lp.tick(x);
        }
        assert!(nyquist_y.abs() < 0.1);
    }

    /// Tests subtractive filter bank mathematical identity: low + mid + high == input.
    #[test]
    fn test_filter_bank_subtractive_identity() {
        let mut fb = FilterBank::new(16000.0);

        for i in 0..500 {
            let t = i as f32 / 16000.0;
            let x = (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                + 0.5 * (2.0 * std::f32::consts::PI * 3000.0 * t).sin();
            let (low, mid, high) = fb.tick(x);
            let sum = low + mid + high;
            assert!(
                (sum - x).abs() < 1e-5,
                "Subtractive filter bank identity failed: sum={}, x={}",
                sum,
                x
            );
        }
    }

    /// Tests chunk RMS calculations, empty slice safety, and silence handling.
    #[test]
    fn test_filter_bank_process_chunk_and_boundaries() {
        let mut fb = FilterBank::new(16000.0);

        assert_eq!(fb.process_chunk(&[]), (0.0, 0.0, 0.0));

        let silence = [0.0f32; 256];
        let (rms_l, rms_m, rms_h) = fb.process_chunk(&silence);
        assert!(rms_l.abs() < 1e-5);
        assert!(rms_m.abs() < 1e-5);
        assert!(rms_h.abs() < 1e-5);

        let mut signal = [0.0f32; 256];
        for (i, sample) in signal.iter_mut().enumerate() {
            let t = i as f32 / 16000.0;
            *sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        }
        let (r_l, r_m, r_h) = fb.process_chunk(&signal);
        assert!(r_l >= 0.0 && r_m >= 0.0 && r_h >= 0.0);
        assert!(r_m > 0.05);
    }
}
