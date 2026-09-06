use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{
    services::{
        llm::{QWEN_MODEL_DIR, QWEN_MODEL_FILE},
        stt::{MODEL_FILE_ASR_ENCODER, NEMOTRON_MODEL_DIR, QWEN_ASR_MODEL_DIR},
        vad::{MODEL_DIR_VAD, MODEL_FILE_VAD},
    },
    utils::paths,
};

/// Lazily constructs the wizard setup window on-demand.
pub fn ensure_wizard_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window("wizard") {
        return Ok(existing);
    }

    log::info!("[Wizard] Lazily constructing 'wizard' setup webview window...");
    let window = WebviewWindowBuilder::new(app, "wizard", WebviewUrl::App("/wizard".into()))
        .title("Vox Setup Wizard")
        .inner_size(900.0, 650.0)
        .min_inner_size(900.0, 650.0)
        .max_inner_size(900.0, 650.0)
        .transparent(false)
        .decorations(false)
        .always_on_top(false)
        .resizable(true)
        .visible(false)
        .center()
        .build()
        .map_err(|e| format!("Failed to create wizard window: {}", e))?;

    Ok(window)
}

/// Checks if all required models for a functional Vox experience are present on disk.
pub fn check_setup_health() -> bool {
    let p = paths::get();

    let vad_ok = p.models.join(MODEL_DIR_VAD).join(MODEL_FILE_VAD).exists();
    if !vad_ok {
        return false;
    }

    let stt_ok = p
        .models
        .join(NEMOTRON_MODEL_DIR)
        .join(MODEL_FILE_ASR_ENCODER)
        .exists()
        || p.models
            .join(QWEN_ASR_MODEL_DIR)
            .join(MODEL_FILE_ASR_ENCODER)
            .exists();
    if !stt_ok {
        return false;
    }

    if !p.models.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE).exists() {
        return false;
    }

    let tts_ok = true;
    if !tts_ok {
        return false;
    }

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
