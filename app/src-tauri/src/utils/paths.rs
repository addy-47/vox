use std::{
    env::{current_dir, var},
    fs::{create_dir_all, read_dir, remove_dir_all, remove_file, rename},
    io::Result,
    path::{Path, PathBuf},
};

use parking_lot::RwLock;

/// Fully-resolved 6-tier filesystem layout for the Vox application.
#[derive(Clone)]
pub struct VoxPaths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub settings: PathBuf,
    pub providers: PathBuf,
    pub agent: PathBuf,
    pub data: PathBuf,
    pub data_db: PathBuf,
    pub db: PathBuf,
    pub data_history: PathBuf,
    pub dictation_history: PathBuf,
    pub voices: PathBuf,
    pub models: PathBuf,
    pub cache: PathBuf,
    pub temp: PathBuf,
    pub diagnostics: PathBuf,
    pub logs: PathBuf,
    pub crashes: PathBuf,
    pub run: PathBuf,
    pub socket: PathBuf,
    pub trigger_script: PathBuf,
    pub icon: PathBuf,
}

static PATHS: RwLock<Option<VoxPaths>> = RwLock::new(None);

pub const DB_FILENAME: &str = "vox.db";
pub const SETTINGS_FILENAME: &str = "settings.jsonc";
pub const PROVIDERS_FILENAME: &str = "providers.jsonc";
pub const AGENT_FILENAME: &str = "agent.jsonc";
pub const DICTATION_HISTORY_FILENAME: &str = "dictation.jsonl";
pub const ICON_FILENAME: &str = "vox.png";

