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
            EDGE_TTS_USER_AGENT, EDGE_TTS_VOICES_URL_BASE, TTS_SAMPLE_RATE,
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
    create_dir_all(baked_dir)
        .map_err(|e| format!("Failed to create baked voice directory: {}", e))?;

    let tts_model_dir = model_dir(CHATTERBOX_MODEL_DIR);
    let model_dir_str = tts_model_dir.to_string_lossy().to_string();

    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let base_url = crate::services::sidecar::global_sidecar_supervisor()
                .ensure_ready(
                    crate::services::sidecar::SidecarKind::TtsClone,
                    &[("--model-dir", &model_dir_str)],
                )
                .await
                .map_err(|e| format!("Failed to start Chatterbox sidecar: {}", e))?;

            let client = reqwest::Client::new();
            let res = client
                .post(format!("{}/v1/voices/pre-bake", base_url))
                .json(&serde_json::json!({
                    "source_wav": source_wav,
                    "baked_dir": baked_dir,
                }))
                .send()
                .await
                .map_err(|e| format!("Pre-bake request failed: {}", e))?;

            if !res.status().is_success() {
                return Err(format!("Pre-bake failed: HTTP {}", res.status()));
            }
            Ok(())
        })
    })?;

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
    if engine.eq_ignore_ascii_case("gemini_live")
        || engine.eq_ignore_ascii_case("deepgram_voice_agent")
    {
        return VoiceScopeDto {
            voice_provider: engine.to_string(),
            can_clone: false,
            cloud_sync: true,
        };
    }
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

