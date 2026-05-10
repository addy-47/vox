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
use tauri::{Emitter, Manager};

use crate::core::events::VoxEvent;
use crate::core::metrics::{MetricField, PipelineMetrics};
use crate::core::settings::{VoxSettings};
use std::sync::RwLock;
use crate::core::state::InteractionOwner;

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
    // Threshold set to 12 words: at RTF 4x, ~2s of audio takes ~8s synthesis.
    // Larger chunks = fewer gaps between playback segments on slow hardware.
    word_count >= 12
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
    state:            Arc<std::sync::Mutex<crate::core::state::InteractionState>>,
    event_tx:         std::sync::mpsc::Sender<VoxEvent>,
    settings:         Arc<RwLock<VoxSettings>>,
    is_engaged:       Arc<AtomicBool>,
    pub transcript_history: Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    
    // Lifecycle management
    llm_tx:           Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<crate::services::llm::LlmCommand>>>>,
    tts_tx:           Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<(u32, i32, String)>>>>,
}

impl PipelineOrchestrator {
    pub fn new(
        cancel_flag:     Arc<AtomicBool>,
        playback_active: Arc<AtomicBool>,
        llm_generating:  Arc<AtomicBool>,
        tts_generating:  Arc<AtomicBool>,
        session_id:      Arc<AtomicU32>,
        state:           Arc<std::sync::Mutex<crate::core::state::InteractionState>>,
        event_tx:        std::sync::mpsc::Sender<VoxEvent>,
        settings:        Arc<RwLock<VoxSettings>>,
        is_engaged:      Arc<AtomicBool>,
        transcript_history: Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    ) -> Self {
        Self {
            cancel_flag,
            _playback_active: playback_active,
            llm_generating,
            tts_generating,
            session_id,
            state,
            event_tx,
            settings,
            is_engaged,
            transcript_history,
            llm_tx: Arc::new(std::sync::Mutex::new(None)),
            tts_tx: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Initialize the LLM worker if it's not already running.
    pub fn warm_up_llm(&self) -> Result<(), String> {
        let mut lock = self.llm_tx.lock().map_err(|e| e.to_string())?;
        if lock.is_some() {
            return Ok(());
        }

        log::info!("[Pipeline] Warming up LLM worker...");
        let (tx, rx) = std::sync::mpsc::channel();
        
        let (llm_path, ctx_size, n_threads) = {
            let s = self.settings.read().map_err(|e| e.to_string())?;
            let path = crate::utils::paths::get().models.join(&s.llm.model);
            (path, s.llm.ctx_size, s.llm.threads)
        };
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

    /// Initialize the TTS worker if it's not already running.
    pub fn warm_up_tts(&self, en_tts_dir: PathBuf, hi_tts_dir: PathBuf) -> Result<(), String> {
        let mut lock = self.tts_tx.lock().map_err(|e| e.to_string())?;
        if lock.is_some() {
            return Ok(());
        }

        log::info!("[Pipeline] Warming up TTS worker...");
        let (tx, rx) = std::sync::mpsc::channel::<(u32, i32, String)>();
        
        let cancel_tts = Arc::clone(&self.cancel_flag);
        let tts_flag = Arc::clone(&self.tts_generating);
        let event_tx = self.event_tx.clone();

        std::thread::Builder::new()
            .name("vox-tts-persistent".to_string())
            .spawn(move || {
                let mut engine = match crate::services::tts::TtsEngine::new(&en_tts_dir, &hi_tts_dir) {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("[TTS Worker] Init failed: {}", e);
                        return;
                    }
                };

                log::info!("[TTS Worker] Persistent loop started.");
                while let Ok((session_id, voice_sid, text)) = rx.recv() {
                    if cancel_tts.load(Ordering::Relaxed) {
                        continue;
                    }
                    tts_flag.store(true, Ordering::Relaxed);
                    if let Err(e) = engine.synthesize_chunk(&text, voice_sid, session_id, cancel_tts.clone(), event_tx.clone()) {
                        log::error!("[TTS] Synthesis error (session {}): {}", session_id, e);
                    }
                    tts_flag.store(false, Ordering::Relaxed);
                }
                log::info!("[TTS Worker] Channel closed. Exiting thread.");
            })
            .map_err(|e| e.to_string())?;

        *lock = Some(tx);
        Ok(())
    }

    /// Update internal state and emit IPC event to the **owning** window only.
    pub fn update_interaction_state(&self, new_state: crate::core::state::InteractionState, owner: InteractionOwner, app_handle: &tauri::AppHandle) {
        let mut state_lock = self.state.lock().unwrap();
        if *state_lock != new_state {
            log::debug!("[Pipeline] State changed -> {:?} (Owner: {:?})", new_state, owner);
            *state_lock = new_state;
            
            let target = match owner {
                InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                InteractionOwner::Tray => "tray",
            };
            let _ = app_handle.emit_to(target, "state_changed", new_state);
        }
    }

    fn get_idle_state(&self) -> crate::core::state::InteractionState {
        if self.is_engaged.load(Ordering::Relaxed) {
            crate::core::state::InteractionState::Listening
        } else {
            crate::core::state::InteractionState::Idle
        }
    }

    fn get_current_owner(&self, app: &tauri::AppHandle) -> InteractionOwner {
        let state: tauri::State<'_, crate::core::state::AppState> = app.state();
        let owner = state.owner.blocking_lock();
        *owner
    }

    /// Shutdown the LLM worker and release memory.
    pub fn cool_down_llm(&self) {
        let mut lock = self.llm_tx.lock().unwrap();
        if let Some(tx) = lock.take() {
            log::info!("[Pipeline] Cooling down LLM worker (releasing memory)...");
            let _ = tx.send(crate::services::llm::LlmCommand::Shutdown);
        }
    }

    /// Handle a `TranscriptFinal` event: ensure LLM is warm and send generation command.
    pub fn on_transcript_final(&self, text: String, owner: InteractionOwner, _app_handle: tauri::AppHandle) {
        // Get the current session_id before bumping so we can cancel it
        let old_session = self.session_id.load(Ordering::Relaxed);

        // Cancel any existing turn — emit Cancelled event so the event loop
        // resets awaiting_playback_finish and drains any stale state.
        self.cancel_flag.store(true, Ordering::Relaxed);
        let _ = self.event_tx.send(VoxEvent::Cancelled { session_id: old_session });

        // Bump session ID
        let new_session = self.session_id.fetch_add(1, Ordering::Relaxed) + 1;
        log::info!("[Pipeline] New session {} (owner: {:?}) — transcript: {:?}", new_session, owner, text);

        // Store in ephemeral in-memory history if owner is Tray (skipping empty results)
        if owner == InteractionOwner::Tray && !text.trim().is_empty() {
            let mut history = self.transcript_history.lock().unwrap();
            history.push_back(text.clone());
            if history.len() > crate::core::constants::TRANSCRIPT_HISTORY_LIMIT {
                history.pop_front();
            }
        }

        // Reset cancellation flag AFTER the Cancelled event is queued
        self.cancel_flag.store(false, Ordering::Relaxed);

        // ── Phase 5 Dormancy Check ──────────────────────────────────────────
        // LLM/TTS only triggers if:
        // 1. The user explicitly engaged the main app via the Home screen.
        // 2. OR the interaction owner is already MainWindow/Ptt.
        let is_engaged = self.is_engaged.load(Ordering::Relaxed);
        let should_trigger_pipeline = is_engaged || owner != InteractionOwner::Tray;

        if !should_trigger_pipeline {
            log::info!("[Pipeline] System is dormant. Skipping LLM/TTS for Tray interaction.");
            // Reset UI state to idle since we won't be "Thinking" or "Speaking"
            self.update_interaction_state(crate::core::state::InteractionState::Idle, owner, &_app_handle);
            return;
        }

        // RCA Fix: Empty transcript handling
        if text.trim().is_empty() {
            log::info!("[Pipeline] Empty transcript received. Resetting to Listening.");
            self.update_interaction_state(crate::core::state::InteractionState::Listening, owner, &_app_handle);
            return;
        }

        // ── Active Pipeline ──────────────────────────────────────────────────
        // Ensure LLM is warm
        if let Err(e) = self.warm_up_llm() {
            log::error!("[Pipeline] Failed to warm up LLM: {}", e);
            return;
        }

        let lock = self.llm_tx.lock().unwrap();
        if let Some(tx) = &*lock {
            // RCA Fix: Ensure cancel_flag is false right before generation starts.
            // If SpeechStart set it to true but no playback was active to emit
            // a Cancelled event (which usually resets this), the LLM would stall.
            self.cancel_flag.store(false, Ordering::Relaxed);

            let cmd = crate::services::llm::LlmCommand::Generate {
                text,
                session_id: new_session,
                cancel_flag: Arc::clone(&self.cancel_flag),
            };
            
            if let Err(e) = tx.send(cmd) {
                log::error!("[Pipeline] Failed to send generate command to LLM: {}", e);
            }
        }
    }

    /// Process the internal event bus in a blocking loop.
    pub fn run_event_loop(
        &self,
        rx: std::sync::mpsc::Receiver<VoxEvent>,
        en_tts_dir: PathBuf,
        hi_tts_dir: PathBuf,
        playback_engine: Arc<crate::services::playback::PlaybackEngine>,
        app_handle: tauri::AppHandle,
    ) {
        // Directive 2: Sub-sentence token accumulator ───────────────────────
        let mut token_buf    = String::new();
        let mut current_sid  = 0u32;
        let mut voice_sid    = 0i32; // Default to English Female
        let mut thinking     = false;
        let mut metrics      = PipelineMetrics::new();
        let _audio_mode = {
            let s = self.settings.read().unwrap();
            s.audio.output_mode.clone()
        };
        // True after LlmFinished: we're waiting for TTS+Playback to drain
        let mut awaiting_playback_finish = false;

        // Use recv_timeout so we can poll playback state to detect when audio drains.
        // 150ms is frequent enough for responsive state transitions without CPU waste.
        log::info!("[Pipeline] Event loop starting...");
        loop {
            let event = match rx.recv_timeout(std::time::Duration::from_millis(150)) {
                Ok(e) => e,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Poll: if LLM+TTS are done and playback just drained → finalize turn
                    if awaiting_playback_finish
                        && playback_engine.is_idle()
                        && !self.tts_generating.load(Ordering::Relaxed)
                    {
                        awaiting_playback_finish = false;
                        metrics.mark(MetricField::PlaybackFinish);
                        let report = metrics.latency_report();
                        log::info!("[Pipeline] Turn complete (polled). Latencies: {}", report);
                        let owner = self.get_current_owner(&app_handle);
                        self.update_interaction_state(self.get_idle_state(), owner, &app_handle);
                        metrics.reset();
                    }
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };
            match event {
                VoxEvent::Shutdown => {
                    log::info!("[Pipeline] Shutdown signal received. Exiting event loop.");
                    if let Ok(mut lock) = self.tts_tx.lock() {
                        *lock = None; // Dropping the sender will close the worker thread
                    }
                    break;
                }
                // ── Pre-warm: load LLM and TTS in background on engage ───────
                VoxEvent::WarmUp => {
                    if let Err(e) = self.warm_up_llm() {
                        log::error!("[Pipeline] WarmUp (LLM): failed: {}", e);
                    }
                    if let Err(e) = self.warm_up_tts(en_tts_dir.clone(), hi_tts_dir.clone()) {
                        log::error!("[Pipeline] WarmUp (TTS): failed: {}", e);
                    }
                    log::info!("[Pipeline] WarmUp: workers started in background.");
                }
                // ── Speech start: barge-in cancellation ───────────
                VoxEvent::SpeechStart { session_id, owner } => {
                    let buffer_len = playback_engine.buffer_len();
                    // Only log as "Barge-in" if there is significant audio left (>50ms at 48kHz)
                    if buffer_len > 2400 {
                        log::info!("[Pipeline] Barge-in detected — cancelling turn {} ({} samples left)", session_id, buffer_len);
                        self.cancel_flag.store(true, Ordering::Relaxed);
                        playback_engine.cancel();
                        awaiting_playback_finish = false;
                        self.update_interaction_state(crate::core::state::InteractionState::Interrupted, owner, &app_handle);
                    } else if !playback_engine.is_idle() {
                        // Trailing silence or very short audio — cancel silently
                        playback_engine.cancel();
                        awaiting_playback_finish = false;
                        self.update_interaction_state(crate::core::state::InteractionState::UserSpeaking, owner, &app_handle);
                    } else {
                        self.update_interaction_state(crate::core::state::InteractionState::UserSpeaking, owner, &app_handle);
                    }
                }

                // ── Transcript partial: update HUD UI ─────────────────────
                VoxEvent::TranscriptPartial { session_id, owner, text } => {
                    if session_id < current_sid { continue; }
                    let target = match owner {
                        crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                    };
                    let _ = app_handle.emit_to(target, "transcript_partial", serde_json::json!({
                        "text": text,
                        "session_id": session_id,
                        "owner": owner
                    }));
                }

                // ── Transcript final: hand off to LLM ────────────────────
                VoxEvent::TranscriptFinal { session_id, owner, text } => {
                    if session_id < current_sid { continue; }
                    current_sid = session_id;
                    token_buf.clear();
                    voice_sid = 0;   
                    thinking = false;
                    metrics.mark(MetricField::FinalTranscript);
                    
                    self.update_interaction_state(crate::core::state::InteractionState::Thinking, owner, &app_handle);

                    let target = match owner {
                        crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                    };
                    let _ = app_handle.emit_to(target, "transcript_final", serde_json::json!({
                        "text": text.clone(),
                        "session_id": session_id,
                        "owner": owner
                    }));

                    self.on_transcript_final(text, owner, app_handle.clone());
                }

                // ── LLM token: accumulate + sub-sentence chunking ─────────
                VoxEvent::LlmToken { session_id, token } => {
                    if session_id != current_sid { continue; }
                    
                    if token.contains("<|channel>thought") {
                        thinking = true;
                        continue;
                    }
                    if token.contains("<channel|>") {
                        thinking = false;
                        continue;
                    }
                    if thinking { continue; }

                    token_buf.push_str(&token);
                    let word_count = count_words(&token_buf);

                    if word_count >= 6 || should_flush(&token_buf, word_count) {
                        let chunk = token_buf.trim().to_string();
                        if !chunk.is_empty() {
                            // Detect language for voice selection
                            voice_sid = if is_devanagari(&chunk) { 1 } else { 0 };
                            if let Ok(lock) = self.tts_tx.lock() {
                                if let Some(tx) = lock.as_ref() {
                                    let _ = tx.send((session_id, voice_sid, chunk));
                                }
                            }
                            token_buf.clear();
                        }
                    }
                     
                     let target = {
                         let state: tauri::State<'_, crate::core::state::AppState> = app_handle.state();
                         let owner = state.owner.blocking_lock();
                         match *owner {
                             crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                             crate::core::state::InteractionOwner::Tray => "tray",
                         }
                     };
                     let _ = app_handle.emit_to(target, "llm_token", &token);
                 }

                VoxEvent::LlmFinished { session_id } => {
                    if session_id != current_sid { continue; }
                    thinking = false;
                    let remainder = token_buf.trim().to_string();
                    if !remainder.is_empty() {
                        if let Ok(lock) = self.tts_tx.lock() {
                            if let Some(tx) = lock.as_ref() {
                                let _ = tx.send((session_id, voice_sid, remainder));
                            }
                        }
                    }
                    token_buf.clear();
                    // Signal that all text has been dispatched. The polling loop
                    // will detect when TTS+Playback drains and finalize the turn.
                    awaiting_playback_finish = true;
                }

                VoxEvent::TtsChunk { session_id, samples } => {
                    if session_id != current_sid { continue; }
                    if metrics.first_audio.is_none() {
                        metrics.mark(MetricField::FirstAudio);
                    }
                    playback_engine.ingest_chunk(&samples);
                    if metrics.playback_start.is_none() && !playback_engine.is_idle() {
                        metrics.mark(MetricField::PlaybackStart);
                        let owner = self.get_current_owner(&app_handle);
                        self.update_interaction_state(crate::core::state::InteractionState::AssistantSpeaking, owner, &app_handle);
                    }
                }

                VoxEvent::PlaybackFinished { session_id } => {
                    if session_id != current_sid { continue; }
                    metrics.mark(MetricField::PlaybackFinish);
                    let report = metrics.latency_report();
                    log::info!("[Pipeline] Turn complete. Latencies: {}", report);
                    let owner = self.get_current_owner(&app_handle);
                    self.update_interaction_state(self.get_idle_state(), owner, &app_handle);
                    
                    let target = {
                        let state: tauri::State<'_, crate::core::state::AppState> = app_handle.state();
                        let owner = state.owner.blocking_lock();
                        match *owner {
                            crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                            crate::core::state::InteractionOwner::Tray => "tray",
                        }
                    };
                    let _ = app_handle.emit_to(target, "playback_finished", &report);
                    metrics.reset();
                }

                VoxEvent::Cancelled { session_id } => {
                    log::info!("[Pipeline] Cancelled (session {})", session_id);
                    // Only cancel playback if it's actually active — avoid phantom
                    // "Playback Cancelled" logs when there's nothing playing.
                    if !playback_engine.is_idle() {
                        playback_engine.cancel();
                        log::info!("[Pipeline] Playback stopped (was active).");
                    }
                    token_buf.clear();
                    awaiting_playback_finish = false;
                    // Reset cancel flag so new sessions can proceed
                    self.cancel_flag.store(false, Ordering::Relaxed);
                    let owner = self.get_current_owner(&app_handle);
                    self.update_interaction_state(self.get_idle_state(), owner, &app_handle);
                }

                VoxEvent::Error { session_id, message } => {
                    log::error!("[Pipeline] Error (session {}): {}", session_id, message);
                    let target = {
                        let state: tauri::State<'_, crate::core::state::AppState> = app_handle.state();
                        let owner = state.owner.blocking_lock();
                        match *owner {
                            crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                            crate::core::state::InteractionOwner::Tray => "tray",
                        }
                    };
                    let _ = app_handle.emit_to(target, "pipeline_error", &message);
                    let owner = self.get_current_owner(&app_handle);
                    self.update_interaction_state(self.get_idle_state(), owner, &app_handle);
                }
                _ => {} 
            }
        }
    }
}
