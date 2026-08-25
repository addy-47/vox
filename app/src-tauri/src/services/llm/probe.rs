use super::capabilities::{CapabilityObservation, CapabilityRegistry, CapabilitySource};
use crate::services::llm::providers::ProviderKind;
use crate::services::llm::types::Support;
use std::time::Duration;

/// Dedicated client for testing parameter compatibility against LLM endpoints.
pub struct ActiveProbeEngine {
    client: reqwest::Client,
    registry: CapabilityRegistry,
}

impl ActiveProbeEngine {
    /// Creates a new probe engine backed by the provided capability registry.
    pub fn new(registry: CapabilityRegistry) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(4))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client, registry }
    }

    /// Returns a reference to the underlying capability registry.
    pub fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }

    /// Run an isolated probe for `top_k` on an OpenAI-compatible endpoint.
    pub async fn probe_top_k(
        &self,
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
        kind: ProviderKind,
    ) -> CapabilityObservation {
        let key = format!("{:?}:{}:{}", kind, base_url, model);
        let url = if base_url.ends_with("/chat/completions") {
            base_url.to_string()
        } else if base_url.ends_with("/v1") || base_url.ends_with("/openai") {
            format!("{}/chat/completions", base_url)
        } else {
            format!("{}/v1/chat/completions", base_url)
        };

        let req_body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1,
            "top_k": 40,
            "stream": false
        });

        let mut builder = self.client.post(&url).json(&req_body);
        if let Some(key) = api_key {
            builder = builder.bearer_auth(key);
        }

        let obs = match builder.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if resp.status().is_success() {
                    CapabilityObservation {
                        support: Support::Supported,
                        source: CapabilitySource::ActiveProbe,
                        status_code: Some(status),
                        detail: Some("Active top_k probe succeeded".to_string()),
                    }
                } else {
                    let err_text = resp.text().await.unwrap_or_default();
                    if err_text.contains("top_k")
                        || err_text.contains("unknown field")
                        || status == 400
                    {
                        CapabilityObservation {
                            support: Support::Unsupported,
                            source: CapabilitySource::ActiveProbe,
                            status_code: Some(status),
                            detail: Some(format!("top_k rejected by server: {}", err_text)),
                        }
                    } else {
                        CapabilityObservation {
                            support: Support::Unknown,
                            source: CapabilitySource::ActiveProbe,
                            status_code: Some(status),
                            detail: Some(err_text),
                        }
                    }
                }
            }
            Err(e) => CapabilityObservation {
                support: Support::Unknown,
                source: CapabilitySource::ActiveProbe,
                status_code: None,
                detail: Some(e.to_string()),
            },
        };

        let obs_clone = obs.clone();
        self.registry.update_observation(&key, kind, move |caps| {
            caps.top_k = obs_clone;
        });

        obs
    }
}
