//! Pipeline Orchestrator — event-driven coordination of LLM→TTS→Playback.
//!
//! Receives VoxEvents from the STT layer and drives the downstream pipeline.
//! All inference workers run on dedicated OS threads (not tokio). This module
//! is the coordination layer — it owns the channels and the cancellation atomics.
//!
//! Directive 2: Sub-sentence chunker flushes to TTS on `.!?,;—` or ≥6 words.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::core::events::VoxEvent;
use crate::core::metrics::{MetricField, PipelineMetrics};
use crate::core::settings::{AudioOutputMode, VoxSettings};

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

/// Detect if string contains Devanagari (Hindi) characters.
pub fn is_devanagari(text: &str) -> bool {
    text.chars().any(|c| c >= '\u{0900}' && c <= '\u{097F}')
}

// ─── Pipeline Orchestrator ────────────────────────────────────────────────────

pub enum PipelineState {
    Cold,
    Warm,
}

pub struct PipelineOrchestrator {
    cancel_flag:      Arc<AtomicBool>,
    _playback_active:  Arc<AtomicBool>,
    llm_generating:   Arc<AtomicBool>,
    tts_generating:   Arc<AtomicBool>,
    session_id:       Arc<AtomicU32>,
    event_tx:         Sender<VoxEvent>,
    settings:         VoxSettings,
    
