use crate::services::realtime::{resampler::AudioResampler, RealtimeAudioConfig, RealtimeSession};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub struct AudioBridge {
    tx: Option<UnboundedSender<Vec<i16>>>,
    queue_depth: Arc<AtomicUsize>,
}

impl AudioBridge {
    pub fn new() -> Self {
        Self {
            tx: None,
            queue_depth: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn start(
        &mut self,
        session: Arc<dyn RealtimeSession>,
        config: RealtimeAudioConfig,
        handle: &tokio::runtime::Handle,
    ) {
        let (tx, mut rx) = unbounded_channel::<Vec<i16>>();
        self.tx = Some(tx);
        self.queue_depth.store(0, Ordering::SeqCst);
        let queue_depth = self.queue_depth.clone();

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
                queue_depth.fetch_sub(1, Ordering::SeqCst);

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
        self.queue_depth.store(0, Ordering::SeqCst);
    }

    pub fn get_sender(&self) -> Option<UnboundedSender<Vec<i16>>> {
        self.tx.clone()
    }

    pub fn send_pcm(&self, samples: &[i16]) {
        if let Some(ref tx) = self.tx {
            let depth = self.queue_depth.load(Ordering::SeqCst);
            if depth > 100 {
                static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
                let prev = DROP_COUNT.fetch_add(1, Ordering::Relaxed);
                if prev % 100 == 0 {
                    log::warn!(
                        "[AudioBridge] Queue depth is {}, dropping input audio chunk. Dropped {} chunks so far.",
                        depth,
                        prev + 1
                    );
                }
                return;
            }

            self.queue_depth.fetch_add(1, Ordering::SeqCst);
            if let Err(_) = tx.send(samples.to_vec()) {
                self.queue_depth.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }
}
