use super::{
    SttProvider, STT_DEFAULT_INFERENCE_DURATION_MS, STT_MIN_PARTIAL_THROTTLE_MS,
    STT_PARTIAL_ERROR_PENALTY_MS, STT_WORKER_RECV_TIMEOUT_MS, STT_WORKER_THREAD_PRIORITY,
};
use crate::core::events::VoxEvent;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub enum SttCommand {
    Partial {
        turn_id: u32,
        audio: Vec<f32>,
        recycle_tx: std::sync::mpsc::SyncSender<Vec<f32>>,
    },
    Final(u32, Vec<f32>),
    ResetStream,
    Shutdown,
}

pub struct SttActorChannels {
    pub rx: std::sync::mpsc::Receiver<SttCommand>,
    pub pipeline_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
    pub partial_emitter: Option<Arc<dyn Fn(u32, String) + Send + Sync>>,
}

pub struct SttActorHandles {
    pub cancel_flag: Arc<AtomicBool>,
    pub engine_shutdown: Arc<AtomicBool>,
}

struct WorkerState {
    last_emit_time: Instant,
    last_transcript: String,
    current_active_turn: u32,
    last_inference_duration: Duration,
}

struct WorkerContext<'a> {
    pub provider: &'a dyn SttProvider,
    pub pipeline_event_tx: &'a Option<std::sync::mpsc::Sender<VoxEvent>>,
    pub partial_emitter: &'a Option<Arc<dyn Fn(u32, String) + Send + Sync>>,
    pub cancel_flag: &'a Arc<AtomicBool>,
}

/// Drains subsequent partial commands from the channel and returns the latest audio slice.
fn coalesce_partials(
    cmd: SttCommand,
    rx: &std::sync::mpsc::Receiver<SttCommand>,
    pending_cmd: &mut Option<SttCommand>,
) -> SttCommand {
    if let SttCommand::Partial {
        mut turn_id,
        mut audio,
        recycle_tx,
    } = cmd
    {
        let mut skipped = 0;
        while let Ok(next_cmd) = rx.try_recv() {
            match next_cmd {
                SttCommand::Partial {
                    turn_id: next_tid,
                    audio: next_audio,
                    recycle_tx: _,
                } => {
                    let _ = recycle_tx.try_send(audio);
                    turn_id = next_tid;
                    audio = next_audio;
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
        SttCommand::Partial {
            turn_id,
            audio,
            recycle_tx,
        }
    } else {
        cmd
    }
}

/// Dispatches a partial transcript event directly to UI via partial_emitter if changed.
fn emit_partial_event(ctx: &WorkerContext<'_>, tid: u32, text: String, state: &mut WorkerState) {
    if !text.is_empty() && text != state.last_transcript {
        if let Some(ref emitter) = ctx.partial_emitter {
            emitter(tid, text.clone());
        }
        state.last_transcript = text;
    }
}

/// Processes incoming partial speech frames with dynamic throttling and emits partial events.
fn handle_partial_command(
    ctx: &WorkerContext<'_>,
    tid: u32,
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

    let dynamic_throttle = state
        .last_inference_duration
        .max(Duration::from_millis(STT_MIN_PARTIAL_THROTTLE_MS));
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

            emit_partial_event(ctx, tid, text, state);
        }
        Err(e) => {
            log::error!("[STT] Partial transcription failed: {}", e);
            state.last_inference_duration = Duration::from_millis(STT_PARTIAL_ERROR_PENALTY_MS);
        }
    }

    state.last_emit_time = Instant::now();
}

/// Emits the final turn event to the pipeline event channel.
fn emit_final_events(ctx: &WorkerContext<'_>, tid: u32, transcript: String) {
    if let Some(ref pipeline_tx) = ctx.pipeline_event_tx {
        if let Err(e) = pipeline_tx.send(VoxEvent::TranscriptFinal {
            turn_id: tid,
            text: transcript,
        }) {
            log::warn!("[STT] Error sending final transcript event: {:?}", e);
        }
    }
}

/// Transcribes final speech buffer, dispatches final events, and resets worker state.
fn handle_final_command(
    ctx: &WorkerContext<'_>,
    tid: u32,
    utterance: &[f32],
    state: &mut WorkerState,
) {
    if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) || tid < state.current_active_turn
    {
        state.last_transcript.clear();
        if let Err(e) = ctx.provider.reset_state() {
            log::warn!("[STT] Error resetting state on stale final: {:?}", e);
        }
        if let Some(ref pipeline_tx) = ctx.pipeline_event_tx {
            if let Err(e) = pipeline_tx.send(VoxEvent::Cancelled { turn_id: tid }) {
                log::warn!(
                    "[STT] Error sending cancelled event on stale final: {:?}",
                    e
                );
            }
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

    emit_final_events(ctx, tid, transcript);

    state.last_transcript.clear();
    state.last_emit_time = Instant::now();
}

/// Drains stream reset commands and clears transcription state.
fn drain_reset_stream(
    rx: &std::sync::mpsc::Receiver<SttCommand>,
    provider: &dyn SttProvider,
    state: &mut WorkerState,
    pending_cmd: &mut Option<SttCommand>,
) -> bool {
    log::info!("[STT] ResetStream received. Aggressively clearing state.");
    state.last_transcript.clear();
    if let Err(e) = provider.reset_state() {
        log::warn!("[STT] Error resetting provider on stream reset: {:?}", e);
    }
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            SttCommand::Partial {
                audio, recycle_tx, ..
            } => {
                let _ = recycle_tx.try_send(audio);
                continue;
            }
            SttCommand::ResetStream => continue,
            SttCommand::Final(..) => {
                *pending_cmd = Some(cmd);
                break;
            }
            SttCommand::Shutdown => return true,
        }
    }
    false
}

