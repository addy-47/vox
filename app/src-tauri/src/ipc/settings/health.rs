//! ============================================================================
//! src/ipc/settings/health.rs — Provider health checks, capability probes, and remote server setup
//! ============================================================================

use crate::core::state::AppState;
use tauri::{Emitter, Manager, State};

#[tauri::command]
pub async fn check_llm_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
) -> Result<bool, String> {
    use crate::core::settings::LlmProviderConfig;
    use crate::services::llm::{LlmProvider, OpenAiCompatProvider};
    use crate::utils::paths;

    let (config, llm_model) = {
        if let Some(prov) = provider {
            (prov, "".to_string())
        } else {
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
                            .join(crate::services::llm::MODEL_DIR_LLM)
                            .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
                    }
                } else {
                    models_dir
                        .join(crate::services::llm::MODEL_DIR_LLM)
                        .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
                }
            } else {
                models_dir
                    .join(crate::services::llm::MODEL_DIR_LLM)
                    .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
            };

            Ok(llm_path.exists())
        }
        LlmProviderConfig::OpenAiCompat {
            base_url,
            model,
            api_key,
            provider_name,
        } => {
            let provider = OpenAiCompatProvider::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let healthy = tokio::task::spawn_blocking(move || provider.health_check())
                .await
                .map_err(|e| e.to_string())?;
            Ok(healthy)
        }
    }
}

#[tauri::command]
pub async fn check_stt_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::SttProviderConfig>,
) -> Result<bool, String> {
    use crate::core::settings::SttProviderConfig;
    use crate::utils::paths;

    let config = if let Some(prov) = provider {
        prov
    } else {
        let settings = state.settings.read().map_err(|e| e.to_string())?;
        settings.stt.to_provider_config()
    };

    match config {
        SttProviderConfig::Embedded { model_type } => {
            let models_dir = paths::get().models.clone();
            let model_path = match model_type.as_str() {
                "nvidia_nemotron" => models_dir.join(crate::services::stt::MODEL_DIR_STT_NEMOTRON),
                _ => models_dir.join(crate::services::stt::MODEL_DIR_STT_QWEN),
            };
            Ok(model_path.exists())
        }
        SttProviderConfig::Cloud { .. } => {
            let healthy = tokio::task::spawn_blocking(move || {
                match crate::services::stt::providers::create_stt_provider(
                    &config,
                    &std::path::PathBuf::new(),
                ) {
                    Ok(provider) => provider.health_check(),
                    Err(e) => {
                        log::warn!("[Settings] Cloud STT provider health check failed: {}", e);
                        false
                    }
                }
            })
            .await
            .map_err(|e| e.to_string())?;
            Ok(healthy)
        }
    }
}

#[tauri::command]
pub async fn check_tts_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::TtsProviderConfig>,
) -> Result<bool, String> {
    use crate::core::settings::TtsProviderConfig;
    use crate::utils::paths;

    let config = if let Some(prov) = provider {
        prov
    } else {
        let settings = state.settings.read().map_err(|e| e.to_string())?;
        settings.tts.to_provider_config()
    };

    match config {
        TtsProviderConfig::Supertonic => {
            let models_dir = paths::get().models.clone();
            let model_path = models_dir.join(crate::services::tts::MODEL_DIR_TTS_SUPER);
            Ok(model_path.exists())
        }
        TtsProviderConfig::Chatterbox { .. } => {
            let models_dir = paths::get().models.clone();
            let model_path = models_dir.join(crate::services::tts::MODEL_DIR_TTS_CHATTERBOX);
            Ok(model_path.exists())
        }
        TtsProviderConfig::ChatterboxRemote { ref endpoint, .. } => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .map_err(|e| e.to_string())?;
            let health_url = format!("{}/health", endpoint.trim_end_matches('/'));
            match client.get(&health_url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        if let Ok(body) = resp.json::<serde_json::Value>().await {
                            if body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                                return Ok(true);
                            }
                        }
                    }
                    Ok(false)
                }
                Err(_) => Ok(false),
            }
        }
        TtsProviderConfig::EdgeTts { .. } => Ok(true),
    }
}

