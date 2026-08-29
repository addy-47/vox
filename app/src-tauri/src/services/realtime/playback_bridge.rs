use crate::services::audio::PlaybackEngine;
use crate::services::realtime::{
    resampler::AudioResampler, RealtimeAudioConfig, BRIDGE_CHANNEL_CAPACITY,
    DEFAULT_OUTPUT_SAMPLE_RATE, PCM_INT16_DIVISOR_FLOAT, SINC_CHUNK_SIZE_OUTPUT,
};
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Sender};

/// Bridges incoming synthesized audio PCM stream from realtime WebSocket to the local PlaybackEngine.
pub struct PlaybackBridge {
    tx: Option<Sender<Vec<i16>>>,
}

impl Default for PlaybackBridge {
    /// Creates a default uninitialized playback bridge.
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackBridge {
    /// Creates a new PlaybackBridge instance.
    pub fn new() -> Self {
        Self { tx: None }
    }

    /// Spawns worker task receiving remote PCM chunks, resampling to 24kHz, and feeding PlaybackEngine.
    pub fn start(
        &mut self,
        playback_engine: Arc<PlaybackEngine>,
        config: RealtimeAudioConfig,
        handle: &tokio::runtime::Handle,
    ) {
        let (tx, mut rx) = channel::<Vec<i16>>(BRIDGE_CHANNEL_CAPACITY);
        self.tx = Some(tx);

        handle.spawn(async move {
            let mut resampler = if config.requires_output_resampling
                || config.output_sample_rate != DEFAULT_OUTPUT_SAMPLE_RATE
            {
                match AudioResampler::new(
                    config.output_sample_rate,
                    DEFAULT_OUTPUT_SAMPLE_RATE,
                    SINC_CHUNK_SIZE_OUTPUT,
                ) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        log::error!(
                            "[PlaybackBridge] Failed to create output resampler: {:?}",
                            e
                        );
                        None
                    }
                }
            } else {
                None
            };

            let mut f32_chunk = Vec::with_capacity(1024);
            while let Some(pcm) = rx.recv().await {
                let pcm_24k = if let Some(ref mut r) = resampler {
                    match r.process_i16(&pcm) {
                        Ok(out) => out,
                        Err(e) => {
                            log::error!("[PlaybackBridge] Resampling error: {:?}", e);
                            continue;
                        }
                    }
                } else {
                    pcm
                };

                f32_chunk.clear();
                f32_chunk.extend(pcm_24k.iter().map(|&x| x as f32 / PCM_INT16_DIVISOR_FLOAT));

                playback_engine.ingest_chunk(&f32_chunk);
                playback_engine.start_playback();
            }
        });
    }

    /// Stops the playback bridge and closes the channel sender.
    pub fn stop(&mut self) {
        self.tx = None;
    }

    /// Returns a clone of the channel sender for dispatching PCM audio.
    pub fn get_sender(&self) -> Option<Sender<Vec<i16>>> {
        self.tx.clone()
    }
}
