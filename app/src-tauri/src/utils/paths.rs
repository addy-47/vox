use crate::core::constants::*;
use std::path::PathBuf;
use std::sync::OnceLock;

// ─── Path Singleton ───────────────────────────────────────────────────────────

/// Fully-resolved filesystem layout for the Vox application.
///
/// Initialized ONCE at startup via `paths::init(app_handle)`.
/// After init, `paths::get()` is safe to call from any thread without locks.
pub struct VoxPaths {
    /// Base directory (~/.vox/ or platform-specific data dir)
    pub root: PathBuf,
    /// models/
    pub models: PathBuf,
    /// logs/
    pub logs: PathBuf,
    /// vox.db
    pub db: PathBuf,
    /// settings.json
    pub settings: PathBuf,
    /// cache/
    pub cache: PathBuf,
    /// temp/
    pub temp: PathBuf,
    /// voices/ — cloned voice WAVs and pre-baked tensors
    pub voices: PathBuf,
}

static PATHS: OnceLock<VoxPaths> = OnceLock::new();

/// Initialize the path singleton. Must be called ONCE at startup,
/// before any call to `paths::get()`.
///
/// Priority:
/// 1. `VOX_HOME` environment variable (for testing/hardening)
/// 2. Platform-specific local data dir (e.g. ~/.local/share/vox)
/// 3. Fallback to `$HOME/.vox`
pub fn init() {
    let root = if let Ok(env_path) = std::env::var("VOX_HOME") {
        PathBuf::from(env_path)
    } else {
        #[cfg(target_os = "linux")]
        {
            // On Linux, prioritize ~/.vox for visibility/accessibility like a dev tool
            if let Some(home) = dirs::home_dir() {
                home.join(".vox")
            } else if let Some(data_dir) = dirs::data_local_dir() {
                data_dir.join("vox")
            } else {
                std::env::current_dir().unwrap_or_default().join(".vox")
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // On Windows/Mac, follow standard app data conventions
            if let Some(data_dir) = dirs::data_local_dir() {
                data_dir.join("vox")
            } else if let Some(home) = dirs::home_dir() {
                home.join(".vox")
            } else {
                std::env::current_dir().unwrap_or_default().join(".vox")
            }
        }
    };

    init_with_root(root);
}

/// Specialized initializer for testing or custom environments.
pub fn init_with_root(root: PathBuf) {
    let paths = VoxPaths {
        models: root.join(MODELS_DIRNAME),
        logs: root.join(LOG_DIRNAME),
        db: root.join(DB_FILENAME),
        settings: root.join(SETTINGS_FILENAME),
        cache: root.join("cache"),
        temp: root.join("temp"),
        voices: root.join("voices"),
        root,
    };

    if PATHS.set(paths).is_err() {
        log::debug!(
            "[Paths] VoxPaths singleton was already initialized; skipping re-initialization."
        );
    }
}

/// Returns the initialized `VoxPaths` singleton if initialized, or None.
pub fn try_get() -> Option<&'static VoxPaths> {
    PATHS.get()
}

/// Returns the initialized `VoxPaths` singleton.
///
/// # Panics
/// Panics if `paths::init()` was not called before this.
pub fn get() -> &'static VoxPaths {
    PATHS.get().expect(
        "[FATAL] paths::init() was not called before paths::get(). Check app startup order.",
    )
}

/// Ensures all required directories exist on disk. Called once at startup.
pub fn ensure_dirs() -> std::io::Result<()> {
    let p = get();
    std::fs::create_dir_all(&p.root)?;
    std::fs::create_dir_all(&p.models)?;
    std::fs::create_dir_all(&p.logs)?;
    std::fs::create_dir_all(&p.cache)?;
    std::fs::create_dir_all(&p.temp)?;
    std::fs::create_dir_all(&p.voices)?;
    Ok(())
}

// ─── Required API ────────────────────────────────────────────────────────────

pub fn models_dir() -> PathBuf {
    get().models.clone()
}

pub fn db_path() -> PathBuf {
    get().db.clone()
}

pub fn settings_path() -> PathBuf {
    get().settings.clone()
}

pub fn cache_dir() -> PathBuf {
    get().cache.clone()
}

/// Returns the absolute path to a specific model subdirectory.
/// e.g. `model_dir("kokoro")` → `~/.vox/models/kokoro/`
pub fn model_dir(name: &str) -> PathBuf {
    get().models.join(name)
}

/// Returns the directory for a specific voice entry: `~/.vox/voices/{id}/`
pub fn voice_dir(id: &str) -> PathBuf {
    get().voices.join(id)
}
