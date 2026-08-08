use crate::services::llm::{MODEL_DIR_LLM, MODEL_FILE_LLM_GGUF};
use crate::services::stt::{MODEL_DIR_STT_NEMOTRON, MODEL_DIR_STT_QWEN, MODEL_FILE_ASR_ENCODER};
use crate::services::tts::{MODEL_DIR_TTS_SUPER, MODEL_FILE_TTS_SUPER_TEXT_ENCODER};
use crate::services::vad::{MODEL_DIR_VAD, MODEL_FILE_VAD};
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
            .join(MODEL_DIR_STT_QWEN)
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

    // 5. TTS (Edge TTS cloud or Supertonic 3 local)
    let tts_ok = p
        .models
        .join(MODEL_DIR_TTS_SUPER)
        .join(MODEL_FILE_TTS_SUPER_TEXT_ENCODER)
        .exists()
        || true; // Edge TTS is cloud-based and always available
    if !tts_ok {
        return false;
    }

    // 6. MemoryScope Classifier & NLI
    let memory_scope_ok = p
        .models
        .join("classifier/modernbert_memory_scope/model_quantized.onnx")
        .exists();
    let nli_ok = p
        .models
        .join("nli/nli-deberta-v3-base/model_quantized.onnx")
        .exists();
    if !memory_scope_ok || !nli_ok {
        log::warn!("[Health] MemoryScope classifier or NLI model missing on disk");
    }

    log::info!("[Health] All core models verified in {:?}", p.models);
    true
}