/// Syncs realtime voice roster for Gemini Live or Deepgram Agent into Turso `voices` table.
pub async fn sync_realtime_voices_to_db(
    conn: &turso::Connection,
    provider_id: &str,
) -> anyhow::Result<usize> {
    let rows: Vec<crate::persistence::voices::RemoteVoiceRow> =
        match provider_id.to_lowercase().as_str() {
            "gemini_live" => vec![
                crate::persistence::voices::RemoteVoiceRow {
                    id: "Aoede".to_string(),
                    provider_id: "gemini_live".to_string(),
                    name: "Aoede".to_string(),
                    gender: "female".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Warm and confident voice".to_string()),
                    tags_json: "[\"Realtime\",\"Google\"]".to_string(),
                    sample_text: "Hello! I am Aoede, ready to converse in real time.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "Charon".to_string(),
                    provider_id: "gemini_live".to_string(),
                    name: "Charon".to_string(),
                    gender: "male".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Deep and resonant voice".to_string()),
                    tags_json: "[\"Realtime\",\"Google\"]".to_string(),
                    sample_text: "Hello! I am Charon, ready to converse in real time.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "Fenrir".to_string(),
                    provider_id: "gemini_live".to_string(),
                    name: "Fenrir".to_string(),
                    gender: "male".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Authoritative and steady voice".to_string()),
                    tags_json: "[\"Realtime\",\"Google\"]".to_string(),
                    sample_text: "Hello! I am Fenrir, ready to converse in real time.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "Kore".to_string(),
                    provider_id: "gemini_live".to_string(),
                    name: "Kore".to_string(),
                    gender: "female".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Calm and empathetic voice".to_string()),
                    tags_json: "[\"Realtime\",\"Google\"]".to_string(),
                    sample_text: "Hello! I am Kore, ready to converse in real time.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "Puck".to_string(),
                    provider_id: "gemini_live".to_string(),
                    name: "Puck".to_string(),
                    gender: "male".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Playful and engaging voice".to_string()),
                    tags_json: "[\"Realtime\",\"Google\"]".to_string(),
                    sample_text: "Hello! I am Puck, ready to converse in real time.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "Leda".to_string(),
                    provider_id: "gemini_live".to_string(),
                    name: "Leda".to_string(),
                    gender: "female".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Clear and professional voice".to_string()),
                    tags_json: "[\"Realtime\",\"Google\"]".to_string(),
                    sample_text: "Hello! I am Leda, ready to converse in real time.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "Orus".to_string(),
                    provider_id: "gemini_live".to_string(),
                    name: "Orus".to_string(),
                    gender: "male".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Balanced and casual voice".to_string()),
                    tags_json: "[\"Realtime\",\"Google\"]".to_string(),
                    sample_text: "Hello! I am Orus, ready to converse in real time.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "Zephyr".to_string(),
                    provider_id: "gemini_live".to_string(),
                    name: "Zephyr".to_string(),
                    gender: "female".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Bright and friendly voice".to_string()),
                    tags_json: "[\"Realtime\",\"Google\"]".to_string(),
                    sample_text: "Hello! I am Zephyr, ready to converse in real time.".to_string(),
                },
            ],
            "deepgram_voice_agent" => vec![
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-asteria-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Asteria".to_string(),
                    gender: "female".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Warm and conversational voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Asteria from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-luna-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Luna".to_string(),
                    gender: "female".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Clear and cheerful voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Luna from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-stella-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Stella".to_string(),
                    gender: "female".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Expressive voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Stella from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-athena-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Athena".to_string(),
                    gender: "female".to_string(),
                    locale: "en-GB".to_string(),
                    accent: Some("British".to_string()),
                    description: Some("British sophisticated voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Athena from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-hera-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Hera".to_string(),
                    gender: "female".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Professional voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Hera from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-orion-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Orion".to_string(),
                    gender: "male".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Calm and confident voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Orion from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-arcas-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Arcas".to_string(),
                    gender: "male".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Resonant and clear voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Arcas from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-perseus-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Perseus".to_string(),
                    gender: "male".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Natural and direct voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Perseus from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "aura-angus-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Angus".to_string(),
                    gender: "male".to_string(),
                    locale: "en-IE".to_string(),
                    accent: Some("Irish".to_string()),
                    description: Some("Irish melodic voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Angus from Deepgram.".to_string(),
                },
                crate::persistence::voices::RemoteVoiceRow {
                    id: "flux-alexis-en".to_string(),
                    provider_id: "deepgram_voice_agent".to_string(),
                    name: "Alexis".to_string(),
                    gender: "neutral".to_string(),
                    locale: "en-US".to_string(),
                    accent: Some("American".to_string()),
                    description: Some("Fast and smooth voice".to_string()),
                    tags_json: "[\"Realtime\",\"Deepgram\"]".to_string(),
                    sample_text: "Hello, this is Alexis from Deepgram.".to_string(),
                },
            ],
            _ => return Ok(0),
        };
    let count = rows.len();
    crate::persistence::voices::upsert_remote_voices_batch(conn, &rows).await?;
    Ok(count)
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

/// Queries Google `voices:list` for streaming-capable Chirp 3: HD English voices via the sidecar.
pub async fn fetch_remote_google_voices() -> Result<Vec<EdgeTtsVoiceEntry>, String> {
    let base_url = crate::services::sidecar::global_sidecar_supervisor()
        .ensure_ready(crate::services::sidecar::SidecarKind::TtsCloud, &[])
        .await
        .map_err(|e| format!("Google TTS sidecar not ready: {}", e))?;

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/v1/google/voices", base_url))
        .json(&serde_json::json!({
            "regions": ["en-US", "en-GB", "en-AU", "en-IN"]
        }))
        .send()
        .await
        .map_err(|e| format!("Google voices request failed: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Google voices failed: HTTP {}", res.status()));
    }

    #[derive(serde::Deserialize)]
    struct RemoteVoice {
        name: String,
        short_name: String,
        gender: String,
        locale: String,
        friendly_name: String,
    }

    let remote_voices: Vec<RemoteVoice> = res.json().await.map_err(|e| e.to_string())?;
    Ok(remote_voices
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::services::tts::{caps_for_id, TtsActiveProvider};

    /// Every provider id the provider factory accepts.
    ///
    /// The list mirrors the canonical enum: a provider the factory can build is a
    /// provider the voice catalogue must classify.
    const FACTORY_PROVIDER_IDS: [&str; 8] = [
        "supertonic",
        "kokoro",
        "chatterbox",
        "chatterbox_remote",
        "edge_tts",
        "zipvoice",
        "azure_tts",
        "google_tts",
    ];

    #[test]
    fn an_unrecognised_provider_id_is_an_explicit_error() {
        let error = resolve_voice_index_for_provider("supertonnic", "james")
            .expect_err("a typo in a provider id must not resolve to a catalogue index");

        assert!(
            error.contains("supertonnic"),
            "the error must name the provider id, it said: {error}"
        );

        for unknown in ["", "Kokoro", "kokoro ", "kokoro2", "supertonic\n"] {
            assert!(
                resolve_voice_index_for_provider(unknown, "james").is_err(),
                "the provider id `{unknown}` is not a provider the factory accepts, so it must \
             not resolve to a catalogue index"
            );
        }
    }

    // ============================================================================
    // Test 2: Every provider the factory accepts is classified
    // ============================================================================

    #[test]
    fn every_factory_provider_resolves_into_its_own_catalogue() {
        for provider_id in FACTORY_PROVIDER_IDS {
            // The factory must accept the id.
            caps_for_id(provider_id)
                .unwrap_or_else(|error| panic!("the factory must accept `{provider_id}`: {error}"));

            // The voice catalogue must resolve it — never to an error, and never
            // to another provider's catalogue.
            let resolved =
                resolve_voice_index_for_provider(provider_id, "james").unwrap_or_else(|error| {
                    panic!("the voice catalogue must resolve `{provider_id}`: {error}")
                });
            assert!(
                resolved >= 0,
                "the resolved index for `{provider_id}` must not be negative, it was {resolved}"
            );
        }

        // The catalogue-bearing providers resolve to their own default voice.
        let kokoro = resolve_voice_index_for_provider("kokoro", "james")
            .expect("Kokoro is a known provider id");
        let supertonic = resolve_voice_index_for_provider("supertonic", "james")
            .expect("Supertonic is a known provider id");
        assert_ne!(
            kokoro, supertonic,
            "Kokoro and Supertonic must not resolve to the same catalogue index"
        );
    }

    // ============================================================================
    // Test 3: The Kokoro and Supertonic id sets are distinct
    // ============================================================================

    #[test]
    fn kokoro_and_supertonic_voice_ids_do_not_overlap() {
        let manifest = crate::services::tts::voice::get_manifest_voices();
        let kokoro: BTreeSet<String> = manifest
            .iter()
            .filter(|entry| entry.provider_id == "kokoro")
            .map(|entry| entry.id.clone())
            .collect();
        let supertonic: BTreeSet<String> = manifest
            .iter()
            .filter(|entry| entry.provider_id == "supertonic")
            .map(|entry| entry.id.clone())
            .collect();

        assert!(!kokoro.is_empty(), "the Kokoro voice set must not be empty");
        assert!(
            !supertonic.is_empty(),
            "the Supertonic voice set must not be empty"
        );

        let overlap: Vec<&String> = kokoro.intersection(&supertonic).collect();
        assert!(
            overlap.is_empty(),
            "the Kokoro and Supertonic voice id sets must not overlap, they share {overlap:?}"
        );
        assert!(
            !kokoro.contains("james"),
            "a Supertonic voice id must not be a Kokoro voice id"
        );
        assert!(
            !supertonic.contains("bella"),
            "a Kokoro voice id must not be a Supertonic voice id"
        );
    }

    #[test]
    fn each_provider_voice_resolves_inside_its_own_catalogue() {
        let kokoro = crate::services::tts::voice::get_voice_profiles();
        let supertonic = crate::services::tts::voice::get_supertonic_voice_profiles();

        for profile in &kokoro {
            let name = profile.name.to_lowercase();
            let index = resolve_voice_index_for_provider("kokoro", &name)
                .expect("Kokoro is a known provider id");
            assert_eq!(
                kokoro[index as usize].name.to_lowercase(),
                name,
                "the Kokoro voice `{name}` must resolve inside the Kokoro catalogue"
            );
        }
        for profile in &supertonic {
            let name = profile.name.to_lowercase();
            let index = resolve_voice_index_for_provider("supertonic", &name)
                .expect("Supertonic is a known provider id");
            assert_eq!(
                supertonic[index as usize].name.to_lowercase(),
                name,
                "the Supertonic voice `{name}` must resolve inside the Supertonic catalogue"
            );
        }
    }

    // ============================================================================
    // Test 4: The provider-scoped default voice still resolves as specified
    // ============================================================================

    #[test]
    fn provider_scoped_default_voice_resolves_as_specified() {
        assert_eq!(
            crate::services::tts::voice::default_voice_id_for_provider("kokoro"),
            Some("bella"),
            "Kokoro's default voice must be Bella"
        );
        assert_eq!(
            crate::services::tts::voice::default_voice_id_for_provider("supertonic"),
            Some("james"),
            "Supertonic's default voice must be James"
        );
        assert_eq!(
            crate::services::tts::voice::default_voice_id_for_provider("edge_tts"),
            None,
            "a provider without a bundled catalogue has no default catalogue voice"
        );

        // A foreign stored id resolves to the active provider's default, not to a
        // catalogue index.
        let kokoro = resolve_voice_index_for_provider("kokoro", "james")
            .expect("Kokoro is a known provider id");
        let kokoro_profiles = crate::services::tts::voice::get_voice_profiles();
        assert_eq!(
            kokoro_profiles[kokoro as usize].name, "Bella",
            "a Supertonic voice id under Kokoro must resolve to Kokoro's default"
        );

        let supertonic = resolve_voice_index_for_provider("supertonic", "bella")
            .expect("Supertonic is a known provider id");
        let supertonic_profiles = crate::services::tts::voice::get_supertonic_voice_profiles();
        assert_eq!(
            supertonic_profiles[supertonic as usize].name, "James",
            "a Kokoro voice id under Supertonic must resolve to Supertonic's default"
        );
    }

    // ============================================================================
    // Test 5: The canonical enum is the only provider vocabulary
    // ============================================================================

    #[test]
    fn factory_provider_ids_match_the_canonical_enum() {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for provider_id in FACTORY_PROVIDER_IDS {
            let parsed: TtsActiveProvider =
                serde_json::from_value(serde_json::Value::String(provider_id.to_string()))
                    .unwrap_or_else(|error| {
                        panic!("`{provider_id}` must be a canonical provider id: {error}")
                    });
            assert!(
                seen.insert(format!("{parsed:?}")),
                "each provider id must map to its own canonical variant, `{provider_id}` does not"
            );
        }
        assert_eq!(
            seen.len(),
            FACTORY_PROVIDER_IDS.len(),
            "every provider id must map to a distinct canonical variant"
        );
    }
}
