use crate::services::playback::PlaybackEngine;
use crate::services::realtime::{resampler::AudioResampler, RealtimeAudioConfig};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub struct PlaybackBridge {
    tx: Option<UnboundedSender<Vec<i16>>>,
}

impl PlaybackBridge {
    pub fn new() -> Self {
        Self { tx: None }
    }

    pub fn start(
        &mut self,
        playback_engine: Arc<PlaybackEngine>,
        config: RealtimeAudioConfig,
        handle: &tokio::runtime::Handle,
    ) {
        let (tx, mut rx) = unbounded_channel::<Vec<i16>>();
        self.tx = Some(tx);

        handle.spawn(async move {
            let mut resampler =
                if config.requires_output_resampling && config.output_sample_rate != 24000 {
                    match AudioResampler::new(config.output_sample_rate, 24000, 512) {
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

                let f32_chunk: Vec<f32> = pcm_24k.iter().map(|&x| x as f32 / 32768.0).collect();

                playback_engine.ingest_chunk(&f32_chunk);
                playback_engine.start_playback();
            }
        });
    }

    pub fn stop(&mut self) {
        self.tx = None;
    }

    pub fn get_sender(&self) -> Option<UnboundedSender<Vec<i16>>> {
        self.tx.clone()
    }
}
