//! ============================================================================
//! src/ipc/pipeline/test_clip.rs — Audio clip testing and WAV sample decoder
//! ============================================================================

use super::lifecycle::launch_engine;
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner};
use crate::services::stt::SttCommand;
use tauri::{AppHandle, Manager, State};

/// Decode a WAV file to mono f32 samples.
/// Handles both integer and float sample formats. Stereo is averaged to mono.
fn decode_wav_to_mono_f32(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV '{}': {}", path.display(), e))?;
    let spec = reader.spec();

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => {
            // Normalise integer samples to [-1.0, 1.0]
            let max_val = (2u64.pow(spec.bits_per_sample as u32) / 2 - 1) as f64;
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| (s as f64 / max_val) as f32)
                .collect()
        }
    };

    // Average channels to mono
    let mono: Vec<f32> = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };

    log::info!(
        "[TestClip] Decoded WAV: {} samples, {} channels, {} Hz, {} bits",
        mono.len(),
        spec.channels,
        spec.sample_rate,
        spec.bits_per_sample,
    );

    Ok(mono)
}

/// Inject a pre-recorded test clip into the pipeline as if the user spoke it.
///
/// The clip is decoded from a bundled WAV resource, then sent directly to the
/// STT worker as a `SttCommand::Final`, bypassing VAD. If the engine is not
/// running, it is auto-launched first.
#[tauri::command]
pub async fn test_clip(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
    clip_id: String,
) -> Result<(), String> {
    log::info!("[TestClip] Requested: {}", clip_id);

    // 1. Resolve bundled clip path from Tauri resource directory
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?
        .join("test-clips")
        .join(format!("{}.wav", clip_id));

    // Fallback: relative to CARGO_MANIFEST_DIR for dev mode
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test-clips")
        .join(format!("{}.wav", clip_id));

    let clip_path = if resource_path.exists() {
        resource_path
    } else if dev_path.exists() {
        log::info!("[TestClip] Using dev path: {:?}", dev_path);
        dev_path
    } else {
        return Err(format!(
            "Test clip '{}' not found at {:?} or {:?}",
            clip_id, resource_path, dev_path
        ));
    };

    // 2. Decode WAV to mono f32
    let audio = decode_wav_to_mono_f32(&clip_path)?;

    if audio.is_empty() {
        return Err("Decoded audio is empty".to_string());
    }

    log::info!(
        "[TestClip] Decoded {} samples from '{}'",
        audio.len(),
        clip_id
    );

    // 3. Auto-launch engine if not running
    {
        let engine_lock = state.engine.lock().await;
        if engine_lock.is_none() {
            drop(engine_lock);
            log::info!("[TestClip] Engine not running. Launching...");
            launch_engine(app.clone()).await?;
        }
    }

    // 4. Set owner to MainWindow so state_changed / llm_token events route to main window
    state.owner.store(
        InteractionOwner::MainWindow as u32,
        std::sync::atomic::Ordering::Relaxed,
    );
    state
        .pipeline
        .is_engaged
        .store(true, std::sync::atomic::Ordering::Relaxed);

    // Re-acquire engine lock and send clip into the pipeline
    let engine_lock = state.engine.lock().await;
    let engine = engine_lock
        .as_ref()
        .ok_or_else(|| "Engine failed to start after launch".to_string())?;

    let _ = engine
        .vad_tx
        .send(crate::core::state::VadCommand::UpdateOwner(
            InteractionOwner::MainWindow,
        ));

    // Generate a unique turn_id (bump atomic to avoid collision with VAD)
    let turn_id = state
        .pipeline
        .turn_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;

    // WarmUp first — spawns LLM + TTS workers so they're ready when STT finishes
    let _ = engine.pipeline_tx.send(VoxEvent::WarmUp);

    // Emit SpeechStart so the pipeline sets up turn state
    let _ = engine.pipeline_tx.send(VoxEvent::SpeechStart {
        turn_id,
        owner: InteractionOwner::MainWindow,
    });

    // Send the audio as a Final STT command (bypasses VAD completely)
    engine
        .stt_tx
        .send(SttCommand::Final(
            turn_id,
            InteractionOwner::MainWindow,
            audio,
        ))
        .map_err(|e| format!("STT channel closed: {}", e))?;

    log::info!(
        "[TestClip] Injected turn_id={} into pipeline (WarmUp sent)",
        turn_id
    );

    Ok(())
}

/// Cancel a running test clip — flushes the pipeline (cancel flag + Cancelled event + STT ResetStream).
#[tauri::command]
pub async fn test_clip_cancel(
    _app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    log::info!("[TestClip] Cancel requested — flushing pipeline.");

    state
        .pipeline
        .cancel_flag
        .store(true, std::sync::atomic::Ordering::Relaxed);
    state
        .pipeline
        .is_engaged
        .store(false, std::sync::atomic::Ordering::Relaxed);

    if let Some(engine) = state.engine.lock().await.as_ref() {
        let turn_id = state
            .pipeline
            .turn_id
            .load(std::sync::atomic::Ordering::Relaxed);
        let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id });
        let _ = engine.stt_tx.send(SttCommand::ResetStream);
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::UpdateOwner(
                InteractionOwner::Tray,
            ));
    } else {
        log::warn!("[TestClip] No engine to cancel.");
    }

    Ok(())
}
