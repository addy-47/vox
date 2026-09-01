use super::buffer::{current_timestamp_ms, ConversationContext};
use super::manager::ConversationManager;
use super::prompt_builder::format_retrieved_profile;
use crate::core::constants::{TRANSITION_MESSAGES_EN, TRANSITION_MESSAGES_HI};
use crate::core::error::MemoryError;
use crate::core::settings::{LlmSettings, MemorySettings};
use crate::services::llm::{
    ConversationInput, GenerationOptions, GenerationPurpose, GenerationRequest, LlmProvider,
    OutputConstraint, ProviderKind,
};
use parking_lot::Mutex;
use query_sieve::MemoryScope;
use std::collections::HashMap;
use std::sync::Arc;
use turso::Connection;

/// Bundled parameters for the `prepare_turn_context` public facade.
pub struct PrepareTurnParams<'a> {
    pub harness: &'a Arc<Mutex<ConversationManager>>,
    pub tts_tx: Option<&'a std::sync::mpsc::Sender<crate::services::tts::TtsCommand>>,
    pub memory_tx: Option<
        &'a parking_lot::Mutex<
            Option<crossbeam_channel::Sender<crate::persistence::events::MemoryWorkerEvent>>,
        >,
    >,
    pub conn: Option<&'a Connection>,
    pub query: &'a str,
    pub turn_id: u32,
    pub session_id: &'a str,
    pub memory: &'a MemorySettings,
    pub context_window: usize,
    pub provider_kind: ProviderKind,
    pub llm_provider: Option<&'a dyn LlmProvider>,
    pub llm_settings: Option<&'a LlmSettings>,
}

