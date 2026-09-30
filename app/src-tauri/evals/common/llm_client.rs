//! ============================================================================
//! evals/common/llm_client.rs — Recording LLM Provider Proxy & NVIDIA Judge Client
//! ============================================================================

use std::{
    fs,
    future::Future,
    path::Path,
    pin::Pin,
    sync::{mpsc, Arc},
    time::Instant,
};

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use vox_lib::services::{
    harness::Role,
    llm::{
        ConnectionConfig, GenerationOptions, GenerationRequest, LlmError, LlmProvider,
        LlmStreamEvent, OutputConstraint, ProviderCapabilities, ProviderKind, RemoteTransport,
    },
};

/// Runtime request payload (excluding system prompt as it is in code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRequestTrace {
    pub messages: Vec<RuntimeMessageTrace>,
    pub options: GenerationOptions,
    pub output: OutputConstraint,
    pub purpose: String,
}

/// Message payload captured in runtime traces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMessageTrace {
    pub role: String,
    pub content: String,
}

/// Verbatim record of an LLM generation call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawLlmTrace {
    pub timestamp_utc: String,
    pub case_id: String,
    pub layer: String,
    pub turn_id: u32,
    pub model: String,
    pub request: RuntimeRequestTrace,
    pub raw_response: String,
    pub duration_ms: u64,
}

struct RecorderState {
    current_case_id: String,
    current_layer: String,
    traces: Vec<RawLlmTrace>,
}

/// Transparent recording wrapper around any `LlmProvider`.
pub struct RecordingLlmProvider {
    inner: Arc<dyn LlmProvider>,
    model_name: String,
    state: Arc<Mutex<RecorderState>>,
}

impl RecordingLlmProvider {
    pub fn new(inner: Arc<dyn LlmProvider>, model_name: String) -> Self {
        Self {
            inner,
            model_name,
            state: Arc::new(Mutex::new(RecorderState {
                current_case_id: "init".to_string(),
                current_layer: "init".to_string(),
                traces: Vec::new(),
            })),
        }
    }

    /// Sets the active evaluation context for upcoming generations.
    pub fn set_context(&self, case_id: &str, layer: &str) {
        let mut state = self.state.lock();
        state.current_case_id = case_id.to_string();
        state.current_layer = layer.to_string();
    }

    /// Flushes all traces captured for the current case to `<case_dir>/raw_llm_traces.json`.
    pub fn flush_case_traces(&self, case_dir: &Path) -> Result<()> {
        let mut state = self.state.lock();
        let traces_file = case_dir.join("raw_llm_traces.json");
        let json_data = serde_json::to_string_pretty(&state.traces)
            .map_err(|e| anyhow!("Failed to serialize raw LLM traces: {}", e))?;
        fs::write(&traces_file, json_data)
            .map_err(|e| anyhow!("Failed to write raw LLM traces to {:?}: {}", traces_file, e))?;
        state.traces.clear();
        Ok(())
    }
}

impl LlmProvider for RecordingLlmProvider {
    fn kind(&self) -> ProviderKind {
        self.inner.kind()
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        self.inner.capabilities()
    }

    fn health_check<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<(), LlmError>> + Send + 'a>> {
        self.inner.health_check()
    }

    fn list_models<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<vox_lib::services::llm::LlmModelInfo>, LlmError>> + Send + 'a>> {
        self.inner.list_models()
    }

    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel: &'a CancellationToken,
        tx: &'a mpsc::Sender<LlmStreamEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<(), LlmError>> + Send + 'a>> {
        Box::pin(async move {
            let start = Instant::now();
            let (inter_tx, inter_rx) = mpsc::channel();
            let external_tx = tx.clone();

            // Filter out system messages from the runtime trace per user constraint
            let runtime_messages: Vec<RuntimeMessageTrace> = request
                .input
                .messages
                .iter()
                .filter(|m| m.role != Role::System)
                .map(|m| RuntimeMessageTrace {
                    role: format!("{:?}", m.role).to_lowercase(),
                    content: m.content.clone(),
                })
                .collect();

            let purpose_str = format!("{:?}", request.purpose);
            let options_clone = request.options.clone();
            let output_clone = request.output.clone();

            // Forwarder thread capturing streamed tokens
            let (done_tx, done_rx) = tokio::sync::oneshot::channel::<String>();
            std::thread::spawn(move || {
                let mut accumulated = String::new();
                while let Ok(event) = inter_rx.recv() {
                    match &event {
                        LlmStreamEvent::Token(tok) => {
                            accumulated.push_str(tok);
                        }
                        LlmStreamEvent::Finished => {}
                        _ => {}
                    }
                    let _ = external_tx.send(event);
                }
                let _ = done_tx.send(accumulated);
            });

            let res = self.inner.generate(request, turn_id, cancel, &inter_tx).await;
            drop(inter_tx);

            let raw_response = done_rx.await.unwrap_or_default();
            let duration_ms = start.elapsed().as_millis() as u64;

            let (case_id, layer) = {
                let s = self.state.lock();
                (s.current_case_id.clone(), s.current_layer.clone())
            };

            let trace = RawLlmTrace {
                timestamp_utc: chrono::Utc::now().to_rfc3339(),
                case_id,
                layer,
                turn_id,
                model: self.model_name.clone(),
                request: RuntimeRequestTrace {
                    messages: runtime_messages,
                    options: options_clone,
                    output: output_clone,
                    purpose: purpose_str,
                },
                raw_response,
                duration_ms,
            };

            self.state.lock().traces.push(trace);
            res
        })
    }
}

/// Builds the memory pipeline provider connecting to an Ollama or OpenAI-compatible server.
pub fn create_pipeline_provider(
    base_url: &str,
    model: &str,
    api_key: Option<&str>,
) -> Arc<dyn LlmProvider> {
    let conn_cfg = ConnectionConfig::new(base_url, model, api_key, Some("pipeline_eval"));
    Arc::new(RemoteTransport::new(conn_cfg))
}

/// Dedicated HTTP client for querying the NVIDIA NIM Judge LLM API.
pub struct NvidiaJudgeClient {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl NvidiaJudgeClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            api_url: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
            api_key,
            model,
        }
    }

    /// Evaluates a prompt via the judge model and returns the verbatim markdown output.
    pub async fn evaluate(&self, prompt: &str) -> Result<String> {
        let payload = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.1,
            "top_p": 0.9,
            "max_tokens": 4096
        });

        let resp = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| anyhow!("NVIDIA Judge HTTP request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("NVIDIA Judge returned error {}: {}", status, body));
        }

        let resp_json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse NVIDIA Judge JSON response: {}", e))?;

        let content = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing content in NVIDIA Judge response choices"))?;

        Ok(content.to_string())
    }
}
