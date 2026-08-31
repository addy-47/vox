use crate::core::events::{emit_ipc, IpcEvent};
use crate::core::state::AppState;
use crate::setup::model_manager::{ModelSetupStatus, SetupStep};
use tauri::{Manager, State};

#[tauri::command]
pub async fn check_llm_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
) -> Result<bool, String> {
    use crate::core::settings::LlmProviderConfig;
    use crate::services::llm::{LlmProvider, RemoteTransport};
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
            let healthy = provider.health_check().await.is_ok();
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
                "nvidia_nemotron" => models_dir.join(crate::services::stt::NEMOTRON_MODEL_DIR),
                _ => models_dir.join(crate::services::stt::QWEN_ASR_MODEL_DIR),
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
            let model_path = models_dir.join(crate::services::tts::SUPERTONIC_MODEL_DIR);
            Ok(model_path.exists())
        }
        TtsProviderConfig::Chatterbox { .. } => {
            let models_dir = paths::get().models.clone();
            let model_path = models_dir.join(crate::services::tts::CHATTERBOX_MODEL_DIR);
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
        TtsProviderConfig::EdgeTts { .. } => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .map_err(|e| e.to_string())?;
            match client.head("https://speech.platform.bing.com").send().await {
                Ok(resp) => Ok(resp.status().is_success() || resp.status().as_u16() < 500),
                Err(_) => Ok(false),
            }
        }
    }
}

