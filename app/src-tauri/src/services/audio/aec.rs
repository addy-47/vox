use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

use anyhow::{anyhow, Result};
use ringbuf::{traits::*, HeapCons};
use sonora::{
    config::{Config, EchoCanceller, HighPassFilter},
    AudioProcessing, StreamConfig,
};

use super::{AEC_FRAME_SAMPLES, AEC_SAMPLE_RATE};

const DEFAULT_STREAM_DELAY_MS: i32 = 40;
const MAX_RENDER_QUEUE_BACKLOG_SAMPLES: usize = 3200;

/// Commands dispatched to the dedicated AEC worker thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AecCommand {
    SetEnabled(bool),
    SetStreamDelayMs(i32),
    ClearRenderQueue,
}

/// Telemetry statistics from the AEC processor.
#[derive(Debug, Clone, Default)]
pub struct AecStatistics {
    pub echo_return_loss: Option<f64>,
    pub echo_return_loss_enhancement: Option<f64>,
    pub delay_ms: Option<i32>,
    pub delay_median_ms: Option<i32>,
    pub residual_echo_likelihood: Option<f64>,
    pub underruns: u64,
    pub overflows: u64,
}

/// Acoustic Echo Cancellation (AEC3) processing engine wrapping Sonora WebRTC APM.
pub struct AecProcessor {
    apm: AudioProcessing,
    render_consumer: Option<HeapCons<f32>>,
    render_buf: [f32; AEC_FRAME_SAMPLES],
    render_dest: [f32; AEC_FRAME_SAMPLES],
    capture_buf: [f32; AEC_FRAME_SAMPLES],
    capture_dest: [f32; AEC_FRAME_SAMPLES],
    enabled: bool,
    stream_delay_ms: i32,
    ringbuf_underruns: u64,
    ringbuf_overflows: u64,
    frame_counter: u32,
}

impl AecProcessor {
    /// Creates a new AEC processor for 16kHz mono audio with an optional render reference consumer.
    pub fn new(render_consumer: Option<HeapCons<f32>>) -> Result<Self> {
        let config = Config {
            echo_canceller: Some(EchoCanceller::default()),
            high_pass_filter: Some(HighPassFilter::default()),
            ..Default::default()
        };

        let stream_config = StreamConfig::new(AEC_SAMPLE_RATE, 1);
        let mut apm = AudioProcessing::builder()
            .config(config)
            .capture_config(stream_config)
            .render_config(stream_config)
            .echo_detector(true)
            .build();

        apm.set_stream_delay_ms(DEFAULT_STREAM_DELAY_MS)
            .map_err(|e| anyhow!("Failed to set AEC initial stream delay: {}", e))?;

        Ok(Self {
            apm,
            render_consumer,
            render_buf: [0.0; AEC_FRAME_SAMPLES],
            render_dest: [0.0; AEC_FRAME_SAMPLES],
            capture_buf: [0.0; AEC_FRAME_SAMPLES],
            capture_dest: [0.0; AEC_FRAME_SAMPLES],
            enabled: true,
            stream_delay_ms: DEFAULT_STREAM_DELAY_MS,
            ringbuf_underruns: 0,
            ringbuf_overflows: 0,
            frame_counter: 0,
        })
    }

    /// Clears any pending render reference audio immediately (e.g. on playback cancel or interrupt).
    pub fn clear_render_queue(&mut self) {
        if let Some(ref mut cons) = self.render_consumer {
            let occupied = cons.occupied_len();
            if occupied > 0 {
                cons.skip(occupied);
            }
        }
    }