/// Prepares full generation request with waterfall retrieval and threshold maintenance.
pub async fn prepare_turn_context(
    params: PrepareTurnParams<'_>,
) -> Result<(GenerationRequest, Option<String>), MemoryError> {
    let query_trimmed = params.query.trim();

    let mut retrieved_profile = crate::services::memory::retrieval::RetrievedProfile::default();
    if params.memory.context_retrieval_enabled && params.conn.is_some() && !query_trimmed.is_empty()
    {
        let scope = crate::services::memory::ml::classify_scope(query_trimmed);
        if scope != MemoryScope::ChitChat {
            if let Ok(Some(embedding)) =
                crate::services::memory::ml::generate_embedding(query_trimmed)
            {
                if let Some(conn) = params.conn {
                    if let Ok(profile) = crate::services::memory::retrieval::retrieve_turn_profile(
                        conn,
                        &embedding,
                        scope,
                        params.memory,
                        params.context_window,
                    )
                    .await
                    {
                        retrieved_profile = profile;
                    }
                }
            }
        }
    }

    let profile_rendered = format_retrieved_profile(&retrieved_profile);
    let profile_opt = if profile_rendered.is_empty() {
        None
    } else {
        Some(profile_rendered)
    };

    let is_deva = crate::services::translit::is_devanagari(query_trimmed);

    let (transition_speech, compaction_job) = {
        let mut cm = params.harness.lock();
        cm.update_dynamic_user_profile(profile_opt);
        cm.push_user_turn(query_trimmed.to_string());

        let mut context_harness = super::accountant::ContextHarness::new(params.context_window);
        context_harness.sync_tokens_from_buffer(&cm.buffer);

        let mut transition_speech = None;
        let mut compaction_job = None;

        if context_harness.needs_threshold_maintenance() {
            log::warn!(
                "[Harness] Critical threshold reached ({:.1}% utilization). Performing Maintenance...",
                context_harness.context_utilization() * 100.0
            );

            let msg_set = if is_deva {
                TRANSITION_MESSAGES_HI
            } else {
                TRANSITION_MESSAGES_EN
            };
            let random_idx = (current_timestamp_ms() as usize) % msg_set.len();
            transition_speech = Some(msg_set[random_idx].to_string());

            let use_fifo = match params.provider_kind {
                ProviderKind::Embedded => context_harness.accountant.max_context_tokens() <= 4096,
                ProviderKind::OpenAiCompat => false,
            };

            if use_fifo || cm.buffer.messages.len() <= 3 {
                context_harness.perform_fifo_maintenance(&mut cm.buffer);
            } else if let Some(last_user_turn) = cm.buffer.pop_last_user_turn() {
                let history_slice = cm.buffer.messages[1..].to_vec();
                compaction_job = Some((history_slice, last_user_turn));
            }
        }

        (transition_speech, compaction_job)
    };

    let mut diff_to_enqueue = HashMap::new();
    if let Some((history_slice, last_user_turn)) = compaction_job {
        if let Some(ref filler) = transition_speech {
            if let Some(tts_sender) = params.tts_tx {
                if let Err(e) = tts_sender.send(crate::services::tts::TtsCommand::Generate {
                    turn_id: params.turn_id,
                    text: filler.clone(),
                }) {
                    log::warn!(
                        "[ContextHarness] Failed to dispatch transition speech filler to TTS: {}",
                        e
                    );
                }
            }
        }

        let provider_box: Option<Box<dyn LlmProvider>> = if params.llm_provider.is_none() {
            if let Some(s) = params.llm_settings {
                let models_dir = crate::utils::paths::get().models.clone();
                let llm_path = models_dir
                    .join(crate::services::llm::QWEN_MODEL_DIR)
                    .join(crate::services::llm::QWEN_MODEL_FILE);
                crate::services::llm::actor::create_llm_provider_from_llm_settings(s, &llm_path)
                    .ok()
            } else {
                None
            }
        } else {
            None
        };

        let active_provider: Option<&dyn LlmProvider> =
            params.llm_provider.or(provider_box.as_deref());

        if let Some(provider) = active_provider {
            match crate::services::memory::compaction::run_compaction(
                provider,
                &history_slice,
                params.llm_settings,
                None,
            )
            .await
            {
                Ok(result) => {
                    let mut lock = params.harness.lock();
                    let mut context_harness =
                        super::accountant::ContextHarness::new(params.context_window);
                    let sys_prompt = lock.system_prompt().clone();
                    diff_to_enqueue = context_harness.apply_compaction_result(
                        &mut lock.buffer,
                        &sys_prompt,
                        &result,
                        last_user_turn,
                    );
                }
                Err(e) => {
                    log::warn!(
                        "[Harness] LLM compaction failed: {}. Falling back to FIFO shift.",
                        e
                    );
                    let mut lock = params.harness.lock();
                    let mut context_harness =
                        super::accountant::ContextHarness::new(params.context_window);
                    context_harness.sync_tokens_from_buffer(&lock.buffer);
                    lock.buffer.messages.push(last_user_turn);
                    context_harness.perform_fifo_maintenance(&mut lock.buffer);
                }
            }
        } else {
            let mut lock = params.harness.lock();
            let mut context_harness = super::accountant::ContextHarness::new(params.context_window);
            context_harness.sync_tokens_from_buffer(&lock.buffer);
            lock.buffer.messages.push(last_user_turn);
            context_harness.perform_fifo_maintenance(&mut lock.buffer);
        }
    }

    let conv_ctx = {
        let mut cm = params.harness.lock();
        let mut context_harness = super::accountant::ContextHarness::new(params.context_window);
        context_harness.sync_tokens_from_buffer(&cm.buffer);

        let soft_cap = ((context_harness.accountant.max_context_tokens() as f32)
            * crate::services::memory::NARRATIVE_CHAIN_SOFT_CAP_SHARE)
            as usize;
        let session_history = context_harness.build_session_history_xml(soft_cap);
        let sys_prompt = cm.system_prompt().clone();
        context_harness.consolidate_system_message(&mut cm.buffer, &sys_prompt, &session_history);

        let kv_idx = if params.provider_kind == ProviderKind::Embedded {
            cm.buffer.kv_synced_index
        } else {
            0
        };

        let mut total_tokens = 0;
        for msg in &cm.buffer.messages {
            total_tokens += crate::services::memory::ml::estimate_tokens(&msg.content);
        }

        ConversationContext {
            messages: cm.buffer.messages.clone(),
            token_count: total_tokens,
            kv_cache_index: kv_idx,
        }
    };

    if !diff_to_enqueue.is_empty() && params.memory.pipeline_processing_enabled {
        let mem_sender = params.memory_tx.and_then(|m| m.lock().clone());
        if let Some(tx) = mem_sender {
            if let Err(e) = tx.try_send(
                crate::persistence::events::MemoryWorkerEvent::PersonalFactsReady {
                    facts: diff_to_enqueue.clone(),
                    session_id: params.session_id.to_string(),
                },
            ) {
                log::warn!(
                    "[Harness] Failed to dispatch PersonalFactsReady to worker: {}",
                    e
                );
            }
        } else if let Some(conn) = params.conn {
            let session_id = params.session_id.to_string();
            if let Err(e) = crate::persistence::mutations::enqueue_personal_facts(
                conn,
                diff_to_enqueue,
                &session_id,
                true,
            )
            .await
            {
                log::warn!("[Harness] Failed to enqueue personal memory: {}", e);
            }
        }
    }

    let request = GenerationRequest {
        input: ConversationInput {
            messages: conv_ctx.messages,
        },
        options: GenerationOptions {
            temperature: params.llm_settings.map(|s| s.temperature),
            max_output_tokens: params.llm_settings.map(|s| s.max_output_tokens),
            ..Default::default()
        },
        output: OutputConstraint::Text,
        purpose: GenerationPurpose::Conversation,
    };

    Ok((request, transition_speech))
}

