use crate::utils::paths;
use crate::core::constants::*;

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

    // 3. STT (Encoder is the largest, good enough indicator)
    if !p.models.join(MODEL_DIR_STT).join(MODEL_FILE_ASR_ENCODER).exists() { 
        return false; 
    }

    // 4. LLM
    if !p.models.join(MODEL_DIR_LLM).join(MODEL_FILE_LLM_GGUF).exists() { 
        return false; 
    }

    // 5. TTS (EN)
    if !p.models.join(MODEL_DIR_TTS_EN).join(MODEL_FILE_TTS_ONNX).exists() { 
        return false; 
    }

    log::info!("[Health] All core models verified in {:?}", p.models);
    true
}
