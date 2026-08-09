use anyhow::{anyhow, Result};
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

pub struct AudioResampler {
    inner: Async<f32>,
    input_buf: Vec<f32>,
    resampler_in_buf: Vec<Vec<f32>>,
    resampler_out_buf: Vec<Vec<f32>>,
    nbr_frames_needed: usize,
}

impl AudioResampler {
    pub fn new(from_hz: u32, to_hz: u32, chunk_size: usize) -> Result<Self> {
        let ratio = to_hz as f64 / from_hz as f64;
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };

        // Create resampler: ratio, max ratio 2.0, params, chunk size, 1 channel, fixed input
        let inner = Async::<f32>::new_sinc(
            ratio,
            2.0,
            &params,
            chunk_size,
            1, // mono channel
            FixedAsync::Input,
        )
        .map_err(|e| anyhow!("Failed to create rubato resampler: {:?}", e))?;

        let nbr_frames_needed = inner.input_frames_next();
        let resampler_in_buf = vec![vec![0.0f32; inner.input_frames_max()]; 1];
        let resampler_out_buf = vec![vec![0.0f32; inner.output_frames_max()]; 1];

        Ok(Self {
            inner,
            input_buf: Vec::new(),
            resampler_in_buf,
            resampler_out_buf,
            nbr_frames_needed,
        })
    }

    pub fn process_i16(&mut self, input: &[i16]) -> Result<Vec<i16>> {
        // 1. Convert input i16 to f32 and append to input_buf
        let f32_samples = input.iter().map(|&s| s as f32 / 32768.0);
        self.input_buf.extend(f32_samples);

        let mut output_samples = Vec::new();

        // 2. Process in blocks of nbr_frames_needed
        while self.input_buf.len() >= self.nbr_frames_needed {
            self.resampler_in_buf[0].clear();
            self.resampler_in_buf[0].extend_from_slice(&self.input_buf[..self.nbr_frames_needed]);
            self.input_buf.drain(..self.nbr_frames_needed);

            // Wrap in SequentialSliceOfVecs adapters (1 channel)
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

            // Append results
            output_samples.extend_from_slice(&self.resampler_out_buf[0][..out_len]);

            // Retrieve frames needed for next chunk
            self.nbr_frames_needed = self.inner.input_frames_next();
        }

        // 3. Convert resampled f32 output back to i16
        let output_i16 = output_samples
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * 32767.0) as i16
            })
            .collect();

        Ok(output_i16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_process_exact_sample_count() {
        let mut resampler = AudioResampler::new(16000, 24000, 256).unwrap();
        let input = vec![0i16; 512];
        let output = resampler.process_i16(&input).unwrap();
        // 512 input samples at 16kHz resampled to 24kHz (1.5x ratio)
        // Rubato sinc filter: Chunk 1 produces 381 frames (initial transient delay),
        // Chunk 2 produces 384 frames (exact 256 * 1.5 ratio). Total = 765 frames.
        assert_eq!(output.len(), 765);
        assert!(output.iter().all(|&s| s == 0));
    }

    #[test]
    fn test_resampler_empty_input() {
        let mut resampler = AudioResampler::new(16000, 24000, 256).unwrap();
        let output = resampler.process_i16(&[]).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_resampler_boundary_and_chunk_buffering() {
        let mut resampler = AudioResampler::new(16000, 24000, 256).unwrap();

        // 1. Pass input smaller than chunk_size (100 < 256) -> output should be empty (buffered)
        let output_1 = resampler.process_i16(&[0i16; 100]).unwrap();
        assert_eq!(output_1.len(), 0);

        // 2. Pass 156 samples to complete first chunk of 256 -> outputs 381 samples (initial transient)
        let output_2 = resampler.process_i16(&vec![0i16; 156]).unwrap();
        assert_eq!(output_2.len(), 381);

        // 3. Pass non-multiple chunk input (600 samples = 2 * 256 + 88 remainder)
        // Should process 2 full chunks (384 + 384 = 768 samples) and buffer 88 samples
        let output_3 = resampler.process_i16(&vec![0i16; 600]).unwrap();
        assert_eq!(output_3.len(), 768);

        // 4. Pass 168 samples to complete buffered chunk (88 + 168 = 256) -> outputs 384 samples
        let output_4 = resampler.process_i16(&vec![0i16; 168]).unwrap();
        assert_eq!(output_4.len(), 384);
    }

    #[test]
    fn test_resampler_downsampling() {
        let mut resampler = AudioResampler::new(48000, 16000, 384).unwrap();
        let input = vec![0i16; 768]; // 2 chunks of 384 samples
        let output = resampler.process_i16(&input).unwrap();
        // 768 samples at 48kHz resampled to 16kHz (1/3 ratio)
        // Chunk 1 = 127 frames, Chunk 2 = 128 frames. Total = 255 frames.
        assert_eq!(output.len(), 255);
    }
}
