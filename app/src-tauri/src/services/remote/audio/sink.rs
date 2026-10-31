use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc,
};

use tokio::sync::{mpsc, RwLock as AsyncRwLock};

use crate::services::remote::{
    link::RemoteLink,
    protocol::{
        framing::{AudioFrame, AudioPayload},
        messages::{AudioControlMessage, Flush, PttMarker},
        AUDIO_CODEC_PCM16,
    },
};

/// Spec §7.4: 20 ms frames (480 samples = 960 bytes at 24 kHz mono PCM16).
pub const FRAME_SIZE_24KHZ_20MS: usize = 480;

/// Channel capacity for downlink audio frame queue.
const AUDIO_SINK_QUEUE_CAPACITY: usize = 256;

/// Downlink network audio sink that packetizes 24 kHz Mono PCM16 frames into
/// strictly ordered 20 ms frames and transmits them over a FIFO sender loop.
pub struct NetworkAudioSink {
    seq: AtomicU32,
    epoch: Arc<AtomicU64>,
    sample_offset: AtomicU64,
    tx: mpsc::Sender<AudioPayload>,
}

impl NetworkAudioSink {
    /// Creates a new `NetworkAudioSink` referencing the active audio link and epoch counter.
    pub fn new(
        audio_link: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
        epoch: Arc<AtomicU64>,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<AudioPayload>(AUDIO_SINK_QUEUE_CAPACITY);
        let link_slot = Arc::clone(&audio_link);

        tokio::spawn(async move {
            while let Some(payload) = rx.recv().await {
                let link_opt = link_slot.read().await.clone();
                if let Some(link) = link_opt {
                    let _ = link.send_audio(payload).await;
                }
            }
        });

        Self {
            seq: AtomicU32::new(1),
            epoch,
            sample_offset: AtomicU64::new(0),
            tx,
        }
    }

    /// Transmits 24 kHz Mono PCM16 audio samples downlink over the audio socket,
    /// slicing them into 20ms (480 sample) frames to comply with the 64KB Noise limit.
    pub async fn send_samples(&self, samples: Vec<i16>) {
        if samples.is_empty() {
            return;
        }

        let epoch = self.epoch.load(Ordering::Acquire);
        for chunk in samples.chunks(FRAME_SIZE_24KHZ_20MS) {
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let offset = self
                .sample_offset
                .fetch_add(chunk.len() as u64, Ordering::Relaxed);

            let frame = AudioFrame {
                codec: AUDIO_CODEC_PCM16,
                seq,
                epoch,
                sample_offset: offset,
                samples: chunk.to_vec(),
            };

            if self.tx.send(AudioPayload::Frame(frame)).await.is_err() {
                break;
            }
        }
    }

    /// Transmits a Flush message over the audio socket, advancing the epoch monotonically.
    pub async fn send_flush(&self) -> u64 {
        let new_epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let flush = AudioControlMessage::Flush(Flush { epoch: new_epoch });
        let _ = self.tx.send(AudioPayload::Control(flush)).await;
        new_epoch
    }

    /// Transmits a PttMarker message over the audio socket.
    pub async fn send_ptt_marker(&self, marker: PttMarker) {
        let ptt = AudioControlMessage::PttMarker(marker);
        let _ = self.tx.send(AudioPayload::Control(ptt)).await;
    }

    /// Returns the current epoch number.
    pub fn current_epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::remote::link::loopback::LoopbackLink;

    #[tokio::test]
    async fn network_audio_sink_sends_frames_and_flush() {
        let (host_link, client_link) = LoopbackLink::pair();
        let link_slot = Arc::new(AsyncRwLock::new(Some(
            Arc::new(host_link) as Arc<dyn RemoteLink>
        )));
        let epoch = Arc::new(AtomicU64::new(1));
        let sink = NetworkAudioSink::new(link_slot, epoch);

        sink.send_samples(vec![100, 200, 300]).await;
        let received = client_link.recv_audio().await.expect("must receive frame");
        match received {
            AudioPayload::Frame(f) => {
                assert_eq!(f.codec, AUDIO_CODEC_PCM16);
                assert_eq!(f.epoch, 1);
                assert_eq!(f.samples, vec![100, 200, 300]);
            }
            other => panic!("expected AudioPayload::Frame, got {other:?}"),
        }

        let new_epoch = sink.send_flush().await;
        assert_eq!(new_epoch, 2);
        let received_flush = client_link.recv_audio().await.expect("must receive flush");
        match received_flush {
            AudioPayload::Control(AudioControlMessage::Flush(fl)) => {
                assert_eq!(fl.epoch, 2);
            }
            other => panic!("expected AudioPayload::Control(Flush), got {other:?}"),
        }
    }
}
