use crate::core::constants::*;
use crate::utils::paths;

/// Configures the wizard window with specific attributes.
pub fn setup_wizard_window(window: &tauri::WebviewWindow) {
    let _ = window.set_min_size(Some(tauri::LogicalSize::new(900.0, 650.0)));
    let _ = window.set_max_size(Some(tauri::LogicalSize::new(900.0, 650.0)));
    let _ = window.set_resizable(true);
    let _ = window.set_decorations(false);
    let _ = window.set_always_on_top(false);
    let _ = window.center();
}

/// Checks if all required models for a functional Vox experience are present on disk.
pub fn check_setup_health() -> bool {
    let p = paths::get();

    // 1. Manifest
    if !p.models.join("models_manifest.json").exists() {
        return false;
    }

    // 2. VAD
    if !p.models.join(MODEL_DIR_VAD).join(MODEL_FILE_VAD).exists() {
        return false;
    }

    // 3. STT — check whichever model is configured (Nemotron or Qwen)
    let stt_ok = p
        .models
        .join(MODEL_DIR_STT_NEMOTRON)
        .join(MODEL_FILE_ASR_ENCODER)
        .exists()
        || p.models
            .join(MODEL_DIR_STT)
            .join(MODEL_FILE_ASR_ENCODER)
            .exists();
    if !stt_ok {
        return false;
    }

    // 4. LLM
    if !p
        .models
        .join(MODEL_DIR_LLM)
        .join(MODEL_FILE_LLM_GGUF)
        .exists()
    {
        return false;
    }

    // 5. TTS (Supertonic)
    if !p
        .models
        .join(MODEL_DIR_TTS_SUPER)
        .join(MODEL_FILE_TTS_SUPER_TEXT_ENCODER)
        .exists()
    {
        return false;
    }

    log::info!("[Health] All core models verified in {:?}", p.models);
    true
}