use crate::core::state::{AppState, InteractionState};
use std::sync::LazyLock;
use std::time::Instant;

pub const SOFT_COMPACTION_DEBOUNCE_SECS: u64 = 20;

static LAST_SOFT_COMPACTION: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

/// Triggers opportunistic background compaction if conversation memory utilization is in the soft window,
/// pipeline state is in {Ready, Paused}, and at least 20 seconds have elapsed since last compaction.
pub fn trigger_background_compaction(
    state: &AppState,
    provider: Option<Arc<dyn LlmProvider>>,
    settings: Option<LlmSettings>,
) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Ready && current_state != InteractionState::Paused {
        return;
    }

    let is_modular = state
        .settings
        .read()
        .map(|s| s.interaction.pipeline_mode == crate::core::settings::PipelineMode::Modular)
        .unwrap_or(true);
    if !is_modular {
        return;
    }

    {
        let last = LAST_SOFT_COMPACTION.lock();
        if let Some(instant) = *last {
            if instant.elapsed().as_secs() < crate::services::memory::SOFT_COMPACTION_DEBOUNCE_SECS
            {
                return;
            }
        }
    }

    let context_window = state
        .settings
        .read()
        .map(|s| s.llm.context_window as usize)
        .unwrap_or(4096);

    let harness = &state.conversation_manager;
    let candidate = {
        let cm = harness.lock();
        let mut context_harness = super::accountant::ContextHarness::new(context_window);
        context_harness.sync_tokens_from_buffer(&cm.buffer);
        context_harness.try_trigger_opportunistic(&cm.buffer)
    };

    if let Some((snapshot_len, messages, cancel_flag)) = candidate {
        if messages.len() <= 3 {
            return;
        }
        let h: std::sync::Arc<Mutex<ConversationManager>> = std::sync::Arc::clone(harness);
        let settings_resolved =
            settings.or_else(|| state.settings.read().ok().map(|s| s.llm.clone()));
        let cached_provider = state.llm_provider.read().clone();

        tauri::async_runtime::spawn(async move {
            if cancel_flag.is_cancelled() {
                return;
            }
            let history_slice = &messages[1..messages.len().saturating_sub(1)];

            let provider_inst = match provider.or(cached_provider) {
                Some(p) => p,
                None => {
                    let s_ref = settings_resolved.as_ref();
                    let models_dir = crate::utils::paths::get().models.clone();
                    let llm_path = models_dir
                        .join(crate::services::llm::QWEN_MODEL_DIR)
                        .join(crate::services::llm::QWEN_MODEL_FILE);
                    match s_ref.and_then(|s| {
                        crate::services::llm::actor::create_llm_provider_from_llm_settings(
                            s, &llm_path,
                        )
                        .ok()
                    }) {
                        Some(p) => Arc::from(p),
                        None => {
                            log::warn!("[Harness] Failed to instantiate LLM provider for background compaction.");
                            return;
                        }
                    }
                }
            };

            match crate::services::memory::compaction::run_compaction(
                provider_inst.as_ref(),
                history_slice,
                settings_resolved.as_ref(),
                Some(&cancel_flag),
            )
            .await
            {
                Ok(result) => {
                    let mut lock = h.lock();
                    let mut context_harness =
                        super::accountant::ContextHarness::new(context_window);
                    context_harness.sync_tokens_from_buffer(&lock.buffer);
                    let sys_prompt = lock.system_prompt().clone();
                    let committed = context_harness.commit_opportunistic(
                        &mut lock.buffer,
                        &sys_prompt,
                        snapshot_len,
                        result.context_summary,
                    );
                    if committed {
                        *LAST_SOFT_COMPACTION.lock() = Some(Instant::now());
                        log::info!(
                            "[Harness] Opportunistic background compaction committed successfully."
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[Harness] Opportunistic background compaction failed: {}",
                        e
                    );
                }
            }
        });
    }
}

/// Spawns a background observer task that watches for InteractionState transitions into {Ready, Paused}
/// and triggers opportunistic soft compaction after the debounce window.
pub fn spawn_state_compaction_observer(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut state_rx = state.pipeline.state_rx.clone();
        log::info!("[Memory::Compaction] Compaction observer spawned.");

        while state_rx.changed().await.is_ok() {
            let current_state = *state_rx.borrow_and_update();
            if current_state == InteractionState::Ready || current_state == InteractionState::Paused
            {
                trigger_background_compaction(&state, None, None);
            }
        }
    });
}
