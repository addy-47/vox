use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

use anyhow::Result;
use parking_lot::Mutex;
use ringbuf::traits::*;

use crate::services::{
    audio::AudioCapture,
    remote::protocol::{
        errors::ProtocolError,
        framing::{decode_audio_payload, AudioPayload},
        messages::{AudioControlMessage, DecodeOutcome, PttMarker},
    },
};

/// Ingestor trait for decoded audio payloads (used by the host reader loop).
pub trait NetworkAudioIngestor: Send + Sync {
    fn ingest(&self, payload: AudioPayload) -> Result<(), ProtocolError>;
}

/// Scales 16-bit signed integer sample to 32-bit floating point range [-1.0, 1.0].
const PCM16_TO_F32_SCALE: f32 = 1.0 / 32768.0;

/// Processes one decrypted payload from the audio WebSocket without blocking or allocating on the audio thread.
///
/// Spec §7.4 & VERIFY-RC-12: Every payload on the audio socket is tagged (0x00 binary AudioFrame,
/// 0x01 JSON AudioControlMessage). Frames with epoch < active_epoch are discarded immediately.
pub fn handle_inbound_audio_payload<FEnqueuePcm, FOnPtt>(
    decrypted_bytes: &[u8],
    active_epoch: &AtomicU64,
    mut enqueue_pcm16: FEnqueuePcm,
    mut on_ptt: FOnPtt,
) -> Result<(), ProtocolError>
where
    FEnqueuePcm: FnMut(&[i16], u32, u64),
    FOnPtt: FnMut(PttMarker),
{
    match decode_audio_payload(decrypted_bytes)? {
        DecodeOutcome::Known(AudioPayload::Frame(frame)) => {
            let min_epoch = active_epoch.load(Ordering::Acquire);
            if frame.epoch < min_epoch {
                return Ok(());
            }
            enqueue_pcm16(&frame.samples, frame.seq, frame.sample_offset);
            Ok(())
        }
        DecodeOutcome::Known(AudioPayload::Control(AudioControlMessage::Flush(flush))) => {
            active_epoch.fetch_max(flush.epoch, Ordering::AcqRel);
            Ok(())
        }
        DecodeOutcome::Known(AudioPayload::Control(AudioControlMessage::PttMarker(ptt))) => {
            let min_epoch = active_epoch.load(Ordering::Acquire);
            if ptt.epoch >= min_epoch {
                on_ptt(ptt);
            }
            Ok(())
        }
        DecodeOutcome::UnknownType { type_name } => {
            log::warn!("[Remote::Audio] Skipping unknown audio control message: {type_name}");
            Ok(())
        }
    }
}

/// Network-backed audio capture receiving 16 kHz Mono PCM16 frames from a connected controller.
///
/// Implements `AudioCapture` so it can be swapped seamlessly into `AudioStream` via `AudioStream::from_capture`.
pub struct NetworkAudioCapture<P> {
    producer: Arc<Mutex<P>>,
    active_epoch: Arc<AtomicU64>,
    ingestion_gate: Arc<AtomicBool>,
    active: AtomicBool,
}

impl<P> NetworkAudioCapture<P>
where
    P: Producer<Item = f32> + Send + 'static,
{
    /// Creates a new `NetworkAudioCapture` feeding into the given ring-buffer producer.
    pub fn new(
        producer: P,
        active_epoch: Arc<AtomicU64>,
        ingestion_gate: Arc<AtomicBool>,
    ) -> Self {
        Self {
            producer: Arc::new(Mutex::new(producer)),
            active_epoch,
            ingestion_gate,
            active: AtomicBool::new(false),
        }
    }

    /// Ingests a raw decrypted audio WebSocket payload, checking epoch and pushing to the ring buffer.
    pub fn ingest_payload(&self, decrypted_bytes: &[u8]) -> Result<(), ProtocolError> {
        if !self.active.load(Ordering::Relaxed) || !self.ingestion_gate.load(Ordering::Relaxed) {
            return Ok(());
        }

        let mut producer = self.producer.lock();
        handle_inbound_audio_payload(
            decrypted_bytes,
            &self.active_epoch,
            |samples, _seq, _offset| {
                for &sample in samples {
                    let float_val = sample as f32 * PCM16_TO_F32_SCALE;
                    let _ = producer.try_push(float_val);
                }
            },
            |_ptt| {},
        )
    }

    /// Ingests a decoded audio payload, checking epoch and pushing to the ring buffer.
    pub fn ingest(&self, payload: AudioPayload) -> Result<(), ProtocolError> {
        if !self.active.load(Ordering::Relaxed) || !self.ingestion_gate.load(Ordering::Relaxed) {
            return Ok(());
        }

        match payload {
            AudioPayload::Frame(frame) => {
                let min_epoch = self.active_epoch.load(Ordering::Acquire);
                if frame.epoch < min_epoch {
                    return Ok(());
                }
                let mut producer = self.producer.lock();
                for &sample in &frame.samples {
                    let float_val = sample as f32 * PCM16_TO_F32_SCALE;
                    let _ = producer.try_push(float_val);
                }
                Ok(())
            }
            AudioPayload::Control(AudioControlMessage::Flush(flush)) => {
                self.active_epoch.fetch_max(flush.epoch, Ordering::AcqRel);
                Ok(())
            }
            AudioPayload::Control(AudioControlMessage::PttMarker(_)) => Ok(()),
        }
    }

    /// Returns a reference to the active epoch counter.
    pub fn active_epoch(&self) -> &Arc<AtomicU64> {
        &self.active_epoch
    }
}

