use crate::core::settings::VoxSettings;
use crate::core::state::AppState;
use crate::services::llm::{
    ConversationInput, GenerationOptions, GenerationPurpose, GenerationRequest, OutputConstraint,
    ProviderKind,
};
use crate::services::memory::ConversationManager;
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;

/// Initializes and warms up the LLM and TTS actor threads if not already loaded.
pub async fn ensure_modular_workers<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let (llm_path, tts_path, settings) = {
        let s = state.settings.read().unwrap().clone();
        let models_dir = crate::utils::paths::get().models.clone();
        let llm = models_dir
            .join(crate::services::llm::QWEN_MODEL_DIR)
            .join(crate::services::llm::QWEN_MODEL_FILE);
        let tts = models_dir.join(crate::services::tts::SUPERTONIC_MODEL_DIR);
        (llm, tts, s)
    };

    let mut lock = state.engine.lock().await;
    let engine = lock.as_mut().ok_or("Audio engine not ready")?;

    crate::services::llm::actor::warm_up_llm(
        app,
        crate::services::llm::actor::LlmWarmUpHandles {
            llm_tx: &mut engine.llm_tx,
            llm_handle: &mut engine.llm_handle,
            is_loaded: Arc::clone(&state.is_llm_loaded),
            is_sleeping: Arc::clone(&state.is_sleeping),
        },
        &settings,
        &llm_path,
        engine.pipeline_tx.clone(),
    )?;

    crate::services::tts::actor::warm_up_tts(
        app,
        crate::services::tts::actor::TtsWarmUpHandles {
            tts_tx: &mut engine.tts_tx,
            tts_handle: &mut engine.tts_handle,
            cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
            is_loaded: Arc::clone(&state.is_tts_loaded),
            is_sleeping: Arc::clone(&state.is_sleeping),
        },
        &settings,
        &tts_path,
        engine.pipeline_tx.clone(),
    )?;

    Ok(())
}

/// Prepares context with query scope classification, dense embedding, and waterfall graph retrieval.
pub async fn build_generation_request(
    settings: &VoxSettings,
    cm_arc: &Arc<Mutex<ConversationManager>>,
    conv_id: u64,
    text: &str,
    _turn_id: u32,
) -> (GenerationRequest, Option<String>) {
    let is_deva = crate::services::translit::is_devanagari(text);

    let mut retrieved_profile = None;
    if settings.memory.context_retrieval_enabled {
        let scope = crate::services::memory::classify_scope(text);
        if scope != query_sieve::MemoryScope::ChitChat {
            if let Ok(Some(query_embedding)) = crate::services::memory::generate_embedding(text) {
                let db_path = crate::utils::paths::db_path();
                if let Ok(conn) = crate::persistence::db::VoxDb::open_readonly(&db_path).await {
                    if let Ok(profile) = crate::services::memory::retrieve_personal_context_v7(
                        &conn,
                        &query_embedding,
                        scope,
                        &settings.memory,
                        settings.llm.context_window as usize,
                    )
                    .await
                    {
                        if !profile.trim().is_empty() {
                            retrieved_profile = Some(profile);
                        }
                    }
                }
            }
        }
    }

    let provider_kind = match settings.llm.active {
        crate::core::settings::LlmActiveProvider::Embedded => ProviderKind::Embedded,
        crate::core::settings::LlmActiveProvider::Server
        | crate::core::settings::LlmActiveProvider::Cloud => ProviderKind::OpenAiCompat,
    };

    let (conv_ctx, transition_speech, personal_memory) = {
        let mut cm = cm_arc.lock();
        cm.update_dynamic_user_profile(retrieved_profile);
        cm.push_user_turn(text.to_string());
        cm.build_context(provider_kind, is_deva, None, None)
    };

    if !personal_memory.is_empty() {
        let db_path = crate::utils::paths::db_path();
        let session_id = conv_id.to_string();
        tauri::async_runtime::spawn(async move {
            if let Ok(conn) = crate::persistence::db::VoxDb::open(&db_path).await {
                if let Err(e) = crate::persistence::mutations::enqueue_personal_facts(
                    &conn,
                    personal_memory,
                    &session_id,
                    true,
                )
                .await
                {
                    log::warn!("[Modular::Context] Failed to enqueue personal memory: {}", e);
                }
            }
        });
    }

    let request = GenerationRequest {
        input: ConversationInput {
            messages: conv_ctx.messages,
        },
        options: GenerationOptions {
            temperature: Some(settings.llm.temperature),
            max_output_tokens: Some(settings.llm.max_output_tokens),
            ..Default::default()
        },
        output: OutputConstraint::Text,
        purpose: GenerationPurpose::Conversation,
    };

    (request, transition_speech)
}

/// Triggers opportunistic background compaction if conversation memory utilization is in the soft window.
pub fn trigger_background_compaction(state: &AppState) {
    let candidate = state.conversation_manager.lock().try_trigger_opportunistic();
    if let Some((snapshot_len, messages, cancel_flag)) = candidate {
        let cm = Arc::clone(&state.conversation_manager);
        tauri::async_runtime::spawn(async move {
            if cancel_flag.load(Ordering::Relaxed) {
                return;
            }
            let mut summary_parts = Vec::new();
            for msg in &messages[1..messages.len().saturating_sub(1)] {
                let trimmed = msg.content.trim();
                if !trimmed.is_empty() {
                    summary_parts.push(trimmed.to_string());
                }
            }
            let summary = summary_parts.join("; ");
            if !cancel_flag.load(Ordering::Relaxed) && !summary.is_empty() {
                cm.lock().commit_opportunistic(snapshot_len, summary);
            }
        });
    }
}
