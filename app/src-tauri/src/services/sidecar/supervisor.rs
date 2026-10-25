use std::{
    collections::HashMap,
    path::PathBuf,
    sync::OnceLock,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use tokio::process::{Child, Command};

use super::{download::ensure_sidecar_binary, types::SidecarKind};
use crate::utils::paths::get as get_paths;

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(100);
const IDLE_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

struct ActiveInstance {
    child: Child,
    port: u16,
    socket_path: Option<PathBuf>,
    last_accessed: Instant,
}

pub struct SidecarSupervisor {
    instances: Mutex<HashMap<SidecarKind, ActiveInstance>>,
}

impl Default for SidecarSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl SidecarSupervisor {
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(HashMap::new()),
        }
    }

    /// Finds a free TCP port by binding to 127.0.0.1:0 and immediately closing it.
    fn get_free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .unwrap_or_else(|_| 18090)
    }

    /// Ensures the sidecar of the requested kind is running and healthy.
    /// Returns the base HTTP URL (e.g., `http://127.0.0.1:18081`).
    pub async fn ensure_ready(
        &self,
        kind: SidecarKind,
        extra_args: &[(&str, &str)],
    ) -> Result<String> {
        // 1. Check if already running and healthy
        {
            let mut guard = self.instances.lock();
            if let Some(inst) = guard.get_mut(&kind) {
                // Check if child process has exited
                if let Ok(None) = inst.child.try_wait() {
                    inst.last_accessed = Instant::now();
                    return Ok(format!("http://127.0.0.1:{}", inst.port));
                }
            }
        }

        // 2. Ensure binary exists
        let bin_path = ensure_sidecar_binary(kind).await?;
        let port = Self::get_free_port();

        // Socket path in ~/.vox/run/<name>.sock
        let run_dir = get_paths().root.join("run");
        std::fs::create_dir_all(&run_dir)?;
        let socket_path = run_dir.join(format!("{}.sock", kind.binary_name()));

        let mut cmd = Command::new(&bin_path);
        cmd.arg("--port").arg(port.to_string());

        #[cfg(unix)]
        cmd.arg("--socket").arg(&socket_path);

        for (k, v) in extra_args {
            cmd.arg(k).arg(v);
        }

        // Set low process priority
        #[cfg(all(unix, not(target_os = "android")))]
        unsafe {
            cmd.pre_exec(|| {
                libc::nice(10);
                Ok(())
            });
        }

        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::inherit());

        log::info!(
            "[Sidecar] Spawning {} on port {} (binary: {:?})",
            kind.binary_name(),
            port,
            bin_path
        );

        let child = cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn {}: {}", kind.binary_name(), e))?;

        let base_url = format!("http://127.0.0.1:{}", port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;

        // 3. Poll health check
        let start = Instant::now();
        let health_url = format!("{}/health", base_url);
        let mut ready = false;

        while start.elapsed() < HEALTH_CHECK_TIMEOUT {
            if let Ok(resp) = client.get(&health_url).send().await {
                if resp.status().is_success() {
                    ready = true;
                    break;
                }
            }
            tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
        }

        if !ready {
            log::warn!(
                "[Sidecar] {} health check timed out after 5s. Proceeding anyway.",
                kind.binary_name()
            );
        }

        let instance = ActiveInstance {
            child,
            port,
            socket_path: Some(socket_path),
            last_accessed: Instant::now(),
        };

        self.instances.lock().insert(kind, instance);
        Ok(base_url)
    }

    /// Marks an instance as active right now.
    pub fn touch(&self, kind: SidecarKind) {
        let mut guard = self.instances.lock();
        if let Some(inst) = guard.get_mut(&kind) {
            inst.last_accessed = Instant::now();
        }
    }

    /// Reaps processes that have been idle for more than 5 minutes.
    pub async fn reap_idle(&self) {
        let to_kill = {
            let mut guard = self.instances.lock();
            let idle_keys: Vec<_> = guard
                .iter()
                .filter(|(_, inst)| inst.last_accessed.elapsed() > IDLE_TIMEOUT)
                .map(|(k, _)| *k)
                .collect();
            let mut removed = Vec::new();
            for k in idle_keys {
                if let Some(inst) = guard.remove(&k) {
                    removed.push((k, inst));
                }
            }
            removed
        };

        for (kind, mut inst) in to_kill {
            log::info!("[Sidecar] Reaping idle sidecar {}", kind.binary_name());
            let _ = inst.child.kill().await;
            if let Some(p) = inst.socket_path {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    /// Kills all child processes cleanly on shutdown.
    pub fn shutdown_all(&self) {
        let mut guard = self.instances.lock();
        for (kind, mut inst) in guard.drain() {
            log::info!("[Sidecar] Shutting down sidecar {}", kind.binary_name());
            let _ = inst.child.start_kill();
            if let Some(p) = inst.socket_path {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

pub fn global_sidecar_supervisor() -> &'static SidecarSupervisor {
    static SUPERVISOR: OnceLock<SidecarSupervisor> = OnceLock::new();
    SUPERVISOR.get_or_init(SidecarSupervisor::new)
}
