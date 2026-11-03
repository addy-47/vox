use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc,
};

use tokio::sync::{mpsc, RwLock as AsyncRwLock};

use crate::services::remote::{
    link::RemoteLink,
    protocol::{
        framing::{AudioFrame, AudioPayload},
        messages::{AudioControlMessage, Flush, PttMarker, SpeechEnd, SpeechStart},
        AUDIO_CODEC_PCM16,
    },
};

/// Spec §7.4: 20 ms frames (320 samples = 640 bytes at 16 kHz mono PCM16).
pub const FRAME_SIZE_16KHZ_20MS: usize = 320;

/// Channel capacity for uplink audio frame queue.
const AUDIO_SENDER_QUEUE_CAPACITY: usize = 256;

/// Uplink network audio sender on the controller that transmits VAD speech-start,
/// speech-end, and 16 kHz Mono PCM16 speech frames over the audio WebSocket.
pub struct NetworkAudioSender {
    seq: AtomicU32,
    epoch: Arc<AtomicU64>,
    sample_offset: AtomicU64,
    tx: mpsc::Sender<AudioPayload>,
}

impl NetworkAudioSender {
    /// Creates a new `NetworkAudioSender` referencing the active audio link and epoch counter.
    pub fn new(
        audio_link: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
        epoch: Arc<AtomicU64>,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<AudioPayload>(AUDIO_SENDER_QUEUE_CAPACITY);
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

    /// Transmits a SpeechStart control message over the audio socket.
    pub async fn send_speech_start(&self) {
        let epoch = self.epoch.load(Ordering::Acquire);
        let sample_offset = self.sample_offset.load(Ordering::Acquire);
        let start = AudioControlMessage::SpeechStart(SpeechStart {
            epoch,
            sample_offset,
        });
        let _ = self.tx.send(AudioPayload::Control(start)).await;
    }

    /// Transmits 16 kHz Mono float audio samples uplink over the audio socket,
    /// converting them to PCM16 and slicing into 20ms (320 sample) frames.
    pub async fn send_samples(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let epoch = self.epoch.load(Ordering::Acquire);
        for chunk in samples.chunks(FRAME_SIZE_16KHZ_20MS) {
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let offset = self
                .sample_offset
                .fetch_add(chunk.len() as u64, Ordering::Relaxed);

            let pcm16_samples: Vec<i16> = chunk
                .iter()
                .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect();

            let frame = AudioFrame {
                codec: AUDIO_CODEC_PCM16,
                seq,
                epoch,
                sample_offset: offset,
                samples: pcm16_samples,
            };

            if self.tx.send(AudioPayload::Frame(frame)).await.is_err() {
                break;
            }
        }
    }

    /// Transmits a SpeechEnd control message over the audio socket.
    pub async fn send_speech_end(&self, sample_count: u64) {
        let epoch = self.epoch.load(Ordering::Acquire);
        let end = AudioControlMessage::SpeechEnd(SpeechEnd {
            epoch,
            sample_count,
        });
        let _ = self.tx.send(AudioPayload::Control(end)).await;
    }

    /// Transmits a PttMarker message over the audio socket.
    pub async fn send_ptt_marker(&self, marker: PttMarker) {
        let ptt = AudioControlMessage::PttMarker(marker);
        let _ = self.tx.send(AudioPayload::Control(ptt)).await;
    }

    /// Transmits a Flush message over the audio socket, advancing the epoch monotonically.
    pub async fn send_flush(&self) -> u64 {
        let new_epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let flush = AudioControlMessage::Flush(Flush { epoch: new_epoch });
        let _ = self.tx.send(AudioPayload::Control(flush)).await;
        new_epoch
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
    async fn network_audio_sender_sends_speech_start_frames_and_end() {
        let (controller_link, host_link) = LoopbackLink::pair();
        let link_slot = Arc::new(AsyncRwLock::new(Some(
            Arc::new(controller_link) as Arc<dyn RemoteLink>
        )));
        let epoch = Arc::new(AtomicU64::new(1));
        let sender = NetworkAudioSender::new(link_slot, epoch);

        sender.send_speech_start().await;
        sender.send_samples(&[0.5; 320]).await;
        sender.send_speech_end(320).await;

        let p1 = host_link.recv_audio().await.expect("p1");
        match p1 {
            AudioPayload::Control(AudioControlMessage::SpeechStart(s)) => {
                assert_eq!(s.epoch, 1);
                assert_eq!(s.sample_offset, 0);
            }
            other => panic!("expected SpeechStart, got {other:?}"),
        }

        let p2 = host_link.recv_audio().await.expect("p2");
        match p2 {
            AudioPayload::Frame(f) => {
                assert_eq!(f.samples.len(), 320);
                assert_eq!(f.epoch, 1);
            }
            other => panic!("expected AudioFrame, got {other:?}"),
        }

        let p3 = host_link.recv_audio().await.expect("p3");
        match p3 {
            AudioPayload::Control(AudioControlMessage::SpeechEnd(e)) => {
                assert_eq!(e.epoch, 1);
                assert_eq!(e.sample_count, 320);
            }
            other => panic!("expected SpeechEnd, got {other:?}"),
        }
    }
}
