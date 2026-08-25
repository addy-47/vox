use crate::core::events::VoxEvent;
use crate::core::state::InteractionOwner;
use crate::services::stt::providers::SttProvider;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Commands sent to the background STT worker actor thread.
pub enum SttCommand {
    Partial(u32, crate::core::state::InteractionOwner, Vec<f32>),
    Final(u32, crate::core::state::InteractionOwner, Vec<f32>),
    ResetStream,
    Shutdown,
}

struct WorkerState {
    last_emit_time: Instant,
    last_transcript: String,
    current_active_turn: u32,
    last_inference_duration: Duration,
}

struct WorkerContext<'a> {
    app: &'a AppHandle,
    provider: &'a dyn SttProvider,
    pipeline_event_tx: &'a Option<std::sync::mpsc::Sender<VoxEvent>>,
    cancel_flag: &'a Arc<AtomicBool>,
}

/// Drains subsequent partial commands from the channel and returns the latest audio slice.
fn coalesce_partials(
    cmd: SttCommand,
    rx: &std::sync::mpsc::Receiver<SttCommand>,
    pending_cmd: &mut Option<SttCommand>,
) -> SttCommand {
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
                    *pending_cmd = Some(other);
                    break;
                }
            }
        }
        if skipped > 0 {
            log::debug!("[STT] Coalesced {} stale partials in queue", skipped);
        }
        SttCommand::Partial(tid, owner, utterance)
    } else {
        cmd
    }
}

/// Processes an incoming partial speech frame with adaptive throttle and emits partial events.
fn handle_partial_command(
    ctx: &WorkerContext<'_>,
    tid: u32,
    owner: InteractionOwner,
    utterance: &[f32],
    state: &mut WorkerState,
) {
    if tid != state.current_active_turn {
        log::info!(
            "[STT] New turn ID {} detected (prev {}). Resetting buffers.",
            tid,
            state.current_active_turn
        );
        state.current_active_turn = tid;
        state.last_transcript.clear();
        if let Err(e) = ctx.provider.reset_state() {
            log::warn!("[STT] Error resetting state for new turn: {:?}", e);
        }
    }

    let dynamic_throttle = state.last_inference_duration.max(Duration::from_millis(300));
    if state.last_emit_time.elapsed() < dynamic_throttle {
        return;
    }

    if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        state.last_transcript.clear();
        if let Err(e) = ctx.provider.reset_state() {
            log::warn!("[STT] Error resetting state on cancellation: {:?}", e);
        }
        return;
    }

    let start_inference = Instant::now();
    match ctx.provider.transcribe_chunk(utterance, false) {
        Ok(text) => {
            state.last_inference_duration = start_inference.elapsed();

            if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                state.last_transcript.clear();
                if let Err(e) = ctx.provider.reset_state() {
                    log::warn!("[STT] Error resetting state on cancellation: {:?}", e);
                }
                return;
            }

            if !text.is_empty() && text != state.last_transcript {
                if let Some(ref pipeline_tx) = ctx.pipeline_event_tx {
                    if let Err(e) = pipeline_tx.send(VoxEvent::TranscriptPartial {
                        turn_id: tid,
                        owner,
                        text: text.clone(),
                    }) {
                        log::warn!("[STT] Error dispatching partial transcript: {:?}", e);
                    }
                }
                state.last_transcript = text;
            }
        }
        Err(e) => {
            log::error!("[STT] Partial transcription failed: {}", e);
            state.last_inference_duration = Duration::from_millis(500);
        }
    }

    state.last_emit_time = Instant::now();
}

