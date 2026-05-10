use std::path::PathBuf;
use crate::core::constants::*;

/// Returns the base directory for Vox data: ~/.vox/
pub fn vox_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let mut p = PathBuf::from(home);
            p.push(".vox");
            return p;
        }
    }

    // Fallback or non-linux (though project is linux-focused)
    let mut p = std::env::current_dir().unwrap_or_default();
    p.push(".vox");
    p
}

/// Returns the directory where models are stored: ~/.vox/models/
pub fn models_dir() -> PathBuf {
    vox_dir().join(MODELS_DIRNAME)
}

/// Returns the directory where logs are stored: ~/.vox/logs/
pub fn logs_dir() -> PathBuf {
    vox_dir().join(LOG_DIRNAME)
}

/// Returns the path to the SQLite database: ~/.vox/vox.db
pub fn db_path() -> PathBuf {
    vox_dir().join(DB_FILENAME)
}

/// Returns the path to the settings file: ~/.vox/settings.json
pub fn settings_path() -> PathBuf {
    vox_dir().join(SETTINGS_FILENAME)
}

/// Ensures all required directories exist.
pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(vox_dir())?;
    std::fs::create_dir_all(models_dir())?;
    std::fs::create_dir_all(logs_dir())?;
    Ok(())
}
