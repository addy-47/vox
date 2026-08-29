//! ============================================================================
//! benches/common/audio.rs — Audio File Loading, Resampling & Streaming Buffers
//! ============================================================================

use std::path::{Path, PathBuf};

/// Resamples an f32 audio slice from `source_rate` to 16,000 Hz using linear interpolation.
pub fn resample_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if source_rate == 16000 || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = 16000.0 / source_rate as f64;
    let target_len = (samples.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(target_len);
    for i in 0..target_len {
        let src_pos = i as f64 / ratio;
        let idx0 = src_pos.floor() as usize;
        let frac = (src_pos - idx0 as f64) as f32;
        let s0 = samples[idx0.min(samples.len() - 1)];
        let s1 = samples[(idx0 + 1).min(samples.len() - 1)];
        out.push(s0 + frac * (s1 - s0));
    }
    out
}

/// Decodes a WAV file to mono 16kHz f32 PCM samples, returning samples and duration in seconds.
pub fn load_wav(path: &Path) -> Result<(Vec<f32>, f32), String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV at {:?}: {}", path, e))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => {
            let max_val = (2u64.pow(spec.bits_per_sample as u32) / 2 - 1) as f64;
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| (s as f64 / max_val) as f32)
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
    let resampled = resample_to_16k(&mono, spec.sample_rate);
    let duration_sec = resampled.len() as f32 / 16000.0;
    Ok((resampled, duration_sec))
}

/// Resolves audio clip candidate paths across multiple relative locations.
pub fn resolve_clip_path(input: &str, default_dir: Option<&Path>) -> Result<PathBuf, String> {
    let direct_path = PathBuf::from(input);
    if direct_path.exists() {
        return Ok(direct_path);
    }

    if let Some(dir) = default_dir {
        let joined = dir.join(input);
        if joined.exists() {
            return Ok(joined);
        }
    }

    let candidates = [
        PathBuf::from(input),
        PathBuf::from("test-clips").join(input),
        PathBuf::from("app/src-tauri/test-clips").join(input),
        PathBuf::from("../test-clips").join(input),
        PathBuf::from("../../test-clips").join(input),
        PathBuf::from("tests/assets").join(input),
        PathBuf::from("app/src-tauri/tests/assets").join(input),
    ];

    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }

    Err(format!(
        "Could not locate audio file '{}'. Searched candidates: {:?}",
        input, candidates
    ))
}
