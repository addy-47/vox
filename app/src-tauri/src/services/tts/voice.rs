//! Voice service for audio validation, decoding, resampling, speaker pre-baking, and recording.

use std::{
    fs::{create_dir_all, File},
    io::ErrorKind,
    mem,
    path::Path,
    sync::Arc,
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use symphonia_core::audio::{Audio, GenericAudioBufferRef};

use crate::{
    core::defaults::{DEFAULT_TTS_KOKORO_VOICE_ID, DEFAULT_TTS_SUPERTONIC_VOICE_ID},
    services::{
        audio::resolve_audio_host,
        tts::{
            providers::edge_tts::get_trusted_client_token, TtsActiveProvider, CHATTERBOX_MODEL_DIR,
            EDGE_TTS_USER_AGENT, EDGE_TTS_VOICES_URL_BASE, MODEL_FILE_TTS_CHATTERBOX_S3GEN,
            MODEL_FILE_TTS_CHATTERBOX_T3, TTS_SAMPLE_RATE,
        },
    },
    utils::paths::model_dir,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VoiceProfile {
    pub id: i32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Metadata describing an online Edge TTS neural voice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeTtsVoiceEntry {
    pub name: String,
    pub short_name: String,
    pub gender: String,
    pub locale: String,
    pub friendly_name: String,
}

pub const MIN_VOICE_CLONE_DURATION_SECS: f32 = 1.0;
pub const TARGET_VOICE_SAMPLE_DURATION_SECS: f32 = 30.0;

fn extract_mono_f32_samples(buf_ref: GenericAudioBufferRef<'_>, raw_samples: &mut Vec<f32>) {
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
    use symphonia_core::{
        codecs::audio::AudioDecoderOptions,
        errors::Error,
        formats::{probe::Hint, FormatOptions},
        io::MediaSourceStream,
        meta::MetadataOptions,
    };

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
        Some(symphonia_core::codecs::CodecParameters::Audio(params)) => params,
        _ => return Err("Track is not an audio track or lacks codec parameters".to_string()),
    };

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|e| format!("Failed to initialize decoder: {}", e))?;

    let track_id = track.id;
    let input_sample_rate = codec_params.sample_rate.unwrap_or(TTS_SAMPLE_RATE);
    let mut raw_samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(Error::IoError(ref err)) if err.kind() == ErrorKind::UnexpectedEof => break,
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

fn truncate_audio_if_exceeds(samples: &[f32], sample_rate: u32, max_duration_secs: f32) -> &[f32] {
    let max_len = (max_duration_secs * sample_rate as f32) as usize;
    if samples.len() > max_len {
        &samples[..max_len]
    } else {
        samples
    }
}

/// Write mono f32 samples to a standard 16-bit PCM WAV file with peak normalization.
/// Standard 16-bit PCM uses WAVE_FORMAT_PCM (0x0001) without extensible speaker masks,
/// ensuring both left and right earphones/speakers receive balanced, properly amplified audio.
pub fn write_f32_wav(dest_path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(dest_path, spec)
        .map_err(|e| format!("Failed to create destination WAV writer: {}", e))?;

    // Find peak magnitude to normalize volume without clipping
    let peak = samples
        .iter()
        .copied()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    let scale = if peak > 1e-4 && peak < 0.95 {
        (0.95 / peak) * 32767.0
    } else {
        32767.0
    };

    for sample in samples {
        let scaled = (sample * scale).clamp(-32768.0, 32767.0);
        writer
            .write_sample(scaled as i16)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))
}

/// Decode any supported audio file to mono 24kHz f32 WAV capped at 30s without artificial looping.
pub fn convert_and_validate_audio(src_path: &str, dest_path: &Path) -> Result<(), String> {
    let (raw_samples, src_rate) = decode_audio_stream(src_path)?;
    let resampled = resample_linear(&raw_samples, src_rate, TTS_SAMPLE_RATE);

    let duration_secs = resampled.len() as f32 / TTS_SAMPLE_RATE as f32;
    if duration_secs < MIN_VOICE_CLONE_DURATION_SECS {
        return Err(format!(
            "Audio too short ({:.1}s). Minimum is {}s for voice cloning.",
            duration_secs, MIN_VOICE_CLONE_DURATION_SECS
        ));
    }

    let final_samples = truncate_audio_if_exceeds(
        &resampled,
        TTS_SAMPLE_RATE,
        TARGET_VOICE_SAMPLE_DURATION_SECS,
    );
    write_f32_wav(dest_path, final_samples, TTS_SAMPLE_RATE)
}

/// Write in-memory PCM float samples as a WAV file capped at 30s without artificial looping.
pub fn write_pcm_to_wav(
    pcm_f32: &[f32],
    sample_rate: u32,
    dest_path: &Path,
) -> Result<usize, String> {
    let final_samples =
        truncate_audio_if_exceeds(pcm_f32, sample_rate, TARGET_VOICE_SAMPLE_DURATION_SECS);
    let len = final_samples.len();
    write_f32_wav(dest_path, final_samples, sample_rate)?;
    Ok(len)
}

/// Pre-bake Chatterbox speaker embedding tensors from a source reference WAV.
pub fn pre_bake_speaker_tensors(source_wav: &Path, baked_dir: &Path) -> Result<(), String> {
    use chatterbox_rs::{Engine, EngineOptions};

    create_dir_all(baked_dir)
        .map_err(|e| format!("Failed to create baked voice directory: {}", e))?;

    let tts_model_dir = model_dir(CHATTERBOX_MODEL_DIR);
    let t3_path = tts_model_dir.join(MODEL_FILE_TTS_CHATTERBOX_T3);
    let s3_path = tts_model_dir.join(MODEL_FILE_TTS_CHATTERBOX_S3GEN);

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

    let host = resolve_audio_host();
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

    let samples = mem::take(&mut *recorder.samples.lock());
    let sample_rate = recorder.sample_rate;
    log::info!(
        "[Voice] Recording stopped. Captured {} samples at {} Hz ({:.2}s)",
        samples.len(),
        sample_rate,
        samples.len() as f32 / sample_rate as f32
    );
    Ok((samples, sample_rate))
}

/// Query Microsoft's Read Aloud endpoint for available Edge TTS voices.
pub async fn fetch_remote_edge_voices() -> Result<Vec<EdgeTtsVoiceEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let token = get_trusted_client_token();
    let url = format!("{}{}", EDGE_TTS_VOICES_URL_BASE, token);
    let resp = client
        .get(url)
        .header("User-Agent", EDGE_TTS_USER_AGENT)
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

/// Returns canonical Kokoro voice profiles (accurate 11-speaker ledger with correct gender and accent tags).
pub fn get_voice_profiles() -> Vec<VoiceProfile> {
    vec![
        VoiceProfile {
            id: 0,
            name: "Alloy".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 1,
            name: "Bella".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 2,
            name: "Nicole".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 3,
            name: "Sarah".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 4,
            name: "Sky".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 5,
            name: "Adam".to_string(),
            gender: Some("Male".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 6,
            name: "Michael".to_string(),
            gender: Some("Male".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 7,
            name: "Emma".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("British".to_string()),
            language: Some("en-GB".to_string()),
        },
        VoiceProfile {
            id: 8,
            name: "Isabella".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("British".to_string()),
            language: Some("en-GB".to_string()),
        },
        VoiceProfile {
            id: 9,
            name: "George".to_string(),
            gender: Some("Male".to_string()),
            accent: Some("British".to_string()),
            language: Some("en-GB".to_string()),
        },
        VoiceProfile {
            id: 10,
            name: "Heart".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
    ]
}

/// Resolves a canonical voice id to a catalog index. Matches by
/// case-insensitive name first, then numeric parse. Falls back to 0.
pub fn resolve_catalog_index(profiles: &[VoiceProfile], voice_id: &str) -> i32 {
    let needle = voice_id.trim().to_lowercase();
    for p in profiles {
        if p.name.to_lowercase() == needle {
            return p.id;
        }
    }
    if let Ok(n) = needle.parse::<i32>() {
        if profiles.iter().any(|p| p.id == n) {
            return n;
        }
    }
    profiles.first().map(|p| p.id).unwrap_or(0)
}

/// Returns the voice profiles of a provider that ships a bundled catalogue.
///
/// A provider without a bundled catalogue (Chatterbox, ZipVoice, the cloud
/// providers) addresses its voices by id or by directory, not by catalogue
/// index.
///
/// The match is exhaustive over the canonical provider enum, so every
/// provider the factory accepts is classified here. A provider can never
/// fall through to another provider's catalogue.
fn provider_voice_profiles(provider: TtsActiveProvider) -> Option<Vec<VoiceProfile>> {
    match provider {
        TtsActiveProvider::Kokoro => Some(get_voice_profiles()),
        TtsActiveProvider::Supertonic => Some(get_supertonic_voice_profiles()),
        TtsActiveProvider::EdgeTts
        | TtsActiveProvider::Chatterbox
        | TtsActiveProvider::ChatterboxRemote
        | TtsActiveProvider::Zipvoice
        | TtsActiveProvider::AzureTts
        | TtsActiveProvider::GoogleTts => None,
    }
}

/// Parses a provider id into the canonical provider enum.
///
/// The voice catalogue resolves providers through the same enum the provider
/// factory parses. A typo in a provider id is therefore an explicit error
/// instead of a silent fallback to the first catalogue entry — which gave
/// the user a voice they never chose.
fn parse_tts_provider(provider_id: &str) -> Result<TtsActiveProvider, String> {
    serde_json::from_value(serde_json::Value::String(provider_id.to_string()))
        .map_err(|_| format!("Unknown TTS provider id: {provider_id}"))
}

/// The default voice id of a provider that ships a bundled catalogue.
///
/// The default voice is provider-scoped: a voice id belongs to exactly one
/// provider's catalogue, so there is no single global default.
pub fn default_voice_id_for_provider(provider_id: &str) -> Option<&'static str> {
    let provider = parse_tts_provider(provider_id).ok()?;
    default_voice_id_for(provider)
}

/// The default voice id of a provider, resolved through the canonical enum.
fn default_voice_id_for(provider: TtsActiveProvider) -> Option<&'static str> {
    match provider {
        TtsActiveProvider::Kokoro => Some(DEFAULT_TTS_KOKORO_VOICE_ID),
        TtsActiveProvider::Supertonic => Some(DEFAULT_TTS_SUPERTONIC_VOICE_ID),
        TtsActiveProvider::EdgeTts
        | TtsActiveProvider::Chatterbox
        | TtsActiveProvider::ChatterboxRemote
        | TtsActiveProvider::Zipvoice
        | TtsActiveProvider::AzureTts
        | TtsActiveProvider::GoogleTts => None,
    }
}

/// Resolves the voice id a provider must use.
///
/// A stored id that names a voice of the provider is kept. A stored id that
/// belongs to another provider — or to none — resolves to that provider's own
/// default voice.
///
/// The resolution never falls through to an arbitrary catalogue index: a
/// catalogue-index fallback silently gives the user a voice they did not
/// choose.
pub fn resolve_provider_voice_id(provider_id: &str, voice_id: &str) -> String {
    let trimmed = voice_id.trim();
    let Ok(provider) = parse_tts_provider(provider_id) else {
        log::warn!(
            "[Voice] Unknown TTS provider id '{provider_id}': the stored voice id is kept unchanged"
        );
        return trimmed.to_string();
    };
    resolve_provider_voice_id_for(provider, trimmed)
}

/// Resolves the voice id of a provider, resolved through the canonical enum.
fn resolve_provider_voice_id_for(provider: TtsActiveProvider, voice_id: &str) -> String {
    let Some(profiles) = provider_voice_profiles(provider) else {
        return voice_id.to_string();
    };

    let needle = voice_id.to_lowercase();
    if profiles.iter().any(|p| p.name.to_lowercase() == needle) {
        return needle;
    }

    default_voice_id_for(provider)
        .unwrap_or(voice_id)
        .to_lowercase()
}

/// Resolves the catalogue index of the voice a provider must use.
///
/// The index is taken from the provider-aware voice id, so it names the
/// provider's default voice rather than the first entry of the catalogue.
///
/// An unrecognised provider id is an explicit error. The old behaviour
/// resolved it to index zero, which silently selected a voice the user never
/// chose.
pub fn resolve_voice_index_for_provider(provider_id: &str, voice_id: &str) -> Result<i32, String> {
    let provider = parse_tts_provider(provider_id)?;
    let Some(profiles) = provider_voice_profiles(provider) else {
        // A provider without a bundled catalogue addresses its voices by id
        // or by directory. There is no catalogue position to resolve, so the
        // index stays at the provider's default position.
        return Ok(0);
    };
    let resolved = resolve_provider_voice_id_for(provider, voice_id);
    Ok(resolve_catalog_index(&profiles, &resolved))
}

/// Returns default Supertonic voice profiles.
pub fn get_supertonic_voice_profiles() -> Vec<VoiceProfile> {
    vec![
        VoiceProfile {
            id: 0,
            name: "James".to_string(),
            gender: Some("Male".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 1,
            name: "David".to_string(),
            gender: Some("Male".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 2,
            name: "Alex".to_string(),
            gender: Some("Male".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 3,
            name: "Ryan".to_string(),
            gender: Some("Male".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 4,
            name: "Ethan".to_string(),
            gender: Some("Male".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 5,
            name: "Sophia".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 6,
            name: "Olivia".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 7,
            name: "Emma".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 8,
            name: "Ava".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
        VoiceProfile {
            id: 9,
            name: "Mia".to_string(),
            gender: Some("Female".to_string()),
            accent: Some("American".to_string()),
            language: Some("en-US".to_string()),
        },
    ]
}

pub const VOICES_MANIFEST_JSON: &str = include_str!("voices_manifest.json");

/// Engine-to-catalog scope resolved by the backend so the frontend never
/// branches on provider id literals (Invariant 6: model agnosticism).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceScopeDto {
    /// DB `provider_id` to scope the catalog query. Empty = all providers.
    pub voice_provider: String,
    /// Mirrors `ProviderCaps.clone` for the active engine.
    pub can_clone: bool,
    /// True when voices arrive via background cloud sync (Edge family).
    pub cloud_sync: bool,
}

/// Maps an active TTS engine id to its voice catalog scope.
///
/// `chatterbox_remote` shares the `chatterbox` voice family rows: no
/// `provider_id = 'chatterbox_remote'` rows exist in the voices table, so an
/// unmapped query would return an empty catalog.
pub fn resolve_voice_scope(engine: &str) -> VoiceScopeDto {
    let engine = engine.trim();
    let voice_provider = if engine.eq_ignore_ascii_case("chatterbox_remote") {
        "chatterbox".to_string()
    } else {
        engine.to_string()
    };
    let (can_clone, cloud_sync) = match crate::services::tts::caps_for_id(engine) {
        Ok(caps) => (
            caps.clone,
            caps.voices == crate::services::tts::TtsVoiceSource::Edge
                || caps.voices == crate::services::tts::TtsVoiceSource::Cloud,
        ),
        Err(_) => (false, false),
    };
    VoiceScopeDto {
        voice_provider,
        can_clone,
        cloud_sync,
    }
}

/// Frontend-safe representation of a unified voice entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedVoiceDto {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub description: Option<String>,
    pub gender: String,
    pub locale: String,
    pub accent: Option<String>,
    pub tags: Vec<String>,
    pub sample_text: String,
    pub source_kind: String,
    pub is_favorite: bool,
    pub last_used_at: Option<i64>,
    pub has_preview: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestVoiceEntry {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub gender: String,
    pub locale: String,
    pub accent: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub sample_text: String,
}

pub fn get_manifest_voices() -> Vec<ManifestVoiceEntry> {
    serde_json::from_str(VOICES_MANIFEST_JSON).unwrap_or_default()
}

pub fn split_locale(locale: &str) -> (String, Option<String>) {
    let mut parts = locale.splitn(2, '-');
    let lang = parts.next().unwrap_or("en").to_string();
    match parts.next() {
        Some(region) => {
            let accent = match region.to_uppercase().as_str() {
                "US" => "American",
                "GB" | "UK" => "British",
                "AU" => "Australian",
                "IN" => "Indian",
                "CA" => "Canadian",
                _ => return (locale.to_string(), Some(region.to_string())),
            };
            (locale.to_string(), Some(accent.to_string()))
        }
        None => (lang, None),
    }
}

pub fn normalize_gender_str(raw: &str) -> String {
    match raw.to_lowercase().as_str() {
        "female" | "f" => "female".to_string(),
        "male" | "m" => "male".to_string(),
        _ => "neutral".to_string(),
    }
}

pub fn simplify_edge_friendly_name(short_name: &str, friendly_name: &str) -> String {
    let raw = friendly_name
        .replace("Microsoft ", "")
        .replace(" Server Speech Text to Speech Voice", "")
        .replace(" Online (Natural)", "");
    let parts: Vec<&str> = raw.split('(').collect();
    let name_part = parts.first().unwrap_or(&short_name).trim();
    if name_part.is_empty() {
        short_name.to_string()
    } else {
        name_part.to_string()
    }
}

/// Syncs declarative bundled manifest voices into the Turso voices table.
pub async fn sync_manifest_voices_to_db(conn: &turso::Connection) -> anyhow::Result<()> {
    let manifest = get_manifest_voices();
    if manifest.is_empty() {
        return Ok(());
    }
    crate::persistence::voices::sync_manifest_voices_batch(conn, &manifest).await
}

/// Metadata describing an Azure Speech neural voice from `voices/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureTtsVoiceEntry {
    pub short_name: String,
    pub display_name: String,
    pub gender: String,
    pub locale: String,
    pub voice_type: String,
    pub status: String,
    pub sample_rate_hertz: i32,
}

/// Queries Azure `voices/list` for GA English Neural voices at 24 kHz or above.
pub async fn fetch_remote_azure_voices(
    base_url: &str,
    key: &str,
) -> Result<Vec<AzureTtsVoiceEntry>, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct RawAzureVoice {
        #[serde(default)]
        short_name: String,
        #[serde(default)]
        display_name: String,
        #[serde(default)]
        gender: String,
        #[serde(default)]
        locale: String,
        #[serde(default)]
        voice_type: String,
        #[serde(default)]
        status: String,
        #[serde(default)]
        sample_rate_hertz: String,
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let url = format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        crate::services::tts::AZURE_TTS_VOICES_PATH
    );
    let resp = client
        .get(url)
        .header("Ocp-Apim-Subscription-Key", key)
        .send()
        .await
        .map_err(|e| format!("Network error reaching Azure voices endpoint: {}", e))?;
    if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
        return Err("Azure Speech rejected the resource key (HTTP 401/403)".to_string());
    }
    if !resp.status().is_success() {
        return Err(format!("Azure Speech voices error: HTTP {}", resp.status()));
    }
    let raw_voices: Vec<RawAzureVoice> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Azure voices response: {}", e))?;
    let total = raw_voices.len();
    let kept: Vec<AzureTtsVoiceEntry> = raw_voices
        .into_iter()
        .filter(|v| {
            v.status.eq_ignore_ascii_case("GA")
                && v.voice_type.eq_ignore_ascii_case("Neural")
                && v.locale.to_lowercase().starts_with("en")
                && v.sample_rate_hertz.parse::<i32>().unwrap_or(0) >= 24000
                && !v.short_name.is_empty()
        })
        .map(|v| {
            let short_name = v.short_name;
            let display_name = if v.display_name.is_empty() {
                short_name.clone()
            } else {
                v.display_name
            };
            AzureTtsVoiceEntry {
                short_name,
                display_name,
                gender: v.gender,
                locale: v.locale,
                voice_type: v.voice_type,
                status: v.status,
                sample_rate_hertz: v.sample_rate_hertz.parse::<i32>().unwrap_or(24000),
            }
        })
        .collect();
    log::info!(
        "[Voice] Azure voices filtered {}/{} (kept GA English Neural at 24 kHz or above)",
        kept.len(),
        total
    );
    Ok(kept)
}

/// Syncs live Azure voices into the Turso voices table in a single transaction.
pub async fn sync_azure_tts_voices_to_db(
    conn: &turso::Connection,
    base_url: &str,
    key: &str,
) -> anyhow::Result<usize> {
    let voices = fetch_remote_azure_voices(base_url, key)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let count = voices.len();
    if count == 0 {
        return Ok(0);
    }
    let rows: Vec<crate::persistence::voices::RemoteVoiceRow> = voices
        .into_iter()
        .map(|v| {
            let (locale, accent) = split_locale(&v.locale);
            let gender = normalize_gender_str(&v.gender);
            crate::persistence::voices::RemoteVoiceRow {
                id: v.short_name,
                provider_id: "azure_tts".to_string(),
                name: v.display_name,
                gender,
                locale,
                accent,
                description: None,
                tags_json: "[\"Cloud\",\"Neural\"]".to_string(),
                sample_text: "Azure AI Speech neural preview.".to_string(),
            }
        })
        .collect();
    crate::persistence::voices::upsert_remote_voices_batch(conn, &rows).await?;
    Ok(count)
}

/// Queries Google `voices:list` for streaming-capable Chirp 3: HD English voices.
pub async fn fetch_remote_google_voices() -> Result<Vec<EdgeTtsVoiceEntry>, String> {
    let client = google_cloud_texttospeech_v1::client::TextToSpeech::builder()
        .build()
        .await
        .map_err(|e| {
            format!(
                "Google TTS ADC failed (run `gcloud auth application-default login`): {}",
                e
            )
        })?;
    let mut voices = Vec::new();
    for region in ["en-US", "en-GB", "en-AU", "en-IN"] {
        let response = client
            .list_voices()
            .set_language_code(region)
            .send()
            .await
            .map_err(|e| format!("Google voices:list failed for {}: {}", region, e))?;
        for v in response.voices {
            if !v
                .name
                .contains(crate::services::tts::GOOGLE_TTS_STREAMING_VOICE_INFIX)
            {
                continue;
            }
            let locale = v.language_codes.first().cloned().unwrap_or_default();
            let style = v.name.rsplit('-').next().unwrap_or(&v.name).to_string();
            voices.push(EdgeTtsVoiceEntry {
                name: v.name.clone(),
                short_name: v.name,
                gender: match v.ssml_gender {
                    google_cloud_texttospeech_v1::model::SsmlVoiceGender::Male => {
                        "male".to_string()
                    }
                    google_cloud_texttospeech_v1::model::SsmlVoiceGender::Female => {
                        "female".to_string()
                    }
                    _ => "neutral".to_string(),
                },
                locale: locale.clone(),
                friendly_name: format!("{style} ({locale})"),
            });
        }
    }
    Ok(voices)
}

/// Syncs live Google voices into the Turso voices table in a single transaction.
pub async fn sync_google_tts_voices_to_db(conn: &turso::Connection) -> anyhow::Result<usize> {
    let voices = fetch_remote_google_voices()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let count = voices.len();
    if count == 0 {
        return Ok(0);
    }
    let rows: Vec<crate::persistence::voices::RemoteVoiceRow> = voices
        .into_iter()
        .map(|v| {
            let (locale, accent) = split_locale(&v.locale);
            let gender = normalize_gender_str(&v.gender);
            let display = format!(
                "{} ({})",
                v.short_name.rsplit('-').next().unwrap_or(&v.short_name),
                v.locale
            );
            let id = v.short_name.clone();
            crate::persistence::voices::RemoteVoiceRow {
                id,
                provider_id: "google_tts".to_string(),
                name: display,
                gender,
                locale,
                accent,
                description: None,
                tags_json: "[\"Cloud\",\"Chirp\"]".to_string(),
                sample_text: "Google Cloud Chirp 3 HD preview.".to_string(),
            }
        })
        .collect();
    crate::persistence::voices::upsert_remote_voices_batch(conn, &rows).await?;
    Ok(count)
}

/// Syncs remote Edge TTS voices into the Turso voices table in a single transaction.
pub async fn sync_edge_tts_voices_to_db(conn: &turso::Connection) -> anyhow::Result<usize> {
    let edge_voices = fetch_remote_edge_voices()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let count = edge_voices.len();
    if count == 0 {
        return Ok(0);
    }
    let rows: Vec<crate::persistence::voices::RemoteVoiceRow> = edge_voices
        .into_iter()
        .map(|e| {
            let (locale, accent) = split_locale(&e.locale);
            let gender = normalize_gender_str(&e.gender);
            let clean_name = simplify_edge_friendly_name(&e.short_name, &e.friendly_name);
            crate::persistence::voices::RemoteVoiceRow {
                id: e.short_name,
                provider_id: "edge_tts".to_string(),
                name: clean_name,
                gender,
                locale,
                accent,
                description: None,
                tags_json: "[\"Cloud\",\"Neural\"]".to_string(),
                sample_text: "Microsoft Edge Neural Text to Speech preview.".to_string(),
            }
        })
        .collect();
    crate::persistence::voices::upsert_remote_voices_batch(conn, &rows).await?;
    Ok(count)
}

/// Lists unified voices from Turso, ensuring manifest voices are populated offline.
pub async fn list_unified_voices(
    conn: &turso::Connection,
    provider: Option<&str>,
) -> anyhow::Result<Vec<UnifiedVoiceDto>> {
    // 1. Self-healing: ensure packaged chatterbox and zipvoice voices have their correct provider_id
    let _ = crate::persistence::voices::migrate_legacy_voice_providers(conn).await;

    // 2. Query Turso voices table offline-first. The engine id maps through
    // the same scope resolution as `resolve_voice_scope` so remote-family
    // engines share their local voice rows.
    let scoped = provider.map(|p| {
        if p.eq_ignore_ascii_case("chatterbox_remote") {
            "chatterbox"
        } else {
            p
        }
    });

    let mut entries = crate::persistence::voices::list_voices(
        conn,
        scoped,
        crate::persistence::voices::VoiceListScope::All,
    )
    .await?;

    // 3. If table has not yet been seeded (e.g. fresh ephemeral test DB), seed manifest once
    if entries.is_empty() {
        let _ = sync_manifest_voices_to_db(conn).await;
        entries = crate::persistence::voices::list_voices(
            conn,
            scoped,
            crate::persistence::voices::VoiceListScope::All,
        )
        .await?;
    }

    let dtos = entries
        .into_iter()
        .map(|e| {
            let tags: Vec<String> = serde_json::from_str(&e.tags).unwrap_or_default();
            UnifiedVoiceDto {
                id: e.id,
                provider_id: e.provider_id,
                name: e.name,
                description: e.description,
                gender: e.gender,
                locale: e.locale,
                accent: e.accent,
                tags,
                sample_text: e.sample_text,
                source_kind: e.source_kind.as_str().to_string(),
                is_favorite: e.is_favorite,
                last_used_at: e.last_used_at,
                has_preview: true,
            }
        })
        .collect();

    Ok(dtos)
}
