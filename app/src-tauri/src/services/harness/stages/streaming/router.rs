use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc::{Receiver, Sender},
    Arc,
};

use parking_lot::Mutex;
use tauri::{AppHandle, Runtime};

use crate::{
    core::{
        events::{emit_ipc_to, AudioIntent, IpcEvent, LlmTokenPayload, VoxEvent},
        metrics::TurnMetricsCollector,
        state::InteractionOwner,
    },
    pipeline::{
        assistant::TurnAccumulator,
        target_window,
    },
    services::{
        harness::stages::streaming::{ClauseChunker, TextNormalizer},
        llm::{actor::LlmResponse, CanonicalToolCall},
        tts::actor::TtsCommand,
    },
};

/// Outcome of a single streaming response pass.
#[derive(Debug, Clone)]
pub enum StreamPassOutcome {
    Completed {
        assistant_text: String,
    },
    ToolCallReceived {
        partial_text: String,
        call: CanonicalToolCall,
    },
    Cancelled {
        partial_text: String,
    },
    Error(String),
}

/// Bundled handles and shared state required to route streaming LLM responses.
pub struct StreamRoutingHandles<R: Runtime> {
    pub turn_id: u32,
    pub owner: InteractionOwner,
    pub accumulator: Arc<Mutex<TurnAccumulator>>,
    pub tts_tx: Option<Sender<TtsCommand>>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub cancel: Arc<AtomicBool>,
    pub event_tx: Sender<VoxEvent>,
    pub app: AppHandle<R>,
    pub turn_metrics: Arc<TurnMetricsCollector>,
}

