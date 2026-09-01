// ─── Setup Subsystem Constants ───────────────────────────────────────────────
pub const MODEL_DOWNLOAD_TIMEOUT_SECS: u64 = 300;
pub const MODEL_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const PROGRESS_EMIT_INTERVAL_MS: u64 = 150;
pub const MANIFEST_FETCH_TIMEOUT_SECS: u64 = 15;
pub const APP_MANIFEST_FETCH_TIMEOUT_SECS: u64 = 10;
pub const TRANSLIT_MODEL_DIR: &str = "translit";

pub const MODELS_MANIFEST_URL: &str =
    "https://huggingface.co/addyo07/vox-models/resolve/main/models_manifest.json";
pub const APP_MANIFEST_URL: &str =
    "https://addy-47.github.io/vox/manifests/app_manifest.json";

pub mod manager_ops;
pub mod manifest;
pub mod model_manager;
pub mod remote_server;
pub mod runtime_check;
pub mod update_check;
