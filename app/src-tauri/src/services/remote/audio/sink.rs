use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc,
};

use tokio::sync::RwLock as AsyncRwLock;

use crate::services::remote::{
    link::RemoteLink,
    protocol::{
        framing::{AudioFrame, AudioPayload},
        messages::{AudioControlMessage, Flush, PttMarker},
        AUDIO_CODEC_PCM16,
    },
};

/// Downlink network audio sink that packetizes 24 kHz Mono PCM16 frames and transmits
/// them over the dedicated audio WebSocket to the controller.
///
/// Spec §7.4: audio payloads are tagged with 0x00 for binary frames and 0x01 for control messages.
pub struct NetworkAudioSink {
    audio_link: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
    seq: AtomicU32,
    epoch: Arc<AtomicU64>,
    sample_offset: AtomicU64,
}

impl NetworkAudioSink {
    /// Creates a new `NetworkAudioSink` referencing the active audio link and epoch counter.
    pub fn new(
        audio_link: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
        epoch: Arc<AtomicU64>,
    ) -> Self {
        Self {
            audio_link,
            seq: AtomicU32::new(1),
            epoch,
            sample_offset: AtomicU64::new(0),
        }
    }

    /// Transmits 24 kHz Mono PCM16 audio samples downlink over the audio socket.
    pub async fn send_samples(&self, samples: Vec<i16>) {
        if samples.is_empty() {
            return;
        }
        let link_opt = self.audio_link.read().await.clone();
        let Some(link) = link_opt else {
            return;
        };

        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let epoch = self.epoch.load(Ordering::Acquire);
        let sample_count = samples.len() as u64;
        let offset = self.sample_offset.fetch_add(sample_count, Ordering::Relaxed);

        let frame = AudioFrame {
            codec: AUDIO_CODEC_PCM16,
            seq,
            epoch,
            sample_offset: offset,
            samples,
        };

        let _ = link.send_audio(AudioPayload::Frame(frame)).await;
    }

    /// Transmits a Flush message over the audio socket, advancing the epoch monotonically.
    pub async fn send_flush(&self) -> u64 {
        let new_epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let link_opt = self.audio_link.read().await.clone();
        if let Some(link) = link_opt {
            let flush = AudioControlMessage::Flush(Flush { epoch: new_epoch });
            let _ = link.send_audio(AudioPayload::Control(flush)).await;
        }
        new_epoch
    }

    /// Transmits a PttMarker message over the audio socket.
    pub async fn send_ptt_marker(&self, marker: PttMarker) {
        let link_opt = self.audio_link.read().await.clone();
        if let Some(link) = link_opt {
            let ptt = AudioControlMessage::PttMarker(marker);
            let _ = link.send_audio(AudioPayload::Control(ptt)).await;
        }
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
        let link_slot = Arc::new(AsyncRwLock::new(Some(Arc::new(host_link) as Arc<dyn RemoteLink>)));
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
