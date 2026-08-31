use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner};
use crate::services::stt::SttCommand;
use std::borrow::Cow;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Resamples audio samples linearly from source sample rate to 16kHz for STT.
fn resample_to_16k(samples: &[f32], source_rate: u32) -> Cow<'_, [f32]> {
    if source_rate == 0 || samples.is_empty() {
        return Cow::Borrowed(&[]);
    }
    if source_rate == 16000 {
        return Cow::Borrowed(samples);
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
    Cow::Owned(out)
}

/// Decodes a WAV file to mono f32 samples resampled to 16kHz.
fn decode_wav_to_mono_f32(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV '{}': {}", path.display(), e))?;
    let spec = reader.spec();

    if spec.sample_rate == 0 {
        return Err(format!(
            "Invalid WAV '{}': sample rate cannot be 0",
            path.display()
        ));
    }

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

    let resampled = resample_to_16k(&mono, spec.sample_rate).into_owned();
    Ok(resampled)
}

/// Resolves the filesystem path for a designated QA test clip.
fn resolve_clip_path(clip_id: &str) -> Result<std::path::PathBuf, String> {
    if clip_id.contains('/') || clip_id.contains('\\') || clip_id.contains("..") {
        return Err("Invalid clip ID: directory traversal not permitted".into());
    }

    let filename = if clip_id.ends_with(".wav") {
        clip_id.to_string()
    } else {
        format!("{}.wav", clip_id)
    };

    let candidate_dirs = [
        std::path::PathBuf::from("test-clips"),
        std::path::PathBuf::from("app/src-tauri/test-clips"),
        crate::utils::paths::get().models.join("test_clips"),
        crate::utils::paths::get().models.join("test-clips"),
        crate::utils::paths::cache_dir().join("test_clips"),
        crate::utils::paths::cache_dir().join("test-clips"),
    ];

    for dir in &candidate_dirs {
        let candidate = dir.join(&filename);
        if let Ok(canon) = candidate.canonicalize() {
            if let Ok(canon_dir) = dir.canonicalize() {
                if canon.starts_with(&canon_dir) && canon.exists() {
                    return Ok(canon);
                }
            }
        }
    }

    Err(format!("Clip '{}' not found in test_clips paths", filename))
}

/// Injects a pre-recorded audio clip directly into the STT engine for automated testing.
#[tauri::command]
pub async fn test_clip(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    clip_id: String,
) -> Result<(), String> {
    let clip_path = resolve_clip_path(&clip_id)?;
    let audio = decode_wav_to_mono_f32(&clip_path)?;

    if let Err(e) = crate::core::start_audio_engine(&app, &state).await {
        state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
        return Err(e);
    }

    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

    let engine_lock = state.engine.lock().await;
    let engine = match engine_lock.as_ref() {
        Some(e) => e,
        None => {
            state
                .owner
                .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
            return Err("Engine failed to start after launch".to_string());
        }
    };

    let turn_id = state.pipeline.next_turn_id();
    if let Err(e) = engine.pipeline_tx.send(VoxEvent::SpeechStart { turn_id }) {
        log::warn!("[TestClip] Failed to send SpeechStart event: {}", e);
    }

    if let Err(e) = engine.stt_tx.send(SttCommand::Final(turn_id, audio)) {
        state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
        return Err(format!("STT channel closed: {}", e));
    }

    log::info!("[TestClip] Injected turn_id={} into pipeline", turn_id);
    Ok(())
}

/// Cancels a running test clip and resets the speech recognition stream.
#[tauri::command]
pub async fn test_clip_cancel(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.pipeline.renew_turn_token();
    state
        .owner
        .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);

    if let Some(engine) = state.engine.lock().await.as_ref() {
        let turn_id = state.pipeline.peek_turn_id();
        if let Err(e) = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id }) {
            log::warn!("[TestClip] Failed to send Cancelled event: {}", e);
        }
        if let Err(e) = engine.stt_tx.send(SttCommand::ResetStream) {
            log::warn!("[TestClip] Failed to send ResetStream command: {}", e);
        }
    }

    log::info!("[TestClip] Test clip execution cancelled");
    Ok(())
}
