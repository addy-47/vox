use std::{
    path::Path,
    sync::{mpsc, Arc},
    thread::{Builder, JoinHandle},
};

use super::{
    ConversationInput, EmbeddedProvider, GenerationOptions, GenerationPurpose, GenerationRequest,
    LlmProvider, OutputConstraint, RemoteTransport,
};
use crate::{
    core::{
        error::{Actionability, PipelineError, PipelineImpact},
        settings::{LlmProviderConfig, LlmSettings, VoxSettings},
    },
    services::harness::{ChatMessage, Role},
};

pub type LlmProviderCache = Arc<parking_lot::RwLock<Option<Arc<dyn LlmProvider>>>>;
pub type LlmWarmUpHandles<'a> = LlmWorkerHandles<'a>;

/// Stream event emitted back across the duplex pipe from LLM actor to harness plugin chassis.
#[derive(Debug, Clone)]
pub enum LlmResponse {
    Token(String),
    Finished,
    Cancelled,
    Error(PipelineError),
}

/// Policy defaults for a given generation purpose.
#[derive(Debug, Clone)]
pub struct GenerationDefaults {
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub output: OutputConstraint,
}

/// Generation policy engine translating user/system settings into generation requests.
#[derive(Debug, Clone)]
pub struct GenerationPolicy {
    pub conversation: GenerationDefaults,
    pub compaction: GenerationDefaults,
}

/// Mutable handles for managing the LLM worker lifecycle.
pub struct LlmWorkerHandles<'a> {
    pub llm_tx: &'a mut Option<mpsc::Sender<LlmCommand>>,
    pub llm_handle: &'a mut Option<JoinHandle<()>>,
    pub llm_provider_cache: Option<LlmProviderCache>,
}

/// Commands processed by the background LLM worker thread.
#[derive(Debug)]
pub enum LlmCommand {
    Warmup {
        system_prompt: String,
    },
    Generate {
        request: Box<GenerationRequest>,
        turn_id: u32,
        cancel: tokio_util::sync::CancellationToken,
        response_tx: mpsc::Sender<LlmResponse>,
    },
    Shutdown,
}

impl GenerationPolicy {
    /// Constructs policy from current `LlmSettings` and optional explicit compaction token ceiling.
    pub fn from_settings(settings: &LlmSettings, compaction_max_tokens: Option<u32>) -> Self {
        let compaction_tokens = compaction_max_tokens.unwrap_or(settings.max_output_tokens);

        Self {
            conversation: GenerationDefaults {
                temperature: settings.temperature,
                max_output_tokens: settings.max_output_tokens,
                output: OutputConstraint::Text,
            },
            compaction: GenerationDefaults {
                temperature: settings.compaction_temperature,
                max_output_tokens: compaction_tokens,
                output: OutputConstraint::JsonObject,
            },
        }
    }

    /// Builds a provider-neutral `GenerationRequest` for a specified purpose.
    pub fn build_request(
        &self,
        purpose: GenerationPurpose,
        input: ConversationInput,
    ) -> GenerationRequest {
        let defaults = match purpose {
            GenerationPurpose::Conversation => &self.conversation,
            GenerationPurpose::MemoryCompaction | GenerationPurpose::StructuredExtraction => {
                &self.compaction
            }
        };

        GenerationRequest {
            input,
            options: GenerationOptions {
                temperature: Some(defaults.temperature),
                max_output_tokens: Some(defaults.max_output_tokens),
                ..Default::default()
            },
            output: defaults.output.clone(),
            purpose,
        }
    }
}

/// Spawns the dedicated LLM generation worker thread and runs its command loop.
pub fn spawn_llm_worker(rx: mpsc::Receiver<LlmCommand>, provider: Arc<dyn LlmProvider>) {
    log::info!("[Llm::Worker] Persistent loop started.");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to build LLM worker runtime");

    while let Ok(cmd) = rx.recv() {
        match cmd {
            LlmCommand::Warmup { system_prompt } => {
                handle_warmup(&runtime, &provider, system_prompt);
            }
            LlmCommand::Generate {
                request,
                turn_id,
                cancel,
                response_tx,
            } => {
                handle_generate(&runtime, &provider, request, turn_id, cancel, response_tx);
            }
            LlmCommand::Shutdown => {
                log::info!("[Llm::Worker] Shutdown command received. Exiting loop.");
                break;
            }
        }
    }

    log::info!("[Llm::Worker] Loop exited. Provider will be dropped.");
}