impl<R: Runtime> Clone for StreamRoutingHandles<R> {
    fn clone(&self) -> Self {
        Self {
            turn_id: self.turn_id,
            owner: self.owner,
            accumulator: Arc::clone(&self.accumulator),
            tts_tx: self.tts_tx.clone(),
            pending_synthesis_jobs: Arc::clone(&self.pending_synthesis_jobs),
            cancel: Arc::clone(&self.cancel),
            event_tx: self.event_tx.clone(),
            app: self.app.clone(),
            turn_metrics: Arc::clone(&self.turn_metrics),
        }
    }
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
    /// emits IPC token events, and routes streaming outcomes.
    pub fn route_stream<R: Runtime + 'static>(
        &self,
        handles: StreamRoutingHandles<R>,
        response_rx: Receiver<LlmResponse>,
    ) -> Result<StreamPassOutcome, String> {
        let mut buffered_clauses: Vec<String> = Vec::new();
        let outcome = self.consume_stream_events(&handles, response_rx, &mut buffered_clauses)?;

        if let StreamPassOutcome::Completed { .. } = &outcome {
            self.dispatch_clauses(buffered_clauses, AudioIntent::TurnResponse, &handles);
            self.flush_remainder(&handles);
            self.emit_finished(&handles);
        }

        Ok(outcome)
    }

    /// Consumes stream responses until completion, tool call, cancellation, or error.
    fn consume_stream_events<R: Runtime + 'static>(
        &self,
        handles: &StreamRoutingHandles<R>,
        response_rx: Receiver<LlmResponse>,
        buffered_clauses: &mut Vec<String>,
    ) -> Result<StreamPassOutcome, String> {
        let mut saw_finished = false;

        while let Ok(response) = response_rx.recv() {
            if handles.cancel.load(Ordering::Relaxed) {
                log::info!(
                    "[Harness::Stream] Stream cancelled (turn {})",
                    handles.turn_id
                );
                self.emit_cancelled(handles);
                return Ok(StreamPassOutcome::Cancelled {
                    partial_text: handles.accumulator.lock().assistant_response.clone(),
                });
            }

            match response {
                LlmResponse::Token(token) => {
                    handles.turn_metrics.record_llm_first_token();
                    self.buffer_token(token, handles, buffered_clauses);
                }
                LlmResponse::ToolCall(call) => {
                    handles.turn_metrics.record_llm_first_token();
                    buffered_clauses.clear();
                    let partial_text = {
                        let mut acc = handles.accumulator.lock();
                        acc.chunker.clear();
                        acc.assistant_response.clone()
                    };
                    log::info!(
                        "[Harness::Stream] ToolCall '{}' ({}) received (turn {}, dropped prefix_chars {})",
                        call.name,
                        call.id,
                        handles.turn_id,
                        partial_text.len()
                    );
                    return Ok(StreamPassOutcome::ToolCallReceived { partial_text, call });
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
                    self.emit_cancelled(handles);
                    return Ok(StreamPassOutcome::Cancelled {
                        partial_text: handles.accumulator.lock().assistant_response.clone(),
                    });
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
                    return Ok(StreamPassOutcome::Error(format!(
                        "LLM stream error: {:?}",
                        err
                    )));
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

        let full_text = handles.accumulator.lock().assistant_response.clone();
        Ok(StreamPassOutcome::Completed {
            assistant_text: full_text,
        })
    }

    /// Emits IPC token event and buffers extracted clauses for pass conclusion.
    fn buffer_token<R: Runtime>(
        &self,
        token: String,
        handles: &StreamRoutingHandles<R>,
        buffered_clauses: &mut Vec<String>,
    ) {
        self.emit_token_ipc(&token, handles);
        let clauses = handles.accumulator.lock().push_token(&token);
        buffered_clauses.extend(clauses);
    }

    /// Emits a single token to the frontend IPC rail.
    fn emit_token_ipc<R: Runtime>(&self, token: &str, handles: &StreamRoutingHandles<R>) {
        let target = target_window(handles.owner);
        let payload = IpcEvent::LlmToken(LlmTokenPayload {
            turn_id: handles.turn_id,
            token: token.to_string(),
        });
        if let Err(e) = emit_ipc_to(&handles.app, target, payload) {
            log::trace!("[Harness::Stream] Failed to emit LlmToken IPC: {}", e);
        }
    }

    /// Dispatches extracted text clauses to the TTS synthesis worker with the requested intent.
    fn dispatch_clauses<R: Runtime>(
        &self,
        clauses: Vec<String>,
        intent: AudioIntent,
        handles: &StreamRoutingHandles<R>,
    ) {
        let Some(ref tx) = handles.tts_tx else {
            return;
        };

        let first_clause_id = handles.accumulator.lock().claim_clause_ids(clauses.len());
        for (index, clause) in clauses.into_iter().enumerate() {
            let normalized = TextNormalizer::normalize_for_speech(&clause);
            if normalized.is_empty() {
                continue;
            }
            let clause_id = first_clause_id + index as u32;
            let clause_chars = normalized.chars().count();
            let clause_words = normalized.split_whitespace().count();
            let queued = handles
                .pending_synthesis_jobs
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            let cmd = TtsCommand::Generate {
                turn_id: handles.turn_id,
                text: normalized,
                intent,
            };
            if let Err(e) = tx.send(cmd) {
                handles
                    .pending_synthesis_jobs
                    .fetch_sub(1, Ordering::Relaxed);
                log::warn!("[Harness::Stream] Failed to dispatch clause to TTS: {}", e);
            } else {
                handles.turn_metrics.record_tts_chunk0_dispatch();
                log::info!(
                    "[Harness::Stream] Clause dispatched (turn {}, clause {}, chars {}, words {}, intent {:?}, pending_jobs {})",
                    handles.turn_id,
                    clause_id,
                    clause_chars,
                    clause_words,
                    intent,
                    queued
                );
            }
        }
    }

    /// Dispatches a complete text string through speech normalization and clause chunking to TTS.
    pub fn dispatch_spoken_response<R: Runtime>(
        &self,
        text: &str,
        intent: AudioIntent,
        handles: &StreamRoutingHandles<R>,
    ) {
        let mut chunker = ClauseChunker::default();
        let clauses = chunker.push_str(text);
        self.dispatch_clauses(clauses, intent, handles);
        if let Some(tail) = chunker.flush() {
            self.dispatch_clauses(vec![tail], intent, handles);
        }
    }

    /// Flushes unpunctuated tail remainder to the TTS worker.
    fn flush_remainder<R: Runtime>(&self, handles: &StreamRoutingHandles<R>) {
        let remainder = handles.accumulator.lock().flush_chunker();
        let Some(remainder_text) = remainder else {
            return;
        };
        let normalized = TextNormalizer::normalize_for_speech(&remainder_text);
        if normalized.is_empty() {
            return;
        }
        let Some(ref tx) = handles.tts_tx else {
            return;
        };

        let remainder_chars = normalized.chars().count();
        let remainder_words = normalized.split_whitespace().count();
        let clause_id = handles.accumulator.lock().claim_clause_ids(1);
        handles
            .pending_synthesis_jobs
            .fetch_add(1, Ordering::Relaxed);
        let queued = handles.pending_synthesis_jobs.load(Ordering::Relaxed);
        let cmd = TtsCommand::Generate {
            turn_id: handles.turn_id,
            text: normalized,
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
    fn emit_cancelled<R: Runtime>(&self, handles: &StreamRoutingHandles<R>) {
        let event = VoxEvent::Cancelled {
            turn_id: handles.turn_id,
        };
        if let Err(e) = handles.event_tx.send(event) {
            log::warn!("[Harness::Stream] Failed to dispatch Cancelled: {}", e);
        }
    }

    /// Emits `VoxEvent::LlmFinished` upon complete stream consumption.
    pub fn emit_finished<R: Runtime>(&self, handles: &StreamRoutingHandles<R>) {
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
        handles.turn_metrics.record_llm_stats(response_chars, response_words);
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