#[tauri::command]
pub async fn list_llm_models(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<crate::core::settings::LlmProviderConfig>,
) -> Result<Vec<crate::core::settings::LlmModelInfo>, String> {
    use crate::core::settings::LlmProviderConfig;
    use crate::services::llm::{EmbeddedProvider, LlmProvider, RemoteTransport};
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
                .join(crate::services::llm::QWEN_MODEL_DIR);
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
            let conn_cfg = crate::services::llm::ConnectionConfig::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let provider = RemoteTransport::new(conn_cfg);
            let models = provider.list_models().await.map_err(|e| e.to_string())?;
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
        if let Err(e) = tokio::fs::create_dir_all(&cache_dir).await {
            log::warn!(
                "[Settings::Health] Failed to create cache directory {:?}: {}",
                cache_dir,
                e
            );
        }
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
                if let Err(e) = tokio::fs::rename(&tmp, &cache_file).await {
                    log::warn!("[Settings::Health] Failed to rename temp cache file: {}", e);
                }
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

fn parse_setup_progress(line: &str) -> (SetupStep, f32) {
    if line.contains("Phase 1") {
        (SetupStep::Downloading, 10.0)
    } else if line.contains("Phase 2") {
        (SetupStep::Downloading, 25.0)
    } else if line.contains("Phase 3") {
        (SetupStep::Downloading, 40.0)
    } else if line.contains("Phase 4") {
        (SetupStep::Extracting, 75.0)
    } else if line.contains("Phase 5") {
        (SetupStep::Verifying, 85.0)
    } else if line.contains("Phase 6") {
        (SetupStep::Verifying, 90.0)
    } else if line.contains("Phase 7") {
        (SetupStep::Verifying, 95.0)
    } else if line.contains("Smoke test passed") {
        (SetupStep::Completed, 100.0)
    } else {
        (SetupStep::Downloading, 0.0)
    }
}

static REMOTE_SETUP_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

struct RemoteSetupGuard;

impl Drop for RemoteSetupGuard {
    fn drop(&mut self) {
        REMOTE_SETUP_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
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
    let _guard = RemoteSetupGuard;
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
            let _ = emit_ipc(
                &app,
                IpcEvent::ModelProgress(ModelSetupStatus {
                    model_id: "chatterbox_remote_server".to_string(),
                    step: SetupStep::Failed,
                    progress: 0.0,
                    bytes_downloaded: 0,
                    total_bytes: 100,
                    error: Some(err_msg),
                }),
            );
            return;
        }
    };

    let script_content = match tokio::fs::read(&script_path).await {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to read setup script: {}", e);
            log::error!("{}", err_msg);
            let _ = emit_ipc(
                &app,
                IpcEvent::ModelProgress(ModelSetupStatus {
                    model_id: "chatterbox_remote_server".to_string(),
                    step: SetupStep::Failed,
                    progress: 0.0,
                    bytes_downloaded: 0,
                    total_bytes: 100,
                    error: Some(err_msg),
                }),
            );
            return;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let app_clone = app.clone();
        tokio::spawn(async move {
            if let Err(e) = stdin.write_all(&script_content).await {
                log::error!("[SetupRemote] Failed to write script to stdin: {}", e);
                let _ = emit_ipc(
                    &app_clone,
                    IpcEvent::ModelProgress(ModelSetupStatus {
                        model_id: "chatterbox_remote_server".to_string(),
                        step: SetupStep::Failed,
                        progress: 0.0,
                        bytes_downloaded: 0,
                        total_bytes: 100,
                        error: Some(format!("Failed to stream script: {}", e)),
                    }),
                );
            }
        });
    }

    let stdout_loop = {
        let stdout = child.stdout.take();
        let app_clone = app.clone();
        async move {
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log::info!("[SetupRemote:STDOUT] {}", line);
                    let (step, progress) = parse_setup_progress(&line);
                    let _ = emit_ipc(
                        &app_clone,
                        IpcEvent::ModelProgress(ModelSetupStatus {
                            model_id: "chatterbox_remote_server".to_string(),
                            step,
                            progress,
                            bytes_downloaded: progress as u64,
                            total_bytes: 100,
                            error: None,
                        }),
                    );
                }
            }
        }
    };

    let stderr_loop = {
        let stderr = child.stderr.take();
        let app_clone = app.clone();
        async move {
            if let Some(err) = stderr {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log::warn!("[SetupRemote:STDERR] {}", line);
                    let (step, progress) = parse_setup_progress(&line);
                    let _ = emit_ipc(
                        &app_clone,
                        IpcEvent::ModelProgress(ModelSetupStatus {
                            model_id: "chatterbox_remote_server".to_string(),
                            step,
                            progress,
                            bytes_downloaded: progress as u64,
                            total_bytes: 100,
                            error: None,
                        }),
                    );
                }
            }
        }
    };

    tokio::join!(stdout_loop, stderr_loop);

    match child.wait().await {
        Ok(status) if status.success() => {
            log::info!("[SetupRemote] Setup completed successfully.");
            let _ = emit_ipc(
                &app,
                IpcEvent::ModelProgress(ModelSetupStatus {
                    model_id: "chatterbox_remote_server".to_string(),
                    step: SetupStep::Completed,
                    progress: 100.0,
                    bytes_downloaded: 100,
                    total_bytes: 100,
                    error: None,
                }),
            );
        }
        Ok(status) => {
            let err_msg = format!("SSH command exited with code: {:?}", status.code());
            log::error!("[SetupRemote] {}", err_msg);
            let _ = emit_ipc(
                &app,
                IpcEvent::ModelProgress(ModelSetupStatus {
                    model_id: "chatterbox_remote_server".to_string(),
                    step: SetupStep::Failed,
                    progress: 0.0,
                    bytes_downloaded: 0,
                    total_bytes: 100,
                    error: Some(err_msg),
                }),
            );
        }
        Err(e) => {
            let err_msg = format!("Failed to wait for SSH child: {}", e);
            log::error!("[SetupRemote] {}", err_msg);
            let _ = emit_ipc(
                &app,
                IpcEvent::ModelProgress(ModelSetupStatus {
                    model_id: "chatterbox_remote_server".to_string(),
                    step: SetupStep::Failed,
                    progress: 0.0,
                    bytes_downloaded: 0,
                    total_bytes: 100,
                    error: Some(err_msg),
                }),
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
    if REMOTE_SETUP_RUNNING
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        return Err("Remote server setup is already in progress".to_string());
    }

    log::info!(
        "[SetupRemote] Triggering remote server setup. connection_string={}, remote_path={}, server_port={}",
        connection_string,
        remote_path,
        server_port
    );

    let script_path = match resolve_setup_script(&app) {
        Ok(p) => p,
        Err(e) => {
            REMOTE_SETUP_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
            return Err(e);
        }
    };

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