    // Lifecycle management
    llm_tx:           Arc<std::sync::Mutex<Option<tokio::sync::mpsc::Sender<crate::services::llm::LlmCommand>>>>,
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
            _playback_active: playback_active,
            llm_generating,
            tts_generating,
            session_id,
            event_tx,
            settings,
            llm_tx: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Lazily initialize the LLM worker if it's not already running.
    pub fn warm_up_llm(&self) -> Result<(), String> {
        let mut lock = self.llm_tx.lock().map_err(|e| e.to_string())?;
        if lock.is_some() {
            return Ok(());
        }

        log::info!("[Pipeline] Warming up LLM worker...");
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        
        let llm_path    = self.settings.llm_model_path.clone();
        let ctx_size    = self.settings.llm_ctx_size;
        let n_threads   = self.settings.llm_threads;
        let event_tx    = self.event_tx.clone();
        let llm_flag    = Arc::clone(&self.llm_generating);

        std::thread::Builder::new()
            .name("vox-llm-persistent".to_string())
            .spawn(move || {
                llm_flag.store(true, Ordering::Relaxed);
                
                // Resolve symlinks for HuggingFace hub paths
                let resolved = llm_path.canonicalize()
                    .unwrap_or_else(|_| llm_path.clone());

                match crate::services::llm::LlmWorker::new(&resolved, ctx_size, n_threads) {
                    Ok(worker) => {
                        llm_flag.store(false, Ordering::Relaxed);
                        worker.run_loop(rx, event_tx);
                    }
                    Err(e) => {
                        log::error!("[LLM Init] Failed: {}", e);
                        llm_flag.store(false, Ordering::Relaxed);
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        *lock = Some(tx);
        Ok(())
    }

    /// Shutdown the LLM worker and release memory.
    pub fn cool_down_llm(&self) {
        let mut lock = self.llm_tx.lock().unwrap();
        if let Some(tx) = lock.take() {
            log::info!("[Pipeline] Cooling down LLM worker (releasing memory)...");
            let _ = tx.blocking_send(crate::services::llm::LlmCommand::Shutdown);
        }
    }

    /// Handle a `TranscriptFinal` event: ensure LLM is warm and send generation command.
    pub fn on_transcript_final(&self, text: String, _app_handle: tauri::AppHandle) {
        // Cancel any existing turn
        self.cancel_flag.store(true, Ordering::Relaxed);

        // Bump session ID
        let new_session = self.session_id.fetch_add(1, Ordering::Relaxed) + 1;
        log::info!("[Pipeline] New session {} — transcript: {:?}", new_session, text);

        // Reset cancellation flag
        self.cancel_flag.store(false, Ordering::Relaxed);

        // Ensure LLM is warm
        if let Err(e) = self.warm_up_llm() {
            log::error!("[Pipeline] Failed to warm up LLM: {}", e);
            return;
        }

        let lock = self.llm_tx.lock().unwrap();
        if let Some(tx) = &*lock {
            let cmd = crate::services::llm::LlmCommand::Generate {
                text,
                session_id: new_session,
                cancel_flag: Arc::clone(&self.cancel_flag),
            };
            
            if let Err(e) = tx.blocking_send(cmd) {
                log::error!("[Pipeline] Failed to send generate command to LLM: {}", e);
            }
        }
    }

    /// Process the internal event bus in a blocking loop.
    ///
    /// Handles: LlmToken (sub-sentence chunking) → TTS dispatch
    ///          TtsChunk → Playback ingestion
    ///          SpeechStart (headset mode barge-in) → cancellation
    pub fn run_event_loop(
        &self,
        mut rx: Receiver<VoxEvent>,
        en_tts_dir: PathBuf,
        hi_tts_dir: PathBuf,
        playback_engine: Arc<crate::services::playback::PlaybackEngine>,
        app_handle: tauri::AppHandle,
    ) {
        // TTS runs on its own thread, receives text chunks via a channel
        let (tts_tx, tts_rx) = std::sync::mpsc::channel::<(u32, i32, String)>();

        // Spawn the TTS worker thread
        let cancel_tts   = Arc::clone(&self.cancel_flag);
        let tts_flag     = Arc::clone(&self.tts_generating);
        let event_tx     = self.event_tx.clone();

        std::thread::Builder::new()
            .name("vox-tts".to_string())
            .spawn(move || {
                let mut engine = match crate::services::tts::TtsEngine::new(&en_tts_dir, &hi_tts_dir) {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("[TTS Thread] Init failed: {}", e);
                        return;
                    }
                };

                loop {
                    let (session_id, voice_sid, text) = match tts_rx.recv() {
                        Ok(data) => data,
                        Err(_) => break, // Channel closed
                    };

                    if cancel_tts.load(Ordering::Relaxed) {
                        continue;
                    }
                    tts_flag.store(true, Ordering::Relaxed);
                    if let Err(e) = engine.synthesize_chunk(&text, voice_sid, session_id, cancel_tts.clone(), event_tx.clone()) {
                        log::error!("[TTS] Synthesis error (session {}): {}", session_id, e);
                    }
                    tts_flag.store(false, Ordering::Relaxed);
                }
            })
            .expect("[Pipeline] Failed to spawn TTS thread");

        // ── Directive 2: Sub-sentence token accumulator ───────────────────────
        let mut token_buf    = String::new();
        let mut current_sid  = 0u32;
        let mut voice_sid    = 0i32; // Default to English Female
        let mut thinking     = false;
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
                        voice_sid = 0;   // Reset to English default
                        thinking = false; // Reset thinking state
                        metrics.mark(MetricField::FinalTranscript);
                        log::info!("[Pipeline] TranscriptFinal sid={}: {:?}", session_id, text);
                        // Emit to frontend for display
                        let _ = app_handle.emit("transcript_final", &text);
                        // LLM is spawned externally via on_transcript_final
                    }

                    // ── LLM token: accumulate + sub-sentence chunking ─────────
                    VoxEvent::LlmToken { session_id, token } => {
                        if session_id != current_sid { continue; }
                        
                        // 1. Stateful thinking-block detection (Directive: Drop thoughts from TTS)
                        if token.contains("<|channel>thought") {
                            thinking = true;
                            log::debug!("[Pipeline] Entered thinking mode — silencing TTS");
                            continue;
                        }
                        if token.contains("<channel|>") {
                            thinking = false;
                            log::debug!("[Pipeline] Exited thinking mode");
                            continue;
                        }
                        if thinking { continue; }

                        if metrics.first_token.is_none() {
                            metrics.mark(MetricField::FirstToken);
                        }

                        token_buf.push_str(&token);
                        
                        // 2. Language detection (Devanagari check)
                        if is_devanagari(&token_buf) {
                            voice_sid = 10; // hf_alpha (Hindi Female)
                        }

                        let words = count_words(&token_buf);

                        // Directive 2: check flush condition
                        if should_flush(&token_buf, words) {
                            let chunk = token_buf.trim().to_string();
                            if !chunk.is_empty() {
                                log::debug!("[Pipeline] Flushing to TTS (sid={}): {:?} ({} words)", voice_sid, chunk, words);
                                metrics.mark(MetricField::TtsStart);
                                let _ = tts_tx.send((session_id, voice_sid, chunk));
                            }
                            token_buf.clear();
                        }

                        // Forward token to frontend for streaming display
                        let _ = app_handle.emit("llm_token", &token);
                    }

                    // ── LLM finished: flush any remaining buffer ───────────────
                    VoxEvent::LlmFinished { session_id } => {
                        if session_id != current_sid { continue; }
                        thinking = false; // Reset just in case
                        let remainder = token_buf.trim().to_string();
                        if !remainder.is_empty() {
                            log::debug!("[Pipeline] Final flush to TTS (sid={}): {:?}", voice_sid, remainder);
                            let _ = tts_tx.send((session_id, voice_sid, remainder));
                        }
                        token_buf.clear();
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
