use std::path::PathBuf;
use std::sync::OnceLock;
use crate::core::constants::*;

// ─── Path Singleton ───────────────────────────────────────────────────────────

/// Fully-resolved filesystem layout for the Vox application.
///
/// Initialized ONCE at startup via `paths::init(app_handle)`.
/// After init, `paths::get()` is safe to call from any thread without locks.
pub struct VoxPaths {
    /// ~/.vox/ (base) — or Tauri's app_data_dir fallback
    pub root:     PathBuf,
    /// ~/.vox/models/
    pub models:   PathBuf,
    /// ~/.vox/logs/
    pub logs:     PathBuf,
    /// ~/.vox/vox.db
    pub db:       PathBuf,
    /// ~/.vox/settings.json
    pub settings: PathBuf,
}

static PATHS: OnceLock<VoxPaths> = OnceLock::new();

/// Initialize the path singleton. Must be called ONCE at startup with the AppHandle,
/// before any call to `paths::get()`.
///
/// Uses `app.path().app_data_dir()` for platform-correct resolution (respects macOS
/// sandboxing, Windows %APPDATA%, and Linux XDG). Falls back to `$HOME/.vox` on failure.
pub fn init(_app: &tauri::AppHandle) {
    // User Directive: strictly use ~/.vox for all model and config persistence.
    // This overrides Tauri's default app_data_dir() behavior on Linux.
    let root = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".vox"))
        .unwrap_or_else(|_| {
            std::env::current_dir().unwrap_or_default().join(".vox")
        });

    let paths = VoxPaths {
        models:   root.join(MODELS_DIRNAME),
        logs:     root.join(LOG_DIRNAME),
        db:       root.join(DB_FILENAME),
        settings: root.join(SETTINGS_FILENAME),
        root,
    };

    // Ignore error if already initialized (idempotent in tests)
    let _ = PATHS.set(paths);
}

/// Returns the initialized `VoxPaths` singleton.
///
/// # Panics
/// Panics if `paths::init()` was not called before this. This is intentional —
/// it enforces correct startup ordering.
pub fn get() -> &'static VoxPaths {
    PATHS.get().expect("[FATAL] paths::init() was not called before paths::get(). Check app startup order.")
}

/// Ensures all required directories exist on disk. Called once at startup.
pub fn ensure_dirs() -> std::io::Result<()> {
    let p = get();
    std::fs::create_dir_all(&p.root)?;
    std::fs::create_dir_all(&p.models)?;
    std::fs::create_dir_all(&p.logs)?;
    Ok(())
}

/// Returns the absolute path to a specific model subdirectory.
/// e.g. `model_dir("kokoro")` → `~/.vox/models/kokoro/`
pub fn model_dir(name: &str) -> PathBuf {
    get().models.join(name)
}
