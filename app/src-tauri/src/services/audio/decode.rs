//! Audio decoding utilities — convert any supported format to 24 kHz mono f32 WAV.

use std::fs::File;
use std::path::Path;

use super::{
    CLONE_SAMPLE_RATE, PCM_I16_SCALE, PCM_S16_SCALE, PCM_S32_SCALE, PCM_U16_SCALE, PCM_U8_SCALE,
};
use symphonia_core::audio::Audio;
use symphonia_core::audio::GenericAudioBufferRef;
use symphonia_core::codecs::audio::AudioDecoder;
use symphonia_core::codecs::audio::AudioDecoderOptions;
use symphonia_core::codecs::CodecParameters;
use symphonia_core::errors::Error;
use symphonia_core::formats::probe::Hint;
use symphonia_core::formats::FormatOptions;
use symphonia_core::formats::FormatReader;
use symphonia_core::io::MediaSourceStream;
use symphonia_core::meta::MetadataOptions;

/// Result type for decode operations.
pub type DecodeResult<T> = Result<T, String>;

/// Decoded audio data: 24 kHz mono f32 PCM.
#[derive(Clone, Debug)]
pub struct DecodedAudio {
    /// PCM samples, 24 kHz mono f32, normalized [-1.0, 1.0].
    pub samples: Vec<f32>,
    /// Sample rate (always 24000).
    pub sample_rate: u32,
    /// Duration in seconds.
    pub duration_secs: f32,
}

/// Decode raw in-memory audio bytes to 24 kHz mono f32 PCM given a format hint.
pub fn decode_bytes_to_24khz_mono(bytes: &[u8], format_hint: &str) -> DecodeResult<DecodedAudio> {
    let cursor = std::io::Cursor::new(bytes);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    hint.with_extension(format_hint);

    decode_mss(mss, hint)
}

/// Decode any supported audio file on disk to 24 kHz mono f32 PCM.
pub fn decode_to_24khz_mono<P: AsRef<Path>>(path: P) -> DecodeResult<DecodedAudio> {
    let path = path.as_ref();
    let file =
        File::open(path).map_err(|e| format!("Failed to open file '{}': {}", path.display(), e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|os| os.to_str()) {
        hint.with_extension(ext);
    }

    decode_mss(mss, hint)
}

/// Decode an audio media stream into 24 kHz mono f32 PCM using format probing.
fn decode_mss(mss: MediaSourceStream, hint: Hint) -> DecodeResult<DecodedAudio> {
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| format!("Unsupported or corrupt audio format: {}", e))?;

    let track = format
        .tracks()
        .first()
        .ok_or_else(|| "No audio track found in file".to_string())?;

    let codec_params = match &track.codec_params {
        Some(CodecParameters::Audio(params)) => params,
        _ => return Err("Track is not an audio track".to_string()),
    };

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|e| format!("Failed to initialize audio decoder: {}", e))?;

    let track_id = track.id;
    let input_sample_rate = codec_params.sample_rate.unwrap_or(CLONE_SAMPLE_RATE);
    let raw_samples = decode_packets(format.as_mut(), decoder.as_mut(), track_id)?;

    if raw_samples.is_empty() {
        return Err("Decoded audio contains no samples".to_string());
    }

    let resampled = if input_sample_rate != CLONE_SAMPLE_RATE {
        resample_linear(&raw_samples, input_sample_rate, CLONE_SAMPLE_RATE)
    } else {
        raw_samples
    };

    let duration_secs = resampled.len() as f32 / CLONE_SAMPLE_RATE as f32;

    Ok(DecodedAudio {
        samples: resampled,
        sample_rate: CLONE_SAMPLE_RATE,
        duration_secs,
    })
}

/// Decode format packets for the selected track into raw mono f32 samples.
fn decode_packets(
    format: &mut dyn FormatReader,
    decoder: &mut dyn AudioDecoder,
    track_id: u32,
) -> DecodeResult<Vec<f32>> {
    let mut raw_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(Error::IoError(ref err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(e) => return Err(format!("Decoding error: {}", e)),
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(Error::DecodeError(err)) => {
                log::warn!(
                    "[Audio::Decode] skipping packet with decode error: {}",
                    err
                );
                continue;
            }
            Err(e) => return Err(format!("Decoder error: {}", e)),
        };

        append_samples_as_f32_mono(&decoded, &mut raw_samples)?;
    }

    Ok(raw_samples)
}

