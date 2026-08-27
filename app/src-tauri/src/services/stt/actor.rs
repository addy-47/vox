use super::{
    STT_DEFAULT_INFERENCE_DURATION_MS, STT_MIN_PARTIAL_THROTTLE_MS, STT_PARTIAL_ERROR_PENALTY_MS,
    STT_WORKER_RECV_TIMEOUT_MS, STT_WORKER_THREAD_PRIORITY,
};
use crate::core::events::VoxEvent;
use crate::services::stt::providers::SttProvider;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub enum SttCommand {
    Partial(u32, Vec<f32>),
    Final(u32, Vec<f32>),
    ResetStream,
    Shutdown,
}

pub struct SttActorChannels {
    pub rx: std::sync::mpsc::Receiver<SttCommand>,
    pub pipeline_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
}

pub struct SttActorHandles {
    pub cancel_flag: Arc<AtomicBool>,
    pub is_loaded: Arc<AtomicBool>,
    pub engine_shutdown: Arc<AtomicBool>,
}

struct WorkerState {
    last_emit_time: Instant,
    last_transcript: String,
    current_active_turn: u32,
    last_inference_duration: Duration,
}

struct WorkerContext<'a, R: tauri::Runtime> {
    app: &'a AppHandle<R>,
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
    if let SttCommand::Partial(mut tid, mut utterance) = cmd {
        let mut skipped = 0;
        while let Ok(next_cmd) = rx.try_recv() {
            match next_cmd {
                SttCommand::Partial(next_tid, next_utterance) => {
                    tid = next_tid;
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
        SttCommand::Partial(tid, utterance)
    } else {
        cmd
    }
}

/// Dispatches a partial transcript event to the pipeline event channel if changed.
fn emit_partial_event<R: tauri::Runtime>(
    ctx: &WorkerContext<'_, R>,
    tid: u32,
    text: String,
    state: &mut WorkerState,
) {
    if !text.is_empty() && text != state.last_transcript {
        if let Some(ref pipeline_tx) = ctx.pipeline_event_tx {
            if let Err(e) = pipeline_tx.send(VoxEvent::TranscriptPartial {
                turn_id: tid,
                text: text.clone(),
            }) {
                log::warn!("[STT] Error dispatching partial transcript: {:?}", e);
            }
        }
        state.last_transcript = text;
    }
}

/// Processes incoming partial speech frames with dynamic throttling and emits partial events.
fn handle_partial_command<R: tauri::Runtime>(
    ctx: &WorkerContext<'_, R>,
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

/// Emits the final or cancelled turn event to the pipeline event channel.
fn emit_final_events<R: tauri::Runtime>(ctx: &WorkerContext<'_, R>, tid: u32, transcript: String) {
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
            text: transcript,
        }) {
            log::warn!("[STT] Error sending final transcript event: {:?}", e);
        }
    }

    if let Err(e) = ctx
        .app
        .emit_to("main", "ptt_status", serde_json::json!({ "state": "IDLE" }))
    {
        log::warn!("[STT] Error emitting ptt_status idle event: {:?}", e);
    }
}

/// Transcribes final speech buffer, dispatches final events, and resets worker state.
fn handle_final_command<R: tauri::Runtime>(
    ctx: &WorkerContext<'_, R>,
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
            SttCommand::Partial(..) | SttCommand::ResetStream => continue,
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
fn run_worker_loop<R: tauri::Runtime>(
    app: &AppHandle<R>,
    provider: &dyn SttProvider,
    channels: SttActorChannels,
    handles: SttActorHandles,
) {
    let ctx = WorkerContext {
        app,
        provider,
        pipeline_event_tx: &channels.pipeline_event_tx,
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
        if handles.engine_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            log::info!("[STT] Engine shutdown flag detected. Exiting loop.");
            break;
        }

        let raw_cmd = if let Some(c) = pending_cmd.take() {
            c
        } else {
            match channels.rx.recv_timeout(Duration::from_millis(STT_WORKER_RECV_TIMEOUT_MS)) {
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
            SttCommand::Partial(tid, utterance) => {
                handle_partial_command(&ctx, tid, &utterance, &mut state);
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
    handles.is_loaded.store(false, std::sync::atomic::Ordering::Relaxed);
    log::info!("[STT] Worker thread exiting.");
}

/// Spawns dedicated OS worker thread for speech recognition inference and event dispatching.
pub fn spawn_stt_worker<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
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
            handles.is_loaded.store(true, std::sync::atomic::Ordering::Relaxed);

            run_worker_loop(&app, &*provider, channels, handles);
        })
        .map_err(|e| e.to_string())
}

