use crate::services::realtime::{resampler::AudioResampler, RealtimeAudioConfig, RealtimeSession};
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Sender};

pub struct AudioBridge {
    tx: Option<Sender<Vec<i16>>>,
}

impl AudioBridge {
    pub fn new() -> Self {
        Self { tx: None }
    }

    pub fn start(
        &mut self,
        session: Arc<dyn RealtimeSession>,
        config: RealtimeAudioConfig,
        handle: &tokio::runtime::Handle,
    ) {
        let (tx, mut rx) = channel::<Vec<i16>>(100);
        self.tx = Some(tx);

        handle.spawn(async move {
            let mut resampler = if config.requires_input_resampling {
                match AudioResampler::new(16000, config.input_sample_rate, 320) {
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

    pub fn stop(&mut self) {
        self.tx = None;
    }

    pub fn get_sender(&self) -> Option<Sender<Vec<i16>>> {
        self.tx.clone()
    }

    pub fn send_pcm(&self, samples: &[i16]) {
        if let Some(ref tx) = self.tx {
            if let Err(e) = tx.try_send(samples.to_vec()) {
                match e {
                    tokio::sync::mpsc::error::TrySendError::Full(_) => {
                        static DROP_COUNT: std::sync::atomic::AtomicUsize =
                            std::sync::atomic::AtomicUsize::new(0);
                        let prev = DROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if prev % 100 == 0 {
                            log::warn!(
                                "[AudioBridge] Channel buffer full, dropping input audio chunk. Dropped {} chunks so far.",
                                prev + 1
                            );
                        }
                    }
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                        log::debug!("[AudioBridge] Channel closed.");
                    }
                }
            }
        }
    }
}
