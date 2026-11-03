use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    mpsc::Sender,
    Arc,
};

use anyhow::Result;

use crate::{
    core::{events::VoxEvent, state::InteractionOwner},
    services::{
        audio::{AudioCapture, SharedRingProducer},
        remote::protocol::{
            errors::ProtocolError,
            framing::{decode_audio_payload, AudioPayload},
            messages::{
                AudioControlMessage, DecodeOutcome, PttAction, PttMarker, SpeechEnd, SpeechStart,
            },
        },
        stt::actor::SttCommand,
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
/// Spec §7.4 & §7.6 & VERIFY-RC-12: Every payload on the audio socket is tagged (0x00 binary AudioFrame,
/// 0x01 JSON AudioControlMessage). Frames with epoch < active_epoch are discarded immediately.
pub fn handle_inbound_audio_payload<FEnqueuePcm, FOnPtt, FOnSpeechStart, FOnSpeechEnd>(
    decrypted_bytes: &[u8],
    active_epoch: &AtomicU64,
    mut enqueue_pcm16: FEnqueuePcm,
    mut on_ptt: FOnPtt,
    mut on_speech_start: FOnSpeechStart,
    mut on_speech_end: FOnSpeechEnd,
) -> Result<(), ProtocolError>
where
    FEnqueuePcm: FnMut(&[i16], u32, u64),
    FOnPtt: FnMut(PttMarker),
    FOnSpeechStart: FnMut(SpeechStart),
    FOnSpeechEnd: FnMut(SpeechEnd),
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
        DecodeOutcome::Known(AudioPayload::Control(AudioControlMessage::SpeechStart(start))) => {
            let min_epoch = active_epoch.load(Ordering::Acquire);
            if start.epoch >= min_epoch {
                on_speech_start(start);
            }
            Ok(())
        }
        DecodeOutcome::Known(AudioPayload::Control(AudioControlMessage::SpeechEnd(end))) => {
            let min_epoch = active_epoch.load(Ordering::Acquire);
            if end.epoch >= min_epoch {
                on_speech_end(end);
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
pub struct NetworkAudioCapture {
    producer: SharedRingProducer,
    active_epoch: Arc<AtomicU64>,
    ingestion_gate: Arc<AtomicBool>,
    active: AtomicBool,
    event_tx: Option<Sender<VoxEvent>>,
    stt_tx: Option<Sender<SttCommand>>,
    turn_id_atomic: Option<Arc<AtomicU32>>,
    utterance_buffer: parking_lot::Mutex<Vec<f32>>,
}

impl NetworkAudioCapture {
    /// Creates a new `NetworkAudioCapture` feeding into the given ring-buffer producer.
    pub fn new(
        producer: SharedRingProducer,
        active_epoch: Arc<AtomicU64>,
        ingestion_gate: Arc<AtomicBool>,
    ) -> Self {
        Self {
            producer,
            active_epoch,
            ingestion_gate,
            active: AtomicBool::new(true),
            event_tx: None,
            stt_tx: None,
            turn_id_atomic: None,
            utterance_buffer: parking_lot::Mutex::new(Vec::new()),
        }
    }

    /// Attaches an event channel to dispatch PttMarker and SpeechStart/SpeechEnd events.
    pub fn with_event_tx(mut self, event_tx: Sender<VoxEvent>) -> Self {
        self.event_tx = Some(event_tx);
        self
    }

    /// Attaches direct STT channel and turn ID atomic for controller-side VAD mode (spec §7.6).
    pub fn with_stt(mut self, stt_tx: Sender<SttCommand>, turn_id_atomic: Arc<AtomicU32>) -> Self {
        self.stt_tx = Some(stt_tx);
        self.turn_id_atomic = Some(turn_id_atomic);
        self
    }

    /// Ingests a raw decrypted audio WebSocket payload, checking epoch and pushing to the ring buffer.
    pub fn ingest_payload(&self, decrypted_bytes: &[u8]) -> Result<(), ProtocolError> {
        let is_active = self.active.load(Ordering::Relaxed);
        let gate_open = self.ingestion_gate.load(Ordering::Relaxed);

        handle_inbound_audio_payload(
            decrypted_bytes,
            &self.active_epoch,
            |samples, _seq, _offset| {
                if is_active && gate_open {
                    let mut float_chunk = Vec::with_capacity(samples.len());
                    for &sample in samples {
                        let float_val = sample as f32 * PCM16_TO_F32_SCALE;
                        float_chunk.push(float_val);
                        let _ = self.producer.try_push(float_val);
                    }
                    if let Some(ref stt) = self.stt_tx {
                        let turn_id = self
                            .turn_id_atomic
                            .as_ref()
                            .map(|t| t.load(Ordering::Acquire))
                            .unwrap_or(0);
                        let _ = stt.send(SttCommand::StreamChunk {
                            turn_id,
                            audio: float_chunk.clone(),
                        });
                    }
                    self.utterance_buffer.lock().extend_from_slice(&float_chunk);
                }
            },
            |ptt| {
                self.dispatch_ptt(ptt);
            },
            |start| {
                self.dispatch_speech_start(start);
            },
            |end| {
                self.dispatch_speech_end(end);
            },
        )
    }

    /// Ingests a decoded audio payload, checking epoch and pushing to the ring buffer.
    pub fn ingest(&self, payload: AudioPayload) -> Result<(), ProtocolError> {
        match payload {
            AudioPayload::Frame(frame) => {
                if !self.active.load(Ordering::Relaxed)
                    || !self.ingestion_gate.load(Ordering::Relaxed)
                {
                    return Ok(());
                }
                let min_epoch = self.active_epoch.load(Ordering::Acquire);
                if frame.epoch < min_epoch {
                    return Ok(());
                }
                let mut float_chunk = Vec::with_capacity(frame.samples.len());
                for &sample in &frame.samples {
                    let float_val = sample as f32 * PCM16_TO_F32_SCALE;
                    float_chunk.push(float_val);
                    let _ = self.producer.try_push(float_val);
                }
                if let Some(ref stt) = self.stt_tx {
                    let turn_id = self
                        .turn_id_atomic
                        .as_ref()
                        .map(|t| t.load(Ordering::Acquire))
                        .unwrap_or(0);
                    let _ = stt.send(SttCommand::StreamChunk {
                        turn_id,
                        audio: float_chunk.clone(),
                    });
                }
                self.utterance_buffer.lock().extend_from_slice(&float_chunk);
                Ok(())
            }
            AudioPayload::Control(AudioControlMessage::Flush(flush)) => {
                self.active_epoch.fetch_max(flush.epoch, Ordering::AcqRel);
                Ok(())
            }
            AudioPayload::Control(AudioControlMessage::PttMarker(ptt)) => {
                let min_epoch = self.active_epoch.load(Ordering::Acquire);
                if ptt.epoch >= min_epoch {
                    self.dispatch_ptt(ptt);
                }
                Ok(())
            }
            AudioPayload::Control(AudioControlMessage::SpeechStart(start)) => {
                let min_epoch = self.active_epoch.load(Ordering::Acquire);
                if start.epoch >= min_epoch {
                    self.dispatch_speech_start(start);
                }
                Ok(())
            }
            AudioPayload::Control(AudioControlMessage::SpeechEnd(end)) => {
                let min_epoch = self.active_epoch.load(Ordering::Acquire);
                if end.epoch >= min_epoch {
                    self.dispatch_speech_end(end);
                }
                Ok(())
            }
        }
    }

    fn dispatch_ptt(&self, ptt: PttMarker) {
        if let Some(ref tx) = self.event_tx {
            let event = match ptt.action {
                PttAction::Start => VoxEvent::PttStart {
                    owner: InteractionOwner::Assistant,
                },
                PttAction::Stop => VoxEvent::PttStop {
                    owner: InteractionOwner::Assistant,
                },
                PttAction::Cancel => VoxEvent::PttCancel {
                    owner: InteractionOwner::Assistant,
                },
            };
            let _ = tx.send(event);
        }
    }

    fn dispatch_speech_start(&self, _start: SpeechStart) {
        log::info!("[Remote::Audio] Controller VAD SpeechStart received");
        self.utterance_buffer.lock().clear();
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(VoxEvent::SpeechStart {
                owner: InteractionOwner::Assistant,
            });
        }
    }

    fn dispatch_speech_end(&self, _end: SpeechEnd) {
        log::info!("[Remote::Audio] Controller VAD SpeechEnd received");
        let utterance = std::mem::take(&mut *self.utterance_buffer.lock());
        if let Some(ref stt) = self.stt_tx {
            let turn_id = self
                .turn_id_atomic
                .as_ref()
                .map(|t| t.load(Ordering::Acquire))
                .unwrap_or(0);
            let _ = stt.send(SttCommand::Final {
                turn_id,
                audio: utterance,
                owner: InteractionOwner::Assistant,
            });
        }
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(VoxEvent::SpeechEnd {
                owner: InteractionOwner::Assistant,
            });
        }
    }

    /// Returns a reference to the active epoch counter.
    pub fn active_epoch(&self) -> &Arc<AtomicU64> {
        &self.active_epoch
    }
}

impl NetworkAudioIngestor for NetworkAudioCapture {
    fn ingest(&self, payload: AudioPayload) -> Result<(), ProtocolError> {
        self.ingest(payload)
    }
}

impl AudioCapture for std::sync::Arc<NetworkAudioCapture> {
    fn start(&self) -> Result<()> {
        self.as_ref().start()
    }
    fn pause(&self) -> Result<()> {
        self.as_ref().pause()
    }
}

impl AudioCapture for NetworkAudioCapture {
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
        framing::{
            encode_audio_frame, AudioFrame, AUDIO_PAYLOAD_TAG_BINARY, AUDIO_PAYLOAD_TAG_JSON,
        },
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
            |_| {},
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
            |_| {},
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

        let res =
            handle_inbound_audio_payload(&bytes, &epoch, |_, _, _| {}, |_| {}, |_| {}, |_| {});
        assert!(res.is_ok());
        assert_eq!(epoch.load(Ordering::Acquire), 10);
    }

    #[test]
    fn speech_start_and_end_control_messages_dispatched() {
        let epoch = AtomicU64::new(2);
        let mut speech_started = false;
        let mut speech_ended = false;

        let start_msg = AudioControlMessage::SpeechStart(SpeechStart {
            epoch: 2,
            sample_offset: 640,
        });
        let mut start_bytes = vec![AUDIO_PAYLOAD_TAG_JSON];
        start_bytes.extend_from_slice(&encode_audio_control(&start_msg).unwrap());

        let res = handle_inbound_audio_payload(
            &start_bytes,
            &epoch,
            |_, _, _| {},
            |_| {},
            |_| {
                speech_started = true;
            },
            |_| {},
        );
        assert!(res.is_ok());
        assert!(speech_started);

        let end_msg = AudioControlMessage::SpeechEnd(SpeechEnd {
            epoch: 2,
            sample_count: 3200,
        });
        let mut end_bytes = vec![AUDIO_PAYLOAD_TAG_JSON];
        end_bytes.extend_from_slice(&encode_audio_control(&end_msg).unwrap());

        let res = handle_inbound_audio_payload(
            &end_bytes,
            &epoch,
            |_, _, _| {},
            |_| {},
            |_| {},
            |_| {
                speech_ended = true;
            },
        );
        assert!(res.is_ok());
        assert!(speech_ended);
    }
}
