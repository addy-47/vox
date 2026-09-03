use std::collections::VecDeque;

/// Bounded circular buffer for retaining pre-roll audio before speech onset.
#[derive(Debug)]
pub struct PreRollBuffer {
    buffer: VecDeque<f32>,
    max_capacity: usize,
}

impl PreRollBuffer {
    /// Constructs a pre-roll buffer with a fixed maximum sample capacity.
    pub fn new(max_capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_capacity),
            max_capacity,
        }
    }

    /// Appends new audio samples to the pre-roll buffer, popping oldest samples if capacity is exceeded.
    pub fn push(&mut self, chunk: &[f32]) {
        let chunk_len = chunk.len();
        if chunk_len >= self.max_capacity {
            self.buffer.clear();
            self.buffer
                .extend(chunk[chunk_len - self.max_capacity..].iter().copied());
            return;
        }
        let excess = (self.buffer.len() + chunk_len).saturating_sub(self.max_capacity);
        for _ in 0..excess {
            self.buffer.pop_front();
        }
        self.buffer.extend(chunk.iter().copied());
    }

    /// Clears all stored audio samples.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Copies stored samples into the target buffer in chronological order without linear shifting.
    pub fn copy_into(&self, target: &mut Vec<f32>) {
        let (front, back) = self.buffer.as_slices();
        target.extend_from_slice(front);
        target.extend_from_slice(back);
    }
}

/// Calculates Root Mean Square (RMS) energy of an audio sample slice.
pub fn calculate_rms(chunk: &[f32]) -> f32 {
    if chunk.is_empty() {
        return 0.0;
    }
    (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt()
}

/// Converts normalized f32 audio samples [-1.0, 1.0] to 16-bit PCM i16 samples.
pub fn f32_to_i16_pcm(chunk: &[f32], target: &mut Vec<i16>) {
    target.clear();
    target.reserve(chunk.len());
    for &sample in chunk {
        let clamped = sample.clamp(-1.0, 1.0);
        target.push((clamped * 32767.0) as i16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests PreRollBuffer retains chronological order and respects capacity.
    #[test]
    fn test_preroll_push_and_capacity() {
        let mut buf = PreRollBuffer::new(10);
        buf.push(&[1.0, 2.0, 3.0]);
        let mut out = Vec::new();
        buf.copy_into(&mut out);
        assert_eq!(out, vec![1.0, 2.0, 3.0]);

        buf.push(&[4.0, 5.0, 6.0, 7.0, 8.0]);
        buf.push(&[9.0, 10.0, 11.0]);
        let mut out2 = Vec::new();
        buf.copy_into(&mut out2);
        assert_eq!(out2.len(), 10);
        assert_eq!(out2[0], 2.0);
        assert_eq!(out2[9], 11.0);
    }

    /// Tests chunk larger than capacity truncates to last max_capacity samples.
    #[test]
    fn test_preroll_large_chunk_truncates() {
        let mut buf = PreRollBuffer::new(5);
        buf.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let mut out = Vec::new();
        buf.copy_into(&mut out);
        assert_eq!(out, vec![4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    /// Tests clear empties buffer and copy_into on empty yields empty.
    #[test]
    fn test_preroll_clear_and_empty_copy() {
        let mut buf = PreRollBuffer::new(4);
        buf.push(&[1.0, 2.0]);
        buf.clear();
        let mut out = Vec::new();
        buf.copy_into(&mut out);
        assert!(out.is_empty());
        buf.copy_into(&mut out);
        assert!(out.is_empty());
    }

    /// Tests calculate_rms returns 0 for empty, correct for constant and silence.
    #[test]
    fn test_calculate_rms_boundaries() {
        assert_eq!(calculate_rms(&[]), 0.0);
        assert_eq!(calculate_rms(&[0.0, 0.0, 0.0]), 0.0);
        let rms = calculate_rms(&[1.0, 1.0, 1.0]);
        assert!((rms - 1.0).abs() < 1e-5);
        let rms2 = calculate_rms(&[1.0, -1.0, 1.0, -1.0]);
        assert!((rms2 - 1.0).abs() < 1e-5);
    }

    /// Tests f32_to_i16_pcm clamping and conversion.
    #[test]
    fn test_f32_to_i16_pcm_clamping() {
        let mut out = Vec::new();
        f32_to_i16_pcm(&[1.0, -1.0, 0.0, 2.0, -2.0, 0.5], &mut out);
        assert_eq!(out[0], 32767);
        assert_eq!(out[1], -32767);
        assert_eq!(out[2], 0);
        assert_eq!(out[3], 32767);
        assert_eq!(out[4], -32767);
        assert_eq!(out[5], (0.5 * 32767.0) as i16);

        let mut out2 = vec![9, 9, 9];
        f32_to_i16_pcm(&[0.1, 0.2], &mut out2);
        assert_eq!(out2.len(), 2);
    }
}