/// Convert decoded audio buffer frames across supported sample types into mono f32 samples.
fn append_samples_as_f32_mono(
    decoded: &GenericAudioBufferRef<'_>,
    raw_samples: &mut Vec<f32>,
) -> DecodeResult<()> {
    match decoded {
        GenericAudioBufferRef::F32(buf) => {
            let channels = buf.spec().channels().count();
            let frames = buf.frames();
            raw_samples.reserve(frames);
            for f in 0..frames {
                let mut sum = 0.0f32;
                for c in 0..channels {
                    sum += buf[c][f];
                }
                raw_samples.push(sum / channels as f32);
            }
        }
        GenericAudioBufferRef::U8(buf) => {
            let channels = buf.spec().channels().count();
            let frames = buf.frames();
            raw_samples.reserve(frames);
            for f in 0..frames {
                let mut sum = 0.0f32;
                for c in 0..channels {
                    sum += (buf[c][f] as f32 / PCM_U8_SCALE) - 1.0;
                }
                raw_samples.push(sum / channels as f32);
            }
        }
        GenericAudioBufferRef::U16(buf) => {
            let channels = buf.spec().channels().count();
            let frames = buf.frames();
            raw_samples.reserve(frames);
            for f in 0..frames {
                let mut sum = 0.0f32;
                for c in 0..channels {
                    sum += (buf[c][f] as f32 / PCM_U16_SCALE) - 1.0;
                }
                raw_samples.push(sum / channels as f32);
            }
        }
        GenericAudioBufferRef::S16(buf) => {
            let channels = buf.spec().channels().count();
            let frames = buf.frames();
            raw_samples.reserve(frames);
            for f in 0..frames {
                let mut sum = 0.0f32;
                for c in 0..channels {
                    sum += buf[c][f] as f32 / PCM_S16_SCALE;
                }
                raw_samples.push(sum / channels as f32);
            }
        }
        GenericAudioBufferRef::S32(buf) => {
            let channels = buf.spec().channels().count();
            let frames = buf.frames();
            raw_samples.reserve(frames);
            for f in 0..frames {
                let mut sum = 0.0f32;
                for c in 0..channels {
                    sum += buf[c][f] as f32 / PCM_S32_SCALE;
                }
                raw_samples.push(sum / channels as f32);
            }
        }
        _ => return Err("Unsupported sample buffer type (24-bit or other)".to_string()),
    }
    Ok(())
}

/// Resample an f32 audio slice from one sample rate to another using linear interpolation.
fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let num_out = (input.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(num_out);
    let max_idx = input.len().saturating_sub(1);

    for i in 0..num_out {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(max_idx);
        let frac = src_idx - idx_floor as f64;
        let sample = (1.0 - frac) as f32 * input[idx_floor] + frac as f32 * input[idx_ceil];
        out.push(sample);
    }

    out
}

/// Truncate decoded audio samples to a maximum duration in seconds.
pub fn truncate_to(audio: DecodedAudio, max_secs: f32) -> DecodedAudio {
    let max_samples = (max_secs * CLONE_SAMPLE_RATE as f32) as usize;
    if audio.samples.len() > max_samples {
        DecodedAudio {
            samples: audio.samples[..max_samples].to_vec(),
            duration_secs: max_secs,
            ..audio
        }
    } else {
        audio
    }
}

/// Write f32 PCM samples as a 16-bit integer mono WAV file at the target sample rate.
pub fn write_wav_f32<P: AsRef<Path>>(
    path: P,
    samples: &[f32],
    sample_rate: u32,
) -> DecodeResult<()> {
    use hound::WavSpec;

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path.as_ref(), spec)
        .map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        writer
            .write_sample((clamped * PCM_I16_SCALE) as i16)
            .map_err(|e| format!("Failed to write WAV sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(())
}

/// Write f32 PCM samples as a 32-bit floating point mono WAV file preserving full precision.
pub fn write_wav_f32_raw<P: AsRef<Path>>(
    path: P,
    samples: &[f32],
    sample_rate: u32,
) -> DecodeResult<()> {
    use hound::WavSpec;

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path.as_ref(), spec)
        .map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    for &sample in samples {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write WAV sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(())
}
