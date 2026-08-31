//! ============================================================================
//! tests/common/audio.rs — Shared Audio Helpers for Integration Tests
//! ============================================================================

use ringbuf::traits::{Observer, Producer};
use std::path::Path;
use std::time::Duration;
use vox_lib::services::vad::VAD_CHUNK_SIZE;

/// Decodes a WAV file into 16kHz mono f32 PCM samples normalized to [-1.0, 1.0].
pub fn decode_wav_to_mono_16k(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV at {:?}: {}", path, e))?;
    let spec = reader.spec();

    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, _) => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        (hound::SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / 32768.0)
            .collect(),
        (hound::SampleFormat::Int, 24) => reader
            .samples::<i32>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / 8388608.0)
            .collect(),
        (hound::SampleFormat::Int, 32) => reader
            .samples::<i32>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / 2147483648.0)
            .collect(),
        (hound::SampleFormat::Int, bits) => {
            let max_val = (2u64.pow(bits as u32) / 2 - 1) as f32;
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
    };

    let mono: Vec<f32> = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };

    if spec.sample_rate == 16000 || mono.is_empty() {
        Ok(mono)
    } else {
        let ratio = 16000.0 / spec.sample_rate as f64;
        let target_len = (mono.len() as f64 * ratio).round() as usize;
        let mut out = Vec::with_capacity(target_len);
        for i in 0..target_len {
            let src_pos = i as f64 / ratio;
            let idx0 = src_pos.floor() as usize;
            let frac = (src_pos - idx0 as f64) as f32;
            let s0 = mono[idx0.min(mono.len() - 1)];
            let s1 = mono[(idx0 + 1).min(mono.len() - 1)];
            out.push(s0 + frac * (s1 - s0));
        }
        Ok(out)
    }
}

/// Decodes a WAV file to mono i16 PCM samples (for realtime PTT engines).
pub fn decode_wav_to_i16(path: &Path) -> Result<Vec<i16>, String> {
    let f32_samples = decode_wav_to_mono_16k(path)?;
    Ok(f32_samples
        .into_iter()
        .map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect())
}

/// Streams f32 audio samples to a ring buffer producer in VAD_CHUNK_SIZE chunks.
pub fn stream_audio_to_ring_buffer(audio: &[f32], producer: &mut impl Producer<Item = f32>) {
    for chunk in audio.chunks(VAD_CHUNK_SIZE) {
        if chunk.len() == VAD_CHUNK_SIZE {
            while producer.vacant_len() < chunk.len() {
                std::thread::sleep(Duration::from_millis(1));
            }
            producer.push_slice(chunk);
        } else {
            let mut padded = chunk.to_vec();
            padded.resize(VAD_CHUNK_SIZE, 0.0);
            while producer.vacant_len() < padded.len() {
                std::thread::sleep(Duration::from_millis(1));
            }
            producer.push_slice(&padded);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Injects N frames of zero-valued silence into the ring buffer.
pub fn stream_silence_frames(producer: &mut impl Producer<Item = f32>, n_frames: usize) {
    let silence = vec![0.0f32; VAD_CHUNK_SIZE];
    for _ in 0..n_frames {
        while producer.vacant_len() < silence.len() {
            std::thread::sleep(Duration::from_millis(1));
        }
        producer.push_slice(&silence);
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Spin-waits until the ring buffer producer reports occupied_len == 0 or timeout expires.
pub fn wait_for_buffer_drain(producer: &impl Observer, timeout_secs: u64) {
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    while producer.occupied_len() > 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}
