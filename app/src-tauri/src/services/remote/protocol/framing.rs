use serde::{Deserialize, Serialize};

use super::{
    errors::ProtocolError,
    messages::{AudioControlMessage, DecodeOutcome},
};

/// Rejects a control message that exceeds the cap for its lifecycle phase.
///
/// Remote-control spec §7.1: the cap is 4 KB before the Noise handshake
/// completes and the transport's own payload limit afterwards. The two caps
/// are named constants; the caller passes the one that applies to the
/// message it is checking.
pub fn check_control_message_size(size: usize, limit: usize) -> Result<(), ProtocolError> {
    if size > limit {
        return Err(ProtocolError::Oversized { size, limit });
    }
    Ok(())
}

/// The codec identifier for PCM16 audio. Remote-control spec §7.4.
pub const AUDIO_CODEC_PCM16: u8 = 0x00;

/// Bytes taken by the codec field of an audio frame header.
const CODEC_BYTES: usize = 1;

/// Bytes taken by the sequence field of an audio frame header.
const SEQUENCE_BYTES: usize = 4;

/// Bytes taken by the epoch field of an audio frame header.
const EPOCH_BYTES: usize = 8;

/// Bytes taken by the sample offset field of an audio frame header.
const SAMPLE_OFFSET_BYTES: usize = 8;

/// Bytes taken by the header of an audio frame. Remote-control spec §7.4.
pub const AUDIO_FRAME_HEADER_BYTES: usize =
    CODEC_BYTES + SEQUENCE_BYTES + EPOCH_BYTES + SAMPLE_OFFSET_BYTES;

/// Bytes taken by one PCM16 sample.
pub const PCM16_SAMPLE_BYTES: usize = 2;

/// The audio-socket payload tag of a binary audio frame.
///
/// Remote-control spec §7.4: the WebSocket text/binary distinction does not
/// survive Noise encryption, so every inner payload on the audio socket
/// starts with a one-byte tag.
pub const AUDIO_PAYLOAD_TAG_BINARY: u8 = 0x00;

/// The audio-socket payload tag of a JSON audio-control message.
pub const AUDIO_PAYLOAD_TAG_JSON: u8 = 0x01;

/// The declared audio format of one direction of the link.
///
/// Remote-control spec §7.4: the format of each direction is declared in the
/// `Hello` message, and a receiver MUST reject a mismatched format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamFormat {
    /// Samples per second.
    pub sample_rate: u32,
    /// Channel count. The v1 protocol is mono in both directions.
    pub channels: u16,
    /// Samples in one frame.
    pub frame_size_samples: u32,
}

impl StreamFormat {
    /// The uplink format: 16 kHz mono, 20 ms of 320 samples.
    pub const UPLINK: StreamFormat = StreamFormat {
        sample_rate: 16_000,
        channels: 1,
        frame_size_samples: 320,
    };

    /// The downlink format: 24 kHz mono, 20 ms of 480 samples.
    pub const DOWNLINK: StreamFormat = StreamFormat {
        sample_rate: 24_000,
        channels: 1,
        frame_size_samples: 480,
    };
}

/// Rejects a stream format that does not match the one the receiver supports.
///
/// Remote-control spec §7.4: the receiver MUST reject a mismatched format.
pub fn check_stream_format(
    expected: StreamFormat,
    actual: StreamFormat,
) -> Result<(), ProtocolError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ProtocolError::FormatMismatch { expected, actual })
    }
}

/// One PCM16 audio frame. Remote-control spec §7.4.
///
/// The binary layout is fixed by the spec:
///
/// ```text
/// [u8: codec] [u32_be: seq] [u64_be: epoch] [u64_be: sample_offset] [i16_le samples]
/// ```
///
/// The header is big-endian; the samples are little-endian. A swap is a
/// silent corruption, not a crash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    /// The codec of the sample body. Only `AUDIO_CODEC_PCM16` is defined.
    pub codec: u8,
    /// The monotonic frame counter. Gaps indicate loss.
    pub seq: u32,
    /// The monotonic epoch counter. Increments on every `Flush`.
    pub epoch: u64,
    /// The absolute sample position from session start.
    pub sample_offset: u64,
    /// The PCM16 samples.
    pub samples: Vec<i16>,
}

/// Encodes an audio frame into its wire layout.
pub fn encode_audio_frame(frame: &AudioFrame) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(AUDIO_FRAME_HEADER_BYTES + frame.samples.len() * PCM16_SAMPLE_BYTES);
    out.push(frame.codec);
    out.extend_from_slice(&frame.seq.to_be_bytes());
    out.extend_from_slice(&frame.epoch.to_be_bytes());
    out.extend_from_slice(&frame.sample_offset.to_be_bytes());
    for sample in &frame.samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Decodes an audio frame from its wire layout.
