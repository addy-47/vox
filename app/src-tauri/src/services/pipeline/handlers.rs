//! ============================================================================
//! src/services/pipeline/handlers.rs — State management & transcript processing handlers
//! ============================================================================

use super::PipelineOrchestrator;
use crate::core::events::VoxEvent;
use crate::core::state::InteractionOwner;
use crate::services::utils::is_devanagari;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Emitter, Manager};

impl PipelineOrchestrator {
    /// Update internal state and emit IPC event to the **owning** window only.
    pub fn update_interaction_state(
        &self,
        new_state: crate::core::state::InteractionState,
        owner: InteractionOwner,
        app_handle: &tauri::AppHandle,
    ) {
        let mut state_lock = self.state.lock();
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

            // Notify Memory Worker of Pipeline Idle / Active state change
            if let Some(app_state) =
                app_handle.try_state::<std::sync::Arc<crate::core::state::AppState>>()
            {
                let memory_tx = app_state.memory_tx.lock();
                if let Some(ref tx) = *memory_tx {
                    let event = if new_state == crate::core::state::InteractionState::Idle {
                        crate::persistence::memory_worker::MemoryWorkerEvent::PipelineIdle
                    } else {
                        crate::persistence::memory_worker::MemoryWorkerEvent::PipelineActive
                    };
                    let _ = tx.try_send(event);
                }
            }
        }
    }

    pub(crate) fn get_idle_state(&self) -> crate::core::state::InteractionState {
        if self.is_engaged.load(Ordering::Relaxed) {
            crate::core::state::InteractionState::Listening
        } else {
            crate::core::state::InteractionState::Idle
        }
    }

    pub(crate) fn get_current_owner(&self, app: &tauri::AppHandle) -> InteractionOwner {
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
        // 1. The user explicitly engaged the main app (is_engaged is true).
        let is_engaged = self.is_engaged.load(Ordering::Relaxed);
        let should_trigger_pipeline = is_engaged;

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

        let lock = self.llm_tx.lock();
        if let Some(tx) = &*lock {
            // RCA Fix: Ensure cancel_flag is false right before generation starts.
            // If SpeechStart set it to true but no playback was active to emit
            // a Cancelled event (which usually resets this), the LLM would stall.
            self.cancel_flag.store(false, Ordering::Relaxed);

            let settings_snap = self.settings.read().unwrap().clone();
            let assistant_settings = &settings_snap.assistant;
            let is_hi = is_devanagari(&text);
            let (lang, script) = if is_hi {
                ("Hindi", "Devanagari")
            } else {
                ("English", "Latin")
            };
            let resolved_prompt = assistant_settings
                .modular_prompt
                .replace("<lang>", lang)
                .replace("<script>", script);

            let provider_kind = match &settings_snap.llm.provider {
                crate::core::settings::LlmProviderConfig::Embedded => {
                    crate::services::llm::ProviderKind::Embedded
                }
                crate::core::settings::LlmProviderConfig::OpenAiCompat { .. } => {
                    crate::services::llm::ProviderKind::OpenAiCompat
                }
            };

            let db_path = crate::utils::paths::db_path();
            let rt = crate::persistence::db::get_tokio_handle();

            let scope = crate::services::memory::classify_scope(&text);
            log::info!("[Pipeline] MemoryScope Classification: {:?}", scope);

            let personal_memory_block = if scope == crate::services::memory::MemoryScope::ChitChat {
                log::info!("[Pipeline] MemoryScope: ChitChat turn. Bypassing embedding generation and database RAG lookup.");
                String::new()
            } else {
                let query_embedding = rt
                    .block_on(async {
                        crate::services::memory::ensure_embedder_loaded(
                            settings_snap.memory.context_retrieval_enabled,
                        )
                        .ok();
                        crate::services::memory::generate_embedding(&text).unwrap_or(None)
                    })
                    .unwrap_or_else(|| vec![0.0; 1024]);

                rt.block_on(async {
                    if let Ok(conn) = crate::persistence::db::VoxDb::open_readonly(&db_path).await {
                        crate::services::memory::retrieval::retrieve_personal_context_v7(
                            &conn,
                            &query_embedding,
                            scope,
                            &settings_snap.memory,
                            settings_snap.llm.ctx_size as usize,
                        )
                        .await
                        .unwrap_or_default()
                    } else {
                        String::new()
                    }
                })
            };

            let mut final_prompt = resolved_prompt.clone();
            if !personal_memory_block.is_empty() {
                final_prompt.push_str(&format!("\n\n{}", personal_memory_block));
            }

            let provider_ref: Option<Box<dyn crate::services::llm::LlmProvider>> = {
                let is_tier_1a = provider_kind == crate::services::llm::ProviderKind::Embedded
                    && settings_snap.llm.ctx_size <= 4096;

                if settings_snap.memory.context_retrieval_enabled && !is_tier_1a {
                    match &settings_snap.llm.provider {
                        crate::core::settings::LlmProviderConfig::Embedded => {
                            let provider =
                                crate::services::llm::providers::embedded::EmbeddedProvider::new(
                                    &self.llm_path,
                                    settings_snap.llm.ctx_size,
                                    settings_snap.llm.threads,
                                );
                            if let Ok(p) = provider {
                                Some(Box::new(p) as Box<dyn crate::services::llm::LlmProvider>)
                            } else {
                                None
                            }
                        }
                        crate::core::settings::LlmProviderConfig::OpenAiCompat {
                            base_url,
                            model,
                            api_key,
                            provider_name,
                        } => {
                            let provider = crate::services::llm::providers::openai_compat::OpenAiCompatProvider::new(
                                base_url,
                                model,
                                api_key.as_deref(),
                                provider_name.as_deref(),
                            );
                            Some(Box::new(provider) as Box<dyn crate::services::llm::LlmProvider>)
                        }
                    }
                } else {
                    None
                }
            };

            let (ctx, transition_speech, personal_memory) = {
                let mut mgr = self.conversation_manager.lock();
                mgr.set_max_context_tokens(settings_snap.llm.ctx_size as usize);
                mgr.update_system_prompt(&final_prompt);
                if mgr.context_utilization() == 0.0 {
                    mgr.new_session(&final_prompt);
                }
                mgr.push_user_turn(text.clone());
                let provider_dyn: Option<&dyn crate::services::llm::LlmProvider> =
                    provider_ref.as_ref().map(|p| p.as_ref());
                mgr.build_context(provider_kind, is_hi, provider_dyn, Some(&settings_snap.llm))
            };

            if !personal_memory.is_empty() {
                if let Some(app_state) =
                    _app_handle.try_state::<std::sync::Arc<crate::core::state::AppState>>()
                {
                    let memory_tx = app_state.memory_tx.lock();
                    if let Some(ref tx) = *memory_tx {
                        let _ = tx.try_send(crate::persistence::memory_worker::MemoryWorkerEvent::PersonalFactsReady {
                            facts: personal_memory,
                            session_id: self.conversation_id.load(Ordering::Relaxed).to_string(),
                        });
                    }
                }
            }

            if let Some(speech_text) = transition_speech {
                log::info!(
                    "[Pipeline] MaintainingContext transition speech triggered: {:?}",
                    speech_text
                );
                self.update_interaction_state(
                    crate::core::state::InteractionState::MaintainingContext,
                    owner,
                    &_app_handle,
                );
                let lock = self.tts_tx.lock();
                if let Some(tts_sender) = lock.as_ref() {
                    let _ = tts_sender.send(crate::services::tts::TtsCommand::Generate {
                        text: speech_text,
                        turn_id: new_turn,
                    });
                }
            }

            let policy = crate::services::llm::GenerationPolicy::from_settings(&settings_snap.llm);
            let request = policy.build_request(
                crate::services::llm::GenerationPurpose::Conversation,
                crate::services::llm::ConversationInput {
                    messages: ctx.messages,
                },
            );

            let cmd = crate::services::llm::LlmCommand::Generate {
                request,
                turn_id: new_turn,
                cancel_flag: Arc::clone(&self.cancel_flag),
            };

            if let Err(e) = tx.send(cmd) {
                log::error!("[Pipeline] Failed to send generate command to LLM: {}", e);
            }
        }
        new_turn
    }
}
