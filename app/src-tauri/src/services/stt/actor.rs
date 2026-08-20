use crate::core::events::VoxEvent;
use crate::core::state::InteractionOwner;
use crate::services::stt::providers::SttProvider;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub enum SttCommand {
    Partial(u32, crate::core::state::InteractionOwner, Vec<f32>),
    Final(u32, crate::core::state::InteractionOwner, Vec<f32>),
    ResetStream,
    Shutdown,
}

pub fn spawn_stt_worker(
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<SttCommand>,
    provider: Box<dyn SttProvider>,
    pipeline_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
    engine_shutdown: Arc<AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("vox-stt-worker".to_string())
        .spawn(move || {
            use thread_priority::*;
            if let Err(e) = set_current_thread_priority(ThreadPriority::Crossplatform(
                ThreadPriorityValue::try_from(80u8).unwrap(),
            )) {
                log::warn!("[STT] Failed to set high priority: {:?}", e);
            }

            log::info!("[STT] >>> Dedicated worker thread started.");

            // Provider is pre-constructed and ready to use
            is_loaded.store(true, std::sync::atomic::Ordering::Relaxed);

            let mut last_emit_time = Instant::now();
            let mut last_transcript = String::new();
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

                // Queue-coalescing: If we got a Partial, drain any further sequential Partials
                // to process only the freshest audio
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
                            log::info!(
                                "[STT] New turn ID {} detected (prev {}). Resetting buffers.",
                                tid,
                                current_active_turn
                            );
                            current_active_turn = tid;
                            last_transcript.clear();
                            let _ = provider.reset_state();
                        }

                        let dynamic_throttle =
                            last_inference_duration.max(Duration::from_millis(300));
                        if last_emit_time.elapsed() >= dynamic_throttle {
                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                last_transcript.clear();
                                let _ = provider.reset_state();
                                continue;
                            }

                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                last_transcript.clear();
                                let _ = provider.reset_state();
                                continue;
                            }

                            let start_inference = Instant::now();

                            match provider.transcribe_chunk(&utterance, false) {
                                Ok(text) => {
                                    last_inference_duration = start_inference.elapsed();

                                    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                        last_transcript.clear();
                                        let _ = provider.reset_state();
                                        continue;
                                    }

                                    if !text.is_empty() && text != last_transcript {
                                        if let Some(ref pipeline_tx) = pipeline_event_tx {
                                            let _ = pipeline_tx.send(VoxEvent::TranscriptPartial {
                                                turn_id: tid,
                                                owner,
                                                text: text.clone(),
                                            });
                                        }
                                        last_transcript = text;
                                    }
                                }
                                Err(e) => {
                                    log::error!("[STT] Partial transcription failed: {}", e);
                                    last_inference_duration = Duration::from_millis(500);
                                }
                            }

                            last_emit_time = Instant::now();
                        }
                    }
                    SttCommand::Final(tid, owner, utterance) => {
                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            last_transcript.clear();
                            let _ = provider.reset_state();
                            continue;
                        }

                        if tid < current_active_turn {
                            log::info!(
                                "[STT] Discarding stale Final from turn {} (active: {})",
                                tid,
                                current_active_turn
                            );
                            last_transcript.clear();
                            let _ = provider.reset_state();
                            continue;
                        }
                        current_active_turn = tid;

                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            last_transcript.clear();
                            let _ = provider.reset_state();
                            continue;
                        }

                        let transcript = match provider.transcribe_chunk(&utterance, true) {
                            Ok(text) => text,
                            Err(e) => {
                                log::error!("[STT] Final transcription failed: {}", e);
                                String::new()
                            }
                        };

                        let _ = provider.reset_state();

                        if transcript.trim().is_empty() {
                            log::info!("[STT] Discarding empty final transcript.");
                            if let Some(ref pipeline_tx) = pipeline_event_tx {
                                let _ = pipeline_tx.send(VoxEvent::Cancelled { turn_id: tid });
                            }
                        } else if let Some(ref pipeline_tx) = pipeline_event_tx {
                            let _ = pipeline_tx.send(VoxEvent::TranscriptFinal {
                                turn_id: tid,
                                owner,
                                text: transcript.clone(),
                            });
                        }

                        let target = match owner {
                            InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                            InteractionOwner::Dictation => "tray",
                            InteractionOwner::Wizard => "wizard",
                        };
                        let _ = app.emit_to(
                            target,
                            "ptt_status",
                            serde_json::json!({ "state": "IDLE" }),
                        );

                        last_transcript.clear();
                        last_emit_time = Instant::now();
                    }
                    SttCommand::ResetStream => {
                        log::info!("[STT] ResetStream received. Aggressively clearing state.");
                        last_transcript.clear();
                        let _ = provider.reset_state();
                        while let Ok(pending_cmd) = rx.try_recv() {
                            match pending_cmd {
                                SttCommand::Partial(..)
                                | SttCommand::Final(..)
                                | SttCommand::ResetStream => continue,
                                SttCommand::Shutdown => return,
                            }
                        }
                    }
                }
            }
            is_loaded.store(false, std::sync::atomic::Ordering::Relaxed);
            log::info!("[STT] Worker thread exiting.");
        })
        .map_err(|e| e.to_string())
}
