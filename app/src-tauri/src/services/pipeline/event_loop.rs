//! ============================================================================
//! src/services/pipeline/event_loop.rs — Main pipeline event loop processor
//! ============================================================================

use super::types::TranslitTask;
use super::PipelineOrchestrator;
use crate::core::events::VoxEvent;
use crate::core::metrics::{MetricField, PipelineMetrics};
use crate::core::state::InteractionOwner;
use crate::services::utils::{count_words, should_flush, transliterate_if_hi};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Emitter, Manager};

impl PipelineOrchestrator {
    /// Process the internal event bus in a blocking loop.
    pub fn run_event_loop(
        &self,
        rx: std::sync::mpsc::Receiver<VoxEvent>,
        playback_engine: Arc<crate::services::audio::PlaybackEngine>,
        app_handle: tauri::AppHandle,
    ) {
        let mut last_interaction = std::time::Instant::now();
        let mut current_turn_owner = self.get_current_owner(&app_handle);

        // Local settings cache to avoid RwLock contention in the hot path (Directive: Real-Time Safety)
        let mut local_pipeline_mode = {
            let s = self.settings.read().unwrap();
            s.interaction.pipeline_mode.clone()
        };
        let local_is_external_llm = {
            let s = self.settings.read().unwrap();
            matches!(
                s.llm.provider,
                crate::core::settings::LlmProviderConfig::OpenAiCompat { .. }
            )
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
        let mut local_silence_time: Option<std::time::Instant> = None;
        let mut tts_queued_chunks = 0usize;
        let mut tts_finished_chunks = 0usize;

        // Turn persistence buffers
        let mut turn_user_text = String::new();
        let mut turn_assistant_text = String::new();
        let mut turn_stt_ms = 0u32;
        let mut turn_ttft_ms = 0u32;
        let mut last_tts_flush = std::time::Instant::now();
        let mut last_committed_session_id = 0u32;
        let mut turn_first_token_time: Option<std::time::Instant> = None;
        let mut turn_tokens_generated = 0usize;

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
                    let owner = current_turn_owner;
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

            // Sync Realtime state changes based on playback activity
            if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                let playback_active = self._playback_active.load(Ordering::Relaxed);
                let current_state = *self.state.lock();
                let owner = self.get_current_owner(&app_handle);

                if playback_active {
                    if current_state != crate::core::state::InteractionState::AssistantSpeaking {
                        self.update_interaction_state(
                            crate::core::state::InteractionState::AssistantSpeaking,
                            owner,
                            &app_handle,
                        );
                        if metrics.first_audio.is_none() {
                            metrics.mark(MetricField::FirstAudio);
                            metrics.mark(MetricField::PlaybackStart);
                            if let (Some(s), Some(p)) =
                                (metrics.speech_start, metrics.playback_start)
                            {
                                let ms = p.duration_since(s).as_millis() as u32;
                                self.latest_playback_start_ms.store(ms, Ordering::Relaxed);
                                self.latest_voice_latency_ms.store(ms, Ordering::Relaxed);
                            }
                        }
                    }
                } else {
                    if current_state == crate::core::state::InteractionState::AssistantSpeaking {
                        self.update_interaction_state(
                            crate::core::state::InteractionState::Listening,
                            owner,
                            &app_handle,
                        );
                    }
                }

                // Check for UserSpeaking timeout recovery (Passive mode only)
                if current_state == crate::core::state::InteractionState::UserSpeaking
                    && owner != InteractionOwner::Ptt
                {
                    if let Some(silence_start) = local_silence_time {
                        if silence_start.elapsed() > std::time::Duration::from_secs(10) {
                            log::info!("[Pipeline] UserSpeaking state timeout (10s since local silence). Triggering Automatic Pause Recovery.");

                            self.is_paused.store(true, Ordering::SeqCst);
                            self.cancel_flag.store(true, Ordering::SeqCst);
                            playback_engine.cancel();

                            let state: tauri::State<
                                '_,
                                std::sync::Arc<crate::core::state::AppState>,
                            > = app_handle.state();
                            if let Ok(rt_guard) = state.realtime_engine.try_lock() {
                                if let Some(rt_engine) = &*rt_guard {
                                    let _ = rt_engine.activity_end();
                                }
                            }

                            let target = match owner {
                                crate::core::state::InteractionOwner::MainWindow
                                | crate::core::state::InteractionOwner::Ptt => "main",
                                crate::core::state::InteractionOwner::Tray => "tray",
                                crate::core::state::InteractionOwner::Wizard => "wizard",
                            };
                            let _ = app_handle.emit_to(target, "pipeline_paused", ());
                            let _ = app_handle.emit_to(
                                target,
                                "pipeline_error",
                                "Speech detection timeout: No response from server. Paused."
                                    .to_string(),
                            );
                            self.update_interaction_state(
                                crate::core::state::InteractionState::Idle,
                                owner,
                                &app_handle,
                            );
                            local_silence_time = None;
                        }
                    }
                } else {
                    local_silence_time = None;
                }
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

                                    // Trigger Memory SessionEnd consolidation
                                    if let Some(app_state) = app_handle
                                        .try_state::<std::sync::Arc<crate::core::state::AppState>>()
                                    {
                                        let memory_tx = app_state.memory_tx.lock();
                                        if let Some(ref tx) = *memory_tx {
                                            let summary =
                                                self.conversation_manager.lock().latest_summary();
                                            let _ = tx.try_send(crate::persistence::memory_worker::MemoryWorkerEvent::SessionEnd {
                                                session_id: conv_id.to_string(),
                                                summary,
                                            });
                                        }
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
                        let owner = current_turn_owner;
                        let input_duration = (count_words(&turn_user_text) as f64 / 2.5).max(0.5);
                        let output_duration =
                            playback_engine.total_samples_ingested() as f64 / 48000.0;
                        let report = metrics.latency_report(
                            input_duration,
                            output_duration,
                            local_pipeline_mode.clone(),
                            owner == InteractionOwner::Ptt,
                        );
                        log::info!("[Pipeline] Turn complete (polled). Latencies: {}", report);
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
                    if let Err(e) = self.warm_up_tts(&app_handle) {
                        log::error!("[Pipeline] WarmUp (TTS): failed: {}", e);
                    }
                    log::info!("[Pipeline] WarmUp: workers started in background.");
                }
                VoxEvent::SpeechStart { turn_id, owner } => {
                    self.conversation_manager.lock().on_speech_start();
                    let is_engaged = self.is_engaged.load(Ordering::Relaxed);

                    if !is_engaged
                        && (owner == InteractionOwner::MainWindow || owner == InteractionOwner::Ptt)
                    {
                        continue;
                    }
                    current_turn_owner = owner;
                    metrics.mark(MetricField::SpeechStart);
                    playback_engine.reset_samples_ingested();
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        if owner != InteractionOwner::Ptt {
                            let current_state = *self.state.lock();
                            if current_state != crate::core::state::InteractionState::Listening
                                && current_state != crate::core::state::InteractionState::Idle
                            {
                                log::debug!("[Pipeline] SpeechStart ignored (Realtime Passive mode in state {:?})", current_state);
                                continue;
                            }
                        }

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
                        local_silence_time = None;
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

                VoxEvent::TranscriptPartial {
                    turn_id,
                    owner,
                    text,
                } => {
                    let is_engaged = self.is_engaged.load(Ordering::Relaxed);
                    if !is_engaged
                        && (owner == InteractionOwner::MainWindow || owner == InteractionOwner::Ptt)
                    {
                        continue;
                    }
                    if turn_id < current_tid {
                        continue;
                    }
                    current_turn_owner = owner;
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

                VoxEvent::TranscriptFinal {
                    turn_id,
                    owner,
                    text,
                } => {
                    let is_engaged = self.is_engaged.load(Ordering::Relaxed);
                    if !is_engaged
                        && (owner == InteractionOwner::MainWindow || owner == InteractionOwner::Ptt)
                    {
                        continue;
                    }
                    if turn_id < current_tid
                        && local_pipeline_mode != crate::core::settings::PipelineMode::Realtime
                    {
                        continue;
                    }
                    current_turn_owner = owner;
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        turn_user_text = text.clone();
                        turn_assistant_text.clear();
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

                        // Populate metrics for Realtime turn
                        metrics.mark(MetricField::FinalTranscript);
                        metrics.mark(MetricField::LlmStart);
                        metrics.input_len_chars = text.len();

                        if owner != InteractionOwner::Ptt {
                            self.update_interaction_state(
                                crate::core::state::InteractionState::Thinking,
                                owner,
                                &app_handle,
                            );
                            self.cancel_flag.store(false, Ordering::Relaxed);
                        }
                        local_silence_time = None;

                        continue;
                    }
                    if turn_id < last_committed_session_id {
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
                    tts_queued_chunks = 0;
                    tts_finished_chunks = 0;
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
                        let target = current_turn_owner;
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

                        // Populate metrics for Realtime turn
                        if metrics.first_token.is_none() {
                            metrics.mark(MetricField::FirstToken);
                        }
                        metrics.tokens_generated += 1;

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
                    } else if local_is_external_llm {
                        30.0
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

                            let lock = self.tts_tx.lock();
                            if let Some(tx) = lock.as_ref() {
                                let _ = tx.send(crate::services::tts::TtsCommand::Generate {
                                    turn_id,
                                    text: chunk,
                                });
                                tts_queued_chunks += 1;
                                self.tts_generating.store(true, Ordering::Relaxed);
                            }
                            token_buf.clear();
                            last_tts_flush = std::time::Instant::now();
                        }
                    }

                    let target = match current_turn_owner {
                        crate::core::state::InteractionOwner::MainWindow
                        | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                        crate::core::state::InteractionOwner::Wizard => "wizard",
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
                            "[Pipeline] Realtime server response complete: {:?}",
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
                    self.conversation_manager
                        .lock()
                        .push_assistant_turn(turn_assistant_text.clone());

                    let remainder = token_buf.trim().to_string();
                    if !remainder.is_empty() {
                        log::info!(
                            "[Pipeline] Flushing remainder text chunk to TTS: {:?}",
                            remainder
                        );
                        if metrics.tts_start.is_none() {
                            metrics.mark(MetricField::TtsStart);
                        }
                        let lock = self.tts_tx.lock();
                        if let Some(tx) = lock.as_ref() {
                            let _ = tx.send(crate::services::tts::TtsCommand::Generate {
                                turn_id,
                                text: remainder,
                            });
                            tts_queued_chunks += 1;
                            self.tts_generating.store(true, Ordering::Relaxed);
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
                    if metrics.first_audio.is_none() {
                        metrics.mark(MetricField::FirstAudio);
                    }
                    playback_engine.ingest_chunk(&samples);

                    // Adaptive buffering: trigger playback if buffer size exceeds 300ms (14,400 samples at 48kHz)
                    if playback_engine.buffer_len() >= 14_400 {
                        trigger_playback!("buffer >= 300ms");
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

                    // Always trigger playback when a chunk finishes synthesis to keep audio flowing
                    trigger_playback!("chunk finished");

                    if tts_finished_chunks >= tts_queued_chunks && awaiting_playback_finish {
                        self.tts_generating.store(false, Ordering::Relaxed);
                        trigger_playback!("all chunks finished (TTS end)");
                    }
                }

                VoxEvent::SpeechEnd { turn_id: _, owner } => {
                    let is_engaged = self.is_engaged.load(Ordering::Relaxed);
                    if !is_engaged
                        && (owner == InteractionOwner::MainWindow || owner == InteractionOwner::Ptt)
                    {
                        continue;
                    }
                    current_turn_owner = owner;
                    metrics.mark(MetricField::SpeechEnd);
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        if owner != InteractionOwner::Ptt {
                            // Realtime Passive mode: SpeechEnd is a noop for state transition.
                            // Start local_silence_time tracking when silence is detected.
                            let current_state = *self.state.lock();
                            if current_state == crate::core::state::InteractionState::UserSpeaking
                                && local_silence_time.is_none() {
                                    log::info!("[Pipeline] Local VAD detected silence in UserSpeaking state. Starting 10s timeout guard.");
                                    local_silence_time = Some(std::time::Instant::now());
                                }
                        } else {
                            self.update_interaction_state(
                                crate::core::state::InteractionState::Thinking,
                                owner,
                                &app_handle,
                            );
                            self.cancel_flag.store(false, Ordering::Relaxed);
                        }
                    }
                }

                VoxEvent::PlaybackFinished { turn_id } => {
                    if turn_id != current_tid {
                        continue;
                    }
                    metrics.mark(MetricField::PlaybackFinish);

                    let input_duration = (count_words(&turn_user_text) as f64 / 2.5).max(0.5);
                    let output_duration = playback_engine.total_samples_ingested() as f64 / 48000.0;
                    let report = metrics.latency_report(
                        input_duration,
                        output_duration,
                        local_pipeline_mode.clone(),
                        current_turn_owner == InteractionOwner::Ptt,
                    );
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

                    let owner = current_turn_owner;
                    self.update_interaction_state(self.get_idle_state(), owner, &app_handle);

                    let target = match owner {
                        crate::core::state::InteractionOwner::MainWindow
                        | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                        crate::core::state::InteractionOwner::Wizard => "wizard",
                    };
                    let _ = app_handle.emit_to(target, "playback_finished", &report);

                    // Persist Turn
                    if let Some(ref tx) = self.persist_tx {
                        if tx.try_send(
                            crate::persistence::events::PersistenceEvent::TurnCompleted {
                                conversation_id: self.conversation_id.load(Ordering::Relaxed),
                                turn_id,
                                user_text: turn_user_text.clone(),
                                assistant_text: turn_assistant_text.clone(),
                                stt_latency_ms: turn_stt_ms,
                                ttft_ms: turn_ttft_ms,
                            },
                        ).is_err() {
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
                    self.conversation_manager.lock().pop_last_user_turn();

                    // Only cancel playback if it's actually active — avoid phantom
                    // "Playback Cancelled" logs when there's nothing playing.
                    if !playback_engine.is_idle() {
                        playback_engine.cancel();
                        log::info!("[Pipeline] Playback stopped (was active).");
                    }
                    token_buf.clear();
                    turn_user_text.clear();
                    turn_assistant_text.clear();
                    awaiting_playback_finish = false;
                    self.tts_generating.store(false, Ordering::Relaxed);
                    // Reset cancel flag so new sessions can proceed
                    self.cancel_flag.store(false, Ordering::Relaxed);
                    let _ = translit_tx.send(TranslitTask::Cancel { turn_id });

                    // Persist Cancellation
                    if let Some(ref tx) = self.persist_tx {
                        if tx.try_send(
                            crate::persistence::events::PersistenceEvent::TurnCancelled {
                                conversation_id: self.conversation_id.load(Ordering::Relaxed),
                                turn_id,
                            },
                        ).is_err() {
                            self.dropped_persistence_events
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    let owner = current_turn_owner;
                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime
                        && owner != InteractionOwner::Ptt
                    {
                        // In Realtime Passive mode, an interruption means the user has started speaking.
                        // Transition state directly to UserSpeaking.
                        self.update_interaction_state(
                            crate::core::state::InteractionState::UserSpeaking,
                            owner,
                            &app_handle,
                        );
                        local_silence_time = None;
                    } else {
                        self.update_interaction_state(self.get_idle_state(), owner, &app_handle);
                    }
                }

                VoxEvent::Error { turn_id, message } => {
                    log::error!("[Pipeline] Error (turn {}): {}", turn_id, message);
                    let target = match current_turn_owner {
                        crate::core::state::InteractionOwner::MainWindow
                        | crate::core::state::InteractionOwner::Ptt => "main",
                        crate::core::state::InteractionOwner::Tray => "tray",
                        crate::core::state::InteractionOwner::Wizard => "wizard",
                    };
                    let _ = app_handle.emit_to(target, "pipeline_error", &message);
                    awaiting_playback_finish = false;
                    self.tts_generating.store(false, Ordering::Relaxed);
                    let owner = current_turn_owner;

                    if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                        // Point 2: Trigger automatic pause recovery on backend and frontend
                        log::info!("[Pipeline] Connection/Engine error received in Realtime mode. Triggering Automatic Pause Recovery.");
                        self.is_paused.store(true, Ordering::SeqCst);
                        self.cancel_flag.store(true, Ordering::SeqCst);
                        playback_engine.cancel();

                        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                            app_handle.state();
                        if let Ok(rt_guard) = state.realtime_engine.try_lock() {
                            if let Some(rt_engine) = &*rt_guard {
                                let _ = rt_engine.activity_end();
                            }
                        }

                        let _ = app_handle.emit_to(target, "pipeline_paused", ());
                        self.update_interaction_state(
                            crate::core::state::InteractionState::Idle,
                            owner,
                            &app_handle,
                        );
                    } else {
                        self.update_interaction_state(self.get_idle_state(), owner, &app_handle);
                    }
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
                    {
                        let mut lock = self.llm_tx.lock();
                        if let Some(tx) = lock.take() {
                            let _ = tx.send(crate::services::llm::LlmCommand::Shutdown);
                        }
                    }
                    let llm_handle_opt = self.llm_handle.lock().take();

                    // 2. Shutdown TTS Worker
                    {
                        let mut lock = self.tts_tx.lock();
                        if let Some(tx) = lock.take() {
                            let _ = tx.send(crate::services::tts::TtsCommand::Shutdown);
                        }
                    }
                    let tts_handle_opt = self.tts_handle.lock().take();

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
                        let lock = self.tts_tx.lock();
                        if let Some(tx) = lock.as_ref() {
                            let _ = tx.send(crate::services::tts::TtsCommand::UpdateQualitySteps(
                                local_quality_steps,
                            ));
                            log::debug!(
                                "[Pipeline] Dispatched UpdateQualitySteps({}) to TTS worker",
                                local_quality_steps
                            );
                        }
                    }
                    if (new_settings.tts.speed - local_speed).abs() > f32::EPSILON {
                        local_speed = new_settings.tts.speed;
                        let lock = self.tts_tx.lock();
                        if let Some(tx) = lock.as_ref() {
                            let _ =
                                tx.send(crate::services::tts::TtsCommand::UpdateSpeed(local_speed));
                            log::debug!(
                                "[Pipeline] Dispatched UpdateSpeed({:.2}) to TTS worker",
                                local_speed
                            );
                        }
                    }
                }

                // Handle remaining events that don't require orchestrator logic
                _ => {}
            }
        }
    }
}
