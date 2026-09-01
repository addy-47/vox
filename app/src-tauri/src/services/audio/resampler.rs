use super::{
    PCM_I16_SCALE, PCM_S16_SCALE, SINC_CUTOFF_FREQUENCY, SINC_OVERSAMPLING_FACTOR, SINC_WINDOW_LEN,
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
        let f32_samples = input.iter().map(|&s| s as f32 / PCM_S16_SCALE);
        self.input_buf.extend(f32_samples);

        let mut output_samples = Vec::new();

        while self.input_buf.len() >= self.nbr_frames_needed {
            self.resampler_in_buf[0].clear();
            self.resampler_in_buf[0].extend_from_slice(&self.input_buf[..self.nbr_frames_needed]);

            let input_adapter =
                SequentialSliceOfVecs::new(&self.resampler_in_buf, 1, self.nbr_frames_needed)
                    .map_err(|e| anyhow!("Failed to create input adapter: {:?}", e))?;
            let mut output_adapter = SequentialSliceOfVecs::new_mut(
                &mut self.resampler_out_buf,
                1,
                self.inner.output_frames_max(),
            )
            .map_err(|e| anyhow!("Failed to create output adapter: {:?}", e))?;

            let (_in_frames, out_frames) = self
                .inner
                .process_into_buffer(&input_adapter, &mut output_adapter, None)
                .map_err(|e| anyhow!("Resampling processing failed: {:?}", e))?;

            let converted = self.resampler_out_buf[0][..out_frames]
                .iter()
                .map(|&s| (s.clamp(-1.0, 1.0) * PCM_I16_SCALE) as i16);
            output_samples.extend(converted);

            self.input_buf.drain(..self.nbr_frames_needed);
            self.nbr_frames_needed = self.inner.input_frames_next();
        }

        Ok(output_samples)
    }

    /// Resamples an input slice of 32-bit floating point audio samples to target sampling rate.
    pub fn process_f32(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        self.input_buf.extend_from_slice(input);

        let mut output_samples = Vec::new();

        while self.input_buf.len() >= self.nbr_frames_needed {
            self.resampler_in_buf[0].clear();
            self.resampler_in_buf[0].extend_from_slice(&self.input_buf[..self.nbr_frames_needed]);

            let input_adapter =
                SequentialSliceOfVecs::new(&self.resampler_in_buf, 1, self.nbr_frames_needed)
                    .map_err(|e| anyhow!("Failed to create input adapter: {:?}", e))?;
            let mut output_adapter = SequentialSliceOfVecs::new_mut(
                &mut self.resampler_out_buf,
                1,
                self.inner.output_frames_max(),
            )
            .map_err(|e| anyhow!("Failed to create output adapter: {:?}", e))?;

            let (_in_frames, out_frames) = self
                .inner
                .process_into_buffer(&input_adapter, &mut output_adapter, None)
                .map_err(|e| anyhow!("Resampling processing failed: {:?}", e))?;

            output_samples.extend_from_slice(&self.resampler_out_buf[0][..out_frames]);

            self.input_buf.drain(..self.nbr_frames_needed);
            self.nbr_frames_needed = self.inner.input_frames_next();
        }

        Ok(output_samples)
    }
}

/// Upsample 24kHz mono PCM to 48kHz via cubic Hermite interpolation into a reusable buffer.
#[inline]
pub fn upsample_2x_into(input: &[f32], out: &mut Vec<f32>) {
    out.clear();
    if input.is_empty() {
        return;
    }
    let len = input.len();
    out.reserve(len * 2);
    for i in 0..len {
        let p1 = input[i];
        out.push(p1);

        let p0 = if i > 0 { input[i - 1] } else { p1 };
        let p2 = if i + 1 < len { input[i + 1] } else { p1 };
        let p3 = if i + 2 < len { input[i + 2] } else { p2 };

        let v = 0.5 * (p2 - p0);
        let v_next = if i + 2 < len {
            let p4 = if i + 3 < len { input[i + 3] } else { p3 };
            0.5 * (p4 - p1)
        } else {
            0.0
        };

        let interp = 0.5 * (p1 + p2) + 0.125 * (v - v_next);
        out.push(interp);
    }
}

/// Upsample 24kHz mono PCM to 48kHz via cubic Hermite interpolation.
#[inline]
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(input.len() * 2);
    upsample_2x_into(input, &mut out);
    out
}
