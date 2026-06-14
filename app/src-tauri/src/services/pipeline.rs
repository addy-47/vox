//! Pipeline Orchestrator — event-driven coordination of LLM→TTS→Playback.
//!
//! Receives VoxEvents from the STT layer and drives the downstream pipeline.
//! All inference workers run on dedicated OS threads (not tokio). This module
//! is the coordination layer — it owns the channels and the cancellation atomics.
//!
//! Directive 2: Sub-sentence chunker flushes to TTS on `.!?,;—` or ≥6 words.

use crate::core::events::VoxEvent;
use crate::core::metrics::{MetricField, PipelineMetrics};
use crate::core::settings::VoxSettings;
use crate::core::state::InteractionOwner;
use crate::services::utils::{count_words, is_devanagari, should_flush, transliterate_if_hi};
use crossbeam_channel::Sender;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use tauri::{Emitter, Manager};

pub enum TranslitTask {
    Token {
        turn_id: u32,
        target: String,
        token: String,
        local_transliterate_enabled: bool,
    },
    Partial {
        turn_id: u32,
        target: String,
        text: String,
        owner: InteractionOwner,
        local_transliterate_enabled: bool,
    },
    Final {
        turn_id: u32,
        target: String,
        text: String,
        owner: InteractionOwner,
        local_transliterate_enabled: bool,
    },
    Cancel {
        turn_id: u32,
    },
    Shutdown,
}

// ─── Pipeline Orchestrator ────────────────────────────────────────────────────

pub enum PipelineState {
    Cold,
    Warm,
}

pub struct PipelineOrchestrator {
    cancel_flag: Arc<AtomicBool>,
    _playback_active: Arc<AtomicBool>,
    tts_generating: Arc<AtomicBool>,
    turn_id: Arc<AtomicU32>,
    state: Arc<std::sync::Mutex<crate::core::state::InteractionState>>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    settings: Arc<RwLock<VoxSettings>>,
    llm_path: PathBuf,
    is_engaged: Arc<AtomicBool>,
    pub transcript_history: Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    pub conversation_id: Arc<std::sync::atomic::AtomicU64>,
    pub persist_tx: Option<Sender<crate::persistence::events::PersistenceEvent>>,
    pub dropped_persistence_events: Arc<std::sync::atomic::AtomicU64>,

    // Monitoring atomics
    pub latest_voice_latency_ms: Arc<std::sync::atomic::AtomicU32>,
    pub latest_tts_rtf: Arc<std::sync::atomic::AtomicU32>,
    pub latest_playback_start_ms: Arc<std::sync::atomic::AtomicU32>,

    // Lifecycle management
    llm_tx:
        Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<crate::services::llm::LlmCommand>>>>,
    tts_tx:
        Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<crate::services::tts::TtsCommand>>>>,
    pub llm_handle: Arc<std::sync::Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub tts_handle: Arc<std::sync::Mutex<Option<std::thread::JoinHandle<()>>>>,

    // Residency Flags
    pub is_llm_loaded: Arc<AtomicBool>,
    pub is_tts_loaded: Arc<AtomicBool>,
    pub is_sleeping: Arc<AtomicBool>,
}

impl PipelineOrchestrator {
    pub fn new(
        cancel_flag: Arc<AtomicBool>,
        playback_active: Arc<AtomicBool>,
        tts_generating: Arc<AtomicBool>,
        turn_id: Arc<AtomicU32>,
        state: Arc<std::sync::Mutex<crate::core::state::InteractionState>>,
        event_tx: std::sync::mpsc::Sender<VoxEvent>,
        settings: Arc<RwLock<VoxSettings>>,
        llm_path: PathBuf,
        is_engaged: Arc<AtomicBool>,
        transcript_history: Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
        conversation_id: Arc<std::sync::atomic::AtomicU64>,
        persist_tx: Option<Sender<crate::persistence::events::PersistenceEvent>>,
        dropped_persistence_events: Arc<std::sync::atomic::AtomicU64>,
        latest_voice_latency_ms: Arc<std::sync::atomic::AtomicU32>,
        latest_tts_rtf: Arc<std::sync::atomic::AtomicU32>,
        latest_playback_start_ms: Arc<std::sync::atomic::AtomicU32>,
        is_llm_loaded: Arc<AtomicBool>,
        is_tts_loaded: Arc<AtomicBool>,
        is_sleeping: Arc<AtomicBool>,
    ) -> Self {
        Self {
            cancel_flag,
            _playback_active: playback_active,
            tts_generating,
            turn_id,
            state,
            event_tx,
            settings,
            llm_path,
            is_engaged,
            transcript_history,
            conversation_id,
            persist_tx,
            dropped_persistence_events,
            latest_voice_latency_ms,
            latest_tts_rtf,
            latest_playback_start_ms,
            llm_tx: Arc::new(std::sync::Mutex::new(None)),
            tts_tx: Arc::new(std::sync::Mutex::new(None)),
            llm_handle: Arc::new(std::sync::Mutex::new(None)),
            tts_handle: Arc::new(std::sync::Mutex::new(None)),
            is_llm_loaded,
            is_tts_loaded,
            is_sleeping,
        }
    }

