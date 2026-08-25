//! Voice service for audio validation, decoding, resampling, speaker pre-baking, and recording.

use crate::services::tts::providers::TtsProvider;
use crate::symphonia_core::audio::Audio;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

// ─── Edge TTS Voice DTO ───────────────────────────────────────────────────────

/// Metadata describing an online Edge TTS neural voice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeTtsVoiceEntry {
    pub name: String,
    pub short_name: String,
    pub gender: String,
    pub locale: String,
    pub friendly_name: String,
}

// ─── WAV File Validation ──────────────────────────────────────────────────────

/// Validate that a WAV file is readable and satisfies a minimum duration.
pub fn validate_wav_file(path: &str, min_duration_secs: f32) -> Result<(u32, f32), String> {
    let reader =
        hound::WavReader::open(path).map_err(|e| format!("Cannot read WAV file: {}", e))?;
    let spec = reader.spec();
    let num_samples = reader.len();
    if spec.sample_rate == 0 {
        return Err("WAV file has invalid sample rate (0)".to_string());
    }
    let duration = num_samples as f32 / spec.sample_rate as f32;
    if duration < min_duration_secs {
        return Err(format!(
            "Audio too short ({:.1}s). Minimum is {}s for voice cloning.",
            duration, min_duration_secs
        ));
    }
    Ok((spec.sample_rate, duration))
}

// ─── Multi-Format Audio Decoding & Resampling ─────────────────────────────────

fn extract_mono_f32_samples(
    buf_ref: crate::symphonia_core::audio::GenericAudioBufferRef<'_>,
    raw_samples: &mut Vec<f32>,
) {
    use crate::symphonia_core::audio::GenericAudioBufferRef;
    match buf_ref {
        GenericAudioBufferRef::F32(buf) => {
            let channels = buf.spec().channels().count();
            for f in 0..buf.frames() {
                let sum: f32 = (0..channels).map(|c| buf[c][f]).sum();
                raw_samples.push(sum / channels as f32);
            }
        }
        GenericAudioBufferRef::U8(buf) => {
            let channels = buf.spec().channels().count();
            for f in 0..buf.frames() {
                let sum: f32 = (0..channels).map(|c| buf[c][f] as f32 / 128.0 - 1.0).sum();
                raw_samples.push(sum / channels as f32);
            }
        }
        GenericAudioBufferRef::U16(buf) => {
            let channels = buf.spec().channels().count();
            for f in 0..buf.frames() {
                let sum: f32 = (0..channels)
                    .map(|c| buf[c][f] as f32 / 32768.0 - 1.0)
                    .sum();
                raw_samples.push(sum / channels as f32);
            }
        }
        GenericAudioBufferRef::S16(buf) => {
            let channels = buf.spec().channels().count();
            for f in 0..buf.frames() {
                let sum: f32 = (0..channels).map(|c| buf[c][f] as f32 / 32768.0).sum();
                raw_samples.push(sum / channels as f32);
            }
        }
        GenericAudioBufferRef::S32(buf) => {
            let channels = buf.spec().channels().count();
            for f in 0..buf.frames() {
                let sum: f32 = (0..channels).map(|c| buf[c][f] as f32 / 2147483648.0).sum();
                raw_samples.push(sum / channels as f32);
            }
        }
        _ => log::warn!("[Voice] Skipping unsupported sample buffer format"),
    }
}

fn decode_audio_stream(src_path: &str) -> Result<(Vec<f32>, u32), String> {
    use crate::symphonia_core::codecs::audio::AudioDecoderOptions;
    use crate::symphonia_core::errors::Error;
    use crate::symphonia_core::formats::probe::Hint;
    use crate::symphonia_core::formats::FormatOptions;
    use crate::symphonia_core::io::MediaSourceStream;
    use crate::symphonia_core::meta::MetadataOptions;

    let file = File::open(src_path).map_err(|e| format!("Failed to open audio file: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(src_path).extension().and_then(|os| os.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| format!("Unsupported audio format: {}", e))?;

    let track = format
        .tracks()
        .first()
        .ok_or_else(|| "No audio track found in file".to_string())?;

    let codec_params = match &track.codec_params {
        Some(crate::symphonia_core::codecs::CodecParameters::Audio(params)) => params,
        _ => return Err("Track is not an audio track or lacks codec parameters".to_string()),
    };

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|e| format!("Failed to initialize decoder: {}", e))?;

    let track_id = track.id;
    let input_sample_rate = codec_params.sample_rate.unwrap_or(24000);
    let mut raw_samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(Error::IoError(ref err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            Err(e) => return Err(format!("Audio decoding error: {}", e)),
        };

        if packet.track_id != track_id {
            continue;
        }

        if let Ok(decoded) = decoder.decode(&packet) {
            extract_mono_f32_samples(decoded, &mut raw_samples);
        }
    }

    if raw_samples.is_empty() {
        return Err("Decoded audio stream contains no samples".to_string());
    }

    Ok((raw_samples, input_sample_rate))
}

fn resample_linear(samples: &[f32], src_rate: u32, target_rate: u32) -> Vec<f32> {
    if src_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = src_rate as f64 / target_rate as f64;
    let target_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(target_len);

    for i in 0..target_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(samples.len().saturating_sub(1));
        let frac = (src_idx - idx_floor as f64) as f32;
        let sample = (1.0 - frac) * samples[idx_floor] + frac * samples[idx_ceil];
        out.push(sample);
    }
    out
}

