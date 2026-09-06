use super::{
    ConversationInput, EmbeddedProvider, GenerationOptions, GenerationPurpose, GenerationRequest,
    LlmProvider, OutputConstraint, RemoteTransport,
};
use crate::core::events::emit_ipc_to;
use crate::core::events::IpcEvent;
use crate::core::events::{Actionability, PipelineError, PipelineImpact, VoxEvent};
use crate::core::settings::{LlmProviderConfig, LlmSettings, VoxSettings};
use crate::core::state::InteractionOwner;
use std::path::Path;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
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
    pub llm_tx: &'a mut Option<mpsc::Sender<LlmCommand>>,
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
            Arc<parking_lot::Mutex<crate::pipeline::assistant::accumulator::TurnAccumulator>>,
        tts_tx: Option<mpsc::Sender<crate::services::tts::actor::TtsCommand>>,
        pending_synthesis_jobs: Arc<AtomicU32>,
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
    rx: mpsc::Receiver<LlmCommand>,
    provider: Arc<dyn LlmProvider>,
    event_tx: mpsc::Sender<VoxEvent>,
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
                let (stream_tx, stream_rx) = mpsc::channel::<super::LlmStreamEvent>();
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
                                    pending_synthesis_jobs.fetch_add(1, Ordering::Relaxed);
                                    if let Err(e) =
                                        tx.send(crate::services::tts::actor::TtsCommand::Generate {
                                            turn_id,
                                            text: clause,
                                        })
                                    {
                                        pending_synthesis_jobs.fetch_sub(1, Ordering::Relaxed);
                                        log::warn!(
                                            "[LLM Worker] Failed to dispatch clause to TTS: {}",
                                            e
                                        );
                                    }
                                }
                            }

                            let target =
                                crate::pipeline::target_window(InteractionOwner::Assistant);
                            if let Err(e) = emit_ipc_to(
                                &app,
                                target,
                                IpcEvent::LlmToken(crate::core::events::LlmTokenPayload {
                                    turn_id,
                                    token,
                                }),
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
                        if cancel.is_cancelled() {
                            log::info!("[LLM Worker] Generation cancelled (turn {})", turn_id);
                            let _ = event_tx.send(VoxEvent::Cancelled { turn_id });
                        } else if let Err(e) = event_tx.send(VoxEvent::LlmFinished { turn_id }) {
                            log::warn!("[LLM Worker] Failed to dispatch LlmFinished: {}", e);
                        }
                    }
                    Ok(Err(e)) => {
                        if cancel.is_cancelled() {
                            log::info!(
                                "[LLM Worker] Generation cancelled with error (turn {}): {}",
                                turn_id,
                                e
                            );
                            let _ = event_tx.send(VoxEvent::Cancelled { turn_id });
                        } else {
                            log::error!("[LLM Worker] Generation error (turn {}): {}", turn_id, e);
                            let err_str = e.to_string();
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
                                        hint: "LLM API Key is invalid or expired. Update credentials in Settings.".to_string(),
                                    },
                                )
                            } else {
                                (PipelineImpact::TurnAborted, Actionability::None)
                            };

                            if let Err(send_err) = event_tx.send(VoxEvent::Error(PipelineError {
                                turn_id,
                                message: err_str,
                                source: "LlmActor".to_string(),
                                impact,
                                actionability,
                            })) {
                                log::warn!("[LLM Worker] Failed to dispatch Error: {}", send_err);
                            }
                        }
                    }
                    Err(join_err) => {
                        if cancel.is_cancelled() {
                            log::info!(
                                "[LLM Worker] Generation task cancelled during join (turn {})",
                                turn_id
                            );
                            let _ = event_tx.send(VoxEvent::Cancelled { turn_id });
                        } else {
                            log::error!("[LLM Worker] Provider task join error: {}", join_err);
                            let is_panic = join_err.is_panic();
                            let msg = if is_panic {
                                "LLM provider panicked during generation".to_string()
                            } else {
                                format!("LLM provider task join failed: {}", join_err)
                            };
                            let _ = event_tx.send(VoxEvent::Error(PipelineError {
                                turn_id,
                                message: msg,
                                source: "LlmActor".to_string(),
                                impact: PipelineImpact::TurnAborted,
                                actionability: if is_panic {
                                    Actionability::Actionable {
                                        category: "llm_panic".to_string(),
                                        hint: "LLM worker recovered from internal panic. Please retry your turn.".to_string(),
                                    }
                                } else {
                                    Actionability::None
                                },
                            }));
                        }
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
    event_tx: mpsc::Sender<VoxEvent>,
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

    let (tx, rx) = mpsc::channel();
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
    llm_tx: &mut Option<mpsc::Sender<LlmCommand>>,
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
