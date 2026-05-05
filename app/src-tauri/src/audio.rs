use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::*;

/// Manages the low-level CPAL hardware audio stream.
/// 
/// This struct is responsible for selecting the default input device, 
/// configuring the stream, and performing real-time mono-conversion 
/// and resampling in a performance-critical callback.
pub struct AudioStream {
    _stream: cpal::Stream,
}

// CPAL Stream is not Send/Sync by default, but we wrap it in a way that is safe 
// for our engine's lifecycle.
unsafe impl Send for AudioStream {}
unsafe impl Sync for AudioStream {}

impl AudioStream {
    /// Creates and configures a new hardware audio ingestion stream.
    /// 
    /// # Architecture Details:
    /// - **Zero-Allocation**: The internal CPAL callback reuses pre-allocated 
    ///   buffers (`mono_buffer`, `resampled_buffer`) to avoid heap allocations 
    ///   during the hardware interrupt.
    /// - **Linear Resampling**: Dynamically converts any hardware sample rate 
    ///   (e.g., 44.1kHz or 48kHz) to the 16kHz required by Sherpa-ONNX.
    /// - **Lock-Free Communication**: Uses an SPSC (Single-Producer Single-Consumer) 
    ///   ring buffer to pass audio data to the VAD thread without blocking the 
    ///   audio callback.
    /// 
    /// # Arguments
    /// * `producer` - The producer side of a lock-free ringbuf.
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
        
        log::info!("[AUDIO] Using input device: {}", device.name().unwrap_or_else(|_| "Unknown".into()));
        log::info!("[AUDIO] Config: {}Hz, {} channels", sample_rate, channels);
        
        let mut producer = producer;
        
        // Resampling state tracking
        let mut source_index: f32 = 0.0;
        let resample_ratio = sample_rate as f32 / 16000.0;
        
        // Pre-allocate buffers to ensure ZERO allocations in the hot path callback.
        // Capacity is chosen to handle up to ~500ms of audio (8192 samples).
        let mut mono_buffer = Vec::with_capacity(8192);
        let mut resampled_buffer = Vec::with_capacity(8192);
        
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Clear buffers without deallocating the underlying memory.
                mono_buffer.clear();
                resampled_buffer.clear();

                // 1. Mono conversion (Channel Averaging)
                // Hardware often provides stereo or multi-channel audio. 
                // We average all channels into a single mono stream.
                for chunk in data.chunks_exact(channels) {
                    let avg: f32 = chunk.iter().sum::<f32>() / channels as f32;
                    mono_buffer.push(avg);
                }
                
                let n_mono = mono_buffer.len();
                
                // 2. Linear Resampling to 16kHz
                // Sherpa-ONNX models (Qwen3, TenVAD) strictly require 16kHz audio.
                while (source_index as usize) < n_mono {
                    let idx = source_index as usize;
                    let next_idx = (idx + 1).min(n_mono - 1);
                    let frac = source_index - idx as f32;
                    
                    // Basic linear interpolation
                    let sample = (1.0 - frac) * mono_buffer[idx] + frac * mono_buffer[next_idx];
                    resampled_buffer.push(sample);
                    
                    source_index += resample_ratio;
                }
                
                // 3. Keep index relative for the next callback block.
                source_index -= n_mono as f32;
                
                // 4. Push to the lock-free SPSC ring buffer for Tier 2 (VAD) processing.
                if !resampled_buffer.is_empty() {
                    let pushed = producer.push_slice(&resampled_buffer);
                    if pushed < resampled_buffer.len() {
                        // Throttled logging for buffer overflows. 
                        // If this happens often, the VAD thread is running too slowly.
                        static DROP_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                        let prev = DROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if prev % 100 == 0 {
                            log::warn!("[AUDIO] Ring buffer overflow! Dropped {} chunks so far.", prev + 1);
                        }
                    }
                }
            },
            move |err| {
                log::error!("[AUDIO] Stream error: {}", err);
            },
            None,
        )?;

        Ok(Self { _stream: stream })
    }

    /// Starts the hardware audio stream.
    pub fn start(&self) -> Result<()> {
        log::info!("[AUDIO] Starting hardware ingestion...");
        self._stream.play()?;
        Ok(())
    }
}