fn pad_or_truncate_audio(samples: &[f32], sample_rate: u32, target_duration_secs: f32) -> Vec<f32> {
    let target_len = (target_duration_secs * sample_rate as f32) as usize;
    if samples.len() < target_len {
        samples.iter().copied().cycle().take(target_len).collect()
    } else if samples.len() > target_len {
        samples[..target_len].to_vec()
    } else {
        samples.to_vec()
    }
}

/// Write mono 32-bit float samples to a WAV file at the specified sample rate.
pub fn write_f32_wav(dest_path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(dest_path, spec)
        .map_err(|e| format!("Failed to create destination WAV writer: {}", e))?;
    for sample in samples {
        writer
            .write_sample(*sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))
}

/// Decode any supported audio file to mono 24kHz f32 WAV with 30s auto-stitching/truncation.
pub fn convert_and_validate_audio(src_path: &str, dest_path: &Path) -> Result<(), String> {
    let (raw_samples, src_rate) = decode_audio_stream(src_path)?;
    let resampled = resample_linear(&raw_samples, src_rate, 24000);

    let duration_secs = resampled.len() as f32 / 24000.0;
    if duration_secs < 1.0 {
        return Err(format!(
            "Audio too short ({:.1}s). Minimum is 1.0s for voice cloning.",
            duration_secs
        ));
    }

    let final_samples = pad_or_truncate_audio(&resampled, 24000, 30.0);
    write_f32_wav(dest_path, &final_samples, 24000)
}

/// Write in-memory PCM float samples as a 30s auto-stitched WAV file.
pub fn write_pcm_to_wav(
    pcm_f32: &[f32],
    sample_rate: u32,
    dest_path: &Path,
) -> Result<usize, String> {
    let final_samples = pad_or_truncate_audio(pcm_f32, sample_rate, 30.0);
    let len = final_samples.len();
    write_f32_wav(dest_path, &final_samples, sample_rate)?;
    Ok(len)
}

// ─── Speaker Embedding Pre-Baking ─────────────────────────────────────────────

/// Pre-bake Chatterbox speaker embedding tensors from a source reference WAV.
pub fn pre_bake_speaker_tensors(source_wav: &Path, baked_dir: &Path) -> Result<(), String> {
    use chatterbox_rs::{Engine, EngineOptions};

    std::fs::create_dir_all(baked_dir)
        .map_err(|e| format!("Failed to create baked voice directory: {}", e))?;

    let tts_model_dir =
        crate::utils::paths::model_dir("tts").join(crate::services::tts::MODEL_DIR_TTS_CHATTERBOX);
    let t3_path = tts_model_dir.join(crate::services::tts::MODEL_FILE_TTS_CHATTERBOX_T3);
    let s3_path = tts_model_dir.join(crate::services::tts::MODEL_FILE_TTS_CHATTERBOX_S3GEN);

    if !t3_path.exists() || !s3_path.exists() {
        return Err("Chatterbox models not found on disk. Ensure setup is complete.".to_string());
    }

    let engine = Engine::new(EngineOptions {
        t3_gguf_path: t3_path.to_string_lossy().into_owned(),
        s3gen_gguf_path: s3_path.to_string_lossy().into_owned(),
        reference_audio: source_wav.to_string_lossy().into_owned(),
        verbose: false,
        ..Default::default()
    })
    .map_err(|e| format!("Failed to initialize pre-bake engine: {}", e))?;

    engine
        .save_voice(baked_dir)
        .map_err(|e| format!("Failed to save pre-baked voice tensors: {}", e))?;

    log::info!(
        "[Voice] Speaker tensors successfully pre-baked to {:?}",
        baked_dir
    );
    Ok(())
}

// ─── Preview Clip Synthesis ───────────────────────────────────────────────────

