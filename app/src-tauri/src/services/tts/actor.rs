use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc,
    },
    thread::{Builder, JoinHandle},
};

use crate::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::{AudioIntent, VoxEvent},
        settings::VoxSettings,
    },
    services::{
        audio::PlaybackEngine,
        tts::{factory::create_tts_provider, providers::TtsProvider},
    },
};

/// Commands accepted by the dedicated TTS synthesis worker thread.
#[derive(Debug)]
pub enum TtsCommand {
    Generate {
        turn_id: u32,
        text: String,
        intent: AudioIntent,
    },
    SetVoice(i32),
    SetSpeed(f32),
    SetQualitySteps(u32),
    Shutdown,
}

/// Execution handles and atomics passed to the dedicated TTS worker thread.
pub struct TtsWorkerHandles {
    pub playback: Arc<PlaybackEngine>,
    pub event_tx: mpsc::Sender<VoxEvent>,
    pub cancel_flag: Arc<AtomicBool>,
    pub pending_synthesis_jobs: Option<Arc<AtomicU32>>,
    pub telemetry_rtf: Option<Arc<AtomicU32>>,
}

pub fn spawn_tts_worker(
    rx: mpsc::Receiver<TtsCommand>,
    provider: Box<dyn TtsProvider>,
    handles: TtsWorkerHandles,
) {
    log::info!("[TTS Worker] Persistent loop started.");

    let mut job_seq = 0u64;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCommand::Generate {
                turn_id,
                text,
                intent,
            } => {
                job_seq += 1;
                let pending = handles
                    .pending_synthesis_jobs
                    .as_ref()
                    .map(|j| j.load(Ordering::Relaxed))
                    .unwrap_or(0);
                log::info!(
                    "[TTS Worker] Job started (job {}, turn {}, intent {:?}, chars {}, words {}, pending_jobs {})",
                    job_seq,
                    turn_id,
                    intent,
                    text.chars().count(),
                    text.split_whitespace().count(),
                    pending
                );
                log::debug!(
                    "[TTS Worker] Job text (job {}, turn {}): '{}'",
                    job_seq,
                    turn_id,
                    text
                );

                let text_clone = text.clone();
                let provider_ref = AssertUnwindSafe(&*provider);
                let ctx = super::providers::SynthesisContext {
                    turn_id,
                    intent,
                    cancel: handles.cancel_flag.clone(),
                    playback: &handles.playback,
                    event_tx: handles.event_tx.clone(),
                    telemetry_rtf: handles.telemetry_rtf.as_ref(),
                };

                let res = catch_unwind(AssertUnwindSafe(|| {
                    provider_ref.synthesize_chunk(&text_clone, &ctx)
                }));

                let mut remaining_jobs = 0u32;
                let mut flushed_pre_roll = false;
                if let Some(ref jobs) = handles.pending_synthesis_jobs {
                    let prev = jobs
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                            Some(v.saturating_sub(1))
                        })
                        .unwrap_or(0);
                    remaining_jobs = prev.saturating_sub(1);
                    if prev <= 1 {
                        handles.playback.flush_pre_roll();
                        flushed_pre_roll = true;
                    }
                }
                log::info!(
                    "[TTS Worker] Job finished (job {}, turn {}, remaining_jobs {}, flushed_pre_roll {})",
                    job_seq, turn_id, remaining_jobs, flushed_pre_roll
                );

                match res {
                    Ok(Err(e)) => {
                        log::warn!(
                            "[TTS Worker] Synthesis chunk failed for turn {}: {}",
                            turn_id,
                            e
                        );
                        if let Err(send_err) =
                            handles.event_tx.send(VoxEvent::Error(PipelineError {
                                turn_id,
                                message: format!("TTS synthesis failed: {}", e),
                                source: "tts".into(),
                                impact: PipelineImpact::Degraded,
                            }))
                        {
                            log::warn!(
                                "[TTS Worker] Failed to dispatch synthesis error event: {}",
                                send_err
                            );
                        }
                    }
                    Err(payload) => {
                        let msg = payload
                            .downcast_ref::<&str>()
                            .copied()
                            .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
                            .unwrap_or("unknown panic");
                        log::error!("[TTS Worker] Engine panicked on turn {}: {}", turn_id, msg);
                        if let Err(send_err) =
                            handles.event_tx.send(VoxEvent::Error(PipelineError {
                                turn_id,
                                message: format!("TTS engine panic: {}", msg),
                                source: "tts".into(),
                                impact: PipelineImpact::Degraded,
                            }))
                        {
                            log::warn!(
                                "[TTS Worker] Failed to dispatch engine panic error event: {}",
                                send_err
                            );
                        }
                    }
                    Ok(Ok(())) => {}
                }
            }
            TtsCommand::SetVoice(voice) => {
                log::info!("[TTS Worker] Setting voice to index: {}", voice);
                provider.set_voice(voice);
            }
            TtsCommand::SetSpeed(speed) => {
                log::info!("[TTS Worker] Setting speech speed to: {}", speed);
                provider.set_speed(speed);
            }
            TtsCommand::SetQualitySteps(steps) => {
                log::info!("[TTS Worker] Setting synthesis quality steps to: {}", steps);
                provider.set_quality_steps(steps);
            }
            TtsCommand::Shutdown => {
                log::info!("[TTS Worker] Shutdown command received. Exiting loop.");
                break;
            }
        }
    }

    log::info!("[TTS Worker] Loop exited. Provider will be dropped.");
}

/// Handles and flags passed when warming up the TTS actor.
pub struct TtsWarmUpHandles<'a> {
    pub tts_tx: &'a mut Option<mpsc::Sender<TtsCommand>>,
    pub tts_handle: &'a mut Option<JoinHandle<()>>,
    pub cancel_flag: Arc<AtomicBool>,
    pub playback_engine: Arc<PlaybackEngine>,
    pub pending_synthesis_jobs: Option<Arc<AtomicU32>>,
    pub telemetry_rtf: Option<Arc<AtomicU32>>,
}

/// Spawns and initializes a persistent TTS worker actor thread.
pub fn warm_up_tts(
    handles: TtsWarmUpHandles<'_>,
    settings: &VoxSettings,
    super_tts_path: &Path,
    reference_audio: Option<&str>,
    event_tx: mpsc::Sender<VoxEvent>,
) -> Result<(), String> {
    if handles.tts_tx.is_some() {
        return Ok(());
    }

    log::info!("[TTS Actor] Warming up TTS worker");
    let provider = create_tts_provider(settings, super_tts_path, reference_audio)?;

    let (tx, rx) = mpsc::channel::<TtsCommand>();
    *handles.tts_tx = Some(tx);

    let worker_handles = TtsWorkerHandles {
        playback: handles.playback_engine,
        event_tx,
        cancel_flag: handles.cancel_flag,
        pending_synthesis_jobs: handles.pending_synthesis_jobs,
        telemetry_rtf: handles.telemetry_rtf,
    };

    let handle = Builder::new()
        .name("vox-tts-persistent".to_string())
        .spawn(move || {
            let _ =
                thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max);
            spawn_tts_worker(rx, provider, worker_handles);
        })
        .map_err(|e| e.to_string())?;

    *handles.tts_handle = Some(handle);
    Ok(())
}

/// Signals the running TTS worker thread to shutdown and drop its model instance.
pub fn cool_down_tts(tts_tx: &mut Option<mpsc::Sender<TtsCommand>>) {
    if let Some(tx) = tts_tx.take() {
        let _ = tx.send(TtsCommand::Shutdown);
        log::info!("[TTS Actor] Shutdown command sent (offloaded)");
    }
}
