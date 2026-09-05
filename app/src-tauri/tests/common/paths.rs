//! ============================================================================
//! tests/common/paths.rs — Path Resolution Helpers for Integration Tests
//! ============================================================================

use std::path::PathBuf;
use vox_lib::services::llm::{GEMMA_MODEL_DIR, GEMMA_MODEL_FILE, QWEN_MODEL_DIR, QWEN_MODEL_FILE};
use vox_lib::services::stt::{NEMOTRON_MODEL_DIR, QWEN_ASR_MODEL_DIR};
use vox_lib::services::tts::{CHATTERBOX_MODEL_DIR, SUPERTONIC_MODEL_DIR};
use vox_lib::services::vad::{MODEL_DIR_VAD, MODEL_FILE_VAD};

/// Resolves path to a test asset in `tests/assets/` directory.
/// Self-containment rule: integration tests may ONLY use clips shipped in
/// `tests/assets/` (the 4 golden clips). No fallback outside `tests/` —
/// unknown files fail loudly here instead of silently resolving elsewhere.
pub fn get_asset_path(filename: &str) -> PathBuf {
    let candidates = [
        PathBuf::from("tests/assets").join(filename),
        PathBuf::from("app/src-tauri/tests/assets").join(filename),
        PathBuf::from("../tests/assets").join(filename),
    ];
    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }
    panic!(
        "Test asset '{}' not found in candidate paths: {:?}",
        filename, candidates
    );
}

/// Resolves the local Nemotron STT model directory.
pub fn get_nemotron_model_dir() -> PathBuf {
    vox_lib::utils::paths::init();
    let models_dir = vox_lib::utils::paths::get().models.clone();
    let nemotron_dir = models_dir.join(NEMOTRON_MODEL_DIR);
    assert!(
        nemotron_dir.exists(),
        "Nemotron model directory does not exist at {:?}",
        nemotron_dir
    );
    nemotron_dir
}

/// Resolves the local Qwen ASR model directory.
pub fn get_qwen_asr_model_dir() -> PathBuf {
    vox_lib::utils::paths::init();
    let models_dir = vox_lib::utils::paths::get().models.clone();
    models_dir.join(QWEN_ASR_MODEL_DIR)
}

/// Resolves the local Supertonic TTS model directory.
pub fn get_supertonic_model_dir() -> PathBuf {
    vox_lib::utils::paths::init();
    let models_dir = vox_lib::utils::paths::get().models.clone();
    let supertonic_dir = models_dir.join(SUPERTONIC_MODEL_DIR);
    assert!(
        supertonic_dir.exists(),
        "Supertonic model directory does not exist at {:?}",
        supertonic_dir
    );
    supertonic_dir
}

/// Resolves the local Chatterbox TTS model directory.
pub fn get_chatterbox_model_dir() -> PathBuf {
    vox_lib::utils::paths::init();
    let models_dir = vox_lib::utils::paths::get().models.clone();
    models_dir.join(CHATTERBOX_MODEL_DIR)
}

/// Resolves the local Ten VAD model file path.
pub fn get_ten_vad_model_path() -> PathBuf {
    vox_lib::utils::paths::init();
    let models_dir = vox_lib::utils::paths::get().models.clone();
    models_dir.join(MODEL_DIR_VAD).join(MODEL_FILE_VAD)
}

/// Resolves the local Qwen LLM model path.
pub fn get_qwen_model_path() -> PathBuf {
    vox_lib::utils::paths::init();
    let models_dir = vox_lib::utils::paths::get().models.clone();
    models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE)
}

/// Resolves the local Gemma LLM model path.
pub fn get_gemma_model_path() -> PathBuf {
    vox_lib::utils::paths::init();
    let models_dir = vox_lib::utils::paths::get().models.clone();
    models_dir.join(GEMMA_MODEL_DIR).join(GEMMA_MODEL_FILE)
}

/// RAII guard to initialize VoxPaths with a temporary root directory and isolated test database.
pub struct TempPathsGuard {
    _dir: tempfile::TempDir,
    prev_vox_home: Option<String>,
}

impl TempPathsGuard {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("Failed to create temporary directory for test");
        let temp_path = dir.path().to_path_buf();

        // Seed test database from tests/assets/test_vox.db if available
        let asset_db = get_asset_path("test_vox.db");
        if asset_db.exists() {
            let target_db = temp_path.join(vox_lib::core::constants::DB_FILENAME);
            let _ = std::fs::copy(&asset_db, &target_db);
        }

        let prev_vox_home = std::env::var("VOX_HOME").ok();
        std::env::set_var("VOX_HOME", &temp_path);
        vox_lib::utils::paths::init_with_root(temp_path);

        Self {
            _dir: dir,
            prev_vox_home,
        }
    }
}

impl Drop for TempPathsGuard {
    fn drop(&mut self) {
        if let Some(ref prev) = self.prev_vox_home {
            std::env::set_var("VOX_HOME", prev);
            vox_lib::utils::paths::init_with_root(std::path::PathBuf::from(prev));
        } else {
            std::env::remove_var("VOX_HOME");
            // Reset back to user default vox home
            if let Some(home) = dirs::home_dir() {
                vox_lib::utils::paths::init_with_root(home.join(".vox"));
            }
        }
    }
}