///
/// A body shorter than the header, or a sample body that is not a whole
/// number of PCM16 samples, is rejected.
pub fn decode_audio_frame(bytes: &[u8]) -> Result<AudioFrame, ProtocolError> {
    if bytes.len() < AUDIO_FRAME_HEADER_BYTES {
        return Err(ProtocolError::TruncatedFrame {
            expected: AUDIO_FRAME_HEADER_BYTES,
            actual: bytes.len(),
        });
    }

    let codec = bytes[0];
    if codec != AUDIO_CODEC_PCM16 {
        return Err(ProtocolError::UnknownCodec(codec));
    }

    let sample_bytes = &bytes[AUDIO_FRAME_HEADER_BYTES..];
    if !sample_bytes.len().is_multiple_of(PCM16_SAMPLE_BYTES) {
        return Err(ProtocolError::TruncatedFrame {
            expected: bytes.len() - sample_bytes.len() % PCM16_SAMPLE_BYTES,
            actual: bytes.len(),
        });
    }

    Ok(AudioFrame {
        codec,
        seq: read_u32_be(bytes, CODEC_BYTES),
        epoch: read_u64_be(bytes, CODEC_BYTES + SEQUENCE_BYTES),
        sample_offset: read_u64_be(bytes, CODEC_BYTES + SEQUENCE_BYTES + EPOCH_BYTES),
        samples: sample_bytes
            .chunks_exact(PCM16_SAMPLE_BYTES)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect(),
    })
}

/// Reads a big-endian `u32` at `offset`.
///
/// The caller has already checked that the frame carries a whole header.
fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    let array: [u8; SEQUENCE_BYTES] = bytes[offset..offset + SEQUENCE_BYTES]
        .try_into()
        .expect("the header length was checked by the caller");
    u32::from_be_bytes(array)
}

/// Reads a big-endian `u64` at `offset`.
///
/// The caller has already checked that the frame carries a whole header.
fn read_u64_be(bytes: &[u8], offset: usize) -> u64 {
    let array: [u8; EPOCH_BYTES] = bytes[offset..offset + EPOCH_BYTES]
        .try_into()
        .expect("the header length was checked by the caller");
    u64::from_be_bytes(array)
}

/// One payload on the audio socket.
///
/// Remote-control spec §7.4: the audio socket carries two payload shapes, a
/// binary audio frame and a JSON audio-control message. The one-byte payload
/// tag is what tells them apart.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioPayload {
    /// A binary PCM16 audio frame.
    Frame(AudioFrame),
    /// A JSON audio-control message (`PttMarker` or `Flush`).
    Control(AudioControlMessage),
}

/// Encodes an audio payload, including its one-byte payload tag.
pub fn encode_audio_payload(payload: &AudioPayload) -> Result<Vec<u8>, ProtocolError> {
    let mut out = Vec::new();
    match payload {
        AudioPayload::Frame(frame) => {
            out.push(AUDIO_PAYLOAD_TAG_BINARY);
            out.extend_from_slice(&encode_audio_frame(frame));
        }
        AudioPayload::Control(message) => {
            out.push(AUDIO_PAYLOAD_TAG_JSON);
            out.extend_from_slice(&super::messages::encode_audio_control(message)?);
        }
    }
    Ok(out)
}

/// Decodes an audio payload, including its one-byte payload tag.
///
/// An unknown tag is a protocol error: a mis-tagged frame cannot be safely
/// interpreted as either shape.
pub fn decode_audio_payload(bytes: &[u8]) -> Result<DecodeOutcome<AudioPayload>, ProtocolError> {
    let Some((&tag, body)) = bytes.split_first() else {
        return Err(ProtocolError::MissingPayloadTag);
    };
    match tag {
        AUDIO_PAYLOAD_TAG_BINARY => Ok(DecodeOutcome::Known(AudioPayload::Frame(
            decode_audio_frame(body)?,
        ))),
        AUDIO_PAYLOAD_TAG_JSON => match super::messages::decode_audio_control(body)? {
            DecodeOutcome::Known(message) => {
                Ok(DecodeOutcome::Known(AudioPayload::Control(message)))
            }
            DecodeOutcome::UnknownType { type_name } => {
                Ok(DecodeOutcome::UnknownType { type_name })
            }
        },
        other => Err(ProtocolError::UnknownPayloadTag(other)),
    }
}
