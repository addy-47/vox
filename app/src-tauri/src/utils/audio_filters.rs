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
