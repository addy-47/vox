use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::{
    core::{defaults::DEFAULT_STT_THREADS, state::AppState},
    services::{
        llm::{
            ConnectionConfig, LlmProvider, LlmProviderConfig, RemoteTransport, QWEN_MODEL_DIR,
            QWEN_MODEL_FILE,
        },
        stt::{create_stt_provider, SttProviderConfig, NEMOTRON_MODEL_DIR, QWEN_ASR_MODEL_DIR},
        tts::{
            TtsProviderConfig, CHATTERBOX_MODEL_DIR, KOKORO_MODEL_DIR, SUPERTONIC_MODEL_DIR,
            ZIPVOICE_MODEL_DIR,
        },
    },
    utils::paths,
};

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum ProviderConfigPayload {
    Llm(LlmProviderConfig),
    Stt(SttProviderConfig),
    Tts(TtsProviderConfig),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderHealthCheckResult {
    pub healthy: bool,
    pub dialect: Option<String>,
    pub reason: Option<String>,
}

/// Verify health status across LLM, STT, and TTS providers.
pub async fn check_health(
    state: &Arc<AppState>,
    kind: &str,
    provider: Option<ProviderConfigPayload>,
) -> Result<ProviderHealthCheckResult, String> {
    match kind.to_lowercase().as_str() {
        "llm" => check_llm_health(state, provider).await,
        "stt" => {
            let healthy = check_stt_health(state, provider).await?;
            Ok(ProviderHealthCheckResult {
                healthy,
                dialect: None,
                reason: None,
            })
        }
        "tts" => {
            let healthy = check_tts_health(state, provider).await?;
            Ok(ProviderHealthCheckResult {
                healthy,
                dialect: None,
                reason: None,
            })
        }
        _ => Err(format!("Unknown provider health check kind: {}", kind)),
    }
}

pub async fn check_llm_health(
    state: &Arc<AppState>,
    provider: Option<ProviderConfigPayload>,
) -> Result<ProviderHealthCheckResult, String> {
    let (config, llm_model) = match provider {
        Some(ProviderConfigPayload::Llm(prov)) => (prov, "".to_string()),
        _ => {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            (
                settings.llm.to_provider_config(),
                settings.llm.active_model().to_string(),
            )
        }
    };

    match config {
        LlmProviderConfig::Embedded => {
            let models_dir = paths::get().models.clone();
            let manifest_lock = state.manifest.read().await;

            let llm_path = if let Some(ref manifest) = *manifest_lock {
                if let Some(group) = manifest.model_groups.iter().find(|g| g.id == llm_model) {
                    if let Some(file) = group.files.first() {
                        models_dir.join(&file.path)
                    } else {
                        models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE)
                    }
                } else {
                    models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE)
                }
            } else {
                models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE)
            };

            let healthy = llm_path.exists();
            Ok(ProviderHealthCheckResult {
                healthy,
                dialect: if healthy {
                    Some("Embedded".to_string())
                } else {
                    None
                },
                reason: if healthy {
                    None
                } else {
                    Some(format!("Model file not found: {:?}", llm_path))
                },
            })
        }
        LlmProviderConfig::Server {
            base_url,
            model,
            api_key,
            provider_name,
            ..
        }
        | LlmProviderConfig::Cloud {
            base_url,
            model,
            api_key,
            provider_name,
            ..
        } => {
            let conn_cfg = ConnectionConfig::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let provider = RemoteTransport::new(conn_cfg.clone());
            let health_error = provider.health_check().await.err();
            let healthy = health_error.is_none();
            let reason = health_error.map(|err| err.to_string());
            let dialect = if healthy {
                let client = match reqwest::Client::builder()
                    .timeout(Duration::from_secs(2))
                    .build()
                {
                    Ok(client) => client,
                    Err(err) => {
                        log::warn!(
                            "[Health] Dialect probe HTTP client failed to build: {}",
                            err
                        );
                        reqwest::Client::new()
                    }
                };
                let discovered = crate::services::llm::catalog::discover_server_dialect(
                    &client,
                    &base_url,
                    &conn_cfg.auth,
                )
                .await;
                Some(discovered.display_name().to_string())
            } else {
                None
            };
            Ok(ProviderHealthCheckResult {
                healthy,
                dialect,
                reason,
            })
        }
    }
}

pub async fn check_stt_health(
    state: &Arc<AppState>,
    provider: Option<ProviderConfigPayload>,
) -> Result<bool, String> {
    let config = match provider {
        Some(ProviderConfigPayload::Stt(prov)) => prov,
        _ => {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            settings.stt.to_provider_config()
        }
    };

    match config {
        SttProviderConfig::Embedded { model_type } => {
            let models_dir = paths::get().models.clone();
            let model_path = match model_type.as_str() {
                "nvidia_nemotron" => models_dir.join(NEMOTRON_MODEL_DIR),
                _ => models_dir.join(QWEN_ASR_MODEL_DIR),
            };
            Ok(model_path.exists())
        }
        SttProviderConfig::Cloud { .. } => tokio::task::spawn_blocking(move || {
            match create_stt_provider(&config, &PathBuf::new(), DEFAULT_STT_THREADS) {
                Ok(provider) => provider.health_check(),
                Err(e) => {
                    log::warn!("[Settings] Cloud STT provider health check failed: {}", e);
                    false
                }
            }
        })
        .await
        .map_err(|e| e.to_string()),
    }
}

pub async fn check_tts_health(
    state: &Arc<AppState>,
    provider: Option<ProviderConfigPayload>,
) -> Result<bool, String> {
    let config = match provider {
        Some(ProviderConfigPayload::Tts(prov)) => prov,
        _ => {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            settings.tts.to_provider_config()
        }
    };

    match config {
        TtsProviderConfig::Supertonic => {
            let models_dir = paths::get().models.clone();
            Ok(models_dir.join(SUPERTONIC_MODEL_DIR).exists())
        }
        TtsProviderConfig::Kokoro => {
            let models_dir = paths::get().models.clone();
            Ok(models_dir.join(KOKORO_MODEL_DIR).exists())
        }
        TtsProviderConfig::Chatterbox { .. } => {
            let models_dir = paths::get().models.clone();
            Ok(models_dir.join(CHATTERBOX_MODEL_DIR).exists())
        }
        TtsProviderConfig::ChatterboxRemote { ref endpoint, .. } => {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|e| e.to_string())?;
            let health_url = format!("{}/health", endpoint.trim_end_matches('/'));
            match client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        Ok(body.get("status").and_then(|s| s.as_str()) == Some("ok"))
                    } else {
                        Ok(false)
                    }
                }
                _ => Ok(false),
            }
        }
        TtsProviderConfig::EdgeTts { .. } => {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|e| e.to_string())?;
            match client.head("https://speech.platform.bing.com").send().await {
                Ok(resp) => Ok(resp.status().is_success() || resp.status().as_u16() < 500),
                _ => Ok(false),
            }
        }
        TtsProviderConfig::Zipvoice { .. } => {
            let models_dir = paths::get().models.clone();
            Ok(models_dir.join(ZIPVOICE_MODEL_DIR).exists())
        }
    }
}
