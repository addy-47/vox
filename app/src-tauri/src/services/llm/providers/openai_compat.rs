use super::{LlmProvider, ProviderKind};
use crate::core::events::VoxEvent;
use crate::core::settings::RemoteModelInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;

pub struct OpenAiCompatProvider {
    base_url: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::blocking::Client,
    async_client: reqwest::Client,
}

impl OpenAiCompatProvider {
    pub fn new(base_url: &str, model: &str, api_key: Option<&str>) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        let async_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            base_url,
            model: model.to_string(),
            api_key: api_key.map(|s| s.to_string()),
            client,
            async_client,
        }
    }
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Deserialize)]
struct ChunkDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
    size: u64,
    details: OllamaModelDetails,
}

#[derive(Deserialize)]
struct OllamaModelDetails {
    quantization_level: Option<String>,
    family: Option<String>,
}

impl LlmProvider for OpenAiCompatProvider {
    fn generate(
        &self,
        text: &str,
        system_prompt: &str,
        turn_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &mpsc::Sender<VoxEvent>,
    ) -> anyhow::Result<()> {
        log::info!(
            "[OpenAiCompat] Starting generation for turn {} on model {} with url {}",
            turn_id,
            self.model,
            self.base_url
        );

        if user_text_is_warmup(text) {
            log::info!("[OpenAiCompat] Warmup request received. Skipping remote LLM call.");
            return Ok(());
        }

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: text.to_string(),
            },
        ];

        let req_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            stream: true,
        };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        rt.block_on(async {
            let url = format!("{}/v1/chat/completions", self.base_url);
            let mut builder = self.async_client.post(&url).json(&req_body);

            if let Some(ref key) = self.api_key {
                builder = builder.bearer_auth(key);
            }

            let response = builder.send().await?;

            if !response.status().is_success() {
                let status = response.status();
                let err_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(anyhow::anyhow!(
                    "HTTP Error: {} - Content: {}",
                    status,
                    err_text
                ));
            }

            let mut stream = response.bytes_stream();
            let mut buffer = Vec::new();
            let mut finished = false;

            loop {
                if cancel_flag.load(Ordering::Relaxed) {
                    log::info!("[OpenAiCompat] Generation cancelled for turn {}", turn_id);
                    let _ = tx.send(VoxEvent::Cancelled { turn_id });
                    return Ok(());
                }

                // Await the next stream chunk with a 150ms timeout to ensure we check cancel_flag frequently
                let chunk_opt = match tokio::time::timeout(Duration::from_millis(150), stream.next()).await {
                    Ok(Some(chunk_result)) => {
                        match chunk_result {
                            Ok(c) => Some(c),
                            Err(e) => {
                                log::error!("[OpenAiCompat] Stream read error: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        // EOF
                        break;
                    }
                    Err(_) => {
                        // Timeout hit, loop again to check cancel_flag
                        None
                    }
                };

                if let Some(chunk) = chunk_opt {
                    buffer.extend_from_slice(&chunk);

                    // Extract lines from the buffer
                    while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                        let line_bytes = buffer.drain(..=pos).collect::<Vec<u8>>();
                        if let Ok(line) = String::from_utf8(line_bytes) {
                            let trimmed = line.trim();
                            process_line(trimmed, turn_id, tx, &mut finished);
                            if finished {
                                break;
                            }
                        }
                    }

                    if finished {
                        break;
                    }
                }
            }

            // Process any remaining data in buffer as a final line
            if !finished && !buffer.is_empty() {
                if let Ok(line) = String::from_utf8(buffer) {
                    let trimmed = line.trim();
                    process_line(trimmed, turn_id, tx, &mut finished);
                }
            }

            if !finished && cancel_flag.load(Ordering::Relaxed) {
                let _ = tx.send(VoxEvent::Cancelled { turn_id });
            } else {
                let _ = tx.send(VoxEvent::LlmFinished { turn_id });
            }

            Ok(())
        })
    }

    fn health_check(&self) -> bool {
        let url = format!("{}/v1/models", self.base_url);
        let mut builder = self.client.get(&url).timeout(Duration::from_secs(3));
        if let Some(ref key) = self.api_key {
            builder = builder.bearer_auth(key);
        }

        match builder.send() {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    fn list_models(&self) -> anyhow::Result<Vec<RemoteModelInfo>> {
        use crate::core::settings::RemoteModelInfo;

        // Try Ollama-specific /api/tags first
        let ollama_url = format!("{}/api/tags", self.base_url);
        let mut builder = self.client.get(&ollama_url).timeout(Duration::from_secs(3));
        if let Some(ref key) = self.api_key {
            builder = builder.bearer_auth(key);
        }

        if let Ok(resp) = builder.send() {
            if resp.status().is_success() {
                if let Ok(ollama_resp) = resp.json::<OllamaTagsResponse>() {
                    let models = ollama_resp
                        .models
                        .into_iter()
                        .map(|m| {
                            let clean_name = m.name
                                .replace(':', " ")
                                .replace('_', " ")
                                .replace('-', " ");
                            RemoteModelInfo {
                                id: m.name.clone(),
                                name: clean_name,
                                size_bytes: Some(m.size),
                                quantization: m.details.quantization_level,
                                family: m.details.family.map(|f| {
                                    let mut c = f.chars();
                                    match c.next() {
                                        None => String::new(),
                                        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                                    }
                                }),
                                provider_kind: "open_ai_compat".to_string(),
                            }
                        })
                        .collect();
                    return Ok(models);
                }
            }
        }

        // Fallback: standard /v1/models
        let url = format!("{}/v1/models", self.base_url);
        let mut builder = self.client.get(&url).timeout(Duration::from_secs(3));
        if let Some(ref key) = self.api_key {
            builder = builder.bearer_auth(key);
        }

        let resp = builder.send()?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error listing models: {}", resp.status()));
        }

        let model_list = resp.json::<ModelList>()?;
        let models = model_list
            .data
            .into_iter()
            .map(|m| {
                let (clean_name, quantization, family) = parse_heuristic_metadata(&m.id);
                RemoteModelInfo {
                    id: m.id,
                    name: clean_name,
                    size_bytes: None,
                    quantization,
                    family,
                    provider_kind: "open_ai_compat".to_string(),
                }
            })
            .collect();

        Ok(models)
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAiCompat
    }
}

fn user_text_is_warmup(text: &str) -> bool {
    text.is_empty() || text == "[WARMUP]"
}

fn process_line(
    line: &str,
    turn_id: u32,
    tx: &mpsc::Sender<VoxEvent>,
    finished: &mut bool,
) {
    if line.is_empty() {
        return;
    }

    if let Some(data) = line.strip_prefix("data:") {
        let data = data.trim();
        if data == "[DONE]" {
            *finished = true;
            return;
        }

        if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
            if let Some(choice) = chunk.choices.first() {
                if let Some(token) = &choice.delta.content {
                    if !token.is_empty() {
                        let _ = tx.send(VoxEvent::LlmToken {
                            turn_id,
                            token: token.clone(),
                        });
                    }
                }
            }
        }
    }
}

fn parse_heuristic_metadata(id: &str) -> (String, Option<String>, Option<String>) {
    let id_lower = id.to_lowercase();
    let quantization = if id_lower.contains("q4_k_m") || id_lower.contains("q4_k") || id_lower.contains("q4") {
        Some("Q4_K_M".to_string())
    } else if id_lower.contains("q6_k") || id_lower.contains("q6") {
        Some("Q6_K".to_string())
    } else if id_lower.contains("q2_k") || id_lower.contains("q2") {
        Some("Q2_K".to_string())
    } else if id_lower.contains("fp16") {
        Some("FP16".to_string())
    } else {
        None
    };

    let family = if id_lower.contains("gemma") {
        Some("Gemma".to_string())
    } else if id_lower.contains("llama") {
        Some("Llama".to_string())
    } else {
        None
    };

    let clean_name = id
        .replace(':', " ")
        .replace('_', " ")
        .replace('-', " ");

    (clean_name, quantization, family)
}