/// Transcribes the final utterance, dispatches final events, and resets PTT status.
fn handle_final_command(
    ctx: &WorkerContext<'_>,
    tid: u32,
    owner: InteractionOwner,
    utterance: &[f32],
    state: &mut WorkerState,
) {
    if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) || tid < state.current_active_turn
    {
        state.last_transcript.clear();
        if let Err(e) = ctx.provider.reset_state() {
            log::warn!("[STT] Error resetting state on stale final: {:?}", e);
        }
        return;
    }
    state.current_active_turn = tid;

    let transcript = match ctx.provider.transcribe_chunk(utterance, true) {
        Ok(text) => text,
        Err(e) => {
            log::error!("[STT] Final transcription failed: {}", e);
            String::new()
        }
    };

    if let Err(e) = ctx.provider.reset_state() {
        log::warn!("[STT] Error resetting provider state post-final: {:?}", e);
    }

    if transcript.trim().is_empty() {
        log::info!("[STT] Discarding empty final transcript.");
        if let Some(ref pipeline_tx) = ctx.pipeline_event_tx {
            if let Err(e) = pipeline_tx.send(VoxEvent::Cancelled { turn_id: tid }) {
                log::warn!("[STT] Error sending cancelled event: {:?}", e);
            }
        }
    } else if let Some(ref pipeline_tx) = ctx.pipeline_event_tx {
        if let Err(e) = pipeline_tx.send(VoxEvent::TranscriptFinal {
            turn_id: tid,
            owner,
            text: transcript,
        }) {
            log::warn!("[STT] Error sending final transcript event: {:?}", e);
        }
    }

    let target = match owner {
        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
        InteractionOwner::Dictation => "tray",
        InteractionOwner::Wizard => "wizard",
    };
    if let Err(e) = ctx
        .app
        .emit_to(target, "ptt_status", serde_json::json!({ "state": "IDLE" }))
    {
        log::warn!("[STT] Error emitting ptt_status idle event: {:?}", e);
    }

    state.last_transcript.clear();
    state.last_emit_time = Instant::now();
}

/// Drains stream reset commands and discards stale audio messages in the queue.
fn drain_reset_stream(
    rx: &std::sync::mpsc::Receiver<SttCommand>,
    provider: &dyn SttProvider,
    state: &mut WorkerState,
) -> bool {
    log::info!("[STT] ResetStream received. Aggressively clearing state.");
    state.last_transcript.clear();
    if let Err(e) = provider.reset_state() {
        log::warn!("[STT] Error resetting provider on stream reset: {:?}", e);
    }
    while let Ok(pending_cmd) = rx.try_recv() {
        match pending_cmd {
            SttCommand::Partial(..)
            | SttCommand::Final(..)
            | SttCommand::ResetStream => continue,
            SttCommand::Shutdown => return true,
        }
    }
    false
}

/// Spawns a dedicated OS worker thread for speech recognition inference and event dispatching.
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
            is_loaded.store(true, std::sync::atomic::Ordering::Relaxed);

            let ctx = WorkerContext {
                app: &app,
                provider: &*provider,
                pipeline_event_tx: &pipeline_event_tx,
                cancel_flag: &cancel_flag,
            };

            let mut state = WorkerState {
                last_emit_time: Instant::now(),
                last_transcript: String::new(),
                current_active_turn: 0u32,
                last_inference_duration: Duration::from_millis(300),
            };
            let mut pending_cmd = None;

            loop {
                if engine_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                    log::info!("[STT] Engine shutdown flag detected. Exiting loop.");
                    break;
                }

                let raw_cmd = if let Some(c) = pending_cmd.take() {
                    c
                } else {
                    match rx.recv_timeout(Duration::from_millis(150)) {
                        Ok(c) => c,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                };

                let cmd = coalesce_partials(raw_cmd, &rx, &mut pending_cmd);

                match cmd {
                    SttCommand::Shutdown => {
                        log::info!("[STT] Shutdown signal received. Exiting worker thread.");
                        break;
                    }
                    SttCommand::Partial(tid, owner, utterance) => {
                        handle_partial_command(&ctx, tid, owner, &utterance, &mut state);
                    }
                    SttCommand::Final(tid, owner, utterance) => {
                        handle_final_command(&ctx, tid, owner, &utterance, &mut state);
                    }
                    SttCommand::ResetStream => {
                        if drain_reset_stream(&rx, &*provider, &mut state) {
                            break;
                        }
                    }
                }
            }
            is_loaded.store(false, std::sync::atomic::Ordering::Relaxed);
            log::info!("[STT] Worker thread exiting.");
        })
        .map_err(|e| e.to_string())
}
