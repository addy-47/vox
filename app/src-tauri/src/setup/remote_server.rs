use crate::core::events::{emit_ipc, IpcEvent};
use crate::setup::model_manager::{ModelSetupStatus, SetupStep};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

static REMOTE_SETUP_RUNNING: AtomicBool = AtomicBool::new(false);

struct RemoteSetupGuard;

impl Drop for RemoteSetupGuard {
    fn drop(&mut self) {
        REMOTE_SETUP_RUNNING.store(false, Ordering::SeqCst);
    }
}

pub fn resolve_setup_script(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to resolve resource dir: {}", e))?
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

pub fn parse_setup_progress(line: &str) -> (SetupStep, f32) {
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

pub async fn run_remote_ssh_task(
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

pub fn start_remote_setup(
    app: tauri::AppHandle,
    connection_string: String,
    ssh_port: Option<u16>,
    identity_key_path: Option<String>,
    remote_path: String,
    server_port: u16,
) -> Result<(), String> {
    if REMOTE_SETUP_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
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
            REMOTE_SETUP_RUNNING.store(false, Ordering::SeqCst);
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
