use std::path::PathBuf;

use parking_lot::RwLock;

use crate::core::constants::*;

/// Fully-resolved filesystem layout for the Vox application.
#[derive(Clone)]
pub struct VoxPaths {
    pub root: PathBuf,
    pub models: PathBuf,
    pub logs: PathBuf,
    pub db: PathBuf,
    pub settings: PathBuf,
    pub cache: PathBuf,
    pub temp: PathBuf,
    pub voices: PathBuf,
}

static PATHS: RwLock<Option<VoxPaths>> = RwLock::new(None);

/// Initialize the path singleton. Must be called ONCE at startup,
/// before any call to `paths::get()`.
pub fn init() {
    if PATHS.read().is_some() {
        return;
    }

    let root = if let Ok(env_path) = std::env::var("VOX_HOME") {
        PathBuf::from(env_path)
    } else {
        #[cfg(target_os = "linux")]
        {
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

    let mut lock = PATHS.write();
    *lock = Some(paths);
}

/// Returns a clone of the initialized `VoxPaths` singleton if initialized, or None.
pub fn try_get() -> Option<VoxPaths> {
    PATHS.read().clone()
}

/// Returns a clone of the initialized `VoxPaths` singleton.
pub fn get() -> VoxPaths {
    PATHS.read().clone().expect(
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
pub fn model_dir(name: &str) -> PathBuf {
    get().models.join(name)
}

/// Returns the directory for a specific voice entry: `~/.vox/voices/{id}/`
pub fn voice_dir(id: &str) -> PathBuf {
    get().voices.join(id)
}