    /// Enables or disables AEC processing (bypass mode).
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear_render_queue();
        }
    }

    /// Returns whether AEC processing is currently active.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets the estimated delay in milliseconds between playback render and microphone capture.
    pub fn set_stream_delay_ms(&mut self, delay_ms: i32) -> Result<()> {
        self.stream_delay_ms = delay_ms;
        self.apm
            .set_stream_delay_ms(delay_ms)
            .map_err(|e| anyhow!("Failed to set AEC delay: {}", e))
    }

    /// Returns the current stream delay in milliseconds.
    pub fn stream_delay_ms(&self) -> i32 {
        self.stream_delay_ms
    }

    /// Directly processes a 10ms render reference audio frame (160 samples @ 16kHz).
    pub fn process_render_frame(&mut self, render_frame: &[f32]) -> Result<()> {
        if render_frame.len() != AEC_FRAME_SAMPLES {
            return Err(anyhow!(
                "Render frame length mismatch: expected {}, got {}",
                AEC_FRAME_SAMPLES,
                render_frame.len()
            ));
        }
        self.render_buf.copy_from_slice(render_frame);
        let render_in: [&[f32]; 1] = [&self.render_buf];
        let mut render_out: [&mut [f32]; 1] = [&mut self.render_dest];
        self.apm
            .process_render_f32(&render_in, &mut render_out)
            .map_err(|e| anyhow!("AEC render processing error: {}", e))?;
        Ok(())
    }

    /// Drains available samples from the render reference queue into the internal render buffer.
    fn drain_render_reference(&mut self) {
        if let Some(ref mut cons) = self.render_consumer {
            let occupied = cons.occupied_len();
            // Critical fix: discard only the excess over MAX_RENDER_QUEUE_BACKLOG_SAMPLES,
            // rather than wiping the entire backlog down to a single frame and destroying AEC reference continuity.
            if occupied > MAX_RENDER_QUEUE_BACKLOG_SAMPLES {
                let excess = occupied - MAX_RENDER_QUEUE_BACKLOG_SAMPLES;
                cons.skip(excess);
                self.ringbuf_overflows += 1;
            }
            let mut frames_processed = 0;
            // Drain up to 4 frames (40ms) if playback runs ahead of capture to maintain synchronization.
            while cons.occupied_len() >= AEC_FRAME_SAMPLES && frames_processed < 4 {
                let popped = cons.pop_slice(&mut self.render_buf);
                if popped == AEC_FRAME_SAMPLES {
                    let render_in: [&[f32]; 1] = [&self.render_buf];
                    let mut render_out: [&mut [f32]; 1] = [&mut self.render_dest];
                    let _ = self.apm.process_render_f32(&render_in, &mut render_out);
                    frames_processed += 1;
                }
            }
            if frames_processed == 0 {
                // If reference buffer has no frame (e.g. slight jitter or initial startup),
                // feed silence so APM render clock stays synchronous with capture clock.
                self.render_buf.fill(0.0);
                let render_in: [&[f32]; 1] = [&self.render_buf];
                let mut render_out: [&mut [f32]; 1] = [&mut self.render_dest];
                let _ = self.apm.process_render_f32(&render_in, &mut render_out);
            }
        }
    }

    /// Processes a single 10ms capture frame (160 samples @ 16kHz), removing acoustic echo.
    pub fn process_frame(&mut self, capture: &[f32], output: &mut [f32]) -> Result<()> {
        if capture.len() != AEC_FRAME_SAMPLES || output.len() != AEC_FRAME_SAMPLES {
            return Err(anyhow!(
                "Frame length mismatch: expected {}, got capture={}, output={}",
                AEC_FRAME_SAMPLES,
                capture.len(),
                output.len()
            ));
        }

        if !self.enabled {
            output.copy_from_slice(capture);
            self.clear_render_queue();
            return Ok(());
        }

        self.drain_render_reference();

        self.capture_buf.copy_from_slice(capture);
        let capture_in: [&[f32]; 1] = [&self.capture_buf];
        let mut capture_out: [&mut [f32]; 1] = [&mut self.capture_dest];
        self.apm
            .process_capture_f32(&capture_in, &mut capture_out)
            .map_err(|e| anyhow!("AEC capture processing error: {}", e))?;

        output.copy_from_slice(&self.capture_dest);

        // Dynamic delay adaptation: poll AEC3 reported delay median every 100 frames (~1 sec)
        self.frame_counter = (self.frame_counter + 1) % 100;
        if self.frame_counter == 0 {
            let stats = self.apm.statistics();
            if let Some(median) = stats.delay_median_ms {
                if median > 0 && (median - self.stream_delay_ms).abs() >= 15 {
                    log::info!(
                        "[Audio::AEC] Dynamic delay adaptation: adapting stream_delay_ms from {} ms to {} ms (reported median)",
                        self.stream_delay_ms,
                        median
                    );
                    let _ = self.set_stream_delay_ms(median);
                }
            }
        }

        Ok(())
    }

    /// Processes an arbitrary slice of 16kHz audio samples in 160-sample increments.
    pub fn process_stream(&mut self, capture: &[f32], output: &mut Vec<f32>) -> Result<()> {
        output.clear();
        output.reserve(capture.len());
        let mut out_frame = [0.0f32; AEC_FRAME_SAMPLES];

        for chunk in capture.chunks(AEC_FRAME_SAMPLES) {
            if chunk.len() == AEC_FRAME_SAMPLES {
                self.process_frame(chunk, &mut out_frame)?;
                output.extend_from_slice(&out_frame);
            } else {
                let mut padded = [0.0f32; AEC_FRAME_SAMPLES];
                padded[..chunk.len()].copy_from_slice(chunk);
                self.process_frame(&padded, &mut out_frame)?;
                output.extend_from_slice(&out_frame[..chunk.len()]);
            }
        }

        Ok(())
    }

    /// Collects AEC operational statistics and telemetry metrics.
    pub fn statistics(&self) -> AecStatistics {
        let stats = self.apm.statistics();
        AecStatistics {
            echo_return_loss: stats.echo_return_loss,
            echo_return_loss_enhancement: stats.echo_return_loss_enhancement,
            delay_ms: stats.delay_ms,
            delay_median_ms: stats.delay_median_ms,
            residual_echo_likelihood: stats.residual_echo_likelihood,
            underruns: self.ringbuf_underruns,
            overflows: self.ringbuf_overflows,
        }
    }
}