    /// Initialize the LLM worker if it's not already running.
    pub fn warm_up_llm(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let mut lock = self.llm_tx.lock().map_err(|e| e.to_string())?;
        if lock.is_some() {
            return Ok(());
        }

        log::info!("[Pipeline] Warming up LLM worker...");
        let (tx, rx) = std::sync::mpsc::channel();

        let (provider_config, ctx_size, n_threads) = {
            let s = self.settings.read().map_err(|e| e.to_string())?;
            (s.llm.provider.clone(), s.llm.ctx_size, s.llm.threads)
        };

        let event_tx = self.event_tx.clone();
        let is_loaded = Arc::clone(&self.is_llm_loaded);
        let app_clone = app.clone();
        *lock = Some(tx);

        let llm_path_clone = self.llm_path.clone();
        let handle = std::thread::Builder::new()
            .name("vox-llm-persistent".to_string())
            .spawn(move || {
                use crate::core::settings::LlmProviderConfig;
                use crate::services::llm::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider};
                use tauri::Emitter;

                let _ = app_clone.emit(crate::core::constants::EVENT_MODEL_LOADING, "LLM");

                let provider_res: Result<Box<dyn LlmProvider>, String> = match &provider_config {
                    LlmProviderConfig::Embedded => {
                        EmbeddedProvider::new(&llm_path_clone, ctx_size, n_threads)
                            .map(|p| Box::new(p) as Box<dyn LlmProvider>)
                            .map_err(|e| e.to_string())
                    }
                    LlmProviderConfig::OpenAiCompat {
                        base_url,
                        model,
                        api_key,
                        provider_name,
                    } => {
                        let provider = OpenAiCompatProvider::new(
                            base_url,
                            model,
                            api_key.as_deref(),
                            provider_name.as_deref(),
                        );
                        Ok(Box::new(provider) as Box<dyn LlmProvider>)
                    }
                };

                match provider_res {
                    Ok(provider) => {
                        crate::services::llm::spawn_llm_worker(
                            app_clone, rx, provider, event_tx, is_loaded,
                        );
                    }
                    Err(e) => {
                        log::error!("[LLM] CRITICAL: Failed to load provider: {}", e);
                        let _ = app_clone.emit(
                            crate::core::constants::EVENT_MODEL_FAILED,
                            format!("LLM: {}", e),
                        );
                        is_loaded.store(false, Ordering::Relaxed);
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        let mut handle_lock = self.llm_handle.lock().map_err(|e| e.to_string())?;
        *handle_lock = Some(handle);

        // Reset sleep state when warming up
        self.is_sleeping.store(false, Ordering::Relaxed);

        Ok(())
    }

    pub fn cool_down_llm(&self) {
        if let Ok(mut lock) = self.llm_tx.lock() {
            if let Some(tx) = lock.take() {
                let _ = tx.send(crate::services::llm::LlmCommand::Shutdown);
                log::info!("[Pipeline] LLM Shutdown sent (Offloading).");
            }
        }
    }

    /// Initialize the TTS worker if it's not already running.
    pub fn warm_up_tts(
        &self,
        app: &tauri::AppHandle,
        super_tts_path: PathBuf,
    ) -> Result<(), String> {
        let mut lock = self.tts_tx.lock().map_err(|e| e.to_string())?;
        if lock.is_some() {
            return Ok(());
        }

        let (quality_steps, speed) = {
            let s = self.settings.read().map_err(|e| e.to_string())?;
            (s.tts.quality_steps, s.tts.speed)
        };

        log::info!("[Pipeline] Warming up TTS worker (Supertonic)...");
        let (tx, rx) = std::sync::mpsc::channel::<crate::services::tts::TtsCommand>();

        let cancel_tts = Arc::clone(&self.cancel_flag);
        let event_tx = self.event_tx.clone();
        let is_loaded = Arc::clone(&self.is_tts_loaded);
        *lock = Some(tx);

        let app_clone = app.clone();
        let handle = std::thread::Builder::new()
            .name("vox-tts-persistent".to_string())
            .spawn(move || {
                crate::services::tts::spawn_tts_worker(
                    app_clone,
                    rx,
                    super_tts_path,
                    event_tx,
                    cancel_tts,
                    is_loaded,
                    quality_steps,
                    speed,
                );
            })
            .map_err(|e| e.to_string())?;

        let mut handle_lock = self.tts_handle.lock().map_err(|e| e.to_string())?;
        *handle_lock = Some(handle);

        // Reset sleep state when warming up
        self.is_sleeping.store(false, Ordering::Relaxed);

        Ok(())
    }

    pub fn cool_down_tts(&self) {
        if let Ok(mut lock) = self.tts_tx.lock() {
            *lock = None; // Dropping sender closes worker
            log::info!("[Pipeline] TTS Shutdown (Offloading).");
        }
    }

    /// Update internal state and emit IPC event to the **owning** window only.
    pub fn update_interaction_state(
        &self,
        new_state: crate::core::state::InteractionState,
        owner: InteractionOwner,
        app_handle: &tauri::AppHandle,
    ) {
        let mut state_lock = self.state.lock().unwrap();
        if *state_lock != new_state {
            log::debug!(
                "[Pipeline] State changed -> {:?} (Owner: {:?})",
                new_state,
                owner
            );
            *state_lock = new_state;

            let target = match owner {
                InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                InteractionOwner::Tray => "tray",
                InteractionOwner::Wizard => "wizard",
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
        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
        state
            .owner
            .load(std::sync::atomic::Ordering::Relaxed)
            .into()
    }

    /// Handle a `TranscriptFinal` event: ensure LLM is warm and send generation command.
    pub fn on_transcript_final(
        &self,
        text: String,
        owner: InteractionOwner,
        _app_handle: tauri::AppHandle,
    ) -> u32 {
        // Get the current session_id before bumping so we can cancel it
        // Get the current turn_id before bumping so we can cancel it
        let old_turn = self.turn_id.load(Ordering::Relaxed);

        // Cancel any existing turn — emit Cancelled event so the event loop
        // resets awaiting_playback_finish and drains any stale state.
        self.cancel_flag.store(true, Ordering::Relaxed);
        let _ = self
            .event_tx
            .send(VoxEvent::Cancelled { turn_id: old_turn });

        // Bump turn ID
        let new_turn = self.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
        log::info!(
            "[Pipeline] New turn {} (owner: {:?}) — transcript: {:?}",
            new_turn,
            owner,
            text
        );

        // Reset cancellation flag AFTER the Cancelled event is queued
        self.cancel_flag.store(false, Ordering::Relaxed);

        // ── Phase 5 Dormancy Check ──────────────────────────────────────────
        // LLM/TTS only triggers if:
        // 1. The user explicitly engaged the main app via the Home screen.
        // 2. OR the interaction owner is already MainWindow/Ptt.
        let is_engaged = self.is_engaged.load(Ordering::Relaxed);
        let should_trigger_pipeline =
            is_engaged || (owner != InteractionOwner::Tray && owner != InteractionOwner::Wizard);

        if !should_trigger_pipeline {
            log::info!("[Pipeline] System is dormant. Skipping LLM/TTS for Tray interaction.");
            // Reset UI state to idle since we won't be "Thinking" or "Speaking"
            self.update_interaction_state(
                crate::core::state::InteractionState::Idle,
                owner,
                &_app_handle,
            );
            return new_turn;
        }

        // RCA Fix: Empty transcript handling
        if text.trim().is_empty() {
            log::info!("[Pipeline] Empty transcript received. Resetting to Listening.");
            self.update_interaction_state(
                crate::core::state::InteractionState::Listening,
                owner,
                &_app_handle,
            );
            return new_turn;
        }

        // ── Active Pipeline ──────────────────────────────────────────────────
        // Ensure LLM is warm
        if let Err(e) = self.warm_up_llm(&_app_handle) {
            log::error!("[Pipeline] Failed to warm up LLM: {}", e);
            return new_turn;
        }

        let lock = self.llm_tx.lock().unwrap();
        if let Some(tx) = &*lock {
            // RCA Fix: Ensure cancel_flag is false right before generation starts.
            // If SpeechStart set it to true but no playback was active to emit
            // a Cancelled event (which usually resets this), the LLM would stall.
            self.cancel_flag.store(false, Ordering::Relaxed);

            let assistant_settings = self.settings.read().unwrap().assistant.clone();
            let (lang, script) = if is_devanagari(&text) {
                ("Hindi", "Devanagari")
            } else {
                ("English", "Latin")
            };
            let resolved_prompt = assistant_settings.modular_prompt
                .replace("<lang>", lang)
                .replace("<script>", script);

            // Inject expression tag instructions (Supertonic supports <laugh>, <breath>, <sigh>)
            let system_prompt = format!(
                "{} You may use <laugh>, <breath>, <sigh> tags for expressive speech.",
                resolved_prompt
            );

            let cmd = crate::services::llm::LlmCommand::Generate {
                text,
                system_prompt,
                turn_id: new_turn,
                cancel_flag: Arc::clone(&self.cancel_flag),
            };

            if let Err(e) = tx.send(cmd) {
                log::error!("[Pipeline] Failed to send generate command to LLM: {}", e);
            }
        }
        new_turn
    }

    /// Process the internal event bus in a blocking loop.
    pub fn run_event_loop(
        &self,
        rx: std::sync::mpsc::Receiver<VoxEvent>,
        super_tts_path: PathBuf,
        playback_engine: Arc<crate::services::playback::PlaybackEngine>,
        app_handle: tauri::AppHandle,
    ) {
        let mut last_interaction = std::time::Instant::now();

        // Local settings cache to avoid RwLock contention in the hot path (Directive: Real-Time Safety)
        let mut local_pipeline_mode = {
            let s = self.settings.read().unwrap();
            s.interaction.pipeline_mode.clone()
        };
        let mut local_voice = {
            let s = self.settings.read().unwrap();
            s.tts.voice
        };
        let mut local_transliterate_enabled = {
            let s = self.settings.read().unwrap();
            s.asr.transliterate_enabled
        };
        let mut local_sleep_timeout = {
            let s = self.settings.read().unwrap();
            std::time::Duration::from_secs(s.interaction.auto_sleep_timeout as u64)
        };
        let mut local_main_mode = {
            let s = self.settings.read().unwrap();
            s.interaction.main_app_mode.clone()
        };
        let mut local_quality_steps = {
            let s = self.settings.read().unwrap();
            s.tts.quality_steps
        };
        let mut local_speed = {
            let s = self.settings.read().unwrap();
            s.tts.speed
        };

        // Turn-Locked state (Directive 5: Language Detection Stability)
        let mut turn_voice_id: Option<u32> = None;

        // Directive 2: Sub-sentence token accumulator
        let mut token_buf = String::new();
        let mut current_tid = 0u32;
        let mut thinking = false;
        let mut metrics = PipelineMetrics::new();

        // True after LlmFinished: we're waiting for TTS+Playback to drain
        let mut awaiting_playback_finish = false;
        let mut tts_queued_chunks = 0usize;
        let mut tts_finished_chunks = 0usize;
        let mut tts_chunks_finished_in_turn = 0usize;

        // Turn persistence buffers
        let mut turn_user_text = String::new();
        let mut turn_assistant_text = String::new();
        let mut turn_stt_ms = 0u32;
        let mut turn_ttft_ms = 0u32;
        let mut last_tts_flush = std::time::Instant::now();
        let mut last_committed_session_id = 0u32;
        let mut turn_first_token_time: Option<std::time::Instant> = None;
        let mut turn_tokens_generated = 0usize;
        let mut turn_output_samples = 0usize;

        log::info!("[Pipeline] Event loop starting...");
        let engine_shutdown = {
            let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                app_handle.state();
            state.pipeline.engine_shutdown.clone()
        };

        let (translit_tx, translit_rx) = std::sync::mpsc::channel::<TranslitTask>();
        let app_handle_translit = app_handle.clone();
        std::thread::Builder::new()
            .name("vox-translit".into())
            .spawn(move || {
                let mut worker_turn_id = 0;
                let mut raw_accum = String::new();
                while let Ok(task) = translit_rx.recv() {
                    match task {
                        TranslitTask::Cancel { turn_id } => {
                            if turn_id >= worker_turn_id {
                                worker_turn_id = turn_id;
                                raw_accum.clear();
                            }
                        }
                        TranslitTask::Token {
                            turn_id,
                            target,
                            token,
                            local_transliterate_enabled,
                        } => {
                            if turn_id < worker_turn_id {
                                continue;
                            }
                            if turn_id > worker_turn_id {
                                worker_turn_id = turn_id;
                                raw_accum.clear();
                            }
                            raw_accum.push_str(&token);
                            let output =
                                transliterate_if_hi(&raw_accum, false, local_transliterate_enabled);
                            let _ = app_handle_translit.emit_to(&target, "llm_token", output);
                        }
                        TranslitTask::Partial {
                            turn_id,
                            target,
                            text,
                            owner,
                            local_transliterate_enabled,
                        } => {
                            if turn_id < worker_turn_id {
                                continue;
                            }
                            if turn_id > worker_turn_id {
                                worker_turn_id = turn_id;
                                raw_accum.clear();
                            }
                            let output =
                                transliterate_if_hi(&text, false, local_transliterate_enabled);
                            log::info!("[Translit] Emitting partial to {}: {:?}", target, output);
                            let _ = app_handle_translit.emit_to(
                                &target,
                                "transcript_partial",
                                serde_json::json!({
                                    "text": output, "turn_id": turn_id, "owner": owner
                                }),
                            );
                        }
                        TranslitTask::Final {
                            turn_id,
                            target,
                            text,
                            owner,
                            local_transliterate_enabled,
                        } => {
                            if turn_id < worker_turn_id {
                                continue;
                            }
                            if turn_id > worker_turn_id {
                                worker_turn_id = turn_id;
                                raw_accum.clear();
                            }
                            let output =
                                transliterate_if_hi(&text, true, local_transliterate_enabled);
                            log::info!("[Translit] Emitting final to {}: {:?}", target, output);
                            let _ = app_handle_translit.emit_to(
                                &target,
                                "transcript_final",
                                serde_json::json!({
                                    "text": output, "turn_id": turn_id, "owner": owner
                                }),
                            );
                        }
                        TranslitTask::Shutdown => break,
                    }
                }
            })
            .expect("Failed to spawn Translit worker");

        macro_rules! trigger_playback {
            ($reason:expr) => {
                playback_engine.start_playback();
                if metrics.playback_start.is_none() && !playback_engine.is_idle() {
                    metrics.mark(MetricField::PlaybackStart);
                    if let (Some(s), Some(p)) = (metrics.speech_start, metrics.playback_start) {
                        let ms = p.duration_since(s).as_millis() as u32;
                        self.latest_playback_start_ms.store(ms, Ordering::Relaxed);
                        self.latest_voice_latency_ms.store(ms, Ordering::Relaxed);
                    }
                    let owner = self.get_current_owner(&app_handle);
                    self.update_interaction_state(
                        crate::core::state::InteractionState::AssistantSpeaking,
                        owner,
                        &app_handle,
                    );
                    log::info!("[Pipeline] Playback started (Reason: {})", $reason);
                }
            };
        }

        loop {
            // Check for global engine shutdown signal
            if engine_shutdown.load(Ordering::Relaxed) {
                log::info!("[Pipeline] Engine shutdown flag detected. Exiting loop.");
                break;
            }

            // Get timeout from local cache
            let sleep_timeout = local_sleep_timeout;

            let event = match rx.recv_timeout(std::time::Duration::from_millis(150)) {
                Ok(e) => e,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Check for auto-sleep
                    if last_interaction.elapsed() > sleep_timeout
                        && !self.is_sleeping.load(Ordering::Relaxed)
                    {
                        log::info!("[Pipeline] Inactivity detected ({}s). Triggering Auto-Sleep/Timeout...", last_interaction.elapsed().as_secs());
                        self.is_sleeping.store(true, Ordering::Relaxed);

                        // Tiered offloading
                        self.cool_down_llm();
                        self.cool_down_tts();

                        let owner = self.get_current_owner(&app_handle);
                        if owner == crate::core::state::InteractionOwner::Tray {
                            log::info!("[Pipeline] Auto-Sleep Timeout: Ending Tray user session.");
                            if let Some(window) = app_handle.get_webview_window("tray") {
                                log::info!("[Pipeline] Auto-Sleep Timeout: Hiding Tray window.");
                                let _ = window.hide();
                            }
                        } else {
                            // If in Passive mode, disengage entirely
                            if self.is_engaged.load(Ordering::Relaxed)
                                && local_main_mode
                                    == crate::core::settings::InteractionMode::Passive
                            {
                                let conv_id = self.conversation_id.swap(0, Ordering::Relaxed);
                                log::info!("[Pipeline] Auto-Sleep Timeout: Disengaging passive session. Ended Session: id={}", conv_id);
                                self.is_engaged.store(false, Ordering::Relaxed);

                                // Send SessionEnded persistence event
                                if conv_id != 0 {
                                    if let Some(ref tx) = self.persist_tx {
                                        let now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis()
                                            as u64;
                                        let _ = tx.try_send(crate::persistence::events::PersistenceEvent::SessionEnded {
                                            id: conv_id,
                                            timestamp_ms: now,
                                        });
                                    }
                                }
                            }
                        }

                        let _ = app_handle.emit("auto_sleep_state", true);
                    }

                    // Poll: if LLM+TTS are done and playback just drained → finalize turn
                    if awaiting_playback_finish
                        && playback_engine.is_idle()
                        && !self.tts_generating.load(Ordering::Relaxed)
                    {
                        awaiting_playback_finish = false;
                        metrics.mark(MetricField::PlaybackFinish);
                        let input_duration = (count_words(&turn_user_text) as f64 / 2.5).max(0.5);
                        let output_duration = turn_output_samples as f64 / 24000.0;
                        let report = metrics.latency_report(input_duration, output_duration);
                        log::info!("[Pipeline] Turn complete (polled). Latencies: {}", report);
                        let owner = self.get_current_owner(&app_handle);
                        self.update_interaction_state(self.get_idle_state(), owner, &app_handle);

                        // Persist Turn
                        if let Some(ref tx) = self.persist_tx {
                            let _ = tx.try_send(
                                crate::persistence::events::PersistenceEvent::TurnCompleted {
                                    conversation_id: self.conversation_id.load(Ordering::Relaxed),
                                    turn_id: current_tid,
                                    user_text: turn_user_text.clone(),
                                    assistant_text: turn_assistant_text.clone(),
                                    stt_latency_ms: turn_stt_ms,
                                    ttft_ms: turn_ttft_ms,
                                },
                            );
                        }

                        metrics.reset();
                        turn_user_text.clear();
                        turn_assistant_text.clear();
                    }
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };

            // Activity detected — update timer
            last_interaction = std::time::Instant::now();
            if self.is_sleeping.load(Ordering::Relaxed) {
                self.is_sleeping.store(false, Ordering::Relaxed);
                let _ = app_handle.emit("auto_sleep_state", false);
            }

            match event {
                // ── Pre-warm: load LLM and TTS in background on engage ───────
                VoxEvent::WarmUp => {
                    if let Err(e) = self.warm_up_llm(&app_handle) {
                        log::error!("[Pipeline] WarmUp (LLM): failed: {}", e);
                    }
                    if let Err(e) = self.warm_up_tts(&app_handle, super_tts_path.clone()) {
                        log::error!("[Pipeline] WarmUp (TTS): failed: {}", e);
                    }
                    log::info!("[Pipeline] WarmUp: workers started in background.");
                }
                // ── Speech start: barge-in cancellation ───────────
                VoxEvent::SpeechStart { turn_id, owner } => {
                    metrics.mark(MetricField::SpeechStart);
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                            app_handle.state();
                        if let Ok(mut engine_guard) = state.realtime_engine.try_lock() {
                            if let Some(ref mut engine) = *engine_guard {
                                engine.barge_in(&playback_engine);
                            }
                        }
                        awaiting_playback_finish = false;
                        self.update_interaction_state(
                            crate::core::state::InteractionState::UserSpeaking,
                            owner,
                            &app_handle,
                        );
                        continue;
                    }
                    let buffer_len = playback_engine.buffer_len();
                    // Only log as "Barge-in" if there is significant audio left (>50ms at 48kHz)
                    if buffer_len > 2400 {
                        log::info!(
                            "[Pipeline] Barge-in detected — cancelling turn {} ({} samples left)",
                            turn_id,
                            buffer_len
                        );
                        self.cancel_flag.store(true, Ordering::Relaxed);
                        let _ = translit_tx.send(TranslitTask::Cancel { turn_id });
                        playback_engine.cancel();
                        awaiting_playback_finish = false;
                        self.update_interaction_state(
                            crate::core::state::InteractionState::Interrupted,
                            owner,
                            &app_handle,
                        );
                    } else if !playback_engine.is_idle() {
                        // Trailing silence or very short audio — cancel silently
                        playback_engine.cancel();
                        awaiting_playback_finish = false;
                        self.update_interaction_state(
                            crate::core::state::InteractionState::UserSpeaking,
                            owner,
                            &app_handle,
                        );
                    } else {
                        self.update_interaction_state(
                            crate::core::state::InteractionState::UserSpeaking,
                            owner,
                            &app_handle,
                        );
                    }
                }

                // ── Transcript partial: update HUD UI ─────────────────────
                VoxEvent::TranscriptPartial {
                    turn_id,
                    owner,
                    text,
                } => {
                    if turn_id < current_tid {
                        continue;
                    }
                    if metrics.first_partial.is_none() {
                        metrics.mark(MetricField::FirstPartial);
                    }
                    let target = match owner {
                        crate::core::state::InteractionOwner::MainWindow
                        | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                        crate::core::state::InteractionOwner::Wizard => "wizard",
                    };
                    let _ = translit_tx.send(TranslitTask::Partial {
                        turn_id,
                        target: target.to_string(),
                        text,
                        owner,
                        local_transliterate_enabled,
                    });
                }

                // ── Transcript final: hand off to LLM ────────────────────
                VoxEvent::TranscriptFinal {
                    turn_id,
                    owner,
                    text,
                } => {
                    if turn_id < current_tid
                        && local_pipeline_mode != crate::core::settings::PipelineMode::Realtime
                    {
                        continue;
                    }
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        turn_user_text = text.clone();
                        let target = match owner {
                            crate::core::state::InteractionOwner::MainWindow
                            | crate::core::state::InteractionOwner::Ptt => "main",
                            crate::core::state::InteractionOwner::Tray => "tray",
                            crate::core::state::InteractionOwner::Wizard => "wizard",
                        };
                        let _ = translit_tx.send(TranslitTask::Final {
                            turn_id,
                            target: target.to_string(),
                            text: text.clone(),
                            owner,
                            local_transliterate_enabled,
                        });
                        continue;
                    }
                    if turn_id <= last_committed_session_id {
                        log::info!("[Pipeline] Guard triggered: Skipping adjacent double-final from turn_id {} (last committed: {})", turn_id, last_committed_session_id);
                        continue;
                    }
                    last_committed_session_id = turn_id;

                    token_buf.clear();
                    turn_voice_id = None; // Reset language lock for new turn
                    thinking = false;

                    metrics.mark(MetricField::FinalTranscript);
                    metrics.mark(MetricField::LlmStart);
                    metrics.input_len_chars = text.len();

                    turn_user_text = text.clone();
                    turn_assistant_text.clear();
                    turn_first_token_time = None;
                    turn_tokens_generated = 0;
                    turn_output_samples = 0;
                    tts_queued_chunks = 0;
                    tts_finished_chunks = 0;
                    tts_chunks_finished_in_turn = 0;
                    last_tts_flush = std::time::Instant::now();
                    awaiting_playback_finish = false;

                    self.update_interaction_state(
                        crate::core::state::InteractionState::Thinking,
                        owner,
                        &app_handle,
                    );

                    let target = match owner {
                        crate::core::state::InteractionOwner::MainWindow
                        | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                        crate::core::state::InteractionOwner::Wizard => "wizard",
                    };
                    let _ = translit_tx.send(TranslitTask::Final {
                        turn_id,
                        target: target.to_string(),
                        text: text.clone(),
                        owner,
                        local_transliterate_enabled,
                    });

                    current_tid = self.on_transcript_final(text, owner, app_handle.clone());
                }

                // ── LLM token: accumulate + sub-sentence chunking ─────────
                VoxEvent::LlmToken { turn_id, token } => {
                    if turn_id != current_tid
                        && local_pipeline_mode != crate::core::settings::PipelineMode::Realtime
                    {
                        continue;
                    }
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        turn_assistant_text.push_str(&token);
                        let target = self.get_current_owner(&app_handle);
                        let target_str = match target {
                            crate::core::state::InteractionOwner::MainWindow
                            | crate::core::state::InteractionOwner::Ptt => "main",
                            crate::core::state::InteractionOwner::Tray => "tray",
                            crate::core::state::InteractionOwner::Wizard => "wizard",
                        };
                        let _ = translit_tx.send(TranslitTask::Token {
                            turn_id,
                            target: target_str.to_string(),
                            token: token.clone(),
                            local_transliterate_enabled,
                        });
                        continue;
                    }

                    if token.contains("<|channel>thought") {
                        thinking = true;
                        continue;
                    }
                    if token.contains("<channel|>") {
                        thinking = false;
                        continue;
                    }
                    if thinking {
                        continue;
                    }

                    if metrics.first_token.is_none() {
                        metrics.mark(MetricField::FirstToken);
                    }
                    metrics.tokens_generated += 1;

                    token_buf.push_str(&token);
                    turn_assistant_text.push_str(&token);

                    let first_time =
                        turn_first_token_time.get_or_insert_with(std::time::Instant::now);
                    turn_tokens_generated += 1;
                    let elapsed_secs = first_time.elapsed().as_secs_f32();
                    let tps = if elapsed_secs > 0.5 {
                        turn_tokens_generated as f32 / elapsed_secs
                    } else {
                        3.5
                    };

                    let word_count = count_words(&token_buf);
                    let elapsed_ms = last_tts_flush.elapsed().as_millis();

                    if should_flush(&token_buf, word_count, elapsed_ms, tps) {
                        let chunk = token_buf.trim().to_string();
                        if !chunk.is_empty() {
                            log::info!("[Pipeline] Flushing text chunk to TTS: {:?}", chunk);
                            if metrics.tts_start.is_none() {
                                metrics.mark(MetricField::TtsStart);
                            }
                            // Lock voice for the remainder of the turn
                            if turn_voice_id.is_none() {
                                turn_voice_id = Some(local_voice as u32);
                                log::info!(
                                    "[Pipeline] Voice locked: turn_voice_id={:?}",
                                    turn_voice_id
                                );
                            }

                            let voice_sid = turn_voice_id.unwrap_or(local_voice as u32);
                            if let Ok(lock) = self.tts_tx.lock() {
                                if let Some(tx) = lock.as_ref() {
                                    let _ = tx.send(crate::services::tts::TtsCommand::Generate {
                                        turn_id,
                                        voice_sid: voice_sid as i32,
                                        text: chunk,
                                    });
                                    tts_queued_chunks += 1;
                                    self.tts_generating.store(true, Ordering::Relaxed);
                                }
                            }
                            token_buf.clear();
                            last_tts_flush = std::time::Instant::now();
                        }
                    }

                    let target = {
                        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                            app_handle.state();
                        let owner: crate::core::state::InteractionOwner =
                            state.owner.load(Ordering::Relaxed).into();
                        match owner {
                            crate::core::state::InteractionOwner::MainWindow
                            | crate::core::state::InteractionOwner::Ptt => "main",
                            crate::core::state::InteractionOwner::Tray => "tray",
                            crate::core::state::InteractionOwner::Wizard => "wizard",
                        }
                    };
                    let _ = translit_tx.send(TranslitTask::Token {
                        turn_id,
                        target: target.to_string(),
                        token,
                        local_transliterate_enabled,
                    });
                }

                VoxEvent::LlmFinished { turn_id } => {
                    if turn_id != current_tid
                        && local_pipeline_mode != crate::core::settings::PipelineMode::Realtime
                    {
                        continue;
                    }
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        thinking = false;
                        metrics.mark(MetricField::LlmEnd);
                        metrics.output_len_chars = turn_assistant_text.len();
                        log::info!(
                            "[Pipeline] LLM finished response in Realtime mode: {:?}",
                            turn_assistant_text
                        );
                        token_buf.clear();
                        awaiting_playback_finish = true;
                        continue;
                    }
                    thinking = false;
                    metrics.mark(MetricField::LlmEnd);
                    metrics.output_len_chars = turn_assistant_text.len();
                    log::info!(
                        "[Pipeline] LLM finished response: {:?}",
                        turn_assistant_text
                    );

                    let remainder = token_buf.trim().to_string();
                    if !remainder.is_empty() {
                        log::info!(
                            "[Pipeline] Flushing remainder text chunk to TTS: {:?}",
                            remainder
                        );
                        if metrics.tts_start.is_none() {
                            metrics.mark(MetricField::TtsStart);
                        }
                        if let Ok(lock) = self.tts_tx.lock() {
                            if let Some(tx) = lock.as_ref() {
                                let voice_sid = turn_voice_id.unwrap_or(local_voice as u32);
                                let _ = tx.send(crate::services::tts::TtsCommand::Generate {
                                    turn_id,
                                    voice_sid: voice_sid as i32,
                                    text: remainder,
                                });
                                tts_queued_chunks += 1;
                                self.tts_generating.store(true, Ordering::Relaxed);
                            }
                        }
                        last_tts_flush = std::time::Instant::now();
                    }
                    token_buf.clear();
                    // Signal that all text has been dispatched. The polling loop
                    // will detect when TTS+Playback drains and finalize the turn.
                    awaiting_playback_finish = true;
                    if tts_finished_chunks >= tts_queued_chunks {
                        self.tts_generating.store(false, Ordering::Relaxed);
                        trigger_playback!("all chunks finished (LLM end)");
                    }
                }

                VoxEvent::TtsChunk { turn_id, samples } => {
                    if turn_id != current_tid {
                        continue;
                    }
                    turn_output_samples += samples.len();
                    if metrics.first_audio.is_none() {
                        metrics.mark(MetricField::FirstAudio);
                    }
                    playback_engine.ingest_chunk(&samples);

                    // Adaptive buffering: trigger playback if buffer size exceeds 1.2 seconds (57,600 samples at 48kHz)
                    if playback_engine.buffer_len() >= 57_600 {
                        trigger_playback!("buffer >= 1.2s");
                    } else if !playback_engine.is_idle() {
                        trigger_playback!("playback already active");
                    }
                }

                VoxEvent::TtsFinished { turn_id, rtf } => {
                    if turn_id != current_tid {
                        continue;
                    }
                    self.latest_tts_rtf.store(rtf.to_bits(), Ordering::Relaxed);
                    metrics.mark(MetricField::TtsEnd);
                    tts_finished_chunks += 1;
                    tts_chunks_finished_in_turn += 1;

                    if tts_chunks_finished_in_turn == 1 {
                        trigger_playback!("first chunk finished");
                    }

                    if tts_finished_chunks >= tts_queued_chunks && awaiting_playback_finish {
                        self.tts_generating.store(false, Ordering::Relaxed);
                        trigger_playback!("all chunks finished (TTS end)");
                    }
                }

                VoxEvent::SpeechEnd { turn_id: _, owner } => {
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        self.update_interaction_state(
                            crate::core::state::InteractionState::Thinking,
                            owner,
                            &app_handle,
                        );
                    }
                }

                VoxEvent::PlaybackFinished { turn_id } => {
                    if turn_id != current_tid {
                        continue;
                    }
                    metrics.mark(MetricField::PlaybackFinish);

                    let input_duration = (count_words(&turn_user_text) as f64 / 2.5).max(0.5);
                    let output_duration = turn_output_samples as f64 / 24000.0;
                    let report = metrics.latency_report(input_duration, output_duration);
                    tracing::info!("[Pipeline] Turn complete. Latencies: {}", report);

                    // Emit structured telemetry
                    let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                        app_handle.state();
                    let stt_ms = match (metrics.speech_start, metrics.final_transcript) {
                        (Some(s), Some(e)) => e.duration_since(s).as_millis() as u32,
                        _ => 0,
                    };
                    let ttft_ms = match (metrics.speech_start, metrics.first_audio) {
                        (Some(s), Some(e)) => e.duration_since(s).as_millis() as u32,
                        _ => 0,
                    };

                    let tts_rtf_val = f32::from_bits(self.latest_tts_rtf.load(Ordering::Relaxed));
                    let conv_id = self.conversation_id.load(Ordering::Relaxed);
                    let _ = state.telemetry_tx.send(
                        crate::monitoring::aggregator::TelemetryEvent::InteractionMetric {
                            conversation_id: conv_id,
                            turn_id,
                            stt_latency_ms: stt_ms,
                            ttft_ms,
                            tts_rtf: tts_rtf_val,
                        },
                    );

                    turn_stt_ms = stt_ms;
                    turn_ttft_ms = ttft_ms;

                    let owner = self.get_current_owner(&app_handle);
                    self.update_interaction_state(self.get_idle_state(), owner, &app_handle);

                    let target = {
                        let owner: crate::core::state::InteractionOwner =
                            state.owner.load(Ordering::Relaxed).into();
                        match owner {
                            crate::core::state::InteractionOwner::MainWindow
                            | crate::core::state::InteractionOwner::Ptt => "main",
                            crate::core::state::InteractionOwner::Tray => "tray",
                            crate::core::state::InteractionOwner::Wizard => "wizard",
                        }
                    };
                    let _ = app_handle.emit_to(target, "playback_finished", &report);

                    // Persist Turn
                    if let Some(ref tx) = self.persist_tx {
                        if let Err(_) = tx.try_send(
                            crate::persistence::events::PersistenceEvent::TurnCompleted {
                                conversation_id: self.conversation_id.load(Ordering::Relaxed),
                                turn_id,
                                user_text: turn_user_text.clone(),
                                assistant_text: turn_assistant_text.clone(),
                                stt_latency_ms: turn_stt_ms,
                                ttft_ms: turn_ttft_ms,
                            },
                        ) {
                            self.dropped_persistence_events
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    metrics.reset();
                    turn_user_text.clear();
                    turn_assistant_text.clear();
                }

                VoxEvent::Cancelled { turn_id } => {
                    log::info!("[Pipeline] Cancelled (turn {})", turn_id);
                    // Only cancel playback if it's actually active — avoid phantom
                    // "Playback Cancelled" logs when there's nothing playing.
                    if !playback_engine.is_idle() {
                        playback_engine.cancel();
                        log::info!("[Pipeline] Playback stopped (was active).");
                    }
                    token_buf.clear();
                    awaiting_playback_finish = false;
                    self.tts_generating.store(false, Ordering::Relaxed);
                    // Reset cancel flag so new sessions can proceed
                    self.cancel_flag.store(false, Ordering::Relaxed);
                    let _ = translit_tx.send(TranslitTask::Cancel { turn_id });

                    // Persist Cancellation
                    if let Some(ref tx) = self.persist_tx {
                        if let Err(_) = tx.try_send(
                            crate::persistence::events::PersistenceEvent::TurnCancelled {
                                conversation_id: self.conversation_id.load(Ordering::Relaxed),
                                turn_id,
                            },
                        ) {
                            self.dropped_persistence_events
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    let owner = self.get_current_owner(&app_handle);
                    self.update_interaction_state(self.get_idle_state(), owner, &app_handle);
                }

                VoxEvent::Error { turn_id, message } => {
                    log::error!("[Pipeline] Error (turn {}): {}", turn_id, message);
                    let target = {
                        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                            app_handle.state();
                        let owner: crate::core::state::InteractionOwner =
                            state.owner.load(Ordering::Relaxed).into();
                        match owner {
                            crate::core::state::InteractionOwner::MainWindow
                            | crate::core::state::InteractionOwner::Ptt => "main",
                            crate::core::state::InteractionOwner::Tray => "tray",
                            crate::core::state::InteractionOwner::Wizard => "wizard",
                        }
                    };
                    let _ = app_handle.emit_to(target, "pipeline_error", &message);
                    awaiting_playback_finish = false;
                    self.tts_generating.store(false, Ordering::Relaxed);
                    let owner = self.get_current_owner(&app_handle);
                    self.update_interaction_state(self.get_idle_state(), owner, &app_handle);
                }
                VoxEvent::Shutdown => {
                    log::info!(
                        "[Pipeline] Shutdown signal received. Dispatched thread shutdown..."
                    );

                    // Directive 3: ASSERT CANCELLATION before joining.
                    // This forces C++ loops (llama.cpp) to abort instantly, unblocking the thread.
                    self.cancel_flag.store(true, Ordering::Relaxed);
                    let _ = translit_tx.send(TranslitTask::Shutdown);

                    // 1. Shutdown LLM Worker
                    if let Ok(mut lock) = self.llm_tx.lock() {
                        if let Some(tx) = lock.take() {
                            let _ = tx.send(crate::services::llm::LlmCommand::Shutdown);
                        }
                    }
                    let llm_handle_opt = if let Ok(mut lock) = self.llm_handle.lock() {
                        lock.take()
                    } else {
                        None
                    };

                    // 2. Shutdown TTS Worker
                    if let Ok(mut lock) = self.tts_tx.lock() {
                        if let Some(tx) = lock.take() {
                            let _ = tx.send(crate::services::tts::TtsCommand::Shutdown);
                        }
                    }
                    let tts_handle_opt = if let Ok(mut lock) = self.tts_handle.lock() {
                        lock.take()
                    } else {
                        None
                    };

                    // Join workers asynchronously in a background thread to prevent Tauri exit/shutdown deadlocks
                    std::thread::spawn(move || {
                        if let Some(h) = llm_handle_opt {
                            log::info!("[Pipeline Shutdown] Joining LLM worker thread...");
                            let _ = h.join();
                        }
                        if let Some(h) = tts_handle_opt {
                            log::info!("[Pipeline Shutdown] Joining TTS worker thread...");
                            let _ = h.join();
                        }
                        log::info!("[Pipeline Shutdown] Both worker threads cleaned up.");
                    });

                    log::info!("[Pipeline] Event loop exited. Model cleanup detached.");
                    break;
                }

                VoxEvent::SettingsUpdated(new_settings) => {
                    log::info!("[Pipeline] Local settings cache updated (Asynchronous).");
                    local_pipeline_mode = new_settings.interaction.pipeline_mode.clone();
                    local_voice = new_settings.tts.voice;
                    local_sleep_timeout = std::time::Duration::from_secs(
                        new_settings.interaction.auto_sleep_timeout as u64,
                    );
                    local_main_mode = new_settings.interaction.main_app_mode;
                    local_transliterate_enabled = new_settings.asr.transliterate_enabled;

                    // Forward TTS hot-updatable settings to the worker
                    if new_settings.tts.quality_steps != local_quality_steps {
                        local_quality_steps = new_settings.tts.quality_steps;
                        if let Ok(lock) = self.tts_tx.lock() {
                            if let Some(tx) = lock.as_ref() {
                                let _ =
                                    tx.send(crate::services::tts::TtsCommand::UpdateQualitySteps(
                                        local_quality_steps,
                                    ));
                                log::debug!(
                                    "[Pipeline] Dispatched UpdateQualitySteps({}) to TTS worker",
                                    local_quality_steps
                                );
                            }
                        }
                    }
                    if (new_settings.tts.speed - local_speed).abs() > f32::EPSILON {
                        local_speed = new_settings.tts.speed;
                        if let Ok(lock) = self.tts_tx.lock() {
                            if let Some(tx) = lock.as_ref() {
                                let _ = tx.send(crate::services::tts::TtsCommand::UpdateSpeed(
                                    local_speed,
                                ));
                                log::debug!(
                                    "[Pipeline] Dispatched UpdateSpeed({:.2}) to TTS worker",
                                    local_speed
                                );
                            }
                        }
                    }
                }

                // Handle remaining events that don't require orchestrator logic
                _ => {}
            }
        }
    }
}
