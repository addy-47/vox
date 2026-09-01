use ringbuf::traits::Consumer;
use std::path::Path;

/// Persist mono f32 samples as 32-bit float WAV.
pub fn write_wav_f32(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create WAV dir {:?}: {}", parent, e))?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV {:?}: {}", path, e))?;
    for s in samples {
        writer
            .write_sample(*s)
            .map_err(|e| format!("WAV write failed: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("WAV finalize failed: {}", e))?;
    Ok(())
}

/// Drain all available samples from a HeapCons without blocking.
pub fn drain_consumer(consumer: &mut ringbuf::HeapCons<f32>) -> Vec<f32> {
    let mut out = Vec::new();
    while let Some(s) = consumer.try_pop() {
        out.push(s);
    }
    out
}

/// Downsample 48 kHz playback buffer back to 24 kHz by decimation (even indices).
pub fn downsample_48k_to_24k(input_48k: &[f32]) -> Vec<f32> {
    input_48k.iter().step_by(2).copied().collect()
}
