use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::*;

pub struct AudioStream {
    _stream: cpal::Stream,
}

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
        let step = (sample_rate as f32 / 16000.0).round() as usize;
        
        let mut producer = producer;
        
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // 1. Mono conversion & Resampling
                // Naive downsampling: take every step-th block
                let mut processed = Vec::with_capacity(data.len() / (channels * step));
                
                for chunk in data.chunks_exact(channels * step) {
                    // Average channels of the first sample in the step
                    let mut mono = 0.0;
                    for i in 0..channels {
                        mono += chunk[i];
                    }
                    processed.push(mono / channels as f32);
                }

                let _ = producer.push_slice(&processed);
            },
            move |err| {
                eprintln!("[Audio] Stream error: {}", err);
            },
            None,
        )?;

        Ok(Self { _stream: stream })
    }

    pub fn start(&self) -> Result<()> {
        self._stream.play()?;
        Ok(())
    }
}
