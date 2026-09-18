// NOTE: Streaming tag demuxing is intentionally deferred.
// Text flows directly through ClauseChunker to TTS for robust uninhibited pipeline audio testing.

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
pub struct StreamRoutingStage;

impl Default for StreamRoutingStage {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamRoutingStage {
    /// Constructs a new `StreamRoutingStage` instance.
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
        let mut saw_finished = false;

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
                    saw_finished = true;
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
                    if let Err(e) = handles.event_tx.send(VoxEvent::Error(err.clone())) {
                        log::warn!("[Harness::Stream] Failed to dispatch Error: {}", e);
                    }
                    return Err(format!("LLM stream error: {:?}", err));
                }
            }
        }

        if !saw_finished && !handles.cancel.load(Ordering::Relaxed) {
            log::error!(
                "[Harness::Stream] LLM stream disconnected prematurely before Finished (turn {})",
                handles.turn_id
            );
            return Err("LLM stream disconnected prematurely".to_string());
        }

        self.flush_remainder(&handles);
        self.emit_finished(&handles);

        let full_text = handles.accumulator.lock().assistant_response.clone();
        Ok(full_text)
    }

    /// Emits IPC token event, pushes token to clause chunker, and dispatches clauses to TTS.
    fn handle_token<R: tauri::Runtime>(&self, token: String, handles: &StreamRoutingHandles<R>) {
        self.emit_token_ipc(&token, handles);
        let clauses = handles.accumulator.lock().push_token(&token);
        self.dispatch_clauses(clauses, handles);
    }

    /// Emits a single token to the frontend IPC rail.
    fn emit_token_ipc<R: tauri::Runtime>(&self, token: &str, handles: &StreamRoutingHandles<R>) {
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

        let first_clause_id = handles.accumulator.lock().claim_clause_ids(clauses.len());
        for (index, clause) in clauses.into_iter().enumerate() {
            let clause_id = first_clause_id + index as u32;
            let clause_chars = clause.chars().count();
            let clause_words = clause.split_whitespace().count();
            let queued = handles
                .pending_synthesis_jobs
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            let cmd = TtsCommand::Generate {
                turn_id: handles.turn_id,
                text: clause,
                intent: AudioIntent::TurnResponse,
            };
            if let Err(e) = tx.send(cmd) {
                let _ = handles.pending_synthesis_jobs.fetch_update(
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                    |val| Some(val.saturating_sub(1)),
                );
                log::warn!("[Harness::Stream] Failed to dispatch clause to TTS: {}", e);
            } else {
                log::info!(
                    "[Harness::Stream] Clause dispatched (turn {}, clause {}, chars {}, words {}, intent {:?}, pending_jobs {})",
                    handles.turn_id,
                    clause_id,
                    clause_chars,
                    clause_words,
                    AudioIntent::TurnResponse,
                    queued
                );
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

        let remainder_chars = remainder_text.chars().count();
        let remainder_words = remainder_text.split_whitespace().count();
        let clause_id = handles.accumulator.lock().claim_clause_ids(1);
        handles
            .pending_synthesis_jobs
            .fetch_add(1, Ordering::Relaxed);
        let queued = handles.pending_synthesis_jobs.load(Ordering::Relaxed);
        let cmd = TtsCommand::Generate {
            turn_id: handles.turn_id,
            text: remainder_text,
            intent: AudioIntent::TurnResponse,
        };
        if let Err(e) = tx.send(cmd) {
            let _ = handles.pending_synthesis_jobs.fetch_update(
                Ordering::Relaxed,
                Ordering::Relaxed,
                |val| Some(val.saturating_sub(1)),
            );
            log::warn!(
                "[Harness::Stream] Failed to dispatch remainder to TTS: {}",
                e
            );
        } else {
            log::info!(
                "[Harness::Stream] Remainder flushed (turn {}, clause {}, chars {}, words {}, intent {:?}, pending_jobs {})",
                handles.turn_id,
                clause_id,
                remainder_chars,
                remainder_words,
                AudioIntent::TurnResponse,
                queued
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

        let (response_chars, response_words) = {
            let acc = handles.accumulator.lock();
            (
                acc.assistant_response.chars().count(),
                acc.assistant_response.split_whitespace().count(),
            )
        };
        log::info!(
            "[Harness::Stream] Stream finished (turn {}, response_chars {}, response_words {})",
            handles.turn_id,
            response_chars,
            response_words
        );

        let event = VoxEvent::LlmFinished {
            turn_id: handles.turn_id,
        };
        if let Err(e) = handles.event_tx.send(event) {
            log::warn!("[Harness::Stream] Failed to dispatch LlmFinished: {}", e);
        }
    }
}
