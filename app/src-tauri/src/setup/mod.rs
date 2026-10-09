// ─── Setup Subsystem Constants ───────────────────────────────────────────────
pub const MODEL_DOWNLOAD_TIMEOUT_SECS: u64 = 300;
pub const MODEL_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const PROGRESS_EMIT_INTERVAL_MS: u64 = 150;
pub const MANIFEST_FETCH_TIMEOUT_SECS: u64 = 15;
pub const APP_MANIFEST_FETCH_TIMEOUT_SECS: u64 = 10;
pub const TRANSLIT_MODEL_DIR: &str = "translit";

pub const MODELS_MANIFEST_URL: &str =
    "https://huggingface.co/addyo07/vox-models/resolve/main/models_manifest.json";
pub const APP_MANIFEST_URL: &str = "https://addy-47.github.io/vox/manifests/app_manifest.json";

pub mod manager_ops;
pub mod manifest;
pub mod model_manager;
pub mod runtime_check;
pub mod update_check;

/// Remote GPU-box provisioning.
///
/// This spawns `ssh` to run `setup_server.sh` on a remote host and streams
/// setup progress back. `ssh` does not exist on stock Android, so the module is
/// desktop-only and `start_remote_setup` returns a typed error on mobile.
///
/// This is the *provisioning* path, not the inference path. Vox talks to an
/// already-provisioned box over plain HTTP via `services::llm::transport`,
/// which works on every platform. Provisioning is a desktop-admin task: set the
/// box up from a laptop, then point the phone at it.
///
/// See .agents/rules/android-pitfalls.md trap 7.
#[cfg(desktop)]
pub mod remote_server;

/// Mobile stub for [`remote_server`].
///
/// Declared inline rather than behind an `#[cfg]` at the call site so
/// `ipc/catalog.rs` stays free of platform conditionals — the same dual-arm
/// shape used by `ipc/tray.rs`.
#[cfg(not(desktop))]
pub mod remote_server {
    /// Always fails. Remote provisioning requires an `ssh` client, which stock
    /// Android does not ship.
    pub fn start_remote_setup<R: tauri::Runtime + 'static>(
        _app: tauri::AppHandle<R>,
        _connection_string: String,
        _ssh_port: Option<u16>,
        _identity_key_path: Option<String>,
        _remote_path: String,
        _server_port: u16,
    ) -> Result<(), String> {
        log::warn!(
            "[SetupRemote] Remote server provisioning is unavailable on this platform (no ssh client)."
        );
        Err("Remote server provisioning is not supported on this platform".to_string())
    }
}
