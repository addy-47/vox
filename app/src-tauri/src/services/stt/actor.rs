use anyhow::Result;
use tauri::{AppHandle, Emitter};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use crate::core::state::InteractionOwner;
use crate::core::events::VoxEvent;
use crate::services::traits::SttEngine as _;
use super::qwen_onnx::SttEngine;

pub enum SttCommand {
    Partial(u32, crate::core::state::InteractionOwner, Vec<f32>),
    Final(u32, crate::core::state::InteractionOwner, Vec<f32>),
    ResetStream,
    Shutdown,
}

pub fn spawn_stt_worker(
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<SttCommand>,
    model_path: std::path::PathBuf,
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

        log::info!("[STT] >>> Dedicated worker thread started.");
        
        let mut engine: Option<SttEngine> = if pre_load {
            app.emit(crate::core::constants::EVENT_MODEL_LOADING, "STT").ok();
            match SttEngine::new(&model_path) {
                Ok(e) => {
                    is_loaded.store(true, std::sync::atomic::Ordering::Relaxed);
                    app.emit(crate::core::constants::EVENT_MODEL_READY, "STT").ok();
                    Some(e)
                },
                Err(err) => {
                    log::error!("[STT] CRITICAL: Failed to initialize Sherpa engine: {}", err);
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
                        log::info!("[STT] New turn ID {} detected (prev {}). Resetting stitched buffers.", tid, current_active_turn);
                        current_active_turn = tid;
                        stitched_transcript.clear();
                        last_transcript.clear();
                    }

                    let dynamic_throttle = last_inference_duration.max(Duration::from_millis(300));
                    if last_emit_time.elapsed() >= dynamic_throttle {
                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            stitched_transcript.clear();
                            last_transcript.clear();
                            continue;
                        }

                        if engine.is_none() {
                            app.emit(crate::core::constants::EVENT_MODEL_LOADING, "STT").ok();
                            match SttEngine::new(&model_path) {
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
                            // Audio Guillotine Fix: Keep last 4.5 seconds (72000 samples) instead of slicing aggressively
                            // and stitch with previous partial transcripts.
                            let start_idx = utterance.len().saturating_sub(72000);
                            let rolling_utterance = &utterance[start_idx..];

                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                stitched_transcript.clear();
                                last_transcript.clear();
                                continue;
                            }

                            let start_inference = Instant::now();
                            match eng.transcribe(rolling_utterance) {
                                Ok(text) => {
                                    last_inference_duration = start_inference.elapsed();
                                    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                        stitched_transcript.clear();
                                        last_transcript.clear();
                                        continue;
                                    }
                                    let raw_partial: String = text;
                                    
                                    // Stitch the raw partial with our stable prefix buffer statefully; overwrite if we are transcribing from the beginning
                                    if start_idx == 0 {
                                        stitched_transcript = raw_partial;
                                    } else {
                                        stitched_transcript = crate::services::utils::stitch_transcripts(&stitched_transcript, &raw_partial);
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
                                Err(e) => log::error!("[STT] Partial transcription failed: {}", e),
                            }
                        }
                    }
                }
                        SttCommand::Final(tid, owner, utterance) => {
                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                stitched_transcript.clear();
                                last_transcript.clear();
                                continue;
                            }

                            if tid < current_active_turn {
                                log::info!("[STT] Discarding stale Final from superseded turn {} (active: {})", tid, current_active_turn);
                                stitched_transcript.clear();
                                last_transcript.clear();
                                continue;
                            }

                            if engine.is_none() {
                                match SttEngine::new(&model_path) {
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
                                // BUGFIX: Slicing final utterance to the trailing 4.5s chunk to avoid O(N^2) offline transformer death.
                                let start_idx = utterance.len().saturating_sub(72000);
                                let rolling_utterance = &utterance[start_idx..];

                                if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                    stitched_transcript.clear();
                                    last_transcript.clear();
                                    continue;
                                }

                                match eng.transcribe(rolling_utterance) {
                                    Ok(text) => {
                                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                            stitched_transcript.clear();
                                            last_transcript.clear();
                                            continue;
                                        }
                                        
                                        let raw_final: String = text;
                                        
                                        // Overwrite if start_idx == 0; otherwise stitch
                                        if start_idx == 0 {
                                            stitched_transcript = raw_final;
                                        } else {
                                            stitched_transcript = crate::services::utils::stitch_transcripts(&stitched_transcript, &raw_final);
                                        }
                                        
                                        if stitched_transcript.trim().is_empty() {
                                            log::info!("[STT] Discarding empty final transcript.");
                                            stitched_transcript.clear();
                                            last_transcript.clear();
                                            continue;
                                        }

                                        if let Some(ref pipeline_tx) = pipeline_event_tx {
                                            let _ = pipeline_tx.send(VoxEvent::TranscriptFinal {
                                                turn_id: tid,
                                                owner,
                                                text: stitched_transcript.clone(),
                                            });
                                        }
                                        stitched_transcript.clear();
                                        last_transcript.clear();
                                    }
                                    Err(e) => {
                                        log::error!("[STT] Final transcription failed: {}", e);
                                        stitched_transcript.clear();
                                        last_transcript.clear();
                                    }
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
                            last_emit_time = Instant::now();
                        }
                        SttCommand::ResetStream => {
                            log::info!("[STT] ResetStream received. Aggressively clearing state.");
                            stitched_transcript.clear();
                            last_transcript.clear();
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
