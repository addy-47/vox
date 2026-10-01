use std::sync::Arc;

use tokio::sync::mpsc::{channel, Sender};

use crate::services::{
    audio::{AudioResampler, SINC_CHUNK_SIZE_INPUT},
    realtime::{
        RealtimeAudioConfig, RealtimeSession, BRIDGE_CHANNEL_CAPACITY, DEFAULT_INPUT_SAMPLE_RATE,
    },
};

/// Bridges captured PCM audio frames to active realtime streaming sessions with automatic resampling.
pub struct AudioBridge {
    tx: Option<Sender<Vec<i16>>>,
}

impl Default for AudioBridge {
    /// Creates an empty default audio bridge.
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBridge {
    /// Creates a new uninitialized AudioBridge instance.
    pub fn new() -> Self {
        Self { tx: None }
    }

    /// Starts streaming worker task routing PCM audio to the realtime session.
    pub fn start(
        &mut self,
        session: Arc<dyn RealtimeSession>,
        config: RealtimeAudioConfig,
        handle: &tokio::runtime::Handle,
    ) {
        let (tx, mut rx) = channel::<Vec<i16>>(BRIDGE_CHANNEL_CAPACITY);
        self.tx = Some(tx);

        handle.spawn(async move {
            let mut resampler = if config.requires_input_resampling
                || config.input_sample_rate != DEFAULT_INPUT_SAMPLE_RATE
            {
                match AudioResampler::new(
                    DEFAULT_INPUT_SAMPLE_RATE,
                    config.input_sample_rate,
                    SINC_CHUNK_SIZE_INPUT,
                ) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        log::error!("[AudioBridge] Failed to create input resampler: {:?}", e);
                        None
                    }
                }
            } else {
                None
            };

            while let Some(pcm) = rx.recv().await {
                let resampled = if let Some(ref mut r) = resampler {
                    match r.process_i16(&pcm) {
                        Ok(out) => out,
                        Err(e) => {
                            log::error!("[AudioBridge] Resampling error: {:?}", e);
                            continue;
                        }
                    }
                } else {
                    pcm
                };

                if let Err(e) = session.send_audio(&resampled) {
                    log::error!("[AudioBridge] Failed to send audio to session: {:?}", e);
                    break;
                }
            }
        });
    }

    /// Stops the audio bridge and drops channel sender.
    pub fn stop(&mut self) {
        self.tx = None;
    }

    /// Returns a clone of the internal audio channel sender if active.
    pub fn get_sender(&self) -> Option<Sender<Vec<i16>>> {
        self.tx.clone()
    }


}
