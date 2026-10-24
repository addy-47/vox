use std::{
    fs::read_dir,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};

use futures_util::future::BoxFuture;
use futures_util::StreamExt;
use serde_json::Value;

use crate::services::{
    harness::ConversationContext,
    llm::{
        GenerationRequest, LlmError, LlmModelInfo, LlmProvider, LlmStreamEvent,
        ProviderCapabilities, ProviderKind, Support,
    },
    sidecar::{global_sidecar_supervisor, SidecarKind},
};

pub struct SidecarLlmProvider {
    model_path: PathBuf,
    ctx_size: u32,
    n_threads: u32,
    capabilities: ProviderCapabilities,
}

impl SidecarLlmProvider {
    pub fn new(model_path: &Path, ctx_size: u32, n_threads: u32) -> Result<Self, LlmError> {
        let capabilities = ProviderCapabilities {
            temperature: Support::Supported,
            top_p: Support::Supported,
            top_k: Support::Supported,
            max_output_tokens: Support::Supported,
            json_object: Support::Supported,
            json_schema: Support::Unsupported,
            streaming: Support::Supported,
            seed: Support::Supported,
        };

        Ok(Self {
            model_path: model_path.to_path_buf(),
            ctx_size,
            n_threads,
            capabilities,
        })
    }

    pub fn list_models_in_dir(dir: &Path) -> Result<Vec<LlmModelInfo>, LlmError> {
        let mut models = Vec::new();
        if dir.exists() {
            if let Ok(entries) = read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("gguf") {
                        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                            let metadata = entry.metadata().ok();
                            let size_bytes = metadata.map(|m| m.len());
                            let clean_name = filename
                                .strip_suffix(".gguf")
                                .unwrap_or(filename)
                                .replace(['_', '-'], " ");

                            models.push(LlmModelInfo {
                                id: filename.to_string(),
                                name: clean_name,
                                description: format!("Local GGUF model: {}", filename),
                                context_length: Some(4096),
                                size_bytes,
                                is_local: true,
                            });
                        }
                    }
                }
            }
        }
        Ok(models)
    }
}

impl LlmProvider for SidecarLlmProvider {
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel: &'a tokio_util::sync::CancellationToken,
        tx: &'a mpsc::Sender<LlmStreamEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let model_str = self.model_path.to_string_lossy().to_string();
            let ctx_str = self.ctx_size.to_string();
            let threads_str = self.n_threads.to_string();

            let extra_args = [
                ("--model", model_str.as_str()),
                ("--ctx-size", ctx_str.as_str()),
                ("--threads", threads_str.as_str()),
            ];

            let base_url = global_sidecar_supervisor()
                .ensure_ready(SidecarKind::Llm, &extra_args)
                .await
                .map_err(|e| LlmError::Engine(format!("Failed to start LLM sidecar: {}", e)))?;

            let client = reqwest::Client::new();
            let completions_url = format!("{}/v1/chat/completions", base_url);

            let messages_payload: Vec<Value> = request
                .input
                .messages
                .into_iter()
                .map(|m| {
                    serde_json::json!({
                        "role": format!("{:?}", m.role).to_lowercase(),
                        "content": m.content
                    })
                })
                .collect();

            let payload = serde_json::json!({
                "model": "local",
                "messages": messages_payload,
                "stream": true,
                "temperature": request.options.temperature,
                "top_p": request.options.top_p,
                "top_k": request.options.top_k,
                "max_tokens": request.options.max_output_tokens,
                "stop": request.options.stop,
                "seed": request.options.seed,
            });

            let resp = client
                .post(&completions_url)
                .json(&payload)
                .send()
                .await
                .map_err(|e| LlmError::Engine(format!("LLM sidecar request failed: {}", e)))?;

            if !resp.status().is_success() {
                return Err(LlmError::Engine(format!(
                    "LLM sidecar HTTP error: {}",
                    resp.status()
                )));
            }

            let mut stream = resp.bytes_stream();
            let mut line_buf = String::new();

            while let Some(chunk_res) = stream.next().await {
                if cancel.is_cancelled() {
                    break;
                }

                let chunk = chunk_res
                    .map_err(|e| LlmError::Engine(format!("SSE chunk error: {}", e)))?;
                let text = String::from_utf8_lossy(&chunk);
                line_buf.push_str(&text);

                while let Some(newline_pos) = line_buf.find('\n') {
                    let line = line_buf[..newline_pos].trim().to_string();
                    line_buf.drain(..=newline_pos);

                    if let Some(data) = line.strip_prefix("data:") {
                        let trimmed = data.trim();
                        if trimmed == "[DONE]" {
                            let _ = tx.send(LlmStreamEvent::Finished {
                                finish_reason: Some("stop".to_string()),
                                input_tokens: None,
                                output_tokens: None,
                            });
                            return Ok(());
                        }

                        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                            if let Some(content) = val
                                .pointer("/choices/0/delta/content")
                                .and_then(|v| v.as_str())
                            {
                                if !content.is_empty() {
                                    let _ = tx.send(LlmStreamEvent::Token(content.to_string()));
                                }
                            }
                        }
                    }
                }
            }

            let _ = tx.send(LlmStreamEvent::Finished {
                finish_reason: Some("stop".to_string()),
                input_tokens: None,
                output_tokens: None,
            });

            Ok(())
        })
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    fn health_check<'a>(&'a self) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            if self.model_path.exists() {
                Ok(())
            } else {
                Err(LlmError::Engine(format!(
                    "Model path not found on disk: {:?}",
                    self.model_path
                )))
            }
        })
    }

    fn list_models<'a>(&'a self) -> BoxFuture<'a, Result<Vec<LlmModelInfo>, LlmError>> {
        Box::pin(async move {
            let dir = self.model_path.parent().unwrap_or_else(|| Path::new("."));
            Self::list_models_in_dir(dir)
        })
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Embedded
    }
}
