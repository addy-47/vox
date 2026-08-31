//! ============================================================================
//! tests/common/paths.rs — Path Resolution Helpers for Integration Tests
//! ============================================================================

use std::path::PathBuf;
use vox_lib::services::llm::{GEMMA_MODEL_DIR, GEMMA_MODEL_FILE, QWEN_MODEL_DIR, QWEN_MODEL_FILE};
use vox_lib::services::stt::{NEMOTRON_MODEL_DIR, QWEN_ASR_MODEL_DIR};
use vox_lib::services::tts::{CHATTERBOX_MODEL_DIR, SUPERTONIC_MODEL_DIR};
use vox_lib::services::vad::{MODEL_DIR_VAD, MODEL_FILE_VAD};

/// Resolves path to a test asset in `tests/assets/` directory.
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
