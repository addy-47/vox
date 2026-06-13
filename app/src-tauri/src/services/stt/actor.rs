use anyhow::Result;
use tauri::{AppHandle, Emitter};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use crate::core::state::InteractionOwner;
use crate::core::events::VoxEvent;

pub enum SttCommand {
    Partial(u32, crate::core::state::InteractionOwner, Vec<f32>),
    Final(u32, crate::core::state::InteractionOwner, Vec<f32>),
    ResetStream,
    Shutdown,
}

fn init_engine(
    engine_type: &str,
    model_path: &std::path::Path,
) -> Result<Box<dyn crate::services::traits::SttEngine>> {
    if engine_type == "nvidia_nemotron" {
        let eng = super::nemotron_onnx::SttEngine::new(model_path)?;
        Ok(Box::new(eng))
    } else {
        let eng = super::qwen_onnx::SttEngine::new(model_path)?;
        Ok(Box::new(eng))
    }
}

pub fn spawn_stt_worker(
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<SttCommand>,
    model_path: std::path::PathBuf,
    engine_type: String,
    pipeline_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
    engine_shutdown: Arc<AtomicBool>,
    pre_load: bool,
) -> Result<std::thread::JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("vox-stt-worker".to_string())
        .spawn(move || {
        use thread_priority::*;
        if let Err(e) = set_current_thread_priority(ThreadPriority::Crossplatform(ThreadPriorityValue::try_from(80u8).unwrap())) {
            log::warn!("[STT] Failed to set high priority: {:?}", e);
        }

        log::info!("[STT] >>> Dedicated worker thread started with engine type: {}.", engine_type);
        
        let mut engine: Option<Box<dyn crate::services::traits::SttEngine>> = if pre_load {
            app.emit(crate::core::constants::EVENT_MODEL_LOADING, "STT").ok();
            match init_engine(&engine_type, &model_path) {
                Ok(e) => {
                    is_loaded.store(true, std::sync::atomic::Ordering::Relaxed);
                    app.emit(crate::core::constants::EVENT_MODEL_READY, "STT").ok();
                    Some(e)
                },
                Err(err) => {
                    log::error!("[STT] CRITICAL: Failed to initialize engine: {}", err);
                    app.emit(crate::core::constants::EVENT_MODEL_FAILED, format!("STT: {}", err)).ok();
                    return;
                }
            }
        } else {
            log::info!("[STT] Lazy loading enabled. Waiting for engagement/audio to load engine.");
            None
        };

        let mut last_emit_time = Instant::now();
        let mut last_transcript = String::new();
        let mut stitched_transcript = String::new();
        let mut current_active_turn = 0u32;
        let mut last_inference_duration = Duration::from_millis(300);
        let mut pending_cmd = None;

        // Stateful streaming parameters for Nemotron
        let mut processed_samples = 0usize;
        let mut stt_audio_buffer = Vec::<f32>::new();

        loop {
            if engine_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                log::info!("[STT] Engine shutdown flag detected. Exiting loop.");
                break;
            }

            let mut cmd = if let Some(c) = pending_cmd.take() {
                c
            } else {
                match rx.recv_timeout(Duration::from_millis(150)) {
                    Ok(c) => c,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            };

            // Queue-coalescing: If we got a Partial, drain any further sequential Partials to process only the freshest audio
            if let SttCommand::Partial(mut tid, mut owner, mut utterance) = cmd {
                let mut skipped = 0;
                while let Ok(next_cmd) = rx.try_recv() {
                    match next_cmd {
                        SttCommand::Partial(next_tid, next_owner, next_utterance) => {
                            tid = next_tid;
                            owner = next_owner;
                            utterance = next_utterance;
                            skipped += 1;
                        }
                        other => {
                            pending_cmd = Some(other);
                            break;
                        }
                    }
                }
                if skipped > 0 {
                    log::debug!("[STT] Coalesced {} stale partials in queue", skipped);
                }
                cmd = SttCommand::Partial(tid, owner, utterance);
            }

            match cmd {
                SttCommand::Shutdown => {
                    log::info!("[STT] Shutdown signal received. Exiting worker thread.");
                    break;
                }
                SttCommand::Partial(tid, owner, utterance) => {
                    if tid != current_active_turn {
                        log::info!("[STT] New turn ID {} detected (prev {}). Resetting buffers.", tid, current_active_turn);
                        current_active_turn = tid;
                        stitched_transcript.clear();
                        last_transcript.clear();
                        processed_samples = 0;
                        stt_audio_buffer.clear();
                        if let Some(ref eng) = engine {
                            let _ = eng.reset_state();
                        }
                    }

                    let dynamic_throttle = last_inference_duration.max(Duration::from_millis(300));
                    if last_emit_time.elapsed() >= dynamic_throttle {
                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            stitched_transcript.clear();
                            last_transcript.clear();
                            stt_audio_buffer.clear();
                            processed_samples = 0;
                            continue;
                        }

                        if engine.is_none() {
                            app.emit(crate::core::constants::EVENT_MODEL_LOADING, "STT").ok();
                            match init_engine(&engine_type, &model_path) {
                                Ok(e) => {
                                    is_loaded.store(true, std::sync::atomic::Ordering::Relaxed);
                                    app.emit(crate::core::constants::EVENT_MODEL_READY, "STT").ok();
                                    engine = Some(e);
                                }
                                Err(e) => {
                                    log::error!("[STT] Lazy load failed: {}", e);
                                    app.emit(crate::core::constants::EVENT_MODEL_FAILED, format!("STT: {}", e)).ok();
                                    continue;
                                }
                            }
                        }

                        if let Some(ref eng) = engine {
                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                stitched_transcript.clear();
                                last_transcript.clear();
                                stt_audio_buffer.clear();
                                processed_samples = 0;
                                continue;
                            }

                            let start_inference = Instant::now();

                            if engine_type == "nvidia_nemotron" {
                                // Dynamic non-overlapping chunking
                                if processed_samples < utterance.len() {
                                    let new_samples = &utterance[processed_samples..];
                                    stt_audio_buffer.extend_from_slice(new_samples);
                                    processed_samples = utterance.len();
                                }

                                // Stride is 560ms (8960 samples)
                                const STRIDE_SAMPLES: usize = 8960;
                                let mut partial_text = String::new();
                                while stt_audio_buffer.len() >= STRIDE_SAMPLES {
                                    let chunk: Vec<f32> = stt_audio_buffer.drain(..STRIDE_SAMPLES).collect();
                                    match eng.transcribe_chunk(&chunk, false) {
                                        Ok(text) => {
                                            if !text.trim().is_empty() {
                                                partial_text.push_str(&text);
                                            }
                                        }
                                        Err(e) => log::error!("[STT] Nemotron chunk transcribe failed: {}", e),
                                    }
                                }

                                last_inference_duration = start_inference.elapsed();

                                if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                    stitched_transcript.clear();
                                    last_transcript.clear();
                                    stt_audio_buffer.clear();
                                    processed_samples = 0;
                                    continue;
                                }

                                if !partial_text.is_empty() {
                                    stitched_transcript.push_str(&partial_text);
                                }
                            } else {
                                // Qwen sliding window mode
                                let start_idx = utterance.len().saturating_sub(72000);
                                let rolling_utterance = &utterance[start_idx..];

                                match eng.transcribe(rolling_utterance) {
                                    Ok(text) => {
                                        last_inference_duration = start_inference.elapsed();
                                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                            stitched_transcript.clear();
                                            last_transcript.clear();
                                            continue;
                                        }
                                        let raw_partial: String = text;
                                        
                                        if start_idx == 0 {
                                            stitched_transcript = raw_partial;
                                        } else {
                                            stitched_transcript = crate::services::utils::stitch_transcripts(&stitched_transcript, &raw_partial);
                                        }
                                    }
                                    Err(e) => log::error!("[STT] Partial transcription failed: {}", e),
                                }
                            }

                            if !stitched_transcript.is_empty() && stitched_transcript != last_transcript {
                                if let Some(ref pipeline_tx) = pipeline_event_tx {
                                    let _ = pipeline_tx.send(VoxEvent::TranscriptPartial {
                                        turn_id: tid,
                                        owner,
                                        text: stitched_transcript.clone(),
                                    });
                                }
                                last_transcript = stitched_transcript.clone();
                            }
                            last_emit_time = Instant::now();
                        }
                    }
                }
                SttCommand::Final(tid, owner, utterance) => {
                    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        stitched_transcript.clear();
                        last_transcript.clear();
                        stt_audio_buffer.clear();
                        processed_samples = 0;
                        continue;
                    }

                    if tid < current_active_turn {
                        log::info!("[STT] Discarding stale Final from turn {} (active: {})", tid, current_active_turn);
                        stitched_transcript.clear();
                        last_transcript.clear();
                        stt_audio_buffer.clear();
                        processed_samples = 0;
                        continue;
                    }
                    current_active_turn = tid;

                    if engine.is_none() {
                        match init_engine(&engine_type, &model_path) {
                            Ok(e) => {
                                is_loaded.store(true, std::sync::atomic::Ordering::Relaxed);
                                engine = Some(e);
                            }
                            Err(e) => {
                                log::error!("[STT] Lazy load failed: {}", e);
                            }
                        }
                    }

                    if let Some(ref eng) = engine {
                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            stitched_transcript.clear();
                            last_transcript.clear();
                            stt_audio_buffer.clear();
                            processed_samples = 0;
                            continue;
                        }

                        if engine_type == "nvidia_nemotron" {
                            if processed_samples < utterance.len() {
                                let new_samples = &utterance[processed_samples..];
                                stt_audio_buffer.extend_from_slice(new_samples);
                            }

                            // Process remaining audio in 8960-sample stride chunks,
                            // then flush with zero-padded final chunk.
                            // This handles both:
                            //   (a) normal VAD flow — remaining has < 8960 samples
                            //   (b) test clip injection — remaining has full audio
                            const STRIDE_SAMPLES: usize = 8960;
                            let mut remaining = std::mem::take(&mut stt_audio_buffer);
                            if !remaining.is_empty() {
                                // Process full stride chunks
                                while remaining.len() >= STRIDE_SAMPLES {
                                    let chunk: Vec<f32> = remaining.drain(..STRIDE_SAMPLES).collect();
                                    match eng.transcribe_chunk(&chunk, false) {
                                        Ok(text) => {
                                            if !text.trim().is_empty() {
                                                stitched_transcript.push_str(&text);
                                            }
                                        }
                                        Err(e) => log::error!("[STT] Nemotron final stride failed: {}", e),
                                    }
                                }
                                // Flush remainder (< STRIDE_SAMPLES) with zero-pad
                                let mut pad = remaining;
                                pad.resize(STRIDE_SAMPLES, 0.0);
                                match eng.transcribe_chunk(&pad, true) {
                                    Ok(text) => {
                                        if !text.trim().is_empty() {
                                            stitched_transcript.push_str(&text);
                                        }
                                    }
                                    Err(e) => log::error!("[STT] Nemotron final flush failed: {}", e),
                                }
                            } else {
                                // Flush cache with zero padding chunk
                                let _ = eng.transcribe_chunk(&vec![0.0; STRIDE_SAMPLES], true);
                            }

                            let _ = eng.reset_state();
                        } else {
                            // Qwen mode
                            let start_idx = utterance.len().saturating_sub(72000);
                            let rolling_utterance = &utterance[start_idx..];

                            match eng.transcribe(rolling_utterance) {
                                Ok(text) => {
                                    let raw_final: String = text;
                                    if start_idx == 0 {
                                        stitched_transcript = raw_final;
                                    } else {
                                        stitched_transcript = crate::services::utils::stitch_transcripts(&stitched_transcript, &raw_final);
                                    }
                                }
                                Err(e) => log::error!("[STT] Final transcription failed: {}", e),
                            }
                        }

                        if stitched_transcript.trim().is_empty() {
                            log::info!("[STT] Discarding empty final transcript.");
                        } else if let Some(ref pipeline_tx) = pipeline_event_tx {
                            let _ = pipeline_tx.send(VoxEvent::TranscriptFinal {
                                turn_id: tid,
                                owner,
                                text: stitched_transcript.clone(),
                            });
                        }
                    }

                    let target = match owner {
                        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                        InteractionOwner::Tray => "tray",
                        InteractionOwner::Wizard => "wizard",
                    };
                    let _ = app.emit_to(target, "ptt_status", serde_json::json!({ "state": "IDLE" }));

                    stitched_transcript.clear();
                    last_transcript.clear();
                    stt_audio_buffer.clear();
                    processed_samples = 0;
                    last_emit_time = Instant::now();
                }
                SttCommand::ResetStream => {
                    log::info!("[STT] ResetStream received. Aggressively clearing state.");
                    stitched_transcript.clear();
                    last_transcript.clear();
                    stt_audio_buffer.clear();
                    processed_samples = 0;
                    if let Some(ref eng) = engine {
                        let _ = eng.reset_state();
                    }
                    while let Ok(pending_cmd) = rx.try_recv() {
                        match pending_cmd {
                            SttCommand::Partial(..) | SttCommand::Final(..) | SttCommand::ResetStream => continue,
                            SttCommand::Shutdown => return,
                        }
                    }
                }
            }
        }
        is_loaded.store(false, std::sync::atomic::Ordering::Relaxed);
        log::info!("[STT] Worker thread exiting.");
    }).map_err(|e| e.to_string())
}
