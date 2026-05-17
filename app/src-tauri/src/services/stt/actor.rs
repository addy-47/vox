use anyhow::Result;
use tauri::{AppHandle, Manager, Emitter};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use crate::core::state::{InteractionOwner, InteractionState};
use crate::core::events::VoxEvent;
use crate::core::constants::STT_THROTTLE_MS;
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

        loop {
            if engine_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                log::info!("[STT] Engine shutdown flag detected. Exiting loop.");
                break;
            }

            match rx.try_recv() {
                Ok(cmd) => {
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

                            if last_emit_time.elapsed() >= Duration::from_millis(STT_THROTTLE_MS) {
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
                                    // Rolling window logic: only transcribe last 2.5 seconds of audio to cut O(N^2) CPU overhead
                                    let start_idx = utterance.len().saturating_sub(40000);
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
                                            let raw_partial: String = text;
                                            
                                            // Stitch the raw partial with our stable prefix buffer statefully
                                            stitched_transcript = crate::services::utils::stitch_transcripts(&stitched_transcript, &raw_partial);

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
                                if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                    stitched_transcript.clear();
                                    last_transcript.clear();
                                    continue;
                                }

                                match eng.transcribe(&utterance) {
                                    Ok(text) => {
                                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                            stitched_transcript.clear();
                                            last_transcript.clear();
                                            continue;
                                        }
                                        let text_str: String = text;
                                        if let Some(ref pipeline_tx) = pipeline_event_tx {
                                            let _ = pipeline_tx.send(VoxEvent::TranscriptFinal {
                                                turn_id: tid,
                                                owner,
                                                text: text_str,
                                            });
                                        }
                                    }
                                    Err(e) => log::error!("[STT] Final transcription failed: {}", e),
                                }
                            }
                            
                            let target = match owner {
                                InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                                InteractionOwner::Tray => "tray",
                            };
                            let _ = app.emit_to(target, "ptt_status", serde_json::json!({ "state": "IDLE" }));

                            let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
                            let idle_state = if state.pipeline.is_engaged.load(std::sync::atomic::Ordering::Relaxed) {
                                InteractionState::Listening
                            } else {
                                InteractionState::Idle
                            };
                            state.pipeline.update_interaction_state(idle_state, owner, &app);

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
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
        is_loaded.store(false, std::sync::atomic::Ordering::Relaxed);
        log::info!("[STT] Worker thread exiting.");
    }).map_err(|e| e.to_string())
}
