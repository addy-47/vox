use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Runtime};

use crate::core::error::VoxIpcError;
use crate::core::events::VoxEvent;
use crate::core::settings::PipelineMode;
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::pipeline::{transition, RoutingContext};
use crate::services::stt::SttCommand;

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
fn decode_wav_to_mono_f32(path: &Path) -> Result<Vec<f32>, VoxIpcError> {
    let mut reader = hound::WavReader::open(path).map_err(|e| {
        VoxIpcError::InvalidArgument(format!("Failed to open WAV '{}': {}", path.display(), e))
    })?;
    let spec = reader.spec();

    if spec.sample_rate == 0 {
        return Err(VoxIpcError::InvalidArgument(format!(
            "Invalid WAV '{}': sample rate cannot be 0",
            path.display()
        )));
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
fn resolve_clip_path(clip_id: &str) -> Result<PathBuf, VoxIpcError> {
    if clip_id.contains('/') || clip_id.contains('\\') || clip_id.contains("..") {
        return Err(VoxIpcError::InvalidArgument(
            "Invalid clip ID: directory traversal not permitted".into(),
        ));
    }

    let filename = if clip_id.ends_with(".wav") {
        clip_id.to_string()
    } else {
        format!("{}.wav", clip_id)
    };

    let candidate_dirs = [
        PathBuf::from("test-clips"),
        PathBuf::from("app/src-tauri/test-clips"),
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

    Err(VoxIpcError::NotFound(format!(
        "Clip '{}' not found in test_clips paths",
        filename
    )))
}

/// Ensures the audio engine is running and working memory context is initialized.
async fn ensure_test_pipeline_ready<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) -> Result<(), VoxIpcError> {
    if let Err(e) = crate::core::start_audio_engine(app, state).await {
        return Err(VoxIpcError::Engine(e));
    }

    let prompt = {
        let settings = state.settings.read().unwrap_or_else(|p| p.into_inner());
        match ctx.pipeline_mode {
            PipelineMode::Modular => settings.persona.modular_prompt.clone(),
            PipelineMode::Realtime => settings.persona.realtime_prompt.clone(),
        }
    };
    crate::pipeline::init_new_session_sync(state, &prompt);

    if ctx.pipeline_mode == PipelineMode::Modular {
        if let Err(e) = crate::core::engine::ensure_modular_workers_sync(app, state) {
            return Err(VoxIpcError::Engine(format!(
                "Failed to arm modular workers: {}",
                e
            )));
        }
    }

    Ok(())
}

/// Executes a pre-recorded QA test clip treating it as a live user speech utterance.
pub async fn execute_test_clip<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    clip_id: &str,
) -> Result<(), VoxIpcError> {
    let clip_path = resolve_clip_path(clip_id)?;
    let audio = decode_wav_to_mono_f32(&clip_path)?;

    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    state
        .telemetry
        .is_private_mode
        .store(true, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(state);
    ensure_test_pipeline_ready(app, state, &ctx).await?;

    let (turn_id, _) = state.pipeline.next_turn();
    state.pipeline_accumulator.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if let Err(e) = engine.pipeline_tx.send(VoxEvent::SpeechStart) {
                log::warn!("[Pipeline::Test] Failed to send SpeechStart event: {}", e);
            }
        }
    }

    transition(InteractionState::Thinking, &ctx, app, state);

    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            let engine_lock = state.engine.lock().await;
            let engine = engine_lock.as_ref().ok_or_else(|| {
                VoxIpcError::Engine("Engine unallocated after launch".to_string())
            })?;
            if let Err(e) = engine.stt_tx.send(SttCommand::Final(turn_id, audio)) {
                return Err(VoxIpcError::Engine(format!("STT channel closed: {}", e)));
            }
        }
        PipelineMode::Realtime => {
            let i16_samples: Vec<i16> = audio
                .iter()
                .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect();
            let mut rt_guard = state.realtime_engine.lock().await;
            if let Some(ref mut rt_actor) = *rt_guard {
                if let Err(e) = rt_actor.signal_speech_committed(&i16_samples) {
                    return Err(VoxIpcError::Engine(format!(
                        "Failed to commit test audio to realtime provider: {}",
                        e
                    )));
                }
            } else {
                return Err(VoxIpcError::Engine(
                    "Realtime actor unavailable for test injection".to_string(),
                ));
            }
        }
    }

    log::info!(
        "[Pipeline::Test] Injected test clip '{}' into pipeline (turn: {}, mode: {:?})",
        clip_id,
        turn_id,
        ctx.pipeline_mode
    );
    Ok(())
}

/// Cancels a running test clip turn, silences playback, and resets speech stream state.
pub async fn cancel_test_clip<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), VoxIpcError> {
    state.pipeline.turn_token().cancel();
    state.pipeline.rearm_turn_token();
    state.pipeline_accumulator.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            let turn_id = state.pipeline.peek_turn_id();
            if let Err(e) = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id }) {
                log::warn!("[Pipeline::Test] Failed to send Cancelled event: {}", e);
            }
            if let Err(e) = engine.stt_tx.send(SttCommand::ResetStream) {
                log::warn!("[Pipeline::Test] Failed to send ResetStream command: {}", e);
            }
        }
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);

    log::info!("[Pipeline::Test] Test clip execution cancelled -> Ready");
    Ok(())
}
