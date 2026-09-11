use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc::{Receiver, Sender},
    Arc,
};

use parking_lot::Mutex;
use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc_to, AudioIntent, IpcEvent, LlmTokenPayload, VoxEvent},
        state::InteractionOwner,
    },
    pipeline::{assistant::accumulator::TurnAccumulator, target_window},
    services::{llm::actor::LlmResponse, tts::actor::TtsCommand},
};

/// Bundled handles and shared state required to route streaming LLM responses.
pub struct StreamRoutingHandles<R: tauri::Runtime> {
    pub turn_id: u32,
    pub owner: InteractionOwner,
    pub accumulator: Arc<Mutex<TurnAccumulator>>,
    pub tts_tx: Option<Sender<TtsCommand>>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub cancel: Arc<AtomicBool>,
    pub event_tx: Sender<VoxEvent>,
    pub app: AppHandle<R>,
}

/// Plugin managing egress token stream demuxing, TTS clause dispatch, and turn finalization.
#[derive(Debug, Clone)]
pub struct StreamRoutingPlugin;

impl Default for StreamRoutingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamRoutingPlugin {
    /// Constructs a new `StreamRoutingPlugin` instance.
    pub fn new() -> Self {
        Self
    }

    /// Consumes incoming tokens over the duplex pipe, chunks into clauses for TTS,
    /// emits IPC token events, flushes tail remainder, and emits `VoxEvent::LlmFinished`.
    pub fn route_stream<R: tauri::Runtime + 'static>(
        &self,
        handles: StreamRoutingHandles<R>,
        response_rx: Receiver<LlmResponse>,
    ) -> Result<String, String> {
        while let Ok(response) = response_rx.recv() {
            if handles.cancel.load(Ordering::Relaxed) {
                log::info!(
                    "[Harness::Stream] Stream cancelled (turn {})",
                    handles.turn_id
                );
                self.emit_cancelled(&handles);
                return Ok(handles.accumulator.lock().assistant_response.clone());
            }

            match response {
                LlmResponse::Token(token) => {
                    self.handle_token(token, &handles);
                }
                LlmResponse::Finished => {
                    break;
                }
                LlmResponse::Cancelled => {
                    log::info!(
                        "[Harness::Stream] Stream cancelled (turn {})",
                        handles.turn_id
                    );
                    self.emit_cancelled(&handles);
                    return Ok(handles.accumulator.lock().assistant_response.clone());
                }
                LlmResponse::Error(err) => {
                    log::error!(
                        "[Harness::Stream] Stream error received (turn {}): {:?}",
                        handles.turn_id,
                        err
                    );
                    if let Err(e) = handles.event_tx.send(VoxEvent::Error(err)) {
                        log::warn!("[Harness::Stream] Failed to dispatch Error: {}", e);
                    }
                    return Ok(handles.accumulator.lock().assistant_response.clone());
                }
            }
        }

        self.flush_remainder(&handles);
        self.emit_finished(&handles);

        let full_text = handles.accumulator.lock().assistant_response.clone();
        Ok(full_text)
    }

    /// Emits IPC token event, pushes token to clause chunker, and dispatches clauses to TTS.
    fn handle_token<R: tauri::Runtime>(
        &self,
        token: String,
        handles: &StreamRoutingHandles<R>,
    ) {
        self.emit_token_ipc(&token, handles);
        let clauses = handles.accumulator.lock().push_token(&token);
        self.dispatch_clauses(clauses, handles);
    }

    /// Emits a single token to the frontend IPC rail.
    fn emit_token_ipc<R: tauri::Runtime>(
        &self,
        token: &str,
        handles: &StreamRoutingHandles<R>,
    ) {
        let target = target_window(handles.owner);
        let payload = IpcEvent::LlmToken(LlmTokenPayload {
            turn_id: handles.turn_id,
            token: token.to_string(),
        });
        if let Err(e) = emit_ipc_to(&handles.app, target, payload) {
            log::trace!("[Harness::Stream] Failed to emit LlmToken IPC: {}", e);
        }
    }

    /// Dispatches extracted text clauses to the TTS synthesis worker with `AudioIntent::TurnResponse`.
    fn dispatch_clauses<R: tauri::Runtime>(
        &self,
        clauses: Vec<String>,
        handles: &StreamRoutingHandles<R>,
    ) {
        let Some(ref tx) = handles.tts_tx else {
            return;
        };

        for clause in clauses {
            handles
                .pending_synthesis_jobs
                .fetch_add(1, Ordering::Relaxed);
            let cmd = TtsCommand::Generate {
                turn_id: handles.turn_id,
                text: clause,
                intent: AudioIntent::TurnResponse,
            };
            if let Err(e) = tx.send(cmd) {
                handles
                    .pending_synthesis_jobs
                    .fetch_sub(1, Ordering::Relaxed);
                log::warn!("[Harness::Stream] Failed to dispatch clause to TTS: {}", e);
            }
        }
    }

    /// Flushes unpunctuated tail remainder to the TTS worker.
    fn flush_remainder<R: tauri::Runtime>(&self, handles: &StreamRoutingHandles<R>) {
        let remainder = handles.accumulator.lock().flush_chunker();
        let Some(remainder_text) = remainder else {
            return;
        };
        let Some(ref tx) = handles.tts_tx else {
            return;
        };

        handles
            .pending_synthesis_jobs
            .fetch_add(1, Ordering::Relaxed);
        let cmd = TtsCommand::Generate {
            turn_id: handles.turn_id,
            text: remainder_text,
            intent: AudioIntent::TurnResponse,
        };
        if let Err(e) = tx.send(cmd) {
            handles
                .pending_synthesis_jobs
                .fetch_sub(1, Ordering::Relaxed);
            log::warn!(
                "[Harness::Stream] Failed to dispatch remainder to TTS: {}",
                e
            );
        }
    }

    /// Emits `VoxEvent::Cancelled` if turn was aborted during stream.
    fn emit_cancelled<R: tauri::Runtime>(&self, handles: &StreamRoutingHandles<R>) {
        let event = VoxEvent::Cancelled {
            turn_id: handles.turn_id,
        };
        if let Err(e) = handles.event_tx.send(event) {
            log::warn!("[Harness::Stream] Failed to dispatch Cancelled: {}", e);
        }
    }

    /// Emits `VoxEvent::LlmFinished` upon complete stream consumption.
    fn emit_finished<R: tauri::Runtime>(&self, handles: &StreamRoutingHandles<R>) {
        if handles.cancel.load(Ordering::Relaxed) {
            return;
        }

        let event = VoxEvent::LlmFinished {
            turn_id: handles.turn_id,
        };
        if let Err(e) = handles.event_tx.send(event) {
            log::warn!("[Harness::Stream] Failed to dispatch LlmFinished: {}", e);
        }
    }
}