/// Synthesize a short preview clip with reference audio and save to destination WAV.
pub fn synthesize_preview_clip(wav_path: &str, preview_path: &Path) -> Result<(), String> {
    let chatterbox_path = crate::utils::paths::model_dir("tts").join("chatterbox");
    let engine =
        crate::services::tts::ChatterboxEngine::new(&chatterbox_path, "en", 8, 1.0, Some(wav_path))
            .map_err(|e| format!("Failed to create preview engine: {}", e))?;

    let preview_text = "Hello, I'm your Vox assistant.";
    let (tx, rx) = std::sync::mpsc::channel::<crate::core::events::VoxEvent>();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    engine
        .synthesize_chunk(preview_text, 0, cancel, tx)
        .map_err(|e| format!("Preview synthesis failed: {}", e))?;

    let mut pcm: Vec<f32> = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let crate::core::events::VoxEvent::TtsChunk { samples, .. } = event {
            pcm.extend_from_slice(&samples);
        }
    }

    if pcm.is_empty() {
        return Err("Preview synthesis produced no audio".to_string());
    }

    write_f32_wav(preview_path, &pcm, 24000)
}

// ─── CPAL Audio Recording Service ─────────────────────────────────────────────

struct ActiveRecorder {
    _stream: cpal::Stream,
    samples: Arc<parking_lot::Mutex<Vec<f32>>>,
    sample_rate: u32,
}

unsafe impl Send for ActiveRecorder {}
unsafe impl Sync for ActiveRecorder {}

static ACTIVE_RECORDER: Lazy<parking_lot::Mutex<Option<ActiveRecorder>>> =
    Lazy::new(|| parking_lot::Mutex::new(None));

/// Start recording mono float32 audio samples from the default input device.
pub fn start_recording() -> Result<(), String> {
    let mut recorder_guard = ACTIVE_RECORDER.lock();
    if recorder_guard.is_some() {
        return Err("Recording is already in progress".to_string());
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No default input device found".to_string())?;

    let config: cpal::StreamConfig = device
        .default_input_config()
        .map_err(|e| format!("Failed to get default input config: {}", e))?
        .into();

    let sample_rate = config.sample_rate.0;
    let channels = config.channels as usize;
    let samples = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let samples_clone = Arc::clone(&samples);

    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut guard = samples_clone.lock();
                if channels == 1 {
                    guard.extend_from_slice(data);
                } else {
                    for chunk in data.chunks_exact(channels) {
                        let avg: f32 = chunk.iter().sum::<f32>() / channels as f32;
                        guard.push(avg);
                    }
                }
            },
            |err| log::error!("Error in recording stream: {}", err),
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start recording stream: {}", e))?;

    *recorder_guard = Some(ActiveRecorder {
        _stream: stream,
        samples,
        sample_rate,
    });

    log::info!("[Voice] Recording started at {} Hz", sample_rate);
    Ok(())
}

/// Stop the active recording and return captured mono float32 samples and sample rate.
pub fn stop_recording() -> Result<(Vec<f32>, u32), String> {
    let mut recorder_guard = ACTIVE_RECORDER.lock();
    let recorder = recorder_guard
        .take()
        .ok_or_else(|| "No active recording found to stop".to_string())?;

    drop(recorder._stream);

    let samples = recorder.samples.lock().clone();
    let sample_rate = recorder.sample_rate;
    log::info!(
        "[Voice] Recording stopped. Captured {} samples at {} Hz ({:.2}s)",
        samples.len(),
        sample_rate,
        samples.len() as f32 / sample_rate as f32
    );
    Ok((samples, sample_rate))
}

// ─── Edge TTS Remote Query ────────────────────────────────────────────────────

/// Query Microsoft's Read Aloud endpoint for available Edge TTS voices.
pub async fn fetch_remote_edge_voices() -> Result<Vec<EdgeTtsVoiceEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let token = crate::services::tts::providers::edge_tts::get_trusted_client_token();
    let url = format!(
        "https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/voices/list?trustedclienttoken={}",
        token
    );
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
        )
        .send()
        .await
        .map_err(|e| format!("Network error reaching Edge TTS endpoint: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Edge TTS service error: HTTP {}", resp.status()));
    }

    #[derive(Deserialize)]
    struct RawEdgeVoice {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "ShortName")]
        short_name: String,
        #[serde(rename = "Gender")]
        gender: String,
        #[serde(rename = "Locale")]
        locale: String,
        #[serde(rename = "FriendlyName")]
        friendly_name: String,
    }

    let raw_voices: Vec<RawEdgeVoice> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Edge TTS voices response: {}", e))?;

    Ok(raw_voices
        .into_iter()
        .map(|v| EdgeTtsVoiceEntry {
            name: v.name,
            short_name: v.short_name,
            gender: v.gender,
            locale: v.locale,
            friendly_name: v.friendly_name,
        })
        .collect())
}