/// Spawns the dedicated, synchronous AEC worker thread on an elevated OS thread.
pub fn spawn_aec_worker<CapC, CleanP>(
    mut aec: AecProcessor,
    mut capture_consumer: CapC,
    mut clean_producer: CleanP,
    shutdown: Arc<AtomicBool>,
    command_rx: Option<mpsc::Receiver<AecCommand>>,
) -> Result<std::thread::JoinHandle<()>>
where
    CapC: Consumer<Item = f32> + Send + 'static,
    CleanP: Producer<Item = f32> + Send + 'static,
{
    std::thread::Builder::new()
        .name("vox-aec-worker".to_string())
        .spawn(move || {
            if let Err(e) =
                thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max)
            {
                log::warn!("[Audio::AEC] Thread priority elevation failed: {:?}", e);
            }
            log::info!("[Audio::AEC] Synchronous AEC worker loop started on dedicated thread");

            let mut capture_frame = [0.0f32; AEC_FRAME_SAMPLES];
            let mut clean_frame = [0.0f32; AEC_FRAME_SAMPLES];

            while !shutdown.load(Ordering::Relaxed) {
                if let Some(ref rx) = command_rx {
                    while let Ok(cmd) = rx.try_recv() {
                        match cmd {
                            AecCommand::SetEnabled(enabled) => {
                                log::info!("[Audio::AEC] Updating enabled state to: {}", enabled);
                                aec.set_enabled(enabled);
                            }
                            AecCommand::SetStreamDelayMs(delay) => {
                                log::info!("[Audio::AEC] Updating stream delay to: {} ms", delay);
                                if let Err(e) = aec.set_stream_delay_ms(delay) {
                                    log::warn!("[Audio::AEC] Failed to set stream delay: {}", e);
                                }
                            }
                            AecCommand::ClearRenderQueue => {
                                aec.clear_render_queue();
                                log::debug!("[Audio::AEC] Flushed render reference backlog queue");
                            }
                        }
                    }
                }

                if capture_consumer.occupied_len() >= AEC_FRAME_SAMPLES {
                    let popped = capture_consumer.pop_slice(&mut capture_frame);
                    if popped == AEC_FRAME_SAMPLES {
                        if let Err(e) = aec.process_frame(&capture_frame, &mut clean_frame) {
                            log::warn!("[Audio::AEC] AEC processing error: {}", e);
                            clean_frame.copy_from_slice(&capture_frame);
                        }
                        let pushed = clean_producer.push_slice(&clean_frame);
                        if pushed < AEC_FRAME_SAMPLES {
                            log::warn!(
                                "[Audio::AEC] Clean audio buffer overflow — dropped {} samples",
                                AEC_FRAME_SAMPLES - pushed
                            );
                        }
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }

            log::info!("[Audio::AEC] AEC worker loop stopped");
        })
        .map_err(|e| anyhow!("Failed to spawn AEC worker thread: {}", e))
}
