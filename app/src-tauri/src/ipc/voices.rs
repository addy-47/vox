//! IPC commands for the voice library.
//!
//! All DB operations run on `spawn_blocking` threads — same pattern as
//! `ipc/history.rs`. No AppState involvement; voices are a standalone
//! persistence concern separate from the pipeline.

use crate::persistence::voices::{self, VoiceEntry};
use crate::services::tts::providers::TtsProvider;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── DTO ─────────────────────────────────────────────────────────────────────

/// Frontend-safe representation of a voice entry.
#[derive(Debug, Clone, Serialize)]
pub struct VoiceEntryDto {
    pub id: String,
    pub name: String,
    pub source_kind: String,
    /// True if a preview WAV has been synthesized for this voice.
    pub has_preview: bool,
    pub created_at: i64,
}

impl From<VoiceEntry> for VoiceEntryDto {
    fn from(e: VoiceEntry) -> Self {
        Self {
            has_preview: e.preview_wav.is_some(),
            id: e.id,
            name: e.name,
            source_kind: e.source_kind,
            created_at: e.created_at,
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn open_db() -> Result<turso::Connection, String> {
    let db_path = crate::utils::paths::db_path();
    crate::persistence::db::VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Validates a WAV file via hound:
/// - Readable as WAV
/// - Duration ≥ min_duration_secs
/// Returns (sample_rate, duration_secs) on success.
#[tauri::command]
pub async fn validate_wav(path: String, min_duration_secs: f32) -> Result<(u32, f32), String> {
    tokio::task::spawn_blocking(move || {
        let reader =
            hound::WavReader::open(&path).map_err(|e| format!("Cannot read WAV file: {}", e))?;
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
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))?
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Returns all saved voices ordered by creation date (newest first).
#[tauri::command]
pub async fn list_voices() -> Result<Vec<VoiceEntryDto>, String> {
    let conn = open_db().await?;
    voices::list_voices(&conn)
        .await
        .map(|entries| entries.into_iter().map(VoiceEntryDto::from).collect())
        .map_err(|e| format!("Failed to list voices: {}", e))
}

/// Adds a new cloned voice from an existing audio file.
///
/// Steps:
/// 1. Validate WAV (≥5s duration)
/// 2. Generate UUID, create `~/.vox/voices/{uuid}/` dir
/// 3. Copy source to `~/.vox/voices/{uuid}/source.wav`
///
/// Decodes any supported audio format (MP3, FLAC, M4A, WAV, etc.) to a standard mono f32 24kHz WAV file.
/// If duration exceeds 30.0 seconds, it truncates the input audio.
fn convert_and_validate_audio(src_path: &str, dest_path: &std::path::Path) -> Result<(), String> {
    use crate::symphonia_core::audio::Audio;
    use crate::symphonia_core::codecs::audio::AudioDecoderOptions;
    use crate::symphonia_core::errors::Error;
    use crate::symphonia_core::formats::probe::Hint;
    use crate::symphonia_core::formats::FormatOptions;
    use crate::symphonia_core::io::MediaSourceStream;
    use crate::symphonia_core::meta::MetadataOptions;
    use std::fs::File;

    let file = File::open(src_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(src_path)
        .extension()
        .and_then(|os| os.to_str())
    {
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
        _ => return Err("Track is not an audio track or has no codec parameters".to_string()),
    };

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|e| format!("Failed to initialize decoder: {}", e))?;

    let track_id = track.id;
    let input_sample_rate = codec_params.sample_rate.unwrap_or(24000);

    let mut raw_samples = Vec::new();

    // Decode loop
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
                log::warn!("Decode error: {}, skipping packet", err);
                continue;
            }
            Err(e) => return Err(format!("Decoder error: {}", e)),
        };

        // Convert decoded samples to f32 and mix channels down to mono
        use crate::symphonia_core::audio::GenericAudioBufferRef;
        match decoded {
            GenericAudioBufferRef::F32(buf) => {
                let channels = buf.spec().channels().count();
                let frames = buf.frames();
                for f in 0..frames {
                    let mut sum = 0.0;
                    for c in 0..channels {
                        sum += buf[c][f];
                    }
                    raw_samples.push(sum / channels as f32);
                }
            }
            GenericAudioBufferRef::U8(buf) => {
                let channels = buf.spec().channels().count();
                let frames = buf.frames();
                for f in 0..frames {
                    let mut sum = 0.0;
                    for c in 0..channels {
                        let val = buf[c][f] as f32 / 128.0 - 1.0;
                        sum += val;
                    }
                    raw_samples.push(sum / channels as f32);
                }
            }
            GenericAudioBufferRef::U16(buf) => {
                let channels = buf.spec().channels().count();
                let frames = buf.frames();
                for f in 0..frames {
                    let mut sum = 0.0;
                    for c in 0..channels {
                        let val = buf[c][f] as f32 / 32768.0 - 1.0;
                        sum += val;
                    }
                    raw_samples.push(sum / channels as f32);
                }
            }
            GenericAudioBufferRef::S16(buf) => {
                let channels = buf.spec().channels().count();
                let frames = buf.frames();
                for f in 0..frames {
                    let mut sum = 0.0;
                    for c in 0..channels {
                        let val = buf[c][f] as f32 / 32768.0;
                        sum += val;
                    }
                    raw_samples.push(sum / channels as f32);
                }
            }
            GenericAudioBufferRef::S32(buf) => {
                let channels = buf.spec().channels().count();
                let frames = buf.frames();
                for f in 0..frames {
                    let mut sum = 0.0;
                    for c in 0..channels {
                        let val = buf[c][f] as f32 / 2147483648.0;
                        sum += val;
                    }
                    raw_samples.push(sum / channels as f32);
                }
            }
            _ => return Err("Unsupported sample buffer type".to_string()),
        }
    }

    if raw_samples.is_empty() {
        return Err("Decoded audio contains no samples".to_string());
    }

    // Resample to 24000 Hz if necessary
    let resampled = if input_sample_rate != 24000 {
        log::info!(
            "[Voices] Resampling from {}Hz to 24000Hz",
            input_sample_rate
        );
        let mut out = Vec::new();
        let ratio = input_sample_rate as f64 / 24000.0;
        let num_resampled_samples = (raw_samples.len() as f64 / ratio) as usize;
        for i in 0..num_resampled_samples {
            let src_idx = i as f64 * ratio;
            let idx_floor = src_idx.floor() as usize;
            let idx_ceil = (idx_floor + 1).min(raw_samples.len() - 1);
            let frac = src_idx - idx_floor as f64;
            let sample =
                (1.0 - frac) as f32 * raw_samples[idx_floor] + frac as f32 * raw_samples[idx_ceil];
            out.push(sample);
        }
        out
    } else {
        raw_samples
    };

    let duration_secs = resampled.len() as f32 / 24000.0;
    if duration_secs < 1.0 {
        return Err(format!(
            "Audio too short ({:.1}s). Minimum is 1.0s for voice cloning.",
            duration_secs
        ));
    }

    // Auto-stitch/loop short audio to reach exactly 30.0 seconds, or truncate if longer
    let final_samples = if duration_secs < 30.0 {
        log::info!(
            "[Voices] Audio is only {:.1}s — auto-stitching to 30.0s for better cloning",
            duration_secs
        );
        resampled
            .iter()
            .copied()
            .cycle()
            .take(30 * 24000)
            .collect::<Vec<f32>>()
    } else if duration_secs > 30.0 {
        log::info!(
            "[Voices] Truncating audio from {:.1}s to 30.0s",
            duration_secs
        );
        resampled[0..(30 * 24000)].to_vec()
    } else {
        resampled
    };

    // Write final samples to destination WAV (24000Hz mono float32)
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 24000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(dest_path, spec)
        .map_err(|e| format!("Failed to create destination WAV writer: {}", e))?;
    for sample in final_samples {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}

/// Helper that pre-bakes speaker embeddings using a temporary cold Chatterbox engine.
fn pre_bake_voice(source_wav: &std::path::Path, baked_dir: &std::path::Path) -> Result<(), String> {
    use chatterbox_rs::{Engine, EngineOptions};

    std::fs::create_dir_all(baked_dir)
        .map_err(|e| format!("Failed to create baked voice directory: {}", e))?;

    let tts_model_dir = crate::utils::paths::model_dir("tts").join("chatterbox");
    let t3_path = if tts_model_dir.join("t3-q4_0.gguf").exists() {
        tts_model_dir.join("t3-q4_0.gguf")
    } else {
        tts_model_dir.join("chatterbox-t3-mtl-q4_0.gguf")
    };
    let s3_path = if tts_model_dir.join("s3gen-f16.gguf").exists() {
        tts_model_dir.join("s3gen-f16.gguf")
    } else {
        tts_model_dir.join("chatterbox-s3gen-mtl-f16.gguf")
    };

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
        "[Voices] Speaker tensors successfully pre-baked to {:?}",
        baked_dir
    );
    Ok(())
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Adds a new cloned voice from an existing audio file.
///
/// Steps:
/// 1. Decode and validate audio (WAV, MP3, FLAC, M4A, etc.) -> 24kHz mono Float32 WAV
/// 2. Check 10s-30s limits and truncate if needed
/// 3. Pre-bake embeddings (Option B speaker tensors)
/// 4. Save voice entry to DB
#[tauri::command]
pub async fn add_voice_from_file(name: String, file_path: String) -> Result<VoiceEntryDto, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Voice name cannot be empty".to_string());
    }

    // Create voice directory
    let id = uuid::Uuid::new_v4().to_string();
    let voice_dir = crate::utils::paths::voice_dir(&id);
    std::fs::create_dir_all(&voice_dir)
        .map_err(|e| format!("Failed to create voice directory: {}", e))?;

    // 1. Decode to WAV and validate duration limits
    let dest = voice_dir.join("source.wav");
    let file_path_clone = file_path.clone();
    let dest_clone = dest.clone();
    tokio::task::spawn_blocking(move || convert_and_validate_audio(&file_path_clone, &dest_clone))
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;

    // 2. Pre-bake speaker tensors (Option B)
    let baked_dir = voice_dir.join("baked");
    let dest_clone2 = dest.clone();
    let baked_dir_clone = baked_dir.clone();
    tokio::task::spawn_blocking(move || pre_bake_voice(&dest_clone2, &baked_dir_clone))
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;

    // 3. Insert into DB
    let entry = VoiceEntry {
        id: id.clone(),
        name: name.clone(),
        source_kind: "pre_baked".to_string(),
        wav_path: Some(dest.to_string_lossy().into_owned()),
        voice_dir: Some(baked_dir.to_string_lossy().into_owned()),
        created_at: now_epoch(),
        preview_wav: None,
    };

    let conn = open_db().await?;
    voices::insert_voice(&conn, &entry)
        .await
        .map_err(|e| format!("Failed to save voice: {}", e))?;

    log::info!(
        "[Voices] Added voice '{}' (id={}) with pre-baked tensors",
        name,
        id
    );
    Ok(VoiceEntryDto::from(entry))
}

/// Adds a new cloned voice from raw PCM audio captured in-app.
///
/// The frontend passes 32-bit float samples with the capture sample rate.
/// This fn writes them as a WAV file, pre-bakes, and inserts it.
#[tauri::command]
pub async fn add_voice_from_recording(
    name: String,
    pcm_f32: Vec<f32>,
    sample_rate: u32,
) -> Result<VoiceEntryDto, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Voice name cannot be empty".to_string());
    }
    if sample_rate == 0 {
        return Err("Invalid sample rate (0)".to_string());
    }

    let duration = pcm_f32.len() as f32 / sample_rate as f32;
    if duration < 1.0 {
        return Err(format!(
            "Recording too short ({:.1}s). Minimum is 1.0s for voice cloning.",
            duration
        ));
    }

    // Create voice directory
    let id = uuid::Uuid::new_v4().to_string();
    let voice_dir = crate::utils::paths::voice_dir(&id);
    std::fs::create_dir_all(&voice_dir)
        .map_err(|e| format!("Failed to create voice directory: {}", e))?;

    let dest = voice_dir.join("source.wav");
    let dest_clone = dest.clone();
    let pcm_f32_clone = pcm_f32.clone();

    // 1. Write PCM as WAV
    let final_samples_len = tokio::task::spawn_blocking(move || {
        let final_samples = if duration < 30.0 {
            log::info!(
                "[Voices] Recording is only {:.1}s — auto-stitching to 30.0s for better cloning",
                duration
            );
            let limit = (30.0 * sample_rate as f32) as usize;
            pcm_f32_clone
                .iter()
                .copied()
                .cycle()
                .take(limit)
                .collect::<Vec<f32>>()
        } else if duration > 30.0 {
            log::info!(
                "[Voices] Truncating recording from {:.1}s to 30.0s",
                duration
            );
            let limit = (30.0 * sample_rate as f32) as usize;
            pcm_f32_clone[0..limit].to_vec()
        } else {
            pcm_f32_clone
        };

        // Write PCM as WAV (f32 mono)
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&dest_clone, spec)
            .map_err(|e| format!("Failed to create WAV writer: {}", e))?;
        for sample in &final_samples {
            writer
                .write_sample(*sample)
                .map_err(|e| format!("Failed to write sample: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

        Ok::<usize, String>(final_samples.len())
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))??;

    // 2. Pre-bake speaker tensors
    let baked_dir = voice_dir.join("baked");
    let dest_clone2 = dest.clone();
    let baked_dir_clone = baked_dir.clone();
    tokio::task::spawn_blocking(move || pre_bake_voice(&dest_clone2, &baked_dir_clone))
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;

    // 3. Insert into DB
    let entry = VoiceEntry {
        id: id.clone(),
        name: name.clone(),
        source_kind: "pre_baked".to_string(),
        wav_path: Some(dest.to_string_lossy().into_owned()),
        voice_dir: Some(baked_dir.to_string_lossy().into_owned()),
        created_at: now_epoch(),
        preview_wav: None,
    };

    let conn = open_db().await?;
    voices::insert_voice(&conn, &entry)
        .await
        .map_err(|e| format!("Failed to save voice: {}", e))?;

    log::info!(
        "[Voices] Added voice from recording '{}' (id={}, {:.1}s @ {}Hz) with pre-baked tensors",
        name,
        id,
        final_samples_len as f32 / sample_rate as f32,
        sample_rate
    );
    Ok(VoiceEntryDto::from(entry))
}

/// Deletes a voice entry and removes all associated files from disk.
///
/// If the deleted voice is currently selected in settings, the caller
/// (frontend) must prompt for a settings restart to fall back to built-in.
#[tauri::command]
pub async fn delete_voice(id: String) -> Result<(), String> {
    let conn = open_db().await?;

    // Fetch first so we know the directory path
    let entry = voices::get_voice(&conn, &id)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Voice not found: {}", id))?;

    // Delete DB row
    voices::delete_voice(&conn, &id)
        .await
        .map_err(|e| format!("Failed to delete voice from DB: {}", e))?;

    // Remove voice directory (blocking thread pool is fine for filesystem IO)
    let voice_dir = crate::utils::paths::voice_dir(&entry.id);
    if voice_dir.exists() {
        tokio::task::spawn_blocking(move || {
            std::fs::remove_dir_all(&voice_dir)
                .map_err(|e| format!("Failed to remove voice files: {}", e))
        })
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;
    }

    log::info!("[Voices] Deleted voice '{}' (id={})", entry.name, id);
    Ok(())
}

/// Renames a voice entry. Display-only change, no file system impact.
#[tauri::command]
pub async fn rename_voice(id: String, name: String) -> Result<(), String> {
    let name = name.trim().to_string();
    let conn = open_db().await?;
    voices::rename_voice(&conn, &id, &name)
        .await
        .map_err(|e| format!("Failed to rename voice: {}", e))?;
    log::info!("[Voices] Renamed voice {} to '{}'", id, name);
    Ok(())
}

/// Synthesizes a short preview clip using the specified cloned voice.
///
/// Creates a cold Chatterbox engine with the voice's reference audio and
/// synthesizes "Hello, I'm your Vox assistant." (~2s). The preview WAV is
/// written to `~/.vox/voices/{uuid}/preview.wav` and its path is returned.
///
/// ⚠️ This is slow on CPU (~8–15s). The frontend MUST show a spinner and
/// allow cancellation. The ENGINE_INIT_MUTEX serializes this against the
/// active TTS worker.
#[tauri::command]
pub async fn preview_voice(id: String) -> Result<String, String> {
    let conn = open_db().await?;
    let entry = voices::get_voice(&conn, &id)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Voice not found: {}", id))?;

    let wav_path = entry
        .wav_path
        .as_deref()
        .ok_or("Voice has no source WAV")?
        .to_string();

    if !std::path::Path::new(&wav_path).exists() {
        return Err(format!("Source WAV not found: {}", wav_path));
    }

    let chatterbox_path = crate::utils::paths::model_dir("tts").join("chatterbox");
    if !chatterbox_path.exists() {
        return Err("Chatterbox model not installed".to_string());
    }

    log::info!("[Voices] Generating preview for voice {} ...", id);

    let preview_path = crate::utils::paths::voice_dir(&id).join("preview.wav");
    let preview_path_clone = preview_path.clone();
    tokio::task::spawn_blocking(move || {
        use crate::services::tts::ChatterboxEngine;
        let engine = ChatterboxEngine::new(
            &chatterbox_path,
            "en",
            8,   // quality_steps — balanced
            1.0, // normal speed
            Some(&wav_path),
        )
        .map_err(|e| format!("Failed to create preview engine: {}", e))?;

        // Synthesize the preview text
        let preview_text = "Hello, I'm your Vox assistant.";
        let (tx, rx) = std::sync::mpsc::channel::<crate::core::events::VoxEvent>();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        engine
            .synthesize_chunk(preview_text, 0, cancel, tx)
            .map_err(|e| format!("Preview synthesis failed: {}", e))?;

        // Collect PCM samples from TtsChunk events
        let mut pcm: Vec<f32> = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let crate::core::events::VoxEvent::TtsChunk { samples, .. } = event {
                pcm.extend_from_slice(&samples);
            }
        }

        if pcm.is_empty() {
            return Err("Preview synthesis produced no audio".to_string());
        }

        // Write preview WAV (f32 mono 24kHz — Chatterbox native output rate)
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 24000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&preview_path_clone, spec)
            .map_err(|e| format!("Failed to create preview WAV: {}", e))?;
        for sample in &pcm {
            writer
                .write_sample(*sample)
                .map_err(|e| format!("Failed to write preview sample: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize preview WAV: {}", e))?;

        Ok(())
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))??;

    // Update DB with preview path
    let path_str = preview_path.to_string_lossy().into_owned();
    voices::update_preview_wav(&conn, &id, &path_str)
        .await
        .map_err(|e| format!("Failed to record preview path: {}", e))?;

    log::info!("[Voices] Preview generated for voice {}: {}", id, path_str);
    Ok(path_str)
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

#[tauri::command]
pub async fn start_backend_recording() -> Result<(), String> {
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
            |err| {
                log::error!("Error in recording stream: {}", err);
            },
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

    log::info!(
        "[Voices] Backend recording started successfully at {} Hz",
        sample_rate
    );
    Ok(())
}

#[tauri::command]
pub async fn stop_backend_recording() -> Result<(Vec<f32>, u32), String> {
    let mut recorder_guard = ACTIVE_RECORDER.lock();
    let recorder = recorder_guard
        .take()
        .ok_or_else(|| "No active recording found to stop".to_string())?;

    // Drop the recorder stream to stop recording immediately
    drop(recorder._stream);

    let samples = recorder.samples.lock().clone();
    let sample_rate = recorder.sample_rate;

    log::info!(
        "[Voices] Backend recording stopped. Captured {} samples at {} Hz ({:.2}s)",
        samples.len(),
        sample_rate,
        samples.len() as f32 / sample_rate as f32
    );

    Ok((samples, sample_rate))
}

// ─── Edge TTS Voice DTO & IPC Command ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct EdgeTtsVoiceDto {
    pub name: String,
    pub short_name: String,
    pub gender: String,
    pub locale: String,
    pub friendly_name: String,
}

#[tauri::command]
pub async fn fetch_edge_tts_voices() -> Result<Vec<EdgeTtsVoiceDto>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let token = crate::services::tts::providers::edge_tts::get_trusted_client_token();
    let url = format!("https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/voices/list?trustedclienttoken={}", token);
    let resp = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0")
        .send()
        .await
        .map_err(|e| format!("Network error: Cannot reach Edge TTS voices endpoint. Internet connection required: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Edge TTS service error: HTTP {}", resp.status()));
    }

    #[derive(serde::Deserialize)]
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

    let voices = raw_voices
        .into_iter()
        .map(|v| EdgeTtsVoiceDto {
            name: v.name,
            short_name: v.short_name,
            gender: v.gender,
            locale: v.locale,
            friendly_name: v.friendly_name,
        })
        .collect();

    Ok(voices)
}
