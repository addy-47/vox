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
            // Drain nbr_frames_needed from input_buf
            let block: Vec<f32> = self.input_buf.drain(..self.nbr_frames_needed).collect();
            self.resampler_in_buf[0] = block;

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

            self.inner
                .process_into_buffer(&input_adapter, &mut output_adapter, None)
                .map_err(|e| anyhow!("Resampling execution error: {:?}", e))?;

            // Append results
            output_samples.extend_from_slice(&self.resampler_out_buf[0]);

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
