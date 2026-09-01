use crate::core::settings::{LlmProviderConfig, SttProviderConfig, TtsProviderConfig};
use crate::core::state::AppState;
use crate::services::llm::{LlmProvider, RemoteTransport};
use crate::utils::paths;
use std::sync::Arc;

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum ProviderConfigPayload {
    Llm(LlmProviderConfig),
    Stt(SttProviderConfig),
    Tts(TtsProviderConfig),
}

/// Verify health status across LLM, STT, and TTS providers.
pub async fn check_health(
    state: &Arc<AppState>,
    kind: &str,
    provider: Option<ProviderConfigPayload>,
) -> Result<bool, String> {
    match kind.to_lowercase().as_str() {
        "llm" => check_llm_health(state, provider).await,
        "stt" => check_stt_health(state, provider).await,
        "tts" => check_tts_health(state, provider).await,
        _ => Err(format!("Unknown provider health check kind: {}", kind)),
    }
}

pub async fn check_llm_health(
    state: &Arc<AppState>,
    provider: Option<ProviderConfigPayload>,
) -> Result<bool, String> {
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
                        models_dir
                            .join(crate::services::llm::QWEN_MODEL_DIR)
                            .join(crate::services::llm::QWEN_MODEL_FILE)
                    }
                } else {
                    models_dir
                        .join(crate::services::llm::QWEN_MODEL_DIR)
                        .join(crate::services::llm::QWEN_MODEL_FILE)
                }
            } else {
                models_dir
                    .join(crate::services::llm::QWEN_MODEL_DIR)
                    .join(crate::services::llm::QWEN_MODEL_FILE)
            };

            Ok(llm_path.exists())
        }
        LlmProviderConfig::OpenAiCompat {
            base_url,
            model,
            api_key,
            provider_name,
        } => {
            let conn_cfg = crate::services::llm::ConnectionConfig::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let provider = RemoteTransport::new(conn_cfg);
            Ok(provider.health_check().await.is_ok())
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
                "nvidia_nemotron" => models_dir.join(crate::services::stt::NEMOTRON_MODEL_DIR),
                _ => models_dir.join(crate::services::stt::QWEN_ASR_MODEL_DIR),
            };
            Ok(model_path.exists())
        }
        SttProviderConfig::Cloud { .. } => tokio::task::spawn_blocking(move || {
            match crate::services::stt::create_stt_provider(&config, &std::path::PathBuf::new()) {
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
            Ok(models_dir
                .join(crate::services::tts::SUPERTONIC_MODEL_DIR)
                .exists())
        }
        TtsProviderConfig::Kokoro => {
            let models_dir = paths::get().models.clone();
            Ok(models_dir
                .join(crate::services::tts::KOKORO_MODEL_DIR)
                .exists())
        }
        TtsProviderConfig::Chatterbox { .. } => {
            let models_dir = paths::get().models.clone();
            Ok(models_dir
                .join(crate::services::tts::CHATTERBOX_MODEL_DIR)
                .exists())
        }
        TtsProviderConfig::ChatterboxRemote { ref endpoint, .. } => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
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
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .map_err(|e| e.to_string())?;
            match client.head("https://speech.platform.bing.com").send().await {
                Ok(resp) => Ok(resp.status().is_success() || resp.status().as_u16() < 500),
                _ => Ok(false),
            }
        }
    }
}
