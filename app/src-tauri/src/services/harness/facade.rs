use std::{
    collections::HashMap,
    sync::{atomic::Ordering, mpsc, Arc, LazyLock},
    time::Instant,
};

use parking_lot::Mutex;
use turso::Connection;

use super::{
    buffer::{current_timestamp_ms, ConversationContext},
    manager::ConversationManager,
};
use crate::{
    core::{
        constants::{TRANSITION_MESSAGES_EN, TRANSITION_MESSAGES_HI},
        error::MemoryError,
        settings::{LlmSettings, MemorySettings, PipelineMode},
        state::{AppState, InteractionState},
    },
    persistence::{commit_compaction_output, enqueue_fact, record_compaction_start},
    services::{
        llm::{
            actor::create_llm_provider_from_llm_settings, ConversationInput, GenerationOptions,
            GenerationPurpose, GenerationRequest, LlmProvider, OutputConstraint, ProviderKind,
            QWEN_MODEL_DIR, QWEN_MODEL_FILE,
        },
        memory::{
            compaction::run_compaction,
            ml::estimate_tokens,
            NARRATIVE_CHAIN_SOFT_CAP_SHARE,
        },
        translit::is_devanagari,
        tts::TtsCommand,
    },
    utils::paths,
};

pub const SOFT_COMPACTION_DEBOUNCE_SECS: u64 = 20;

static LAST_SOFT_COMPACTION: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

/// Bundled parameters for the `prepare_turn_context` public facade.
pub struct PrepareTurnParams<'a> {
    pub harness: &'a Arc<Mutex<ConversationManager>>,
    pub tts_tx: Option<&'a mpsc::Sender<TtsCommand>>,
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

    let profile_opt: Option<String> = None;

    let is_deva = is_devanagari(query_trimmed);

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
                if let Err(e) = tts_sender.send(TtsCommand::Generate {
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
                let models_dir = paths::get().models.clone();
                let llm_path = models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE);
                create_llm_provider_from_llm_settings(s, &llm_path).ok()
            } else {
                None
            }
        } else {
            None
        };

        let active_provider: Option<&dyn LlmProvider> =
            params.llm_provider.or(provider_box.as_deref());

        if let Some(provider) = active_provider {
            match run_compaction(provider, &history_slice, params.llm_settings, None).await {
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
                        "[Harness] Critical LLM compaction failed: {}. Falling back to FIFO maintenance.",
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
            * NARRATIVE_CHAIN_SOFT_CAP_SHARE) as usize;
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
            total_tokens += estimate_tokens(&msg.content);
        }

        ConversationContext {
            messages: cm.buffer.messages.clone(),
            token_count: total_tokens,
            kv_cache_index: kv_idx,
        }
    };

    if !diff_to_enqueue.is_empty() && params.memory.pipeline_processing_enabled {
        if let Some(conn) = params.conn {
            let session_id_int = params.session_id.parse::<i64>().ok();
            for (category, texts) in &diff_to_enqueue {
                for text in texts {
                    if let Err(e) = enqueue_fact(
                        conn,
                        session_id_int,
                        0,
                        category,
                        text,
                    )
                    .await
                    {
                        log::warn!("[Harness] Failed to enqueue extracted personal fact: {}", e);
                    }
                }
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
        .map(|s| s.interaction.pipeline_mode == PipelineMode::Modular)
        .unwrap_or(true);
    if !is_modular {
        return;
    }

    {
        let last = LAST_SOFT_COMPACTION.lock();
        if let Some(instant) = *last {
            if instant.elapsed().as_secs() < SOFT_COMPACTION_DEBOUNCE_SECS {
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
        let h: Arc<Mutex<ConversationManager>> = Arc::clone(harness);
        let settings_resolved =
            settings.or_else(|| state.settings.read().ok().map(|s| s.llm.clone()));
        let cached_provider = state.llm_provider.read().clone();
        let session_id = state.conversation_id.load(Ordering::Relaxed).to_string();
        let db = Arc::clone(&state.db);
        log::debug!("[Harness] Preparing background compaction candidate for session_id={}", session_id);

        tauri::async_runtime::spawn(async move {
            if cancel_flag.is_cancelled() {
                return;
            }
            let history_slice = &messages[1..messages.len().saturating_sub(1)];

            let provider_inst = match provider.or(cached_provider) {
                Some(p) => p,
                None => {
                    let s_ref = settings_resolved.as_ref();
                    let models_dir = paths::get().models.clone();
                    let llm_path = models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE);
                    match s_ref
                        .and_then(|s| create_llm_provider_from_llm_settings(s, &llm_path).ok())
                    {
                        Some(p) => Arc::from(p),
                        None => {
                            log::warn!("[Harness] Failed to instantiate LLM provider for background compaction.");
                            return;
                        }
                    }
                }
            };

            match run_compaction(
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

                        let db_inner = Arc::clone(&db);
                        let session_id_int: i64 = session_id.parse().unwrap_or(0);
                        let facts = result.facts.clone();
                        let raw_json = result.raw_json.clone();

                        tauri::async_runtime::spawn(async move {
                            if session_id_int > 0 {
                                match record_compaction_start(
                                    &db_inner,
                                    session_id_int,
                                    "soft",
                                    1,
                                    snapshot_len as u32,
                                )
                                .await
                                {
                                    Ok(run_id) => {
                                        if let Err(e) = commit_compaction_output(
                                            &db_inner,
                                            run_id,
                                            &raw_json,
                                            &facts,
                                            session_id_int,
                                        )
                                        .await
                                        {
                                            log::warn!(
                                                "[Harness] Failed to commit soft compaction output: {}",
                                                e
                                            );
                                        } else {
                                            log::info!(
                                                "[Harness] Staged {} soft compaction facts into queue for session {}",
                                                facts.len(),
                                                session_id_int
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "[Harness] Failed to record soft compaction start: {}",
                                            e
                                        );
                                    }
                                }
                            }
                        });
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
/// and triggers opportunistic soft compaction after a sustained quiet debounce window.
pub fn spawn_state_compaction_observer(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut state_rx = state.pipeline.state_rx.clone();
        log::info!("[Memory::Compaction] Debounced compaction observer spawned.");

        loop {
            let current = *state_rx.borrow_and_update();
            if current == InteractionState::Ready || current == InteractionState::Paused {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(SOFT_COMPACTION_DEBOUNCE_SECS)) => {
                        let latest = state.pipeline.state();
                        if latest == InteractionState::Ready || latest == InteractionState::Paused {
                            trigger_background_compaction(&state, None, None);
                        }
                    }
                    res = state_rx.changed() => {
                        if res.is_err() {
                            break;
                        }
                    }
                }
            } else if state_rx.changed().await.is_err() {
                break;
            }
        }
    });
}
