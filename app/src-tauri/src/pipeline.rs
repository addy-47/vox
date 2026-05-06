//! Pipeline Orchestrator — event-driven coordination of LLM→TTS→Playback.
//!
//! Receives VoxEvents from the STT layer and drives the downstream pipeline.
//! All inference workers run on dedicated OS threads (not tokio). This module
//! is the coordination layer — it owns the channels and the cancellation atomics.
//!
//! Directive 2: Sub-sentence chunker flushes to TTS on `.!?,;—` or ≥6 words.

use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::events::VoxEvent;
use crate::metrics::{MetricField, PipelineMetrics};
use crate::settings::{AudioOutputMode, VoxSettings};

// ─── Directive 2: Sub-Sentence Chunker ───────────────────────────────────────

/// Returns `true` if the accumulated token buffer should be flushed to TTS.
///
/// Flush conditions (in priority order):
///   1. Hard boundaries: `.` `!` `?`
///   2. Soft boundaries: `,` `;` ` — ` ` - `
///   3. Word count limit: ≥ 6 words accumulated without any boundary
///
/// This guarantees Time-to-First-Audio ≤ ~500ms regardless of LLM sentence length.
#[inline]
pub fn should_flush(buf: &str, word_count: usize) -> bool {
    let trimmed = buf.trim_end();
    let last = trimmed.chars().last().unwrap_or(' ');

    // Hard boundaries — always flush
    if matches!(last, '.' | '!' | '?') {
        return true;
    }

    // Soft boundaries — flush to begin audio early
    if matches!(last, ',' | ';') {
        return true;
    }
    if trimmed.ends_with(" — ") || trimmed.ends_with(" - ") {
        return true;
    }

    // Word count gate — prevent long-sentence lag (Directive 2)
    word_count >= 6
}

/// Count words in the accumulated buffer.
#[inline]
fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

// ─── Pipeline Orchestrator ────────────────────────────────────────────────────

pub struct PipelineOrchestrator {
    cancel_flag:     Arc<AtomicBool>,
    playback_active: Arc<AtomicBool>,
    llm_generating:  Arc<AtomicBool>,
    tts_generating:  Arc<AtomicBool>,
    session_id:      Arc<AtomicU32>,
    event_tx:        Sender<VoxEvent>,
    settings:        VoxSettings,
}

impl PipelineOrchestrator {
    pub fn new(
        cancel_flag:     Arc<AtomicBool>,
        playback_active: Arc<AtomicBool>,
        llm_generating:  Arc<AtomicBool>,
        tts_generating:  Arc<AtomicBool>,
        session_id:      Arc<AtomicU32>,
        event_tx:        Sender<VoxEvent>,
        settings:        VoxSettings,
    ) -> Self {
        Self {
            cancel_flag,
            playback_active,
            llm_generating,
            tts_generating,
            session_id,
            event_tx,
            settings,
        }
    }

