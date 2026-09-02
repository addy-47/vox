use super::{
    ConversationInput, EmbeddedProvider, GenerationOptions, GenerationPurpose, GenerationRequest,
    LlmProvider, OutputConstraint, RemoteTransport,
};
use crate::core::events::VoxEvent;
use crate::core::settings::{LlmProviderConfig, LlmSettings, VoxSettings};
use std::path::Path;
use std::sync::Arc;

pub type LlmProviderCache = Arc<parking_lot::RwLock<Option<Arc<dyn LlmProvider>>>>;

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

/// Handles and flags passed when warming up the LLM actor.
pub struct LlmWarmUpHandles<'a> {
    pub llm_tx: &'a mut Option<std::sync::mpsc::Sender<LlmCommand>>,
    pub llm_handle: &'a mut Option<std::thread::JoinHandle<()>>,
    pub llm_provider_cache: Option<LlmProviderCache>,
}

/// Commands processed by the background LLM worker thread.
#[derive(Debug)]
pub enum LlmCommand {
    Generate {
        request: Box<GenerationRequest>,
        turn_id: u32,
        cancel: tokio_util::sync::CancellationToken,
        accumulator:
            Arc<parking_lot::Mutex<crate::pipeline::handlers::accumulator::TurnAccumulator>>,
        tts_tx: Option<std::sync::mpsc::Sender<crate::services::tts::actor::TtsCommand>>,
        pending_synthesis_jobs: Arc<std::sync::atomic::AtomicU32>,
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
pub fn spawn_llm_worker<R: tauri::Runtime + 'static>(
    app: tauri::AppHandle<R>,
    rx: std::sync::mpsc::Receiver<LlmCommand>,
    provider: Arc<dyn LlmProvider>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
) {
    log::info!("[LLM Worker] Persistent loop started.");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to build LLM worker runtime");

    while let Ok(cmd) = rx.recv() {
        match cmd {
            LlmCommand::Generate {
                request,
                turn_id,
                cancel,
                accumulator,
                tts_tx,
                pending_synthesis_jobs,
            } => {
                let (stream_tx, stream_rx) = std::sync::mpsc::channel::<super::LlmStreamEvent>();
                let provider_clone = Arc::clone(&provider);
                let cancel_clone = cancel.clone();
                let gen_handle = runtime.spawn(async move {
                    provider_clone
                        .generate(*request, turn_id, &cancel_clone, &stream_tx)
                        .await
                });

                while let Ok(event) = stream_rx.recv() {
                    match event {
                        super::LlmStreamEvent::Token(token) => {
                            let clauses = accumulator.lock().push_token(&token);
                            if let Some(ref tx) = tts_tx {
                                for clause in clauses {
                                    pending_synthesis_jobs
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    if let Err(e) =
                                        tx.send(crate::services::tts::actor::TtsCommand::Generate {
                                            turn_id,
                                            text: clause,
                                        })
                                    {
                                        pending_synthesis_jobs
                                            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                                        log::warn!(
                                            "[LLM Worker] Failed to dispatch clause to TTS: {}",
                                            e
                                        );
                                    }
                                }
                            }

                            let target = crate::pipeline::target_window(
                                crate::core::state::InteractionOwner::Assistant,
                            );
                            if let Err(e) = crate::core::events::emit_ipc_to(
                                &app,
                                target,
                                crate::core::events::IpcEvent::LlmToken(
                                    crate::core::events::LlmTokenPayload { turn_id, token },
                                ),
                            ) {
                                log::trace!("[LLM Worker] Failed to emit LlmToken IPC: {}", e);
                            }
                        }
                        super::LlmStreamEvent::Finished => {
                            break;
                        }
                    }
                }

                match runtime.block_on(gen_handle) {
                    Ok(Ok(())) => {
                        if let Err(e) = event_tx.send(VoxEvent::LlmFinished { turn_id }) {
                            log::warn!("[LLM Worker] Failed to dispatch LlmFinished: {}", e);
                        }
                    }
                    Ok(Err(e)) => {
                        log::error!("[LLM Worker] Generation error (turn {}): {}", turn_id, e);
                        if let Err(send_err) = event_tx.send(VoxEvent::Error {
                            turn_id,
                            message: e.to_string(),
                            source: "LlmActor".to_string(),
                        }) {
                            log::warn!("[LLM Worker] Failed to dispatch Error: {}", send_err);
                        }
                    }
                    Err(join_err) => {
                        log::error!("[LLM Worker] Provider task join error: {}", join_err);
                    }
                }
            }
            LlmCommand::Shutdown => {
                log::info!("[LLM Worker] Shutdown command received. Exiting loop.");
                break;
            }
        }
    }

    log::info!("[LLM Worker] Loop exited. Provider will be dropped.");
}

/// Creates a boxed LLM provider directly from LlmSettings configuration.
pub fn create_llm_provider_from_llm_settings(
    llm_settings: &crate::core::settings::LlmSettings,
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
pub fn warm_up_llm<R: tauri::Runtime + 'static>(
    app: &tauri::AppHandle<R>,
    handles: LlmWarmUpHandles<'_>,
    settings: &VoxSettings,
    llm_path: &Path,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
) -> Result<(), String> {
    if handles.llm_tx.is_some() {
        return Ok(());
    }

    log::info!("[LLM Actor] Warming up LLM worker");

    let provider = match create_llm_provider(settings, llm_path) {
        Ok(p) => p,
        Err(e) => {
            log::error!("[LLM Actor] Failed to create provider: {}", e);
            return Err(e);
        }
    };

    let provider_arc: Arc<dyn LlmProvider> = Arc::from(provider);
    if let Some(ref cache) = handles.llm_provider_cache {
        *cache.write() = Some(Arc::clone(&provider_arc));
    }

    let (tx, rx) = std::sync::mpsc::channel();
    *handles.llm_tx = Some(tx);

    let app_clone = app.clone();
    let worker_provider = Arc::clone(&provider_arc);

    let handle = std::thread::Builder::new()
        .name("vox-llm-persistent".to_string())
        .spawn(move || {
            spawn_llm_worker(app_clone, rx, worker_provider, event_tx);
        })
        .map_err(|e| e.to_string())?;

    *handles.llm_handle = Some(handle);
    Ok(())
}

/// Signals the running LLM worker thread to shutdown and drop its model instance.
pub fn cool_down_llm(
    llm_tx: &mut Option<std::sync::mpsc::Sender<LlmCommand>>,
    llm_provider_cache: Option<&LlmProviderCache>,
) {
    if let Some(cache) = llm_provider_cache {
        *cache.write() = None;
    }
    if let Some(tx) = llm_tx.take() {
        if let Err(e) = tx.send(LlmCommand::Shutdown) {
            log::warn!("[LLM Actor] Failed to send Shutdown command: {}", e);
        }
        log::info!("[LLM Actor] Shutdown command sent (offloaded)");
    }
}