/// Initialize the path singleton. Must be called ONCE at startup,
/// before any call to `paths::get()`.
pub fn init() {
    if PATHS.read().is_some() {
        return;
    }

    let root = if let Ok(env_path) = var("VOX_HOME") {
        PathBuf::from(env_path)
    } else {
        #[cfg(target_os = "linux")]
        {
            if let Some(home) = dirs::home_dir() {
                home.join(".vox")
            } else if let Some(data_dir) = dirs::data_local_dir() {
                data_dir.join("vox")
            } else {
                current_dir().unwrap_or_default().join(".vox")
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            if let Some(data_dir) = dirs::data_local_dir() {
                data_dir.join("vox")
            } else if let Some(home) = dirs::home_dir() {
                home.join(".vox")
            } else {
                current_dir().unwrap_or_default().join(".vox")
            }
        }
    };

    init_with_root(root);
}

/// Specialized initializer for testing or custom environments.
pub fn init_with_root(root: PathBuf) {
    let config = root.join("config");
    let settings = config.join(SETTINGS_FILENAME);
    let providers = config.join(PROVIDERS_FILENAME);
    let agent = config.join(AGENT_FILENAME);

    let data = root.join("data");
    let data_db = data.join("db");
    let db = data_db.join(DB_FILENAME);
    let data_history = data.join("history");
    let dictation_history = data_history.join(DICTATION_HISTORY_FILENAME);
    let voices = data.join("voices");

    let models = if let Ok(env_models) = var("VOX_MODELS_DIR") {
        PathBuf::from(env_models)
    } else {
        root.join("models")
    };

    let cache = root.join("cache");
    let temp = cache.join("temp");

    let diagnostics = root.join("diagnostics");
    let logs = diagnostics.join("logs");
    let crashes = diagnostics.join("crashes");

    let run = root.join("run");
    let socket = run.join("vox.sock");
    let trigger_script = run.join("bin").join("vox-trigger");
    let icon = cache.join(ICON_FILENAME);

    let paths = VoxPaths {
        root,
        config,
        settings,
        providers,
        agent,
        data,
        data_db,
        db,
        data_history,
        dictation_history,
        voices,
        models,
        cache,
        temp,
        diagnostics,
        logs,
        crashes,
        run,
        socket,
        trigger_script,
        icon,
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

/// Performs a one-way atomic migration of legacy root files into the 6-tier layout.
/// Enforces Zero Backward Compatibility (ZBC): moves files directly without symlinks.
pub fn migrate_legacy_layout(root: &Path) {
    let legacy_db = root.join(DB_FILENAME);
    let target_db_dir = root.join("data").join("db");
    let target_db = target_db_dir.join(DB_FILENAME);

    if legacy_db.exists() && !target_db.exists() && create_dir_all(&target_db_dir).is_ok() {
        let _ = rename(&legacy_db, &target_db);
        let legacy_wal = root.join("vox.db-wal");
        if legacy_wal.exists() {
            let _ = rename(&legacy_wal, target_db_dir.join("vox.db-wal"));
        }
        let legacy_log = root.join("vox.db-log");
        if legacy_log.exists() {
            let _ = rename(&legacy_log, target_db_dir.join("vox.db-log"));
        }
        log::info!("[Storage] Migrated legacy vox.db to data/db/vox.db");
    }

    // Debris sweep: clean up stale *.tmp and *.corrupt.* files in config/
    let target_config_dir = root.join("config");
    if let Ok(entries) = read_dir(&target_config_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
                if name.ends_with(".tmp") || name.contains(".corrupt.") {
                    let _ = remove_file(&path);
                    log::debug!("[Storage] Swept stale config debris: {:?}", path);
                }
            }
        }
    }

    let legacy_history = root.join("cache").join("dictation_history.jsonl");
    let target_history_dir = root.join("data").join("history");
    let target_history = target_history_dir.join(DICTATION_HISTORY_FILENAME);

    if legacy_history.exists() && create_dir_all(&target_history_dir).is_ok() {
        if !target_history.exists() {
            let _ = rename(&legacy_history, &target_history);
            log::info!("[Storage] Migrated dictation history to data/history/dictation.jsonl");
        } else {
            let _ = remove_file(&legacy_history);
        }
    }

    let legacy_crashes = root.join("crash_reports");
    let target_crashes = root.join("diagnostics").join("crashes");
    if legacy_crashes.exists() && create_dir_all(&target_crashes).is_ok() {
        if let Ok(entries) = read_dir(&legacy_crashes) {
            for entry in entries.flatten() {
                let dest = target_crashes.join(entry.file_name());
                let _ = rename(entry.path(), dest);
            }
        }
        let _ = remove_dir_all(&legacy_crashes);
        log::info!("[Storage] Migrated crash reports to diagnostics/crashes");
    }

    let legacy_logs = root.join("logs");
    let target_logs = root.join("diagnostics").join("logs");
    if legacy_logs.exists() && !target_logs.exists() {
        let _ = create_dir_all(root.join("diagnostics"));
        let _ = rename(&legacy_logs, &target_logs);
        log::info!("[Storage] Migrated logs to diagnostics/logs");
    }

    let legacy_bin = root.join("bin");
    let legacy_trigger = legacy_bin.join("vox-trigger");
    let target_run_bin = root.join("run").join("bin");
    let target_trigger = target_run_bin.join("vox-trigger");

    if legacy_trigger.exists() && create_dir_all(&target_run_bin).is_ok() {
        let _ = rename(&legacy_trigger, &target_trigger);
        let _ = remove_dir_all(&legacy_bin);
        log::info!("[Storage] Migrated vox-trigger to run/bin/vox-trigger");
    }

    let legacy_icons = root.join("icons");
    let legacy_icon_file = legacy_icons.join("vox.png");
    let target_icon = root.join("cache").join("vox.png");
    if legacy_icon_file.exists() && !target_icon.exists() {
        let _ = rename(&legacy_icon_file, &target_icon);
        let _ = remove_dir_all(&legacy_icons);
        log::info!("[Storage] Migrated vox.png to cache/vox.png");
    }
    let legacy_cache_icons = root.join("cache").join("icons");
    if legacy_cache_icons.exists() {
        let nested_icon = legacy_cache_icons.join("vox.png");
        if nested_icon.exists() && !target_icon.exists() {
            let _ = rename(&nested_icon, &target_icon);
        }
        let _ = remove_dir_all(&legacy_cache_icons);
    }

    let legacy_sock = root.join("vox.sock");
    if legacy_sock.exists() {
        let _ = remove_file(&legacy_sock);
    }

    let legacy_lib = root.join("lib");
    if legacy_lib.exists() {
        let _ = remove_dir_all(&legacy_lib);
        log::info!("[Storage] Cleaned abandoned legacy lib directory");
    }
}

/// Ensures all required directories exist on disk with appropriate permissions.
pub fn ensure_dirs() -> Result<()> {
    let p = get();
    create_dir_all(&p.root)?;
    create_dir_all(&p.config)?;
    create_dir_all(&p.data_db)?;
    create_dir_all(&p.data_history)?;
    create_dir_all(&p.voices)?;
    create_dir_all(&p.models)?;
    create_dir_all(&p.cache)?;
    create_dir_all(&p.temp)?;
    create_dir_all(&p.logs)?;
    create_dir_all(&p.crashes)?;
    create_dir_all(&p.run)?;
    create_dir_all(p.run.join("bin"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p.data_db, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::set_permissions(&p.run, std::fs::Permissions::from_mode(0o700));
    }

    #[cfg(target_os = "linux")]
    {
        if !p.icon.exists() {
            static VOX_ICON_BYTES: &[u8] = include_bytes!("../../icons/128x128.png");
            let _ = std::fs::write(&p.icon, VOX_ICON_BYTES);
        }
    }

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

pub fn providers_path() -> PathBuf {
    get().providers.clone()
}

pub fn agent_path() -> PathBuf {
    get().agent.clone()
}

pub fn cache_dir() -> PathBuf {
    get().cache.clone()
}

pub fn config_dir() -> PathBuf {
    get().config.clone()
}

pub fn data_dir() -> PathBuf {
    get().data.clone()
}

pub fn diagnostics_dir() -> PathBuf {
    get().diagnostics.clone()
}

pub fn logs_dir() -> PathBuf {
    get().logs.clone()
}

pub fn crashes_dir() -> PathBuf {
    get().crashes.clone()
}

pub fn run_dir() -> PathBuf {
    get().run.clone()
}

pub fn socket_path() -> PathBuf {
    get().socket.clone()
}

pub fn dictation_history_file() -> PathBuf {
    get().dictation_history.clone()
}

pub fn voices_dir() -> PathBuf {
    get().voices.clone()
}

/// Returns the absolute path to a specific model subdirectory.
pub fn model_dir(name: &str) -> PathBuf {
    get().models.join(name)
}

/// Returns the directory for a specific voice entry: `~/.vox/data/voices/{id}/`
pub fn voice_dir(id: &str) -> PathBuf {
    get().voices.join(id)
}

/// Returns the absolute path to the materialized application icon: `~/.vox/cache/vox.png`
pub fn icon_file() -> PathBuf {
    get().icon.clone()
}

/// Returns the path string to the application notification icon, falling back to a standard
/// desktop theme icon if uninitialized or unavailable.
pub fn notification_icon() -> String {
    if let Some(p) = try_get() {
        #[cfg(target_os = "linux")]
        {
            if !p.icon.exists() {
                static VOX_ICON_BYTES: &[u8] = include_bytes!("../../icons/128x128.png");
                let _ = std::fs::write(&p.icon, VOX_ICON_BYTES);
            }
        }
        if p.icon.exists() {
            return p.icon.to_string_lossy().to_string();
        }
    }
    "dialog-information".to_string()
}
