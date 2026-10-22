use std::f32::consts::PI;

pub struct LowPass {
    alpha: f32,
    prev_y: f32,
}

impl LowPass {
    pub fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let omega = 2.0 * PI * cutoff_hz / sample_rate;
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

/// 2nd-order IIR Biquad Low-Pass Filter (Butterworth, Direct Form II Transposed).
pub struct BiquadLowPass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    s1: f32,
    s2: f32,
}

impl BiquadLowPass {
    /// Creates a 2nd-order Butterworth low-pass filter with cutoff `cutoff_hz` at `sample_rate`.
    pub fn new_butterworth(cutoff_hz: f32, sample_rate: f32) -> Self {
        let q = std::f32::consts::FRAC_1_SQRT_2;
        let omega = 2.0 * PI * cutoff_hz / sample_rate;
        let cos_w = omega.cos();
        let sin_w = omega.sin();
        let alpha = sin_w / (2.0 * q);

        let b0 = (1.0 - cos_w) / 2.0;
        let b1 = 1.0 - cos_w;
        let b2 = (1.0 - cos_w) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Ticks a single sample through Direct Form II Transposed structure.
    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }

    /// Resets filter history/delay memories.
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_pass_passes_dc_and_attenuates_the_nyquist() {
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

    #[test]
    fn filter_bank_bands_sum_back_to_the_input() {
        let mut fb = FilterBank::new(16000.0);

        for i in 0..500 {
            let t = i as f32 / 16000.0;
            let x = (2.0 * PI * 440.0 * t).sin() + 0.5 * (2.0 * PI * 3000.0 * t).sin();
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

    #[test]
    fn filter_bank_chunk_reports_rms_per_band() {
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
            *sample = (2.0 * PI * 1000.0 * t).sin();
        }
        let (r_l, r_m, r_h) = fb.process_chunk(&signal);
        assert!(r_l >= 0.0 && r_m >= 0.0 && r_h >= 0.0);
        assert!(r_m > 0.05);
    }

    #[test]
    fn biquad_low_pass_attenuates_the_stopband() {
        let mut lp = BiquadLowPass::new_butterworth(7500.0, 48000.0);

        // 1. DC unity gain
        let mut y = 0.0;
        for _ in 0..1000 {
            y = lp.tick(1.0);
        }
        assert!(
            (y - 1.0).abs() < 1e-3,
            "DC response must be unity gain (got {})",
            y
        );

        // 2. High-frequency stopband attenuation (> 12 kHz, e.g. 16 kHz)
        lp.reset();
        let mut max_amp = 0.0f32;
        for i in 0..1000 {
            let t = i as f32 / 48000.0;
            let x = (2.0 * PI * 16000.0 * t).sin();
            let out = lp.tick(x);
            if i > 500 {
                max_amp = max_amp.max(out.abs());
            }
        }
        // At 16 kHz, attenuation should be ~ -20 dB (amplitude <= 0.12)
        assert!(
            max_amp < 0.15,
            "16 kHz signal must be attenuated below 0.15 amplitude (got {})",
            max_amp
        );
    }
}
