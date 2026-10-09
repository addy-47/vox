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
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<vox_lib::services::llm::LlmModelInfo>, LlmError>>
                + Send
                + 'a,
        >,
    > {
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

            let res = self
                .inner
                .generate(request, turn_id, cancel, &inter_tx)
                .await;
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

/// Dedicated HTTP client for querying the judge LLM.
///
/// Two transports:
/// - **Local Ollama native** (`http://127.0.0.1:11434/api/chat`) — the default.
///   Full control over `num_ctx`/`num_predict`, thinking disabled. No auth.
/// - **Cloud gateways** (OpenRouter / NVIDIA NIM, OpenAI-compatible
///   `/chat/completions`) — legacy path, selected when an API key is supplied.
pub struct JudgeClient {
    client: reqwest::Client,
    api_url: String,
    api_key: Option<String>,
    model: String,
    seed: Option<u64>,
    temperature: f32,
    num_ctx: u32,
    num_predict: u32,
}

impl JudgeClient {
    pub fn new(api_key: Option<String>, model: String) -> Self {
        Self::with_endpoint(api_key, model, "http://127.0.0.1:11434/api/chat")
    }

    /// Builds a judge client against an explicit endpoint. A URL containing
    /// `/api/chat` selects the native Ollama protocol; anything else is treated
    /// as OpenAI-compatible `/chat/completions`.
    pub fn with_endpoint(api_key: Option<String>, model: String, api_url: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(900))
                .build()
                .unwrap_or_default(),
            api_url: api_url.to_string(),
            api_key,
            model,
            seed: None,
            temperature: 0.0,
            num_ctx: 32768,
            num_predict: 16384,
        }
    }

    pub fn with_seed(mut self, seed: Option<u64>) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.num_predict = max_tokens;
        self
    }

    fn is_native(&self) -> bool {
        self.api_url.contains("/api/chat")
    }

    fn is_local(&self) -> bool {
        self.api_key.is_none()
    }

    /// Evaluates a prompt via the judge model and returns the raw response body.
    ///
    /// Temperature is pinned to 0.0 and thinking is disabled on the native
    /// transport, so the judge is reproducible for a given seed. The judge is
    /// never asked for markdown: callers parse the body into a typed verdict.
    pub async fn evaluate(&self, prompt: &str) -> Result<String> {
        if self.is_native() {
            self.evaluate_native(prompt).await
        } else {
            self.evaluate_openai_compat(prompt).await
        }
    }

    async fn evaluate_native(&self, prompt: &str) -> Result<String> {
        let mut options = serde_json::json!({
            "temperature": self.temperature,
            "num_ctx": self.num_ctx,
            "num_predict": self.num_predict,
        });
        if let Some(seed) = self.seed {
            options["seed"] = serde_json::json!(seed);
        }

        let payload = serde_json::json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": prompt }],
            "stream": false,
            "think": false,
            "options": options,
        });

        let resp = self
            .client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                anyhow!(
                    "Judge HTTP request failed (is_timeout: {}, is_connect: {}): {}",
                    e.is_timeout(),
                    e.is_connect(),
                    e
                )
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Judge returned error {}: {}", status, body));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse Judge response: {}", e))?;

        let done_reason = body["done_reason"].as_str().unwrap_or("unknown");
        // Native Ollama reports "length" when num_predict (or num_ctx) is exhausted.
        if done_reason == "length" {
            return Err(anyhow!(
                "Judge output was truncated by the model (done_reason = length). Verdict was not produced."
            ));
        }

        body["message"]["content"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Missing content in Judge response: {}", body))
    }

    async fn evaluate_openai_compat(&self, prompt: &str) -> Result<String> {
        let mut payload = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": self.temperature,
            "max_tokens": self.num_predict,
            "stream": false
        });
        if let Some(seed) = self.seed {
            payload["seed"] = serde_json::json!(seed);
        }

        let mut req = self
            .client
            .post(&self.api_url)
            .header("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            req = req
                .header("Authorization", format!("Bearer {}", key))
                .header("HTTP-Referer", "https://github.com/addy-47/vox")
                .header("X-Title", "Vox Memory Eval");
        }

        let resp = req.json(&payload).send().await.map_err(|e| {
            anyhow!(
                "Judge HTTP request failed (is_timeout: {}, is_connect: {}): {}",
                e.is_timeout(),
                e.is_connect(),
                e
            )
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Judge returned error {}: {}", status, body));
        }

        let resp_json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse Judge JSON response: {}", e))?;

        let choice = &resp_json["choices"][0];
        let finish_reason = choice["finish_reason"].as_str().unwrap_or("unknown");
        if finish_reason == "length" {
            return Err(anyhow!(
                "Judge output was truncated by LLM provider (finish_reason = length). Verdict was not produced."
            ));
        }

        let choice_msg = &choice["message"];
        let content = choice_msg["content"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "Missing content in Judge response choices: {:?}",
                    choice_msg
                )
            })?;

        Ok(content.to_string())
    }

    /// Evaluates a prompt, persists the full request/response pair to
    /// `<case_dir>/judge_traces.json`, and returns the raw body.
    pub async fn evaluate_with_trace(
        &self,
        prompt: &str,
        case_dir: &std::path::Path,
        stage: &str,
    ) -> Result<String> {
        let content = self.evaluate(prompt).await?;

        let trace_entry = serde_json::json!({
            "stage": stage,
            "model": self.model,
            "endpoint": if self.is_local() { "local" } else { "cloud" },
            "transport": if self.is_native() { "ollama_native" } else { "openai_compat" },
            "temperature": self.temperature,
            "seed": self.seed,
            "num_ctx": self.num_ctx,
            "num_predict": self.num_predict,
            "prompt": prompt,
            "response": content,
            "timestamp_ms": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        });

        let traces_path = case_dir.join("judge_traces.json");
        let mut traces: Vec<serde_json::Value> = if traces_path.exists() {
            let data = std::fs::read_to_string(&traces_path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        traces.push(trace_entry);
        if let Ok(serialized) = serde_json::to_string_pretty(&traces) {
            let _ = std::fs::write(&traces_path, serialized);
        }

        Ok(content)
    }
}
