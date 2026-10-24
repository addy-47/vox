use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::{
    core::{
        compute::{self, Execution, Workload},
        defaults::DEFAULT_TTS_SPEED,
        state::AppState,
    },
    services::{
        llm::{
            catalog::discover_server_dialect, ConnectionConfig, LlmProvider, LlmRuntimeTarget,
            RemoteTransport, QWEN_MODEL_DIR, QWEN_MODEL_FILE,
        },
        stt::{create_stt_provider, SttProviderConfig, NEMOTRON_MODEL_DIR, QWEN_ASR_MODEL_DIR},
        tts::{
            TtsProviderConfig, AZURE_TTS_VOICES_PATH, CHATTERBOX_MODEL_DIR, KOKORO_MODEL_DIR,
            SUPERTONIC_MODEL_DIR, ZIPVOICE_MODEL_DIR,
        },
    },
    utils::paths,
};

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum ProviderConfigPayload {
    Llm(LlmRuntimeTarget),
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
            let (healthy, reason) = check_tts_health_with_reason(state, provider).await?;
            Ok(ProviderHealthCheckResult {
                healthy,
                dialect: None,
                reason,
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
                settings
                    .llm
                    .to_provider_config(Some(&settings.providers.llm)),
                settings.llm.active_model().to_string(),
            )
        }
    };

    match config {
        LlmRuntimeTarget::Embedded => {
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
        LlmRuntimeTarget::Unresolved { provider_id } => Ok(ProviderHealthCheckResult {
            healthy: false,
            dialect: None,
            reason: Some(format!(
                "cloud provider '{}' is not in the declarative manifest (RULE-PROV-01)",
                provider_id
            )),
        }),
        LlmRuntimeTarget::Server {
            base_url,
            model,
            api_key,
            provider_name,
            ..
        }
        | LlmRuntimeTarget::Cloud {
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
            let discovered = discover_server_dialect(&client, &base_url, &conn_cfg.auth).await;

            let provider = RemoteTransport::new(conn_cfg.clone());
            let health_error = provider.health_check().await.err();

            let reason = health_error.map(|err| {
                let msg = err.to_string();
                let lower = msg.to_lowercase();
                if lower.contains("connection refused") {
                    format!("unreachable: {}", msg)
                } else if lower.contains("timed out") || lower.contains("timeout") {
                    format!("timed out: {}", msg)
                } else {
                    msg
                }
            });

            Ok(ProviderHealthCheckResult {
                healthy: reason.is_none(),
                dialect: Some(discovered.display_name().to_string()),
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
        SttProviderConfig::Cloud { .. } => {
            // A cloud provider runs off this machine, so it owns no local
            // inference threads.
            let num_threads = compute::resolve_threads(Workload::Stt, Execution::Remote);
            tokio::task::spawn_blocking(move || {
                match create_stt_provider(&config, &PathBuf::new(), num_threads) {
                    Ok(provider) => provider.health_check(),
                    Err(e) => {
                        log::warn!("[Settings] Cloud STT provider health check failed: {}", e);
                        false
                    }
                }
            })
            .await
            .map_err(|e| e.to_string())
        }
    }
}

/// Returns true if Google Application Default Credentials (ADC) file exists on the host.
pub fn has_google_adc() -> bool {
    if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        if std::path::Path::new(&path).exists() {
            return true;
        }
    }
    if let Some(config_dir) = dirs::config_dir() {
        if config_dir
            .join("gcloud/application_default_credentials.json")
            .exists()
        {
            return true;
        }
    }
    if let Some(home) = dirs::home_dir() {
        if home
            .join(".config/gcloud/application_default_credentials.json")
            .exists()
        {
            return true;
        }
    }
    false
}

pub async fn check_tts_health_with_reason(
    state: &Arc<AppState>,
    provider: Option<ProviderConfigPayload>,
) -> Result<(bool, Option<String>), String> {
    let config = match provider {
        Some(ProviderConfigPayload::Tts(prov)) => prov,
        _ => {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            settings
                .tts
                .to_provider_config(Some(&settings.providers.tts), DEFAULT_TTS_SPEED)
        }
    };

    match config {
        TtsProviderConfig::Supertonic => {
            let models_dir = paths::get().models.clone();
            let exists = models_dir.join(SUPERTONIC_MODEL_DIR).exists();
            let reason = if exists {
                None
            } else {
                Some("Supertonic model weights not found on disk.".to_string())
            };
            Ok((exists, reason))
        }
        TtsProviderConfig::Kokoro => {
            let models_dir = paths::get().models.clone();
            let exists = models_dir.join(KOKORO_MODEL_DIR).exists();
            let reason = if exists {
                None
            } else {
                Some("Kokoro model weights not found on disk.".to_string())
            };
            Ok((exists, reason))
        }
        TtsProviderConfig::Chatterbox { .. } => {
            let models_dir = paths::get().models.clone();
            let exists = models_dir.join(CHATTERBOX_MODEL_DIR).exists();
            let reason = if exists {
                None
            } else {
                Some("Chatterbox model weights not found on disk.".to_string())
            };
            Ok((exists, reason))
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
                        let ok = body.get("status").and_then(|s| s.as_str()) == Some("ok");
                        let reason = if ok {
                            None
                        } else {
                            Some("Remote server responded with non-ok health status.".to_string())
                        };
                        Ok((ok, reason))
                    } else {
                        Ok((
                            false,
                            Some(
                                "Remote server returned invalid JSON health response.".to_string(),
                            ),
                        ))
                    }
                }
                Ok(resp) => Ok((
                    false,
                    Some(format!("Remote server returned HTTP {}.", resp.status())),
                )),
                Err(e) => Ok((
                    false,
                    Some(format!("Cannot reach remote server at {}: {}", endpoint, e)),
                )),
            }
        }
        TtsProviderConfig::EdgeTts { .. } => {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|e| e.to_string())?;
            match client.head("https://speech.platform.bing.com").send().await {
                Ok(resp) if resp.status().is_success() || resp.status().as_u16() < 500 => {
                    Ok((true, None))
                }
                Ok(resp) => Ok((
                    false,
                    Some(format!("Edge TTS service returned HTTP {}.", resp.status())),
                )),
                Err(e) => Ok((false, Some(format!("Edge TTS service unreachable: {}", e)))),
            }
        }
        TtsProviderConfig::Zipvoice { .. } => {
            let models_dir = paths::get().models.clone();
            let exists = models_dir.join(ZIPVOICE_MODEL_DIR).exists();
            let reason = if exists {
                None
            } else {
                Some("Zipvoice model weights not found on disk.".to_string())
            };
            Ok((exists, reason))
        }
        TtsProviderConfig::AzureTts {
            region_or_endpoint,
            key,
            ..
        } => {
            if key.trim().is_empty() {
                return Ok((false, Some("Azure AI Speech key is required.".to_string())));
            }
            let base = crate::services::tts::auth::normalize_azure_endpoint(&region_or_endpoint);
            let url = format!("{}{}", base.trim_end_matches('/'), AZURE_TTS_VOICES_PATH);
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|e| e.to_string())?;
            match client
                .get(&url)
                .header("Ocp-Apim-Subscription-Key", key.trim())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => Ok((true, None)),
                Ok(resp) => Ok((
                    false,
                    Some(format!(
                        "Azure AI Speech auth failed: HTTP {}.",
                        resp.status()
                    )),
                )),
                Err(e) => Ok((
                    false,
                    Some(format!("Azure AI Speech service unreachable: {}", e)),
                )),
            }
        }
        TtsProviderConfig::GoogleTts { .. } => {
            if !has_google_adc() {
                return Ok((
                    false,
                    Some("Google Application Default Credentials (ADC) not found. Run 'gcloud auth application-default login' in terminal.".to_string()),
                ));
            }
            match crate::services::sidecar::global_sidecar_supervisor()
                .ensure_ready(crate::services::sidecar::SidecarKind::TtsCloud, &[])
                .await
            {
                Ok(base_url) => {
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(2))
                        .build()
                        .map_err(|e| e.to_string())?;
                    match client.get(format!("{}/health", base_url)).send().await {
                        Ok(resp) if resp.status().is_success() => Ok((true, None)),
                        Ok(resp) => Ok((
                            false,
                            Some(format!(
                                "Google Cloud TTS sidecar health check failed: HTTP {}",
                                resp.status()
                            )),
                        )),
                        Err(e) => Ok((
                            false,
                            Some(format!("Google Cloud TTS sidecar unreachable: {}", e)),
                        )),
                    }
                }
                Err(e) => Ok((
                    false,
                    Some(format!("Failed to start Google Cloud TTS sidecar: {}", e)),
                )),
            }
        }
    }
}

