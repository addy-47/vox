use crate::services::realtime::{resampler::AudioResampler, RealtimeAudioConfig, RealtimeSession};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Sender};

static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct AudioBridge {
    tx: Option<Sender<Vec<i16>>>,
}

impl Default for AudioBridge {
    fn default() -> Self {
        Self::new()
    }
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
                        let prev = DROP_COUNT.fetch_add(1, Ordering::Relaxed);
                        if prev.is_multiple_of(100) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_bridge_non_blocking_drop() {
        let mut bridge = AudioBridge::new();
        let (tx, _rx) = channel::<Vec<i16>>(100);
        bridge.tx = Some(tx);

        let initial_drops = DROP_COUNT.load(Ordering::Relaxed);

        let chunk = vec![0i16; 320];
        // Fill bounded MPSC audio channel (100 capacity) to saturation
        for _ in 0..100 {
            bridge.send_pcm(&chunk);
        }

        let drops_after_filling = DROP_COUNT.load(Ordering::Relaxed);
        assert_eq!(
            drops_after_filling, initial_drops,
            "No chunks should be dropped when channel is within capacity"
        );

        // Invoke send_pcm with 101st chunk
        bridge.send_pcm(&chunk);

        let drops_after_overflow = DROP_COUNT.load(Ordering::Relaxed);
        assert_eq!(
            drops_after_overflow,
            initial_drops + 1,
            "101st chunk should trigger non-blocking drop and increment drop counter"
        );
    }

    #[test]
    fn test_audio_bridge_closed_channel_safety() {
        let mut bridge = AudioBridge::new();
        let (tx, rx) = channel::<Vec<i16>>(100);
        bridge.tx = Some(tx);

        // Drop receiver to simulate WebSocket disconnect
        drop(rx);

        let chunk = vec![0i16; 320];
        // Verify send_pcm completes safely without panic
        bridge.send_pcm(&chunk);
    }
}