#[tauri::command]
pub async fn list_llm_models(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
) -> Result<Vec<crate::core::settings::LlmModelInfo>, String> {
    use crate::core::settings::LlmProviderConfig;
    use crate::services::llm::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider};
    use crate::utils::paths;

    let config = {
        if let Some(prov) = provider {
            prov
        } else {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            settings.llm.to_provider_config()
        }
    };

    match config {
        LlmProviderConfig::Embedded => {
            let llm_dir = paths::get()
                .models
                .join(crate::services::llm::MODEL_DIR_LLM);
            let models =
                EmbeddedProvider::list_models_in_dir(&llm_dir).map_err(|e| e.to_string())?;
            Ok(models)
        }
        LlmProviderConfig::OpenAiCompat {
            base_url,
            model,
            api_key,
            provider_name,
        } => {
            let provider = OpenAiCompatProvider::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let models = tokio::task::spawn_blocking(move || provider.list_models())
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(models)
        }
    }
}

#[tauri::command]
pub async fn get_cached_capabilities(
) -> Result<std::collections::HashMap<String, crate::core::settings::ModelCapabilities>, String> {
    use crate::utils::paths;
    let cache_file = paths::get().cache.join("model_capabilities.json");
    if cache_file.exists() {
        if let Ok(content) = tokio::fs::read_to_string(&cache_file).await {
            if let Ok(map) = serde_json::from_str(&content) {
                return Ok(map);
            }
        }
    }
    Ok(std::collections::HashMap::new())
}

