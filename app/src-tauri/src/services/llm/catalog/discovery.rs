use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::services::llm::transport::{inject_auth_headers, AuthScheme};

/// Discovered dialect of a self-hosted or remote LLM server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerDialect {
    Ollama { version: Option<String> },
    Vllm { version: Option<String> },
    LlamaCpp,
    LmStudio,
    GenericOpenAiCompat,
}

impl ServerDialect {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Ollama { .. } => "Ollama",
            Self::Vllm { .. } => "vLLM",
            Self::LlamaCpp => "llama.cpp",
            Self::LmStudio => "LM Studio",
            Self::GenericOpenAiCompat => "OpenAI-Compatible",
        }
    }

    pub fn preset_id(&self) -> Option<&'static str> {
        match self {
            Self::Ollama { .. } => Some("ollama_openai_compat"),
            Self::Vllm { .. } | Self::LlamaCpp => Some("vllm"),
            Self::LmStudio => Some("lm_studio"),
            Self::GenericOpenAiCompat => None,
        }
    }
}

/// Dynamically probe a remote endpoint using standard protocols to discover its server dialect.
pub async fn discover_server_dialect(
    client: &Client,
    base_url: &str,
    auth: &AuthScheme,
) -> ServerDialect {
    let base = base_url.trim_end_matches('/');
    let root = base.strip_suffix("/v1").unwrap_or(base);

    // Fast concurrent probe for standard dialect endpoints (1s timeout each)
    let (ollama_res, vllm_res, llama_res) = tokio::join!(
        probe_ollama_endpoint(client, root, auth),
        probe_vllm_endpoint(client, root, auth),
        probe_llama_endpoint(client, root, auth),
    );

    if let Some(dialect) = ollama_res {
        return dialect;
    }
    if let Some(dialect) = vllm_res {
        return dialect;
    }
    if let Some(dialect) = llama_res {
        return dialect;
    }

    // Secondary check: inspect models endpoint `owned_by` field
    if let Some(dialect) = probe_models_endpoint(client, base, auth).await {
        return dialect;
    }

    ServerDialect::GenericOpenAiCompat
}

async fn probe_ollama_endpoint(client: &Client, root: &str, auth: &AuthScheme) -> Option<ServerDialect> {
    let url = format!("{}/api/version", root);
    let mut builder = client.get(&url).timeout(Duration::from_secs(1));
    builder = inject_auth_headers(builder, auth);

    let resp = builder.send().await.ok()?;
    if resp.status().is_success() {
        let version = resp
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v.get("version").and_then(|s| s.as_str()).map(ToString::to_string));
        Some(ServerDialect::Ollama { version })
    } else {
        None
    }
}

async fn probe_vllm_endpoint(client: &Client, root: &str, auth: &AuthScheme) -> Option<ServerDialect> {
    let url = format!("{}/version", root);
    let mut builder = client.get(&url).timeout(Duration::from_secs(1));
    builder = inject_auth_headers(builder, auth);

    let resp = builder.send().await.ok()?;
    if resp.status().is_success() {
        let version = resp
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v.get("version").and_then(|s| s.as_str()).map(ToString::to_string));
        Some(ServerDialect::Vllm { version })
    } else {
        None
    }
}

async fn probe_llama_endpoint(client: &Client, root: &str, auth: &AuthScheme) -> Option<ServerDialect> {
    let url = format!("{}/props", root);
    let mut builder = client.get(&url).timeout(Duration::from_secs(1));
    builder = inject_auth_headers(builder, auth);

    let resp = builder.send().await.ok()?;
    if resp.status().is_success() {
        Some(ServerDialect::LlamaCpp)
    } else {
        None
    }
}

async fn probe_models_endpoint(client: &Client, base: &str, auth: &AuthScheme) -> Option<ServerDialect> {
    let url = if base.ends_with("/v1") {
        format!("{}/models", base)
    } else {
        format!("{}/v1/models", base)
    };

    let mut builder = client.get(&url).timeout(Duration::from_secs(2));
    builder = inject_auth_headers(builder, auth);

    let resp = builder.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let body = resp.json::<serde_json::Value>().await.ok()?;
    let data = body.get("data").and_then(|d| d.as_array())?;

    for item in data {
        if let Some(owned_by) = item.get("owned_by").and_then(|o| o.as_str()) {
            let lower = owned_by.to_lowercase();
            if lower == "library" || lower.contains("ollama") {
                return Some(ServerDialect::Ollama { version: None });
            }
            if lower.contains("vllm") {
                return Some(ServerDialect::Vllm { version: None });
            }
            if lower.contains("lmstudio") || lower.contains("lm-studio") {
                return Some(ServerDialect::LmStudio);
            }
        }
    }

    None
}