pub async fn check_tts_health(
    state: &Arc<AppState>,
    provider: Option<ProviderConfigPayload>,
) -> Result<bool, String> {
    check_tts_health_with_reason(state, provider)
        .await
        .map(|(healthy, _)| healthy)
}

/// Structured outcome of probing an arbitrary provider target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProbeProviderTargetResult {
    pub healthy: bool,
    pub dialect: Option<String>,
    pub reason: Option<String>,
    #[serde(default)]
    pub models: Vec<crate::services::llm::catalog::LlmModelInfo>,
}

/// Dedicated probe and model discovery for an arbitrary or unsaved provider target.
pub async fn probe_target(
    state: &Arc<AppState>,
    kind: &str,
    provider_id: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
) -> Result<ProbeProviderTargetResult, String> {
    match kind.to_lowercase().as_str() {
        "tts" => {
            let endpoint = base_url.unwrap_or_default();
            let trimmed = endpoint.trim().trim_end_matches('/');
            if trimmed.is_empty() {
                return Ok(ProbeProviderTargetResult {
                    healthy: false,
                    dialect: Some("Chatterbox Remote".to_string()),
                    reason: Some("No base URL specified".to_string()),
                    models: Vec::new(),
                });
            }
            let client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    return Ok(ProbeProviderTargetResult {
                        healthy: false,
                        dialect: Some("Chatterbox Remote".to_string()),
                        reason: Some(e.to_string()),
                        models: Vec::new(),
                    });
                }
            };
            let health_url = format!("{}/health", trimmed);
            match client.get(&health_url).send().await {
                Ok(resp) => {
                    let healthy = resp.status().is_success();
                    let reason = if healthy {
                        None
                    } else {
                        Some(format!("HTTP {}", resp.status()))
                    };
                    Ok(ProbeProviderTargetResult {
                        healthy,
                        dialect: Some("Chatterbox Remote".to_string()),
                        reason,
                        models: Vec::new(),
                    })
                }
                Err(err) => Ok(ProbeProviderTargetResult {
                    healthy: false,
                    dialect: Some("Chatterbox Remote".to_string()),
                    reason: Some(err.to_string()),
                    models: Vec::new(),
                }),
            }
        }
        "stt" => {
            let healthy = check_stt_health(state, None).await.unwrap_or(false);
            Ok(ProbeProviderTargetResult {
                healthy,
                dialect: None,
                reason: None,
                models: Vec::new(),
            })
        }
        _ => {
            // "llm", "server", "cloud"
            let trimmed_url = base_url
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());
            let (effective_url, effective_provider_name) = if let Some(url) = trimmed_url {
                (url.to_string(), provider_id.clone())
            } else if let Some(pid) = provider_id.as_deref().filter(|s| !s.trim().is_empty()) {
                if let Some(preset) = crate::services::llm::catalog::lookup_preset(pid) {
                    (preset.base_url.to_string(), Some(pid.to_string()))
                } else if pid == "gemini_live" {
                    (
                        "https://generativelanguage.googleapis.com".to_string(),
                        Some("gemini_live".to_string()),
                    )
                } else if pid == "deepgram_voice_agent" {
                    (
                        "https://agent.deepgram.com".to_string(),
                        Some("deepgram_voice_agent".to_string()),
                    )
                } else {
                    return Ok(ProbeProviderTargetResult {
                        healthy: false,
                        dialect: None,
                        reason: Some(format!("Unknown provider preset '{}'", pid)),
                        models: Vec::new(),
                    });
                }
            } else {
                return Ok(ProbeProviderTargetResult {
                    healthy: false,
                    dialect: None,
                    reason: Some("Neither baseUrl nor providerId specified".to_string()),
                    models: Vec::new(),
                });
            };

            let effective_api_key = api_key.filter(|k| !k.trim().is_empty()).or_else(|| {
                if let Some(ref pid) = provider_id {
                    let guard = state.settings.read().ok()?;
                    guard
                        .providers
                        .realtime
                        .api_keys
                        .get(pid)
                        .cloned()
                        .or_else(|| guard.providers.llm.cloud_keys.get(pid).cloned())
                        .or_else(|| {
                            if pid == "gemini_live" {
                                guard.providers.llm.cloud_keys.get("gemini").cloned()
                            } else {
                                None
                            }
                        })
                } else {
                    None
                }
            });

            let effective_model = model.unwrap_or_default();

            if effective_provider_name.as_deref() == Some("deepgram_voice_agent")
                || effective_provider_name.as_deref() == Some("gemini_live")
            {
                let is_gemini = effective_provider_name.as_deref() == Some("gemini_live");
                let target = crate::services::llm::LlmRuntimeTarget::Cloud {
                    base_url: effective_url,
                    model: effective_model,
                    api_key: effective_api_key,
                    provider_name: effective_provider_name,
                    protocol: Some("openai".to_string()),
                };
                let models = crate::services::llm::catalog::list_models(state, Some(target))
                    .await
                    .unwrap_or_default();
                let healthy = !models.is_empty();
                return Ok(ProbeProviderTargetResult {
                    healthy,
                    dialect: Some(if is_gemini {
                        "Gemini Live API".to_string()
                    } else {
                        "Deepgram Voice Agent API".to_string()
                    }),
                    reason: if healthy {
                        None
                    } else {
                        Some("No models returned or API key invalid".to_string())
                    },
                    models,
                });
            }

            let conn_cfg = ConnectionConfig::new(
                &effective_url,
                &effective_model,
                effective_api_key.as_deref(),
                effective_provider_name.as_deref(),
            );

            let client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("[Probe] Dialect probe HTTP client failed to build: {}", e);
                    reqwest::Client::new()
                }
            };

            let dialect = discover_server_dialect(&client, &effective_url, &conn_cfg.auth).await;
            let transport = RemoteTransport::new(conn_cfg);
            let health_res = transport.health_check().await;

            let models = match transport.list_models().await {
                Ok(mut m) => {
                    let kind_str = if kind.eq_ignore_ascii_case("cloud") {
                        crate::services::llm::catalog::CAP_KIND_CLOUD
                    } else {
                        crate::services::llm::catalog::CAP_KIND_SERVER
                    };
                    for item in &mut m {
                        item.provider_kind = kind_str.to_string();
                    }
                    m
                }
                Err(err) => {
                    log::info!("[Probe] list_models returned non-fatal error: {}", err);
                    Vec::new()
                }
            };

            let reason = match health_res {
                Ok(_) => None,
                Err(ref err) => {
                    let msg = err.to_string();
                    let lower = msg.to_lowercase();
                    if lower.contains("connection refused") {
                        Some(format!("unreachable: {}", msg))
                    } else if lower.contains("timed out") || lower.contains("timeout") {
                        Some(format!("timed out: {}", msg))
                    } else {
                        Some(msg)
                    }
                }
            };

            let healthy = reason.is_none() || !models.is_empty();
            let reason = if healthy { None } else { reason };

            Ok(ProbeProviderTargetResult {
                healthy,
                dialect: Some(dialect.display_name().to_string()),
                reason,
                models,
            })
        }
    }
}