    /// Handle a `TranscriptFinal` event: cancel any in-progress turn and
    /// spawn a new LLM worker for the new transcript.
    pub fn on_transcript_final(&self, text: String, app_handle: tauri::AppHandle) {
        // Cancel any existing turn
        self.cancel_flag.store(true, Ordering::Relaxed);

        // Bump session ID — stale events from previous turn are ignored by consumers
        let new_session = self.session_id.fetch_add(1, Ordering::Relaxed) + 1;
        log::info!("[Pipeline] New session {} — transcript: {:?}", new_session, text);

        // Reset cancellation flag for the new turn
        self.cancel_flag.store(false, Ordering::Relaxed);

        let cancel      = Arc::clone(&self.cancel_flag);
        let llm_flag    = Arc::clone(&self.llm_generating);
        let event_tx    = self.event_tx.clone();
        let llm_path    = self.settings.llm_model_path.clone();
        let ctx_size    = self.settings.llm_ctx_size;
        let n_threads   = self.settings.llm_threads;

        // LLM runs on a dedicated OS thread — never tokio
        std::thread::Builder::new()
            .name(format!("vox-llm-{}", new_session))
            .spawn(move || {
                llm_flag.store(true, Ordering::Relaxed);
                log::info!("[LLM Thread] session={}", new_session);

                // Resolve symlinks for HuggingFace hub paths
                let resolved = llm_path.canonicalize()
                    .unwrap_or_else(|_| llm_path.clone());

                match crate::llm::LlmWorker::new(&resolved, ctx_size, n_threads) {
                    Ok(worker) => {
                        if let Err(e) = worker.generate(&text, new_session, &cancel, &event_tx) {
                            log::error!("[LLM] Generation error (session {}): {}", new_session, e);
                            let _ = event_tx.blocking_send(VoxEvent::Error {
                                session_id: new_session,
                                message: e.to_string(),
                            });
                        }
                    }
                    Err(e) => {
                        log::error!("[LLM] Init error (session {}): {}", new_session, e);
                        let _ = event_tx.blocking_send(VoxEvent::Error {
                            session_id: new_session,
                            message: e.to_string(),
                        });
                    }
                }

                llm_flag.store(false, Ordering::Relaxed);
                log::info!("[LLM Thread] Done (session={})", new_session);
            })
            .expect("[Pipeline] Failed to spawn LLM thread");
    }