#[tauri::command]
pub async fn probe_model_capabilities(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
    model_id: Option<String>,
) -> Result<crate::core::settings::ModelCapabilities, String> {
    use crate::services::llm::CapabilityProbeEngine;
    use crate::utils::paths;

    let (config, active_model) = {
        let settings = state.settings.read().map_err(|e| e.to_string())?;
        (
            provider.unwrap_or_else(|| settings.llm.to_provider_config()),
            settings.llm.active_model().to_string(),
        )
    };

    let target = model_id.or(Some(active_model));

    let caps = CapabilityProbeEngine::probe_capabilities(&config, target.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    // Persist to cache file
    let cache_dir = paths::get().cache.clone();
    let cache_file = cache_dir.join("model_capabilities.json");
    let caps_clone = caps.clone();

    tokio::spawn(async move {
        let _ = tokio::fs::create_dir_all(&cache_dir).await;
        let mut map: std::collections::HashMap<String, crate::core::settings::ModelCapabilities> =
            if cache_file.exists() {
                tokio::fs::read_to_string(&cache_file)
                    .await
                    .ok()
                    .and_then(|c| serde_json::from_str(&c).ok())
                    .unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            };

        let key = format!("{}:{}", caps_clone.provider_kind, caps_clone.model_id);
        map.insert(key, caps_clone);

        if let Ok(json) = serde_json::to_string_pretty(&map) {
            let tmp = cache_file.with_extension("tmp");
            if tokio::fs::write(&tmp, json).await.is_ok() {
                let _ = tokio::fs::rename(tmp, cache_file).await;
            }
        }
    });

    Ok(caps)
}

#[tauri::command]
pub async fn validate_llm_token_cap(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
    model_id: Option<String>,
    target_cap: u32,
) -> Result<Option<u32>, String> {
    use crate::services::llm::CapabilityProbeEngine;

    let (config, active_model) = {
        let settings = state.settings.read().map_err(|e| e.to_string())?;
        (
            provider.unwrap_or_else(|| settings.llm.to_provider_config()),
            settings.llm.active_model().to_string(),
        )
    };

    let target = model_id.or(Some(active_model));

    CapabilityProbeEngine::validate_token_cap(&config, target.as_deref(), target_cap).await
}

fn resolve_setup_script(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?
        .join("resources")
        .join("setup_server.sh");

    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("setup_server.sh");

    if resource_path.exists() {
        Ok(resource_path)
    } else if dev_path.exists() {
        log::info!("[SetupRemote] Using dev path: {:?}", dev_path);
        Ok(dev_path)
    } else {
        Err(format!(
            "Remote setup script not found at {:?} or {:?}",
            resource_path, dev_path
        ))
    }
}

fn parse_setup_progress(line: &str) -> (&'static str, u32) {
    if line.contains("Phase 1") {
        ("setup", 10)
    } else if line.contains("Phase 2") {
        ("sync", 25)
    } else if line.contains("Phase 3") {
        ("models", 40)
    } else if line.contains("Phase 4") {
        ("build", 75)
    } else if line.contains("Phase 5") {
        ("launch", 85)
    } else if line.contains("Phase 6") {
        ("health", 90)
    } else if line.contains("Phase 7") {
        ("smoke", 95)
    } else if line.contains("Smoke test passed") {
        ("complete", 100)
    } else {
        ("setup", 0)
    }
}

async fn run_remote_ssh_task(
    app: tauri::AppHandle,
    script_path: std::path::PathBuf,
    connection_string: String,
    ssh_port: Option<u16>,
    identity_key_path: Option<String>,
    remote_path: String,
    server_port: u16,
) {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;

    let mut cmd = Command::new("ssh");
    if let Some(ref key_path) = identity_key_path {
        if !key_path.trim().is_empty() {
            cmd.arg("-i").arg(key_path);
        }
    }
    if let Some(port_val) = ssh_port {
        cmd.arg("-p").arg(port_val.to_string());
    }
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg(&connection_string)
        .arg("bash")
        .arg("-s")
        .arg("--")
        .arg(&remote_path)
        .arg(server_port.to_string());
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to spawn ssh command: {}", e);
            log::error!("{}", err_msg);
            let _ = app.emit(
                "remote_setup_status",
                serde_json::json!({ "step": "failed", "progress": 0, "log_line": err_msg.clone(), "error": err_msg }),
            );
            return;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        if let Ok(script_content) = tokio::fs::read_to_string(&script_path).await {
            if let Err(e) = stdin.write_all(script_content.as_bytes()).await {
                log::warn!("Failed to write script to ssh stdin: {}", e);
            }
            let _ = stdin.flush().await;
        }
    }

    let Some(stdout) = child.stdout.take() else {
        let err_msg = "Failed to capture SSH stdout stream".to_string();
        log::error!("[SetupRemote] {}", err_msg);
        let _ = app.emit(
            "remote_setup_status",
            serde_json::json!({ "step": "failed", "progress": 0, "log_line": err_msg.clone(), "error": err_msg }),
        );
        return;
    };
    let Some(stderr) = child.stderr.take() else {
        let err_msg = "Failed to capture SSH stderr stream".to_string();
        log::error!("[SetupRemote] {}", err_msg);
        let _ = app.emit(
            "remote_setup_status",
            serde_json::json!({ "step": "failed", "progress": 0, "log_line": err_msg.clone(), "error": err_msg }),
        );
        return;
    };
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let app_out = app.clone();
    let stdout_loop = async move {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            log::info!("[RemoteSetup stdout] {}", line);
            let (step, progress) = parse_setup_progress(&line);
            let _ = app_out.emit(
                "remote_setup_status",
                serde_json::json!({ "step": step, "progress": progress, "log_line": line }),
            );
        }
    };

    let app_err = app.clone();
    let stderr_loop = async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            log::warn!("[RemoteSetup stderr] {}", line);
            let _ = app_err.emit(
                "remote_setup_status",
                serde_json::json!({ "step": "log", "progress": 0, "log_line": line }),
            );
        }
    };

    tokio::join!(stdout_loop, stderr_loop);

    match child.wait().await {
        Ok(status) if status.success() => {
            log::info!("[SetupRemote] Setup completed successfully.");
            let _ = app.emit(
                "remote_setup_status",
                serde_json::json!({ "step": "complete", "progress": 100, "log_line": "Remote setup completed successfully!" }),
            );
        }
        Ok(status) => {
            let err_msg = format!("SSH command exited with code: {:?}", status.code());
            log::error!("[SetupRemote] {}", err_msg);
            let _ = app.emit(
                "remote_setup_status",
                serde_json::json!({ "step": "failed", "progress": 0, "log_line": err_msg.clone(), "error": err_msg }),
            );
        }
        Err(e) => {
            let err_msg = format!("Failed to wait for SSH child: {}", e);
            log::error!("[SetupRemote] {}", err_msg);
            let _ = app.emit(
                "remote_setup_status",
                serde_json::json!({ "step": "failed", "progress": 0, "log_line": err_msg.clone(), "error": err_msg }),
            );
        }
    }
}

/// Execute remote server bootstrap script over SSH and stream progress events.
#[tauri::command]
pub async fn setup_remote_server(
    app: tauri::AppHandle,
    connection_string: String,
    ssh_port: Option<u16>,
    identity_key_path: Option<String>,
    remote_path: String,
    server_port: u16,
) -> Result<(), String> {
    log::info!(
        "[SetupRemote] Triggering remote server setup. connection_string={}, remote_path={}, server_port={}",
        connection_string,
        remote_path,
        server_port
    );

    let script_path = resolve_setup_script(&app)?;

    tauri::async_runtime::spawn(run_remote_ssh_task(
        app,
        script_path,
        connection_string,
        ssh_port,
        identity_key_path,
        remote_path,
        server_port,
    ));

    Ok(())
}
