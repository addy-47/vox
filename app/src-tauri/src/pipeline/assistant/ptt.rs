use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::AppHandle;

use crate::core::settings::{InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionState};
use crate::pipeline::assistant::interrupt::on_interrupt;
use crate::pipeline::{transition, RoutingContext};
use crate::services::stt::actor::SttCommand;
use crate::services::vad::{VadCommand, VAD_VALIDATION_TIMEOUT_MS};

/// Handles Push-To-Talk key press event, evaluating barge-in and opening the VAD validation window.
pub fn on_ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, ctx: &RoutingContext) {
    if ctx.interaction_mode == InteractionMode::Passive {
        log::debug!("[Pipeline::Ptt] PttStart dropped in Passive mode");
        return;
    }

    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        log::warn!(
            "[Pipeline::Ptt] PttStart dropped: session not active ({:?})",
            current_state
        );
        return;
    }

    if current_state == InteractionState::Listening {
        log::warn!("[Pipeline::Ptt] PttStart dropped: already Listening");
        return;
    }

    let turn_id = if current_state == InteractionState::Thinking
        || current_state == InteractionState::Speaking
    {
        on_interrupt(app, state, ctx)
    } else {
        let (new_turn_id, _) = state.pipeline.next_turn();
        state.pipeline_accumulator.lock().clear();
        state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                engine.playback_engine.cancel();
            }
        }

        transition(InteractionState::Listening, ctx, app, state);
        new_turn_id
    };

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine.vad_tx.send(VadCommand::StartWindowValidation) {
                log::warn!("[Pipeline::Ptt] Failed to start window validation: {}", e);
            }
        }
    }

    log::info!("[Pipeline::Ptt] PTT recording started (turn: {})", turn_id);
}

/// Dispatches validated speech audio to STT worker or realtime actor.
fn dispatch_ptt_speech_audio<R: tauri::Runtime>(
    turn_id: u32,
    audio: Vec<f32>,
    stt_tx: &std::sync::mpsc::Sender<SttCommand>,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    transition(InteractionState::Thinking, ctx, app, state);

    if ctx.pipeline_mode == PipelineMode::Modular {
        if let Err(e) = stt_tx.send(SttCommand::Final(turn_id, audio)) {
            log::warn!("[Pipeline::Ptt] Failed to send Final to STT: {}", e);
            transition(InteractionState::Ready, ctx, app, state);
        }
    } else if ctx.pipeline_mode == PipelineMode::Realtime {
        let i16_samples: Vec<i16> = audio
            .iter()
            .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        if let Ok(guard) = state.realtime_engine.try_lock() {
            if let Some(ref rt_actor) = *guard {
                if let Err(e) = rt_actor.signal_speech_committed(&i16_samples) {
                    log::warn!("[Pipeline::Ptt] Failed to commit speech turn: {}", e);
                }
            }
        }
    }
}

/// Handles Push-To-Talk key release event, querying VAD validation and dispatching audio if confirmed.
pub fn on_ptt_stop<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, ctx: &RoutingContext) {
    if ctx.interaction_mode == InteractionMode::Passive {
        log::debug!("[Pipeline::Ptt] PttStop dropped in Passive mode");
        return;
    }

    if state.pipeline.state() != InteractionState::Listening {
        log::debug!("[Pipeline::Ptt] PttStop dropped: state is not Listening");
        return;
    }

    let turn_id = state.pipeline.peek_turn_id();
    let (vad_tx, stt_tx) = {
        match state.engine.try_lock() {
            Ok(guard) => match guard.as_ref() {
                Some(engine) => (engine.vad_tx.clone(), engine.stt_tx.clone()),
                None => {
                    log::error!("[Pipeline::Ptt] Engine not ready on PTT stop");
                    return;
                }
            },
            Err(_) => {
                log::warn!("[Pipeline::Ptt] Engine lock contended on PTT stop; discarding hold");
                transition(InteractionState::Ready, ctx, app, state);
                return;
            }
        }
    };

    let (response_tx, response_rx) = std::sync::mpsc::channel();
    if let Err(e) = vad_tx.send(VadCommand::StopWindowValidation { response_tx }) {
        log::warn!("[Pipeline::Ptt] Failed to stop window validation: {}", e);
        transition(InteractionState::Ready, ctx, app, state);
        return;
    }

    let validation_result = response_rx
        .recv_timeout(Duration::from_millis(VAD_VALIDATION_TIMEOUT_MS))
        .ok();

    let (is_speech, audio) = match validation_result {
        Some(val) => (val.is_speech_detected, val.audio),
        None => (false, Vec::new()),
    };

    if !is_speech || audio.is_empty() {
        log::info!(
            "[Pipeline::Ptt] Non-speech PTT hold discarded (turn: {})",
            turn_id
        );
        transition(InteractionState::Ready, ctx, app, state);
        return;
    }

    dispatch_ptt_speech_audio(turn_id, audio, &stt_tx, app, state, ctx);

    log::info!("[Pipeline::Ptt] PTT stop processed (turn: {})", turn_id);
}

/// Cancels an in-progress Push-To-Talk recording without speech dispatch and restores Ready state.
pub fn on_ptt_cancel<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    if ctx.interaction_mode == InteractionMode::Passive {
        log::debug!("[Pipeline::Ptt] PttCancel dropped in Passive mode");
        return;
    }

    if state.pipeline.state() != InteractionState::Listening {
        log::debug!("[Pipeline::Ptt] PttCancel dropped: state is not Listening");
        return;
    }

    state.pipeline_accumulator.lock().clear();
    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.pipeline.turn_token().cancel();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            let (response_tx, _) = std::sync::mpsc::channel();
            if let Err(e) = engine
                .vad_tx
                .send(VadCommand::StopWindowValidation { response_tx })
            {
                log::warn!("[Pipeline::Ptt] Failed to stop window validation: {}", e);
            }
        }
    }

    transition(InteractionState::Ready, ctx, app, state);
    log::info!(
        "[Pipeline::Ptt] PTT recording cancelled (turn: {})",
        state.pipeline.peek_turn_id()
    );
}

/// Convenience test and invocation wrapper for starting PTT recording.
pub fn ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(state);
    on_ptt_start(app, state, &ctx);
    Ok(())
}

/// Convenience test and invocation wrapper for stopping PTT recording.
pub async fn ptt_stop<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(state);
    on_ptt_stop(app, state, &ctx);
    Ok(())
}

/// Convenience test and invocation wrapper for cancelling PTT recording.
pub fn ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(state);
    on_ptt_cancel(app, state, &ctx);
    Ok(())
}
