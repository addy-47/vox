use super::{
    PCM_INT16_DIVISOR_FLOAT, PCM_INT16_MAX_FLOAT, SINC_CUTOFF_FREQUENCY, SINC_OVERSAMPLING_FACTOR,
    SINC_WINDOW_LEN,
};
use anyhow::{anyhow, Result};
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

/// High-quality sinc interpolation audio resampler based on Rubato.
pub struct AudioResampler {
    inner: Async<f32>,
    input_buf: Vec<f32>,
    resampler_in_buf: Vec<Vec<f32>>,
    resampler_out_buf: Vec<Vec<f32>>,
    nbr_frames_needed: usize,
}

impl AudioResampler {
    /// Creates a new AudioResampler configured for the specified input/output sampling rate ratio.
    pub fn new(from_hz: u32, to_hz: u32, chunk_size: usize) -> Result<Self> {
        let ratio = to_hz as f64 / from_hz as f64;
        let params = SincInterpolationParameters {
            sinc_len: SINC_WINDOW_LEN,
            f_cutoff: SINC_CUTOFF_FREQUENCY,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: SINC_OVERSAMPLING_FACTOR,
            window: WindowFunction::BlackmanHarris2,
        };

        let inner = Async::<f32>::new_sinc(ratio, 2.0, &params, chunk_size, 1, FixedAsync::Input)
            .map_err(|e| anyhow!("Failed to create rubato resampler: {:?}", e))?;

        let nbr_frames_needed = inner.input_frames_next();
        let resampler_in_buf = vec![vec![0.0f32; inner.input_frames_max()]; 1];
        let resampler_out_buf = vec![vec![0.0f32; inner.output_frames_max()]; 1];

        let input_buf = Vec::with_capacity(nbr_frames_needed * 2);

        Ok(Self {
            inner,
            input_buf,
            resampler_in_buf,
            resampler_out_buf,
            nbr_frames_needed,
        })
    }

    /// Resamples an input slice of 16-bit PCM integer samples and returns the converted buffer.
    pub fn process_i16(&mut self, input: &[i16]) -> Result<Vec<i16>> {
        let f32_samples = input.iter().map(|&s| s as f32 / PCM_INT16_DIVISOR_FLOAT);
        self.input_buf.extend(f32_samples);

        let mut output_samples = Vec::new();

        while self.input_buf.len() >= self.nbr_frames_needed {
            self.resampler_in_buf[0].clear();
            self.resampler_in_buf[0].extend_from_slice(&self.input_buf[..self.nbr_frames_needed]);
            self.input_buf.drain(..self.nbr_frames_needed);

            let input_adapter =
                SequentialSliceOfVecs::new(&self.resampler_in_buf, 1, self.nbr_frames_needed)
                    .map_err(|e| anyhow!("Failed to create input adapter: {:?}", e))?;
            let mut output_adapter = SequentialSliceOfVecs::new_mut(
                &mut self.resampler_out_buf,
                1,
                self.inner.output_frames_max(),
            )
            .map_err(|e| anyhow!("Failed to create output adapter: {:?}", e))?;

            let (_in_len, out_len) = self
                .inner
                .process_into_buffer(&input_adapter, &mut output_adapter, None)
                .map_err(|e| anyhow!("Resampling execution error: {:?}", e))?;

            output_samples.extend_from_slice(&self.resampler_out_buf[0][..out_len]);
            self.nbr_frames_needed = self.inner.input_frames_next();
        }

        let output_i16 = output_samples
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * PCM_INT16_MAX_FLOAT) as i16
            })
            .collect();

        Ok(output_i16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_resampler_16k_to_24k() {
        let mut resampler = AudioResampler::new(16000, 24000, 320).expect("Resampler init");
        let input: Vec<i16> = (0..640)
            .map(|i| ((i as f32 * 0.1).sin() * 10000.0) as i16)
            .collect();
        let output = resampler.process_i16(&input).expect("Process i16");
        assert!(!output.is_empty(), "Output should contain resampled frames");
    }

    #[test]
    fn test_audio_resampler_24k_to_16k() {
        let mut resampler = AudioResampler::new(24000, 16000, 512).expect("Resampler init");
        let input: Vec<i16> = (0..960)
            .map(|i| ((i as f32 * 0.1).sin() * 10000.0) as i16)
            .collect();
        let output = resampler.process_i16(&input).expect("Process i16");
        assert!(!output.is_empty(), "Output should contain resampled frames");
    }

    #[test]
    fn test_audio_resampler_44k_to_16k() {
        let mut resampler = AudioResampler::new(44100, 16000, 882).expect("Resampler init");
        let input: Vec<i16> = (0..1764)
            .map(|i| ((i as f32 * 0.1).sin() * 10000.0) as i16)
            .collect();
        let output = resampler.process_i16(&input).expect("Process i16");
        assert!(!output.is_empty(), "Output should contain resampled frames");
    }

    #[test]
    fn test_audio_resampler_8k_to_16k() {
        let mut resampler = AudioResampler::new(8000, 16000, 160).expect("Resampler init");
        let input: Vec<i16> = (0..320)
            .map(|i| ((i as f32 * 0.1).sin() * 10000.0) as i16)
            .collect();
        let output = resampler.process_i16(&input).expect("Process i16");
        assert!(!output.is_empty(), "Output should contain resampled frames");
    }
}
