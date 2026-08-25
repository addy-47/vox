use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner};
use crate::services::stt::SttCommand;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Decodes a WAV file to mono f32 samples.
fn decode_wav_to_mono_f32(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV '{}': {}", path.display(), e))?;
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

    Ok(mono)
}

/// Resolves the filesystem path for a designated QA test clip.
fn resolve_clip_path(clip_id: &str) -> Result<std::path::PathBuf, String> {
    let filename = match clip_id {
        "short" => "sample_short.wav",
        "medium" => "sample_medium.wav",
        "long" => "sample_long.wav",
        "question" => "sample_question.wav",
        other => return Err(format!("Unknown clip_id: '{}'", other)),
    };

    let candidate_dirs = [
        crate::utils::paths::get().models.join("test_clips"),
        crate::utils::paths::cache_dir().join("test_clips"),
        std::path::PathBuf::from("test_clips"),
    ];

    for dir in &candidate_dirs {
        let candidate = dir.join(filename);
        if candidate.exists() {
            return Ok(candidate);
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

    crate::services::audio::start_audio_engine(&app, &state).await?;
    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    state.pipeline.is_engaged.store(true, Ordering::Relaxed);

    let engine_lock = state.engine.lock().await;
    let engine = engine_lock
        .as_ref()
        .ok_or_else(|| "Engine failed to start after launch".to_string())?;

    let turn_id = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
    let _ = engine.pipeline_tx.send(VoxEvent::WarmUp);
    let _ = engine.pipeline_tx.send(VoxEvent::SpeechStart { turn_id });

    engine
        .stt_tx
        .send(SttCommand::Final(turn_id, audio))
        .map_err(|e| format!("STT channel closed: {}", e))?;

    log::info!("[TestClip] Injected turn_id={} into pipeline", turn_id);
    Ok(())
}

/// Cancels a running test clip and resets the speech recognition stream.
#[tauri::command]
pub async fn test_clip_cancel(
    _app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.pipeline.is_engaged.store(false, Ordering::Relaxed);

    if let Some(engine) = state.engine.lock().await.as_ref() {
        let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);
        let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id });
        let _ = engine.stt_tx.send(SttCommand::ResetStream);
    }

    log::info!("[TestClip] Test clip execution cancelled");
    Ok(())
}
