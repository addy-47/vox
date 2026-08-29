use std::collections::VecDeque;

use super::EARSHOT_NOISE_GATE_MULTIPLIER;

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
            self.buffer.extend(chunk[chunk_len - self.max_capacity..].iter().copied());
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

/// Evaluates if raw energy satisfies the noise gate threshold.
pub fn is_above_noise_gate(raw_energy: f32, noise_gate: f32, is_earshot: bool) -> bool {
    let effective_noise_gate = if is_earshot {
        noise_gate * EARSHOT_NOISE_GATE_MULTIPLIER
    } else {
        noise_gate
    };
    raw_energy >= effective_noise_gate
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
