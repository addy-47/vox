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
    endpoint: SidecarEndpoint,
    last_accessed: Instant,
}

/// Endpoint for a running sidecar (sidecar-spec §6.2).
/// On Unix, communication goes over the Unix Domain Socket per the spec
/// (`reqwest` Unix socket transport). Elsewhere, over a loopback TCP port.
#[derive(Clone)]
pub struct SidecarEndpoint {
    base_url: String,
    #[cfg_attr(not(unix), allow(dead_code))]
    socket_path: Option<PathBuf>,
}

impl SidecarEndpoint {
    /// URL for `path`, e.g. `/health`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Builds a reqwest client that reaches the sidecar over its transport.
    pub fn client(&self, timeout: Option<Duration>) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder();
        if let Some(t) = timeout {
            builder = builder.timeout(t);
        }
        #[cfg(unix)]
        if let Some(path) = &self.socket_path {
            builder = builder.unix_socket(path.as_path());
        }
        let _ = &self.socket_path;
        builder.build()
    }
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

    /// Finds a free TCP port by binding to 127.0.0.1:0 and binding it again on first use.
    #[cfg(not(unix))]
    fn get_free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .unwrap_or_else(|_| 18090)
    }

    /// Builds the endpoint for the cached or freshly spawned instance.
    #[cfg(unix)]
    fn socket_endpoint(socket_path: &std::path::Path) -> SidecarEndpoint {
        SidecarEndpoint {
            // The host is placeholder metadata: with a UDS transport the actual
            // connection goes over the socket path, not this URL.
            base_url: "http://localhost".to_string(),
            socket_path: Some(socket_path.to_path_buf()),
        }
    }

    #[cfg(not(unix))]
    fn tcp_endpoint(port: u16) -> SidecarEndpoint {
        SidecarEndpoint {
            base_url: format!("http://127.0.0.1:{}", port),
            socket_path: None,
        }
    }

    /// Ensures the sidecar of the requested kind is running and healthy.
    /// Per RULE-SIDECAR-07 this fails once the health gate cannot be
    /// satisfied; it never returns an unreachable endpoint.
    pub async fn ensure_ready(
        &self,
        kind: SidecarKind,
        extra_args: &[(&str, &str)],
    ) -> Result<SidecarEndpoint> {
        // 1. Check if already running and healthy
        {
            let mut guard = self.instances.lock();
            if let Some(inst) = guard.get_mut(&kind) {
                // Check if child process has exited
                if let Ok(None) = inst.child.try_wait() {
                    inst.last_accessed = Instant::now();
                    return Ok(inst.endpoint.clone());
                }
            }
        }

        // 2. Ensure binary exists
        let bin_path = ensure_sidecar_binary(kind).await?;

        let run_dir = get_paths().root.join("run");
        std::fs::create_dir_all(&run_dir)?;
        let socket_path = run_dir.join(format!("{}.sock", kind.binary_name()));

        #[cfg(not(unix))]
        let port = Self::get_free_port();

        let mut cmd = Command::new(&bin_path);
        // Always pass the socket path (spec: `--socket ~/.vox/run/{name}.sock`).
        #[cfg(unix)]
        {
            cmd.arg("--socket").arg(&socket_path);
        }
        #[cfg(not(unix))]
        {
            cmd.arg("--port").arg(port.to_string());
        }

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

        #[cfg(unix)]
        let endpoint = Self::socket_endpoint(&socket_path);
        #[cfg(not(unix))]
        let endpoint = Self::tcp_endpoint(port);

        log::info!(
            "[Sidecar] Spawning {} (binary: {:?})",
            kind.binary_name(),
            bin_path
        );

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn {}: {}", kind.binary_name(), e))?;

        let client = endpoint.client(Some(Duration::from_millis(500)))?;

        // 3. Poll health check
        let start = Instant::now();
        let health_url = endpoint.url("/health");
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
            let _ = child.start_kill();
            anyhow::bail!(
                "[Sidecar] {} did not become healthy within {}s; refusing to route traffic (RULE-SIDECAR-07)",
                kind.binary_name(),
                HEALTH_CHECK_TIMEOUT.as_secs()
            );
        }

        let instance = ActiveInstance {
            child,
            endpoint: endpoint.clone(),
            last_accessed: Instant::now(),
        };

        self.instances.lock().insert(kind, instance);
        Ok(endpoint)
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
            #[cfg(unix)]
            if let Some(p) = inst.endpoint.socket_path {
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
            #[cfg(unix)]
            if let Some(p) = inst.endpoint.socket_path {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

pub fn global_sidecar_supervisor() -> &'static SidecarSupervisor {
    static SUPERVISOR: OnceLock<SidecarSupervisor> = OnceLock::new();
    SUPERVISOR.get_or_init(SidecarSupervisor::new)
}