/// Warms up LLM model with system prompt and empty warmup request.
fn handle_warmup(
    runtime: &tokio::runtime::Runtime,
    provider: &Arc<dyn LlmProvider>,
    system_prompt: String,
) {
    let (stream_tx, stream_rx) = mpsc::channel::<super::LlmStreamEvent>();
    let provider_clone = Arc::clone(provider);
    let cancel = tokio_util::sync::CancellationToken::new();
    let warmup_request = GenerationRequest {
        input: ConversationInput {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: system_prompt,
                    timestamp_ms: 0,
                },
                ChatMessage {
                    role: Role::User,
                    content: "[WARMUP]".to_string(),
                    timestamp_ms: 0,
                },
            ],
        },
        options: GenerationOptions::default(),
        output: OutputConstraint::Text,
        purpose: GenerationPurpose::Conversation,
    };
    let gen_handle = runtime.spawn(async move {
        provider_clone
            .generate(warmup_request, 0, &cancel, &stream_tx)
            .await
    });
    while stream_rx.recv().is_ok() {}
    if let Err(e) = runtime.block_on(gen_handle) {
        log::warn!("[Llm::Worker] Warmup task join error: {:?}", e);
    }
}

/// Executes LLM generation and streams tokens and completion across duplex response pipe.
fn handle_generate(
    runtime: &tokio::runtime::Runtime,
    provider: &Arc<dyn LlmProvider>,
    request: Box<GenerationRequest>,
    turn_id: u32,
    cancel: tokio_util::sync::CancellationToken,
    response_tx: mpsc::Sender<LlmResponse>,
) {
    let (stream_tx, stream_rx) = mpsc::channel::<super::LlmStreamEvent>();
    let provider_clone = Arc::clone(provider);
    let cancel_clone = cancel.clone();
    let gen_handle = runtime.spawn(async move {
        provider_clone
            .generate(*request, turn_id, &cancel_clone, &stream_tx)
            .await
    });

    while let Ok(event) = stream_rx.recv() {
        match event {
            super::LlmStreamEvent::Token(token) => {
                if cancel.is_cancelled() {
                    break;
                }
                if let Err(e) = response_tx.send(LlmResponse::Token(token)) {
                    log::debug!("[Llm::Worker] response_tx disconnected: {}", e);
                    break;
                }
            }
            super::LlmStreamEvent::Finished => break,
        }
    }

    match runtime.block_on(gen_handle) {
        Ok(Ok(())) => {
            if cancel.is_cancelled() {
                log::info!("[Llm::Worker] Generation cancelled (turn {})", turn_id);
                if let Err(e) = response_tx.send(LlmResponse::Cancelled) {
                    log::debug!("[Llm::Worker] Failed to send Cancelled: {}", e);
                }
            } else if let Err(e) = response_tx.send(LlmResponse::Finished) {
                log::debug!(
                    "[Llm::Worker] response_tx disconnected before Finished: {}",
                    e
                );
            }
        }
        Ok(Err(e)) => {
            if cancel.is_cancelled() {
                log::info!(
                    "[Llm::Worker] Generation cancelled with error (turn {}): {}",
                    turn_id,
                    e
                );
                if let Err(send_err) = response_tx.send(LlmResponse::Cancelled) {
                    log::debug!("[Llm::Worker] Failed to send Cancelled: {}", send_err);
                }
            } else {
                log::error!("[Llm::Worker] Generation error (turn {}): {}", turn_id, e);
                let err = classify_llm_error(turn_id, e.to_string());
                if let Err(send_err) = response_tx.send(LlmResponse::Error(err)) {
                    log::warn!("[Llm::Worker] Failed to dispatch Error: {}", send_err);
                }
            }
        }
        Err(join_err) => {
            if cancel.is_cancelled() {
                log::info!(
                    "[Llm::Worker] Generation task cancelled during join (turn {})",
                    turn_id
                );
                if let Err(e) = response_tx.send(LlmResponse::Cancelled) {
                    log::debug!("[Llm::Worker] Failed to send Cancelled: {}", e);
                }
            } else {
                log::error!("[Llm::Worker] Provider task join error: {}", join_err);
                let is_panic = join_err.is_panic();
                let msg = if is_panic {
                    "LLM provider panicked during generation".to_string()
                } else {
                    format!("LLM provider task join failed: {}", join_err)
                };
                let err = PipelineError {
                    turn_id,
                    message: msg,
                    source: "LlmActor".to_string(),
                    impact: PipelineImpact::TurnAborted,
                    actionability: if is_panic {
                        Actionability::Actionable {
                            category: "llm_panic".to_string(),
                            hint:
                                "LLM worker recovered from internal panic. Please retry your turn."
                                    .to_string(),
                        }
                    } else {
                        Actionability::None
                    },
                };
                if let Err(send_err) = response_tx.send(LlmResponse::Error(err)) {
                    log::warn!("[Llm::Worker] Failed to dispatch Error: {}", send_err);
                }
            }
        }
    }
}

