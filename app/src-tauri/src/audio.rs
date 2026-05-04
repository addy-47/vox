use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::*;

pub struct AudioStream {
    _stream: cpal::Stream,
}

unsafe impl Send for AudioStream {}
unsafe impl Sync for AudioStream {}

impl AudioStream {
    pub fn new<P>(producer: P) -> Result<Self> 
    where 
        P: Producer<Item = f32> + Send + 'static 
    {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device found"))?;

        let config: cpal::StreamConfig = device.default_input_config()?.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels as usize;
        
        log::info!("[Audio] Using input device: {}", device.name()?);
        log::info!("[Audio] Config: {}Hz, {} channels", sample_rate, channels);
        
        let mut producer = producer;
        
        // Accurate resampling state
        let mut source_index: f32 = 0.0;
        let resample_ratio = sample_rate as f32 / 16000.0;
        
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // 1. Mono conversion (Channel Averaging)
                let mut mono_buffer = Vec::with_capacity(data.len() / channels);
                for chunk in data.chunks_exact(channels) {
                    let avg: f32 = chunk.iter().sum::<f32>() / channels as f32;
                    mono_buffer.push(avg);
                }
                
                let n_mono = mono_buffer.len();
                let mut resampled = Vec::with_capacity((n_mono as f32 / resample_ratio) as usize + 1);
                
                // 2. Linear Resampling
                while (source_index as usize) < n_mono {
                    let idx = source_index as usize;
                    let next_idx = (idx + 1).min(n_mono - 1);
                    let frac = source_index - idx as f32;
                    
                    let sample = (1.0 - frac) * mono_buffer[idx] + frac * mono_buffer[next_idx];
                    resampled.push(sample);
                    
                    source_index += resample_ratio;
                }
                
                // 3. Subtract processed from total source_index to keep it relative
                source_index -= n_mono as f32;
                
                // 4. Push to ring buffer
                if !resampled.is_empty() {
                    let pushed = producer.push_slice(&resampled);
                    if pushed < resampled.len() {
                        log::warn!("[Audio] Ring buffer overflow: {}/{} samples pushed", pushed, resampled.len());
                    }
                    
                }
            },
            move |err| {
                eprintln!("[Audio] Stream error: {}", err);
            },
            None,
        )?;

        Ok(Self { _stream: stream })
    }

    pub fn start(&self) -> Result<()> {
        log::info!("[Audio] Stream play() called successfully");
        self._stream.play()?;
        Ok(())
    }
}