/// Executes the core event polling and dispatch loop for speech recognition commands.
fn run_worker_loop(
    provider: &dyn SttProvider,
    channels: SttActorChannels,
    handles: SttActorHandles,
) {
    let ctx = WorkerContext {
        provider,
        pipeline_event_tx: &channels.pipeline_event_tx,
        partial_emitter: &channels.partial_emitter,
        cancel_flag: &handles.cancel_flag,
    };

    let mut state = WorkerState {
        last_emit_time: Instant::now(),
        last_transcript: String::new(),
        current_active_turn: 0u32,
        last_inference_duration: Duration::from_millis(STT_DEFAULT_INFERENCE_DURATION_MS),
    };
    let mut pending_cmd = None;

    loop {
        if handles
            .engine_shutdown
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            log::info!("[STT] Engine shutdown flag detected. Exiting loop.");
            break;
        }

        let raw_cmd = if let Some(c) = pending_cmd.take() {
            c
        } else {
            match channels
                .rx
                .recv_timeout(Duration::from_millis(STT_WORKER_RECV_TIMEOUT_MS))
            {
                Ok(c) => c,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };

        let cmd = coalesce_partials(raw_cmd, &channels.rx, &mut pending_cmd);

        match cmd {
            SttCommand::Shutdown => {
                log::info!("[STT] Shutdown signal received. Exiting worker thread.");
                break;
            }
            SttCommand::Partial {
                turn_id,
                audio,
                recycle_tx,
            } => {
                handle_partial_command(&ctx, turn_id, &audio, &mut state);
                let _ = recycle_tx.try_send(audio);
            }
            SttCommand::Final(tid, utterance) => {
                handle_final_command(&ctx, tid, &utterance, &mut state);
            }
            SttCommand::ResetStream => {
                if drain_reset_stream(&channels.rx, ctx.provider, &mut state, &mut pending_cmd) {
                    break;
                }
            }
        }
    }
    log::info!("[STT] Worker thread exiting.");
}

/// Spawns dedicated OS worker thread for speech recognition inference and event dispatching.
pub fn spawn_stt_worker(
    channels: SttActorChannels,
    provider: Box<dyn SttProvider>,
    handles: SttActorHandles,
) -> Result<std::thread::JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("vox-stt-worker".to_string())
        .spawn(move || {
            use thread_priority::*;
            if let Err(e) = set_current_thread_priority(ThreadPriority::Crossplatform(
                ThreadPriorityValue::try_from(STT_WORKER_THREAD_PRIORITY).unwrap(),
            )) {
                log::warn!("[STT] Failed to set high priority: {:?}", e);
            }

            log::info!("[STT] >>> Dedicated worker thread started.");
            run_worker_loop(&*provider, channels, handles);
        })
        .map_err(|e| e.to_string())
}