/// Classifies error string into standard pipeline impact and actionability metadata.
fn classify_llm_error(turn_id: u32, err_str: String) -> PipelineError {
    let (impact, actionability) = if err_str.contains("context window")
        || err_str.contains("context length")
        || err_str.contains("prompt too long")
        || err_str.contains("NoKvCacheSlot")
    {
        (
            PipelineImpact::TurnAborted,
            Actionability::Actionable {
                category: "context_overflow".to_string(),
                hint: "Prompt exceeded LLM context window. Increase context_window in Settings or run compaction.".to_string(),
            },
        )
    } else if err_str.contains("401")
        || err_str.contains("Unauthorized")
        || err_str.contains("API key")
    {
        (
            PipelineImpact::SessionHalted,
            Actionability::Actionable {
                category: "auth_failure".to_string(),
                hint: "LLM API Key is invalid or expired. Update credentials in Settings."
                    .to_string(),
            },
        )
    } else {
        (PipelineImpact::TurnAborted, Actionability::None)
    };

    PipelineError {
        turn_id,
        message: err_str,
        source: "LlmActor".to_string(),
        impact,
        actionability,
    }
}

/// Creates a boxed LLM provider directly from `LlmSettings` configuration.
pub fn create_llm_provider_from_llm_settings(
    llm_settings: &LlmSettings,
    llm_path: &Path,
) -> Result<Box<dyn LlmProvider>, String> {
    let provider_config = llm_settings.to_provider_config();
    let ctx_size = llm_settings.context_window;
    let n_threads = llm_settings.threads;

    match provider_config {
        LlmProviderConfig::Embedded => EmbeddedProvider::new(llm_path, ctx_size, n_threads)
            .map(|p| Box::new(p) as Box<dyn LlmProvider>)
            .map_err(|e| e.to_string()),
        LlmProviderConfig::OpenAiCompat {
            base_url,
            model,
            api_key,
            provider_name,
        } => {
            let conn_cfg = super::transport::ConnectionConfig::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let provider = RemoteTransport::new(conn_cfg);
            Ok(Box::new(provider) as Box<dyn LlmProvider>)
        }
    }
}

/// Creates a boxed LLM provider based on settings configuration.
pub fn create_llm_provider(
    settings: &VoxSettings,
    llm_path: &Path,
) -> Result<Box<dyn LlmProvider>, String> {
    create_llm_provider_from_llm_settings(&settings.llm, llm_path)
}

/// Spawns and initializes a persistent LLM worker actor thread.
pub fn warm_up_llm(
    handles: LlmWarmUpHandles<'_>,
    settings: &VoxSettings,
    llm_path: &Path,
) -> Result<(), String> {
    if handles.llm_tx.is_some() {
        return Ok(());
    }

    log::info!("[Llm::Actor] Warming up LLM worker");

    let provider = match create_llm_provider(settings, llm_path) {
        Ok(p) => p,
        Err(e) => {
            log::error!("[Llm::Actor] Failed to create provider: {}", e);
            return Err(e);
        }
    };

    let provider_arc: Arc<dyn LlmProvider> = Arc::from(provider);
    if let Some(ref cache) = handles.llm_provider_cache {
        *cache.write() = Some(Arc::clone(&provider_arc));
    }

    let (tx, rx) = mpsc::channel();
    *handles.llm_tx = Some(tx);

    let worker_provider = Arc::clone(&provider_arc);

    let handle = Builder::new()
        .name("vox-llm-persistent".to_string())
        .spawn(move || {
            spawn_llm_worker(rx, worker_provider);
        })
        .map_err(|e| e.to_string())?;

    *handles.llm_handle = Some(handle);
    Ok(())
}

/// Signals the running LLM worker thread to shutdown and drop its model instance.
pub fn cool_down_llm(
    llm_tx: &mut Option<mpsc::Sender<LlmCommand>>,
    llm_provider_cache: Option<&LlmProviderCache>,
) {
    if let Some(cache) = llm_provider_cache {
        *cache.write() = None;
    }
    if let Some(tx) = llm_tx.take() {
        if let Err(e) = tx.send(LlmCommand::Shutdown) {
            log::warn!("[Llm::Actor] Failed to send Shutdown command: {}", e);
        }
        log::info!("[Llm::Actor] Shutdown command sent (offloaded)");
    }
}