impl<P> NetworkAudioIngestor for NetworkAudioCapture<P>
where
    P: Producer<Item = f32> + Send + 'static,
{
    fn ingest(&self, payload: AudioPayload) -> Result<(), ProtocolError> {
        NetworkAudioCapture::ingest(self, payload)
    }
}

impl<P> AudioCapture for std::sync::Arc<NetworkAudioCapture<P>>
where
    P: Producer<Item = f32> + Send + 'static,
{
    fn start(&self) -> Result<()> {
        self.as_ref().start()
    }
    fn pause(&self) -> Result<()> {
        self.as_ref().pause()
    }
}

impl<P> AudioCapture for NetworkAudioCapture<P>
where
    P: Producer<Item = f32> + Send + 'static,
{
    fn start(&self) -> Result<()> {
        log::info!("[Remote::Audio] Starting network audio capture");
        self.active.store(true, Ordering::Release);
        Ok(())
    }

    fn pause(&self) -> Result<()> {
        log::info!("[Remote::Audio] Pausing network audio capture");
        self.active.store(false, Ordering::Release);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::remote::protocol::{
        framing::{encode_audio_frame, AudioFrame, AUDIO_PAYLOAD_TAG_BINARY, AUDIO_PAYLOAD_TAG_JSON},
        messages::{encode_audio_control, AudioControlMessage, Flush},
        AUDIO_CODEC_PCM16,
    };

    #[test]
    fn epoch_gating_discards_stale_frames() {
        let epoch = AtomicU64::new(5);
        let mut received_samples = Vec::new();

        let stale_frame = AudioFrame {
            codec: AUDIO_CODEC_PCM16,
            seq: 1,
            epoch: 4,
            sample_offset: 0,
            samples: vec![100, 200, 300],
        };
        let mut stale_bytes = vec![AUDIO_PAYLOAD_TAG_BINARY];
        stale_bytes.extend_from_slice(&encode_audio_frame(&stale_frame));

        let res = handle_inbound_audio_payload(
            &stale_bytes,
            &epoch,
            |samples, _, _| received_samples.extend_from_slice(samples),
            |_| {},
        );
        assert!(res.is_ok());
        assert!(received_samples.is_empty(), "Stale frame must be discarded");

        let fresh_frame = AudioFrame {
            codec: AUDIO_CODEC_PCM16,
            seq: 2,
            epoch: 5,
            sample_offset: 3,
            samples: vec![400, 500],
        };
        let mut fresh_bytes = vec![AUDIO_PAYLOAD_TAG_BINARY];
        fresh_bytes.extend_from_slice(&encode_audio_frame(&fresh_frame));

        let res = handle_inbound_audio_payload(
            &fresh_bytes,
            &epoch,
            |samples, _, _| received_samples.extend_from_slice(samples),
            |_| {},
        );
        assert!(res.is_ok());
        assert_eq!(received_samples, vec![400, 500]);
    }

    #[test]
    fn flush_advances_active_epoch() {
        let epoch = AtomicU64::new(1);
        let flush_msg = AudioControlMessage::Flush(Flush { epoch: 10 });
        let mut bytes = vec![AUDIO_PAYLOAD_TAG_JSON];
        bytes.extend_from_slice(&encode_audio_control(&flush_msg).unwrap());

        let res = handle_inbound_audio_payload(&bytes, &epoch, |_, _, _| {}, |_| {});
        assert!(res.is_ok());
        assert_eq!(epoch.load(Ordering::Acquire), 10);
    }
}