    /// Process the internal event bus in a blocking loop.
    ///
    /// Handles: LlmToken (sub-sentence chunking) → TTS dispatch
    ///          TtsChunk → Playback ingestion
    ///          SpeechStart (headset mode barge-in) → cancellation
    pub fn run_event_loop(
        &self,
        mut rx: Receiver<VoxEvent>,
        tts_model_dir: PathBuf,
        playback_engine: Arc<crate::playback::PlaybackEngine>,
        app_handle: tauri::AppHandle,
    ) {
        // TTS runs on its own thread, receives text chunks via a channel
        let (tts_tx, tts_rx) = mpsc::channel::<(u32, String)>(32);

        // Spawn the TTS worker thread
        let cancel_tts   = Arc::clone(&self.cancel_flag);
        let tts_flag     = Arc::clone(&self.tts_generating);
        let event_tx     = self.event_tx.clone();
        let tts_dir      = tts_model_dir.clone();

        std::thread::Builder::new()
            .name("vox-tts".to_string())
            .spawn(move || {
                let engine = match crate::tts::TtsEngine::new(&tts_dir) {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("[TTS Thread] Init failed: {}", e);
                        return;
                    }
                };

                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut rx = tts_rx;
                    while let Some((session_id, text)) = rx.recv().await {
                        if cancel_tts.load(Ordering::Relaxed) {
                            continue;
                        }
                        tts_flag.store(true, Ordering::Relaxed);
                        if let Err(e) = engine.synthesize_chunk(&text, session_id, &cancel_tts, &event_tx) {
                            log::error!("[TTS] Synthesis error (session {}): {}", session_id, e);
                        }
                        tts_flag.store(false, Ordering::Relaxed);
                    }
                });
            })
            .expect("[Pipeline] Failed to spawn TTS thread");

        // ── Directive 2: Sub-sentence token accumulator ───────────────────────
        let mut token_buf    = String::new();
        let mut word_count   = 0usize;
        let mut first_flush  = true;
        let mut current_sid  = 0u32;
        let mut metrics      = PipelineMetrics::new();
        let audio_mode       = self.settings.audio_output_mode.clone();
        let cancel           = Arc::clone(&self.cancel_flag);

        // Process events synchronously — this runs on the pipeline coordinator thread
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    // ── Speech start: headset barge-in cancellation ───────────
                    VoxEvent::SpeechStart { session_id } => {
                        metrics.mark(MetricField::SpeechStart);
                        if audio_mode == AudioOutputMode::Headset
                            && playback_engine.is_idle() == false
                        {
                            log::info!("[Pipeline] Barge-in detected (headset) — cancelling turn {}", session_id);
                            cancel.store(true, Ordering::Relaxed);
                            playback_engine.cancel();
                        }
                        let _ = app_handle.emit("speech_start", ());
                    }

                    // ── Transcript final: hand off to LLM ────────────────────
                    VoxEvent::TranscriptFinal { session_id, text } => {
                        current_sid = session_id;
                        token_buf.clear();
                        word_count  = 0;
                        first_flush = true;
                        metrics.mark(MetricField::FinalTranscript);
                        log::info!("[Pipeline] TranscriptFinal sid={}: {:?}", session_id, text);
                        // Emit to frontend for display
                        let _ = app_handle.emit("transcript_final", &text);
                        // LLM is spawned externally via on_transcript_final
                    }

                    // ── LLM token: accumulate + sub-sentence chunking ─────────
                    VoxEvent::LlmToken { session_id, token } => {
                        if session_id != current_sid { continue; }
                        if metrics.first_token.is_none() {
                            metrics.mark(MetricField::FirstToken);
                        }

                        token_buf.push_str(&token);
                        word_count = count_words(&token_buf);

                        // Directive 2: check flush condition
                        if should_flush(&token_buf, word_count) {
                            let chunk = token_buf.trim().to_string();
                            if !chunk.is_empty() {
                                log::debug!("[Pipeline] Flushing to TTS: {:?} ({} words)", chunk, word_count);
                                metrics.mark(MetricField::TtsStart);
                                let _ = tts_tx.send((session_id, chunk)).await;
                            }
                            token_buf.clear();
                            word_count  = 0;
                            first_flush = false;
                        }

                        // Forward token to frontend for streaming display
                        let _ = app_handle.emit("llm_token", &token);
                    }

                    // ── LLM finished: flush any remaining buffer ───────────────
                    VoxEvent::LlmFinished { session_id } => {
                        if session_id != current_sid { continue; }
                        let remainder = token_buf.trim().to_string();
                        if !remainder.is_empty() {
                            log::debug!("[Pipeline] Final flush to TTS: {:?}", remainder);
                            let _ = tts_tx.send((session_id, remainder)).await;
                        }
                        token_buf.clear();
                        word_count = 0;
                        log::info!("[Pipeline] LLM finished (session {})", session_id);
                    }

                    // ── TTS chunk: ingest into playback buffer ─────────────────
                    VoxEvent::TtsChunk { session_id, samples } => {
                        if session_id != current_sid { continue; }
                        if metrics.first_audio.is_none() {
                            metrics.mark(MetricField::FirstAudio);
                        }
                        // upsample_2x runs inside ingest_chunk (Directive 3)
                        playback_engine.ingest_chunk(&samples);
                        if metrics.playback_start.is_none() && !playback_engine.is_idle() {
                            metrics.mark(MetricField::PlaybackStart);
                            let _ = app_handle.emit("playback_started", ());
                        }
                    }

                    // ── Playback finished ─────────────────────────────────────
                    VoxEvent::PlaybackFinished { session_id } => {
                        if session_id != current_sid { continue; }
                        metrics.mark(MetricField::PlaybackFinish);
                        let report = metrics.latency_report();
                        log::info!("[Pipeline] Turn complete. Latencies: {}", report);
                        let _ = app_handle.emit("playback_finished", &report);
                        metrics.reset();
                    }

                    // ── Cancellation ──────────────────────────────────────────
                    VoxEvent::Cancelled { session_id } => {
                        log::info!("[Pipeline] Cancelled (session {})", session_id);
                        playback_engine.cancel();
                        token_buf.clear();
                        word_count = 0;
                        let _ = app_handle.emit("pipeline_cancelled", session_id);
                    }

                    VoxEvent::Error { session_id, message } => {
                        log::error!("[Pipeline] Error (session {}): {}", session_id, message);
                        let _ = app_handle.emit("pipeline_error", &message);
                    }

                    _ => {} // SpeechEnd, TtsFinished, etc. — no action needed
                }
            }
        });
    }
}
